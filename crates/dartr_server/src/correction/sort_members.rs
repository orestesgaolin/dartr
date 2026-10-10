// Dart source: pkg/analysis_server/lib/src/services/correction/sort_members.dart (MemberSorter)
// Dart source: pkg/analysis_server/lib/src/utilities/strings.dart (computeSimpleDiff, findCommonPrefix)

//! Dart `MemberSorter`: sorts the members of the classes and of the unit,
//! then the directives (on a parsed unit).

use std::cmp::Ordering;

use dartr_ast::*;
use dartr_syntax::LineInfo;

use super::change::SourceEdit;
use super::imports::dart_compare;
use super::organize_imports::{ImportOrganizer, find_common_suffix};
use super::utils::RangeFactory;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum MemberKind {
    ClassAccessor,
    ClassConstructor,
    ClassField,
    ClassMethod,
    PrimaryConstructorBody,
    UnitAccessor,
    UnitClass,
    UnitExtension,
    UnitExtensionType,
    UnitFunction,
    UnitFunctionMain,
    UnitFunctionType,
    UnitGenericTypeAlias,
    UnitVariable,
    UnitVariableConst,
}

#[derive(Clone, Copy, Debug)]
struct PriorityItem {
    kind: MemberKind,
    is_private: bool,
    is_static: bool,
}

impl PriorityItem {
    fn new(is_static: bool, kind: MemberKind, is_private: bool) -> Self {
        PriorityItem {
            kind,
            is_private,
            is_static,
        }
    }

    fn for_name(is_static: bool, name: &str, kind: MemberKind) -> Self {
        PriorityItem::new(is_static, kind, name.starts_with('_'))
    }

    /// Dart `==`.
    fn equals(&self, other: &PriorityItem) -> bool {
        if self.kind == MemberKind::ClassField {
            return other.kind == self.kind && other.is_static == self.is_static;
        }
        other.kind == self.kind
            && other.is_private == self.is_private
            && other.is_static == self.is_static
    }
}

#[derive(Clone, Debug)]
struct MemberInfo {
    item: PriorityItem,
    name: String,
    offset: u32,
    end: u32,
    text: Vec<u16>,
}

/// Dart `computeSimpleDiff`: (offset, length, replacement).
pub fn compute_simple_diff(old: &[u16], new: &[u16]) -> (usize, usize, Vec<u16>) {
    let n = old.len().min(new.len());
    let mut prefix = 0;
    while prefix < n && old[prefix] == new[prefix] {
        prefix += 1;
    }
    let suffix = find_common_suffix(old, new);
    let mut prefix = prefix as i64;
    while prefix >= 0 {
        let old_len = old.len() as i64 - prefix - suffix as i64;
        let new_len = new.len() as i64 - prefix - suffix as i64;
        if old_len >= 0 && new_len >= 0 {
            let p = prefix as usize;
            return (p, old_len as usize, new[p..new.len() - suffix].to_vec());
        }
        prefix -= 1;
    }
    (0, old.len(), new.to_vec())
}

/// Dart `MemberSorter`.
pub struct MemberSorter<'a> {
    ast: &'a Ast,
    unit: Id<CompilationUnit>,
    line_info: &'a LineInfo,
    priority_items: Vec<PriorityItem>,
    code: Vec<u16>,
    initial: Vec<u16>,
}

impl<'a> MemberSorter<'a> {
    pub fn new(
        ast: &'a Ast,
        unit: Id<CompilationUnit>,
        line_info: &'a LineInfo,
        sort_constructors_first: bool,
    ) -> Self {
        let initial: Vec<u16> = ast.tokens.source.encode_utf16().collect();
        MemberSorter {
            ast,
            unit,
            line_info,
            priority_items: priority_items(sort_constructors_first),
            code: initial.clone(),
            initial,
        }
    }

    /// Dart `sort`.
    pub fn sort(mut self) -> Vec<SourceEdit> {
        self.sort_classes_members();
        self.sort_unit_members();
        self.sort_unit_directives();
        let mut edits = Vec::new();
        if self.code != self.initial {
            let (offset, length, replacement) = compute_simple_diff(&self.initial, &self.code);
            edits.push(SourceEdit {
                offset: offset as u32,
                length: length as u32,
                replacement: String::from_utf16_lossy(&replacement),
                id: 0,
            });
        }
        edits
    }

    fn get_priority(&self, item: &PriorityItem) -> usize {
        self.priority_items
            .iter()
            .position(|p| p.equals(item))
            .unwrap_or(0)
    }

    fn sorted_members(&self, members: &[MemberInfo]) -> Vec<MemberInfo> {
        let mut sorted = members.to_vec();
        sorted.sort_by(|o1, o2| {
            let p1 = self.get_priority(&o1.item);
            let p2 = self.get_priority(&o2.item);
            if p1 == p2 {
                if o1.item.kind == MemberKind::ClassField {
                    return o1.offset.cmp(&o2.offset);
                }
                let mut result = dart_compare(&o1.name.to_lowercase(), &o2.name.to_lowercase());
                if result == Ordering::Equal {
                    result = dart_compare(&o1.name, &o2.name);
                }
                if result == Ordering::Equal {
                    result = o1.offset.cmp(&o2.offset);
                }
                return result;
            }
            p1.cmp(&p2)
        });
        sorted
    }

    fn sort_and_reorder_members(&mut self, members: Vec<MemberInfo>) {
        let sorted = self.sorted_members(&members);
        let size = sorted.len();
        for i in 0..size {
            let new_info = &sorted[size - 1 - i];
            let old_info = &members[size - 1 - i];
            if new_info.offset != old_info.offset {
                let mut code = self.code[..old_info.offset as usize].to_vec();
                code.extend(&new_info.text);
                code.extend(&self.code[old_info.end as usize..]);
                self.code = code;
            }
        }
    }

    fn member_info(&self, node: NodeId, item: PriorityItem, name: String) -> MemberInfo {
        let range = RangeFactory::new(self.ast).node_with_comments(self.line_info, node);
        let text = self.code[range.offset as usize..range.end() as usize].to_vec();
        MemberInfo {
            item,
            name,
            offset: range.offset,
            end: range.end(),
            text,
        }
    }

    fn lexeme(&self, t: dartr_syntax::TokenId) -> String {
        self.ast.tokens.lexeme(t).to_string()
    }

    fn sort_classes_members(&mut self) {
        let ast = self.ast;
        let declarations: Vec<NodeId> = ast.list_raw(ast[self.unit].declarations).to_vec();
        for member in declarations {
            let members = class_members(ast, member);
            if let Some(members) = members {
                self.sort_class_members(&members);
            }
        }
    }

    fn sort_class_members(&mut self, members_to_sort: &[NodeId]) {
        let ast = self.ast;
        let mut members = Vec::new();
        for &member in members_to_sort {
            let (kind, is_static, name) =
                if let Some(c) = ast.cast::<ConstructorDeclaration>(member) {
                    (
                        MemberKind::ClassConstructor,
                        false,
                        ast[c].name.map(|t| self.lexeme(t)).unwrap_or_default(),
                    )
                } else if let Some(f) = ast.cast::<FieldDeclaration>(member) {
                    let list = ast[f].fields;
                    let vars = ast.list(ast[list].variables);
                    if vars.is_empty() {
                        return;
                    }
                    (
                        MemberKind::ClassField,
                        ast[f].static_keyword.is_some(),
                        self.lexeme(ast[vars[0]].name),
                    )
                } else if let Some(m) = ast.cast::<MethodDeclaration>(member) {
                    let is_static = ast[m]
                        .modifier_keyword
                        .is_some_and(|t| ast.tokens.lexeme(t) == "static");
                    let mut name = self.lexeme(ast[m].name);
                    let property = ast[m]
                        .property_keyword
                        .map(|t| ast.tokens.lexeme(t).to_string());
                    let kind = match property.as_deref() {
                        Some("get") => {
                            name.push_str(" getter");
                            MemberKind::ClassAccessor
                        }
                        Some("set") => {
                            name.push_str(" setter");
                            MemberKind::ClassAccessor
                        }
                        _ => MemberKind::ClassMethod,
                    };
                    (kind, is_static, name)
                } else if ast.is::<PrimaryConstructorBody>(member) {
                    (
                        MemberKind::PrimaryConstructorBody,
                        false,
                        "this".to_string(),
                    )
                } else {
                    return;
                };
            let item = PriorityItem::for_name(is_static, &name, kind);
            members.push(self.member_info(member, item, name));
        }
        self.sort_and_reorder_members(members);
    }

    fn sort_unit_members(&mut self) {
        let ast = self.ast;
        let mut members = Vec::new();
        let declarations: Vec<NodeId> = ast.list_raw(ast[self.unit].declarations).to_vec();
        for member in declarations {
            let (kind, name) = if let Some(c) = ast.cast::<ClassDeclaration>(member) {
                (
                    MemberKind::UnitClass,
                    self.lexeme(type_name_token(ast, ast[c].name_part)),
                )
            } else if let Some(c) = ast.cast::<ClassTypeAlias>(member) {
                (MemberKind::UnitClass, self.lexeme(ast[c].name))
            } else if let Some(e) = ast.cast::<EnumDeclaration>(member) {
                (
                    MemberKind::UnitClass,
                    self.lexeme(type_name_token(ast, ast[e].name_part)),
                )
            } else if let Some(e) = ast.cast::<ExtensionTypeDeclaration>(member) {
                (
                    MemberKind::UnitExtensionType,
                    self.lexeme(type_name_token(ast, ast[e].name_part)),
                )
            } else if let Some(e) = ast.cast::<ExtensionDeclaration>(member) {
                (
                    MemberKind::UnitExtension,
                    ast[e].name.map(|t| self.lexeme(t)).unwrap_or_default(),
                )
            } else if let Some(f) = ast.cast::<FunctionDeclaration>(member) {
                let mut name = self.lexeme(ast[f].name);
                let property = ast[f]
                    .property_keyword
                    .map(|t| ast.tokens.lexeme(t).to_string());
                let kind = match property.as_deref() {
                    Some("get") => {
                        name.push_str(" getter");
                        MemberKind::UnitAccessor
                    }
                    Some("set") => {
                        name.push_str(" setter");
                        MemberKind::UnitAccessor
                    }
                    _ if name == "main" => MemberKind::UnitFunctionMain,
                    _ => MemberKind::UnitFunction,
                };
                (kind, name)
            } else if let Some(f) = ast.cast::<FunctionTypeAlias>(member) {
                (MemberKind::UnitFunctionType, self.lexeme(ast[f].name))
            } else if let Some(g) = ast.cast::<GenericTypeAlias>(member) {
                (MemberKind::UnitGenericTypeAlias, self.lexeme(ast[g].name))
            } else if let Some(m) = ast.cast::<MixinDeclaration>(member) {
                (MemberKind::UnitClass, self.lexeme(ast[m].name))
            } else if let Some(v) = ast.cast::<TopLevelVariableDeclaration>(member) {
                let list = ast[v].variables;
                let vars = ast.list(ast[list].variables);
                if vars.is_empty() {
                    return;
                }
                let is_const = ast[list]
                    .keyword
                    .is_some_and(|t| ast.tokens.lexeme(t) == "const");
                let kind = if is_const {
                    MemberKind::UnitVariableConst
                } else {
                    MemberKind::UnitVariable
                };
                (kind, self.lexeme(ast[vars[0]].name))
            } else {
                return;
            };
            let item = PriorityItem::for_name(false, &name, kind);
            members.push(self.member_info(member, item, name));
        }
        self.sort_and_reorder_members(members);
    }

    /// Dart `_sortUnitDirectives`: the import organizer on the current code
    /// without diagnostics and without removing unused imports.
    fn sort_unit_directives(&mut self) {
        let organizer = ImportOrganizer {
            resolution: None,
            ast: self.ast,
            unit: self.unit,
            line_info: self.line_info,
            diagnostics: &[],
            remove_unused: false,
        };
        let text = super::utils::Text {
            units: self.code.clone(),
        };
        self.code = organizer.organize_code(&text);
    }
}

/// The members of a class-like declaration.
fn class_members(ast: &Ast, member: NodeId) -> Option<Vec<NodeId>> {
    let body = if let Some(c) = ast.cast::<ClassDeclaration>(member) {
        ast[c].body.raw()
    } else if let Some(e) = ast.cast::<EnumDeclaration>(member) {
        ast[e].body.raw()
    } else if let Some(e) = ast.cast::<ExtensionDeclaration>(member) {
        ast[e].body.raw()
    } else if let Some(e) = ast.cast::<ExtensionTypeDeclaration>(member) {
        ast[e].body.raw()
    } else if let Some(m) = ast.cast::<MixinDeclaration>(member) {
        ast[m].body.raw()
    } else {
        return None;
    };
    if let Some(b) = ast.cast::<BlockClassBody>(body) {
        return Some(ast.list_raw(ast[b].members).to_vec());
    }
    if let Some(b) = ast.cast::<BlockEnumBody>(body) {
        return Some(ast.list_raw(ast[b].members).to_vec());
    }
    Some(Vec::new())
}

/// Dart `namePart.typeName`.
fn type_name_token(ast: &Ast, name_part: Id<ClassNamePart>) -> dartr_syntax::TokenId {
    dartr_resolver::error::support::class_name_token(ast, name_part)
}

/// Dart `_getPriorityItems`.
fn priority_items(sort_constructors_first: bool) -> Vec<PriorityItem> {
    use MemberKind::*;
    let p = PriorityItem::new;
    let mut v = vec![
        p(false, UnitFunctionMain, false),
        p(false, UnitVariableConst, false),
        p(false, UnitVariableConst, true),
        p(false, UnitVariable, false),
        p(false, UnitVariable, true),
        p(false, UnitAccessor, false),
        p(false, UnitAccessor, true),
        p(false, UnitFunction, false),
        p(false, UnitFunction, true),
        p(false, UnitGenericTypeAlias, false),
        p(false, UnitGenericTypeAlias, true),
        p(false, UnitFunctionType, false),
        p(false, UnitFunctionType, true),
        p(false, UnitClass, false),
        p(false, UnitClass, true),
        p(false, UnitExtensionType, false),
        p(false, UnitExtensionType, true),
        p(false, UnitExtension, false),
        p(false, UnitExtension, true),
    ];
    if sort_constructors_first {
        v.push(p(false, PrimaryConstructorBody, false));
        v.push(p(false, ClassConstructor, false));
        v.push(p(false, ClassConstructor, true));
    }
    v.push(p(true, ClassField, false));
    v.push(p(true, ClassAccessor, false));
    v.push(p(true, ClassAccessor, true));
    v.push(p(false, ClassField, false));
    if !sort_constructors_first {
        v.push(p(false, PrimaryConstructorBody, false));
        v.push(p(false, ClassConstructor, false));
        v.push(p(false, ClassConstructor, true));
    }
    v.extend([
        p(false, ClassAccessor, false),
        p(false, ClassAccessor, true),
        p(false, ClassMethod, false),
        p(false, ClassMethod, true),
        p(true, ClassMethod, false),
        p(true, ClassMethod, true),
    ]);
    v
}

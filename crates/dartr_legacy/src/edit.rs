// Dart source: pkg/analysis_server/lib/src/handler/legacy/edit_format.dart
// Dart source: pkg/analysis_server/lib/src/handler/legacy/edit_sort_members.dart
// Dart source: pkg/analysis_server/lib/src/handler/legacy/edit_organize_directives.dart
// Dart source: pkg/analysis_server/lib/src/services/correction/sort_members.dart
// Dart source: pkg/analysis_server/lib/src/services/correction/organize_imports.dart
// Dart source: pkg/analyzer_plugin/lib/src/utilities/directive_sort.dart
// Dart source: pkg/analysis_server/lib/src/utilities/extensions/range_factory.dart
// Dart source: pkg/analysis_server/lib/src/utilities/strings.dart

use std::cmp::Ordering;

use dartr_ast::{
    Annotation, Ast, BlockClassBody, BlockEnumBody, ClassDeclaration, CompilationUnit,
    ConstructorDeclaration, EnumDeclaration, ExportDirective, ExtensionDeclaration,
    ExtensionTypeDeclaration, FieldDeclaration, FunctionDeclaration, FunctionTypeAlias,
    GenericTypeAlias, Id, ImportDirective, LibraryDirective, MethodDeclaration, MixinDeclaration,
    NameWithTypeParameters, NodeId, NodeKind, PartDirective, PrimaryConstructorBody,
    PrimaryConstructorDeclaration, ShowCombinator, SimpleIdentifier, TopLevelVariableDeclaration,
};
use dartr_diagnostics::{Diagnostic, DiagnosticCode, diag};
use dartr_element::{Ctx, FId, LibraryFragment, ResolutionTables};
use dartr_format::text::{substring_utf16, utf16_len};
use dartr_format::{DartFormatter, SourceCode, TrailingCommas, Version};
use dartr_project::analysis_options::TrailingCommas as OptionTrailingCommas;
use dartr_resolver::element_metadata::{
    AnnotationRef, TargetKind, UnitAst, string_value, target_kinds,
};
use dartr_syntax::token::flags;
use dartr_syntax::{LineInfo, TokenId, TokenType};

use crate::protocol;

pub enum FormatOutcome {
    Ok(protocol::EditFormatResult),
    FormatWithErrors,
    InvalidSelection(String),
}

#[allow(clippy::too_many_arguments)]
pub fn format_code(
    unformatted_code: &str,
    version: (u32, u32),
    experiments: &[&str],
    options_page_width: Option<i64>,
    options_trailing_commas: Option<OptionTrailingCommas>,
    selection_offset: i64,
    selection_length: i64,
    line_length: Option<i64>,
) -> FormatOutcome {
    let (start, length) = if selection_offset == 0 && selection_length == 0 {
        (None, None)
    } else {
        if selection_offset < 0 || selection_length < 0 {
            return FormatOutcome::InvalidSelection("Invalid selection.".into());
        }
        (
            Some(selection_offset as usize),
            Some(selection_length as usize),
        )
    };

    let source = match SourceCode::new(unformatted_code, None, true, start, length) {
        Ok(s) => s,
        Err(msg) => return FormatOutcome::InvalidSelection(msg),
    };

    let mut formatter = DartFormatter::new(Version::new(version.0, version.1));
    if let Some(w) = options_page_width.or(line_length) {
        if w <= 0 {
            return FormatOutcome::InvalidSelection("Page width must be positive.".into());
        }
        formatter.page_width = w as usize;
    }
    if let Some(tc) = options_trailing_commas {
        formatter.trailing_commas = match tc {
            OptionTrailingCommas::Automate => TrailingCommas::Automate,
            OptionTrailingCommas::Preserve => TrailingCommas::Preserve,
        };
    }
    formatter.experiment_flags = experiments.iter().map(|s| (*s).to_string()).collect();

    let formatted = match formatter.format_source(&source) {
        Ok(res) => res,
        Err(_) => return FormatOutcome::FormatWithErrors,
    };

    let mut edits = Vec::new();
    if formatted.text != unformatted_code {
        edits.push(protocol::SourceEdit {
            offset: 0,
            length: utf16_len(unformatted_code) as i64,
            replacement: formatted.text,
            id: None,
            description: None,
        });
    }

    FormatOutcome::Ok(protocol::EditFormatResult {
        edits,
        selection_offset: formatted.selection_start.unwrap_or(0) as i64,
        selection_length: formatted.selection_length.unwrap_or(0) as i64,
    })
}

pub fn sort_members(
    initial_code: &str,
    ast: &Ast,
    unit: Id<CompilationUnit>,
    line_info: &LineInfo,
    sort_constructors_first: bool,
) -> Vec<protocol::SourceEdit> {
    let mut sorter = MemberSorter::new(initial_code, ast, unit, line_info, sort_constructors_first);
    sorter.sort()
}

pub fn organize_directives(
    initial_code: &str,
    ast: &Ast,
    unit: Id<CompilationUnit>,
    line_info: &LineInfo,
    diagnostics: &[Diagnostic],
    resolution: Option<(&Ctx<'_>, &ResolutionTables, FId<LibraryFragment>)>,
) -> Vec<protocol::SourceEdit> {
    let mut organizer = ImportOrganizer::new(
        initial_code,
        ast,
        unit,
        line_info,
        diagnostics,
        resolution,
        true,
    );
    organizer.organize()
}

// ---------------------------------------------------------------------------
// MemberSorter (sort_members.dart)
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
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
    is_static: bool,
    kind: MemberKind,
    is_private: bool,
}

impl PriorityItem {
    fn new(is_static: bool, kind: MemberKind, is_private: bool) -> Self {
        Self {
            is_static,
            kind,
            is_private,
        }
    }

    fn for_name(is_static: bool, name: &str, kind: MemberKind) -> Self {
        let is_private = name.starts_with('_');
        Self::new(is_static, kind, is_private)
    }
}

impl PartialEq for PriorityItem {
    fn eq(&self, other: &Self) -> bool {
        if self.kind == MemberKind::ClassField {
            return other.kind == self.kind && other.is_static == self.is_static;
        }
        other.kind == self.kind
            && other.is_private == self.is_private
            && other.is_static == self.is_static
    }
}
impl Eq for PriorityItem {}

#[derive(Clone, PartialEq, Eq, Debug)]
struct MemberInfo {
    index: usize,
    item: PriorityItem,
    name: String,
    offset: usize,
    length: usize,
    end: usize,
    text: String,
}

struct MemberSorter<'a> {
    initial_code: &'a str,
    ast: &'a Ast,
    unit: Id<CompilationUnit>,
    line_info: &'a LineInfo,
    priority_items: Vec<PriorityItem>,
    code: String,
}

impl<'a> MemberSorter<'a> {
    fn new(
        initial_code: &'a str,
        ast: &'a Ast,
        unit: Id<CompilationUnit>,
        line_info: &'a LineInfo,
        sort_constructors_first: bool,
    ) -> Self {
        Self {
            initial_code,
            ast,
            unit,
            line_info,
            priority_items: Self::get_priority_items(sort_constructors_first),
            code: initial_code.to_string(),
        }
    }

    fn sort(&mut self) -> Vec<protocol::SourceEdit> {
        self.sort_classes_members();
        self.sort_unit_members();
        self.sort_unit_directives();
        let mut edits = Vec::new();
        if self.code != self.initial_code {
            let (offset, length, replacement) = compute_simple_diff(self.initial_code, &self.code);
            edits.push(protocol::SourceEdit {
                offset: offset as i64,
                length: length as i64,
                replacement,
                id: None,
                description: None,
            });
        }
        edits
    }

    fn get_priority(&self, item: PriorityItem) -> usize {
        self.priority_items
            .iter()
            .position(|&p| p == item)
            .unwrap_or(0)
    }

    fn get_sorted_members(&self, members: &[MemberInfo]) -> Vec<MemberInfo> {
        let mut members_sorted = members.to_vec();
        members_sorted.sort_by(|o1, o2| {
            let priority1 = self.get_priority(o1.item);
            let priority2 = self.get_priority(o2.item);
            if priority1 == priority2 {
                if o1.item.kind == MemberKind::ClassField {
                    return o1.offset.cmp(&o2.offset);
                }
                let name1 = o1.name.to_lowercase();
                let name2 = o2.name.to_lowercase();
                let mut result = cmp_utf16(&name1, &name2);
                if result == Ordering::Equal {
                    result = cmp_utf16(&o1.name, &o2.name);
                }
                if result == Ordering::Equal {
                    result = o1.offset.cmp(&o2.offset);
                }
                return result;
            }
            priority1.cmp(&priority2)
        });
        members_sorted
    }

    fn sort_and_reorder_members(&mut self, members: &[MemberInfo]) {
        let members_sorted = self.get_sorted_members(members);
        let size = members_sorted.len();
        for i in 0..size {
            let new_info = &members_sorted[size - 1 - i];
            let old_info = &members[size - 1 - i];
            if new_info.index != old_info.index {
                let before_code = substring_utf16(&self.code, 0, old_info.offset).to_string();
                let after_code = substring_utf16(&self.code, old_info.end, usize::MAX).to_string();
                self.code = format!("{before_code}{}{after_code}", new_info.text);
            }
        }
    }

    fn sort_classes_members(&mut self) {
        let declarations: Vec<NodeId> =
            self.ast.list_raw(self.ast[self.unit].declarations).to_vec();
        for unit_member in declarations {
            let body_members: Option<Vec<NodeId>> = match self.ast.kind(unit_member) {
                NodeKind::ClassDeclaration => {
                    let body = self.ast[Id::<ClassDeclaration>::from_raw(unit_member)].body;
                    self.ast
                        .cast::<BlockClassBody>(body)
                        .map(|b| self.ast.list_raw(self.ast[b].members).to_vec())
                }
                NodeKind::EnumDeclaration => {
                    let body = self.ast[Id::<EnumDeclaration>::from_raw(unit_member)].body;
                    self.ast
                        .cast::<BlockEnumBody>(body)
                        .map(|b| self.ast.list_raw(self.ast[b].members).to_vec())
                }
                NodeKind::ExtensionDeclaration => {
                    let body = self.ast[Id::<ExtensionDeclaration>::from_raw(unit_member)].body;
                    self.ast
                        .cast::<BlockClassBody>(body)
                        .map(|b| self.ast.list_raw(self.ast[b].members).to_vec())
                }
                NodeKind::ExtensionTypeDeclaration => {
                    let body = self.ast[Id::<ExtensionTypeDeclaration>::from_raw(unit_member)].body;
                    self.ast
                        .cast::<BlockClassBody>(body)
                        .map(|b| self.ast.list_raw(self.ast[b].members).to_vec())
                }
                NodeKind::MixinDeclaration => {
                    let body = self.ast[Id::<MixinDeclaration>::from_raw(unit_member)].body;
                    self.ast
                        .cast::<BlockClassBody>(body)
                        .map(|b| self.ast.list_raw(self.ast[b].members).to_vec())
                }
                _ => None,
            };
            if let Some(members) = body_members {
                self.sort_class_members(&members);
            }
        }
    }

    fn sort_class_members(&mut self, members_to_sort: &[NodeId]) {
        let mut members = Vec::new();
        for (idx, &member) in members_to_sort.iter().enumerate() {
            let kind: MemberKind;
            let mut is_static = false;
            let mut name: String;
            match self.ast.kind(member) {
                NodeKind::ConstructorDeclaration => {
                    let decl = &self.ast[Id::<ConstructorDeclaration>::from_raw(member)];
                    kind = MemberKind::ClassConstructor;
                    name = decl
                        .name
                        .map(|t| self.ast.tokens.lexeme(t).to_string())
                        .unwrap_or_default();
                }
                NodeKind::FieldDeclaration => {
                    let decl = &self.ast[Id::<FieldDeclaration>::from_raw(member)];
                    let variables = self.ast[decl.fields].variables;
                    let var_list = self.ast.list(variables);
                    if let Some(&first_var) = var_list.first() {
                        kind = MemberKind::ClassField;
                        is_static = decl.static_keyword.is_some();
                        name = self.ast.tokens.lexeme(self.ast[first_var].name).to_string();
                    } else {
                        return;
                    }
                }
                NodeKind::MethodDeclaration => {
                    let decl = &self.ast[Id::<MethodDeclaration>::from_raw(member)];
                    is_static = decl
                        .modifier_keyword
                        .is_some_and(|t| self.ast.tokens.lexeme(t) == "static");
                    name = self.ast.tokens.lexeme(decl.name).to_string();
                    let prop = decl.property_keyword.map(|t| self.ast.tokens.lexeme(t));
                    if prop == Some("get") {
                        kind = MemberKind::ClassAccessor;
                        name.push_str(" getter");
                    } else if prop == Some("set") {
                        kind = MemberKind::ClassAccessor;
                        name.push_str(" setter");
                    } else {
                        kind = MemberKind::ClassMethod;
                    }
                }
                NodeKind::PrimaryConstructorBody => {
                    let _ = Id::<PrimaryConstructorBody>::from_raw(member);
                    kind = MemberKind::PrimaryConstructorBody;
                    name = "this".to_string();
                }
                _ => return,
            }
            let item = PriorityItem::for_name(is_static, &name, kind);
            let (offset, length) = node_with_comments(self.ast, self.unit, self.line_info, member);
            let text = substring_utf16(&self.code, offset, offset + length).to_string();
            members.push(MemberInfo {
                index: idx,
                item,
                name,
                offset,
                length,
                end: offset + length,
                text,
            });
        }
        self.sort_and_reorder_members(&members);
    }

    fn sort_unit_directives(&mut self) {
        let mut organizer = ImportOrganizer::new(
            &self.code,
            self.ast,
            self.unit,
            self.line_info,
            &[],
            None,
            false,
        );
        organizer.organize();
        self.code = organizer.code;
    }

    fn sort_unit_members(&mut self) {
        let declarations: Vec<NodeId> =
            self.ast.list_raw(self.ast[self.unit].declarations).to_vec();
        let mut members = Vec::new();
        for (idx, &member) in declarations.iter().enumerate() {
            let kind: MemberKind;
            let mut name: String;
            match self.ast.kind(member) {
                NodeKind::ClassDeclaration => {
                    let decl = &self.ast[Id::<ClassDeclaration>::from_raw(member)];
                    kind = MemberKind::UnitClass;
                    name = name_part_lexeme(self.ast, decl.name_part.raw());
                }
                NodeKind::ClassTypeAlias => {
                    let decl = &self.ast[Id::<dartr_ast::ClassTypeAlias>::from_raw(member)];
                    kind = MemberKind::UnitClass;
                    name = self.ast.tokens.lexeme(decl.name).to_string();
                }
                NodeKind::EnumDeclaration => {
                    let decl = &self.ast[Id::<EnumDeclaration>::from_raw(member)];
                    kind = MemberKind::UnitClass;
                    name = name_part_lexeme(self.ast, decl.name_part.raw());
                }
                NodeKind::ExtensionTypeDeclaration => {
                    let decl = &self.ast[Id::<ExtensionTypeDeclaration>::from_raw(member)];
                    kind = MemberKind::UnitExtensionType;
                    name = name_part_lexeme(self.ast, decl.name_part.raw());
                }
                NodeKind::ExtensionDeclaration => {
                    let decl = &self.ast[Id::<ExtensionDeclaration>::from_raw(member)];
                    kind = MemberKind::UnitExtension;
                    name = decl
                        .name
                        .map(|t| self.ast.tokens.lexeme(t).to_string())
                        .unwrap_or_default();
                }
                NodeKind::FunctionDeclaration => {
                    let decl = &self.ast[Id::<FunctionDeclaration>::from_raw(member)];
                    name = self.ast.tokens.lexeme(decl.name).to_string();
                    let prop = decl.property_keyword.map(|t| self.ast.tokens.lexeme(t));
                    if prop == Some("get") {
                        kind = MemberKind::UnitAccessor;
                        name.push_str(" getter");
                    } else if prop == Some("set") {
                        kind = MemberKind::UnitAccessor;
                        name.push_str(" setter");
                    } else if name == "main" {
                        kind = MemberKind::UnitFunctionMain;
                    } else {
                        kind = MemberKind::UnitFunction;
                    }
                }
                NodeKind::FunctionTypeAlias => {
                    let decl = &self.ast[Id::<FunctionTypeAlias>::from_raw(member)];
                    kind = MemberKind::UnitFunctionType;
                    name = self.ast.tokens.lexeme(decl.name).to_string();
                }
                NodeKind::GenericTypeAlias => {
                    let decl = &self.ast[Id::<GenericTypeAlias>::from_raw(member)];
                    kind = MemberKind::UnitGenericTypeAlias;
                    name = self.ast.tokens.lexeme(decl.name).to_string();
                }
                NodeKind::MixinDeclaration => {
                    let decl = &self.ast[Id::<MixinDeclaration>::from_raw(member)];
                    kind = MemberKind::UnitClass;
                    name = self.ast.tokens.lexeme(decl.name).to_string();
                }
                NodeKind::TopLevelVariableDeclaration => {
                    let decl = &self.ast[Id::<TopLevelVariableDeclaration>::from_raw(member)];
                    let var_list_node = &self.ast[decl.variables];
                    let vars = self.ast.list(var_list_node.variables);
                    if let Some(&first_var) = vars.first() {
                        let is_const = var_list_node
                            .keyword
                            .is_some_and(|t| self.ast.tokens.lexeme(t) == "const");
                        if is_const {
                            kind = MemberKind::UnitVariableConst;
                        } else {
                            kind = MemberKind::UnitVariable;
                        }
                        name = self.ast.tokens.lexeme(self.ast[first_var].name).to_string();
                    } else {
                        return;
                    }
                }
                _ => return,
            }
            let item = PriorityItem::for_name(false, &name, kind);
            let (offset, length) = node_with_comments(self.ast, self.unit, self.line_info, member);
            let text = substring_utf16(&self.code, offset, offset + length).to_string();
            members.push(MemberInfo {
                index: idx,
                item,
                name,
                offset,
                length,
                end: offset + length,
                text,
            });
        }
        self.sort_and_reorder_members(&members);
    }

    fn get_priority_items(sort_constructors_first: bool) -> Vec<PriorityItem> {
        use MemberKind::*;
        let mut items = vec![
            PriorityItem::new(false, UnitFunctionMain, false),
            PriorityItem::new(false, UnitVariableConst, false),
            PriorityItem::new(false, UnitVariableConst, true),
            PriorityItem::new(false, UnitVariable, false),
            PriorityItem::new(false, UnitVariable, true),
            PriorityItem::new(false, UnitAccessor, false),
            PriorityItem::new(false, UnitAccessor, true),
            PriorityItem::new(false, UnitFunction, false),
            PriorityItem::new(false, UnitFunction, true),
            PriorityItem::new(false, UnitGenericTypeAlias, false),
            PriorityItem::new(false, UnitGenericTypeAlias, true),
            PriorityItem::new(false, UnitFunctionType, false),
            PriorityItem::new(false, UnitFunctionType, true),
            PriorityItem::new(false, UnitClass, false),
            PriorityItem::new(false, UnitClass, true),
            PriorityItem::new(false, UnitExtensionType, false),
            PriorityItem::new(false, UnitExtensionType, true),
            PriorityItem::new(false, UnitExtension, false),
            PriorityItem::new(false, UnitExtension, true),
        ];
        if sort_constructors_first {
            items.push(PriorityItem::new(false, PrimaryConstructorBody, false));
            items.push(PriorityItem::new(false, ClassConstructor, false));
            items.push(PriorityItem::new(false, ClassConstructor, true));
        }
        items.push(PriorityItem::new(true, ClassField, false));
        items.push(PriorityItem::new(true, ClassAccessor, false));
        items.push(PriorityItem::new(true, ClassAccessor, true));
        items.push(PriorityItem::new(false, ClassField, false));
        if !sort_constructors_first {
            items.push(PriorityItem::new(false, PrimaryConstructorBody, false));
            items.push(PriorityItem::new(false, ClassConstructor, false));
            items.push(PriorityItem::new(false, ClassConstructor, true));
        }
        items.push(PriorityItem::new(false, ClassAccessor, false));
        items.push(PriorityItem::new(false, ClassAccessor, true));
        items.push(PriorityItem::new(false, ClassMethod, false));
        items.push(PriorityItem::new(false, ClassMethod, true));
        items.push(PriorityItem::new(true, ClassMethod, false));
        items.push(PriorityItem::new(true, ClassMethod, true));
        items
    }
}

fn name_part_lexeme(ast: &Ast, name_part: NodeId) -> String {
    let token = match ast.kind(name_part) {
        NodeKind::NameWithTypeParameters => {
            ast[Id::<NameWithTypeParameters>::from_raw(name_part)].type_name
        }
        NodeKind::PrimaryConstructorDeclaration => {
            ast[Id::<PrimaryConstructorDeclaration>::from_raw(name_part)].type_name
        }
        _ => return String::new(),
    };
    ast.tokens.lexeme(token).to_string()
}

fn node_with_comments(
    ast: &Ast,
    unit: Id<CompilationUnit>,
    line_info: &LineInfo,
    node: NodeId,
) -> (usize, usize) {
    let begin_token = ast.begin_token(node);
    let is_first_item = begin_token == ast[unit].begin_token;
    let this_leading_comment = if is_first_item {
        begin_token
    } else {
        leading_comment(ast, line_info, begin_token)
    };
    let this_trailing_comment = trailing_comment(ast, line_info, ast.end_token(node), false);
    let start = ast.tokens.offset(this_leading_comment) as usize;
    let end = ast.tokens.get(this_trailing_comment).end() as usize;
    (start, end.saturating_sub(start))
}

fn are_different_lines(ast: &Ast, line_info: &LineInfo, token: TokenId, other: TokenId) -> bool {
    !line_info.on_same_line(ast.tokens.offset(token), ast.tokens.offset(other))
}

fn leading_comment(ast: &Ast, line_info: &LineInfo, token: TokenId) -> TokenId {
    let previous = ast.tokens.get(token).previous;
    if previous.is_none() || ast.tokens.get(previous).is_eof() {
        return ast
            .tokens
            .get(token)
            .preceding_comments
            .get()
            .unwrap_or(token);
    }
    let mut comment = ast.tokens.get(token).preceding_comments.get();
    if are_different_lines(ast, line_info, token, previous) {
        while let Some(c) = comment {
            if are_different_lines(ast, line_info, previous, c) {
                break;
            }
            comment = ast.tokens.get(c).next.get();
        }
    }
    comment.unwrap_or(token)
}

fn trailing_comment(
    ast: &Ast,
    line_info: &LineInfo,
    token: TokenId,
    return_comma: bool,
) -> TokenId {
    let mut last_token = token;
    let Some(mut next_token) = ast.tokens.get(last_token).next.get() else {
        return token;
    };
    let includes_comma = ast.tokens.ty(next_token) == TokenType::COMMA
        && should_include_comments_after_comma(ast, line_info, next_token);
    if includes_comma {
        last_token = next_token;
        let Some(nt) = ast.tokens.get(last_token).next.get() else {
            return if return_comma { last_token } else { token };
        };
        next_token = nt;
    }
    let mut comment = ast.tokens.get(next_token).preceding_comments.get();
    if comment.is_none() && includes_comma && are_different_lines(ast, line_info, token, last_token)
    {
        comment = ast.tokens.get(last_token).preceding_comments.get();
        last_token = token;
    }
    if let Some(mut c) = comment
        && line_info.on_same_line(ast.tokens.offset(c), ast.tokens.offset(last_token))
    {
        let mut next = ast.tokens.get(c).next.get();
        while let Some(n) = next {
            if !line_info.on_same_line(ast.tokens.offset(n), ast.tokens.offset(last_token)) {
                break;
            }
            c = n;
            next = ast.tokens.get(c).next.get();
        }
        return c;
    }
    if return_comma { last_token } else { token }
}

fn should_include_comments_after_comma(ast: &Ast, line_info: &LineInfo, comma: TokenId) -> bool {
    let Some(token_after_comma) = ast.tokens.get(comma).next.get() else {
        return false;
    };
    let ty = ast.tokens.ty(token_after_comma);
    if matches!(
        ty,
        TokenType::CLOSE_CURLY_BRACKET | TokenType::CLOSE_PAREN | TokenType::CLOSE_SQUARE_BRACKET
    ) {
        return true;
    }
    are_different_lines(ast, line_info, comma, token_after_comma)
}

// ---------------------------------------------------------------------------
// ImportOrganizer (organize_imports.dart & directive_sort.dart)
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug)]
enum DirectiveSortPriority {
    ImportSdk = 0,
    ImportPkg = 1,
    ImportOther = 2,
    ImportRel = 3,
    ExportSdk = 4,
    ExportPkg = 5,
    ExportOther = 6,
    ExportRel = 7,
    Part = 8,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum DirectiveSortKind {
    Import,
    Export,
    Part,
}

impl DirectiveSortPriority {
    fn new(uri: &str, kind: DirectiveSortKind) -> Self {
        match kind {
            DirectiveSortKind::Import => {
                if uri.starts_with("dart:") {
                    Self::ImportSdk
                } else if uri.starts_with("package:") {
                    Self::ImportPkg
                } else if uri.contains("://") {
                    Self::ImportOther
                } else {
                    Self::ImportRel
                }
            }
            DirectiveSortKind::Export => {
                if uri.starts_with("dart:") {
                    Self::ExportSdk
                } else if uri.starts_with("package:") {
                    Self::ExportPkg
                } else if uri.contains("://") {
                    Self::ExportOther
                } else {
                    Self::ExportRel
                }
            }
            DirectiveSortKind::Part => Self::Part,
        }
    }
}

fn compare_directive_uri(a: &str, b: &str) -> Ordering {
    if (!a.starts_with("package:") || !b.starts_with("package:"))
        && !a.starts_with('/')
        && !b.starts_with('/')
    {
        return cmp_utf16(a, b);
    }
    let Some(index_a) = a.find('/') else {
        return cmp_utf16(a, b);
    };
    let Some(index_b) = b.find('/') else {
        return cmp_utf16(a, b);
    };
    let res = cmp_utf16(&a[..index_a], &b[..index_b]);
    if res != Ordering::Equal {
        return res;
    }
    cmp_utf16(&a[index_a + 1..], &b[index_b + 1..])
}

#[derive(Clone)]
struct DirectiveInfo {
    directive: NodeId,
    priority: DirectiveSortPriority,
    uri: String,
    offset: usize,
    end: usize,
    text: String,
}

impl PartialEq for DirectiveInfo {
    fn eq(&self, other: &Self) -> bool {
        self.cmp(other) == Ordering::Equal
    }
}
impl Eq for DirectiveInfo {}
impl PartialOrd for DirectiveInfo {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}
impl Ord for DirectiveInfo {
    fn cmp(&self, other: &Self) -> Ordering {
        if self.priority == other.priority {
            let cmp = compare_directive_uri(&self.uri, &other.uri);
            if cmp != Ordering::Equal {
                return cmp;
            }
            return cmp_utf16(&self.text, &other.text);
        }
        self.priority.cmp(&other.priority)
    }
}

struct ImportOrganizer<'a> {
    initial_code: &'a str,
    ast: &'a Ast,
    unit: Id<CompilationUnit>,
    line_info: &'a LineInfo,
    diagnostics: &'a [Diagnostic],
    resolution: Option<(&'a Ctx<'a>, &'a ResolutionTables, FId<LibraryFragment>)>,
    remove_unused: bool,
    pub code: String,
    end_of_line: &'static str,
    has_unresolved_identifier_error: bool,
}

impl<'a> ImportOrganizer<'a> {
    fn new(
        initial_code: &'a str,
        ast: &'a Ast,
        unit: Id<CompilationUnit>,
        line_info: &'a LineInfo,
        diagnostics: &'a [Diagnostic],
        resolution: Option<(&'a Ctx<'a>, &'a ResolutionTables, FId<LibraryFragment>)>,
        remove_unused: bool,
    ) -> Self {
        let end_of_line = if initial_code.contains("\r\n") {
            "\r\n"
        } else {
            "\n"
        };
        let has_unresolved_identifier_error =
            diagnostics.iter().any(|d| d.code.is_unresolved_identifier);
        Self {
            initial_code,
            ast,
            unit,
            line_info,
            diagnostics,
            resolution,
            remove_unused,
            code: initial_code.to_string(),
            end_of_line,
            has_unresolved_identifier_error,
        }
    }

    fn organize(&mut self) -> Vec<protocol::SourceEdit> {
        self.organize_directives();
        let mut edits = Vec::new();
        if self.code != self.initial_code {
            let suffix_length = find_common_suffix(self.initial_code, &self.code);
            let initial_len = utf16_len(self.initial_code);
            let new_len = utf16_len(&self.code);
            let replacement =
                substring_utf16(&self.code, 0, new_len.saturating_sub(suffix_length)).to_string();
            edits.push(protocol::SourceEdit {
                offset: 0,
                length: (initial_len.saturating_sub(suffix_length)) as i64,
                replacement,
                id: None,
                description: None,
            });
        }
        edits
    }

    fn is_unused_import(&self, uri_node: NodeId) -> bool {
        let uri_offset = self.ast.offset(uri_node) as usize;
        for d in self.diagnostics {
            if (code_eq(d.code, &diag::DUPLICATE_IMPORT)
                || code_eq(d.code, &diag::UNUSED_IMPORT)
                || code_eq(d.code, &diag::UNNECESSARY_IMPORT))
                && d.offset == uri_offset
            {
                return true;
            }
        }
        false
    }

    fn is_unused_show_name(&self, name: Id<SimpleIdentifier>) -> bool {
        let offset = self.ast.offset(name.raw()) as usize;
        for d in self.diagnostics {
            if code_eq(d.code, &diag::UNUSED_SHOWN_NAME) && d.offset == offset {
                return true;
            }
        }
        false
    }

    fn is_library_target_annotation(&self, annotation: Id<Annotation>) -> bool {
        let Some((ctx, tables, fragment)) = self.resolution else {
            return false;
        };
        let anno_ref = AnnotationRef::of_node(
            ctx,
            UnitAst {
                ast: self.ast,
                tables,
            },
            annotation,
            fragment,
        );
        target_kinds(ctx, &anno_ref).is_some_and(|kinds| kinds.contains(&TargetKind::Library))
    }

    fn organize_directives(&mut self) {
        let mut has_library_directive = false;
        let mut directives = Vec::new();
        let unit_directives = self.ast.list_raw(self.ast[self.unit].directives);
        let first_unit_directive = unit_directives.first().copied();

        for &directive in unit_directives {
            if self.ast.is::<LibraryDirective>(directive) {
                has_library_directive = true;
            }
            let (kind, uri_node, metadata, doc_comment) = match self.ast.kind(directive) {
                NodeKind::ImportDirective => {
                    let d = &self.ast[Id::<ImportDirective>::from_raw(directive)];
                    (
                        DirectiveSortKind::Import,
                        d.uri.raw(),
                        d.metadata,
                        d.documentation_comment,
                    )
                }
                NodeKind::ExportDirective => {
                    let d = &self.ast[Id::<ExportDirective>::from_raw(directive)];
                    (
                        DirectiveSortKind::Export,
                        d.uri.raw(),
                        d.metadata,
                        d.documentation_comment,
                    )
                }
                NodeKind::PartDirective => {
                    let d = &self.ast[Id::<PartDirective>::from_raw(directive)];
                    (
                        DirectiveSortKind::Part,
                        d.uri.raw(),
                        d.metadata,
                        d.documentation_comment,
                    )
                }
                _ => continue,
            };

            let uri_content = string_value(self.ast, uri_node).unwrap_or_default();
            let priority = DirectiveSortPriority::new(&uri_content, kind);

            let mut offset = self.ast.offset(directive) as usize;
            let mut end = self.ast.end(directive) as usize;

            let is_pseudo_library_directive =
                !has_library_directive && Some(directive) == first_unit_directive;
            let mut last_library_annotation: Option<Id<Annotation>> = None;
            let mut library_docs_and_annotations_end_offset: Option<usize> = None;

            if is_pseudo_library_directive {
                last_library_annotation = self
                    .ast
                    .list(metadata)
                    .iter()
                    .copied()
                    .take_while(|&a| self.is_library_target_annotation(a))
                    .last();

                let raw_end = last_library_annotation
                    .map(|a| self.ast.end(a.raw()))
                    .or_else(|| doc_comment.map(|c| self.ast.end(c.raw())));

                if let Some(end_off) = raw_end
                    && let Ok(mut after_line) = self.line_info.get_offset_of_line_after(end_off)
                {
                    if let Ok(next_line_offset) =
                        self.line_info.get_offset_of_line_after(after_line)
                        && substring_utf16(
                            &self.code,
                            after_line as usize,
                            next_line_offset as usize,
                        )
                        .trim()
                        .is_empty()
                    {
                        after_line = next_line_offset;
                    }
                    library_docs_and_annotations_end_offset = Some(after_line as usize);
                }
            }

            let leading_token = if last_library_annotation.is_none() {
                Some(self.ast.begin_token(directive))
            } else {
                None
            };
            let leading_comment = leading_token.and_then(|lt| {
                Self::get_leading_comment(
                    self.ast,
                    self.unit,
                    lt,
                    self.line_info,
                    is_pseudo_library_directive,
                )
            });
            let trailing_comment = Self::get_trailing_comment(self.ast, directive, self.line_info);

            if let (Some(lc), Some(_)) = (leading_comment, leading_token) {
                let lc_offset = self.ast.tokens.offset(lc) as usize;
                offset = match library_docs_and_annotations_end_offset {
                    Some(lib_end) => lib_end.max(lc_offset),
                    None => lc_offset,
                };
            }
            if let Some(tc) = trailing_comment {
                end = self.ast.tokens.get(tc).end() as usize;
            }
            offset = library_docs_and_annotations_end_offset.unwrap_or(offset);
            let text = substring_utf16(&self.code, offset, end).to_string();
            directives.push(DirectiveInfo {
                directive,
                priority,
                uri: uri_content,
                offset,
                end,
                text,
            });
        }

        if directives.is_empty() {
            return;
        }
        let first_directive_offset = directives.first().unwrap().offset;
        let last_directive_end = directives.last().unwrap().end;

        directives.sort();

        let mut sb = String::new();
        let mut current_priority: Option<DirectiveSortPriority> = None;
        let mut previous_directive_text = String::new();

        for directive_info in &directives {
            let mut unused_show_names: Vec<Id<SimpleIdentifier>> = Vec::new();
            if !self.has_unresolved_identifier_error {
                let directive = directive_info.directive;
                let uri_node = match self.ast.kind(directive) {
                    NodeKind::ImportDirective => self.ast
                        [Id::<ImportDirective>::from_raw(directive)]
                    .uri
                    .raw(),
                    NodeKind::ExportDirective => self.ast
                        [Id::<ExportDirective>::from_raw(directive)]
                    .uri
                    .raw(),
                    NodeKind::PartDirective => {
                        self.ast[Id::<PartDirective>::from_raw(directive)].uri.raw()
                    }
                    _ => directive,
                };
                if (self.remove_unused && self.is_unused_import(uri_node))
                    || (self.remove_unused && previous_directive_text == directive_info.text)
                {
                    continue;
                }
                if let Some(import_id) = self.ast.cast::<ImportDirective>(directive) {
                    let combinators = self.ast.list_raw(self.ast[import_id].combinators);
                    if !combinators.is_empty() {
                        for &comb in combinators {
                            if let Some(show_id) = self.ast.cast::<ShowCombinator>(comb) {
                                for &shown in self.ast.list(self.ast[show_id].shown_names) {
                                    if self.is_unused_show_name(shown) {
                                        unused_show_names.push(shown);
                                    }
                                }
                            }
                        }
                    }
                }
            }
            if current_priority != Some(directive_info.priority) {
                if current_priority.is_some() {
                    sb.push_str(self.end_of_line);
                }
                current_priority = Some(directive_info.priority);
            }
            let mut text = directive_info.text.clone();
            if !unused_show_names.is_empty()
                && let Some(show_offset) = text.find("show")
            {
                for name_id in unused_show_names {
                    let name = self.ast.tokens.lexeme(self.ast[name_id].token);
                    let pat1 = format!("{name},");
                    let pat1_repl = format!("{name}, ");
                    let pat2 = format!(", {name}");
                    if text.contains(&pat1) {
                        text = replace_first_from(&text, &pat1_repl, "", show_offset);
                    } else if text.contains(&pat2) {
                        text = replace_first_from(&text, &pat2, "", show_offset);
                    }
                }
            }
            sb.push_str(&text);
            sb.push_str(self.end_of_line);
            previous_directive_text = text;
        }
        let directives_code = sb.trim_end().to_string();
        let before_directives = substring_utf16(&self.code, 0, first_directive_offset).to_string();
        let after_directives =
            substring_utf16(&self.code, last_directive_end, usize::MAX).to_string();
        self.code = format!("{before_directives}{directives_code}{after_directives}");
    }

    fn get_leading_comment(
        ast: &Ast,
        unit: Id<CompilationUnit>,
        begin_token: TokenId,
        line_info: &LineInfo,
        is_pseudo_library_directive: bool,
    ) -> Option<TokenId> {
        let mut first_comment = ast.tokens.get(begin_token).preceding_comments.get()?;
        let mut comment = Some(first_comment);
        let mut next_comment = ast.tokens.get(first_comment).next.get();

        while is_pseudo_library_directive && comment.is_some() && next_comment.is_some() {
            let c = comment.unwrap();
            let nc = next_comment.unwrap();
            if line_info.line_number_difference(ast.tokens.offset(c), ast.tokens.offset(nc)) > 1 {
                first_comment = nc;
            }
            comment = Some(nc);
            next_comment = ast.tokens.get(nc).next.get();
        }

        let mut fc_opt = Some(first_comment);
        if let Some(fc) = fc_opt
            && ast.tokens.get(fc).flags & flags::LANGUAGE_VERSION != 0
        {
            fc_opt = ast.tokens.get(fc).next.get();
        }

        let unit_first_comment = ast
            .tokens
            .get(ast[unit].begin_token)
            .preceding_comments
            .get();
        if let Some(fc) = fc_opt
            && Some(fc) == unit_first_comment
        {
            return if is_ignore_comment(ast.tokens.lexeme(fc)) {
                Some(fc)
            } else {
                None
            };
        }

        comment = fc_opt;
        if is_pseudo_library_directive && let Some(c) = comment {
            if line_info
                .line_number_difference(ast.tokens.offset(begin_token), ast.tokens.offset(c))
                == -1
            {
                return Some(c);
            } else {
                return None;
            }
        }
        let prev = ast.tokens.get(begin_token).previous;
        if let Some(prev_tok) = prev.get() {
            let prev_end = ast.tokens.get(prev_tok).end();
            while let Some(c) = comment {
                if !line_info.on_same_line(prev_end, ast.tokens.offset(c)) {
                    break;
                }
                comment = ast.tokens.get(c).next.get();
            }
        }
        comment
    }

    fn get_trailing_comment(ast: &Ast, directive: NodeId, line_info: &LineInfo) -> Option<TokenId> {
        let end_token = ast.end_token(directive);
        let next_tok = ast.tokens.get(end_token).next.get()?;
        let mut comment = ast.tokens.get(next_tok).preceding_comments.get();
        let directive_end = ast.end(directive);
        while let Some(c) = comment {
            if line_info.on_same_line(ast.tokens.offset(c), directive_end) {
                return Some(c);
            }
            comment = ast.tokens.get(c).next.get();
        }
        None
    }
}

fn code_eq(a: &'static DiagnosticCode, b: &'static DiagnosticCode) -> bool {
    std::ptr::eq(a, b) || a.unique_name == b.unique_name
}

fn is_ignore_comment(s: &str) -> bool {
    let bytes = s.as_bytes();
    if bytes.len() < 9 || bytes[0] != b'/' || bytes[1] != b'/' {
        return false;
    }
    let mut from = 2;
    while from < bytes.len() && bytes[from] == b'/' {
        from += 1;
    }
    while from < bytes.len() && bytes[from] == b' ' {
        from += 1;
    }
    s[from..].starts_with("ignore:")
}

fn replace_first_from(text: &str, from: &str, to: &str, start_index: usize) -> String {
    if start_index > text.len() {
        return text.to_string();
    }
    if let Some(pos) = text[start_index..].find(from) {
        let abs = start_index + pos;
        let mut out = String::with_capacity(text.len() - from.len() + to.len());
        out.push_str(&text[..abs]);
        out.push_str(to);
        out.push_str(&text[abs + from.len()..]);
        out
    } else {
        text.to_string()
    }
}

fn cmp_utf16(a: &str, b: &str) -> Ordering {
    a.encode_utf16().cmp(b.encode_utf16())
}

fn find_common_prefix(a: &[u16], b: &[u16]) -> usize {
    let n = a.len().min(b.len());
    for i in 0..n {
        if a[i] != b[i] {
            return i;
        }
    }
    n
}

fn find_common_suffix_u16(a: &[u16], b: &[u16]) -> usize {
    let a_len = a.len();
    let b_len = b.len();
    let n = a_len.min(b_len);
    for i in 1..=n {
        if a[a_len - i] != b[b_len - i] {
            return i - 1;
        }
    }
    n
}

fn find_common_suffix(a: &str, b: &str) -> usize {
    let au: Vec<u16> = a.encode_utf16().collect();
    let bu: Vec<u16> = b.encode_utf16().collect();
    find_common_suffix_u16(&au, &bu)
}

fn compute_simple_diff(old_str: &str, new_str: &str) -> (usize, usize, String) {
    let old_u: Vec<u16> = old_str.encode_utf16().collect();
    let new_u: Vec<u16> = new_str.encode_utf16().collect();
    let mut prefix_length = find_common_prefix(&old_u, &new_u) as isize;
    let suffix_length = find_common_suffix_u16(&old_u, &new_u) as isize;
    while prefix_length >= 0 {
        let old_replace_length = old_u.len() as isize - prefix_length - suffix_length;
        let new_replace_length = new_u.len() as isize - prefix_length - suffix_length;
        if old_replace_length >= 0 && new_replace_length >= 0 {
            let p = prefix_length as usize;
            let end = new_u.len() - suffix_length as usize;
            return (
                p,
                old_replace_length as usize,
                String::from_utf16_lossy(&new_u[p..end]),
            );
        }
        prefix_length -= 1;
    }
    (0, old_u.len(), new_str.to_string())
}

pub struct LegacyWorkspace<'s> {
    pub session: &'s mut dartr_cli::DriverSession,
    pub collection: &'s dartr_project::AnalysisContextCollection,
}

impl dartr_server::correction::change_builder::ChangeWorkspace for LegacyWorkspace<'_> {
    fn resolved_unit(&mut self, path: &str) -> Option<dartr_server::server::ResolvedUnitRef> {
        if !path.ends_with(".dart") || self.collection.context_for(path).is_none() {
            return None;
        }
        let library = self.session.resolved_library(self.collection, path)?;
        let index = library.unit_index(path)?;
        Some(dartr_server::server::ResolvedUnitRef { library, index })
    }

    fn analysis_options(&self, path: &str) -> std::rc::Rc<dartr_project::AnalysisOptions> {
        match self.collection.context_for(path) {
            Some(context) => self.collection.options_for(context, path).clone(),
            None => std::rc::Rc::new(dartr_project::AnalysisOptions::default()),
        }
    }

    fn content(&self, path: &str) -> Option<String> {
        dartr_project::fs::read_string(path)
    }

    fn fix_data_files(&self, path: &str) -> Vec<(String, Option<String>)> {
        let Some(context) = self.collection.context_for(path) else {
            return Vec::new();
        };
        let mut out = Vec::new();
        for package in context.packages.packages() {
            out.push((
                format!("{}/fix_data.yaml", package.lib),
                Some(package.name.clone()),
            ));
            let mut files = Vec::new();
            yaml_files_recursively(&format!("{}/fix_data", package.lib), &mut files);
            out.extend(files.into_iter().map(|f| (f, Some(package.name.clone()))));
        }
        if let Some(sdk) = context.sdk.as_ref().or(self.collection.sdk.as_ref()) {
            out.push((format!("{}/_internal/fix_data.yaml", sdk.lib_path()), None));
        }
        out
    }

    fn top_level_declarations(
        &mut self,
        path: &str,
        name: &str,
    ) -> Vec<dartr_server::correction::change_builder::TopLevelDeclaration> {
        use dartr_server::correction::change_builder::TopLevelDeclaration;
        use dartr_server::correction::producers::import_library::{element_kind, exported_element};

        let collection = self.collection;
        let Some(context) = collection.context_for(path) else {
            return Vec::new();
        };
        let index = collection
            .contexts
            .iter()
            .position(|x| std::ptr::eq(x, context))
            .unwrap_or(0);
        let mut candidates = self.session.known_files(index);
        candidates.extend(
            context
                .root
                .analyzed_files()
                .into_iter()
                .filter(|f| f.ends_with(".dart")),
        );
        if let Some(sdk) = context.sdk.as_ref() {
            candidates.extend(
                sdk.libraries()
                    .iter()
                    .filter(|l| !l.is_internal())
                    .filter_map(|l| sdk.map_dart_uri(&l.short_name)),
            );
        }
        let mut seen = std::collections::HashSet::new();
        let mut result = Vec::new();
        for candidate in candidates {
            if !seen.insert(candidate.clone()) {
                continue;
            }
            let Some(linked) = self
                .session
                .linked_library_in(collection, index, &candidate)
            else {
                continue;
            };
            if linked.library_path != candidate {
                continue;
            }
            let sink = dartr_element::NoopSink;
            let features = dartr_element::FeatureSet::new(Vec::<std::sync::Arc<str>>::new());
            let ctx = linked.ctx(&sink, &features);
            let Some(library) = ctx.library_by_uri(&linked.uri) else {
                continue;
            };
            let element = exported_element(&ctx, library, name)
                .or_else(|| exported_element(&ctx, library, &format!("{name}=")));
            let Some(mut element) = element else { continue };
            if matches!(
                element.tag(),
                dartr_element::Tag::Getter | dartr_element::Tag::Setter
            ) && let Some(v) =
                dartr_resolver::element_metadata::accessor_variable_any(&ctx, element)
            {
                element = v;
            }
            let declared_in_library =
                dartr_resolver::error::support::library_of(&ctx, element) == Some(library);
            result.push(TopLevelDeclaration {
                library_uri: linked.uri.clone(),
                library_path: linked.library_path.clone(),
                kind: element_kind(element),
                declared_in_library,
            });
        }
        result
    }
}

fn yaml_files_recursively(folder: &str, out: &mut Vec<String>) {
    let Some(children) = dartr_project::fs::children(folder) else {
        return;
    };
    for child in children {
        match child.kind {
            dartr_project::fs::ResourceKind::File => {
                if child.path.ends_with(".yaml") {
                    out.push(child.path);
                }
            }
            dartr_project::fs::ResourceKind::Folder => yaml_files_recursively(&child.path, out),
        }
    }
}

pub fn to_protocol_source_change(
    change: dartr_server::correction::change::SourceChange,
) -> protocol::SourceChange {
    use dartr_server::correction::change::LinkedEditSuggestionKind as K;
    let edits = change
        .edits
        .into_iter()
        .map(|fe| protocol::SourceFileEdit {
            file_stamp: if dartr_project::fs::file_exists(&fe.file) {
                0
            } else {
                -1
            },
            file: fe.file,
            edits: fe
                .edits
                .into_iter()
                .map(|e| protocol::SourceEdit {
                    offset: e.offset as i64,
                    length: e.length as i64,
                    replacement: e.replacement,
                    id: None,
                    description: None,
                })
                .collect(),
        })
        .collect();
    let linked_edit_groups = change
        .linked_edit_groups
        .into_iter()
        .map(|g| protocol::LinkedEditGroup {
            positions: g
                .positions
                .into_iter()
                .map(|p| protocol::Position {
                    file: p.file,
                    offset: p.offset as i64,
                })
                .collect(),
            length: g.length as i64,
            suggestions: g
                .suggestions
                .into_iter()
                .map(|s| protocol::LinkedEditSuggestion {
                    value: s.value,
                    kind: match s.kind {
                        K::Method => protocol::LinkedEditSuggestionKind::METHOD,
                        K::Parameter => protocol::LinkedEditSuggestionKind::PARAMETER,
                        K::Type => protocol::LinkedEditSuggestionKind::TYPE,
                        K::Variable => protocol::LinkedEditSuggestionKind::VARIABLE,
                    },
                })
                .collect(),
        })
        .collect();
    protocol::SourceChange {
        message: change.message,
        edits,
        linked_edit_groups,
        selection: change.selection.map(|p| protocol::Position {
            file: p.file,
            offset: p.offset as i64,
        }),
        selection_length: change.selection_length.map(|l| l as i64),
        id: change.id,
    }
}

pub fn compute_import_elements(
    workspace: &mut dyn dartr_server::correction::change_builder::ChangeWorkspace,
    resolved: &crate::search::ResolvedUnitRef,
    file: &str,
    elements: &[protocol::ImportedElements],
) -> Option<protocol::SourceFileEdit> {
    if elements.is_empty() {
        return None;
    }
    let sink = dartr_element::NoopSink;
    let ctx = resolved.ctx(&sink);
    let unit = resolved.unit();
    let eol = dartr_server::correction::change_builder::end_of_line(&unit.ast.tokens.source);
    let mut builder = dartr_server::correction::change_builder::ChangeBuilder::new(workspace, eol);

    let ok = builder.add_dart_file_edit(file, |file_builder| {
        for imported in elements {
            let uri = ctx
                .world
                .libraries
                .iter()
                .find_map(|(u, &lib_id)| {
                    let first = ctx.get(lib_id).first_fragment();
                    if ctx.fragment(first).source.path.as_ref() == imported.path {
                        Some(u.to_string())
                    } else {
                        None
                    }
                })
                .unwrap_or_else(|| format!("file://{}", imported.path));
            let prefix = if imported.prefix.is_empty() {
                None
            } else {
                Some(imported.prefix.as_str())
            };
            if imported.elements.is_empty() {
                file_builder.import_library_element(&uri, prefix, None, false);
            } else {
                for elem_name in &imported.elements {
                    file_builder.import_library_element(
                        &uri,
                        prefix,
                        Some(elem_name.as_str()),
                        false,
                    );
                }
            }
        }
    });
    if !ok {
        return None;
    }
    let change = to_protocol_source_change(builder.source_change());
    change.edits.into_iter().find(|fe| !fe.edits.is_empty())
}

// Dart source: pkg/analyzer/lib/src/dart/analysis/index.dart
// Dart source: sdk/lib/internal/sort.dart (Sort, the List.sort of the VM)

//! The search index of a resolved unit (Dart `indexUnit`): the relations
//! between the unit and the elements it uses, in the order of Dart's
//! `AnalysisDriverUnitIndex` (the relations sorted by element id with the
//! unstable `List.sort` of the Dart VM, which is ported here, because the
//! order of the references that the server returns comes from it).

use dartr_ast::*;
use dartr_element::{Ctx, ElemRef, ElementId, FragmentFlags, ResolutionTables, Tag, TypeKind};
use dartr_resolver::error::support;
use dartr_syntax::TokenId;
use dartr_typesystem::member;

/// Dart `IndexRelationKind` (the kinds that dartr records).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum RelationKind {
    IsExtendedBy,
    IsImplementedBy,
    IsMixedInBy,
    Constrains,
    IsInvokedBy,
    IsInvokedByDotShorthandsConstructor,
    IsInvokedByEnumConstantWithoutArguments,
    IsReadBy,
    IsReadWrittenBy,
    IsReferencedBy,
    IsReferencedByConstructorTearOff,
    IsReferencedByDotShorthandConstructorTearOff,
    IsReferencedByNamedArgument,
    IsReferencedByPatternField,
    IsWrittenBy,
}

/// Dart `IndexSyntheticElementKind`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum SyntheticKind {
    NotSynthetic,
    Getter,
    Setter,
}

/// The identity of an element in an index (Dart `_ElementInfo` without the
/// id): the unit of its first fragment, its name components and kind.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct ElementKey {
    pub library_path: String,
    pub unit_path: String,
    pub unit_member: Option<String>,
    pub class_member: Option<String>,
    pub parameter: Option<String>,
    pub kind: SyntheticKind,
}

/// One relation: the index of the element in [UnitIndex::elements], the
/// kind, the offset and length, and whether it is qualified.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Relation {
    pub element: usize,
    pub kind: RelationKind,
    pub offset: u32,
    pub length: u32,
}

/// Dart `AnalysisDriverUnitIndex` (the element relations).
#[derive(Debug, Default)]
pub struct UnitIndex {
    /// The elements in id order.
    pub elements: Vec<ElementKey>,
    /// The relations sorted by element id (Dart order).
    pub relations: Vec<Relation>,
}

impl UnitIndex {
    /// Dart `_IndexRequest.findElementId` + `getRelations`: the relations of
    /// [key] with a kind accepted by [accept], in index order.
    pub fn relations_of(&self, key: &ElementKey, accept: impl Fn(RelationKind) -> bool) -> Vec<&Relation> {
        let Some(id) = self.elements.iter().position(|e| e == key) else {
            return Vec::new();
        };
        self.relations
            .iter()
            .filter(|r| r.element == id && accept(r.kind))
            .collect()
    }
}

// ---- keys ----

/// The path of the library fragment of [element]'s first fragment and of
/// its library's first fragment.
fn unit_paths(ctx: &Ctx<'_>, element: ElementId) -> Option<(String, String)> {
    let first = ctx.element_data(element)?.first_fragment;
    let unit = crate::navigation::library_fragment_of(ctx, first)?;
    let unit_path = crate::navigation::fragment_path(ctx, unit)?;
    let library = ctx
        .fragment_data(unit)?
        .element
        .try_get()
        .copied()
        .and_then(|l| l.cast::<dartr_element::LibraryElement>())?;
    let library_first = ctx.get(library).first_fragment();
    let library_path = ctx.fragment(library_first).source.path.to_string();
    Some((library_path, unit_path))
}

fn lookup_name(ctx: &Ctx<'_>, e: ElementId) -> Option<String> {
    member::lookup_name(ctx, ElemRef::Base(e))
}

/// Dart `IndexElementInfo` + `ElementNameComponents` + `getUnitElement`.
pub fn element_key(ctx: &Ctx<'_>, element: ElementId) -> Option<ElementKey> {
    let mut element = element;
    let mut kind = SyntheticKind::NotSynthetic;
    if matches!(element.tag(), Tag::Getter | Tag::Setter) {
        kind = if element.tag() == Tag::Getter {
            SyntheticKind::Getter
        } else {
            SyntheticKind::Setter
        };
        element = dartr_resolver::element_metadata::accessor_variable_any(ctx, element)?;
    }
    let (library_path, unit_path) = unit_paths(ctx, element)?;
    let mut e = element;
    let mut parameter = None;
    if matches!(
        e.tag(),
        Tag::FormalParameter | Tag::FieldFormalParameter | Tag::SuperFormalParameter
    ) {
        let first = ctx.element_data(e)?.first_fragment;
        let data = ctx.fragment_data(first)?;
        parameter = data.name.map(|n| ctx.name_str(n).to_string());
        e = ctx
            .fragment_data(data.enclosing_fragment?)?
            .element
            .try_get()
            .copied()?;
    }
    let mut class_member = None;
    let enclosing = ctx.element_data(e).and_then(|d| d.enclosing);
    if let Some(enc) = enclosing
        && (enc.cast::<dartr_element::InterfaceElement>().is_some() || enc.tag() == Tag::Extension)
    {
        if let Some(mut name) = lookup_name(ctx, e) {
            if e.tag() == Tag::Constructor {
                name = format!(".{name}");
            }
            class_member = Some(name);
        }
        e = enc;
    }
    let mut unit_member = None;
    let first = ctx.element_data(e).map(|d| d.first_fragment);
    let enclosing_fragment = first
        .and_then(|f| ctx.fragment_data(f))
        .and_then(|f| f.enclosing_fragment);
    if enclosing_fragment.is_some_and(|f| f.tag() == Tag::Library) {
        unit_member = lookup_name(ctx, e);
        if e.tag() == Tag::Extension && unit_member.is_none() {
            let library = support::library_of(ctx, e)?;
            let index = library_extensions(ctx, library).iter().position(|x| *x == e)?;
            unit_member = Some(format!("extension-{index}"));
        }
    }
    Some(ElementKey {
        library_path,
        unit_path,
        unit_member,
        class_member,
        parameter,
        kind,
    })
}

/// Dart `LibraryElement.extensions` (of all units, in order).
fn library_extensions(ctx: &Ctx<'_>, library: dartr_element::EId<dartr_element::LibraryElement>) -> Vec<ElementId> {
    let mut out = Vec::new();
    let first = ctx.get(library).first_fragment();
    let mut stack = vec![first];
    while let Some(f) = stack.pop() {
        let data = ctx.fragment(f);
        for e in &data.extensions {
            if let Some(el) = ctx.fragment_data(e.raw()).and_then(|d| d.element.try_get().copied())
                && !out.contains(&el)
            {
                out.push(el);
            }
        }
        for part in data.parts.iter().rev() {
            if let dartr_element::DirectiveUri::Unit { library_fragment, .. } = &part.directive.uri {
                stack.push(*library_fragment);
            }
        }
    }
    out
}

// ---- Dart List.sort ----

/// Dart `Sort.sort(list, compare)` (dual-pivot quicksort, insertion sort
/// below 33 elements): not stable, so it is ported to get the same order.
pub fn dart_sort<T: Clone>(a: &mut [T], compare: &dyn Fn(&T, &T) -> i64) {
    if a.is_empty() {
        return;
    }
    do_sort(a, 0, a.len() as isize - 1, compare);
}

fn do_sort<T: Clone>(a: &mut [T], left: isize, right: isize, compare: &dyn Fn(&T, &T) -> i64) {
    if right - left <= 32 {
        insertion_sort(a, left, right, compare);
    } else {
        dual_pivot_quicksort(a, left, right, compare);
    }
}

fn insertion_sort<T: Clone>(a: &mut [T], left: isize, right: isize, compare: &dyn Fn(&T, &T) -> i64) {
    let mut i = left + 1;
    while i <= right {
        let el = a[i as usize].clone();
        let mut j = i;
        while j > left && compare(&a[(j - 1) as usize], &el) > 0 {
            a[j as usize] = a[(j - 1) as usize].clone();
            j -= 1;
        }
        a[j as usize] = el;
        i += 1;
    }
}

fn dual_pivot_quicksort<T: Clone>(a: &mut [T], left: isize, right: isize, compare: &dyn Fn(&T, &T) -> i64) {
    let u = |i: isize| i as usize;
    let sixth = (right - left + 1) / 6;
    let index1 = left + sixth;
    let index5 = right - sixth;
    let index3 = (left + right) / 2;
    let index2 = index3 - sixth;
    let index4 = index3 + sixth;

    let mut el1 = a[u(index1)].clone();
    let mut el2 = a[u(index2)].clone();
    let mut el3 = a[u(index3)].clone();
    let mut el4 = a[u(index4)].clone();
    let mut el5 = a[u(index5)].clone();

    if compare(&el1, &el2) > 0 {
        std::mem::swap(&mut el1, &mut el2);
    }
    if compare(&el4, &el5) > 0 {
        std::mem::swap(&mut el4, &mut el5);
    }
    if compare(&el1, &el3) > 0 {
        std::mem::swap(&mut el1, &mut el3);
    }
    if compare(&el2, &el3) > 0 {
        std::mem::swap(&mut el2, &mut el3);
    }
    if compare(&el1, &el4) > 0 {
        std::mem::swap(&mut el1, &mut el4);
    }
    if compare(&el3, &el4) > 0 {
        std::mem::swap(&mut el3, &mut el4);
    }
    if compare(&el2, &el5) > 0 {
        std::mem::swap(&mut el2, &mut el5);
    }
    if compare(&el2, &el3) > 0 {
        std::mem::swap(&mut el2, &mut el3);
    }
    if compare(&el4, &el5) > 0 {
        std::mem::swap(&mut el4, &mut el5);
    }

    let pivot1 = el2;
    let pivot2 = el4;

    a[u(index1)] = el1;
    a[u(index3)] = el3;
    a[u(index5)] = el5;

    a[u(index2)] = a[u(left)].clone();
    a[u(index4)] = a[u(right)].clone();

    let mut less = left + 1;
    let mut great = right - 1;

    let pivots_are_equal = compare(&pivot1, &pivot2) == 0;
    if pivots_are_equal {
        let pivot = pivot1.clone();
        let mut k = less;
        while k <= great {
            let ak = a[u(k)].clone();
            let mut comp = compare(&ak, &pivot);
            if comp == 0 {
                k += 1;
                continue;
            }
            if comp < 0 {
                if k != less {
                    a[u(k)] = a[u(less)].clone();
                    a[u(less)] = ak;
                }
                less += 1;
            } else {
                loop {
                    comp = compare(&a[u(great)], &pivot);
                    if comp > 0 {
                        great -= 1;
                        continue;
                    } else if comp < 0 {
                        a[u(k)] = a[u(less)].clone();
                        a[u(less)] = a[u(great)].clone();
                        less += 1;
                        a[u(great)] = ak;
                        great -= 1;
                        break;
                    } else {
                        a[u(k)] = a[u(great)].clone();
                        a[u(great)] = ak;
                        great -= 1;
                        break;
                    }
                }
            }
            k += 1;
        }
    } else {
        let mut k = less;
        while k <= great {
            let ak = a[u(k)].clone();
            let comp_pivot1 = compare(&ak, &pivot1);
            if comp_pivot1 < 0 {
                if k != less {
                    a[u(k)] = a[u(less)].clone();
                    a[u(less)] = ak;
                }
                less += 1;
            } else {
                let comp_pivot2 = compare(&ak, &pivot2);
                if comp_pivot2 > 0 {
                    loop {
                        let comp = compare(&a[u(great)], &pivot2);
                        if comp > 0 {
                            great -= 1;
                            if great < k {
                                break;
                            }
                            continue;
                        } else {
                            let comp = compare(&a[u(great)], &pivot1);
                            if comp < 0 {
                                a[u(k)] = a[u(less)].clone();
                                a[u(less)] = a[u(great)].clone();
                                less += 1;
                                a[u(great)] = ak;
                                great -= 1;
                            } else {
                                a[u(k)] = a[u(great)].clone();
                                a[u(great)] = ak;
                                great -= 1;
                            }
                            break;
                        }
                    }
                }
            }
            k += 1;
        }
    }

    a[u(left)] = a[u(less - 1)].clone();
    a[u(less - 1)] = pivot1.clone();
    a[u(right)] = a[u(great + 1)].clone();
    a[u(great + 1)] = pivot2.clone();

    do_sort(a, left, less - 2, compare);
    do_sort(a, great + 2, right, compare);

    if pivots_are_equal {
        return;
    }

    if less < index1 && great > index5 {
        while compare(&a[u(less)], &pivot1) == 0 {
            less += 1;
        }
        while compare(&a[u(great)], &pivot2) == 0 {
            great -= 1;
        }
        let mut k = less;
        while k <= great {
            let ak = a[u(k)].clone();
            let comp_pivot1 = compare(&ak, &pivot1);
            if comp_pivot1 == 0 {
                if k != less {
                    a[u(k)] = a[u(less)].clone();
                    a[u(less)] = ak;
                }
                less += 1;
            } else {
                let comp_pivot2 = compare(&ak, &pivot2);
                if comp_pivot2 == 0 {
                    loop {
                        let comp = compare(&a[u(great)], &pivot2);
                        if comp == 0 {
                            great -= 1;
                            if great < k {
                                break;
                            }
                            continue;
                        } else {
                            let comp = compare(&a[u(great)], &pivot1);
                            if comp < 0 {
                                a[u(k)] = a[u(less)].clone();
                                a[u(less)] = a[u(great)].clone();
                                less += 1;
                                a[u(great)] = ak;
                                great -= 1;
                            } else {
                                a[u(k)] = a[u(great)].clone();
                                a[u(great)] = ak;
                                great -= 1;
                            }
                            break;
                        }
                    }
                }
            }
            k += 1;
        }
        do_sort(a, less, great, compare);
    } else {
        do_sort(a, less, great, compare);
    }
}

// ---- the contributor ----

/// Dart `indexUnit(unit)`.
pub fn index_unit(ctx: &Ctx<'_>, ast: &Ast, tables: &ResolutionTables, unit: Id<CompilationUnit>) -> UnitIndex {
    let mut c = Contributor {
        ctx,
        ast,
        tables,
        element_map: Vec::new(),
        infos: Vec::new(),
        relations: Vec::new(),
    };
    ast.accept(unit, &mut c);
    // Dart `assemble`: element infos sorted by their name ids (string
    // order), then the relations by element id.
    const NULL: &str = "--nullString--";
    let infos = std::mem::take(&mut c.infos);
    let mut order: Vec<usize> = (0..infos.len()).collect();
    let name = |s: &Option<String>| -> Vec<u16> { s.as_deref().unwrap_or(NULL).encode_utf16().collect() };
    let keys: Vec<(Vec<u16>, Vec<u16>, Vec<u16>)> = infos
        .iter()
        .map(|k| (name(&k.unit_member), name(&k.class_member), name(&k.parameter)))
        .collect();
    let cmp = |a: &usize, b: &usize| -> i64 {
        let (ka, kb) = (&keys[*a], &keys[*b]);
        let o = ka.0.cmp(&kb.0).then(ka.1.cmp(&kb.1)).then(ka.2.cmp(&kb.2));
        o as i64
    };
    dart_sort(&mut order, &cmp);
    let mut id_of = vec![0usize; infos.len()];
    for (id, &info) in order.iter().enumerate() {
        id_of[info] = id;
    }
    let mut relations: Vec<Relation> = c
        .relations
        .into_iter()
        .map(|mut r| {
            r.element = id_of[r.element];
            r
        })
        .collect();
    dart_sort(&mut relations, &|a: &Relation, b: &Relation| a.element as i64 - b.element as i64);
    UnitIndex {
        elements: order.into_iter().map(|i| infos[i].clone()).collect(),
        relations,
    }
}

struct Contributor<'c, 'a> {
    ctx: &'c Ctx<'a>,
    ast: &'c Ast,
    tables: &'c ResolutionTables,
    /// Dart `elementMap` keys (base elements) with their info index.
    element_map: Vec<(ElementId, usize)>,
    infos: Vec<ElementKey>,
    relations: Vec<Relation>,
}

impl Contributor<'_, '_> {
    fn base(&self, e: ElemRef) -> ElementId {
        member::base_element(self.ctx, e)
    }

    fn element(&self, node: impl Into<NodeId>) -> Option<ElementId> {
        self.tables.element.get(node.into()).map(|&e| self.base(e))
    }

    fn declared(&self, node: impl Into<NodeId>) -> Option<ElementId> {
        support::declared_element(self.ctx, self.tables, node)
    }

    /// Dart `_IndexAssembler._getElementInfo`.
    fn element_info(&mut self, element: ElementId) -> Option<usize> {
        if let Some((_, i)) = self.element_map.iter().find(|(e, _)| *e == element) {
            return Some(*i);
        }
        let key = element_key(self.ctx, element)?;
        let index = self.infos.len();
        self.infos.push(key);
        self.element_map.push((element, index));
        Some(index)
    }

    /// Dart `addPrefixForElement`: creates the element info.
    fn add_prefix_for_element(&mut self, element: ElementId) {
        if matches!(element.tag(), Tag::MultiplyDefined | Tag::Dynamic | Tag::Never) {
            return;
        }
        self.element_info(element);
    }

    /// Dart `recordRelationOffset`.
    fn record_offset(&mut self, element: Option<ElementId>, kind: RelationKind, offset: u32, length: u32) {
        let Some(element) = element else { return };
        match element.tag() {
            Tag::Dynamic
            | Tag::Never
            | Tag::MultiplyDefined
            | Tag::Label
            | Tag::LocalVariable
            | Tag::PatternVariable
            | Tag::BindPatternVariable
            | Tag::JoinPatternVariable
            | Tag::Prefix
            | Tag::TypeParameter
            | Tag::LocalFunction => return,
            _ => {}
        }
        if matches!(
            element.tag(),
            Tag::FormalParameter | Tag::FieldFormalParameter | Tag::SuperFormalParameter
        ) {
            let enclosing = self.ctx.element_data(element).and_then(|d| d.enclosing);
            match enclosing {
                None => return,
                Some(e) if matches!(e.tag(), Tag::LocalFunction | Tag::GenericFunctionType) => return,
                _ => {}
            }
        }
        let Some(info) = self.element_info(element) else {
            return;
        };
        self.relations.push(Relation {
            element: info,
            kind,
            offset,
            length,
        });
    }

    fn record_token(&mut self, element: Option<ElementId>, kind: RelationKind, token: TokenId) {
        let t = self.ast.tokens.get(token);
        self.record_offset(element, kind, t.offset, t.end() - t.offset);
    }

    fn record_node(&mut self, element: Option<ElementId>, kind: RelationKind, node: NodeId) {
        let (o, l) = (self.ast.offset(node), self.ast.length(node));
        self.record_offset(element, kind, o, l);
    }

    /// Dart `recordSuperType`.
    fn record_super_type(&mut self, named_type: Id<NamedType>, kind: RelationKind) {
        let element = self.element(named_type);
        let name = self.ast[named_type].name;
        self.record_token(element, kind, name);
    }

    /// Dart `_recordImportPrefixedElement`.
    fn record_import_prefixed(&mut self, import_prefix: Option<Id<ImportPrefixReference>>, name: TokenId, element: Option<ElementId>) {
        let Some(element) = element else { return };
        if let Some(prefix) = import_prefix {
            if let Some(p) = self.element(prefix)
                && p.tag() == Tag::Prefix
            {
                // The prefix relation is filtered (PREFIX).
                self.add_prefix_for_element(element);
            }
        } else {
            self.add_prefix_for_element(element);
        }
        self.record_token(Some(element), RelationKind::IsReferencedBy, name);
    }

    /// Dart `_getActualConstructorElement`: through mixin application
    /// constructors to the forwarded constructor.
    fn actual_constructor(&self, constructor: Option<ElementId>) -> Option<ElementId> {
        let mut c = constructor?;
        let mut seen = Vec::new();
        loop {
            if c.tag() != Tag::Constructor {
                return Some(c);
            }
            let flags = dartr_resolver::element_ext::first_fragment_flags(self.ctx, c);
            if !flags.contains(FragmentFlags::CONSTRUCTOR_FRAGMENT_IS_ORIGIN_MIXIN_APPLICATION) {
                return Some(c);
            }
            let next = self
                .ctx
                .get(c.cast::<dartr_element::ConstructorElement>()?)
                .super_constructor
                .get()
                .map(|s| self.base(s));
            match next {
                Some(n) => {
                    if seen.contains(&n) {
                        return None;
                    }
                    seen.push(n);
                    c = n;
                }
                None => return Some(c),
            }
        }
    }

    fn super_constructor_of(&self, constructor: ElementId) -> Option<ElementId> {
        let c = constructor.cast::<dartr_element::ConstructorElement>()?;
        self.ctx.get(c).super_constructor.get().map(|s| self.base(s))
    }

    /// Dart `SimpleIdentifier.isQualified` (and `Combinator`, `Label`).
    fn is_qualified(&self, node: Id<SimpleIdentifier>) -> bool {
        let ast = self.ast;
        let Some(parent) = ast.parent(node) else { return false };
        if let Some(p) = ast.cast::<PrefixedIdentifier>(parent) {
            return ast[p].identifier == node;
        }
        if let Some(p) = ast.cast::<PropertyAccess>(parent) {
            return ast[p].property_name == node;
        }
        if let Some(p) = ast.cast::<ConstructorName>(parent) {
            return ast[p].name == Some(node);
        }
        if let Some(m) = ast.cast::<MethodInvocation>(parent) {
            return ast[m].method_name == node && dartr_resolver::ast_ext::method_invocation_real_target(ast, m).is_some();
        }
        ast.is::<Combinator>(parent) || ast.is::<Label>(parent)
    }
}

impl AstVisitor for Contributor<'_, '_> {
    fn visit_annotation(&mut self, ast: &Ast, node: Id<Annotation>) {
        if let Some(element) = self.element(node)
            && element.tag() == Tag::Constructor
        {
            let base = self.actual_constructor(Some(element));
            let name = ast[node].name;
            let identifier_is_constructor = ast
                .cast::<PrefixedIdentifier>(name)
                .is_some_and(|p| self.element(ast[p].identifier).is_some_and(|e| e.tag() == Tag::Constructor));
            if let Some(constructor_name) = ast[node].constructor_name {
                let offset = ast.tokens.get(ast[node].period.expect("period")).offset;
                self.record_offset(base, RelationKind::IsInvokedBy, offset, ast.end(constructor_name) - offset);
            } else if identifier_is_constructor {
                let p = ast.cast::<PrefixedIdentifier>(name).unwrap();
                let offset = ast.tokens.get(ast[p].period).offset;
                self.record_offset(base, RelationKind::IsInvokedBy, offset, ast.end(name) - offset);
            } else {
                let offset = ast[node]
                    .type_arguments
                    .map(|t| ast.end(t))
                    .unwrap_or_else(|| ast.end(name));
                self.record_offset(base, RelationKind::IsInvokedBy, offset, 0);
            }
            if identifier_is_constructor {
                let p = ast.cast::<PrefixedIdentifier>(name).unwrap();
                ast.accept(ast[p].prefix, self);
            } else {
                ast.accept(name, self);
            }
            if let Some(t) = ast[node].type_arguments {
                ast.accept(t, self);
            }
            if let Some(a) = ast[node].arguments {
                ast.accept(a, self);
            }
            return;
        }
        ast.visit_children(node, self);
    }

    fn visit_assigned_variable_pattern(&mut self, ast: &Ast, node: Id<AssignedVariablePattern>) {
        let element = self.element(node);
        self.record_token(element, RelationKind::IsWrittenBy, ast[node].name);
        ast.visit_children(node, self);
    }

    fn visit_assignment_expression(&mut self, ast: &Ast, node: Id<AssignmentExpression>) {
        let element = self.element(node);
        self.record_token(element, RelationKind::IsInvokedBy, ast[node].operator);
        ast.visit_children(node, self);
    }

    fn visit_binary_expression(&mut self, ast: &Ast, node: Id<BinaryExpression>) {
        let element = self.element(node);
        self.record_token(element, RelationKind::IsInvokedBy, ast[node].operator);
        ast.visit_children(node, self);
    }

    fn visit_class_declaration(&mut self, ast: &Ast, node: Id<ClassDeclaration>) {
        let declared = self.declared(node);
        let name_token = support::class_name_token(ast, ast[node].name_part);
        if let Some(class) = declared.and_then(|d| d.cast::<dartr_element::InterfaceElement>()) {
            if ast[node].extends_clause.is_none() {
                let object = self
                    .ctx
                    .interface(class)
                    .supertype
                    .get()
                    .and_then(|t| match self.ctx.ty(t) {
                        TypeKind::Interface { element, .. } => Some(element.raw()),
                        _ => None,
                    });
                let offset = ast.tokens.get(name_token).offset;
                self.record_offset(object, RelationKind::IsExtendedBy, offset, 0);
            }
            let constructors = &self.ctx.interface(class).constructors;
            if constructors.len() == 1 {
                let c = constructors[0].raw();
                let flags = dartr_resolver::element_ext::first_fragment_flags(self.ctx, c);
                if flags.contains(FragmentFlags::CONSTRUCTOR_FRAGMENT_IS_ORIGIN_IMPLICIT_DEFAULT)
                    && let Some(s) = self.super_constructor_of(c)
                {
                    self.record_token(Some(s), RelationKind::IsInvokedBy, name_token);
                }
            }
        }
        ast.visit_children(node, self);
    }

    fn visit_class_type_alias(&mut self, ast: &Ast, node: Id<ClassTypeAlias>) {
        self.record_super_type(ast[node].superclass, RelationKind::IsExtendedBy);
        ast.visit_children(node, self);
    }

    fn visit_comment_reference(&mut self, ast: &Ast, node: Id<CommentReference>) {
        let expression = ast[node].expression;
        if ast.is::<Identifier>(expression)
            && let Some(element) = self.element(expression)
            && element.tag() == Tag::Constructor
        {
            if let Some(p) = ast.cast::<PrefixedIdentifier>(expression) {
                let offset = ast.end(ast[p].prefix);
                self.record_offset(Some(element), RelationKind::IsReferencedBy, offset, ast.end(expression) - offset);
            } else {
                self.record_offset(Some(element), RelationKind::IsReferencedBy, ast.end(expression), 0);
            }
            return;
        }
        ast.visit_children(node, self);
    }

    fn visit_constructor_declaration(&mut self, ast: &Ast, node: Id<ConstructorDeclaration>) {
        let has_super = ast
            .list_raw(ast[node].initializers)
            .iter()
            .any(|i| ast.is::<SuperConstructorInvocation>(*i));
        if !has_super
            && let Some(element) = self.declared(node)
            && let Some(s) = self.super_constructor_of(element)
        {
            // Dart `errorRange`: the return type through the name.
            let start = ast[node]
                .type_name
                .map(|t| ast.offset(t))
                .or(ast[node].new_keyword.map(|t| ast.tokens.get(t).offset))
                .or(ast[node].factory_keyword.map(|t| ast.tokens.get(t).offset))
                .unwrap_or(0);
            let end = ast[node]
                .name
                .map(|t| ast.tokens.get(t).end())
                .or(ast[node].type_name.map(|t| ast.end(t)))
                .or(ast[node].new_keyword.map(|t| ast.tokens.get(t).end()))
                .or(ast[node].factory_keyword.map(|t| ast.tokens.get(t).end()))
                .unwrap_or(start);
            self.record_offset(Some(s), RelationKind::IsInvokedBy, start, end - start);
        }
        ast.visit_children(node, self);
    }

    fn visit_constructor_field_initializer(&mut self, ast: &Ast, node: Id<ConstructorFieldInitializer>) {
        let field_name = ast[node].field_name;
        let element = self.element(field_name);
        self.record_node(element, RelationKind::IsWrittenBy, field_name.raw());
        ast.accept(ast[node].expression, self);
    }

    fn visit_constructor_name(&mut self, ast: &Ast, node: Id<ConstructorName>) {
        let element = self.actual_constructor(self.element(node));
        let parent = ast.parent(node);
        let kind = if parent.is_some_and(|p| ast.is::<ConstructorReference>(p)) {
            RelationKind::IsReferencedByConstructorTearOff
        } else if parent.is_some_and(|p| ast.is::<InstanceCreationExpression>(p)) {
            RelationKind::IsInvokedBy
        } else {
            RelationKind::IsReferencedBy
        };
        let (offset, length) = match ast[node].name {
            Some(name) => {
                let o = ast.tokens.get(ast[node].period.expect("period")).offset;
                (o, ast.end(name) - o)
            }
            None => (ast.end(ast[node].type_), 0),
        };
        self.record_offset(element, kind, offset, length);
        ast.accept(ast[node].type_, self);
    }

    fn visit_dot_shorthand_constructor_invocation(&mut self, ast: &Ast, node: Id<DotShorthandConstructorInvocation>) {
        let element = self.actual_constructor(self.element(node).or_else(|| self.element(ast[node].constructor_name)));
        self.record_node(element, RelationKind::IsInvokedByDotShorthandsConstructor, ast[node].constructor_name.raw());
        ast.accept(ast[node].argument_list, self);
    }

    fn visit_dot_shorthand_invocation(&mut self, ast: &Ast, node: Id<DotShorthandInvocation>) {
        let name = ast[node].member_name;
        let element = self.element(name);
        self.record_node(element, RelationKind::IsInvokedBy, name.raw());
        if let Some(t) = ast[node].type_arguments {
            ast.accept(t, self);
        }
        ast.accept(ast[node].argument_list, self);
    }

    fn visit_dot_shorthand_property_access(&mut self, ast: &Ast, node: Id<DotShorthandPropertyAccess>) {
        let name = ast[node].property_name;
        let mut element = self.element(name);
        let kind = if element.is_some_and(|e| e.tag() == Tag::Constructor) {
            element = self.actual_constructor(element);
            RelationKind::IsReferencedByDotShorthandConstructorTearOff
        } else {
            RelationKind::IsReferencedBy
        };
        self.record_node(element, kind, name.raw());
    }

    fn visit_enum_constant_declaration(&mut self, ast: &Ast, node: Id<EnumConstantDeclaration>) {
        if let Some(constructor) = self.element(node) {
            let arguments = ast[node].arguments;
            let selector = arguments.and_then(|a| ast[a].constructor_selector);
            let (offset, length) = match selector {
                Some(s) => {
                    let o = ast.tokens.get(ast[s].period).offset;
                    (o, ast.end(ast[s].name) - o)
                }
                None => (ast.tokens.get(ast[node].name).end(), 0),
            };
            let kind = if arguments.is_none() {
                RelationKind::IsInvokedByEnumConstantWithoutArguments
            } else {
                RelationKind::IsInvokedBy
            };
            self.record_offset(Some(constructor), kind, offset, length);
        }
        ast.visit_children(node, self);
    }

    fn visit_extends_clause(&mut self, ast: &Ast, node: Id<ExtendsClause>) {
        let superclass = ast[node].superclass;
        self.record_super_type(superclass, RelationKind::IsExtendedBy);
        ast.accept(superclass, self);
    }

    fn visit_extension_override(&mut self, ast: &Ast, node: Id<ExtensionOverride>) {
        let element = self.element(node);
        self.record_import_prefixed(ast[node].import_prefix, ast[node].name, element);
        if let Some(t) = ast[node].type_arguments {
            ast.accept(t, self);
        }
        ast.accept(ast[node].argument_list, self);
    }

    fn visit_field_formal_parameter(&mut self, ast: &Ast, node: Id<FieldFormalParameter>) {
        if let Some(e) = self.declared(node)
            && e.tag() == Tag::FieldFormalParameter
        {
            let field = match self.ctx.any(e) {
                dartr_element::AnyElement::FormalParameter(p) => p.field.get().map(|f| f.raw()),
                _ => None,
            };
            if field.is_some() {
                self.record_token(field, RelationKind::IsWrittenBy, ast[node].name);
            }
        }
        ast.visit_children(node, self);
    }

    fn visit_implements_clause(&mut self, ast: &Ast, node: Id<ImplementsClause>) {
        for &t in ast.list(ast[node].interfaces) {
            self.record_super_type(t, RelationKind::IsImplementedBy);
            ast.accept(t, self);
        }
    }

    fn visit_index_expression(&mut self, ast: &Ast, node: Id<IndexExpression>) {
        let element = self
            .tables
            .write_element
            .get(node.raw())
            .or_else(|| self.tables.read_element.get(node.raw()))
            .or_else(|| self.tables.element.get(node.raw()))
            .map(|&e| self.base(e));
        if element.is_some_and(|e| e.tag() == Tag::Method) {
            self.record_token(element, RelationKind::IsInvokedBy, ast[node].left_bracket);
        }
        ast.visit_children(node, self);
    }

    fn visit_label_reference(&mut self, _ast: &Ast, _node: Id<LabelReference>) {
        // Labels are not indexed (LABEL).
    }

    fn visit_method_invocation(&mut self, ast: &Ast, node: Id<MethodInvocation>) {
        let name = ast[node].method_name;
        let element = self.element(name);
        let kind = if element.is_some_and(|e| e.cast::<dartr_element::InterfaceElement>().is_some()) {
            RelationKind::IsReferencedBy
        } else {
            RelationKind::IsInvokedBy
        };
        self.record_node(element, kind, name.raw());
        if let Some(t) = ast[node].target {
            ast.accept(t, self);
        }
        if let Some(t) = ast[node].type_arguments {
            ast.accept(t, self);
        }
        ast.accept(ast[node].argument_list, self);
    }

    fn visit_mixin_on_clause(&mut self, ast: &Ast, node: Id<MixinOnClause>) {
        for &t in ast.list(ast[node].superclass_constraints) {
            self.record_super_type(t, RelationKind::Constrains);
            ast.accept(t, self);
        }
    }

    fn visit_named_argument(&mut self, ast: &Ast, node: Id<NamedArgument>) {
        let parameter = support::corresponding_parameter(self.ctx, ast, self.tables, node.raw());
        let parameter = declared_named_argument_parameter(self, ast, node, parameter);
        if parameter.is_some() {
            self.record_token(parameter, RelationKind::IsReferencedByNamedArgument, ast[node].name);
        }
        ast.visit_children(node, self);
    }

    fn visit_named_type(&mut self, ast: &Ast, node: Id<NamedType>) {
        let element = self.element(node);
        self.record_import_prefixed(ast[node].import_prefix, ast[node].name, element);
        if let Some(t) = ast[node].type_arguments {
            ast.accept(t, self);
        }
    }

    fn visit_pattern_field(&mut self, ast: &Ast, node: Id<PatternField>) {
        if let Some(name_node) = ast[node].name {
            let (offset, length) = match ast[name_node].name {
                Some(t) => {
                    let t = ast.tokens.get(t);
                    (t.offset, t.end() - t.offset)
                }
                None => (ast.offset(name_node), 0),
            };
            let element = self.element(node);
            self.record_offset(element, RelationKind::IsReferencedByPatternField, offset, length);
        }
        ast.visit_children(node, self);
    }

    fn visit_postfix_expression(&mut self, ast: &Ast, node: Id<PostfixExpression>) {
        let element = self.element(node);
        self.record_token(element, RelationKind::IsInvokedBy, ast[node].operator);
        ast.visit_children(node, self);
    }

    fn visit_prefix_expression(&mut self, ast: &Ast, node: Id<PrefixExpression>) {
        let element = self.element(node);
        self.record_token(element, RelationKind::IsInvokedBy, ast[node].operator);
        ast.visit_children(node, self);
    }

    fn visit_prefixed_identifier(&mut self, ast: &Ast, node: Id<PrefixedIdentifier>) {
        let element = self.element(node);
        let prefix = self.element(ast[node].prefix);
        if let Some(e) = element
            && prefix.is_some_and(|p| p.tag() == Tag::Prefix)
        {
            self.add_prefix_for_element(e);
        }
        ast.visit_children(node, self);
    }

    fn visit_redirecting_constructor_invocation(&mut self, ast: &Ast, node: Id<RedirectingConstructorInvocation>) {
        let element = self.element(node);
        match ast[node].constructor_name {
            Some(name) => {
                let o = ast.tokens.get(ast[node].period.expect("period")).offset;
                self.record_offset(element, RelationKind::IsInvokedBy, o, ast.end(name) - o);
            }
            None => {
                let o = ast.tokens.get(ast[node].this_keyword).end();
                self.record_offset(element, RelationKind::IsInvokedBy, o, 0);
            }
        }
        ast.accept(ast[node].argument_list, self);
    }

    fn visit_simple_identifier(&mut self, ast: &Ast, node: Id<SimpleIdentifier>) {
        if support::in_declaration_context(ast, node) {
            return;
        }
        let element = support::write_or_read_element(self.ctx, ast, self.tables, node)
            .or_else(|| support::read_element(self.ctx, self.tables, node));
        let parent = ast.parent(node);
        if let Some(e) = element {
            let top_level = self
                .ctx
                .element_data(e)
                .and_then(|d| self.ctx.fragment_data(d.first_fragment))
                .and_then(|f| f.enclosing_fragment)
                .is_some_and(|f| f.tag() == Tag::Library);
            let prefixed_identifier = parent.and_then(|p| ast.cast::<PrefixedIdentifier>(p));
            if top_level && prefixed_identifier.is_none_or(|p| ast[p].prefix == node) {
                self.add_prefix_for_element(e);
            }
        }
        let mut kind = RelationKind::IsReferencedBy;
        if let Some(e) = element
            && matches!(e.tag(), Tag::FormalParameter | Tag::FieldFormalParameter | Tag::SuperFormalParameter)
        {
            let is_get = dartr_resolver::ast_ext::simple_identifier_in_getter_context(ast, node);
            let is_set = dartr_resolver::ast_ext::simple_identifier_in_setter_context(ast, node);
            if parent.is_some_and(|p| ast.is::<CommentReference>(p)) {
                kind = RelationKind::IsReferencedBy;
            } else if is_get && is_set {
                kind = RelationKind::IsReadWrittenBy;
            } else if is_get {
                if parent
                    .and_then(|p| ast.cast::<MethodInvocation>(p))
                    .is_some_and(|m| ast[m].method_name == node)
                {
                    kind = RelationKind::IsInvokedBy;
                } else {
                    kind = RelationKind::IsReadBy;
                }
            } else if is_set {
                kind = RelationKind::IsWrittenBy;
            }
        }
        let _ = self.is_qualified(node);
        self.record_node(element, kind, node.raw());
    }

    fn visit_super_constructor_invocation(&mut self, ast: &Ast, node: Id<SuperConstructorInvocation>) {
        let element = self.element(node);
        match ast[node].constructor_name {
            Some(name) => {
                let o = ast.tokens.get(ast[node].period.expect("period")).offset;
                self.record_offset(element, RelationKind::IsInvokedBy, o, ast.end(name) - o);
            }
            None => {
                let o = ast.tokens.get(ast[node].super_keyword).end();
                self.record_offset(element, RelationKind::IsInvokedBy, o, 0);
            }
        }
        ast.accept(ast[node].argument_list, self);
    }

    fn visit_super_formal_parameter(&mut self, ast: &Ast, node: Id<SuperFormalParameter>) {
        if let Some(e) = self.declared(node)
            && e.tag() == Tag::SuperFormalParameter
            && let Some(sp) = dartr_resolver::constant::evaluation::super_constructor_parameter(self.ctx, e)
        {
            let sp = self.base(sp);
            let kind = if ast[node].kind.is_named() {
                RelationKind::IsReferencedByNamedArgument
            } else {
                RelationKind::IsReferencedBy
            };
            self.record_token(Some(sp), kind, ast[node].name);
        }
        ast.visit_children(node, self);
    }

    fn visit_with_clause(&mut self, ast: &Ast, node: Id<WithClause>) {
        for &t in ast.list(ast[node].mixin_types) {
            self.record_super_type(t, RelationKind::IsMixedInBy);
            ast.accept(t, self);
        }
    }
}

/// Dart `declaredNamedArgumentParameter`.
fn declared_named_argument_parameter(
    c: &Contributor<'_, '_>,
    ast: &Ast,
    node: Id<NamedArgument>,
    element: Option<ElementId>,
) -> Option<ElementId> {
    let element = element?;
    if c.ctx.element_data(element).and_then(|d| d.enclosing).is_some() {
        return Some(element);
    }
    let name = ast.tokens.lexeme(ast[node].name).to_string();
    let named_parameter = |executable: Option<ElementId>| -> Option<ElementId> {
        let executable = executable?.cast::<dartr_element::ExecutableElement>()?;
        c.ctx
            .executable(executable)
            .formal_params
            .iter()
            .copied()
            .find(|&p| {
                let data = c.ctx.get(p);
                data.kind.is_named() && data.name.map(|n| c.ctx.name_str(n)) == Some(name.as_str())
            })
            .map(|p| p.raw())
    };
    let list = ast.parent(node)?;
    if !ast.is::<ArgumentList>(list) {
        return Some(element);
    }
    let invocation = ast.parent(list)?;
    if let Some(i) = ast.cast::<InstanceCreationExpression>(invocation) {
        return named_parameter(c.element(ast[i].constructor_name));
    }
    if let Some(m) = ast.cast::<MethodInvocation>(invocation) {
        let e = c.element(ast[m].method_name);
        if e.is_some_and(crate::element_locator::is_executable) {
            return named_parameter(e);
        }
        return Some(element);
    }
    if let Some(r) = ast.cast::<RedirectingConstructorInvocation>(invocation) {
        return named_parameter(c.element(r));
    }
    if let Some(s) = ast.cast::<SuperConstructorInvocation>(invocation) {
        return named_parameter(c.element(s));
    }
    Some(element)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The orders that the Dart VM gives for these lists (`List.sort` is
    /// not stable; the expected orders were printed by `dart`).
    #[test]
    fn dart_sort_is_the_vm_sort() {
        let mut v: Vec<(i64, u32)> = (0..80u32).map(|i| (((i * 7) % 5) as i64, i)).collect();
        dart_sort(&mut v, &|a, b| a.0 - b.0);
        let order: Vec<String> = v.iter().map(|e| e.1.to_string()).collect();
        assert_eq!(order.join(","), "5,10,75,15,20,25,0,30,70,35,40,45,65,50,55,60,63,78,3,8,13,18,73,23,28,33,38,68,43,48,53,58,26,36,1,66,21,41,11,6,46,76,71,51,56,16,31,61,9,34,19,69,44,24,14,49,79,4,54,29,64,59,74,39,57,47,52,67,42,37,32,27,72,22,17,12,7,77,2,62");
        let mut w: Vec<(i64, u32)> = (0..200u32).map(|i| (((i * 13 + 7) % 11) as i64, i)).collect();
        dart_sort(&mut w, &|a, b| a.0 - b.0);
        let order: Vec<String> = w.iter().map(|e| e.1.to_string()).collect();
        assert_eq!(order.join(","), "24,2,68,145,90,57,13,134,178,167,46,79,123,189,112,35,156,101,30,184,41,19,107,151,162,52,129,195,96,63,8,140,173,85,74,118,47,190,157,25,102,124,14,179,3,58,91,135,146,80,36,69,113,168,97,53,64,42,130,31,108,86,163,9,20,185,75,152,141,196,174,119,136,169,70,191,92,48,59,4,180,103,158,37,114,81,26,125,147,15,54,87,98,186,32,197,153,65,164,76,21,120,131,142,109,10,43,175,93,104,126,159,115,192,16,60,148,49,71,181,82,137,170,5,38,27,66,0,187,99,77,88,165,154,198,11,176,121,22,55,33,44,110,132,143,50,28,61,72,149,6,17,127,171,193,39,116,94,138,105,160,182,83,23,111,89,166,188,144,78,34,100,155,199,67,12,133,177,56,45,1,122,62,183,40,172,73,29,84,95,161,18,106,117,194,128,7,150,139,51");
    }
}

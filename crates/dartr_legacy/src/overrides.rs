use dartr_ast::*;
use dartr_element::{
    Ctx, EId, ElementId, InterfaceElement, LibraryElement, MixinElement, ResolutionTables, Tag,
    TypeKind,
};
use dartr_resolver::element_ext::is_final;
use dartr_resolver::error::support::library_of;
use dartr_syntax::TokenId;
use rustc_hash::FxHashSet;

use crate::convert::new_overridden_member;
use crate::protocol::Override;

pub fn compute_dart_overrides(
    ctx: &Ctx<'_>,
    ast: &Ast,
    tables: &ResolutionTables,
    unit: Id<CompilationUnit>,
) -> Vec<Override> {
    let mut computer = OverridesComputer {
        ctx,
        ast,
        tables,
        overrides: Vec::new(),
    };
    computer.compute(unit);
    computer.overrides
}

struct OverridesComputer<'a> {
    ctx: &'a Ctx<'a>,
    ast: &'a Ast,
    tables: &'a ResolutionTables,
    overrides: Vec<Override>,
}

impl<'a> OverridesComputer<'a> {
    fn compute(&mut self, unit: Id<CompilationUnit>) {
        let ast = self.ast;
        for &member in ast.list(ast[unit].declarations) {
            let m = member.raw();
            match ast.kind(m) {
                NodeKind::ClassDeclaration => {
                    let node = Id::<ClassDeclaration>::from_raw(m);
                    let members = self.class_members(ast[node].body);
                    self.visit_class_members(members);
                }
                NodeKind::EnumDeclaration => {
                    let node = Id::<EnumDeclaration>::from_raw(m);
                    if let Some(body) = ast.cast::<BlockEnumBody>(ast[node].body) {
                        self.visit_class_members(ast.list(ast[body].members));
                    }
                }
                NodeKind::ExtensionTypeDeclaration => {
                    let node = Id::<ExtensionTypeDeclaration>::from_raw(m);
                    let members = self.class_members(ast[node].body);
                    self.visit_class_members(members);
                }
                NodeKind::MixinDeclaration => {
                    let node = Id::<MixinDeclaration>::from_raw(m);
                    let members = self.class_members(ast[node].body);
                    self.visit_class_members(members);
                }
                _ => {}
            }
        }
    }

    fn class_members(&self, body: Id<ClassBody>) -> &'a [Id<ClassMember>] {
        match self.ast.cast::<BlockClassBody>(body) {
            Some(b) => self.ast.list(self.ast[b].members),
            None => &[],
        }
    }

    fn declared_element(&self, node: impl Into<NodeId>) -> Option<ElementId> {
        let frag = self.tables.declared_fragment.get(node.into()).copied()?;
        self.ctx.fragment_data(frag)?.element.try_get().copied()
    }

    fn visit_class_members(&mut self, members: &[Id<ClassMember>]) {
        let ast = self.ast;
        for &cm in members {
            let m = cm.raw();
            if let Some(md) = ast.cast::<MethodDeclaration>(m) {
                let is_static = ast[md]
                    .modifier_keyword
                    .is_some_and(|t| ast.tokens.lexeme(t) == "static");
                if is_static {
                    continue;
                }
                let el = self.declared_element(md);
                self.add_override(ast[md].name, el);
            } else if let Some(fd) = ast.cast::<FieldDeclaration>(m) {
                if ast[fd].static_keyword.is_some() {
                    continue;
                }
                let vars = ast.list(ast[ast[fd].fields].variables);
                for &field in vars {
                    let el = self.declared_element(field);
                    self.add_override(ast[field].name, el);
                }
            }
        }
    }

    fn add_override(&mut self, token: TokenId, element: Option<ElementId>) {
        let Some(element) = element else { return };
        let (super_elements, interface_elements) = find_overridden_elements(self.ctx, element);
        if super_elements.is_empty() && interface_elements.is_empty() {
            return;
        }
        let superclass_member = super_elements.first().map(|&e| {
            let non_syn = dartr_element::diagnostics::non_synthetic(self.ctx, e);
            new_overridden_member(self.ctx, non_syn, None)
        });
        let interface_members: Vec<_> = interface_elements
            .into_iter()
            .map(|e| {
                let non_syn = dartr_element::diagnostics::non_synthetic(self.ctx, e);
                new_overridden_member(self.ctx, non_syn, None)
            })
            .collect();
        let t = self.ast.tokens.get(token);
        self.overrides.push(Override {
            offset: t.offset as i64,
            length: (t.end() - t.offset) as i64,
            superclass_member,
            interface_members: if interface_members.is_empty() {
                None
            } else {
                Some(interface_members)
            },
        });
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum MemberKind {
    Method,
    Getter,
    Setter,
}

pub fn find_overridden_elements(
    ctx: &Ctx<'_>,
    seed: ElementId,
) -> (Vec<ElementId>, Vec<ElementId>) {
    let Some(class_id) = ctx
        .element_data(seed)
        .and_then(|d| d.enclosing)
        .and_then(|enc| enc.cast::<InterfaceElement>())
    else {
        return (Vec::new(), Vec::new());
    };
    let Some(library_id) = library_of(ctx, class_id.raw()) else {
        return (Vec::new(), Vec::new());
    };
    let Some(name_id) = ctx.element_data(seed).and_then(|d| d.name) else {
        return (Vec::new(), Vec::new());
    };
    let name = ctx.name_str(name_id);
    let kinds: Vec<MemberKind> = match seed.tag() {
        Tag::Field => {
            if is_final(ctx, seed) {
                vec![MemberKind::Getter]
            } else {
                vec![MemberKind::Getter, MemberKind::Setter]
            }
        }
        Tag::Method => vec![MemberKind::Method],
        Tag::Getter => vec![MemberKind::Getter],
        Tag::Setter => vec![MemberKind::Setter],
        _ => Vec::new(),
    };
    if kinds.is_empty() {
        return (Vec::new(), Vec::new());
    }
    let mut finder = OverriddenElementsFinder {
        ctx,
        library: library_id,
        class_id,
        name,
        kinds,
        super_elements: Vec::new(),
        interface_elements: Vec::new(),
        visited: FxHashSet::default(),
    };
    finder.find();
    (finder.super_elements, finder.interface_elements)
}

struct OverriddenElementsFinder<'a> {
    ctx: &'a Ctx<'a>,
    library: EId<LibraryElement>,
    class_id: EId<InterfaceElement>,
    name: &'a str,
    kinds: Vec<MemberKind>,
    super_elements: Vec<ElementId>,
    interface_elements: Vec<ElementId>,
    visited: FxHashSet<EId<InterfaceElement>>,
}

impl<'a> OverriddenElementsFinder<'a> {
    fn find(&mut self) {
        self.visited.clear();
        self.add_super_overrides(Some(self.class_id), false);
        self.visited.clear();
        self.add_interface_overrides(Some(self.class_id), false);
        self.interface_elements
            .retain(|e| !self.super_elements.contains(e));
    }

    fn interface_element_of_type(
        &self,
        ty: Option<dartr_element::TypeId>,
    ) -> Option<EId<InterfaceElement>> {
        let ty = ty?;
        match self.ctx.ty(ty) {
            TypeKind::Interface { element, .. } => Some(*element),
            _ => None,
        }
    }

    fn add_super_overrides(
        &mut self,
        class_opt: Option<EId<InterfaceElement>>,
        with_this_type: bool,
    ) {
        let Some(class_) = class_opt else { return };
        if !self.visited.insert(class_) {
            return;
        }
        if with_this_type {
            if let Some(el) = self.lookup_member(class_)
                && !self.super_elements.contains(&el)
            {
                self.super_elements.push(el);
            }
        }
        let iface = self.ctx.interface(class_);
        let supertype = self.interface_element_of_type(iface.supertype.get());
        let mixins: Vec<_> = iface
            .mixins
            .get()
            .map(|l| self.ctx.list(l))
            .unwrap_or_default()
            .iter()
            .filter_map(|&m| self.interface_element_of_type(Some(m)))
            .collect();
        self.add_super_overrides(supertype, true);
        for mixin_ in mixins {
            self.add_super_overrides(Some(mixin_), true);
        }
        if let Some(mixin_el) = class_.raw().cast::<MixinElement>() {
            let constraints: Vec<_> = self
                .ctx
                .get(mixin_el)
                .superclass_constraints
                .get()
                .map(|l| self.ctx.list(l))
                .unwrap_or_default()
                .iter()
                .filter_map(|&c| self.interface_element_of_type(Some(c)))
                .collect();
            for constraint in constraints {
                self.add_super_overrides(Some(constraint), true);
            }
        }
    }

    fn add_interface_overrides(
        &mut self,
        class_opt: Option<EId<InterfaceElement>>,
        check_type: bool,
    ) {
        let Some(class_) = class_opt else { return };
        if !self.visited.insert(class_) {
            return;
        }
        if check_type {
            if let Some(el) = self.lookup_member(class_)
                && !self.interface_elements.contains(&el)
            {
                self.interface_elements.push(el);
            }
        }
        let iface = self.ctx.interface(class_);
        let interfaces: Vec<_> = iface
            .interfaces
            .get()
            .map(|l| self.ctx.list(l))
            .unwrap_or_default()
            .iter()
            .filter_map(|&i| self.interface_element_of_type(Some(i)))
            .collect();
        let supertype = self.interface_element_of_type(iface.supertype.get());
        for iface_el in interfaces {
            self.add_interface_overrides(Some(iface_el), true);
        }
        self.add_interface_overrides(supertype, check_type);
        if let Some(mixin_el) = class_.raw().cast::<MixinElement>() {
            let constraints: Vec<_> = self
                .ctx
                .get(mixin_el)
                .superclass_constraints
                .get()
                .map(|l| self.ctx.list(l))
                .unwrap_or_default()
                .iter()
                .filter_map(|&c| self.interface_element_of_type(Some(c)))
                .collect();
            for constraint in constraints {
                self.add_interface_overrides(Some(constraint), true);
            }
        }
    }

    fn matches_member(&self, elem: ElementId) -> bool {
        if self.name.starts_with('_') && library_of(self.ctx, elem) != Some(self.library) {
            return false;
        }
        self.ctx
            .element_data(elem)
            .and_then(|d| d.name)
            .is_some_and(|n| self.ctx.name_str(n) == self.name)
    }

    fn lookup_member(&self, class_element: EId<InterfaceElement>) -> Option<ElementId> {
        let inst = self.ctx.instance(class_element.upcast());
        if self.kinds.contains(&MemberKind::Method) {
            for &m in &inst.methods {
                if self.matches_member(m.raw()) {
                    return Some(m.raw());
                }
            }
        }
        if self.kinds.contains(&MemberKind::Getter) {
            for &g in &inst.getters {
                if self.matches_member(g.raw()) {
                    return Some(g.raw());
                }
            }
        }
        if self.kinds.contains(&MemberKind::Setter) {
            for &s in &inst.setters {
                if self.matches_member(s.raw()) {
                    return Some(s.raw());
                }
            }
        }
        None
    }
}

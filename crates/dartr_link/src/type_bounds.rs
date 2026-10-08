// Dart source: pkg/analyzer/lib/src/summary2/variance_builder.dart,
// pkg/analyzer/lib/src/summary2/simply_bounded.dart,
// pkg/analyzer/lib/src/summary2/type_alias.dart

//! Unit B3: variance of type parameters of type aliases, the
//! `isSimplyBounded` flag, and self references of type aliases. They work
//! on the resolved type annotation nodes and the unbuilt types of
//! [`TypeResolution`].

use dartr_ast::NamedType as NamedTypeNode;
use dartr_ast::*;
use dartr_element::*;
use indexmap::{IndexMap, IndexSet};

use crate::ast_util::*;
use crate::link::Linker;
use crate::scope::ScopeElement;
use crate::types::*;
use crate::types_builder::link_ctx;

/// The type of the representation field of an extension type (Dart
/// `representation.type`), `InvalidType` when not known.
pub fn representation_type(ctx: &Ctx<'_>, e: EId<ExtensionTypeElement>) -> TypeId {
    let first = ctx.get(e).fields.first().copied();
    first
        .and_then(|f| ctx.get(f).type_.get())
        .unwrap_or(TypeId::INVALID)
}

/// Dart `Variance.fromKeywordString`.
fn variance_from_keyword(k: &str) -> Variance {
    match k {
        "in" => Variance::Contravariant,
        "inout" => Variance::Invariant,
        "out" => Variance::Covariant,
        _ => Variance::Unrelated,
    }
}

/// Dart `VarianceBuilder.perform`: the variances to set.
pub fn compute_variances(
    lk: &Linker<'_>,
    tp: &TypeProvider,
    tr: &TypeResolution,
) -> IndexMap<EId<TypeParameterElement>, Variance> {
    let features = FeatureSet::default();
    let ctx = link_ctx(lk, tp, &features);
    let mut vb = VarianceBuilder {
        lk,
        ctx: &ctx,
        tr,
        pending: IndexSet::new(),
        visit: IndexSet::new(),
        result: IndexMap::new(),
    };
    for (lib, builder) in lk.builders.iter().enumerate() {
        for (unit, u) in builder.units.iter().enumerate() {
            let ast = &u.parsed.ast;
            for &d in ast.list(ast.get(u.parsed.unit).declarations) {
                if ast.is::<FunctionTypeAlias>(d.raw()) || ast.is::<GenericTypeAlias>(d.raw()) {
                    vb.pending.insert((lib as u32, unit as u32, d.raw()));
                }
            }
        }
    }
    for (lib, builder) in lk.builders.iter().enumerate() {
        for (unit, u) in builder.units.iter().enumerate() {
            let ast = &u.parsed.ast;
            let key = |n: NodeId| (lib as u32, unit as u32, n);
            for &d in ast.list(ast.get(u.parsed.unit).declarations) {
                let d = d.raw();
                if let Some(n) = ast.cast::<ClassTypeAlias>(d) {
                    vb.type_parameters(lib as u32, unit as u32, ast.get(n).type_parameters);
                } else if let Some(n) = ast.cast::<ClassDeclaration>(d) {
                    vb.type_parameters(lib as u32, unit as u32, class_name_part_type_parameters(ast, ast.get(n).name_part));
                } else if let Some(n) = ast.cast::<EnumDeclaration>(d) {
                    vb.type_parameters(lib as u32, unit as u32, class_name_part_type_parameters(ast, ast.get(n).name_part));
                } else if ast.is::<FunctionTypeAlias>(d) {
                    vb.function_type_alias(key(d));
                } else if ast.is::<GenericTypeAlias>(d) {
                    vb.generic_type_alias(key(d));
                } else if let Some(n) = ast.cast::<MixinDeclaration>(d) {
                    vb.type_parameters(lib as u32, unit as u32, ast.get(n).type_parameters);
                }
            }
        }
    }
    vb.result
}

struct VarianceBuilder<'v, 'l, 'a> {
    lk: &'l Linker<'a>,
    ctx: &'v Ctx<'l>,
    tr: &'v TypeResolution,
    pending: IndexSet<NodeKey>,
    visit: IndexSet<NodeKey>,
    result: IndexMap<EId<TypeParameterElement>, Variance>,
}

impl VarianceBuilder<'_, '_, '_> {
    fn variance(&self, p: EId<TypeParameterElement>) -> Variance {
        match self.result.get(&p) {
            Some(&v) => v,
            None => type_parameter_variance(self.ctx, p),
        }
    }

    fn set_variance(&mut self, lib: u32, unit: u32, node: Id<TypeParameter>, v: Variance) {
        if let Some(e) = declared_element(self.lk, (lib, unit, node.raw())).and_then(|e| e.cast()) {
            self.result.insert(e, v);
        }
    }

    /// Dart `_compute`.
    fn compute(&mut self, variable: EId<TypeParameterElement>, t: Option<LType>) -> Variance {
        let Some(t) = t else {
            return Variance::Unrelated;
        };
        match t {
            LType::Built(t) => match *self.ctx.ty(t) {
                TypeKind::TypeParameter { param, .. } => {
                    if param == variable {
                        Variance::Covariant
                    } else {
                        Variance::Unrelated
                    }
                }
                _ => Variance::Unrelated,
            },
            LType::Builder(b) => match self.tr.builders[b as usize].kind.clone() {
                BuilderKind::Named {
                    element, arguments, ..
                } => {
                    let Some(element) = element else {
                        return Variance::Unrelated;
                    };
                    let parameters: Vec<EId<TypeParameterElement>> = match element.tag() {
                        Tag::Class | Tag::Enum | Tag::Mixin | Tag::ExtensionType => {
                            self.ctx.instance(EId::from_raw(element)).type_params.clone()
                        }
                        Tag::TypeAlias => {
                            self.type_alias_element(EId::from_raw(element));
                            self.ctx.get(EId::<TypeAliasElement>::from_raw(element)).type_params.clone()
                        }
                        _ => return Variance::Unrelated,
                    };
                    let mut result = Variance::Unrelated;
                    for (i, &a) in arguments.iter().enumerate() {
                        let Some(&p) = parameters.get(i) else { break };
                        let v = self.compute(variable, Some(a));
                        result = variance_meet(result, variance_combine(self.variance(p), v));
                    }
                    result
                }
                BuilderKind::Function {
                    type_params,
                    params,
                    return_type,
                    ..
                } => {
                    let params: Vec<Option<LType>> = params.iter().map(|p| Some(p.ty)).collect();
                    self.compute_function_type(variable, Some(return_type), Some(&type_params), &params)
                }
                BuilderKind::Record {
                    positional, named, ..
                } => {
                    let mut result = Variance::Unrelated;
                    for t in positional.into_iter().chain(named.into_iter().map(|(_, t)| t)) {
                        result = variance_meet(result, self.compute(variable, Some(t)));
                    }
                    result
                }
            },
        }
    }

    /// Dart `_computeFunctionType`.
    fn compute_function_type(
        &mut self,
        variable: EId<TypeParameterElement>,
        return_type: Option<LType>,
        type_parameters: Option<&[EId<TypeParameterElement>]>,
        formal_parameters: &[Option<LType>],
    ) -> Variance {
        let mut result = Variance::Unrelated;
        result = variance_meet(result, self.compute(variable, return_type));
        if let Some(tps) = type_parameters {
            for &tp in tps {
                let bound = self.tr.bound_of(self.ctx, tp);
                if bound.is_some() && self.compute(variable, bound) != Variance::Unrelated {
                    result = Variance::Invariant;
                }
            }
        }
        for &p in formal_parameters {
            let v = self.compute(variable, p);
            result = variance_meet(result, variance_combine(Variance::Contravariant, v));
        }
        result
    }

    /// The types of the parameters of a formal parameter list as Dart
    /// `FunctionTypeBuilder.getParameters` gives them; a function-typed
    /// parameter is computed with its own function type (here: its
    /// variance directly).
    fn parameter_variances(&mut self, variable: EId<TypeParameterElement>, lib: u32, unit: u32, list: Id<FormalParameterList>) -> Variance {
        let ast = unit_ast(self.lk, lib, unit);
        let mut result = Variance::Unrelated;
        for &p in ast.list(ast.get(list).parameters) {
            let (_, _, suffix) = crate::informative_data::formal_parameter_parts(ast, p);
            let type_node = formal_parameter_type_node(ast, p);
            let v = match suffix {
                Some(s) => {
                    let s = ast.get(s);
                    let ret = type_node.and_then(|t| self.tr.node_type((lib, unit, t.raw()))).or(Some(LType::Built(TypeId::DYNAMIC)));
                    let tps: Vec<EId<TypeParameterElement>> = match s.type_parameters {
                        Some(l) => ast
                            .list(ast.get(l).type_parameters)
                            .iter()
                            .filter_map(|tp| declared_element(self.lk, (lib, unit, tp.raw())).and_then(|e| e.cast()))
                            .collect(),
                        None => Vec::new(),
                    };
                    let mut r = Variance::Unrelated;
                    r = variance_meet(r, self.compute(variable, ret));
                    for &tp in &tps {
                        let bound = self.tr.bound_of(self.ctx, tp);
                        if bound.is_some() && self.compute(variable, bound) != Variance::Unrelated {
                            r = Variance::Invariant;
                        }
                    }
                    let inner = self.parameter_variances(variable, lib, unit, s.formal_parameters);
                    variance_meet(r, inner)
                }
                None => {
                    let t = type_node.and_then(|t| self.tr.node_type((lib, unit, t.raw()))).or(Some(LType::Built(TypeId::DYNAMIC)));
                    self.compute(variable, t)
                }
            };
            result = variance_meet(result, variance_combine(Variance::Contravariant, v));
        }
        result
    }

    fn type_alias_params(&self, key: NodeKey) -> Option<Id<TypeParameterList>> {
        let ast = unit_ast(self.lk, key.0, key.1);
        if let Some(f) = ast.cast::<FunctionTypeAlias>(key.2) {
            ast.get(f).type_parameters
        } else {
            ast.get(ast.cast::<GenericTypeAlias>(key.2).unwrap()).type_parameters
        }
    }

    /// Dart `_functionTypeAlias`.
    fn function_type_alias(&mut self, key: NodeKey) {
        let Some(list) = self.type_alias_params(key) else { return };
        let ast = unit_ast(self.lk, key.0, key.1);
        let params: Vec<Id<TypeParameter>> = ast.list(ast.get(list).type_parameters).to_vec();
        if self.visit.contains(&key) {
            for p in params {
                self.set_variance(key.0, key.1, p, Variance::Covariant);
            }
            return;
        }
        if !self.pending.shift_remove(&key) {
            return;
        }
        self.visit.insert(key);
        let f = ast.get(ast.cast::<FunctionTypeAlias>(key.2).unwrap());
        for p in params {
            let Some(e) = declared_element(self.lk, (key.0, key.1, p.raw())).and_then(|e| e.cast()) else {
                continue;
            };
            let ret = f
                .return_type
                .and_then(|t| self.tr.node_type((key.0, key.1, t.raw())));
            let mut v = Variance::Unrelated;
            v = variance_meet(v, self.compute(e, ret));
            v = variance_meet(v, self.parameter_variances(e, key.0, key.1, f.parameters));
            self.set_variance(key.0, key.1, p, v);
        }
        self.visit.shift_remove(&key);
    }

    /// Dart `_genericTypeAlias`.
    fn generic_type_alias(&mut self, key: NodeKey) {
        let Some(list) = self.type_alias_params(key) else { return };
        let ast = unit_ast(self.lk, key.0, key.1);
        let params: Vec<Id<TypeParameter>> = ast.list(ast.get(list).type_parameters).to_vec();
        if self.visit.contains(&key) {
            for p in params {
                self.set_variance(key.0, key.1, p, Variance::Covariant);
            }
            return;
        }
        if !self.pending.shift_remove(&key) {
            return;
        }
        let g = ast.get(ast.cast::<GenericTypeAlias>(key.2).unwrap());
        let t = self.tr.node_type((key.0, key.1, g.type_.raw()));
        if t.is_none() {
            for &p in &params {
                self.set_variance(key.0, key.1, p, Variance::Covariant);
            }
        }
        self.visit.insert(key);
        for p in params {
            let Some(e) = declared_element(self.lk, (key.0, key.1, p.raw())).and_then(|e| e.cast()) else {
                continue;
            };
            let v = self.compute(e, t);
            self.set_variance(key.0, key.1, p, v);
        }
        self.visit.shift_remove(&key);
    }

    /// Dart `_typeAliasElement`.
    fn type_alias_element(&mut self, element: EId<TypeAliasElement>) {
        if element.store() != self.lk.core.store.id {
            return;
        }
        let first = self.ctx.get(element).first_fragment().raw();
        let Some(&(lib, unit, node)) = self.lk.core.fragment_nodes.get(&first) else {
            return;
        };
        let key = (lib as u32, unit as u32, node);
        let ast = unit_ast(self.lk, key.0, key.1);
        if ast.is::<GenericTypeAlias>(node) {
            self.generic_type_alias(key);
        } else if ast.is::<FunctionTypeAlias>(node) {
            self.function_type_alias(key);
        }
    }

    /// Dart `_typeParameters`: the declared variances.
    fn type_parameters(&mut self, lib: u32, unit: u32, list: Option<Id<TypeParameterList>>) {
        let Some(list) = list else { return };
        let ast = unit_ast(self.lk, lib, unit);
        for &p in ast.list(ast.get(list).type_parameters) {
            if let Some(k) = ast.get(p).variance_keyword {
                let v = variance_from_keyword(ast.tokens.lexeme(k));
                self.set_variance(lib, unit, p, v);
            }
        }
    }
}

// ---- simply bounded ----

struct SbNode {
    key: NodeKey,
    type_parameters: Vec<Id<TypeParameter>>,
    rhs_types: Vec<Id<TypeAnnotation>>,
    index: u32,
    low_link: u32,
    evaluated: bool,
    simply_bounded: bool,
    dependencies: Option<Vec<usize>>,
}

/// Dart `computeSimplyBounded`.
pub fn compute_simply_bounded(lk: &Linker<'_>, ctx: &Ctx<'_>, tr: &TypeResolution) {
    let mut walker = SbWalker {
        lk,
        ctx,
        tr,
        nodes: Vec::new(),
        node_map: IndexMap::new(),
        index: 1,
        stack: Vec::new(),
    };
    let mut order = Vec::new();
    for builder in &lk.builders {
        let l = ctx.get(builder.element);
        let elements: Vec<ElementId> = l
            .classes
            .iter()
            .map(|e| e.raw())
            .chain(l.enums.iter().map(|e| e.raw()))
            .chain(l.extension_types.iter().map(|e| e.raw()))
            .chain(l.mixins.iter().map(|e| e.raw()))
            .chain(l.type_aliases.iter().map(|e| e.raw()))
            .collect();
        for e in elements {
            if let Some(n) = walker.get_node(e) {
                order.push((e, n));
            }
        }
    }
    for (e, n) in order {
        walker.walk(n);
        let simply_bounded = walker.nodes[n].simply_bounded;
        let flag = if e.tag() == Tag::TypeAlias {
            ElementFlags::TYPE_ALIAS_ELEMENT_IS_SIMPLY_BOUNDED
        } else {
            ElementFlags::INSTANCE_ELEMENT_IS_SIMPLY_BOUNDED
        };
        ctx.element_data(e).unwrap().flags.set(flag, simply_bounded);
    }
}

struct SbWalker<'w, 'l, 'a> {
    lk: &'l Linker<'a>,
    ctx: &'w Ctx<'l>,
    tr: &'w TypeResolution,
    nodes: Vec<SbNode>,
    node_map: IndexMap<ElementId, usize>,
    index: u32,
    stack: Vec<usize>,
}

/// Dart `_TypeCollector`.
fn collect_parameter_types(ast: &Ast, list: Id<FormalParameterList>, out: &mut Vec<Id<TypeAnnotation>>) {
    for &p in ast.list(ast.get(list).parameters) {
        if ast.is::<FieldFormalParameter>(p.raw()) || ast.is::<SuperFormalParameter>(p.raw()) {
            continue;
        }
        let (_, _, suffix) = crate::informative_data::formal_parameter_parts(ast, p);
        out.extend(formal_parameter_type_node(ast, p));
        if let Some(s) = suffix {
            collect_parameter_types(ast, ast.get(s).formal_parameters, out);
        }
    }
}

fn collect_type_parameter_bounds(ast: &Ast, list: Option<Id<TypeParameterList>>, out: &mut Vec<Id<TypeAnnotation>>) {
    if let Some(list) = list {
        for &tp in ast.list(ast.get(list).type_parameters) {
            out.extend(ast.get(tp).bound);
        }
    }
}

impl SbWalker<'_, '_, '_> {
    /// Dart `getNode`.
    fn get_node(&mut self, e: ElementId) -> Option<usize> {
        if let Some(&n) = self.node_map.get(&e) {
            return Some(n);
        }
        let first = self.ctx.element_data(e)?.first_fragment;
        let &(lib, unit, node) = self.lk.core.fragment_nodes.get(&first)?;
        let key = (lib as u32, unit as u32, node);
        let ast = unit_ast(self.lk, key.0, key.1);
        let tps = |list: Option<Id<TypeParameterList>>| -> Vec<Id<TypeParameter>> {
            list.map(|l| ast.list(ast.get(l).type_parameters).to_vec()).unwrap_or_default()
        };
        let (type_parameters, rhs_types) = if let Some(n) = ast.cast::<ClassDeclaration>(node) {
            (tps(class_name_part_type_parameters(ast, ast.get(n).name_part)), Vec::new())
        } else if let Some(n) = ast.cast::<ClassTypeAlias>(node) {
            (tps(ast.get(n).type_parameters), Vec::new())
        } else if let Some(n) = ast.cast::<EnumDeclaration>(node) {
            (tps(class_name_part_type_parameters(ast, ast.get(n).name_part)), Vec::new())
        } else if let Some(n) = ast.cast::<ExtensionTypeDeclaration>(node) {
            (tps(class_name_part_type_parameters(ast, ast.get(n).name_part)), Vec::new())
        } else if let Some(n) = ast.cast::<FunctionTypeAlias>(node) {
            let n = ast.get(n);
            let mut rhs = Vec::new();
            rhs.extend(n.return_type);
            collect_parameter_types(ast, n.parameters, &mut rhs);
            (tps(n.type_parameters), rhs)
        } else if let Some(n) = ast.cast::<GenericTypeAlias>(node) {
            let n = ast.get(n);
            let mut rhs = Vec::new();
            if let Some(g) = ast.cast::<GenericFunctionType>(n.type_.raw()) {
                let g = ast.get(g);
                rhs.extend(g.return_type);
                collect_type_parameter_bounds(ast, g.type_parameters, &mut rhs);
                collect_parameter_types(ast, g.parameters, &mut rhs);
            } else {
                rhs.push(n.type_);
            }
            (tps(n.type_parameters), rhs)
        } else {
            let n = ast.cast::<MixinDeclaration>(node)?;
            (tps(ast.get(n).type_parameters), Vec::new())
        };
        self.nodes.push(SbNode {
            key,
            type_parameters,
            rhs_types,
            index: 0,
            low_link: 0,
            evaluated: false,
            simply_bounded: true,
            dependencies: None,
        });
        let n = self.nodes.len() - 1;
        self.node_map.insert(e, n);
        Some(n)
    }

    /// Dart `SimplyBoundedNode.computeDependencies`.
    fn dependencies(&mut self, n: usize) -> Vec<usize> {
        if let Some(d) = &self.nodes[n].dependencies {
            return d.clone();
        }
        let key = self.nodes[n].key;
        let ast = unit_ast(self.lk, key.0, key.1);
        let mut dependencies = Vec::new();
        let mut ok = true;
        let bounds: Vec<Id<TypeAnnotation>> = self.nodes[n]
            .type_parameters
            .iter()
            .filter_map(|&tp| ast.get(tp).bound)
            .collect();
        for bound in bounds {
            if !self.visit_type(&mut dependencies, key.0, key.1, bound, false) {
                ok = false;
                break;
            }
        }
        if ok {
            let rhs = self.nodes[n].rhs_types.clone();
            for t in rhs {
                if !self.visit_type(&mut dependencies, key.0, key.1, t, true) {
                    ok = false;
                    break;
                }
            }
        }
        if !ok {
            self.nodes[n].simply_bounded = false;
            dependencies.clear();
        }
        self.nodes[n].dependencies = Some(dependencies.clone());
        dependencies
    }

    /// Dart `_visitType`.
    fn visit_type(&mut self, deps: &mut Vec<usize>, lib: u32, unit: u32, t: Id<TypeAnnotation>, allow_type_parameters: bool) -> bool {
        let ast = unit_ast(self.lk, lib, unit);
        let raw = t.raw();
        if let Some(n) = ast.cast::<NamedTypeNode>(raw) {
            let element = self.tr.node_elements.get(&(lib, unit, raw)).cloned().flatten();
            let element = match element {
                Some(ScopeElement::Element(e)) => Some(e),
                _ => None,
            };
            if element.is_some_and(|e| e.tag() == Tag::TypeParameter) {
                return allow_type_parameters;
            }
            match ast.get(n).type_arguments {
                None => {
                    let Some(e) = element else { return true };
                    match self.node_map.get(&e) {
                        Some(&node) => deps.push(node),
                        None => {
                            return match e.tag() {
                                Tag::Class | Tag::Enum | Tag::Mixin | Tag::ExtensionType | Tag::Extension => self
                                    .ctx
                                    .element_data(e)
                                    .unwrap()
                                    .flags
                                    .has(ElementFlags::INSTANCE_ELEMENT_IS_SIMPLY_BOUNDED),
                                Tag::TypeAlias => self
                                    .ctx
                                    .element_data(e)
                                    .unwrap()
                                    .flags
                                    .has(ElementFlags::TYPE_ALIAS_ELEMENT_IS_SIMPLY_BOUNDED),
                                _ => true,
                            };
                        }
                    }
                }
                Some(args) => {
                    for &a in ast.list(ast.get(args).arguments) {
                        if !self.visit_type(deps, lib, unit, a, allow_type_parameters) {
                            return false;
                        }
                    }
                }
            }
            return true;
        }
        if let Some(g) = ast.cast::<GenericFunctionType>(raw) {
            let g = ast.get(g);
            let mut types = Vec::new();
            types.extend(g.return_type);
            collect_type_parameter_bounds(ast, g.type_parameters, &mut types);
            collect_parameter_types(ast, g.parameters, &mut types);
            for t in types {
                if !self.visit_type(deps, lib, unit, t, allow_type_parameters) {
                    return false;
                }
            }
            return true;
        }
        if let Some(r) = ast.cast::<RecordTypeAnnotation>(raw) {
            let r = ast.get(r);
            let mut types: Vec<Id<TypeAnnotation>> = ast.list(r.positional_fields).iter().map(|&f| ast.get(f).type_).collect();
            if let Some(nf) = r.named_fields {
                types.extend(ast.list(ast.get(nf).fields).iter().map(|&f| ast.get(f).type_));
            }
            for t in types {
                if !self.visit_type(deps, lib, unit, t, allow_type_parameters) {
                    return false;
                }
            }
            return true;
        }
        true
    }

    fn walk(&mut self, n: usize) {
        if self.nodes[n].evaluated {
            return;
        }
        self.strong_connect(n);
    }

    fn strong_connect(&mut self, n: usize) {
        let mut has_trivial_cycle = false;
        let index = self.index;
        self.index += 1;
        self.nodes[n].index = index;
        self.nodes[n].low_link = index;
        self.stack.push(n);
        for d in self.dependencies(n) {
            if self.nodes[d].evaluated {
                continue;
            }
            if d == n {
                has_trivial_cycle = true;
            } else if self.nodes[d].index == 0 {
                self.strong_connect(d);
                self.nodes[n].low_link = self.nodes[n].low_link.min(self.nodes[d].low_link);
            } else {
                self.nodes[n].low_link = self.nodes[n].low_link.min(self.nodes[d].index);
            }
        }
        if self.nodes[n].low_link == self.nodes[n].index {
            if *self.stack.last().unwrap() == n {
                self.stack.pop();
                if has_trivial_cycle {
                    self.mark_circular(n);
                } else {
                    // Dart `_evaluate`.
                    for d in self.dependencies(n) {
                        if !self.nodes[d].simply_bounded {
                            self.nodes[n].simply_bounded = false;
                            break;
                        }
                    }
                    self.nodes[n].evaluated = true;
                }
            } else {
                loop {
                    let other = self.stack.pop().unwrap();
                    self.mark_circular(other);
                    if other == n {
                        break;
                    }
                }
            }
        }
    }

    fn mark_circular(&mut self, n: usize) {
        self.nodes[n].simply_bounded = false;
        self.nodes[n].evaluated = true;
    }
}

// ---- type alias self references ----

/// Dart `TypeAliasSelfReferenceFinder.perform`.
pub fn find_type_alias_self_references(lk: &Linker<'_>, ctx: &Ctx<'_>, tr: &TypeResolution) {
    for (lib, builder) in lk.builders.iter().enumerate() {
        for (unit, u) in builder.units.iter().enumerate() {
            let ast = &u.parsed.ast;
            for &d in ast.list(ast.get(u.parsed.unit).declarations) {
                let key = (lib as u32, unit as u32, d.raw());
                if !(ast.is::<FunctionTypeAlias>(d.raw()) || ast.is::<GenericTypeAlias>(d.raw())) {
                    continue;
                }
                let mut finder = Finder {
                    lk,
                    tr,
                    self_: key,
                    visited: IndexSet::new(),
                    has_self_reference: false,
                };
                finder.type_alias(key);
                if let Some(f) = declared_fragment(lk, key) {
                    ctx.fragment(FId::<TypeAliasFragment>::from_raw(f))
                        .has_self_reference
                        .set(finder.has_self_reference);
                }
            }
        }
    }
}

/// Dart `_Finder`.
struct Finder<'f, 'l, 'a> {
    lk: &'l Linker<'a>,
    tr: &'f TypeResolution,
    self_: NodeKey,
    visited: IndexSet<NodeKey>,
    has_self_reference: bool,
}

impl Finder<'_, '_, '_> {
    fn type_alias(&mut self, key: NodeKey) {
        let ast = unit_ast(self.lk, key.0, key.1);
        if let Some(f) = ast.cast::<FunctionTypeAlias>(key.2) {
            let f = ast.get(f);
            self.type_parameter_list(key.0, key.1, f.type_parameters);
            self.formal_parameter_list(key.0, key.1, f.parameters);
            self.visit(key.0, key.1, f.return_type);
        } else if let Some(g) = ast.cast::<GenericTypeAlias>(key.2) {
            let g = ast.get(g);
            self.type_parameter_list(key.0, key.1, g.type_parameters);
            self.visit(key.0, key.1, Some(g.type_));
        }
    }

    fn type_parameter_list(&mut self, lib: u32, unit: u32, list: Option<Id<TypeParameterList>>) {
        let Some(list) = list else { return };
        let ast = unit_ast(self.lk, lib, unit);
        for &tp in ast.list(ast.get(list).type_parameters) {
            self.visit(lib, unit, ast.get(tp).bound);
        }
    }

    fn formal_parameter_list(&mut self, lib: u32, unit: u32, list: Id<FormalParameterList>) {
        let ast = unit_ast(self.lk, lib, unit);
        for &p in ast.list(ast.get(list).parameters) {
            let (_, _, suffix) = crate::informative_data::formal_parameter_parts(ast, p);
            self.visit(lib, unit, formal_parameter_type_node(ast, p));
            if let Some(s) = suffix {
                self.formal_parameter_list(lib, unit, ast.get(s).formal_parameters);
            }
        }
    }

    fn visit(&mut self, lib: u32, unit: u32, node: Option<Id<TypeAnnotation>>) {
        if self.has_self_reference {
            return;
        }
        let Some(node) = node else { return };
        let ast = unit_ast(self.lk, lib, unit);
        let raw = node.raw();
        if let Some(n) = ast.cast::<NamedTypeNode>(raw) {
            let element = match self.tr.node_elements.get(&(lib, unit, raw)) {
                Some(Some(ScopeElement::Element(e))) => Some(*e),
                _ => None,
            };
            if let Some(e) = element
                && e.store() == self.lk.core.store.id
                && let Some(data) = self.lk.core.store.element_data(e)
                && let Some(&(l, u, type_node)) = self.lk.core.fragment_nodes.get(&data.first_fragment)
            {
                let key = (l as u32, u as u32, type_node);
                if key == self.self_ {
                    self.has_self_reference = true;
                    return;
                }
                let tast = unit_ast(self.lk, key.0, key.1);
                if let Some(c) = tast.cast::<ClassDeclaration>(type_node) {
                    if self.visited.insert(key) {
                        self.type_parameter_list(key.0, key.1, class_name_part_type_parameters(tast, tast.get(c).name_part));
                    }
                } else if let Some(c) = tast.cast::<ClassTypeAlias>(type_node) {
                    if self.visited.insert(key) {
                        self.type_parameter_list(key.0, key.1, tast.get(c).type_parameters);
                    }
                } else if tast.is::<FunctionTypeAlias>(type_node) || tast.is::<GenericTypeAlias>(type_node) {
                    if self.visited.insert(key) {
                        self.type_alias(key);
                    }
                } else if let Some(m) = tast.cast::<MixinDeclaration>(type_node)
                    && self.visited.insert(key)
                {
                    self.type_parameter_list(key.0, key.1, tast.get(m).type_parameters);
                }
            }
            if let Some(args) = ast.get(n).type_arguments {
                for &a in ast.list(ast.get(args).arguments) {
                    self.visit(lib, unit, Some(a));
                }
            }
        } else if let Some(g) = ast.cast::<GenericFunctionType>(raw) {
            let g = ast.get(g);
            self.type_parameter_list(lib, unit, g.type_parameters);
            self.formal_parameter_list(lib, unit, g.parameters);
            self.visit(lib, unit, g.return_type);
        } else if let Some(r) = ast.cast::<RecordTypeAnnotation>(raw) {
            let r = ast.get(r);
            let mut types: Vec<Id<TypeAnnotation>> = ast.list(r.positional_fields).iter().map(|&f| ast.get(f).type_).collect();
            if let Some(nf) = r.named_fields {
                types.extend(ast.list(ast.get(nf).fields).iter().map(|&f| ast.get(f).type_));
            }
            for t in types {
                self.visit(lib, unit, Some(t));
            }
        }
    }
}

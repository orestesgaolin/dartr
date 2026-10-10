use dartr_ast::*;
use dartr_element::{Ctx, ElementId, ResolutionTables, Tag};
use dartr_resolver::element_metadata::UnitAst;
use dartr_server::element_locator::Unit;
use dartr_server::highlights::canonical;
use dartr_syntax::TokenId;
use indexmap::IndexMap;

use crate::convert::convert_element;
use crate::protocol::Occurrences;

pub fn compute_dart_occurrences(
    ctx: &Ctx<'_>,
    ast: &Ast,
    tables: &ResolutionTables,
    root: Id<CompilationUnit>,
) -> Vec<Occurrences> {
    let unit = Unit { ctx, ast, tables };
    let mut visitor = OccurrencesVisitor {
        unit: &unit,
        occurrences: IndexMap::new(),
    };
    ast.accept(root, &mut visitor);

    let mut collector: IndexMap<String, Occurrences> = IndexMap::new();
    for (engine_element, tokens) in visitor.occurrences {
        let server_element = convert_element(ctx, engine_element, Some(UnitAst { ast, tables }));
        let length = server_element
            .location
            .as_ref()
            .map(|l| l.length)
            .unwrap_or_else(|| server_element.name.encode_utf16().count() as i64);
        let offsets: Vec<i64> = tokens
            .into_iter()
            .filter_map(|tok| {
                let t = ast.tokens.get(tok);
                let tok_len = (t.end() - t.offset) as i64;
                if tok_len == length {
                    Some(t.offset as i64)
                } else {
                    None
                }
            })
            .collect();
        let key = serde_json::to_string(&server_element).unwrap_or_default();
        if let Some(existing) = collector.get_mut(&key) {
            existing.offsets.extend(offsets);
        } else {
            collector.insert(
                key,
                Occurrences {
                    element: server_element,
                    offsets,
                    length,
                },
            );
        }
    }
    collector.into_values().collect()
}

struct OccurrencesVisitor<'u, 'c, 'a> {
    unit: &'u Unit<'c, 'a>,
    occurrences: IndexMap<ElementId, Vec<TokenId>>,
}

impl OccurrencesVisitor<'_, '_, '_> {
    fn add_occurrence(&mut self, element: Option<ElementId>, token: Option<TokenId>) {
        let (Some(element), Some(token)) = (element, token) else {
            return;
        };
        let Some(canonical_el) = canonical(self.unit, element) else {
            return;
        };
        self.occurrences
            .entry(canonical_el)
            .or_default()
            .push(token);
    }

    fn declared(&self, node: impl Into<NodeId>) -> Option<ElementId> {
        self.unit.declared_element(node)
    }
}

impl AstVisitor for OccurrencesVisitor<'_, '_, '_> {
    fn visit_assigned_variable_pattern(&mut self, ast: &Ast, node: Id<AssignedVariablePattern>) {
        let element = self.unit.element(node);
        self.add_occurrence(element, Some(ast[node].name));
        ast.visit_children(node, self);
    }

    fn visit_catch_clause_parameter(&mut self, ast: &Ast, node: Id<CatchClauseParameter>) {
        let element = self.declared(node);
        self.add_occurrence(element, Some(ast[node].name));
        ast.visit_children(node, self);
    }

    fn visit_class_declaration(&mut self, ast: &Ast, node: Id<ClassDeclaration>) {
        let element = self.declared(node);
        let name = dartr_resolver::error::support::class_name_token(ast, ast[node].name_part);
        self.add_occurrence(element, Some(name));
        ast.visit_children(node, self);
    }

    fn visit_class_type_alias(&mut self, ast: &Ast, node: Id<ClassTypeAlias>) {
        let element = self.declared(node);
        self.add_occurrence(element, Some(ast[node].name));
        ast.visit_children(node, self);
    }

    fn visit_constructor_declaration(&mut self, ast: &Ast, node: Id<ConstructorDeclaration>) {
        let element = self.declared(node);
        if let Some(name) = ast[node].name {
            self.add_occurrence(element, Some(name));
        } else if let Some(type_name) = ast[node].type_name {
            self.add_occurrence(element, Some(ast.begin_token(type_name.raw())));
        }
        ast.visit_children(node, self);
    }

    fn visit_constructor_name(&mut self, ast: &Ast, node: Id<ConstructorName>) {
        if ast[node].name.is_none() {
            let element = self.unit.element(node);
            if element.is_some() {
                let named_type = ast[node].type_;
                self.add_occurrence(element, Some(ast[named_type].name));
            }
            if let Some(prefix) = ast[ast[node].type_].import_prefix {
                ast.accept(prefix, self);
            }
            return;
        }
        ast.visit_children(node, self);
    }

    fn visit_declared_identifier(&mut self, ast: &Ast, node: Id<DeclaredIdentifier>) {
        let element = self.declared(node);
        self.add_occurrence(element, Some(ast[node].name));
        ast.visit_children(node, self);
    }

    fn visit_declared_variable_pattern(&mut self, ast: &Ast, node: Id<DeclaredVariablePattern>) {
        let declared_element = self.declared(node);
        let target = declared_element
            .and_then(|e| dartr_resolver::element_ext::pattern_variable_join(self.unit.ctx, e))
            .or(declared_element);
        self.add_occurrence(target, Some(ast[node].name));
        ast.visit_children(node, self);
    }

    fn visit_enum_constant_declaration(&mut self, ast: &Ast, node: Id<EnumConstantDeclaration>) {
        let element = self.declared(node);
        self.add_occurrence(element, Some(ast[node].name));
        ast.visit_children(node, self);
    }

    fn visit_enum_declaration(&mut self, ast: &Ast, node: Id<EnumDeclaration>) {
        let element = self.declared(node);
        let name = dartr_resolver::error::support::class_name_token(ast, ast[node].name_part);
        self.add_occurrence(element, Some(name));
        ast.visit_children(node, self);
    }

    fn visit_extension_declaration(&mut self, ast: &Ast, node: Id<ExtensionDeclaration>) {
        if let Some(name) = ast[node].name {
            let element = self.declared(node);
            self.add_occurrence(element, Some(name));
        }
        ast.visit_children(node, self);
    }

    fn visit_extension_override(&mut self, ast: &Ast, node: Id<ExtensionOverride>) {
        let element = self.unit.element(node);
        self.add_occurrence(element, Some(ast[node].name));
        ast.visit_children(node, self);
    }

    fn visit_extension_type_declaration(&mut self, ast: &Ast, node: Id<ExtensionTypeDeclaration>) {
        let element = self.declared(node);
        let name = dartr_resolver::error::support::class_name_token(ast, ast[node].name_part);
        self.add_occurrence(element, Some(name));
        ast.visit_children(node, self);
    }

    fn visit_field_formal_parameter(&mut self, ast: &Ast, node: Id<FieldFormalParameter>) {
        if let Some(declared_element) = self.declared(node)
            && declared_element.tag() == Tag::FieldFormalParameter
        {
            let field = match self.unit.ctx.any(declared_element) {
                dartr_element::AnyElement::FormalParameter(p) => p.field.get().map(|f| f.raw()),
                _ => None,
            };
            self.add_occurrence(field, Some(ast[node].name));
        }
        ast.visit_children(node, self);
    }

    fn visit_function_declaration(&mut self, ast: &Ast, node: Id<FunctionDeclaration>) {
        let element = self.declared(node);
        self.add_occurrence(element, Some(ast[node].name));
        ast.visit_children(node, self);
    }

    fn visit_function_type_alias(&mut self, ast: &Ast, node: Id<FunctionTypeAlias>) {
        let element = self.declared(node);
        self.add_occurrence(element, Some(ast[node].name));
        ast.visit_children(node, self);
    }

    fn visit_generic_type_alias(&mut self, ast: &Ast, node: Id<GenericTypeAlias>) {
        let element = self.declared(node);
        self.add_occurrence(element, Some(ast[node].name));
        ast.visit_children(node, self);
    }

    fn visit_import_prefix_reference(&mut self, ast: &Ast, node: Id<ImportPrefixReference>) {
        let element = self.unit.element(node);
        self.add_occurrence(element, Some(ast[node].name));
        ast.visit_children(node, self);
    }

    fn visit_method_declaration(&mut self, ast: &Ast, node: Id<MethodDeclaration>) {
        let element = self.declared(node);
        self.add_occurrence(element, Some(ast[node].name));
        ast.visit_children(node, self);
    }

    fn visit_mixin_declaration(&mut self, ast: &Ast, node: Id<MixinDeclaration>) {
        let element = self.declared(node);
        self.add_occurrence(element, Some(ast[node].name));
        ast.visit_children(node, self);
    }

    fn visit_named_argument(&mut self, ast: &Ast, node: Id<NamedArgument>) {
        let parameter = dartr_resolver::error::support::corresponding_parameter(
            self.unit.ctx,
            ast,
            self.unit.tables,
            node.raw(),
        );
        self.add_occurrence(parameter, Some(ast[node].name));
        ast.visit_children(node, self);
    }

    fn visit_named_type(&mut self, ast: &Ast, node: Id<NamedType>) {
        let element = self.unit.element(node);
        self.add_occurrence(element, Some(ast[node].name));
        ast.visit_children(node, self);
    }

    fn visit_pattern_field(&mut self, ast: &Ast, node: Id<PatternField>) {
        let element = self.unit.element(node);
        let pattern = ast[node].pattern;
        let explicit_name = ast[node].name.and_then(|n| ast[n].name);
        let name = if explicit_name.is_none() {
            if let Some(p) = ast.cast::<DeclaredVariablePattern>(pattern) {
                Some(ast[p].name)
            } else if let Some(p) = ast.cast::<AssignedVariablePattern>(pattern) {
                Some(ast[p].name)
            } else {
                None
            }
        } else {
            explicit_name
        };
        self.add_occurrence(element, name);
        ast.visit_children(node, self);
    }

    fn visit_primary_constructor_name(&mut self, ast: &Ast, node: Id<PrimaryConstructorName>) {
        if let Some(parent) = ast.parent(node)
            && ast.is::<PrimaryConstructorDeclaration>(parent)
        {
            let element = self.declared(parent);
            self.add_occurrence(element, Some(ast[node].name));
        }
        ast.visit_children(node, self);
    }

    fn visit_regular_formal_parameter(&mut self, ast: &Ast, node: Id<RegularFormalParameter>) {
        if let Some(name_token) = ast[node].name {
            if let Some(element) = self.declared(node) {
                if element.tag() == Tag::FieldFormalParameter {
                    let field = match self.unit.ctx.any(element) {
                        dartr_element::AnyElement::FormalParameter(p) => {
                            p.field.get().map(|f| f.raw())
                        }
                        _ => None,
                    };
                    self.add_occurrence(field, Some(name_token));
                } else {
                    self.add_occurrence(Some(element), Some(name_token));
                }
            }
        }
        ast.visit_children(node, self);
    }

    fn visit_simple_identifier(&mut self, ast: &Ast, node: Id<SimpleIdentifier>) {
        if let Some(parent) = ast.parent(node)
            && let Some(c) = ast.cast::<ConstructorDeclaration>(parent)
            && ast[c].name.is_none()
            && ast[c].type_name == Some(node)
        {
            return;
        }
        let element = self.unit.write_or_read_element(node);
        self.add_occurrence(element, Some(ast[node].token));
        ast.visit_children(node, self);
    }

    fn visit_super_formal_parameter(&mut self, ast: &Ast, node: Id<SuperFormalParameter>) {
        let element = self.declared(node);
        self.add_occurrence(element, Some(ast[node].name));
        ast.visit_children(node, self);
    }

    fn visit_type_parameter(&mut self, ast: &Ast, node: Id<TypeParameter>) {
        let element = self.declared(node);
        self.add_occurrence(element, Some(ast[node].name));
        ast.visit_children(node, self);
    }

    fn visit_variable_declaration(&mut self, ast: &Ast, node: Id<VariableDeclaration>) {
        let element = self.declared(node);
        self.add_occurrence(element, Some(ast[node].name));
        ast.visit_children(node, self);
    }
}

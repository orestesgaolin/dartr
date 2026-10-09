// Dart source: pkg/analyzer/lib/src/generated/resolver.dart (the
// `visitX` methods of declarations, directives, class members, formal
// parameters, type annotations and the other nodes that are neither
// expressions nor statements; _visitFormalParameter)

//! The declaration visitors of [`ResolverVisitor`] and the visitors of the
//! remaining node kinds.

use dartr_ast::*;
use dartr_element::{
    EId, ElemRef, ElementId, ExecutableElement, ExtensionElement, FormalParameterElement,
    InterfaceElement, TypeId, TypeKind,
};
use dartr_flow::flow_analysis::FlowAnalysis;

use crate::resolver::ResolverVisitor;
use crate::{
    annotation_resolver, comment_reference_resolver, for_resolver, typed_literal_resolver,
    variable_declaration_resolver,
};

impl<'a> ResolverVisitor<'a> {
    /// The element of the declaration [node] (Dart
    /// `node.declaredFragment!.element`), if the element binding pass bound
    /// it.
    pub fn declared_element(&self, node: impl Into<NodeId>) -> Option<ElementId> {
        let fragment = *self.tables.declared_fragment.get(node)?;
        self.ctx.fragment_data(fragment)?.element.try_get().copied()
    }

    /// The formal parameters of an executable element.
    pub fn formal_parameters_of(
        &self,
        element: EId<ExecutableElement>,
    ) -> Vec<EId<FormalParameterElement>> {
        self.ctx.executable(element).formal_params.clone()
    }

    // ------------------------------------------------------------ unit

    pub fn visit_compilation_unit(&mut self, node: Id<CompilationUnit>) {
        let directives = self.ast.list_raw(self.ast[node].directives).to_vec();
        for directive in directives {
            self.visit_node(directive);
        }
        let declarations = self.ast.list_raw(self.ast[node].declarations).to_vec();
        for declaration in declarations {
            self.visit_node(declaration);
        }
        self.flush_type_analyzer_errors();
    }

    pub fn visit_script_tag(&mut self, _node: Id<ScriptTag>) {}

    // ------------------------------------------------------------ directives

    pub fn visit_library_directive(&mut self, node: Id<LibraryDirective>) {
        self.check_unreachable_node(node);
        self.visit_children(node);
    }

    pub fn visit_import_directive(&mut self, node: Id<ImportDirective>) {
        self.check_unreachable_node(node);
        self.visit_children(node);
        // Dart `elementResolver.visitImportDirective`: combinators (C9).
    }

    pub fn visit_export_directive(&mut self, node: Id<ExportDirective>) {
        self.check_unreachable_node(node);
        self.visit_children(node);
        // Dart `elementResolver.visitExportDirective`: combinators (C9).
    }

    pub fn visit_part_directive(&mut self, node: Id<PartDirective>) {
        self.check_unreachable_node(node);
        self.visit_children(node);
    }

    pub fn visit_part_of_directive(&mut self, node: Id<PartOfDirective>) {
        self.check_unreachable_node(node);
        self.visit_children(node);
    }

    pub fn visit_configuration(&mut self, _node: Id<Configuration>) {
        // Don't visit the children. For the time being we don't resolve
        // anything inside the configuration.
    }

    pub fn visit_dotted_name(&mut self, _node: Id<DottedName>) {}

    pub fn visit_show_combinator(&mut self, _node: Id<ShowCombinator>) {}

    pub fn visit_hide_combinator(&mut self, _node: Id<HideCombinator>) {}

    pub fn visit_import_prefix_reference(&mut self, _node: Id<ImportPrefixReference>) {}

    // ------------------------------------------------------------ metadata

    pub fn visit_annotation(&mut self, node: Id<Annotation>) {
        annotation_resolver::visit_annotation(self, node);
    }

    pub fn visit_comment(&mut self, node: Id<Comment>) {
        self.check_unreachable_node(node);
        self.visit_children(node);
    }

    pub fn visit_comment_reference(&mut self, node: Id<CommentReference>) {
        // We do not visit the expression because it needs to be visited in
        // the context of the reference.
        comment_reference_resolver::visit_comment_reference(self, node);
    }

    // ------------------------------------------------------------ classes

    /// The common part of the class-like declarations: sets
    /// `enclosingClass` while the children are visited.
    fn visit_interface_declaration(&mut self, node: NodeId) {
        let element = self
            .declared_element(node)
            .and_then(|e| e.cast::<InterfaceElement>());
        let outer = self.enclosing_class;
        self.enclosing_class = element;
        self.check_unreachable_node(node);
        self.visit_children(node);
        self.enclosing_class = outer;
        // Dart `baseOrFinalTypeVerifier.checkElement` (wave D).
    }

    pub fn visit_class_declaration(&mut self, node: Id<ClassDeclaration>) {
        self.visit_interface_declaration(node.raw());
    }

    pub fn visit_class_type_alias(&mut self, node: Id<ClassTypeAlias>) {
        self.check_unreachable_node(node);
        self.visit_children(node);
    }

    pub fn visit_enum_declaration(&mut self, node: Id<EnumDeclaration>) {
        self.visit_interface_declaration(node.raw());
    }

    pub fn visit_mixin_declaration(&mut self, node: Id<MixinDeclaration>) {
        self.visit_interface_declaration(node.raw());
    }

    pub fn visit_extension_type_declaration(&mut self, node: Id<ExtensionTypeDeclaration>) {
        self.visit_interface_declaration(node.raw());
    }

    pub fn visit_extension_declaration(&mut self, node: Id<ExtensionDeclaration>) {
        let element = self
            .declared_element(node)
            .and_then(|e| e.cast::<ExtensionElement>());
        let outer = self.enclosing_extension;
        self.enclosing_extension = element;
        self.check_unreachable_node(node);
        self.visit_children(node);
        self.enclosing_extension = outer;
    }

    pub fn visit_name_with_type_parameters(&mut self, node: Id<NameWithTypeParameters>) {
        self.visit_children(node);
    }

    pub fn visit_block_class_body(&mut self, node: Id<BlockClassBody>) {
        self.visit_children(node);
    }

    pub fn visit_empty_class_body(&mut self, _node: Id<EmptyClassBody>) {}

    pub fn visit_block_enum_body(&mut self, node: Id<BlockEnumBody>) {
        self.visit_children(node);
    }

    pub fn visit_empty_enum_body(&mut self, _node: Id<EmptyEnumBody>) {}

    pub fn visit_extends_clause(&mut self, node: Id<ExtendsClause>) {
        self.check_unreachable_node(node);
        self.visit_children(node);
    }

    pub fn visit_with_clause(&mut self, node: Id<WithClause>) {
        self.check_unreachable_node(node);
        self.visit_children(node);
    }

    pub fn visit_implements_clause(&mut self, node: Id<ImplementsClause>) {
        self.check_unreachable_node(node);
        self.visit_children(node);
    }

    pub fn visit_mixin_on_clause(&mut self, node: Id<MixinOnClause>) {
        self.check_unreachable_node(node);
        self.visit_children(node);
    }

    pub fn visit_extension_on_clause(&mut self, node: Id<ExtensionOnClause>) {
        self.check_unreachable_node(node);
        self.visit_children(node);
    }

    pub fn visit_native_clause(&mut self, node: Id<NativeClause>) {
        self.check_unreachable_node(node);
        self.visit_children(node);
    }

    /// Dart `visitEnumConstantDeclaration`.
    pub fn visit_enum_constant_declaration(&mut self, node: Id<EnumConstantDeclaration>) {
        let doc = self.ast[node].documentation_comment;
        self.visit_opt(doc);
        let metadata = self.ast[node].metadata;
        self.visit_list(metadata);
        crate::instance_creation_expression_resolver::visit_enum_constant_declaration(self, node);
    }

    pub fn visit_enum_constant_arguments(&mut self, node: Id<EnumConstantArguments>) {
        self.check_unreachable_node(node);
        self.visit_children(node);
    }

    // ------------------------------------------------------------ members

    pub fn visit_constructor_declaration(&mut self, node: Id<ConstructorDeclaration>) {
        let Some(element) = self
            .declared_element(node)
            .and_then(|e| e.cast::<ExecutableElement>())
        else {
            return;
        };
        let return_type = self.constructor_return_type(element);
        let outer_function = self.enclosing_function;
        self.enclosing_function = Some(element);
        self.setup_this_type();
        self.check_unreachable_node(node);
        let doc = self.ast[node].documentation_comment;
        self.visit_opt(doc);
        let metadata = self.ast[node].metadata;
        self.visit_list(metadata);
        let type_name = self.ast[node].type_name;
        self.visit_opt(type_name);
        let parameters = self.ast[node].parameters;
        self.visit_node(parameters.raw());

        let formal_parameters = self.formal_parameters_of(element);
        {
            let ast = &*self.ast;
            self.flow_analysis.body_or_initializer_enter(
                ast,
                self.tables,
                node.raw(),
                Some(&formal_parameters),
                None,
            );
        }
        self.flow_analysis.executable_declaration_enter(
            node.raw(),
            Some(&formal_parameters),
            false,
        );

        let initializers = self.ast[node].initializers;
        self.visit_list(initializers);
        let redirected = self.ast[node].redirected_constructor;
        self.visit_opt(redirected);
        let body = self.ast[node].body;
        let imposed = if matches!(self.ctx.ty(return_type), TypeKind::Dynamic) {
            None
        } else {
            Some(return_type)
        };
        self.resolve_function_body(body, imposed);
        if self.ast[node].factory_keyword.is_some() {
            self.check_for_body_may_complete_normally(body.raw(), node.raw());
        }
        self.flow_analysis
            .executable_declaration_exit(body.raw(), false);
        self.flow_analysis.body_or_initializer_exit();
        self.enclosing_function = outer_function;
        self.set_this_type(None);
    }

    /// Dart `element.type.returnType` of a constructor.
    fn constructor_return_type(&self, element: EId<ExecutableElement>) -> TypeId {
        let ty = dartr_typesystem::element_type::executable_type(&self.ctx, element);
        match *self.ctx.ty(ty) {
            TypeKind::Function(f) => f.ret,
            _ => TypeId::DYNAMIC,
        }
    }

    pub fn visit_constructor_field_initializer(&mut self, node: Id<ConstructorFieldInitializer>) {
        // We visit the expression, but do not visit the field name because
        // it needs to be visited in the context of the constructor field
        // initializer node.
        let field_name = self.ast[node].field_name;
        let name = self.lexeme(self.ast[field_name].token).to_string();
        let field = self
            .enclosing_class
            .and_then(|c| crate::element_ext::get_field(&self.ctx, c.upcast(), &name));
        self.set_element(field_name, field.map(|f| ElemRef::Base(f.raw())));
        let field_type = field
            .map(|f| crate::element_ext::variable_type(&self.ctx, f.raw()))
            .unwrap_or(TypeId::UNKNOWN);
        let expression = self.ast[node].expression;
        let expression = self.resolve_expression(expression, field_type);
        if let Some(field) = field
            && let Some(enclosing_function) = self.enclosing_function
        {
            let is_const_constructor =
                crate::element_ext::first_fragment_flags(&self.ctx, enclosing_function.raw())
                    .contains(dartr_element::FragmentFlags::CONSTRUCTOR_FRAGMENT_IS_CONST);
            let _ = field;
            self.check_for_field_initializer_not_assignable(
                expression,
                field_type,
                is_const_constructor,
            );
        }
    }

    pub fn visit_constructor_name(&mut self, node: Id<ConstructorName>) {
        let ty = self.ast[node].type_;
        self.visit_node(ty.raw());
        crate::element_resolver::visit_constructor_name(self, node);
    }

    pub fn visit_constructor_selector(&mut self, node: Id<ConstructorSelector>) {
        self.check_unreachable_node(node);
        self.visit_children(node);
    }

    /// Dart `visitSuperConstructorInvocation`.
    pub fn visit_super_constructor_invocation(&mut self, node: Id<SuperConstructorInvocation>) {
        crate::instance_creation_expression_resolver::visit_super_constructor_invocation(
            self, node,
        );
    }

    /// Dart `visitRedirectingConstructorInvocation`.
    pub fn visit_redirecting_constructor_invocation(
        &mut self,
        node: Id<RedirectingConstructorInvocation>,
    ) {
        crate::instance_creation_expression_resolver::visit_redirecting_constructor_invocation(
            self, node,
        );
    }

    pub fn visit_method_declaration(&mut self, node: Id<MethodDeclaration>) {
        let Some(element) = self
            .declared_element(node)
            .and_then(|e| e.cast::<ExecutableElement>())
        else {
            return;
        };
        let return_type = self
            .ctx
            .executable(element)
            .return_type
            .get()
            .unwrap_or(TypeId::DYNAMIC);
        let outer_function = self.enclosing_function;
        self.enclosing_function = Some(element);
        self.setup_this_type();
        self.check_unreachable_node(node);
        let doc = self.ast[node].documentation_comment;
        self.visit_opt(doc);
        let metadata = self.ast[node].metadata;
        self.visit_list(metadata);
        let return_type_node = self.ast[node].return_type;
        self.visit_opt(return_type_node);
        let type_parameters = self.ast[node].type_parameters;
        self.visit_opt(type_parameters);
        let parameters = self.ast[node].parameters;
        self.visit_opt(parameters);

        let formal_parameters = self.formal_parameters_of(element);
        {
            let ast = &*self.ast;
            self.flow_analysis.body_or_initializer_enter(
                ast,
                self.tables,
                node.raw(),
                Some(&formal_parameters),
                None,
            );
        }
        self.flow_analysis.executable_declaration_enter(
            node.raw(),
            Some(&formal_parameters),
            false,
        );
        let body = self.ast[node].body;
        let imposed = if matches!(self.ctx.ty(return_type), TypeKind::Dynamic) {
            None
        } else {
            Some(return_type)
        };
        self.resolve_function_body(body, imposed);
        let is_setter = self.ast[node]
            .property_keyword
            .is_some_and(|k| self.lexeme(k) == "set");
        if !is_setter {
            // Dart `checkForBodyMayCompleteNormally(body:, errorNode: node.name)`.
            self.check_for_body_may_complete_normally(body.raw(), node.raw());
        }
        self.flow_analysis
            .executable_declaration_exit(body.raw(), false);
        self.flow_analysis.body_or_initializer_exit();
        self.enclosing_function = outer_function;
        self.set_this_type(None);
    }

    pub fn visit_field_declaration(&mut self, node: Id<FieldDeclaration>) {
        self.setup_this_type();
        self.check_unreachable_node(node);
        self.visit_children(node);
        self.set_this_type(None);
    }

    /// Dart `visitPrimaryConstructorBody` (primary constructors, an
    /// experiment). STUB.
    pub fn visit_primary_constructor_body(&mut self, node: Id<PrimaryConstructorBody>) {
        let _ = node;
    }

    pub fn visit_primary_constructor_declaration(
        &mut self,
        node: Id<PrimaryConstructorDeclaration>,
    ) {
        self.visit_children(node);
    }

    pub fn visit_primary_constructor_name(&mut self, node: Id<PrimaryConstructorName>) {
        let _ = node;
    }

    // ------------------------------------------------------------ functions

    pub fn visit_function_declaration(&mut self, node: Id<FunctionDeclaration>) {
        let parent = self.ast.parent(node).expect("parent");
        let is_local = self.ast.is::<FunctionDeclarationStatement>(parent);
        let Some(element) = self
            .declared_element(node)
            .and_then(|e| e.cast::<ExecutableElement>())
        else {
            return;
        };
        let function_type = dartr_typesystem::element_type::executable_type(&self.ctx, element);
        let outer_function = self.enclosing_function;
        self.enclosing_function = Some(element);
        self.check_unreachable_node(node);
        let doc = self.ast[node].documentation_comment;
        self.visit_opt(doc);
        let metadata = self.ast[node].metadata;
        self.visit_list(metadata);
        let return_type = self.ast[node].return_type;
        self.visit_opt(return_type);

        let formal_parameters = self.formal_parameters_of(element);
        if is_local {
            self.flow().function_expression_begin(node.raw());
        } else {
            let ast = &*self.ast;
            self.flow_analysis.body_or_initializer_enter(
                ast,
                self.tables,
                node.raw(),
                Some(&formal_parameters),
                None,
            );
        }
        self.flow_analysis.executable_declaration_enter(
            node.raw(),
            Some(&formal_parameters),
            is_local,
        );

        let function_expression = self.ast[node].function_expression;
        self.resolve_expression(function_expression.upcast(), function_type);

        let body = self.ast[function_expression].body;
        let is_setter = self.ast[node]
            .property_keyword
            .is_some_and(|k| self.lexeme(k) == "set");
        if !is_setter {
            self.check_for_body_may_complete_normally(body.raw(), node.raw());
        }
        self.flow_analysis
            .executable_declaration_exit(body.raw(), is_local);
        if is_local {
            self.flow().function_expression_end();
        } else {
            self.flow_analysis.body_or_initializer_exit();
        }
        self.enclosing_function = outer_function;
    }

    pub fn visit_function_type_alias(&mut self, node: Id<FunctionTypeAlias>) {
        self.check_unreachable_node(node);
        self.visit_children(node);
    }

    pub fn visit_generic_type_alias(&mut self, node: Id<GenericTypeAlias>) {
        self.check_unreachable_node(node);
        self.visit_children(node);
    }

    pub fn visit_generic_function_type(&mut self, node: Id<GenericFunctionType>) {
        self.check_unreachable_node(node);
        self.visit_children(node);
    }

    // ------------------------------------------------------------ parameters

    pub fn visit_formal_parameter_list(&mut self, node: Id<FormalParameterList>) {
        // Formal parameter lists can contain default values, which in turn
        // contain expressions, so we need flow analysis to be available to
        // process those expressions.
        self.with_flow_analysis(node.raw(), |rv| {
            rv.check_unreachable_node(node);
            rv.visit_children(node);
        });
    }

    pub fn visit_regular_formal_parameter(&mut self, node: Id<RegularFormalParameter>) {
        let n = &self.ast[node];
        let parts = (
            n.documentation_comment,
            n.metadata,
            n.type_,
            n.function_typed_suffix,
            n.default_clause,
        );
        self.visit_formal_parameter(node.raw(), parts);
    }

    pub fn visit_field_formal_parameter(&mut self, node: Id<FieldFormalParameter>) {
        let n = &self.ast[node];
        let parts = (
            n.documentation_comment,
            n.metadata,
            n.type_,
            n.function_typed_suffix,
            n.default_clause,
        );
        self.visit_formal_parameter(node.raw(), parts);
    }

    pub fn visit_super_formal_parameter(&mut self, node: Id<SuperFormalParameter>) {
        let n = &self.ast[node];
        let parts = (
            n.documentation_comment,
            n.metadata,
            n.type_,
            n.function_typed_suffix,
            n.default_clause,
        );
        self.visit_formal_parameter(node.raw(), parts);
    }

    /// Dart `_visitFormalParameter`.
    fn visit_formal_parameter(
        &mut self,
        node: NodeId,
        (doc, metadata, ty, suffix, default_clause): (
            Option<Id<Comment>>,
            NodeList<Annotation>,
            Option<Id<TypeAnnotation>>,
            Option<Id<FunctionTypedFormalParameterSuffix>>,
            Option<Id<FormalParameterDefaultClause>>,
        ),
    ) {
        self.check_unreachable_node(node);
        self.visit_opt(doc);
        self.visit_list(metadata);
        self.visit_opt(ty);
        if let Some(suffix) = suffix {
            let type_parameters = self.ast[suffix].type_parameters;
            self.visit_opt(type_parameters);
            let formal_parameters = self.ast[suffix].formal_parameters;
            self.visit_node(formal_parameters.raw());
        }
        if let Some(default_clause) = default_clause {
            let value = self.ast[default_clause].value;
            let element_type = self
                .declared_element(node)
                .map(|e| crate::element_ext::variable_type(&self.ctx, e))
                .unwrap_or(TypeId::UNKNOWN);
            self.resolve_expression(value, element_type);
            // Dart `fragment.constantInitializer = defaultValue` for
            // parameters of local functions (constant evaluation, wave D).
        }
    }

    pub fn visit_formal_parameter_default_clause(
        &mut self,
        node: Id<FormalParameterDefaultClause>,
    ) {
        self.visit_children(node);
    }

    pub fn visit_function_typed_formal_parameter_suffix(
        &mut self,
        node: Id<FunctionTypedFormalParameterSuffix>,
    ) {
        self.visit_children(node);
    }

    // ------------------------------------------------------------ variables

    pub fn visit_top_level_variable_declaration(&mut self, node: Id<TopLevelVariableDeclaration>) {
        self.check_unreachable_node(node);
        self.visit_children(node);
    }

    pub fn visit_variable_declaration_list(&mut self, node: Id<VariableDeclarationList>) {
        {
            let ast = &*self.ast;
            let ctx = self.ctx;
            self.flow_analysis
                .variable_declaration_list(ast, self.tables, &ctx, node);
        }
        self.check_unreachable_node(node);
        self.visit_children(node);
    }

    pub fn visit_variable_declaration(&mut self, node: Id<VariableDeclaration>) {
        variable_declaration_resolver::resolve(self, node);
        if let Some(initializer) = self.ast[node].initializer {
            if self.flow_analysis.is_active() {
                let parent: Id<VariableDeclarationList> =
                    self.ast.cast(self.ast.parent(node).unwrap()).unwrap();
                let declared_type = self.ast[parent].type_;
                let keyword = self.ast[parent].keyword;
                let is_final = keyword.is_some_and(|k| matches!(self.lexeme(k), "final" | "const"));
                let is_late = self.ast[parent].late_keyword.is_some();
                let initializer_type = self.type_or_throw(initializer);
                let element = self
                    .declared_element(node)
                    .and_then(|e| e.cast::<dartr_element::PromotableElement>());
                let info = self.flow_analysis.get_expression_info(Some(initializer));
                if let Some(element) = element {
                    self.flow().initialize(
                        element,
                        dartr_flow::shared_type::SharedTypeView::new(initializer_type),
                        info,
                        is_final,
                        is_late,
                        declared_type.is_none(),
                        false,
                    );
                }
            }
        }
        // Dart `_checkTopLevelCycle` (wave D).
    }

    // ------------------------------------------------------------ types

    pub fn visit_named_type(&mut self, node: Id<NamedType>) {
        // All type names are already resolved, so we don't resolve it here.
        // But there might be type arguments with expressions, such as
        // default values for formal parameters of generic function types.
        // These are invalid, but if they exist, they should be resolved.
        let type_arguments = self.ast[node].type_arguments;
        self.visit_opt(type_arguments);
    }

    pub fn visit_type_argument_list(&mut self, node: Id<TypeArgumentList>) {
        self.check_unreachable_node(node);
        self.visit_children(node);
    }

    pub fn visit_type_parameter(&mut self, node: Id<TypeParameter>) {
        self.check_unreachable_node(node);
        self.visit_children(node);
    }

    pub fn visit_type_parameter_list(&mut self, node: Id<TypeParameterList>) {
        self.check_unreachable_node(node);
        self.visit_children(node);
    }

    pub fn visit_record_type_annotation(&mut self, node: Id<RecordTypeAnnotation>) {
        self.visit_children(node);
    }

    pub fn visit_record_type_annotation_named_field(
        &mut self,
        node: Id<RecordTypeAnnotationNamedField>,
    ) {
        self.visit_children(node);
    }

    pub fn visit_record_type_annotation_named_fields(
        &mut self,
        node: Id<RecordTypeAnnotationNamedFields>,
    ) {
        self.visit_children(node);
    }

    pub fn visit_record_type_annotation_positional_field(
        &mut self,
        node: Id<RecordTypeAnnotationPositionalField>,
    ) {
        self.visit_children(node);
    }

    // ------------------------------------------------------------ misc

    pub fn visit_argument_list(&mut self, node: Id<ArgumentList>) {
        // Arguments are resolved by the invocation inferrer (C3); a plain
        // visit resolves them without context.
        self.check_unreachable_node(node);
        self.visit_children(node);
    }

    pub fn visit_named_argument(&mut self, node: Id<NamedArgument>) {
        self.check_unreachable_node(node);
        let expression = self.ast[node].argument_expression;
        self.resolve_expression(expression, TypeId::UNKNOWN);
    }

    pub fn visit_declared_identifier(&mut self, node: Id<DeclaredIdentifier>) {
        self.check_unreachable_node(node);
        self.visit_children(node);
    }

    pub fn visit_interpolation_expression(&mut self, node: Id<InterpolationExpression>) {
        self.check_unreachable_node(node);
        let expression = self.ast[node].expression;
        self.resolve_expression(expression, TypeId::UNKNOWN);
    }

    pub fn visit_interpolation_string(&mut self, node: Id<InterpolationString>) {
        self.check_unreachable_node(node);
    }

    pub fn visit_label(&mut self, _node: Id<Label>) {}

    pub fn visit_label_reference(&mut self, _node: Id<LabelReference>) {}

    pub fn visit_record_literal_named_field(&mut self, node: Id<RecordLiteralNamedField>) {
        self.visit_children(node);
    }

    // Collection elements visited without a collection literal context.

    pub fn visit_for_element(&mut self, node: Id<ForElement>) {
        for_resolver::visit_for_element(self, node, None);
    }

    pub fn visit_if_element(&mut self, node: Id<IfElement>) {
        typed_literal_resolver::visit_if_element(self, node, None);
    }

    pub fn visit_map_literal_entry(&mut self, node: Id<MapLiteralEntry>) {
        typed_literal_resolver::visit_map_literal_entry(self, node, None);
    }

    pub fn visit_spread_element(&mut self, node: Id<SpreadElement>) {
        typed_literal_resolver::visit_spread_element(self, node, None);
    }

    pub fn visit_null_aware_element(&mut self, node: Id<NullAwareElement>) {
        typed_literal_resolver::visit_null_aware_element(self, node, None);
    }

    // For loop parts: resolved by the for resolver (C7).

    pub fn visit_for_each_parts_with_declaration(
        &mut self,
        _node: Id<ForEachPartsWithDeclaration>,
    ) {
    }

    pub fn visit_for_each_parts_with_identifier(&mut self, _node: Id<ForEachPartsWithIdentifier>) {}

    pub fn visit_for_each_parts_with_pattern(&mut self, _node: Id<ForEachPartsWithPattern>) {}

    pub fn visit_for_parts_with_declarations(&mut self, _node: Id<ForPartsWithDeclarations>) {}

    pub fn visit_for_parts_with_expression(&mut self, _node: Id<ForPartsWithExpression>) {}

    pub fn visit_for_parts_with_pattern(&mut self, _node: Id<ForPartsWithPattern>) {}

    // Patterns and switch members: resolved by the pattern analysis.

    pub fn visit_case_clause(&mut self, _node: Id<CaseClause>) {}

    pub fn visit_guarded_pattern(&mut self, _node: Id<GuardedPattern>) {}

    pub fn visit_when_clause(&mut self, _node: Id<WhenClause>) {}

    pub fn visit_switch_case(&mut self, _node: Id<SwitchCase>) {}

    pub fn visit_switch_default(&mut self, _node: Id<SwitchDefault>) {}

    pub fn visit_switch_pattern_case(&mut self, _node: Id<SwitchPatternCase>) {}

    pub fn visit_switch_expression_case(&mut self, _node: Id<SwitchExpressionCase>) {}

    pub fn visit_pattern_variable_declaration(&mut self, node: Id<PatternVariableDeclaration>) {
        crate::pattern_resolver::visit_pattern_variable_declaration(self, node);
    }

    pub fn visit_pattern_field(&mut self, _node: Id<PatternField>) {}

    pub fn visit_pattern_field_name(&mut self, _node: Id<PatternFieldName>) {}

    pub fn visit_map_pattern_entry(&mut self, _node: Id<MapPatternEntry>) {}

    pub fn visit_rest_pattern_element(&mut self, _node: Id<RestPatternElement>) {}

    pub fn visit_assigned_variable_pattern(&mut self, _node: Id<AssignedVariablePattern>) {}

    pub fn visit_cast_pattern(&mut self, _node: Id<CastPattern>) {}

    pub fn visit_constant_pattern(&mut self, _node: Id<ConstantPattern>) {}

    pub fn visit_declared_variable_pattern(&mut self, _node: Id<DeclaredVariablePattern>) {}

    pub fn visit_list_pattern(&mut self, _node: Id<ListPattern>) {}

    pub fn visit_logical_and_pattern(&mut self, _node: Id<LogicalAndPattern>) {}

    pub fn visit_logical_or_pattern(&mut self, _node: Id<LogicalOrPattern>) {}

    pub fn visit_map_pattern(&mut self, _node: Id<MapPattern>) {}

    pub fn visit_null_assert_pattern(&mut self, _node: Id<NullAssertPattern>) {}

    pub fn visit_null_check_pattern(&mut self, _node: Id<NullCheckPattern>) {}

    pub fn visit_object_pattern(&mut self, _node: Id<ObjectPattern>) {}

    pub fn visit_parenthesized_pattern(&mut self, _node: Id<ParenthesizedPattern>) {}

    pub fn visit_record_pattern(&mut self, _node: Id<RecordPattern>) {}

    pub fn visit_relational_pattern(&mut self, _node: Id<RelationalPattern>) {}

    pub fn visit_wildcard_pattern(&mut self, _node: Id<WildcardPattern>) {}

    // ------------------------------------------------------------ helpers

    /// Dart `flowAnalysis.withFlowAnalysis(node:, formalParameters: null,
    /// operation:)`: runs [operation] with flow analysis available.
    pub fn with_flow_analysis(&mut self, node: NodeId, operation: impl FnOnce(&mut Self)) {
        if self.flow_analysis.is_active() {
            operation(self);
            return;
        }
        {
            let ast = &*self.ast;
            self.flow_analysis
                .body_or_initializer_enter(ast, self.tables, node, None, None);
        }
        operation(self);
        self.flow_analysis.body_or_initializer_exit();
    }
}

/// Unused import guard.
#[allow(dead_code)]
fn _f<F: FlowAnalysis>(_: &F) {}

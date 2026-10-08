// Dart source: pkg/analyzer/lib/src/dart/ast/extensions.dart (the syntactic parts)

//! Syntactic helpers of the Dart AST extensions. The parts that read
//! elements and types belong to the resolver.

use crate::arena::{Ast, Id, NodeId};
use crate::generated::nodes::*;

impl Ast {
    /// Dart `ArgumentList.byName`: the named argument with [name].
    pub fn argument_by_name(
        &self,
        list: Id<ArgumentList>,
        name: &str,
    ) -> Option<Id<NamedArgument>> {
        self.list(self[list].arguments)
            .iter()
            .filter_map(|&a| self.cast::<NamedArgument>(a))
            .find(|&a| self.tokens.lexeme(self[a].name) == name)
    }

    /// Dart `ConstructorDeclaration.isNonRedirectingGenerative`.
    pub fn is_non_redirecting_generative(&self, node: Id<ConstructorDeclaration>) -> bool {
        let n = &self[node];
        if n.external_keyword.is_some() || n.factory_keyword.is_some() {
            return false;
        }
        !self
            .list_raw(n.initializers)
            .iter()
            .any(|&i| self.kind(i) == NodeKind::RedirectingConstructorInvocation)
    }

    /// Dart `FormalParameter.isOfLocalFunction`.
    pub fn is_of_local_function(&self, parameter: Id<FormalParameter>) -> bool {
        self.this_or_ancestor_of_type::<FunctionBody>(parameter)
            .is_some()
    }

    /// Dart `Identifier.simpleName`: the identifier, or the identifier of a
    /// prefixed identifier.
    pub fn simple_name(&self, identifier: Id<Identifier>) -> Id<SimpleIdentifier> {
        match self.kind(identifier) {
            NodeKind::SimpleIdentifier => Id::from_raw(identifier.raw()),
            NodeKind::PrefixedIdentifier => {
                self[Id::<PrefixedIdentifier>::from_raw(identifier.raw())].identifier
            }
            k => unreachable!("Identifier {k:?}"),
        }
    }

    /// Dart `NamedType.qualifiedName`: `prefix.Name` or `Name`.
    pub fn qualified_name(&self, named_type: Id<NamedType>) -> String {
        let n = &self[named_type];
        let name = self.tokens.lexeme(n.name);
        match n.import_prefix {
            Some(p) => format!("{}.{name}", self.tokens.lexeme(self[p].name)),
            None => name.to_string(),
        }
    }

    /// Dart `RecordTypeAnnotation.fields`: the positional fields, then the
    /// named fields.
    pub fn record_type_fields(&self, node: Id<RecordTypeAnnotation>) -> Vec<NodeId> {
        let n = &self[node];
        let mut fields = self.list_raw(n.positional_fields).to_vec();
        if let Some(named) = n.named_fields {
            fields.extend_from_slice(self.list_raw(self[named].fields));
        }
        fields
    }

    /// Dart `IdentifierImpl.toNamedType`: a named type with the tokens of
    /// [identifier] (`p.A` becomes a type with the import prefix `p.`).
    pub fn identifier_to_named_type(
        &mut self,
        identifier: Id<Identifier>,
        type_arguments: Option<Id<TypeArgumentList>>,
        question: Option<dartr_syntax::TokenId>,
    ) -> Id<NamedType> {
        match self.kind(identifier) {
            NodeKind::PrefixedIdentifier => {
                let p = self[Id::<PrefixedIdentifier>::from_raw(identifier.raw())].clone();
                let prefix_token = self[p.prefix].token;
                let name = self[p.identifier].token;
                let import_prefix = self.add(ImportPrefixReference {
                    name: prefix_token,
                    period: p.period,
                });
                self.add(NamedType {
                    import_prefix: Some(import_prefix),
                    name,
                    type_arguments,
                    question,
                })
            }
            NodeKind::SimpleIdentifier => {
                let name = self[Id::<SimpleIdentifier>::from_raw(identifier.raw())].token;
                self.add(NamedType {
                    import_prefix: None,
                    name,
                    type_arguments,
                    question,
                })
            }
            k => unreachable!("Identifier {k:?}"),
        }
    }
}

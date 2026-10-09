// Dart source: dart_style lib/src/front_end/type_builder.dart

use dartr_ast::*;
use dartr_syntax::TokenId;

use crate::ast_extensions::{first_non_comment_token, has_comma_before, has_non_empty_body};
use crate::back_end::code_writer::Indent;
use crate::piece::{
    ClausePiece, InfixPiece, ListStyle, PieceId, PrimaryTypePiece, TypeBodyType, TypePiece,
};

use super::ast_node_visitor::AstNodeVisitor;
use super::delimited_list_builder::DelimitedListBuilder;
use super::piece_factory::NodeContext;
use super::sequence_builder::SequenceBuilder;

/// Builds pieces for a class, enum, extension, extension type, mixin, or
/// mixin application class declaration.
pub struct TypeBuilder<'k> {
    metadata: NodeList<Annotation>,
    keywords: &'k [Option<TokenId>],
    name: Option<TokenId>,
    type_parameters: Option<Id<TypeParameterList>>,
    name_part: Option<Id<ClassNamePart>>,

    clauses: Vec<Clause>,

    /// Whether the first clause can be on the same line as the header even
    /// if the other clauses split.
    allow_leading_clause: bool,
}

/// The clauses of a type declaration (Dart named parameters of the
/// `TypeBuilder` constructor).
#[derive(Default)]
pub struct TypeClauses {
    pub extends_clause: Option<Id<ExtendsClause>>,
    pub with_clause: Option<Id<WithClause>>,
    pub implements_clause: Option<Id<ImplementsClause>>,
    pub mixin_on_clause: Option<Id<MixinOnClause>>,
    pub extension_on_clause: Option<Id<ExtensionOnClause>>,
    pub native_clause: Option<Id<NativeClause>>,
}

impl<'k> TypeBuilder<'k> {
    pub fn new(
        ast: &Ast,
        metadata: NodeList<Annotation>,
        keywords: &'k [Option<TokenId>],
        name: Option<TokenId>,
        type_parameters: Option<Id<TypeParameterList>>,
        name_part: Option<Id<ClassNamePart>>,
        clauses: TypeClauses,
    ) -> TypeBuilder<'k> {
        // Can have a name part or explicit name and type parameters, but not
        // both.
        debug_assert!(name.is_none() && type_parameters.is_none() || name_part.is_none());

        let mut list = Vec::new();
        if let Some(clause) = clauses.extends_clause {
            let clause = &ast[clause];
            list.push(Clause {
                keyword: clause.extends_keyword,
                types: vec![clause.superclass.raw()],
            });
        }
        if let Some(clause) = clauses.mixin_on_clause {
            let clause = &ast[clause];
            list.push(Clause {
                keyword: clause.on_keyword,
                types: ast.list_raw(clause.superclass_constraints).to_vec(),
            });
        }
        if let Some(clause) = clauses.with_clause {
            let clause = &ast[clause];
            list.push(Clause {
                keyword: clause.with_keyword,
                types: ast.list_raw(clause.mixin_types).to_vec(),
            });
        }
        if let Some(clause) = clauses.implements_clause {
            let clause = &ast[clause];
            list.push(Clause {
                keyword: clause.implements_keyword,
                types: ast.list_raw(clause.interfaces).to_vec(),
            });
        }
        if let Some(clause) = clauses.extension_on_clause {
            let clause = &ast[clause];
            list.push(Clause {
                keyword: clause.on_keyword,
                types: vec![clause.extended_type.raw()],
            });
        }
        if let Some(clause) = clauses.native_clause {
            let clause = &ast[clause];
            list.push(Clause {
                keyword: clause.native_keyword,
                types: clause.name.map(Id::raw).into_iter().collect(),
            });
        }

        TypeBuilder {
            metadata,
            keywords,
            name,
            type_parameters,
            name_part,
            clauses: list,
            allow_leading_clause: clauses.extends_clause.is_some()
                || clauses.mixin_on_clause.is_some(),
        }
    }

    /// Builds a type whose [body] is a [ClassBody].
    pub fn build_class_body(&self, v: &mut AstNodeVisitor<'_>, body: Id<ClassBody>) {
        let ast = v.ast;
        let body_type = if ast.is::<BlockClassBody>(body) {
            TypeBodyType::Block
        } else {
            TypeBodyType::Semicolon
        };
        self.build_type(
            v,
            |v| {
                if let Some(block) = ast.cast::<BlockClassBody>(body) {
                    let block = &ast[block];
                    v.build(|v| {
                        v.write_body(
                            block.left_bracket,
                            ast.list_raw(block.members),
                            block.right_bracket,
                            false,
                        );
                    })
                } else {
                    let empty = &ast[ast.cast::<EmptyClassBody>(body).unwrap()];
                    v.token_piece(empty.semicolon)
                }
            },
            body_type,
        );
    }

    /// Builds an enum type.
    pub fn build_enum(&self, v: &mut AstNodeVisitor<'_>, node: Id<EnumDeclaration>) {
        let ast = v.ast;
        let declaration = &ast[node];
        let body = declaration.body;

        let body_type = if let Some(block) = ast.cast::<BlockEnumBody>(body) {
            if ast[block].members.is_empty() {
                TypeBodyType::List
            } else {
                TypeBodyType::Block
            }
        } else {
            TypeBodyType::Semicolon
        };

        self.build_type(
            v,
            |v| {
                // If the enum has any members we definitely need to force the
                // body to split because there's a `;` in there. If it has a
                // primary constructor, we could allow it on one line. But
                // users generally wish enums were more eager to split and
                // having the constructor and values all on one line is pretty
                // hard to read:
                //
                //     enum E(final int x) { a(1), b(2) }
                //
                // So always force the body to split if there is a primary
                // constructor.
                if let Some(block) = ast.cast::<BlockEnumBody>(body) {
                    if ast[block].members.is_empty() {
                        build_normal_block_enum_body(
                            v,
                            block,
                            ast.is::<PrimaryConstructorDeclaration>(declaration.name_part),
                        )
                    } else {
                        build_enhanced_block_enum_body(v, block)
                    }
                } else {
                    let empty = &ast[ast.cast::<EmptyEnumBody>(body).unwrap()];
                    v.token_piece(empty.semicolon)
                }
            },
            body_type,
        );
    }

    /// Builds a mixin application class.
    pub fn build_mixin_application_class(
        &self,
        v: &mut AstNodeVisitor<'_>,
        equals: TokenId,
        superclass: Id<NamedType>,
        semicolon: TokenId,
    ) {
        let ast = v.ast;
        v.with_metadata(ast.list_raw(self.metadata), false, |v| {
            let header = v.build(|v| {
                self.build_header(v, true);

                // Mixin application classes have ` = Superclass` after the
                // declaration name.
                v.space();
                v.token(equals);
                v.space();
                v.visit(superclass);
            });

            let header = self.build_clauses(v, header);

            let semicolon = v.token_piece(semicolon);
            let piece = v
                .arena
                .add(TypePiece::new(header, semicolon, TypeBodyType::Semicolon));
            v.add(piece);
        });
    }

    fn build_type<'a>(
        &self,
        v: &mut AstNodeVisitor<'a>,
        build_body: impl FnOnce(&mut AstNodeVisitor<'a>) -> PieceId,
        body_type: TypeBodyType,
    ) {
        let ast = v.ast;
        v.with_metadata(ast.list_raw(self.metadata), false, |v| {
            if let Some(constructor) = self
                .name_part
                .and_then(|part| ast.cast::<PrimaryConstructorDeclaration>(part))
            {
                let header = v.build(|v| {
                    self.build_header(v, false);
                });

                let parameters = v.node_piece(ast[constructor].formal_parameters);
                let clauses = self.clauses.iter().map(|clause| clause.build(v)).collect();

                let body_piece = build_body(v);

                let piece = v.arena.add(PrimaryTypePiece::new(
                    header, parameters, clauses, body_piece, body_type,
                ));
                v.add(piece);
            } else {
                let header = v.build(|v| self.build_header(v, true));
                let header = self.build_clauses(v, header);
                let body_piece = build_body(v);
                let piece = v.arena.add(TypePiece::new(header, body_piece, body_type));
                v.add(piece);
            }
        });
    }

    /// Writes the leading keywords, name, and type parameters for the type.
    fn build_header(&self, v: &mut AstNodeVisitor<'_>, include_parameters: bool) {
        let ast = v.ast;
        let mut space = false;
        for &keyword in self.keywords {
            if space {
                v.space();
            }
            v.token(keyword);
            if keyword.is_some() {
                space = true;
            }
        }

        match self.name_part {
            None => {
                v.token_with(self.name, space, false, false);
                v.visit(self.type_parameters);
            }
            Some(part) => {
                if let Some(name) = ast.cast::<NameWithTypeParameters>(part) {
                    let name = &ast[name];
                    v.token_with(name.type_name, space, false, false);
                    v.visit(name.type_parameters);
                } else {
                    let primary = &ast[ast.cast::<PrimaryConstructorDeclaration>(part).unwrap()];
                    v.token_with(primary.const_keyword, space, false, false);
                    v.token_with(primary.type_name, true, false, false);
                    v.visit(primary.type_parameters);
                    v.visit(primary.constructor_name);
                    if include_parameters {
                        v.visit(primary.formal_parameters);
                    }
                }
            }
        }
    }

    /// If there are any clauses, wraps [header] in a [ClausePiece] for them.
    fn build_clauses(&self, v: &mut AstNodeVisitor<'_>, header: PieceId) -> PieceId {
        if self.clauses.is_empty() {
            return header;
        }

        let clauses = self.clauses.iter().map(|clause| clause.build(v)).collect();
        v.arena
            .add(ClausePiece::new(header, clauses, self.allow_leading_clause))
    }
}

/// Builds a [Piece] for the body of an enum declaration with values but not
/// members.
///
/// Formats the constants like a list. This keeps the enum declaration on one
/// line if it fits (unless [force_split] is `true` in which case it always
/// splits).
fn build_normal_block_enum_body(
    v: &mut AstNodeVisitor<'_>,
    body: Id<BlockEnumBody>,
    force_split: bool,
) -> PieceId {
    let ast = v.ast;
    let body = &ast[body];
    let mut builder = DelimitedListBuilder::new(ListStyle {
        space_when_unsplit: true,
        ..ListStyle::default()
    });

    builder.left_bracket(v, body.left_bracket);
    for &constant in ast.list_raw(body.constants) {
        builder.visit(v, constant);
    }
    builder.right_bracket(v, body.right_bracket, None, body.semicolon);
    let force_split = force_split
        || v.style
            .preserve_trailing_comma_before(ast, body.semicolon.unwrap_or(body.right_bracket));
    builder.build_with(v, force_split, true)
}

/// Builds a [Piece] for the body of an enum declaration with members.
///
/// Formats it like a block where each constant or member is on its own
/// line.
fn build_enhanced_block_enum_body(v: &mut AstNodeVisitor<'_>, body: Id<BlockEnumBody>) -> PieceId {
    let ast = v.ast;
    let body = &ast[body];
    // If there are members, format it like a block where each constant and
    // member is on its own line.
    let mut builder = SequenceBuilder::new();
    builder.left_bracket(v, body.left_bracket, false);

    // In 3.10 and later, if the source has a trailing comma before the `;`,
    // it is preserved and the `;` is put on its own line. If there is no
    // trailing comma in the source, the `;` stays on the same line as the
    // last constant. Prior to 3.10, the behavior is the same as when
    // preserved trailing commas is off: the last constant's comma is removed
    // and the `;` is placed there instead.
    let preserve_trailing_comma = v.style.preserve_trailing_comma_after_enum_values()
        && has_comma_before(ast, body.semicolon.unwrap());
    let constants = ast.list(body.constants);
    for (i, &constant) in constants.iter().enumerate() {
        let is_last = i == constants.len() - 1;
        builder.add_comments_before(v, first_non_comment_token(ast, constant.raw()));
        let piece = v.create_enum_constant(
            constant,
            !is_last || preserve_trailing_comma,
            if is_last { body.semicolon } else { None },
        );
        builder.add(v, piece);
    }

    // If we are preserving the trailing comma, then put the `;` on its own
    // line after the last constant.
    if preserve_trailing_comma {
        let piece = v.token_piece(body.semicolon.unwrap());
        builder.add(v, piece);
    }

    // Insert a blank line between the constants and members.
    builder.add_blank(v);

    let mut needs_blank = false;
    for &member in ast.list_raw(body.members) {
        if needs_blank {
            builder.add_blank(v);
        }

        builder.visit(v, member);

        // If the node has a non-empty braced body, then require a blank line
        // between it and the next node.
        needs_blank = has_non_empty_body(ast, member);
    }

    builder.right_bracket(v, body.right_bracket);
    builder.build(v)
}

/// A single `extends`, `with`, etc. clause that goes in a type header.
struct Clause {
    keyword: TokenId,
    types: Vec<NodeId>,
}

impl Clause {
    fn build(&self, v: &mut AstNodeVisitor<'_>) -> PieceId {
        let mut operands = vec![v.token_piece(self.keyword)];
        for &ty in &self.types {
            operands.push(v.node_piece_with(ty, true, NodeContext::None));
        }
        let is_3_dot_7 = v.style.is_3_dot_7();
        v.arena
            .add(InfixPiece::new(operands, is_3_dot_7, Indent::Expression))
    }
}

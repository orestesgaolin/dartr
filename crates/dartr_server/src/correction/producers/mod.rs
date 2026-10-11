//! The correction producers (Dart `services/correction/dart/*.dart`), by
//! the name of the Dart generator in the fix registry.

pub mod assists;
pub mod assists2;
pub mod assists3;
pub mod assists4;
pub mod assists5;
pub mod bulk_lints;
pub mod collections;
pub mod convert_quotes;
pub mod create;
pub mod create_types;
pub mod data_driven;
pub mod data_driven_changes;
pub mod dead_code;
pub mod extensions;
pub mod flutter_convert;
pub mod flutter_edit;
pub mod flutter_wrap;
pub mod import_library;
pub mod members;
pub mod modifiers;
pub mod more;
pub mod more2;
pub mod simple;
pub mod small;
pub mod super_parameters;
pub mod types;
pub mod variables;

use super::change_builder::ChangeWorkspace;
use super::producer::{
    Applicability, CorrectionProducer, MultiProducerGenerator, ProducerContext, ProducerGenerator,
};

fn boxed<P: CorrectionProducer + 'static>(p: P) -> Box<dyn CorrectionProducer> {
    Box::new(p)
}

/// The generator of the producer [name] (Dart `ProducerGenerator`), `None`
/// when dartr does not implement it.
pub fn generator(name: &str) -> Option<ProducerGenerator> {
    use convert_quotes::{ConvertQuotes, QuotesKind};
    use modifiers::*;
    use simple::*;
    use variables::*;
    type C<'a, 'b> = &'a ProducerContext<'b>;
    let g: ProducerGenerator = match name {
        "AddConst.new" => |_: C| boxed(AddConst),
        "AddReturnType.new" => |_: C| boxed(create::AddReturnType),
        "AddLate.new" => |_: C| boxed(AddLate { this_: false }),
        "AddLate.this_" => |_: C| boxed(AddLate { this_: true }),
        "AddNullCheck.new" => |c: C| boxed(small::AddNullCheck::new(c, false)),
        "AddNullCheck.withoutAssignabilityCheck" => |c: C| boxed(small::AddNullCheck::new(c, true)),
        "AddOverride.new" => |_: C| boxed(AddOverride),
        "AddRequiredKeyword.new" => |_: C| boxed(AddRequiredKeyword),
        "ConvertQuotes.new" => |_: C| boxed(ConvertQuotes::new(QuotesKind::Swap)),
        "ConvertToDoubleQuotes.new" => |_: C| boxed(ConvertQuotes::new(QuotesKind::ToDouble)),
        "ConvertToSingleQuotes.new" => |_: C| boxed(ConvertQuotes::new(QuotesKind::ToSingle)),
        "ConvertToWildcardVariable.new" => |_: C| {
            boxed(ConvertToWildcardVariable {
                automatically: false,
            })
        },
        "ConvertToWildcardVariable.automatically" => |_: C| {
            boxed(ConvertToWildcardVariable {
                automatically: true,
            })
        },
        "ConvertToMapLiteral.new" => |_: C| boxed(collections::ConvertToMapLiteral),
        "ReplaceWithIsEmpty.new" => |c: C| boxed(collections::ReplaceWithIsEmpty::new(c)),
        "ConvertToSuperParameters.new" => |_: C| boxed(super_parameters::ConvertToSuperParameters),
        "CreateFunction.new" => |_: C| boxed(create::CreateFunction::new()),
        "CreateExtensionGetter.new" => |_: C| boxed(extensions::CreateExtensionGetter::new()),
        "CreateExtensionMethod.new" => |_: C| boxed(extensions::CreateExtensionMethod::new()),
        "CreateField.new" => |_: C| boxed(members::CreateMember::new(members::MemberKind::Field)),
        "CreateGetter.new" => |_: C| boxed(members::CreateMember::new(members::MemberKind::Getter)),
        "CreateSetter.new" => |_: C| boxed(members::CreateMember::new(members::MemberKind::Setter)),
        "CreateParameter.new" => |_: C| boxed(create::CreateParameter::new()),
        "CreateLocalVariable.new" => |_: C| boxed(create::CreateLocalVariable::new()),
        "CreateMethod.method" => |_: C| boxed(create::CreateMethod::new()),
        "ConvertToExpressionFunctionBody.new" => {
            |_: C| boxed(assists::ConvertToExpressionFunctionBody)
        }
        "ConvertIntoBlockBody.missingBody" => |_: C| boxed(assists::ConvertIntoBlockBody),
        "ExchangeOperands.new" => |_: C| boxed(assists::ExchangeOperands),
        "SplitVariableDeclaration.new" => |_: C| boxed(assists::SplitVariableDeclaration),
        "ConvertToPackageImport.new" => |_: C| boxed(assists::ConvertToPackageImport),
        "ConvertToRelativeImport.new" => |_: C| boxed(assists::ConvertToRelativeImport),
        "RemoveDigitSeparators.new" => |_: C| boxed(assists::RemoveDigitSeparators),
        "AddTypeAnnotation.new" => |_: C| {
            boxed(assists::AddTypeAnnotation {
                applicability: Applicability::SingleLocation,
                for_representation_field: false,
            })
        },
        "AddTypeAnnotation.bulkFixable" => |_: C| {
            boxed(assists::AddTypeAnnotation {
                applicability: Applicability::Automatically,
                for_representation_field: false,
            })
        },
        "AddTypeAnnotation.forRepresentationField" => |_: C| {
            boxed(assists::AddTypeAnnotation {
                applicability: Applicability::SingleLocation,
                for_representation_field: true,
            })
        },
        "UseCurlyBraces.new" => |_: C| {
            boxed(assists::UseCurlyBraces {
                applicability: Applicability::AcrossFiles,
            })
        },
        "UseCurlyBraces.nonBulk" => |_: C| {
            boxed(assists::UseCurlyBraces {
                applicability: Applicability::AcrossSingleFile,
            })
        },
        "MakeClassAbstract.new" => |_: C| boxed(more::MakeClassAbstract::new()),
        "ConvertToMultilineString.new" => |_: C| boxed(more::ConvertToMultilineString),
        "ConvertDocumentationIntoBlock.new" => |_: C| boxed(more::ConvertDocumentationIntoBlock),
        "ConvertIntoAsyncBody.new" => |_: C| boxed(more::ConvertIntoAsyncBody),
        "CreateNoSuchMethod.new" => |_: C| boxed(more::CreateNoSuchMethod),
        "ReplaceFinalWithVar.new" => |c: C| boxed(types::ReplaceFinalWithVar::new(c)),
        "RemoveUnnecessaryFinal.new" => |_: C| boxed(types::RemoveUnnecessaryFinal),
        "RemoveVar.new" => |_: C| boxed(types::RemoveVar),
        "RemoveTypeAnnotation.other" => |_: C| {
            boxed(types::RemoveTypeAnnotation {
                var_and_type: false,
            })
        },
        "RemoveTypeAnnotation.fixVarAndType" => {
            |_: C| boxed(types::RemoveTypeAnnotation { var_and_type: true })
        }
        "ReplaceWithVar.new" => |_: C| boxed(types::ReplaceWithVar),
        "RemoveNonNullAssertion.new" => |_: C| boxed(types::RemoveNonNullAssertion),
        "ReplaceCascadeWithDot.new" => |_: C| boxed(types::ReplaceCascadeWithDot),
        "ReplaceWithIdentifier.new" => |_: C| boxed(types::ReplaceWithIdentifier),
        "RemoveDeadCode.new" => |_: C| boxed(dead_code::RemoveDeadCode),
        "ImportAddShow.new" => |_: C| boxed(more::ImportAddShow),
        "CreateExtensionSetter.new" => |_: C| boxed(extensions::CreateExtensionSetter::new()),
        "CreateMissingOverrides.new" => |_: C| boxed(more::CreateMissingOverrides::new()),
        "FlutterMoveUp.new" => |_: C| boxed(flutter_edit::FlutterMoveUp),
        "FlutterMoveDown.new" => |_: C| boxed(flutter_edit::FlutterMoveDown),
        "FlutterConvertToChildren.new" => |_: C| boxed(flutter_edit::FlutterConvertToChildren),
        "FlutterSwapWithChild.new" => |_: C| boxed(flutter_edit::FlutterSwapWithChild),
        "FlutterSwapWithParent.new" => |_: C| boxed(flutter_edit::FlutterSwapWithParent),
        "FlutterConvertToStatefulWidget.new" => {
            |_: C| boxed(flutter_convert::FlutterConvertToStatefulWidget)
        }
        "FlutterConvertToStatelessWidget.new" => {
            |_: C| boxed(flutter_convert::FlutterConvertToStatelessWidget)
        }
        "FlutterRemoveWidget.new" => |_: C| boxed(flutter_edit::FlutterRemoveWidget),
        "FlutterWrapGeneric.new" => |_: C| boxed(flutter_wrap::FlutterWrapGeneric),
        "InvertIfStatement.new" => |_: C| boxed(assists2::InvertIfStatement),
        "InvertConditionalExpression.new" => |_: C| boxed(assists2::InvertConditionalExpression),
        "ConvertToIntLiteral.new" => |_: C| boxed(assists2::ConvertToIntLiteral),
        "ReplaceConditionalWithIfElse.new" => |_: C| boxed(assists2::ReplaceConditionalWithIfElse),
        "AssignToLocalVariable.new" => |_: C| boxed(assists2::AssignToLocalVariable),
        "ConvertIntoFinalField.new" => |_: C| boxed(assists2::ConvertIntoFinalField),
        "ConvertIntoGetter.new" => |_: C| boxed(assists2::ConvertIntoGetter::new(false)),
        "ConvertIntoGetter.this_" => |_: C| boxed(assists2::ConvertIntoGetter::new(true)),
        "AddTypeName.new" => |_: C| boxed(assists3::AddTypeName),
        "BindAllToFields.new" => |_: C| boxed(assists3::BindAllToFields),
        "BindToField.new" => |_: C| boxed(assists3::BindToField),
        "ConvertFieldFormalToNormal.new" => |_: C| boxed(assists3::ConvertFieldFormalToNormal),
        "ConvertToDeclaringParameter.new" => |_: C| boxed(assists3::ConvertToDeclaringParameter),
        "ConvertToDotShorthand.new" => |_: C| boxed(assists3::ConvertToDotShorthand),
        "ConvertToInBodyConstructor.new" => |_: C| boxed(assists3::ConvertToInBodyConstructor),
        "ConvertToInitializingFormal.new" => |_: C| boxed(assists3::ConvertToInitializingFormal),
        "ConvertToPrimaryConstructor.new" => |_: C| boxed(assists3::ConvertToPrimaryConstructor),
        "EncapsulateField.new" => |_: C| boxed(assists3::EncapsulateField),
        "MoveInitializationToFieldDeclaration.new" => {
            |_: C| boxed(assists3::MoveInitializationToFieldDeclaration)
        }
        "ConvertIfStatementToSwitchStatement.new" => {
            |_: C| boxed(assists4::ConvertIfStatementToSwitchStatement)
        }
        "ConvertSwitchExpressionToSwitchStatement.new" => {
            |_: C| boxed(assists4::ConvertSwitchExpressionToSwitchStatement)
        }
        "ConvertToIfCaseStatement.new" => |_: C| boxed(assists4::ConvertToIfCaseStatement),
        "ConvertToIfCaseStatementChain.new" => {
            |_: C| boxed(assists4::ConvertToIfCaseStatementChain)
        }
        "ConvertToSwitchExpression.new" => |_: C| boxed(assists4::ConvertToSwitchExpression),
        "JoinElseWithIf.new" => |_: C| boxed(assists4::JoinElseWithIf),
        "JoinIfWithElse.new" => |_: C| boxed(assists4::JoinIfWithElse),
        "JoinIfWithInner.new" => |_: C| boxed(assists4::JoinIfWithInner),
        "JoinIfWithOuter.new" => |_: C| boxed(assists4::JoinIfWithOuter),
        "ReplaceIfElseWithConditional.new" => |_: C| boxed(assists4::ReplaceIfElseWithConditional),
        "SplitAndCondition.new" => |_: C| boxed(assists4::SplitAndCondition),
        "AddDiagnosticPropertyReference.new" => {
            |_: C| boxed(assists5::AddDiagnosticPropertyReference)
        }
        "AddDigitSeparatorEveryThreeDigits.new" => {
            |_: C| boxed(assists5::AddDigitSeparatorEveryThreeDigits)
        }
        "AddDigitSeparatorEveryTwoDigits.new" => {
            |_: C| boxed(assists5::AddDigitSeparatorEveryTwoDigits)
        }
        "ConvertAddAllToSpread.new" => |_: C| boxed(assists5::ConvertAddAllToSpread::new()),
        "ConvertClassToEnum.new" => |_: C| boxed(assists5::ConvertClassToEnum),
        "ConvertClassToMixin.new" => |_: C| boxed(assists5::ConvertClassToMixin),
        "ConvertConditionalExpressionToIfElement.new" => {
            |_: C| boxed(assists5::ConvertConditionalExpressionToIfElement)
        }
        "ConvertDocumentationIntoLine.new" => |_: C| boxed(assists5::ConvertDocumentationIntoLine),
        "ConvertIntoForIndex.new" => |_: C| boxed(assists5::ConvertIntoForIndex),
        "ConvertIntoIsNot.new" => |_: C| boxed(assists5::ConvertIntoIsNot),
        "ConvertIntoIsNotEmpty.new" => |_: C| boxed(assists5::ConvertIntoIsNotEmpty),
        "ConvertMapFromIterableToForLiteral.new" => {
            |_: C| boxed(assists5::ConvertMapFromIterableToForLiteral)
        }
        "ConvertPartOfToUri.new" => |_: C| boxed(assists5::ConvertPartOfToUri),
        "ConvertToGenericFunctionSyntax.new" => {
            |_: C| boxed(assists5::ConvertToGenericFunctionSyntax)
        }
        "ConvertToNullAware.new" => |_: C| boxed(assists5::ConvertToNullAware),
        "ConvertToSetLiteral.new" => |_: C| boxed(assists5::ConvertToSetLiteral),
        "DestructureLocalVariableAssignment.new" => {
            |_: C| boxed(assists5::DestructureLocalVariableAssignment)
        }
        "InlineInvocation.new" => |_: C| boxed(assists5::InlineInvocation),
        "JoinVariableDeclaration.new" => |_: C| boxed(assists5::JoinVariableDeclaration),
        "RemoveAsync.new" => |_: C| boxed(assists5::RemoveAsync),
        "RemoveUnnecessaryName.new" => |_: C| boxed(assists5::RemoveUnnecessaryName),
        "ShadowField.new" => |_: C| boxed(assists5::ShadowField),
        "SortChildPropertyLast.new" => |_: C| boxed(assists5::SortChildPropertyLast),
        "AddReturnNull.new" => |_: C| boxed(more2::AddReturnNull),
        "RemoveComment.ignore" => |_: C| boxed(more2::RemoveIgnoreComment),
        "RemoveIgnoredDiagnostic.new" => |_: C| boxed(more2::RemoveIgnoredDiagnostic::new()),
        "MakeFinal.new" => |_: C| boxed(MakeFinal),
        "MakeVariableNullable.new" => |_: C| boxed(MakeVariableNullable::new()),
        "RemoveAnnotation.new" => |_: C| boxed(small::RemoveAnnotation::new()),
        "RemoveNameFromDeclarationClause.new" => {
            |_: C| boxed(small::RemoveNameFromDeclarationClause::new())
        }
        "ReplaceWithNullAware.inChain" => |_: C| boxed(small::ReplaceWithNullAware::new(true)),
        "ReplaceWithNullAware.single" => |_: C| boxed(small::ReplaceWithNullAware::new(false)),
        "RemoveEmptyConstructorBody.new" => |_: C| boxed(RemoveEmptyConstructorBody),
        "RemoveInitializer.new" => |_: C| {
            boxed(RemoveInitializer {
                applicability: Applicability::SingleLocation,
                remove_late: true,
            })
        },
        "RemoveInitializer.bulkFixable" => |_: C| {
            boxed(RemoveInitializer {
                applicability: Applicability::Automatically,
                remove_late: true,
            })
        },
        "RemoveInitializer.notLate" => |_: C| {
            boxed(RemoveInitializer {
                applicability: Applicability::SingleLocation,
                remove_late: false,
            })
        },
        "RemoveInterpolationBraces.new" => |_: C| boxed(RemoveInterpolationBraces),
        "RemoveMethodDeclaration.new" => |_: C| boxed(RemoveMethodDeclaration),
        "RemoveNew.new" => |_: C| boxed(RemoveNew),
        "RemoveThisExpression.new" => |_: C| boxed(RemoveThisExpression),
        "RemoveUnnecessaryNew.new" => |_: C| boxed(RemoveUnnecessaryNew),
        "RemoveUnnecessaryParentheses.new" => |_: C| boxed(RemoveUnnecessaryParentheses),
        "RemoveUnusedImport.new" => |_: C| boxed(RemoveUnusedImport),
        "RemoveUnusedLocalVariable.new" => |_: C| boxed(RemoveUnusedLocalVariable),
        "ReplaceFinalWithConst.new" => |_: C| boxed(ReplaceFinalWithConst),
        "ReplaceWithConditionalAssignment.new" => |_: C| boxed(ReplaceWithConditionalAssignment),
        "AddTrailingComma.new" => |_: C| boxed(bulk_lints::AddTrailingComma),
        "SortConstructorFirst.new" => |_: C| boxed(bulk_lints::SortConstructorFirst),
        "SortUnnamedConstructorFirst.new" => |_: C| boxed(bulk_lints::SortUnnamedConstructorFirst),
        "MakeRequiredNamedParametersFirst.new" => {
            |_: C| boxed(bulk_lints::MakeRequiredNamedParametersFirst)
        }
        "ReplaceWithTearOff.new" => |_: C| boxed(bulk_lints::ReplaceWithTearOff),
        "RemoveArgument.new" => |_: C| boxed(bulk_lints::RemoveArgument),
        "RemoveUnnecessaryLibraryDirective.new" => {
            |_: C| boxed(bulk_lints::RemoveUnnecessaryLibraryDirective)
        }
        "RemoveLibraryName.new" => |_: C| boxed(bulk_lints::RemoveLibraryName),
        "SplitMultipleDeclarations.new" => |_: C| boxed(bulk_lints::SplitMultipleDeclarations),
        "RemoveUnnecessaryRawString.new" => |_: C| boxed(bulk_lints::RemoveUnnecessaryRawString),
        "ConvertToRawString.new" => |_: C| boxed(bulk_lints::ConvertToRawString),
        "SortCombinators.new" => |_: C| boxed(bulk_lints::SortCombinators),
        "RemoveKeyword.awaitKeyword" => |_: C| boxed(bulk_lints::RemoveAwait),
        "RemoveLeadingUnderscore.new" => |_: C| boxed(bulk_lints::RemoveLeadingUnderscore),
        "ConvertToBooleanExpression.new" => |_: C| boxed(bulk_lints::ConvertToBooleanExpression),
        "InlineTypedef.new" => |_: C| boxed(bulk_lints::InlineTypedef),
        "RemoveInvocation.new" => |_: C| boxed(bulk_lints::RemoveInvocation),
        "RemoveBreak.new" => |_: C| boxed(bulk_lints::RemoveBreak),
        "ConvertToIfNull.preferIfNull" => |_: C| {
            boxed(bulk_lints::ConvertToIfNull {
                prefer_if_null: true,
            })
        },
        "ConvertToIfNull.useToConvertNullsToBools" => |_: C| {
            boxed(bulk_lints::ConvertToIfNull {
                prefer_if_null: false,
            })
        },
        "ConvertForEachToForLoop.new" => |_: C| boxed(bulk_lints::ConvertForEachToForLoop),
        "RemoveConst.new" => |_: C| boxed(bulk_lints::RemoveConstProducer { unnecessary: false }),
        "RemoveUnnecessaryConst.new" => {
            |_: C| boxed(bulk_lints::RemoveConstProducer { unnecessary: true })
        }
        "AddLeadingNewlineToString.new" => |_: C| boxed(bulk_lints::AddLeadingNewlineToString),
        "ConvertToFunctionDeclaration.new" => {
            |_: C| boxed(bulk_lints::ConvertToFunctionDeclaration)
        }
        "ReplaceWithInterpolation.new" => |_: C| boxed(bulk_lints::ReplaceWithInterpolation),
        "RemoveComparison.new" => |_: C| {
            boxed(bulk_lints::RemoveComparison {
                is_type_check: false,
            })
        },
        "RemoveComparison.typeCheck" => |_: C| {
            boxed(bulk_lints::RemoveComparison {
                is_type_check: true,
            })
        },
        "RemoveUnusedParameter.new" => |_: C| boxed(bulk_lints::RemoveUnusedParameter),
        "ReplaceReturnTypeFuture.new" => |_: C| boxed(bulk_lints::ReplaceReturnTypeFuture),
        "MoveDocCommentToLibraryDirective.new" => {
            |_: C| boxed(bulk_lints::MoveDocCommentToLibraryDirective)
        }
        "RemoveUnnecessaryCast.new" => |_: C| boxed(bulk_lints::RemoveUnnecessaryCast),
        "RemoveUnnecessaryStringInterpolation.new" => {
            |_: C| boxed(bulk_lints::RemoveUnnecessaryStringInterpolation)
        }
        "ReplaceWithDecoratedBox.new" => |_: C| boxed(bulk_lints::ReplaceWithDecoratedBox),
        _ => return None,
    };
    Some(g)
}

/// The generator of the multi producer [name].
pub fn multi_generator(name: &str) -> Option<MultiProducerGenerator> {
    use import_library::{ImportKind, import_library_producers};
    type C<'a, 'b> = &'a ProducerContext<'b>;
    type W<'a> = &'a mut dyn ChangeWorkspace;
    match name {
        "CreateClass.new" => return Some(create_types::create_class_producers),
        "CreateMixin.new" => return Some(create_types::create_mixin_producers),
        "DataDriven.new" => return Some(data_driven::data_driven_producers),
        "FlutterWrap.new" => return Some(flutter_wrap::flutter_wrap_producers),
        "FlutterWrapBuilders.new" => return Some(flutter_wrap::flutter_wrap_builders_producers),
        "SurroundWith.new" => return Some(assists4::surround_with_producers),
        _ => {}
    }
    let g: MultiProducerGenerator = match ImportKind::from_name(name)? {
        ImportKind::ForExtension => {
            |c: C, w: W| import_library_producers(ImportKind::ForExtension, c, w)
        }
        ImportKind::ForExtensionMember => {
            |c: C, w: W| import_library_producers(ImportKind::ForExtensionMember, c, w)
        }
        ImportKind::ForExtensionType => {
            |c: C, w: W| import_library_producers(ImportKind::ForExtensionType, c, w)
        }
        ImportKind::ForFunction => {
            |c: C, w: W| import_library_producers(ImportKind::ForFunction, c, w)
        }
        ImportKind::ForTopLevelVariable => {
            |c: C, w: W| import_library_producers(ImportKind::ForTopLevelVariable, c, w)
        }
        ImportKind::ForType => |c: C, w: W| import_library_producers(ImportKind::ForType, c, w),
    };
    Some(g)
}

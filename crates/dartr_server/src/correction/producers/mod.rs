//! The correction producers (Dart `services/correction/dart/*.dart`), by
//! the name of the Dart generator in the fix registry.

pub mod assists;
pub mod collections;
pub mod convert_quotes;
pub mod create;
pub mod create_types;
pub mod data_driven;
pub mod extensions;
pub mod import_library;
pub mod members;
pub mod modifiers;
pub mod simple;
pub mod small;
pub mod super_parameters;
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

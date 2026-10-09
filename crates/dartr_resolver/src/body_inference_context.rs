// Dart source: pkg/analyzer/lib/src/dart/resolver/body_inference_context.dart

//! [`BodyInferenceContext`]: return type inference of a function body.

use dartr_element::{Ctx, EId, InterfaceElement, TypeId, TypeKind};
use dartr_flow::body_inference_context::SharedBodyInferenceContext;
use dartr_flow::shared_type::SharedTypeSchemaView;
use dartr_typesystem::{TypeExt, TypeSystem};

/// Dart `BodyInferenceContext`.
#[derive(Clone, Debug)]
pub struct BodyInferenceContext {
    pub is_async: bool,
    pub is_generator: bool,
    /// The imposed return type, from the typing context. `None` for an empty
    /// typing context.
    pub imposed_type: Option<TypeId>,
    /// The context type, computed from [`Self::imposed_type`].
    pub context_type: Option<TypeId>,
    /// Types of all `return` or `yield` statements in the body.
    return_types: Vec<TypeId>,
    /// Whether the execution flow can reach the end of the body.
    pub may_complete_normally: bool,
}

impl BodyInferenceContext {
    /// Dart `BodyInferenceContext(typeSystem:, node:, imposedType:)`. The
    /// caller stores it as the body context of the body node.
    pub fn new(
        type_system: &TypeSystem<'_>,
        is_async: bool,
        is_generator: bool,
        imposed_type: Option<TypeId>,
    ) -> BodyInferenceContext {
        let context_type =
            context_type_for_imposed(type_system, is_async, is_generator, imposed_type);
        BodyInferenceContext {
            is_async,
            is_generator,
            imposed_type,
            context_type,
            return_types: Vec::new(),
            may_complete_normally: true,
        }
    }

    /// Dart `BodyInferenceContext.forAnonymousBlockBody`.
    pub fn for_anonymous_block_body(imposed_type: Option<TypeId>) -> BodyInferenceContext {
        BodyInferenceContext {
            is_async: false,
            is_generator: false,
            imposed_type,
            context_type: imposed_type,
            return_types: Vec::new(),
            may_complete_normally: true,
        }
    }

    pub fn is_sync(&self) -> bool {
        !self.is_async
    }

    /// Dart `addReturnExpression`: [expression_type] is the static type of
    /// the returned expression, `None` for `return;`.
    pub fn add_return_expression(&mut self, ts: &TypeSystem<'_>, expression_type: Option<TypeId>) {
        match expression_type {
            None => {
                // If the enclosing function is not marked `sync*` or `async*`:
                //   For each `return;` statement in the block, update
                //   `T` to be `UP(Null, T)`.
                if !self.is_generator {
                    self.return_types.push(ts.ctx.tp.null_type());
                }
            }
            Some(mut ty) => {
                if self.is_async {
                    ty = ts.flatten(ty);
                }
                self.return_types.push(ty);
            }
        }
    }

    /// Dart `addYield`: [expression_type] is the static type of the yielded
    /// expression, [has_star] is `yield*`.
    pub fn add_yield(&mut self, ts: &TypeSystem<'_>, expression_type: TypeId, has_star: bool) {
        if !has_star {
            self.return_types.push(expression_type);
            return;
        }
        if self.is_generator {
            let tp = ts.ctx.tp;
            let required_class = if self.is_async {
                tp.stream_element()
            } else {
                tp.iterable_element()
            };
            if let Some(ty) = argument_of(&ts.ctx, expression_type, required_class.upcast()) {
                self.return_types.push(ty);
            }
        }
    }

    /// Dart `computeInferredReturnType`.
    pub fn compute_inferred_return_type(
        &self,
        ts: &TypeSystem<'_>,
        end_of_block_is_reachable: bool,
    ) -> TypeId {
        let ctx = ts.ctx;
        let actual_returned_type = self.compute_actual_returned_type(ts, end_of_block_is_reachable);
        let clamped_returned_type = self.clamp_to_context_type(ts, actual_returned_type);
        if self.is_generator {
            if self.is_async {
                ctx.tp.stream_type(&ctx, clamped_returned_type)
            } else {
                ctx.tp.iterable_type(&ctx, clamped_returned_type)
            }
        } else if self.is_async {
            ctx.tp.future_type(&ctx, ts.flatten(clamped_returned_type))
        } else {
            clamped_returned_type
        }
    }

    /// Dart `_clampToContextType`: let `T` be the actual returned type of a
    /// function literal.
    fn clamp_to_context_type(&self, ts: &TypeSystem<'_>, t: TypeId) -> TypeId {
        let ctx = ts.ctx;
        // Let `R` be the greatest closure of the typing context `K`.
        let Some(r) = self.context_type else {
            return t;
        };
        // If `R` is `void`, or the function literal is marked `async` and `R`
        // is `FutureOr<void>`, let `S` be `void`.
        if matches!(ctx.ty(r), TypeKind::Void)
            || self.is_async
                && matches!(ctx.ty(r), TypeKind::Interface { .. })
                && ctx.is_dart_async_future_or(r)
                && matches!(ctx.ty(ctx.type_arguments(r)[0]), TypeKind::Void)
        {
            return TypeId::VOID;
        }
        // Otherwise, if `T <: R` then let `S` be `T`.
        if ts.is_subtype_of(t, r) {
            return t;
        }
        // Otherwise, let `S` be `R`.
        r
    }

    /// Dart `_computeActualReturnedType`.
    fn compute_actual_returned_type(
        &self,
        ts: &TypeSystem<'_>,
        end_of_block_is_reachable: bool,
    ) -> TypeId {
        if self.is_generator {
            let Some(&first) = self.return_types.first() else {
                return TypeId::DYNAMIC;
            };
            let mut value = first;
            for &t in &self.return_types[1..] {
                value = ts.least_upper_bound(value, t);
            }
            return value;
        }
        let initial_type = if end_of_block_is_reachable {
            ts.ctx.tp.null_type()
        } else {
            ts.ctx.tp.never_type()
        };
        let mut value = initial_type;
        for &t in &self.return_types {
            value = ts.least_upper_bound(value, t);
        }
        value
    }
}

impl SharedBodyInferenceContext<TypeId> for BodyInferenceContext {
    fn is_async(&self) -> bool {
        self.is_async
    }

    fn shared_yield_context(&self) -> SharedTypeSchemaView<TypeId> {
        SharedTypeSchemaView::new(self.context_type.unwrap_or(TypeId::UNKNOWN))
    }
}

/// Dart `_argumentOf`.
fn argument_of(ctx: &Ctx<'_>, ty: TypeId, element: EId<InterfaceElement>) -> Option<TypeId> {
    let element_type = ctx.as_instance_of(ty, element)?;
    ctx.type_arguments(element_type).first().copied()
}

/// Dart `_contextTypeForImposed`.
fn context_type_for_imposed(
    ts: &TypeSystem<'_>,
    is_async: bool,
    is_generator: bool,
    imposed_type: Option<TypeId>,
) -> Option<TypeId> {
    let imposed_type = imposed_type?;
    let ctx = ts.ctx;
    // If the function expression is neither `async` nor a generator, then the
    // context type is the imposed return type.
    if !is_async && !is_generator {
        return Some(imposed_type);
    }
    let union_free_imposed_type = ts.union_free_type(imposed_type);
    // If the function expression is declared `async*` and the union-free type
    // derived from the imposed return type is of the form `Stream<S>` for some
    // `S`, then the context type is `S`.
    if is_generator && is_async {
        if let Some(element_type) = argument_of(
            &ctx,
            union_free_imposed_type,
            ctx.tp.stream_element().upcast(),
        ) {
            return Some(element_type);
        }
    }
    // If the function expression is declared `sync*` and the union-free type
    // derived from the imposed return type is of the form `Iterable<S>` for
    // some `S`, then the context type is `S`.
    if is_generator && !is_async {
        if let Some(element_type) = argument_of(
            &ctx,
            union_free_imposed_type,
            ctx.tp.iterable_element().upcast(),
        ) {
            return Some(element_type);
        }
    }
    // Otherwise the context type is `FutureOr<futureValueTypeSchema(S)>`,
    // where `S` is the imposed return type.
    Some(
        ctx.tp
            .future_or_type(&ctx, ts.future_value_type(imposed_type)),
    )
}

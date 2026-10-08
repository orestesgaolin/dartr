//! dartr_mini_ast: test support crate (dev-only).

#![allow(
    clippy::new_ret_no_self,
    clippy::should_implement_trait,
    // Keep the Dart structure (index loops, one branch per Dart case).
    clippy::needless_range_loop,
    clippy::if_same_then_else,
    clippy::too_many_arguments,
    clippy::type_complexity
)]

pub mod flow_analysis_mini_ast;
pub mod harness;
pub mod mini_ir;
pub mod mini_type_constraint_gatherer;
pub mod mini_types;
pub mod node;
pub mod operations;
pub mod test_util;

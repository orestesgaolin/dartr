// Dart source: pkg/analyzer/lib/src/generated/element_resolver.dart

//! `ElementResolver`: the elements of the nodes that are not expressions
//! (constructor names, `super(...)` / `this(...)` constructor invocations,
//! combinators of imports and exports, ...). The Dart methods that do
//! nothing are not ported; their call sites in the resolver are comments.
//!
//! Partly STUB (unit C2): `visitConstructorName`,
//! `visitSuperConstructorInvocation`, `visitRedirectingConstructorInvocation`,
//! `visitImportDirective`, `visitExportDirective` (combinators) and
//! `visitCommentReference` are ported with the units that own those nodes
//! (C8 constructors, C9 comment references).

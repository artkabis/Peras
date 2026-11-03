//! JavaScript code generation for managed Peras modules (`.apeiron`).
//!
//! Uses the `oxc` AST and code generator, inserting `using`, `AbortSignal` and
//! `Symbol.dispose` for every resource. Arrives with the managed-mode milestone.

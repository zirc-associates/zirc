//! Zirc VM: executes Zirc bytecode programs.
//!
//! This is the canonical execution backend (the CLI default). It
//! runs a [`zirc_bytecode::Program`] with an explicit operand stack,
//! call frames with locals, and a globals map. Pure builtins are
//! evaluated by the shared implementation in `zirc-bytecode`, so
//! they behave identically to the reference interpreter. Integer
//! overflow and division by zero are errors, never panics.

pub mod display;
pub mod vm;

pub use vm::Vm;

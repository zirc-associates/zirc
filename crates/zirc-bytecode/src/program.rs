//! Program components for Zirc bytecode.

use crate::instruction::Instruction;

#[derive(Debug, Clone)]
pub struct Function {
    pub name: String,
    pub arity: usize,
    pub local_count: usize,
    pub code: Vec<Instruction>,
    /// Per-instruction debug info, parallel to `code`:
    /// `(line, col)` of the emitting statement, or `None`.
    /// The VM uses it to attach source locations to runtime
    /// errors; the optimizer remaps it with the code.
    pub debug: Vec<Option<(usize, usize)>>,
}

/// Bytecode format version. The VM rejects programs whose version
/// does not match [`crate::BYTECODE_VERSION`], so an incompatible
/// producer fails with a clear error instead of mis-executing.
pub const BYTECODE_VERSION: u32 = 1;

#[derive(Debug, Clone)]
pub struct Program {
    /// Bytecode format version (see [`BYTECODE_VERSION`]).
    pub version: u32,
    pub functions: Vec<Function>,
    pub main: Function,
}

//! Instruction set for Zirc bytecode.
//!
//! The VM in `zirc-vm` is the canonical backend: every instruction
//! here must be implemented by it. `push`/`pop` on a variable are
//! compiled to [`Instruction::PushToVar`] / [`Instruction::PopFromVar`]
//! (see [`VarRef`]) instead of a generic builtin call, because
//! mutating a binding requires knowing which binding it is — a plain
//! stack value cannot be written back.

use crate::builtin::Builtin;
use zirc_syntax::ast::Type;

/// A reference to a variable binding, used by mutating instructions.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum VarRef {
    /// A function-local slot.
    Local(u16),
    /// A global variable, by name.
    Global(String),
}

/// One bytecode instruction for the stack-based VM.
#[derive(Debug, Clone, PartialEq)]
pub enum Instruction {
    // Constants
    PushInt(i64),
    PushFloat(f64),
    PushStr(String),
    PushBool(bool),
    PushUnit,

    // Data structures
    MakeList(usize), // pops N items -> pushes List in original order
    Index,           // pops index, base -> pushes element

    // Mutating list builtins on a variable binding.
    /// Pops a value and appends it to the list held by `var`.
    PushToVar(VarRef),
    /// Pops the last element of the list held by `var`, writes the
    /// shortened list back to `var` and pushes the removed element.
    PopFromVar(VarRef),

    // Locals
    LoadLocal(u16),
    StoreLocal(u16),

    // Globals (by name)
    LoadGlobal(String),
    StoreGlobal(String),

    // Stack
    Pop,

    /// Runtime type check: pops a value, errors if it does not
    /// match `Type`, pushes it back unchanged. Emitted for
    /// annotated `let`/`assign`/params/returns so the VM enforces
    /// the same annotations as the interpreter.
    CheckType(Type),

    // Arithmetic
    Add,
    Sub,
    Mul,
    Div,

    // Comparisons
    Eq,
    Ne,
    Lt,
    Le,
    Gt,
    Ge,

    // Logical
    Not,
    // Unary numeric negation (int checked, float negated)
    Neg,
    // Short-circuit handled with jumps

    // Control flow (absolute instruction index targets)
    Jump(usize),
    JumpIfFalse(usize),
    JumpIfTrue(usize),

    // Calls
    Call(usize, usize), // (function_index, arg_count)
    /// Create a closure for `function_index`, popping
    /// `capture_count` captured values (in free-variable order).
    MakeClosure(usize, usize),
    /// Call the closure value below the top `arg_count` arguments
    /// on the stack: `[... closure, arg1, .., argN]`.
    CallIndirect(usize),
    BuiltinCall(Builtin, usize),
    Return, // expects a value on stack (push Unit beforehand if none)

    // Program control
    Halt,
}

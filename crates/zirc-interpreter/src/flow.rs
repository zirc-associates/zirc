//! Control flow for the interpreter.

use crate::value::Value;

#[derive(Debug)]
pub(crate) enum Flow {
    /// Continue normal execution with the given value
    Continue(Value),
    /// Return from function with the given value
    Return(Value),
    /// Break out of current loop
    Break,
    /// Continue to next loop iteration
    ContinueLoop,
}

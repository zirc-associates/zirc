//! Value types for the Zirc interpreter.
//!
//! The interpreter no longer defines its own `Value` enum: it uses
//! the canonical [`zirc_bytecode::Value`] so both engines share one
//! value representation (and one `Display` implementation).

pub use zirc_bytecode::Value;

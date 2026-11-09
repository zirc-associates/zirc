//! Pretty-printer for VM values.
//!
//! Display is implemented once, as `Display` for the canonical
//! [`zirc_bytecode::Value`]; this module is a thin alias kept for
//! backwards compatibility.

use zirc_bytecode::Value;

/// Format a value for display (same as `value.to_string()`).
pub fn display_value(v: &Value) -> String {
    v.to_string()
}

//! Canonical runtime value for Zirc programs.
//!
//! This is the single `Value` type shared by the tree-walking
//! interpreter and the bytecode VM. `zirc-interpreter` re-exports it
//! instead of defining a duplicate enum, so values produced by one
//! engine can be inspected (and displayed) by the other.

use std::fmt;

/// A runtime value: the only value type in the Zirc language.
///
/// # Examples
///
/// ```rust
/// use zirc_bytecode::Value;
///
/// let v = Value::Int(42);
/// assert_eq!(v.type_name(), "int");
/// assert_eq!(v.to_string(), "42");
/// ```
#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    /// A 64-bit signed integer value.
    Int(i64),
    /// A 64-bit IEEE 754 floating-point value.
    Float(f64),
    /// A closure: index of its (synthetic) function in the
    /// program's function table plus captured free-variable values,
    /// in free-variable order (see `zirc_syntax::analysis::free_vars`).
    /// The function index is engine-local: the VM indexes into
    /// `Program.functions`, the interpreter into its closure table.
    Closure { func: usize, captures: Vec<Value> },
    /// A UTF-8 encoded string value.
    Str(String),
    /// A boolean value (`true` or `false`).
    Bool(bool),
    /// A dynamic list containing other values.
    List(Vec<Value>),
    /// The unit value representing "no value".
    Unit,
}

impl Value {
    /// Short type name used by the `type()` builtin: `int`, `string`,
    /// `bool`, `list` or `unit`.
    pub fn type_name(&self) -> &'static str {
        match self {
            Value::Int(_) => "int",
            Value::Float(_) => "float",
            Value::Closure { .. } => "closure",
            Value::Str(_) => "string",
            Value::Bool(_) => "bool",
            Value::List(_) => "list",
            Value::Unit => "unit",
        }
    }

    /// Whether this value is a truthy boolean `true`.
    pub fn is_true(&self) -> bool {
        matches!(self, Value::Bool(true))
    }

    /// Numeric value as `f64`: ints promote to float, floats stay.
    /// `None` for non-numeric values.
    pub fn as_f64(&self) -> Option<f64> {
        match self {
            Value::Int(n) => Some(*n as f64),
            Value::Float(v) => Some(*v),
            _ => None,
        }
    }
}

/// Format a float, always showing a decimal point (`2.0`, not `2`),
/// so floats are visually distinguishable from ints.
pub fn format_float(v: f64) -> String {
    if v.is_finite() && v.fract() == 0.0 {
        format!("{v:.1}")
    } else {
        format!("{v}")
    }
}

impl fmt::Display for Value {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Value::Int(n) => write!(f, "{n}"),
            Value::Float(v) => write!(f, "{}", format_float(*v)),
            Value::Closure { func, .. } => write!(f, "<closure {func}>"),
            Value::Str(s) => write!(f, "{s}"),
            Value::Bool(b) => write!(f, "{}", if *b { "true" } else { "false" }),
            Value::List(items) => {
                write!(f, "[")?;
                for (i, item) in items.iter().enumerate() {
                    if i > 0 {
                        write!(f, ", ")?;
                    }
                    write!(f, "{item}")?;
                }
                write!(f, "]")
            }
            Value::Unit => write!(f, "<unit>"),
        }
    }
}

//! Builtin function identifiers and the single name registry.
//!
//! Every builtin name in the language is defined exactly once, here.
//! The compiler (`builtin_of`), the interpreter dispatch and the CLI
//! help text all derive from [`Builtin::from_name`] / [`Builtin::ALL`]
//! instead of keeping their own copies of the name list, so the three
//! can no longer drift apart.

/// A builtin function callable from Zirc source code.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Builtin {
    Show,
    ShowF,
    Prompt,
    Rf,
    Wf,
    Len,
    Push,
    Pop,
    Slice,
    // Mathematical functions
    Abs,
    Min,
    Max,
    Pow,
    Sqrt,
    // Conversion functions
    Hex,
    Bin,
    // String functions
    Upper,
    Lower,
    Trim,
    Split,
    Join,
    // Type conversion
    Int,
    Str,
    // Utility functions
    Type,
}

impl Builtin {
    /// All builtins, in declaration order.
    pub const ALL: &[Builtin] = &[
        Builtin::Show,
        Builtin::ShowF,
        Builtin::Prompt,
        Builtin::Rf,
        Builtin::Wf,
        Builtin::Len,
        Builtin::Push,
        Builtin::Pop,
        Builtin::Slice,
        Builtin::Abs,
        Builtin::Min,
        Builtin::Max,
        Builtin::Pow,
        Builtin::Sqrt,
        Builtin::Hex,
        Builtin::Bin,
        Builtin::Upper,
        Builtin::Lower,
        Builtin::Trim,
        Builtin::Split,
        Builtin::Join,
        Builtin::Int,
        Builtin::Str,
        Builtin::Type,
    ];

    /// The source-level name of this builtin (e.g. `"showf"`).
    pub fn name(self) -> &'static str {
        match self {
            Builtin::Show => "show",
            Builtin::ShowF => "showf",
            Builtin::Prompt => "prompt",
            Builtin::Rf => "rf",
            Builtin::Wf => "wf",
            Builtin::Len => "len",
            Builtin::Push => "push",
            Builtin::Pop => "pop",
            Builtin::Slice => "slice",
            Builtin::Abs => "abs",
            Builtin::Min => "min",
            Builtin::Max => "max",
            Builtin::Pow => "pow",
            Builtin::Sqrt => "sqrt",
            Builtin::Hex => "hex",
            Builtin::Bin => "bin",
            Builtin::Upper => "upper",
            Builtin::Lower => "lower",
            Builtin::Trim => "trim",
            Builtin::Split => "split",
            Builtin::Join => "join",
            Builtin::Int => "int",
            Builtin::Str => "str",
            Builtin::Type => "type",
        }
    }

    /// Resolve a source-level name to a builtin, if it is one.
    ///
    /// This is the one authoritative name table for the language.
    pub fn from_name(name: &str) -> Option<Builtin> {
        Self::ALL.iter().copied().find(|b| b.name() == name)
    }

    /// Whether this builtin is pure: it depends only on its argument
    /// values and has no side effects (no I/O, no variable mutation).
    /// Pure builtins are implemented once in [`crate::builtins`] and
    /// shared by both engines; the rest (`show`, `showf`, `prompt`,
    /// `rf`, `wf`, `push`, `pop`) need engine-specific handling.
    pub fn is_pure(self) -> bool {
        !matches!(
            self,
            Builtin::Show
                | Builtin::ShowF
                | Builtin::Prompt
                | Builtin::Rf
                | Builtin::Wf
                | Builtin::Push
                | Builtin::Pop
        )
    }
}

# Changelog

All notable changes to Zirc are documented here. This project
follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/)
and uses semantic versioning; pre-1.0, breaking changes increment
the minor version.

## [0.1.0] - 2026-11-09

First stabilized release of the modernized toolchain. The bytecode
VM is the canonical backend (default); the tree-walking
interpreter is the reference backend. Both implement the same
language and are differentially tested.

### Added

- `float` type with int/float promotion and unary minus `-x`.
- Closures: `fun(x): ... end` as a first-class value, captured by
  value, callable directly or via variables/lists (`call` values).
- Real module system: `import name [as alias]`, qualified calls
  (`math.gcd(...)`), cycle detection, `$ZIRC_PATH`, shipped
  `stdlib/` (`math`, `strings`, `collections`).
- Constant-folding optimizer with jump remapping, bytecode
  versioning, and `zirc --disassemble`.
- `zirc-fmt` preserves `~` comments (idempotent, tested over all
  shipped `.zirc` files); `zirc-lint` static checks; `zirc-lsp`
  language server with diagnostics, hover and completion.
- REPL backend switching (`:backend`), bytecode view (`:dis`),
  syntax highlighting and clean line breaks.
- `docs/LANGUAGE_SPEC.md`.

### Changed

- Single `Value`/builtin registry shared by both engines
  (fixes `hex`/`bin` prefixes, `push`/`pop` in VM, division by
  zero, and type-annotation checking parity).
- Block scoping for `let` and `for` variables; checked integer
  arithmetic (overflow is an error, never a panic).
- CLI rewritten with clap; `--backend vm|interp`, VM by default.

### Fixed

- `PushToVar` left the operand stack unbalanced (`reverse()` bug).
- Formatter dropped string escapes and emitted unparseable
  `module name` declarations.
- REPL `:dis` was unreachable; multiline detection ate `while`/`for`.

[0.1.0]: https://github.com/zirc-industries/zirc/releases/tag/v0.1.0

<div align="center">
  <img width="900" height="300" alt="zirc-brand-1-dark" src="https://github.com/user-attachments/assets/c1bca20e-7162-482d-bc18-55c631c054f2" />
</div>

# Zirc

A lightweight, high-performance programming language with a
bytecode VM as its canonical backend and a tree-walking
interpreter as reference backend. Both backends implement the
same language and are differentially tested against each other.

## Quick start

```sh
cargo build --workspace --release

# Run a program (VM backend, canonical)
./target/release/zirc examples/basic/hello.zirc
# Hello, world!

# Reference interpreter backend
./target/release/zirc --backend interp examples/basic/hello.zirc

# REPL
./target/release/zirc-repl

# Inspect compiled bytecode
./target/release/zirc --disassemble examples/basic/hello.zirc
```

## Language tour

```zirc
~ Comments start with ~
fun fact(n) (int):
  if n == 0:
    return 1
  else:
    return n * fact(n - 1)
  end
end

let pi: float = 3.14
let xs: list = [1, 2, 3]
for i in 0..len(xs):
  showf("xs[%d] = %d", i, xs[i])
end
showf("fact(5) = %d", fact(5))
```

Types: `int`, `float`, `string`, `bool`, `list`, `unit`.
See [docs/LANGUAGE_SPEC.md](docs/LANGUAGE_SPEC.md) for the full
specification, including modules (`import math`,
`math.gcd(12, 18)`), block scoping and the builtin list.

The `stdlib/` directory ships `math`, `strings` and
`collections` modules:

```sh
ZIRC_PATH=stdlib ./target/release/zirc program.zirc
```

## Workspace crates

| Crate | Purpose |
|---|---|
| `zirc-syntax` | Tokens, AST, structured errors |
| `zirc-lexer` | Source → tokens |
| `zirc-parser` | Tokens → AST |
| `zirc-bytecode` | Bytecode IR, `Value`, shared builtins, optimizer, disassembler |
| `zirc-compiler` | AST → bytecode (constant folding) |
| `zirc-vm` | Canonical stack VM |
| `zirc-interpreter` | Reference tree-walking interpreter |
| `zirc-cli` | `zirc` / `zirc-repl` binaries, module loader |
| `zirc-fmt` | Formatter (`format_program` library + CLI) |
| `zirc-lint` | Static checks (undefined vars/functions) |
| `zirc-lsp` | Language server (diagnostics over stdio) |
| `zirc-bench` | Benchmark harness (`--backend vm\|interp`) |

## Development

```sh
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo doc --workspace --no-deps
```

See [CONTRIBUTING.md](CONTRIBUTING.md) for process details.
Licensed under MIT OR Apache-2.0.

# Zirc Language Specification (v0.1.0)

Zirc is a small imperative language with a bytecode VM as its
canonical backend and a tree-walking interpreter as reference
backend. Both backends implement this specification identically;
the test suite contains differential tests that run programs on
both and compare output.

## Lexical structure

- Comments start with `~` and run to the end of the line.
- Integer literals: `42`. Float literals: `1.5` (a `.` only starts
  a fraction when followed by a digit, so `0..5` is a range).
- String literals: `"hello"` with escapes `\n`, `\t`, `\r`, `\\`,
  `\"`.
- Keywords: `module import as fun end if else return let while
  break continue true false for in`.

## Types

`int` (i64), `float` (f64), `string`, `bool`, `list`, `unit`.
Annotations are checked at runtime by both backends:

```zirc
let x: int = 42
let pi: float = 3.14
let xs: list = [1, 2]
```

Mixed arithmetic promotes to `float` (`1 + 0.5 == 1.5`); two `int`
operands stay `int` with checked arithmetic — overflow is an
error, and division by zero is an error in both backends.

## Expressions

- Arithmetic `+ - * /`, unary `-x` and `!x`.
- Comparison `== != < <= > >=` (numeric with int/float promotion).
- Logic `&& ||` with short-circuit evaluation.
- Lists `[1, 2, 3]`, indexing `xs[i]`, member access `a.b`
  (qualified global names).
- Calls `f(x)`, qualified calls `math.gcd(12, 18)`, and calling a
  closure value `(fun(x): x + 1 end)(41)` or `fs[0](x)`.

## Statements

```zirc
let name[: type] = expr
name = expr
return [expr]
if cond: ... else: ... end
while cond: ... end
for i in start..end: ... end
break / continue
```

Scoping: `let` bindings are block-scoped; the `for` loop variable
is scoped to the loop (a previous binding of the same name is
restored afterwards). Assignments to outer bindings persist.

## Functions

```zirc
fun fact(n) (int):
  if n == 0:
    return 1
  else:
    return n * fact(n - 1)
  end
end
```

Parameters and return type may be annotated. The last expression
of a body is an implicit return value when there is no `return`.

## Closures

Anonymous functions are first-class values:

```zirc
let base = 100
let add = fun(x, y): x + y + base end
showf("%d", add(1, 2))          ~ 103, base captured by value

fun make_adder(n):
  return fun(x): x + n end
end
```

Free variables are captured **by value** at creation time, in
both backends identically (shared free-variable analysis). A
closure can be stored in variables/lists and called directly:
`(fun(x): x end)(5)`. Recursion goes through named functions
(a closure cannot reference the variable being defined, since
capture happens before the binding exists).

## Comments

`~` line comments are preserved by `zirc-fmt` (they become AST
comment nodes, in source order). A comment at the end of a code
line is moved to its own line by the formatter.

## Modules

```zirc
module "math"        ~ declaration (optional, informational)
import math          ~ loads math.zirc
import math as m     ~ also registers functions as m.<fn>
```

Resolution: `<module>.zirc` relative to the importing file, then
each directory in `$ZIRC_PATH`. The loader inlines the module's
items and registers its functions under `<module>.<fn>` (plus
`<alias>.<fn>`), callable qualified or unqualified. Import cycles
are errors; diamond imports are included once. The shipped
`stdlib/` provides `math`, `strings` and `collections` —
use it with `ZIRC_PATH=stdlib zirc program.zirc`.

## Builtins

`show showf prompt rf wf len push pop slice abs min max pow sqrt
hex bin upper lower trim split join int str type`

- `showf(fmt, ...)` supports `%d`, `%s`, `%%`.
- `push(list_var, value)` / `pop(list_var)` mutate a variable;
  their first argument must be a variable name.
- `hex(n)` → `"0xff"`, `bin(n)` → `"0b101"` in both backends.
- `pow`/`sqrt` are exact integer operations (no `f64` rounding).

## Backends and bytecode

- `zirc [--backend vm|interp] program.zirc` — `vm` is canonical
  and default; `interp` is the reference interpreter.
- `zirc --disassemble program.zirc` prints the compiled bytecode.
- Bytecode carries a format version (`BYTECODE_VERSION`); the VM
  rejects mismatched versions. Programs are constant-folded at
  compile time (jump targets remapped). The VM enforces call-stack
  and operand-stack limits, returning errors instead of crashing.

//! Shared implementation of the pure builtin functions.
//!
//! Both engines (tree-walking interpreter and bytecode VM) call into
//! this module for every builtin that is a pure function of its
//! arguments, so a builtin can no longer behave differently per
//! backend. Side-effecting builtins (`show`, `showf`, `prompt`,
//! `rf`, `wf`, `push`, `pop`) stay in the engines, but they reuse
//! [`format_showf`] and the helpers here.

use zirc_syntax::error::{error, Error, ErrorKind, Result};

use crate::builtin::Builtin;
use crate::value::Value;

fn arity(b: Builtin, args: &[Value], expected: usize) -> Result<()> {
    if args.len() == expected {
        Ok(())
    } else {
        error(format!(
            "{}() expects exactly {} argument(s), got {}",
            b.name(),
            expected,
            args.len()
        ))
    }
}

fn need_int(b: Builtin, v: &Value) -> Result<i64> {
    match v {
        Value::Int(n) => Ok(*n),
        other => error(format!("{}() expects int, got {:?}", b.name(), other)),
    }
}

fn need_str(b: Builtin, v: &Value) -> Result<String> {
    match v {
        Value::Str(s) => Ok(s.clone()),
        other => error(format!("{}() expects string, got {:?}", b.name(), other)),
    }
}

/// Integer power without going through `f64` (no precision loss).
/// `exp` must be non-negative; overflow is an error, not wraparound.
pub fn int_pow(base: i64, exp: i64) -> Result<i64> {
    if exp < 0 {
        return error("pow() exponent cannot be negative");
    }
    let mut result: i64 = 1;
    for _ in 0..exp {
        result = result
            .checked_mul(base)
            .ok_or_else(|| Error::of_kind(ErrorKind::Runtime, "integer overflow in pow()"))?;
    }
    Ok(result)
}

/// Floor integer square root without going through `f64`.
pub fn int_sqrt(n: i64) -> Result<i64> {
    if n < 0 {
        return error("sqrt() argument cannot be negative");
    }
    let mut r = (n as f64).sqrt() as i64;
    while (r + 1).saturating_mul(r + 1) <= n {
        r += 1;
    }
    while r.saturating_mul(r) > n {
        r -= 1;
    }
    Ok(r)
}

/// Format a `showf` format string with already-evaluated arguments.
///
/// Supports `%d` (int), `%s` (string/bool/list) and `%%`. This is the
/// single implementation used by both engines.
pub fn format_showf(fmt: &str, args: &[Value]) -> Result<String> {
    let mut out = String::new();
    let mut arg_i = 0usize;
    let mut chars = fmt.chars();
    while let Some(c) = chars.next() {
        if c != '%' {
            out.push(c);
            continue;
        }
        match chars.next() {
            Some('d') => {
                let arg = args
                    .get(arg_i)
                    .ok_or_else(|| Error::new("showf missing %d argument"))?;
                out.push_str(&need_int(Builtin::ShowF, arg)?.to_string());
                arg_i += 1;
            }
            Some('s') => {
                let arg = args
                    .get(arg_i)
                    .ok_or_else(|| Error::new("showf missing %s argument"))?;
                match arg {
                    Value::Str(s) => out.push_str(s),
                    Value::Bool(b) => out.push_str(if *b { "true" } else { "false" }),
                    Value::List(_) => out.push_str(&arg.to_string()),
                    other => {
                        return error(format!("%s expects string/bool/list, got {:?}", other));
                    }
                }
                arg_i += 1;
            }
            Some('%') => out.push('%'),
            Some(other) => return error(format!("Unsupported format specifier %{}", other)),
            None => return error("Dangling % at end of format string"),
        }
    }
    Ok(out)
}

/// Evaluate a pure builtin on already-evaluated argument values.
///
/// Returns `None` for non-pure builtins (the caller must handle those),
/// `Some(Ok(value))` on success and `Some(Err(e))` on a builtin error.
pub fn eval_pure(b: Builtin, args: &[Value]) -> Option<Result<Value>> {
    if !b.is_pure() {
        return None;
    }
    Some(eval_pure_inner(b, args))
}

fn eval_pure_inner(b: Builtin, args: &[Value]) -> Result<Value> {
    match b {
        Builtin::Len => {
            arity(b, args, 1)?;
            match &args[0] {
                Value::Str(s) => Ok(Value::Int(s.chars().count() as i64)),
                Value::List(items) => Ok(Value::Int(items.len() as i64)),
                other => error(format!("len() expects string or list, got {:?}", other)),
            }
        }
        Builtin::Slice => {
            arity(b, args, 3)?;
            let start = need_int(b, &args[1])?;
            let end = need_int(b, &args[2])?;
            if start < 0 {
                return error("slice() start index cannot be negative");
            }
            if end < start {
                return error("slice() end index must be >= start index");
            }
            match &args[0] {
                Value::Str(s) => {
                    let chars: Vec<char> = s.chars().collect();
                    let start_idx = start as usize;
                    if start_idx >= chars.len() {
                        return Ok(Value::Str(String::new()));
                    }
                    let end_idx = (end as usize).min(chars.len());
                    Ok(Value::Str(chars[start_idx..end_idx].iter().collect()))
                }
                Value::List(items) => {
                    let start_idx = start as usize;
                    if start_idx >= items.len() {
                        return Ok(Value::List(Vec::new()));
                    }
                    let end_idx = (end as usize).min(items.len());
                    Ok(Value::List(items[start_idx..end_idx].to_vec()))
                }
                other => error(format!("slice() expects string or list, got {:?}", other)),
            }
        }
        Builtin::Abs => {
            arity(b, args, 1)?;
            let n = need_int(b, &args[0])?;
            n.checked_abs()
                .map(Value::Int)
                .ok_or_else(|| Error::of_kind(ErrorKind::Runtime, "integer overflow in abs()"))
        }
        Builtin::Min => {
            arity(b, args, 2)?;
            Ok(Value::Int(
                need_int(b, &args[0])?.min(need_int(b, &args[1])?),
            ))
        }
        Builtin::Max => {
            arity(b, args, 2)?;
            Ok(Value::Int(
                need_int(b, &args[0])?.max(need_int(b, &args[1])?),
            ))
        }
        Builtin::Pow => {
            arity(b, args, 2)?;
            int_pow(need_int(b, &args[0])?, need_int(b, &args[1])?).map(Value::Int)
        }
        Builtin::Sqrt => {
            arity(b, args, 1)?;
            int_sqrt(need_int(b, &args[0])?).map(Value::Int)
        }
        Builtin::Hex => {
            arity(b, args, 1)?;
            Ok(Value::Str(format!("0x{:x}", need_int(b, &args[0])?)))
        }
        Builtin::Bin => {
            arity(b, args, 1)?;
            Ok(Value::Str(format!("0b{:b}", need_int(b, &args[0])?)))
        }
        Builtin::Upper => {
            arity(b, args, 1)?;
            Ok(Value::Str(need_str(b, &args[0])?.to_uppercase()))
        }
        Builtin::Lower => {
            arity(b, args, 1)?;
            Ok(Value::Str(need_str(b, &args[0])?.to_lowercase()))
        }
        Builtin::Trim => {
            arity(b, args, 1)?;
            Ok(Value::Str(need_str(b, &args[0])?.trim().to_string()))
        }
        Builtin::Split => {
            arity(b, args, 2)?;
            let text = need_str(b, &args[0])?;
            let delim = need_str(b, &args[1])?;
            Ok(Value::List(
                text.split(&delim)
                    .map(|part| Value::Str(part.to_string()))
                    .collect(),
            ))
        }
        Builtin::Join => {
            arity(b, args, 2)?;
            let sep = need_str(b, &args[1])?;
            match &args[0] {
                Value::List(items) => {
                    let mut parts = Vec::with_capacity(items.len());
                    for item in items {
                        match item {
                            Value::Str(s) => parts.push(s.clone()),
                            other => {
                                return error(format!(
                                    "join() list must contain only strings, got {:?}",
                                    other
                                ));
                            }
                        }
                    }
                    Ok(Value::Str(parts.join(&sep)))
                }
                _ => error("join() expects list and string"),
            }
        }
        Builtin::Int => {
            arity(b, args, 1)?;
            match &args[0] {
                Value::Int(n) => Ok(Value::Int(*n)),
                Value::Float(v) => Ok(Value::Int(*v as i64)),
                Value::Str(s) => s
                    .parse::<i64>()
                    .map(Value::Int)
                    .map_err(|_| Error::new(format!("Cannot convert '{}' to int", s))),
                Value::Bool(true) => Ok(Value::Int(1)),
                Value::Bool(false) => Ok(Value::Int(0)),
                other => error(format!("Cannot convert {:?} to int", other)),
            }
        }
        Builtin::Str => {
            arity(b, args, 1)?;
            Ok(Value::Str(args[0].to_string()))
        }
        Builtin::Type => {
            arity(b, args, 1)?;
            Ok(Value::Str(args[0].type_name().to_string()))
        }
        // Non-pure builtins never reach here (guarded by `eval_pure`).
        Builtin::Show
        | Builtin::ShowF
        | Builtin::Prompt
        | Builtin::Rf
        | Builtin::Wf
        | Builtin::Push
        | Builtin::Pop => error(format!("{}() is not a pure builtin", b.name())),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pure_builtins_are_shared() {
        assert_eq!(
            eval_pure(Builtin::Hex, &[Value::Int(255)])
                .unwrap()
                .unwrap(),
            Value::Str("0xff".to_string())
        );
        assert_eq!(
            eval_pure(Builtin::Bin, &[Value::Int(5)]).unwrap().unwrap(),
            Value::Str("0b101".to_string())
        );
        assert!(eval_pure(Builtin::Show, &[Value::Int(1)]).is_none());
    }

    #[test]
    fn int_pow_and_sqrt_are_exact() {
        assert_eq!(int_pow(2, 40).unwrap(), 1_099_511_627_776);
        assert_eq!(int_sqrt(17).unwrap(), 4);
        assert!(int_pow(2, -1).is_err());
        assert!(int_sqrt(-1).is_err());
    }

    #[test]
    fn showf_formatting() {
        let out = format_showf("n=%d s=%s %%", &[Value::Int(7), Value::Str("x".into())]).unwrap();
        assert_eq!(out, "n=7 s=x %");
    }
}

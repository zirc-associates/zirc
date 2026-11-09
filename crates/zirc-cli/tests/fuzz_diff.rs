//! Differential fuzzer: seeded random programs must produce the
//! same result (or both fail) in the VM and the interpreter.
//!
//! The generator only emits side-effect-free expressions over
//! ints, floats, strings, bools and lists, so both engines can be
//! compared in-process by their final value.

use std::path::PathBuf;

use zirc_compiler::Compiler;
use zirc_interpreter::{Env, Interpreter};
use zirc_loader::lex_and_parse_with_modules;
use zirc_syntax::error::Result;
use zirc_vm::Vm;

/// Minimal xorshift64* PRNG (deterministic across platforms).
struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x >> 12;
        x ^= x << 25;
        x ^= x >> 27;
        self.0 = x;
        x.wrapping_mul(0x2545_F491_4F6C_DD1D)
    }

    fn below(&mut self, n: u64) -> u64 {
        if n == 0 {
            0
        } else {
            self.next() % n
        }
    }
}

fn gen_expr(rng: &mut Rng, vars: &[&str], depth: u32) -> String {
    if depth == 0 || rng.below(3) == 0 {
        return match rng.below(6) {
            0 => format!("{}", rng.below(20) as i64 - 10),
            1 => format!("{}.{}", rng.below(10), rng.below(10)),
            2 => format!("\"s{}\"", rng.below(10)),
            3 => {
                if rng.below(2) == 0 {
                    "true".into()
                } else {
                    "false".into()
                }
            }
            4 if !vars.is_empty() => vars[rng.below(vars.len() as u64) as usize].to_string(),
            _ => format!("{}", rng.below(20) as i64 - 10),
        };
    }
    match rng.below(5) {
        0 => format!(
            "{} {} {}",
            gen_expr(rng, vars, depth - 1),
            ["+", "-", "*", "/"][rng.below(4) as usize],
            gen_expr(rng, vars, depth - 1)
        ),
        1 => format!(
            "{} {} {}",
            gen_expr(rng, vars, depth - 1),
            ["==", "!=", "<", "<=", ">", ">="][rng.below(6) as usize],
            gen_expr(rng, vars, depth - 1)
        ),
        2 => format!("-{}", gen_expr(rng, vars, depth - 1)),
        3 => format!(
            "[{}, {}]",
            gen_expr(rng, vars, depth - 1),
            gen_expr(rng, vars, depth - 1)
        ),
        _ => format!("len({})", gen_expr(rng, vars, depth - 1)),
    }
}

fn gen_program(rng: &mut Rng) -> String {
    let mut vars: Vec<String> = Vec::new();
    let mut src = String::new();
    for i in 0..rng.below(4) {
        let name = match i {
            0 => "a",
            1 => "b",
            2 => "c",
            _ => "d",
        };
        src.push_str(&format!(
            "let {name} = {}\n",
            gen_expr(rng, &vars.iter().map(String::as_str).collect::<Vec<_>>(), 2)
        ));
        vars.push(name.to_string());
    }
    src.push_str(&gen_expr(
        rng,
        &vars.iter().map(String::as_str).collect::<Vec<_>>(),
        3,
    ));
    src
}

fn run_interp(src: &str) -> Result<Option<zirc_bytecode::Value>> {
    let dir = PathBuf::from(".");
    let program = lex_and_parse_with_modules(src, &dir)?;
    let mut interp = Interpreter::new();
    let mut env = Env::new_root();
    interp.run_with_env(program, &mut env)
}

fn run_vm(src: &str) -> Result<Option<zirc_bytecode::Value>> {
    let dir = PathBuf::from(".");
    let program = lex_and_parse_with_modules(src, &dir)?;
    let mut compiler = Compiler::new();
    let bprog = compiler.compile(program)?;
    let mut vm = Vm::new();
    vm.run(&bprog)
}

#[test]
fn differential_fuzz_seeded_programs_agree() {
    let mut rng = Rng(0x1234_5678_9ABC_DEF0);
    for case in 0..300 {
        let src = gen_program(&mut rng);
        let interp = run_interp(&src);
        let vm = run_vm(&src);
        // Normalize the "no meaningful value" artifact: the
        // interpreter reports `Some(Unit)` while the VM reports
        // `None` for let-only programs. They are equivalent.
        let norm = |v: Option<zirc_bytecode::Value>| match v {
            Some(zirc_bytecode::Value::Unit) | None => None,
            other => other,
        };
        match (interp, vm) {
            (Ok(a), Ok(b)) => assert_eq!(
                norm(a),
                norm(b),
                "case {case}: engines disagree on value\nprogram:\n{src}"
            ),
            (Err(e1), Err(e2)) => {
                // Both failed: acceptable (messages may differ by engine).
                let _ = (e1.msg, e2.msg);
            }
            (Ok(a), Err(e)) => panic!(
                "case {case}: interpreter succeeded ({a:?}) but VM failed ({})\nprogram:\n{src}",
                e.msg
            ),
            (Err(e), Ok(b)) => panic!(
                "case {case}: VM succeeded ({b:?}) but interpreter failed ({})\nprogram:\n{src}",
                e.msg
            ),
        }
    }
}

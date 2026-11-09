//! Bytecode optimizations: constant folding with jump remapping.
//!
//! [`optimize_program`] folds sequences like `PushInt 1, PushInt 2,
//! Add` into a single `PushInt 3` and rewrites absolute jump targets
//! so control flow still lands on the same logical instructions.
//! The compiler applies it to every function (see `zirc-compiler`).

use crate::instruction::Instruction;
use crate::program::{Function, Program};

/// Optimize a whole program (each function independently).
pub fn optimize_program(program: Program) -> Program {
    Program {
        version: program.version,
        functions: program
            .functions
            .into_iter()
            .map(optimize_function)
            .collect(),
        main: optimize_function(program.main),
    }
}

/// Optimize one function's instruction stream.
pub fn optimize_function(func: Function) -> Function {
    let folded = fold_constants(&func.code);
    let code = remap_jumps(&func.code, &folded);
    // Debug info is remapped alongside the code: a folded
    // instruction inherits the debug entry of its first input,
    // and kept instructions keep theirs.
    let debug = remap_debug(&func.code, &func.debug, folded.len());
    Function {
        code,
        debug,
        ..func
    }
}

/// Rebuild the debug vec for a folded instruction stream.
/// Mirrors the mapping computed by `remap_jumps`.
fn remap_debug(
    original: &[Instruction],
    debug: &[Option<(usize, usize)>],
    folded_len: usize,
) -> Vec<Option<(usize, usize)>> {
    if debug.is_empty() {
        return Vec::new();
    }
    let mut out: Vec<Option<(usize, usize)>> = Vec::with_capacity(folded_len);
    let mut i = 0;
    while i < original.len() {
        out.push(debug.get(i).copied().flatten());
        i += folded_len_at(original, i);
    }
    out
}

/// Fold adjacent constant operations. Returns the new instruction
/// vector; jump targets are left as-is here (see `remap_jumps`).
fn fold_constants(code: &[Instruction]) -> Vec<Instruction> {
    let mut out: Vec<Instruction> = Vec::with_capacity(code.len());
    let mut i = 0;
    while i < code.len() {
        // Binary int op: PushInt a, PushInt b, Op
        if i + 2 < code.len() {
            if let (Instruction::PushInt(a), Instruction::PushInt(b), op) =
                (&code[i], &code[i + 1], &code[i + 2])
            {
                if let Some(result) = fold_int_binop(*a, *b, op) {
                    out.push(result);
                    i += 3;
                    continue;
                }
            }
            // Binary float op: PushFloat a, PushFloat b, Op
            if let (Instruction::PushFloat(a), Instruction::PushFloat(b), op) =
                (&code[i], &code[i + 1], &code[i + 2])
            {
                if let Some(result) = fold_float_binop(*a, *b, op) {
                    out.push(result);
                    i += 3;
                    continue;
                }
            }
            // String concat: PushStr a, PushStr b, Add
            if let (Instruction::PushStr(a), Instruction::PushStr(b), Instruction::Add) =
                (&code[i], &code[i + 1], &code[i + 2])
            {
                out.push(Instruction::PushStr(format!("{}{}", a, b)));
                i += 3;
                continue;
            }
        }
        // Unary: PushBool b, Not / PushInt n, Neg / PushFloat v, Neg
        if i + 1 < code.len() {
            match (&code[i], &code[i + 1]) {
                (Instruction::PushBool(b), Instruction::Not) => {
                    out.push(Instruction::PushBool(!b));
                    i += 2;
                    continue;
                }
                (Instruction::PushInt(n), Instruction::Neg) => {
                    if let Some(v) = n.checked_neg() {
                        out.push(Instruction::PushInt(v));
                        i += 2;
                        continue;
                    }
                }
                (Instruction::PushFloat(v), Instruction::Neg) => {
                    out.push(Instruction::PushFloat(-v));
                    i += 2;
                    continue;
                }
                _ => {}
            }
        }
        out.push(code[i].clone());
        i += 1;
    }
    out
}

fn fold_int_binop(a: i64, b: i64, op: &Instruction) -> Option<Instruction> {
    match op {
        Instruction::Add => a.checked_add(b).map(Instruction::PushInt),
        Instruction::Sub => a.checked_sub(b).map(Instruction::PushInt),
        Instruction::Mul => a.checked_mul(b).map(Instruction::PushInt),
        // Division by zero must stay a runtime error: never fold it.
        Instruction::Div => {
            if b == 0 {
                None
            } else {
                a.checked_div(b).map(Instruction::PushInt)
            }
        }
        Instruction::Eq => Some(Instruction::PushBool(a == b)),
        Instruction::Ne => Some(Instruction::PushBool(a != b)),
        Instruction::Lt => Some(Instruction::PushBool(a < b)),
        Instruction::Le => Some(Instruction::PushBool(a <= b)),
        Instruction::Gt => Some(Instruction::PushBool(a > b)),
        Instruction::Ge => Some(Instruction::PushBool(a >= b)),
        _ => None,
    }
}

fn fold_float_binop(a: f64, b: f64, op: &Instruction) -> Option<Instruction> {
    match op {
        Instruction::Add => Some(Instruction::PushFloat(a + b)),
        Instruction::Sub => Some(Instruction::PushFloat(a - b)),
        Instruction::Mul => Some(Instruction::PushFloat(a * b)),
        Instruction::Div => {
            if b == 0.0 {
                None
            } else {
                Some(Instruction::PushFloat(a / b))
            }
        }
        Instruction::Eq => Some(Instruction::PushBool(a == b)),
        Instruction::Ne => Some(Instruction::PushBool(a != b)),
        Instruction::Lt => Some(Instruction::PushBool(a < b)),
        Instruction::Le => Some(Instruction::PushBool(a <= b)),
        Instruction::Gt => Some(Instruction::PushBool(a > b)),
        Instruction::Ge => Some(Instruction::PushBool(a >= b)),
        _ => None,
    }
}

/// Rewrite absolute jump targets in `folded` so they point to the
/// same logical instructions as in `original`.
///
/// A jump to an instruction that was folded away lands on the
/// instruction that replaced it (the first folded output produced
/// from that point onward). A jump past the end lands at the end.
fn remap_jumps(original: &[Instruction], folded: &[Instruction]) -> Vec<Instruction> {
    // Map each original index to the folded index where its
    // replacement starts. Recompute folding with provenance.
    let mut mapping: Vec<usize> = vec![folded.len(); original.len() + 1];
    let mut folded_index = 0usize;
    let mut i = 0usize;
    while i < original.len() {
        mapping[i] = folded_index;
        let consumed = folded_len_at(original, i);
        // Instructions folded away map to the replacement as well.
        for j in 1..consumed {
            mapping[i + j] = folded_index;
        }
        i += consumed;
        if consumed > 0 {
            folded_index += 1;
        }
    }
    mapping[original.len()] = folded.len();

    folded
        .iter()
        .map(|instr| match instr {
            Instruction::Jump(t) => Instruction::Jump(remap_target(*t, &mapping)),
            Instruction::JumpIfFalse(t) => Instruction::JumpIfFalse(remap_target(*t, &mapping)),
            Instruction::JumpIfTrue(t) => Instruction::JumpIfTrue(remap_target(*t, &mapping)),
            other => other.clone(),
        })
        .collect()
}

fn remap_target(target: usize, mapping: &[usize]) -> usize {
    *mapping
        .get(target)
        .unwrap_or_else(|| mapping.last().unwrap_or(&0))
}

/// How many original instructions starting at `i` fold into one
/// output instruction (0 means "no fold here", copy as-is).
fn folded_len_at(code: &[Instruction], i: usize) -> usize {
    if i + 2 < code.len() {
        match (&code[i], &code[i + 1], &code[i + 2]) {
            (Instruction::PushInt(a), Instruction::PushInt(b), op) => {
                if fold_int_binop(*a, *b, op).is_some() {
                    return 3;
                }
            }
            (Instruction::PushFloat(a), Instruction::PushFloat(b), op) => {
                if fold_float_binop(*a, *b, op).is_some() {
                    return 3;
                }
            }
            (Instruction::PushStr(_), Instruction::PushStr(_), Instruction::Add) => return 3,
            _ => {}
        }
    }
    if i + 1 < code.len() {
        match (&code[i], &code[i + 1]) {
            (Instruction::PushBool(_), Instruction::Not) => return 2,
            (Instruction::PushInt(n), Instruction::Neg) => {
                if n.checked_neg().is_some() {
                    return 2;
                }
            }
            (Instruction::PushFloat(_), Instruction::Neg) => return 2,
            _ => {}
        }
    }
    1
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::program::BYTECODE_VERSION;

    fn func(code: Vec<Instruction>) -> Function {
        Function {
            name: "main".to_string(),
            arity: 0,
            local_count: 0,
            debug: Vec::new(),
            code,
        }
    }

    #[test]
    fn folds_int_arithmetic() {
        let f = optimize_function(func(vec![
            Instruction::PushInt(1),
            Instruction::PushInt(2),
            Instruction::Add,
        ]));
        assert_eq!(f.code, vec![Instruction::PushInt(3)]);
    }

    #[test]
    fn keeps_division_by_zero_as_runtime_error() {
        let f = optimize_function(func(vec![
            Instruction::PushInt(1),
            Instruction::PushInt(0),
            Instruction::Div,
        ]));
        assert_eq!(f.code.len(), 3);
    }

    #[test]
    fn remaps_jumps_after_folding() {
        // JumpIfFalse targets the Halt at index 4; after folding the
        // first three instructions into one, Halt moves to index 2.
        let f = optimize_function(func(vec![
            Instruction::PushInt(1),
            Instruction::PushInt(2),
            Instruction::Add,
            Instruction::JumpIfFalse(4),
            Instruction::Halt,
        ]));
        assert_eq!(
            f.code,
            vec![
                Instruction::PushInt(3),
                Instruction::JumpIfFalse(2),
                Instruction::Halt
            ]
        );
    }

    #[test]
    fn optimize_program_keeps_version() {
        let program = Program {
            version: BYTECODE_VERSION,
            functions: vec![],
            main: func(vec![Instruction::Halt]),
        };
        assert_eq!(optimize_program(program).version, BYTECODE_VERSION);
    }
}

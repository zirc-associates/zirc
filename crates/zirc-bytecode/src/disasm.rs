//! Bytecode disassembler: renders a program as readable text.
//!
//! Used by `zirc --disassemble` for debugging and by tests to
//! assert on compiler output (e.g. constant folding).

use crate::instruction::{Instruction, VarRef};
use crate::program::Program;

/// Render one instruction in disassembly form.
pub fn disassemble_instruction(instr: &Instruction) -> String {
    match instr {
        Instruction::PushInt(n) => format!("PushInt {n}"),
        Instruction::PushFloat(v) => format!("PushFloat {}", crate::value::format_float(*v)),
        Instruction::PushStr(s) => format!("PushStr {s:?}"),
        Instruction::PushBool(b) => format!("PushBool {b}"),
        Instruction::PushUnit => "PushUnit".to_string(),
        Instruction::MakeList(n) => format!("MakeList {n}"),
        Instruction::Index => "Index".to_string(),
        Instruction::PushToVar(var) => format!("PushToVar {}", dis_var(var)),
        Instruction::PopFromVar(var) => format!("PopFromVar {}", dis_var(var)),
        Instruction::LoadLocal(i) => format!("LoadLocal {i}"),
        Instruction::StoreLocal(i) => format!("StoreLocal {i}"),
        Instruction::LoadGlobal(name) => format!("LoadGlobal {name}"),
        Instruction::StoreGlobal(name) => format!("StoreGlobal {name}"),
        Instruction::Pop => "Pop".to_string(),
        Instruction::CheckType(ty) => format!("CheckType {ty:?}"),
        Instruction::Add => "Add".to_string(),
        Instruction::Sub => "Sub".to_string(),
        Instruction::Mul => "Mul".to_string(),
        Instruction::Div => "Div".to_string(),
        Instruction::Eq => "Eq".to_string(),
        Instruction::Ne => "Ne".to_string(),
        Instruction::Lt => "Lt".to_string(),
        Instruction::Le => "Le".to_string(),
        Instruction::Gt => "Gt".to_string(),
        Instruction::Ge => "Ge".to_string(),
        Instruction::Not => "Not".to_string(),
        Instruction::Neg => "Neg".to_string(),
        Instruction::Jump(t) => format!("Jump {t}"),
        Instruction::JumpIfFalse(t) => format!("JumpIfFalse {t}"),
        Instruction::JumpIfTrue(t) => format!("JumpIfTrue {t}"),
        Instruction::Call(fi, argc) => format!("Call {fi} ({argc} args)"),
        Instruction::MakeClosure(fi, n) => format!("MakeClosure {fi} ({n} captures)"),
        Instruction::CallIndirect(argc) => format!("CallIndirect ({argc} args)"),
        Instruction::BuiltinCall(b, argc) => format!("BuiltinCall {} ({argc} args)", b.name()),
        Instruction::Return => "Return".to_string(),
        Instruction::Halt => "Halt".to_string(),
    }
}

fn dis_var(var: &VarRef) -> String {
    match var {
        VarRef::Local(i) => format!("local {i}"),
        VarRef::Global(name) => format!("global {name}"),
    }
}

/// Render a whole program: header, then each function with
/// `offset: instruction` lines.
pub fn disassemble(program: &Program) -> String {
    let mut out = format!("zirc bytecode v{}\n", program.version);
    for func in program
        .functions
        .iter()
        .chain(std::iter::once(&program.main))
    {
        out.push_str(&format!(
            "fn {} (arity {}, {} locals):\n",
            func.name, func.arity, func.local_count
        ));
        for (i, instr) in func.code.iter().enumerate() {
            out.push_str(&format!("  {i:4}: {}\n", disassemble_instruction(instr)));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::instruction::Instruction;
    use crate::program::Function;

    #[test]
    fn disassemble_renders_instructions() {
        let program = Program {
            version: crate::BYTECODE_VERSION,
            functions: vec![],
            main: Function {
                name: "main".to_string(),
                arity: 0,
                local_count: 0,
                code: vec![
                    Instruction::PushInt(1),
                    Instruction::PushInt(2),
                    Instruction::Add,
                ],
                debug: Vec::new(),
            },
        };
        let text = disassemble(&program);
        assert!(text.contains("PushInt 1"));
        assert!(text.contains("Add"));
    }
}

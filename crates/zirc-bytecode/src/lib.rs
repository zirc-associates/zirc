//! Bytecode IR for the Zirc programming language.
//!
//! This crate defines a simple stack-based bytecode, a program container,
//! and value representation used by the Zirc VM backend.

pub mod builtin;
pub mod builtins;
pub mod disasm;
pub mod instruction;
pub mod optimize;
pub mod program;
pub mod value;

pub use builtin::Builtin;
pub use disasm::disassemble;
pub use instruction::{Instruction, VarRef};
pub use program::{Function, Program, BYTECODE_VERSION};
pub use value::Value;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_value_equality() {
        assert_eq!(Value::Int(42), Value::Int(42));
        assert_eq!(
            Value::Str("hello".to_string()),
            Value::Str("hello".to_string())
        );
        assert_eq!(Value::Bool(true), Value::Bool(true));
        assert_eq!(Value::Unit, Value::Unit);

        let list1 = Value::List(vec![Value::Int(1), Value::Int(2)]);
        let list2 = Value::List(vec![Value::Int(1), Value::Int(2)]);
        assert_eq!(list1, list2);
    }

    #[test]
    fn test_value_inequality() {
        assert_ne!(Value::Int(42), Value::Int(43));
        assert_ne!(
            Value::Str("hello".to_string()),
            Value::Str("world".to_string())
        );
        assert_ne!(Value::Bool(true), Value::Bool(false));
        assert_ne!(Value::Int(42), Value::Str("42".to_string()));

        let list1 = Value::List(vec![Value::Int(1), Value::Int(2)]);
        let list2 = Value::List(vec![Value::Int(2), Value::Int(1)]);
        assert_ne!(list1, list2);
    }

    #[test]
    fn test_value_clone() {
        let original = Value::List(vec![
            Value::Int(1),
            Value::Str("test".to_string()),
            Value::Bool(true),
        ]);
        let cloned = original.clone();
        assert_eq!(original, cloned);
    }

    #[test]
    fn test_builtin_variants() {
        // Test that all builtin variants exist and can be used
        let builtins = vec![
            Builtin::Show,
            Builtin::ShowF,
            Builtin::Prompt,
            Builtin::Rf,
            Builtin::Wf,
            Builtin::Len,
            Builtin::Push,
            Builtin::Pop,
            Builtin::Slice,
        ];

        // Just test that they can be cloned and compared
        for builtin in builtins {
            let cloned = builtin;
            assert_eq!(builtin, cloned);
        }
    }

    #[test]
    fn test_instruction_variants() {
        // Test some instruction variants
        let instructions = vec![
            Instruction::PushInt(42),
            Instruction::PushStr("test".to_string()),
            Instruction::PushBool(true),
            Instruction::PushUnit,
            Instruction::Add,
            Instruction::Sub,
            Instruction::Mul,
            Instruction::Div,
            Instruction::Eq,
            Instruction::Jump(10),
            Instruction::Call(0, 2),
        ];

        for instr in instructions {
            let cloned = instr.clone();
            assert_eq!(instr, cloned);
        }
    }

    #[test]
    fn test_function_creation() {
        let function = Function {
            name: "test_func".to_string(),
            arity: 2,
            local_count: 5,
            code: vec![
                Instruction::LoadLocal(0),
                Instruction::LoadLocal(1),
                Instruction::Add,
                Instruction::Return,
            ],
            debug: Vec::new(),
        };

        assert_eq!(function.name, "test_func");
        assert_eq!(function.arity, 2);
        assert_eq!(function.local_count, 5);
        assert_eq!(function.code.len(), 4);
    }

    #[test]
    fn test_program_creation() {
        let main_func = Function {
            name: "main".to_string(),
            arity: 0,
            local_count: 1,
            code: vec![Instruction::PushInt(42), Instruction::Return],
            debug: Vec::new(),
        };

        let helper_func = Function {
            name: "helper".to_string(),
            arity: 1,
            local_count: 2,
            code: vec![Instruction::LoadLocal(0), Instruction::Return],
            debug: Vec::new(),
        };

        let program = Program {
            version: BYTECODE_VERSION,
            functions: vec![helper_func],
            main: main_func,
        };

        assert_eq!(program.functions.len(), 1);
        assert_eq!(program.main.name, "main");
        assert_eq!(program.functions[0].name, "helper");
    }

    #[test]
    fn test_nested_values() {
        let nested = Value::List(vec![
            Value::Int(1),
            Value::List(vec![Value::Str("nested".to_string()), Value::Bool(true)]),
            Value::Unit,
        ]);

        let cloned = nested.clone();
        assert_eq!(nested, cloned);
    }
}

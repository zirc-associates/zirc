//! Zirc VM core.

use std::collections::HashMap;
use std::fs;
use std::io::{self, Write};

use crate::display::display_value;
use zirc_bytecode::builtins::{eval_pure, format_showf};
use zirc_bytecode::{Builtin, Instruction, Program, Value, VarRef};
use zirc_syntax::error::{error, Result};

#[derive(Clone)]
struct Frame {
    func_ref: CodeRef,
    ip: usize,
    locals: Vec<Value>,
}

/// Numeric equality with int/float promotion (`1 == 1.0` is true).
fn vm_values_equal(x: &Value, y: &Value) -> bool {
    match (x.as_f64(), y.as_f64()) {
        (Some(a), Some(b)) if !matches!(x, Value::Int(_)) || !matches!(y, Value::Int(_)) => a == b,
        _ => x == y,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use zirc_bytecode::{Function, Instruction};

    fn make_simple_program(main_code: Vec<Instruction>) -> Program {
        Program {
            version: zirc_bytecode::BYTECODE_VERSION,
            functions: Vec::new(),
            main: Function {
                name: "main".to_string(),
                arity: 0,
                local_count: 1,
                code: main_code,
                debug: Vec::new(),
            },
        }
    }

    #[test]
    fn test_vm_basic_operations() {
        let mut vm = Vm::new();

        // Test basic arithmetic: 5 + 3
        let program = make_simple_program(vec![
            Instruction::PushInt(5),
            Instruction::PushInt(3),
            Instruction::Add,
        ]);

        let result = vm.run(&program).unwrap();
        assert_eq!(result, None); // No explicit return, so None

        // Check that the stack has the result
        assert_eq!(vm.stack.len(), 1);
        assert_eq!(vm.stack[0], Value::Int(8));
    }

    #[test]
    fn test_vm_arithmetic_operations() {
        let test_cases = vec![
            (
                vec![
                    Instruction::PushInt(10),
                    Instruction::PushInt(3),
                    Instruction::Add,
                ],
                Value::Int(13),
            ),
            (
                vec![
                    Instruction::PushInt(10),
                    Instruction::PushInt(3),
                    Instruction::Sub,
                ],
                Value::Int(7),
            ),
            (
                vec![
                    Instruction::PushInt(10),
                    Instruction::PushInt(3),
                    Instruction::Mul,
                ],
                Value::Int(30),
            ),
            (
                vec![
                    Instruction::PushInt(10),
                    Instruction::PushInt(3),
                    Instruction::Div,
                ],
                Value::Int(3),
            ),
        ];

        for (code, expected) in test_cases {
            let mut vm = Vm::new();
            let program = make_simple_program(code);

            vm.run(&program).unwrap();
            assert_eq!(vm.stack[0], expected);
        }
    }

    #[test]
    fn test_vm_comparison_operations() {
        let test_cases = vec![
            (
                vec![
                    Instruction::PushInt(5),
                    Instruction::PushInt(3),
                    Instruction::Lt,
                ],
                Value::Bool(false),
            ),
            (
                vec![
                    Instruction::PushInt(3),
                    Instruction::PushInt(5),
                    Instruction::Lt,
                ],
                Value::Bool(true),
            ),
            (
                vec![
                    Instruction::PushInt(5),
                    Instruction::PushInt(5),
                    Instruction::Eq,
                ],
                Value::Bool(true),
            ),
            (
                vec![
                    Instruction::PushInt(5),
                    Instruction::PushInt(3),
                    Instruction::Eq,
                ],
                Value::Bool(false),
            ),
            (
                vec![
                    Instruction::PushInt(5),
                    Instruction::PushInt(3),
                    Instruction::Ne,
                ],
                Value::Bool(true),
            ),
        ];

        for (code, expected) in test_cases {
            let mut vm = Vm::new();
            let program = make_simple_program(code);

            vm.run(&program).unwrap();
            assert_eq!(vm.stack[0], expected);
        }
    }

    #[test]
    fn test_vm_string_operations() {
        let mut vm = Vm::new();

        // Test string concatenation
        let program = make_simple_program(vec![
            Instruction::PushStr("Hello, ".to_string()),
            Instruction::PushStr("World!".to_string()),
            Instruction::Add,
        ]);

        vm.run(&program).unwrap();
        assert_eq!(vm.stack[0], Value::Str("Hello, World!".to_string()));
    }

    #[test]
    fn test_vm_boolean_operations() {
        let mut vm = Vm::new();

        // Test boolean negation
        let program = make_simple_program(vec![Instruction::PushBool(true), Instruction::Not]);

        vm.run(&program).unwrap();
        assert_eq!(vm.stack[0], Value::Bool(false));
    }

    #[test]
    fn test_vm_local_variables() {
        let mut vm = Vm::new();

        // Test local variable storage and retrieval
        let program = make_simple_program(vec![
            Instruction::PushInt(42),
            Instruction::StoreLocal(0),
            Instruction::LoadLocal(0),
        ]);

        vm.run(&program).unwrap();
        assert_eq!(vm.stack[0], Value::Int(42));
    }

    #[test]
    fn test_vm_global_variables() {
        let mut vm = Vm::new();

        // Test global variable storage and retrieval
        let program = make_simple_program(vec![
            Instruction::PushStr("test".to_string()),
            Instruction::StoreGlobal("x".to_string()),
            Instruction::LoadGlobal("x".to_string()),
        ]);

        vm.run(&program).unwrap();
        assert_eq!(vm.stack[0], Value::Str("test".to_string()));

        // Check globals snapshot
        let globals = vm.globals_snapshot();
        assert_eq!(globals.len(), 1);
        assert_eq!(
            globals[0],
            ("x".to_string(), Value::Str("test".to_string()))
        );
    }

    #[test]
    fn test_vm_list_operations() {
        let mut vm = Vm::new();

        // Test list creation and indexing
        let program = make_simple_program(vec![
            Instruction::PushInt(1),
            Instruction::PushInt(2),
            Instruction::PushInt(3),
            Instruction::MakeList(3),
            Instruction::PushInt(1),
            Instruction::Index,
        ]);

        vm.run(&program).unwrap();
        assert_eq!(vm.stack[0], Value::Int(2)); // Index 1 should be 2
    }

    #[test]
    fn test_vm_conditional_jumps() {
        let mut vm = Vm::new();

        // Test conditional jump (if true, skip next instruction)
        let program = make_simple_program(vec![
            Instruction::PushBool(true),
            Instruction::JumpIfFalse(4), // Should not jump
            Instruction::PushInt(1),     // Should execute
            Instruction::Jump(5),        // Skip next instruction
            Instruction::PushInt(2),     // Should skip
        ]);

        vm.run(&program).unwrap();
        assert_eq!(vm.stack.len(), 1);
        assert_eq!(vm.stack[0], Value::Int(1));
    }

    #[test]
    fn test_vm_division_by_zero() {
        let mut vm = Vm::new();

        // Test division by zero error
        let program = make_simple_program(vec![
            Instruction::PushInt(10),
            Instruction::PushInt(0),
            Instruction::Div,
        ]);

        let result = vm.run(&program);
        assert!(result.is_err());
        assert!(result.unwrap_err().msg.contains("division by zero"));
    }

    #[test]
    fn test_vm_stack_underflow() {
        let mut vm = Vm::new();

        // Test stack underflow in Add
        let program = make_simple_program(vec![
            Instruction::PushInt(5),
            Instruction::Add, // Only one operand, should cause underflow
        ]);

        let result = vm.run(&program);
        assert!(result.is_err());
        assert!(result.unwrap_err().msg.contains("stack underflow"));
    }

    #[test]
    fn test_vm_invalid_index() {
        let mut vm = Vm::new();

        // Test out of bounds list index
        let program = make_simple_program(vec![
            Instruction::PushInt(1),
            Instruction::PushInt(2),
            Instruction::MakeList(2),
            Instruction::PushInt(5), // Index 5 is out of bounds
            Instruction::Index,
        ]);

        let result = vm.run(&program);
        assert!(result.is_err());
        assert!(result.unwrap_err().msg.contains("index out of bounds"));
    }

    #[test]
    fn test_vm_builtin_len() {
        let mut vm = Vm::new();

        // Test len() builtin with string
        let program = make_simple_program(vec![
            Instruction::PushStr("hello".to_string()),
            Instruction::BuiltinCall(Builtin::Len, 1),
        ]);

        vm.run(&program).unwrap();
        assert_eq!(vm.stack[0], Value::Int(5));
    }

    #[test]
    fn test_vm_pop_operation() {
        let mut vm = Vm::new();

        // Test Pop instruction
        let program = make_simple_program(vec![Instruction::PushInt(42), Instruction::Pop]);

        let result = vm.run(&program).unwrap();
        assert_eq!(result, Some(Value::Int(42))); // Pop sets last_value
        assert_eq!(vm.stack.len(), 0); // Stack should be empty
    }

    #[test]
    fn test_vm_push_pop_var() {
        // push/pop on a global list variable (what the compiler emits
        // for `push(xs, v)` / `pop(xs)`).
        let mut vm = Vm::new();
        let program = make_simple_program(vec![
            Instruction::PushInt(1),
            Instruction::PushInt(2),
            Instruction::MakeList(2),
            Instruction::StoreGlobal("xs".to_string()),
            Instruction::PushInt(3),
            Instruction::PushToVar(VarRef::Global("xs".to_string())),
            Instruction::PopFromVar(VarRef::Global("xs".to_string())),
        ]);
        vm.run(&program).unwrap();
        // PushToVar leaves Unit, PopFromVar leaves the popped value.
        assert_eq!(vm.stack, vec![Value::Unit, Value::Int(3)]);
        assert_eq!(
            vm.globals_snapshot(),
            vec![(
                "xs".to_string(),
                Value::List(vec![Value::Int(1), Value::Int(2)])
            )]
        );
    }

    #[test]
    fn test_vm_hex_bin_have_prefixes() {
        for (builtin, expected) in [(Builtin::Hex, "0xff"), (Builtin::Bin, "0b101")] {
            let mut vm = Vm::new();
            let program = make_simple_program(vec![
                Instruction::PushInt(if builtin == Builtin::Hex { 255 } else { 5 }),
                Instruction::BuiltinCall(builtin, 1),
            ]);
            vm.run(&program).unwrap();
            assert_eq!(vm.stack[0], Value::Str(expected.to_string()));
        }
    }
}

#[derive(Clone, Copy)]
enum CodeRef {
    Main,
    Func(usize),
}

pub struct Vm {
    stack: Vec<Value>,
    globals: HashMap<String, Value>,
    /// Debug span of the last executed instruction (for errors).
    last_span: Option<(usize, usize)>,
    /// Name of the function containing the last instruction.
    last_func: Option<String>,
}

impl Default for Vm {
    fn default() -> Self {
        Self::new()
    }
}

impl Vm {
    pub fn new() -> Self {
        Self {
            stack: Vec::new(),
            globals: HashMap::new(),
            last_span: None,
            last_func: None,
        }
    }

    pub fn globals_snapshot(&self) -> Vec<(String, Value)> {
        let mut v: Vec<(String, Value)> = self
            .globals
            .iter()
            .map(|(k, val)| (k.clone(), val.clone()))
            .collect();
        v.sort_by(|a, b| a.0.cmp(&b.0));
        v
    }

    pub fn run(&mut self, program: &Program) -> Result<Option<Value>> {
        // Track the debug span of the last executed instruction so
        // runtime errors can be located (line:col of the source
        // statement that emitted it).
        self.last_span = None;
        self.last_func = None;
        let result = self.run_inner(program);
        result.map_err(|e| {
            if let Some(span) = self.last_span {
                e.with_span_if_missing(zirc_syntax::error::Span::new(span.0, span.1))
            } else {
                e
            }
        })
    }

    fn run_inner(&mut self, program: &Program) -> Result<Option<Value>> {
        if program.version != zirc_bytecode::BYTECODE_VERSION {
            return error(format!(
                "unsupported bytecode version {} (expected {})",
                program.version,
                zirc_bytecode::BYTECODE_VERSION
            ));
        }
        let mut frames: Vec<Frame> = Vec::new();
        frames.push(Frame {
            func_ref: CodeRef::Main,
            ip: 0,
            locals: vec![Value::Unit; program.main.local_count],
        });

        let mut last_value: Option<Value> = None;
        // Resource limits: unbounded recursion or stack growth is a
        // VM error, never a host crash or OOM. Frame depth is checked
        // in the `Call` arm (where `frames` is not borrowed).
        const MAX_STACK: usize = 10_000;
        while let Some(frame) = frames.last_mut() {
            if self.stack.len() > MAX_STACK {
                return error("stack overflow");
            }
            let func = match frame.func_ref {
                CodeRef::Main => &program.main,
                CodeRef::Func(i) => &program.functions[i],
            };
            if frame.ip >= func.code.len() {
                // Implicit return Unit if we run off the end
                if frames.len() == 1 {
                    break;
                } // main returns ends program
                let ret = Value::Unit;
                frames.pop();
                self.stack.push(ret);
                continue;
            }
            let instr = func.code[frame.ip].clone();
            if let Some(span) = func.debug.get(frame.ip).copied().flatten() {
                self.last_span = Some(span);
                self.last_func = Some(func.name.clone());
            }
            // default ip increment; jumps will override
            frame.ip += 1;
            match instr {
                Instruction::PushInt(n) => self.stack.push(Value::Int(n)),
                Instruction::PushFloat(v) => self.stack.push(Value::Float(v)),
                Instruction::PushStr(s) => self.stack.push(Value::Str(s)),
                Instruction::PushBool(b) => self.stack.push(Value::Bool(b)),
                Instruction::PushUnit => self.stack.push(Value::Unit),
                Instruction::MakeList(n) => {
                    if self.stack.len() < n {
                        return error("stack underflow in MakeList");
                    }
                    let start = self.stack.len() - n;
                    let elems = self.stack.drain(start..).collect::<Vec<_>>();
                    // elems are in original order already because we drained a slice
                    self.stack.push(Value::List(elems));
                }
                Instruction::Index => {
                    let idx = self.stack.pop().ok_or("stack underflow in Index")?;
                    let base = self.stack.pop().ok_or("stack underflow in Index")?;
                    let ix = match idx {
                        Value::Int(n) => n,
                        other => return error(format!("index expects int, got {:?}", other)),
                    };
                    match base {
                        Value::List(items) => {
                            if ix < 0 || (ix as usize) >= items.len() {
                                return error("index out of bounds");
                            }
                            self.stack.push(items[ix as usize].clone());
                        }
                        Value::Str(s) => {
                            let chars: Vec<char> = s.chars().collect();
                            if ix < 0 || (ix as usize) >= chars.len() {
                                return error("index out of bounds");
                            }
                            self.stack.push(Value::Str(chars[ix as usize].to_string()));
                        }
                        other => return error(format!("indexing not supported for {:?}", other)),
                    }
                }
                Instruction::LoadLocal(i) => {
                    let i = i as usize;
                    let v = frame.locals.get(i).ok_or("invalid local index")?.clone();
                    self.stack.push(v);
                }
                Instruction::StoreLocal(i) => {
                    let i = i as usize;
                    let v = self.stack.pop().ok_or("stack underflow in StoreLocal")?;
                    let slot = frame.locals.get_mut(i).ok_or("invalid local index")?;
                    *slot = v;
                }
                Instruction::Pop => {
                    let v = self.stack.pop();
                    if let Some(val) = v {
                        last_value = Some(val);
                    }
                }
                Instruction::CheckType(ty) => {
                    let v = self.stack.pop().ok_or("stack underflow in CheckType")?;
                    let ok = matches!(
                        (&v, &ty),
                        (Value::Int(_), zirc_syntax::ast::Type::Int)
                            | (Value::Float(_), zirc_syntax::ast::Type::Float)
                            | (Value::Str(_), zirc_syntax::ast::Type::String)
                            | (Value::Bool(_), zirc_syntax::ast::Type::Bool)
                            | (Value::List(_), zirc_syntax::ast::Type::List)
                            | (Value::Unit, zirc_syntax::ast::Type::Unit)
                    );
                    if !ok {
                        return Err(zirc_syntax::error::Error::of_kind(
                            zirc_syntax::error::ErrorKind::Type,
                            format!("Type mismatch: value {:?} does not match type {:?}", v, ty),
                        ));
                    }
                    self.stack.push(v);
                }
                Instruction::Add => {
                    let b = self.stack.pop().ok_or("stack underflow in Add")?;
                    let a = self.stack.pop().ok_or("stack underflow in Add")?;
                    match (a, b) {
                        (Value::Int(x), Value::Int(y)) => {
                            self.stack.push(Value::Int(x.checked_add(y).ok_or_else(|| {
                                zirc_syntax::error::Error::of_kind(
                                    zirc_syntax::error::ErrorKind::Vm,
                                    "integer overflow",
                                )
                            })?))
                        }
                        (Value::Str(x), Value::Str(y)) => {
                            self.stack.push(Value::Str(format!("{}{}", x, y)))
                        }
                        (Value::List(mut x), Value::List(y)) => {
                            x.extend(y);
                            self.stack.push(Value::List(x));
                        }
                        (x, y) if x.as_f64().is_some() && y.as_f64().is_some() => self
                            .stack
                            .push(Value::Float(x.as_f64().unwrap() + y.as_f64().unwrap())),
                        (x, y) => return error(format!("Cannot add {:?} and {:?}", x, y)),
                    }
                }
                Instruction::Sub => {
                    let b = self.stack.pop().ok_or("stack underflow in Sub")?;
                    let a = self.stack.pop().ok_or("stack underflow in Sub")?;
                    match (a, b) {
                        (Value::Int(x), Value::Int(y)) => {
                            self.stack.push(Value::Int(x.checked_sub(y).ok_or_else(|| {
                                zirc_syntax::error::Error::of_kind(
                                    zirc_syntax::error::ErrorKind::Vm,
                                    "integer overflow",
                                )
                            })?))
                        }
                        (x, y) if x.as_f64().is_some() && y.as_f64().is_some() => self
                            .stack
                            .push(Value::Float(x.as_f64().unwrap() - y.as_f64().unwrap())),
                        (x, y) => return error(format!("Cannot subtract {:?} and {:?}", x, y)),
                    }
                }
                Instruction::Mul => {
                    let b = self.stack.pop().ok_or("stack underflow in Mul")?;
                    let a = self.stack.pop().ok_or("stack underflow in Mul")?;
                    match (a, b) {
                        (Value::Int(x), Value::Int(y)) => {
                            self.stack.push(Value::Int(x.checked_mul(y).ok_or_else(|| {
                                zirc_syntax::error::Error::of_kind(
                                    zirc_syntax::error::ErrorKind::Vm,
                                    "integer overflow",
                                )
                            })?))
                        }
                        (x, y) if x.as_f64().is_some() && y.as_f64().is_some() => self
                            .stack
                            .push(Value::Float(x.as_f64().unwrap() * y.as_f64().unwrap())),
                        (x, y) => return error(format!("Cannot multiply {:?} and {:?}", x, y)),
                    }
                }
                Instruction::Div => {
                    let b = self.stack.pop().ok_or("stack underflow in Div")?;
                    let a = self.stack.pop().ok_or("stack underflow in Div")?;
                    match (a, b) {
                        (Value::Int(_), Value::Int(0)) => return error("division by zero"),
                        (Value::Int(x), Value::Int(y)) => {
                            self.stack.push(Value::Int(x.checked_div(y).ok_or_else(|| {
                                zirc_syntax::error::Error::of_kind(
                                    zirc_syntax::error::ErrorKind::Vm,
                                    "integer overflow",
                                )
                            })?))
                        }
                        (x, y) if x.as_f64().is_some() && y.as_f64().is_some() => {
                            let d = y.as_f64().unwrap();
                            if d == 0.0 {
                                return error("division by zero");
                            }
                            self.stack.push(Value::Float(x.as_f64().unwrap() / d))
                        }
                        (x, y) => return error(format!("Cannot divide {:?} and {:?}", x, y)),
                    }
                }
                Instruction::Eq => {
                    let b = self.stack.pop().ok_or("stack underflow in Eq")?;
                    let a = self.stack.pop().ok_or("stack underflow in Eq")?;
                    self.stack.push(Value::Bool(vm_values_equal(&a, &b)));
                }
                Instruction::Ne => {
                    let b = self.stack.pop().ok_or("stack underflow in Ne")?;
                    let a = self.stack.pop().ok_or("stack underflow in Ne")?;
                    self.stack.push(Value::Bool(!vm_values_equal(&a, &b)));
                }
                Instruction::Lt => {
                    let b = self.stack.pop().ok_or("stack underflow in Lt")?;
                    let a = self.stack.pop().ok_or("stack underflow in Lt")?;
                    match (a.as_f64(), b.as_f64()) {
                        (Some(x), Some(y)) => self.stack.push(Value::Bool(x < y)),
                        _ => return error("< expects numbers"),
                    }
                }
                Instruction::Le => {
                    let b = self.stack.pop().ok_or("stack underflow in Le")?;
                    let a = self.stack.pop().ok_or("stack underflow in Le")?;
                    match (a.as_f64(), b.as_f64()) {
                        (Some(x), Some(y)) => self.stack.push(Value::Bool(x <= y)),
                        _ => return error("<= expects numbers"),
                    }
                }
                Instruction::Gt => {
                    let b = self.stack.pop().ok_or("stack underflow in Gt")?;
                    let a = self.stack.pop().ok_or("stack underflow in Gt")?;
                    match (a.as_f64(), b.as_f64()) {
                        (Some(x), Some(y)) => self.stack.push(Value::Bool(x > y)),
                        _ => return error("> expects numbers"),
                    }
                }
                Instruction::Ge => {
                    let b = self.stack.pop().ok_or("stack underflow in Ge")?;
                    let a = self.stack.pop().ok_or("stack underflow in Ge")?;
                    match (a.as_f64(), b.as_f64()) {
                        (Some(x), Some(y)) => self.stack.push(Value::Bool(x >= y)),
                        _ => return error(">= expects numbers"),
                    }
                }
                Instruction::Not => {
                    let a = self.stack.pop().ok_or("stack underflow in Not")?;
                    match a {
                        Value::Bool(b) => self.stack.push(Value::Bool(!b)),
                        other => return error(format!("! expects bool, got {:?}", other)),
                    }
                }
                Instruction::Neg => {
                    let a = self.stack.pop().ok_or("stack underflow in Neg")?;
                    match a {
                        Value::Int(n) => {
                            self.stack.push(Value::Int(n.checked_neg().ok_or_else(|| {
                                zirc_syntax::error::Error::of_kind(
                                    zirc_syntax::error::ErrorKind::Vm,
                                    "integer overflow",
                                )
                            })?))
                        }
                        Value::Float(v) => self.stack.push(Value::Float(-v)),
                        other => return error(format!("- expects number, got {:?}", other)),
                    }
                }
                Instruction::Jump(tgt) => {
                    frame.ip = tgt;
                }
                Instruction::JumpIfFalse(tgt) => {
                    let c = self.stack.pop().ok_or("stack underflow in JumpIfFalse")?;
                    match c {
                        Value::Bool(false) => frame.ip = tgt,
                        Value::Bool(true) => (),
                        other => return error(format!("condition must be bool, got {:?}", other)),
                    }
                }
                Instruction::JumpIfTrue(tgt) => {
                    let c = self.stack.pop().ok_or("stack underflow in JumpIfTrue")?;
                    match c {
                        Value::Bool(true) => frame.ip = tgt,
                        Value::Bool(false) => (),
                        other => return error(format!("condition must be bool, got {:?}", other)),
                    }
                }
                Instruction::Call(fi, argc) => {
                    // collect args
                    if frames.len() >= 1000 {
                        return error("call stack overflow (too many nested calls)");
                    }
                    if self.stack.len() < argc {
                        return error("stack underflow in Call");
                    }
                    let start = self.stack.len() - argc;
                    let mut args = self.stack.drain(start..).collect::<Vec<_>>();
                    // args now in original order
                    let func = program.functions.get(fi).ok_or("invalid function index")?;
                    if func.arity != argc {
                        return error(format!(
                            "Function '{}' expected {} args, got {}",
                            func.name, func.arity, argc
                        ));
                    }
                    // prepare locals
                    let mut locals = vec![Value::Unit; func.local_count];
                    for (i, v) in args.drain(..).enumerate() {
                        locals[i] = v;
                    }
                    // push frame
                    frames.push(Frame {
                        func_ref: CodeRef::Func(fi),
                        ip: 0,
                        locals,
                    });
                }
                Instruction::Return => {
                    let ret = self.stack.pop().unwrap_or(Value::Unit);
                    frames.pop();
                    if frames.is_empty() {
                        // returning from main -> end
                        break;
                    }
                    self.stack.push(ret);
                }
                Instruction::MakeClosure(fi, capture_count) => {
                    if self.stack.len() < capture_count {
                        return error("stack underflow in MakeClosure");
                    }
                    let start = self.stack.len() - capture_count;
                    let captures = self.stack.drain(start..).collect::<Vec<_>>();
                    if program.functions.get(fi).is_none() {
                        return error("invalid function index in MakeClosure");
                    }
                    self.stack.push(Value::Closure { func: fi, captures });
                }
                Instruction::CallIndirect(argc) => {
                    if frames.len() >= 1000 {
                        return error("call stack overflow (too many nested calls)");
                    }
                    if self.stack.len() < argc + 1 {
                        return error("stack underflow in CallIndirect");
                    }
                    let args_start = self.stack.len() - argc;
                    let mut args = self.stack.drain(args_start..).collect::<Vec<_>>();
                    let closure = self.stack.pop().ok_or("stack underflow in CallIndirect")?;
                    let (fi, captures) = match closure {
                        Value::Closure { func, captures } => (func, captures),
                        other => {
                            return error(format!(
                                "attempted to call a non-closure value: {:?}",
                                other
                            ))
                        }
                    };
                    let func = program
                        .functions
                        .get(fi)
                        .ok_or("invalid function index in CallIndirect")?;
                    if func.arity != argc {
                        return error(format!(
                            "Function '{}' expected {} args, got {}",
                            func.name, func.arity, argc
                        ));
                    }
                    // Locals layout: [args..., captures...]
                    let mut locals = vec![Value::Unit; func.local_count.max(argc + captures.len())];
                    for (i, v) in args.drain(..).enumerate() {
                        locals[i] = v;
                    }
                    for (i, v) in captures.into_iter().enumerate() {
                        locals[argc + i] = v;
                    }
                    frames.push(Frame {
                        func_ref: CodeRef::Func(fi),
                        ip: 0,
                        locals,
                    });
                }
                Instruction::PushToVar(var_ref) => {
                    let value = self.stack.pop().ok_or("stack underflow in PushToVar")?;
                    match &var_ref {
                        VarRef::Local(i) => {
                            let slot = frame
                                .locals
                                .get_mut(*i as usize)
                                .ok_or("invalid local index")?;
                            match slot {
                                Value::List(items) => items.push(value),
                                other => {
                                    return error(format!(
                                        "push() expects list variable, got {:?}",
                                        other
                                    ))
                                }
                            }
                        }
                        VarRef::Global(name) => {
                            let current = self
                                .globals
                                .get(name)
                                .cloned()
                                .ok_or_else(|| format!("Undefined variable '{}'", name))?;
                            match current {
                                Value::List(mut items) => {
                                    items.push(value);
                                    self.globals.insert(name.clone(), Value::List(items));
                                }
                                other => {
                                    return error(format!(
                                        "push() expects list variable, got {:?}",
                                        other
                                    ))
                                }
                            }
                        }
                    }
                    // Every expression leaves exactly one value on the
                    // stack; `push()` evaluates to `Unit` (as in the
                    // interpreter), so statement-level `Pop`s and
                    // value contexts stay balanced.
                    self.stack.push(Value::Unit);
                }
                Instruction::PopFromVar(var_ref) => {
                    let popped = match &var_ref {
                        VarRef::Local(i) => {
                            let slot = frame
                                .locals
                                .get_mut(*i as usize)
                                .ok_or("invalid local index")?;
                            match slot {
                                Value::List(items) => {
                                    items.pop().ok_or("Cannot pop from empty list")?
                                }
                                other => {
                                    return error(format!(
                                        "pop() expects list variable, got {:?}",
                                        other
                                    ))
                                }
                            }
                        }
                        VarRef::Global(name) => {
                            let current = self
                                .globals
                                .get(name)
                                .cloned()
                                .ok_or_else(|| format!("Undefined variable '{}'", name))?;
                            match current {
                                Value::List(mut items) => {
                                    let v = items.pop().ok_or("Cannot pop from empty list")?;
                                    self.globals.insert(name.clone(), Value::List(items));
                                    v
                                }
                                other => {
                                    return error(format!(
                                        "pop() expects list variable, got {:?}",
                                        other
                                    ))
                                }
                            }
                        }
                    };
                    self.stack.push(popped);
                }
                Instruction::BuiltinCall(which, argc) => {
                    if self.stack.len() < argc {
                        return error("stack underflow in BuiltinCall");
                    }
                    let start = self.stack.len() - argc;
                    let args = self.stack.drain(start..).collect::<Vec<_>>();
                    let silent = std::env::var("ZIRC_BENCH_SILENT").is_ok();
                    // Pure builtins are shared with the interpreter.
                    if let Some(result) = eval_pure(which, &args) {
                        self.stack.push(result?);
                    } else {
                        match which {
                            Builtin::Show => {
                                if args.len() != 1 {
                                    return error("show() expects exactly 1 argument");
                                }
                                if !silent {
                                    println!("{}", display_value(&args[0]));
                                }
                                self.stack.push(Value::Unit);
                            }
                            Builtin::ShowF => {
                                if args.is_empty() {
                                    return error("showf requires at least a format string");
                                }
                                let fmt = match &args[0] {
                                    Value::Str(s) => s.clone(),
                                    _ => return error("showf first argument must be a string"),
                                };
                                let out = format_showf(&fmt, &args[1..])?;
                                if !silent {
                                    println!("{}", out);
                                }
                                self.stack.push(Value::Unit);
                            }
                            Builtin::Prompt => {
                                if args.len() > 1 {
                                    return error("prompt() expects 0 or 1 arguments");
                                }
                                if args.len() == 1 {
                                    if let Value::Str(s) = &args[0] {
                                        if !silent {
                                            print!("{}", s);
                                            io::stdout()
                                                .flush()
                                                .map_err(|e| format!("IO error: {}", e))?;
                                        }
                                    } else {
                                        return error("prompt() prompt must be string");
                                    }
                                }
                                let input = if silent {
                                    std::env::var("ZIRC_BENCH_PROMPT_REPLY").unwrap_or_default()
                                } else {
                                    let mut input = String::new();
                                    io::stdin()
                                        .read_line(&mut input)
                                        .map_err(|e| format!("IO error: {}", e))?;
                                    if input.ends_with('\n') {
                                        input.pop();
                                        if input.ends_with('\r') {
                                            input.pop();
                                        }
                                    }
                                    input
                                };
                                self.stack.push(Value::Str(input));
                            }
                            Builtin::Rf => {
                                if args.len() != 1 {
                                    return error("rf() expects exactly 1 argument");
                                }
                                let path = match &args[0] {
                                    Value::Str(s) => s.clone(),
                                    _ => return error("rf() path must be string"),
                                };
                                let content = fs::read_to_string(&path).map_err(|e| {
                                    format!("Failed to read file '{}': {}", path, e)
                                })?;
                                self.stack.push(Value::Str(content));
                            }
                            Builtin::Wf => {
                                if args.len() != 2 {
                                    return error(
                                        "wf() expects exactly 2 arguments: path and content",
                                    );
                                }
                                let path = match &args[0] {
                                    Value::Str(s) => s.clone(),
                                    _ => return error("wf() path must be string"),
                                };
                                let content = match &args[1] {
                                    Value::Str(s) => s.clone(),
                                    _ => return error("wf() content must be string"),
                                };
                                fs::write(&path, &content).map_err(|e| {
                                    format!("Failed to write file '{}': {}", path, e)
                                })?;
                                self.stack.push(Value::Unit);
                            }
                            Builtin::Push | Builtin::Pop => {
                                return error("push()/pop() require a variable; the compiler emits PushToVar/PopFromVar for them");
                            }
                            pure => {
                                // Unreachable: pure builtins are handled above.
                                return error(format!(
                                    "{}() is not supported in VM mode",
                                    pure.name()
                                ));
                            }
                        }
                    }
                }
                Instruction::Halt => {
                    break;
                }
                Instruction::LoadGlobal(name) => {
                    let v = self
                        .globals
                        .get(&name)
                        .cloned()
                        .ok_or_else(|| format!("Undefined variable '{}'", name))?;
                    self.stack.push(v);
                }
                Instruction::StoreGlobal(name) => {
                    let v = self.stack.pop().ok_or("stack underflow in StoreGlobal")?;
                    self.globals.insert(name, v);
                }
            }
        }
        Ok(last_value)
    }
}

//! `zirc-bench`: benchmark harness for Zirc programs.
//!
//! Discovers scripts under `benchmark/scripts/`, measures lexer,
//! parser and execution time separately, and writes JSON results.
//! The default backend is the canonical VM (`--backend vm`);
//! `--backend interp` benchmarks the reference interpreter.

use std::fs;
use std::path::{Path, PathBuf};
use std::time::Instant;

use clap::{ArgAction, Parser};
use serde::Serialize;

use zirc_compiler::Compiler;
use zirc_interpreter::Interpreter;
use zirc_lexer::Lexer;
use zirc_parser::Parser as ZircParser;
use zirc_vm::Vm;

#[derive(Parser, Debug)]
#[command(name = "zirc-bench", about = "Run Zirc benchmarks")]
struct Cli {
    /// Specific test(s) to run (by name, e.g. fibonacci). If omitted, runs all discovered scripts.
    #[arg(short = 't', long = "test", action = ArgAction::Append)]
    tests: Vec<String>,

    /// Iterations per test (measured)
    #[arg(short = 'n', long = "iterations", default_value_t = 10)]
    iterations: u32,

    /// Warmup iterations (not measured)
    #[arg(short = 'w', long = "warmup", default_value_t = 2)]
    warmup: u32,

    /// Output JSON file path; default: benchmark/results/&lt;timestamp&gt;.json
    #[arg(short = 'o', long = "output")]
    output: Option<PathBuf>,

    /// Include examples/ directory in discovery (off by default to avoid interactive scripts)
    #[arg(long = "include-examples", default_value_t = false)]
    include_examples: bool,

    /// Silence program output (and auto-reply for prompt) during benchmarks. Set to false to see prints.
    #[arg(long = "silent", default_value_t = true)]
    silent: bool,

    /// List discovered tests and exit
    #[arg(long = "list", default_value_t = false)]
    list: bool,

    /// Execution backend to benchmark: `vm` (canonical, default)
    /// or `interp` (reference interpreter).
    #[arg(short = 'b', long = "backend", default_value = "vm")]
    backend: String,
}

#[derive(Debug, Serialize)]
struct BenchResult {
    name: String,
    backend: String,
    iterations: u32,
    avg_total_ms: f64,
    min_total_ms: f64,
    max_total_ms: f64,
    avg_lex_ms: f64,
    avg_parse_ms: f64,
    avg_exec_ms: f64,
    memory_usage_kb: u64,
}

#[derive(Debug, Serialize)]
struct OutputDoc {
    timestamp: String,
    zirc_version: String,
    benchmarks: Vec<BenchResult>,
}

#[derive(Debug, Clone)]
struct ScriptCase {
    name: String,
    path: PathBuf,
}

fn workspace_root() -> PathBuf {
    // crates/benchmark-runner -> crates -> root
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .to_path_buf()
}

fn discover_scripts(include_examples: bool) -> Vec<ScriptCase> {
    let root = workspace_root();
    let mut out = Vec::new();

    let mut candidates = vec![root.join("benchmark/scripts")];
    if include_examples {
        candidates.push(root.join("examples"));
    }

    for dir in candidates {
        if !dir.exists() {
            continue;
        }
        if let Ok(entries) = fs::read_dir(&dir) {
            for e in entries.flatten() {
                let p = e.path();
                if p.extension().and_then(|s| s.to_str()) == Some("zirc") {
                    let name = p
                        .file_stem()
                        .and_then(|s| s.to_str())
                        .unwrap_or("")
                        .to_string();
                    out.push(ScriptCase { name, path: p });
                }
            }
        }
    }

    out.sort_by(|a, b| a.name.cmp(&b.name));
    out
}

fn read_script(path: &Path) -> String {
    fs::read_to_string(path).unwrap_or_else(|e| panic!("Failed to read {}: {}", path.display(), e))
}

fn run_once(src: &str, backend: &str) -> u64 {
    let mut lexer = Lexer::new(src);
    let tokens = lexer.tokenize().expect("lex error");
    let mut parser = ZircParser::new(tokens);
    let program = parser.parse_program().expect("parse error");
    if backend == "interp" {
        let mut interp = Interpreter::new();
        interp.run(program).expect("runtime error");
        interp.memory_stats().bytes_allocated as u64
    } else {
        let mut compiler = Compiler::new();
        let bprog = compiler.compile(program).expect("compile error");
        let mut vm = Vm::new();
        vm.run(&bprog).expect("runtime error");
        0
    }
}

fn measure_script(
    src: &str,
    iterations: u32,
    warmup: u32,
    backend: &str,
) -> (Vec<f64>, Vec<f64>, Vec<f64>, Vec<f64>, u64) {
    // Warmup
    for _ in 0..warmup {
        run_once(src, backend);
    }

    let mut totals = Vec::with_capacity(iterations as usize);
    let mut lexes = Vec::with_capacity(iterations as usize);
    let mut parses = Vec::with_capacity(iterations as usize);
    let mut execs = Vec::with_capacity(iterations as usize);
    let mut last_mem_bytes: u64 = 0;

    for _i in 0..iterations {
        let t0 = Instant::now();
        let mut t = Instant::now();

        let mut lexer = Lexer::new(src);
        let tokens = lexer.tokenize().expect("lex error");
        let t_lex = t.elapsed();

        t = Instant::now();
        let mut parser = ZircParser::new(tokens);
        let program = parser.parse_program().expect("parse error");
        let t_parse = t.elapsed();

        t = Instant::now();
        // Re-parse is already done above; execution uses a fresh
        // backend run over the same program would double-count, so
        // time a full backend execution of the parsed program here:
        // for VM, compile+run; for interp, run.
        if backend == "interp" {
            let mut interp = Interpreter::new();
            interp.run(program).expect("runtime error");
            last_mem_bytes = interp.memory_stats().bytes_allocated as u64;
        } else {
            let mut compiler = Compiler::new();
            let bprog = compiler.compile(program).expect("compile error");
            let mut vm = Vm::new();
            vm.run(&bprog).expect("runtime error");
        }
        let t_exec = t.elapsed();

        let total = t0.elapsed();

        lexes.push(dur_ms(t_lex));
        parses.push(dur_ms(t_parse));
        execs.push(dur_ms(t_exec));
        totals.push(dur_ms(total));
    }

    (totals, lexes, parses, execs, last_mem_bytes)
}

fn dur_ms(d: std::time::Duration) -> f64 {
    d.as_secs_f64() * 1000.0
}

fn stats(vals: &[f64]) -> (f64, f64, f64) {
    let min = vals.iter().cloned().fold(f64::INFINITY, f64::min);
    let max = vals.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
    let avg = if vals.is_empty() {
        0.0
    } else {
        vals.iter().sum::<f64>() / (vals.len() as f64)
    };
    (avg, min, max)
}

fn ensure_dir(p: &Path) {
    if let Err(e) = fs::create_dir_all(p) {
        panic!("Failed to create {}: {}", p.display(), e);
    }
}

fn main() {
    let cli = Cli::parse();

    // Silence program output and auto-reply for prompt() during benchmarking by default
    if cli.silent {
        std::env::set_var("ZIRC_BENCH_SILENT", "1");
        // default auto-reply text for prompt() in silent mode
        std::env::set_var("ZIRC_BENCH_PROMPT_REPLY", "");
    }

    let mut scripts = discover_scripts(cli.include_examples);

    if cli.list {
        println!("Discovered tests:");
        for s in &scripts {
            println!("- {} ({} )", s.name, s.path.display());
        }
        return;
    }

    if !cli.tests.is_empty() {
        let wanted: std::collections::HashSet<_> =
            cli.tests.iter().map(|s| s.to_lowercase()).collect();
        scripts.retain(|s| wanted.contains(&s.name.to_lowercase()));
        if scripts.is_empty() {
            eprintln!("No matching tests. Use --list to see available.");
            std::process::exit(2);
        }
    }

    if scripts.is_empty() {
        eprintln!("No .zirc scripts found in benchmark/scripts or examples.");
        std::process::exit(2);
    }

    let mut results = Vec::new();

    for case in &scripts {
        let src = read_script(&case.path);
        let (totals, lexes, parses, execs, mem_bytes) =
            measure_script(&src, cli.iterations, cli.warmup, &cli.backend);
        let (avg_t, min_t, max_t) = stats(&totals);
        let (avg_l, _, _) = stats(&lexes);
        let (avg_p, _, _) = stats(&parses);
        let (avg_e, _, _) = stats(&execs);
        let mem_kb = (mem_bytes + 1023) / 1024;

        println!(
            "{:>12}: total avg={:.3}ms min={:.3}ms max={:.3}ms | lex={:.3}ms parse={:.3}ms exec={:.3}ms | mem={}KB",
            case.name, avg_t, min_t, max_t, avg_l, avg_p, avg_e, mem_kb
        );

        results.push(BenchResult {
            name: case.name.clone(),
            backend: cli.backend.clone(),
            iterations: cli.iterations,
            avg_total_ms: avg_t,
            min_total_ms: min_t,
            max_total_ms: max_t,
            avg_lex_ms: avg_l,
            avg_parse_ms: avg_p,
            avg_exec_ms: avg_e,
            memory_usage_kb: mem_kb,
        });
    }

    // Prepare output path
    let out_path = if let Some(p) = cli.output.clone() {
        p
    } else {
        let root = workspace_root();
        let results_dir = root.join("benchmark/results");
        ensure_dir(&results_dir);
        // Human-friendly, Windows-safe filename timestamp
        let ts_file = chrono::Utc::now().format("%Y-%m-%d_%H-%M-%SZ").to_string();
        results_dir.join(format!("{}.json", ts_file))
    };

    let doc = OutputDoc {
        // Human-friendly ISO-8601 UTC without fractional seconds
        timestamp: chrono::Utc::now().format("%Y-%m-%dT%H:%M:%SZ").to_string(),
        zirc_version: env!("CARGO_PKG_VERSION").to_string(),
        benchmarks: results,
    };

    let json = serde_json::to_string_pretty(&doc).expect("serialize json");
    if let Some(parent) = out_path.parent() {
        ensure_dir(parent);
    }
    fs::write(&out_path, json).expect("write results json");

    println!("\nSaved results to {}", out_path.display());
}

use assert_cmd::prelude::*;
use predicates::prelude::*;
use std::path::PathBuf;
use std::process::Command;

fn workspace_root() -> PathBuf {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    manifest_dir
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .to_path_buf()
}

fn zirc_cmd() -> std::process::Command {
    Command::cargo_bin("zirc").unwrap()
}

/// Run a shipped example file under both backends and assert the
/// outputs are identical (the differential invariant).
fn run_example_both_backends(file: &std::path::Path) {
    let mut vm_cmd = zirc_cmd();
    vm_cmd.arg(file);
    let vm = vm_cmd.assert().success().get_output().stdout.clone();

    let mut interp_cmd = zirc_cmd();
    interp_cmd.arg("--backend").arg("interp").arg(file);
    let interp = interp_cmd.assert().success().get_output().stdout.clone();

    assert_eq!(
        String::from_utf8(vm).unwrap(),
        String::from_utf8(interp).unwrap(),
        "backends disagree on {}",
        file.display()
    );
}

#[test]
fn runs_factorial_example() {
    let root = workspace_root();
    let mut cmd = zirc_cmd();
    cmd.arg(root.join("examples/others/factorial.zirc"));
    cmd.assert()
        .success()
        .stdout(predicate::str::contains("fact(5) = 120"));
}

#[test]
fn runs_conditionals_example() {
    let root = workspace_root();
    let mut cmd = zirc_cmd();
    cmd.arg(root.join("examples/basic/conditionals.zirc"));
    cmd.assert()
        .success()
        .stdout(predicate::str::contains("3 is less than 5"));
}

#[test]
fn parse_error_is_nonzero() {
    let bad = "fun x(\n"; // malformed on purpose
    let tmp_dir = tempfile::tempdir().unwrap();
    let bad_path = tmp_dir.path().join("bad.zirc");
    std::fs::write(&bad_path, bad).unwrap();

    let mut cmd = zirc_cmd();
    cmd.arg(bad_path);
    cmd.assert()
        .failure()
        .stderr(predicate::str::contains("Parse error"));
}

/// Run an inline program under both backends and return stdout.
fn run_both_backends(source: &str, file_name: &str) -> (String, String) {
    let tmp_dir = tempfile::tempdir().unwrap();
    let path = tmp_dir.path().join(file_name);
    std::fs::write(&path, source).unwrap();

    let mut vm_cmd = zirc_cmd();
    vm_cmd.arg(&path);
    let vm_out = vm_cmd.assert().success().get_output().stdout.clone();

    let mut interp_cmd = zirc_cmd();
    interp_cmd.arg("--backend").arg("interp").arg(&path);
    let interp_out = interp_cmd.assert().success().get_output().stdout.clone();

    (
        String::from_utf8(vm_out).unwrap(),
        String::from_utf8(interp_out).unwrap(),
    )
}

/// Differential test: both backends must produce identical stdout.
/// This is the test that catches backend drift (hex/bin prefixes,
/// push/pop support, division-by-zero handling, ...).
#[test]
fn backends_agree_on_builtins() {
    let (vm, interp) = run_both_backends(
        r#"
let xs = [1, 2, 3]
push(xs, 4)
let last = pop(xs)
showf("xs=%s last=%d hex=%s bin=%s pow=%d sqrt=%d", xs, last, hex(255), bin(5), pow(2, 10), sqrt(17))
showf("len=%d slice=%s upper=%s", len(xs), slice(xs, 1, 3), upper("hi"))
"#,
        "builtins.zirc",
    );
    assert_eq!(vm, interp, "VM and interpreter outputs differ");
    assert!(vm.contains("xs=[1, 2, 3] last=4 hex=0xff bin=0b101 pow=1024 sqrt=4"));
}

#[test]
fn backends_agree_on_control_flow() {
    let (vm, interp) = run_both_backends(
        r#"
let total = 0
for i in 0..5:
  if i == 3:
    continue
  end
  total = total + i
end
showf("total=%d", total)
"#,
        "flow.zirc",
    );
    assert_eq!(vm, interp, "VM and interpreter outputs differ");
    assert!(vm.contains("total=7"));
}

#[test]
fn backends_agree_on_floats() {
    let (vm, interp) = run_both_backends(
        r#"
let pi = 3.14
let area = pi * 2.0
let mixed = 1 + 0.5
let neg = -pi
showf("area=%s mixed=%s neg=%s eq=%s", str(area), str(mixed), str(neg), str(1 == 1.0))
"#,
        "floats.zirc",
    );
    assert_eq!(vm, interp, "VM and interpreter outputs differ");
    assert!(vm.contains("area=6.28 mixed=1.5 neg=-3.14 eq=true"));
}

#[test]
fn modules_import_and_qualified_calls() {
    let root = workspace_root();
    let mut cmd = zirc_cmd();
    cmd.arg(root.join("examples/modules/main.zirc"));
    cmd.assert()
        .success()
        .stdout(predicate::str::contains("area_rect(3, 4) = 12"))
        .stdout(predicate::str::contains("area_square(5) = 25"));

    let mut interp_cmd = zirc_cmd();
    interp_cmd
        .arg("--backend")
        .arg("interp")
        .arg(root.join("examples/modules/main.zirc"));
    interp_cmd
        .assert()
        .success()
        .stdout(predicate::str::contains("perimeter(3, 4) = 14"));
}

#[test]
fn missing_module_is_an_error() {
    let tmp_dir = tempfile::tempdir().unwrap();
    let path = tmp_dir.path().join("main.zirc");
    std::fs::write(&path, "import does_not_exist_xyz\n").unwrap();
    let mut cmd = zirc_cmd();
    cmd.arg(&path);
    cmd.assert()
        .failure()
        .stderr(predicate::str::contains("not found"));
}

#[test]
fn import_cycle_is_an_error() {
    let tmp_dir = tempfile::tempdir().unwrap();
    std::fs::write(tmp_dir.path().join("a.zirc"), "import b\n").unwrap();
    std::fs::write(tmp_dir.path().join("b.zirc"), "import a\n").unwrap();
    let main = tmp_dir.path().join("main.zirc");
    std::fs::write(&main, "import a\n").unwrap();
    let mut cmd = zirc_cmd();
    cmd.arg(&main);
    cmd.assert()
        .failure()
        .stderr(predicate::str::contains("cycle"));
}

#[test]
fn backends_agree_on_closures() {
    let (vm, interp) = run_both_backends(
        r#"
let base = 100
let add = fun(x, y): x + y + base end
showf("add=%d", add(1, 2))
showf("direct=%d", (fun(x): x + 1 end)(41))
fun make_adder(n):
  return fun(x): x + n end
end
let add5 = make_adder(5)
showf("add5=%d", add5(10))
let fs = []
push(fs, fun(x): x + 1 end)
showf("fs=%d", fs[0](1))
"#,
        "closures.zirc",
    );
    assert_eq!(vm, interp, "VM and interpreter outputs differ");
    assert!(vm.contains("add=103"));
    assert!(vm.contains("add5=15"));
}

/// Every runnable shipped example must succeed on both backends
/// with identical output. Intentional-failure examples live under
/// `errors/` (asserted separately below) and `io/` is interactive.
#[test]
fn all_runnable_examples_agree_across_backends() {
    let root = workspace_root();
    for rel in [
        "examples/basic/hello.zirc",
        "examples/basic/conditionals.zirc",
        "examples/basic/loops.zirc",
        "examples/basic/for_loop.zirc",
        "examples/basic/plus.zirc",
        "examples/basic/test_builtins.zirc",
        "examples/basic/test_builtins_vm.zirc",
        "examples/basic/error_demo.zirc",
        "examples/modules/main.zirc",
        "examples/others/factorial.zirc",
        "examples/others/print-test.zirc",
        "examples/others/scopes.zirc",
    ] {
        run_example_both_backends(&root.join(rel));
    }
}

/// The intentional-failure examples must fail (non-zero) on the VM
/// backend with an error on stderr.
#[test]
fn error_examples_fail() {
    let root = workspace_root();
    for rel in [
        "examples/errors/missing_end.zirc",
        "examples/errors/syntax_error.zirc",
        "examples/errors/type_error.zirc",
        "examples/errors/undefined_func.zirc",
        "examples/errors/undefined_var.zirc",
        "examples/basic/test_errors.zirc",
        "examples/basic/test_undefined.zirc",
    ] {
        let mut cmd = zirc_cmd();
        cmd.arg(root.join(rel));
        cmd.assert().failure();
    }
}

/// The shipped stdlib must work end-to-end via ZIRC_PATH on both
/// backends with identical output.
#[test]
fn stdlib_modules_agree_across_backends() {
    let root = workspace_root();
    let tmp_dir = tempfile::tempdir().unwrap();
    let main = tmp_dir.path().join("main.zirc");
    std::fs::write(
        &main,
        r#"
import math
import strings
import collections
showf("gcd=%d lcm=%d clamp=%d prime=%s", math.gcd(12, 18), math.lcm(4, 6), math.clamp(15, 0, 10), str(math.is_prime(7)))
showf("repeat=%s starts=%s ends=%s", strings.repeat("ab", 3), str(strings.starts_with("hello", "he")), str(strings.ends_with("hello", "lo")))
showf("sum=%d max=%d rev=%s has=%s", collections.sum([1, 2, 3]), collections.max_all([3, 1, 2]), str(collections.reverse([1, 2, 3])), str(collections.contains([1, 2], 2)))
"#,
    )
    .unwrap();

    // Edge cases: empty collections, zero bounds, impossible prefixes.
    std::fs::write(
        &main,
        std::fs::read_to_string(&main).unwrap()
            + r#"
showf("edge gcd(0,5)=%d sum0=%d rep0='%s' has0=%s starts_long=%s", math.gcd(0, 5), collections.sum([]), strings.repeat("ab", 0), str(collections.contains([], 1)), str(strings.starts_with("hi", "hello")))
"#,
    )
    .unwrap();

    let stdlib = root.join("stdlib");
    let run = |backend: Option<&str>| {
        let mut cmd = zirc_cmd();
        if let Some(b) = backend {
            cmd.arg("--backend").arg(b);
        }
        cmd.env("ZIRC_PATH", &stdlib).arg(&main);
        let out = cmd.assert().success().get_output().stdout.clone();
        String::from_utf8(out).unwrap()
    };
    let vm = run(None);
    let interp = run(Some("interp"));
    assert_eq!(vm, interp, "stdlib outputs differ across backends");
    assert!(vm.contains("gcd=6 lcm=12 clamp=10 prime=true"));
    assert!(vm.contains("repeat=ababab starts=true ends=true"));
    assert!(vm.contains("sum=6 max=3 rev=[3, 2, 1] has=true"));
    assert!(vm.contains("edge gcd(0,5)=5 sum0=0 rep0='' has0=false starts_long=false"));
}

#[test]
fn type_mismatch_is_an_error_in_both_backends() {
    let tmp_dir = tempfile::tempdir().unwrap();
    let path = tmp_dir.path().join("types.zirc");
    std::fs::write(&path, "let x: string = 123\n").unwrap();

    let mut vm_cmd = zirc_cmd();
    vm_cmd.arg(&path);
    vm_cmd
        .assert()
        .failure()
        .stderr(predicate::str::contains("Type mismatch"));

    let mut interp_cmd = zirc_cmd();
    interp_cmd.arg("--backend").arg("interp").arg(&path);
    interp_cmd
        .assert()
        .failure()
        .stderr(predicate::str::contains("Type mismatch"));

    // Correct annotation still runs on both.
    std::fs::write(&path, "let x: string = \"hi\"\nshow(x)\n").unwrap();
    let (vm, interp) = run_both_backends(&std::fs::read_to_string(&path).unwrap(), "types_ok.zirc");
    assert_eq!(vm, interp);
    assert!(vm.contains("hi"));
}

#[test]
fn division_by_zero_is_an_error_in_both_backends() {
    let tmp_dir = tempfile::tempdir().unwrap();
    let path = tmp_dir.path().join("div.zirc");
    std::fs::write(&path, "let x = 10 / 0\n").unwrap();

    let mut vm_cmd = zirc_cmd();
    vm_cmd.arg(&path);
    vm_cmd
        .assert()
        .failure()
        .stderr(predicate::str::contains("division by zero"));

    let mut interp_cmd = zirc_cmd();
    interp_cmd.arg("--backend").arg("interp").arg(&path);
    interp_cmd
        .assert()
        .failure()
        .stderr(predicate::str::contains("division by zero"));
}

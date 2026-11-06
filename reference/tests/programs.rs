//! Golden tests: each `tests/programs/NAME.egg` runs and its answer is compared
//! with `NAME.expected` (`error: <part of the message>` for expected failures).

use std::fs;
use std::path::Path;

#[test]
fn programs() {
    // Recursion in a debug build needs more than the test thread's 2 MB stack.
    std::thread::Builder::new().stack_size(1 << 28).spawn(run_all).unwrap().join().unwrap();
}

fn run_all() {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/programs");
    let mut paths: Vec<_> = fs::read_dir(&dir)
        .unwrap()
        .map(|e| e.unwrap().path())
        .filter(|p| p.extension().is_some_and(|e| e == "egg"))
        .collect();
    paths.sort();
    let mut failures = Vec::new();
    for path in &paths {
        let name = path.file_stem().unwrap().to_string_lossy();
        let source = fs::read_to_string(path).unwrap();
        let expected = fs::read_to_string(path.with_extension("expected")).unwrap();
        let expected = expected.trim_end_matches('\n');
        let actual = match eggnog_ref::parse_source(&source).and_then(|p| eggnog_ref::run_program(&p, "", &dir)) {
            Ok(v) => v.to_string(),
            Err(e) => format!("error: {}", e.message),
        };
        let ok = match expected.strip_prefix("error:") {
            Some(part) => actual.starts_with("error:") && actual.contains(part.trim()),
            None => actual == expected,
        };
        if !ok {
            failures.push(format!("  {name}:\n    expected: {expected}\n    actual:   {actual}"));
        }
    }
    assert!(failures.is_empty(), "{} of {} programs failed:\n{}", failures.len(), paths.len(), failures.join("\n"));
}

/// A named test runs the script's functions with the test's own `let`s in
/// place of the script's variables; only the variables the called functions
/// reach must be defined.
#[test]
fn named_test_supplies_script_variables() {
    let source = r#"
let grid = input.lines().to_list()
let other = 5

fun height() -> int:
  grid.len()

fun unused() -> int:
  other

test "supplies grid":
  let grid = ["a", "b"]
  assert height() == 2

test "missing grid":
  assert height() == 2
"#;
    let program = eggnog_ref::parse_source(source).unwrap();
    let results = eggnog_ref::run_tests(&program, Path::new("."));
    assert_eq!(results.len(), 2);
    assert!(results[0].outcome.is_ok(), "the test that defines `grid` should pass");
    match &results[1].outcome {
        Err(eggnog_ref::TestFailure::Error(e)) => {
            assert!(e.message.contains("`grid` is a script variable"), "unexpected message: {}", e.message)
        }
        _ => panic!("the test without `grid` should fail with the script-variable hint"),
    }
}

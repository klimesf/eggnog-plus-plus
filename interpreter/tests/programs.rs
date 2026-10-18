//! Golden tests: every `tests/programs/NAME.egg` is run and its answer is
//! compared with `tests/programs/NAME.expected`.
//!
//! - If `NAME.input` exists, it is passed as `input`. Otherwise `input` is "".
//! - If the program should fail, put `error: <part of the message>` in
//!   `NAME.expected`.
//!
//! Add a new program for every feature you implement.

use std::fs;
use std::path::Path;

#[test]
fn programs() {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/programs");
    let mut paths: Vec<_> = fs::read_dir(&dir)
        .unwrap()
        .map(|e| e.unwrap().path())
        .filter(|p| p.extension().is_some_and(|ext| ext == "egg"))
        .collect();
    paths.sort();

    let mut failures = Vec::new();
    for path in &paths {
        let name = path.file_stem().unwrap().to_string_lossy();
        let source = fs::read_to_string(path).unwrap();
        let input = fs::read_to_string(path.with_extension("input")).unwrap_or_default();
        let expected = fs::read_to_string(path.with_extension("expected"))
            .unwrap_or_else(|_| panic!("missing {name}.expected"));
        let expected = expected.trim();

        let outcome = std::panic::catch_unwind(|| eggnog::run_source(&source, &input));
        let actual = match outcome {
            Ok(Ok(value)) => value.to_string(),
            Ok(Err(e)) => format!("error: {}", e.message),
            Err(_) => "panicked (todo!() or a bug)".to_string(),
        };

        let ok = match expected.strip_prefix("error:") {
            Some(part) => actual.starts_with("error:") && actual.contains(part.trim()),
            None => actual == expected,
        };
        if !ok {
            failures.push(format!("  {name}: expected `{expected}`, got `{actual}`"));
        }
    }

    assert!(failures.is_empty(), "{} of {} programs failed:\n{}", failures.len(), paths.len(), failures.join("\n"));
}

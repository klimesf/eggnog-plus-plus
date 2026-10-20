//! Keeps the editor catalog (`editors/vscode/stdlib.json`) honest: every
//! method and function it lists must exist in the reference interpreter.

use std::fs;
use std::path::Path;

#[test]
fn catalog_names_are_implemented() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let catalog = fs::read_to_string(root.join("../editors/vscode/stdlib.json")).unwrap();
    let source = fs::read_to_string(root.join("src/builtins.rs")).unwrap() + &fs::read_to_string(root.join("src/interp.rs")).unwrap();

    let mut missing = Vec::new();
    for section in ["\"methods\"", "\"functions\""] {
        let start = catalog.find(section).unwrap();
        let body = &catalog[start..];
        let end = body.find(']').unwrap();
        for part in body[..end].split("\"name\": \"").skip(1) {
            let name = &part[..part.find('"').unwrap()];
            if !source.contains(&format!("\"{name}\"")) {
                missing.push(name.to_string());
            }
        }
    }
    assert!(missing.is_empty(), "listed in stdlib.json but not implemented: {missing:?}");
}

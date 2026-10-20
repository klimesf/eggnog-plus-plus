use std::io::{IsTerminal, Read};
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::{env, fs};

use eggnog_ref::{TestFailure, parse_source, run_program, run_tests};

const USAGE: &str = "\
usage:
  eggnog-ref run  <file.egg> [input.txt]      run a program and print its answer
  eggnog-ref run  <file.md>:<N> [input.txt]   run the N-th ```egg block of a Markdown file
  eggnog-ref test <file.egg | file.md>        run the `test` lines (every block of a .md)
  eggnog-ref blocks <file.md>                 list the ```egg blocks of a Markdown file

Input comes from the input file, else piped stdin, else it's empty.";

/// One program: a whole `.egg` file, or one ```egg block of a Markdown file.
struct Unit {
    label: String,
    code: String,
    /// Line number of the code's first line in the file.
    first_line: usize,
}

fn main() -> ExitCode {
    // Deep recursion (e.g. a DFS) needs more stack than the default 8 MB.
    let child = std::thread::Builder::new().stack_size(1 << 30).spawn(real_main).expect("failed to start interpreter thread");
    child.join().unwrap_or(ExitCode::FAILURE)
}

fn real_main() -> ExitCode {
    let args: Vec<String> = env::args().skip(1).collect();
    let (Some(command), Some(target)) = (args.first(), args.get(1)) else {
        eprintln!("{USAGE}");
        return ExitCode::from(2);
    };
    let (path, block) = split_target(target);
    let text = match fs::read_to_string(&path) {
        Ok(t) => t,
        Err(e) => {
            eprintln!("error: can't read {}: {e}", path.display());
            return ExitCode::FAILURE;
        }
    };
    let base_dir = path.parent().map(Path::to_path_buf).unwrap_or_default();
    let is_md = path.extension().is_some_and(|e| e == "md");
    let units = if is_md {
        markdown_blocks(&text)
    } else {
        vec![Unit { label: path.display().to_string(), code: text, first_line: 1 }]
    };
    let file_name = path.display().to_string();

    match command.as_str() {
        "blocks" => {
            for (i, u) in units.iter().enumerate() {
                println!("{:>3}  line {:<5} {}", i + 1, u.first_line, u.label);
            }
            ExitCode::SUCCESS
        }
        "run" => {
            let unit = match (is_md, block) {
                (false, _) => &units[0],
                (true, Some(n)) if n >= 1 && n <= units.len() => &units[n - 1],
                (true, _) if units.len() == 1 => &units[0],
                (true, _) => {
                    eprintln!("{file_name} has {} egg blocks; pick one with {file_name}:<N>:", units.len());
                    for (i, u) in units.iter().enumerate() {
                        eprintln!("{:>3}  {}", i + 1, u.label);
                    }
                    return ExitCode::from(2);
                }
            };
            let input = match read_input(args.get(2)) {
                Ok(s) => s,
                Err(e) => {
                    eprintln!("error: can't read input: {e}");
                    return ExitCode::FAILURE;
                }
            };
            let result = parse_source(&unit.code).and_then(|p| run_program(&p, &input, &base_dir));
            match result {
                Ok(answer) => {
                    println!("{answer}");
                    ExitCode::SUCCESS
                }
                Err(e) => {
                    eprint!("{}", e.render(&file_name, &unit.code, unit.first_line));
                    ExitCode::FAILURE
                }
            }
        }
        "test" => {
            let selected: Vec<&Unit> = match block {
                Some(n) if n >= 1 && n <= units.len() => vec![&units[n - 1]],
                _ => units.iter().collect(),
            };
            let (mut passed, mut failed) = (0, 0);
            for unit in selected {
                if is_md {
                    println!("── {} (line {})", unit.label, unit.first_line);
                }
                let program = match parse_source(&unit.code) {
                    Ok(p) => p,
                    Err(e) => {
                        failed += 1;
                        print!("{}", indent(&e.render(&file_name, &unit.code, unit.first_line)));
                        continue;
                    }
                };
                let results = run_tests(&program, &base_dir);
                if results.is_empty() {
                    println!("   (no tests)");
                }
                for r in results {
                    let line = unit.code[..r.span.start].matches('\n').count() + unit.first_line;
                    match r.outcome {
                        Ok(()) => {
                            passed += 1;
                            println!("   ok    line {line}");
                        }
                        Err(TestFailure::Wrong { expected, actual }) => {
                            failed += 1;
                            println!("   FAIL  line {line}: expected {}, got {}", expected.repr(), actual.repr());
                        }
                        Err(TestFailure::Error(e)) => {
                            failed += 1;
                            println!("   FAIL  line {line}:");
                            print!("{}", indent(&e.render(&file_name, &unit.code, unit.first_line)));
                        }
                    }
                }
            }
            println!("\n{passed} passed, {failed} failed");
            if failed == 0 { ExitCode::SUCCESS } else { ExitCode::FAILURE }
        }
        _ => {
            eprintln!("unknown command `{command}`\n{USAGE}");
            ExitCode::from(2)
        }
    }
}

fn indent(s: &str) -> String {
    s.lines().map(|l| format!("         {l}\n")).collect()
}

/// `examples/x.md:3` → (`examples/x.md`, Some(3)).
fn split_target(target: &str) -> (PathBuf, Option<usize>) {
    if let Some((p, n)) = target.rsplit_once(':') {
        if let Ok(n) = n.parse::<usize>() {
            return (PathBuf::from(p), Some(n));
        }
    }
    (PathBuf::from(target), None)
}

/// Extracts the ```egg blocks, labelled with the nearest heading and the
/// nearest "Part …" line above them.
fn markdown_blocks(text: &str) -> Vec<Unit> {
    let mut units = Vec::new();
    let mut heading = String::new();
    let mut part = String::new();
    let mut lines = text.lines().enumerate();
    while let Some((i, line)) = lines.next() {
        let t = line.trim();
        if t.starts_with('#') {
            heading = t.trim_start_matches('#').trim().to_string();
            part.clear();
        } else if t.starts_with("```egg") {
            let mut code = String::new();
            for (_, l) in lines.by_ref() {
                if l.trim_start().starts_with("```") {
                    break;
                }
                code.push_str(l);
                code.push('\n');
            }
            let label = [heading.as_str(), part.as_str()].iter().filter(|s| !s.is_empty()).cloned().collect::<Vec<_>>().join(" / ");
            units.push(Unit { label: if label.is_empty() { "(block)".into() } else { label }, code, first_line: i + 2 });
        } else if !t.is_empty() && !t.starts_with("```") {
            part = t.trim_end_matches(':').to_string();
            if part.len() > 40 {
                part = format!("{}…", &part[..part.floor_char_boundary(40)]);
            }
        }
    }
    units
}

fn read_input(path: Option<&String>) -> std::io::Result<String> {
    match path {
        Some(p) => fs::read_to_string(p),
        None if std::io::stdin().is_terminal() => Ok(String::new()),
        None => {
            let mut s = String::new();
            std::io::stdin().read_to_string(&mut s)?;
            Ok(s)
        }
    }
}

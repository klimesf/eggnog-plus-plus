use std::io::{IsTerminal, Read};
use std::process::ExitCode;
use std::{env, fs};

use eggnog::{eval, lexer, parser};

const USAGE: &str = "\
usage:
  eggnog run <file.egg> [input.txt]   run a program (input from the file, piped stdin, or empty)
  eggnog tokens <file.egg>            print the lexer's tokens
  eggnog ast <file.egg>               print the parser's AST";

fn main() -> ExitCode {
    let args: Vec<String> = env::args().skip(1).collect();
    let (Some(command), Some(path)) = (args.first(), args.get(1)) else {
        eprintln!("{USAGE}");
        return ExitCode::from(2);
    };

    let source = match fs::read_to_string(path) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("error: cannot read {path}: {e}");
            return ExitCode::FAILURE;
        }
    };

    let result = match command.as_str() {
        "tokens" => lexer::tokenize(&source).map(|tokens| {
            for t in tokens {
                println!("{:>4}..{:<4} {:?}", t.span.start, t.span.end, t.kind);
            }
        }),
        "ast" => lexer::tokenize(&source)
            .and_then(|tokens| parser::parse(&tokens))
            .map(|program| println!("{program:#?}")),
        "run" => {
            let input = match read_input(args.get(2)) {
                Ok(s) => s,
                Err(e) => {
                    eprintln!("error: cannot read input: {e}");
                    return ExitCode::FAILURE;
                }
            };
            lexer::tokenize(&source)
                .and_then(|tokens| parser::parse(&tokens))
                .and_then(|program| eval::run(&program, &input))
                .map(|answer| println!("{answer}"))
        }
        _ => {
            eprintln!("unknown command `{command}`\n{USAGE}");
            return ExitCode::from(2);
        }
    };

    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprint!("{}", e.render(path, &source));
            ExitCode::FAILURE
        }
    }
}

fn read_input(path: Option<&String>) -> std::io::Result<String> {
    match path {
        Some(p) => fs::read_to_string(p),
        // Nothing piped in: run with empty input instead of waiting for the keyboard.
        None if std::io::stdin().is_terminal() => Ok(String::new()),
        None => {
            let mut s = String::new();
            std::io::stdin().read_to_string(&mut s)?;
            Ok(s)
        }
    }
}

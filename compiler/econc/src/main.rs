use std::env;
use std::fs;
use std::process;

use parser::Parser;

fn main() {
    let args: Vec<String> = env::args().collect();

    if args.len() != 3 || args[1] != "check" {
        eprintln!("Usage: econc check <file.econ>");
        process::exit(1);
    }

    let path = &args[2];

    let source = match fs::read_to_string(path) {
        Ok(source) => source,
        Err(error) => {
            eprintln!("error: cannot read '{}': {}", path, error);
            process::exit(1);
        }
    };

    let mut parser = match Parser::from_source(&source) {
        Ok(parser) => parser,
        Err(error) => {
            eprintln!(
                "lexer error at {}..{}: {}",
                error.span.start, error.span.end, error.message
            );
            process::exit(1);
        }
    };

    match parser.parse_program() {
        Ok(program) => {
            println!("EconLang check: OK");
            println!(
                "Parsed {} top-level declaration(s).",
                program.declarations.len()
            );
        }

        Err(error) => {
            eprintln!(
                "parser error at {}..{}: {}",
                error.span.start, error.span.end, error.message
            );
            process::exit(1);
        }
    }
}

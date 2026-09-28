mod token;
mod lexer;
mod ast;
mod parser;
mod errors;
mod module;
mod typechecker;
mod bytecode;
mod compiler;
mod vm;
mod repl;

use std::env;
use std::path::Path;

fn main() {
    let args: Vec<String> = env::args().collect();

    let path_str = match args.get(1) {
        Some(p) => p.as_str(),
        None => { repl::run(); return; }
    };
    let path = Path::new(path_str);

    let resolved = match module::resolve_program(path) {
        Ok(r) => r,
        Err(e) => {
            eprintln!("{}", errors::format_error(&e, &std::collections::HashMap::new()));
            std::process::exit(1);
        }
    };
    let sources = resolved.sources;
    let program = resolved.program;

    if let Err(errs) = typechecker::check_program(&program) {
        eprintln!("{}", errors::format_errors(&errs, &sources));
        std::process::exit(1);
    }

    let compiled = match compiler::compile_program(&program) {
        Ok(c) => c,
        Err(e) => { eprintln!("{}", errors::format_error(&e, &sources)); std::process::exit(1); }
    };

    let mut machine = vm::VM::new(compiled.functions);
    if let Err(e) = machine.run_main(&compiled.main) {
        eprintln!("{}", errors::format_error(&e, &sources));
        std::process::exit(1);
    }
}

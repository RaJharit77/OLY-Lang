//! REPL interactif OLY. Chaque saisie est réellement lexée, analysée,
//! vérifiée puis compilée en bytecode et exécutée par la VM — ce n'est pas
//! un interpréteur séparé, seulement le même pipeline que le mode fichier,
//! rejoué à chaque ligne, avec un état persistant (variables globales et
//! fonctions déjà définies restent disponibles d'une saisie à l'autre).

use crate::ast::{Program, StmtKind};
use crate::compiler;
use crate::errors;
use crate::lexer::Lexer;
use crate::parser::Parser;
use crate::typechecker::Checker;
use crate::vm::VM;
use std::collections::HashMap;
use std::io::{self, Write};

const REPL_FILE: &str = "<repl>";

pub fn run() {
    println!("OLY REPL — teny malagasy, tena compilé (lexer -> parser -> bytecode -> VM).");
    println!("Ampiasao ':quit' na ':q' hiala, ':aide' raha mila fanampiana.\n");

    let mut vm = VM::new(HashMap::new());
    let mut checker = Checker::new();
    let sources: HashMap<String, String> = HashMap::new();
    let mut buffer = String::new();

    loop {
        let prompt = if buffer.is_empty() { "oly> " } else { "...> " };
        print!("{}", prompt);
        if io::stdout().flush().is_err() { break; }

        let mut line = String::new();
        match io::stdin().read_line(&mut line) {
            Ok(0) => { println!(); break; } // Ctrl-D / fin d'entrée
            Ok(_) => {}
            Err(_) => break,
        }

        if buffer.is_empty() {
            match line.trim() {
                ":quit" | ":q" => break,
                ":aide" | ":help" => { print_help(); continue; }
                "" => continue,
                _ => {}
            }
        }

        buffer.push_str(&line);

        let program = match try_parse(&buffer) {
            ParseOutcome::Ok(p) => p,
            ParseOutcome::Incomplete => continue, // bloc pas encore fermé : on lit la suite
            ParseOutcome::Err(e) => {
                eprintln!("{}", errors::format_error(&e, &sources));
                buffer.clear();
                continue;
            }
        };

        buffer.clear();
        run_program(program, &mut vm, &mut checker, &sources);
    }
}

enum ParseOutcome {
    Ok(Program),
    Incomplete,
    Err(crate::errors::OlyError),
}

/// Tente de parser `buffer` tel quel ; si ça échoue juste parce qu'il manque
/// le ';' final (cas courant d'une expression tapée seule, ex: "2 + 2"),
/// réessaie avec un ';' ajouté avant de conclure à une entrée incomplète.
fn try_parse(buffer: &str) -> ParseOutcome {
    match parse_once(buffer) {
        Ok(p) => return ParseOutcome::Ok(p),
        Err(e) if looks_incomplete(&e.message) => {
            let with_semi = format!("{};\n", buffer.trim_end());
            if let Ok(p) = parse_once(&with_semi) {
                return ParseOutcome::Ok(p);
            }
            ParseOutcome::Incomplete
        }
        Err(e) => ParseOutcome::Err(e),
    }
}

fn parse_once(source: &str) -> Result<Program, crate::errors::OlyError> {
    let mut lexer = Lexer::new(source, REPL_FILE.to_string());
    let tokens = lexer.tokenize()?;
    let mut parser = Parser::new(tokens, REPL_FILE.to_string());
    parser.parse_program()
}

/// Heuristique : si le parser s'est arrêté parce qu'il a atteint la fin de
/// l'entrée en attendant encore un token (accolade fermante, etc.), l'erreur
/// mentionne le token 'Eof'. Dans ce cas on redemande une ligne de plus au
/// lieu de signaler une vraie erreur de syntaxe.
fn looks_incomplete(message: &str) -> bool {
    message.contains("Eof")
}

fn run_program(mut program: Program, vm: &mut VM, checker: &mut Checker, sources: &HashMap<String, String>) {
    // Une simple expression tapée seule s'affiche automatiquement, comme dans
    // n'importe quel REPL — équivalent à l'envelopper dans 'asehoy(...)'.
    if program.items.len() == 1 {
        if let StmtKind::ExprStmt(expr) = program.items[0].kind.clone() {
            program.items[0].kind = StmtKind::Print(expr);
        }
    }

    if let Err(errs) = checker.check(&program) {
        eprintln!("{}", errors::format_errors(&errs, sources));
        return;
    }

    let compiled = match compiler::compile_program(&program) {
        Ok(c) => c,
        Err(e) => { eprintln!("{}", errors::format_error(&e, sources)); return; }
    };

    // Les fonctions/méthodes nouvellement définies s'ajoutent à celles déjà
    // connues de la VM ; les variables globales vivent déjà dans vm.globals.
    vm.functions.extend(compiled.functions);
    if let Err(e) = vm.run_main(&compiled.main) {
        eprintln!("{}", errors::format_error(&e, sources));
    }
}

fn print_help() {
    println!("Tapez une instruction OLY (aoka, raha, asa, endrika, isaky, ...) ou une expression.");
    println!("Une expression seule (ex: 2 + 2) affiche automatiquement son résultat.");
    println!("Les variables et fonctions définies restent disponibles d'une ligne à l'autre.");
    println!("':quit' / ':q' pour quitter, ':aide' pour revoir ce message.");
}

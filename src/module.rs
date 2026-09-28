//! Résolution du système de modules OLY.
//! `ampiasao "nom";` charge "nom.oly" (relatif au fichier qui importe),
//! et ses déclarations/instructions sont insérées à la place de l'import,
//! dans l'ordre où elles apparaissent. Les fichiers déjà importés (même
//! transitivement) ne sont chargés qu'une seule fois, ce qui protège aussi
//! contre les imports circulaires.

use crate::ast::{Program, Stmt, StmtKind};
use crate::errors::OlyError;
use crate::lexer::Lexer;
use crate::parser::Parser;
use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};

/// Résultat de la résolution : le programme fusionné, plus le texte source
/// de chaque fichier impliqué (utilisé pour afficher le contexte des erreurs).
pub struct Resolved {
    pub program: Program,
    pub sources: HashMap<String, String>,
}

pub fn resolve_program(entry: &Path) -> Result<Resolved, OlyError> {
    let mut visited = HashSet::new();
    let mut items = Vec::new();
    let mut sources = HashMap::new();
    load_file(entry, &mut visited, &mut items, &mut sources)?;
    Ok(Resolved { program: Program { items }, sources })
}

fn io_error(path: &Path, e: std::io::Error) -> OlyError {
    OlyError::new(path.display().to_string(), 1, format!("Impossible de lire '{}' : {}", path.display(), e))
}

fn load_file(
    path: &Path,
    visited: &mut HashSet<PathBuf>,
    out: &mut Vec<Stmt>,
    sources: &mut HashMap<String, String>,
) -> Result<(), OlyError> {
    let canon = path.canonicalize().map_err(|e| io_error(path, e))?;
    if visited.contains(&canon) {
        // Déjà importé (directement ou via un autre module) : on l'ignore silencieusement.
        return Ok(());
    }
    visited.insert(canon);

    let file_key = path.display().to_string();
    let source = fs::read_to_string(path).map_err(|e| io_error(path, e))?;

    let mut lexer = Lexer::new(&source, file_key.clone());
    let tokens = lexer.tokenize()?;
    sources.insert(file_key.clone(), source);

    let mut parser = Parser::new(tokens, file_key);
    let program = parser.parse_program()?;

    let dir = path.parent().unwrap_or_else(|| Path::new("."));
    for item in program.items {
        match &item.kind {
            StmtKind::Import(name) => {
                let import_path = resolve_import_path(dir, name);
                load_file(&import_path, visited, out, sources)?;
            }
            _ => out.push(item),
        }
    }
    Ok(())
}

fn resolve_import_path(dir: &Path, name: &str) -> PathBuf {
    let mut p = dir.join(name);
    if p.extension().is_none() {
        p.set_extension("oly");
    }
    p
}

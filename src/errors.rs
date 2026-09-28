//! Erreurs OLY avec contexte source : chaque erreur pointe vers un fichier
//! et une ligne précise, et l'affichage inclut un extrait du code source
//! avec un curseur, à la manière d'un compilateur classique (rustc, gcc...).

use std::collections::HashMap;

#[derive(Debug, Clone)]
pub struct OlyError {
    pub file: String,
    pub line: usize,
    /// Colonne optionnelle (1-indexée). Quand connue (erreurs lexicales/syntaxiques),
    /// un curseur '^' est affiché sous la ligne source à cet endroit précis.
    pub col: Option<usize>,
    pub message: String,
}

impl OlyError {
    pub fn new(file: impl Into<String>, line: usize, message: impl Into<String>) -> Self {
        OlyError { file: file.into(), line, col: None, message: message.into() }
    }
    pub fn with_col(file: impl Into<String>, line: usize, col: usize, message: impl Into<String>) -> Self {
        OlyError { file: file.into(), line, col: Some(col), message: message.into() }
    }
}

/// Formate une erreur avec son extrait de source, façon compilateur :
///
/// erreur: message ici
///   --> chemin/fichier.oly:12
///    |
/// 12 |     raha x > 0 {
///    |          ^
pub fn format_error(err: &OlyError, sources: &HashMap<String, String>) -> String {
    let mut out = String::new();
    out.push_str(&format!("erreur: {}\n", err.message));
    out.push_str(&format!("  --> {}:{}\n", err.file, err.line));

    if let Some(src) = sources.get(&err.file) {
        if let Some(text) = src.lines().nth(err.line.saturating_sub(1)) {
            let gutter = err.line.to_string();
            let pad = " ".repeat(gutter.len());
            out.push_str(&format!("{} |\n", pad));
            out.push_str(&format!("{} | {}\n", gutter, text));
            if let Some(col) = err.col {
                let caret_pad = " ".repeat(col.saturating_sub(1));
                out.push_str(&format!("{} | {}^\n", pad, caret_pad));
            } else {
                out.push_str(&format!("{} |\n", pad));
            }
        }
    }
    out
}

pub fn format_errors(errs: &[OlyError], sources: &HashMap<String, String>) -> String {
    errs.iter().map(|e| format_error(e, sources)).collect::<Vec<_>>().join("\n")
}

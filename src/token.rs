//! Définition des tokens du langage OLY.
//! Tous les mots-clés sont en malgache. Cette table est le point unique
//! à modifier si on veut changer le vocabulaire du langage.

use std::fmt;

#[derive(Debug, Clone, PartialEq)]
pub enum TokenKind {
    // --- Littéraux ---
    Isa(i64),          // entier ("isa" = nombre)
    Ampahany(f64),     // flottant ("ampahany" = fraction)
    Teny(String),      // chaîne de caractères ("teny" = mot/texte)
    Anarana(String),   // identifiant ("anarana" = nom)

    // --- Mots-clés ---
    Aoka,          // aoka      -> déclaration de variable (let)
    TsyMiova,      // tsymiova  -> constante
    Raha,          // raha      -> if
    RahaTsyIzany,  // raha tsy izany -> else
    NaRaha,        // na raha   -> else if
    RahaMbola,     // raha mbola -> while
    Isaky,         // isaky     -> for / for-each
    Amin,          // amin      -> "in" (pour isaky ELEM amin LISTE)
    Asa,           // asa       -> function (nommée ou anonyme/lambda)
    Avereno,       // avereno   -> return
    Endrika,       // endrika   -> struct
    Fomba,         // fomba     -> méthode (impl block)
    Ampiasao,      // ampiasao  -> import/module
    Marina,        // marina    -> true
    Diso,          // diso      -> false
    Asehoy,        // asehoy    -> print
    Tapaka,        // tapaka    -> break
    Manohy,        // manohy    -> continue
    Tsisy,         // tsisy     -> null/none
    Andramo,       // andramo   -> try
    Sambotra,      // sambotra  -> catch
    Atsipazo,      // atsipazo  -> throw/raise

    // --- Opérateurs ---
    Plus, Minus, Star, Slash, Percent,
    Eq,      // =
    PlusEq, MinusEq, StarEq, SlashEq, PercentEq, // += -= *= /= %=
    PlusPlus, MinusMinus,                          // ++ --
    EqEq,    // ==
    NotEq,   // !=
    Lt, Gt, LtEq, GtEq,
    Sy,      // sy  -> and (court-circuit)
    Na,      // na  -> or  (court-circuit)
    Tsy,     // tsy -> not
    Question, // ?  (opérateur ternaire cond ? a : b)

    // --- Symboles ---
    LParen, RParen,
    LBrace, RBrace,
    LBracket, RBracket,
    Comma, Semicolon, Colon, Dot, Arrow, // -> pour les types de retour

    Eof,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Token {
    pub kind: TokenKind,
    pub line: usize,
    pub col: usize,
}

impl fmt::Display for Token {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:?} (ligne {}, col {})", self.kind, self.line, self.col)
    }
}

/// Table centrale des mots-clés malgaches -> TokenKind.
/// C'est ICI qu'on change le vocabulaire si besoin.
pub fn lookup_keyword(word: &str) -> Option<TokenKind> {
    match word {
        "aoka" => Some(TokenKind::Aoka),
        "tsymiova" => Some(TokenKind::TsyMiova),
        "raha" => Some(TokenKind::Raha),
        "mbola" => None, // géré via lexer (combiné avec "raha")
        "isaky" => Some(TokenKind::Isaky),
        "amin" => Some(TokenKind::Amin),
        "asa" => Some(TokenKind::Asa),
        "avereno" => Some(TokenKind::Avereno),
        "endrika" => Some(TokenKind::Endrika),
        "fomba" => Some(TokenKind::Fomba),
        "ampiasao" => Some(TokenKind::Ampiasao),
        "marina" => Some(TokenKind::Marina),
        "diso" => Some(TokenKind::Diso),
        "asehoy" => Some(TokenKind::Asehoy),
        "tapaka" => Some(TokenKind::Tapaka),
        "manohy" => Some(TokenKind::Manohy),
        "tsisy" => Some(TokenKind::Tsisy),
        "andramo" => Some(TokenKind::Andramo),
        "sambotra" => Some(TokenKind::Sambotra),
        "atsipazo" => Some(TokenKind::Atsipazo),
        "sy" => Some(TokenKind::Sy),
        "na" => Some(TokenKind::Na),
        "tsy" => Some(TokenKind::Tsy),
        _ => None,
    }
}

//! Lexer OLY : transforme le texte source en une liste de tokens.
//! Gère aussi les mots-clés composés de plusieurs mots
//! ("raha tsy izany" -> sinon, "na raha" -> sinon si, "raha mbola" -> tant que).

use crate::errors::OlyError;
use crate::token::{lookup_keyword, Token, TokenKind};

pub struct Lexer<'a> {
    src: &'a [u8],
    file: String,
    pos: usize,
    line: usize,
    col: usize,
}

impl<'a> Lexer<'a> {
    pub fn new(src: &'a str, file: impl Into<String>) -> Self {
        Lexer { src: src.as_bytes(), file: file.into(), pos: 0, line: 1, col: 1 }
    }

    fn peek(&self) -> u8 {
        if self.pos < self.src.len() { self.src[self.pos] } else { 0 }
    }

    fn peek_next(&self) -> u8 {
        if self.pos + 1 < self.src.len() { self.src[self.pos + 1] } else { 0 }
    }

    fn advance(&mut self) -> u8 {
        let c = self.peek();
        self.pos += 1;
        if c == b'\n' { self.line += 1; self.col = 1; } else { self.col += 1; }
        c
    }

    fn skip_whitespace_and_comments(&mut self) {
        loop {
            match self.peek() {
                b' ' | b'\t' | b'\r' | b'\n' => { self.advance(); }
                b'/' if self.peek_next() == b'/' => {
                    while self.peek() != b'\n' && self.peek() != 0 { self.advance(); }
                }
                b'/' if self.peek_next() == b'*' => {
                    self.advance(); self.advance(); // consomme "/*"
                    while !(self.peek() == b'*' && self.peek_next() == b'/') && self.peek() != 0 {
                        self.advance();
                    }
                    if self.peek() != 0 { self.advance(); self.advance(); } // consomme "*/"
                }
                _ => break,
            }
        }
    }

    fn make(&self, kind: TokenKind, line: usize, col: usize) -> Token {
        Token { kind, line, col }
    }

    /// Tokenise tout le fichier source (sans fusion des mots-clés composés).
    fn raw_tokens(&mut self) -> Result<Vec<Token>, OlyError> {
        let mut tokens = Vec::new();
        loop {
            self.skip_whitespace_and_comments();
            let (line, col) = (self.line, self.col);
            let c = self.peek();
            if c == 0 {
                tokens.push(self.make(TokenKind::Eof, line, col));
                break;
            }

            if c.is_ascii_digit() {
                tokens.push(self.lex_number(line, col));
                continue;
            }
            if c == b'"' {
                tokens.push(self.lex_string(line, col));
                continue;
            }
            if c.is_ascii_alphabetic() || c == b'_' {
                tokens.push(self.lex_word(line, col));
                continue;
            }

            self.advance();
            let kind = match c {
                b'+' if self.peek() == b'+' => { self.advance(); TokenKind::PlusPlus }
                b'+' if self.peek() == b'=' => { self.advance(); TokenKind::PlusEq }
                b'+' => TokenKind::Plus,
                b'-' if self.peek() == b'>' => { self.advance(); TokenKind::Arrow }
                b'-' if self.peek() == b'-' => { self.advance(); TokenKind::MinusMinus }
                b'-' if self.peek() == b'=' => { self.advance(); TokenKind::MinusEq }
                b'-' => TokenKind::Minus,
                b'*' if self.peek() == b'=' => { self.advance(); TokenKind::StarEq }
                b'*' => TokenKind::Star,
                b'/' if self.peek() == b'=' => { self.advance(); TokenKind::SlashEq }
                b'/' => TokenKind::Slash,
                b'%' if self.peek() == b'=' => { self.advance(); TokenKind::PercentEq }
                b'%' => TokenKind::Percent,
                b'=' if self.peek() == b'=' => { self.advance(); TokenKind::EqEq }
                b'=' => TokenKind::Eq,
                b'!' if self.peek() == b'=' => { self.advance(); TokenKind::NotEq }
                b'<' if self.peek() == b'=' => { self.advance(); TokenKind::LtEq }
                b'<' => TokenKind::Lt,
                b'>' if self.peek() == b'=' => { self.advance(); TokenKind::GtEq }
                b'>' => TokenKind::Gt,
                b'?' => TokenKind::Question,
                b'(' => TokenKind::LParen,
                b')' => TokenKind::RParen,
                b'{' => TokenKind::LBrace,
                b'}' => TokenKind::RBrace,
                b'[' => TokenKind::LBracket,
                b']' => TokenKind::RBracket,
                b',' => TokenKind::Comma,
                b';' => TokenKind::Semicolon,
                b':' => TokenKind::Colon,
                b'.' => TokenKind::Dot,
                other => {
                    return Err(OlyError::with_col(
                        self.file.clone(), line, col,
                        format!("Caractère inattendu '{}'", other as char),
                    ));
                }
            };
            tokens.push(self.make(kind, line, col));
        }
        Ok(tokens)
    }

    fn lex_number(&mut self, line: usize, col: usize) -> Token {
        let start = self.pos;
        while self.peek().is_ascii_digit() { self.advance(); }
        let mut is_float = false;
        if self.peek() == b'.' && self.peek_next().is_ascii_digit() {
            is_float = true;
            self.advance();
            while self.peek().is_ascii_digit() { self.advance(); }
        }
        let text = std::str::from_utf8(&self.src[start..self.pos]).unwrap();
        if is_float {
            self.make(TokenKind::Ampahany(text.parse().unwrap()), line, col)
        } else {
            self.make(TokenKind::Isa(text.parse().unwrap()), line, col)
        }
    }

    fn lex_string(&mut self, line: usize, col: usize) -> Token {
        self.advance(); // consomme le guillemet ouvrant
        let start = self.pos;
        while self.peek() != b'"' && self.peek() != 0 { self.advance(); }
        let text = std::str::from_utf8(&self.src[start..self.pos]).unwrap().to_string();
        self.advance(); // consomme le guillemet fermant
        self.make(TokenKind::Teny(text), line, col)
    }

    fn lex_word(&mut self, line: usize, col: usize) -> Token {
        let start = self.pos;
        while self.peek().is_ascii_alphanumeric() || self.peek() == b'_' {
            self.advance();
        }
        let text = std::str::from_utf8(&self.src[start..self.pos]).unwrap();
        match lookup_keyword(text) {
            Some(kind) => self.make(kind, line, col),
            None => self.make(TokenKind::Anarana(text.to_string()), line, col),
        }
    }

    /// Fusionne les mots-clés composés de plusieurs mots :
    /// "raha" + "tsy" + "izany"(identifiant) -> sinon
    /// "na" + "raha"                          -> sinon si
    /// "raha" + "mbola"(identifiant)          -> tant que
    fn merge_compound_keywords(tokens: Vec<Token>) -> Vec<Token> {
        let mut out: Vec<Token> = Vec::with_capacity(tokens.len());
        let mut i = 0;
        while i < tokens.len() {
            let t = &tokens[i];
            // raha tsy izany -> sinon
            if t.kind == TokenKind::Raha
                && tokens.get(i + 1).map(|x| &x.kind) == Some(&TokenKind::Tsy)
                && matches!(tokens.get(i + 2).map(|x| &x.kind), Some(TokenKind::Anarana(w)) if w == "izany")
            {
                out.push(Token { kind: TokenKind::RahaTsyIzany, line: t.line, col: t.col });
                i += 3;
                continue;
            }
            // raha mbola -> tant que
            if t.kind == TokenKind::Raha
                && matches!(tokens.get(i + 1).map(|x| &x.kind), Some(TokenKind::Anarana(w)) if w == "mbola")
            {
                out.push(Token { kind: TokenKind::RahaMbola, line: t.line, col: t.col });
                i += 2;
                continue;
            }
            // na raha -> sinon si
            if t.kind == TokenKind::Na
                && tokens.get(i + 1).map(|x| &x.kind) == Some(&TokenKind::Raha)
            {
                out.push(Token { kind: TokenKind::NaRaha, line: t.line, col: t.col });
                i += 2;
                continue;
            }
            out.push(tokens[i].clone());
            i += 1;
        }
        out
    }

    pub fn tokenize(&mut self) -> Result<Vec<Token>, OlyError> {
        let raw = self.raw_tokens()?;
        Ok(Self::merge_compound_keywords(raw))
    }
}

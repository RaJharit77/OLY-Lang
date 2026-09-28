//! Parser (descente récursive + précédence d'opérateurs) pour OLY.
//! Transforme le flux de tokens produit par le lexer en un AST (voir ast.rs).

use crate::ast::*;
use crate::errors::OlyError;
use crate::token::{Token, TokenKind};

pub struct Parser {
    tokens: Vec<Token>,
    file: String,
    pos: usize,
    /// Vrai quand on est en train de parser une condition (raha / raha mbola / isaky),
    /// où "identifiant {" doit être compris comme le début d'un bloc, pas d'une
    /// instanciation de struct. Redevient faux dans les contextes non ambigus
    /// (parenthèses, arguments d'appel, valeurs de champs de struct).
    no_struct_lit: bool,
}

type PResult<T> = Result<T, OlyError>;

impl Parser {
    pub fn new(tokens: Vec<Token>, file: impl Into<String>) -> Self {
        Parser { tokens, file: file.into(), pos: 0, no_struct_lit: false }
    }

    /// Exécute `f` avec `no_struct_lit` temporairement mis à `value`, puis restaure l'ancienne valeur.
    fn with_struct_lit(&mut self, value: bool, f: impl FnOnce(&mut Self) -> PResult<Expr>) -> PResult<Expr> {
        let old = self.no_struct_lit;
        self.no_struct_lit = value;
        let result = f(self);
        self.no_struct_lit = old;
        result
    }

    // ---------- Utilitaires ----------

    fn peek(&self) -> &TokenKind {
        &self.tokens[self.pos].kind
    }

    fn peek_at(&self, offset: usize) -> &TokenKind {
        let i = (self.pos + offset).min(self.tokens.len() - 1);
        &self.tokens[i].kind
    }

    fn current_line(&self) -> usize {
        self.tokens[self.pos].line
    }

    fn current_col(&self) -> usize {
        self.tokens[self.pos].col
    }

    fn err(&self, message: impl Into<String>) -> OlyError {
        OlyError::with_col(self.file.clone(), self.current_line(), self.current_col(), message)
    }

    fn is_at_end(&self) -> bool {
        matches!(self.peek(), TokenKind::Eof)
    }

    fn advance(&mut self) -> TokenKind {
        let k = self.tokens[self.pos].kind.clone();
        if !self.is_at_end() { self.pos += 1; }
        k
    }

    fn check(&self, kind: &TokenKind) -> bool {
        std::mem::discriminant(self.peek()) == std::mem::discriminant(kind)
    }

    fn match_kind(&mut self, kind: &TokenKind) -> bool {
        if self.check(kind) { self.advance(); true } else { false }
    }

    fn expect(&mut self, kind: &TokenKind, ctx: &str) -> PResult<TokenKind> {
        if self.check(kind) {
            Ok(self.advance())
        } else {
            Err(self.err(format!("Attendu {:?} {} mais trouvé {:?}", kind, ctx, self.peek())))
        }
    }

    fn expect_ident(&mut self, ctx: &str) -> PResult<String> {
        match self.peek().clone() {
            TokenKind::Anarana(name) => { self.advance(); Ok(name) }
            other => Err(self.err(format!("Attendu un identifiant {} mais trouvé {:?}", ctx, other))),
        }
    }

    // ---------- Programme ----------

    pub fn parse_program(&mut self) -> PResult<Program> {
        let mut items = Vec::new();
        while !self.is_at_end() {
            items.push(self.parse_stmt()?);
        }
        Ok(Program { items })
    }

    fn parse_block(&mut self) -> PResult<Vec<Stmt>> {
        self.expect(&TokenKind::LBrace, "pour ouvrir un bloc")?;
        let mut stmts = Vec::new();
        while !self.check(&TokenKind::RBrace) && !self.is_at_end() {
            stmts.push(self.parse_stmt()?);
        }
        self.expect(&TokenKind::RBrace, "pour fermer un bloc")?;
        Ok(stmts)
    }

    fn parse_type(&mut self) -> PResult<Type> {
        let mut ty = match self.advance() {
            // 'marina' est aussi le littéral "vrai" : on l'accepte comme nom de type booléen.
            TokenKind::Marina => Type::Marina,
            TokenKind::Anarana(name) => match name.as_str() {
                "isa" => Type::Isa,
                "ampahany" => Type::Ampahany,
                "teny" => Type::Teny,
                "marina" => Type::Marina,
                other => Type::Endrika(other.to_string()),
            },
            other => return Err(self.err(format!("Type attendu, trouvé {:?}", other))),
        };
        // Suffixe "[]" (répétable) pour les tableaux : isa[], teny[][], ...
        while self.check(&TokenKind::LBracket) && matches!(self.peek_at(1), TokenKind::RBracket) {
            self.advance(); // [
            self.advance(); // ]
            ty = Type::Lisitra(Box::new(ty));
        }
        Ok(ty)
    }

    // ---------- Déclarations / instructions ----------

    fn parse_stmt(&mut self) -> PResult<Stmt> {
        let line = self.current_line();
        let kind = match self.peek().clone() {
            TokenKind::Aoka => self.parse_var_decl(false)?,
            TokenKind::TsyMiova => self.parse_var_decl(true)?,
            TokenKind::Raha => self.parse_if()?,
            TokenKind::RahaMbola => self.parse_while()?,
            TokenKind::Isaky => self.parse_isaky()?,
            TokenKind::Asa if matches!(self.peek_at(1), TokenKind::Anarana(_)) => {
                StmtKind::FunctionDecl(self.parse_function_decl()?)
            }
            TokenKind::Endrika => self.parse_struct_decl()?,
            TokenKind::Fomba => self.parse_impl_decl()?,
            TokenKind::Ampiasao => self.parse_import()?,
            TokenKind::Asehoy => self.parse_print()?,
            TokenKind::Avereno => self.parse_return()?,
            TokenKind::Andramo => self.parse_try()?,
            TokenKind::Atsipazo => self.parse_throw()?,
            TokenKind::Tapaka => { self.advance(); self.expect(&TokenKind::Semicolon, "après 'tapaka'")?; StmtKind::Break }
            TokenKind::Manohy => { self.advance(); self.expect(&TokenKind::Semicolon, "après 'manohy'")?; StmtKind::Continue }
            _ => self.parse_expr_stmt()?,
        };
        Ok(Stmt { kind, file: self.file.clone(), line })
    }

    /// Instruction-expression, avec sucre syntaxique pour ++ / -- en suffixe :
    /// "compteur++;" est équivalent à "compteur = compteur + 1;".
    fn parse_expr_stmt(&mut self) -> PResult<StmtKind> {
        let expr = self.parse_expression()?;
        if self.check(&TokenKind::PlusPlus) || self.check(&TokenKind::MinusMinus) {
            let is_inc = matches!(self.peek(), TokenKind::PlusPlus);
            self.advance();
            self.expect(&TokenKind::Semicolon, "après '++'/'--'")?;
            let op = if is_inc { BinOp::Add } else { BinOp::Sub };
            let incremented = Expr::Binary(Box::new(expr.clone()), op, Box::new(Expr::IntLit(1)));
            return Ok(StmtKind::ExprStmt(Expr::Assign(Box::new(expr), Box::new(incremented))));
        }
        self.expect(&TokenKind::Semicolon, "après une expression")?;
        Ok(StmtKind::ExprStmt(expr))
    }

    /// aoka/tsymiova IDENT [":" type] ["=" expr] ";"
    fn parse_var_decl(&mut self, mutable: bool) -> PResult<StmtKind> {
        self.advance(); // aoka | tsymiova
        let name = self.expect_ident("après 'aoka'/'tsymiova'")?;
        let declared_type = if self.match_kind(&TokenKind::Colon) {
            Some(self.parse_type()?)
        } else {
            None
        };
        let value = if self.match_kind(&TokenKind::Eq) {
            self.parse_expression()?
        } else {
            Expr::IntLit(0) // valeur par défaut tant qu'il n'y a pas d'initialisation explicite
        };
        self.expect(&TokenKind::Semicolon, "après une déclaration de variable")?;
        Ok(StmtKind::VarDecl { name, mutable, declared_type, value })
    }

    /// raha expr bloc (na raha expr bloc)* (raha tsy izany bloc)?
    fn parse_if(&mut self) -> PResult<StmtKind> {
        self.advance(); // raha
        let cond = self.with_struct_lit(true, |p| p.parse_expression())?;
        let then_branch = self.parse_block()?;
        let mut else_ifs = Vec::new();
        let mut else_branch = None;
        loop {
            if self.check(&TokenKind::NaRaha) {
                self.advance();
                let c = self.with_struct_lit(true, |p| p.parse_expression())?;
                let b = self.parse_block()?;
                else_ifs.push((c, b));
            } else if self.check(&TokenKind::RahaTsyIzany) {
                self.advance();
                else_branch = Some(self.parse_block()?);
                break;
            } else {
                break;
            }
        }
        Ok(StmtKind::If { cond, then_branch, else_ifs, else_branch })
    }

    /// raha mbola expr bloc
    fn parse_while(&mut self) -> PResult<StmtKind> {
        self.advance(); // raha mbola
        let cond = self.with_struct_lit(true, |p| p.parse_expression())?;
        let body = self.parse_block()?;
        Ok(StmtKind::While { cond, body })
    }

    /// isaky (...) { } [style C]   OU   isaky ELEM amin ITERABLE { } [style for-each]
    fn parse_isaky(&mut self) -> PResult<StmtKind> {
        self.advance(); // isaky
        if self.check(&TokenKind::LParen) {
            self.parse_for_classic()
        } else {
            let var = self.expect_ident("après 'isaky' (nom de la variable de boucle)")?;
            self.expect(&TokenKind::Amin, "après le nom de variable ('isaky elem amin liste { ... }')")?;
            let iterable = self.with_struct_lit(true, |p| p.parse_expression())?;
            let body = self.parse_block()?;
            Ok(StmtKind::ForEach { var, iterable, body })
        }
    }

    /// ( init ; cond ; update ) bloc — le "isaky" a déjà été consommé par parse_isaky.
    fn parse_for_classic(&mut self) -> PResult<StmtKind> {
        self.expect(&TokenKind::LParen, "après 'isaky'")?;
        let init_line = self.current_line();
        let init = if self.check(&TokenKind::Aoka) {
            self.parse_var_decl(false)?
        } else {
            let e = self.parse_expression()?;
            self.expect(&TokenKind::Semicolon, "après l'initialisation de 'isaky'")?;
            StmtKind::ExprStmt(e)
        };
        let init = Stmt { kind: init, file: self.file.clone(), line: init_line };
        let cond = self.parse_expression()?;
        self.expect(&TokenKind::Semicolon, "après la condition de 'isaky'")?;
        let update_line = self.current_line();
        let update = Stmt {
            kind: StmtKind::ExprStmt(self.parse_expression()?),
            file: self.file.clone(),
            line: update_line,
        };
        self.expect(&TokenKind::RParen, "après la clause de 'isaky'")?;
        let body = self.parse_block()?;
        Ok(StmtKind::For { init: Box::new(init), cond, update: Box::new(update), body })
    }

    /// asa NOM ( params ) [-> type] bloc
    fn parse_function_decl(&mut self) -> PResult<FunctionDecl> {
        self.advance(); // asa
        let name = self.expect_ident("après 'asa'")?;
        let (params, return_type, body) = self.parse_fn_tail()?;
        Ok(FunctionDecl { name, params, return_type, body })
    }

    /// ( params ) [-> type] bloc — partagé entre fonctions nommées et lambdas.
    fn parse_fn_tail(&mut self) -> PResult<(Vec<Param>, Option<Type>, Vec<Stmt>)> {
        self.expect(&TokenKind::LParen, "après le nom de fonction")?;
        let mut params = Vec::new();
        if !self.check(&TokenKind::RParen) {
            loop {
                let pname = self.expect_ident("dans la liste de paramètres")?;
                let ty = if self.match_kind(&TokenKind::Colon) { Some(self.parse_type()?) } else { None };
                params.push(Param { name: pname, ty });
                if !self.match_kind(&TokenKind::Comma) { break; }
            }
        }
        self.expect(&TokenKind::RParen, "après les paramètres")?;
        let return_type = if self.match_kind(&TokenKind::Arrow) { Some(self.parse_type()?) } else { None };
        let body = self.parse_block()?;
        Ok((params, return_type, body))
    }

    /// endrika NOM { champ : type, ... }
    fn parse_struct_decl(&mut self) -> PResult<StmtKind> {
        self.advance(); // endrika
        let name = self.expect_ident("après 'endrika'")?;
        self.expect(&TokenKind::LBrace, "après le nom du struct")?;
        let mut fields = Vec::new();
        while !self.check(&TokenKind::RBrace) {
            let fname = self.expect_ident("dans les champs du struct")?;
            self.expect(&TokenKind::Colon, "après le nom de champ")?;
            let ty = self.parse_type()?;
            fields.push(Param { name: fname, ty: Some(ty) });
            if !self.match_kind(&TokenKind::Comma) { break; }
        }
        self.expect(&TokenKind::RBrace, "après les champs du struct")?;
        Ok(StmtKind::StructDecl(StructDecl { name, fields }))
    }

    /// fomba NOM { asa ... asa ... }
    fn parse_impl_decl(&mut self) -> PResult<StmtKind> {
        self.advance(); // fomba
        let type_name = self.expect_ident("après 'fomba'")?;
        self.expect(&TokenKind::LBrace, "après le nom de type dans 'fomba'")?;
        let mut methods = Vec::new();
        while !self.check(&TokenKind::RBrace) {
            self.expect(&TokenKind::Asa, "dans un bloc 'fomba' (seules des méthodes 'asa' sont autorisées)")?;
            self.pos -= 1; // remettre 'asa' pour parse_function_decl
            methods.push(self.parse_function_decl()?);
        }
        self.expect(&TokenKind::RBrace, "après un bloc 'fomba'")?;
        Ok(StmtKind::ImplDecl(ImplDecl { type_name, methods }))
    }

    fn parse_import(&mut self) -> PResult<StmtKind> {
        self.advance(); // ampiasao
        let path = match self.advance() {
            TokenKind::Teny(s) => s,
            other => return Err(self.err(format!("Chaîne attendue après 'ampiasao', trouvé {:?}", other))),
        };
        self.expect(&TokenKind::Semicolon, "après 'ampiasao'")?;
        Ok(StmtKind::Import(path))
    }

    fn parse_print(&mut self) -> PResult<StmtKind> {
        self.advance(); // asehoy
        self.expect(&TokenKind::LParen, "après 'asehoy'")?;
        let expr = self.parse_expression()?;
        self.expect(&TokenKind::RParen, "après l'argument de 'asehoy'")?;
        self.expect(&TokenKind::Semicolon, "après 'asehoy(...)'")?;
        Ok(StmtKind::Print(expr))
    }

    fn parse_return(&mut self) -> PResult<StmtKind> {
        self.advance(); // avereno
        if self.match_kind(&TokenKind::Semicolon) {
            return Ok(StmtKind::Return(None));
        }
        let expr = self.parse_expression()?;
        self.expect(&TokenKind::Semicolon, "après 'avereno'")?;
        Ok(StmtKind::Return(Some(expr)))
    }

    /// andramo { ... } sambotra (nom) { ... }
    fn parse_try(&mut self) -> PResult<StmtKind> {
        self.advance(); // andramo
        let try_block = self.parse_block()?;
        self.expect(&TokenKind::Sambotra, "après le bloc 'andramo'")?;
        self.expect(&TokenKind::LParen, "après 'sambotra'")?;
        let catch_var = self.expect_ident("dans 'sambotra(...)'")?;
        self.expect(&TokenKind::RParen, "après le nom dans 'sambotra(...)'")?;
        let catch_block = self.parse_block()?;
        Ok(StmtKind::TryCatch { try_block, catch_var, catch_block })
    }

    fn parse_throw(&mut self) -> PResult<StmtKind> {
        self.advance(); // atsipazo
        let expr = self.parse_expression()?;
        self.expect(&TokenKind::Semicolon, "après 'atsipazo'")?;
        Ok(StmtKind::Throw(expr))
    }

    // ---------- Expressions (précédence croissante) ----------

    fn parse_expression(&mut self) -> PResult<Expr> {
        self.parse_assignment()
    }

    fn parse_assignment(&mut self) -> PResult<Expr> {
        let target = self.parse_ternary()?;
        if self.match_kind(&TokenKind::Eq) {
            let value = self.parse_assignment()?;
            return Ok(Expr::Assign(Box::new(target), Box::new(value)));
        }
        if let Some(op) = self.match_compound_op() {
            let value = self.parse_assignment()?;
            let combined = Expr::Binary(Box::new(target.clone()), op, Box::new(value));
            return Ok(Expr::Assign(Box::new(target), Box::new(combined)));
        }
        Ok(target)
    }

    fn match_compound_op(&mut self) -> Option<BinOp> {
        let op = match self.peek() {
            TokenKind::PlusEq => BinOp::Add,
            TokenKind::MinusEq => BinOp::Sub,
            TokenKind::StarEq => BinOp::Mul,
            TokenKind::SlashEq => BinOp::Div,
            TokenKind::PercentEq => BinOp::Mod,
            _ => return None,
        };
        self.advance();
        Some(op)
    }

    /// cond ? si_vrai : si_faux (associatif à droite, comme en C)
    fn parse_ternary(&mut self) -> PResult<Expr> {
        let cond = self.parse_or()?;
        if self.match_kind(&TokenKind::Question) {
            let then_branch = self.with_struct_lit(false, |p| p.parse_expression())?;
            self.expect(&TokenKind::Colon, "dans l'opérateur ternaire ('cond ? a : b')")?;
            let else_branch = self.parse_ternary()?;
            return Ok(Expr::Ternary(Box::new(cond), Box::new(then_branch), Box::new(else_branch)));
        }
        Ok(cond)
    }

    fn parse_or(&mut self) -> PResult<Expr> {
        let mut left = self.parse_and()?;
        while self.check(&TokenKind::Na) {
            self.advance();
            let right = self.parse_and()?;
            left = Expr::Binary(Box::new(left), BinOp::Or, Box::new(right));
        }
        Ok(left)
    }

    fn parse_and(&mut self) -> PResult<Expr> {
        let mut left = self.parse_equality()?;
        while self.check(&TokenKind::Sy) {
            self.advance();
            let right = self.parse_equality()?;
            left = Expr::Binary(Box::new(left), BinOp::And, Box::new(right));
        }
        Ok(left)
    }

    fn parse_equality(&mut self) -> PResult<Expr> {
        let mut left = self.parse_comparison()?;
        loop {
            let op = match self.peek() {
                TokenKind::EqEq => BinOp::Eq,
                TokenKind::NotEq => BinOp::NotEq,
                _ => break,
            };
            self.advance();
            let right = self.parse_comparison()?;
            left = Expr::Binary(Box::new(left), op, Box::new(right));
        }
        Ok(left)
    }

    fn parse_comparison(&mut self) -> PResult<Expr> {
        let mut left = self.parse_term()?;
        loop {
            let op = match self.peek() {
                TokenKind::Lt => BinOp::Lt,
                TokenKind::Gt => BinOp::Gt,
                TokenKind::LtEq => BinOp::LtEq,
                TokenKind::GtEq => BinOp::GtEq,
                _ => break,
            };
            self.advance();
            let right = self.parse_term()?;
            left = Expr::Binary(Box::new(left), op, Box::new(right));
        }
        Ok(left)
    }

    fn parse_term(&mut self) -> PResult<Expr> {
        let mut left = self.parse_factor()?;
        loop {
            let op = match self.peek() {
                TokenKind::Plus => BinOp::Add,
                TokenKind::Minus => BinOp::Sub,
                _ => break,
            };
            self.advance();
            let right = self.parse_factor()?;
            left = Expr::Binary(Box::new(left), op, Box::new(right));
        }
        Ok(left)
    }

    fn parse_factor(&mut self) -> PResult<Expr> {
        let mut left = self.parse_unary()?;
        loop {
            let op = match self.peek() {
                TokenKind::Star => BinOp::Mul,
                TokenKind::Slash => BinOp::Div,
                TokenKind::Percent => BinOp::Mod,
                _ => break,
            };
            self.advance();
            let right = self.parse_unary()?;
            left = Expr::Binary(Box::new(left), op, Box::new(right));
        }
        Ok(left)
    }

    fn parse_unary(&mut self) -> PResult<Expr> {
        match self.peek() {
            TokenKind::Minus => { self.advance(); Ok(Expr::Unary(UnaryOp::Neg, Box::new(self.parse_unary()?))) }
            TokenKind::Tsy => { self.advance(); Ok(Expr::Unary(UnaryOp::Not, Box::new(self.parse_unary()?))) }
            _ => self.parse_call(),
        }
    }

    fn parse_call(&mut self) -> PResult<Expr> {
        let mut expr = self.parse_primary()?;
        loop {
            if self.match_kind(&TokenKind::LParen) {
                let mut args = Vec::new();
                if !self.check(&TokenKind::RParen) {
                    loop {
                        args.push(self.with_struct_lit(false, |p| p.parse_expression())?);
                        if !self.match_kind(&TokenKind::Comma) { break; }
                    }
                }
                self.expect(&TokenKind::RParen, "après les arguments d'appel")?;
                expr = Expr::Call(Box::new(expr), args);
            } else if self.match_kind(&TokenKind::Dot) {
                let field = self.expect_ident("après '.'")?;
                expr = Expr::FieldAccess(Box::new(expr), field);
            } else if self.match_kind(&TokenKind::LBracket) {
                let idx = self.with_struct_lit(false, |p| p.parse_expression())?;
                self.expect(&TokenKind::RBracket, "après un index")?;
                expr = Expr::Index(Box::new(expr), Box::new(idx));
            } else {
                break;
            }
        }
        Ok(expr)
    }

    fn parse_primary(&mut self) -> PResult<Expr> {
        let line = self.current_line();
        let col = self.current_col();
        match self.peek().clone() {
            TokenKind::Isa(n) => { self.advance(); Ok(Expr::IntLit(n)) }
            TokenKind::Ampahany(f) => { self.advance(); Ok(Expr::FloatLit(f)) }
            TokenKind::Teny(s) => { self.advance(); Ok(Expr::StrLit(s)) }
            TokenKind::Marina => { self.advance(); Ok(Expr::BoolLit(true)) }
            TokenKind::Diso => { self.advance(); Ok(Expr::BoolLit(false)) }

            // Fonction anonyme (lambda) : asa(params) [-> type] { corps }
            TokenKind::Asa => {
                self.advance();
                let (params, return_type, body) = self.parse_fn_tail()?;
                Ok(Expr::Lambda(params, return_type, body))
            }

            // Tableau [a, b, c] OU dictionnaire [cle: val, ...]
            TokenKind::LBracket => {
                self.advance();
                if self.match_kind(&TokenKind::RBracket) {
                    return Ok(Expr::ArrayLit(Vec::new()));
                }
                let first = self.with_struct_lit(false, |p| p.parse_expression())?;
                if self.match_kind(&TokenKind::Colon) {
                    let first_val = self.with_struct_lit(false, |p| p.parse_expression())?;
                    let mut pairs = vec![(first, first_val)];
                    while self.match_kind(&TokenKind::Comma) {
                        if self.check(&TokenKind::RBracket) { break; }
                        let k = self.with_struct_lit(false, |p| p.parse_expression())?;
                        self.expect(&TokenKind::Colon, "après la clé dans un littéral de dictionnaire")?;
                        let v = self.with_struct_lit(false, |p| p.parse_expression())?;
                        pairs.push((k, v));
                    }
                    self.expect(&TokenKind::RBracket, "après un littéral de dictionnaire")?;
                    Ok(Expr::DictLit(pairs))
                } else {
                    let mut elems = vec![first];
                    while self.match_kind(&TokenKind::Comma) {
                        if self.check(&TokenKind::RBracket) { break; }
                        elems.push(self.with_struct_lit(false, |p| p.parse_expression())?);
                    }
                    self.expect(&TokenKind::RBracket, "après un littéral de tableau")?;
                    Ok(Expr::ArrayLit(elems))
                }
            }

            // Parenthèses : expression groupée OU tuple (a, b, c) si une virgule suit.
            TokenKind::LParen => {
                self.advance();
                let first = self.with_struct_lit(false, |p| p.parse_expression())?;
                if self.check(&TokenKind::Comma) {
                    let mut elems = vec![first];
                    while self.match_kind(&TokenKind::Comma) {
                        if self.check(&TokenKind::RParen) { break; }
                        elems.push(self.with_struct_lit(false, |p| p.parse_expression())?);
                    }
                    self.expect(&TokenKind::RParen, "après un littéral de tuple")?;
                    Ok(Expr::TupleLit(elems))
                } else {
                    self.expect(&TokenKind::RParen, "après une expression parenthésée")?;
                    Ok(first)
                }
            }

            TokenKind::Anarana(name) => {
                self.advance();
                // Instanciation de struct : NOM { champ: expr, ... }
                // Autorisée sauf directement dans une condition (raha/raha mbola), où
                // "identifiant {" doit être compris comme le début d'un bloc.
                if !self.no_struct_lit && self.check(&TokenKind::LBrace) {
                    self.advance();
                    let mut fields = Vec::new();
                    while !self.check(&TokenKind::RBrace) {
                        let fname = self.expect_ident("dans une instanciation de struct")?;
                        self.expect(&TokenKind::Colon, "après le nom de champ")?;
                        let value = self.with_struct_lit(false, |p| p.parse_expression())?;
                        fields.push((fname, value));
                        if !self.match_kind(&TokenKind::Comma) { break; }
                    }
                    self.expect(&TokenKind::RBrace, "après une instanciation de struct")?;
                    Ok(Expr::StructInit(name, fields))
                } else {
                    Ok(Expr::Ident(name))
                }
            }
            other => Err(OlyError::with_col(self.file.clone(), line, col, format!("Expression attendue, trouvé {:?}", other))),
        }
    }
}

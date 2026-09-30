//! Vérificateur de types OLY.
//!
//! Typage GRADUEL : quand le type d'une expression ne peut pas être déterminé
//! statiquement, le vérificateur n'émet pas d'erreur — il laisse la VM faire
//! le travail à l'exécution. Les lambdas et les tuples ne sont vérifiés que
//! partiellement (limitation assumée) : leur contenu est vérifié, mais un
//! appel dynamique via une variable, ou un accès indexé à un tuple, n'est
//! pas garanti statiquement.
//!
//! Chaque erreur est rattachée au fichier/ligne de la statement en cours de
//! vérification (self.file / self.line), pour un affichage avec contexte source.

use crate::ast::*;
use crate::errors::OlyError;
use std::collections::HashMap;

#[derive(Clone)]
struct FnSig {
    params: Vec<Option<Type>>,
    return_type: Option<Type>,
}

pub struct Checker {
    structs: HashMap<String, Vec<(String, Type)>>,
    functions: HashMap<String, FnSig>,
    errors: Vec<OlyError>,
    file: String,
    line: usize,
    global_env: HashMap<String, Type>,
}

pub fn check_program(program: &Program) -> Result<(), Vec<OlyError>> {
    Checker::new().check(program)
}

fn is_numeric(t: &Type) -> bool {
    matches!(t, Type::Isa | Type::Ampahany)
}

impl Checker {
    pub fn new() -> Self {
        Checker {
            structs: HashMap::new(), functions: HashMap::new(), errors: Vec::new(),
            file: String::new(), line: 0, global_env: HashMap::new(),
        }
    }

    pub fn check(&mut self, program: &Program) -> Result<(), Vec<OlyError>> {
        self.errors.clear();
        self.collect_declarations(program);
        self.check_items(program);
        if self.errors.is_empty() { Ok(()) } else { Err(std::mem::take(&mut self.errors)) }
    }

    fn push_error(&mut self, msg: impl Into<String>) {
        self.errors.push(OlyError::new(self.file.clone(), self.line, msg));
    }

    fn collect_declarations(&mut self, program: &Program) {
        for item in &program.items {
            match &item.kind {
                StmtKind::StructDecl(s) => {
                    let fields = s.fields.iter().map(|p| (p.name.clone(), p.ty.clone().unwrap_or(Type::Isa))).collect();
                    self.structs.insert(s.name.clone(), fields);
                }
                StmtKind::FunctionDecl(f) => {
                    self.functions.insert(f.name.clone(), FnSig {
                        params: f.params.iter().map(|p| p.ty.clone()).collect(),
                        return_type: f.return_type.clone(),
                    });
                }
                StmtKind::ImplDecl(i) => {
                    for m in &i.methods {
                        let mut params: Vec<Option<Type>> = m.params.iter().map(|p| p.ty.clone()).collect();
                        if let Some(first) = params.first_mut() {
                            if first.is_none() { *first = Some(Type::Endrika(i.type_name.clone())); }
                        }
                        let mangled = format!("{}_{}", i.type_name, m.name);
                        self.functions.insert(mangled, FnSig { params, return_type: m.return_type.clone() });
                    }
                }
                _ => {}
            }
        }
    }

    fn check_items(&mut self, program: &Program) {
        let mut global_env: HashMap<String, Type> = std::mem::take(&mut self.global_env);
        for item in &program.items {
            match &item.kind {
                StmtKind::FunctionDecl(f) => self.check_function(f, None),
                StmtKind::ImplDecl(i) => {
                    for m in &i.methods {
                        self.check_function(m, Some(i.type_name.clone()));
                    }
                }
                StmtKind::StructDecl(_) | StmtKind::Import(_) => {}
                _ => self.check_stmt(item, &mut global_env, None),
            }
        }
        self.global_env = global_env;
    }

    fn check_function(&mut self, f: &FunctionDecl, self_type: Option<String>) {
        let mut env: HashMap<String, Type> = HashMap::new();
        for (i, p) in f.params.iter().enumerate() {
            let ty = if i == 0 && p.ty.is_none() {
                self_type.clone().map(Type::Endrika)
            } else {
                p.ty.clone()
            };
            env.insert(p.name.clone(), ty.unwrap_or(Type::Inconnu));
        }
        for stmt in &f.body {
            self.check_stmt(stmt, &mut env, f.return_type.as_ref());
        }
    }

    fn check_stmt(&mut self, stmt: &Stmt, env: &mut HashMap<String, Type>, return_type: Option<&Type>) {
        self.file = stmt.file.clone();
        self.line = stmt.line;
        match &stmt.kind {
            StmtKind::VarDecl { name, declared_type, value, .. } => {
                let value_ty = self.infer_expr(value, env);
                if let (Some(dt), Some(vt)) = (declared_type, &value_ty) {
                    if dt != vt {
                        self.push_error(format!(
                            "Type incompatible pour '{}' : déclaré {:?}, valeur de type {:?}", name, dt, vt
                        ));
                    }
                }
                let effective = declared_type.clone().or(value_ty);
                env.insert(name.clone(), effective.unwrap_or(Type::Inconnu));
            }
            StmtKind::ExprStmt(e) | StmtKind::Print(e) => { self.infer_expr(e, env); }
            StmtKind::If { cond, then_branch, else_ifs, else_branch } => {
                self.check_condition(cond, env);
                for s in then_branch { self.check_stmt(s, env, return_type); }
                for (c, b) in else_ifs {
                    self.check_condition(c, env);
                    for s in b { self.check_stmt(s, env, return_type); }
                }
                if let Some(b) = else_branch {
                    for s in b { self.check_stmt(s, env, return_type); }
                }
            }
            StmtKind::While { cond, body } => {
                self.check_condition(cond, env);
                for s in body { self.check_stmt(s, env, return_type); }
            }
            StmtKind::For { init, cond, update, body } => {
                self.check_stmt(init, env, return_type);
                self.check_condition(cond, env);
                for s in body { self.check_stmt(s, env, return_type); }
                self.check_stmt(update, env, return_type);
            }
            StmtKind::ForEach { var, iterable, body } => {
                let it_ty = self.infer_expr(iterable, env);
                match it_ty {
                    Some(Type::Lisitra(elem)) => { env.insert(var.clone(), *elem); }
                    _ => { env.insert(var.clone(), Type::Inconnu); }
                }
                for s in body { self.check_stmt(s, env, return_type); }
            }
            StmtKind::Return(e) => match (e, return_type) {
                (Some(expr), Some(rt)) => {
                    let vt = self.infer_expr(expr, env);
                    if let Some(vt) = vt {
                        if &vt != rt {
                            self.push_error(format!("Type de retour incohérent : attendu {:?}, trouvé {:?}", rt, vt));
                        }
                    }
                }
                (Some(expr), None) => { self.infer_expr(expr, env); }
                (None, Some(rt)) => {
                    self.push_error(format!("'avereno;' sans valeur mais la fonction doit retourner {:?}", rt));
                }
                (None, None) => {}
            },
            StmtKind::TryCatch { try_block, catch_var, catch_block } => {
                for s in try_block { self.check_stmt(s, env, return_type); }
                env.insert(catch_var.clone(), Type::Teny);
                for s in catch_block { self.check_stmt(s, env, return_type); }
            }
            StmtKind::Throw(e) => {
                let t = self.infer_expr(e, env);
                if let Some(t) = t {
                    if t != Type::Teny {
                        self.push_error(format!("'atsipazo' attend un message de type 'teny', trouvé {:?}", t));
                    }
                }
            }
            StmtKind::Break | StmtKind::Continue | StmtKind::Import(_) => {}
            StmtKind::FunctionDecl(_) | StmtKind::StructDecl(_) | StmtKind::ImplDecl(_) => {}
        }
    }

    fn check_condition(&mut self, cond: &Expr, env: &mut HashMap<String, Type>) {
        if let Some(t) = self.infer_expr(cond, env) {
            if t != Type::Marina {
                self.push_error(format!("La condition doit être de type 'marina' (booléen), trouvé {:?}", t));
            }
        }
    }

    fn infer_expr(&mut self, expr: &Expr, env: &mut HashMap<String, Type>) -> Option<Type> {
        match expr {
            Expr::IntLit(_) => Some(Type::Isa),
            Expr::FloatLit(_) => Some(Type::Ampahany),
            Expr::StrLit(_) => Some(Type::Teny),
            Expr::BoolLit(_) => Some(Type::Marina),

            Expr::Ident(name) => env.get(name).cloned().filter(|t| *t != Type::Inconnu),

            Expr::Unary(op, e) => {
                let t = self.infer_expr(e, env);
                match (op, &t) {
                    (UnaryOp::Neg, Some(ty)) if !is_numeric(ty) => {
                        self.push_error(format!("'-' unaire attend un nombre, trouvé {:?}", ty));
                        None
                    }
                    (UnaryOp::Neg, _) => t,
                    (UnaryOp::Not, _) => Some(Type::Marina),
                }
            }

            Expr::Ternary(cond, a, b) => {
                self.check_condition(cond, env);
                let at = self.infer_expr(a, env);
                let bt = self.infer_expr(b, env);
                match (&at, &bt) {
                    (Some(x), Some(y)) if x == y => Some(x.clone()),
                    (Some(x), Some(y)) => {
                        self.push_error(format!("Les deux branches de l'opérateur ternaire ont des types différents : {:?} et {:?}", x, y));
                        None
                    }
                    _ => None,
                }
            }

            Expr::Binary(l, op, r) => {
                let lt = self.infer_expr(l, env);
                let rt = self.infer_expr(r, env);
                self.infer_binary(op, lt, rt)
            }

            Expr::Assign(target, value) => {
                let vt = self.infer_expr(value, env);
                match target.as_ref() {
                    Expr::Ident(name) => {
                        if let (Some(old), Some(new_t)) = (env.get(name).cloned().filter(|t| *t != Type::Inconnu), &vt) {
                            if &old != new_t {
                                self.push_error(format!(
                                    "Type incompatible pour la variable '{}' : attendu {:?}, trouvé {:?}", name, old, new_t
                                ));
                            }
                        } else if let Some(new_t) = &vt {
                            env.insert(name.clone(), new_t.clone());
                        }
                        vt
                    }
                    Expr::FieldAccess(base, field) => {
                        let base_ty = self.infer_expr(base, env);
                        if let Some(field_ty) = self.field_type(&base_ty, field) {
                            if let Some(new_t) = &vt {
                                if &field_ty != new_t {
                                    self.push_error(format!(
                                        "Type incompatible pour le champ '{}' : attendu {:?}, trouvé {:?}", field, field_ty, new_t
                                    ));
                                }
                            }
                        }
                        vt
                    }
                    Expr::Index(base, idx) => {
                        let base_ty = self.infer_expr(base, env);
                        let idx_ty = self.infer_expr(idx, env);
                        if matches!(base_ty, Some(Type::Lisitra(_))) {
                            if let Some(it) = &idx_ty {
                                if it != &Type::Isa {
                                    self.push_error(format!("Un index de tableau doit être de type 'isa', trouvé {:?}", it));
                                }
                            }
                        }
                        match &base_ty {
                            Some(Type::Lisitra(inner)) => {
                                if let Some(new_t) = &vt {
                                    if inner.as_ref() != new_t {
                                        self.push_error(format!(
                                            "Type incompatible pour l'élément de liste : attendu {:?}, trouvé {:?}", inner, new_t
                                        ));
                                    }
                                }
                            }
                            Some(Type::Rakitra(_, _)) | Some(Type::Voambatra(_)) | None => {}
                            Some(other) => self.push_error(format!("Affectation indexée '[...]' sur un type non indexable : {:?}", other)),
                        }
                        vt
                    }
                    _ => { self.push_error("Cible d'affectation invalide"); None }
                }
            }

            Expr::FieldAccess(base, field) => {
                let base_ty = self.infer_expr(base, env);
                self.field_type(&base_ty, field)
            }

            Expr::ArrayLit(elems) => {
                let mut elem_ty: Option<Type> = None;
                for e in elems {
                    let t = self.infer_expr(e, env);
                    if let Some(t) = t {
                        match &elem_ty {
                            None => elem_ty = Some(t),
                            Some(existing) if existing != &t => {
                                self.push_error(format!("Types incohérents dans le tableau : {:?} et {:?}", existing, t));
                            }
                            _ => {}
                        }
                    }
                }
                elem_ty.map(|t| Type::Lisitra(Box::new(t)))
            }

            Expr::DictLit(pairs) => {
                let mut key_ty: Option<Type> = None;
                let mut val_ty: Option<Type> = None;
                for (k, v) in pairs {
                    if let Some(kt) = self.infer_expr(k, env) {
                        match &key_ty {
                            None => key_ty = Some(kt),
                            Some(existing) if existing != &kt => self.push_error(format!("Clés de types incohérents dans le dictionnaire : {:?} et {:?}", existing, kt)),
                            _ => {}
                        }
                    }
                    if let Some(vt) = self.infer_expr(v, env) {
                        match &val_ty {
                            None => val_ty = Some(vt),
                            Some(existing) if existing != &vt => self.push_error(format!("Valeurs de types incohérents dans le dictionnaire : {:?} et {:?}", existing, vt)),
                            _ => {}
                        }
                    }
                }
                match (key_ty, val_ty) {
                    (Some(k), Some(v)) => Some(Type::Rakitra(Box::new(k), Box::new(v))),
                    _ => None,
                }
            }

            Expr::TupleLit(elems) => {
                let types: Vec<Option<Type>> = elems.iter().map(|e| self.infer_expr(e, env)).collect();
                if types.iter().all(|t| t.is_some()) {
                    Some(Type::Voambatra(types.into_iter().map(|t| t.unwrap()).collect()))
                } else {
                    None
                }
            }

            Expr::Index(base, idx) => {
                let base_ty = self.infer_expr(base, env);
                let idx_ty = self.infer_expr(idx, env);
                if matches!(base_ty, Some(Type::Lisitra(_)) | Some(Type::Teny)) {
                    if let Some(it) = &idx_ty {
                        if it != &Type::Isa {
                            self.push_error(format!("Un index de tableau doit être de type 'isa', trouvé {:?}", it));
                        }
                    }
                }
                match base_ty {
                    Some(Type::Lisitra(inner)) => Some(*inner),
                    Some(Type::Rakitra(_key, val)) => Some(*val),
                    Some(Type::Voambatra(_)) => None, // élément hétérogène : type exact non vérifié statiquement
                    Some(Type::Teny) => Some(Type::Teny), // "abc"[0] -> "a"
                    Some(other) => { self.push_error(format!("Indexation '[...]' sur un type non indexable : {:?}", other)); None }
                    None => None,
                }
            }

            Expr::Lambda(params, return_type, body) => {
                let mut lambda_env: HashMap<String, Type> = HashMap::new();
                for p in params {
                    lambda_env.insert(p.name.clone(), p.ty.clone().unwrap_or(Type::Inconnu));
                }
                for s in body { self.check_stmt(s, &mut lambda_env, return_type.as_ref()); }
                None // le type "fonction" n'est pas modélisé : les appels dynamiques ne sont pas vérifiés statiquement
            }

            Expr::Call(callee, args) => match callee.as_ref() {
                Expr::Ident(name) => {
                    let arg_types: Vec<Option<Type>> = args.iter().map(|a| self.infer_expr(a, env)).collect();
                    if let Some(t) = self.check_builtin_call(name, &arg_types) {
                        return t;
                    }
                    match self.functions.get(name).cloned() {
                        Some(sig) => {
                            self.check_args(name, &sig.params, &arg_types);
                            sig.return_type
                        }
                        None => {
                            // Peut être une variable contenant une lambda : appel dynamique, non vérifié statiquement.
                            if env.contains_key(name) { None } else {
                                self.push_error(format!("Fonction inconnue : '{}'", name));
                                None
                            }
                        }
                    }
                }
                Expr::FieldAccess(base, method) => {
                    let base_ty = self.infer_expr(base, env);
                    let arg_types: Vec<Option<Type>> = args.iter().map(|a| self.infer_expr(a, env)).collect();
                    match &base_ty {
                        Some(Type::Endrika(sname)) => {
                            let mangled = format!("{}_{}", sname, method);
                            match self.functions.get(&mangled).cloned() {
                                Some(sig) => {
                                    let expected: Vec<Option<Type>> = sig.params.iter().skip(1).cloned().collect();
                                    self.check_args(&mangled, &expected, &arg_types);
                                    sig.return_type
                                }
                                None => { self.push_error(format!("Méthode inconnue : '{}' sur '{}'", method, sname)); None }
                            }
                        }
                        Some(other) => {
                            self.push_error(format!("Appel de méthode '{}' sur un type non-struct : {:?}", method, other));
                            None
                        }
                        None => None,
                    }
                }
                _ => None, // appel dynamique sur une expression arbitraire : non vérifié statiquement
            },

            Expr::StructInit(name, fields) => {
                match self.structs.get(name).cloned() {
                    Some(decl_fields) => {
                        for (fname, fexpr) in fields {
                            let fty = self.infer_expr(fexpr, env);
                            match decl_fields.iter().find(|(n, _)| n == fname) {
                                Some((_, expected)) => {
                                    if let Some(actual) = &fty {
                                        if actual != expected {
                                            self.push_error(format!(
                                                "Type incompatible pour le champ '{}' de '{}' : attendu {:?}, trouvé {:?}",
                                                fname, name, expected, actual
                                            ));
                                        }
                                    }
                                }
                                None => self.push_error(format!("Champ inconnu '{}' pour la struct '{}'", fname, name)),
                            }
                        }
                        let provided: Vec<&String> = fields.iter().map(|(n, _)| n).collect();
                        for (dname, _) in &decl_fields {
                            if !provided.contains(&dname) {
                                self.push_error(format!("Champ manquant '{}' dans l'instanciation de '{}'", dname, name));
                            }
                        }
                        Some(Type::Endrika(name.clone()))
                    }
                    None => { self.push_error(format!("Struct inconnue : '{}'", name)); None }
                }
            }
        }
    }

    /// Fonctions natives connues du vérificateur : Some(type_de_retour) si le nom est
    /// bien un builtin (pour ne pas signaler à tort "fonction inconnue"), None sinon.
    /// La vérification des types d'arguments reste volontairement légère pour ces
    /// fonctions génériques (habeny/misy/ampio acceptent liste, dict ou chaîne).
    fn check_builtin_call(&mut self, name: &str, arg_types: &[Option<Type>]) -> Option<Option<Type>> {
        let arity_err = |c: &mut Self, expected: usize| {
            if arg_types.len() != expected {
                c.push_error(format!("'{}' attend {} argument(s), {} fourni(s)", name, expected, arg_types.len()));
            }
        };
        match name {
            "habeny" => { arity_err(self, 1); Some(Some(Type::Isa)) }
            "ampio" => { arity_err(self, 2); Some(None) }
            "esory" => { arity_err(self, 1); Some(None) }
            "misy" => { arity_err(self, 2); Some(Some(Type::Marina)) }
            "fanalahidy" => { arity_err(self, 1); Some(None) }
            "faka" => { arity_err(self, 1); Some(Some(Type::Ampahany)) }
            "heriny" => { arity_err(self, 2); Some(Some(Type::Ampahany)) }
            "habe" => { arity_err(self, 1); Some(None) }
            "kely" | "lehibe" => { arity_err(self, 2); Some(None) }
            "avo" | "ambany" | "fafao" => { arity_err(self, 1); Some(Some(Type::Teny)) }
            "vaky" => { arity_err(self, 2); Some(Some(Type::Lisitra(Box::new(Type::Teny)))) }
            "akambana" => { arity_err(self, 2); Some(Some(Type::Teny)) }
            "henoy" => { arity_err(self, 0); Some(Some(Type::Teny)) }
            _ => None,
        }
    }

    fn field_type(&mut self, base_ty: &Option<Type>, field: &str) -> Option<Type> {
        match base_ty {
            Some(Type::Endrika(sname)) => match self.structs.get(sname) {
                Some(fields) => match fields.iter().find(|(n, _)| n == field) {
                    Some((_, t)) => Some(t.clone()),
                    None => { self.push_error(format!("Champ inconnu '{}' sur la struct '{}'", field, sname)); None }
                },
                None => None,
            },
            Some(other) => {
                self.push_error(format!("Accès au champ '{}' sur un type non-struct : {:?}", field, other));
                None
            }
            None => None,
        }
    }

    fn check_args(&mut self, name: &str, expected: &[Option<Type>], actual: &[Option<Type>]) {
        if expected.len() != actual.len() {
            self.push_error(format!("'{}' attend {} argument(s), {} fourni(s)", name, expected.len(), actual.len()));
            return;
        }
        for (i, (exp, act)) in expected.iter().zip(actual.iter()).enumerate() {
            if let (Some(e), Some(a)) = (exp, act) {
                if e != a {
                    self.push_error(format!(
                        "'{}' : argument {} de type incorrect, attendu {:?}, trouvé {:?}", name, i + 1, e, a
                    ));
                }
            }
        }
    }

    fn infer_binary(&mut self, op: &BinOp, lt: Option<Type>, rt: Option<Type>) -> Option<Type> {
        use Type::*;
        match op {
            BinOp::Add => match (&lt, &rt) {
                (Some(Isa), Some(Isa)) => Some(Isa),
                (Some(Isa), Some(Ampahany)) | (Some(Ampahany), Some(Isa)) | (Some(Ampahany), Some(Ampahany)) => Some(Ampahany),
                (Some(Teny), Some(_)) => Some(Teny), // concaténation : tout se convertit en texte
                (Some(a), Some(b)) => {
                    self.push_error(format!("Type invalide pour '+' : {:?} et {:?}", a, b));
                    None
                }
                _ => None,
            },
            BinOp::Sub | BinOp::Mul | BinOp::Div | BinOp::Mod => match (&lt, &rt) {
                (Some(a), Some(b)) if is_numeric(a) && is_numeric(b) => {
                    if matches!(a, Ampahany) || matches!(b, Ampahany) { Some(Ampahany) } else { Some(Isa) }
                }
                (Some(a), Some(b)) => {
                    self.push_error(format!("Opération arithmétique sur des types non numériques : {:?} et {:?}", a, b));
                    None
                }
                _ => None,
            },
            BinOp::Lt | BinOp::Gt | BinOp::LtEq | BinOp::GtEq => match (&lt, &rt) {
                (Some(a), Some(b)) if !(is_numeric(a) && is_numeric(b)) => {
                    self.push_error(format!("Comparaison entre types non numériques : {:?} et {:?}", a, b));
                    Some(Marina)
                }
                _ => Some(Marina),
            },
            BinOp::Eq | BinOp::NotEq | BinOp::And | BinOp::Or => Some(Marina),
        }
    }
}

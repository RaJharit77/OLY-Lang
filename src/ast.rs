//! AST (arbre syntaxique abstrait) du langage OLY.

#[derive(Debug, Clone, PartialEq)]
pub enum Type {
    Isa,          // entier
    Ampahany,     // flottant
    Teny,         // chaîne
    Marina,       // booléen
    Endrika(String),      // type struct nommé
    Lisitra(Box<Type>),   // tableau/liste ("elementType[]")
    Rakitra(Box<Type>, Box<Type>), // dictionnaire (clé, valeur)
    Voambatra(Vec<Type>), // tuple (types hétérogènes, taille fixe)
    /// Interne au vérificateur : variable déclarée mais de type non déductible
    /// (paramètre sans annotation, lambda...). Jamais produit par le parser.
    Inconnu,
}

#[derive(Debug, Clone, PartialEq)]
pub enum BinOp {
    Add, Sub, Mul, Div, Mod,
    Eq, NotEq, Lt, Gt, LtEq, GtEq,
    And, Or,
}

#[derive(Debug, Clone, PartialEq)]
pub enum UnaryOp {
    Neg,  // -x
    Not,  // tsy x
}

#[derive(Debug, Clone, PartialEq)]
pub enum Expr {
    IntLit(i64),
    FloatLit(f64),
    StrLit(String),
    BoolLit(bool),
    Ident(String),
    Unary(UnaryOp, Box<Expr>),
    Binary(Box<Expr>, BinOp, Box<Expr>),
    /// Opérateur ternaire : cond ? si_vrai : si_faux
    Ternary(Box<Expr>, Box<Expr>, Box<Expr>),
    Assign(Box<Expr>, Box<Expr>),
    Call(Box<Expr>, Vec<Expr>),
    FieldAccess(Box<Expr>, String),
    StructInit(String, Vec<(String, Expr)>),
    ArrayLit(Vec<Expr>),
    DictLit(Vec<(Expr, Expr)>),
    TupleLit(Vec<Expr>),
    Index(Box<Expr>, Box<Expr>),
    /// Fonction anonyme : asa(params) [-> type] { corps }
    Lambda(Vec<Param>, Option<Type>, Vec<Stmt>),
}

#[derive(Debug, Clone, PartialEq)]
pub struct Param {
    pub name: String,
    pub ty: Option<Type>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct FunctionDecl {
    pub name: String,
    pub params: Vec<Param>,
    pub return_type: Option<Type>,
    pub body: Vec<Stmt>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct StructDecl {
    pub name: String,
    pub fields: Vec<Param>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ImplDecl {
    pub type_name: String,
    pub methods: Vec<FunctionDecl>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum StmtKind {
    VarDecl { name: String, mutable: bool, declared_type: Option<Type>, value: Expr },
    ExprStmt(Expr),
    Print(Expr),
    If {
        cond: Expr,
        then_branch: Vec<Stmt>,
        else_ifs: Vec<(Expr, Vec<Stmt>)>,
        else_branch: Option<Vec<Stmt>>,
    },
    While { cond: Expr, body: Vec<Stmt> },
    For { init: Box<Stmt>, cond: Expr, update: Box<Stmt>, body: Vec<Stmt> },
    /// isaky ELEM amin ITERABLE { corps } — boucle for-each (liste ou dictionnaire)
    ForEach { var: String, iterable: Expr, body: Vec<Stmt> },
    Return(Option<Expr>),
    Break,
    Continue,
    Import(String),
    FunctionDecl(FunctionDecl),
    StructDecl(StructDecl),
    ImplDecl(ImplDecl),
    /// andramo { essai } sambotra (nom_erreur) { rattrapage }
    TryCatch { try_block: Vec<Stmt>, catch_var: String, catch_block: Vec<Stmt> },
    /// atsipazo expr; — lève une erreur récupérable par 'sambotra'
    Throw(Expr),
}

/// Une instruction accompagnée de sa position dans le code source
/// (fichier + numéro de ligne), utilisée pour produire des messages
/// d'erreur de type/exécution qui pointent vers le bon endroit.
#[derive(Debug, Clone, PartialEq)]
pub struct Stmt {
    pub kind: StmtKind,
    pub file: String,
    pub line: usize,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Program {
    pub items: Vec<Stmt>,
}

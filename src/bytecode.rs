//! Bytecode OLY : le format intermédiaire produit par le compilateur
//! et exécuté par la VM (voir vm.rs).

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

/// Clé de dictionnaire : on se limite aux entiers et aux chaînes, qui sont
/// les seuls types de clé pour lesquels une égalité/hachage simple et fiable
/// a du sens ici (les flottants ne font pas de bonnes clés de hachage).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum DictKey {
    Int(i64),
    Str(String),
}

impl DictKey {
    pub fn from_value(v: &Value) -> Result<DictKey, String> {
        match v {
            Value::Int(n) => Ok(DictKey::Int(*n)),
            Value::Str(s) => Ok(DictKey::Str(s.clone())),
            other => Err(format!("Une clé de dictionnaire doit être 'isa' ou 'teny', trouvé {:?}", other)),
        }
    }
    pub fn into_value(self) -> Value {
        match self {
            DictKey::Int(n) => Value::Int(n),
            DictKey::Str(s) => Value::Str(s),
        }
    }
}

#[derive(Debug, Clone)]
pub enum Value {
    Int(i64),
    Float(f64),
    Str(String),
    Bool(bool),
    Null,
    /// Rc<RefCell<..>> pour que les champs restent mutables et partagés
    /// (une struct passée à une fonction est passée par référence, comme un objet).
    Struct(String, Rc<RefCell<HashMap<String, Value>>>),
    /// Idem pour les tableaux : passés par référence, mutables en place.
    List(Rc<RefCell<Vec<Value>>>),
    /// Dictionnaire : clés 'isa'/'teny' uniquement (voir DictKey).
    Dict(Rc<RefCell<HashMap<DictKey, Value>>>),
    /// Tuple : taille fixe, hétérogène, immuable.
    Tuple(Rc<Vec<Value>>),
    /// Référence vers une fonction ou lambda compilée (par son nom interne),
    /// permet de stocker une fonction dans une variable et de l'appeler dynamiquement.
    Function(String),
}

impl Value {
    pub fn is_truthy(&self) -> bool {
        match self {
            Value::Bool(b) => *b,
            Value::Null => false,
            Value::Int(n) => *n != 0,
            _ => true,
        }
    }

    pub fn display(&self) -> String {
        match self {
            Value::Int(n) => n.to_string(),
            Value::Float(f) => f.to_string(),
            Value::Str(s) => s.clone(),
            Value::Bool(b) => if *b { "marina".to_string() } else { "diso".to_string() },
            Value::Null => "tsisy".to_string(),
            Value::Struct(name, fields) => {
                let f = fields.borrow();
                let mut parts: Vec<String> = f.iter().map(|(k, v)| format!("{}: {}", k, v.display())).collect();
                parts.sort();
                format!("{} {{ {} }}", name, parts.join(", "))
            }
            Value::List(items) => {
                let parts: Vec<String> = items.borrow().iter().map(|v| v.display()).collect();
                format!("[{}]", parts.join(", "))
            }
            Value::Dict(map) => {
                let m = map.borrow();
                let mut parts: Vec<String> = m.iter().map(|(k, v)| {
                    let ks = match k { DictKey::Int(n) => n.to_string(), DictKey::Str(s) => s.clone() };
                    format!("{}: {}", ks, v.display())
                }).collect();
                parts.sort();
                format!("[{}]", parts.join(", "))
            }
            Value::Tuple(items) => {
                let parts: Vec<String> = items.iter().map(|v| v.display()).collect();
                format!("({})", parts.join(", "))
            }
            Value::Function(name) => format!("<asa {}>", name),
        }
    }
}

#[derive(Debug, Clone)]
pub enum OpCode {
    Const(usize),
    LoadGlobal(String),
    StoreGlobal(String),
    LoadLocal(usize),
    StoreLocal(usize),
    /// Pousse une référence de fonction/lambda par son nom interne.
    LoadFunction(String),

    Add, Sub, Mul, Div, Mod, Neg, Not,
    Eq, NotEq, Lt, Gt, LtEq, GtEq,

    Print,
    Pop,

    Jump(usize),
    JumpIfFalse(usize),
    JumpIfTrue(usize),
    /// Duplique la valeur en haut de pile (utilisé pour l'évaluation en court-circuit de sy/na).
    Dup,

    /// Appel d'une fonction libre, par nom (résolu à la compilation).
    Call(String, usize),
    /// Appel dynamique : la fonction à appeler est une valeur (Value::Function)
    /// déjà empilée sous les arguments (cas d'une lambda stockée dans une variable).
    CallValue(usize),
    /// Appel de méthode : le récepteur est le premier des `usize` arguments
    /// empilés ; son type dynamique détermine quelle fonction "Type_methode" appeler.
    CallMethod(String, usize),
    Return,

    /// Construit une struct à partir des valeurs empilées, dans l'ordre des noms de champ donnés.
    MakeStruct(String, Vec<String>),
    GetField(String),
    SetField(String),

    /// Construit une liste à partir des N valeurs empilées (dans l'ordre).
    MakeList(usize),
    /// Construit un dictionnaire à partir de N paires (clé, valeur) empilées alternées.
    MakeDict(usize),
    /// Construit un tuple à partir des N valeurs empilées (dans l'ordre).
    MakeTuple(usize),
    GetIndex,
    SetIndex,

    /// Marque le début d'un bloc 'andramo' : si une erreur survient avant le
    /// TryEnd correspondant, l'exécution reprend à l'IP donné (bloc 'sambotra'),
    /// avec le message d'erreur poussé sur la pile.
    TryStart(usize),
    TryEnd,
    /// Lève une erreur OLY récupérable à partir de la valeur (texte) en haut de pile.
    Throw,

    Halt,
}

#[derive(Debug, Clone, Default)]
pub struct Chunk {
    pub code: Vec<OpCode>,
    pub constants: Vec<Value>,
    /// Fichier + ligne source de chaque instruction (même longueur que `code`),
    /// utilisé pour situer une erreur d'exécution dans le bon fichier.
    pub locations: Vec<(String, usize)>,
}

impl Chunk {
    pub fn add_const(&mut self, v: Value) -> usize {
        self.constants.push(v);
        self.constants.len() - 1
    }

    /// Ajoute une instruction avec sa position source.
    pub fn push(&mut self, op: OpCode, file: &str, line: usize) {
        self.code.push(op);
        self.locations.push((file.to_string(), line));
    }
}

#[derive(Debug, Clone)]
pub struct FunctionProto {
    pub name: String,
    pub arity: usize,
    pub chunk: Chunk,
}

pub struct CompiledProgram {
    pub main: Chunk,
    pub functions: HashMap<String, FunctionProto>,
    /// Métadonnées des structs (noms de champs), pour un futur vérificateur de types.
    pub structs: HashMap<String, Vec<String>>,
}

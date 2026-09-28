//! Machine virtuelle OLY : exécute le bytecode produit par compiler.rs.
//! Chaque instruction connaît son fichier/ligne d'origine (Chunk::locations),
//! ce qui permet de rapporter une erreur d'exécution avec un contexte source
//! précis. Les blocs andramo/sambotra (try/catch) sont gérés dans `run` via
//! une pile de gestionnaires propre à chaque appel de fonction.

use crate::bytecode::{Chunk, DictKey, FunctionProto, OpCode, Value};
use crate::errors::OlyError;
use std::cell::RefCell;
use std::collections::HashMap;
use std::io::{self, BufRead};
use std::rc::Rc;

/// Erreur "brute" (sans position) levée par les opérations internes ;
/// convertie en OlyError au point d'exécution de l'instruction fautive.
type RResult<T> = Result<T, String>;

pub struct VM {
    pub globals: HashMap<String, Value>,
    pub functions: HashMap<String, FunctionProto>,
}

enum Step {
    Continue,
    Return(Value),
}

/// Gestionnaire d'erreur actif : où reprendre (bloc sambotra) et jusqu'à quelle
/// profondeur ramener la pile d'opérandes.
struct Handler {
    catch_ip: usize,
    stack_depth: usize,
}

impl VM {
    pub fn new(functions: HashMap<String, FunctionProto>) -> Self {
        VM { globals: HashMap::new(), functions }
    }

    pub fn run_main(&mut self, chunk: &Chunk) -> Result<(), OlyError> {
        self.run(chunk, Vec::new())?;
        Ok(())
    }

    fn run(&mut self, chunk: &Chunk, mut locals: Vec<Value>) -> Result<Value, OlyError> {
        let mut stack: Vec<Value> = Vec::new();
        let mut handlers: Vec<Handler> = Vec::new();
        let mut ip: usize = 0;

        loop {
            if ip >= chunk.code.len() {
                return Ok(Value::Null);
            }
            let instr = chunk.code[ip].clone();
            let (file, line) = chunk.locations.get(ip).cloned().unwrap_or_else(|| (String::new(), 0));
            ip += 1;

            // Gestion des blocs try/catch : manipule la pile de gestionnaires locale.
            match &instr {
                OpCode::TryStart(catch_ip) => {
                    handlers.push(Handler { catch_ip: *catch_ip, stack_depth: stack.len() });
                    continue;
                }
                OpCode::TryEnd => {
                    handlers.pop();
                    continue;
                }
                _ => {}
            }

            match self.exec_one(chunk, &instr, &mut stack, &mut locals, &mut ip, &file, line) {
                Ok(Step::Continue) => {}
                Ok(Step::Return(v)) => return Ok(v),
                Err(e) => match handlers.pop() {
                    Some(h) => {
                        // Erreur rattrapée : on ramène la pile, on pousse le message
                        // (lié à la variable de 'sambotra' par le code compilé) et on saute.
                        stack.truncate(h.stack_depth);
                        stack.push(Value::Str(e.message));
                        ip = h.catch_ip;
                    }
                    None => return Err(e),
                },
            }
        }
    }

    fn exec_one(
        &mut self,
        chunk: &Chunk,
        instr: &OpCode,
        stack: &mut Vec<Value>,
        locals: &mut Vec<Value>,
        ip: &mut usize,
        file: &str,
        line: usize,
    ) -> Result<Step, OlyError> {
        let at = |e: String| OlyError::new(file.to_string(), line, e);

        match instr.clone() {
            OpCode::Const(i) => stack.push(chunk.constants[i].clone()),

            OpCode::LoadGlobal(name) => {
                let v = self.globals.get(&name).cloned()
                    .ok_or_else(|| at(format!("Variable globale inconnue : '{}'", name)))?;
                stack.push(v);
            }
            OpCode::StoreGlobal(name) => {
                let v = pop(stack).map_err(at)?;
                self.globals.insert(name, v.clone());
                stack.push(v);
            }
            OpCode::LoadLocal(idx) => {
                stack.push(locals.get(idx).cloned().ok_or_else(|| at("Variable locale inconnue".to_string()))?);
            }
            OpCode::StoreLocal(idx) => {
                let v = pop(stack).map_err(at)?;
                if idx == locals.len() { locals.push(v.clone()); }
                else if idx > locals.len() { locals.resize(idx + 1, Value::Null); locals[idx] = v.clone(); }
                else { locals[idx] = v.clone(); }
                stack.push(v);
            }
            OpCode::LoadFunction(name) => stack.push(Value::Function(name)),
            OpCode::Pop => { pop(stack).map_err(at)?; }
            OpCode::Dup => {
                let v = stack.last().cloned().ok_or_else(|| at("Pile vide (Dup)".to_string()))?;
                stack.push(v);
            }

            OpCode::Add | OpCode::Sub | OpCode::Mul | OpCode::Div | OpCode::Mod
            | OpCode::Eq | OpCode::NotEq | OpCode::Lt | OpCode::Gt | OpCode::LtEq | OpCode::GtEq => {
                let b = pop(stack).map_err(at)?;
                let a = pop(stack).map_err(at)?;
                stack.push(binary_op(instr, a, b).map_err(at)?);
            }
            OpCode::Neg => {
                let v = pop(stack).map_err(at)?;
                stack.push(match v {
                    Value::Int(n) => Value::Int(-n),
                    Value::Float(f) => Value::Float(-f),
                    _ => return Err(at("'-' unaire attend un nombre".to_string())),
                });
            }
            OpCode::Not => {
                let v = pop(stack).map_err(at)?;
                stack.push(Value::Bool(!v.is_truthy()));
            }

            OpCode::Print => {
                let v = pop(stack).map_err(at)?;
                println!("{}", v.display());
            }

            OpCode::Jump(target) => { *ip = target; }
            OpCode::JumpIfFalse(target) => {
                let v = pop(stack).map_err(at)?;
                if !v.is_truthy() { *ip = target; }
            }
            OpCode::JumpIfTrue(target) => {
                let v = pop(stack).map_err(at)?;
                if v.is_truthy() { *ip = target; }
            }

            OpCode::Call(name, argc) => {
                let args = pop_n(stack, argc).map_err(at)?;
                match self.functions.get(&name).cloned() {
                    Some(proto) => {
                        let ret = self.run(&proto.chunk, args)?;
                        stack.push(ret);
                    }
                    None => {
                        let ret = call_builtin(&name, args).map_err(at)?;
                        stack.push(ret);
                    }
                }
            }
            OpCode::CallValue(argc) => {
                let args = pop_n(stack, argc).map_err(at)?;
                let callee = pop(stack).map_err(at)?;
                match callee {
                    Value::Function(name) => {
                        let proto = self.functions.get(&name).cloned()
                            .ok_or_else(|| at(format!("Fonction inconnue : '{}'", name)))?;
                        if proto.arity != args.len() {
                            return Err(at(format!("La fonction attend {} argument(s), {} fourni(s)", proto.arity, args.len())));
                        }
                        let ret = self.run(&proto.chunk, args)?;
                        stack.push(ret);
                    }
                    other => return Err(at(format!("Impossible d'appeler une valeur qui n'est pas une fonction : {}", other.display()))),
                }
            }
            OpCode::CallMethod(method, argc) => {
                let args = pop_n(stack, argc).map_err(at)?;
                let type_name = match args.first() {
                    Some(Value::Struct(n, _)) => n.clone(),
                    _ => return Err(at(format!("Appel de méthode '{}' sur une valeur qui n'est pas une struct", method))),
                };
                let mangled = format!("{}_{}", type_name, method);
                let proto = self.functions.get(&mangled).cloned()
                    .ok_or_else(|| at(format!("Méthode inconnue : '{}' sur '{}'", method, type_name)))?;
                let ret = self.run(&proto.chunk, args)?;
                stack.push(ret);
            }
            OpCode::Return => return Ok(Step::Return(pop(stack).unwrap_or(Value::Null))),

            OpCode::MakeStruct(name, field_names) => {
                let vals = pop_n(stack, field_names.len()).map_err(at)?;
                let mut map = HashMap::new();
                for (n, v) in field_names.into_iter().zip(vals.into_iter()) { map.insert(n, v); }
                stack.push(Value::Struct(name, Rc::new(RefCell::new(map))));
            }
            OpCode::GetField(field) => {
                let base = pop(stack).map_err(at)?;
                match &base {
                    Value::Struct(_, fields) => {
                        let v = fields.borrow().get(&field).cloned().unwrap_or(Value::Null);
                        stack.push(v);
                    }
                    _ => return Err(at(format!("Accès au champ '{}' sur une valeur qui n'est pas une struct", field))),
                }
            }
            OpCode::SetField(field) => {
                let value = pop(stack).map_err(at)?;
                let base = pop(stack).map_err(at)?;
                match &base {
                    Value::Struct(_, fields) => { fields.borrow_mut().insert(field, value.clone()); }
                    _ => return Err(at(format!("Affectation au champ '{}' sur une valeur qui n'est pas une struct", field))),
                }
                stack.push(value);
            }

            OpCode::MakeList(n) => {
                let items = pop_n(stack, n).map_err(at)?;
                stack.push(Value::List(Rc::new(RefCell::new(items))));
            }
            OpCode::MakeDict(n) => {
                let flat = pop_n(stack, n * 2).map_err(at)?;
                let mut map = HashMap::new();
                let mut it = flat.into_iter();
                while let (Some(k), Some(v)) = (it.next(), it.next()) {
                    map.insert(DictKey::from_value(&k).map_err(at)?, v);
                }
                stack.push(Value::Dict(Rc::new(RefCell::new(map))));
            }
            OpCode::MakeTuple(n) => {
                let items = pop_n(stack, n).map_err(at)?;
                stack.push(Value::Tuple(Rc::new(items)));
            }
            OpCode::GetIndex => {
                let idx = pop(stack).map_err(at)?;
                let base = pop(stack).map_err(at)?;
                match &base {
                    Value::List(items) => {
                        let i = index_as_usize(&idx).map_err(at)?;
                        let items = items.borrow();
                        let v = items.get(i).cloned()
                            .ok_or_else(|| at(format!("Index {} hors limites (taille {})", i, items.len())))?;
                        stack.push(v);
                    }
                    Value::Tuple(items) => {
                        let i = index_as_usize(&idx).map_err(at)?;
                        let v = items.get(i).cloned()
                            .ok_or_else(|| at(format!("Index {} hors limites (taille {})", i, items.len())))?;
                        stack.push(v);
                    }
                    Value::Dict(map) => {
                        let key = DictKey::from_value(&idx).map_err(at)?;
                        let v = map.borrow().get(&key).cloned()
                            .ok_or_else(|| at(format!("Clé introuvable dans le dictionnaire : {}", idx.display())))?;
                        stack.push(v);
                    }
                    Value::Str(s) => {
                        let i = index_as_usize(&idx).map_err(at)?;
                        let c = s.chars().nth(i)
                            .ok_or_else(|| at(format!("Index {} hors limites (taille {})", i, s.chars().count())))?;
                        stack.push(Value::Str(c.to_string()));
                    }
                    _ => return Err(at("Indexation '[...]' sur une valeur non indexable".to_string())),
                }
            }
            OpCode::SetIndex => {
                let value = pop(stack).map_err(at)?;
                let idx = pop(stack).map_err(at)?;
                let base = pop(stack).map_err(at)?;
                match &base {
                    Value::List(items) => {
                        let i = index_as_usize(&idx).map_err(at)?;
                        let mut items = items.borrow_mut();
                        let len = items.len();
                        let slot = items.get_mut(i).ok_or_else(|| at(format!("Index {} hors limites (taille {})", i, len)))?;
                        *slot = value.clone();
                    }
                    Value::Dict(map) => {
                        let key = DictKey::from_value(&idx).map_err(at)?;
                        map.borrow_mut().insert(key, value.clone());
                    }
                    Value::Tuple(_) => return Err(at("Un tuple est immuable : affectation indexée impossible".to_string())),
                    _ => return Err(at("Affectation indexée '[...]' sur une valeur non modifiable".to_string())),
                }
                stack.push(value);
            }

            OpCode::Throw => {
                let v = pop(stack).map_err(at)?;
                return Err(at(match v {
                    Value::Str(s) => s,
                    other => other.display(),
                }));
            }
            OpCode::TryStart(_) | OpCode::TryEnd => unreachable!("traités dans run()"),

            OpCode::Halt => return Ok(Step::Return(Value::Null)),
        }
        Ok(Step::Continue)
    }
}

fn pop(stack: &mut Vec<Value>) -> RResult<Value> {
    stack.pop().ok_or_else(|| "Pile vide (erreur interne du compilateur OLY)".to_string())
}

fn pop_n(stack: &mut Vec<Value>, n: usize) -> RResult<Vec<Value>> {
    let mut v = Vec::with_capacity(n);
    for _ in 0..n { v.push(pop(stack)?); }
    v.reverse();
    Ok(v)
}

fn index_as_usize(v: &Value) -> RResult<usize> {
    match v {
        Value::Int(n) if *n >= 0 => Ok(*n as usize),
        Value::Int(n) => Err(format!("Index négatif invalide : {}", n)),
        other => Err(format!("Un index doit être un entier ('isa'), trouvé {:?}", other)),
    }
}

fn as_f64(v: &Value) -> Option<f64> {
    match v { Value::Int(n) => Some(*n as f64), Value::Float(f) => Some(*f), _ => None }
}

fn expect_arity(name: &str, args: &[Value], n: usize) -> RResult<()> {
    if args.len() != n {
        return Err(format!("'{}' attend {} argument(s), {} fourni(s)", name, n, args.len()));
    }
    Ok(())
}

fn expect_str<'a>(name: &str, v: &'a Value) -> RResult<&'a String> {
    match v { Value::Str(s) => Ok(s), _ => Err(format!("'{}' attend une chaîne ('teny')", name)) }
}

/// Fonctions natives de la VM, appelées quand aucune fonction/méthode utilisateur
/// ne porte ce nom.
///  Collections : habeny (taille), ampio (ajouter), esory (retirer dernier),
///                misy (contient ?), fanalahidy (clés d'un dictionnaire)
///  Maths       : faka (racine carrée), heriny (puissance), habe (valeur absolue),
///                kely (minimum), lehibe (maximum)
///  Chaînes     : avo (majuscules), ambany (minuscules), fafao (trim),
///                vaky (split), akambana (join)
///  Entrée      : henoy (lit une ligne au clavier)
fn call_builtin(name: &str, mut args: Vec<Value>) -> RResult<Value> {
    match name {
        "habeny" => {
            expect_arity(name, &args, 1)?;
            match &args[0] {
                Value::List(items) => Ok(Value::Int(items.borrow().len() as i64)),
                Value::Tuple(items) => Ok(Value::Int(items.len() as i64)),
                Value::Dict(map) => Ok(Value::Int(map.borrow().len() as i64)),
                Value::Str(s) => Ok(Value::Int(s.chars().count() as i64)),
                _ => Err("'habeny' attend une liste, un tuple, un dictionnaire ou une chaîne".to_string()),
            }
        }
        "ampio" => {
            expect_arity(name, &args, 2)?;
            let value = args.pop().unwrap();
            match &args[0] {
                Value::List(items) => { items.borrow_mut().push(value); Ok(args[0].clone()) }
                _ => Err("'ampio' attend une liste comme premier argument".to_string()),
            }
        }
        "esory" => {
            expect_arity(name, &args, 1)?;
            match &args[0] {
                Value::List(items) => Ok(items.borrow_mut().pop().unwrap_or(Value::Null)),
                _ => Err("'esory' attend une liste".to_string()),
            }
        }
        "misy" => {
            expect_arity(name, &args, 2)?;
            match &args[0] {
                Value::Dict(map) => {
                    let key = DictKey::from_value(&args[1])?;
                    Ok(Value::Bool(map.borrow().contains_key(&key)))
                }
                Value::List(items) => Ok(Value::Bool(items.borrow().iter().any(|v| values_equal(v, &args[1])))),
                Value::Tuple(items) => Ok(Value::Bool(items.iter().any(|v| values_equal(v, &args[1])))),
                Value::Str(s) => Ok(Value::Bool(s.contains(expect_str(name, &args[1])?.as_str()))),
                _ => Err("'misy' attend une liste, un tuple, un dictionnaire ou une chaîne".to_string()),
            }
        }
        "fanalahidy" => {
            expect_arity(name, &args, 1)?;
            match &args[0] {
                Value::Dict(map) => {
                    let mut keys: Vec<DictKey> = map.borrow().keys().cloned().collect();
                    keys.sort_by(|a, b| match (a, b) {
                        (DictKey::Int(x), DictKey::Int(y)) => x.cmp(y),
                        (DictKey::Str(x), DictKey::Str(y)) => x.cmp(y),
                        (DictKey::Int(_), DictKey::Str(_)) => std::cmp::Ordering::Less,
                        (DictKey::Str(_), DictKey::Int(_)) => std::cmp::Ordering::Greater,
                    });
                    Ok(Value::List(Rc::new(RefCell::new(keys.into_iter().map(DictKey::into_value).collect()))))
                }
                _ => Err("'fanalahidy' attend un dictionnaire".to_string()),
            }
        }

        "faka" => {
            expect_arity(name, &args, 1)?;
            let x = as_f64(&args[0]).ok_or("'faka' attend un nombre")?;
            if x < 0.0 { return Err("'faka' : racine carrée d'un nombre négatif".to_string()); }
            Ok(Value::Float(x.sqrt()))
        }
        "heriny" => {
            expect_arity(name, &args, 2)?;
            let x = as_f64(&args[0]).ok_or("'heriny' attend des nombres")?;
            let y = as_f64(&args[1]).ok_or("'heriny' attend des nombres")?;
            Ok(Value::Float(x.powf(y)))
        }
        "habe" => {
            expect_arity(name, &args, 1)?;
            match &args[0] {
                Value::Int(n) => Ok(Value::Int(n.abs())),
                Value::Float(f) => Ok(Value::Float(f.abs())),
                _ => Err("'habe' attend un nombre".to_string()),
            }
        }
        "kely" | "lehibe" => {
            expect_arity(name, &args, 2)?;
            let x = as_f64(&args[0]).ok_or_else(|| format!("'{}' attend des nombres", name))?;
            let y = as_f64(&args[1]).ok_or_else(|| format!("'{}' attend des nombres", name))?;
            let first_wins = if name == "kely" { x <= y } else { x >= y };
            Ok(if first_wins { args[0].clone() } else { args[1].clone() })
        }

        "avo" => { expect_arity(name, &args, 1)?; Ok(Value::Str(expect_str(name, &args[0])?.to_uppercase())) }
        "ambany" => { expect_arity(name, &args, 1)?; Ok(Value::Str(expect_str(name, &args[0])?.to_lowercase())) }
        "fafao" => { expect_arity(name, &args, 1)?; Ok(Value::Str(expect_str(name, &args[0])?.trim().to_string())) }
        "vaky" => {
            expect_arity(name, &args, 2)?;
            let s = expect_str(name, &args[0])?;
            let sep = expect_str(name, &args[1])?;
            let parts: Vec<Value> = if sep.is_empty() {
                s.chars().map(|c| Value::Str(c.to_string())).collect()
            } else {
                s.split(sep.as_str()).map(|p| Value::Str(p.to_string())).collect()
            };
            Ok(Value::List(Rc::new(RefCell::new(parts))))
        }
        "akambana" => {
            expect_arity(name, &args, 2)?;
            let sep = expect_str(name, &args[1])?;
            match &args[0] {
                Value::List(items) => Ok(Value::Str(items.borrow().iter().map(|v| v.display()).collect::<Vec<_>>().join(sep))),
                Value::Tuple(items) => Ok(Value::Str(items.iter().map(|v| v.display()).collect::<Vec<_>>().join(sep))),
                _ => Err("'akambana' attend une liste ou un tuple".to_string()),
            }
        }

        "henoy" => {
            expect_arity(name, &args, 0)?;
            let mut line = String::new();
            io::stdin().lock().read_line(&mut line).map_err(|e| format!("Lecture impossible : {}", e))?;
            Ok(Value::Str(line.trim_end_matches(&['\n', '\r'][..]).to_string()))
        }

        _ => Err(format!("Fonction inconnue : '{}'", name)),
    }
}

fn binary_op(op: &OpCode, a: Value, b: Value) -> RResult<Value> {
    use Value::*;

    match op {
        OpCode::Add => match (&a, &b) {
            (Int(x), Int(y)) => Ok(Int(x + y)),
            (Str(x), Str(y)) => Ok(Str(format!("{}{}", x, y))),
            (Str(x), other) => Ok(Str(format!("{}{}", x, other.display()))),
            _ => numeric2(op_name(op), a, b, |x, y| x + y),
        },
        OpCode::Sub => numeric2(op_name(op), a, b, |x, y| x - y),
        OpCode::Mul => numeric2(op_name(op), a, b, |x, y| x * y),
        OpCode::Div => {
            if matches!(as_f64(&b), Some(d) if d == 0.0) { return Err("Division par zéro".to_string()); }
            numeric2(op_name(op), a, b, |x, y| x / y)
        }
        OpCode::Mod => {
            if matches!(as_f64(&b), Some(d) if d == 0.0) { return Err("Modulo par zéro".to_string()); }
            match (&a, &b) {
                (Int(x), Int(y)) => Ok(Int(x % y)),
                _ => numeric2(op_name(op), a, b, |x, y| x % y),
            }
        }
        OpCode::Eq => Ok(Bool(values_equal(&a, &b))),
        OpCode::NotEq => Ok(Bool(!values_equal(&a, &b))),
        OpCode::Lt => compare(a, b, |x, y| x < y),
        OpCode::Gt => compare(a, b, |x, y| x > y),
        OpCode::LtEq => compare(a, b, |x, y| x <= y),
        OpCode::GtEq => compare(a, b, |x, y| x >= y),
        _ => unreachable!(),
    }
}

fn op_name(op: &OpCode) -> &'static str {
    match op {
        OpCode::Add => "+", OpCode::Sub => "-", OpCode::Mul => "*", OpCode::Div => "/", OpCode::Mod => "%",
        _ => "?",
    }
}

fn numeric2(op: &str, a: Value, b: Value, f: impl Fn(f64, f64) -> f64) -> RResult<Value> {
    match (as_f64(&a), as_f64(&b)) {
        (Some(x), Some(y)) => {
            if matches!(a, Value::Int(_)) && matches!(b, Value::Int(_)) {
                Ok(Value::Int(f(x, y) as i64))
            } else {
                Ok(Value::Float(f(x, y)))
            }
        }
        _ => Err(format!("Opérande invalide pour '{}' : {:?} / {:?}", op, a, b)),
    }
}

fn compare(a: Value, b: Value, f: impl Fn(f64, f64) -> bool) -> RResult<Value> {
    match (as_f64(&a), as_f64(&b)) {
        (Some(x), Some(y)) => Ok(Value::Bool(f(x, y))),
        _ => Err(format!("Comparaison invalide entre {:?} et {:?}", a, b)),
    }
}

fn values_equal(a: &Value, b: &Value) -> bool {
    match (a, b) {
        (Value::Int(x), Value::Int(y)) => x == y,
        (Value::Float(x), Value::Float(y)) => x == y,
        (Value::Int(x), Value::Float(y)) | (Value::Float(y), Value::Int(x)) => *x as f64 == *y,
        (Value::Str(x), Value::Str(y)) => x == y,
        (Value::Bool(x), Value::Bool(y)) => x == y,
        (Value::Null, Value::Null) => true,
        _ => false,
    }
}

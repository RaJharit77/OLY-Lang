//! Compile l'AST (ast.rs) en bytecode (bytecode.rs).
//! Résout les variables (locales/globales) et pose/rebouche les sauts
//! pour if/sinon-si/sinon, tant-que, isaky, tapaka (break), manohy (continue),
//! andramo/sambotra (try/catch). Chaque instruction émise est étiquetée avec
//! le fichier/ligne de la statement source qui l'a produite (Chunk::push).

use crate::ast::*;
use crate::bytecode::{Chunk, CompiledProgram, FunctionProto, OpCode, Value};
use crate::errors::OlyError;
use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::rc::Rc;

type CResult<T> = Result<T, OlyError>;

/// Fonctions natives de la VM (voir vm.rs::call_builtin) : appelées statiquement par nom.
const BUILTINS: &[&str] = &[
    "habeny", "ampio", "esory", "misy", "fanalahidy",
    "faka", "heriny", "habe", "kely", "lehibe",
    "avo", "ambany", "fafao", "vaky", "akambana", "henoy",
];

#[derive(Default, Clone)]
struct LoopFrame {
    break_jumps: Vec<usize>,
    continue_jumps: Vec<usize>,
}

struct FnCtx {
    is_local: bool,
    locals: Vec<String>,
    loop_stack: Vec<LoopFrame>,
    file: String,
    line: usize,
    /// Noms des fonctions déclarées avec 'asa NOM(...)' au niveau global : permet
    /// de distinguer un appel statique ("Call") d'un appel dynamique via une
    /// variable contenant une lambda ("CallValue").
    known_functions: Rc<HashSet<String>>,
    /// Compteur partagé pour générer des noms uniques (lambdas, variables
    /// temporaires du for-each), à travers toutes les portées de compilation.
    fresh_counter: Rc<RefCell<usize>>,
}

impl FnCtx {
    fn global(known_functions: Rc<HashSet<String>>, fresh_counter: Rc<RefCell<usize>>) -> Self {
        FnCtx { is_local: false, locals: Vec::new(), loop_stack: Vec::new(), file: String::new(), line: 0, known_functions, fresh_counter }
    }
    fn local(params: &[Param], known_functions: Rc<HashSet<String>>, fresh_counter: Rc<RefCell<usize>>) -> Self {
        FnCtx {
            is_local: true,
            locals: params.iter().map(|p| p.name.clone()).collect(),
            loop_stack: Vec::new(), file: String::new(), line: 0,
            known_functions, fresh_counter,
        }
    }
    fn fresh_name(&self, prefix: &str) -> String {
        let mut c = self.fresh_counter.borrow_mut();
        *c += 1;
        format!("__{}_{}", prefix, *c)
    }
}

pub fn compile_program(program: &Program) -> CResult<CompiledProgram> {
    let mut functions: HashMap<String, FunctionProto> = HashMap::new();
    let mut structs: HashMap<String, Vec<String>> = HashMap::new();

    let mut known = HashSet::new();
    for b in BUILTINS { known.insert(b.to_string()); }
    for item in &program.items {
        if let StmtKind::FunctionDecl(f) = &item.kind {
            known.insert(f.name.clone());
        }
    }
    let known_functions = Rc::new(known);
    let fresh_counter = Rc::new(RefCell::new(0usize));

    // Passe 1 : déclarations (fonctions, structs, méthodes), pour que l'ordre
    // d'écriture dans le fichier n'importe pas (appel avant définition autorisé).
    for item in &program.items {
        match &item.kind {
            StmtKind::FunctionDecl(f) => {
                compile_function(&f.name, f, &mut functions, known_functions.clone(), fresh_counter.clone())?;
            }
            StmtKind::StructDecl(s) => {
                structs.insert(s.name.clone(), s.fields.iter().map(|p| p.name.clone()).collect());
            }
            StmtKind::ImplDecl(i) => {
                for m in &i.methods {
                    let mangled = format!("{}_{}", i.type_name, m.name);
                    compile_function(&mangled, m, &mut functions, known_functions.clone(), fresh_counter.clone())?;
                }
            }
            _ => {}
        }
    }

    // Passe 2 : instructions de niveau global (variables, expressions, contrôle de flux).
    let mut main_chunk = Chunk::default();
    let mut ctx = FnCtx::global(known_functions, fresh_counter);
    for item in &program.items {
        match &item.kind {
            StmtKind::FunctionDecl(_) | StmtKind::StructDecl(_) | StmtKind::ImplDecl(_) => {}
            StmtKind::Import(_) => {}
            _ => compile_stmt(item, &mut main_chunk, &mut ctx, &mut functions)?,
        }
    }
    ctx.line = 0;
    main_chunk.push(OpCode::Halt, &ctx.file, ctx.line);

    Ok(CompiledProgram { main: main_chunk, functions, structs })
}

fn compile_function(
    name: &str, f: &FunctionDecl,
    functions: &mut HashMap<String, FunctionProto>,
    known_functions: Rc<HashSet<String>>, fresh_counter: Rc<RefCell<usize>>,
) -> CResult<()> {
    let mut chunk = Chunk::default();
    let mut ctx = FnCtx::local(&f.params, known_functions, fresh_counter);
    for stmt in &f.body {
        compile_stmt(stmt, &mut chunk, &mut ctx, functions)?;
    }
    let idx = chunk.add_const(Value::Null);
    chunk.push(OpCode::Const(idx), &ctx.file, ctx.line);
    chunk.push(OpCode::Return, &ctx.file, ctx.line);
    functions.insert(name.to_string(), FunctionProto { name: name.to_string(), arity: f.params.len(), chunk });
    Ok(())
}

// ---------- Aides pour les sauts ----------

fn emit_jump(chunk: &mut Chunk, ctx: &FnCtx) -> usize {
    chunk.push(OpCode::Jump(0), &ctx.file, ctx.line);
    chunk.code.len() - 1
}
fn emit_jump_if_false(chunk: &mut Chunk, ctx: &FnCtx) -> usize {
    chunk.push(OpCode::JumpIfFalse(0), &ctx.file, ctx.line);
    chunk.code.len() - 1
}
fn emit_jump_if_true(chunk: &mut Chunk, ctx: &FnCtx) -> usize {
    chunk.push(OpCode::JumpIfTrue(0), &ctx.file, ctx.line);
    chunk.code.len() - 1
}
fn patch_jump_to(chunk: &mut Chunk, idx: usize, target: usize) {
    match &mut chunk.code[idx] {
        OpCode::Jump(t) | OpCode::JumpIfFalse(t) | OpCode::JumpIfTrue(t) | OpCode::TryStart(t) => *t = target,
        _ => unreachable!("patch_jump_to sur une instruction non-saut"),
    }
}
fn patch_jump_here(chunk: &mut Chunk, idx: usize) {
    let target = chunk.code.len();
    patch_jump_to(chunk, idx, target);
}

// ---------- Variables ----------

fn emit_load(name: &str, chunk: &mut Chunk, ctx: &FnCtx) {
    if ctx.is_local {
        if let Some(idx) = ctx.locals.iter().position(|n| n == name) {
            chunk.push(OpCode::LoadLocal(idx), &ctx.file, ctx.line);
            return;
        }
    }
    chunk.push(OpCode::LoadGlobal(name.to_string()), &ctx.file, ctx.line);
}

/// Émet le code pour stocker la valeur en haut de pile dans `name`
/// (locale si on est dans une fonction et que la variable y est connue/déclarée, sinon globale).
/// La valeur stockée reste en haut de pile (sémantique d'expression pour '=').
fn emit_store(name: &str, chunk: &mut Chunk, ctx: &mut FnCtx, declare: bool) {
    if ctx.is_local {
        if let Some(idx) = ctx.locals.iter().position(|n| n == name) {
            let op = if declare { OpCode::DeclareLocal(idx) } else { OpCode::StoreLocal(idx) };
            chunk.push(op, &ctx.file, ctx.line);
            return;
        }
        if declare {
            ctx.locals.push(name.to_string());
            let idx = ctx.locals.len() - 1;
            chunk.push(OpCode::DeclareLocal(idx), &ctx.file, ctx.line);
            return;
        }
    }
    chunk.push(OpCode::StoreGlobal(name.to_string()), &ctx.file, ctx.line);
}

// ---------- Instructions ----------

fn compile_stmt(stmt: &Stmt, chunk: &mut Chunk, ctx: &mut FnCtx, functions: &mut HashMap<String, FunctionProto>) -> CResult<()> {
    ctx.file = stmt.file.clone();
    ctx.line = stmt.line;
    match &stmt.kind {
        StmtKind::VarDecl { name, value, .. } => {
            compile_expr(value, chunk, ctx, functions)?;
            emit_store(name, chunk, ctx, true);
            chunk.push(OpCode::Pop, &ctx.file, ctx.line);
        }
        StmtKind::ExprStmt(e) => {
            compile_expr(e, chunk, ctx, functions)?;
            chunk.push(OpCode::Pop, &ctx.file, ctx.line);
        }
        StmtKind::Print(e) => {
            compile_expr(e, chunk, ctx, functions)?;
            chunk.push(OpCode::Print, &ctx.file, ctx.line);
        }
        StmtKind::If { cond, then_branch, else_ifs, else_branch } => {
            compile_expr(cond, chunk, ctx, functions)?;
            let jf = emit_jump_if_false(chunk, ctx);
            for s in then_branch { compile_stmt(s, chunk, ctx, functions)?; }
            let mut end_jumps = vec![emit_jump(chunk, ctx)];
            patch_jump_here(chunk, jf);

            for (c, b) in else_ifs {
                compile_expr(c, chunk, ctx, functions)?;
                let jf2 = emit_jump_if_false(chunk, ctx);
                for s in b { compile_stmt(s, chunk, ctx, functions)?; }
                end_jumps.push(emit_jump(chunk, ctx));
                patch_jump_here(chunk, jf2);
            }
            if let Some(b) = else_branch {
                for s in b { compile_stmt(s, chunk, ctx, functions)?; }
            }
            for j in end_jumps { patch_jump_here(chunk, j); }
        }
        StmtKind::While { cond, body } => {
            let loop_start = chunk.code.len();
            compile_expr(cond, chunk, ctx, functions)?;
            let jf = emit_jump_if_false(chunk, ctx);
            ctx.loop_stack.push(LoopFrame::default());
            for s in body { compile_stmt(s, chunk, ctx, functions)?; }
            let frame = ctx.loop_stack.last().unwrap().clone();
            for cj in &frame.continue_jumps { patch_jump_to(chunk, *cj, loop_start); }
            chunk.push(OpCode::Jump(loop_start), &ctx.file, ctx.line);
            patch_jump_here(chunk, jf);
            let frame = ctx.loop_stack.pop().unwrap();
            for bj in frame.break_jumps { patch_jump_here(chunk, bj); }
        }
        StmtKind::For { init, cond, update, body } => {
            compile_stmt(init, chunk, ctx, functions)?;
            let loop_start = chunk.code.len();
            compile_expr(cond, chunk, ctx, functions)?;
            let jf = emit_jump_if_false(chunk, ctx);
            ctx.loop_stack.push(LoopFrame::default());
            for s in body { compile_stmt(s, chunk, ctx, functions)?; }
            let update_start = chunk.code.len();
            compile_stmt(update, chunk, ctx, functions)?;
            let frame = ctx.loop_stack.last().unwrap().clone();
            for cj in &frame.continue_jumps { patch_jump_to(chunk, *cj, update_start); }
            chunk.push(OpCode::Jump(loop_start), &ctx.file, ctx.line);
            patch_jump_here(chunk, jf);
            let frame = ctx.loop_stack.pop().unwrap();
            for bj in frame.break_jumps { patch_jump_here(chunk, bj); }
        }
        StmtKind::ForEach { var, iterable, body } => {
            // Désucré en boucle indexée équivalente : aoka __iter = iterable; aoka __idx = 0;
            // raha mbola __idx < habeny(__iter) { aoka var = __iter[__idx]; corps; __idx = __idx + 1; }
            let iter_name = ctx.fresh_name("iter");
            let idx_name = ctx.fresh_name("idx");

            compile_expr(iterable, chunk, ctx, functions)?;
            emit_store(&iter_name, chunk, ctx, true);
            chunk.push(OpCode::Pop, &ctx.file, ctx.line);

            let zero = chunk.add_const(Value::Int(0));
            chunk.push(OpCode::Const(zero), &ctx.file, ctx.line);
            emit_store(&idx_name, chunk, ctx, true);
            chunk.push(OpCode::Pop, &ctx.file, ctx.line);

            let loop_start = chunk.code.len();
            emit_load(&idx_name, chunk, ctx);
            emit_load(&iter_name, chunk, ctx);
            chunk.push(OpCode::Call("habeny".to_string(), 1), &ctx.file, ctx.line);
            chunk.push(OpCode::Lt, &ctx.file, ctx.line);
            let jf = emit_jump_if_false(chunk, ctx);

            ctx.loop_stack.push(LoopFrame::default());

            emit_load(&iter_name, chunk, ctx);
            emit_load(&idx_name, chunk, ctx);
            chunk.push(OpCode::GetIndex, &ctx.file, ctx.line);
            emit_store(var, chunk, ctx, true);
            chunk.push(OpCode::Pop, &ctx.file, ctx.line);

            for s in body { compile_stmt(s, chunk, ctx, functions)?; }

            let update_start = chunk.code.len();
            emit_load(&idx_name, chunk, ctx);
            let one = chunk.add_const(Value::Int(1));
            chunk.push(OpCode::Const(one), &ctx.file, ctx.line);
            chunk.push(OpCode::Add, &ctx.file, ctx.line);
            emit_store(&idx_name, chunk, ctx, false);
            chunk.push(OpCode::Pop, &ctx.file, ctx.line);

            let frame = ctx.loop_stack.last().unwrap().clone();
            for cj in &frame.continue_jumps { patch_jump_to(chunk, *cj, update_start); }
            chunk.push(OpCode::Jump(loop_start), &ctx.file, ctx.line);
            patch_jump_here(chunk, jf);
            let frame = ctx.loop_stack.pop().unwrap();
            for bj in frame.break_jumps { patch_jump_here(chunk, bj); }
        }
        StmtKind::Return(e) => {
            match e {
                Some(expr) => compile_expr(expr, chunk, ctx, functions)?,
                None => { let idx = chunk.add_const(Value::Null); chunk.push(OpCode::Const(idx), &ctx.file, ctx.line); }
            }
            chunk.push(OpCode::Return, &ctx.file, ctx.line);
        }
        StmtKind::Break => {
            let j = emit_jump(chunk, ctx);
            ctx.loop_stack.last_mut()
                .ok_or_else(|| OlyError::new(stmt.file.clone(), stmt.line, "'tapaka' en dehors d'une boucle"))?
                .break_jumps.push(j);
        }
        StmtKind::Continue => {
            let j = emit_jump(chunk, ctx);
            ctx.loop_stack.last_mut()
                .ok_or_else(|| OlyError::new(stmt.file.clone(), stmt.line, "'manohy' en dehors d'une boucle"))?
                .continue_jumps.push(j);
        }
        StmtKind::TryCatch { try_block, catch_var, catch_block } => {
            let handler = { chunk.push(OpCode::TryStart(0), &ctx.file, ctx.line); chunk.code.len() - 1 };
            for s in try_block { compile_stmt(s, chunk, ctx, functions)?; }
            chunk.push(OpCode::TryEnd, &ctx.file, ctx.line);
            let skip_catch = emit_jump(chunk, ctx);
            patch_jump_here(chunk, handler);
            // La VM pousse le message d'erreur (Teny) juste avant de sauter ici.
            emit_store(catch_var, chunk, ctx, true);
            chunk.push(OpCode::Pop, &ctx.file, ctx.line);
            for s in catch_block { compile_stmt(s, chunk, ctx, functions)?; }
            patch_jump_here(chunk, skip_catch);
        }
        StmtKind::Throw(e) => {
            compile_expr(e, chunk, ctx, functions)?;
            chunk.push(OpCode::Throw, &ctx.file, ctx.line);
        }
        StmtKind::Import(_) => {}
        StmtKind::FunctionDecl(_) | StmtKind::StructDecl(_) | StmtKind::ImplDecl(_) => {
            return Err(OlyError::new(
                stmt.file.clone(), stmt.line,
                "Les déclarations 'asa'/'endrika'/'fomba' imbriquées ne sont pas encore supportées",
            ));
        }
    }
    Ok(())
}

fn compile_expr(expr: &Expr, chunk: &mut Chunk, ctx: &mut FnCtx, functions: &mut HashMap<String, FunctionProto>) -> CResult<()> {
    match expr {
        Expr::IntLit(n) => { let i = chunk.add_const(Value::Int(*n)); chunk.push(OpCode::Const(i), &ctx.file, ctx.line); }
        Expr::FloatLit(f) => { let i = chunk.add_const(Value::Float(*f)); chunk.push(OpCode::Const(i), &ctx.file, ctx.line); }
        Expr::StrLit(s) => { let i = chunk.add_const(Value::Str(s.clone())); chunk.push(OpCode::Const(i), &ctx.file, ctx.line); }
        Expr::BoolLit(b) => { let i = chunk.add_const(Value::Bool(*b)); chunk.push(OpCode::Const(i), &ctx.file, ctx.line); }

        Expr::Ident(name) => emit_load(name, chunk, ctx),

        Expr::Unary(op, e) => {
            compile_expr(e, chunk, ctx, functions)?;
            chunk.push(match op { UnaryOp::Neg => OpCode::Neg, UnaryOp::Not => OpCode::Not }, &ctx.file, ctx.line);
        }

        Expr::Ternary(cond, a, b) => {
            compile_expr(cond, chunk, ctx, functions)?;
            let jf = emit_jump_if_false(chunk, ctx);
            compile_expr(a, chunk, ctx, functions)?;
            let end = emit_jump(chunk, ctx);
            patch_jump_here(chunk, jf);
            compile_expr(b, chunk, ctx, functions)?;
            patch_jump_here(chunk, end);
        }

        Expr::Binary(l, op, r) => match op {
            BinOp::And => {
                compile_expr(l, chunk, ctx, functions)?;
                chunk.push(OpCode::Dup, &ctx.file, ctx.line);
                let short = emit_jump_if_false(chunk, ctx);
                chunk.push(OpCode::Pop, &ctx.file, ctx.line);
                compile_expr(r, chunk, ctx, functions)?;
                patch_jump_here(chunk, short);
            }
            BinOp::Or => {
                compile_expr(l, chunk, ctx, functions)?;
                chunk.push(OpCode::Dup, &ctx.file, ctx.line);
                let short = emit_jump_if_true(chunk, ctx);
                chunk.push(OpCode::Pop, &ctx.file, ctx.line);
                compile_expr(r, chunk, ctx, functions)?;
                patch_jump_here(chunk, short);
            }
            _ => {
                compile_expr(l, chunk, ctx, functions)?;
                compile_expr(r, chunk, ctx, functions)?;
                let opcode = match op {
                    BinOp::Add => OpCode::Add, BinOp::Sub => OpCode::Sub, BinOp::Mul => OpCode::Mul,
                    BinOp::Div => OpCode::Div, BinOp::Mod => OpCode::Mod,
                    BinOp::Eq => OpCode::Eq, BinOp::NotEq => OpCode::NotEq,
                    BinOp::Lt => OpCode::Lt, BinOp::Gt => OpCode::Gt,
                    BinOp::LtEq => OpCode::LtEq, BinOp::GtEq => OpCode::GtEq,
                    BinOp::And | BinOp::Or => unreachable!("traités au-dessus (court-circuit)"),
                };
                chunk.push(opcode, &ctx.file, ctx.line);
            }
        },

        Expr::Assign(target, value) => match target.as_ref() {
            Expr::Ident(name) => {
                compile_expr(value, chunk, ctx, functions)?;
                emit_store(name, chunk, ctx, false);
            }
            Expr::FieldAccess(base, field) => {
                compile_expr(base, chunk, ctx, functions)?;
                compile_expr(value, chunk, ctx, functions)?;
                chunk.push(OpCode::SetField(field.clone()), &ctx.file, ctx.line);
            }
            Expr::Index(base, idx) => {
                compile_expr(base, chunk, ctx, functions)?;
                compile_expr(idx, chunk, ctx, functions)?;
                compile_expr(value, chunk, ctx, functions)?;
                chunk.push(OpCode::SetIndex, &ctx.file, ctx.line);
            }
            _ => return Err(OlyError::new(ctx.file.clone(), ctx.line, "Cible d'affectation invalide (variable, champ ou index attendu)")),
        },

        Expr::FieldAccess(base, field) => {
            compile_expr(base, chunk, ctx, functions)?;
            chunk.push(OpCode::GetField(field.clone()), &ctx.file, ctx.line);
        }

        Expr::ArrayLit(elems) => {
            for e in elems { compile_expr(e, chunk, ctx, functions)?; }
            chunk.push(OpCode::MakeList(elems.len()), &ctx.file, ctx.line);
        }

        Expr::DictLit(pairs) => {
            for (k, v) in pairs {
                compile_expr(k, chunk, ctx, functions)?;
                compile_expr(v, chunk, ctx, functions)?;
            }
            chunk.push(OpCode::MakeDict(pairs.len()), &ctx.file, ctx.line);
        }

        Expr::TupleLit(elems) => {
            for e in elems { compile_expr(e, chunk, ctx, functions)?; }
            chunk.push(OpCode::MakeTuple(elems.len()), &ctx.file, ctx.line);
        }

        Expr::Index(base, idx) => {
            compile_expr(base, chunk, ctx, functions)?;
            compile_expr(idx, chunk, ctx, functions)?;
            chunk.push(OpCode::GetIndex, &ctx.file, ctx.line);
        }

        Expr::Lambda(params, _return_type, body) => {
            // Analyse des variables libres : tout identifiant référencé dans le corps
            // qui n'est ni un paramètre ni une variable déclarée localement dans la lambda.
            let mut bound: HashSet<String> = params.iter().map(|p| p.name.clone()).collect();
            let mut free: HashSet<String> = HashSet::new();
            for s in body { collect_stmt_free_vars(s, &mut bound, &mut free); }

            // Parmi les variables libres, seules celles qui existent réellement comme
            // locale de la portée englobante (déclarée avant ce point du code) sont
            // capturables ; les autres restent résolues comme globales, comme avant.
            let mut capture_indices = Vec::new();
            let mut capture_names = Vec::new();
            if ctx.is_local {
                for (idx, local_name) in ctx.locals.iter().enumerate() {
                    if free.contains(local_name) && !capture_names.contains(local_name) {
                        capture_indices.push(idx);
                        capture_names.push(local_name.clone());
                    }
                }
            }

            let name = ctx.fresh_name("lambda");
            let mut lambda_locals: Vec<String> = params.iter().map(|p| p.name.clone()).collect();
            lambda_locals.extend(capture_names);
            let mut lambda_ctx = FnCtx {
                is_local: true,
                locals: lambda_locals,
                loop_stack: Vec::new(),
                file: String::new(), line: 0,
                known_functions: ctx.known_functions.clone(),
                fresh_counter: ctx.fresh_counter.clone(),
            };
            let mut lambda_chunk = Chunk::default();
            for s in body { compile_stmt(s, &mut lambda_chunk, &mut lambda_ctx, functions)?; }
            let idx = lambda_chunk.add_const(Value::Null);
            lambda_chunk.push(OpCode::Const(idx), &lambda_ctx.file, lambda_ctx.line);
            lambda_chunk.push(OpCode::Return, &lambda_ctx.file, lambda_ctx.line);
            functions.insert(name.clone(), FunctionProto { name: name.clone(), arity: params.len(), chunk: lambda_chunk });

            if capture_indices.is_empty() {
                chunk.push(OpCode::LoadFunction(name), &ctx.file, ctx.line);
            } else {
                chunk.push(OpCode::MakeClosure(name, capture_indices), &ctx.file, ctx.line);
            }
        }

        Expr::Call(callee, args) => match callee.as_ref() {
            Expr::FieldAccess(base, method) => {
                // obj.methode(args) : le récepteur devient le premier argument implicite.
                compile_expr(base, chunk, ctx, functions)?;
                for a in args { compile_expr(a, chunk, ctx, functions)?; }
                chunk.push(OpCode::CallMethod(method.clone(), args.len() + 1), &ctx.file, ctx.line);
            }
            Expr::Ident(name) => {
                let is_local_var = ctx.is_local && ctx.locals.iter().any(|n| n == name);
                if !is_local_var && ctx.known_functions.contains(name) {
                    for a in args { compile_expr(a, chunk, ctx, functions)?; }
                    chunk.push(OpCode::Call(name.clone(), args.len()), &ctx.file, ctx.line);
                } else {
                    // Appel dynamique : 'name' est une variable (paramètre, lambda stockée...).
                    compile_expr(callee, chunk, ctx, functions)?;
                    for a in args { compile_expr(a, chunk, ctx, functions)?; }
                    chunk.push(OpCode::CallValue(args.len()), &ctx.file, ctx.line);
                }
            }
            _ => {
                // Toute autre expression retournant une fonction (ex: appel immédiat d'une lambda).
                compile_expr(callee, chunk, ctx, functions)?;
                for a in args { compile_expr(a, chunk, ctx, functions)?; }
                chunk.push(OpCode::CallValue(args.len()), &ctx.file, ctx.line);
            }
        },

        Expr::StructInit(name, fields) => {
            for (_, e) in fields { compile_expr(e, chunk, ctx, functions)?; }
            let field_names: Vec<String> = fields.iter().map(|(n, _)| n.clone()).collect();
            chunk.push(OpCode::MakeStruct(name.clone(), field_names), &ctx.file, ctx.line);
        }
    }
    Ok(())
}

// ---------- Analyse des variables libres (pour les fermetures/lambdas) ----------
//
// Parcourt le corps d'une lambda et collecte les identifiants référencés qui ne
// sont liés ni par ses paramètres, ni par une déclaration locale à l'intérieur
// d'elle-même (aoka/tsymiova, boucle for-each, variable de 'sambotra', ou
// paramètres d'une lambda imbriquée). Ce qui reste est "libre" : potentiellement
// une variable de la portée englobante à capturer, ou une variable globale.

fn collect_stmt_free_vars(stmt: &Stmt, bound: &mut HashSet<String>, free: &mut HashSet<String>) {
    match &stmt.kind {
        StmtKind::VarDecl { name, value, .. } => {
            collect_expr_free_vars(value, bound, free);
            bound.insert(name.clone());
        }
        StmtKind::ExprStmt(e) | StmtKind::Print(e) => collect_expr_free_vars(e, bound, free),
        StmtKind::If { cond, then_branch, else_ifs, else_branch } => {
            collect_expr_free_vars(cond, bound, free);
            let mut b = bound.clone();
            for s in then_branch { collect_stmt_free_vars(s, &mut b, free); }
            for (c, blk) in else_ifs {
                collect_expr_free_vars(c, bound, free);
                let mut b2 = bound.clone();
                for s in blk { collect_stmt_free_vars(s, &mut b2, free); }
            }
            if let Some(blk) = else_branch {
                let mut b3 = bound.clone();
                for s in blk { collect_stmt_free_vars(s, &mut b3, free); }
            }
        }
        StmtKind::While { cond, body } => {
            collect_expr_free_vars(cond, bound, free);
            let mut b = bound.clone();
            for s in body { collect_stmt_free_vars(s, &mut b, free); }
        }
        StmtKind::For { init, cond, update, body } => {
            let mut b = bound.clone();
            collect_stmt_free_vars(init, &mut b, free);
            collect_expr_free_vars(cond, &b, free);
            for s in body { collect_stmt_free_vars(s, &mut b, free); }
            collect_stmt_free_vars(update, &mut b, free);
        }
        StmtKind::ForEach { var, iterable, body } => {
            collect_expr_free_vars(iterable, bound, free);
            let mut b = bound.clone();
            b.insert(var.clone());
            for s in body { collect_stmt_free_vars(s, &mut b, free); }
        }
        StmtKind::Return(Some(e)) => collect_expr_free_vars(e, bound, free),
        StmtKind::Return(None) | StmtKind::Break | StmtKind::Continue | StmtKind::Import(_) => {}
        StmtKind::TryCatch { try_block, catch_var, catch_block } => {
            let mut b1 = bound.clone();
            for s in try_block { collect_stmt_free_vars(s, &mut b1, free); }
            let mut b2 = bound.clone();
            b2.insert(catch_var.clone());
            for s in catch_block { collect_stmt_free_vars(s, &mut b2, free); }
        }
        StmtKind::Throw(e) => collect_expr_free_vars(e, bound, free),
        StmtKind::FunctionDecl(_) | StmtKind::StructDecl(_) | StmtKind::ImplDecl(_) => {}
    }
}

fn collect_expr_free_vars(expr: &Expr, bound: &HashSet<String>, free: &mut HashSet<String>) {
    match expr {
        Expr::IntLit(_) | Expr::FloatLit(_) | Expr::StrLit(_) | Expr::BoolLit(_) => {}
        Expr::Ident(name) => { if !bound.contains(name) { free.insert(name.clone()); } }
        Expr::Unary(_, e) => collect_expr_free_vars(e, bound, free),
        Expr::Ternary(c, a, b) => {
            collect_expr_free_vars(c, bound, free);
            collect_expr_free_vars(a, bound, free);
            collect_expr_free_vars(b, bound, free);
        }
        Expr::Binary(l, _, r) => { collect_expr_free_vars(l, bound, free); collect_expr_free_vars(r, bound, free); }
        Expr::Assign(t, v) => { collect_expr_free_vars(t, bound, free); collect_expr_free_vars(v, bound, free); }
        Expr::Call(callee, args) => {
            collect_expr_free_vars(callee, bound, free);
            for a in args { collect_expr_free_vars(a, bound, free); }
        }
        Expr::FieldAccess(b, _) => collect_expr_free_vars(b, bound, free),
        Expr::StructInit(_, fields) => { for (_, e) in fields { collect_expr_free_vars(e, bound, free); } }
        Expr::ArrayLit(es) => { for e in es { collect_expr_free_vars(e, bound, free); } }
        Expr::DictLit(pairs) => { for (k, v) in pairs { collect_expr_free_vars(k, bound, free); collect_expr_free_vars(v, bound, free); } }
        Expr::TupleLit(es) => { for e in es { collect_expr_free_vars(e, bound, free); } }
        Expr::Index(b, i) => { collect_expr_free_vars(b, bound, free); collect_expr_free_vars(i, bound, free); }
        Expr::Lambda(params, _, body) => {
            let mut b = bound.clone();
            for p in params { b.insert(p.name.clone()); }
            for s in body { collect_stmt_free_vars(s, &mut b, free); }
        }
    }
}

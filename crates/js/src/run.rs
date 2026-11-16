//! The syntax tree compiled to nested Rust closures. Each variable is
//! resolved once, at compile time, to a slot in its function's frame (or to
//! a property of the global object), so running code never looks names up.

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use crate::ast::*;
use crate::value::*;

pub enum Flow {
    Normal,
    Return(Value),
    Break(Option<Name>),
    Continue(Option<Name>),
}

pub struct Env {
    pub slots: RefCell<Vec<Value>>,
    pub parent: Option<Rc<Env>>,
}

pub struct Frame {
    pub env: Rc<Env>,
    pub this: Value,
    pub home: Option<Rc<Obj>>,
    pub func: Option<Rc<Obj>>,
    pub new_target: Value,
}

type Ex = Box<dyn Fn(&Rt, &Frame) -> R<Value>>;
type St = Box<dyn Fn(&Rt, &Frame) -> R<Flow>>;
type Bind = Box<dyn Fn(&Rt, &Frame, Value) -> R<()>>;
type Args = Box<dyn Fn(&Rt, &Frame) -> R<ArgBuf>>;

const INLINE_ARGS: usize = 6;

/// Call arguments, kept off the heap when there are few.
pub enum ArgBuf {
    Inline([Value; INLINE_ARGS], usize),
    Heap(Vec<Value>),
}

impl std::ops::Deref for ArgBuf {
    type Target = [Value];
    #[inline]
    fn deref(&self) -> &[Value] {
        match self {
            ArgBuf::Inline(a, n) => &a[..*n],
            ArgBuf::Heap(v) => v,
        }
    }
}

type Key = Box<dyn Fn(&Rt, &Frame) -> R<Name>>;
type Binder = Box<dyn Fn(&Rt, &Frame, &[Value]) -> R<()>>;
type Lexical = (Value, Option<Rc<Obj>>, Option<Rc<Obj>>, Value);
type Place = Box<dyn Fn(&Rt, &Frame, &dyn Fn(&Rt, Value) -> R<(Value, Value)>) -> R<Value>>;

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum ClassKind {
    None,
    Base,
    Derived,
}

pub struct FnCode {
    /// Parameters before the first default or rest.
    pub length: u32,
    pub name: Option<Name>,
    nslots: Cell<usize>,
    simple_params: Vec<usize>,
    bind: Option<Binder>,
    body: Vec<St>,
    arrow: bool,
    generator: bool,
    strict: bool,
    method: bool,
    self_slot: Option<usize>,
    class: ClassKind,
    /// A class with no `constructor`: passes its arguments to the parent's.
    default_ctor: bool,
    pub params: usize,
}

pub struct Closure {
    pub code: Rc<FnCode>,
    pub env: Rc<Env>,
    /// An arrow's `this`, home object and `new.target`, from where it was made.
    lexical: Option<Lexical>,
    pub home: RefCell<Option<Rc<Obj>>>,
    /// A class constructor's instance field initialiser.
    fields: RefCell<Option<Value>>,
}

#[derive(Debug)]
pub struct CompileError(pub String);

type CR<T> = Result<T, CompileError>;

fn unsupported<T>(what: &str) -> CR<T> {
    Err(CompileError(format!("not supported yet: {what}")))
}

enum Loc {
    Slot(usize, usize),
    Global(Name),
}

struct Scope {
    blocks: Vec<Vec<(Name, usize)>>,
    slots: usize,
    global: bool,
}

pub struct Compiler<'s> {
    scopes: Vec<Scope>,
    src: &'s str,
    /// The plain argument list of the call being compiled.
    direct: Option<Rc<[Ex]>>,
}

const MAX_DEPTH: u32 = 300;

/// Generators run to completion when called and hand out what they
/// yielded; this bounds one that never ends.
const MAX_YIELDS: usize = 1 << 20;

thread_local! {
    static YIELDS: RefCell<Vec<Vec<Value>>> = const { RefCell::new(Vec::new()) };
}

#[inline]
fn slot_env(env: &Rc<Env>, depth: usize) -> &Rc<Env> {
    let mut e = env;
    for _ in 0..depth {
        e = e.parent.as_ref().unwrap();
    }
    e
}

fn pattern_names(p: &Pattern, out: &mut Vec<Name>) {
    match p {
        Pattern::Ident(n) => out.push(n.clone()),
        Pattern::Expr(_) => {}
        Pattern::Object(props, rest) => {
            props.iter().for_each(|pp| pattern_names(&pp.value, out));
            if let Some(r) = rest {
                pattern_names(r, out);
            }
        }
        Pattern::Array(items, rest) => {
            items.iter().flatten().for_each(|i| pattern_names(i, out));
            if let Some(r) = rest {
                pattern_names(r, out);
            }
        }
        Pattern::Default(p, _) => pattern_names(p, out),
    }
}

/// `var` names anywhere in a function body, outside nested functions.
fn var_names(stmts: &[Stmt], out: &mut Vec<Name>) {
    for s in stmts {
        var_names_stmt(s, out);
    }
}

fn var_names_stmt(s: &Stmt, out: &mut Vec<Name>) {
    match s {
        Stmt::Var(VarKind::Var, d) => d.iter().for_each(|(p, _)| pattern_names(p, out)),
        Stmt::If(_, a, b) => {
            var_names_stmt(a, out);
            if let Some(b) = b {
                var_names_stmt(b, out);
            }
        }
        Stmt::Block(b) => var_names(b, out),
        Stmt::For(init, _, _, body) => {
            if let Some(ForInit::Var(VarKind::Var, d)) = init {
                d.iter().for_each(|(p, _)| pattern_names(p, out));
            }
            var_names_stmt(body, out);
        }
        Stmt::ForIn(init, _, body) | Stmt::ForOf(init, _, body) => {
            if let ForInit::Var(VarKind::Var, d) = init {
                d.iter().for_each(|(p, _)| pattern_names(p, out));
            }
            var_names_stmt(body, out);
        }
        Stmt::While(_, b) | Stmt::DoWhile(b, _) | Stmt::Labeled(_, b) | Stmt::With(_, b) => {
            var_names_stmt(b, out)
        }
        Stmt::Try(b, c, f) => {
            var_names(b, out);
            if let Some((_, c)) = c {
                var_names(c, out);
            }
            if let Some(f) = f {
                var_names(f, out);
            }
        }
        Stmt::Switch(_, cases) => cases.iter().for_each(|(_, b)| var_names(b, out)),
        _ => {}
    }
}

/// Names a block declares itself: `let`, `const`, classes and functions.
fn lexical_names(stmts: &[Stmt], out: &mut Vec<Name>) {
    for s in stmts {
        match s {
            Stmt::Var(VarKind::Let | VarKind::Const, d) => {
                d.iter().for_each(|(p, _)| pattern_names(p, out))
            }
            Stmt::Class(c) => out.extend(c.name.clone()),
            Stmt::Function(f) => out.extend(f.name.clone()),
            _ => {}
        }
    }
}

fn throw_ref(rt: &Rt, name: &str) -> Exc {
    let p = rt.reference_error_proto.clone();
    match rt.throw::<()>(&p, &format!("{name} is not defined")) {
        Err(e) => e,
        Ok(()) => unreachable!(),
    }
}

impl<'s> Compiler<'s> {
    pub fn new(src: &'s str) -> Self {
        Self {
            scopes: Vec::new(),
            direct: None,
            src,
        }
    }

    fn scope(&mut self) -> &mut Scope {
        self.scopes.last_mut().unwrap()
    }

    fn declare(&mut self, name: &Name) -> Option<usize> {
        let s = self.scope();
        if s.global && s.blocks.len() == 1 {
            return None;
        }
        if let Some(&(_, i)) = s.blocks.last().unwrap().iter().find(|(n, _)| n == name) {
            return Some(i);
        }
        let i = s.slots;
        s.slots += 1;
        s.blocks.last_mut().unwrap().push((name.clone(), i));
        Some(i)
    }

    fn resolve(&self, name: &Name) -> Loc {
        for (depth, s) in self.scopes.iter().rev().enumerate() {
            for b in s.blocks.iter().rev() {
                if let Some(&(_, i)) = b.iter().rev().find(|(n, _)| n == name) {
                    return Loc::Slot(depth, i);
                }
            }
        }
        Loc::Global(name.clone())
    }

    pub fn program(&mut self, p: &Program) -> CR<(Vec<St>, usize, Vec<Name>)> {
        self.scopes.push(Scope {
            blocks: vec![Vec::new()],
            slots: 0,
            global: true,
        });
        let mut vars = Vec::new();
        var_names(&p.body, &mut vars);
        lexical_names(&p.body, &mut vars);
        let mut body = self.block_body(&p.body)?;
        // The script's value: its last statement, when that's an expression.
        if let Some(Stmt::Expr(e)) = p.body.last() {
            body.pop();
            let e = self.expr(e)?;
            body.push(Box::new(move |rt, f| Ok(Flow::Return(e(rt, f)?))));
        }
        let s = self.scopes.pop().unwrap();
        Ok((body, s.slots, vars))
    }

    /// Statements of a block whose scope is already open, functions first.
    fn block_body(&mut self, stmts: &[Stmt]) -> CR<Vec<St>> {
        let mut names = Vec::new();
        lexical_names(stmts, &mut names);
        for n in &names {
            self.declare(n);
        }
        let mut out = Vec::new();
        for s in stmts {
            if let Stmt::Function(f) = s {
                let make = self.function(f, false)?;
                let w = self.writer(f.name.as_ref().unwrap(), true);
                out.push(Box::new(move |rt: &Rt, fr: &Frame| {
                    let v = make(rt, fr)?;
                    w(rt, fr, v)?;
                    Ok(Flow::Normal)
                }) as St);
            }
        }
        for s in stmts {
            if !matches!(s, Stmt::Function(_)) {
                out.push(self.stmt(s, &[])?);
            }
        }
        Ok(out)
    }

    fn block(&mut self, stmts: &[Stmt]) -> CR<St> {
        self.scope().blocks.push(Vec::new());
        let body = self.block_body(stmts)?;
        self.scope().blocks.pop();
        Ok(seq(body))
    }

    fn reader(&self, name: &Name, typeof_: bool) -> Ex {
        match self.resolve(name) {
            Loc::Slot(0, i) => Box::new(move |_, f| Ok(f.env.slots.borrow()[i].clone())),
            Loc::Slot(d, i) => {
                Box::new(move |_, f| Ok(slot_env(&f.env, d).slots.borrow()[i].clone()))
            }
            Loc::Global(n) => {
                if &*n == "undefined" {
                    return Box::new(|_, _| Ok(Value::Undefined));
                }
                let hint = Cell::new(0);
                Box::new(move |rt, _| match rt.global.own_data_hinted(&n, &hint) {
                    Some(v) => Ok(v),
                    None => global_read(rt, &n, typeof_),
                })
            }
        }
    }

    fn writer(&self, name: &Name, _decl: bool) -> Bind {
        match self.resolve(name) {
            Loc::Slot(0, i) => Box::new(move |_, f, v| {
                f.env.slots.borrow_mut()[i] = v;
                Ok(())
            }),
            Loc::Slot(d, i) => Box::new(move |_, f, v| {
                slot_env(&f.env, d).slots.borrow_mut()[i] = v;
                Ok(())
            }),
            Loc::Global(n) => {
                Box::new(move |rt, _, v| rt.set(&Value::Obj(rt.global.clone()), &n, v))
            }
        }
    }

    fn function(&mut self, f: &Rc<Function>, method: bool) -> CR<Ex> {
        let code = self.function_code(f, method, ClassKind::None, false)?;
        Ok(make_closure_ex(code))
    }

    fn function_code(
        &mut self,
        f: &Function,
        method: bool,
        class: ClassKind,
        default_ctor: bool,
    ) -> CR<Rc<FnCode>> {
        if f.is_async {
            return unsupported("async functions");
        }
        self.scopes.push(Scope {
            blocks: vec![Vec::new()],
            slots: 0,
            global: false,
        });
        let mut params = Vec::new();
        for p in f.params.iter().chain(f.rest.iter()) {
            pattern_names(p, &mut params);
        }
        let simple = f.rest.is_none() && f.params.iter().all(|p| matches!(p, Pattern::Ident(_)));
        let mut simple_params = Vec::new();
        for n in &params {
            let i = self.declare(n).unwrap();
            if simple {
                simple_params.push(i);
            }
        }
        let text = self.src.get(f.start..f.end).unwrap_or("");
        let args_slot =
            (!f.arrow && text.contains("arguments") && !params.iter().any(|p| &**p == "arguments"))
                .then(|| self.declare(&"arguments".into()).unwrap());
        let self_slot = match &f.name {
            Some(n) if !method && !params.contains(n) => Some(self.declare(n).unwrap()),
            _ => None,
        };
        let bind = if simple && args_slot.is_none() {
            None
        } else {
            let binders: Vec<Bind> = if simple {
                Vec::new()
            } else {
                f.params
                    .iter()
                    .map(|p| self.pattern(p, true))
                    .collect::<CR<_>>()?
            };
            let rest = match &f.rest {
                Some(r) => Some(self.pattern(r, true)?),
                None => None,
            };
            let n = f.params.len();
            let sp = simple_params.clone();
            Some(Box::new(move |rt: &Rt, fr: &Frame, args: &[Value]| {
                if sp.is_empty() {
                    for (i, b) in binders.iter().enumerate() {
                        b(rt, fr, args.get(i).cloned().unwrap_or_default())?;
                    }
                } else {
                    let mut s = fr.env.slots.borrow_mut();
                    for (i, &slot) in sp.iter().enumerate() {
                        s[slot] = args.get(i).cloned().unwrap_or_default();
                    }
                }
                if let Some(r) = &rest {
                    r(rt, fr, rt.array(args.get(n..).unwrap_or(&[]).to_vec()))?;
                }
                if let Some(a) = args_slot {
                    let v = rt.array(args.to_vec());
                    fr.env.slots.borrow_mut()[a] = v;
                }
                Ok(())
            }) as Binder)
        };
        let mut vars = Vec::new();
        var_names(&f.body, &mut vars);
        for v in &vars {
            if !params.contains(v) {
                self.declare(v);
            }
        }
        let body = self.block_body(&f.body)?;
        let s = self.scopes.pop().unwrap();
        Ok(Rc::new(FnCode {
            length: f
                .params
                .iter()
                .take_while(|p| !matches!(p, Pattern::Default(..)))
                .count() as u32,
            name: f.name.clone(),
            nslots: Cell::new(s.slots),
            simple_params: if bind.is_none() {
                simple_params
            } else {
                Vec::new()
            },
            bind,
            body,
            arrow: f.arrow,
            generator: f.generator,
            strict: f.strict,
            method,
            self_slot,
            class,
            default_ctor,
            params: f.params.len(),
        }))
    }

    fn stmt(&mut self, s: &Stmt, labels: &[Name]) -> CR<St> {
        Ok(match s {
            Stmt::Expr(e) => {
                let e = self.expr(e)?;
                Box::new(move |rt, f| {
                    e(rt, f)?;
                    Ok(Flow::Normal)
                })
            }
            Stmt::Var(_, decls) => {
                let mut parts: Vec<St> = Vec::new();
                for (p, init) in decls {
                    let Some(init) = init else {
                        if let Pattern::Ident(n) = p {
                            if let Loc::Global(n) = self.resolve(n) {
                                parts.push(Box::new(move |rt, _| {
                                    if rt.global.props.borrow().get(&n).is_none() {
                                        rt.global.put_name(&n, Value::Undefined);
                                    }
                                    Ok(Flow::Normal)
                                }));
                            }
                        }
                        continue;
                    };
                    let v = self.named_expr(init, p)?;
                    let b = self.pattern(p, true)?;
                    parts.push(Box::new(move |rt, f| {
                        let x = v(rt, f)?;
                        b(rt, f, x)?;
                        Ok(Flow::Normal)
                    }));
                }
                seq(parts)
            }
            Stmt::Function(_) => Box::new(|_, _| Ok(Flow::Normal)),
            Stmt::Class(c) => {
                let make = self.class(c)?;
                let w = self.writer(c.name.as_ref().unwrap(), true);
                Box::new(move |rt, f| {
                    let v = make(rt, f)?;
                    w(rt, f, v)?;
                    Ok(Flow::Normal)
                })
            }
            Stmt::Return(e) => match e {
                Some(e) => {
                    let e = self.expr(e)?;
                    Box::new(move |rt, f| Ok(Flow::Return(e(rt, f)?)))
                }
                None => Box::new(|_, _| Ok(Flow::Return(Value::Undefined))),
            },
            Stmt::If(c, a, b) => {
                let c = self.cond(c)?;
                let a = self.stmt(a, &[])?;
                match b {
                    Some(b) => {
                        let b = self.stmt(b, &[])?;
                        Box::new(move |rt, f| if c(rt, f)? { a(rt, f) } else { b(rt, f) })
                    }
                    None => Box::new(move |rt, f| {
                        if c(rt, f)? {
                            a(rt, f)
                        } else {
                            Ok(Flow::Normal)
                        }
                    }),
                }
            }
            Stmt::Block(b) => self.block(b)?,
            Stmt::Empty | Stmt::Debugger => Box::new(|_, _| Ok(Flow::Normal)),
            Stmt::Labeled(l, body) => {
                let mut ls = labels.to_vec();
                ls.push(l.clone());
                let body = self.stmt(body, &ls)?;
                let l = l.clone();
                Box::new(move |rt, f| match body(rt, f)? {
                    Flow::Break(Some(x)) if x == l => Ok(Flow::Normal),
                    other => Ok(other),
                })
            }
            Stmt::While(c, body) => {
                let c = self.cond(c)?;
                let body = self.stmt(body, &[])?;
                let labels: Rc<[Name]> = labels.into();
                Box::new(move |rt, f| {
                    while c(rt, f)? {
                        if let Some(out) = loop_flow(body(rt, f)?, &labels) {
                            return out;
                        }
                    }
                    Ok(Flow::Normal)
                })
            }
            Stmt::DoWhile(body, c) => {
                let c = self.cond(c)?;
                let body = self.stmt(body, &[])?;
                let labels: Rc<[Name]> = labels.into();
                Box::new(move |rt, f| loop {
                    if let Some(out) = loop_flow(body(rt, f)?, &labels) {
                        return out;
                    }
                    if !c(rt, f)? {
                        return Ok(Flow::Normal);
                    }
                })
            }
            Stmt::For(init, test, update, body) => {
                self.scope().blocks.push(Vec::new());
                let init: Option<St> = match init {
                    None => None,
                    Some(ForInit::Var(k, d)) => {
                        if *k != VarKind::Var {
                            let mut names = Vec::new();
                            d.iter().for_each(|(p, _)| pattern_names(p, &mut names));
                            for n in &names {
                                self.declare(n);
                            }
                        }
                        Some(self.stmt(&Stmt::Var(*k, d.clone()), &[])?)
                    }
                    Some(ForInit::Expr(e)) => Some(self.stmt(&Stmt::Expr(e.clone()), &[])?),
                    Some(ForInit::Pattern(_)) => return unsupported("pattern in a for head"),
                };
                let test = test.as_ref().map(|t| self.cond(t)).transpose()?;
                let update = update.as_ref().map(|t| self.expr(t)).transpose()?;
                let body = self.stmt(body, &[])?;
                self.scope().blocks.pop();
                let labels: Rc<[Name]> = labels.into();
                Box::new(move |rt, f| {
                    if let Some(i) = &init {
                        i(rt, f)?;
                    }
                    loop {
                        if let Some(t) = &test {
                            if !t(rt, f)? {
                                return Ok(Flow::Normal);
                            }
                        }
                        if let Some(out) = loop_flow(body(rt, f)?, &labels) {
                            return out;
                        }
                        if let Some(u) = &update {
                            u(rt, f)?;
                        }
                    }
                })
            }
            Stmt::ForIn(init, obj, body) | Stmt::ForOf(init, obj, body) => {
                let of = matches!(s, Stmt::ForOf(..));
                self.scope().blocks.push(Vec::new());
                let target = match init {
                    ForInit::Var(k, d) => {
                        let (p, _) = &d[0];
                        if *k != VarKind::Var {
                            let mut names = Vec::new();
                            pattern_names(p, &mut names);
                            for n in &names {
                                self.declare(n);
                            }
                        }
                        self.pattern(p, true)?
                    }
                    ForInit::Expr(e) => self.pattern(&Pattern::Expr(Box::new(e.clone())), false)?,
                    ForInit::Pattern(p) => self.pattern(p, false)?,
                };
                let target = match init {
                    ForInit::Expr(Expr::Ident(n, _)) => self.writer(n, false),
                    _ => target,
                };
                let obj = self.expr(obj)?;
                let body = self.stmt(body, &[])?;
                self.scope().blocks.pop();
                let labels: Rc<[Name]> = labels.into();
                Box::new(move |rt, f| {
                    let o = obj(rt, f)?;
                    let items: Vec<Value> = if of {
                        rt.list(&o)?
                    } else {
                        for_in_keys(rt, &o).into_iter().map(Value::Str).collect()
                    };
                    for it in items {
                        target(rt, f, it)?;
                        if let Some(out) = loop_flow(body(rt, f)?, &labels) {
                            return out;
                        }
                    }
                    Ok(Flow::Normal)
                })
            }
            Stmt::Break(l) => {
                let l = l.clone();
                Box::new(move |_, _| Ok(Flow::Break(l.clone())))
            }
            Stmt::Continue(l) => {
                let l = l.clone();
                Box::new(move |_, _| Ok(Flow::Continue(l.clone())))
            }
            Stmt::Throw(e) => {
                let e = self.expr(e)?;
                Box::new(move |rt, f| Err(Exc::Throw(e(rt, f)?)))
            }
            Stmt::Try(body, catch, finally) => {
                let body = self.block(body)?;
                let catch = match catch {
                    Some((param, c)) => {
                        self.scope().blocks.push(Vec::new());
                        let b = match param {
                            Some(p) => {
                                let mut names = Vec::new();
                                pattern_names(p, &mut names);
                                for n in &names {
                                    self.declare(n);
                                }
                                Some(self.pattern(p, true)?)
                            }
                            None => None,
                        };
                        let c = self.block(c)?;
                        self.scope().blocks.pop();
                        Some((b, c))
                    }
                    None => None,
                };
                let finally = finally.as_ref().map(|f| self.block(f)).transpose()?;
                Box::new(move |rt, f| {
                    let mut r = body(rt, f);
                    if let (Err(Exc::Throw(e)), Some((b, c))) = (&r, &catch) {
                        let e = e.clone();
                        r = (|| {
                            if let Some(b) = b {
                                b(rt, f, e)?;
                            }
                            c(rt, f)
                        })();
                    }
                    if let Some(fin) = &finally {
                        match fin(rt, f)? {
                            Flow::Normal => {}
                            other => return Ok(other),
                        }
                    }
                    r
                })
            }
            Stmt::Switch(d, cases) => {
                let d = self.expr(d)?;
                self.scope().blocks.push(Vec::new());
                let all: Vec<Stmt> = cases.iter().flat_map(|(_, b)| b.iter().cloned()).collect();
                let mut names = Vec::new();
                lexical_names(&all, &mut names);
                for n in &names {
                    self.declare(n);
                }
                let mut compiled = Vec::new();
                for (t, b) in cases {
                    let t = t.as_ref().map(|t| self.expr(t)).transpose()?;
                    let mut body = Vec::new();
                    for s in b {
                        body.push(match s {
                            Stmt::Function(func) => {
                                let make = self.function(func, false)?;
                                let w = self.writer(func.name.as_ref().unwrap(), true);
                                Box::new(move |rt: &Rt, fr: &Frame| {
                                    let v = make(rt, fr)?;
                                    w(rt, fr, v)?;
                                    Ok(Flow::Normal)
                                }) as St
                            }
                            s => self.stmt(s, &[])?,
                        });
                    }
                    compiled.push((t, body));
                }
                self.scope().blocks.pop();
                Box::new(move |rt, f| {
                    let v = d(rt, f)?;
                    let mut start = None;
                    for (i, (t, _)) in compiled.iter().enumerate() {
                        if let Some(t) = t {
                            if t(rt, f)?.strict_eq(&v) {
                                start = Some(i);
                                break;
                            }
                        }
                    }
                    let start = start.or_else(|| compiled.iter().position(|(t, _)| t.is_none()));
                    let Some(start) = start else {
                        return Ok(Flow::Normal);
                    };
                    for (_, body) in &compiled[start..] {
                        for s in body {
                            match s(rt, f)? {
                                Flow::Normal => {}
                                Flow::Break(None) => return Ok(Flow::Normal),
                                other => return Ok(other),
                            }
                        }
                    }
                    Ok(Flow::Normal)
                })
            }
            Stmt::With(..) => return unsupported("`with`"),
        })
    }

    /// A value given a name by where it's put (`var f = function () {}`).
    fn named_expr(&mut self, e: &Expr, _target: &Pattern) -> CR<Ex> {
        self.expr(e)
    }

    fn pattern(&mut self, p: &Pattern, decl: bool) -> CR<Bind> {
        Ok(match p {
            Pattern::Ident(n) => self.writer(n, decl),
            Pattern::Expr(e) => match &**e {
                Expr::Ident(n, _) => self.writer(n, decl),
                Expr::Member(o, name, false) => {
                    let o = self.expr(o)?;
                    let name = name.clone();
                    Box::new(move |rt, f, v| rt.set(&o(rt, f)?, &name, v))
                }
                Expr::Index(o, k, false) => {
                    let o = self.expr(o)?;
                    let k = self.expr(k)?;
                    Box::new(move |rt, f, v| {
                        let ov = o(rt, f)?;
                        let kv = k(rt, f)?;
                        rt.set_index(&ov, &kv, v)
                    })
                }
                Expr::PrivateMember(o, name) => {
                    let o = self.expr(o)?;
                    let name = private(name);
                    Box::new(move |rt, f, v| rt.set(&o(rt, f)?, &name, v))
                }
                _ => return unsupported("this assignment target"),
            },
            Pattern::Default(p, d) => {
                let p = self.pattern(p, decl)?;
                let d = self.expr(d)?;
                Box::new(move |rt, f, v| {
                    let v = if matches!(v, Value::Undefined) {
                        d(rt, f)?
                    } else {
                        v
                    };
                    p(rt, f, v)
                })
            }
            Pattern::Array(items, rest) => {
                let items: Vec<Option<Bind>> = items
                    .iter()
                    .map(|i| i.as_ref().map(|i| self.pattern(i, decl)).transpose())
                    .collect::<CR<_>>()?;
                let rest = rest.as_ref().map(|r| self.pattern(r, decl)).transpose()?;
                Box::new(move |rt, f, v| {
                    let list = rt.list(&v)?;
                    for (i, b) in items.iter().enumerate() {
                        if let Some(b) = b {
                            b(rt, f, list.get(i).cloned().unwrap_or_default())?;
                        }
                    }
                    if let Some(r) = &rest {
                        r(
                            rt,
                            f,
                            rt.array(list.get(items.len()..).unwrap_or(&[]).to_vec()),
                        )?;
                    }
                    Ok(())
                })
            }
            Pattern::Object(props, rest) => {
                let mut parts = Vec::new();
                for pp in props {
                    let key = self.key(&pp.key)?;
                    parts.push((key, self.pattern(&pp.value, decl)?));
                }
                let rest = rest.as_ref().map(|r| self.pattern(r, decl)).transpose()?;
                Box::new(move |rt, f, v| {
                    if v.is_nullish() {
                        return rt.type_error("cannot destructure undefined or null");
                    }
                    let mut used = Vec::new();
                    for (k, b) in &parts {
                        let k = k(rt, f)?;
                        b(rt, f, rt.get(&v, &k)?)?;
                        used.push(k);
                    }
                    if let Some(r) = &rest {
                        let o = rt.object();
                        if let Value::Obj(src) = &v {
                            for k in rt.own_keys(src) {
                                if !used.contains(&k) {
                                    o.put_name(&k, rt.get(&v, &k)?);
                                }
                            }
                        }
                        r(rt, f, Value::Obj(o))?;
                    }
                    Ok(())
                })
            }
        })
    }

    fn key(&mut self, k: &PropKey) -> CR<Key> {
        Ok(match k {
            PropKey::Name(n) => {
                let n = n.clone();
                Box::new(move |_, _| Ok(n.clone()))
            }
            PropKey::Private(n) => {
                let n = private(n);
                Box::new(move |_, _| Ok(n.clone()))
            }
            PropKey::Computed(e) => {
                let e = self.expr(e)?;
                Box::new(move |rt, f| rt.to_key(&e(rt, f)?))
            }
        })
    }

    fn args(&mut self, args: &[Expr]) -> CR<Args> {
        self.direct = None;
        if args.iter().any(|a| matches!(a, Expr::Spread(_))) {
            let parts: Vec<(bool, Ex)> = args
                .iter()
                .map(|a| match a {
                    Expr::Spread(e) => Ok((true, self.expr(e)?)),
                    e => Ok((false, self.expr(e)?)),
                })
                .collect::<CR<_>>()?;
            return Ok(Box::new(move |rt, f| {
                let mut out = Vec::new();
                for (spread, e) in &parts {
                    let v = e(rt, f)?;
                    if *spread {
                        out.extend(rt.list(&v)?);
                    } else {
                        out.push(v);
                    }
                }
                Ok(ArgBuf::Heap(out))
            }));
        }
        let parts: Rc<[Ex]> = args
            .iter()
            .map(|a| self.expr(a))
            .collect::<CR<Vec<_>>>()?
            .into();
        self.direct = Some(parts.clone());
        if parts.len() <= INLINE_ARGS {
            return Ok(Box::new(move |rt, f| {
                let mut out: [Value; INLINE_ARGS] = Default::default();
                for (slot, e) in out.iter_mut().zip(parts.iter()) {
                    *slot = e(rt, f)?;
                }
                Ok(ArgBuf::Inline(out, parts.len()))
            }));
        }
        Ok(Box::new(move |rt, f| {
            let mut out = Vec::with_capacity(parts.len());
            for e in parts.iter() {
                out.push(e(rt, f)?);
            }
            Ok(ArgBuf::Heap(out))
        }))
    }

    pub fn expr(&mut self, e: &Expr) -> CR<Ex> {
        Ok(match e {
            Expr::Num(n) => {
                let n = *n;
                Box::new(move |_, _| Ok(Value::Num(n)))
            }
            Expr::Str(s) => {
                let s = s.clone();
                Box::new(move |_, _| Ok(Value::Str(s.clone())))
            }
            Expr::Bool(b) => {
                let b = *b;
                Box::new(move |_, _| Ok(Value::Bool(b)))
            }
            Expr::Null => Box::new(|_, _| Ok(Value::Null)),
            Expr::Undefined => Box::new(|_, _| Ok(Value::Undefined)),
            Expr::BigInt(_) => return unsupported("BigInt"),
            Expr::Regex(body, flags) => {
                if let Err(e) = crate::regex::Regex::new(body, flags) {
                    return Err(CompileError(e));
                }
                let (body, flags) = (body.clone(), flags.clone());
                Box::new(move |rt, _| rt.regexp(&body, &flags))
            }
            Expr::Await(_) => return unsupported("async functions"),
            Expr::Yield(arg, delegate) => {
                let arg = arg.as_ref().map(|a| self.expr(a)).transpose()?;
                let delegate = *delegate;
                Box::new(move |rt, f| {
                    let v = match &arg {
                        Some(a) => a(rt, f)?,
                        None => Value::Undefined,
                    };
                    let items = if delegate { rt.list(&v)? } else { vec![v] };
                    let full = YIELDS.with(|y| {
                        let mut y = y.borrow_mut();
                        let top = y.last_mut().expect("yield outside a generator");
                        top.extend(items);
                        top.len() > MAX_YIELDS
                    });
                    if full {
                        let p = rt.range_error_proto.clone();
                        return rt.throw(&p, "generator yielded too many values");
                    }
                    Ok(Value::Undefined)
                })
            }
            Expr::Ident(n, _) => self.reader(n, false),
            Expr::This => Box::new(|_, f| Ok(f.this.clone())),
            Expr::NewTarget => Box::new(|_, f| Ok(f.new_target.clone())),
            Expr::Super => return unsupported("`super` here"),
            Expr::Template(tag, parts) => self.template(tag.as_deref(), parts)?,
            Expr::Array(items) => {
                let parts: Vec<Option<(bool, Ex)>> = items
                    .iter()
                    .map(|i| match i {
                        None => Ok(None),
                        Some(Expr::Spread(e)) => Ok(Some((true, self.expr(e)?))),
                        Some(e) => Ok(Some((false, self.expr(e)?))),
                    })
                    .collect::<CR<_>>()?;
                Box::new(move |rt, f| {
                    let mut out = Vec::with_capacity(parts.len());
                    for p in &parts {
                        match p {
                            None => out.push(Value::Hole),
                            Some((false, e)) => out.push(e(rt, f)?),
                            Some((true, e)) => out.extend(rt.list(&e(rt, f)?)?),
                        }
                    }
                    Ok(rt.array(out))
                })
            }
            Expr::Spread(_) => return unsupported("spread here"),
            Expr::Object(props) => self.object(props)?,
            Expr::Function(func) => self.function(func, false)?,
            Expr::Class(c) => {
                self.scope().blocks.push(Vec::new());
                let r = self.class(c);
                self.scope().blocks.pop();
                r?
            }
            Expr::Unary(op, arg) => self.unary(op, arg)?,
            Expr::Update(op, prefix, target) => self.update(op, *prefix, target)?,
            Expr::Binary(op, a, b) => self.binary(op, a, b)?,
            Expr::Logical(op, a, b) => {
                let a = self.expr(a)?;
                let b = self.expr(b)?;
                match *op {
                    "&&" => Box::new(move |rt, f| {
                        let x = a(rt, f)?;
                        if x.truthy() {
                            b(rt, f)
                        } else {
                            Ok(x)
                        }
                    }),
                    "||" => Box::new(move |rt, f| {
                        let x = a(rt, f)?;
                        if x.truthy() {
                            Ok(x)
                        } else {
                            b(rt, f)
                        }
                    }),
                    _ => Box::new(move |rt, f| {
                        let x = a(rt, f)?;
                        if x.is_nullish() {
                            b(rt, f)
                        } else {
                            Ok(x)
                        }
                    }),
                }
            }
            Expr::Assign(op, target, value) => self.assign(op, target, value)?,
            Expr::Cond(c, a, b) => {
                let c = self.cond(c)?;
                let a = self.expr(a)?;
                let b = self.expr(b)?;
                Box::new(move |rt, f| if c(rt, f)? { a(rt, f) } else { b(rt, f) })
            }
            Expr::Seq(items) => {
                let items: Vec<Ex> = items.iter().map(|i| self.expr(i)).collect::<CR<_>>()?;
                Box::new(move |rt, f| {
                    let mut last = Value::Undefined;
                    for i in &items {
                        last = i(rt, f)?;
                    }
                    Ok(last)
                })
            }
            Expr::Member(o, name, optional) => {
                let name = name.clone();
                if matches!(**o, Expr::Super) {
                    return Ok(Box::new(move |rt, f| super_get(rt, f, &name)));
                }
                let base = match &**o {
                    Expr::This => Base::This,
                    Expr::Ident(n, _) => match self.resolve(n) {
                        Loc::Slot(0, i) => Base::Slot(i),
                        _ => Base::Ex,
                    },
                    _ => Base::Ex,
                };
                let o = self.expr(o)?;
                if *optional {
                    Box::new(move |rt, f| {
                        let v = o(rt, f)?;
                        if v.is_nullish() {
                            return Err(Exc::Short);
                        }
                        rt.get(&v, &name)
                    })
                } else {
                    let hint = Cell::new(0);
                    let len = &*name == "length";
                    match base {
                        Base::This => Box::new(move |rt, f| {
                            if let Value::Obj(ob) = &f.this {
                                if let Some(x) = ob.get_atom(&name, &hint, len) {
                                    return Ok(x);
                                }
                            }
                            rt.get(&f.this, &name)
                        }),
                        Base::Slot(i) => Box::new(move |rt, f| {
                            {
                                let s = f.env.slots.borrow();
                                if let Value::Obj(ob) = &s[i] {
                                    if let Some(x) = ob.get_atom(&name, &hint, len) {
                                        return Ok(x);
                                    }
                                }
                            }
                            let v = f.env.slots.borrow()[i].clone();
                            rt.get(&v, &name)
                        }),
                        Base::Ex => Box::new(move |rt, f| {
                            let v = o(rt, f)?;
                            if let Value::Obj(ob) = &v {
                                if let Some(x) = ob.get_atom(&name, &hint, len) {
                                    return Ok(x);
                                }
                            }
                            rt.get(&v, &name)
                        }),
                    }
                }
            }
            Expr::Index(o, k, optional) => {
                let optional = *optional;
                if matches!(**o, Expr::Super) {
                    let k = self.expr(k)?;
                    return Ok(Box::new(move |rt, f| {
                        let k = rt.to_key(&k(rt, f)?)?;
                        super_get(rt, f, &k)
                    }));
                }
                let o = self.expr(o)?;
                let k = self.expr(k)?;
                Box::new(move |rt, f| {
                    let v = o(rt, f)?;
                    if optional && v.is_nullish() {
                        return Err(Exc::Short);
                    }
                    let k = k(rt, f)?;
                    rt.get_index(&v, &k)
                })
            }
            Expr::PrivateMember(o, name) => {
                let o = self.expr(o)?;
                let name = private(name);
                Box::new(move |rt, f| rt.get(&o(rt, f)?, &name))
            }
            Expr::PrivateIn(name, o) => {
                let o = self.expr(o)?;
                let name = private(name);
                Box::new(move |rt, f| match o(rt, f)? {
                    Value::Obj(o) => Ok(Value::Bool(o.props.borrow().get(&name).is_some())),
                    _ => rt.type_error("right side of `in` is not an object"),
                })
            }
            Expr::Chain(e) => {
                let e = self.expr(e)?;
                Box::new(move |rt, f| match e(rt, f) {
                    Err(Exc::Short) => Ok(Value::Undefined),
                    r => r,
                })
            }
            Expr::Call(callee, args, optional) => self.call(callee, args, *optional)?,
            Expr::New(callee, args) => {
                let c = self.expr(callee)?;
                let args = self.args(args)?;
                Box::new(move |rt, f| {
                    let c = c(rt, f)?;
                    let a = args(rt, f)?;
                    rt.construct(&c, &a, &c)
                })
            }
        })
    }

    fn template(&mut self, tag: Option<&Expr>, parts: &[TemplatePart]) -> CR<Ex> {
        if let Some(tag) = tag {
            let (this, tag) = self.callee(tag)?;
            let mut cooked = Vec::new();
            let mut raw = Vec::new();
            let mut exprs = Vec::new();
            for p in parts {
                match p {
                    TemplatePart::Str(c, r) => {
                        cooked.push(c.clone());
                        raw.push(r.clone());
                    }
                    TemplatePart::Expr(e) => exprs.push(self.expr(e)?),
                }
            }
            return Ok(Box::new(move |rt, f| {
                let (t, func) = (this.as_ref().map(|t| t(rt, f)).transpose()?, tag(rt, f)?);
                let strings = rt.array(
                    cooked
                        .iter()
                        .map(|c| c.clone().map_or(Value::Undefined, Value::Str))
                        .collect(),
                );
                let raws = rt.array(raw.iter().map(|r| Value::Str(r.clone())).collect());
                rt.set(&strings, &"raw".into(), raws)?;
                let mut args = vec![strings];
                for e in &exprs {
                    args.push(e(rt, f)?);
                }
                let func = match (&t, func) {
                    (Some(t), Value::Str(n)) => rt.get(t, &n)?,
                    (_, func) => func,
                };
                rt.call(&func, t.unwrap_or_default(), &args)
            }));
        }
        enum Part {
            S(Name),
            E(Ex),
        }
        let parts: Vec<Part> = parts
            .iter()
            .map(|p| match p {
                TemplatePart::Str(c, _) => Ok(Part::S(c.clone().unwrap_or_default())),
                TemplatePart::Expr(e) => Ok(Part::E(self.expr(e)?)),
            })
            .collect::<CR<_>>()?;
        Ok(Box::new(move |rt, f| {
            let mut s = String::new();
            for p in &parts {
                match p {
                    Part::S(t) => s += t,
                    Part::E(e) => {
                        let v = e(rt, f)?;
                        match &v {
                            Value::Str(x) => s += x,
                            _ => s += &rt.to_str(&v)?,
                        }
                    }
                }
            }
            Ok(Value::Str(s.into()))
        }))
    }

    /// For a tag or callee: the object it's read from (if any) and either
    /// the function or, with an object, the property name as a string.
    fn callee(&mut self, e: &Expr) -> CR<(Option<Ex>, Ex)> {
        Ok(match e {
            Expr::Member(o, name, false) if !matches!(**o, Expr::Super) => {
                let n = name.clone();
                (
                    Some(self.expr(o)?),
                    Box::new(move |_, _| Ok(Value::Str(n.clone()))),
                )
            }
            e => (None, self.expr(e)?),
        })
    }

    fn object(&mut self, props: &[Property]) -> CR<Ex> {
        enum P {
            Init(Key, Ex, bool),
            Get(Key, Ex, bool),
            Spread(Ex),
        }
        let mut parts = Vec::new();
        for p in props {
            let key = self.key(&p.key)?;
            parts.push(match p.kind {
                PropKind::Spread => P::Spread(self.expr(&p.value)?),
                PropKind::Init | PropKind::Method => {
                    let method = p.kind == PropKind::Method;
                    let v = match &p.value {
                        Expr::Function(func) => self.function(func, method)?,
                        v => self.expr(v)?,
                    };
                    P::Init(key, v, method)
                }
                PropKind::Get | PropKind::Set => {
                    let Expr::Function(func) = &p.value else {
                        unreachable!()
                    };
                    P::Get(key, self.function(func, true)?, p.kind == PropKind::Get)
                }
            });
        }
        Ok(Box::new(move |rt, f| {
            let o = rt.object();
            for p in &parts {
                match p {
                    P::Init(k, v, method) => {
                        let k = k(rt, f)?;
                        let v = v(rt, f)?;
                        if *method {
                            set_home(&v, &o);
                        }
                        if &*k == "__proto__" {
                            if let Value::Obj(p) = &v {
                                *o.proto.borrow_mut() = Some(p.clone());
                                continue;
                            } else if matches!(v, Value::Null) {
                                *o.proto.borrow_mut() = None;
                                continue;
                            }
                        }
                        o.put_name(&k, v);
                    }
                    P::Get(k, v, getter) => {
                        let k = k(rt, f)?;
                        let v = v(rt, f)?;
                        set_home(&v, &o);
                        define_accessor(&o, &k, *getter, v);
                    }
                    P::Spread(e) => {
                        let v = e(rt, f)?;
                        if let Value::Obj(src) = &v {
                            for k in rt.own_keys(src) {
                                o.put_name(&k, rt.get(&v, &k)?);
                            }
                        } else if let Value::Str(_) = v {
                            for (i, c) in rt.list(&v)?.into_iter().enumerate() {
                                o.put(&i.to_string(), c);
                            }
                        }
                    }
                }
            }
            Ok(Value::Obj(o))
        }))
    }

    fn class(&mut self, c: &Rc<Class>) -> CR<Ex> {
        if let Some(n) = &c.name {
            self.declare(n);
        }
        let name_w = c.name.as_ref().map(|n| self.writer(n, true));
        let parent = c.extends.as_ref().map(|e| self.expr(e)).transpose()?;
        let kind = if c.extends.is_some() {
            ClassKind::Derived
        } else {
            ClassKind::Base
        };
        let ctor_fn =
            c.members
                .iter()
                .find_map(|m| match (&m.kind, &m.key, &m.value, m.is_static) {
                    (PropKind::Method, PropKey::Name(n), Some(Expr::Function(f)), false)
                        if &**n == "constructor" =>
                    {
                        Some(f.clone())
                    }
                    _ => None,
                });
        let empty = Function {
            name: c.name.clone(),
            params: Vec::new(),
            rest: None,
            body: Vec::new(),
            arrow: false,
            is_async: false,
            generator: false,
            expression: false,
            strict: true,
            start: 0,
            end: 0,
        };
        let default_ctor = ctor_fn.is_none();
        let mut ctor_ast = ctor_fn.map(|f| (*f).clone()).unwrap_or(empty);
        ctor_ast.name = c.name.clone();
        let ctor = self.function_code(&ctor_ast, true, kind, default_ctor)?;

        enum M {
            Method(Key, Ex, bool, Option<bool>),
            StaticField(Key, Option<Ex>),
            StaticBlock(Rc<FnCode>),
        }
        let mut members = Vec::new();
        let mut field_body: Vec<(Name, Option<Ex>)> = Vec::new();
        // Instance fields run as a method of the instance.
        self.scopes.push(Scope {
            blocks: vec![Vec::new()],
            slots: 0,
            global: false,
        });
        for m in &c.members {
            if m.kind == PropKind::Init && !m.is_static && m.block.is_none() {
                let k = match &m.key {
                    PropKey::Name(n) => n.clone(),
                    PropKey::Private(n) => private(n),
                    PropKey::Computed(_) => return unsupported("computed field names"),
                };
                let v = m.value.as_ref().map(|v| self.expr(v)).transpose()?;
                field_body.push((k, v));
            }
        }
        let field_scope = self.scopes.pop().unwrap();
        let fields = (!field_body.is_empty()).then(|| {
            let body: St = Box::new(move |rt, f| {
                for (k, v) in &field_body {
                    let v = match v {
                        Some(v) => v(rt, f)?,
                        None => Value::Undefined,
                    };
                    if let Value::Obj(o) = &f.this {
                        o.put_name(k, v);
                    }
                }
                Ok(Flow::Normal)
            });
            Rc::new(FnCode {
                length: 0,
                name: None,
                nslots: Cell::new(field_scope.slots),
                simple_params: Vec::new(),
                bind: None,
                body: vec![body],
                arrow: false,
                generator: false,
                strict: true,
                method: true,
                self_slot: None,
                class: ClassKind::None,
                default_ctor: false,
                params: 0,
            })
        });
        for m in &c.members {
            if let Some(block) = &m.block {
                let f = Function {
                    body: block.clone(),
                    ..ctor_ast.clone()
                };
                let mut f = f;
                f.name = None;
                members.push(M::StaticBlock(self.function_code(
                    &f,
                    true,
                    ClassKind::None,
                    false,
                )?));
                continue;
            }
            match m.kind {
                PropKind::Init if m.is_static => {
                    let k = self.key(&m.key)?;
                    self.scopes.push(Scope {
                        blocks: vec![Vec::new()],
                        slots: 0,
                        global: false,
                    });
                    let v = m.value.as_ref().map(|v| self.expr(v)).transpose()?;
                    if self.scopes.pop().unwrap().slots > 0 {
                        return unsupported("closures in static field initialisers");
                    }
                    members.push(M::StaticField(k, v));
                }
                PropKind::Init => {}
                PropKind::Method
                    if matches!(&m.key, PropKey::Name(n) if &**n == "constructor")
                        && !m.is_static => {}
                kind => {
                    let k = self.key(&m.key)?;
                    let Some(Expr::Function(func)) = &m.value else {
                        unreachable!()
                    };
                    let accessor = match kind {
                        PropKind::Get => Some(true),
                        PropKind::Set => Some(false),
                        _ => None,
                    };
                    members.push(M::Method(
                        k,
                        self.function(func, true)?,
                        m.is_static,
                        accessor,
                    ));
                }
            }
        }
        Ok(Box::new(move |rt, f| {
            let (proto_parent, ctor_parent) = match &parent {
                None => (Some(rt.object_proto.clone()), rt.function_proto.clone()),
                Some(p) => match p(rt, f)? {
                    Value::Null => (None, rt.function_proto.clone()),
                    Value::Obj(po) if po.is_callable() => {
                        let pp = match rt.get(&Value::Obj(po.clone()), "prototype")? {
                            Value::Obj(o) => Some(o),
                            Value::Null => None,
                            _ => {
                                return rt
                                    .type_error("the parent class's prototype is not an object")
                            }
                        };
                        (pp, po)
                    }
                    _ => {
                        return rt.type_error("a class extends something that isn't a constructor")
                    }
                },
            };
            let proto = Obj::new(proto_parent, Kind::Plain);
            let ctor = Obj::new(
                Some(ctor_parent),
                Kind::Closure(Closure {
                    code: ctor.clone(),
                    env: f.env.clone(),
                    lexical: None,
                    home: RefCell::new(Some(proto.clone())),
                    fields: RefCell::new(None),
                }),
            );
            let cv = Value::Obj(ctor.clone());
            ctor.props
                .borrow_mut()
                .set(&rt.prototype, Prop::Data(Value::Obj(proto.clone())));
            proto
                .props
                .borrow_mut()
                .set(&rt.constructor, Prop::Method(cv.clone()));
            if let Some(w) = &name_w {
                w(rt, f, cv.clone())?;
            }
            if let Some(code) = &fields {
                let init = make_closure(rt, f, code.clone());
                set_home(&init, &proto);
                if let Kind::Closure(c) = &ctor.kind {
                    *c.fields.borrow_mut() = Some(init);
                }
            }
            for m in &members {
                match m {
                    M::Method(k, v, is_static, accessor) => {
                        let target = if *is_static { &ctor } else { &proto };
                        let k = k(rt, f)?;
                        let v = v(rt, f)?;
                        set_home(&v, target);
                        match accessor {
                            Some(g) => define_accessor(target, &k, *g, v),
                            None => target.props.borrow_mut().set(&k, Prop::Method(v)),
                        }
                    }
                    M::StaticField(k, v) => {
                        let k = k(rt, f)?;
                        let sf = Frame {
                            env: f.env.clone(),
                            this: cv.clone(),
                            home: Some(ctor.clone()),
                            func: None,
                            new_target: Value::Undefined,
                        };
                        let v = match v {
                            Some(v) => v(rt, &sf)?,
                            None => Value::Undefined,
                        };
                        ctor.put_name(&k, v);
                    }
                    M::StaticBlock(code) => {
                        let b = make_closure(rt, f, code.clone());
                        set_home(&b, &ctor);
                        rt.call(&b, cv.clone(), &[])?;
                    }
                }
            }
            Ok(cv)
        }))
    }

    fn call(&mut self, callee: &Expr, args: &[Expr], optional: bool) -> CR<Ex> {
        let args = self.args(args)?;
        let direct = self.direct.take();
        if matches!(callee, Expr::Super) {
            return Ok(Box::new(move |rt, f| {
                let a = args(rt, f)?;
                super_call(rt, f, &a)?;
                Ok(Value::Undefined)
            }));
        }
        Ok(match callee {
            Expr::Member(o, name, opt_member) if !matches!(**o, Expr::Super) => {
                let o = self.expr(o)?;
                let name = name.clone();
                let opt_member = *opt_member;
                let hint = Cell::new(0);
                let len = &*name == "length";
                Box::new(move |rt, f| {
                    let this = o(rt, f)?;
                    if opt_member && this.is_nullish() {
                        return Err(Exc::Short);
                    }
                    let func = match &this {
                        Value::Obj(ob) => match ob.get_atom(&name, &hint, len) {
                            Some(v) => v,
                            None => rt.get(&this, &name)?,
                        },
                        _ => rt.get(&this, &name)?,
                    };
                    if optional && func.is_nullish() {
                        return Err(Exc::Short);
                    }
                    call_site(rt, f, &func, this, &args, direct.as_deref(), &name)
                })
            }
            Expr::Index(o, k, opt_member) if !matches!(**o, Expr::Super) => {
                let o = self.expr(o)?;
                let k = self.expr(k)?;
                let opt_member = *opt_member;
                Box::new(move |rt, f| {
                    let this = o(rt, f)?;
                    if opt_member && this.is_nullish() {
                        return Err(Exc::Short);
                    }
                    let key = k(rt, f)?;
                    let func = rt.get_index(&this, &key)?;
                    if optional && func.is_nullish() {
                        return Err(Exc::Short);
                    }
                    let a = args(rt, f)?;
                    rt.call(&func, this, &a)
                })
            }
            Expr::Member(_, name, _) => {
                let name = name.clone();
                Box::new(move |rt, f| {
                    let func = super_get(rt, f, &name)?;
                    let a = args(rt, f)?;
                    rt.call(&func, f.this.clone(), &a)
                })
            }
            Expr::PrivateMember(o, name) => {
                let o = self.expr(o)?;
                let name = private(name);
                Box::new(move |rt, f| {
                    let this = o(rt, f)?;
                    let func = rt.get(&this, &name)?;
                    let a = args(rt, f)?;
                    rt.call(&func, this, &a)
                })
            }
            e => {
                let name = match e {
                    Expr::Ident(n, _) => n.clone(),
                    _ => "expression".into(),
                };
                let c = self.expr(e)?;
                Box::new(move |rt, f| {
                    let func = c(rt, f)?;
                    if optional && func.is_nullish() {
                        return Err(Exc::Short);
                    }
                    call_site(
                        rt,
                        f,
                        &func,
                        Value::Undefined,
                        &args,
                        direct.as_deref(),
                        &name,
                    )
                })
            }
        })
    }

    fn unary(&mut self, op: &str, arg: &Expr) -> CR<Ex> {
        if op == "typeof" {
            let a = match arg {
                Expr::Ident(n, _) => self.reader(n, true),
                a => self.expr(a)?,
            };
            return Ok(Box::new(move |rt, f| {
                let v = a(rt, f)?;
                let _ = rt;
                Ok(Value::Str(v.type_of().into()))
            }));
        }
        if op == "delete" {
            return Ok(match arg {
                Expr::Member(o, name, _) => {
                    let o = self.expr(o)?;
                    let name = name.clone();
                    Box::new(move |rt, f| Ok(Value::Bool(delete(rt, &o(rt, f)?, &name))))
                }
                Expr::Index(o, k, _) => {
                    let o = self.expr(o)?;
                    let k = self.expr(k)?;
                    Box::new(move |rt, f| {
                        let ov = o(rt, f)?;
                        let k = rt.to_key(&k(rt, f)?)?;
                        Ok(Value::Bool(delete(rt, &ov, &k)))
                    })
                }
                Expr::Chain(inner) => {
                    let inner = self.unary("delete", inner)?;
                    Box::new(move |rt, f| match inner(rt, f) {
                        Err(Exc::Short) => Ok(Value::Bool(true)),
                        r => r,
                    })
                }
                a => {
                    let a = self.expr(a)?;
                    Box::new(move |rt, f| {
                        a(rt, f)?;
                        Ok(Value::Bool(true))
                    })
                }
            });
        }
        let a = self.expr(arg)?;
        Ok(match op {
            "!" => Box::new(move |rt, f| Ok(Value::Bool(!a(rt, f)?.truthy()))),
            "-" => Box::new(move |rt, f| Ok(Value::Num(-num(rt, a(rt, f)?)?))),
            "+" => Box::new(move |rt, f| Ok(Value::Num(num(rt, a(rt, f)?)?))),
            "~" => Box::new(move |rt, f| Ok(Value::Num(!to_int32(num(rt, a(rt, f)?)?) as f64))),
            _ => Box::new(move |rt, f| {
                a(rt, f)?;
                Ok(Value::Undefined)
            }),
        })
    }

    /// Reads and writes of one target, its object and key evaluated once.
    fn place(&mut self, target: &Expr) -> CR<Place> {
        Ok(match target {
            Expr::Ident(n, _) => {
                let r = self.reader(n, false);
                let w = self.writer(n, false);
                Box::new(move |rt, f, op| {
                    let (store, out) = op(rt, r(rt, f)?)?;
                    w(rt, f, store)?;
                    Ok(out)
                })
            }
            Expr::Member(o, name, false) | Expr::PrivateMember(o, name) => {
                let name = if matches!(target, Expr::PrivateMember(..)) {
                    private(name)
                } else {
                    name.clone()
                };
                let o = self.expr(o)?;
                Box::new(move |rt, f, op| {
                    let ov = o(rt, f)?;
                    let (store, out) = op(rt, rt.get(&ov, &name)?)?;
                    rt.set(&ov, &name, store)?;
                    Ok(out)
                })
            }
            Expr::Index(o, k, false) => {
                let o = self.expr(o)?;
                let k = self.expr(k)?;
                Box::new(move |rt, f, op| {
                    let ov = o(rt, f)?;
                    let kv = k(rt, f)?;
                    let (store, out) = op(rt, rt.get_index(&ov, &kv)?)?;
                    rt.set_index(&ov, &kv, store)?;
                    Ok(out)
                })
            }
            _ => return unsupported("this assignment target"),
        })
    }

    fn update(&mut self, op: &str, prefix: bool, target: &Expr) -> CR<Ex> {
        let d = if op == "++" { 1.0 } else { -1.0 };
        if let Expr::Ident(n, _) = target {
            if let Loc::Slot(0, i) = self.resolve(n) {
                return Ok(Box::new(move |rt, f| {
                    let mut s = f.env.slots.borrow_mut();
                    let old = match &s[i] {
                        Value::Num(x) => *x,
                        v => {
                            let v = v.clone();
                            drop(s);
                            let x = num(rt, v)?;
                            s = f.env.slots.borrow_mut();
                            x
                        }
                    };
                    s[i] = Value::Num(old + d);
                    Ok(Value::Num(if prefix { old + d } else { old }))
                }));
            }
        }
        let place = self.place(target)?;
        Ok(Box::new(move |rt, f| {
            place(rt, f, &|rt, v| {
                let old = num(rt, v)?;
                Ok((
                    Value::Num(old + d),
                    Value::Num(if prefix { old + d } else { old }),
                ))
            })
        }))
    }

    fn assign(&mut self, op: &str, target: &Pattern, value: &Expr) -> CR<Ex> {
        let v = self.expr(value)?;
        if op == "=" {
            if let Pattern::Ident(n) = target {
                if let Loc::Slot(0, i) = self.resolve(n) {
                    return Ok(Box::new(move |rt, f| {
                        let x = v(rt, f)?;
                        f.env.slots.borrow_mut()[i] = x.clone();
                        Ok(x)
                    }));
                }
            }
            if let Pattern::Expr(e) = target {
                if let Expr::Member(o, name, false) = &**e {
                    let o = self.expr(o)?;
                    let name = name.clone();
                    return Ok(Box::new(move |rt, f| {
                        let ov = o(rt, f)?;
                        let x = v(rt, f)?;
                        rt.set(&ov, &name, x.clone())?;
                        Ok(x)
                    }));
                }
                if let Expr::Index(o, k, false) = &**e {
                    let o = self.expr(o)?;
                    let k = self.expr(k)?;
                    return Ok(Box::new(move |rt, f| {
                        let ov = o(rt, f)?;
                        let kv = k(rt, f)?;
                        let x = v(rt, f)?;
                        rt.set_index(&ov, &kv, x.clone())?;
                        Ok(x)
                    }));
                }
            }
            let b = self.pattern(target, false)?;
            return Ok(Box::new(move |rt, f| {
                let x = v(rt, f)?;
                b(rt, f, x.clone())?;
                Ok(x)
            }));
        }
        let target = match target {
            Pattern::Ident(n) => Expr::Ident(n.clone(), 0),
            Pattern::Expr(e) => (**e).clone(),
            _ => return unsupported("compound assignment to a pattern"),
        };
        let place = self.place(&target)?;
        let op: &'static str = match op {
            "+=" => "+",
            "-=" => "-",
            "*=" => "*",
            "/=" => "/",
            "%=" => "%",
            "**=" => "**",
            "<<=" => "<<",
            ">>=" => ">>",
            ">>>=" => ">>>",
            "&=" => "&",
            "|=" => "|",
            "^=" => "^",
            "&&=" => "&&",
            "||=" => "||",
            _ => "??",
        };
        let logical = matches!(op, "&&" | "||" | "??");
        if logical {
            // Short-circuits: only writes when the right side is evaluated.
            let get = self.expr(&target)?;
            let set = self.pattern(&Pattern::Expr(Box::new(target)), false)?;
            return Ok(Box::new(move |rt, f| {
                let cur = get(rt, f)?;
                let go = match op {
                    "&&" => cur.truthy(),
                    "||" => !cur.truthy(),
                    _ => cur.is_nullish(),
                };
                if !go {
                    return Ok(cur);
                }
                let x = v(rt, f)?;
                set(rt, f, x.clone())?;
                Ok(x)
            }));
        }
        let f2 = binop(op);
        Ok(Box::new(move |rt, f| {
            place(rt, f, &|rt, old| {
                let x = v(rt, f)?;
                let r = f2(rt, &old, &x)?;
                Ok((r.clone(), r))
            })
        }))
    }

    /// A left-leaning chain of `+`, run as one: strings build in a single
    /// buffer instead of a new string per step.
    fn concat(&mut self, a: &Expr, b: &Expr) -> CR<Ex> {
        let mut ops = vec![b];
        let mut head = a;
        while let Expr::Binary("+", l, r) = head {
            ops.push(r);
            head = l;
        }
        ops.push(head);
        ops.reverse();
        let parts: Vec<Ex> = ops.into_iter().map(|e| self.expr(e)).collect::<CR<_>>()?;
        Ok(Box::new(move |rt, f| {
            let mut acc = parts[0](rt, f)?;
            let mut rest = parts[1..].iter();
            // Numbers add until a string turns up; from then on, append.
            for p in rest.by_ref() {
                let v = p(rt, f)?;
                acc = match (&acc, &v) {
                    (Value::Num(a), Value::Num(b)) => Value::Num(a + b),
                    (Value::Str(_), Value::Str(_) | Value::Num(_))
                    | (Value::Num(_), Value::Str(_)) => {
                        let mut buf = SCRATCH.take();
                        buf.clear();
                        append(rt, &mut buf, &acc)?;
                        append(rt, &mut buf, &v)?;
                        for p in rest.by_ref() {
                            let v = p(rt, f)?;
                            append(rt, &mut buf, &v)?;
                        }
                        let out = Value::Str(Name::from(buf.as_str()));
                        SCRATCH.set(buf);
                        return Ok(out);
                    }
                    _ => rt.add(&acc, &v)?,
                };
            }
            Ok(acc)
        }))
    }

    /// An operand read inline instead of through a closure when it is a
    /// local or a constant.
    fn opnd(&mut self, e: &Expr) -> CR<Opnd> {
        Ok(match e {
            Expr::Num(n) => Opnd::Num(*n),
            Expr::Ident(n, _) => match self.resolve(n) {
                Loc::Slot(0, i) => Opnd::Slot(i),
                _ => Opnd::Ex(self.expr(e)?),
            },
            _ => Opnd::Ex(self.expr(e)?),
        })
    }

    /// `e` evaluated for its truth only.
    fn cond(&mut self, e: &Expr) -> CR<Cnd> {
        macro_rules! cmp {
            ($a:expr, $b:expr, $num:expr, $slow:expr) => {{
                let x = self.opnd($a)?;
                let y = self.opnd($b)?;
                Ok(match (x, y) {
                    (Opnd::Slot(i), Opnd::Num(n)) => Box::new(move |rt: &Rt, f: &Frame| {
                        let a = &f.env.slots.borrow()[i];
                        if let Value::Num(a) = a {
                            let g: fn(f64, f64) -> bool = $num;
                            return Ok(g(*a, n));
                        }
                        let a = a.clone();
                        let g: fn(&Rt, &Value, &Value) -> R<bool> = $slow;
                        g(rt, &a, &Value::Num(n))
                    }),
                    (Opnd::Slot(i), Opnd::Slot(j)) => Box::new(move |rt: &Rt, f: &Frame| {
                        let s = f.env.slots.borrow();
                        if let (Value::Num(a), Value::Num(b)) = (&s[i], &s[j]) {
                            let g: fn(f64, f64) -> bool = $num;
                            return Ok(g(*a, *b));
                        }
                        let (a, b) = (s[i].clone(), s[j].clone());
                        drop(s);
                        let g: fn(&Rt, &Value, &Value) -> R<bool> = $slow;
                        g(rt, &a, &b)
                    }),
                    (x, y) => Box::new(move |rt: &Rt, f: &Frame| {
                        let a = x.get(rt, f)?;
                        let b = y.get(rt, f)?;
                        if let (Value::Num(a), Value::Num(b)) = (&a, &b) {
                            let g: fn(f64, f64) -> bool = $num;
                            return Ok(g(*a, *b));
                        }
                        let g: fn(&Rt, &Value, &Value) -> R<bool> = $slow;
                        g(rt, &a, &b)
                    }),
                })
            }};
        }
        match e {
            Expr::Binary("<", a, b) => cmp!(a, b, |a, b| a < b, |rt, a, b| Ok(
                rt.less(a, b)? == Some(true)
            )),
            Expr::Binary(">", a, b) => cmp!(a, b, |a, b| a > b, |rt, a, b| Ok(
                rt.less(b, a)? == Some(true)
            )),
            Expr::Binary("<=", a, b) => cmp!(a, b, |a, b| a <= b, |rt, a, b| Ok(
                rt.less(b, a)? == Some(false)
            )),
            Expr::Binary(">=", a, b) => cmp!(a, b, |a, b| a >= b, |rt, a, b| Ok(
                rt.less(a, b)? == Some(false)
            )),
            Expr::Binary("===", a, b) => cmp!(a, b, |a, b| a == b, |_, a, b| Ok(a.strict_eq(b))),
            Expr::Binary("!==", a, b) => cmp!(a, b, |a, b| a != b, |_, a, b| Ok(!a.strict_eq(b))),
            Expr::Unary("!", a) => {
                let c = self.cond(a)?;
                Ok(Box::new(move |rt, f| Ok(!c(rt, f)?)))
            }
            Expr::Logical("&&", a, b) => {
                let (x, y) = (self.cond(a)?, self.cond(b)?);
                Ok(Box::new(move |rt, f| Ok(x(rt, f)? && y(rt, f)?)))
            }
            Expr::Logical("||", a, b) => {
                let (x, y) = (self.cond(a)?, self.cond(b)?);
                Ok(Box::new(move |rt, f| Ok(x(rt, f)? || y(rt, f)?)))
            }
            _ => {
                let x = self.opnd(e)?;
                Ok(match x {
                    Opnd::Slot(i) => Box::new(move |_, f| Ok(f.env.slots.borrow()[i].truthy())),
                    x => Box::new(move |rt, f| Ok(x.get(rt, f)?.truthy())),
                })
            }
        }
    }

    fn binary(&mut self, op: &'static str, a: &Expr, b: &Expr) -> CR<Ex> {
        if op == "+" && matches!(a, Expr::Binary("+", ..)) {
            return self.concat(a, b);
        }
        if matches!(op, "<" | ">" | "<=" | ">=" | "===" | "!==") {
            let c = self.cond(&Expr::Binary(op, Box::new(a.clone()), Box::new(b.clone())))?;
            return Ok(Box::new(move |rt, f| Ok(Value::Bool(c(rt, f)?))));
        }
        let x = self.opnd(a)?;
        let y = self.opnd(b)?;
        macro_rules! num_op {
            ($e:expr) => {
                Box::new(move |rt: &Rt, f: &Frame| {
                    let g: fn(f64, f64) -> f64 = $e;
                    if let (Opnd::Slot(i), Opnd::Num(n)) = (&x, &y) {
                        if let Value::Num(a) = f.env.slots.borrow()[*i] {
                            return Ok(Value::Num(g(a, *n)));
                        }
                    }
                    let a = x.get(rt, f)?;
                    let b = y.get(rt, f)?;
                    if let (Value::Num(a), Value::Num(b)) = (&a, &b) {
                        return Ok(Value::Num(g(*a, *b)));
                    }
                    Ok(Value::Num(g(num(rt, a)?, num(rt, b)?)))
                })
            };
        }
        Ok(match op {
            "+" => Box::new(move |rt, f| {
                let a = x.get(rt, f)?;
                let b = y.get(rt, f)?;
                if let (Value::Num(a), Value::Num(b)) = (&a, &b) {
                    return Ok(Value::Num(a + b));
                }
                rt.add(&a, &b)
            }),
            "-" => num_op!(|a, b| a - b),
            "*" => num_op!(|a, b| a * b),
            "/" => num_op!(|a, b| a / b),
            "%" => num_op!(|a, b| a % b),
            "&" => num_op!(|a, b| (to_int32(a) & to_int32(b)) as f64),
            "|" => num_op!(|a, b| (to_int32(a) | to_int32(b)) as f64),
            "^" => num_op!(|a, b| (to_int32(a) ^ to_int32(b)) as f64),
            "<<" => num_op!(|a, b| to_int32(a).wrapping_shl(to_int32(b) as u32 & 31) as f64),
            ">>" => num_op!(|a, b| (to_int32(a) >> (to_int32(b) as u32 & 31)) as f64),
            ">>>" => num_op!(|a, b| ((to_int32(a) as u32) >> (to_int32(b) as u32 & 31)) as f64),
            op => {
                let g = binop(op);
                Box::new(move |rt, f| {
                    let a = x.get(rt, f)?;
                    let b = y.get(rt, f)?;
                    g(rt, &a, &b)
                })
            }
        })
    }
}

#[derive(Clone, Copy)]
enum Base {
    This,
    Slot(usize),
    Ex,
}

type Cnd = Box<dyn Fn(&Rt, &Frame) -> R<bool>>;

pub enum Opnd {
    Slot(usize),
    Num(f64),
    Ex(Ex),
}

impl Opnd {
    #[inline(always)]
    fn get(&self, rt: &Rt, f: &Frame) -> R<Value> {
        match self {
            Opnd::Slot(i) => Ok(f.env.slots.borrow()[*i].clone()),
            Opnd::Num(n) => Ok(Value::Num(*n)),
            Opnd::Ex(e) => e(rt, f),
        }
    }
}

type BinFn = Box<dyn Fn(&Rt, &Value, &Value) -> R<Value>>;

/// The generic form of a binary operator, chosen once per site.
fn binop(op: &'static str) -> BinFn {
    macro_rules! num {
        ($e:expr) => {
            Box::new(|rt: &Rt, a: &Value, b: &Value| {
                let g: fn(f64, f64) -> f64 = $e;
                Ok(Value::Num(g(rt.to_num(a)?, rt.to_num(b)?)))
            })
        };
    }
    match op {
        "+" => Box::new(|rt, a, b| rt.add(a, b)),
        "-" => num!(|a, b| a - b),
        "*" => num!(|a, b| a * b),
        "/" => num!(|a, b| a / b),
        "%" => num!(|a, b| a % b),
        "**" => num!(pow),
        "&" => num!(|a, b| (to_int32(a) & to_int32(b)) as f64),
        "|" => num!(|a, b| (to_int32(a) | to_int32(b)) as f64),
        "^" => num!(|a, b| (to_int32(a) ^ to_int32(b)) as f64),
        "<<" => num!(|a, b| to_int32(a).wrapping_shl(to_int32(b) as u32 & 31) as f64),
        ">>" => num!(|a, b| (to_int32(a) >> (to_int32(b) as u32 & 31)) as f64),
        ">>>" => num!(|a, b| ((to_int32(a) as u32) >> (to_int32(b) as u32 & 31)) as f64),
        "==" => Box::new(|rt, a, b| Ok(Value::Bool(rt.equals(a, b)?))),
        "!=" => Box::new(|rt, a, b| Ok(Value::Bool(!rt.equals(a, b)?))),
        "instanceof" => Box::new(|rt, a, b| Ok(Value::Bool(rt.instance_of(a, b)?))),
        "in" => Box::new(|rt, a, b| match b {
            Value::Obj(o) => {
                let k = rt.to_key(a)?;
                Ok(Value::Bool(rt.has(o, &k)))
            }
            _ => rt.type_error("right side of `in` is not an object"),
        }),
        _ => unreachable!("{op}"),
    }
}

pub fn pow(a: f64, b: f64) -> f64 {
    if b.is_nan() || (a.abs() == 1.0 && b.is_infinite()) {
        f64::NAN
    } else {
        a.powf(b)
    }
}

#[inline]
fn num(rt: &Rt, v: Value) -> R<f64> {
    match v {
        Value::Num(n) => Ok(n),
        v => rt.to_num(&v),
    }
}

fn private(n: &Name) -> Name {
    format!("\0#{n}").into()
}

fn delete(rt: &Rt, o: &Value, name: &str) -> bool {
    let _ = rt;
    if let Value::Obj(o) = o {
        if let Kind::Array(items) = &o.kind {
            if let Some(i) = index_of_name(name) {
                if let Some(x) = items.borrow_mut().get_mut(i) {
                    *x = Value::Hole;
                }
                return true;
            }
        }
        o.props.borrow_mut().remove(name);
    }
    true
}

fn set_home(v: &Value, home: &Rc<Obj>) {
    if let Value::Obj(o) = v {
        if let Kind::Closure(c) = &o.kind {
            *c.home.borrow_mut() = Some(home.clone());
        }
    }
}

fn define_accessor(o: &Rc<Obj>, k: &Name, getter: bool, f: Value) {
    let mut props = o.props.borrow_mut();
    let (mut g, mut s) = match props.get(k) {
        Some(Prop::Accessor(g, s)) => (g.clone(), s.clone()),
        _ => (None, None),
    };
    if getter {
        g = Some(f);
    } else {
        s = Some(f);
    }
    props.set(k, Prop::Accessor(g, s));
}

fn super_get(rt: &Rt, f: &Frame, name: &str) -> R<Value> {
    let Some(home) = &f.home else {
        return rt.type_error("`super` outside a method");
    };
    let Some(p) = home.proto.borrow().clone() else {
        return Ok(Value::Undefined);
    };
    match p.lookup(name) {
        None => Ok(Value::Undefined),
        Some(Prop::Data(v) | Prop::Method(v)) => Ok(v),
        Some(Prop::Accessor(Some(g), _)) => rt.call(&g, f.this.clone(), &[]),
        Some(Prop::Accessor(None, _)) => Ok(Value::Undefined),
    }
}

/// `super(...)`: the parent constructor runs on this same instance, then
/// this class's fields are set.
fn super_call(rt: &Rt, f: &Frame, args: &[Value]) -> R<()> {
    let Some(func) = &f.func else {
        return rt.type_error("`super()` outside a constructor");
    };
    let Some(parent) = func.proto.borrow().clone() else {
        return rt.type_error("no parent constructor");
    };
    let r = construct_on(rt, &Value::Obj(parent), f.this.clone(), args, &f.new_target)?;
    if let (Value::Obj(r), Value::Obj(t)) = (&r, &f.this) {
        // A parent that returned another object: take its properties.
        if !Rc::ptr_eq(r, t) {
            for (k, p) in r.props.borrow().entries() {
                t.props.borrow_mut().set(k, p.clone());
            }
        }
    }
    init_fields(rt, func, &f.this)
}

fn init_fields(rt: &Rt, func: &Rc<Obj>, this: &Value) -> R<()> {
    if let Kind::Closure(c) = &func.kind {
        let init = c.fields.borrow().clone();
        if let Some(init) = init {
            rt.call(&init, this.clone(), &[])?;
        }
    }
    Ok(())
}

fn for_in_keys(rt: &Rt, v: &Value) -> Vec<Name> {
    let mut out: Vec<Name> = Vec::new();
    let mut o = match v {
        Value::Obj(o) => Some(o.clone()),
        Value::Str(s) => return (0..str_len(s)).map(|i| Name::from(i.to_string())).collect(),
        _ => None,
    };
    while let Some(x) = o {
        for k in rt.own_keys(&x) {
            if !out.contains(&k) {
                out.push(k);
            }
        }
        o = x.proto.borrow().clone();
    }
    out
}

fn loop_flow(flow: Flow, labels: &[Name]) -> Option<R<Flow>> {
    match flow {
        Flow::Normal | Flow::Continue(None) => None,
        Flow::Break(None) => Some(Ok(Flow::Normal)),
        Flow::Break(Some(l)) if labels.contains(&l) => Some(Ok(Flow::Normal)),
        Flow::Continue(Some(l)) if labels.contains(&l) => None,
        other => Some(Ok(other)),
    }
}

fn seq(parts: Vec<St>) -> St {
    match parts.len() {
        0 => Box::new(|_, _| Ok(Flow::Normal)),
        1 => parts.into_iter().next().unwrap(),
        _ => Box::new(move |rt, f| {
            for p in &parts {
                match p(rt, f)? {
                    Flow::Normal => {}
                    other => return Ok(other),
                }
            }
            Ok(Flow::Normal)
        }),
    }
}

fn make_closure_ex(code: Rc<FnCode>) -> Ex {
    Box::new(move |rt, f| Ok(make_closure(rt, f, code.clone())))
}

pub fn make_closure(rt: &Rt, f: &Frame, code: Rc<FnCode>) -> Value {
    let lexical = code.arrow.then(|| {
        (
            f.this.clone(),
            f.home.clone(),
            f.func.clone(),
            f.new_target.clone(),
        )
    });
    let constructor = !code.arrow && !code.method;
    let o = Obj::new(
        Some(rt.function_proto.clone()),
        Kind::Closure(Closure {
            code,
            env: f.env.clone(),
            lexical,
            home: RefCell::new(None),
            fields: RefCell::new(None),
        }),
    );
    if constructor {
        let p = rt.object();
        p.props
            .borrow_mut()
            .set(&rt.constructor, Prop::Method(Value::Obj(o.clone())));
        o.props
            .borrow_mut()
            .set(&rt.prototype, Prop::Data(Value::Obj(p)));
    }
    Value::Obj(o)
}

fn call_named(rt: &Rt, f: &Value, this: Value, args: &[Value], name: &str) -> R<Value> {
    if !f.as_obj().is_some_and(|o| o.is_callable()) {
        return rt.type_error(&format!("{name} is not a function"));
    }
    rt.call(f, this, args)
}

fn construct_on(rt: &Rt, f: &Value, this: Value, args: &[Value], new_target: &Value) -> R<Value> {
    let Some(fo) = f.as_obj() else {
        return rt.type_error("not a constructor");
    };
    match &fo.kind {
        Kind::Closure(c) => run_closure(rt, fo, c, this, args, new_target.clone()),
        Kind::Native(_, n) => {
            let r = n(rt, &this, args)?;
            Ok(if matches!(r, Value::Obj(_)) { r } else { this })
        }
        Kind::Bound(t, _, bound) => {
            let all: Vec<Value> = bound.iter().chain(args).cloned().collect();
            construct_on(rt, t, this, &all, new_target)
        }
        _ => rt.type_error("not a constructor"),
    }
}

fn run_closure(
    rt: &Rt,
    fo: &Rc<Obj>,
    c: &Closure,
    this: Value,
    args: &[Value],
    new_target: Value,
) -> R<Value> {
    let code = &c.code;
    let depth = rt.depth.get() + 1;
    if depth > MAX_DEPTH {
        let p = rt.range_error_proto.clone();
        return rt.throw(&p, "too much recursion");
    }
    rt.depth.set(depth);
    struct Guard<'a>(&'a Cell<u32>);
    impl Drop for Guard<'_> {
        fn drop(&mut self) {
            self.0.set(self.0.get() - 1);
        }
    }
    let _g = Guard(&rt.depth);
    let mut env = take_env(c);
    {
        let slots = Rc::get_mut(&mut env)
            .expect("fresh environment")
            .slots
            .get_mut();
        if code.bind.is_none() {
            for (&s, a) in code.simple_params.iter().zip(args) {
                slots[s] = a.clone();
            }
        }
    }
    enter(rt, fo, c, env, this, args, new_target)
}

/// A call whose arguments are evaluated straight into the callee's slots.
fn call_site(
    rt: &Rt,
    f: &Frame,
    func: &Value,
    this: Value,
    args: &Args,
    direct: Option<&[Ex]>,
    name: &str,
) -> R<Value> {
    if let (Some(parts), Value::Obj(fo)) = (direct, func) {
        if let Kind::Closure(c) = &fo.kind {
            let code = &c.code;
            if code.bind.is_none() && code.class == ClassKind::None && !code.generator {
                let mut env = take_env(c);
                for (i, e) in parts.iter().enumerate() {
                    let v = e(rt, f)?;
                    if let Some(&s) = code.simple_params.get(i) {
                        Rc::get_mut(&mut env)
                            .expect("fresh environment")
                            .slots
                            .get_mut()[s] = v;
                    }
                }
                let depth = rt.depth.get() + 1;
                if depth > MAX_DEPTH {
                    let p = rt.range_error_proto.clone();
                    return rt.throw(&p, "too much recursion");
                }
                rt.depth.set(depth);
                let r = enter(rt, fo, c, env, this, &[], Value::Undefined);
                rt.depth.set(depth - 1);
                return r;
            }
        }
    }
    let a = args(rt, f)?;
    call_named(rt, func, this, &a, name)
}

#[inline]
fn take_env(c: &Closure) -> Rc<Env> {
    let mut env = POOL.with(|p| p.borrow_mut().pop()).unwrap_or_else(|| {
        Rc::new(Env {
            slots: RefCell::new(Vec::new()),
            parent: None,
        })
    });
    let e = Rc::get_mut(&mut env).expect("pooled environments are unshared");
    e.parent = Some(c.env.clone());
    e.slots
        .get_mut()
        .resize(c.code.nslots.get(), Value::Undefined);
    env
}

#[inline]
fn enter(
    rt: &Rt,
    fo: &Rc<Obj>,
    c: &Closure,
    mut env: Rc<Env>,
    this: Value,
    args: &[Value],
    new_target: Value,
) -> R<Value> {
    let code = &c.code;
    if let Some(s) = code.self_slot {
        Rc::get_mut(&mut env)
            .expect("fresh environment")
            .slots
            .get_mut()[s] = Value::Obj(fo.clone());
    }
    let frame = match &c.lexical {
        Some((t, h, func, nt)) => Frame {
            env,
            this: t.clone(),
            home: h.clone(),
            func: func.clone(),
            new_target: nt.clone(),
        },
        None => Frame {
            env,
            this: if !code.strict && this.is_nullish() {
                Value::Obj(rt.global.clone())
            } else {
                this
            },
            home: c.home.borrow().clone(),
            func: Some(fo.clone()),
            new_target,
        },
    };
    if let Some(b) = &code.bind {
        b(rt, &frame, args)?;
    }
    if code.class == ClassKind::Base {
        init_fields(rt, fo, &frame.this)?;
    }
    if code.default_ctor && code.class == ClassKind::Derived {
        super_call(rt, &frame, args)?;
    }
    if code.generator {
        YIELDS.with(|y| y.borrow_mut().push(Vec::new()));
        let r = run_body(&code.body, rt, &frame);
        let items = YIELDS.with(|y| y.borrow_mut().pop().unwrap());
        r?;
        return Ok(rt.generator(items));
    }
    let r = run_body(&code.body, rt, &frame);
    recycle(frame.env);
    r
}

/// Environments no closure kept, ready for reuse by the next call.
const POOL_MAX: usize = 32;

thread_local! {
    static POOL: RefCell<Vec<Rc<Env>>> = const { RefCell::new(Vec::new()) };
}

#[inline]
fn recycle(mut env: Rc<Env>) {
    let Some(e) = Rc::get_mut(&mut env) else {
        return;
    };
    let slots = e.slots.get_mut();
    if slots.capacity() > 64 {
        return;
    }
    // Values are dropped before touching the pool: dropping can free other
    // environments.
    slots.clear();
    e.parent = None;
    POOL.with(|p| {
        let mut p = p.borrow_mut();
        if p.len() < POOL_MAX {
            p.push(env);
        }
    });
}

fn run_body(body: &[St], rt: &Rt, frame: &Frame) -> R<Value> {
    for s in body {
        match s(rt, frame)? {
            Flow::Normal => {}
            Flow::Return(v) => return Ok(v),
            _ => break,
        }
    }
    Ok(Value::Undefined)
}

impl Rt {
    pub fn call(&self, f: &Value, this: Value, args: &[Value]) -> R<Value> {
        let Some(fo) = f.as_obj() else {
            return self.type_error(&format!("{} is not a function", self.to_str(f)?));
        };
        match &fo.kind {
            Kind::Closure(c) => {
                if c.code.class != ClassKind::None {
                    return self.type_error("a class constructor needs `new`");
                }
                run_closure(self, fo, c, this, args, Value::Undefined)
            }
            Kind::Native(_, n) => n(self, &this, args),
            Kind::Bound(t, bt, bound) => {
                if bound.is_empty() {
                    return self.call(t, bt.clone(), args);
                }
                let all: Vec<Value> = bound.iter().chain(args).cloned().collect();
                self.call(t, bt.clone(), &all)
            }
            _ => self.type_error("not a function"),
        }
    }

    pub fn construct(&self, f: &Value, args: &[Value], new_target: &Value) -> R<Value> {
        let Some(fo) = f.as_obj().filter(|o| o.is_callable()) else {
            return self.type_error(&format!("{} is not a constructor", self.to_str(f)?));
        };
        if let Kind::Closure(c) = &fo.kind {
            if c.code.arrow || c.code.generator || c.code.method && c.code.class == ClassKind::None
            {
                return self.type_error("not a constructor");
            }
        }
        let proto = match self.get(new_target, "prototype")? {
            Value::Obj(p) => p,
            _ => self.object_proto.clone(),
        };
        let this = Value::Obj(Obj::new(Some(proto), Kind::Plain));
        let r = construct_on(self, f, this.clone(), args, new_target)?;
        Ok(if matches!(r, Value::Obj(_)) { r } else { this })
    }
}

pub fn run_program(rt: &Rt, body: &[St], nslots: usize, vars: &[Name]) -> R<Value> {
    for v in vars {
        if rt.global.props.borrow().get(v).is_none() {
            rt.global.put_name(v, Value::Undefined);
        }
    }
    let frame = Frame {
        env: Rc::new(Env {
            slots: RefCell::new(vec![Value::Undefined; nslots]),
            parent: None,
        }),
        this: Value::Obj(rt.global.clone()),
        home: None,
        func: None,
        new_target: Value::Undefined,
    };
    let mut last = Value::Undefined;
    for s in body {
        match s(rt, &frame)? {
            Flow::Normal => {}
            Flow::Return(v) => last = v,
            _ => break,
        }
    }
    Ok(last)
}

/// `Function(params, body)`: a function compiled in the global scope.
pub fn compile_function(rt: &Rt, params: &str, body: &str) -> R<Value> {
    let src = format!("(function anonymous({params}\n) {{\n{body}\n}})");
    let compiled = crate::parser::parse(&src)
        .map_err(|e| e.message)
        .and_then(|prog| {
            let Some(Stmt::Expr(e)) = prog.body.first() else {
                return Err("expected a function".to_string());
            };
            let mut c = Compiler::new(&src);
            c.scopes.push(Scope {
                blocks: vec![Vec::new()],
                slots: 0,
                global: true,
            });
            c.expr(e).map_err(|e| e.0)
        });
    let make = match compiled {
        Ok(m) => m,
        Err(m) => {
            let p = rt.global.lookup("SyntaxError");
            return match p {
                Some(Prop::Method(f)) => {
                    Err(Exc::Throw(rt.construct(&f, &[Value::Str(m.into())], &f)?))
                }
                _ => rt.type_error(&m),
            };
        }
    };
    let frame = Frame {
        env: Rc::new(Env {
            slots: RefCell::new(Vec::new()),
            parent: None,
        }),
        this: Value::Obj(rt.global.clone()),
        home: None,
        func: None,
        new_target: Value::Undefined,
    };
    make(rt, &frame)
}

/// `eval(src)` as an indirect eval: runs in the global scope.
pub fn eval_global(rt: &Rt, src: &str) -> R<Value> {
    let compiled = crate::parser::parse(src)
        .map_err(|e| e.message)
        .and_then(|p| Compiler::new(src).program(&p).map_err(|e| e.0));
    match compiled {
        Ok((body, nslots, vars)) => run_program(rt, &body, nslots, &vars),
        Err(m) => {
            let p = rt.global.lookup("SyntaxError");
            match p {
                Some(Prop::Method(f)) => {
                    Err(Exc::Throw(rt.construct(&f, &[Value::Str(m.into())], &f)?))
                }
                _ => rt.type_error(&m),
            }
        }
    }
}

fn global_read(rt: &Rt, n: &Name, typeof_: bool) -> R<Value> {
    match rt.global.lookup(n) {
        Some(Prop::Data(v) | Prop::Method(v)) => Ok(v),
        Some(Prop::Accessor(Some(g), _)) => rt.call(&g, Value::Obj(rt.global.clone()), &[]),
        Some(Prop::Accessor(None, _)) => Ok(Value::Undefined),
        None if typeof_ => Ok(Value::Undefined),
        None => Err(throw_ref(rt, n)),
    }
}

thread_local! {
    static SCRATCH: Cell<String> = const { Cell::new(String::new()) };
}

/// Appends a `+` operand to a string being built.
fn append(rt: &Rt, buf: &mut String, v: &Value) -> R<()> {
    match v {
        Value::Str(x) => buf.push_str(x),
        Value::Num(n) => crate::push_number(buf, *n),
        Value::Obj(_) => {
            let p = rt.to_primitive(v, false)?;
            buf.push_str(&rt.to_str(&p)?);
        }
        _ => buf.push_str(&rt.to_str(v)?),
    }
    Ok(())
}

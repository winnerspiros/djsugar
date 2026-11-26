//! The syntax tree.

use std::rc::Rc;

pub type Name = crate::str::Str;
pub type P<T> = Box<T>;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VarKind {
    Var,
    Let,
    Const,
}

#[derive(Debug, Clone)]
pub enum Pattern {
    Ident(Name),
    /// `a.b` or `a[b]` as an assignment target.
    Expr(P<Expr>),
    Object(Vec<PropPattern>, Option<P<Pattern>>),
    Array(Vec<Option<Pattern>>, Option<P<Pattern>>),
    Default(P<Pattern>, P<Expr>),
}

#[derive(Debug, Clone)]
pub struct PropPattern {
    pub key: PropKey,
    pub value: Pattern,
}

#[derive(Debug, Clone)]
pub enum PropKey {
    Name(Name),
    Computed(P<Expr>),
    Private(Name),
}

#[derive(Debug, Clone)]
pub struct Function {
    pub name: Option<Name>,
    pub params: Vec<Pattern>,
    pub rest: Option<Pattern>,
    pub body: Vec<Stmt>,
    pub arrow: bool,
    pub is_async: bool,
    pub generator: bool,
    /// An arrow's `=> expr` body, held as one `return`.
    pub expression: bool,
    pub strict: bool,
    pub start: usize,
    pub end: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PropKind {
    Init,
    Get,
    Set,
    Method,
    Spread,
}

#[derive(Debug, Clone)]
pub struct Property {
    pub kind: PropKind,
    pub key: PropKey,
    pub value: Expr,
    /// `{a}` written short for `{a: a}`.
    pub shorthand: bool,
}

#[derive(Debug, Clone)]
pub struct Class {
    pub name: Option<Name>,
    pub extends: Option<P<Expr>>,
    pub members: Vec<ClassMember>,
}

#[derive(Debug, Clone)]
pub struct ClassMember {
    pub kind: PropKind,
    pub key: PropKey,
    pub is_static: bool,
    /// The method, or the field's initialiser (None for `x;`).
    pub value: Option<Expr>,
    /// A `static { }` block (kind Init, key unused).
    pub block: Option<Vec<Stmt>>,
}

#[derive(Debug, Clone)]
pub enum TemplatePart {
    Str(Option<Name>, Name),
    Expr(Expr),
}

#[derive(Debug, Clone)]
pub enum Expr {
    Num(f64),
    BigInt(Name),
    Str(Name),
    Template(Option<P<Expr>>, Vec<TemplatePart>),
    Regex(Name, Name),
    Bool(bool),
    Null,
    Undefined,
    Ident(Name, usize),
    This,
    Super,
    NewTarget,
    Array(Vec<Option<Expr>>),
    Spread(P<Expr>),
    Object(Vec<Property>),
    Function(Rc<Function>),
    Class(Rc<Class>),
    Unary(&'static str, P<Expr>),
    Update(&'static str, bool, P<Expr>),
    Binary(&'static str, P<Expr>, P<Expr>),
    Logical(&'static str, P<Expr>, P<Expr>),
    Assign(&'static str, P<Pattern>, P<Expr>),
    Cond(P<Expr>, P<Expr>, P<Expr>),
    Call(P<Expr>, Vec<Expr>, bool),
    New(P<Expr>, Vec<Expr>),
    Member(P<Expr>, Name, bool),
    Index(P<Expr>, P<Expr>, bool),
    PrivateMember(P<Expr>, Name),
    /// `#x in obj`.
    PrivateIn(Name, P<Expr>),
    /// Where a `?.` chain stops short.
    Chain(P<Expr>),
    Seq(Vec<Expr>),
    Await(P<Expr>),
    Yield(Option<P<Expr>>, bool),
}

#[derive(Debug, Clone)]
pub enum ForInit {
    Var(VarKind, Vec<(Pattern, Option<Expr>)>),
    Expr(Expr),
    /// A destructuring assignment target (`for ([a, b] of x)`).
    Pattern(Pattern),
}

#[derive(Debug, Clone)]
pub enum Stmt {
    Expr(Expr),
    Var(VarKind, Vec<(Pattern, Option<Expr>)>),
    Function(Rc<Function>),
    Class(Rc<Class>),
    Return(Option<Expr>),
    If(Expr, P<Stmt>, Option<P<Stmt>>),
    Block(Vec<Stmt>),
    For(Option<ForInit>, Option<Expr>, Option<Expr>, P<Stmt>),
    ForIn(ForInit, Expr, P<Stmt>),
    ForOf(ForInit, Expr, P<Stmt>),
    While(Expr, P<Stmt>),
    DoWhile(P<Stmt>, Expr),
    Break(Option<Name>),
    Continue(Option<Name>),
    Throw(Expr),
    Try(Vec<Stmt>, Option<(Option<Pattern>, Vec<Stmt>)>, Option<Vec<Stmt>>),
    Switch(Expr, Vec<(Option<Expr>, Vec<Stmt>)>),
    Labeled(Name, P<Stmt>),
    With(Expr, P<Stmt>),
    Debugger,
    Empty,
}

#[derive(Debug, Clone)]
pub struct Program {
    pub body: Vec<Stmt>,
    pub strict: bool,
}

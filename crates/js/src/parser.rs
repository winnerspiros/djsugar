//! Source to syntax tree: recursive descent for statements, precedence
//! climbing for operators. Arrow parameters and destructuring targets are
//! read as expressions first and turned into patterns once the `=>` or `=`
//! shows what they were.

use std::rc::Rc;

use crate::ast::*;
use crate::lexer::{Lexer, Result, SyntaxError, Tok, Token};

pub struct Parser<'a> {
    lex: Lexer<'a>,
    tok: Token,
    prev_end: usize,
    in_function: bool,
    in_generator: bool,
    in_async: bool,
    strict: bool,
}

fn binary_prec(op: &str) -> Option<u8> {
    Some(match op {
        "??" => 1,
        "||" => 2,
        "&&" => 3,
        "|" => 4,
        "^" => 5,
        "&" => 6,
        "==" | "!=" | "===" | "!==" => 7,
        "<" | ">" | "<=" | ">=" | "instanceof" | "in" => 8,
        "<<" | ">>" | ">>>" => 9,
        "+" | "-" => 10,
        "*" | "/" | "%" => 11,
        "**" => 12,
        _ => return None,
    })
}

const ASSIGN_OPS: [&str; 16] = [
    "=", "+=", "-=", "*=", "/=", "%=", "**=", "<<=", ">>=", ">>>=", "&=", "|=", "^=", "&&=", "||=",
    "??=",
];

const RESERVED: [&str; 36] = [
    "break",
    "case",
    "catch",
    "class",
    "const",
    "continue",
    "debugger",
    "default",
    "delete",
    "do",
    "else",
    "export",
    "extends",
    "finally",
    "for",
    "function",
    "if",
    "import",
    "in",
    "instanceof",
    "new",
    "return",
    "super",
    "switch",
    "this",
    "throw",
    "try",
    "typeof",
    "var",
    "void",
    "while",
    "with",
    "null",
    "true",
    "false",
    "enum",
];

fn intern(s: &str) -> &'static str {
    for op in ASSIGN_OPS {
        if op == s {
            return op;
        }
    }
    for op in [
        "??",
        "||",
        "&&",
        "|",
        "^",
        "&",
        "==",
        "!=",
        "===",
        "!==",
        "<",
        ">",
        "<=",
        ">=",
        "instanceof",
        "in",
        "<<",
        ">>",
        ">>>",
        "+",
        "-",
        "*",
        "/",
        "%",
        "**",
    ] {
        if op == s {
            return op;
        }
    }
    unreachable!("operator {s}")
}

pub fn parse(src: &str) -> Result<Program> {
    let mut p = Parser::new(src)?;
    let strict = p.directives()?;
    p.strict = strict;
    let mut body = Vec::new();
    while p.tok.tok != Tok::Eof {
        body.push(p.statement()?);
    }
    Ok(Program { body, strict })
}

impl<'a> Parser<'a> {
    fn new(src: &'a str) -> Result<Self> {
        let mut lex = Lexer::new(src);
        let tok = lex.token()?;
        Ok(Self {
            lex,
            tok,
            prev_end: 0,
            in_function: false,
            in_generator: false,
            in_async: false,
            strict: false,
        })
    }

    fn err<T>(&self, message: impl Into<String>) -> Result<T> {
        Err(SyntaxError {
            message: message.into(),
            pos: self.tok.start,
        })
    }

    fn bump(&mut self) -> Result<Token> {
        let next = self.lex.token()?;
        self.prev_end = self.tok.end;
        Ok(std::mem::replace(&mut self.tok, next))
    }

    fn peek(&self) -> Result<Token> {
        self.lex.clone().token()
    }

    fn is(&self, p: &str) -> bool {
        matches!(self.tok.tok, Tok::Punct(q) if q == p)
    }

    fn is_word(&self, w: &str) -> bool {
        matches!(&self.tok.tok, Tok::Ident(n) if &**n == w)
    }

    fn eat(&mut self, p: &str) -> Result<bool> {
        if self.is(p) {
            self.bump()?;
            Ok(true)
        } else {
            Ok(false)
        }
    }

    fn eat_word(&mut self, w: &str) -> Result<bool> {
        if self.is_word(w) {
            self.bump()?;
            Ok(true)
        } else {
            Ok(false)
        }
    }

    fn expect(&mut self, p: &str) -> Result<()> {
        if self.eat(p)? {
            Ok(())
        } else {
            self.err(format!("expected `{p}`, found {:?}", self.tok.tok))
        }
    }

    fn expect_word(&mut self, w: &str) -> Result<()> {
        if self.eat_word(w)? {
            Ok(())
        } else {
            self.err(format!("expected `{w}`"))
        }
    }

    /// A statement's end: `;`, or a line break, `}` or the end standing in.
    fn semicolon(&mut self) -> Result<()> {
        if self.eat(";")? || self.is("}") || self.tok.tok == Tok::Eof || self.tok.newline_before {
            Ok(())
        } else {
            self.err(format!("expected `;`, found {:?}", self.tok.tok))
        }
    }

    /// Any name, keywords included (after `.`, in object keys).
    fn name(&mut self) -> Result<Name> {
        match &self.tok.tok {
            Tok::Ident(n) | Tok::EscapedIdent(n) => {
                let n = n.clone();
                self.bump()?;
                Ok(n)
            }
            _ => self.err("expected a name"),
        }
    }

    /// A binding or reference name: not a reserved word.
    fn ident(&mut self) -> Result<Name> {
        match &self.tok.tok {
            Tok::Ident(n) if RESERVED.contains(&&**n) => self.err(format!("unexpected `{n}`")),
            Tok::Ident(n) if self.in_generator && &**n == "yield" => self.err("unexpected `yield`"),
            Tok::Ident(n) | Tok::EscapedIdent(n) => {
                let n = n.clone();
                self.bump()?;
                Ok(n)
            }
            _ => self.err(format!("expected a name, found {:?}", self.tok.tok)),
        }
    }

    fn is_ident(&self) -> bool {
        match &self.tok.tok {
            Tok::Ident(n) => !RESERVED.contains(&&**n),
            Tok::EscapedIdent(_) => true,
            _ => false,
        }
    }

    /// `"use strict"` and the other leading string statements.
    fn directives(&mut self) -> Result<bool> {
        let mut strict = false;
        let mut lex = self.lex.clone();
        let mut tok = self.tok.clone();
        while let Tok::Str(s) = &tok.tok {
            let raw = &self.lex_src()[tok.start + 1..tok.end - 1];
            let next = lex.clone().token()?;
            let ends = matches!(next.tok, Tok::Punct(";") | Tok::Punct("}") | Tok::Eof)
                || next.newline_before;
            if !ends {
                break;
            }
            if raw == "use strict" && &**s == "use strict" {
                strict = true;
            }
            tok = lex.token()?;
            if tok.tok == Tok::Punct(";") {
                tok = lex.token()?;
            }
        }
        Ok(strict)
    }

    fn lex_src(&self) -> &'a str {
        lexer_src(&self.lex)
    }

    fn statement(&mut self) -> Result<Stmt> {
        if let Tok::Punct(p) = self.tok.tok {
            match p {
                "{" => {
                    self.bump()?;
                    return Ok(Stmt::Block(self.block_body()?));
                }
                ";" => {
                    self.bump()?;
                    return Ok(Stmt::Empty);
                }
                _ => {}
            }
        }
        let word = match &self.tok.tok {
            Tok::Ident(w) => Some(w.clone()),
            _ => None,
        };
        if let Some(w) = word {
            match &*w {
                "var" => {
                    self.bump()?;
                    let d = self.declarations(true)?;
                    self.semicolon()?;
                    return Ok(Stmt::Var(VarKind::Var, d));
                }
                "const" => {
                    self.bump()?;
                    let d = self.declarations(true)?;
                    self.semicolon()?;
                    return Ok(Stmt::Var(VarKind::Const, d));
                }
                "let" if self.let_starts_declaration()? => {
                    self.bump()?;
                    let d = self.declarations(true)?;
                    self.semicolon()?;
                    return Ok(Stmt::Var(VarKind::Let, d));
                }
                "function" => {
                    let start = self.tok.start;
                    self.bump()?;
                    return Ok(Stmt::Function(self.function(start, false, true)?));
                }
                "async" => {
                    let next = self.peek()?;
                    if !next.newline_before
                        && matches!(&next.tok, Tok::Ident(n) if &**n == "function")
                    {
                        let start = self.tok.start;
                        self.bump()?;
                        self.bump()?;
                        return Ok(Stmt::Function(self.function(start, true, true)?));
                    }
                }
                "class" => {
                    self.bump()?;
                    return Ok(Stmt::Class(self.class(true)?));
                }
                "if" => {
                    self.bump()?;
                    self.expect("(")?;
                    let c = self.expression(true)?;
                    self.expect(")")?;
                    let then = Box::new(self.statement()?);
                    let other = if self.eat_word("else")? {
                        Some(Box::new(self.statement()?))
                    } else {
                        None
                    };
                    return Ok(Stmt::If(c, then, other));
                }
                "return" => {
                    if !self.in_function {
                        return self.err("`return` outside a function");
                    }
                    self.bump()?;
                    let v = if self.is(";")
                        || self.is("}")
                        || self.tok.tok == Tok::Eof
                        || self.tok.newline_before
                    {
                        None
                    } else {
                        Some(self.expression(true)?)
                    };
                    self.semicolon()?;
                    return Ok(Stmt::Return(v));
                }
                "while" => {
                    self.bump()?;
                    self.expect("(")?;
                    let c = self.expression(true)?;
                    self.expect(")")?;
                    return Ok(Stmt::While(c, Box::new(self.statement()?)));
                }
                "do" => {
                    self.bump()?;
                    let body = Box::new(self.statement()?);
                    self.expect_word("while")?;
                    self.expect("(")?;
                    let c = self.expression(true)?;
                    self.expect(")")?;
                    self.eat(";")?;
                    return Ok(Stmt::DoWhile(body, c));
                }
                "for" => return self.for_statement(),
                "break" | "continue" => {
                    self.bump()?;
                    let label = if !self.tok.newline_before && self.is_ident() {
                        Some(self.ident()?)
                    } else {
                        None
                    };
                    self.semicolon()?;
                    return Ok(if &*w == "break" {
                        Stmt::Break(label)
                    } else {
                        Stmt::Continue(label)
                    });
                }
                "throw" => {
                    self.bump()?;
                    if self.tok.newline_before {
                        return self.err("line break after `throw`");
                    }
                    let e = self.expression(true)?;
                    self.semicolon()?;
                    return Ok(Stmt::Throw(e));
                }
                "try" => return self.try_statement(),
                "switch" => return self.switch_statement(),
                "with" => {
                    if self.strict {
                        return self.err("`with` in strict code");
                    }
                    self.bump()?;
                    self.expect("(")?;
                    let o = self.expression(true)?;
                    self.expect(")")?;
                    return Ok(Stmt::With(o, Box::new(self.statement()?)));
                }
                "debugger" => {
                    self.bump()?;
                    self.semicolon()?;
                    return Ok(Stmt::Debugger);
                }
                "import" | "export"
                    if !matches!(self.peek()?.tok, Tok::Punct("(") | Tok::Punct(".")) =>
                {
                    return self.err("modules are not supported in mapping scripts");
                }
                _ => {}
            }
            if self.is_ident() && self.peek()?.tok == Tok::Punct(":") {
                let label = self.ident()?;
                self.bump()?;
                return Ok(Stmt::Labeled(label, Box::new(self.statement()?)));
            }
        }
        let e = self.expression(true)?;
        self.semicolon()?;
        Ok(Stmt::Expr(e))
    }

    fn let_starts_declaration(&self) -> Result<bool> {
        let next = self.peek()?;
        Ok(match &next.tok {
            Tok::Punct("[") | Tok::Punct("{") => true,
            Tok::Ident(n) => {
                !(next.newline_before && RESERVED.contains(&&**n))
                    && &**n != "in"
                    && &**n != "instanceof"
            }
            Tok::EscapedIdent(_) => true,
            _ => false,
        })
    }

    fn block_body(&mut self) -> Result<Vec<Stmt>> {
        let mut body = Vec::new();
        while !self.eat("}")? {
            if self.tok.tok == Tok::Eof {
                return self.err("missing `}`");
            }
            body.push(self.statement()?);
        }
        Ok(body)
    }

    fn declarations(&mut self, allow_in: bool) -> Result<Vec<(Pattern, Option<Expr>)>> {
        let mut out = Vec::new();
        loop {
            let target = self.binding_target()?;
            let init = if self.eat("=")? {
                Some(self.assignment(allow_in)?)
            } else {
                None
            };
            out.push((target, init));
            if !self.eat(",")? {
                break;
            }
        }
        Ok(out)
    }

    fn binding_target(&mut self) -> Result<Pattern> {
        if self.is("[") || self.is("{") {
            let e = self.primary()?;
            to_pattern(e, true).ok_or_else(|| SyntaxError {
                message: "bad destructuring pattern".into(),
                pos: self.prev_end,
            })
        } else {
            Ok(Pattern::Ident(self.ident()?))
        }
    }

    fn binding_element(&mut self) -> Result<Pattern> {
        let target = self.binding_target()?;
        if self.eat("=")? {
            Ok(Pattern::Default(
                Box::new(target),
                Box::new(self.assignment(true)?),
            ))
        } else {
            Ok(target)
        }
    }

    fn for_statement(&mut self) -> Result<Stmt> {
        self.bump()?;
        if self.is_word("await") {
            return self.err("`for await` is not supported");
        }
        self.expect("(")?;
        let init = if self.is(";") {
            None
        } else if self.is_word("var")
            || self.is_word("const")
            || (self.is_word("let") && self.let_starts_declaration()?)
        {
            let kind = match &self.tok.tok {
                Tok::Ident(n) if &**n == "var" => VarKind::Var,
                Tok::Ident(n) if &**n == "const" => VarKind::Const,
                _ => VarKind::Let,
            };
            self.bump()?;
            Some(ForInit::Var(kind, self.declarations(false)?))
        } else {
            Some(ForInit::Expr(self.expression(false)?))
        };
        if let Some(init) = &init {
            let of = self.is_word("of");
            if of || self.is_word("in") {
                self.bump()?;
                let init = match init.clone() {
                    ForInit::Var(k, mut d) if d.len() == 1 => {
                        let (pat, value) = d.pop().unwrap();
                        if value.is_some() && (of || k != VarKind::Var || self.strict) {
                            return self.err("initialiser in a for-in/of head");
                        }
                        ForInit::Var(k, vec![(pat, value)])
                    }
                    ForInit::Var(..) => return self.err("one binding in a for-in/of head"),
                    other => other,
                };
                let target = match init {
                    ForInit::Expr(e) => match to_pattern(e, false) {
                        Some(Pattern::Ident(n)) => ForInit::Expr(Expr::Ident(n, 0)),
                        Some(p) => ForInit::Pattern(p),
                        None => return self.err("bad for-in/of target"),
                    },
                    v => v,
                };
                let obj = if of {
                    self.assignment(true)?
                } else {
                    self.expression(true)?
                };
                self.expect(")")?;
                let body = Box::new(self.statement()?);
                return Ok(if of {
                    Stmt::ForOf(target, obj, body)
                } else {
                    Stmt::ForIn(target, obj, body)
                });
            }
        }
        self.expect(";")?;
        let test = if self.is(";") {
            None
        } else {
            Some(self.expression(true)?)
        };
        self.expect(";")?;
        let update = if self.is(")") {
            None
        } else {
            Some(self.expression(true)?)
        };
        self.expect(")")?;
        let body = Box::new(self.statement()?);
        Ok(Stmt::For(init, test, update, body))
    }

    fn try_statement(&mut self) -> Result<Stmt> {
        self.bump()?;
        self.expect("{")?;
        let body = self.block_body()?;
        let catch = if self.eat_word("catch")? {
            let param = if self.eat("(")? {
                let p = self.binding_target()?;
                self.expect(")")?;
                Some(p)
            } else {
                None
            };
            self.expect("{")?;
            Some((param, self.block_body()?))
        } else {
            None
        };
        let finally = if self.eat_word("finally")? {
            self.expect("{")?;
            Some(self.block_body()?)
        } else {
            None
        };
        if catch.is_none() && finally.is_none() {
            return self.err("`try` without `catch` or `finally`");
        }
        Ok(Stmt::Try(body, catch, finally))
    }

    fn switch_statement(&mut self) -> Result<Stmt> {
        self.bump()?;
        self.expect("(")?;
        let d = self.expression(true)?;
        self.expect(")")?;
        self.expect("{")?;
        let mut cases = Vec::new();
        let mut seen_default = false;
        while !self.eat("}")? {
            let test = if self.eat_word("case")? {
                Some(self.expression(true)?)
            } else {
                self.expect_word("default")?;
                if seen_default {
                    return self.err("two `default`s in a switch");
                }
                seen_default = true;
                None
            };
            self.expect(":")?;
            let mut body = Vec::new();
            while !(self.is("}") || self.is_word("case") || self.is_word("default")) {
                if self.tok.tok == Tok::Eof {
                    return self.err("missing `}`");
                }
                body.push(self.statement()?);
            }
            cases.push((test, body));
        }
        Ok(Stmt::Switch(d, cases))
    }

    /// After `function` (and `async`): `*`, the name, parameters and body.
    fn function(&mut self, start: usize, is_async: bool, statement: bool) -> Result<Rc<Function>> {
        let generator = self.eat("*")?;
        let name = if self.is_ident()
            || matches!(&self.tok.tok, Tok::Ident(n) if &**n == "yield" || &**n == "await")
        {
            Some(self.name()?)
        } else if statement {
            return self.err("a function statement needs a name");
        } else {
            None
        };
        self.function_rest(start, name, is_async, generator)
    }

    fn function_rest(
        &mut self,
        start: usize,
        name: Option<Name>,
        is_async: bool,
        generator: bool,
    ) -> Result<Rc<Function>> {
        let saved = (
            self.in_function,
            self.in_generator,
            self.in_async,
            self.strict,
        );
        self.in_function = true;
        self.in_generator = generator;
        self.in_async = is_async;
        self.expect("(")?;
        let mut params = Vec::new();
        let mut rest = None;
        while !self.eat(")")? {
            if self.eat("...")? {
                rest = Some(self.binding_target()?);
                self.expect(")")?;
                break;
            }
            params.push(self.binding_element()?);
            if !self.is(")") {
                self.expect(",")?;
            }
        }
        self.expect("{")?;
        let strict = self.directives()? || saved.3;
        self.strict = strict;
        let body = self.block_body()?;
        let end = self.prev_end;
        (
            self.in_function,
            self.in_generator,
            self.in_async,
            self.strict,
        ) = saved;
        Ok(Rc::new(Function {
            name,
            params,
            rest,
            body,
            arrow: false,
            is_async,
            generator,
            expression: false,
            strict,
            start,
            end,
        }))
    }

    fn class(&mut self, statement: bool) -> Result<Rc<Class>> {
        let saved = self.strict;
        self.strict = true;
        let name = if self.is_ident() {
            Some(self.ident()?)
        } else if statement {
            return self.err("a class statement needs a name");
        } else {
            None
        };
        let extends = if self.eat_word("extends")? {
            Some(Box::new(self.unary_postfix()?))
        } else {
            None
        };
        self.expect("{")?;
        let mut members = Vec::new();
        while !self.eat("}")? {
            if self.eat(";")? {
                continue;
            }
            let start = self.tok.start;
            let mut is_static = false;
            if self.is_word("static")
                && !matches!(
                    self.peek()?.tok,
                    Tok::Punct("(") | Tok::Punct("=") | Tok::Punct(";") | Tok::Punct("}")
                )
            {
                self.bump()?;
                is_static = true;
                if self.is("{") {
                    self.bump()?;
                    let f = self.in_function;
                    self.in_function = false;
                    let block = self.block_body()?;
                    self.in_function = f;
                    members.push(ClassMember {
                        kind: PropKind::Init,
                        key: PropKey::Name("".into()),
                        is_static: true,
                        value: None,
                        block: Some(block),
                    });
                    continue;
                }
            }
            let (kind, key, value) = self.method_or_key(start, true)?;
            let value = match kind {
                PropKind::Init => {
                    let v = if self.eat("=")? {
                        let f = (self.in_function, self.in_generator, self.in_async);
                        self.in_function = true;
                        self.in_generator = false;
                        self.in_async = false;
                        let v = self.assignment(true)?;
                        (self.in_function, self.in_generator, self.in_async) = f;
                        Some(v)
                    } else {
                        None
                    };
                    self.semicolon()?;
                    v
                }
                _ => value,
            };
            members.push(ClassMember {
                kind,
                key,
                is_static,
                value,
                block: None,
            });
        }
        self.strict = saved;
        Ok(Rc::new(Class {
            name,
            extends,
            members,
        }))
    }

    /// A property key, then (for methods and accessors) the function after
    /// it. In a class, a plain key is a field; in an object it needs `:`.
    fn method_or_key(
        &mut self,
        start: usize,
        class: bool,
    ) -> Result<(PropKind, PropKey, Option<Expr>)> {
        let mut is_async = false;
        let mut generator = false;
        let mut accessor = None;
        let followed_by_key = |p: &Self| -> Result<bool> {
            let n = p.peek()?;
            Ok(!n.newline_before
                || !matches!(n.tok, Tok::Punct(_))
                || matches!(n.tok, Tok::Punct("[") | Tok::Punct("*")))
            .map(|ok| {
                ok && !matches!(
                    n.tok,
                    Tok::Punct("(")
                        | Tok::Punct(",")
                        | Tok::Punct(":")
                        | Tok::Punct("=")
                        | Tok::Punct("}")
                        | Tok::Punct(";")
                )
            })
        };
        if self.is_word("async") && followed_by_key(self)? && !self.peek()?.newline_before {
            self.bump()?;
            is_async = true;
        }
        if self.eat("*")? {
            generator = true;
        }
        if !is_async
            && !generator
            && (self.is_word("get") || self.is_word("set"))
            && followed_by_key(self)?
        {
            accessor = Some(if self.is_word("get") {
                PropKind::Get
            } else {
                PropKind::Set
            });
            self.bump()?;
        }
        let key = self.prop_key()?;
        if self.is("(") {
            let name = match &key {
                PropKey::Name(n) => Some(n.clone()),
                _ => None,
            };
            let f = self.function_rest(start, name, is_async, generator)?;
            return Ok((
                accessor.unwrap_or(PropKind::Method),
                key,
                Some(Expr::Function(f)),
            ));
        }
        if is_async || generator || accessor.is_some() {
            return self.err("expected `(`");
        }
        if !class && !self.is(":") {
            // Shorthand, handled by the caller.
            return Ok((PropKind::Init, key, None));
        }
        Ok((PropKind::Init, key, None))
    }

    fn prop_key(&mut self) -> Result<PropKey> {
        let t = self.bump()?;
        Ok(match t.tok {
            Tok::Ident(n) | Tok::EscapedIdent(n) | Tok::Str(n) => PropKey::Name(n),
            Tok::Num(v) => PropKey::Name(crate::number_to_string(v).into()),
            Tok::BigInt(n) => PropKey::Name(n),
            Tok::Private(n) => PropKey::Private(n),
            Tok::Punct("[") => {
                let e = self.assignment(true)?;
                self.expect("]")?;
                PropKey::Computed(Box::new(e))
            }
            other => {
                return Err(SyntaxError {
                    message: format!("expected a property name, found {other:?}"),
                    pos: t.start,
                })
            }
        })
    }

    pub fn expression(&mut self, allow_in: bool) -> Result<Expr> {
        let first = self.assignment(allow_in)?;
        if !self.is(",") {
            return Ok(first);
        }
        let mut all = vec![first];
        while self.eat(",")? {
            all.push(self.assignment(allow_in)?);
        }
        Ok(Expr::Seq(all))
    }

    fn assignment(&mut self, allow_in: bool) -> Result<Expr> {
        if let Some(arrow) = self.try_arrow(allow_in)? {
            return Ok(arrow);
        }
        if self.in_generator && self.is_word("yield") {
            self.bump()?;
            if self.tok.newline_before
                || matches!(
                    self.tok.tok,
                    Tok::Punct(")")
                        | Tok::Punct("]")
                        | Tok::Punct("}")
                        | Tok::Punct(",")
                        | Tok::Punct(";")
                        | Tok::Punct(":")
                )
                || self.tok.tok == Tok::Eof
            {
                return Ok(Expr::Yield(None, false));
            }
            let delegate = self.eat("*")?;
            return Ok(Expr::Yield(
                Some(Box::new(self.assignment(allow_in)?)),
                delegate,
            ));
        }
        let start = self.tok.start;
        let left = self.conditional(allow_in)?;
        if let Tok::Punct(op) = self.tok.tok {
            if ASSIGN_OPS.contains(&op) {
                let target = if op == "=" {
                    to_pattern(left, false)
                } else {
                    match left {
                        e @ (Expr::Ident(..)
                        | Expr::Member(..)
                        | Expr::Index(..)
                        | Expr::PrivateMember(..)) => Some(simple_pattern(e)),
                        _ => None,
                    }
                };
                let Some(target) = target else {
                    return Err(SyntaxError {
                        message: "invalid assignment target".into(),
                        pos: start,
                    });
                };
                self.bump()?;
                let value = self.assignment(allow_in)?;
                return Ok(Expr::Assign(intern(op), Box::new(target), Box::new(value)));
            }
        }
        Ok(left)
    }

    /// An arrow function, if one starts here.
    fn try_arrow(&mut self, allow_in: bool) -> Result<Option<Expr>> {
        let start = self.tok.start;
        let mut is_async = false;
        if self.is_word("async") {
            let n = self.peek()?;
            if !n.newline_before
                && (matches!(n.tok, Tok::Punct("("))
                    || matches!(&n.tok, Tok::Ident(w) if !RESERVED.contains(&&**w))
                    || matches!(n.tok, Tok::EscapedIdent(_)))
            {
                let save = (self.lex.clone(), self.tok.clone(), self.prev_end);
                self.bump()?;
                if self.is_ident() && self.peek()?.tok == Tok::Punct("=>") {
                    is_async = true;
                } else if self.is("(") {
                    if let Some(params) = self.arrow_params()? {
                        return self.arrow_body(start, params, true, allow_in).map(Some);
                    }
                    (self.lex, self.tok, self.prev_end) = save;
                    return Ok(None);
                } else {
                    (self.lex, self.tok, self.prev_end) = save;
                    return Ok(None);
                }
            }
        }
        if self.is_ident() && self.peek()?.tok == Tok::Punct("=>") {
            let name = self.ident()?;
            return self
                .arrow_body(
                    start,
                    (vec![Pattern::Ident(name)], None),
                    is_async,
                    allow_in,
                )
                .map(Some);
        }
        if self.is("(") {
            let save = (self.lex.clone(), self.tok.clone(), self.prev_end);
            if let Some(params) = self.arrow_params()? {
                return self.arrow_body(start, params, false, allow_in).map(Some);
            }
            (self.lex, self.tok, self.prev_end) = save;
        }
        Ok(None)
    }

    /// `( params ) =>` if that's what follows; otherwise None (the caller
    /// rewinds).
    fn arrow_params(&mut self) -> Result<Option<(Vec<Pattern>, Option<Pattern>)>> {
        if !self.arrow_follows()? {
            return Ok(None);
        }
        self.bump()?;
        let mut params = Vec::new();
        let mut rest = None;
        while !self.eat(")")? {
            if self.eat("...")? {
                rest = Some(self.binding_target()?);
                self.expect(")")?;
                break;
            }
            params.push(self.binding_element()?);
            if !self.is(")") {
                self.expect(",")?;
            }
        }
        Ok(Some((params, rest)))
    }

    /// Scans to the `)` matching the current `(`; whether `=>` follows it.
    fn arrow_follows(&self) -> Result<bool> {
        let mut probe = self.lex.clone();
        // true where the open bracket is a template substitution.
        let mut stack = vec![false];
        let mut prev = self.tok.tok.clone();
        loop {
            let Ok(t) = probe.token() else {
                return Ok(false);
            };
            match &t.tok {
                Tok::Punct("(" | "[" | "{") => stack.push(false),
                Tok::Template(_, _, false) => stack.push(true),
                Tok::Punct(")" | "]" | "}") => {
                    if stack.pop() == Some(true) {
                        let Ok(rest) = probe.template_continue(t.start) else {
                            return Ok(false);
                        };
                        if matches!(rest.tok, Tok::Template(_, _, false)) {
                            stack.push(true);
                        }
                        prev = rest.tok;
                        continue;
                    }
                    if stack.is_empty() {
                        let after = probe.token();
                        return Ok(
                            matches!(after, Ok(a) if a.tok == Tok::Punct("=>") && !a.newline_before),
                        );
                    }
                }
                Tok::Eof => return Ok(false),
                Tok::Punct("/" | "/=") if regex_allowed_after(&prev) => {
                    let Ok(r) = probe.regex(t.start, t.newline_before) else {
                        return Ok(false);
                    };
                    prev = r.tok;
                    continue;
                }
                _ => {}
            }
            prev = t.tok;
        }
    }

    fn arrow_body(
        &mut self,
        start: usize,
        (params, rest): (Vec<Pattern>, Option<Pattern>),
        is_async: bool,
        allow_in: bool,
    ) -> Result<Expr> {
        self.expect("=>")?;
        let saved = (
            self.in_function,
            self.in_generator,
            self.in_async,
            self.strict,
        );
        self.in_function = true;
        self.in_generator = false;
        self.in_async = is_async;
        let (body, expression, strict) = if self.eat("{")? {
            let strict = self.directives()? || saved.3;
            self.strict = strict;
            (self.block_body()?, false, strict)
        } else {
            let e = self.assignment(allow_in)?;
            (vec![Stmt::Return(Some(e))], true, saved.3)
        };
        let end = self.prev_end;
        (
            self.in_function,
            self.in_generator,
            self.in_async,
            self.strict,
        ) = saved;
        Ok(Expr::Function(Rc::new(Function {
            name: None,
            params,
            rest,
            body,
            arrow: true,
            is_async,
            generator: false,
            expression,
            strict,
            start,
            end,
        })))
    }

    fn conditional(&mut self, allow_in: bool) -> Result<Expr> {
        let test = self.binary(0, allow_in)?;
        if !self.eat("?")? {
            return Ok(test);
        }
        let then = self.assignment(true)?;
        self.expect(":")?;
        let other = self.assignment(allow_in)?;
        Ok(Expr::Cond(Box::new(test), Box::new(then), Box::new(other)))
    }

    fn binary_op(&self, allow_in: bool) -> Option<(&'static str, u8)> {
        let op: &str = match &self.tok.tok {
            Tok::Punct(p) => p,
            Tok::Ident(w) if &**w == "instanceof" => "instanceof",
            Tok::Ident(w) if &**w == "in" && allow_in => "in",
            _ => return None,
        };
        binary_prec(op).map(|p| (intern(op), p))
    }

    fn binary(&mut self, min: u8, allow_in: bool) -> Result<Expr> {
        let mut left = if let Tok::Private(n) = &self.tok.tok {
            // `#x in obj`
            let n = n.clone();
            self.bump()?;
            if !self.is_word("in") {
                return self.err("a private name stands only before `in`");
            }
            self.bump()?;
            let right = self.binary(9, allow_in)?;
            Expr::PrivateIn(n, Box::new(right))
        } else {
            self.unary()?
        };
        while let Some((op, prec)) = self.binary_op(allow_in) {
            if prec <= min {
                break;
            }
            if op == "**" && matches!(left, Expr::Unary(..)) {
                return self.err("parenthesise a unary operand of `**`");
            }
            self.bump()?;
            // `**` groups to the right.
            let right = self.binary(if op == "**" { prec - 1 } else { prec }, allow_in)?;
            left = match op {
                "&&" | "||" | "??" => {
                    let mixes = |e: &Expr| matches!(e, Expr::Logical(o, ..) if (*o == "??") != (op == "??"));
                    if mixes(&left) || mixes(&right) {
                        return self.err("mix `??` with `&&`/`||` only in parentheses");
                    }
                    Expr::Logical(op, Box::new(left), Box::new(right))
                }
                _ => Expr::Binary(op, Box::new(left), Box::new(right)),
            };
        }
        Ok(left)
    }

    fn unary(&mut self) -> Result<Expr> {
        let op = match &self.tok.tok {
            Tok::Punct(p @ ("!" | "~" | "+" | "-")) => Some(*p),
            Tok::Ident(w) if matches!(&**w, "typeof" | "void" | "delete") => Some(match &**w {
                "typeof" => "typeof",
                "void" => "void",
                _ => "delete",
            }),
            _ => None,
        };
        if let Some(op) = op {
            self.bump()?;
            let arg = self.unary()?;
            if op == "delete" && self.strict && matches!(arg, Expr::Ident(..)) {
                return self.err("`delete` of a plain name in strict code");
            }
            return Ok(Expr::Unary(op, Box::new(arg)));
        }
        if self.is("++") || self.is("--") {
            let op = if self.is("++") { "++" } else { "--" };
            self.bump()?;
            let arg = self.unary()?;
            return self.update(op, true, arg);
        }
        if self.in_async && self.is_word("await") {
            self.bump()?;
            return Ok(Expr::Await(Box::new(self.unary()?)));
        }
        let e = self.unary_postfix()?;
        if (self.is("++") || self.is("--")) && !self.tok.newline_before {
            let op = if self.is("++") { "++" } else { "--" };
            self.bump()?;
            return self.update(op, false, e);
        }
        Ok(e)
    }

    fn update(&self, op: &'static str, prefix: bool, arg: Expr) -> Result<Expr> {
        match arg {
            Expr::Ident(..) | Expr::Member(..) | Expr::Index(..) | Expr::PrivateMember(..) => {
                Ok(Expr::Update(op, prefix, Box::new(arg)))
            }
            _ => self.err(format!("invalid `{op}` operand")),
        }
    }

    /// Member access, calls, `new`, and template tags.
    fn unary_postfix(&mut self) -> Result<Expr> {
        let mut e = if self.is_word("new") {
            self.bump()?;
            if self.eat(".")? {
                let n = self.name()?;
                if &*n != "target" {
                    return self.err("expected `new.target`");
                }
                Expr::NewTarget
            } else {
                let callee = self.member_only()?;
                let args = if self.is("(") {
                    self.arguments()?
                } else {
                    Vec::new()
                };
                Expr::New(Box::new(callee), args)
            }
        } else {
            self.primary()?
        };
        let mut chained = false;
        loop {
            if self.eat(".")? {
                e = match &self.tok.tok {
                    Tok::Private(n) => {
                        let n = n.clone();
                        self.bump()?;
                        Expr::PrivateMember(Box::new(e), n)
                    }
                    _ => Expr::Member(Box::new(e), self.name()?, false),
                };
            } else if self.eat("?.")? {
                chained = true;
                e = if self.is("(") {
                    Expr::Call(Box::new(e), self.arguments()?, true)
                } else if self.eat("[")? {
                    let i = self.expression(true)?;
                    self.expect("]")?;
                    Expr::Index(Box::new(e), Box::new(i), true)
                } else {
                    Expr::Member(Box::new(e), self.name()?, true)
                };
            } else if self.eat("[")? {
                let i = self.expression(true)?;
                self.expect("]")?;
                e = Expr::Index(Box::new(e), Box::new(i), false);
            } else if self.is("(") {
                e = Expr::Call(Box::new(e), self.arguments()?, false);
            } else if matches!(self.tok.tok, Tok::Template(..)) {
                if chained {
                    return self.err("a template after `?.`");
                }
                e = self.template(Some(e))?;
            } else {
                break;
            }
        }
        Ok(if chained { Expr::Chain(Box::new(e)) } else { e })
    }

    /// `new`'s callee: member access but no call.
    fn member_only(&mut self) -> Result<Expr> {
        let mut e = if self.is_word("new") {
            self.bump()?;
            let callee = self.member_only()?;
            let args = if self.is("(") {
                self.arguments()?
            } else {
                Vec::new()
            };
            Expr::New(Box::new(callee), args)
        } else {
            self.primary()?
        };
        loop {
            if self.eat(".")? {
                e = match &self.tok.tok {
                    Tok::Private(n) => {
                        let n = n.clone();
                        self.bump()?;
                        Expr::PrivateMember(Box::new(e), n)
                    }
                    _ => Expr::Member(Box::new(e), self.name()?, false),
                };
            } else if self.eat("[")? {
                let i = self.expression(true)?;
                self.expect("]")?;
                e = Expr::Index(Box::new(e), Box::new(i), false);
            } else if matches!(self.tok.tok, Tok::Template(..)) {
                e = self.template(Some(e))?;
            } else {
                return Ok(e);
            }
        }
    }

    fn arguments(&mut self) -> Result<Vec<Expr>> {
        self.expect("(")?;
        let mut args = Vec::new();
        while !self.eat(")")? {
            if self.eat("...")? {
                args.push(Expr::Spread(Box::new(self.assignment(true)?)));
            } else {
                args.push(self.assignment(true)?);
            }
            if !self.is(")") {
                self.expect(",")?;
            }
        }
        Ok(args)
    }

    fn template(&mut self, tag: Option<Expr>) -> Result<Expr> {
        let mut parts = Vec::new();
        let mut t = self.tok.clone();
        loop {
            let Tok::Template(cooked, raw, done) = t.tok else {
                return self.err("expected template text");
            };
            if cooked.is_none() && tag.is_none() {
                return Err(SyntaxError {
                    message: "bad escape in template".into(),
                    pos: t.start,
                });
            }
            parts.push(TemplatePart::Str(cooked, raw));
            if done {
                self.bump()?;
                break;
            }
            self.bump()?;
            parts.push(TemplatePart::Expr(self.expression(true)?));
            if !self.is("}") {
                return self.err("expected `}` in template");
            }
            t = self.lex.template_continue(self.tok.start)?;
            self.tok = t.clone();
        }
        Ok(Expr::Template(tag.map(Box::new), parts))
    }

    fn primary(&mut self) -> Result<Expr> {
        let t = self.tok.clone();
        match &t.tok {
            Tok::Num(v) => {
                self.bump()?;
                Ok(Expr::Num(*v))
            }
            Tok::BigInt(s) => {
                self.bump()?;
                Ok(Expr::BigInt(s.clone()))
            }
            Tok::Str(s) => {
                self.bump()?;
                Ok(Expr::Str(s.clone()))
            }
            Tok::Template(..) => self.template(None),
            Tok::Punct("/") | Tok::Punct("/=") => {
                let r = self.lex.regex(t.start, t.newline_before)?;
                self.tok = r;
                let Tok::Regex(body, flags) = self.bump()?.tok else {
                    unreachable!()
                };
                Ok(Expr::Regex(body, flags))
            }
            Tok::Punct("(") => {
                self.bump()?;
                let e = self.expression(true)?;
                self.expect(")")?;
                // Parentheses stop `a = 1` targets being patterns; keep
                // simple targets as they are.
                Ok(match e {
                    Expr::Object(_) | Expr::Array(_) => Expr::Seq(vec![e]),
                    e => e,
                })
            }
            Tok::Punct("[") => {
                self.bump()?;
                let mut items = Vec::new();
                while !self.eat("]")? {
                    if self.is(",") {
                        self.bump()?;
                        items.push(None);
                        continue;
                    }
                    let item = if self.eat("...")? {
                        Expr::Spread(Box::new(self.assignment(true)?))
                    } else {
                        self.assignment(true)?
                    };
                    items.push(Some(item));
                    if !self.is("]") {
                        self.expect(",")?;
                    }
                }
                Ok(Expr::Array(items))
            }
            Tok::Punct("{") => self.object(),
            Tok::Ident(w) => {
                let w = w.clone();
                match &*w {
                    "function" => {
                        self.bump()?;
                        Ok(Expr::Function(self.function(t.start, false, false)?))
                    }
                    "async"
                        if matches!(&self.peek()?.tok, Tok::Ident(n) if &**n == "function")
                            && !self.peek()?.newline_before =>
                    {
                        self.bump()?;
                        self.bump()?;
                        Ok(Expr::Function(self.function(t.start, true, false)?))
                    }
                    "class" => {
                        self.bump()?;
                        Ok(Expr::Class(self.class(false)?))
                    }
                    "this" => {
                        self.bump()?;
                        Ok(Expr::This)
                    }
                    "super" => {
                        self.bump()?;
                        Ok(Expr::Super)
                    }
                    "null" => {
                        self.bump()?;
                        Ok(Expr::Null)
                    }
                    "true" | "false" => {
                        self.bump()?;
                        Ok(Expr::Bool(&*w == "true"))
                    }
                    _ => {
                        let n = self.ident()?;
                        Ok(Expr::Ident(n, t.start))
                    }
                }
            }
            Tok::EscapedIdent(_) => {
                let n = self.ident()?;
                Ok(Expr::Ident(n, t.start))
            }
            other => self.err(format!("unexpected {other:?}")),
        }
    }

    fn object(&mut self) -> Result<Expr> {
        self.expect("{")?;
        let mut props = Vec::new();
        while !self.eat("}")? {
            if self.eat("...")? {
                props.push(Prop {
                    kind: PropKind::Spread,
                    key: PropKey::Name("".into()),
                    value: self.assignment(true)?,
                    shorthand: false,
                });
            } else {
                let start = self.tok.start;
                let shorthand_name = match &self.tok.tok {
                    Tok::Ident(n) | Tok::EscapedIdent(n) => Some((
                        n.clone(),
                        self.is_ident() || matches!(&**n, "yield" | "await" | "let"),
                    )),
                    _ => None,
                };
                let (kind, key, value) = self.method_or_key(start, false)?;
                let prop = if let Some(value) = value {
                    Prop {
                        kind,
                        key,
                        value,
                        shorthand: false,
                    }
                } else if self.eat(":")? {
                    Prop {
                        kind,
                        key,
                        value: self.assignment(true)?,
                        shorthand: false,
                    }
                } else {
                    let Some((name, ok)) = shorthand_name else {
                        return self.err("expected `:`");
                    };
                    if !ok {
                        return self.err(format!("unexpected `{name}`"));
                    }
                    let mut value = Expr::Ident(name.clone(), start);
                    // `{a = 1}` only as a destructuring pattern.
                    if self.eat("=")? {
                        let d = self.assignment(true)?;
                        value = Expr::Assign("=", Box::new(Pattern::Ident(name)), Box::new(d));
                        Prop {
                            kind: PropKind::Init,
                            key,
                            value,
                            shorthand: true,
                        }
                    } else {
                        Prop {
                            kind: PropKind::Init,
                            key,
                            value,
                            shorthand: true,
                        }
                    }
                };
                props.push(prop);
            }
            if !self.is("}") {
                self.expect(",")?;
            }
        }
        Ok(Expr::Object(props))
    }
}

fn lexer_src<'a>(l: &Lexer<'a>) -> &'a str {
    l.source()
}

/// Whether a `/` after this token starts a regular expression.
fn regex_allowed_after(t: &Tok) -> bool {
    match t {
        Tok::Num(_) | Tok::BigInt(_) | Tok::Str(_) | Tok::Regex(..) | Tok::Private(_) => false,
        Tok::Template(_, _, done) => !done,
        Tok::Punct(p) => !matches!(*p, ")" | "]" | "}" | "++" | "--"),
        Tok::Ident(w) => {
            RESERVED.contains(&&**w)
                && !matches!(&**w, "this" | "super" | "null" | "true" | "false")
        }
        Tok::EscapedIdent(_) => false,
        Tok::Eof => true,
    }
}

fn simple_pattern(e: Expr) -> Pattern {
    match e {
        Expr::Ident(n, _) => Pattern::Ident(n),
        e => Pattern::Expr(Box::new(e)),
    }
}

/// An expression read before its `=` or `=>` showed it was a target.
pub fn to_pattern(e: Expr, binding: bool) -> Option<Pattern> {
    Some(match e {
        Expr::Ident(n, _) => Pattern::Ident(n),
        e @ (Expr::Member(_, _, false) | Expr::Index(_, _, false) | Expr::PrivateMember(..))
            if !binding =>
        {
            Pattern::Expr(Box::new(e))
        }
        Expr::Assign("=", target, value) => Pattern::Default(target, value),
        Expr::Array(items) => {
            let mut out = Vec::new();
            let mut rest = None;
            let n = items.len();
            for (i, item) in items.into_iter().enumerate() {
                match item {
                    None => out.push(None),
                    Some(Expr::Spread(inner)) if i + 1 == n => {
                        rest = Some(Box::new(to_pattern(*inner, binding)?));
                    }
                    Some(e) => out.push(Some(to_pattern(e, binding)?)),
                }
            }
            Pattern::Array(out, rest)
        }
        Expr::Object(props) => {
            let mut out = Vec::new();
            let mut rest = None;
            let n = props.len();
            for (i, p) in props.into_iter().enumerate() {
                match p.kind {
                    PropKind::Init => out.push(PropPattern {
                        key: p.key,
                        value: to_pattern(p.value, binding)?,
                    }),
                    PropKind::Spread if i + 1 == n => {
                        rest = Some(Box::new(to_pattern(p.value, binding)?));
                    }
                    _ => return None,
                }
            }
            Pattern::Object(out, rest)
        }
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ok(src: &str) -> Program {
        parse(src).unwrap_or_else(|e| panic!("{src}: {} at {}", e.message, e.pos))
    }

    #[test]
    fn statements_and_semicolons() {
        let p = ok("var a = 1\nlet b = 2; const c = a + b\nif (a) b = 3; else { c }\nfor (var i = 0; i < 3; i++) {}\nwhile (0) ;\ndo a++; while (a < 3)\nswitch (a) { case 1: break; default: }\ntry { throw 1 } catch (e) {} finally {}\nl: for (const x of [1]) continue l;\nfor (k in {}) ;");
        assert_eq!(p.body.len(), 11);
    }

    #[test]
    fn functions_arrows_and_classes() {
        ok("function f(a, b = 2, ...c) { return a }\nconst g = (x, {y, z: [w] = []}) => x + y;\nconst h = async x => await x;\nconst k = () => ({});\nclass A extends B { #p = 1; static s = 2; constructor() { super(); this.#p++; } get v() { return this.#p } set v(x) {} static { this.t = 3 } *gen() { yield 1 } async m() {} }");
    }

    #[test]
    fn expressions() {
        ok("a?.b?.[c]?.(d); x ?? y; a ** -b === undefined; `t${a + `n${b}`}`; tag`x`; /re[/]/g.test(s); a = b = c; [a, b] = [b, a]; ({a, b} = o); new A.B(); new A; typeof x === 'y'; void 0; delete o.p; x => y => x * y; !function(){}(); a ? b : c ? d : e; #x in o === false || 1");
    }

    #[test]
    fn regex_after_operators_and_keywords() {
        ok("x = a / b / c; y = /a/.test(z); return_ = 1; if (/x/.test(s)) {} q = [/a/, /b/];");
    }

    #[test]
    fn errors() {
        for bad in [
            "a +",
            "var 1",
            "if (",
            "a = => 1",
            "({a:1} = 2)",
            "1 = 2",
            "return 1",
            "a ?? b || c",
        ] {
            assert!(parse(bad).is_err(), "{bad}");
        }
    }

    #[test]
    fn every_bundled_script_parses() {
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../res/controllers");
        let mut failures = Vec::new();
        let mut n = 0;
        for e in std::fs::read_dir(&dir).unwrap().flatten() {
            let p = e.path();
            if p.extension().is_some_and(|x| x == "js") {
                n += 1;
                let src = std::fs::read_to_string(&p).unwrap();
                if let Err(e) = parse(&src) {
                    let (l, c) = Lexer::line_col(&src, e.pos);
                    failures.push(format!(
                        "{}:{l}:{c}: {}",
                        p.file_name().unwrap().to_string_lossy(),
                        e.message
                    ));
                }
            }
        }
        assert!(n > 100, "{n}");
        assert!(
            failures.is_empty(),
            "{} of {n} failed:\n{}",
            failures.len(),
            failures.join("\n")
        );
    }
}

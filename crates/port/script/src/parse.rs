//! Reading a line into an [`Expr`], and running it.

use crate::verbs::{verb, ArgKind, Kind, TakesDeck, Verb, VERBS};
use crate::{Call, Error, Host};

/// A deck named with `deck`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Deck {
    /// 1 to 4.
    Number(u8),
    Master,
}

/// An argument after its verb, checked and with defaults filled in.
#[derive(Clone, Debug, PartialEq)]
pub enum Arg {
    /// Levels given as `80%` are stored as 0.8.
    Number(f64),
    Text(String),
}

impl Arg {
    pub fn number(&self) -> f64 {
        match self {
            Arg::Number(n) => *n,
            Arg::Text(_) => 0.0,
        }
    }

    pub fn text(&self) -> &str {
        match self {
            Arg::Text(t) => t,
            Arg::Number(_) => "",
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum Expr {
    Action {
        verb: &'static Verb,
        args: Vec<Arg>,
    },
    /// `a & b & c`
    Chain(Vec<Expr>),
    /// `condition ? then : otherwise`
    If {
        condition: Box<Expr>,
        then: Box<Expr>,
        otherwise: Box<Expr>,
    },
    /// `deck 2 …`
    OnDeck(Deck, Box<Expr>),
    /// `vu_meter > 90%`: only asks, how far up a verb is against a level.
    Compare {
        action: Box<Expr>,
        above: bool,
        level: f64,
    },
}

/// A parsed line, ready to run as often as needed.
#[derive(Clone, Debug, PartialEq)]
pub struct Script(pub Expr);

impl Script {
    pub fn run(&self, host: &mut dyn Host) -> Result<(), Error> {
        run(&self.0, None, host)
    }

    /// Whether the line is one verb that only shows a level, such as
    /// `deck 1 filter` or `vu_meter`: a controller light then shows that
    /// level (an encoder's LED ring, a meter) instead of on or off.
    pub fn is_level(&self) -> bool {
        let mut e = &self.0;
        while let Expr::OnDeck(_, inner) = e {
            e = inner;
        }
        matches!(e, Expr::Action { verb, .. } if matches!(
            verb.kind,
            Kind::Level(_) | Kind::Value(_) | Kind::Reading(_) | Kind::Var | Kind::Get | Kind::Adjust
        ))
    }

    /// The level a line from [`Script::is_level`] shows, 0 to 1.
    pub fn level(&self, host: &mut dyn Host) -> Result<f64, Error> {
        let mut e = &self.0;
        let mut deck = None;
        while let Expr::OnDeck(d, inner) = e {
            deck = Some(*d);
            e = inner;
        }
        let Expr::Action { verb, args } = e else {
            return Err(Error::new("Only one verb shows a level"));
        };
        let call = Call { verb, deck: resolve(verb, deck, host), args, input: host.input(), steps: host.steps() };
        host.level(&call)
    }
}

pub fn parse(src: &str) -> Result<Script, Error> {
    let tokens = lex(src)?;
    let mut p = Parser { tokens, pos: 0, end: src.len() };
    if p.tokens.is_empty() {
        return Err(Error::at("Nothing to do", 0));
    }
    let e = p.chain()?;
    match p.peek() {
        None => Ok(Script(e)),
        Some((t, at)) => Err(Error::at(format!("Unexpected {}", t.describe()), at)),
    }
}

#[derive(Clone, Debug, PartialEq)]
enum Token {
    Word(String),
    Number(f64),
    Percent(f64),
    Text(String),
    And,
    Question,
    Colon,
    Open,
    Close,
    Greater,
    Less,
}

impl Token {
    fn describe(&self) -> String {
        match self {
            Token::Word(w) => format!("'{w}'"),
            Token::Number(n) => format!("{n}"),
            Token::Percent(n) => format!("{n}%"),
            Token::Text(t) => format!("'{t}'"),
            Token::And => "'&'".into(),
            Token::Question => "'?'".into(),
            Token::Colon => "':'".into(),
            Token::Open => "'('".into(),
            Token::Close => "')'".into(),
            Token::Greater => "'>'".into(),
            Token::Less => "'<'".into(),
        }
    }
}

fn lex(src: &str) -> Result<Vec<(Token, usize)>, Error> {
    let mut out = Vec::new();
    let mut chars = src.char_indices().peekable();
    while let Some(&(at, c)) = chars.peek() {
        if c.is_whitespace() {
            chars.next();
            continue;
        }
        let single = match c {
            '&' => Some(Token::And),
            '?' => Some(Token::Question),
            ':' => Some(Token::Colon),
            '(' => Some(Token::Open),
            ')' => Some(Token::Close),
            '>' => Some(Token::Greater),
            '<' => Some(Token::Less),
            _ => None,
        };
        if let Some(t) = single {
            chars.next();
            out.push((t, at));
        } else if c == '\'' || c == '"' {
            chars.next();
            let mut text = String::new();
            loop {
                match chars.next() {
                    Some((_, q)) if q == c => break,
                    Some((_, ch)) => text.push(ch),
                    None => return Err(Error::at("This quote is never closed", at)),
                }
            }
            out.push((Token::Text(text), at));
        } else if c.is_ascii_digit() || c == '.' || c == '-' {
            let mut s = String::new();
            while let Some(&(_, ch)) = chars.peek() {
                if ch.is_ascii_digit() || ch == '.' || (ch == '-' && s.is_empty()) {
                    s.push(ch);
                    chars.next();
                } else {
                    break;
                }
            }
            let n: f64 = s.parse().map_err(|_| Error::at(format!("'{s}' isn't a number"), at))?;
            if chars.peek().map(|&(_, ch)| ch) == Some('%') {
                chars.next();
                out.push((Token::Percent(n), at));
            } else {
                out.push((Token::Number(n), at));
            }
        } else if c.is_alphabetic() || c == '_' {
            let mut s = String::new();
            while let Some(&(_, ch)) = chars.peek() {
                if ch.is_alphanumeric() || ch == '_' {
                    s.push(ch.to_ascii_lowercase());
                    chars.next();
                } else {
                    break;
                }
            }
            out.push((Token::Word(s), at));
        } else {
            return Err(Error::at(format!("'{c}' isn't part of DJSugarScript"), at));
        }
    }
    Ok(out)
}

struct Parser {
    tokens: Vec<(Token, usize)>,
    pos: usize,
    /// The line's length, for mistakes at its end.
    end: usize,
}

impl Parser {
    fn peek(&self) -> Option<(Token, usize)> {
        self.tokens.get(self.pos).cloned()
    }

    fn next(&mut self) -> Option<(Token, usize)> {
        let t = self.peek();
        self.pos += 1;
        t
    }

    fn here(&self) -> usize {
        self.tokens.get(self.pos).map_or(self.end, |t| t.1)
    }

    fn expect(&mut self, want: Token, what: &str) -> Result<(), Error> {
        match self.next() {
            Some((t, _)) if t == want => Ok(()),
            Some((t, at)) => Err(Error::at(format!("Expected {what}, found {}", t.describe()), at)),
            None => Err(Error::at(format!("Expected {what}"), self.end)),
        }
    }

    /// item ('&' item)*
    fn chain(&mut self) -> Result<Expr, Error> {
        let mut items = vec![self.item()?];
        while matches!(self.peek(), Some((Token::And, _))) {
            self.next();
            items.push(self.item()?);
        }
        Ok(if items.len() == 1 { items.pop().unwrap() } else { Expr::Chain(items) })
    }

    /// 'deck' deck item | ('(' chain ')' | action) ['?' chain ':' chain]
    fn item(&mut self) -> Result<Expr, Error> {
        let start = self.here();
        let first = match self.next() {
            Some((Token::Word(w), _)) if w == "deck" => {
                let deck = self.deck()?;
                return Ok(Expr::OnDeck(deck, Box::new(self.item()?)));
            }
            Some((Token::Open, _)) => {
                let e = self.chain()?;
                self.expect(Token::Close, "')'")?;
                e
            }
            Some((Token::Word(w), at)) => {
                let action = self.action(&w, at)?;
                self.compare(action)?
            }
            Some((t, at)) => return Err(Error::at(format!("Expected a verb, found {}", t.describe()), at)),
            None => return Err(Error::at("Expected a verb", start)),
        };
        if !matches!(self.peek(), Some((Token::Question, _))) {
            return Ok(first);
        }
        self.next();
        let then = self.chain()?;
        self.expect(Token::Colon, "':' and what to do otherwise")?;
        let otherwise = self.chain()?;
        Ok(Expr::If { condition: Box::new(first), then: Box::new(then), otherwise: Box::new(otherwise) })
    }

    /// An action, or the action compared with a level: `vu_meter > 90%`.
    fn compare(&mut self, action: Expr) -> Result<Expr, Error> {
        let above = match self.peek() {
            Some((Token::Greater, _)) => true,
            Some((Token::Less, _)) => false,
            _ => return Ok(action),
        };
        self.next();
        let level = match self.next() {
            Some((Token::Number(n), _)) => n,
            Some((Token::Percent(p), _)) => p / 100.0,
            Some((t, at)) => {
                return Err(Error::at(format!("Expected a level to compare with, found {}", t.describe()), at))
            }
            None => return Err(Error::at("Expected a level to compare with", self.end)),
        };
        Ok(Expr::Compare { action: Box::new(action), above, level })
    }

    fn deck(&mut self) -> Result<Deck, Error> {
        match self.next() {
            Some((Token::Number(n), _)) if (1.0..=4.0).contains(&n) && n.fract() == 0.0 => Ok(Deck::Number(n as u8)),
            Some((Token::Word(w), _)) if w == "master" => Ok(Deck::Master),
            Some((t, at)) => {
                Err(Error::at(format!("Expected 1 to 4 or 'master' after 'deck', found {}", t.describe()), at))
            }
            None => Err(Error::at("Expected 1 to 4 or 'master' after 'deck'", self.end)),
        }
    }

    fn action(&mut self, name: &str, at: usize) -> Result<Expr, Error> {
        let Some(verb) = verb(name) else {
            let close = VERBS
                .iter()
                .find(|v| v.name.replace('_', "") == name.replace('_', ""))
                .map(|v| format!("; did you mean '{}'?", v.name))
                .unwrap_or_default();
            return Err(Error::at(format!("No verb '{name}'{close}"), at));
        };
        let mut args = Vec::new();
        for spec in verb.args {
            let given = match self.peek() {
                Some((t @ (Token::Number(_) | Token::Percent(_) | Token::Text(_)), at)) => Some((t, at)),
                // A bare word names a control, but only where one is wanted.
                Some((t @ Token::Word(_), at)) if spec.kind == ArgKind::Control => Some((t, at)),
                _ => None,
            };
            let Some((token, arg_at)) = given else {
                match spec.default {
                    Some(d) => {
                        args.push(Arg::Number(d));
                        continue;
                    }
                    None => {
                        return Err(Error::at(format!("'{}' needs {}", verb.name, article(spec.name)), self.here()))
                    }
                }
            };
            self.next();
            args.push(check_arg(verb, spec.name, spec.kind, token, arg_at)?);
        }
        if let Some((t @ (Token::Number(_) | Token::Percent(_) | Token::Text(_)), at)) = self.peek() {
            return Err(Error::at(format!("'{}' takes no more than that; {} is extra", verb.name, t.describe()), at));
        }
        Ok(Expr::Action { verb, args })
    }
}

fn article(name: &str) -> String {
    match name.chars().next() {
        Some('a' | 'e' | 'i' | 'o' | 'u') => format!("an {name}"),
        _ => format!("a {name}"),
    }
}

fn check_arg(verb: &Verb, name: &str, kind: ArgKind, token: Token, at: usize) -> Result<Arg, Error> {
    let wrong = || Error::at(format!("'{}' needs {} here, not {}", verb.name, article(name), token.describe()), at);
    match (kind, &token) {
        (ArgKind::Number { min, max }, Token::Number(n)) => {
            if *n < min || *n > max {
                Err(Error::at(format!("The {name} for '{}' goes from {min} to {max}", verb.name), at))
            } else {
                Ok(Arg::Number(*n))
            }
        }
        (ArgKind::Level, Token::Number(n)) if (0.0..=1.0).contains(n) => Ok(Arg::Number(*n)),
        (ArgKind::Level, Token::Percent(p)) if (0.0..=100.0).contains(p) => Ok(Arg::Number(p / 100.0)),
        (ArgKind::Level, Token::Number(_) | Token::Percent(_)) => {
            Err(Error::at("A level goes from 0 to 1, or 0% to 100%", at))
        }
        (ArgKind::Control, Token::Text(t) | Token::Word(t)) => Ok(Arg::Text(t.clone())),
        (ArgKind::Text, Token::Text(t)) => Ok(Arg::Text(t.clone())),
        _ => Err(wrong()),
    }
}

fn resolve(verb: &Verb, deck: Option<Deck>, host: &dyn Host) -> Option<u8> {
    let number = |d: Deck| match d {
        Deck::Number(n) => n,
        Deck::Master => host.master_deck(),
    };
    match verb.deck {
        TakesDeck::No => None,
        TakesDeck::Yes => Some(deck.map_or(host.master_deck(), number)),
        TakesDeck::Optional => deck.map(number),
    }
}

fn run(e: &Expr, deck: Option<Deck>, host: &mut dyn Host) -> Result<(), Error> {
    match e {
        Expr::Action { verb, args } => {
            let call = Call { verb, deck: resolve(verb, deck, host), args, input: host.input(), steps: host.steps() };
            host.run(&call)
        }
        Expr::Chain(items) => {
            let mut first_error = None;
            for item in items {
                if let Err(err) = run(item, deck, host) {
                    first_error.get_or_insert(err);
                }
            }
            first_error.map_or(Ok(()), Err)
        }
        Expr::If { condition, then, otherwise } => {
            if query(condition, deck, host)? {
                run(then, deck, host)
            } else {
                run(otherwise, deck, host)
            }
        }
        Expr::OnDeck(d, inner) => run(inner, Some(*d), host),
        Expr::Compare { .. } => {
            Err(Error::new("A comparison only asks, in a condition such as vu_meter > 90% ? … : …"))
        }
    }
}

/// A condition: one verb asked, or every verb of a chain in parentheses.
fn query(e: &Expr, deck: Option<Deck>, host: &mut dyn Host) -> Result<bool, Error> {
    match e {
        Expr::Action { verb, args } => {
            let call = Call { verb, deck: resolve(verb, deck, host), args, input: host.input(), steps: host.steps() };
            host.query(&call)
        }
        Expr::Chain(items) => {
            for item in items {
                if !query(item, deck, host)? {
                    return Ok(false);
                }
            }
            Ok(true)
        }
        Expr::If { .. } => Err(Error::new("A condition can't hold another '?'")),
        Expr::OnDeck(d, inner) => query(inner, Some(*d), host),
        Expr::Compare { action, above, level } => {
            let mut action = action.as_ref();
            let mut deck = deck;
            while let Expr::OnDeck(d, inner) = action {
                deck = Some(*d);
                action = inner;
            }
            let Expr::Action { verb, args } = action else {
                return Err(Error::new("Only one verb can be compared with a level"));
            };
            let call = Call { verb, deck: resolve(verb, deck, host), args, input: host.input(), steps: host.steps() };
            let now = host.level(&call)?;
            Ok(if *above { now > *level } else { now < *level })
        }
    }
}

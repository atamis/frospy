use std::fmt::Display;

use parser::Expr;
use rpds::HashTrieMap;

pub mod compiler;
pub mod compiler2;
pub mod cps;
pub mod header;
pub mod parser;
pub mod util;

// #[derive(Debug, PartialEq, Clone, Eq, Hash)]
// enum Token {
//     Integer(i64),
//     Atom(Option<AtomMod>, String),
//     LeftParen,
//     RightParen,
// }

// fn tokenizer<'a>() -> impl Parser<char, Vec<Token>, Error = Simple<char>> {
//     let atom = choice((
//         just('\'').to(AtomMod::Quote),
//         just('$').to(AtomMod::QuotePop),
//         just('^').to(AtomMod::QuotePush),
//     ))
//     .or_not()
//     .then(text::ident())
//     .map(|(m, s)| -> Token { Token::Atom(m, s) });

//     choice((
//         primitive::just('(').to(Token::LeftParen),
//         primitive::just(')').to(Token::RightParen),
//         ,
//         atom,
//     ))
//     .padded()
//     .repeated()
//     .then_ignore(end())
// }

// #[cfg(test)]
// mod test_tokenizer {
//     use super::*;

//     #[test]
//     fn test_tokenizer() {
//         let s = "(fn) 'a $fn force 40".to_string();
//         let tokens = tokenizer().parse(s);

//         dbg!(tokens);

//         assert!(false);
//     }
// }

// #[derive(Debug)]
// struct AST {
//     elements: Vec<ASTElement>,
// }

type Env = HashTrieMap<String, Value>;

#[derive(Debug, Clone)]
enum Value {
    Integer(i64),
    Atom(String),
    Thunk { env: Env, exprs: Vec<Expr> },
}
impl Value {
    fn from_quoted_expr(e: &Expr) -> Self {
        match e {
            Expr::Integer(i, _) => Value::Integer(*i),
            Expr::Atom(a, _) => Value::Atom(a.to_string()),
            Expr::Thunk(_, _) => panic!("Can't get quote of tunk"),
        }
    }

    fn get_name(&self) -> Option<&str> {
        match self {
            Value::Atom(s) => Some(s),
            _ => None,
        }
    }
}

impl Display for Value {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Value::Integer(i) => f.write_fmt(format_args!("{}", i)),
            Value::Atom(s) => f.write_str(s),
            Value::Thunk { env, exprs } => {
                f.write_str("( ")?;
                for e in exprs.iter() {
                    f.write_fmt(format_args!("{} ", e))?;
                }
                f.write_str(")")?;
                f.write_fmt(format_args!("<{}>", env))
            }
        }
    }
}

#[derive(Debug)]
struct Frame {
    idx: usize,
    exprs: Vec<Expr>,
    env: Option<Env>,
}

impl Frame {
    fn new_with_env(exprs: Vec<Expr>, env: Env) -> Self {
        Self {
            env: Some(env),
            exprs,
            idx: 0,
        }
    }

    fn new(exprs: Vec<Expr>) -> Self {
        Self {
            env: None,
            exprs,
            idx: 0,
        }
    }

    fn get_expr(&mut self) -> Option<&Expr> {
        let e = self.exprs.get(self.idx)?;
        self.idx += 1;
        Some(e)
    }

    fn is_done(&self) -> bool {
        self.exprs.get(self.idx).is_none()
    }
}

#[derive(Debug)]
pub struct Ctx {
    env: Env,
    stack: Vec<Value>,
    frames: Vec<Frame>,
}

impl Ctx {
    pub fn new(exprs: Vec<Expr>) -> Self {
        Ctx {
            env: Env::new(),
            stack: Vec::new(),
            frames: vec![Frame::new(exprs)],
        }
    }

    // fn current_frame(&mut self) -> Option<&mut Frame> {
    //     self.frames.last_mut()
    // }

    fn get_env_mut(&mut self) -> &mut Env {
        if let Some(e) = self.frames.last_mut().and_then(|f| f.env.as_mut()) {
            e
        } else {
            &mut self.env
        }
    }

    pub fn pump(&mut self) -> bool {
        // True if done, false otherwise
        let cf = if let Some(f) = self.frames.last_mut() {
            f
        } else {
            return true;
        };

        if cf.is_done() {
            self.frames.pop();
            return false;
        }

        // Safe because if the frame is done, we've already returned
        let e = cf.get_expr().unwrap();

        match e.clone() {
            // get_env_mut needs access to the ctx, but e borrows.
            Expr::Integer(i, _) => self.stack.push(Value::Integer(i)),
            Expr::Atom(a, _) => match a.as_str() {
                "quote" => {
                    let ev = cf.get_expr().expect("Can't quote missing expr");
                    self.stack.push(Value::from_quoted_expr(ev));
                }
                "pop" => {
                    let name = self
                        .stack
                        .pop()
                        .expect("can't pop name")
                        .get_name()
                        .expect("can't convert to name")
                        .to_string();
                    let value = self.stack.pop().expect("can't pop value");

                    let env = self.get_env_mut();

                    *env = env.insert(name, value);
                }
                "push" => {
                    let name = self
                        .stack
                        .pop()
                        .expect("can't pop name")
                        .get_name()
                        .expect("can't convert to name")
                        .to_string();
                    let value = self
                        .get_env_mut()
                        .get(&name)
                        .unwrap_or_else(|| panic!("name {} unbound in env", name))
                        .clone();

                    self.stack.push(value);
                }
                "force" => {
                    let v = self.stack.pop().expect("can't pop for force");
                    // TODO built-ins
                    if let Value::Thunk { exprs, env } = v {
                        self.frames
                            .push(Frame::new_with_env(exprs.clone(), env.clone()))
                    } else {
                        panic!("Can't apply non-thunk")
                    }
                }
                a => {
                    let v = self
                        .get_env_mut()
                        .get(a)
                        .expect("couldn't find atom in env")
                        .clone();
                    // TODO built-ins
                    if let Value::Thunk { exprs, env } = v {
                        self.frames
                            .push(Frame::new_with_env(exprs.clone(), env.clone()))
                    } else {
                        panic!("Can't apply non-thunk")
                    }
                }
            },
            Expr::Thunk(e, _) => {
                let t = Value::Thunk {
                    env: self.env.clone(),
                    exprs: e.to_vec(),
                };

                self.stack.push(t);
            }
        }

        false
    }
}

pub fn trace_ctx(c: &Ctx) {
    print!("{}\t\t", c.env);

    print!("[");
    for v in c.stack.iter() {
        print!("{} ", v)
    }
    print!("]");

    print!("\t\t");

    print!(
        "{} - {:?}",
        c.frames.len(),
        c.frames.last().map(|f| &f.exprs[f.idx..])
    );

    println!();
}

pub mod eval;

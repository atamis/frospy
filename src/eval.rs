use std::fmt::Display;

use rpds::HashTrieMap;
use thiserror::Error;

use crate::parser::{Expr, Span};

type Env = HashTrieMap<String, Value>;

type BuiltInFn = fn(&mut Env, &mut Vec<Value>) -> Result<(), EvalStacktrace>;

#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    Integer(i64),
    Atom(String),
    Thunk { env: Env, exprs: Vec<Expr> },
    BuiltIn(&'static str, Box<BuiltInFn>),
}

impl Value {
    fn get_name(&self) -> Option<&str> {
        match self {
            Value::Atom(s) => Some(s),
            _ => None,
        }
    }

    fn from_quoted_expr(e: &Expr) -> Self {
        match e {
            Expr::Integer(i, _) => Value::Integer(*i),
            Expr::Atom(a, _) => Value::Atom(a.to_string()),
            Expr::Thunk(_, _) => panic!("Can't get quote of thunk"),
        }
    }

    fn get_integer(&self) -> Result<i64, EvalError> {
        match self {
            Value::Integer(i) => Ok(*i),
            Value::Atom(_) => Err(EvalError::TypeMismatch(
                "integer".to_string(),
                "atom".to_string(),
            )),
            Value::Thunk { .. } => Err(EvalError::TypeMismatch(
                "thunk".to_string(),
                "atom".to_string(),
            )),
            Value::BuiltIn(_, _) => Err(EvalError::TypeMismatch(
                "builtin".to_string(),
                "atom".to_string(),
            )),
        }
    }
}

impl Display for Value {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Value::Integer(i) => f.write_fmt(format_args!("{}", i)),
            Value::Atom(s) => f.write_str(s),
            Value::Thunk { exprs, .. } => {
                f.write_str("( ")?;
                for e in exprs.iter() {
                    f.write_fmt(format_args!("{} ", e))?;
                }
                f.write_str(")")
                // f.write_fmt(format_args!("<{}>", env))
            }
            Value::BuiltIn(n, _) => f.write_fmt(format_args!("*{}", n)),
        }
    }
}

trait ResultSpanCtx<E> {
    fn to_stacktrace(self) -> E;
    fn with_span(self, s: Span) -> E;
}

#[derive(Debug, Error, PartialEq)]
pub struct EvalStacktrace {
    pub stack: Vec<Span>,

    #[source]
    pub error: EvalError,
}

impl Display for EvalStacktrace {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_fmt(format_args!("Error: {}", self.error))?;
        f.write_str("Stacktrace:")?;
        for s in self.stack.iter() {
            f.write_fmt(format_args!("\t{:?}", s))?;
        }
        Ok(())
    }
}

impl<T> ResultSpanCtx<Result<T, EvalStacktrace>> for Result<T, EvalError> {
    fn with_span(self, s: Span) -> Result<T, EvalStacktrace> {
        self.map_err(|e| {
            let mut es: EvalStacktrace = e.into();
            es.stack.push(s);
            es
        })
    }

    fn to_stacktrace(self) -> Result<T, EvalStacktrace> {
        self.map_err(|e| e.into())
    }
}

impl<T> ResultSpanCtx<Result<T, EvalStacktrace>> for Result<T, EvalStacktrace> {
    fn with_span(self, s: Span) -> Result<T, EvalStacktrace> {
        self.map_err(|mut e| {
            e.stack.push(s);
            e
        })
    }

    fn to_stacktrace(self) -> Result<T, EvalStacktrace> {
        self
    }
}

impl From<EvalError> for EvalStacktrace {
    fn from(value: EvalError) -> Self {
        Self {
            stack: vec![],
            error: value,
        }
    }
}

#[derive(Debug, Error, PartialEq)]
pub enum EvalError {
    #[error("Unbound name {0} in env")]
    Unbound(String),

    #[error("Can't apply type {0} as a function")]
    InvalidApply(String),

    #[error("Attempted to pop from empty stack")]
    PopEmpty,

    #[error("Type mismatch, expected {0}, got {1}")]
    TypeMismatch(String, String),

    #[error("Attempt to quote missing expr")]
    BareQuote,
}

struct EvalCtx<'a, 'b> {
    exprs: &'a [Expr],
    env: Env,
    stack: &'b mut Vec<Value>,
    tracing: bool,
}

fn apply_value(v: Value, env: &mut Env, stack: &mut Vec<Value>) -> Result<(), EvalStacktrace> {
    //println!("applying {}", v);
    match v {
        Value::Integer(_) => Err(EvalError::InvalidApply("integer".to_string())).to_stacktrace(),
        Value::Atom(_) => Err(EvalError::InvalidApply("atom".to_string())).to_stacktrace(),
        Value::Thunk { env, exprs } => {
            let nec = EvalCtx {
                env: env.clone(),
                exprs: &exprs,
                stack,
                tracing: false,
            };
            nec.eval()
        }
        Value::BuiltIn(_, f) => f(env, stack).to_stacktrace(),
    }
}

impl EvalCtx<'_, '_> {
    fn eval(self) -> Result<(), EvalStacktrace> {
        let EvalCtx {
            exprs,
            mut env,
            stack,
            tracing,
        } = self;

        let mut exs = exprs;

        loop {
            if exs.is_empty() {
                break;
            }

            let e;

            (e, exs) = exs.split_first().unwrap();

            if tracing {
                print!("TRACE {:?}\t", e);
                for v in stack.iter() {
                    print!("{} ", v);
                }
                println!();
            }

            match e {
                Expr::Integer(i, _) => stack.push(Value::Integer(*i)),
                Expr::Atom(a, span) => match a.as_str() {
                    "quote" => {
                        let qe;
                        (qe, exs) = exs
                            .split_first()
                            .ok_or(EvalError::BareQuote)
                            .with_span(span.clone())?;

                        stack.push(Value::from_quoted_expr(qe));
                    }
                    a => {
                        let v = env
                            .get(a)
                            .ok_or_else(|| EvalError::Unbound(a.to_string()))
                            .with_span(span.clone())?
                            .clone();

                        apply_value(v, &mut env, stack).with_span(span.clone())?;
                    }
                },
                Expr::Thunk(e, _) => {
                    let t = Value::Thunk {
                        env: env.clone(),
                        exprs: e.to_vec(),
                    };

                    stack.push(t);
                }
            }
        }
        println!("RETURN");

        Ok(())
    }
}

mod builtin {
    use super::*;

    pub fn inc(_e: &mut Env, s: &mut Vec<Value>) -> Result<(), EvalStacktrace> {
        let i = s.pop().ok_or(EvalError::PopEmpty)?.get_integer()?;

        s.push(Value::Integer(i + 1));

        Ok(())
    }

    pub fn pop(env: &mut Env, stack: &mut Vec<Value>) -> Result<(), EvalStacktrace> {
        let name = stack
            .pop()
            .ok_or(EvalError::PopEmpty)?
            .get_name()
            .unwrap() // TODO
            .to_string();

        let value = stack.pop().ok_or(EvalError::PopEmpty)?;

        println!("POP {name} = {value}");

        env.insert_mut(name, value);

        Ok(())
    }

    pub fn push(env: &mut Env, stack: &mut Vec<Value>) -> Result<(), EvalStacktrace> {
        let name = stack
            .pop()
            .ok_or(EvalError::PopEmpty)?
            .get_name()
            .unwrap() // TODO
            .to_string();

        let value = env
            .get(&name)
            .ok_or_else(|| EvalError::Unbound(name.to_string()))?;

        println!("PUSH {name}, {value}");

        stack.push(value.clone());

        Ok(())
    }

    pub fn force(env: &mut Env, stack: &mut Vec<Value>) -> Result<(), EvalStacktrace> {
        let value = stack.pop().ok_or(EvalError::PopEmpty)?;

        println!("FORCE {}", value);

        apply_value(value, env, stack)
    }

    pub fn cswap(_env: &mut Env, stack: &mut Vec<Value>) -> Result<(), EvalStacktrace> {
        let value = stack.pop().ok_or(EvalError::PopEmpty)?;

        if value == Value::Atom("t".to_string()) {
            let i_last = stack.len() - 1;
            let i_scnd = stack.len() - 2;
            stack.swap(i_last, i_scnd);
        }

        Ok(())
    }

    pub fn println(_env: &mut Env, stack: &mut Vec<Value>) -> Result<(), EvalStacktrace> {
        let value = stack.pop().ok_or(EvalError::PopEmpty)?;

        println!("{value}");

        Ok(())
    }

    // pub fn eq(env: &mut Env, stack: &mut Vec<Value>) -> Result<(), EvalStacktrace> {
    //     let value = stack.pop().ok_or(EvalError::PopEmpty)?;

    //     println!("{}", value);

    //     apply_value(value, env, stack)
    // }

    #[cfg(test)]
    mod builtin_tests {
        use super::*;

        // #[test]
        // fn test_inc() {
        //     let mut s = vec![Value::Integer(0)];
        //     let mut e = Env::new();
        //     assert_eq!(inc(&mut e, &mut s), Ok(()));
        //     assert_eq!(s, vec![Value::Integer(1)]);

        //     let mut s = vec![];
        //     let mut e = Env::new();
        //     assert_eq!(inc(&mut e, &mut s), Err(EvalError::));
        //     assert_eq!(s, vec![Value::Integer(1)]);
        // }

        #[test]
        fn test_cswap() {
            use Value::*;
            let mut stack = vec![Integer(2), Integer(1), Atom("t".to_string())];
            cswap(&mut Env::new(), &mut stack).unwrap();

            assert_eq!(stack, vec![Integer(1), Integer(2)]);

            let mut stack = vec![Integer(2), Integer(1), Atom("f".to_string())];
            cswap(&mut Env::new(), &mut stack).unwrap();

            assert_eq!(stack, vec![Integer(2), Integer(1)]);
        }
    }
}

fn env_with_builtins() -> Env {
    let mut env = Env::new();

    let mut insert = |s: &'static str, f: BuiltInFn| {
        env.insert_mut(s.to_string(), Value::BuiltIn(s, Box::new(f)))
    };

    insert("inc", builtin::inc);
    insert("pop", builtin::pop);
    insert("push", builtin::push);
    insert("force", builtin::force);
    insert("cswap", builtin::cswap);
    insert("println", builtin::println);

    env
}

pub fn eval(exprs: &[Expr]) -> Result<Vec<Value>, EvalStacktrace> {
    let mut stack = Vec::new();
    let ec = EvalCtx {
        exprs,
        env: env_with_builtins(),
        stack: &mut stack,
        tracing: true,
    };

    ec.eval()?;

    Ok(stack)
}

#[cfg(test)]
mod eval_test {
    use crate::parser::parser;
    use chumsky::Parser;

    use super::*;

    #[test]
    fn test_binding() {
        let e = parser()
            .parse(r"1 $test (^test) $f 2 $test ^f force ^test")
            .unwrap();

        assert_eq!(
            eval(&e).unwrap(),
            vec![Value::Integer(1), Value::Integer(2)]
        );
    }

    #[test]
    fn test_if() {
        let e = parser()
            .parse(
                r"(
  ($x $y ^x) $true
  ($x $y ^y) $false
  ($p $x $y ^y ^x p) $if
  2 1 ^true if
) force",
            )
            .unwrap();

        assert_eq!(eval(&e).unwrap(), vec![Value::Integer(1)]);
    }
}

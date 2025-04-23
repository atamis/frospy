use std::fmt::Display;
use std::ops::Range;

use chumsky::prelude::*;
use chumsky::Parser;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AtomMod {
    Quote,
    QuotePop,
    QuotePush,
}

pub type Span = Range<usize>;

pub fn span_combine(s1: &Span, s2: &Span) -> Span {
    s1.start..s2.end
}

pub fn dummy_span() -> Span {
    0..0
}

pub fn span_start_span(s: &Span) -> Span {
    s.start..s.start
}
pub fn span_end_span(s: &Span) -> Span {
    s.end..s.end
}

#[derive(Debug, PartialEq, Clone)]
pub enum Expr {
    Integer(i64, Span),
    Atom(String, Span),
    Thunk(Vec<Self>, Span),
}

impl Display for Expr {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Expr::Integer(i, _) => f.write_fmt(format_args!("{}", i)),
            Expr::Atom(a, _) => f.write_fmt(format_args!("{}", a)),
            Expr::Thunk(es, _) => {
                f.write_str("( ")?;
                for e in es.iter() {
                    f.write_fmt(format_args!("{} ", e))?;
                }
                f.write_str(")")
            }
        }
    }
}

impl Expr {
    pub fn get_span(&self) -> &Span {
        match self {
            Expr::Integer(_, s) => s,
            Expr::Atom(_, s) => s,
            Expr::Thunk(_, s) => s,
        }
    }
}

fn atom_parser() -> impl Parser<char, Vec<Expr>, Error = Simple<char>> {
    choice((
        just('\'').to(AtomMod::Quote),
        just('$').to(AtomMod::QuotePop),
        just('^').to(AtomMod::QuotePush),
    ))
    .or_not()
    .then(text::ident())
    .labelled("atom")
    .map_with_span(|(m, s), span: Span| -> Vec<Expr> {
        match m {
            Some(AtomMod::Quote) => vec!["quote".to_string(), s],
            Some(AtomMod::QuotePop) => vec!["quote".to_string(), s, "pop".to_string()],
            Some(AtomMod::QuotePush) => vec!["quote".to_string(), s, "push".to_string()],
            None => vec![s],
        }
        .into_iter()
        .map(|a| Expr::Atom(a, span.clone()))
        .collect::<Vec<Expr>>()
    })
    .padded()
}

pub fn parser() -> impl Parser<char, Vec<Expr>, Error = Simple<char>> {
    let integer = text::int(10)
        .from_str()
        .unwrapped()
        .labelled("integer")
        .map_with_span(|i, span| vec![Expr::Integer(i, span)])
        .padded();

    let mut expr = Recursive::declare();

    let thunk = expr
        .clone()
        .repeated()
        .flatten()
        // This padding is semantically necessary, but screws up the span
        .delimited_by(just('(').padded(), just(')').padded())
        .map_with_span(|elements, span| vec![Expr::Thunk(elements, span)])
        .labelled("thunk");

    expr.define(choice((atom_parser(), integer, thunk)).labelled("expr"));

    expr.repeated().flatten().then_ignore(end())
}

#[cfg(test)]
mod test_parser {
    use super::*;

    #[test]
    fn test_atom_parser() {
        assert_eq!(
            atom_parser().parse("test"),
            Ok(vec![Expr::Atom("test".to_string(), 0..4)])
        );
        assert_eq!(
            atom_parser().parse("   test   "),
            Ok(vec![Expr::Atom("test".to_string(), 3..7)])
        );

        assert_eq!(
            atom_parser().parse("   t123   "),
            Ok(vec![Expr::Atom("t123".to_string(), 3..7)])
        );

        assert_eq!(
            atom_parser().parse("   'test   "),
            Ok(vec![
                Expr::Atom("quote".to_string(), 3..8),
                Expr::Atom("test".to_string(), 3..8),
            ])
        );
        assert_eq!(
            atom_parser().parse("   $test   "),
            Ok(vec![
                Expr::Atom("quote".to_string(), 3..8),
                Expr::Atom("test".to_string(), 3..8),
                Expr::Atom("pop".to_string(), 3..8),
            ])
        );
        assert_eq!(
            atom_parser().parse("   ^test   "),
            Ok(vec![
                Expr::Atom("quote".to_string(), 3..8),
                Expr::Atom("test".to_string(), 3..8),
                Expr::Atom("push".to_string(), 3..8),
            ])
        );
    }

    #[test]
    fn test_parser() {
        assert_eq!(parser().parse(""), Ok(vec![]));
        assert_eq!(
            parser().parse("$test"),
            Ok(vec![
                Expr::Atom("quote".to_string(), 0..5),
                Expr::Atom("test".to_string(), 0..5),
                Expr::Atom("pop".to_string(), 0..5),
            ])
        );
        assert_eq!(
            parser().parse("()\n"),
            Ok(vec![Expr::Thunk(vec![], 0..3),]) // FIXME
        );
        assert_eq!(
            parser().parse("( )\n"),
            Ok(vec![Expr::Thunk(vec![], 0..4),]) // FIXME
        );
        assert_eq!(
            parser().parse(" ( ) \n"),
            Ok(vec![Expr::Thunk(vec![], 0..6),]) // FIXME
        );
        assert_eq!(
            parser().parse("(test asdf)\n"),
            Ok(vec![Expr::Thunk(
                vec![
                    Expr::Atom("test".to_string(), 1..5),
                    Expr::Atom("asdf".to_string(), 6..10),
                ],
                0..12
            ),])
        );
    }
}

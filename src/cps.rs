use std::fmt::Display;

use crate::{
    parser::{self, Expr, Span},
    util,
};

#[derive(Debug, Clone, PartialEq)]
pub enum ExprCPS {
    IntegerLiteral(i64, Span),
    AtomLiteral(String, Span),
    Thunk(Vec<ExprCPS>, Span),
    Force(Span),
    ForceCC(Span),
    ForceCCBare(Span),
    Terminate,
    Pop(Span),
    Push(Span),
}

fn exprs_to_exprs_cps(exprs: &[Expr]) -> Vec<ExprCPS> {
    let mut v2 = vec![];

    let mut exs = exprs;
    loop {
        if exs.is_empty() {
            break;
        }

        let e;

        (e, exs) = exs.split_first().unwrap();

        match e {
            Expr::Integer(i, s) => v2.push(ExprCPS::IntegerLiteral(*i, s.clone())),
            Expr::Atom(a, atom_span) => match a.as_str() {
                "quote" => {
                    let qe;
                    (qe, exs) = exs.split_first().unwrap();

                    match qe {
                        Expr::Integer(i, s) => v2.push(ExprCPS::IntegerLiteral(
                            *i,
                            parser::span_combine(atom_span, s),
                        )),
                        Expr::Atom(a, s) => v2.push(ExprCPS::AtomLiteral(
                            a.to_string(),
                            parser::span_combine(atom_span, s),
                        )),
                        Expr::Thunk(_, _) => panic!("Can't quote a thunk"),
                    }
                }
                "push" => v2.push(ExprCPS::Push(atom_span.clone())),
                "pop" => v2.push(ExprCPS::Pop(atom_span.clone())),
                "force" => v2.push(ExprCPS::Force(atom_span.clone())),
                a => {
                    v2.push(ExprCPS::AtomLiteral(a.to_string(), atom_span.clone()));
                    v2.push(ExprCPS::Push(atom_span.clone()));
                    v2.push(ExprCPS::Force(atom_span.clone()));
                }
            },
            Expr::Thunk(vec, s) => {
                v2.push(ExprCPS::Thunk(exprs_to_exprs_cps(vec), s.clone()));
            }
        }
    }

    v2
}

impl Display for ExprCPS {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ExprCPS::IntegerLiteral(i, _) => f.write_fmt(format_args!("{}", i)),
            ExprCPS::AtomLiteral(a, _) => f.write_fmt(format_args!("'{}", a)),
            ExprCPS::Thunk(es, _) => {
                f.write_str("( ")?;
                for e in es.iter() {
                    f.write_fmt(format_args!("{} ", e))?;
                }
                f.write_str(")")
            }
            ExprCPS::Force(_) => f.write_fmt(format_args!("force")),
            ExprCPS::ForceCC(_) => f.write_fmt(format_args!("forceCC")),
            ExprCPS::ForceCCBare(_) => f.write_fmt(format_args!("forceCCbare")),
            ExprCPS::Terminate => f.write_fmt(format_args!("terminate")),
            ExprCPS::Pop(_) => f.write_fmt(format_args!("pop")),
            ExprCPS::Push(_) => f.write_fmt(format_args!("push")),
        }
    }
}

fn cps_thunk(exprs: &[ExprCPS], span: &Span) -> ExprCPS {
    let cc = util::random_name();
    let cc_atom = ExprCPS::AtomLiteral(cc, parser::dummy_span());
    let cont = vec![cc_atom.clone(), ExprCPS::Push(parser::dummy_span())];

    let mut v = vec![cc_atom.clone(), ExprCPS::Pop(parser::dummy_span())];

    v.extend(cps_internal(exprs, &cont));

    ExprCPS::Thunk(v, span.clone())
}

fn cps_internal(exprs: &[ExprCPS], cont: &[ExprCPS]) -> Vec<ExprCPS> {
    let mut ne = vec![];

    let mut exs = exprs;

    let mut is_bare = true;

    loop {
        if exs.is_empty() {
            break;
        }

        let e;

        (e, exs) = exs.split_first().unwrap();

        let mut forcing = false;

        ne.extend(match e {
            ExprCPS::Thunk(te, s) => vec![cps_thunk(te, s)],
            ExprCPS::Force(s) => {
                let mut v = vec![];
                if exs.is_empty() {
                    // Note that this is very important. Otherwise, you
                    // a new thunk (cont forcebare), capturing the
                    // current environment. When in a Y-combinator
                    // "loop", this repeatedly adds a new thunk around
                    // the continuation, leading to a memory leak.
                    v.extend(cont.iter().cloned());
                } else {
                    v.push(ExprCPS::Thunk(cps_internal(exs, cont), s.clone()));
                }
                v.push(ExprCPS::ForceCC(s.clone()));
                is_bare = false;
                forcing = true;
                v
            }
            ExprCPS::ForceCC(_) => todo!(),
            ExprCPS::ForceCCBare(_) => todo!(),
            x => vec![x.clone()],
        });

        if forcing {
            break;
        }
    }

    if is_bare {
        ne.extend(cont.iter().cloned());
        ne.push(ExprCPS::ForceCCBare(parser::dummy_span()));
    }

    ne
}

pub fn expr_cps(exprs: &[Expr]) -> Vec<ExprCPS> {
    let exprs = exprs_to_exprs_cps(exprs);

    cps_internal(
        &exprs,
        &[ExprCPS::Thunk(
            vec![ExprCPS::Terminate],
            parser::dummy_span(),
        )],
    )
}

// Notes from prior attempts:
//
// TODO "CPS" so each call is the last thing in the thunk

// ( <whatever> ) => ($cc <whatever> $cc force)
// 2 1 ($x $y 'x push force ) force terminate   => 2 1 ($cc $x $y 'x push 'cc push force) ($cc terminate) force
// (2) (1) ($x $y 'x push force ) force terminate   => ($cc 2 ^cc force) ($cc 1 ^cc force) (terminate) ($cc $x $y ^cc 'x force)  force
// (1 inc) =>
//
// 1 'inc force 'inc force terminate => 1
// force = pop thunk,
//
// <stuff1> force <stuff2> => <stuff1> (<stuff2>) force-by-cc
// <program> => ($cc program 'cc force-by-cc-bare) (terminate) force-by-cc
// <stuff> => ($cc <stuff> 'cc force-by-cc-bare)
// <stuff> force => $cc <stuff> 'cc force-by-cc

// (1 inc) => ($cc 1 'inc force 'cc force)
// (2) (1) ($x $y 'x push force) force => ($cc 2 'cc force) ($cc 1 'cc force) (terminate) ($cc $x $y ^x force 'cc force) force
//
// (2) (1) ($x $y 'x push force) force
// (terminate) ($cc (2) (1) ($x $y 'x push force) force 'cc force) force // wrap with terminate
// (terminate) ($cc ($cc 2 'cc force) ($cc 1 'cc force) ($cc $x $y 'x push force 'cc force) force 'cc force) force // Make thunks CC
// (terminate) ($cc ($cc 2 'cc force) ($cc 1 'cc force) ($cc $x $y 'x push force 'cc force) force 'cc force) force

// (2) (1) ($x $y 'x push force) force force
// (terminate) ($cc (2) (1) ($x $y 'x push force) force force 'cc force) force // wrap
// (terminate) ($cc ($cc 2 'cc force) ($cc 1 'cc force) ($cc $x $y 'x push force 'cc force) force force 'cc force) force // Make thunks CC
// (terminate) ($cc ($cc 2 'cc force) ($cc 1 'cc force) ($cc $x $y 'x push force 'cc force) force 'cc ('cc force $cc force) force) force // Code after force converted to thunk, passed CC and forced

// n1 n2 s1 n3 s2 => n1 n2 s1 (n3 s2) force

// Append terminate. WRONG
// Process "nothing" exprs
// On "something" expr, process it, then put the rest in a thunk and call it.

// (2) (1) ($x $y 'x push force) force force
// (2) (1) ($x $y 'x push force) force force terminate
// (2) (1) ($x $y 'x push force) force force terminate
// (2) (1) ($x $y 'x push force) force (force terminate) force
// (2) (1) ($x $y 'x push force) force (force (terminate) force) force
// (2) (1) ($x $y 'x push force) force (force (terminate) force) force

// ($s $n ^n 'println force ^n 'inc force ^s ^s force) $l 0 ^l ^l force
// ($s $n ^n 'println force ^n 'inc force ^s ^s force) $l 0 ^l ^l force terminate
// ($s $n ^n 'println force ^n 'inc force ^s ^s force) $l 0 ^l ^l force (terminate) force
// ($s $n ^n 'println force (^n 'inc force (^s ^s force) force) force) $l 0 ^l ^l force (terminate) force

// No force can return
// ForceCC => swap force
//
// (2) (1) ($x $y ^x force) force force
// (2) (1) ($x $y 'x push force) force force terminate
// ($cc 2 'cc force) ($cc 1 'cc force) ($cc $x $y 'x push 'cc force-cc) force force terminate
// ($cc ($cc ($cc 2 'cc force) ($cc 1 'cc force) ($cc $x $y 'x push 'cc force-cc) 'cc force-by-cc) 'cc force-cc) (terminate) force-cc
//
//
// program => program terminate
// stuff... Ft1 Ft2 => (stuff Ft1) (Ft2) forcecc -WRONG
// stuff... Ft1 stuff2... => stuff... (stuff2) Ft1-cc
// (stuff) => ($cc stuff ^cc forcecc) if stuff ends in force
// (stuff) => ($cc stuff ^cc force) if stuff doesn't end in force
// (terminate) => (terminate)
//

// (2) (1) ($x $y ^x force) force force
// ($cc (2) (1) ($x $y ^x force) ^cc forcecc) (force) forcecc

// schunk is any number of non-force exprs
// fchunk is any number of exprs and then a force
// u(fchunk) is fchunk without the force, so [u(fchunk) force] = fchunk
//
// (schunk) => ($cc schunk cc)
// (fchunk) => ($cc u(fchunk) ^cc forcecc)
// chunk1 chunk2 => ((chunk1) (chunk2) forcecc) TODO
// program = (program) (terminate) forcecc
// (chunk1 chunk2) => ($cc )
//
// TODO CPS conversion needs to assume its contents will end up in a thunk.
// The entire program needs to be come a thunk that CCs to terminate.
// That way cps(exprs) can always be wrapped in a thunk, so cps(exprs) includes the CC stuff.
//
// TODO I think CPS conversion is unsound somehow.
//
// And then I just did it correctly

#[derive(Debug, Clone)]
enum EExpr {
    Integer(i64, parser::Span),
    Atom(String, parser::Span),
    ThunkRef(String, parser::Span),
}

type Program = HashMap<String, Vec<EExpr>>;

fn exprs_to_program(exprs: &[Expr]) -> Program {
    fn internal(prog: &mut Program, name: String, exprs: &[Expr]) {
        let v = exprs
            .iter()
            .map(|e| match e {
                Expr::Integer(i, span) => EExpr::Integer(*i, span.clone()),
                Expr::Atom(a, span) => EExpr::Atom(a.to_string(), span.clone()),
                Expr::Thunk(vec, span) => {
                    let name = compiler::random_name();
                    internal(prog, name.to_string(), vec);
                    EExpr::ThunkRef(name.to_string(), span.clone())
                }
            })
            .collect();

        prog.insert(name, v);
    }

    let mut prog = HashMap::new();

    internal(&mut prog, "entry".to_string(), exprs);

    prog
}

fn make_thunk_ref_enum(prog: &Program) -> String {
    let mut code = String::new();
    code.push_str("#[derive(Clone, Debug)] enum ThunkRef {");
    code.push_str(&prog.keys().join(","));
    code.push_str("}");

    code
}

fn compile_eexprs(eexprs: &[EExpr]) -> String {
    let mut code = String::new();

    let mut exs = eexprs;
    loop {
        if exs.is_empty() {
            break;
        }

        let e;

        (e, exs) = exs.split_first().unwrap();

        match e {
            EExpr::Integer(i, _) => code.push_str(&format!("stack.push(Value::Integer({i}));")),
            EExpr::Atom(a, _) => {
                match a.as_str() {
                    "quote" => {
                        let qe;
                        (qe, exs) = exs.split_first().unwrap();
                        //.ok_or_else(|| EvalError::BareQuote)
                        // .with_span(span.clone())?;
                        match qe {
                            EExpr::Integer(i, _) => {
                                code.push_str(&format!("stack.push(Value::Integer({}));", i))
                            }
                            EExpr::Atom(a, _) => code.push_str(&format!(
                                "stack.push(Value::Atom(\"{}\".to_string()));",
                                a
                            )),
                            EExpr::ThunkRef(_, _) => panic!("Can't quote a thunk"),
                        }
                    }

                    a => {
                        code.push_str(&format!(
                            "{{
let t = env.get(\"{}\").expect(\"Unbound var {}\").clone();
match t {{
            Value::Integer(_) => panic!(\"Can't call integer\"),
            Value::Atom(s) => panic!(\"Can't call atom\"),
            Value::Thunk {{ env, fp }} => {{
let nf = Frame {{ env: cur_frame.env.clone(), tr: fp }};
cs.push(nf);

}},
            Value::BuiltIn(f) => f(env, stack),
}};
}}",
                            a, a,
                        ));
                    }
                }
            }
            EExpr::ThunkRef(tf, _) => code.push_str(&format!(
                "stack.push(Value::Thunk {{ env: env.clone(), fp: ThunkRef::{tf} }});"
            )),
        }
    }

    code.push_str("cs.pop().expect(\"Popped empty stack\");");

    code
}

fn compile_toplevel(prog: &HashMap<String, Vec<EExpr>>) -> String {
    let mut code = String::new();
    code.push_str("fn top_level(env: &mut Env, stack: &mut Stack) {");

    code.push_str("struct Frame { tr: ThunkRef, env: Env }");

    code.push_str("let mut cs: Vec<Frame> = vec![];");

    code.push_str("cs.push( Frame{tr: ThunkRef::entry, env: env.clone()});");

    code.push_str("loop { if cs.last().is_none() {break} ");
    code.push_str("let cur_frame = cs.last_mut().unwrap();");

    code.push_str("match cur_frame.tr {");

    for (name, eexprs) in prog.iter() {
        code.push_str(&format!("ThunkRef::{name} => {{"));
        code.push_str("/*");
        code.push_str(&format!("{:?}", eexprs));
        code.push_str("*/");
        code.push_str(&compile_eexprs(eexprs));
        code.push_str("},");
    }

    code.push_str("}"); // Match
    code.push_str("}"); // loop

    code.push_str("}");

    code
}

pub fn compile2(exprs: &[Expr]) -> String {
    let prog = exprs_to_program(exprs);

    let mut code = String::new();
    code.push_str(HEADER);

    code.push_str(&make_thunk_ref_enum(&prog));

    code.push_str(&compile_toplevel(&prog));

    code.push_str(&main_function());

    code
}

fn expr3_cps4(exprs: &[Expr3]) -> Vec<Expr3> {
    fn chunk_type(es: &[Expr3]) -> ChunkType {
        use ChunkType::*;
        if es.iter().any(|e| e.is_forcelike()) {
            FChunk
        } else {
            SChunk
        }
    }

    fn finish_thunk_exprs(v: &mut Vec<Expr3>) {
        let t = chunk_type(&v);

        if t == ChunkType::FChunk {
            v.pop().expect("Removing force");
        }

        v.push(Expr3::AtomLiteral("-cc".to_string()));
        v.push(Expr3::Push);

        v.push(match t {
            ChunkType::FChunk => Expr3::ForceCC,
            ChunkType::SChunk => Expr3::ForceCCBare,
        });
    }

    fn next_chunk(es: &[Expr3]) -> Option<(Vec<Expr3>, &[Expr3])> {
        if es.is_empty() {
            return None;
        }

        let mut c = vec![];

        for e in es.iter().take_while_inclusive(|e| !e.is_forcelike()) {
            c.push(e.clone())
        }

        let c_len = c.len();

        Some((c, &es[c_len..]))
    }

    fn thunk_chunk(es: &[Expr3], cont: &Expr3) -> Expr3 {
        // TODO
        Expr3::Thunk(handle_exprs(es, cont))
    }

    fn handle_2chunk(c1: &[Expr3], c2: &[Expr3], cont: &Expr3) -> Vec<Expr3> {
        let hc2 = thunk_chunk(c2, cont);
        let hc1 = thunk_chunk(c1, &hc2);

        vec![hc1]
    }

    // TODO does this need to return a thunk?
    fn handle_chunks(exprs: &[Expr3], cont: &Expr3) -> Vec<Expr3> {
        if exprs.is_empty() {
            return exprs.to_vec();
        }

        let mut exs: &[Expr3] = &exprs;

        let mut cur_chunk;

        (cur_chunk, exs) = next_chunk(exs).expect("Exprs isn't empty, but no chunk");

        loop {
            let maybe_chunk = next_chunk(exs);

            if maybe_chunk.is_none() {
                break;
            }

            let n_chunk;

            (n_chunk, exs) = maybe_chunk.unwrap();

            let c_chunk = handle_2chunk(&cur_chunk, &n_chunk, cont);

            cur_chunk = c_chunk;
        }

        dbg!(cur_chunk)
    }

    fn is_thunk_already_cps(es: &[Expr3]) -> bool {
        let atom_literal = Expr3::AtomLiteral("-cc".to_string());
        (es.len() >= 2 && es[0] == atom_literal && es[1] == Expr3::Pop)
            && ((es.len() >= 5)
                && es[es.len() - 3] == atom_literal
                && es[es.len() - 2] == Expr3::Push
                && ((es[es.len() - 1] == Expr3::ForceCC)
                    || (es[es.len() - 1] == Expr3::ForceCCBare)))
    }

    fn handle_exprs(exprs: &[Expr3], cont: &Expr3) -> Vec<Expr3> {
        let mut ne = vec![];

        // ne.push(Expr3::AtomLiteral("-cc".to_string()));
        // ne.push(Expr3::Pop);

        // Recurse on thunks

        // let p_e: Vec<_> = exprs
        //     .iter()
        //     .map(|e| {
        //         if let Expr3::Thunk(te) = e {
        //             // This is a real hack
        //             eprintln!("Maybe processing thunk {te:?}");
        //             if !is_thunk_already_cps(te) {
        //                 Expr3::Thunk(handle_exprs(te))
        //             } else {
        //                 eprintln!("Skipping already CPS thunk {te:?}");
        //                 e.clone()
        //             }
        //         } else {
        //             e.clone()
        //         }
        //     })
        //     .collect();

        // CPS on chunks
        let chunks = handle_chunks(&exprs, cont);
        ne.extend(chunks.into_iter());

        // Add CC push code
        // finish_thunk_exprs(&mut ne);

        ne
    }

    handle_exprs(exprs, &Expr3::Thunk(vec![Expr3::Terminate]))
}

fn handle_chunks(
    exprs: &[Expr3],
    handle_2_chunks: impl Fn(&[Expr3], &[Expr3]) -> Vec<Expr3>,
) -> Vec<Expr3> {
    if exprs.is_empty() {
        return exprs.to_vec();
    }

    let mut exs: &[Expr3] = &exprs;

    let mut cur_chunk;

    (cur_chunk, exs) = next_chunk(exs).expect("Exprs isn't empty, but no chunk");

    loop {
        let maybe_chunk = next_chunk(exs);

        if maybe_chunk.is_none() {
            break;
        }

        let n_chunk;

        (n_chunk, exs) = maybe_chunk.unwrap();

        let c_chunk = handle_2_chunks(&cur_chunk, &n_chunk);

        cur_chunk = c_chunk;
    }

    dbg!(cur_chunk)
}

fn expr3_cps3(exprs: &[Expr3]) -> Vec<Expr3> {
    fn finish_thunk_exprs(v: &mut Vec<Expr3>) {
        let t = chunk_type(&v);

        if t == ChunkType::FChunk {
            v.pop().expect("Removing force");
        }

        v.push(Expr3::AtomLiteral("-cc".to_string()));
        v.push(Expr3::Push);

        v.push(match t {
            ChunkType::FChunk => Expr3::ForceCC,
            ChunkType::SChunk => Expr3::ForceCCBare,
        });
    }

    fn thunk_chunk(es: &[Expr3]) -> Expr3 {
        Expr3::Thunk(handle_exprs(es))
    }

    fn handle_2chunk(c1: &[Expr3], c2: &[Expr3]) -> Vec<Expr3> {
        let hc1 = thunk_chunk(c1);
        let hc2 = thunk_chunk(c2);

        vec![
            Expr3::Thunk(handle_exprs(&vec![hc1, hc2])),
            Expr3::Force,
            Expr3::ForceCC,
        ]
    }

    // TODO does this need to return a thunk?
    fn handle_chunks(exprs: &[Expr3]) -> Vec<Expr3> {
        if exprs.is_empty() {
            return exprs.to_vec();
        }

        let mut exs: &[Expr3] = &exprs;

        let mut cur_chunk;

        (cur_chunk, exs) = next_chunk(exs).expect("Exprs isn't empty, but no chunk");

        loop {
            let maybe_chunk = next_chunk(exs);

            if maybe_chunk.is_none() {
                break;
            }

            let n_chunk;

            (n_chunk, exs) = maybe_chunk.unwrap();

            let c_chunk = handle_2chunk(&cur_chunk, &n_chunk);

            cur_chunk = c_chunk;
        }

        dbg!(cur_chunk)
    }

    fn is_thunk_already_cps(es: &[Expr3]) -> bool {
        let atom_literal = Expr3::AtomLiteral("-cc".to_string());
        (es.len() >= 2 && es[0] == atom_literal && es[1] == Expr3::Pop)
            && ((es.len() >= 5)
                && es[es.len() - 3] == atom_literal
                && es[es.len() - 2] == Expr3::Push
                && ((es[es.len() - 1] == Expr3::ForceCC)
                    || (es[es.len() - 1] == Expr3::ForceCCBare)))
    }

    fn handle_exprs(exprs: &[Expr3]) -> Vec<Expr3> {
        let mut ne = vec![];

        ne.push(Expr3::AtomLiteral("-cc".to_string()));
        ne.push(Expr3::Pop);

        // Recurse on thunks

        let p_e: Vec<_> = exprs
            .iter()
            .map(|e| {
                if let Expr3::Thunk(te) = e {
                    // This is a real hack
                    eprintln!("Maybe processing thunk {te:?}");
                    if !is_thunk_already_cps(te) {
                        Expr3::Thunk(handle_exprs(te))
                    } else {
                        eprintln!("Skipping already CPS thunk {te:?}");
                        e.clone()
                    }
                } else {
                    e.clone()
                }
            })
            .collect();

        // CPS on chunks
        let chunks = handle_chunks(&p_e);
        ne.extend(chunks.into_iter());

        // Add CC push code
        finish_thunk_exprs(&mut ne);

        ne
    }

    fn program(exprs: &[Expr3]) -> Vec<Expr3> {
        vec![
            Expr3::Thunk(vec![Expr3::Terminate]),
            Expr3::Thunk(handle_exprs(exprs)),
            Expr3::Force,
        ]
    }

    program(exprs)
}

fn expr3_cps2(exprs: &[Expr3]) -> Vec<Expr3> {
    enum ChunkType {
        FChunk,
        SChunk,
    }

    fn chunk_type(es: &[Expr3]) -> ChunkType {
        use ChunkType::*;
        if es.iter().any(|e| e.is_forcelike()) {
            FChunk
        } else {
            SChunk
        }
    }

    fn next_chunk(es: &[Expr3]) -> Option<(Vec<Expr3>, &[Expr3])> {
        if es.is_empty() {
            return None;
        }

        let mut c = vec![];

        for e in es.iter().take_while_inclusive(|e| !e.is_forcelike()) {
            c.push(e.clone())
        }

        let c_len = c.len();

        Some((c, &es[c_len..]))
    }

    fn thunk_schunk(es: &[Expr3]) -> Expr3 {
        let mut v = vec![];

        v.push(Expr3::AtomLiteral("-cc".to_string()));
        v.push(Expr3::Pop);

        v.extend(es.iter().cloned());

        // for e in es {
        //     if let Expr3::Thunk(te) = e {
        //         v.push(thunk_chunk(&expr3_cps2(&te)))
        //     } else {
        //         v.push(e.clone())
        //     }
        // }

        v.push(Expr3::AtomLiteral("-cc".to_string()));
        v.push(Expr3::Push);

        v.push(Expr3::ForceCCBare);

        Expr3::Thunk(v)
    }

    fn thunk_fchunk(es: &[Expr3]) -> Expr3 {
        let u_f = fchunk_clean_force(es);

        let mut v = vec![];

        v.push(Expr3::AtomLiteral("-cc".to_string()));
        v.push(Expr3::Pop);

        v.extend(u_f);
        // for e in u_f {
        //     if let Expr3::Thunk(te) = e {
        //         v.push(thunk_chunk(&expr3_cps2(&te)))
        //     } else {
        //         v.push(e)
        //     }
        // }

        v.push(Expr3::AtomLiteral("-cc".to_string()));
        v.push(Expr3::Push);

        v.push(Expr3::ForceCC);

        Expr3::Thunk(v)
    }

    fn thunk_chunk(es: &[Expr3]) -> Expr3 {
        match chunk_type(es) {
            ChunkType::FChunk => thunk_fchunk(es),
            ChunkType::SChunk => thunk_schunk(es),
        }
    }

    fn fchunk_clean_force(es: &[Expr3]) -> Vec<Expr3> {
        es.iter()
            .take_while(|e| !e.is_forcelike())
            .cloned()
            .collect()
    }

    fn handle_2chunk(c1: &[Expr3], c2: &[Expr3]) -> Vec<Expr3> {
        if c1.is_empty() {
            return c2.to_vec();
        }

        let hc1 = thunk_chunk(c1);
        let hc2 = thunk_chunk(c2);

        vec![hc1, hc2, Expr3::ForceCC]
    }

    let mut cur_chunk = vec![];
    let post_exprs: Vec<_> = exprs
        .iter()
        .map(|e| {
            if let Expr3::Thunk(te) = e {
                Expr3::Thunk(expr3_cps2(te))
            } else {
                e.clone()
            }
        })
        .collect();

    let mut exs: &[Expr3] = &post_exprs;

    loop {
        let maybe_chunk = next_chunk(exs);

        if maybe_chunk.is_none() {
            break;
        }

        let n_chunk;

        (n_chunk, exs) = maybe_chunk.unwrap();

        let c_chunk = handle_2chunk(&cur_chunk, &n_chunk);

        cur_chunk = c_chunk;
    }

    dbg!(&cur_chunk);

    cur_chunk
}

#[derive(PartialEq)]
enum ChunkType {
    FChunk,
    SChunk,
}

fn chunk_type(es: &[Expr3]) -> ChunkType {
    use ChunkType::*;
    if es.iter().any(|e| e.is_forcelike()) {
        FChunk
    } else {
        SChunk
    }
}

fn next_chunk(es: &[Expr3]) -> Option<(Vec<Expr3>, &[Expr3])> {
    if es.is_empty() {
        return None;
    }

    let mut c = vec![];

    for e in es.iter().take_while_inclusive(|e| !e.is_forcelike()) {
        c.push(e.clone())
    }

    let c_len = c.len();

    Some((c, &es[c_len..]))
}

fn expr3_cps(exprs: &[Expr3]) -> Vec<Expr3> {
    // (stuff forcecc) => ($cc stuff ^cc forcecc) if stuff ends in force
    // (stuff) => ($cc stuff ^cc force) if stuff doesn't end in force
    // fn convert_thunk(exprs: &[Expr3]) -> Vec<Expr3> {
    //     print_exprs("THUNK", exprs);

    //     if exprs.len() == 1 && exprs[0] == Expr3::Terminate {
    //         return exprs.to_vec();
    //     }

    //     let mut ne = vec![];

    //     ne.push(Expr3::AtomLiteral("-cc".to_string()));
    //     ne.push(Expr3::Pop);

    //     // if exprs.last().map(|e| e.is_forcelike()).unwrap_or(false) {
    //     //     todo!()
    //     // }

    //     ne.extend(convert_exprs(&exprs).into_iter());

    //     match *ne.last().unwrap() {
    //         Expr3::Force => {
    //             ne.pop(); // Remove force
    //             ne.push(Expr3::AtomLiteral("-cc".to_string()));
    //             ne.push(Expr3::Push);
    //             ne.push(Expr3::ForceCC);
    //         }
    //         Expr3::ForceCC => (),
    //         Expr3::ForceCCBare => (),
    //         _ => {
    //             ne.push(Expr3::AtomLiteral("-cc".to_string()));
    //             ne.push(Expr3::Push);
    //             ne.push(Expr3::ForceCCBare);
    //         }
    //     }

    //     ne
    // }

    // stuff... Ft1 Ft2 => stuff (Ft2) Ft1-cc
    fn convert_exprs(exprs: &[Expr3]) -> Vec<Expr3> {
        print_exprs("EXPRS", exprs);
        let mut ne = vec![];

        let mut exs = exprs;
        loop {
            if exs.is_empty() {
                break;
            }

            let e;

            (e, exs) = exs.split_first().unwrap();

            if !e.is_forcelike() {
                if let Expr3::Thunk(te) = e {
                    ne.push(Expr3::Thunk(convert_thunk(te)));
                } else {
                    ne.push(e.clone());
                }
            } else {
                print_exprs("EXPRS EXS", exs);
                if exs.is_empty() {
                    ne.push(e.clone());
                } else {
                    // TODO becomse self thunk?
                    let cc = Expr3::Thunk(convert_thunk(exs));
                    ne.push(cc);
                    ne.push(Expr3::ForceCC);
                }
                break;
            }
        }

        ne
    }

    fn convert_thunk(exprs: &[Expr3]) -> Vec<Expr3> {
        print_exprs("THUNK", exprs);

        let mut te = vec![];

        te.push(Expr3::AtomLiteral("-cc".to_string()));
        te.push(Expr3::Pop);

        let mut exs = exprs;
        loop {
            if exs.is_empty() {
                break;
            }

            let e;

            (e, exs) = exs.split_first().unwrap();

            if !e.is_forcelike() {
                if let Expr3::Thunk(ete) = e {
                    te.push(Expr3::Thunk(convert_thunk(ete)));
                } else {
                    te.push(e.clone());
                }
            } else {
                // Don't copy forcelike

                te.push(Expr3::AtomLiteral("-cc".to_string()));
                te.push(Expr3::Push);
                te.push(Expr3::ForceCC);
                let me = Expr3::Thunk(te);
                let mut nte = vec![me.clone()];

                let mut v = vec![me.clone()];
                //v.extend(rest.into_iter());
                return v;
            }
        }

        te.push(Expr3::AtomLiteral("-cc".to_string()));
        te.push(Expr3::Push);
        te.push(Expr3::ForceCCBare);

        te
    }

    vec![Expr3::Thunk(convert_thunk(exprs))]
}

fn print_exprs(tag: &str, exprs: &[Expr3]) {
    print!("{tag} [");
    for e in exprs.iter() {
        print!("{e} ");
    }
    println!("]");
}

// fn exprs2_to_exprs3(exprs: &[Expr2]) -> Vec<Expr3> {
//     fn convert_thunk(exprs: &[Expr2]) -> Vec<Expr3> {
//         let mut vt = vec![];

//         vt.push(Expr3::AtomLiteral("__cc".to_string())); // Store the CC
//         vt.push(Expr3::Pop);

//         let mut exs = exprs;

//         loop {
//             if exs.is_empty() {
//                 vt.push(Expr3::AtomLiteral("__cc".to_string()));
//                 vt.push(Expr3::Push);
//                 vt.push(Expr3::ForceByCCBare);
//                 break;
//             }

//             let e;

//             (e, exs) = exs.split_first().unwrap();
//             match e {
//                 Expr2::IntegerLiteral(i) => vt.push(Expr3::IntegerLiteral(*i)),
//                 Expr2::AtomLiteral(a) => vt.push(Expr3::AtomLiteral(a.to_string())),
//                 Expr2::Thunk(vec) => vt.push(Expr3::Thunk(convert_thunk(vec))),
//                 Expr2::Force => {
//                     let cc = convert_thunk(exs);
//                     vt.push(Expr3::Thunk(cc));
//                     vt.push(Expr3::ForceByCC);
//                     break;
//                 }
//             }
//         }

//         vt
//     }

//     vec![
//         Expr3::Thunk(convert_thunk(exprs)),
//         Expr3::Thunk(vec![Expr3::Terminate]),
//         Expr3::ForceByCC,
//     ]
// }

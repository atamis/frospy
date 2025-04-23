use crate::{parser::Expr, util};

pub const HEADER: &str = include_str!("../wasm/header.rs");

fn compile_exprs_to_f(exprs: &[Expr], name: String) -> String {
    let mut code = String::new();
    code.push_str(&format!("fn {}(mut env: Env, stack: &mut Stack) {{", name));

    let mut exs = exprs;
    loop {
        if exs.is_empty() {
            break;
        }

        let e;

        (e, exs) = exs.split_first().unwrap();

        match e {
            Expr::Integer(i, _) => code.push_str(&format!("stack.push(Value::Integer({}));", i)),
            Expr::Atom(a, _) => match a.as_str() {
                "quote" => {
                    let qe;
                    (qe, exs) = exs.split_first().unwrap();
                    //.ok_or_else(|| EvalError::BareQuote)
                    // .with_span(span.clone())?;
                    match qe {
                        Expr::Integer(i, _) => {
                            code.push_str(&format!("stack.push(Value::Integer({}));", i))
                        }
                        Expr::Atom(a, _) => code
                            .push_str(&format!("stack.push(Value::Atom(\"{}\".to_string()));", a)),
                        Expr::Thunk(_, _) => panic!("Can't quote a thunk"),
                    }
                }
                a => {
                    code.push_str(&format!(
                        "{{
let t = env.get(\"{}\").expect(\"Unbound var {}\").clone();
call_value(&mut env, stack, t);
}}",
                        a, a,
                    ));
                }
            },
            Expr::Thunk(exprs, _) => {
                let name = util::random_name();
                let f = compile_exprs_to_f(exprs, name.clone());

                code.push_str(&f);
                code.push(';');

                code.push_str(&format!(
                    "stack.push(Value::Thunk {{ env: env.clone(), fp: {name} }});"
                ));
            }
        }
    }

    code.push('}');
    code
}

pub fn main_function() -> String {
    r#"
fn main() {
    println!("Hello, world!");
    let mut stack = Vec::new();
    let env = make_env();
    top_level(env, &mut stack)
}"#
    .to_string()
}

pub fn compile(exprs: &[Expr]) -> String {
    let mut code = String::new();
    code.push_str(HEADER);

    code.push_str(&compile_exprs_to_f(exprs, "top_level".to_string()));

    code.push_str(&main_function());

    code
}

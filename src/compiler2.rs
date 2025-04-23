use std::{collections::HashMap, fmt::Display};

use itertools::Itertools;

use crate::{
    cps::{self, ExprCPS},
    parser::Expr,
    util,
};

// Push and pop are reserved primitives

pub const HEADER: &str = include_str!("./header/header.rs");

fn eprint_expr_cps_ref(tag: &str, exprs: &[ExprCPSRef]) {
    eprint!("{tag} [");
    for e in exprs.iter() {
        eprint!("{e} ");
    }
    eprintln!("]");
}

#[derive(Debug, Clone)]
pub enum ExprCPSRef {
    IntegerLiteral(i64),
    AtomLiteral(String),
    ThunkRef(String),
    ForceByCC,     // Pops CC first, then the thunk to force
    ForceByCCBare, // Pops CC, forces CC
    Terminate,
    Push,
    Pop,
}

impl Display for ExprCPSRef {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ExprCPSRef::IntegerLiteral(i) => f.write_fmt(format_args!("{}", i)),
            ExprCPSRef::AtomLiteral(a) => f.write_fmt(format_args!("'{}", a)),
            ExprCPSRef::ThunkRef(tr) => f.write_fmt(format_args!("&{tr}")),
            ExprCPSRef::ForceByCC => f.write_fmt(format_args!("-forceCC")),
            ExprCPSRef::ForceByCCBare => f.write_fmt(format_args!("-forceCCbare")),
            ExprCPSRef::Terminate => f.write_fmt(format_args!("-terminate")),
            ExprCPSRef::Pop => f.write_fmt(format_args!("-pop")),
            ExprCPSRef::Push => f.write_fmt(format_args!("-push")),
        }
    }
}

pub type CPSProgram = HashMap<String, Vec<ExprCPSRef>>;

pub fn expr_cps_to_program(exprs: &[ExprCPS]) -> CPSProgram {
    fn internal(prog: &mut CPSProgram, name: String, exprs: &[ExprCPS]) {
        let v = exprs
            .iter()
            .map(|e| match e {
                ExprCPS::IntegerLiteral(i, _) => ExprCPSRef::IntegerLiteral(*i),
                ExprCPS::AtomLiteral(a, _) => ExprCPSRef::AtomLiteral(a.to_string()),
                ExprCPS::Thunk(vec, _) => {
                    let name = util::random_name();
                    internal(prog, name.to_string(), vec);
                    ExprCPSRef::ThunkRef(name.to_string())
                }
                ExprCPS::ForceCC(_) => ExprCPSRef::ForceByCC,
                ExprCPS::Terminate => ExprCPSRef::Terminate,
                ExprCPS::Pop(_) => ExprCPSRef::Pop,
                ExprCPS::Push(_) => ExprCPSRef::Push,
                ExprCPS::Force(_) => panic!("Force without CC not possible here"),
                ExprCPS::ForceCCBare(_) => ExprCPSRef::ForceByCCBare,
            })
            .collect();

        prog.insert(name, v);
    }

    let mut prog = HashMap::new();

    internal(&mut prog, "entry".to_string(), exprs);

    prog
}

pub fn main_function() -> String {
    r#"
fn main() {
    println!("Hello, world!");
    let mut stack = Vec::new();
    let mut env = make_env();
    top_level(&mut env, &mut stack);
//println!("{stack:#?}");
}"#
    .to_string()
}

fn make_thunk_ref_enum(prog: &CPSProgram) -> String {
    let mut code = String::new();
    code.push_str(
        "#[allow(non_camel_case_types)] #[derive(Clone, Debug, PartialEq)] enum ThunkRef {",
    );
    code.push_str(&prog.keys().join(","));
    code.push('}');

    code
}

fn compile_toplevel(prog: &CPSProgram, opts: &CompilerOptions) -> String {
    let mut code = String::new();
    code.push_str("fn top_level(env: &mut Env, stack: &mut Stack) {");

    code.push_str("let mut cur_frame =  Frame{tr: ThunkRef::entry, env: env.clone()};");

    code.push_str("loop {");

    if opts.tracing_exec() {
        code.push_str("eprintln!(\"EXEC {:?}\", cur_frame.tr);");
    }

    if opts.tracing_env() {
        code.push_str("eprintln!(\"ENV {:?}\", cur_frame.env);");
    }

    code.push_str("match cur_frame.tr {");

    for (name, eexprs) in prog.iter() {
        code.push_str(&format!("ThunkRef::{name} => {{"));
        code.push_str("/*");
        code.push_str(&format!("{:?}", eexprs));
        code.push_str("*/");
        code.push_str(&compile_expr_cps_ref(eexprs, opts));
        code.push_str("},");
    }

    code.push('}'); // Match

    if opts.tracing_stack() {
        code.push_str("eprint!(\"STACK\");");
        code.push_str("for v in stack.iter() {eprint!(\" {v}\")} eprintln!(\"\");");
    }

    code.push('}'); // loop

    code.push('}'); // fn top_level

    code
}

fn compile_instruction_tracing(code: &mut String, ee: &ExprCPSRef) {
    code.push_str(&match ee {
        ExprCPSRef::IntegerLiteral(i) => format!("eprintln!(\"INST int {i}\");"),
        ExprCPSRef::AtomLiteral(a) => format!("eprintln!(\"INST atom {a}\");"),
        ExprCPSRef::ThunkRef(tf) => format!("eprintln!(\"INST tr {tf}\");"),
        ExprCPSRef::Terminate => "eprintln!(\"INST terminate\");".to_string(),
        ExprCPSRef::Push => "eprintln!(\"INST push\");".to_string(),
        ExprCPSRef::Pop => "eprintln!(\"INST pop\");".to_string(),
        ExprCPSRef::ForceByCC => "eprintln!(\"INST force-cc\");".to_string(),
        ExprCPSRef::ForceByCCBare => "eprintln!(\"INST force-cc-bare\");".to_string(),
    })
}

fn compile_expr_cps_ref(eexprs: &[ExprCPSRef], opts: &CompilerOptions) -> String {
    let mut code = String::new();

    let mut exs = eexprs;
    loop {
        if exs.is_empty() {
            break;
        }

        let e;

        (e, exs) = exs.split_first().unwrap();

        if opts.tracing_instructions() {
            compile_instruction_tracing(&mut code, e);
        }

        match e {
            ExprCPSRef::IntegerLiteral(i) => {
                code.push_str(&format!("stack.push(Value::Integer({i}));"))
            }
            ExprCPSRef::AtomLiteral(a) => {
                code.push_str(&format!("stack.push(Value::Atom(\"{}\".to_string()));", a))
            }

            ExprCPSRef::ThunkRef(tf) => code.push_str(&format!(
                "stack.push(Value::Thunk {{ env: cur_frame.env.clone(), fp: ThunkRef::{tf} }});"
            )),

            ExprCPSRef::Terminate => code.push_str("break;"),

            ExprCPSRef::Push => code.push_str("builtin_push(&mut cur_frame.env, stack);"),
            ExprCPSRef::Pop => code.push_str("builtin_pop(&mut cur_frame.env, stack);"),

            ExprCPSRef::ForceByCC => {
                code.push_str(r#"{ cur_frame = builtin_force_cc(stack, &mut cur_frame); }"#);
            }
            ExprCPSRef::ForceByCCBare => {
                code.push_str(r#"{ cur_frame = builtin_force_cc_bare(stack); }"#)
            }
        }
    }

    code
}

#[derive(Debug, Default)]
pub struct CompilerOptions {
    pub debug: bool,
    pub tracing: bool,
    pub tracing_exec: bool,
    pub tracing_env: bool,
    pub tracing_instructions: bool,
    pub tracing_stack: bool,
}

impl CompilerOptions {
    fn tracing_exec(&self) -> bool {
        self.tracing || self.tracing_exec
    }
    fn tracing_env(&self) -> bool {
        self.tracing || self.tracing_env
    }
    fn tracing_instructions(&self) -> bool {
        self.tracing || self.tracing_instructions
    }
    fn tracing_stack(&self) -> bool {
        self.tracing || self.tracing_stack
    }
}

pub fn compile(exprs: &[Expr], opts: &CompilerOptions) -> String {
    let expr_cps = cps::expr_cps(exprs);

    if opts.debug {
        for e in expr_cps.iter() {
            eprintln!("{e}");
        }
    }

    let prog3 = expr_cps_to_program(&expr_cps);

    if opts.debug {
        for (name, eexprs) in prog3.iter() {
            eprint_expr_cps_ref(&format!("{name} -> "), eexprs);
        }
    }

    let mut code = String::new();

    code.push_str(&filter_header(HEADER));

    code.push_str(&make_thunk_ref_enum(&prog3));

    code.push_str(&compile_toplevel(&prog3, opts));

    code.push_str(&main_function());

    code
}

fn filter_header(header: &str) -> String {
    let mut lv: Vec<_> = header.lines().collect();
    let mut retaining = true;
    lv.retain(|l| {
        if l.starts_with("// REMOVE") {
            retaining = false;
        }
        if l.starts_with("// ENDREMOVE") {
            retaining = true;
            return false; // Skip this line
        }
        retaining
    });
    lv.join("\n")
}

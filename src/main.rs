use std::io;

use chumsky::Parser;
use clap::{Parser as ClapParser, Subcommand};
use frospy::{
    compiler2,
    eval::{self, EvalStacktrace},
    parser::parser, //trace_ctx, Ctx
};

#[derive(ClapParser, Debug)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand, Debug)]
enum Command {
    Eval,
    Compile,
}

fn eval() {
    let src = io::read_to_string(io::stdin()).expect("reading stdin");

    println!("{:?}", src);

    let (v, errs) = parser().parse_recovery_verbose(src.clone());

    errs.into_iter().for_each(|e| println!("{:#?}", e));

    if let Some(ast) = v {
        println!("{:?}", ast);

        match eval::eval(&ast) {
            Ok(s) => {
                for (i, v) in s.iter().enumerate() {
                    println!("s {}: {:}", i, v);
                }
            }
            Err(EvalStacktrace { stack, error }) => {
                println!("Error: {:?}", error);
                for (idx, span) in stack.iter().enumerate() {
                    println!("{} - {:?} \t {}", idx, span.clone(), &src[span.clone()]);
                }
            }
        }

        // let mut ctx = Ctx::new(ast);

        // trace_ctx(&ctx);

        // while !ctx.pump() {
        //     trace_ctx(&ctx);
        // }
    }
}

fn compile() {
    let src = io::read_to_string(io::stdin()).expect("reading stdin");

    let (v, errs) = parser().parse_recovery_verbose(src.clone());

    errs.into_iter().for_each(|e| println!("{:#?}", e));

    if let Some(ast) = v {
        println!(
            "{}",
            compiler2::compile(
                &ast,
                &compiler2::CompilerOptions {
                    debug: true,
                    tracing_exec: true,
                    ..Default::default()
                }
            )
        );
    } else {
        panic!();
    }
}

fn main() {
    let cli = Cli::parse();
    match cli.command {
        Command::Eval => eval(),
        Command::Compile => compile(),
    }
}

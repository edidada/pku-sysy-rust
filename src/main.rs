use std::{env, fs, process::ExitCode};
fn run() -> Result<(), String> {
    let mut args: Vec<_> = env::args().skip(1).collect();
    let object=args.first().is_some_and(|arg|arg=="--object");
    if object {args.remove(0);}
    if args.len() != 4 || args[2] != "-o" {
        return Err("usage: sysy_compiler [--object] -koopa|-riscv|-perf INPUT -o OUTPUT".into());
    }
    let source = fs::read_to_string(&args[1]).map_err(|error| format!("{}: {error}", args[1]))?;
    let output = if object {sysy_compiler::compile(&source, &args[0])?}else{sysy_compiler::compile_program(&source, &args[0])?};
    fs::write(&args[3], output).map_err(|error| format!("{}: {error}", args[3]))
}
fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => { eprintln!("{error}"); ExitCode::FAILURE }
    }
}

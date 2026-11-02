use std::{env, fs, process::ExitCode};
fn run() -> Result<(), String> {
    let args: Vec<_> = env::args().skip(1).collect();
    if args.len() != 4 || args[2] != "-o" {
        return Err("usage: sysy_compiler -koopa|-riscv|-perf INPUT -o OUTPUT".into());
    }
    let source = fs::read_to_string(&args[1]).map_err(|error| format!("{}: {error}", args[1]))?;
    let output = sysy_compiler::compile(&source, &args[0])?;
    fs::write(&args[3], output).map_err(|error| format!("{}: {error}", args[3]))
}
fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => { eprintln!("{error}"); ExitCode::FAILURE }
    }
}

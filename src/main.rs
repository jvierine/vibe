mod ast;
mod check;
mod codegen;
mod lexer;
mod parser;
mod units;

use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

fn main() {
    if let Err(e) = run() {
        eprintln!("vibec: {e}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), String> {
    let args: Vec<String> = env::args().skip(1).collect();
    if args.is_empty() || matches!(args[0].as_str(), "-h" | "--help") {
        help();
        return Ok(());
    }
    let command = args[0].as_str();
    let source = args
        .get(1)
        .ok_or_else(|| format!("{command} requires a .vibe file"))?;
    let checked = load(source)?;
    match command {
        "check" => println!(
            "checked {}: {} semantic objects, types and units valid",
            source,
            checked.functions.len()
        ),
        "graph" | "show" => print!("{}", codegen::architecture(&checked)),
        "export-json" => print!("{}", codegen::semantic_json(&checked)),
        "emit-c" => print!("{}", codegen::emit_c(&checked)?),
        "build" | "run" => {
            let output = output_arg(&args).unwrap_or_else(|| default_output(source));
            build(&checked, &output)?;
            if command == "build" {
                println!("built {}", output.display());
            } else {
                let status = Command::new(&output)
                    .status()
                    .map_err(|e| format!("could not run {}: {e}", output.display()))?;
                if !status.success() {
                    return Err(format!("program exited with {status}"));
                }
            }
        }
        _ => return Err(format!("unknown command '{command}'")),
    }
    Ok(())
}

fn load(path: &str) -> Result<check::CheckedProgram, String> {
    let source = fs::read_to_string(path).map_err(|e| format!("cannot read {path}: {e}"))?;
    let tokens = lexer::lex(&source)?;
    let program = parser::parse(tokens)?;
    check::check(program).map_err(|errors| errors.join("\n"))
}

fn build(checked: &check::CheckedProgram, output: &Path) -> Result<(), String> {
    let c = codegen::emit_c(checked)?;
    let parent = output.parent().unwrap_or_else(|| Path::new("."));
    fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    let stem = output
        .file_name()
        .and_then(|x| x.to_str())
        .unwrap_or("a.out");
    let c_path = parent.join(format!(".{stem}.vibec.c"));
    fs::write(&c_path, c).map_err(|e| format!("cannot write generated C: {e}"))?;
    let result = Command::new("clang")
        .args(["-std=c17", "-O2", "-Wall", "-Wextra", "-Werror"])
        .arg(&c_path)
        .arg("-o")
        .arg(output)
        .output()
        .map_err(|e| format!("could not invoke clang: {e}"))?;
    let _ = fs::remove_file(&c_path);
    if !result.status.success() {
        return Err(format!(
            "native backend failed:\n{}",
            String::from_utf8_lossy(&result.stderr)
        ));
    }
    Ok(())
}

fn output_arg(args: &[String]) -> Option<PathBuf> {
    args.windows(2)
        .find(|w| w[0] == "-o")
        .map(|w| PathBuf::from(&w[1]))
}
fn default_output(source: &str) -> PathBuf {
    Path::new(source).with_extension("")
}

fn help() {
    println!(
        "vibec 0.1.0-bootstrap\n\nUsage:\n  vibec check FILE\n  vibec build FILE [-o OUTPUT]\n  vibec run FILE [-o OUTPUT]\n  vibec graph FILE              # human-readable architecture\n  vibec show FILE               # human-readable semantic view\n  vibec export-json FILE        # optional machine interchange on stdout\n  vibec emit-c FILE\n"
    );
}

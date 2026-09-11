mod ast;
mod check;
mod codegen;
mod environment;
mod lexer;
mod parser;
mod units;

use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

fn main() {
    if let Err(e) = run() {
        if env::args().nth(1).as_deref() == Some("env") {
            eprintln!("{}", environment::error_json(&e));
        } else {
            eprintln!("vibec: {e}");
        }
        std::process::exit(1);
    }
}

fn run() -> Result<(), String> {
    let args: Vec<String> = env::args().skip(1).collect();
    if args.is_empty() || matches!(args[0].as_str(), "-h" | "--help") {
        help();
        return Ok(());
    }
    if args[0] == "env" {
        return run_environment(&args);
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
        "graph" => print!("{}", codegen::architecture(&checked)),
        "show" => match args.get(2) {
            Some(id) => print!("{}", codegen::object_view(&checked, id)?),
            None => print!("{}", codegen::architecture(&checked)),
        },
        "inspect-json" => {
            let id = args
                .get(2)
                .ok_or_else(|| "inspect-json requires a semantic identity".to_string())?;
            print!("{}", codegen::object_json(&checked, id)?);
        }
        "callers" | "callees" | "impact" => {
            let id = args
                .get(2)
                .ok_or_else(|| format!("{command} requires a semantic identity"))?;
            let ids = match command {
                "callers" => codegen::callers(&checked, id)?,
                "callees" => codegen::callees(&checked, id)?,
                "impact" => codegen::impact(&checked, id)?,
                _ => unreachable!(),
            };
            if ids.is_empty() {
                println!("-");
            } else {
                for id in ids {
                    println!("{id}");
                }
            }
        }
        "export-json" => print!("{}", codegen::semantic_json(&checked)),
        "emit-c" => print!("{}", codegen::emit_c(&checked)?),
        "build" | "run" => {
            if !checked.functions.contains_key("@app.main") {
                return Err("native programs require @app.main with signature () -> i32".into());
            }
            let output = output_arg(&args).unwrap_or_else(|| default_output(source));
            build(&checked, &output)?;
            if command == "build" {
                println!("built {}", output.display());
            } else {
                let executable = fs::canonicalize(&output).map_err(|e| {
                    format!("could not resolve built program {}: {e}", output.display())
                })?;
                let status = Command::new(&executable)
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

fn run_environment(args: &[String]) -> Result<(), String> {
    let command = args
        .get(1)
        .ok_or_else(|| "env requires a command".to_string())?;
    let project = args
        .get(2)
        .ok_or_else(|| format!("env {command} requires a project directory"))?;
    let project = Path::new(project);
    match command.as_str() {
        "apply" => {
            let request = environment::read_request()?;
            println!("{}", environment::apply(project, &request)?);
        }
        "inspect" => {
            let id = args
                .get(3)
                .ok_or_else(|| "env inspect requires a semantic identity".to_string())?;
            println!("{}", environment::inspect(project, id)?);
        }
        "check" => {
            let (checked, revision) = environment::load(project)?;
            println!(
                "{{\"schema\":\"vibe.check.result.v0\",\"revision\":\"{revision}\",\"objects\":{},\"status\":\"checked\"}}",
                checked.functions.len()
            );
        }
        "graph" => {
            let (checked, _) = environment::load(project)?;
            print!("{}", codegen::architecture(&checked));
        }
        "build" | "run" => {
            let (checked, _) = environment::load(project)?;
            if !checked.functions.contains_key("@app.main") {
                return Err("native programs require @app.main with signature () -> i32".into());
            }
            let output = output_arg(args).unwrap_or_else(|| {
                if project.extension().and_then(|value| value.to_str()) == Some("vibepack") {
                    project.with_extension("")
                } else {
                    project.join("build/program")
                }
            });
            build(&checked, &output)?;
            if command == "build" {
                println!("built {}", output.display());
            } else {
                let executable = fs::canonicalize(&output).map_err(|error| {
                    format!(
                        "could not resolve built program {}: {error}",
                        output.display()
                    )
                })?;
                let status = Command::new(&executable)
                    .status()
                    .map_err(|error| format!("could not run {}: {error}", output.display()))?;
                if !status.success() {
                    return Err(format!("program exited with {status}"));
                }
            }
        }
        _ => return Err(format!("unknown environment command '{command}'")),
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
        "vibec 0.1.0-bootstrap\n\nEnvironment-owned programs:\n  vibec env apply PROJECT       # typed transaction on stdin\n  vibec env inspect PROJECT @id # bounded semantic query\n  vibec env check PROJECT\n  vibec env graph PROJECT\n  vibec env build PROJECT [-o OUTPUT]\n  vibec env run PROJECT [-o OUTPUT]\n\nBootstrap imports:\n  vibec check FILE\n  vibec build FILE [-o OUTPUT]\n  vibec run FILE [-o OUTPUT]\n  vibec graph FILE              # semantic architecture projection\n  vibec show FILE [@id]         # bounded semantic projection\n  vibec inspect-json FILE @id   # bounded LLM/tool projection\n  vibec callers FILE @id\n  vibec callees FILE @id\n  vibec impact FILE @id         # transitive affected callers\n  vibec export-json FILE        # explicit whole-program export\n  vibec emit-c FILE             # internal bootstrap backend output\n"
    );
}

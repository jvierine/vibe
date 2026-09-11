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
            println!(
                "{}",
                environment::inspect(project, id, args.get(4).map(String::as_str))?
            );
        }
        "check" => {
            let (checked, revision) =
                environment::load_at(project, args.get(3).map(String::as_str))?;
            println!(
                "{{\"schema\":\"vibe.check.result.v0\",\"revision\":\"{revision}\",\"objects\":{},\"status\":\"checked\"}}",
                checked.functions.len()
            );
        }
        "graph" => {
            let (checked, _) = environment::load_at(project, args.get(3).map(String::as_str))?;
            print!("{}", codegen::architecture(&checked));
        }
        "branches" => println!("{}", environment::branches(project)?),
        "history" => println!(
            "{}",
            environment::history(project, args.get(3).map(String::as_str))?
        ),
        "branch" => {
            let name = args
                .get(3)
                .ok_or_else(|| "env branch requires a branch name".to_string())?;
            println!(
                "{}",
                environment::create_branch(project, name, args.get(4).map(String::as_str))?
            );
        }
        "diff" => {
            let from = args
                .get(3)
                .ok_or_else(|| "env diff requires FROM and TO selectors".to_string())?;
            let to = args
                .get(4)
                .ok_or_else(|| "env diff requires FROM and TO selectors".to_string())?;
            println!("{}", environment::diff(project, from, to)?);
        }
        "merge" => {
            let target = args
                .get(3)
                .ok_or_else(|| "env merge requires TARGET and SOURCE branches".to_string())?;
            let source = args
                .get(4)
                .ok_or_else(|| "env merge requires TARGET and SOURCE branches".to_string())?;
            let name = args.get(5).map(String::as_str).unwrap_or("semantic_merge");
            println!("{}", environment::merge(project, target, source, name)?);
        }
        "upgrade" => println!("{}", environment::upgrade(project)?),
        "git-textconv" => print!("{}", environment::git_textconv(project)?),
        "git-merge-driver" => {
            let current = args
                .get(3)
                .ok_or_else(|| "git-merge-driver requires BASE CURRENT OTHER".to_string())?;
            let other = args
                .get(4)
                .ok_or_else(|| "git-merge-driver requires BASE CURRENT OTHER".to_string())?;
            environment::git_merge_driver(project, Path::new(current), Path::new(other))?;
        }
        "git-configure" => println!("{}", environment::git_configure(project)?),
        "build" | "run" => {
            let (checked, _) = environment::load_at(project, option_arg(args, "--at"))?;
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
fn option_arg<'a>(args: &'a [String], option: &str) -> Option<&'a str> {
    args.windows(2)
        .find(|window| window[0] == option)
        .map(|window| window[1].as_str())
}
fn default_output(source: &str) -> PathBuf {
    Path::new(source).with_extension("")
}

fn help() {
    println!(
        "vibec 0.1.0-bootstrap\n\nEnvironment-owned programs:\n  vibec env apply PROJECT                         # typed transaction on stdin\n  vibec env inspect PROJECT @id [REV_OR_BRANCH]  # bounded semantic query\n  vibec env check PROJECT [REV_OR_BRANCH]\n  vibec env graph PROJECT [REV_OR_BRANCH]\n  vibec env branches PROJECT\n  vibec env history PROJECT [REV_OR_BRANCH]\n  vibec env branch PROJECT NAME [FROM]\n  vibec env diff PROJECT FROM TO\n  vibec env merge PROJECT TARGET SOURCE [NAME]\n  vibec env upgrade PROJECT\n  vibec env git-configure PROJECT\n  vibec env build PROJECT [--at REV_OR_BRANCH] [-o OUTPUT]\n  vibec env run PROJECT [--at REV_OR_BRANCH] [-o OUTPUT]\n\nBootstrap imports:\n  vibec check FILE\n  vibec build FILE [-o OUTPUT]\n  vibec run FILE [-o OUTPUT]\n  vibec graph FILE              # semantic architecture projection\n  vibec show FILE [@id]         # bounded semantic projection\n  vibec inspect-json FILE @id   # bounded LLM/tool projection\n  vibec callers FILE @id\n  vibec callees FILE @id\n  vibec impact FILE @id         # transitive affected callers\n  vibec export-json FILE        # explicit whole-program export\n  vibec emit-c FILE             # internal bootstrap backend output\n"
    );
}

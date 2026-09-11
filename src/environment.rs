use crate::ast::{Expr, Function, Param, Program, Scalar, Stmt, TypeSyntax};
use crate::{check, codegen};
use fs2::FileExt;
use serde::{Deserialize, Serialize};
use serde_json::json;
use sha2::{Digest, Sha256};
use std::fs::{self, File, OpenOptions};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};

const STORE_SCHEMA: u32 = 1;
const TRANSACTION_SCHEMA: &str = "vibe.transaction.v0";
const MAX_TRANSACTION_BYTES: usize = 1024 * 1024;
const MAX_OPERATIONS: usize = 1024;

#[derive(Serialize, Deserialize)]
struct Store {
    schema: u32,
    program: Program,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Transaction {
    schema: String,
    name: String,
    base_revision: Option<String>,
    reads: Vec<Precondition>,
    operations: Vec<Operation>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Precondition {
    id: String,
    revision: String,
}

#[derive(Deserialize)]
#[serde(tag = "op", rename_all = "snake_case", deny_unknown_fields)]
enum Operation {
    PutFunction {
        expected_revision: Option<String>,
        object: ApiFunction,
    },
    DeleteObject {
        id: String,
        expected_revision: Option<String>,
    },
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ApiFunction {
    id: String,
    #[serde(default)]
    parameters: Vec<ApiParameter>,
    result: ApiType,
    body: Vec<ApiStatement>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ApiParameter {
    name: String,
    r#type: ApiType,
}

#[derive(Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
enum ApiType {
    Scalar {
        scalar: Scalar,
        #[serde(default)]
        unit: Option<String>,
    },
    Array {
        element: Scalar,
        rank: usize,
        #[serde(default)]
        unit: Option<String>,
        #[serde(default)]
        mutable: bool,
    },
    None,
}

#[derive(Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
enum ApiStatement {
    Let {
        name: String,
        #[serde(default)]
        mutable: bool,
        #[serde(default)]
        annotation: Option<ApiType>,
        value: ApiExpression,
    },
    Assign {
        target: ApiExpression,
        value: ApiExpression,
    },
    For {
        index: String,
        start: ApiExpression,
        end: ApiExpression,
        body: Vec<ApiStatement>,
    },
    Return {
        #[serde(default)]
        value: Option<ApiExpression>,
    },
    Print {
        value: ApiExpression,
        #[serde(default)]
        unit: Option<String>,
    },
    Expression {
        value: ApiExpression,
    },
}

#[derive(Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
enum ApiExpression {
    String {
        value: String,
    },
    Number {
        value: String,
        scalar: Scalar,
        #[serde(default)]
        unit: Option<String>,
    },
    Variable {
        name: String,
    },
    Array {
        values: Vec<ApiExpression>,
    },
    Index {
        array: Box<ApiExpression>,
        index: Box<ApiExpression>,
    },
    Call {
        function: String,
        #[serde(default)]
        arguments: Vec<ApiExpression>,
    },
    Binary {
        operator: String,
        left: Box<ApiExpression>,
        right: Box<ApiExpression>,
    },
    Unary {
        operator: String,
        value: Box<ApiExpression>,
    },
}

pub fn apply(project: &Path, request: &str) -> Result<String, String> {
    if request.len() > MAX_TRANSACTION_BYTES {
        return Err(format!(
            "transaction exceeds the {MAX_TRANSACTION_BYTES}-byte limit"
        ));
    }
    let raw: serde_json::Value = serde_json::from_str(request)
        .map_err(|error| format!("invalid transaction JSON: {error}"))?;
    if !raw
        .as_object()
        .is_some_and(|object| object.contains_key("base_revision"))
    {
        return Err("transaction must include base_revision (null only for a new project)".into());
    }
    let root = raw
        .as_object()
        .ok_or_else(|| "transaction must be a JSON object".to_string())?;
    if !root.contains_key("reads") {
        return Err("transaction must include an explicit reads array".into());
    }
    if !root
        .get("operations")
        .and_then(serde_json::Value::as_array)
        .is_some_and(|operations| {
            operations.iter().all(|operation| {
                operation
                    .as_object()
                    .is_some_and(|operation| operation.contains_key("expected_revision"))
            })
        })
    {
        return Err("every operation must include expected_revision (null for creation)".into());
    }
    let transaction: Transaction = serde_json::from_value(raw)
        .map_err(|error| format!("invalid transaction schema: {error}"))?;
    validate_transaction_header(&transaction)?;

    let paths = Paths::new(project);
    fs::create_dir_all(&paths.state_dir)
        .map_err(|error| format!("cannot create environment store: {error}"))?;
    let lock = OpenOptions::new()
        .create(true)
        .read(true)
        .write(true)
        .truncate(false)
        .open(&paths.lock)
        .map_err(|error| format!("cannot open environment lock: {error}"))?;
    lock.lock_exclusive()
        .map_err(|error| format!("cannot lock environment: {error}"))?;

    let current = read_store(&paths.store)?;
    let current_revision = current.as_ref().map(|(_, bytes)| revision(bytes));
    match (&current_revision, &transaction.base_revision) {
        (None, None) | (Some(_), Some(_)) => {}
        (None, Some(expected)) => {
            return Err(format!(
                "revision conflict: project is new but transaction expects {expected}"
            ));
        }
        (Some(_), None) => {
            return Err(
                "revision conflict: base_revision null is only valid for a new project".into(),
            );
        }
    }

    let mut program = current
        .map(|(store, _)| store.program)
        .unwrap_or(Program { functions: vec![] });
    validate_read_set(&program, &transaction.reads)?;
    let declared_reads = transaction
        .reads
        .iter()
        .map(|read| read.id.as_str())
        .collect::<std::collections::BTreeSet<_>>();
    let mut changed = Vec::new();
    for operation in transaction.operations {
        match operation {
            Operation::PutFunction {
                expected_revision,
                object,
            } => {
                let function = object.into_function()?;
                validate_expected_revision(&program, &function.id, expected_revision.as_deref())?;
                changed.push(function.id.clone());
                if let Some(existing) = program
                    .functions
                    .iter_mut()
                    .find(|existing| existing.id == function.id)
                {
                    *existing = function;
                } else {
                    program.functions.push(function);
                }
            }
            Operation::DeleteObject {
                id,
                expected_revision,
            } => {
                validate_identity(&id)?;
                validate_expected_revision(&program, &id, expected_revision.as_deref())?;
                let before = program.functions.len();
                program.functions.retain(|function| function.id != id);
                if program.functions.len() == before {
                    return Err(format!("cannot delete unknown semantic object '{id}'"));
                }
                changed.push(id);
            }
        }
    }
    program
        .functions
        .sort_by(|left, right| left.id.cmp(&right.id));
    let checked = check::check(program.clone()).map_err(|errors| {
        format!(
            "transaction '{}' failed validation:\n{}",
            transaction.name,
            errors.join("\n")
        )
    })?;
    let changed_ids = changed
        .iter()
        .map(String::as_str)
        .collect::<std::collections::BTreeSet<_>>();
    for id in &changed_ids {
        for dependency in checked.calls.get(*id).into_iter().flatten() {
            if !changed_ids.contains(dependency.as_str())
                && !declared_reads.contains(dependency.as_str())
            {
                return Err(format!(
                    "transaction used '{dependency}' without declaring its object revision in reads"
                ));
            }
        }
    }

    let store = Store {
        schema: STORE_SCHEMA,
        program,
    };
    let bytes = encode_store(&store)?;
    write_store(&paths, &bytes)?;
    changed.sort();
    changed.dedup();
    let changed = changed
        .into_iter()
        .map(|id| {
            let revision = store
                .program
                .functions
                .iter()
                .find(|function| function.id == id)
                .map(object_revision);
            json!({"id": id, "revision": revision})
        })
        .collect::<Vec<_>>();
    Ok(json!({
        "schema": "vibe.transaction.result.v0",
        "status": "committed",
        "name": transaction.name,
        "previous_revision": current_revision,
        "revision": revision(&bytes),
        "changed": changed,
    })
    .to_string())
}

pub fn load(project: &Path) -> Result<(check::CheckedProgram, String), String> {
    let paths = Paths::new(project);
    let (store, bytes) = read_store(&paths.store)?
        .ok_or_else(|| format!("no Vibe environment exists at {}", project.display()))?;
    if store.schema != STORE_SCHEMA {
        return Err(format!("unsupported environment schema {}", store.schema));
    }
    let checked = check::check(store.program).map_err(|errors| errors.join("\n"))?;
    Ok((checked, revision(&bytes)))
}

pub fn inspect(project: &Path, id: &str) -> Result<String, String> {
    let (checked, revision) = load(project)?;
    let object = codegen::object_json(&checked, id)?;
    let object: serde_json::Value = serde_json::from_str(&object)
        .map_err(|error| format!("internal object encoding failed: {error}"))?;
    Ok(json!({
        "schema": "vibe.query.result.v0",
        "revision": revision,
        "object_revision": object_revision(function_for(&checked.program, id)?),
        "result": object["object"],
    })
    .to_string())
}

pub fn read_request() -> Result<String, String> {
    let mut request = String::new();
    std::io::stdin()
        .take((MAX_TRANSACTION_BYTES + 1) as u64)
        .read_to_string(&mut request)
        .map_err(|error| format!("cannot read transaction from stdin: {error}"))?;
    Ok(request)
}

pub fn error_json(error: &str) -> String {
    json!({
        "schema": "vibe.error.v0",
        "status": "rejected",
        "error": error,
    })
    .to_string()
}

impl ApiFunction {
    fn into_function(self) -> Result<Function, String> {
        validate_identity(&self.id)?;
        let params = self
            .parameters
            .into_iter()
            .map(|parameter| {
                validate_local_name(&parameter.name)?;
                Ok(Param {
                    name: parameter.name,
                    ty: parameter.r#type.into_type()?,
                })
            })
            .collect::<Result<Vec<_>, String>>()?;
        Ok(Function {
            id: self.id,
            params,
            result: self.result.into_type()?,
            body: self
                .body
                .into_iter()
                .map(ApiStatement::into_statement)
                .collect::<Result<Vec<_>, _>>()?,
            line: 0,
        })
    }
}

impl ApiType {
    fn into_type(self) -> Result<TypeSyntax, String> {
        Ok(match self {
            Self::Scalar { scalar, unit } => TypeSyntax::Scalar(scalar, unit),
            Self::Array {
                element,
                rank,
                unit,
                mutable,
            } => {
                if rank == 0 {
                    return Err("array rank must be at least 1".into());
                }
                TypeSyntax::Array {
                    element,
                    rank,
                    unit,
                    mutable,
                }
            }
            Self::None => TypeSyntax::None,
        })
    }
}

impl ApiStatement {
    fn into_statement(self) -> Result<Stmt, String> {
        Ok(match self {
            Self::Let {
                name,
                mutable,
                annotation,
                value,
            } => {
                validate_local_name(&name)?;
                Stmt::Let {
                    name,
                    mutable,
                    annotation: annotation.map(ApiType::into_type).transpose()?,
                    value: value.into_expression()?,
                }
            }
            Self::Assign { target, value } => Stmt::Assign {
                target: target.into_expression()?,
                value: value.into_expression()?,
            },
            Self::For {
                index,
                start,
                end,
                body,
            } => {
                validate_local_name(&index)?;
                Stmt::For {
                    index,
                    start: start.into_expression()?,
                    end: end.into_expression()?,
                    body: body
                        .into_iter()
                        .map(ApiStatement::into_statement)
                        .collect::<Result<Vec<_>, _>>()?,
                }
            }
            Self::Return { value } => {
                Stmt::Return(value.map(ApiExpression::into_expression).transpose()?)
            }
            Self::Print { value, unit } => Stmt::Print {
                value: value.into_expression()?,
                unit,
            },
            Self::Expression { value } => Stmt::Expr(value.into_expression()?),
        })
    }
}

impl ApiExpression {
    fn into_expression(self) -> Result<Expr, String> {
        Ok(match self {
            Self::String { value } => Expr::String(value),
            Self::Number {
                value,
                scalar,
                unit,
            } => Expr::Number {
                text: value,
                scalar,
                unit,
            },
            Self::Variable { name } => {
                validate_local_name(&name)?;
                Expr::Var(name)
            }
            Self::Array { values } => Expr::Array(
                values
                    .into_iter()
                    .map(ApiExpression::into_expression)
                    .collect::<Result<Vec<_>, _>>()?,
            ),
            Self::Index { array, index } => Expr::Index {
                array: Box::new(array.into_expression()?),
                index: Box::new(index.into_expression()?),
            },
            Self::Call {
                function,
                arguments,
            } => {
                if function != "len" {
                    validate_identity(&function)?;
                }
                Expr::Call {
                    function,
                    args: arguments
                        .into_iter()
                        .map(ApiExpression::into_expression)
                        .collect::<Result<Vec<_>, _>>()?,
                }
            }
            Self::Binary {
                operator,
                left,
                right,
            } => Expr::Binary {
                op: operator_char(&operator, "+-*/")?,
                left: Box::new(left.into_expression()?),
                right: Box::new(right.into_expression()?),
            },
            Self::Unary { operator, value } => Expr::Unary {
                op: operator_char(&operator, "+-")?,
                value: Box::new(value.into_expression()?),
            },
        })
    }
}

fn validate_transaction_header(transaction: &Transaction) -> Result<(), String> {
    if transaction.schema != TRANSACTION_SCHEMA {
        return Err(format!(
            "unsupported transaction schema '{}'; expected {TRANSACTION_SCHEMA}",
            transaction.schema
        ));
    }
    if transaction.name.is_empty() || transaction.name.len() > 128 {
        return Err("transaction name must contain 1 to 128 bytes".into());
    }
    if transaction.operations.is_empty() || transaction.operations.len() > MAX_OPERATIONS {
        return Err(format!(
            "transaction must contain 1 to {MAX_OPERATIONS} operations"
        ));
    }
    Ok(())
}

fn validate_read_set(program: &Program, reads: &[Precondition]) -> Result<(), String> {
    let mut seen = std::collections::BTreeSet::new();
    for read in reads {
        validate_identity(&read.id)?;
        if !seen.insert(&read.id) {
            return Err(format!("duplicate read precondition for '{}'", read.id));
        }
        let actual = function_for(program, &read.id).map(object_revision)?;
        if actual != read.revision {
            return Err(format!(
                "object revision conflict for '{}': expected {}, actual {actual}",
                read.id, read.revision
            ));
        }
    }
    Ok(())
}

fn validate_expected_revision(
    program: &Program,
    id: &str,
    expected: Option<&str>,
) -> Result<(), String> {
    let current = program
        .functions
        .iter()
        .find(|function| function.id == id)
        .map(object_revision);
    match (current.as_deref(), expected) {
        (None, None) => Ok(()),
        (Some(actual), Some(expected)) if actual == expected => Ok(()),
        (None, Some(expected)) => Err(format!(
            "object revision conflict for '{id}': object is absent, expected {expected}"
        )),
        (Some(actual), None) => Err(format!(
            "object revision conflict for '{id}': creation expected absence, actual {actual}"
        )),
        (Some(actual), Some(expected)) => Err(format!(
            "object revision conflict for '{id}': expected {expected}, actual {actual}"
        )),
    }
}

fn function_for<'a>(program: &'a Program, id: &str) -> Result<&'a Function, String> {
    program
        .functions
        .iter()
        .find(|function| function.id == id)
        .ok_or_else(|| format!("unknown semantic object '{id}'"))
}

fn operator_char(operator: &str, allowed: &str) -> Result<char, String> {
    let mut chars = operator.chars();
    let value = chars.next().filter(|value| allowed.contains(*value));
    if value.is_none() || chars.next().is_some() {
        return Err(format!("invalid operator '{operator}'"));
    }
    Ok(value.expect("validated operator"))
}

fn validate_local_name(name: &str) -> Result<(), String> {
    let mut chars = name.chars();
    let valid = chars
        .next()
        .is_some_and(|value| value.is_ascii_alphabetic() || value == '_')
        && chars.all(|value| value.is_ascii_alphanumeric() || value == '_')
        && !matches!(
            name,
            "const"
                | "type"
                | "fn"
                | "let"
                | "mut"
                | "if"
                | "for"
                | "in"
                | "while"
                | "match"
                | "return"
                | "extern"
                | "unsafe"
                | "test"
                | "property"
                | "print"
                | "none"
        );
    if valid {
        Ok(())
    } else {
        Err(format!("invalid local identity '{name}'"))
    }
}

fn validate_identity(id: &str) -> Result<(), String> {
    let valid = id.strip_prefix('@').is_some_and(|rest| {
        !rest.is_empty()
            && rest
                .split('.')
                .all(|segment| validate_local_name(segment).is_ok())
    });
    if valid {
        Ok(())
    } else {
        Err(format!("invalid semantic identity '{id}'"))
    }
}

fn encode_store(store: &Store) -> Result<Vec<u8>, String> {
    let mut bytes = Vec::new();
    ciborium::ser::into_writer(store, &mut bytes)
        .map_err(|error| format!("cannot encode environment store: {error}"))?;
    Ok(bytes)
}

fn read_store(path: &Path) -> Result<Option<(Store, Vec<u8>)>, String> {
    let bytes = match fs::read(path) {
        Ok(bytes) => bytes,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(format!("cannot read environment store: {error}")),
    };
    let store: Store = ciborium::de::from_reader(bytes.as_slice())
        .map_err(|error| format!("invalid environment store: {error}"))?;
    Ok(Some((store, bytes)))
}

fn write_store(paths: &Paths, bytes: &[u8]) -> Result<(), String> {
    let temporary = &paths.temporary;
    let mut file = File::create(temporary)
        .map_err(|error| format!("cannot create temporary store: {error}"))?;
    file.write_all(bytes)
        .map_err(|error| format!("cannot write temporary store: {error}"))?;
    file.sync_all()
        .map_err(|error| format!("cannot sync temporary store: {error}"))?;
    fs::rename(temporary, &paths.store)
        .map_err(|error| format!("cannot commit environment store: {error}"))?;
    File::open(&paths.state_dir)
        .and_then(|directory| directory.sync_all())
        .map_err(|error| format!("cannot sync environment directory: {error}"))?;
    Ok(())
}

fn revision(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    let mut output = String::from("sha256:");
    for byte in digest {
        output.push_str(&format!("{byte:02x}"));
    }
    output
}

fn object_revision(function: &Function) -> String {
    let mut bytes = Vec::new();
    ciborium::ser::into_writer(function, &mut bytes)
        .expect("serializing a semantic function into memory cannot fail");
    revision(&bytes)
}

struct Paths {
    state_dir: PathBuf,
    store: PathBuf,
    lock: PathBuf,
    temporary: PathBuf,
}

impl Paths {
    fn new(project: &Path) -> Self {
        if project.extension().and_then(|value| value.to_str()) == Some("vibepack") {
            let state_dir = project
                .parent()
                .unwrap_or_else(|| Path::new("."))
                .to_path_buf();
            let name = project
                .file_name()
                .and_then(|value| value.to_str())
                .unwrap_or("program.vibepack");
            return Self {
                state_dir: state_dir.clone(),
                store: project.to_path_buf(),
                lock: state_dir.join(format!(".{name}.lock")),
                temporary: state_dir.join(format!(".{name}.tmp-{}", std::process::id())),
            };
        }
        let state_dir = project.join(".vibe");
        Self {
            store: state_dir.join("program.cbor"),
            lock: state_dir.join("write.lock"),
            temporary: state_dir.join(format!(".program.cbor.tmp-{}", std::process::id())),
            state_dir,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::{Value, json};
    use std::sync::{Arc, Barrier};

    fn temporary_project(name: &str) -> PathBuf {
        std::env::temp_dir().join(format!("vibec-{name}-{}", std::process::id()))
    }

    fn put_transaction(name: &str, base: Value, id: &str) -> String {
        let (result, body) = if id == "@app.main" {
            (
                json!({"kind": "scalar", "scalar": "i32"}),
                json!([{"kind": "return", "value": {
                    "kind": "number", "value": "0", "scalar": "i32"
                }}]),
            )
        } else {
            (json!({"kind": "none"}), json!([{"kind": "return"}]))
        };
        json!({
            "schema": TRANSACTION_SCHEMA,
            "name": name,
            "base_revision": base,
            "reads": [],
            "operations": [{
                "op": "put_function",
                "expected_revision": null,
                "object": {
                    "id": id,
                    "parameters": [],
                    "result": result,
                    "body": body,
                }
            }]
        })
        .to_string()
    }

    fn result_revision(result: &str) -> String {
        serde_json::from_str::<Value>(result).unwrap()["revision"]
            .as_str()
            .unwrap()
            .to_string()
    }

    #[test]
    fn transaction_round_trip_and_validation_are_atomic() {
        let project = temporary_project("transaction");
        let _ = fs::remove_dir_all(&project);
        let initial = apply(
            &project,
            &put_transaction("initialize", Value::Null, "@app.main"),
        )
        .unwrap();
        let revision = result_revision(&initial);
        let before = load(&project).unwrap().1;
        let inspected: Value =
            serde_json::from_str(&inspect(&project, "@app.main").unwrap()).unwrap();
        let object_revision = inspected["object_revision"].as_str().unwrap();

        let invalid = put_transaction(
            "invalid_main",
            Value::String(revision),
            "@app.main.replacement",
        );
        let mut invalid: Value =
            serde_json::from_str(&invalid.replace("@app.main.replacement", "@app.main")).unwrap();
        invalid["operations"][0]["expected_revision"] = Value::String(object_revision.into());
        assert!(apply(&project, &invalid.to_string()).is_err());
        assert_eq!(load(&project).unwrap().1, before);
        let _ = fs::remove_dir_all(project);
    }

    #[test]
    fn concurrent_disjoint_writers_merge_from_the_same_base_revision() {
        let project = temporary_project("concurrency");
        let _ = fs::remove_dir_all(&project);
        let initial = apply(
            &project,
            &put_transaction("initialize", Value::Null, "@app.main"),
        )
        .unwrap();
        let revision = result_revision(&initial);
        let barrier = Arc::new(Barrier::new(3));
        let mut writers = Vec::new();
        for (name, id) in [("writer_one", "@worker.one"), ("writer_two", "@worker.two")] {
            let project = project.clone();
            let barrier = Arc::clone(&barrier);
            let request = put_transaction(name, Value::String(revision.clone()), id);
            writers.push(std::thread::spawn(move || {
                barrier.wait();
                apply(&project, &request)
            }));
        }
        barrier.wait();
        let results = writers
            .into_iter()
            .map(|writer| writer.join().unwrap())
            .collect::<Vec<_>>();
        assert_eq!(results.iter().filter(|result| result.is_ok()).count(), 2);
        assert!(results.iter().all(|result| result.is_ok()));
        let checked = load(&project).unwrap().0;
        assert!(checked.functions.contains_key("@worker.one"));
        assert!(checked.functions.contains_key("@worker.two"));
        let _ = fs::remove_dir_all(project);
    }

    #[test]
    fn concurrent_writers_to_one_object_conflict() {
        let project = temporary_project("overlap");
        let _ = fs::remove_dir_all(&project);
        let initial = apply(
            &project,
            &put_transaction("initialize", Value::Null, "@app.main"),
        )
        .unwrap();
        let root_revision = result_revision(&initial);
        let inspected: Value =
            serde_json::from_str(&inspect(&project, "@app.main").unwrap()).unwrap();
        let object_revision = inspected["object_revision"].as_str().unwrap().to_string();
        let barrier = Arc::new(Barrier::new(3));
        let mut writers = Vec::new();
        for (name, value) in [("writer_one", "1"), ("writer_two", "2")] {
            let project = project.clone();
            let barrier = Arc::clone(&barrier);
            let mut request: Value = serde_json::from_str(&put_transaction(
                name,
                Value::String(root_revision.clone()),
                "@app.main",
            ))
            .unwrap();
            request["operations"][0]["expected_revision"] = Value::String(object_revision.clone());
            request["operations"][0]["object"]["body"][0]["value"]["value"] =
                Value::String(value.into());
            writers.push(std::thread::spawn(move || {
                barrier.wait();
                apply(&project, &request.to_string())
            }));
        }
        barrier.wait();
        let results = writers
            .into_iter()
            .map(|writer| writer.join().unwrap())
            .collect::<Vec<_>>();
        assert_eq!(results.iter().filter(|result| result.is_ok()).count(), 1);
        assert_eq!(results.iter().filter(|result| result.is_err()).count(), 1);
        assert!(
            results
                .iter()
                .filter_map(|result| result.as_ref().err())
                .any(|error| error.contains("object revision conflict"))
        );
        let _ = fs::remove_dir_all(project);
    }
}

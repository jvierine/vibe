use crate::ast::{Expr, Function, Param, Program, Scalar, Stmt, TypeSyntax};
use crate::{check, codegen};
use fs2::FileExt;
use serde::{Deserialize, Serialize};
use serde_json::json;
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::fs::{self, File, OpenOptions};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::process::Command;

const STORE_SCHEMA: u32 = 2;
const TRANSACTION_SCHEMA: &str = "vibe.transaction.v0";
const MAX_TRANSACTION_BYTES: usize = 1024 * 1024;
const MAX_OPERATIONS: usize = 1024;
const MAX_BOOTSTRAP_STORE_BYTES: usize = 64 * 1024 * 1024;

#[derive(Serialize, Deserialize)]
struct Store {
    schema: u32,
    objects: BTreeMap<String, Function>,
    commits: BTreeMap<String, Commit>,
    branches: BTreeMap<String, String>,
    default_branch: String,
}

#[derive(Clone, Serialize, Deserialize)]
struct Commit {
    parents: Vec<String>,
    basis: Option<String>,
    name: String,
    tree: BTreeMap<String, String>,
}

#[derive(Deserialize)]
struct LegacyStore {
    schema: u32,
    program: Program,
}

#[derive(Deserialize)]
#[serde(untagged)]
enum StoredFormat {
    Current(Store),
    Legacy(LegacyStore),
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Transaction {
    schema: String,
    name: String,
    base_revision: Option<String>,
    branch: String,
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
    validate_branch_name(&transaction.branch)?;

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

    let mut store = read_store(&paths.store)?.unwrap_or_else(empty_store);
    let current_revision = store.branches.get(&transaction.branch).cloned();
    if current_revision.is_none() && !store.commits.is_empty() {
        return Err(format!(
            "unknown branch '{}'; create it from an existing revision first",
            transaction.branch
        ));
    }
    if store.commits.is_empty() && transaction.branch != store.default_branch {
        return Err(format!(
            "the first transaction must initialize default branch '{}'",
            store.default_branch
        ));
    }
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
    if let (Some(base), Some(head)) = (&transaction.base_revision, &current_revision) {
        if !store.commits.contains_key(base) {
            return Err(format!("unknown base revision '{base}'"));
        }
        if !is_ancestor(&store, base, head) {
            return Err(format!(
                "base revision {base} is not an ancestor of branch '{}' at {head}",
                transaction.branch
            ));
        }
    }

    let mut program = match current_revision.as_deref() {
        Some(revision) => program_at(&store, revision)?,
        None => Program { functions: vec![] },
    };
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

    let tree = intern_program(&mut store, &program);
    let current_tree = current_revision
        .as_deref()
        .and_then(|revision| store.commits.get(revision))
        .map(|commit| &commit.tree);
    if current_tree == Some(&tree) {
        return Ok(json!({
            "schema": "vibe.transaction.result.v0",
            "status": "unchanged",
            "name": transaction.name,
            "branch": transaction.branch,
            "previous_revision": current_revision,
            "revision": current_revision,
            "changed": [],
        })
        .to_string());
    }
    let commit = Commit {
        parents: current_revision.iter().cloned().collect(),
        basis: transaction
            .base_revision
            .filter(|basis| Some(basis) != current_revision.as_ref()),
        name: transaction.name.clone(),
        tree,
    };
    let new_revision = commit_revision(&commit);
    store.commits.insert(new_revision.clone(), commit);
    store
        .branches
        .insert(transaction.branch.clone(), new_revision.clone());
    validate_store(&store)?;
    let bytes = encode_store(&store)?;
    write_store(&paths, &bytes)?;
    changed.sort();
    changed.dedup();
    let changed = changed
        .into_iter()
        .map(|id| {
            let revision = store
                .commits
                .get(&new_revision)
                .and_then(|commit| commit.tree.get(&id))
                .cloned();
            json!({"id": id, "revision": revision})
        })
        .collect::<Vec<_>>();
    Ok(json!({
        "schema": "vibe.transaction.result.v0",
        "status": "committed",
        "name": transaction.name,
        "branch": transaction.branch,
        "previous_revision": current_revision,
        "revision": new_revision,
        "changed": changed,
    })
    .to_string())
}

pub fn load_at(
    project: &Path,
    selector: Option<&str>,
) -> Result<(check::CheckedProgram, String), String> {
    let paths = Paths::new(project);
    let store = read_store(&paths.store)?
        .ok_or_else(|| format!("no Vibe environment exists at {}", project.display()))?;
    if store.schema != STORE_SCHEMA {
        return Err(format!("unsupported environment schema {}", store.schema));
    }
    let revision = resolve_revision(&store, selector)?;
    let program = program_at(&store, &revision)?;
    let checked = check::check(program).map_err(|errors| errors.join("\n"))?;
    Ok((checked, revision))
}

pub fn inspect(project: &Path, id: &str, selector: Option<&str>) -> Result<String, String> {
    let (checked, revision) = load_at(project, selector)?;
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

pub fn branches(project: &Path) -> Result<String, String> {
    let paths = Paths::new(project);
    let store = read_store(&paths.store)?
        .ok_or_else(|| format!("no Vibe environment exists at {}", project.display()))?;
    let branches = store
        .branches
        .iter()
        .map(|(name, revision)| {
            json!({
                "name": name,
                "revision": revision,
                "default": name == &store.default_branch,
            })
        })
        .collect::<Vec<_>>();
    Ok(json!({"schema": "vibe.branches.v0", "branches": branches}).to_string())
}

pub fn history(project: &Path, selector: Option<&str>) -> Result<String, String> {
    let paths = Paths::new(project);
    let store = read_store(&paths.store)?
        .ok_or_else(|| format!("no Vibe environment exists at {}", project.display()))?;
    let head = resolve_revision(&store, selector)?;
    let mut pending = VecDeque::from([head.clone()]);
    let mut seen = BTreeSet::new();
    let mut commits = Vec::new();
    let mut truncated = false;
    while let Some(revision) = pending.pop_front() {
        if commits.len() == 100 {
            truncated = true;
            break;
        }
        if !seen.insert(revision.clone()) {
            continue;
        }
        let commit = store
            .commits
            .get(&revision)
            .ok_or_else(|| format!("store references missing commit '{revision}'"))?;
        commits.push(json!({
            "revision": revision,
            "parents": commit.parents,
            "basis": commit.basis,
            "name": commit.name,
            "objects": commit.tree.len(),
        }));
        pending.extend(commit.parents.iter().cloned());
    }
    Ok(json!({
        "schema": "vibe.history.v0",
        "head": head,
        "commits": commits,
        "truncated": truncated || !pending.is_empty(),
    })
    .to_string())
}

pub fn create_branch(project: &Path, name: &str, from: Option<&str>) -> Result<String, String> {
    validate_branch_name(name)?;
    let paths = Paths::new(project);
    fs::create_dir_all(&paths.state_dir)
        .map_err(|error| format!("cannot create environment store: {error}"))?;
    let _lock = lock_environment(&paths)?;
    let mut store = read_store(&paths.store)?
        .ok_or_else(|| format!("no Vibe environment exists at {}", project.display()))?;
    if store.branches.contains_key(name) {
        return Err(format!("branch '{name}' already exists"));
    }
    let revision = resolve_revision(&store, from)?;
    store.branches.insert(name.into(), revision.clone());
    write_store(&paths, &encode_store(&store)?)?;
    Ok(json!({
        "schema": "vibe.branch.result.v0",
        "status": "created",
        "branch": name,
        "revision": revision,
    })
    .to_string())
}

pub fn diff(project: &Path, from: &str, to: &str) -> Result<String, String> {
    let paths = Paths::new(project);
    let store = read_store(&paths.store)?
        .ok_or_else(|| format!("no Vibe environment exists at {}", project.display()))?;
    let from_revision = resolve_revision(&store, Some(from))?;
    let to_revision = resolve_revision(&store, Some(to))?;
    let from_tree = &store.commits[&from_revision].tree;
    let to_tree = &store.commits[&to_revision].tree;
    let identities = from_tree
        .keys()
        .chain(to_tree.keys())
        .cloned()
        .collect::<BTreeSet<_>>();
    let mut added = Vec::new();
    let mut removed = Vec::new();
    let mut modified = Vec::new();
    for id in identities {
        match (from_tree.get(&id), to_tree.get(&id)) {
            (None, Some(revision)) => added.push(json!({"id": id, "revision": revision})),
            (Some(revision), None) => removed.push(json!({"id": id, "revision": revision})),
            (Some(before), Some(after)) if before != after => modified.push(json!({
                "id": id,
                "before": before,
                "after": after,
            })),
            _ => {}
        }
    }
    Ok(json!({
        "schema": "vibe.diff.v0",
        "from": from_revision,
        "to": to_revision,
        "added": added,
        "removed": removed,
        "modified": modified,
    })
    .to_string())
}

pub fn merge(project: &Path, target: &str, source: &str, name: &str) -> Result<String, String> {
    validate_branch_name(target)?;
    validate_branch_name(source)?;
    if target == source {
        return Err("merge source and target branches must differ".into());
    }
    let paths = Paths::new(project);
    let _lock = lock_environment(&paths)?;
    let mut store = read_store(&paths.store)?
        .ok_or_else(|| format!("no Vibe environment exists at {}", project.display()))?;
    let ours = store
        .branches
        .get(target)
        .cloned()
        .ok_or_else(|| format!("unknown target branch '{target}'"))?;
    let theirs = store
        .branches
        .get(source)
        .cloned()
        .ok_or_else(|| format!("unknown source branch '{source}'"))?;
    if is_ancestor(&store, &theirs, &ours) {
        return Ok(json!({
            "schema": "vibe.merge.result.v0",
            "status": "unchanged",
            "branch": target,
            "revision": ours,
        })
        .to_string());
    }
    if is_ancestor(&store, &ours, &theirs) {
        store.branches.insert(target.into(), theirs.clone());
        write_store(&paths, &encode_store(&store)?)?;
        return Ok(json!({
            "schema": "vibe.merge.result.v0",
            "status": "fast_forward",
            "branch": target,
            "revision": theirs,
        })
        .to_string());
    }
    let base = merge_base(&store, &ours, &theirs)?;
    let base_tree = &store.commits[&base].tree;
    let our_tree = &store.commits[&ours].tree;
    let their_tree = &store.commits[&theirs].tree;
    let identities = base_tree
        .keys()
        .chain(our_tree.keys())
        .chain(their_tree.keys())
        .cloned()
        .collect::<BTreeSet<_>>();
    let mut merged = BTreeMap::new();
    let mut conflicts = Vec::new();
    for id in identities {
        let base_value = base_tree.get(&id);
        let our_value = our_tree.get(&id);
        let their_value = their_tree.get(&id);
        let selected = if our_value == their_value {
            our_value
        } else if our_value == base_value {
            their_value
        } else if their_value == base_value {
            our_value
        } else {
            conflicts.push(json!({
                "id": id,
                "base": base_value,
                "target": our_value,
                "source": their_value,
            }));
            None
        };
        if let Some(revision) = selected {
            merged.insert(id, revision.clone());
        }
    }
    if !conflicts.is_empty() {
        return Err(json!({
            "code": "semantic_merge_conflict",
            "base": base,
            "target": ours,
            "source": theirs,
            "conflicts": conflicts,
        })
        .to_string());
    }
    let program = program_from_tree(&store, &merged)?;
    check::check(program)
        .map_err(|errors| format!("merged program failed validation:\n{}", errors.join("\n")))?;
    let commit = Commit {
        parents: vec![ours.clone(), theirs.clone()],
        basis: Some(base),
        name: name.into(),
        tree: merged,
    };
    let revision = commit_revision(&commit);
    store.commits.insert(revision.clone(), commit);
    store.branches.insert(target.into(), revision.clone());
    write_store(&paths, &encode_store(&store)?)?;
    Ok(json!({
        "schema": "vibe.merge.result.v0",
        "status": "merged",
        "branch": target,
        "revision": revision,
        "parents": [ours, theirs],
    })
    .to_string())
}

pub fn upgrade(project: &Path) -> Result<String, String> {
    let paths = Paths::new(project);
    let _lock = lock_environment(&paths)?;
    let store = read_store(&paths.store)?
        .ok_or_else(|| format!("no Vibe environment exists at {}", project.display()))?;
    let revision = resolve_revision(&store, None)?;
    write_store(&paths, &encode_store(&store)?)?;
    Ok(json!({
        "schema": "vibe.upgrade.result.v0",
        "status": "current",
        "store_schema": STORE_SCHEMA,
        "revision": revision,
    })
    .to_string())
}

pub fn git_textconv(project: &Path) -> Result<String, String> {
    let paths = Paths::new(project);
    let store = read_store(&paths.store)?
        .ok_or_else(|| format!("no Vibe environment exists at {}", project.display()))?;
    let revision = resolve_revision(&store, None)?;
    let checked =
        check::check(program_at(&store, &revision)?).map_err(|errors| errors.join("\n"))?;
    let tree = &store.commits[&revision].tree;
    let mut output = format!("vibe.semantic-tree.v0 {revision}\n");
    for (id, object_revision) in tree {
        let view = codegen::object_json(&checked, id)?;
        let view: serde_json::Value = serde_json::from_str(&view)
            .map_err(|error| format!("internal object encoding failed: {error}"))?;
        output.push_str(
            &json!({
                "id": id,
                "revision": object_revision,
                "interface": view["object"],
            })
            .to_string(),
        );
        output.push('\n');
    }
    Ok(output)
}

pub fn git_merge_driver(base: &Path, current: &Path, other: &Path) -> Result<(), String> {
    let base_store = read_store_if_nonempty(base)?.unwrap_or_else(empty_store);
    let mut current_store = read_store(current)?
        .ok_or_else(|| format!("Git merge current file '{}' is missing", current.display()))?;
    let other_store = read_store(other)?
        .ok_or_else(|| format!("Git merge other file '{}' is missing", other.display()))?;
    let base_head = optional_head(&base_store);
    let current_head = resolve_revision(&current_store, None)?;
    let other_head = resolve_revision(&other_store, None)?;
    union_store(&mut current_store, &other_store)?;
    union_store(&mut current_store, &base_store)?;

    let empty_tree = BTreeMap::new();
    let base_tree = base_head
        .as_deref()
        .and_then(|revision| current_store.commits.get(revision))
        .map(|commit| &commit.tree)
        .unwrap_or(&empty_tree);
    let current_tree = &current_store.commits[&current_head].tree;
    let other_tree = &current_store.commits[&other_head].tree;
    let merged = merge_tree_maps(base_tree, current_tree, other_tree).map_err(|conflicts| {
        json!({
            "code": "git_semantic_merge_conflict",
            "base": base_head,
            "current": current_head,
            "other": other_head,
            "conflicts": conflicts,
        })
        .to_string()
    })?;
    let program = program_from_tree(&current_store, &merged)?;
    check::check(program).map_err(|errors| {
        format!(
            "Git semantic merge failed validation:\n{}",
            errors.join("\n")
        )
    })?;
    let revision = if merged == current_store.commits[&current_head].tree {
        current_head
    } else if merged == current_store.commits[&other_head].tree {
        other_head
    } else {
        let commit = Commit {
            parents: vec![current_head, other_head],
            basis: base_head,
            name: "git_semantic_merge".into(),
            tree: merged,
        };
        let revision = commit_revision(&commit);
        current_store.commits.insert(revision.clone(), commit);
        revision
    };
    current_store
        .branches
        .insert(current_store.default_branch.clone(), revision);
    validate_store(&current_store)?;
    write_direct_atomic(current, &encode_store(&current_store)?)
}

pub fn git_configure(project: &Path) -> Result<String, String> {
    let directory = if project.is_dir() {
        project
    } else {
        project.parent().unwrap_or_else(|| Path::new("."))
    };
    let root_output = Command::new("git")
        .arg("-C")
        .arg(directory)
        .args(["rev-parse", "--show-toplevel"])
        .output()
        .map_err(|error| format!("cannot invoke Git: {error}"))?;
    if !root_output.status.success() {
        return Err(format!(
            "cannot find Git repository: {}",
            String::from_utf8_lossy(&root_output.stderr).trim()
        ));
    }
    let root = String::from_utf8(root_output.stdout)
        .map_err(|error| format!("Git returned a non-UTF-8 repository path: {error}"))?;
    let root = root.trim();
    let executable = std::env::current_exe()
        .map_err(|error| format!("cannot locate vibec executable: {error}"))?;
    let executable = shell_quote(&executable.to_string_lossy());
    let settings = [
        ("merge.vibe.name", "Vibe semantic merge".to_string()),
        (
            "merge.vibe.driver",
            format!("{executable} env git-merge-driver %O %A %B"),
        ),
        (
            "diff.vibe.textconv",
            format!("{executable} env git-textconv"),
        ),
        ("diff.vibe.cachetextconv", "true".to_string()),
    ];
    for (key, value) in settings {
        let status = Command::new("git")
            .arg("-C")
            .arg(root)
            .args(["config", "--local", key, &value])
            .status()
            .map_err(|error| format!("cannot configure Git: {error}"))?;
        if !status.success() {
            return Err(format!("Git configuration failed for '{key}'"));
        }
    }
    Ok(json!({
        "schema": "vibe.git.config.result.v0",
        "status": "configured",
        "repository": root,
        "merge_driver": "vibe",
        "diff_driver": "vibe",
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
    if let Ok(detail) = serde_json::from_str::<serde_json::Value>(error) {
        return json!({
            "schema": "vibe.error.v0",
            "status": "rejected",
            "detail": detail,
        })
        .to_string();
    }
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

fn validate_branch_name(name: &str) -> Result<(), String> {
    let forbidden = [' ', '~', '^', ':', '?', '*', '[', '\\'];
    let valid = !name.is_empty()
        && name.len() <= 255
        && name.is_ascii()
        && !name.starts_with(['.', '/'])
        && !name.ends_with(['.', '/'])
        && !name.contains("..")
        && !name.contains("//")
        && !name.contains("@{")
        && !name.ends_with(".lock")
        && !name
            .chars()
            .any(|value| value.is_control() || forbidden.contains(&value));
    if valid {
        Ok(())
    } else {
        Err(format!("invalid branch name '{name}'"))
    }
}

fn encode_store(store: &Store) -> Result<Vec<u8>, String> {
    let mut bytes = Vec::new();
    ciborium::ser::into_writer(store, &mut bytes)
        .map_err(|error| format!("cannot encode environment store: {error}"))?;
    Ok(bytes)
}

fn read_store(path: &Path) -> Result<Option<Store>, String> {
    let bytes = match fs::read(path) {
        Ok(bytes) => bytes,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(format!("cannot read environment store: {error}")),
    };
    if bytes.len() > MAX_BOOTSTRAP_STORE_BYTES {
        return Err(format!(
            "environment store exceeds the {}-byte bootstrap limit",
            MAX_BOOTSTRAP_STORE_BYTES
        ));
    }
    let stored: StoredFormat = ciborium::de::from_reader(bytes.as_slice())
        .map_err(|error| format!("invalid environment store: {error}"))?;
    let store = match stored {
        StoredFormat::Current(store) => {
            if store.schema != STORE_SCHEMA {
                return Err(format!("unsupported environment schema {}", store.schema));
            }
            store
        }
        StoredFormat::Legacy(legacy) => migrate_legacy(legacy)?,
    };
    validate_store(&store)?;
    Ok(Some(store))
}

fn empty_store() -> Store {
    Store {
        schema: STORE_SCHEMA,
        objects: BTreeMap::new(),
        commits: BTreeMap::new(),
        branches: BTreeMap::new(),
        default_branch: "main".into(),
    }
}

fn migrate_legacy(mut legacy: LegacyStore) -> Result<Store, String> {
    if legacy.schema != 1 {
        return Err(format!("unsupported legacy store schema {}", legacy.schema));
    }
    legacy
        .program
        .functions
        .sort_by(|left, right| left.id.cmp(&right.id));
    check::check(legacy.program.clone())
        .map_err(|errors| format!("legacy store is invalid:\n{}", errors.join("\n")))?;
    let mut store = empty_store();
    let tree = intern_program(&mut store, &legacy.program);
    let commit = Commit {
        parents: vec![],
        basis: None,
        name: "migrate_v1".into(),
        tree,
    };
    let revision = commit_revision(&commit);
    store.commits.insert(revision.clone(), commit);
    store.branches.insert("main".into(), revision);
    Ok(store)
}

fn validate_store(store: &Store) -> Result<(), String> {
    validate_branch_name(&store.default_branch)?;
    if !store.commits.is_empty() && !store.branches.contains_key(&store.default_branch) {
        return Err(format!(
            "default branch '{}' has no head",
            store.default_branch
        ));
    }
    for (revision, function) in &store.objects {
        if object_revision(function) != *revision {
            return Err(format!("object content hash mismatch for '{revision}'"));
        }
        validate_identity(&function.id)?;
    }
    for (branch, revision) in &store.branches {
        validate_branch_name(branch)?;
        if !store.commits.contains_key(revision) {
            return Err(format!(
                "branch '{branch}' references missing commit '{revision}'"
            ));
        }
    }
    for (revision, commit) in &store.commits {
        if commit_revision(commit) != *revision {
            return Err(format!("commit content hash mismatch for '{revision}'"));
        }
        for parent in &commit.parents {
            if !store.commits.contains_key(parent) {
                return Err(format!("commit '{revision}' has missing parent '{parent}'"));
            }
        }
        for (id, stored_revision) in &commit.tree {
            let function = store.objects.get(stored_revision).ok_or_else(|| {
                format!("commit '{revision}' references missing object '{stored_revision}'")
            })?;
            if function.id != *id {
                return Err(format!(
                    "object '{id}' does not match stored revision '{stored_revision}'"
                ));
            }
        }
    }
    Ok(())
}

fn intern_program(store: &mut Store, program: &Program) -> BTreeMap<String, String> {
    let mut tree = BTreeMap::new();
    for function in &program.functions {
        let revision = object_revision(function);
        store
            .objects
            .entry(revision.clone())
            .or_insert_with(|| function.clone());
        tree.insert(function.id.clone(), revision);
    }
    tree
}

fn program_from_tree(store: &Store, tree: &BTreeMap<String, String>) -> Result<Program, String> {
    let functions = tree
        .iter()
        .map(|(id, revision)| {
            let function = store
                .objects
                .get(revision)
                .ok_or_else(|| format!("missing object revision '{revision}'"))?;
            if function.id != *id {
                return Err(format!(
                    "tree identity '{id}' does not match object '{}'",
                    function.id
                ));
            }
            Ok(function.clone())
        })
        .collect::<Result<Vec<_>, String>>()?;
    Ok(Program { functions })
}

fn program_at(store: &Store, revision: &str) -> Result<Program, String> {
    let commit = store
        .commits
        .get(revision)
        .ok_or_else(|| format!("unknown revision '{revision}'"))?;
    program_from_tree(store, &commit.tree)
}

fn resolve_revision(store: &Store, selector: Option<&str>) -> Result<String, String> {
    let selector = selector.unwrap_or(&store.default_branch);
    if let Some(revision) = store.branches.get(selector) {
        return Ok(revision.clone());
    }
    if store.commits.contains_key(selector) {
        return Ok(selector.into());
    }
    Err(format!("unknown branch or revision '{selector}'"))
}

fn commit_revision(commit: &Commit) -> String {
    let mut bytes = Vec::new();
    ciborium::ser::into_writer(commit, &mut bytes)
        .expect("serializing a commit into memory cannot fail");
    revision(&bytes)
}

fn is_ancestor(store: &Store, ancestor: &str, descendant: &str) -> bool {
    let mut pending = vec![descendant];
    let mut seen = BTreeSet::new();
    while let Some(revision) = pending.pop() {
        if revision == ancestor {
            return true;
        }
        if seen.insert(revision)
            && let Some(commit) = store.commits.get(revision)
        {
            pending.extend(commit.parents.iter().map(String::as_str));
        }
    }
    false
}

fn ancestor_distances(store: &Store, start: &str) -> BTreeMap<String, usize> {
    let mut distances = BTreeMap::new();
    let mut pending = VecDeque::from([(start.to_string(), 0usize)]);
    while let Some((revision, distance)) = pending.pop_front() {
        if distances.contains_key(&revision) {
            continue;
        }
        distances.insert(revision.clone(), distance);
        if let Some(commit) = store.commits.get(&revision) {
            pending.extend(
                commit
                    .parents
                    .iter()
                    .cloned()
                    .map(|parent| (parent, distance + 1)),
            );
        }
    }
    distances
}

fn merge_base(store: &Store, left: &str, right: &str) -> Result<String, String> {
    let left_distances = ancestor_distances(store, left);
    let right_distances = ancestor_distances(store, right);
    left_distances
        .iter()
        .filter_map(|(revision, left_distance)| {
            right_distances
                .get(revision)
                .map(|right_distance| (left_distance + right_distance, revision))
        })
        .min_by(|left, right| left.cmp(right))
        .map(|(_, revision)| revision.clone())
        .ok_or_else(|| format!("branches at {left} and {right} have no common ancestor"))
}

fn merge_tree_maps(
    base: &BTreeMap<String, String>,
    current: &BTreeMap<String, String>,
    other: &BTreeMap<String, String>,
) -> Result<BTreeMap<String, String>, Vec<serde_json::Value>> {
    let identities = base
        .keys()
        .chain(current.keys())
        .chain(other.keys())
        .cloned()
        .collect::<BTreeSet<_>>();
    let mut merged = BTreeMap::new();
    let mut conflicts = Vec::new();
    for id in identities {
        let base_value = base.get(&id);
        let current_value = current.get(&id);
        let other_value = other.get(&id);
        let selected = if current_value == other_value {
            current_value
        } else if current_value == base_value {
            other_value
        } else if other_value == base_value {
            current_value
        } else {
            conflicts.push(json!({
                "id": id,
                "base": base_value,
                "current": current_value,
                "other": other_value,
            }));
            None
        };
        if let Some(revision) = selected {
            merged.insert(id, revision.clone());
        }
    }
    if conflicts.is_empty() {
        Ok(merged)
    } else {
        Err(conflicts)
    }
}

fn union_store(target: &mut Store, source: &Store) -> Result<(), String> {
    for (revision, function) in &source.objects {
        if let Some(existing) = target.objects.get(revision)
            && object_revision(existing) != object_revision(function)
        {
            return Err(format!("object hash collision at '{revision}'"));
        }
        target
            .objects
            .entry(revision.clone())
            .or_insert_with(|| function.clone());
    }
    for (revision, commit) in &source.commits {
        if let Some(existing) = target.commits.get(revision)
            && commit_revision(existing) != commit_revision(commit)
        {
            return Err(format!("commit hash collision at '{revision}'"));
        }
        target
            .commits
            .entry(revision.clone())
            .or_insert_with(|| commit.clone());
    }
    for (branch, revision) in &source.branches {
        target
            .branches
            .entry(branch.clone())
            .or_insert_with(|| revision.clone());
    }
    Ok(())
}

fn optional_head(store: &Store) -> Option<String> {
    store.branches.get(&store.default_branch).cloned()
}

fn read_store_if_nonempty(path: &Path) -> Result<Option<Store>, String> {
    match fs::metadata(path) {
        Ok(metadata) if metadata.len() == 0 => Ok(None),
        Ok(_) => read_store(path),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(format!("cannot inspect environment store: {error}")),
    }
}

fn write_direct_atomic(path: &Path, bytes: &[u8]) -> Result<(), String> {
    let parent = path.parent().unwrap_or_else(|| Path::new("."));
    let name = path
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or("vibepack");
    let temporary = parent.join(format!(".{name}.merge-{}", std::process::id()));
    let mut file =
        File::create(&temporary).map_err(|error| format!("cannot create merged store: {error}"))?;
    file.write_all(bytes)
        .map_err(|error| format!("cannot write merged store: {error}"))?;
    file.sync_all()
        .map_err(|error| format!("cannot sync merged store: {error}"))?;
    fs::rename(&temporary, path)
        .map_err(|error| format!("cannot install merged store: {error}"))?;
    Ok(())
}

fn shell_quote(value: &str) -> String {
    format!("'{}'", value.replace('\'', "'\\''"))
}

fn lock_environment(paths: &Paths) -> Result<File, String> {
    let lock = OpenOptions::new()
        .create(true)
        .read(true)
        .write(true)
        .truncate(false)
        .open(&paths.lock)
        .map_err(|error| format!("cannot open environment lock: {error}"))?;
    lock.lock_exclusive()
        .map_err(|error| format!("cannot lock environment: {error}"))?;
    Ok(lock)
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
            "branch": "main",
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
        let before = load_at(&project, None).unwrap().1;
        let inspected: Value =
            serde_json::from_str(&inspect(&project, "@app.main", None).unwrap()).unwrap();
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
        assert_eq!(load_at(&project, None).unwrap().1, before);
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
        let checked = load_at(&project, None).unwrap().0;
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
            serde_json::from_str(&inspect(&project, "@app.main", None).unwrap()).unwrap();
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

    #[test]
    fn branches_retain_history_and_merge_disjoint_objects() {
        let project = temporary_project("branches");
        let _ = fs::remove_dir_all(&project);
        let initial = apply(
            &project,
            &put_transaction("initialize", Value::Null, "@app.main"),
        )
        .unwrap();
        let base = result_revision(&initial);
        create_branch(&project, "feature", Some(&base)).unwrap();

        let mut feature: Value = serde_json::from_str(&put_transaction(
            "feature_change",
            Value::String(base.clone()),
            "@feature.object",
        ))
        .unwrap();
        feature["branch"] = Value::String("feature".into());
        apply(&project, &feature.to_string()).unwrap();
        apply(
            &project,
            &put_transaction("main_change", Value::String(base.clone()), "@main.object"),
        )
        .unwrap();

        let merged: Value =
            serde_json::from_str(&merge(&project, "main", "feature", "merge_feature").unwrap())
                .unwrap();
        assert_eq!(merged["status"], "merged");
        let checked = load_at(&project, Some("main")).unwrap().0;
        assert!(checked.functions.contains_key("@feature.object"));
        assert!(checked.functions.contains_key("@main.object"));
        let historical = load_at(&project, Some(&base)).unwrap().0;
        assert!(!historical.functions.contains_key("@feature.object"));
        assert!(!historical.functions.contains_key("@main.object"));
        let history: Value = serde_json::from_str(&history(&project, None).unwrap()).unwrap();
        assert_eq!(
            history["commits"][0]["parents"].as_array().unwrap().len(),
            2
        );
        let _ = fs::remove_dir_all(project);
    }

    #[test]
    fn branch_merge_reports_same_object_conflicts_without_mutation() {
        let project = temporary_project("merge-conflict");
        let _ = fs::remove_dir_all(&project);
        let initial = apply(
            &project,
            &put_transaction("initialize", Value::Null, "@app.main"),
        )
        .unwrap();
        let base = result_revision(&initial);
        create_branch(&project, "feature", Some(&base)).unwrap();
        let inspected: Value =
            serde_json::from_str(&inspect(&project, "@app.main", None).unwrap()).unwrap();
        let object_revision = inspected["object_revision"].as_str().unwrap();

        for (branch, value) in [("main", "1"), ("feature", "2")] {
            let mut request: Value = serde_json::from_str(&put_transaction(
                branch,
                Value::String(base.clone()),
                "@app.main",
            ))
            .unwrap();
            request["branch"] = Value::String(branch.into());
            request["operations"][0]["expected_revision"] = Value::String(object_revision.into());
            request["operations"][0]["object"]["body"][0]["value"]["value"] =
                Value::String(value.into());
            apply(&project, &request.to_string()).unwrap();
        }
        let main_before = load_at(&project, Some("main")).unwrap().1;
        let error = merge(&project, "main", "feature", "conflict").unwrap_err();
        assert!(error.contains("semantic_merge_conflict"));
        assert!(error.contains("@app.main"));
        assert_eq!(load_at(&project, Some("main")).unwrap().1, main_before);
        let _ = fs::remove_dir_all(project);
    }

    #[test]
    fn git_merge_driver_rejects_same_object_conflicts() {
        let root = temporary_project("git-driver-conflict");
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).unwrap();
        let base_file = root.join("base.vibepack");
        let current_file = root.join("current.vibepack");
        let other_file = root.join("other.vibepack");
        let initial = apply(
            &base_file,
            &put_transaction("initialize", Value::Null, "@app.main"),
        )
        .unwrap();
        let base_revision = result_revision(&initial);
        fs::copy(&base_file, &current_file).unwrap();
        fs::copy(&base_file, &other_file).unwrap();
        let inspected: Value =
            serde_json::from_str(&inspect(&base_file, "@app.main", None).unwrap()).unwrap();
        let object_revision = inspected["object_revision"].as_str().unwrap();
        for (file, name, value) in [(&current_file, "current", "1"), (&other_file, "other", "2")] {
            let mut request: Value = serde_json::from_str(&put_transaction(
                name,
                Value::String(base_revision.clone()),
                "@app.main",
            ))
            .unwrap();
            request["operations"][0]["expected_revision"] = Value::String(object_revision.into());
            request["operations"][0]["object"]["body"][0]["value"]["value"] =
                Value::String(value.into());
            apply(file, &request.to_string()).unwrap();
        }
        let before = load_at(&current_file, None).unwrap().1;
        let error = git_merge_driver(&base_file, &current_file, &other_file).unwrap_err();
        assert!(error.contains("git_semantic_merge_conflict"));
        assert!(error.contains("@app.main"));
        assert_eq!(load_at(&current_file, None).unwrap().1, before);
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn git_merge_driver_merges_disjoint_objects_and_retains_parents() {
        let root = temporary_project("git-driver-merge");
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).unwrap();
        let base_file = root.join("base.vibepack");
        let current_file = root.join("current.vibepack");
        let other_file = root.join("other.vibepack");
        let initial = apply(
            &base_file,
            &put_transaction("initialize", Value::Null, "@app.main"),
        )
        .unwrap();
        let base_revision = result_revision(&initial);
        fs::copy(&base_file, &current_file).unwrap();
        fs::copy(&base_file, &other_file).unwrap();
        apply(
            &current_file,
            &put_transaction(
                "current",
                Value::String(base_revision.clone()),
                "@current.object",
            ),
        )
        .unwrap();
        apply(
            &other_file,
            &put_transaction("other", Value::String(base_revision), "@other.object"),
        )
        .unwrap();
        git_merge_driver(&base_file, &current_file, &other_file).unwrap();
        let checked = load_at(&current_file, None).unwrap().0;
        assert!(checked.functions.contains_key("@current.object"));
        assert!(checked.functions.contains_key("@other.object"));
        let history: Value = serde_json::from_str(&history(&current_file, None).unwrap()).unwrap();
        assert_eq!(
            history["commits"][0]["parents"].as_array().unwrap().len(),
            2
        );
        let _ = fs::remove_dir_all(root);
    }
}

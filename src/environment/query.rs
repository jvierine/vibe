//! Snapshot-bound queries and revision-scoped expression handles.
//! Indexes are derived once per request, not yet persisted across processes.
use super::*;
use crate::check::{CheckedProgram, CheckedType};

const SCHEMA: &str = "vibe.query.v1";
const MAX_OUTPUT: usize = 64 * 1024;

#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Request {
    schema: String,
    snapshot: String,
    op: String,
    #[serde(default)]
    id: Option<String>,
    #[serde(default)]
    depth: Option<usize>,
    #[serde(default)]
    transitive: bool,
    #[serde(default = "default_limit")]
    limit: usize,
    #[serde(default)]
    cursor: Option<String>,
}

fn default_limit() -> usize {
    64
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct QueryRead {
    query: Request,
    fingerprint: String,
}

pub fn capabilities() -> String {
    json!({
        "schema": "vibe.capabilities.v1",
        "query_schema": SCHEMA,
        "operations": ["structure", "definition", "type", "units", "shape", "precision",
            "deps", "users", "calls", "callers", "nodes", "effects", "pure", "layout",
            "alias", "range", "uncertainty", "tests", "cost", "generated",
            "why-not-vectorized", "why-allocation", "why-slow", "affected"],
        "request": {"required": ["schema", "snapshot", "op"],
            "optional": ["id", "depth", "transitive", "limit", "cursor"],
            "depth_scope": ["deps", "users", "calls", "callers"],
            "structure_scope": "all functions", "identity_scope": "function"},
        "pagination": {"max_records": 256, "max_bytes": MAX_OUTPUT,
            "cursor": "opaque; bind to resolved snapshot and unchanged request"},
        "editing": {"operation": "replace_expression", "fields":
            ["id", "expected_revision", "node", "value"],
            "node_identity": "revision-scoped structural path, not durable identity",
            "value_schema": "existing transaction ApiExpression (kind-tagged)"},
        "query_reads": "optional transaction preconditions: query plus returned fingerprint",
        "limits": ["whole-pack loading/checking", "in-memory call index only",
            "effects limited to io.stdout", "no alias/purity proof",
            "no persisted test or numerical evidence index", "no opaque stable identities"]
    })
    .to_string()
}

impl Request {
    fn validate(&self) -> Result<(), String> {
        if self.schema != SCHEMA {
            return Err(format!("expected query schema {SCHEMA}"));
        }
        if self.snapshot.is_empty() {
            return Err("snapshot must be a branch or revision".into());
        }
        if !(1..=256).contains(&self.limit) {
            return Err("query limit must be 1..256".into());
        }
        if self.depth == Some(0) || self.depth.is_some_and(|d| d > 1024) {
            return Err("query depth must be 1..1024".into());
        }
        if self.depth.is_some() && self.transitive {
            return Err("choose depth or transitive".into());
        }
        if (self.depth.is_some() || self.transitive)
            && !matches!(self.op.as_str(), "deps" | "users" | "calls" | "callers")
        {
            return Err("depth/transitive only applies to dependency queries".into());
        }
        if self.op == "structure" {
            if self.id.is_some() {
                return Err("structure currently lists all functions; omit id".into());
            }
        } else {
            validate_identity(self.id.as_deref().ok_or("query requires id")?)?;
        }
        Ok(())
    }
}

pub fn query(project: &Path, input: &str) -> Result<String, String> {
    if input.len() > MAX_TRANSACTION_BYTES {
        return Err("query request too large".into());
    }
    let request: Request =
        serde_json::from_str(input).map_err(|e| format!("invalid query: {e}"))?;
    request.validate()?;
    let (checked, snapshot) = load_at(project, Some(&request.snapshot))?;
    let (status, records) = evaluate(&checked, &request)?;
    let fingerprint = fingerprint(&status, &records);
    let mut bound = request.clone();
    bound.snapshot = snapshot.clone();
    bound.cursor = None;
    let binding = revision(&serde_json::to_vec(&bound).map_err(|e| e.to_string())?);
    let offset = match &request.cursor {
        None => 0,
        Some(cursor) => {
            let (key, offset) = cursor.rsplit_once('/').ok_or("invalid cursor")?;
            if key != binding {
                return Err(
                    "cursor does not match snapshot/query; use the returned snapshot".into(),
                );
            }
            offset
                .parse::<usize>()
                .map_err(|_| "invalid cursor offset")?
        }
    };
    if offset > records.len() {
        return Err("cursor offset out of range".into());
    }
    let mut end = offset;
    let mut bytes = 0;
    while end < records.len() && end - offset < request.limit {
        let next = records[end].len() + 1;
        if bytes + next > MAX_OUTPUT - 2048 {
            break;
        }
        bytes += next;
        end += 1;
    }
    if end == offset && offset < records.len() {
        return Err("single query record exceeds output budget".into());
    }
    let complete = end == records.len();
    let cursor = if complete {
        "none".into()
    } else {
        format!("{binding}/{end}")
    };
    let object_revision = request
        .id
        .as_deref()
        .map(|id| function_for(&checked.program, id).map(super::object_revision))
        .transpose()?;
    let mut output = format!(
        "schema=vibe.query.result.v1 snapshot={snapshot} status={status} complete={complete} records={} total={} cursor={cursor} fingerprint={fingerprint}\n",
        end - offset,
        records.len()
    );
    if let Some(revision) = object_revision {
        output.push_str(&format!("object_revision={revision}\n"));
    }
    for record in &records[offset..end] {
        output.push_str(record);
        output.push('\n');
    }
    Ok(output)
}

fn fingerprint(status: &str, records: &[String]) -> String {
    // No snapshot/offset: unrelated changes are allowed, including new history.
    revision(&serde_json::to_vec(&(SCHEMA, status, records)).expect("string records serialize"))
}

pub(super) fn validate_reads(checked: &CheckedProgram, reads: &[QueryRead]) -> Result<(), String> {
    if reads.len() > MAX_OPERATIONS {
        return Err("too many query read preconditions".into());
    }
    for read in reads {
        read.query.validate()?;
        if read.query.cursor.is_some() {
            return Err(
                "query read preconditions must omit cursor; fingerprints cover the full result"
                    .into(),
            );
        }
        let (status, records) = evaluate(checked, &read.query)?;
        if fingerprint(&status, &records) != read.fingerprint {
            return Err(format!(
                "query read conflict: {} {}; re-query current snapshot",
                read.query.op,
                read.query.id.as_deref().unwrap_or("workspace")
            ));
        }
    }
    Ok(())
}

fn evaluate(checked: &CheckedProgram, request: &Request) -> Result<(String, Vec<String>), String> {
    let mut records = Vec::new();
    let mut status = "known";
    if request.op == "structure" {
        for f in &checked.program.functions {
            records.push(format!("{} function revision={}", f.id, object_revision(f)));
        }
    } else {
        let id = request.id.as_deref().ok_or("query requires id")?;
        let f = function_for(&checked.program, id)?;
        let info = &checked.functions[id];
        match request.op.as_str() {
            "definition" => records.push(format!(
                "{id} function parameters={} revision={}",
                f.params.len(),
                object_revision(f)
            )),
            "type" | "units" | "shape" | "precision" => {
                for (p, ty) in f.params.iter().zip(&info.params) {
                    records.push(type_record(
                        &format!("parameter:{}", p.name),
                        ty,
                        &request.op,
                    ));
                }
                records.push(type_record("result", &info.result, &request.op));
            }
            "deps" | "calls" | "users" | "callers" => {
                let reverse = matches!(request.op.as_str(), "users" | "callers");
                let mut edges: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
                for (caller, callees) in &checked.calls {
                    for callee in callees {
                        let (a, b) = if reverse {
                            (callee, caller)
                        } else {
                            (caller, callee)
                        };
                        edges.entry(a.clone()).or_default().insert(b.clone());
                    }
                }
                let max_depth = if request.transitive {
                    usize::MAX
                } else {
                    request.depth.unwrap_or(1)
                };
                let mut queue = VecDeque::from([(id.to_string(), 0usize)]);
                let mut seen = BTreeSet::from([id.to_string()]);
                while let Some((source, depth)) = queue.pop_front() {
                    if depth >= max_depth {
                        continue;
                    }
                    for target in edges.get(&source).into_iter().flatten() {
                        records.push(format!(
                            "{source} {} {target}",
                            if reverse { "caller" } else { "call" }
                        ));
                        if seen.insert(target.clone()) {
                            queue.push_back((target.clone(), depth + 1));
                        }
                    }
                }
            }
            "nodes" => {
                statement_records(&f.body, "body", &mut records);
                let mut body = f.body.clone();
                walk_block(&mut body, "body", &mut |path, expr| {
                    records.push(format!("{path} {}", expression_summary(expr)));
                });
            }
            "effects" => {
                status = "partial";
                records.push("analysis=io.stdout-only memory-effects=unknown".into());
                records.extend(checked.effects[id].iter().map(|e| format!("effect={e}")));
            }
            "pure" | "layout" | "alias" | "range" | "uncertainty" => {
                status = "unknown";
                records.push(format!("{} no-verified-analysis", request.op));
            }
            "tests" | "cost" | "generated" | "why-not-vectorized" | "why-allocation"
            | "why-slow" | "affected" => {
                status = "unsupported";
                records.push(format!("{} no-query-provider", request.op));
            }
            other => return Err(format!("unknown query operation '{other}'")),
        }
    }
    records.sort();
    records.dedup();
    Ok((status.into(), records))
}

fn type_record(label: &str, ty: &CheckedType, op: &str) -> String {
    let (scalar, unit, rank, mutable) = match ty {
        CheckedType::Scalar(s, u) => (s, u, None, false),
        CheckedType::Array(s, r, u, m) => (s, u, Some(*r), *m),
        CheckedType::None => return format!("{label} none"),
        CheckedType::Error => return format!("{label} unknown"),
    };
    let detail = match op {
        "units" => format!("units={}", quoted(&unit.display)),
        "shape" => rank.map_or("scalar".into(), |r| format!("rank={r} dimensions=unknown")),
        "precision" => format!("representation={} accumulator=not-inferred", scalar.name()),
        _ => format!(
            "scalar={} units={} rank={} mutable={mutable}",
            scalar.name(),
            quoted(&unit.display),
            rank.unwrap_or(0)
        ),
    };
    format!("{label} {detail}")
}

fn quoted(value: &str) -> String {
    serde_json::to_string(value).expect("string serialization")
}

fn expression_summary(expr: &Expr) -> String {
    match expr {
        Expr::Number { text, scalar, unit } => format!(
            "number value={} scalar={} unit={}",
            quoted(text),
            scalar.name(),
            quoted(unit.as_deref().unwrap_or("1"))
        ),
        Expr::Complex { real, imag } => {
            format!("complex real={} imag={}", quoted(real), quoted(imag))
        }
        Expr::String(s) => {
            if s.len() <= 1024 {
                format!("string value={}", quoted(s))
            } else {
                format!(
                    "string bytes={} value=omitted hash={}",
                    s.len(),
                    revision(s.as_bytes())
                )
            }
        }
        Expr::Var(s) => format!("variable name={}", quoted(s)),
        Expr::Array(v) => format!("array elements={}", v.len()),
        Expr::Index { .. } => "index".into(),
        Expr::Call { function, args } => format!(
            "call function={} arguments={}",
            quoted(function),
            args.len()
        ),
        Expr::Binary { op, .. } => format!("binary operator={op}"),
        Expr::Unary { op, .. } => format!("unary operator={op}"),
    }
}

fn statement_records(body: &[Stmt], prefix: &str, records: &mut Vec<String>) {
    for (i, stmt) in body.iter().enumerate() {
        let path = format!("{prefix}/{i}");
        let summary = match stmt {
            Stmt::Let {
                name,
                mutable,
                annotation,
                ..
            } => format!(
                "let name={} mutable={mutable} annotation={}",
                quoted(name),
                serde_json::to_string(annotation).expect("type serialization")
            ),
            Stmt::Assign { .. } => "assign".into(),
            Stmt::For { index, body, .. } => {
                statement_records(body, &format!("{path}/body"), records);
                format!("for index={} statements={}", quoted(index), body.len())
            }
            Stmt::Return(value) => format!("return has_value={}", value.is_some()),
            Stmt::Print { unit, .. } => {
                format!("print unit={}", quoted(unit.as_deref().unwrap_or("1")))
            }
            Stmt::Expr(_) => "expression".into(),
        };
        records.push(format!("{path} {summary}"));
    }
}

pub(super) fn replace_expression(
    function: &mut Function,
    node: &str,
    value: Expr,
) -> Result<(), String> {
    let mut replacement = Some(value);
    walk_block(&mut function.body, "body", &mut |path, expr| {
        if path == node
            && let Some(value) = replacement.take()
        {
            *expr = value;
        }
    });
    if replacement.is_some() {
        return Err(format!(
            "unknown expression node '{node}'; query nodes at the expected object revision"
        ));
    }
    Ok(())
}

fn walk_block(body: &mut [Stmt], prefix: &str, visit: &mut impl FnMut(&str, &mut Expr)) {
    for (i, stmt) in body.iter_mut().enumerate() {
        let path = format!("{prefix}/{i}");
        match stmt {
            Stmt::Let { value, .. } | Stmt::Print { value, .. } | Stmt::Expr(value) => {
                walk_expr(value, &format!("{path}/value"), visit)
            }
            Stmt::Return(Some(value)) => walk_expr(value, &format!("{path}/value"), visit),
            Stmt::Return(None) => {}
            Stmt::Assign { target, value } => {
                walk_expr(target, &format!("{path}/target"), visit);
                walk_expr(value, &format!("{path}/value"), visit);
            }
            Stmt::For {
                start, end, body, ..
            } => {
                walk_expr(start, &format!("{path}/start"), visit);
                walk_expr(end, &format!("{path}/end"), visit);
                walk_block(body, &format!("{path}/body"), visit);
            }
        }
    }
}

fn walk_expr(expr: &mut Expr, path: &str, visit: &mut impl FnMut(&str, &mut Expr)) {
    visit(path, expr);
    match expr {
        Expr::Array(values) | Expr::Call { args: values, .. } => {
            for (i, value) in values.iter_mut().enumerate() {
                walk_expr(value, &format!("{path}/items/{i}"), visit);
            }
        }
        Expr::Index { array, index } => {
            walk_expr(array, &format!("{path}/array"), visit);
            walk_expr(index, &format!("{path}/index"), visit);
        }
        Expr::Binary { left, right, .. } => {
            walk_expr(left, &format!("{path}/left"), visit);
            walk_expr(right, &format!("{path}/right"), visit);
        }
        Expr::Unary { value, .. } => walk_expr(value, &format!("{path}/value"), visit),
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn checked(source: &str) -> CheckedProgram {
        check::check(crate::parser::parse(crate::lexer::lex(source).unwrap()).unwrap()).unwrap()
    }

    fn request(op: &str, id: &str) -> Request {
        serde_json::from_value(json!({"schema":SCHEMA,"snapshot":"main","op":op,"id":id})).unwrap()
    }

    #[test]
    fn dependency_traversal_is_depth_limited_and_cycle_safe() {
        let program = checked(
            "fn @a() -> i32 { return @b(); }\nfn @b() -> i32 { return @c(); }\nfn @c() -> i32 { return @a(); }",
        );
        let mut query = request("deps", "@a");
        assert_eq!(evaluate(&program, &query).unwrap().1, vec!["@a call @b"]);
        query.depth = Some(2);
        assert_eq!(
            evaluate(&program, &query).unwrap().1,
            vec!["@a call @b", "@b call @c"]
        );
        query.depth = None;
        query.transitive = true;
        assert_eq!(
            evaluate(&program, &query).unwrap().1,
            vec!["@a call @b", "@b call @c", "@c call @a"]
        );
        query.op = "callers".into();
        query.transitive = false;
        assert_eq!(evaluate(&program, &query).unwrap().1, vec!["@a caller @c"]);
    }

    #[test]
    fn nested_nodes_preserve_bindings_and_allow_exact_replacement() {
        let mut program = checked(
            "fn @scale(x: Array<f64,1>, y: mut Array<f64,1>) -> none { for i in 0i64..len(x) { y[i] = 2.0f64 * x[i]; } return; }",
        );
        let query = request("nodes", "@scale");
        let nodes = evaluate(&program, &query).unwrap().1;
        assert!(
            nodes
                .iter()
                .any(|n| n == "body/0 for index=\"i\" statements=1")
        );
        assert!(
            nodes
                .iter()
                .any(|n| n.starts_with("body/0/body/0/value/left number"))
        );
        replace_expression(
            &mut program.program.functions[0],
            "body/0/body/0/value/left",
            Expr::Number {
                text: "3.0".into(),
                scalar: Scalar::F64,
                unit: None,
            },
        )
        .unwrap();
        let updated = check::check(program.program).unwrap();
        let nodes = evaluate(&updated, &query).unwrap().1;
        assert!(
            nodes
                .iter()
                .any(|n| n.contains("value/left number value=\"3.0\""))
        );
        let shape = evaluate(&updated, &request("shape", "@scale")).unwrap().1;
        assert!(shape.contains(&"parameter:x rank=1 dimensions=unknown".into()));
    }

    #[test]
    fn request_validation_rejects_ambiguous_or_ignored_options() {
        let mut query = request("nodes", "@a");
        query.depth = Some(1);
        assert!(query.validate().is_err());
        query.op = "deps".into();
        query.transitive = true;
        assert!(query.validate().is_err());
        query.transitive = false;
        query.depth = Some(0);
        assert!(query.validate().is_err());
        assert!(
            serde_json::from_value::<Request>(
                json!({"schema":SCHEMA,"snapshot":"main","op":"nodes","id":"@a","typo":true})
            )
            .is_err()
        );
    }
}

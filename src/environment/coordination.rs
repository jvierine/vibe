//! Local advisory work board. Agent reports never become compiler facts.
use super::*;
use std::time::{SystemTime, UNIX_EPOCH};

const SCHEMA: &str = "vibe.coordination.v1";
const MAX_EVENTS: usize = 512;
const MAX_BYTES: u64 = 16 * 1024 * 1024;

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Request {
    schema: String,
    action: Action,
}

#[derive(Deserialize, Serialize)]
#[serde(tag = "op", rename_all = "snake_case", deny_unknown_fields)]
enum Action {
    Put {
        task: TaskInput,
        expected_version: Option<u64>,
    },
    List {
        scope: Vec<String>,
        limit: usize,
        after: Option<String>,
        snapshot: Option<u64>,
    },
    Events {
        scope: Vec<String>,
        since: u64,
        limit: usize,
    },
}

#[derive(Clone, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "snake_case")]
enum Status {
    Active,
    Blocked,
    Validating,
    Done,
    Cancelled,
}

#[derive(Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Evidence {
    revision: String,
    artifact: String,
    sha256: String,
    summary: String,
}

#[derive(Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct TaskInput {
    id: String,
    owner: String,
    branch: String,
    base_revision: String,
    scope: Vec<String>,
    intent: String,
    acceptance: String,
    status: Status,
    summary: String,
    blocked_on: Vec<String>,
    lease_seconds: u64,
    evidence: Vec<Evidence>,
}

#[derive(Clone, Deserialize, Serialize)]
struct Task {
    record: TaskInput,
    version: u64,
    lease_until: u64,
    request_hash: String,
}

#[derive(Deserialize, Serialize)]
struct Event {
    sequence: u64,
    // Union of old/new scopes: subscribers also see a task leave their scope.
    scope: Vec<String>,
    task: Task,
}

#[derive(Deserialize, Serialize)]
struct Board {
    schema: u32,
    sequence: u64,
    tasks: BTreeMap<String, Task>,
    events: VecDeque<Event>,
}
impl Default for Board {
    fn default() -> Self {
        Self {
            schema: 1,
            sequence: 0,
            tasks: BTreeMap::new(),
            events: VecDeque::new(),
        }
    }
}

fn terminal(status: &Status) -> bool {
    matches!(status, Status::Done | Status::Cancelled)
}
fn bounded(value: &str, max: usize) -> Result<(), String> {
    if value.trim().is_empty() || value.len() > max || value.contains('\0') {
        return Err(format!("coordination text must contain 1..{max} bytes"));
    }
    Ok(())
}
fn scopes(scope: &[String]) -> Result<(), String> {
    if scope.len() > 32 || scope.iter().collect::<BTreeSet<_>>().len() != scope.len() {
        return Err("scope must contain at most 32 unique semantic identities".into());
    }
    for id in scope {
        bounded(id, 128)?;
        validate_identity(id)?;
    }
    Ok(())
}
fn intersects(a: &[String], b: &[String]) -> bool {
    a.iter().any(|id| b.contains(id))
}
fn view(task: &Task, store: &Store, now: u64) -> serde_json::Value {
    let current = store.branches.get(&task.record.branch);
    let basis = &store.commits[&task.record.base_revision].tree;
    let stale: Vec<_> = task
        .record
        .scope
        .iter()
        .filter(|id| {
            current
                .and_then(|head| store.commits.get(head))
                .is_none_or(|commit| commit.tree.get(*id) != basis.get(*id))
        })
        .collect();
    json!({"task":task.record,"version":task.version,"lease_until":task.lease_until,
        "lease_active":!terminal(&task.record.status) && now<task.lease_until,
        "stale_scope":stale,"observed_program_revision":current,
        "evidence_authority":"agent_report_not_verified",
        "evidence_current":task.record.evidence.iter().map(|e|current==Some(&e.revision)).collect::<Vec<_>>()})
}

pub fn coordinate(project: &Path, request: &str) -> Result<String, String> {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|e| e.to_string())?
        .as_secs();
    execute(project, request, now)
}

fn execute(project: &Path, request: &str, now: u64) -> Result<String, String> {
    if request.len() > 32768 {
        return Err("coordination request exceeds 32 KiB".into());
    }
    let raw: serde_json::Value = serde_json::from_str(request).map_err(|e| e.to_string())?;
    if raw["action"]["op"] == "put" && raw["action"].get("expected_version").is_none() {
        return Err("put requires expected_version (null for creation)".into());
    }
    let req: Request =
        serde_json::from_value(raw).map_err(|e| format!("invalid coordination request: {e}"))?;
    if req.schema != SCHEMA {
        return Err("unsupported coordination schema".into());
    }
    let program_paths = Paths::new(project);
    let store = read_store(&program_paths.store)?.ok_or("project not found")?;
    let name = program_paths
        .store
        .file_name()
        .ok_or("invalid store path")?
        .to_string_lossy();
    let dir = program_paths
        .state_dir
        .join(format!(".{name}.coordination"));
    fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let paths = Paths {
        store: dir.join("board.cbor"),
        lock: dir.join("write.lock"),
        temporary: dir.join("pending.cbor"),
        state_dir: dir,
    };
    let _lock = lock_environment(&paths)?;
    let mut board: Board = if paths.store.exists() {
        if fs::metadata(&paths.store).map_err(|e| e.to_string())?.len() > MAX_BYTES {
            return Err("coordination board exceeds 16 MiB".into());
        }
        ciborium::de::from_reader(File::open(&paths.store).map_err(|e| e.to_string())?)
            .map_err(|e| format!("invalid coordination board: {e}"))?
    } else {
        Board::default()
    };
    // A replaced/rewound pack must not cause dangling-reference panics.
    if board.schema != 1 {
        return Err("unsupported coordination board schema".into());
    }
    if board
        .tasks
        .values()
        .any(|t| !store.commits.contains_key(&t.record.base_revision))
        || board
            .events
            .iter()
            .any(|e| !store.commits.contains_key(&e.task.record.base_revision))
    {
        return Err(
            "coordination references history absent from this pack; restore matching history"
                .into(),
        );
    }
    let request_hash = revision(&serde_json::to_vec(&req).map_err(|e| e.to_string())?);
    let response = match req.action {
        Action::Put {
            task,
            expected_version,
        } => {
            bounded(&task.id, 128)?;
            bounded(&task.owner, 128)?;
            bounded(&task.intent, 512)?;
            bounded(&task.acceptance, 512)?;
            bounded(&task.summary, 1024)?;
            for id in &task.blocked_on {
                bounded(id, 128)?;
            }
            scopes(&task.scope)?;
            if task.scope.is_empty()
                || task.lease_seconds == 0
                || task.lease_seconds > 3600
                || task.evidence.len() > 8
                || task.blocked_on.len() > 32
            {
                return Err(
                    "task requires scope, 1..3600 second lease, <=8 evidence and <=32 dependencies"
                        .into(),
                );
            }
            if !store.commits.contains_key(&task.base_revision)
                || !store.branches.contains_key(&task.branch)
            {
                return Err("task must name an existing branch and immutable base revision".into());
            }
            for e in &task.evidence {
                if !store.commits.contains_key(&e.revision)
                    || e.sha256.len() != 64
                    || !e.sha256.bytes().all(|b| b.is_ascii_hexdigit())
                {
                    return Err(
                        "evidence requires known revision and 64-digit artifact SHA256".into(),
                    );
                }
                bounded(&e.artifact, 512)?;
                bounded(&e.summary, 512)?;
            }
            if let Some(old) = board.tasks.get(&task.id) {
                if old.record.owner != task.owner {
                    return Err(
                        "task owner mismatch (local advisory identity, not authentication)".into(),
                    );
                }
                if old.request_hash == request_hash {
                    return Ok(json!({"schema":SCHEMA,"replayed":true,"sequence":board.sequence,"result":view(old,&store,now)}).to_string());
                }
                if expected_version != Some(old.version) {
                    return Err("task version conflict; refresh task before updating".into());
                }
            } else if expected_version.is_some() || board.tasks.len() >= 1000 {
                return Err("task does not exist or board reached 1000-task limit".into());
            }
            let mut pending = task.blocked_on.clone();
            let mut visited = BTreeSet::new();
            while let Some(id) = pending.pop() {
                if id == task.id {
                    return Err("task dependency cycle".into());
                }
                if visited.insert(id.clone()) {
                    let dependency = board
                        .tasks
                        .get(&id)
                        .ok_or_else(|| format!("unknown dependency task '{id}'"))?;
                    pending.extend(dependency.record.blocked_on.clone());
                }
            }
            let overlaps: Vec<_> = board
                .tasks
                .values()
                .filter(|old| {
                    old.record.id != task.id
                        && old.record.branch == task.branch
                        && !terminal(&old.record.status)
                        && old.lease_until > now
                        && intersects(&old.record.scope, &task.scope)
                })
                .map(|old| old.record.id.clone())
                .collect();
            let mut event_scope = task.scope.clone();
            if let Some(old) = board.tasks.get(&task.id) {
                event_scope.extend(old.record.scope.clone());
            }
            event_scope.sort();
            event_scope.dedup();
            board.sequence = board.sequence.checked_add(1).ok_or("sequence overflow")?;
            let task = Task {
                lease_until: now.saturating_add(task.lease_seconds),
                record: task,
                version: board.sequence,
                request_hash,
            };
            board.tasks.insert(task.record.id.clone(), task.clone());
            board.events.push_back(Event {
                sequence: board.sequence,
                scope: event_scope,
                task: task.clone(),
            });
            while board.events.len() > MAX_EVENTS {
                board.events.pop_front();
            }
            let mut bytes = Vec::new();
            ciborium::ser::into_writer(&board, &mut bytes).map_err(|e| e.to_string())?;
            if bytes.len() as u64 > MAX_BYTES {
                return Err("coordination board exceeds 16 MiB".into());
            }
            write_store(&paths, &bytes)?;
            json!({"schema":SCHEMA,"replayed":false,"sequence":board.sequence,"overlap_count":overlaps.len(),"overlaps":overlaps.iter().take(8).collect::<Vec<_>>(),"result":view(&task,&store,now)})
        }
        Action::List {
            scope,
            limit,
            after,
            snapshot,
        } => {
            scopes(&scope)?;
            check_limit(limit)?;
            if snapshot.is_some_and(|s| s != board.sequence)
                || after.is_some() && snapshot.is_none()
            {
                return Err("task snapshot changed; restart list".into());
            }
            let matches: Vec<_> = board
                .tasks
                .values()
                .filter(|t| {
                    after.as_ref().is_none_or(|a| &t.record.id > a)
                        && (scope.is_empty() || intersects(&scope, &t.record.scope))
                })
                .collect();
            let mut records = Vec::new();
            let mut bytes = 0;
            for task in matches.iter().take(limit) {
                let value = view(task, &store, now);
                let size = value.to_string().len();
                if bytes + size > 60 * 1024 {
                    break;
                }
                bytes += size;
                records.push(value);
            }
            if records.is_empty() && !matches.is_empty() {
                return Err("task exceeds response byte limit".into());
            }
            let next = if matches.len() > records.len() {
                Some(matches[records.len() - 1].record.id.clone())
            } else {
                None
            };
            json!({"schema":SCHEMA,"sequence":board.sequence,"next_after":next,"tasks":records})
        }
        Action::Events {
            scope,
            since,
            limit,
        } => {
            scopes(&scope)?;
            check_limit(limit)?;
            if since > board.sequence {
                return Err("event cursor is ahead of board".into());
            }
            if board.events.front().is_some_and(|e| since < e.sequence - 1) {
                return Err("event cursor expired; refresh task list and use its sequence".into());
            }
            let mut events = Vec::new();
            let mut cursor = since;
            let mut bytes = 0;
            for event in board.events.iter().filter(|e| e.sequence > since) {
                if scope.is_empty() || intersects(&scope, &event.scope) {
                    let value =
                        json!({"sequence":event.sequence,"result":view(&event.task,&store,now)});
                    let size = value.to_string().len();
                    if size > 60 * 1024 {
                        return Err("event exceeds response byte limit".into());
                    }
                    if bytes + size > 60 * 1024 {
                        break;
                    }
                    bytes += size;
                    events.push(value);
                    cursor = event.sequence;
                    if events.len() == limit {
                        break;
                    }
                }
                cursor = event.sequence;
            }
            json!({"schema":SCHEMA,"sequence":board.sequence,"next_since":cursor,"more":cursor<board.sequence,"events":events})
        }
    };
    Ok(response.to_string())
}
fn check_limit(limit: usize) -> Result<(), String> {
    if !(1..=8).contains(&limit) {
        return Err("coordination limit must be 1..8".into());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::{Value, json};
    use std::sync::atomic::{AtomicU64, Ordering};
    static NEXT: AtomicU64 = AtomicU64::new(0);
    struct Fixture {
        dir: PathBuf,
        pack: PathBuf,
        base: String,
    }
    impl Fixture {
        fn new() -> Self {
            let dir = std::env::temp_dir().join(format!(
                "vibe-coordination-{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            ));
            fs::create_dir_all(&dir).unwrap();
            let pack = dir.join("example.vibepack");
            fs::write(&pack, include_bytes!("../../examples/hello_world.vibepack")).unwrap();
            let store = read_store(&pack).unwrap().unwrap();
            let base = resolve_revision(&store, None).unwrap();
            Self { dir, pack, base }
        }
        fn put(&self, id: &str, scope: &str) -> Value {
            json!({"op":"put","expected_version":null,"task":{
                "id":id,"owner":"agent-a","branch":"main","base_revision":self.base,
                "scope":[scope],"intent":"validate numerical solver","acceptance":"reference checks pass",
                "status":"active","summary":"investigating","blocked_on":[],"lease_seconds":60,"evidence":[]}})
        }
        fn call(&self, action: Value, time: u64) -> Result<Value, String> {
            execute(
                &self.pack,
                &json!({"schema":SCHEMA,"action":action}).to_string(),
                time,
            )
            .map(|s| serde_json::from_str(&s).unwrap())
        }
        fn list(&self, time: u64) -> Value {
            self.call(json!({"op":"list","scope":[],"limit":8}), time)
                .unwrap()
        }
    }
    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.dir);
        }
    }

    #[test]
    fn revisions_retries_leases_and_scope_overlap_are_advisory() {
        let f = Fixture::new();
        let before = fs::read(&f.pack).unwrap();
        let request = f.put("solver", "@app.main");
        assert_eq!(f.call(request.clone(), 100).unwrap()["sequence"], 1);
        assert_eq!(f.call(request.clone(), 101).unwrap()["replayed"], true);
        let second = f.call(f.put("tests", "@app.main"), 101).unwrap();
        assert_eq!(second["overlaps"], json!(["solver"]));
        let mut update = request.clone();
        update["task"]["summary"] = json!("checked");
        assert!(
            f.call(update.clone(), 102)
                .unwrap_err()
                .contains("conflict")
        );
        update["expected_version"] = json!(1);
        update["task"]["owner"] = json!("agent-b");
        assert!(f.call(update.clone(), 102).unwrap_err().contains("owner"));
        update["task"]["owner"] = json!("agent-a");
        update["task"]["status"] = json!("done");
        assert_eq!(
            f.call(update, 103).unwrap()["result"]["lease_active"],
            false
        );
        assert_eq!(f.list(200)["tasks"][1]["lease_active"], false);
        assert_eq!(
            before,
            fs::read(&f.pack).unwrap(),
            "coordination must never mutate program"
        );
    }

    #[test]
    fn filtered_deltas_include_old_scope_and_paginate_without_replays() {
        let f = Fixture::new();
        let mut task = f.put("solver", "@app.main");
        f.call(task.clone(), 100).unwrap();
        f.call(f.put("other", "@greeting.say"), 100).unwrap();
        task["expected_version"] = json!(1);
        task["task"]["scope"] = json!(["@greeting.say"]);
        f.call(task, 101).unwrap();
        let events = |since| json!({"op":"events","scope":["@app.main"],"since":since,"limit":1});
        let first = f.call(events(0), 101).unwrap();
        assert_eq!(first["next_since"], 1);
        let second = f.call(events(1), 101).unwrap();
        assert_eq!(second["next_since"], 3);
        assert_eq!(
            second["events"][0]["result"]["task"]["scope"],
            json!(["@greeting.say"])
        );
        assert_eq!(f.call(events(3), 101).unwrap()["events"], json!([]));
        assert!(f.call(events(4), 101).unwrap_err().contains("ahead"));
        assert!(
            f.call(
                json!({"op":"list","scope":[],"limit":1,"after":"other","snapshot":2}),
                101
            )
            .is_err()
        );
    }

    #[test]
    fn cycles_invalid_evidence_and_unknown_fields_reject_without_writes() {
        let f = Fixture::new();
        let mut a = f.put("a", "@app.main");
        f.call(a.clone(), 100).unwrap();
        let mut b = f.put("b", "@app.main");
        b["task"]["blocked_on"] = json!(["a"]);
        f.call(b, 100).unwrap();
        a["expected_version"] = json!(1);
        a["task"]["blocked_on"] = json!(["b"]);
        assert!(f.call(a, 100).unwrap_err().contains("cycle"));
        let mut c = f.put("c", "@app.main");
        c["task"]["evidence"] =
            json!([{"revision":f.base,"artifact":"test.h5","sha256":"bad","summary":"passes"}]);
        assert!(f.call(c, 100).unwrap_err().contains("SHA256"));
        let mut c = f.put("c", "@app.main");
        c["ignored_field"] = json!(true);
        assert!(f.call(c, 100).is_err());
        let mut c = f.put("c", "@app.main");
        c.as_object_mut().unwrap().remove("expected_version");
        assert!(f.call(c, 100).is_err());
        assert_eq!(f.list(100)["sequence"], 2);
    }

    #[test]
    fn program_changes_mark_scope_and_agent_evidence_stale() {
        let f = Fixture::new();
        let mut a = f.put("a", "@app.main");
        a["task"]["evidence"] = json!([{"revision":f.base,"artifact":"reference.h5","sha256":"a".repeat(64),"summary":"agent reports passing tests"}]);
        f.call(a, 100).unwrap();
        let store = read_store(&f.pack).unwrap().unwrap();
        let object = &store.commits[&f.base].tree["@app.main"];
        let transaction = json!({"schema":TRANSACTION_SCHEMA,"name":"change entry","base_revision":f.base,"branch":"main","reads":[],"operations":[{
            "op":"put_function","expected_revision":object,"object":{"id":"@app.main","parameters":[],"result":{"kind":"scalar","scalar":"i32"},"body":[{"kind":"return","value":{"kind":"number","scalar":"i32","value":"1"}}]}}]});
        apply(&f.pack, &transaction.to_string()).unwrap();
        let list = f.list(100);
        let task = &list["tasks"][0];
        assert_eq!(task["stale_scope"], json!(["@app.main"]));
        assert_eq!(task["evidence_current"], json!([false]));
        assert_eq!(task["evidence_authority"], "agent_report_not_verified");
    }

    #[test]
    fn concurrent_agents_do_not_lose_disjoint_updates() {
        let f = Fixture::new();
        std::thread::scope(|s| {
            for i in 0..8 {
                let f = &f;
                s.spawn(move || {
                    f.call(f.put(&format!("task-{i}"), "@app.main"), 100)
                        .unwrap();
                });
            }
        });
        assert_eq!(f.list(100)["tasks"].as_array().unwrap().len(), 8);
        assert_eq!(f.list(100)["sequence"], 8);
    }

    #[test]
    fn retention_expiry_requires_explicit_resynchronization() {
        let f = Fixture::new();
        let mut task = f.put("a", "@app.main");
        for version in 0..=MAX_EVENTS {
            task["expected_version"] = if version == 0 {
                Value::Null
            } else {
                json!(version)
            };
            f.call(task.clone(), 100 + version as u64).unwrap();
        }
        assert!(
            f.call(json!({"op":"events","scope":[],"since":0,"limit":8}), 100)
                .unwrap_err()
                .contains("expired")
        );
        assert_eq!(f.list(100)["sequence"], MAX_EVENTS + 1);
    }

    #[test]
    fn native_abi_is_typed_and_revision_pinned() {
        let pack =
            Path::new(env!("CARGO_MANIFEST_DIR")).join("examples/metablate/metablate.vibepack");
        let thermal: Value =
            serde_json::from_str(&abi(&pack, "@metablate.thermal_mass_loss_si", None).unwrap())
                .unwrap();
        assert_eq!(
            thermal["result"]["si_dimensions"],
            json!({"mass":1,"time":-1})
        );
        assert_eq!(thermal["parameters"][0]["type"]["scalar"], "f64");
        let integrate: Value = serde_json::from_str(
            &abi(&pack, "@metablate.integrate", thermal["revision"].as_str()).unwrap(),
        )
        .unwrap();
        assert_eq!(integrate["parameters"][4]["type"]["mutable"], true);
        assert_eq!(integrate["parameters"][0]["type"]["mutable"], false);
        assert_eq!(integrate["symbol"], codegen::mangle("@metablate.integrate"));
        assert_eq!(integrate["revision"], thermal["revision"]);
        assert!(abi(&pack, "@missing", None).is_err());
    }

    #[test]
    fn large_records_respect_byte_budget_without_losing_events() {
        let f = Fixture::new();
        for i in 0..8 {
            let mut task = f.put(&format!("large-{i}"), "@app.main");
            task["task"]["summary"] = json!("s".repeat(1024));
            task["task"]["scope"] = json!(
                (0..32)
                    .map(|j| format!("@{}.s{j}", "x".repeat(115)))
                    .collect::<Vec<_>>()
            );
            task["task"]["evidence"]=json!((0..8).map(|_|json!({"revision":f.base,"artifact":"a".repeat(512),"sha256":"a".repeat(64),"summary":"e".repeat(512)})).collect::<Vec<_>>());
            f.call(task, 100).unwrap();
        }
        let list = f.list(100);
        assert!(list.to_string().len() < 65536);
        assert!(!list["next_after"].is_null());
        let mut since = 0;
        let mut count = 0;
        while since < 8 {
            let page = f
                .call(
                    json!({"op":"events","scope":[],"since":since,"limit":8}),
                    100,
                )
                .unwrap();
            assert!(page.to_string().len() < 65536);
            count += page["events"].as_array().unwrap().len();
            let next = page["next_since"].as_u64().unwrap();
            assert!(next > since);
            since = next;
        }
        assert_eq!(count, 8);
    }
}

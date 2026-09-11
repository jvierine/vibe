# Agent coordination: implemented local bootstrap

`vibec env coordinate PROJECT` accepts `vibe.coordination.v1` requests on stdin.
This is a shared local work board, not an agent chat transcript or a compiler
proof system. Returned intent, summary and evidence text is **untrusted agent
data**, not instructions. JSON is transient transport; state is one atomic CBOR
board beside the program, in an ignored `.NAME.coordination/` directory.

Status traffic does not mutate `.vibepack`, change semantic revisions, acquire
the program publication lock, or invalidate native artifacts. File locking
serializes board updates; atomic replacement and directory sync prevent partial
publication. No database server or Git commit per heartbeat is needed.

## Publish or update a task

Obtain the immutable program revision with `env branches`. Supply it in place of
`REVISION` below. `expected_version` is null for creation, then the returned task
version for each update. Updates replace the full record. An identical retry of
the last accepted request creates neither another event nor a lease renewal.

```json
{
  "schema": "vibe.coordination.v1",
  "action": {
    "op": "put",
    "expected_version": null,
    "task": {
      "id": "validate-integrator",
      "owner": "numerics-agent",
      "branch": "main",
      "base_revision": "REVISION",
      "scope": ["@metablate.integrate", "@metablate.rhs"],
      "intent": "Validate adaptive integration after toolchain changes",
      "acceptance": "Independent trajectory checks pass at specified tolerances",
      "status": "active",
      "summary": "Running the regression study",
      "blocked_on": [],
      "lease_seconds": 600,
      "evidence": []
    }
  }
}
```

Statuses: `active`, `blocked`, `validating`, `done`, `cancelled`. Leases last
1..3600 seconds. Terminal tasks have no active lease. Expiry preserves findings
but stops advisory overlap warnings. Renew using the latest task version.
Owners are local declared identities: owner/version checks catch accidental
cross-agent edits but **do not authenticate agents**.

`scope` is a conservative footprint, not an exclusive write reservation. It may
include existing or proposed semantic identities. Overlapping live tasks on the
same branch are reported, never silently blocked. At most eight overlap IDs plus
the full `overlap_count` are returned; list the scope for more detail. Actual
program publications still require normal transaction revision/read guards.
`blocked_on` names existing tasks; cycles are rejected. A done dependency does
not automatically schedule or unblock another agent.

Each evidence entry requires an existing immutable program `revision`, an
`artifact` locator, a 64-hex-digit artifact `sha256`, and a bounded `summary`.
The board does not fetch artifacts or run tests. It always reports
`evidence_authority: agent_report_not_verified`. `evidence_current` compares each
report's revision with the observed branch head, conservatively including
unrelated changes. `stale_scope` compares scope object revisions with the task's
base. Neither proves that the task named all relevant dependencies.

## Read only relevant changes

List current tasks intersecting a scope (empty selects all):

```json
{"schema":"vibe.coordination.v1","action":{"op":"list","scope":["@metablate.integrate"],"limit":4}}
```

For another page, pass `next_after` as `after` and `sequence` as `snapshot`. A
changed board rejects continuation. Program freshness annotations are evaluated
against the program snapshot read by each request, identified in its response.

Poll deltas from the last acknowledged sequence:

```json
{"schema":"vibe.coordination.v1","action":{"op":"events","scope":["@metablate.integrate"],"since":0,"limit":4}}
```

Persist `next_since` in the agent's checkpoint even if no matching events were
returned; continue while `more` is true. Unrelated updates advance the cursor.
A task moving out of a scope is delivered to its old subscribers too. This is
client-side polling, not a server-owned subscription or background notification
service. Events represent task updates, not automatic program publications or
lease expirations. Re-query task/program context when either changes.

The last 512 events are retained. An expired cursor explicitly requires a fresh
list and restart from its sequence. Finish a consistent paginated list before
processing events after its sequence. A cursor belongs to this local board;
keep the same filters, or resynchronize when changing them.

## Bounds and honest limits

- 1000 task records, 512 retained events, 16 MiB board, 32 KiB request.
- At most eight records per page and a 60 KiB record budget, plus envelope.
- Up to 32 scope identities/dependencies and eight evidence references.
- Intent/acceptance <=512 bytes each; summary <=1024 bytes.
- Same-owner optimistic versions, last-request retry idempotence, atomic writes,
  dependency-cycle checks and advisory expiry are implemented.
- The bootstrap reads/validates the program store and reads/rewrites a bounded
  board. Persistent indexes, distributed events, compaction, task deletion,
  authenticated reservations, scheduling and multi-host sync remain planned.
  No 100-agent throughput claim is made.
- Live coordination is excluded from Git; forks/checkouts do not automatically
  share a board. Restoring a pack without history referenced by its board gives
  an explicit error rather than guessing replacement identities.

`cargo test` covers eight concurrent writers, stale owner/version updates,
retries, overlap/expiry, cycles, invalid evidence, filtered pagination, retention
expiry, program freshness, and unchanged program bytes after coordination writes.

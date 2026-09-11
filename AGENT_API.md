# Semantic agent API

This API is the programming language's sole authoring interface. An LLM writes a
Vibe program by querying objects and submitting typed transactions to the
programming environment. It never edits source files, line ranges, or serialized
AST text. The environment chooses storage layout, canonicalizes every accepted
change, updates dependency edges, computes invalidation, and runs required
validation before commit.

The API is local-first, typed, and versioned. Its LLM tool boundary uses compact,
schema-constrained JSON because current model tool calling handles JSON reliably.
This JSON is a transient request/response message, never a project file. Embedded
compiler stages use native typed values; high-throughput non-LLM clients may use
framed CBOR. Reads use stable identities and bounded graph projections so an agent
need not ingest a repository or create intermediate files.

Core read operations:

```text
get(id, revision=current, fields=...)
neighborhood(id, edges=[calls,validated_by], depth=1)
find(kind, predicate)
callers(id) / callees(id) / dependents(id)
explain_dependency(from, to)
architecture(component)
validation(id) / uncertainty(id) / provenance(id)
trace(id, scenario, budget) / explain(claim, evidence=true)
counterexample(contract_or_claim, budget)
diff(base_root, new_root, semantic=true)
```

The environment API implemented now is:

```text
vibec env apply PROJECT        # one typed transaction from stdin
vibec env capabilities PROJECT # versioned capabilities and limits
vibec env query PROJECT        # vibe.query.v1 request from stdin; compact text result
vibec env inspect PROJECT @id  # bounded object plus root/object revisions
vibec env check PROJECT
vibec env graph PROJECT
vibec env branches PROJECT
vibec env history PROJECT [revision-or-branch]
vibec env branch PROJECT name [from]
vibec env diff PROJECT from to
vibec env merge PROJECT target source [name]
vibec env git-configure PROJECT
vibec env build PROJECT
vibec env run PROJECT
```

`PROJECT` may be one portable `.vibepack` file or an environment directory. The
current mutation set is `put_function`, `delete_object`, and `replace_expression`. Each operation has an
expected object revision, and the transaction declares revisions for all objects
it read. Disjoint writes based on the same old project root may merge; overlapping
or stale reads reject. Direct function dependencies must appear in the read set
unless changed in the same transaction. The bootstrap currently holds its lock
through whole-pack loading, validation, and commit; moving expensive validation
outside the publication lock remains planned.

## Implemented query protocol v1

Pass one request to `vibec env query PROJECT` on stdin:

```json
{"schema":"vibe.query.v1","snapshot":"main","op":"nodes","id":"@greeting.say","limit":32}
```

Requests reject unknown fields. `snapshot` is a branch or immutable revision.
`structure` lists all functions and omits `id`; other queries select a function.
`definition`, `type`, `units`, `shape`, and `precision` expose checked interfaces.
Shape reports known rank and unknown extents, not inferred static sizes. Precision
reports representations, not a numerical guarantee or inferred accumulator policy.
`nodes` returns a flat body projection: statement bindings/control structure and
expression kinds, values, and child paths. Large string contents are omitted with
their hash. No full AST is emitted by default.

`env abi PROJECT @id [REVISION]` exposes `vibe.abi.v1` typed host-C signatures,
including actual native symbols, scalar types, SI dimensions and array mutability.
Do not parse pretty-printed type descriptions for host bindings. See
[METABLATE_LESSONS.md](METABLATE_LESSONS.md). The separate local task protocol
is documented in [AGENT_COORDINATION.md](AGENT_COORDINATION.md).

The numerical control-flow increment adds statements `if` (`condition`,
`then_body`, `else_body`) and `while` (`condition`, `body`), plus expressions
`compare` (`op`, `left`, `right`). Comparison `op` is exactly one of `==`, `!=`,
`<`, `<=`, `>`, `>=`. Conditions must type-check as `bool`. Node projections
expose condition/branch/body paths for revision-scoped edits. Reserved math and
cast identities are documented in [LANGUAGE.md](LANGUAGE.md); they are compiler
intrinsics rather than stored objects, so they do not require object read guards
and are omitted from stored-function call edges.

`calls`/`deps` and `callers`/`users` traverse forward/reverse call indexes, with
optional `depth` (1–1024) or `transitive:true`. Default depth is one. At this stage
dependency queries cover function call references, not future type, data, contract,
or optimization edges. Results are sorted and cycles terminate.

The first output line contains `schema`, resolved `snapshot`, `status`, `complete`,
page `records`, full-result `total`, `cursor`, and a full-result `fingerprint`.
Function queries also return `object_revision`. Limits are 256 records and 64 KiB
per response. A cursor is bound to the resolved snapshot and all request options;
continue with that snapshot, not a moving branch name. `complete` describes
pagination only: `status=unknown` is not a proof even when `complete=true`.

`effects` returns `partial` and identifies its stdout-only scope. `pure`, `layout`,
`alias`, `range`, and `uncertainty` report `unknown`. Unsupported evidence/profile/
optimization queries return `unsupported`, not invented data or an empty success.
Unknown operations/identities are errors. `capabilities` reports the available
operations and these limitations. Requests still load/check the complete pack;
bounded output does not yet mean incremental analysis or persistent indexing.

### Expression edits and query read preconditions

`nodes` paths are scoped to the returned object revision; they are **not** durable
node identities. Submit a typed replacement with the exact object revision:

```json
{
  "schema":"vibe.transaction.v0",
  "name":"update greeting",
  "branch":"main",
  "base_revision":"<snapshot from query>",
  "reads":[],
  "operations":[{
    "op":"replace_expression",
    "id":"@greeting.say",
    "expected_revision":"<object_revision from query>",
    "node":"body/0/value",
    "value":{"kind":"string","value":"Hello from a semantic edit!"}
  }]
}
```

This example targets the checked-in Hello World pack's print expression. Query
first: do not assume the same path denotes the same expression after another edit.
Replacement values use the same closed, kind-tagged expression schema as
`put_function`. Invalid paths, stale revisions, invalid types/units, and undeclared
call dependencies reject atomically. Old snapshots remain queryable/buildable.

A transaction can also include `query_reads`, an array of
`{"query": <original query without cursor>, "fingerprint": "<returned fingerprint>"}`.
The environment reruns the query against the current branch under the commit lock
and compares the complete result, not just the returned page. The request snapshot
records where the fact was observed; it does not make the commit recheck old data.
Thus a newly added caller invalidates a callers-query read, while unrelated history
does not. These guards supplement, not replace, mandatory direct object reads.
Unknown/unsupported results can detect status changes, but cannot establish a
scientific property. No extra persisted JSON files are created.

The store retains immutable object revisions and a commit DAG with named heads.
Historical revisions can be queried, checked, built, and compared. Object-level
three-way merges either create a validated two-parent commit or return bounded
identity conflicts without moving a branch. Git invokes the same semantics through
the configured `.vibepack` merge and text-conversion drivers. See
`VERSION_CONTROL.md`.

The old `show`, `inspect-json`, and `export-json` commands operate on bootstrap
text imports and are not the production workflow.

The target mutation vocabulary, beyond the implemented whole-function operations,
operates only inside a transaction:

```text
begin(name, base_root)
replace_body(id, typed_ast)
add_parameter(id, parameter, propagation=callers)
add_object(object)
add_edge(from, relation, to)
rename(id, display_name)          // identity does not change
supersede(old_id, new_id)
require(selector_or_id)
preserve(contract_or_query)
preview() / validate(level) / commit() / abort()
```

Typed AST nodes and object schemas are exchanged in memory, framed canonical CBOR,
or bounded JSON tool messages—not source fragments or temporary JSON files. JSON
messages are ephemeral protocol frames and are not generated as project files. The
LLM schema uses closed tagged unions, required fields, stable enum values, object
revision tokens, and explicit units/types. It avoids deeply nested generic AST
dumps and sends only fields requested by the agent. Each mutation carries the
expected object revision for optimistic concurrency. Diagnostics contain machine
codes, object IDs, semantic paths, expected/actual typed values, and suggested
valid operations. Bootstrap-import locations never become semantic identity.

## Designing for LLM coding

The agent protocol and environment object model are optimized together:

- one canonical semantic representation; no style or syntax alternatives;
- durable semantic IDs, so edits do not depend on unstable line numbers;
- small local graph projections instead of repository-sized context;
- typed edit operations; source regeneration is not an authoring operation;
- compiler diagnostics that state object, violated rule, expected value, actual
  value, and valid repair operations;
- transactions with declared preservation requirements and automatic validation;
- semantic diffs that summarize behavioral consequences before textual changes;
- idempotent operations and revision tokens, so retries cannot duplicate edits;
- examples and schemas generated from the compiler's actual object model.

JSON is valuable here only when it reduces ambiguity. Large syntax trees, tensor
data, traces, checkpoints, and compiler IR are referenced by identity/hash and
queried in slices; they are never pasted wholesale into an LLM context.

`context(id, budget, purpose)` returns a deterministic context capsule bounded by
bytes or tokens. It contains the object, component interface, direct graph
neighbors, and relevant obligations and evidence; deeper implementation remains a
reference. `expand(id, fields, budget)` is the normal way to cross that boundary.
Every included object reports its relevance path so an agent can detect irrelevant
or missing context.

Abbreviated transaction envelope:

```json
{
  "schema": "vibe.transaction.v0",
  "name": "add_atmospheric_drag",
  "base_revision": "sha256:...",
  "branch": "main",
  "reads": [
    {"id": "@orbit.propagate", "revision": "sha256:..."}
  ],
  "operations": [
    {
      "op": "put_function",
      "expected_revision": null,
      "object": {"id": "@orbit.drag_acceleration", "...": "typed fields"}
    }
  ]
}
```

Semantic diffs report objects/edges changed, API and effect deltas, unit/shape
status, validation results, performance deltas, invalidated artifacts, unsafe
reachability, and target compatibility. Debugging uses semantic diffs and typed
traces, never source diffs.

The API stores concise engineering rationale and linked evidence. It must never
request or persist private chain-of-thought. Human-facing LLM answers carry the
program revision and typed fact/evidence handles supporting every material claim;
unsupported interpretation is visibly labeled rather than presented as compiler
fact.

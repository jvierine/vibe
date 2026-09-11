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
vibec env inspect PROJECT @id  # bounded object plus root/object revisions
vibec env check PROJECT
vibec env graph PROJECT
vibec env build PROJECT
vibec env run PROJECT
```

`PROJECT` may be one portable `.vibepack` file or an environment directory. The
current mutation set is `put_function` and `delete_object`. Each operation has an
expected object revision, and the transaction declares revisions for all objects
it read. Disjoint writes based on the same old project root may merge; overlapping
or stale reads reject. Direct function dependencies must appear in the read set
unless changed in the same transaction. The environment locks only the short
compare, validate, and atomic commit section.

These hashes currently provide concurrency control, not version history. Built-in
version control requires retained immutable object revisions, a root revision DAG,
named branches, historical queries, and semantic three-way merge. Until that is
implemented, `.vibepack` is Git-compatible only as an opaque binary snapshot.

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

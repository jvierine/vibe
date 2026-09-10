# Semantic agent API

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
diff(base_root, new_root, semantic=true)
```

Core mutation operations operate only inside a transaction:

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
or bounded JSON tool messages—not source fragments or temporary JSON files. The
LLM schema uses closed tagged unions, required fields, stable enum values, object
revision tokens, and explicit units/types. It avoids deeply nested generic AST
dumps and sends only fields requested by the agent. Each mutation carries the
expected object revision for optimistic concurrency. Diagnostics contain machine
codes, object IDs, semantic paths, expected/actual typed values, and suggested
valid operations; line spans are only secondary presentation data.

## Designing for LLM coding

The agent interface and source syntax are optimized together:

- one canonical spelling and formatter; no style or syntax alternatives;
- durable semantic IDs, so edits do not depend on unstable line numbers;
- small local graph projections instead of repository-sized context;
- typed edit operations instead of unconstrained source regeneration;
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

Example transaction:

```json
{
  "name": "add_atmospheric_drag",
  "base_root": "sha256:...",
  "modify": ["@orbit.propagate"],
  "add": ["@atmosphere.density", "@orbit.drag_acceleration"],
  "preserve": ["@contract.public_api", "@property.vacuum_behavior"],
  "require": ["@test.orbit.*", "@property.energy_without_drag"]
}
```

Semantic diffs report objects/edges changed, API and effect deltas, unit/shape
status, validation results, performance deltas, invalidated artifacts, unsafe
reachability, and target compatibility. Text diffs remain attached for debugging.

The API stores concise engineering rationale and linked evidence. It must never
request or persist private chain-of-thought.

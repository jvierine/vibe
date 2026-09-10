# Semantic program object model

The canonical store is one compact SQLite database at `.vibe/project.db`, not one
file per object. Revisions and edges are append-only logical records; indexes and
cache rows may be rebuilt. Kind-specific payloads use schema-versioned canonical
CBOR blobs where normalized columns are not useful. A readable `.vibe` file is a
deterministic human serialization/import unit, never identity. Each object has:

```text
id            durable project-scoped identity (@physics.frequency)
kind          closed core kind plus versioned extension kind
revision      immutable content hash
schema        semantic schema version
payload       kind-specific typed fields
edges         typed references to other object identities/revisions
evidence      validation records and their tool/version/input hashes
provenance    source artifacts, transforms, compiler/build environment
rationale     concise decision, alternatives rejected, linked evidence
```

Core kinds are project, component, function, type, unit, array, tensor, dataset,
equation, symbolic expression, algorithm, intent, assumption, contract, test,
property, benchmark, figure, result, external library, data artifact, decision,
build, model, parameter, optimizer, training run, checkpoint, agent, prompt, tool,
capability, policy, trace, incident, and evaluation.

Edges are semantic: `calls`, `implements_equation`, `reads`, `writes`,
`derived_from`, `validated_by`, `assumes`, `uses_unit`, `renders`, `invalidates`,
and `supersedes`. Arbitrary labels are rejected unless registered by an extension
schema. Reverse indexes answer callers and dependents without reparsing source.

Names and locations are attributes; identity survives rename and movement. An
object deletion creates a tombstone. References may pin a revision for reproduced
results or follow the current revision for source development. Results always pin.

Derived compiler facts—types, units, shapes, effects, call edges, target
compatibility—are cached by `(object revision, compiler version, configuration)`.
They are reproducible and disposable, unlike author claims. Staleness propagates
over typed edges: changing an equation invalidates implementations for review;
changing a function invalidates dependent tests, results, and figures for rerun.

Transactions use a private overlay and declare `modify`, `add`, `remove`,
`preserve`, and `require`. Commit performs parse/type/unit/shape/effect checks,
runs required validation, detects concurrent revision changes, and atomically
publishes a new project root. A failure returns structured diagnostics; partial
semantic state is never visible.

Large arrays, checkpoints, plots, traces, and scientific results do not become
database blobs by default. They remain HDF5 or content-addressed artifacts; the
database stores typed metadata, checksums, provenance, and locations. SQLite WAL
and compaction are implementation details, and history retention is policy-driven.

JSON is never the persistent representation. It is appropriate for bounded,
schema-constrained LLM tool calls and explicit interchange exports. `export-json`
produces a requested projection on stdout; `graph` and `show` are human-readable
by default. The bootstrap does not yet implement SQLite/CBOR.

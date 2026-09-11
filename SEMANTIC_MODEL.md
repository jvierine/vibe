# Semantic program object model

The environment owns the canonical semantic graph in a transactional object
store. Agents author it only through typed operations. A small number of
version-controlled packfiles records canonical object revisions; it is never one
file per object and is not intended for manual editing. A rebuildable, gitignored
SQLite index at `.vibe/index.sqlite` accelerates queries. Generated `.vibe` text
is an inspection, interchange, and bootstrap-import representation. Derived
revisions, edges, facts, and cache rows may be rebuilt.
Packfiles and cached payloads use schema-versioned canonical CBOR where normalized columns are
not useful. Source location is never identity. Each object has:

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

Large-program containment is `workspace → project → package → component → object`.
Containment and exported-interface edges determine visibility, separate compilation,
context-capsule boundaries, and ownership. Cross-component dependencies are
acyclic; cycles are a compile-time architectural error.

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
index blobs. They remain HDF5 or content-addressed artifacts; canonical semantic
objects store their typed metadata, checksums, provenance, and locations. SQLite
WAL and compaction apply only to the disposable index. Artifact history retention
is an explicit project policy.

JSON is never the persistent representation. It is appropriate for bounded,
schema-constrained LLM tool calls and explicit interchange exports. `export-json`
produces a requested projection on stdout; `graph` and `show` produce semantic
views for the LLM interface. The bootstrap does not yet implement SQLite/CBOR.

# Large-program architecture

Neither a human nor an LLM reads a large program as text. Agents operate on
bounded semantic neighborhoods through the environment. Humans converse with an
LLM that grounds architecture, behavior, evidence, provenance, and validation
answers in those same versioned objects.

Large-program comprehension is a primary language constraint. Vibe may not rely
on an LLM reading an entire repository, humans remembering informal boundaries,
or whole-program compilation for ordinary edits.

Concurrent multi-agent development is equally fundamental. The concrete task,
reservation, contract-first scheduling, context-server, semantic integration,
and 100-worker acceptance model is specified in
[DESIGN.md section 27](DESIGN.md#27-massively-parallel-ai-software-engineering).
Its work DAG is distinct from the program call graph: stable interfaces enable
parallel implementation even when finished components depend on each other.

## Hierarchy and boundaries

The semantic hierarchy is fixed and intentionally shallow:

```text
workspace -> project -> package -> component -> semantic object
```

A workspace selects projects and dependency versions. A project produces one or
more deployable programs or libraries. A package is independently versioned and
compiled. A component is the architectural unit an agent can operate on and an
LLM can explain in one bounded context. Functions, types, datasets, equations, tests, and results
are semantic objects.

Packages and components declare explicit public interfaces. Everything else is
private. Cross-component references must use exported identities; ambient name
lookup across a boundary does not exist. Component dependencies form a directed
acyclic graph. Mutually dependent objects must share a component or communicate
through a smaller extracted interface. This keeps impact and compilation bounded.

Environment-owned semantic objects and packfiles are canonical; files are not
modules or identities. Storage records may move without API changes. A generated
package view reports exports, dependency constraints, capabilities, and release
policy.

## Semantic interfaces

An exported interface contains identity, type, units, shapes, layout constraints,
ownership, effects, capabilities, errors, contracts, invariants, precision policy,
target availability, stability, purpose, equations, and assumptions.

`vibec api-diff` classifies compatibility changes. A unit, shape, effect, error,
precision, capability, or contract change cannot hide behind an unchanged binary
signature. Packages compile against compact interface summaries. Implementation
body objects are loaded only when an agent changes or debugs them.

Separate compilation distinguishes interface and implementation hashes. A private
body change invalidates its component code and relevant validation, not every
dependent package. Link-time optimization is optional, not necessary for normal
performance. Dependency edges are typed, and red/green query invalidation stops
propagation whenever a recomputed semantic fingerprint is unchanged. See
`INCREMENTAL_COMPILATION.md`.

## Agent context capsules

The compiler constructs a deterministic context capsule containing:

```text
requested object and purpose
public component/package interface
typed body only when requested
direct dependencies and callers as summaries
contracts, assumptions, effects, units, shapes, precision
relevant tests, failures, recent changes, and provenance
open transaction obligations
```

Capsules are token/byte-budgeted graph projections, not generated prose guesses.
Every omitted region is represented by an interface and can be expanded by
identity. The API reports why each included object is relevant. Agents request
`expand`, `explain_dependency`, `callers`, `callees`, or `impact` instead of
searching all source. Revision hashes make unchanged capsules cacheable.

Compiler-generated summaries are facts-first: declared purpose; interface and data
edges; inferred effects, units, and shapes; and measured validation/performance.
An LLM may explain these facts but may not invent the structure.

## Changes at scale

Every transaction names its allowed package/component scope, expected revisions,
preserved interfaces/contracts, and required validation. Before commit, `impact`
computes affected objects, packages, artifacts, deployments, tests, and documents.
A cross-package edit produces separate semantic diffs for each package.

Large changes use graph operations—propagate a parameter, migrate a schema, split
a component—rather than thousands of unrelated text edits. Checkpoints permit
staged validation while the final published project root remains atomic.

## Production requirements

The package system provides locked dependencies, offline reproducible builds,
signed metadata, duplicate-version isolation where ABI-safe, and explicit native/
WASM/accelerator capabilities. Production runtime support requires typed errors
and configuration, cancellation, structured concurrency, resource limits, logs,
metrics, traces, health contracts, migrations, rolling-version compatibility,
crash diagnostics, and deployment manifests.

## Scale gates

Benchmark representative projects with at least 100,000 semantic objects, one
million dependency edges, 1,000 packages, and dependency depth 50. On pinned
development hardware:

- object context defaults to at most 32 KiB and never silently exceeds its budget;
- warm `show`, dependency, and caller queries complete under 50 ms p95;
- transitive impact over 100,000 objects completes under 500 ms p95;
- a private body edit recompiles only its affected closure and completes under one
  second p95 for a warm development build;
- a no-change build completes under 100 ms p95 and adding an unrelated package
  invalidates nothing;
- peak index memory stays below 1 GiB;
- rebuilding the derived index produces identical semantic facts;
- unrelated packages increase context for a fixed object by no more than 5%.

Agent benchmarks hide implementation outside the capsule and test architecture
questions, cross-component changes, regression localization, component splits,
and dependency upgrades. Success must stay stable as the workspace grows while
inspected bytes and tokens remain bounded.

# Incremental compilation

Avoidable recompilation is a correctness and architecture failure, not merely a
future optimization. Every compiler query is keyed by semantic identity, revision,
target, and only the configuration fields it actually consumes.

## Fingerprints

Each semantic object has independent fingerprints:

```text
source        canonical serialized object
interface     exported type, units, shapes, effects, contracts, capabilities
body          resolved typed implementation
hir           normalized high-level IR
codegen       lowered artifact for target/features/profile
evidence      validation inputs and results
documentation human-view dependencies
```

Changing prose must not invalidate machine code. Changing a private body must not
invalidate a caller's type check. Changing an exported unit or effect must
invalidate interface consumers even when the machine-level ABI is unchanged.
Hashes use canonical semantic content; file timestamps and source locations are
never correctness inputs.

## Typed dependency edges

Compiler queries record the facts they read while executing. Edges include:

```text
resolves_identity       name/visibility changes
uses_interface          type/unit/shape/effect/contract changes
calls_body              interpretation, AD, or cross-object optimization
embeds_codegen          inlining/specialization of another artifact
reads_data_schema       dataset schema changes
reads_value             constants and compile-time evaluation
validated_by            tests/properties/reference evidence
renders                 documents and figures
deploys                 runtime manifests and service compatibility
```

Invalidation follows edge kind, not every graph connection. New edge kinds must
declare their invalidation rule and appear in `vibec explain-rebuild`.

## Red/green invalidation

When an input changes, directly dependent queries become unknown (“red”). The
compiler recomputes them in topological order and compares their canonical output
fingerprints. If output is unchanged, the query becomes reusable (“green”) and
propagation stops. For example, editing a function body may rerun its type/unit
check; if its interface summary is unchanged, dependent components stay green.

The rebuild plan is computed before expensive work and is inspectable:

```text
vibec impact @physics.model
vibec build --explain
vibec explain-rebuild @component.inversion
```

Every scheduled action names the changed fingerprint and dependency edge that
caused it. Unexpected broad invalidation is testable as a compiler regression.

## Separate compilation policy

Components produce independently cached typed-interface, HIR, native/WASM, debug,
and validation artifacts. Package consumers depend on interface hashes. In the
development profile, cross-component inlining and whole-program optimization are
disabled by default; stable calling conventions preserve locality. Release builds
may embed downstream bodies, but each embedding creates an explicit
`embeds_codegen` edge. Linkers use incremental/reproducible linking where available.

Generic functions create explicit specialization objects keyed by generic body,
type/shape arguments, target, and FP policy. Only used specializations compile.
Shared specializations are cached across components when provenance and compiler
configuration match. Dynamic shapes are preferred when specialization has not
demonstrated a material performance benefit.

External packages expose compact signed interface summaries. Their implementation
changes do not affect consumers unless the interface or selected link artifact
changes. Build scripts and environment discovery are isolated query objects whose
declared inputs prevent ambient environment changes from invalidating everything.

## Scheduling and storage

The rebuild DAG executes independent nodes in parallel with bounded CPU, memory,
I/O, and accelerator resources. Interactive work prioritizes diagnostics and the
edited component; release code generation and deep validation may continue later.
Completed artifacts enter a content-addressed cache atomically and can be shared
between worktrees. Failed or cancelled computations never poison cache entries.

The gitignored SQLite index stores query keys, dependency edges, and small results.
Large HIR/codegen artifacts live in a bounded content-addressed cache. Both are
rebuildable from canonical source and locked dependencies. Cache garbage
collection respects active project roots and reproducibility retention policy.

## Acceptance gates

For the large synthetic project defined in `LARGE_PROGRAMS.md`:

- whitespace/format-only changes compile zero semantic objects;
- documentation-only changes compile zero code artifacts;
- private body changes recheck that object and affected validation, but compile at
  most its component unless an explicit body/codegen edge crosses the boundary;
- unchanged interface fingerprints cause zero dependent-package type checks;
- one public interface change invalidates exactly its transitive interface users;
- a target-specific flag invalidates only codegen queries that read that flag;
- warm no-change builds finish under 100 ms p95;
- the compiler can explain 100% of scheduled rebuild nodes with a dependency path;
- adding an unrelated package produces zero invalidations;
- incremental and clean builds produce byte-identical interfaces and semantically
  identical executable manifests.


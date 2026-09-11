# Vibe: compiler and scientific agent designed together

Status: architectural decisions, September 2026. This document incorporates the
agent-centric scientific-language brief and takes precedence over older design
documents where their direction differs. It is not a claim that the bootstrap
implements these facilities. Examples below are proposed projections/protocols
unless explicitly identified as executable today.

Store semantics once; provide representations optimized for the consumer.
Do not make an agent infer facts the compiler can determine. Make known facts
cheap to retrieve, but never confuse a cheap lookup with a cheap analysis.

Concurrent multi-agent development is a first-class requirement, not an optional
Git workflow. Conflict is a semantic concept, not a textual concept. Give each
agent the minimum sufficient semantic context. Section 27 specifies how the
environment must support tens to hundreds of agents without making each agent
understand or revalidate the entire project.

## 1. Architecture and boundaries

The authoritative program is editable, typed semantic IR, not source text and
not target-optimized IR. Stable identities connect definitions, contracts, and
scientific evidence. Lowered IR is a derived artifact. The layers are:

| Layer | Responsibility | Persistence |
| --- | --- | --- |
| Semantic store | Definitions, interfaces, assumptions, revision roots | Versioned CBOR |
| Query/compiler engine | Checking, dependency tracking, lowering, diagnostics | Disposable indexed cache |
| Execution/analysis services | Tests, profiles, error estimation, AD, symbolic work | Evidence linked to exact inputs |
| Agent interface | Bounded queries and atomic semantic transactions | No required JSON files |
| Human interface | Grounded explanations, equations, graphs, plots | Optional reports |

No database server is required to run a compiled numerical program. The semantic
database belongs to the development environment. A local SQLite index is a
rebuildable accelerator, not a second source of truth. HDF5 holds numerical
products; build caches, profiles, and datasets are not embedded in every program.

Keep the C backend for bootstrap verification. Introduce typed HIR lowering to
MLIR and LLVM incrementally, using numerical operations before bufferization so
shape and algebra are still visible. Do not make every edit wait for LLVM,
whole-program optimization, or a symbolic solver.

## 2. Minimal core

The core is functions, immutable values, explicit mutation, scalar arithmetic,
records/tagged unions, conditionals, loops, calls, buffers/views, and checked
contracts. No inheritance, implicit numeric coercions, ambient mutable globals,
unbounded compile-time metaprogramming, or user-defined operator precedence.
Generics specialize on explicitly bounded types and shape parameters.

Matrix multiplication, FFT, reductions, convolution, random generators, I/O,
plotting, and agent runtimes are versioned standard operations/libraries. Their
schemas describe types, effects, shape rules, and legal lowerings. They are not
separate bespoke language grammars. Library abstractions must also have a valid
fallback implementation or a clear unsupported-target diagnostic.

## 3. Compact mathematical projection

Text is a deterministic view, not an independently maintained program. Familiar
names and mathematical notation help agents; replacing them with opaque binary
tokens does not inherently reduce model errors or token usage.

```vibe
fn @physics.kinetic_energy(m: f64[kg], v: f64[m/s]) -> f64[J] {
  return 0.5f64 * m * v * v;
}
```

Retain one unambiguous spelling: typed literals, explicit conversions, braces,
and semicolons. The environment may accept an expression fragment within a typed
edit request, resolving all references against its snapshot and returning the
parsed replacement before commit. This does not authorize arbitrary source-file
patching. Humans can request a projection but never need to read code to validate
a program. Node handles are returned separately; agents need not repeat entire
functions to change one expression.

## 4. Types

Use `bool`, signed/unsigned integers with explicit widths, `f16`, `bf16`, `f32`,
`f64`, and optional `f128`. `complex32`, `complex64`, and `complex128` mean two
`f16`, `f32`, and `f64` components respectively. A target capability query reports
native, emulated, and unavailable representations. Never silently implement
`f128` as platform `long double` or silently widen an unsupported type.

Conversions specify rounding and overflow behavior. Integer arithmetic is
checked by default; wrapping and saturating operations have explicit identities.
Storage type, compute type, and reduction accumulator type are separate choices.
Infer local types, but make exported signatures explicit. Avoid general dependent
types: use a restricted shape/range constraint language with bounded analysis.

## 5. Units

Dimensions are canonical compile-time exponent vectors; unit scale conversion is
explicit at boundaries. Compatible scalar values use coherent internal units;
dimension metadata erases before native arithmetic. A unit annotation is not a
runtime wrapper. Rational exponents require dimensionally valid results.

Keep affine quantities such as Celsius temperatures separate from temperature
differences. Angles can carry semantic tags despite being dimensionless in SI.
Function interfaces, gradients, datasets, and plotting axes retain unit metadata.
Trigonometric operations require the appropriate dimensionless/angle inputs.
Unit correctness alone does not establish equation or physical-model correctness.

## 6. Arrays, shapes, memory, and parallelism

Separate owned `Buffer<T, shape, layout, space>` from borrowed
`View<T, shape, strides, access, space>`. Shapes combine constants and runtime
dimensions, e.g. `[batch, 1024]`. Layout includes strides, alignment, and permitted
padding; memory space distinguishes host and device. Rank is normally static.

Views preserve ownership/lifetime information and do not copy elements. A reshape
requiring data movement must request a copy. Bounds checks remain unless proven
redundant. Shape equalities unresolved statically become boundary guards; invalid
sizes must not become generated-C undefined behavior.

Read-only borrows may coexist. Mutable access is exclusive over an established
region; disjoint slices require a proof or a checked split operation. `noalias`
is a consequence of this discipline, not an unchecked optimization annotation.
FFI assertions live behind an unsafe boundary. Parallel loops require independence
or explicit reductions; reduction ordering and reproducibility are separate
contracts. Shape specialization has a cache/variant budget and dynamic fallback.

## 7. Precision and floating-point semantics

Default operations preserve specified types and evaluation order. Reassociation,
FMA contraction, approximate reciprocals, subnormal handling, and assumptions
about NaNs/infinities/signed zero are separately declared permissions. Do not hide
all numerical changes behind a single undocumented fast-math switch.

Bitwise reproducibility is a scoped mode with a specified target, backend, math
library, reduction schedule, and toolchain. Cross-device identical results are
not generally promised. Mixed precision is a transformation proposal with an
explicit accumulator type and evidence, never a silent inference from dtype alone.

An error requirement must name a metric, scale/units, input domain, and evidence
policy. `error < 1e-5` without those fields is incomplete. A universal bound may
authorize a transformation within its assumptions. Finite test measurements may
authorize only a user-selected empirical acceptance policy, not a universal claim.

## 8. Uncertainty, ranges, and numerical error

Keep input measurement uncertainty, model uncertainty, discretization error, and
floating-point error distinct. Evidence records method, assumptions, validity
domain, input revisions, tool version, and status: `declared`, `proven`,
`empirical`, `unknown`, or `unsupported`. A failed proof is not a proved failure.

Offer range analysis, interval arithmetic with validated outward rounding,
first-order covariance propagation, Monte Carlo sampling, and promoted-precision
cross-checks as separate services. Correlation matters: propagate covariance as
`J C J^T`, not independent standard deviations by default. Linearization needs
validity checks for nonlinear or discontinuous models. Intervals can overestimate
because of dependency; conditioning can dominate rounding error.

The existing FFT example measures disagreement against a promoted computation
and independent references. It does not establish a worst-case error bound or
account for uncertainty in the measured input. Query results must preserve that
distinction, including when an LLM summarizes them to a human.

## 9. Canonical semantic representation

An object has an immutable opaque identity, a mutable display name, a kind, and a
content revision. Renaming does not change references. Use local node identities
inside function regions for precise edits; do not make every arithmetic operation
an independently versioned global object.

Store authored definitions and claims once. Derive calls, users, types, and
analysis facts with provenance rather than allowing agents to edit those indexes.
Comments/prose are attached purpose, assumption, invariant, or evidence records;
generated summaries are marked as generated and link to the supporting facts.
Source locations and display formatting do not affect executable fingerprints.

Keep the core object taxonomy small: definition, interface, contract, evidence,
and artifact reference. Domain schemas extend records without making each kind
of model, plot, training run, or agent incident a compiler primitive.

## 10. Binary storage and Git

Specify a versioned deterministic CBOR profile, including map order, duplicate-key
rejection, limits, and schema migration. Typed floating literals store exact bit
patterns so negative zero and NaN payloads are not lost through numeric encoder
normalization. Hashes include schema/domain identifiers. CBOR alone does not
guarantee canonical bytes; the profile must do so and have cross-implementation
test vectors. See [RFC 8949](https://www.rfc-editor.org/rfc/rfc8949.html).

Retain one-file `.vibepack` export for small examples and exchange. For large
workspaces, use immutable bounded pack segments and a persistent revision tree,
not a rewrite of every historical object and full symbol map on every edit.
Segment packing/compaction is explicit and bounded to avoid gratuitous Git churn.
The exact segment policy requires measurements before a format migration.

Git remains responsible for distributed commits, branches, remotes, forks,
signatures, pull, and push. Semantic transactions are working-state checkpoints;
Git commits select published roots. Do not require a second independently moving
branch pointer for ordinary Git operations. Current in-pack branches remain a
supported bootstrap facility, with explicit root selection when exporting.

Merge semantic roots by stable identity against the common base. Disjoint edits
can merge; overlapping edits yield structured conflicts, followed by validation
of the entire affected closure. Syntactically disjoint changes can still conflict
through an interface or contract. Never resolve scientific disagreement with
last-writer-wins. Keep conflict sessions resumable and Git merges uncommitted
until resolution. Disposable indexes are not versioned; artifacts have explicit
retention and transport policies. A fresh clone must build without the old index.

## 11. Compiler query system

All requests bind to a snapshot and, when relevant, a target/build configuration.
Provide these query families through one engine:

| Query | Meaning |
| --- | --- |
| `definition`, `inspect`, `structure`, `summary` | Identity, interface, bounded body or hierarchy |
| `deps`, `users`, `affected` | Forward/reverse dependencies and change-specific impact |
| `calls`, `callers` | Direct call edges; indirect calls marked conservative/unknown |
| `type`, `units`, `shape`, `layout`, `range` | Checked representations and constraints |
| `effects`, `pure`, `alias A B` | Effect set, scoped purity, memory relation |
| `precision`, `uncertainty`, `numerics` | Policies, estimates, bounds, and their evidence |
| `tests`, `assumptions`, `invariants`, `provenance` | Scientific validation and input lineage |
| `generated`, `ir` | Bounded target-code/IR projection with origin mapping |
| `cost`, `why-cost`, `why-slow` | Static estimates or measured attribution, explicitly distinguished |
| `why-not-vectorized`, `why-allocation`, `decisions` | Recorded pass decisions and blockers |

`deps` defaults to direct semantic references; `transitive` computes closure and
`depth=2` limits traversal. Edge kinds distinguish calls, types, constants,
contracts, data, and optimization assumptions. `users` reverses selected edges.
`affected` additionally needs a proposed change or fingerprint category: editing
a description is not equivalent to changing an ABI. Cycles are handled explicitly.
Dynamic dataset lineage is recorded during execution, not invented from static
call edges. Purity excludes external effects but is not a termination proof.

## 12. LLM protocol

Use a versioned typed tool schema; compact deterministic text is the default
result. JSON is acceptable for small transient tool envelopes, not persistent
per-symbol files or huge default AST dumps. Expose capabilities and schemas first.

```text
query v=1 snapshot=S7 op=deps id=@orbit.energy depth=2 limit=32
snapshot=S7 status=complete edges=3
@orbit.energy call @math.norm
@orbit.energy constant @constants.mu
@math.norm call @math.sqrt
```

Sort records by stable identity and edge kind. No ASCII diagrams. Every response
reports snapshot, status, completeness, and a continuation cursor when truncated.
Pagination is snapshot-stable. Empty, unknown, unsupported, stale, and not-yet-run
are different outcomes. Large code, tensor values, profiles, or evidence require
explicit expansion or artifact handles.

Cheap facts come from indexes. Expensive analyses require an explicit request
and budget; return an analysis handle rather than silently blocking a simple
query. Diagnostics include stable code, object/node, expected/actual facts, and
candidate repairs. Batch related queries to avoid one tool round trip per fact.

## 13. Human inspection

Dependency, call, dataflow, unit, and uncertainty graphs consume the same snapshot
and edge records as agent queries. Group by package/component and expand on demand;
never render a million nodes at once. Selecting an edge exposes its provenance.

Human validation centers on equations, assumptions, units, representative outputs,
tests, uncertainty, and semantic before/after comparisons. An LLM explanation
links its assertions to exact facts/evidence; generated prose is not itself a
verified compiler fact. The optional source projection is an additional view.

## 14. Optimizer

Separate legality, profitability, transformation, and validation. Passes consume
typed facts and emit decision records containing rule/version, origin nodes,
required facts, selected/rejected alternatives, and resulting fingerprints.
Absence of a decision record must return unavailable, not a plausible explanation.

Start with constant folding under precise FP rules, dead-code elimination,
shape specialization, unit erasure, and allocation/alias analysis. Add fusion,
tiling, SIMD, parallel reductions, and library selection without discarding
high-level operations too early. Purity permits CSE only where exceptions and
observable behavior also allow it. Device transfers and synchronization count
in accelerator cost models.

MLIR provides a useful structured-linear-algebra lowering boundary; its
[Linalg dialect](https://mlir.llvm.org/docs/Dialects/Linalg/) and
[bufferization](https://mlir.llvm.org/docs/Bufferization/) are building blocks,
not a substitute for Vibe's ownership and numerical contracts. Keep expensive
autotuning opt-in and cache it by hardware, library, shapes, layouts, and policy.

## 15. BLAS, LAPACK, FFTW

Represent matrix multiplication and FFT explicitly so selection does not require
guessing an algorithm from arbitrary loops. `f32` matrix multiplication can lower
to SGEMM when dimensions, layouts, alias rules, ABI sizes, and FP policy permit.
OpenBLAS is a selectable BLAS/LAPACK provider, not a separate language feature.
Packing is an explicit recorded cost; strided inputs are not necessarily illegal.

LAPACK operations expose workspace, pivoting, status, and conditioning information.
Prefer solves to forming matrix inverses. Backend choice is pinned in build/run
provenance. No claim of zero overhead when allocation, packing, dispatch, or device
movement actually occurs.

FFT operations specify axes, direction, normalization, dtype, strides, batching,
and destructive/in-place permissions. Plans are runtime/cache resources, not
canonical program definitions. Planning has separate timing and synchronization;
FFTW documents the distinction between thread-safe execution and planning in its
[thread-safety guidance](https://ftp.fftw.org/doc/Thread-safety.html).

## 16. WASM, visualization, and scientific data

Target WASM for compute and a small explicit host ABI for files, browser events,
and rendering. Detect required SIMD/thread features; provide supported fallback
builds instead of assuming every deployment has identical capabilities. Feature
availability is tracked by [WebAssembly](https://webassembly.org/features/).
WebGL is a visualization target, not the universal numerical GPU backend.
Future device lowering may target native GPU APIs and browser WebGPU separately.

HDF5 and NetCDF readers, raw binary, mmap, and extensible readers are libraries
with typed dataset schemas and explicit I/O effects. CSV is supported interchange
when explicitly requested; HDF5 is the default numerical data product. Browser
I/O and memory limits need separate adapters; native mmap is not assumed there.

Plot operations consume typed arrays/units and produce vector SVG/PDF or an
interactive scene. Record input revisions and generating function/script in
provenance. Scientific reports show that provenance by default with a hide toggle.
Do not put plotting or file-format parsers into the numerical compiler core.

## 17. Automatic differentiation and AI workloads

Provide explicit `jvp`, `vjp`, and `grad` transformations over differentiable
typed operations before destructive lowering. Forward mode fits few input
directions; reverse mode fits scalar losses with many parameters. Generated
derivatives remain inspectable and inherit units: derivative units are output
units divided by input units.

Specify behavior at nondifferentiable points, mutation boundaries, control flow,
randomness, and custom foreign operations. Complex differentiation distinguishes
holomorphic derivatives from real-loss/Wirtinger conventions. Validate gradients
with independent finite differences where well-conditioned. Evaluate
[Enzyme](https://enzyme.mit.edu/) as a backend option, not a blanket guarantee
that arbitrary effectful programs are differentiable.

CNNs and transformers use tensor operations with layout, batch dimensions,
precision/accumulator policies, and explicit parameter/state buffers. Training
adds explicit RNG state, gradient accumulation, and checkpoint artifacts.
Attention and convolution retain semantic identities until backend selection.
Agent workflows are ordinary typed state machines with effectful model/tool calls;
record redacted input/output traces, model/config versions, budgets, and replay
fixtures. Replay distinguishes recorded responses from inherently nondeterministic
fresh model calls. No separate giant agent language is needed.

## 18. Symbolic mathematics

Use an optional exact symbolic algebra domain with explicit conversion to numeric
evaluation. Bounded simplification, differentiation, dimensional reasoning, and
selected equation solving are services; undecidable or expensive requests may
return unknown or budget exhausted. Record solver assumptions and certificates
where available. Symbolic real identities do not automatically authorize IEEE
floating-point rewrites: even `x - x` is not universally zero with NaNs/infinities.

## 19. FFI and escape hatches

Start with C ABI scalar/record functions and explicit array descriptors: pointer,
element type, rank, shape, byte strides, access, and lifetime. Erase descriptor
fields known statically at specialized call sites. Borrow compatible contiguous
or strided memory without copying; otherwise reject or request explicit packing.
Pin integer widths and complex ABI through platform probes rather than assuming
all C compilers use identical complex calling conventions.

Foreign declarations specify effects, ownership, failure behavior, alignment,
and thread safety. Unverified claims are unsafe obligations, not compiler proofs.
Target intrinsics and inline assembly require explicit target capabilities,
operands/clobbers, and a portability diagnostic or fallback. Ordinary numerical
programs should not need these escape hatches.

## 20. Incremental compilation and recomputation

Use demand-driven queries with separate interface, body, codegen, contract,
evidence, and presentation fingerprints. A caller depends on an interface unless
it consumed implementation facts through inlining/specialization. Changes
invalidate the recorded dependencies, then unchanged result fingerprints stop
propagation. Avoid eager materialization of every transitive closure.

Keys include compiler/schema version, target, numerical policy, dependency/library
versions, and relevant inputs. Persist query dependencies and verify cache hits.
Do not reload/recheck the whole workspace to answer a one-object question.
Use component compilation and per-function artifacts, with optional link-time
optimization as a separate expensive mode.

Incremental execution is a distinct facility: cache only pure computations with
complete input/data/configuration fingerprints. File paths alone do not identify
datasets. Nondeterminism and external effects require explicit replay policies.

## 21. Semantic editing and concurrent agents

Operations include inspect, replace-node, add-definition, rename, add-test,
change-precision, specialize, optimize, and validate. Optimization returns a
candidate/evidence diff rather than silently changing scientific requirements.
Stable node identities permit local edits, but the transaction unit is the
validated semantic object set, not an arbitrary byte range.

Each transaction specifies snapshot, expected object revisions, read-fact
fingerprints, writes, and required checks. Validate in an isolated candidate;
publish atomically with compare-and-swap after checking the read/write set.
Disjoint edits can commit concurrently. Reads of sets (such as all callers) also
need dependency/index generation checks to catch newly added members. A global
validation/publication lock must not cover lengthy compiler work.

On conflict, return changed identities and stale assumptions. Do not retry a
scientifically meaningful edit against changed evidence without revalidation.
Derived indexes publish with the root or are reconstructed for that root; readers
never observe mixed snapshots. Failed transactions leave the prior program intact.

## 22. Validation and self-checking

Provide tiers: structural/type/unit checks; shape/range/ownership/effect checks;
contracts and focused tests; numerical cross-checks; performance/reproducibility
checks. Compile-time discharge removes runtime guards only when justified.
Foreign boundaries and unresolved dynamic conditions retain checks.

Tests, properties, equations, and contracts reference definitions by identity.
Results reference the exact program, input dataset, backend, and seed. Changing
an input marks evidence stale. An assumption is never promoted to a proof merely
because a model or an agent wrote it. Unsupported analysis must stay visible.

Use property-based tests, independent implementations, high-precision references,
metamorphic relations, and physical invariants. None alone establishes all
correctness. Human acceptance reports state what was checked and what remains
unknown. Code execution and external tool effects remain permission-controlled.

## 23. Scientific examples

These are proposed compact projections, not runnable bootstrap syntax:

```text
orbital_energy(r: f64[m; 3], v: f64[m/s; 3], mu: f64[m^3/s^2])
  -> f64[m^2/s^2]
  0.5f64 * dot(v, v) - mu / norm(r)
  contract norm(r) > 0.0f64[m]

radar_psd(x: complex64[V; pulses, 1024], fs: f64[Hz])
  X = fft(x, axis=1, direction=forward, normalization=none)
  P = abs2(X) / (1024.0f32 * cast<f32>(fs))
  result units=V^2/Hz spectrum=two_sided window=rectangular

least_squares(A: f64[observations, parameters], b: f64[observations])
  result = linalg.least_squares(A, b, method=qr)
  validate residual_norm, rank, condition_estimate

cnn_loss(weights, images: f16[batch, channels, height, width], labels)
  features = conv2d(images, weights, accumulator=f32)
  loss = cross_entropy(classify(features), labels, accumulator=f32)
  gradients = grad(loss, wrt=weights)
```

The orbital example requires a positive radius and tests invariance under rotations.
The PSD convention requires positive sampling frequency; a window changes the
normalization and must be represented explicitly. Least squares exposes rank
deficiency rather than hiding solver status. Mixed-precision training uses
declared numerical policies and gradient tests, not a promise of bounded training
error. These programs can link results to HDF5 inputs and vector/interactive plots.

Executable examples today include [kinetic energy](examples/kinetic_energy.vibe),
[Hello World](examples/hello_world.vibepack), and the
[complex64 FFT and empirical error benchmark](examples/FFT.md).

## 24. Agent navigation in a large project

Proposed interaction, abbreviated for exposition:

```text
structure @radar depth=1
inspect @radar.calibrate fields=interface,assumptions,tests
deps @radar.calibrate depth=1
users @constants.receiver_gain
affected @constants.receiver_gain change=value
precision @radar.calibrate
uncertainty @radar.output
```

The environment returns bounded, snapshot-bound lists, not a source-tree dump.
An agent learns which calibration owns the gain, its units, the affected outputs,
and whether uncertainty is measured or assumed. It obtains the specific node and
revision, submits `replace-node` plus an updated calibration test, then requests
validation of the affected closure. The commit checks its earlier read facts,
including the users query. A second agent modifying an unrelated plot can commit
independently; a change to the gain's units forces revalidation. The human receives
the calibration change, affected products, and validation evidence, not raw IR.

## 25. Compiler-guided performance optimization

For a hypothetical matrix pipeline, query layout, aliasing, numerical policy,
and an actual profile before selecting a transformation:

```text
layout @pipeline.A
alias @pipeline.A @pipeline.C
decisions @pipeline.matmul
cost @pipeline profile=P17
```

A recorded rejection might state `blas-lowering: rejected; reason=strict-reduction-order`.
Changing layout cannot fix that blocker. The agent may propose a revised reduction
policy with evidence and required approval, or keep the existing semantics. If
BLAS is legal but packing dominates, try preserving contiguous layout upstream
and measure the complete pipeline, not just GEMM. No invented profile percentages
or SIMD width are returned in the absence of measurements/target analysis.

For the existing FFT, fixed size alone does not prove vectorization or optimality.
Compare the native radix-2 kernel against a batched FFT provider with matched
precision, allocation policy, normalization, and input data. Separate planning,
warmup, execution, transfers, and validation. Promoted-reference agreement is
empirical evidence, not permission to relax every rounding rule. The optimizer
should be able to explain the chosen implementation; that query infrastructure
does not exist yet in the bootstrap.

## 26. Avoided complexity and implementation order

Avoid binary data in model context, giant JSON ASTs, source scraping for known
facts, a global mutable database as the only portable program, unbounded generic
specialization, eager whole-program analysis, and automatic silent copies.
Do not invent a syntax for every scientific domain. Do not equate more metadata
with more correctness or promise GPUs without modeling transfers and ownership.

Measure agent tokens, repair rate, task success, query latency, rebuild scope,
and generated performance. Compact output is useful only if it remains precise.
Test scale with at least 100,000 objects, disjoint concurrent transactions,
interface-changing edits, clean clone/rebuild, crash recovery, and cold caches.
Publish measured p50/p95 results; targets are not existing performance guarantees.

| Stage | Deliverable / acceptance gate |
| --- | --- |
| Current bootstrap | Semantic CBOR transactions/history/Git integration; scalar/unit checks; rank-one arrays; C backend; FFT empirical comparison; v1 snapshot queries, revision-scoped expression edits, full-result query read guards |
| Next: trustworthy queries | Versioned capabilities, body/node projection, typed dependency indexes, stable opaque IDs, explicit unknown/evidence statuses; snapshot/pagination tests |
| Next: safe numerical memory | Extend implemented runtime indexed-access bounds with shapes, ownership/alias rules and effect ceilings; reject unsafe FFT buffer configurations |
| Next: scalable environment | Incremental checking/codegen, read-set concurrency, segmented storage; no unrelated rebuilds and bounded transaction costs |
| Then: fast kernels | Structured tensor operations, BLAS/FFT adapters, recorded lowering decisions, reproducible benchmarks |
| Then: analysis and targets | AD, bounded uncertainty/symbolic services, WASM and accelerator paths, with independent validation |

The bootstrap still loads/validates and rewrites a whole pack for transactions;
its symbolic names are not yet rename-independent identities. The first query
protocol increment is documented in [AGENT_API.md](AGENT_API.md); its call indexes
are request-local and its expression paths are revision-scoped. General ownership,
multidimensional shape checking, the full proposed query coverage, incremental query
engine, MLIR/LLVM pipeline, AD, rigorous error bounds, and GPU/WASM execution are
not implemented. A comprehensive design must not disguise those gaps.

## 27. Massively parallel AI software engineering

This section is normative design, not implemented command documentation. The
objective is higher throughput of validated software changes, not a larger agent
count. A Git branch per agent is neither the concurrency primitive nor a required
unit of work. Agents submit proposals against shared semantic snapshots; one
transactional publication authority per workspace establishes accepted roots.
Git still supplies distributed history and transport as specified in section 10.

### Semantic work units and stable identities

Functions, interfaces, types/data structures, modules, tests, specifications,
algorithms, and numerical kernels can be task targets. These remain definitions,
contracts, evidence, and attached records under the small core taxonomy—not new
language syntax for each agent role. Separate interface, implementation, tests,
and rationale so an implementation edit and an added test are not competing
whole-function replacements.

Give definitions and editable expressions opaque identities independent of names,
formatting, and projections. Preserve identity through renaming and permitted
module moves; recheck visibility and ownership when moving. A copied expression
gets a new identity, while a replacement explicitly preserves or retires the old
identity. Deleted identities remain tombstoned in retained history. Names resolve
at a snapshot; an ambiguous name never silently selects an object. Structural
paths currently returned by `nodes` are not sufficient for same-function merging.

### Task specification, reservation, and authority

Implemented bootstrap subset: [AGENT_COORDINATION.md](AGENT_COORDINATION.md)
provides local task records, advisory leases, filtered polling and explicitly
unverified revision-linked reports, outside the program pack. Authenticated
reservations, distributed delivery and scheduler behavior below remain targets.

A task is a versioned orchestration record referencing semantic targets and an
accepted contract revision. A compact projection might be:

```text
task #task:filter name=create_fft_filter base=S7
writes #fn:filter.implementation #test:filter_accuracy
reads #interface:signal #interface:fft #interface:window
preserves #interface:Filter revision=R4
requires types units shapes tests
requires error metric=relative_l2 tolerance=1e-6 domain=#domain:filter_inputs
requires evidence=empirical policy=#policy:filter_acceptance
budget context_bytes=32768 validation_seconds=120 candidates=4
```

The handles are illustrative. An error requirement must also define its reference
and zero-reference behavior; a policy requiring a rigorous bound cannot be
satisfied by this empirical example. Task write scope is enforced separately from
object revisions. Reading an implementation outside the context capsule requires
an authorized expansion; external tools and sensitive datasets have their own
capabilities. A declaration of ownership does not grant permission.

`reserve`, `renew`, `release`, and `transfer` operate on semantic scopes. Default
reservations are advisory coordination leases: they reduce duplicate effort but
never replace revision/contract checks. An explicit exclusive mode blocks other
publishers to that scope, with authenticated owner, expiry, cancellation, and a
monotonic fencing token so an expired worker cannot publish using an old lease.
Acquire multiple scopes atomically in identity order or fail with blockers;
never hold partial reservations while waiting indefinitely for the rest.

Durable task specifications, proposals, and accepted-change rationale are distinct
from transient leases/heartbeats. Runtime scheduler state is not rewritten into
Git on every heartbeat. A crashed worker's lease expires; its isolated proposal
remains available for recovery. Retried submissions use idempotency keys so a
timeout cannot create duplicate accepted changes.

### Contract-first scheduling

Keep the work DAG distinct from the program dependency graph. A call dependency
does not necessarily block development: callers can be implemented against an
accepted interface while a separate task implements its provider. Actual build,
integration, and execution still require a valid linked implementation. Interface
stubs are allowed in explicit speculative workspaces, never silently treated as
successful executable implementations.

The planner combines user objectives, declared deliverables, compiler dependencies,
contract readiness, and validation gates. The compiler can determine dependency
facts; it cannot prove that an arbitrary natural-language project plan is complete.
Require review of the proposed work DAG. Reject scheduling cycles with concrete
blockers; resolve them by defining shared interfaces or grouping a joint task.

For an interpolator contract, separate accepted inputs/outputs, shape/units,
sorted-input precondition, extrapolation policy, duplicate-abscissa handling, and
error metric/domain. Implementation, consumer, test, and benchmark tasks can then
run concurrently against that contract. Integration waits for their required
evidence; changing the contract invalidates affected proposals and evidence.

Example radar work plan, a deterministic list rather than a diagram:

```text
interfaces after=[] delivers=reader,fft,detector contracts
reader after=[interfaces] delivers=reader implementation
fft after=[interfaces] delivers=fft implementation
detector after=[interfaces] delivers=detector implementation
reader_tests after=[interfaces] delivers=reader test definitions
fft_tests after=[interfaces] delivers=fft test definitions
integration after=[reader,fft,detector,reader_tests,fft_tests] requires=executed tests
```

Writing a test and passing that test are different deliverables. Likewise, an
agent saying it has finished is not a validated completion event. Tasks progress
through proposed, ready, leased/running, validating, and accepted states, with
blocked, failed, and cancelled outcomes carrying reasons. Only accepted evidence
unblocks gates that require it. Changed assumptions can reopen an accepted task's
dependent obligations without erasing its historical completion record.

### Context server and structured communication

Construct a snapshot-bound capsule containing the task specification, target
interfaces, direct dependencies, required invariants/assumptions, relevant tests,
affected-user summaries, numerical/performance requirements, and open obligations.
Each entry has identity, revision, inclusion reason, and evidence status. Default
to interfaces; fetch implementation only when needed. Enforce token/byte budgets,
provide omission counts and expansion cursors, and never omit a required obligation
while labelling the capsule sufficient. If necessary, split the task or request
more context budget. The capsule's fact fingerprints become transaction reads.

Roles are orchestrator policies, not language constructs. An implementation agent,
numerical reviewer, optimizer, or security reviewer requests different fields
from the same snapshot and evidence graph. Free-form discussion can explain a
decision but never substitutes for an updated interface, contract, or validation
record. Treat attached rationale as untrusted content, not tool instructions.

Publish structured change events containing actor/task, before/after revisions,
changed facts, rationale, assumptions, and exact validation handles. Commit the
event durably with the accepted root; consumers resume by event cursor and
deduplicate delivery. Reverse dependency subscriptions identify impacted tasks
without broadcasting the whole program to every agent. A notification is a hint;
commit-time read checks remain authoritative if an event is delayed or missed.

### Speculation, validation, and atomic integration

Agents write private candidate roots over a shared immutable base. Candidates may
be incomplete and must not alter the last validated published root. Candidate
validation reads immutable objects outside the publication lock and produces
evidence keyed by program facts, datasets, toolchain, target, numerical policy,
and test definitions. A failed candidate cannot corrupt another agent's work.

At integration, compare written identities and all consumed facts—including
predicate/set reads such as callers or applicable tests—against the current root.
Rebase only demonstrably compatible deltas. Rerun invalidated checks on the merged
candidate, then atomically publish only if the checked inputs are still current.
Limit retries and return actionable conflicts rather than repeatedly doing large
validations under contention. Never hold the publication lock while compiling or
benchmarking. Global invariants require an explicit shared predicate/version guard;
disjoint write sets alone do not prevent conflicting combined changes.

Validation reuse must distinguish type/unit checks, shape/ownership checks,
code generation, test execution, benchmarks, and numerical analysis. Recompute
only consumers of changed fingerprints, but do not reuse an individual candidate's
test result as evidence for an untested combination. Benchmark evidence also
depends on the execution environment; unrelated simultaneous benchmarks can make
measurements incomparable even when their source changes are independent.

### Deterministic semantic conflict rules

| Concurrent changes | Integration rule |
| --- | --- |
| Different new functions | Merge if identities/exports and joint contracts remain compatible |
| Implementation and a new test | Merge definitions; run the test on the merged implementation |
| Two distinct tests | Merge unless identity or shared fixture/contract changes conflict |
| Separate expressions in one function | Merge only with durable node IDs, compatible binding/dataflow reads, and joint validation |
| Rename and body edit | Preserve underlying identity; validate name resolution and visibility |
| Interface change and an existing consumer | Recheck affected consumers and their contracts; do not guess compatibility |
| `velocity` changed to f32 and a new f64 requirement | Reject the combined state and identify the incompatible representation contract |
| Conflicting invariants or incompatible scientific assumptions | Require arbitration; never last-writer-wins |

Return conflict class, base/current/proposed facts, affected obligations, and
bounded resolution options. Preserve both proposals for inspection. Do not report
automatic safety when an analysis is unknown. Start with transactional three-way
semantic integration; defer same-function auto-merging until durable IDs and
dependency checks exist. CRDT-like combination is an option for genuinely
commutative annotations or append-only observations, not executable definitions,
precision policies, ownership, or scientific invariants. No CRDT is required for
the initial architecture.

### Alternative implementations and fair selection

Allow multiple candidates for one contract without competing for its accepted
implementation slot. A coordinator can request ten numerical-kernel alternatives,
each with an isolated root and resource budget. Reject candidates that fail type,
unit, shape, allocation, or numerical gates before ranking performance. Specify
whether an inverse is genuinely required rather than substituting a solve without
approval. Compare candidates on identical inputs, hardware, build policy, warmup,
and timing protocol; use independent/held-out numerical checks to reduce selection
overfitting. Store distributions and uncertainty, not only the fastest sample.

Selection yields an evidence-backed proposal, not an automatic contract relaxation.
Use a declared objective/tie policy; retain a Pareto set when runtime, memory, and
error trade off without an approved ordering. Revalidate the winner at integration
and mark losers superseded, not necessarily incorrect. Cap candidate count,
spend, simultaneous compiler jobs, memory, and accelerator occupancy.

### Scheduling and throughput queries

Expose these as versioned orchestration queries over the same semantic identities:

```text
ready tasks
blocked tasks
blockers #task:integration
dependents #task:fft
parallelizable project
parallelism project
critical-path project metric=estimated-duration
conflicts proposed_changes
independent changeA changeB
affected changeA
```

Responses use compact sorted records with snapshot, evidence status, pagination,
and reason codes. Distinguish dependency-ready tasks from reservation-free and
resource-schedulable tasks. Thirty-seven ready tasks is an upper bound on immediate
task-level concurrency, not a promise of 37 usable workers or a globally maximal
schedule. `independent` can be proven under recorded assumptions, false with a
conflict, or unknown. `affected` classifies invalidation by artifact/check type.

A critical path is computed over the declared work DAG and stated gate semantics.
Report unit-weight path length separately from a duration estimate, and identify
missing/uncertain durations. Resource-constrained makespan and token expenditure
are separate estimates. Break scheduling ties deterministically by explicit
priority and stable task identity; apply fairness/aging to avoid starving tasks.
Disconnected agents may create proposals, but cannot claim publication without
the authoritative workspace accepting their transaction.

### Scale acceptance and implementation dependencies

Run 1, 10, 32, and 100-worker workloads on pinned hardware against at least
100,000 semantic objects and one million dependency edges; also measure a
million-source-line-equivalent scientific application. Include disjoint writes,
hot interfaces, alternative kernels, task/worker crashes, lost replies, stale
leases, invalidation storms, and edits that violate a shared invariant.

Measure validated changes per minute, end-to-end completion time, p50/p95 query
and commit latency, lock duration, retry/conflict rate, duplicated validation,
cache hit rate, context tokens, cost, and scientific regression rate. Compare
against one worker and a per-agent Git-branch workflow on the same workload.
Do not promise linear speedup. Acceptance requires no lost accepted updates,
no stale evidence accepted as current, deterministic conflict classification,
recovery without manual pack surgery, and bounded context/unrelated invalidation.
Throughput targets must follow a published baseline, not invented measurements.

Implement in dependency order: durable definition/node IDs and separated contract/
test records; incremental fact indexes and validation dependencies; enforced task
scopes and recoverable candidate roots; short atomic publication with fact guards;
reservations and task/event services; bounded context capsules and DAG scheduling;
then automatic alternative selection and measured 100-worker scale tests.

Already implemented: atomic validated pack transactions, expected object revisions,
explicit object reads, full-result query read guards detecting newly added callers,
snapshot-bound queries, and disjoint-object integration. Not yet implemented:
reservations, task scopes/DAGs, contract enforcement, separate first-class tests,
durable expression IDs, same-function merge, short-lock publication, persistent
incremental indexes, automated context capsules, candidate selection, or verified
100-agent throughput. Whole-pack validation under one lock is the current scaling
limit; it is not the intended production concurrency architecture.

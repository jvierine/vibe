# Roadmap and objective gates

## Current: v0.1-bootstrap

Transactional environment API with a single CBOR program store, atomic commits,
object-level optimistic revision checks, declared read sets, safe disjoint-write
merging, bounded object queries, plus a hand-written bootstrap
import parser, stable function IDs, explicit numeric literals, SI dimensional
checking, rank-one arrays and loops, C17 emission, and native execution. Gate:
environment-authored Hello World runs, conflicting writes reject, examples run,
invalid units fail, and compiler unit/integration tests pass.

The first [DESIGN.md](DESIGN.md) implementation increment adds query protocol v1,
snapshot-bound pagination, flat semantic body projections, expression replacement,
and full-result query read guards (including newly added callers). Unsupported
analyses are explicitly reported. See [AGENT_API.md](AGENT_API.md). Persistent
indexes, durable node identities, ownership/bounds checks, and incremental checking
remain the next gates, not completed facilities.

## First-class concurrent development requirement

[DESIGN.md section 27](DESIGN.md#27-massively-parallel-ai-software-engineering)
defines the production concurrency model. It is not a branch-per-agent workflow.
Prioritize durable identities and separate interface/implementation/test records,
incremental fact/validation indexes, and candidate validation outside publication
locks. Build task scopes, leases, context capsules, event delivery, and work-DAG
scheduling on those foundations. Add speculative kernel selection only after
evidence validity and resource isolation are enforced. Gate throughput claims on
1/10/32/100-worker tests, including hot interfaces and crash recovery.

## Ordered milestones

1. Expand the transactional object API, now implemented for atomic function
   put/delete operations, revision tokens, bounded reads, commit-time validation,
   immutable history, branches, historical builds, object-level three-way merge,
   and Git diff/merge drivers. Add typed partial edits, context capsules, explicit
   conflict-resolution sessions, signed semantic commits, segmented packs,
   compaction, reflogs, garbage collection, and server validation hooks. Retain
   text only as bootstrap import and internal compiler output.
2. Freeze the object schemas and typed HIR; add diagnostics, scalar
   comparisons/control flow, inferred effects, explicit parametric types, closed
   tagged unions, static/dynamic shapes, bounds, and ownership rules. Benchmark
   tool schemas and context projections across LLMs for token cost, first-pass
   validity, repair count, and semantic error rate.
3. Implement workspace/project/package/component containment, explicit exports,
   acyclic component dependencies, interface hashes, semantic API compatibility,
   separate compilation, typed query dependencies, red/green invalidation, and
   explainable rebuild plans. Large-program boundaries precede backend expansion.
4. Replace the bootstrap single-file store with bounded append-only compressed
   pack segments, a small root/ref manifest, and the rebuildable SQLite semantic
   index. Support compact CBOR for bulk clients and constrained JSON tool messages
   for LLM agents. Never use per-object JSON files.
5. Add MLIR backend beside C; prove rank-1/2/3 explicit loops and fused array
   expressions, then retire C as a conformance oracle.
5. Add contracts, first-class tests/properties/references, `verify`, evidence
   manifests, and staleness propagation.
6. Build empirical precision tool using f32/f64/MPFR clones; explicitly label it
   empirical. Add interval backend later for eligible rigorous regions.
7. Add C ABI array descriptor and BLAS/LAPACK adapters; demonstrate zero-copy
   `matmul` and `solve` with selectable Accelerate/OpenBLAS.
8. Prototype Enzyme AD through loops; compare with central differences and
   symbolic derivatives, including derivative units.
9. Add small symbolic DAG (`simplify`, `diff`, simple solve, compile) without
   changing ordinary numeric execution.
10. Add HDF5 datasets with units/provenance and plots to PDF/SVG/PNG. HDF5 is the
   default scientific data product; CSV is import/export only, never an internal
   data product.
11. Compile pure kernels to WASM; add a browser demonstration and WebGPU plotting
    adapter with WebGL fallback.
12. Add tensor HIR, reverse-mode AD, StableHLO interchange, and CPU reference
    kernels; validate a small CNN and transformer forward/backward pass.
13. Add capability-based GPU placement, asynchronous runtime, fusion/autotuning,
    and one accelerator backend without vendor concepts entering model source.
14. Add typed LLM calls, tool capabilities, agent state machines, causal traces,
    deterministic stub replay, semantic breakpoints, and agent evaluation suites.
15. Validate a coding agent that transactionally creates/modifies another agent,
    with regression gates and a first-divergence debugger.

No milestone advances because a demo merely works. Each requires conformance,
performance, semantic-query, and regression evidence.

## Numerical performance benchmarks

On pinned hardware/compiler versions compare Vibe release, C17 `-O3`, and Fortran
`-O3`: SAXPY, reduction, stencil, 2D element expression, 3D seven-point stencil,
strided view, GEMM, FFT, and small ODE kernel. Use identical allocations/data,
warmups, at least 30 timed samples, confidence intervals, checksums, and assembly/
vectorization reports. Gates: generated scalar/loop kernels within 10% of the best
C/Fortran median, no unexpected allocations, compile time under 200 ms for a
100-object incremental edit and under 2 s cold for the benchmark suite. Library
kernels must add under 2% adapter overhead when zero-copy eligible.

AI suites add convolution, batched GEMM, normalization, embedding, attention,
transformer MLP, forward/backward training step, and quantized inference. Compare
against a pinned mature framework/backend on the same device. Gates separately
cover compilation/warmup, steady-state latency and throughput, peak memory,
transfer bytes, numerical/gradient agreement, and model quality. The initial
accelerator gate is within 15% of reference throughput and memory for supported
shapes with no model-source changes between CPU and accelerator.

## Agent-coding benchmarks

Use paired, blinded tasks against Python/Rust/C text-first repositories: propagate
a parameter; safely change f64 to f32; add units; trace dataset users; replace an
algorithm under a contract; optimize a loop within tolerance; locate an effect;
update an equation and figures; trace result provenance; repair a shape bug.
Pin models/prompts/tool budgets and rotate task order. Record input/output tokens,
source bytes and semantic objects inspected, wall time, tool calls, failed edits,
repair iterations, regressions, validation failures caught, and independent
correctness score. Success gate across at least 30 trials/task: median 30% less
context, 25% fewer repair iterations, no lower task success, and at least 50% more
seeded scientific-regression detection than the strongest baseline.

Agent-system benchmarks add tool selection, permission handling, long-task state,
prompt-injection resistance, trace diagnosis, deterministic replay, regression
bisection, and one agent safely modifying another. Record task success, harmful
effects prevented, invalid calls, first-divergence localization time, reproducible
replay rate, semantic regressions caught, tokens, latency, and cost.

## Five hardest problems

1. A useful ownership/alias model that enables zero-copy and optimization without
   Rust-level interaction cost.
2. Shape/unit/effect reasoning that is strong, decidable, diagnosable, and fast.
3. Honest numerical error analysis across branches, libraries, parallelism, and
   ill-conditioned models without overstating guarantees.
4. Preserving high-level array/linear-algebra/AD semantics while reliably reaching
   C/Fortran performance on diverse CPUs, WASM, and later GPUs.
5. Durable semantic identity and transactional evolution across schema changes,
   merges, generated objects, external data, and reproducible results.

Accelerator and agent targets intensify these problems: dynamic shapes, layouts,
fusion, mixed precision, asynchronous state, stochastic model calls, permissions,
privacy, and distributed execution must remain explainable without freezing Vibe
to one vendor or provider.

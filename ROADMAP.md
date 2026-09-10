# Roadmap and objective gates

## Current: v0.1-bootstrap

Hand-written Rust lexer/parser, stable function IDs, explicit numeric literals,
SI dimensional checking, rank-one arrays and loops, human-readable architecture,
optional JSON stdout export, C17 emission, and native execution. Gate: examples run, invalid units fail,
and compiler unit/integration tests pass.

## Ordered milestones

1. Freeze canonical grammar and typed HIR; add formatter, diagnostics, scalar
   comparisons/control flow, static/dynamic shapes, bounds and ownership rules.
2. Persist semantic objects and revisioned edges in one SQLite project database;
   implement transactional API, semantic diff, compact CBOR transport for bulk
   clients, and bounded schema-constrained JSON tools for LLM agents. Never use
   per-object JSON files.
3. Add MLIR backend beside C; prove rank-1/2/3 explicit loops and fused array
   expressions, then retire C as a conformance oracle.
4. Add contracts, first-class tests/properties/references, `verify`, evidence
   manifests, and staleness propagation.
5. Build empirical precision tool using f32/f64/MPFR clones; explicitly label it
   empirical. Add interval backend later for eligible rigorous regions.
6. Add C ABI array descriptor and BLAS/LAPACK adapters; demonstrate zero-copy
   `matmul` and `solve` with selectable Accelerate/OpenBLAS.
7. Prototype Enzyme AD through loops; compare with central differences and
   symbolic derivatives, including derivative units.
8. Add small symbolic DAG (`simplify`, `diff`, simple solve, compile) without
   changing ordinary numeric execution.
9. Add HDF5 datasets with units/provenance and plots to PDF/SVG/PNG. HDF5 is the
   default scientific data product; CSV is import/export only, never an internal
   data product.
10. Compile pure kernels to WASM; add a browser demonstration and WebGPU plotting
    adapter with WebGL fallback.
11. Add tensor HIR, reverse-mode AD, StableHLO interchange, and CPU reference
    kernels; validate a small CNN and transformer forward/backward pass.
12. Add capability-based GPU placement, asynchronous runtime, fusion/autotuning,
    and one accelerator backend without vendor concepts entering model source.
13. Add typed LLM calls, tool capabilities, agent state machines, causal traces,
    deterministic stub replay, semantic breakpoints, and agent evaluation suites.
14. Validate a coding agent that transactionally creates/modifies another agent,
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

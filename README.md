> "ASCII computer code is a terrible compromise between a human and compiler readable representations of a computer program"
>
> — author

# Vibe / `vibec`

[Browse human-readable programs](examples/PACKS.md) — generated views of the stored `.vibepack` functions.

Vibe is an experimental, compiled scientific language whose source of truth is a
typed semantic program graph. Programs are authored only through typed
interaction between an LLM and the programming environment. Humans inspect
generated intent, architecture, equations, validation, uncertainty, and results
by conversing with an environment-grounded LLM. Humans never read code, and
agents never patch source text.

This repository contains the design and a deliberately small bootstrap compiler.
The consolidated [scientific agent-first design](DESIGN.md) specifies the core,
compiler queries, numerical evidence, semantic editing, and implementation order.
It takes precedence over older design directions and explicitly separates planned
facilities from executable bootstrap behavior.

The prototype supports atomic semantic transactions into a compact CBOR store,
object-level optimistic concurrency, bounded object queries, explicit `f32`/`f64`
values, units, functions, rank-one arrays, indexed loops, static type/unit
checking, and native compilation through generated C.

```sh
cargo build
target/debug/vibec env inspect examples/hello_world.vibepack @app.main
target/debug/vibec env run examples/hello_world.vibepack
target/debug/vibec check examples/kinetic_energy.vibe
target/debug/vibec run examples/kinetic_energy.vibe -o /tmp/kinetic
target/debug/vibec graph examples/kinetic_energy.vibe
target/debug/vibec export-json examples/kinetic_energy.vibe
target/debug/vibec show examples/kinetic_energy.vibe
target/debug/vibec show examples/kinetic_energy.vibe @physics.kinetic_energy
target/debug/vibec inspect-json examples/kinetic_energy.vibe @physics.kinetic_energy
target/debug/vibec impact examples/kinetic_energy.vibe @physics.kinetic_energy
```

[`examples/hello_world.vibepack`](examples/hello_world.vibepack) is a complete,
small environment-authored program in one file. It contains no authored source
text. Transaction JSON travels through standard input and is not persisted.
Independent agents can commit disjoint objects from the same base revision; edits
to the same object require its latest revision.

`.vibepack` now contains immutable object revisions, a commit DAG, and branches.
`vibec` provides historical queries/builds, semantic diffs, three-way merge, and
Git diff/merge drivers. Git remains the distributed transport, fork, signing,
remote, pull, push, worktree, and CI layer. Run `vibec env git-configure` once per
clone. See [VERSION_CONTROL.md](VERSION_CONTROL.md).

The C backend is scaffolding, not the intended architecture. The stable boundary
is typed Vibe HIR; the planned production path is HIR → MLIR → LLVM/native or
WASM. See [COMPILER.md](COMPILER.md) and [ROADMAP.md](ROADMAP.md).

Status and limits:

- [Programming token-efficiency benchmark](benchmarks/programming/README.md):
  paired Vibe/Python/Rust tasks, correctness gates, real-usage telemetry and
  token-per-success accounting. Calibration is explicitly separate from agent runs.

- [Agent coordination](AGENT_COORDINATION.md): local versioned task records,
  advisory leases, scoped change feeds and revision-linked agent reports.
  [Metablate lessons](METABLATE_LESSONS.md) also introduce structured native ABI
  queries and a content-addressed native cache for the study host.

- [Metablate scientific example](examples/metablate/README.md): the SiO-study
  thermal-ablation backend, adaptive RK45, events and boundary searches run in
  compiled Vibe. Reproduces the local `ablatemm` studies with HDF5 results and
  Python plots; includes independent numerical validation and a LaTeX report.

- [Agent query API](AGENT_API.md#implemented-query-protocol-v1): snapshot-bound,
  paginated compiler facts and flat expression projections, typed expression
  replacement, and full-result query read guards for concurrent agents. Unknown
  analyses are explicit; persistent indexes and incremental checking remain planned.

- [FFT example](examples/FFT.md): 1024-point complex64 radix-2 FFT authored through
  semantic transactions, 1024-transform CPU benchmark against NumPy, and empirical
  numerical-error estimates using Vibe's promoted complex128 clone. Reproduce with
  `conda run -n base python scripts/benchmark_fft.py` after `cargo build`.

- Units use SI dimensions and compile away; `m`, `km`, `s`, `Hz`, `kHz`, `MHz`,
  `kg`, `g`, `A`, `K`, `mol`, `cd`, `J`, `kJ`, `N`, `Pa`, and `rad` exist.
- Arrays are rank-known but only rank-one literals/indexing are lowered today.
- String literals are currently supported only as direct arguments to `print`.
- Integer and floating-point representations never promote implicitly.
- Mutation is explicit. `io.stdout` effects are inferred through the call graph.
  Indexed accesses now have runtime bounds checks. Declared effect ceilings,
  general ownership, contracts, tests, and shapes are design
  commitments but only partially or not yet implemented.
- Generated C is an internal bootstrap artifact; it is not a user interface or
  canonical source.
- `print` and `len` are temporary bootstrap intrinsics, not permanent core syntax.

The design documents are normative for direction. Executable behavior and tests
are authoritative for what v0.1-bootstrap currently implements.

Vibe does not persist a forest of JSON files. The production environment owns a
compact semantic store and emits bounded JSON only as transient LLM tool
messages. The bootstrap `.vibe` serialization exists for compiler bring-up and
import; it is not a human or agent interface. Large
numerical products remain HDF5 or content-addressed artifacts. Small constrained
JSON messages are used at the LLM tool boundary, and explicit JSON export is
available on stdout. Human explanations use grounded Markdown, HTML, diagrams,
or conversation and never expose implementation serialization.

AI training/inference, future accelerator portability, and inspectable LLM agent
systems are first-class targets. See [AI_WORKLOADS.md](AI_WORKLOADS.md) and
[AGENT_SYSTEMS.md](AGENT_SYSTEMS.md). The bootstrap does not yet claim accelerator
or LLM-runtime support.

Large programs are an architectural requirement, with explicit packages,
components, semantic interfaces, separate compilation, bounded agent context, and
scale gates. See [LARGE_PROGRAMS.md](LARGE_PROGRAMS.md).

[Concurrent multi-agent development](DESIGN.md#27-massively-parallel-ai-software-engineering)
is a first-class requirement: semantic task scopes, contract-first parallel work,
minimal context, dependency-aware scheduling, and validated atomic integration.
Current revision/query guards are the foundation; reservations, scheduling, and
100-agent throughput remain design targets, not implemented capabilities.

The planned incremental compiler uses separate interface/body/codegen fingerprints and
typed dependency edges so unchanged interfaces stop invalidation. The design and
acceptance gates are in
[INCREMENTAL_COMPILATION.md](INCREMENTAL_COMPILATION.md).

# Vibe / `vibec`

Vibe is an experimental, compiled scientific language whose source of truth is a
typed semantic program graph. Programs are authored only through typed
interaction between an LLM and the programming environment. Humans inspect
generated intent, architecture, equations, validation, uncertainty, and results
by conversing with an environment-grounded LLM. Humans never read code, and
agents never patch source text.

This repository contains the design and a deliberately small bootstrap compiler.
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

- Units use SI dimensions and compile away; `m`, `km`, `s`, `Hz`, `kHz`, `MHz`,
  `kg`, `g`, `A`, `K`, `mol`, `cd`, `J`, `kJ`, `N`, `Pa`, and `rad` exist.
- Arrays are rank-known but only rank-one literals/indexing are lowered today.
- String literals are currently supported only as direct arguments to `print`.
- Integer and floating-point representations never promote implicitly.
- Mutation is explicit. `io.stdout` effects are inferred through the call graph.
  Bounds, declared effect ceilings, contracts, tests, and shapes are design
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

Incremental compilation uses separate interface/body/codegen fingerprints and
typed dependency edges so unchanged interfaces stop invalidation. The design and
acceptance gates are in
[INCREMENTAL_COMPILATION.md](INCREMENTAL_COMPILATION.md).

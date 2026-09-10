# Vibe / `vibec`

Vibe is an experimental, compiled scientific language whose primary source of
truth is a typed semantic program graph. Humans inspect intent, equations, data
flow, validation, uncertainty, and results; agents edit stable semantic objects.

This repository contains the design and a deliberately small bootstrap compiler.
The prototype supports explicit `f32`/`f64` values, units, functions, rank-one
arrays, indexed loops, static type/unit checking, human-readable semantic and
architecture views, optional JSON export, and native compilation through generated C.

```sh
cargo build
target/debug/vibec check examples/kinetic_energy.vibe
target/debug/vibec run examples/hello_world.vibe -o /tmp/hello-vibe
target/debug/vibec run examples/kinetic_energy.vibe -o /tmp/kinetic
target/debug/vibec graph examples/kinetic_energy.vibe
target/debug/vibec export-json examples/kinetic_energy.vibe
target/debug/vibec show examples/kinetic_energy.vibe
target/debug/vibec show examples/kinetic_energy.vibe @physics.kinetic_energy
target/debug/vibec impact examples/kinetic_energy.vibe @physics.kinetic_energy
```

The C backend is scaffolding, not the intended architecture. The stable boundary
is typed Vibe HIR; the planned production path is HIR → MLIR → LLVM/native or
WASM. See [COMPILER.md](COMPILER.md) and [ROADMAP.md](ROADMAP.md).

Status and limits:

- Units use SI dimensions and compile away; `m`, `km`, `s`, `Hz`, `kHz`, `MHz`,
  `kg`, `g`, `A`, `K`, `mol`, `cd`, `J`, `kJ`, `N`, `Pa`, and `rad` exist.
- Arrays are rank-known but only rank-one literals/indexing are lowered today.
- String literals are currently supported only as direct arguments to `print`.
- Integer and floating-point representations never promote implicitly.
- Mutation is explicit. Bounds, effects, contracts, tests, and shapes are design
  commitments but only partially or not yet implemented.
- Generated C is inspectable with `vibec emit-c`; it is not canonical source.

The design documents are normative for direction. Executable behavior and tests
are authoritative for what v0.1-bootstrap currently implements.

Vibe does not persist a forest of JSON files. Human-readable Vibe source is
canonical; one compact, rebuildable SQLite index accelerates graph queries. Large
numerical products remain HDF5 or content-addressed artifacts. Small constrained
JSON messages are used at the LLM tool boundary, and explicit JSON export is
available on stdout. Human exports use Vibe source, Markdown, HTML, or text.

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

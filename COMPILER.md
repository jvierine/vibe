# Compiler architecture

## Pipeline

```text
canonical source / semantic transaction
  -> lossless parse tree
  -> identity-resolved typed AST
  -> one SQLite semantic database + dependency index
  -> Vibe HIR (units, shapes, effects, ownership, FP mode)
  -> MLIR numerical pipeline
  -> LLVM IR
  -> native object/link OR wasm32 object/link
  -> build + provenance manifest
```

MLIR is the primary optimization infrastructure because Vibe needs to retain
array, loop, linear-algebra, shape, AD, and GPU meaning above LLVM. Initial
dialects are `func`, `arith`, `scf`, `cf`, `memref`, `tensor`, `linalg`, `math`,
and `vector`; use `affine` only for provably affine regions. A very small `vibe`
dialect carries source identities, physical dimensions, effects, contracts, FP
policy, uncertainty hooks, and provenance until each is checked or materialized.
Avoid a broad custom dialect that duplicates established MLIR operations.

Array expressions first become `linalg.generic`/tensor operations, enabling
fusion, then bufferize to explicit ownership-aware `memref`s. Explicit loops
become `scf.for` or `affine.for`. Passes perform canonicalization, fusion,
interchange, tiling, invariant motion, bounds elimination, vectorization, and
parallel lowering. BLAS-shaped ops remain `linalg.matmul` until cost-based library
or generated-kernel selection. EnzymeMLIR is evaluated after the HIR semantics and
mutation/alias model stabilize.

LLVM handles scalar optimization, final vector lowering, target features, object
emission, debug info, sanitizers, and native/WASM code generation. WASM rejects or
adapts effects through capability manifests; pure numerical HIR is unchanged.
WebGPU is a library/runtime target built on explicit GPU dialect lowering, not a
core-language browser dependency. Named convolution, contraction, reduction, and
attention operations remain high-level for fusion and kernel selection. A
versioned StableHLO import/export boundary provides accelerator-compiler
interoperability; Vibe HIR remains canonical so units, provenance, contracts,
effects, and precision are retained. Placement lowers capability-constrained
regions to CPU, MLIR GPU/SPIR-V/NVVM/ROCDL, WebGPU, or external adapters.

The current bootstrap replaces the MLIR/LLVM middle with readable C17 and invokes
`clang`. It validates the surface language and end-to-end unit erasure but is not
evidence for final performance.

Compiler stages exchange typed in-memory structures. Incremental facts, HIR,
dependency indexes, and evidence are stored as normalized rows or canonical CBOR
inside the project database. No stage communicates by emitting directories of
JSON. Large numerical artifacts remain in their native compact formats.

## Incrementality and latency

Parsing, name resolution, typing, facts, HIR, validation, and objects are cached by
semantic revision plus relevant configuration. Dependency fingerprints separate
interface from implementation so private body edits do not recheck unrelated
objects. Development builds use minimal passes and incremental native linking;
release builds enable whole-kernel optimization. External templates/generics are
monomorphized only when static specialization has measured value.

The query engine records typed fact dependencies while each computation runs and
uses red/green invalidation: a changed input dirties direct queries, recomputation
compares canonical outputs, and propagation stops when a result is unchanged.
Development builds prohibit implicit cross-component inlining; release inlining
adds explicit codegen dependency edges. `vibec build --explain` must account for
every rebuild. See `INCREMENTAL_COMPILATION.md`.

Package and component interfaces are compiled before implementations. Cross-boundary
resolution sees only compact exported semantic interfaces. Each component produces
a separately cacheable object/IR artifact keyed by interface hash, implementation
hash, target, and compiler configuration. Cyclic component dependencies are
rejected so build scheduling, invalidation, and agent context remain bounded.

Compile-time risks are excessive shape specialization, unbounded monomorphization,
whole-program effect/alias fixed points, large symbolic expressions, aggressive
fusion search, AD code expansion, MLIR pass pipelines repeated per target, and
validation accidentally placed on the critical path. Budgets, memoization,
canonical IR, explicit specialization, bounded optimization search, and separate
`check`/`verify` levels control them.

FFI uses versioned C-layout descriptors for arrays and datasets. Adapters declare
ownership, aliasing, alignment, layout, effects, unit assumptions, error behavior,
thread safety, and target availability. A zero-copy claim is compiler-verifiable
from the descriptor; conversions appear as graph objects and in profiles.

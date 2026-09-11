# Open questions

Questions stay here until an experiment or decision record resolves them.

- Can affine/refinement types express common shapes and ranges without dependent
  types or slow solver behavior? Benchmark Presburger reasoning on real kernels.
- What ownership rule gives agents one obvious edit while proving view lifetimes
  and noalias? Compare lexical regions, uniqueness with explicit views, and a
  restricted borrow model.
- Should angle carry a semantic tag despite SI dimensionlessness, and how should
  transcendental functions consume it?
- What is the stable ABI for dynamic-rank arrays, strings, errors, and datasets?
- Which MLIR version/distribution strategy gives reproducible builds without
  making `vibec` installation enormous?
- Can Enzyme differentiate the chosen bufferized loop subset predictably, or must
  AD occur on Vibe HIR before bufferization?
- How are external library error/rounding semantics represented when vendors do
  not specify them fully?
- Which same-object changes can safely field-merge beyond the implemented
  identity-level three-way merge, and which must remain explicit conflicts?
- Which symbolic simplifications are safe under `strict_fp` versus real algebra?
- What minimum browser capability/effect model supports WASM and WebGPU without
  leaking JavaScript concepts into the language?
- Which StableHLO subset is a durable interchange contract, and how are Vibe
  units/effects/provenance round-tripped without claiming they are native HLO
  semantics?
- Should tensor placement be inferred entirely by build plans, or can placement
  constraints appear in public contracts without fragmenting APIs?
- What deterministic RNG model composes across vectorization, rematerialization,
  batching, and distributed sharding?
- Where should reverse-mode AD occur relative to fusion, checkpoint planning,
  bufferization, and accelerator partitioning?
- Which parts of an LLM-agent run are replayable across provider/model changes,
  and what exact claim should “deterministic replay” make?
- How should trace redaction preserve causal debugging while proving that secrets
  and disallowed private model reasoning were never persisted?
- What typed protocol lets an agent modify another agent without allowing policy
  or capability escalation through generated semantic objects?
- Which object fields deserve normalized SQLite columns versus canonical CBOR,
  and what compaction policy bounds revision/evidence growth without weakening
  reproducibility?
- Which open-source license should cover the compiler, runtime, and standard
  library? Cargo publishing remains disabled until this is decided.

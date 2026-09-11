# Vibe vision

Working name: **Vibe**. Compiler: **`vibec`**.

Vibe tests one proposition: scientific software is best represented as a typed,
persistent computational model, not a directory of text files. Programs are
created exclusively by LLM agents interacting with the programming environment
through typed semantic operations. The environment owns the canonical model;
serialized source is an internal compiler, interchange, and bootstrap form,
never a human or agent authoring interface. Humans never read it.

## Product

A Vibe project is a typed semantic graph in an environment-owned,
content-addressed object store, plus a reproducible build description. Immutable
object revisions and commit trees form a semantic history DAG; Git provides
distributed transport, forks, signing, remotes, and CI while `vibec` provides
semantic diff and merge. Every significant object has a durable identity such as
`@physics.plasma_frequency`, even when its display name or serialized location
changes. Compiler facts connect functions, equations, datasets, tests, figures,
results, assumptions, decisions, and external artifacts.

Large-program structure is explicit: workspace → project → package → component →
object. Packages and components expose semantic interfaces; cross-component
dependencies form an acyclic graph. Separate compilation and agent context use
these boundaries, so comprehension and rebuild cost depend on the affected
neighborhood rather than total repository size.

The human interface is a conversation with an LLM grounded in environment
queries. It efficiently explains purpose; inputs and outputs; component/data-flow
graphs; equations and assumptions; precision, units, effects, and unsafe regions;
validation; uncertainty; runtime; and provenance. Every material claim links to
typed semantic facts and evidence at a specific program revision. A human can ask
for scenarios, counterexamples, alternative explanations, semantic diffs, or new
validation without seeing implementation serialization.

The only authoring interface is typed and transactional. An agent asks for one
object and its neighborhood, applies operations such as `replace_body` or
`add_parameter`, declares preserved behavior, and commits only if validation
passes. There is no text-patch authoring or human source-view path. Text can be
imported into the bootstrap compiler, but the environment immediately normalizes
it into semantic objects; production Vibe programs are changed only through
semantic operations.

For large programs, the compiler constructs bounded context capsules containing
the requested object, component interface, direct dependency summaries, and
relevant contracts, tests, and evidence. Omitted code is represented by typed
interfaces and expanded only by identity. Whole-repository context is never the
normal operating mode.

## Language

Vibe stays small: values, types, functions, bindings, explicit mutation, ordinary
control flow, arrays/views, calls, external functions, tests/properties,
contracts, effects, and explicit unsafe regions. There are no classes, implicit
exceptions, arbitrary macros, runtime reflection, inheritance, or hidden numeric
promotion. Canonical formatting removes stylistic choice.

Scientific semantics are not library conventions. Numeric representation, units,
array rank/layout, effects, floating-point mode, contracts, and provenance are
visible to the compiler. Ordinary nested loops must be a first-class fast path.
Array expressions remain high-level long enough to fuse without NumPy-style
temporaries. C ABI interoperation provides immediate access to BLAS, LAPACK,
FFTW, HDF5, MPI, and domain libraries.

## Trust model

Vibe reports distinct evidence instead of a single “correct” badge: syntax,
types, dimensions, shapes, memory, effects, contracts, tests, properties,
algorithm comparisons, roundoff, discretization error, input uncertainty, model
uncertainty, and provenance. A numerical statement always names its strength:
rigorous bound, method-supplied estimate, heuristic estimate, or empirical result.
Unknown remains unknown.

Compilation may erase checked metadata, but it must preserve a manifest that can
explain each result. Unsafe and foreign code explicitly lose guarantees and name
the tests or reviews that compensate.

## Execution

The production environment validates semantic transactions into typed HIR, then
lowers numerical kernels through MLIR to LLVM. Native CPU is first; WASM is
a coequal pure-computation target. CNNs, transformers, training, and inference use
ordinary typed tensor graphs whose device placement is portable by default. LLM
agents—including agents that build other agents—use typed tools, capabilities,
state machines, transactions, evaluations, and replayable semantic traces. GPU/
NPU backends, AD, and symbolic operations enter through explicit dialects and
capabilities, not core-language growth. Incremental compilation keys off
semantic-object identity and dependency hashes.

Success is measured, not asserted. Vibe numerical loops must match competent C
and Fortran within stated thresholds. Agent benchmark tasks must require less
context and fewer repair cycles while catching more regressions than text-first
baselines. Compile latency, reproducibility, and scientific audit time are release
criteria.

## Boundary

Vibe is not a notebook syntax, Python replacement, giant standard library,
theorem prover, or promise of automatic correctness. It is a narrow language and
program database that makes scientifically important facts explicit, derives what
it can, records evidence, and admits uncertainty where it cannot.

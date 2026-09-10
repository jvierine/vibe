# Validation model

Validation produces immutable evidence records; it never mutates a “valid” flag.
Every record identifies object revisions, inputs, compiler/runtime/library
versions, target, configuration, time, and result. A result is current only when
all pinned evidence inputs match.

`vibec check` is fast and deterministic: parse, identity/schema, names, types,
units, statically knowable shapes, ownership/alias rules, effects, target
capabilities, and statically decidable contract clauses. The bootstrap implements
parse, identity, representations, calls, mutation basics, and dimensions.

`vibec verify` builds and runs tests, example contracts, property samples,
sanitizers, and declared reference comparisons. `verify --deep` adds randomized
properties, high-precision and alternate-algorithm comparisons, AD versus finite
differences, precision/backend/optimization sweeps, and expensive sanitizers.

Evidence categories remain separate:

```text
syntax | type | unit | shape | memory | effect | contract
example test | randomized property | invariant | reference comparison
roundoff | discretization | measurement | model | provenance
```

A green unit check says nothing about model validity. A passing empirical
precision sweep is not a rigorous roundoff bound. User interfaces must display
these distinctions and unknown categories.

Contracts use `requires` and `ensures`. Static proofs may discharge simple range,
shape, and unit clauses; remaining clauses become runtime checks in development
profiles and generated test obligations. Unsafe/FFI objects declare guarantees
lost and compensating evidence. Reference implementations are ordinary semantic
objects with a `reference_for` edge, never hidden test code.

Release policy is a project object. It may require, for example, all static checks,
zero stale figures, named conservation properties, no new unsafe reachability,
performance within 5%, and empirical output agreement within a physical tolerance.

AI policies may additionally require forward/gradient/optimizer reference
agreement, deterministic replay at a declared level, no unplanned device transfers,
bounded accelerator memory, checkpoint compatibility, and pinned evaluation
metrics. Agent policies add capability non-escalation, prompt-injection fixtures,
effect ceilings, semantic-edit validity, and candidate-versus-baseline evaluation.
Statistical task/model quality remains separate from numerical agreement.

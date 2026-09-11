# Human understanding and validation interface

Humans never read implementation serialization. They interact with an LLM that
queries the programming environment and renders answers from the same versioned
semantic facts used by the compiler and dependency tracker. It is a conversational
explanation, experimentation, and audit interface, not an authoring surface.

An answer about program behavior must name the program revision and attach its
supporting object identities, contracts, tests, measurements, or uncertainty. The
environment distinguishes compiler-derived fact, measured evidence, model
interpretation, and unverified LLM explanation. A human can request a semantic
diff, trace, scenario run, counterexample search, sensitivity analysis, or an
independent validation transaction. This makes validation interactive without
turning source code into the evidence.

A document is a semantic object graph rendered into prose, equations,
tables, diagrams, executable results, tests, and figures. It references objects by
identity. Display order is editorial; dependency order comes from graph edges.

An equation links to implementations with `implements_equation`, assumptions with
`assumes`, tests/properties with `validated_by`, and figures/results with
`derived_from`. Changing any pinned dependency marks downstream material stale.
Rendering refuses to silently substitute old results; a draft may show them only
with an explicit stale banner.

The default project page answers:

1. purpose, scientific scope, inputs, and outputs;
2. component and data-flow graph;
3. equations, algorithms, assumptions, and rationale;
4. units, representations, FP modes, effects, unsafe/FFI regions, and targets;
5. validation by evidence category;
6. measurement/model/numerical uncertainty separately;
7. runtime, memory, dominant kernels, and regressions;
8. provenance and reproducible build identity.

The architecture view is derived from `component`, `calls`, `reads`, `writes`, and
`derived_from` edges. For example:

```text
@dataset.raw_voltage [V]
  -> @io.decode {filesystem.read}
  -> @calibration.apply [V]
  -> @spectrum.fft [V^2/Hz]
  -> @inversion.fit [m^-3, K]
  -> @result.plasma_parameters
       -> @artifact.results_hdf5
       -> @figure.electron_density
```

Selecting a node reveals purpose, equations, typed ports, shapes/layouts, effects,
precision, contracts, evidence, uncertainty, performance, provenance, callers,
and dependents. The prototype `vibec show` provides the first text projection.

Figures store data revisions, transforms, plotting specification, units, renderer,
and the script/function identity that generated them. Scientific document
templates default to showing script provenance and may hide it only through an
explicit publication toggle.

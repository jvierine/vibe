# Metablate / SiO scientific workload in Vibe

This is a working port of the **Kero-Szasz thermal-ablation backend used by the
local `ablatemm` studies**, not a replacement for every API/model in metablate.
Canonical executable program: `metablate.vibepack` (21 semantic functions).
The pack is authored by typed atomic environment operations, never by patching
`.vibe` files. The Python authoring builder constructs IR, not trajectory data.

## Reproduce

From the Vibe checkout, with the base conda environment containing the original
study's dependencies (`numpy`, `scipy`, `pymsis`, `h5py`, `xarray`, `matplotlib`):

```sh
cargo build
target/debug/vibec env check examples/metablate/metablate.vibepack
conda run -n base python scripts/run_metablate.py
conda run -n base python scripts/run_metablate.py --validate-only --sanitize --output target/metablate-sanitize
conda run -n base python scripts/report_metablate.py
```

Optional reconstruction uses `conda run -n base python scripts/author_metablate.py`.
That script reads SciPy's RK45 coefficient tables at authoring time and embeds
them as constants; there are no runtime SciPy solver callbacks in Vibe results.
The native host requires Clang and the built `target/debug/vibec`.
It now consumes structured `env abi` metadata and reuses content-addressed native
artifacts. See [toolchain lessons](../../METABLATE_LESSONS.md) and
[agent coordination](../../AGENT_COORDINATION.md) for the post-report increment.

Default original package/study paths are `/Users/jvi019/src/ablate` and
`/Users/jvi019/src/ablatemm`; the runner accepts `--metablate`, `--study-root`
and `--output` overrides. No original Python model or stored reference output
is modified. Data and eight PNG/SVG plot pairs are written to
`/Users/jvi019/src/ablatemm/output_vibe`. Report figures, timing evidence and the
compiled report are under `output/pdf/metablate`; the editable report source is
`reports/metablate/report.tex`. Plot provenance is on by default; the runner's
`--hide-provenance` and report's `\hidescriptprovenance` disable its display.

## Computation boundary

Vibe implements mass/thermal evolution, WGS84 conversions, fixed-atmosphere
log interpolation, geometry, adaptive Dormand-Prince 5(4), dense event locations,
temperature/mass-fraction boundary searches, tangency, local escape speed, and
radiation-pressure/Poynting-Robertson functions. Python supplies NRLMSIS 2.1 as
an 801-point input table, allocates contiguous buffers, invokes compiled entry
points, validates against the original RHS, writes HDF5 and plots results.
Compiler-generated C is disposable output, not handwritten physics.

Not ported: alternative metablate models (including Stulov and plasma models),
sputtering and general dynamic transfer-coefficient facilities, a native MSIS
implementation, or the package's complete public Python API. The study uses
fixed Lambda=Gamma=1 except explicitly labelled sensitivity sweeps.

## Evidence and numerical policy

The full runner passes 260 numerical assertions, executes 521 study trajectories
and 22 cached boundary searches (search-internal integrations are extra). It
compares nine trajectories with the original metablate RHS driven independently
by SciPy at matched component tolerances, including the three worst saved-output
discrepancies. The compiler now has 30 unit/integration tests plus its shell
smoke suite. Sanitizer validation is an additional run, not a formal proof.

The solver evolves mass fraction rather than tiny SI mass: rtol=1e-7;
component absolute tolerances `[1e-10, 1e-5, 1e-3, 1e-5]` for
`[fraction, velocity_m_s, signed_distance_m, temperature_K]`; max step 0.05 s.
Refinement uses rtol=1e-9, atols/100, max step 0.0125 s. Maximum selected peak
temperature refinement is 0.0795 K. This is empirical convergence evidence,
not a rigorous uncertainty bound and not an exhaustive parameter-space proof.
Peak temperature uses accepted-state samples, so the root tolerance (0.05 m/s)
does not imply that much physical accuracy.

The archived loose-tolerance studies differ by up to 25.8045 K in peak temperature
and 0.0102712 in retained mass fraction. Those differences are preserved in HDF5,
not hidden by an exact-match claim. Tight independent checks pass at the worst
cases; the older scalar absolute tolerance did not resolve microscopic mass.

The local Stefan-Boltzmann correction and zenith convention are preserved.
The legacy distance-based ground event is corrected to WGS84 altitude. Geometry
remains a fixed ray with scalar gravity, **not a gravity-curved orbit or a capture
test**. A 1350 K ceiling is a study criterion, not an implemented phase-transition
or melt-fraction model. Atmospheric/material uncertainty is not propagated.

## ABI schemas and failure behavior

All host arrays are contiguous `f64`. Configuration indices (28 values):
0 density, 1 heat capacity, 2 latent heat, 3 molecular mass, 4 CA, 5 CB,
6 Lambda, 7 Gamma, 8 emissivity, 9 shape, 10 initial T, 11 initial mass,
12 mass floor, 13 max step, 14 rtol, 15 diameter, 16 latitude degrees,
17 longitude degrees, 18 starting altitude, 19 reserved, 20 max duration,
21-23 reserved, 24-27 component absolute tolerances. Physical values are SI.

Trajectory columns: time, mass, velocity, signed position, temperature, altitude.
Summary: peak T, retained fraction, peak height, end time, end height, event code,
accepted/rejected steps, initial speed, initial mass. Event codes: 0 max duration,
1 downward 400 K cooling, 2 mass floor, 3 ground. Failure returns: -2 step floor,
-3 insufficient output capacity, -4 attempt limit. The host converts failures
to exceptions and checks input shape, finite values and mutable-buffer overlap.

The compiler checks SI interfaces for particle mass and thermal functions, and
every indexed access checks bounds. Mixed config/state arrays are explicitly
SI-normalized records, **not fully dimension-typed fields**. General ownership,
record schemas and stable FFI generation remain language work.

## Attribution

Physics and coordinates are adapted from Daniel Kastinen and Johan Kero's
metablate source in the specified checkout, with local `ablatemm` corrections.
The derived example and its host/authoring scripts use GPL-3.0-or-later; the full
upstream license is included in `LICENSE`. HDF5 provenance records original
source hashes, package commit, dirty compiler state, semantic revision, compiler
command, generated-C hash and dependency versions.

# Numerical model

Representations are explicit: signed/unsigned integers, `f16/f32/f64`, and
`complex64/complex128` (total complex storage bits). v0.1-bootstrap implements
`i32/i64/f32/f64/bool/complex64/complex128`. Complex64 holds two f32 components;
complex128 holds two f64 components. Mixed-representation arithmetic is an error unless an
operation names its input, accumulation, and output representations.

The first empirical precision experiment is implemented as
`vibec env emit-c PROGRAM --precision f64`: clone the semantic algorithm, widen
f32 to f64 and complex64 to complex128, recheck, and lower without changing the
stored program revision. Complex literal promotion is currently rejected explicitly;
complex inputs and coefficients can be supplied through array parameters.
The FFT example compares both executions on identical inputs, with wider twiddle
coefficients for the promoted run. The difference estimates arithmetic and
coefficient-rounding sensitivity for those inputs; it is not a rigorous bound,
does not certify all inputs, and does not quantify model uncertainty.
See `examples/FFT.md` for validation, benchmark scope, and reproduction.

`strict_fp` preserves the specified evaluation order and IEEE-observable behavior
within a documented target profile. `fast_math` separately enables reassociation,
FMA contraction, reciprocal/transcendental approximations, and non-finite-value
assumptions. Manifests report every function's mode. Reproducibility means same
profile, compiler, target features, libraries, threads, and inputs—not universal
bit identity.

Units are normalized into an integer exponent vector over SI base dimensions and
a rational/declared scale. Addition requires identical dimensions; multiplication
and division combine exponents. Values convert to coherent SI at boundaries and
units disappear from machine arithmetic. Angle is dimensionless but retains a
semantic tag in future revisions. Offset/logarithmic units require explicit
conversion functions and are not multiplicative units.

Arrays expose element representation, rank, shape symbols, strides, layout,
alignment, alias set, ownership, and mutability. Views are borrowed descriptors
and zero-copy by default. A copy is a semantic operation. Bounds checks are
removed only when range analysis proves safety. Array expressions lower to a
side-effect-free numerical IR where fusion, tiling, vectorization, and parallel
scheduling can occur before buffers are materialized.

Tensors use the same storage and numerical rules, adding named axes and immutable
value semantics suitable for graph optimization and AD. Model code states input,
accumulation, and output precision separately. Device placement belongs to a
build plan, so pure tensor functions stay portable until they opt into a target
intrinsic. Quantization calibration and approximation error are evidence, not
merely tensor attributes. See `AI_WORKLOADS.md`.

## Error and uncertainty architecture

Every result may have four separate channels: measurement uncertainty, model
uncertainty, numerical uncertainty, and validation evidence. None is silently
combined. Each estimate carries method, assumptions, coverage/meaning, units,
input revision, and classification.

1. **Rigorous bound**: directed-rounding interval/affine arithmetic or a proven
   method bound. Valid only when all operations and external calls participate.
2. **Method-supplied estimate**: truncation/residual/error estimator from an ODE,
   quadrature, interpolation, or iterative solver, with its mathematical scope.
3. **Heuristic estimate**: condition analysis or analytic roundoff model whose
   assumptions are not mechanically complete.
4. **Empirical estimate**: repeated execution across `f32`, `f64`, higher
   precision, stochastic rounding, alternate algorithms, or backends. It detects
   observed sensitivity and is never reported as a bound.

The practical first implementation is shadow evaluation. The compiler clones a
pure function at selected precisions, creates identical exactly representable
inputs where possible, and compares against an MPFR reference. It reports absolute
and relative discrepancy, ULPs, estimated reliable digits, branching divergence,
and operation-level sensitivity. AD supplies Jacobians for linear covariance
propagation `J Cov(x) J^T`; Monte Carlo handles nonlinear/non-Gaussian input
uncertainty. Correlation identities are explicit semantic objects.

Precision recommendations are requirements-driven: try the cheapest profile,
validate representative domains against the requested tolerance and reference,
and retain the evidence. “f32 sufficient” always names the sampled domain and is
an empirical conclusion unless a bound proves it.

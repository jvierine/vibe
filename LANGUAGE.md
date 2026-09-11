# Minimal v0.1 language

Vibe is authored through typed LLM-to-environment operations, not by editing this
surface syntax. The syntax below is the bootstrap compiler's deterministic
serialization and import form. It is optimized for compact, unambiguous internal
processing. The updated [design](DESIGN.md) also permits compact mathematical
projections and parsed expression fragments inside typed transactions; text is
never an independently maintained source of truth. Generated serialization contains no comments.
Rationale, equations, assumptions, and provenance are typed semantic objects.

## Decisions

- UTF-8 input, ASCII keywords, brace blocks, mandatory semicolons.
- One declaration per semantic object. Functions require durable `@` identities.
- Every numeric literal carries its representation: `1.0f32`, `1.0f64`, `1i32`.
- Units use the same bracket form on literals and types: `300.0f64[km]`,
  `f64[m/s]`. Whitespace unit suffixes deliberately do not exist because they make
  `value m / divisor` ambiguous.
- Values are immutable unless introduced with `mut`.
- Lexical shadowing is forbidden; every local name has one meaning within its
  visible scope, reducing accidental and agent-generated ambiguity.
- Arrays carry element representation and rank. Shapes are static where named,
  dynamic otherwise. v0.1-bootstrap lowers only rank one.
- No implicit numeric promotion or unit conversion. Unit scaling is explicit at
  literals, boundaries, and display; internal physical values use coherent SI.
- Function effects are inferred. An optional declaration is a checked upper bound;
  `effects none` is therefore an enforceable purity contract, not a default guess.
- `strict_fp` is the default. `fast_math` is an explicit function/build property.

Canonical core keywords are `const`, `type`, `fn`, `let`, `mut`, `if`, `for`,
`while`, `match`, `return`, `extern`, `unsafe`, `test`, and `property`. The
bootstrap implements `fn`, `let`, `mut`, `if`/`else`, `for`, `while`, and
`return`, plus `print`. Conditions must be `bool`; comparisons `==`, `!=`,
`<`, `<=`, `>`, `>=` require compatible real scalar representations and units.
Branch and loop bindings do not escape their scopes.

The numerical bootstrap also provides dimensionless `f64` intrinsics under
`@math.f64`: `sqrt`, `cbrt`, `exp`, `log`, `sin`, `cos`, `atan`, `atan2`, `abs`,
`pow`, `min`, `max`, and `isfinite` (the latter returns `bool`). Explicit
`@cast.i64_from_f64` rejects nonfinite/out-of-range values before truncation;
`@cast.f64_from_i64` is an explicit, potentially inexact conversion. These are
reserved compiler identities, not stored semantic objects. Unit normalization
must be explicit at a mathematical boundary. `min`/`max` use C `fmin`/`fmax`
NaN semantics. All indexed reads and writes check runtime bounds; failure
terminates the native process. This is not a general ownership or alias proof.
Native builds disable floating-point contraction and link the platform math
library; cross-platform bitwise reproducibility is not promised.

The bootstrap-only `print(...)` statement and unqualified `len(...)` builtin will
be replaced by stable standard identities such as `@io.println(...)` and
`@array.len(...)` once strings and package imports exist. They are not permanent
core syntax. This keeps effects, documentation, dependency tracking, and agent
queries uniform instead of hiding behavior in compiler magic.

LLM generation is a syntax acceptance criterion. Grammar alternatives, optional
punctuation, contextual meaning, implicit conversions, and far-away name lookup
all increase repair probability and are rejected unless measured agent benchmarks
show a larger benefit. Parser recovery and diagnostics are designed for one-edit
repair. Stable semantic IDs and typed edit APIs replace whole-file generation.
The serialized text remains compact for internal diagnostics and imports, but
arbitrary text-file patching is never the production editing path. A projection
may be requested by a human, but human understanding must not depend on reading it.

Large-program boundaries are semantic declarations, not inferred from directories.
The only hierarchy is workspace, project, package, component, object. Packages and
components have explicit exports; cross-component calls require exported stable
identities and component dependencies must be acyclic. Imports select interfaces,
not namespaces full of implementation details. See `LARGE_PROGRAMS.md`.

## Tiny grammar

```ebnf
program   = function* EOF ;
function  = "fn" identity "(" params? ")" "->" type block ;
params    = param ("," param)* ;
param     = name ":" type ;
type      = scalar unit? | "mut"? "Array" "<" scalar "," integer ">" unit?
          | "none" ;
unit      = "[" unit_expr "]" ;
block     = "{" statement* "}" ;
statement = "let" "mut"? name (":" type)? "=" expr ";"
          | place "=" expr ";"
          | "for" name "in" expr ".." expr block
          | "if" expr block ("else" block)?
          | "while" expr block
          | "return" expr? ";"
          | "print" "(" expr ("->" unit_expr)? ")" ";"
          | expr ";" ;
expr      = literal | name | identity | array | call | index | unary | binary ;
array     = "[" (expr ("," expr)*)? "]" ;
literal   = number unit? | string ;
```

Precedence is call/index, unary, multiplicative, additive, comparison. Comparison
chaining is rejected. There is one canonical
formatter; redundant parentheses are removed. Future syntax is added only when
it represents a new semantic concept unavailable through ordinary functions.

## Deliberate omissions

Vibe does not have classes, inheritance, exceptions, null, truthy values,
overloadable operators, user syntax extensions, textual macros, default numeric
types, implicit casts, implicit broadcasting, hidden allocation, ambient global
mutation, runtime reflection, or multiple loop syntaxes. Dynamic dispatch,
closures, higher-kinded types, implicit trait resolution, and async syntax are
postponed until a measured use case cannot be represented more simply.

Two formerly postponed features are required for production and become narrow
v0.2 commitments: explicit parametric types/functions, without traits or implicit
resolution, and closed tagged unions declared with `type`, enabling exhaustive
`match` and `Result<T,E>`. Async syntax remains postponed; structured concurrency
is first explored through typed library operations and effects.

## Structural declarations for v0.2

Large-program containment cannot depend on directories or name prefixes. The
canonical structural form is:

```vibe
package @radar.core version "1.0.0" {
    requires @vibe.linalg version "^1.2";
}

component @radar.processing in @radar.core {
    export @radar.calibrate;
    depends @radar.io;
}

fn @radar.calibrate in @radar.processing(
    input: Array<f32,2>[V]
) -> Array<f32,2>[V]
effects none {
    return input;
}
```

`package`, `component`, `in`, `export`, `depends`, `requires`, `version`, and
`effects` describe semantic structure rather than runtime behavior. An object is
contained by exactly one component. A component may reference another component
only through its exports and declared dependency. Version constraints resolve to
exact content hashes in the workspace lock.

## Ten representative programs

These are design examples; the first two are executable in `examples/`.

1. Unit-safe kinetic energy: `@physics.kinetic_energy` in
   `examples/kinetic_energy.vibe`.
2. Explicit array loop: `@numeric.scale2` in `examples/array_loop.vibe`.
3. Fused expression: `let y = sin(x) * exp(-x / tau);` lowers to one loop.
4. Matrix solve: `let x = solve(A, b, backend=build.blas);` retains a semantic
   `linear_solve` op until backend selection.
5. FFT: `let spectrum = fft(voltage);` records convention, normalization,
   precision, and selected FFT backend.
6. ODE: `let orbit = integrate_ode(@orbit.rhs, state0, times, rtol=1e-10f64);`
   returns value plus method-supplied error diagnostics.
7. AD: `let gradient = grad(@model.loss, wrt=@param.state);` differentiates typed
   HIR and checks derivative units.
8. Uncertain measurement: `let range = uncertain(100.0f64[km], sigma=0.2f64[km]);`
   propagates covariance separately from numerical error.
9. HDF5: `let ds = hdf5.read(@data.raw, schema=@schema.radar);` is an explicit
   filesystem effect with provenance.
10. Web view: a pure `@model.spectrum` kernel compiles unchanged for WASM while a
    separate UI component binds sliders and WebGPU plots.

CNNs and transformers use `Tensor<T,[named,shape,axes]>`, ordinary functions, AD,
and standard-library operations such as `convolution`, `matmul`, normalization,
and `attention`. Layers are not classes, and vendor APIs do not enter portable
model source. LLM agent orchestration likewise uses library-level typed state
machines and semantic objects, not new control-flow syntax. See `AI_WORKLOADS.md`
and `AGENT_SYSTEMS.md`.

## BLAS boundary

```vibe
extern @linalg.matmul(
    a: Matrix<f64,N,K>,
    b: Matrix<f64,K,M>
) -> Matrix<f64,N,M>
effects none;

let c = @linalg.matmul(a, b);
```

`Matrix` is a constrained `Array` view with element type, dimensions, strides,
layout, alignment, aliasing, and ownership. The HIR operation remains `matmul`.
At link planning, `--blas=accelerate|openblas|mkl` selects an adapter that passes
data pointers without copies when layout permits; otherwise the compiler emits an
explicit, visible layout conversion. Raw names such as `dgemm` live only in the
adapter.

# Minimal v0.1 language

## Decisions

- UTF-8 input, ASCII keywords, brace blocks, mandatory semicolons.
- One declaration per semantic object. Functions require durable `@` identities.
- Every numeric literal carries its representation: `1.0f32`, `1.0f64`, `1i32`.
- Units follow literals and appear in type brackets: `300.0f64 km`, `f64[m/s]`.
- Values are immutable unless introduced with `mut`.
- Arrays carry element representation and rank. Shapes are static where named,
  dynamic otherwise. v0.1-bootstrap lowers only rank one.
- No implicit numeric promotion or unit conversion. Unit scaling is explicit at
  literals, boundaries, and display; internal physical values use coherent SI.
- Function effects default to `none`; later effect inference may narrow declared
  upper bounds but never hide an effect.
- `strict_fp` is the default. `fast_math` is an explicit function/build property.

Canonical core keywords are `const`, `type`, `fn`, `let`, `mut`, `if`, `for`,
`while`, `match`, `return`, `extern`, `unsafe`, `test`, and `property`. The
bootstrap implements only `fn`, `let`, `mut`, `for`, and `return`, plus `print`.

LLM generation is a syntax acceptance criterion. Grammar alternatives, optional
punctuation, contextual meaning, implicit conversions, and far-away name lookup
all increase repair probability and are rejected unless measured agent benchmarks
show a larger benefit. Parser recovery and diagnostics are designed for one-edit
repair. Stable semantic IDs and typed edit APIs are preferred over asking a model
to reproduce whole source files; canonical Vibe text remains compact and usable
when direct generation is appropriate.

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
          | "return" expr? ";"
          | "print" "(" expr ("->" unit_expr)? ")" ";"
          | expr ";" ;
expr      = literal | name | identity | array | call | index | unary | binary ;
array     = "[" (expr ("," expr)*)? "]" ;
```

Precedence is call/index, unary, multiplicative, additive. There is one canonical
formatter; redundant parentheses are removed. Future syntax is added only when
it represents a new semantic concept unavailable through ordinary functions.

## Deliberate omissions

Vibe does not have classes, inheritance, exceptions, null, truthy values,
overloadable operators, user syntax extensions, textual macros, default numeric
types, implicit casts, implicit broadcasting, hidden allocation, ambient global
mutation, runtime reflection, or multiple loop syntaxes. Dynamic dispatch,
closures, generics, sum types, and async are postponed until a measured use case
cannot be represented more simply.

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
8. Uncertain measurement: `let range = uncertain(100.0f64 km, sigma=0.2f64 km);`
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

# AI workloads and accelerator portability

Training and inference are first-class numerical workloads. Vibe does not add a
second “AI language” or make neural networks opaque runtime objects. A model is a
typed semantic subgraph of ordinary functions, tensor operations, parameters,
state, losses, optimizers, datasets, and validation objects.

## Tensor model

`Tensor<T, Shape>` is the value-semantics, optimizer-facing form of `Array`; both
share storage descriptors and units. Tensor axes can have semantic names:

```vibe
x: Tensor<f32,[batch,channels,height,width]>
w: Tensor<f32,[out_channels,channels,kernel_h,kernel_w]>
```

Symbolic dimensions are checked across calls. Dynamic dimensions use explicit
symbols and runtime guards. Layout is metadata/constraint, not model meaning. The
compiler may choose packed layouts; any transpose, padding, copy, or host/device
transfer stays visible in the semantic graph and profiler.

Core tensor semantics stay small: elementwise maps, broadcasts with explicit axis
mapping, reductions, contractions, gather/scatter, convolution, pooling, random
generation with explicit RNG state, and shape transforms. Dense layers, CNNs,
normalization, embedding, scaled-dot-product attention, transformer blocks, and
optimizers are standard-library compositions, not language keywords. Named
operations remain intact in HIR long enough for fusion or specialized-kernel
selection instead of expanding prematurely into scalar loops.

```vibe
fn @model.block<N,D,H>(
    x: Tensor<bf16,[N,D]>,
    p: @model.Parameters<D,H>
) -> Tensor<bf16,[N,D]>
precision { input=bf16, accumulate=f32, output=bf16 }
effects none {
    let q = matmul(x, p.wq, accumulate=f32);
    let k = matmul(x, p.wk, accumulate=f32);
    let v = matmul(x, p.wv, accumulate=f32);
    let a = attention(q, k, v, causal=true, accumulate=f32);
    return layer_norm(x + matmul(a, p.wo, accumulate=f32));
}
```

`parameter`, `state`, `model`, `loss`, `optimizer`, `training_run`, `checkpoint`,
and `evaluation` are semantic object kinds. Reverse-mode AD creates a linked
derivative graph. Gradient accumulation, rematerialization, batching, sharding,
and optimizer state are explicit plans whose choices appear in provenance.

## Device contract

Pure tensor functions are device-polymorphic by default; device is deliberately
not baked into every tensor type. A build plan assigns semantic regions to
capabilities such as `cpu`, `gpu`, `webgpu`, or a future accelerator. Requirements
describe memory, representations, collectives, and determinism—not vendor names.
Backend plugins lower stable Vibe HIR through MLIR `linalg`/`tensor`/`vector`, a
StableHLO interchange boundary, MLIR `gpu` and target dialects, or versioned
external-kernel adapters. StableHLO is an interoperability format, not canonical
Vibe IR, so units, effects, contracts, and provenance remain intact.

StableHLO explicitly defines a portability layer between ML frameworks and ML
compilers, while MLIR Linalg supports fusion, tiling, vectorization, loop/library
lowering, and special instructions. [StableHLO specification](https://openxla.org/stablehlo/spec),
[MLIR Linalg dialect](https://mlir.llvm.org/docs/Dialects/Linalg/).

The runtime owns streams, command queues, asynchronous events, memory pools, and
collectives. Ordinary model code does not. Explicit device regions and low-level
intrinsics remain visible, target-restricted escape hatches.

## Precision and validation

AI types add `bf16`, `f8e4m3`, `f8e5m2`, and quantized integers, but storage,
compute, accumulation, and output representations are separate. Quantized values
carry scale, zero point, axis, calibration dataset, and error evidence. Hardware
formats require capability checks; vendor documentation warns that some low
precision formats are valid only for specialized matrix hardware.
[CUDA Programming Guide](https://docs.nvidia.com/cuda/cuda-programming-guide/).

Randomness is a value: algorithm, key/counter, data order, sharding, and
non-deterministic operations are recorded. `verify` compares forward outputs,
gradients, optimizer steps, and exported models with references. Deep verification
adds finite-difference gradients, precision sweeps, deterministic replay,
CPU/accelerator comparison, NaN/Inf tracing, distribution tests, and small-model
overfit tests. Model quality and numerical error remain separate evidence.


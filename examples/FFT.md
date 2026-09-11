# FFT1024 numerical validation and benchmark

`fft1024.vibepack` contains `@fft.forward1024`, authored by a typed environment
transaction. The algorithm is a 1024-point, complex64, radix-2 decimation-in-time
FFT: bit-reversed input gather followed by ten butterfly stages. Its convention
is `X[k] = sum(x[n] exp(-2 pi i k n / 1024))`, with no normalization and bins in
natural order. Complex64 uses two f32 components. There is no external FFT call.

The input and output arrays must each contain 1024 complex values, the twiddle
array must contain 512 forward coefficients, and all three buffers must be
disjoint. These are caller preconditions in the bootstrap: general shape/bounds
and alias contracts are not yet enforced by the compiler. The harness satisfies
them. Other lengths and inverse transforms are not implemented by this example.

Reproduce from the repository root:

```sh
cargo build
conda run -n base python scripts/benchmark_fft.py
```

If the pack is absent, the script authors it through `vibec env apply`. Otherwise
it benchmarks the existing pack. The compiler lowers its semantic algorithm to C;
`scripts/fft_harness.c` supplies inputs, precomputed twiddles, timers, and output
collection. It does not implement an FFT. Native compilation uses Clang `-O2
-ffp-contract=off` without fast-math. Timing excludes compilation, allocation,
twiddle preparation, validation, and checksum collection. It includes bit reversal
and every butterfly. Each batch transforms 1024 separate 1024-point arrays on one
CPU thread. Two warmup batches precede nine measured batches; all output values
feed a volatile checksum after each batch so results remain observable.

The Vibe precision experiment emits a second checked clone with `--precision
f64`. That clone uses complex128 arithmetic and double-precision twiddles, while
receiving exactly the same complex64 input values. Absolute output differences
and relative L2 differences are empirical precision estimates, not error bounds.
They include twiddle-coefficient rounding as well as butterfly arithmetic.

Five cases exercise impulse, DC, positive complex tone at bin 17, deterministic
complex random input, and the Nyquist alternating sequence. Both precisions are
checked against NumPy's double-precision FFT; six random-input bins are also
checked by direct double-precision DFT summation. The complex64 relative L2 gate
is 2e-6; the complex128 reference gate is 1e-10 absolute plus 1e-11 relative.
Near-zero bins use absolute errors because relative error there is ill-conditioned.

An Apple M4 Pro run measured a median **6.466 ms per 1024 FFTs**, or **6.314 us per
FFT**, approximately **158,367 FFT/s**. Nine batches ranged from 5.629 to 8.702 ms.
The maximum empirical absolute discrepancy was **3.357e-5**, and maximum relative
L2 discrepancy across the five cases was **1.114e-7**. Independent reference errors
agreed to the shown precision. These are local CPU measurements.

NumPy 2.1.3 was measured on exactly the same timed input batch, using complex64
inputs and complex64 outputs, with two warmups and nine trials per mode:

| Implementation | Median per 1024 transforms |
|---|---:|
| Vibe radix-2, reusable buffers | 6.466 ms |
| NumPy, 1024 individual calls | 8.423 ms |
| NumPy, batched `axis=1`, allocating output | 3.543 ms |
| NumPy, batched `axis=1`, reusable output | 2.815 ms |

NumPy's batched reusable-output path is about **2.30 times faster** than this first
Vibe kernel. Vibe is about 1.30 times faster than individual Python calls; that
advantage includes removal of Python dispatch and per-transform allocations.
Both kernels are single-precision. Vibe uses precomputed twiddles; NumPy's public
call includes any internal setup. Trials run sequentially on a live machine;
timing variation and cache state limit fine-grained conclusions. This example
does not yet implement optimized FFT codelets, SIMD-specific layouts, or parallel
batch execution.

`target/fft_benchmark.h5` records the inputs, complex64 outputs, double reference,
promoted outputs, per-element empirical errors, relative L2 errors, batch timings,
semantic revision, compiler flags/version, host, and producing scripts. It is a
regenerable data product; no JSON data files are produced. The reported semantic
revision is `sha256:9b21bb326cc903e685fc66403b2c584643294c2bfe1e1edba6d5bfc38e5e57dc`.

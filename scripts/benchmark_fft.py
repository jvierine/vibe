"""Author a Vibe FFT through transactions, compile it, validate and benchmark it.

Run with: conda run -n base python scripts/benchmark_fft.py
No Vibe source or transaction JSON files are written.
"""
import json
import pathlib
import subprocess
import tempfile
import time
import numpy as np
import h5py

ROOT = pathlib.Path(__file__).resolve().parents[1]
COMPILER = ROOT / "target/debug/vibec"
PACK = ROOT / "examples/fft1024.vibepack"


def num(value):
    return dict(kind="number", scalar="i64", value=str(value))


def var(name):
    return dict(kind="variable", name=name)


def binary(op, a, b):
    return dict(kind="binary", operator=op, left=a, right=b)


def index(name, i):
    return dict(kind="index", array=var(name), index=i)


def let(name, value, mutable=False):
    return dict(kind="let", name=name, value=value, mutable=mutable)


def assign(target, value):
    return dict(kind="assign", target=target, value=value)


def loop(name, end, body):
    return dict(kind="for", index=name, start=num(0), end=num(end), body=body)


def author():
    # Bit reversal followed by ten iterative decimation-in-time butterfly stages.
    low_bit = binary("-", var("remaining"), binary("*", binary("/", var("remaining"), num(2)), num(2)))
    gather = loop("i", 1024, [
        let("remaining", var("i"), True), let("reversed", num(0), True),
        loop("bit", 10, [
            assign(var("reversed"), binary("+", binary("*", var("reversed"), num(2)), low_bit)),
            assign(var("remaining"), binary("/", var("remaining"), num(2))),
        ]),
        assign(index("output", var("reversed")), index("input", var("i"))),
    ])
    body = [gather]
    for stage in range(1, 11):
        width = 1 << stage
        half = width // 2
        body.append(loop(f"block{stage}", 1024 // width, [
            loop(f"offset{stage}", half, [
                let(f"p{stage}", binary("+", binary("*", var(f"block{stage}"), num(width)), var(f"offset{stage}"))),
                let(f"q{stage}", binary("+", var(f"p{stage}"), num(half))),
                let(f"even{stage}", index("output", var(f"p{stage}"))),
                let(f"odd{stage}", binary("*", index("output", var(f"q{stage}")),
                    index("twiddle", binary("*", var(f"offset{stage}"), num(1024 // width))))),
                assign(index("output", var(f"p{stage}")), binary("+", var(f"even{stage}"), var(f"odd{stage}"))),
                assign(index("output", var(f"q{stage}")), binary("-", var(f"even{stage}"), var(f"odd{stage}"))),
            ])
        ]))
    body.append(dict(kind="return"))
    params = [dict(name=name, type=dict(kind="array", element="complex64", rank=1, mutable=mutable))
              for name, mutable in [("input", False), ("twiddle", False), ("output", True)]]
    request = dict(schema="vibe.transaction.v0", name="radix2_fft1024_complex64", branch="main",
                   base_revision=None, reads=[], operations=[dict(op="put_function", expected_revision=None,
                   object=dict(id="@fft.forward1024", parameters=params, result=dict(kind="none"), body=body))])
    subprocess.run([str(COMPILER), "env", "apply", str(PACK)], input=json.dumps(request), text=True, check=True)


def main():
    if not PACK.exists():
        author()
    source = subprocess.check_output([str(COMPILER), "env", "emit-c", str(PACK)], text=True)
    symbol = "vibe" + "".join(f"_{byte:02x}" for byte in b"@fft.forward1024")
    harness = (ROOT / "scripts/fft_harness.c").read_text().replace("VIBE_FFT_SYMBOL", symbol)
    with tempfile.TemporaryDirectory(prefix="vibe-fft-") as temporary:
        exe = pathlib.Path(temporary) / "benchmark"
        command = ["clang", "-x", "c", "-std=c17", "-O2", "-ffp-contract=off", "-Wall", "-Wextra", "-Werror", "-", "-lm", "-o", str(exe)]
        subprocess.run(command, input=source + harness, text=True, check=True)
        lines = subprocess.check_output([str(exe)], text=True).splitlines()
        wide_source = subprocess.check_output([str(COMPILER), "env", "emit-c", str(PACK), "--precision", "f64"], text=True)
        wide_harness = harness.replace("float", "double").replace("complex64", "complex128").replace("crealf", "creal").replace("cimagf", "cimag").replace("== 8", "== 16").replace("%.9g", "%.17g")
        wide_harness = wide_harness.replace("(double)creal(input[k])", "(float)creal(input[k])").replace("(double)cimag(input[k])", "(float)cimag(input[k])")
        subprocess.run(command, input=wide_source + wide_harness, text=True, check=True)
        wide_lines = subprocess.check_output([str(exe)], text=True).splitlines()
    samples = np.array([[float(v) for v in line.split()[1:]] for line in lines if line.startswith("V ")])
    inputs = (samples[:, 0] + 1j * samples[:, 1]).reshape(5, 1024).astype(np.complex64)
    outputs = (samples[:, 2] + 1j * samples[:, 3]).reshape(5, 1024).astype(np.complex64)
    expected = np.fft.fft(inputs.astype(np.complex128), axis=1)
    wide_samples = np.array([[float(v) for v in line.split()[1:]] for line in wide_lines if line.startswith("V ")])
    wide_outputs = (wide_samples[:, 2] + 1j * wide_samples[:, 3]).reshape(5, 1024)
    wide_inputs = (wide_samples[:, 0] + 1j * wide_samples[:, 1]).reshape(5, 1024)
    wide_reference = np.fft.fft(wide_inputs, axis=1)
    # Account for input-generation rounding by comparing the two kernels on the
    # exact same complex64 input (the harness explicitly rounds input data).
    np.testing.assert_array_equal(inputs, wide_inputs)
    estimate = np.abs(outputs - wide_outputs)
    empirical_l2 = np.linalg.norm(outputs - wide_outputs, axis=1) / np.linalg.norm(wide_outputs, axis=1)
    np.testing.assert_allclose(wide_outputs, wide_reference, rtol=1e-11, atol=1e-10)
    error = np.abs(outputs - expected)
    relative_l2 = np.linalg.norm(outputs - expected, axis=1) / np.linalg.norm(expected, axis=1)
    assert np.all(relative_l2 < 2e-6), relative_l2
    # Direct DFT at selected bins independently checks convention and bin ordering.
    bins = np.array([0, 1, 17, 511, 512, 1023])
    direct = np.exp(-2j * np.pi * bins[:, None] * np.arange(1024) / 1024) @ inputs[3].astype(np.complex128)
    np.testing.assert_allclose(outputs[3, bins], direct, rtol=2e-5, atol=2e-4)
    times = np.array([float(line.split()[1]) for line in lines if line.startswith("T ")])
    # Reproduce precisely the harness's timed input batch and RNG state.
    rng = 0x12345678
    for _ in range(5 * 1024 * 2):
        rng = (rng * 1664525 + 1013904223) & 0xffffffff
    batch = np.empty((1024, 1024), dtype=np.complex64)
    for k in range(batch.size):
        rng = (rng * 1664525 + 1013904223) & 0xffffffff
        batch.flat[k] = ((rng >> 8) / 8388608.0 - 1.0) + 0.25j
    numpy_individual, numpy_batched, numpy_reused = [], [], []
    reusable = np.empty_like(batch)
    for trial in range(-2, 9):
        start = time.perf_counter()
        individual = [np.fft.fft(row) for row in batch]
        elapsed = time.perf_counter() - start
        if trial >= 0:
            numpy_individual.append(elapsed)
        start = time.perf_counter()
        batched = np.fft.fft(batch, axis=1)
        elapsed = time.perf_counter() - start
        if trial >= 0:
            numpy_batched.append(elapsed)
        np.testing.assert_array_equal(np.stack(individual), batched)
        start = time.perf_counter()
        np.fft.fft(batch, axis=1, out=reusable)
        elapsed = time.perf_counter() - start
        if trial >= 0:
            numpy_reused.append(elapsed)
        np.testing.assert_array_equal(reusable, batched)
    revision = json.loads(subprocess.check_output([str(COMPILER), "env", "check", str(PACK)], text=True))["revision"]
    artifact = ROOT / "target/fft_benchmark.h5"
    with h5py.File(artifact, "w") as result:
        result["input"] = inputs
        result["output"] = outputs
        result["reference"] = expected
        result["seconds_per_1024_ffts"] = times
        result["numpy_individual_seconds"] = numpy_individual
        result["numpy_batched_seconds"] = numpy_batched
        result["numpy_reused_output_seconds"] = numpy_reused
        result.attrs["numpy_version"] = np.__version__
        result.attrs["numpy_output_dtype"] = str(batched.dtype)
        result["relative_l2_error"] = relative_l2
        result["promoted_output"] = wide_outputs
        result["empirical_absolute_error_estimate"] = estimate
        result["empirical_relative_l2_error_estimate"] = empirical_l2
        result.attrs["evidence_category"] = "empirical precision comparison; not a rigorous bound"
        result.attrs["semantic_revision"] = revision
        result.attrs["compiler_flags"] = " ".join(command[1:-2])
        result.attrs["clang"] = subprocess.check_output(["clang", "--version"], text=True)
        result.attrs["machine"] = subprocess.check_output(["uname", "-a"], text=True)
        result.attrs["script"] = "scripts/benchmark_fft.py; scripts/fft_harness.c"
    median = np.median(times)
    print(f"1024 x 1024-point complex64 FFTs: median {median * 1000:.3f} ms; range {times.min()*1000:.3f}-{times.max()*1000:.3f} ms")
    print(f"{median / 1024 * 1e6:.3f} us/FFT; {1024 / median:.0f} FFT/s")
    print(f"Maximum absolute error {error.max():.6g}; maximum relative L2 error {relative_l2.max():.6g}")
    print(f"Vibe precision estimate: max absolute {estimate.max():.6g}; max relative L2 {empirical_l2.max():.6g} (empirical, not a bound)")
    print(f"NumPy {np.__version__}, input complex64, output {batched.dtype}")
    for label, measurements in [("individual calls", numpy_individual), ("batched axis=1", numpy_batched), ("batched reusable output", numpy_reused)]:
        numpy_median = np.median(measurements)
        print(f"NumPy {label}: median {numpy_median*1000:.3f} ms, range {min(measurements)*1000:.3f}-{max(measurements)*1000:.3f} ms; NumPy/Vibe time ratio {numpy_median/median:.3f}")
    print(f"Evidence: {artifact}\nRevision: {revision}")


if __name__ == "__main__":
    main()

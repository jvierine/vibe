# Token efficiency: Vibe vs Python and Rust

This measures **LLM programming tokens**, not execution throughput, source length,
CBOR size, or an estimate obtained by tokenizing finished code. No language winner
is claimed until instrumented agent trials have actually been run.

## Primary measures

1. Correctness and budget-qualified task success rate.
2. Total input + output tokens per accepted task, **including tokens spent on
   failed attempts**. Missing telemetry makes this aggregate unknown, not zero.
3. Paired Python/Vibe and Rust/Vibe token ratios on tasks both agents solve,
   accompanied by failure counts. A ratio above one favors Vibe. Do not report
   this conditional statistic alone: it excludes tasks either language failed.

Input totals include the initial task, language documentation, tool schemas,
source/semantic queries, repeated context and tool results sent to the model.
Output totals include reasoning tokens if the provider includes them in output
usage, as well as generated tool calls/code. Count cached input tokens as input
tokens, without adding that subcategory twice. Dollar cost is a separate metric.

Time, check failures, tool calls and execution/load/build timings are secondary
diagnostics. Failed public checks are a **repair proxy**, not an exact count of
edits. First-public-pass token checkpoints are available when the driver sends
cumulative telemetry before checks; final task cost remains the primary total.

## Initial paired suite

| Task | Kind | Contract |
| --- | --- | --- |
| stable_norm | Implement | Overflow/underflow-resistant Euclidean norm |
| trapezoid | Debug | Repair a missing interval in nonuniform integration |
| weighted_mean | Modify | Extend an arithmetic mean to nonnegative weights |
| logsumexp | Implement | Stable logarithmic reduction over a wide range |

All candidates implement one immutable `float64 array -> float64` function.
The same inputs, expected results and tolerances apply to each language. Python
uses a list, Rust a slice, Vibe a semantic function `@bench.solve`. A Vibe IR
builder is provided; agents may instead transact directly. No authored `.vibe`
text or generated-C editing is allowed. Equivalent faulty behavior is seeded in
the debug/modify starters. Reference solutions are controller-only calibration
fixtures, never measured agent implementations.

This is deliberately a **small kernel-level baseline**. It does not cover large
repository navigation, tensor frameworks, multi-agent coordination or production
refactors. Standard libraries are allowed, including Python/Rust facilities not
yet in Vibe; therefore results measure the present language+toolchain ecosystem,
not syntax in isolation. No external packages or cross-language numerical calls.

## Prepare a counterbalanced experiment

From `/Users/jvi019/src/vibec`:

```sh
cargo build
conda run -n base python benchmarks/programming/bench.py plan \
  --output target/token-efficiency-plan.h5 --repetitions 6
```

This creates 72 planned trials: four tasks × three languages × six repetitions.
Each paired repetition shares its task instance seed. All six language orders
are used; task order is randomized. Use a fresh agent context for every trial,
with the same exact model revision, reasoning setting, tools, context limits,
sampling policy and token budget. Record these settings in the `--model` label
(for example, `model-version:reasoning-setting:tools-profile`). Do not carry
solutions or findings between trials. Use one common tokenizer/model for each
comparison; raw token counts from different models are not interchangeable.

Read plan rows from the HDF5 `trials` dataset. Prepare each row with its language,
task and seed, for example:

```sh
conda run -n base python benchmarks/programming/bench.py prepare \
  --task stable_norm --language vibe --seed 42 \
  --model YOUR_EXACT_MODEL_AND_SETTINGS --tokens 64000 --seconds 600
```

Preparation emits a fresh trial path containing `workspace/TASK.md`, starter
artifacts, public examples and a controller HDF5 record. Setup is outside the
development budget; all agent task-directed work must happen after start.

## Plug in an agent driver

The harness is model-provider independent. It does **not** fabricate model usage
from text length, silently call a paid model API, or claim desktop conversation
tokens are per-task usage. Supply a driver that launches a fresh agent and
captures its provider/framework's actual usage totals:

```sh
conda run -n base python benchmarks/programming/bench.py run TRIAL_PATH -- \
  /absolute/path/to/your-agent-driver
```

The driver runs in the candidate workspace and receives:

- `BENCH_TASK`: task Markdown path.
- `BENCH_TRIAL`: controller trial directory.
- `BENCH_HARNESS`: this CLI script.
- `BENCH_PYTHON`: the base-conda Python used by the harness.

The driver must supply the task to the agent, expose editing/query tools, allow
public checks, and report usage **before it exits**. Use the following command
with a compact stdin object; it writes HDF5, not a JSON file:

```sh
"$BENCH_PYTHON" "$BENCH_HARNESS" usage "$BENCH_TRIAL"
```

Input schema (numbers here are illustrative, not measured results):

```json
{"input_tokens":12000,"output_tokens":3000,"tool_calls":14,"source":"provider usage totals across every turn, exact model version","complete":true}
```

Snapshots are cumulative and monotonic; identical retries are idempotent, not
added again. Partial checkpoints are helpful for public-pass measurements. The
driver must ensure the final snapshot covers all calls, including its concluding
response. Set `complete:false` (the default) for partial checkpoints and
`complete:true` only for final totals. A partial snapshot after a timeout cannot
qualify a run for token comparisons. Final totals cannot subsequently be revised.
Counts are runner-reported, not independently audited by this
harness. Retain the provider transcript/usage source for audit outside model
context. Do not include API credentials in records.

The harness enforces a wall timeout and terminates the driver's process group.
The driver should also stop at the token budget; final scoring rejects a reported
token overrun. This is not a provider-side spending cap. Crashes/timeouts remain
failed attempts. If telemetry is missing, summary tables retain the attempt and
its functional outcome but mark token cost unknown; they do not silently drop it
from the success-rate denominator.

After exit, heldout evaluation runs once against the final candidate. A previous
public pass cannot certify a subsequently broken edit. Verifier time is recorded
separately from agent development time. `finish` is terminal; further checks or
usage writes are rejected. Manual `start`/`check`/`usage`/`finish` workflows are
available for exploration but excluded from managed comparison rankings.

## Evidence and reporting

```sh
conda run -n base python benchmarks/programming/bench.py summary target/programming-benchmark
```

Each `trial.h5` stores model/settings labels, budgets, input seeds, expected and
actual outputs, candidate hashes, checks, usage snapshots, success/failure, cache
observations, platform/compiler/harness hashes and timings. The report stratifies
aggregate costs by model and compares matched instances. Duplicate matching
trials are not arbitrarily paired. Raw HDF5 supports later uncertainty analysis;
six repetitions are a starting plan, not a power analysis. Increase repetitions
and task diversity before drawing general conclusions.

Vibe and Rust native checks reuse content-addressed artifacts after unchanged
edits. Python executes current source bytes, avoiding stale `.pyc` reuse after
same-size rapid edits. None of these runtime timings substitute for token cost.

## Validate the benchmark itself

```sh
conda run -n base python benchmarks/programming/bench.py self-test
conda run -n base python benchmarks/programming/test_bench.py
```

The self-test verifies that all 12 starters fail, and all 12 reference solutions
pass public and independent-seed heldout cases. These trials have the explicit
`calibration` cohort and no invented token counts. They are excluded from agent
comparisons.

## Isolation and validity limits

The local harness is **not a security sandbox**. Candidates execute native/Python
code; workspace boundaries and controller-only tests are procedural restrictions,
not access controls. Use a container/VM with read-only trusted controller/tools,
separate controller storage, no network, and resource limits for serious trials.
Do not run untrusted third-party drivers directly on a sensitive workstation.
Agents must not inspect `tasks.py` reference fixtures or heldout HDF5. Fresh
contexts and isolated mounts are necessary to avoid answer/test leakage.

Actual agent trials require a selected model and a usage-instrumented driver.
Until those run, the correct result is **token efficiency not yet measured**.

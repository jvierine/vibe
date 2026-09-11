# Changes driven by the metablate workload

The six-page report records the baseline port. This follow-up implements a
focused toolchain increment, not every proposed language feature.

| Observed friction | Implemented response | Remaining work |
| --- | --- | --- |
| No shared scoped work/finding record | `env coordinate`: tasks, blockers, leases, overlap warnings, filtered deltas and revision-linked agent evidence | Authenticated/distributed service, indexes and scheduling |
| Host parses printed types and duplicates symbol mangling | `env abi`: typed scalar/rank/mutability/SI dimensions and actual compiler symbol | Named unit-typed records, generated bindings, full shape/ownership contracts |
| Recompiling identical numerical C output | Content-addressed cache keyed by compiler binary/version, platform, flags, source and relevant environment | Function-level incremental checking/lowering/linking |
| Builds overwrite a library loaded by another worker | Immutable key-specific paths and per-key compile locking | Garbage collection and authenticated remote artifacts |
| Reports confused with compiler truth or reused after changes | Explicit unverified evidence, pinned revisions and freshness annotations | Compiler-owned test execution and dependency-sensitive evidence invalidation |

## Native build cache

`scripts/native_build.py` caches standalone compiler-emitted C in
`target/metablate/cache/KEY/`, used by the metablate host. Successful artifacts
are atomically installed and SHA256-checked before reuse. Corruption causes
rebuilding; concurrent identical requests compile once. Sanitizer mode gets a
different key. No JSON manifests are written.

This skips Clang compilation/linking, not semantic loading/checking/emission or
host binding. It is not an arbitrary-C dependency scanner: its contract is
standalone compiler output with platform standard headers. Manually modified
SDK headers under unchanged toolchain identities require cache invalidation.
Content addressing is not signature verification or remote-cache authentication.

## Structured host ABI

```sh
target/debug/vibec env abi examples/metablate/metablate.vibepack @metablate.integrate
```

`vibe.abi.v1` identifies program/object revisions, native symbol, parameter names,
scalar representations, array rank/mutability, SI dimensions, effects and result.
Rank-one arrays use a host C pointer followed by `size_t`. Extents remain dynamic;
the host must enforce shape/alias preconditions. Complex scalars follow platform
C complex ABI and are not automatically supported by the example's ctypes host.

The host now consumes these fields rather than parsing `Array<f64,1>` strings.
Named dimension-typed records remain a priority; positional config arrays are
not presented as a substitute.

## Regression commands

```sh
bash scripts/test.sh
conda run -n base python scripts/test_native_build.py
conda run -n base python scripts/run_metablate.py --output target/metablate-toolchain-regression
```

A separate output directory preserves the original report's evidence and plots.
The equations and semantic program revision are unchanged. The full regression
still executes 521 trajectories, 22 boundary searches and 260 numerical checks.
Python supplies atmosphere data, hosting, independent references, HDF5 and plots,
not delivered ODE computation.

Next: dimension-typed records/generated bindings, structured solver failures and
validation evidence, then measured incremental compilation and concurrent-agent
throughput. CPU success does not prove GPU portability or reduced LLM repair and
token costs; those require separate experiments.

# hello_world — readable program

Generated from [hello_world.vibepack](hello_world.vibepack). **Do not edit this companion by hand.**

This is a compiler-derived pseudocode view of the stored program, not an executable source file or a correctness certificate. Numeric types, declared units, mutation and branch structure are retained. Unmarked integers use i64; decimals and short math calls use f64. Other literal types are shown. `range(start, end)` excludes the end. Long strings may be explicitly omitted by the compiler query. Internal revisions and hashes are intentionally not displayed.

Build the compiler with `cargo build`, then regenerate from the repository root: `conda run -n base python scripts/render_packs.py`.

## Functions

| Function | Calls |
|---|---|
| [@app.main](#function-1) | [@greeting.say](#function-2) |
| [@greeting.say](#function-2) | None |

<a id="function-1"></a>
### @app.main

Called by: None. Calls: [@greeting.say](#function-2).

<details>
<summary>View implementation</summary>

```text
function @app.main() -> i32:
    @greeting.say()
    return 0:i32
```

</details>

<a id="function-2"></a>
### @greeting.say

Called by: [@app.main](#function-1). Calls: None.

<details>
<summary>View implementation</summary>

```text
function @greeting.say() -> none:
    print "Hello, world!"
    return
```

</details>

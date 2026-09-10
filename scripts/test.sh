#!/bin/sh
set -eu

root=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
compiler="$root/target/debug/vibec"
tmp_dir=$(mktemp -d)
trap 'rm -rf "$tmp_dir"' EXIT

cargo build --manifest-path "$root/Cargo.toml"
cargo test --manifest-path "$root/Cargo.toml"

"$compiler" check "$root/examples/kinetic_energy.vibe"
"$compiler" run "$root/examples/kinetic_energy.vibe" -o "$tmp_dir/kinetic" | grep -qx '200 kJ'
"$compiler" run "$root/examples/array_loop.vibe" -o "$tmp_dir/array" | grep -qx '6 m'
"$compiler" run "$root/examples/dot_product.vibe" -o "$tmp_dir/dot" | grep -qx '32 1'
"$compiler" run "$root/examples/hello_world.vibe" -o "$tmp_dir/hello" | grep -qx 'Hello, world!'
"$compiler" show "$root/examples/kinetic_energy.vibe" @physics.kinetic_energy | grep -q 'interface:'
"$compiler" callers "$root/examples/kinetic_energy.vibe" @physics.kinetic_energy | grep -qx '@app.main'
"$compiler" callees "$root/examples/kinetic_energy.vibe" @app.main | grep -qx '@physics.kinetic_energy'
"$compiler" impact "$root/examples/kinetic_energy.vibe" @physics.kinetic_energy | grep -qx '@app.main'

if "$compiler" check "$root/tests/bad_units.vibe" >"$tmp_dir/out" 2>"$tmp_dir/err"; then
    echo "bad_units.vibe unexpectedly passed" >&2
    exit 1
fi
grep -q 'unit mismatch' "$tmp_dir/err"

if "$compiler" check "$root/tests/immutable_output.vibe" >"$tmp_dir/out" 2>"$tmp_dir/err"; then
    echo "immutable_output.vibe unexpectedly passed" >&2
    exit 1
fi
grep -q 'must be a mutable array binding' "$tmp_dir/err"

echo "all Vibe bootstrap tests passed"

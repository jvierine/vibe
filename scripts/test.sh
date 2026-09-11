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
"$compiler" run "$root/examples/dot_product.vibe" -o "$tmp_dir/dot" | grep -qx '32'
"$compiler" run "$root/examples/hello_world.vibe" -o "$tmp_dir/hello" | grep -qx 'Hello, world!'
"$compiler" env check "$root/examples/hello_world.vibepack" | jq -e '.status == "checked"' >/dev/null
"$compiler" env inspect "$root/examples/hello_world.vibepack" @app.main | jq -e '.result.effects == ["io.stdout"]' >/dev/null
"$compiler" env branches "$root/examples/hello_world.vibepack" | jq -e '.branches[0].name == "main"' >/dev/null
"$compiler" env history "$root/examples/hello_world.vibepack" | jq -e '.commits | length >= 1' >/dev/null
"$compiler" env git-textconv "$root/examples/hello_world.vibepack" | grep -q '@app.main'
"$compiler" env run "$root/examples/hello_world.vibepack" -o "$tmp_dir/env-hello" | grep -qx 'Hello, world!'
"$compiler" show "$root/examples/kinetic_energy.vibe" @physics.kinetic_energy | grep -q 'interface:'
"$compiler" callers "$root/examples/kinetic_energy.vibe" @physics.kinetic_energy | grep -qx '@app.main'
"$compiler" callees "$root/examples/kinetic_energy.vibe" @app.main | grep -qx '@physics.kinetic_energy'
"$compiler" impact "$root/examples/kinetic_energy.vibe" @physics.kinetic_energy | grep -qx '@app.main'
"$compiler" show "$root/examples/hello_world.vibe" @app.main | grep -q 'effects: io.stdout'
"$compiler" inspect-json "$root/examples/hello_world.vibe" @app.main | jq -e '.object.effects == ["io.stdout"]' >/dev/null
"$compiler" emit-c "$root/examples/dot_product.vibe" | grep -Eq 'float v_[0-9a-f_]+ = vibe_'

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

for invalid in bad_assignment bad_main empty_unit loop_scope loop_width malformed_number missing_brace missing_return old_unit_syntax rank_zero shadowing; do
    if "$compiler" check "$root/tests/$invalid.vibe" >"$tmp_dir/out" 2>"$tmp_dir/err"; then
        echo "$invalid.vibe unexpectedly passed" >&2
        exit 1
    fi
done

echo "all Vibe bootstrap tests passed"

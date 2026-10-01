# Workgroup synchronization V1

This standalone package defines fixed-launch workgroup reduction, scan, and
atomic kernels with CPU oracles and formal contracts. LDS collectives are
target-neutral through semantic MIR and Kernel IR, then bind independently to
the production compiler profile for gfx942 or gfx950.

## LDS publish/read reduction

`lds_publish_read_reduce_i32_v1` is ordinary attributed Rust source. Every
admitted lane publishes one `i32` to its same-index LDS slot. The public
`fe2o3-device` target-neutral workgroup reduction supplies the uniform publish
barrier, deterministic reduction barriers, and final reuse barrier. Lane zero
is the only global output writer. Admission rejects mathematical sums outside
`i32`, so the device's wrapping tree computes the exact mathematical sum.

## Inclusive and exclusive scans

Six scan families expose 18 feature-isolated ordinary attributed Rust entries:
both modes for every admitted scalar at each exact 3-, 65-, and 255-lane
workgroup extent. They use the same affine dynamic-LDS and workgroup-authority
contract. The public
`inclusive_scan_sum` and `exclusive_scan_sum` terminals order prefixes by
linear work-item rank and return one result per lane through a typed
`DisjointSlice`. Their exact compiler, CPU simulation, schedule replay, and
debug evidence contract is documented in
[`target-neutral-workgroup-scan-v1.md`](../../docs/target-neutral-workgroup-scan-v1.md).
The checked-in [`scan-u32-request.json`](scan-u32-request.json) drives the
ordinary 3-lane source through Bundle V5 and the CPU simulator with the command
documented there.

The reduction source requests `DynamicLds::<i32>::exact_current::<64>` and
passes that linear capability directly to the collective terminal. It obtains
`WorkgroupCollectives::current()` and calls
`WorkgroupCollectives::reduce_sum_portable`; neither operation names or selects a GPU
family. It cannot substitute a host/global raw pointer or expose the LDS
pointer. The production importer authenticates this source, its
launch-resource sidecar, and its complete reachable portable-MIR closure. The
generic semantic lowerer creates one aligned, epoch-branded workgroup
allocation shared by all lanes; no workload profile or prebuilt Kernel IR is
selected. V1 admits only sum over `u32`, `i32`, and `f32`, with an exact
one-dimensional workgroup in `1..=256`; scan also admits odd sizes and partial
waves while reduction remains power-of-two-only. Every lane participates in
every acquire-release barrier phase. Unsupported scalar types, operations,
geometry, provider identity, and target profiles fail before target-bound
Kernel IR is created.

## Masked wrapping row transform/reduction

The opt-in `row-affine-u32-kernel` feature selects the explicit SIMT
`row_affine_sum_u32_v1` source. A checked shared view specifies an element
offset, rows, columns and row stride. For every logical row the result is
`sum(value * scale + bias) mod 2^32`, over active columns only.

The initial contract admits 0..=128 columns and complete 64-lane workgroups,
with one group per row or extra inactive groups, 256 bytes of LDS and at least
`rows` output elements. Each lane handles columns `2 * lane` and
`2 * lane + 1`; every lane reaches the existing target-neutral reduction.
Only lane zero stores through checked row-striped output ownership. Empty rows
perform no stores; empty columns produce zero without applying bias. Trailing
output elements remain unchanged.

Invalid widths, output extents, physical geometry, insufficient groups and
invalid checked input views trap before writes. Nonempty views require
`row_stride >= columns` and only the last logical element, not trailing row
padding, must fit the input. Empty views still require `offset <= input.len()`.
The caller must supply complete groups: a rounded physical launch extent is
not evidence that a simulator request contains every lane.

`row_affine_oracle` supplies the independent u128 logical-row specification
and shared boundary corpus, including true zero-length views. Host oracle
tests are not device-kernel execution. The required source-to-SIM test uses the
existing production driver, not a second CPU kernel body. It exports the
ordinary Rust source for gfx942 and gfx950 profiles, then checks all 86 cases
with canonical and seeded schedules and persisted replay. Input bytes,
initialization masks, output canaries, invalid requests and stale replay
bindings are checked. These are CPU semantic checks, not hardware execution or
performance predictions.

Run the source-to-SIM regression with the repository's pinned nightly:

```sh
cargo test --locked -p rustc-codegen-fe2o3 \
  --test production_neutral_workgroup_reduce_driver_v1 \
  ordinary_row_affine_source_matches_oracle_and_replay -- --ignored --exact
```

This does not establish the required structured tile or mixed variant,
generated host/artifact admission, direct-KFD execution or M2 completion.
The CPU simulation lesson's whole-file tab 6 and paired inventory bind this
exact SIMT source through its production test and feature selection. Tile and
mixed source variants remain pending under
[issue #275](https://github.com/harsh-nod/fe2o3/issues/275); this source association
and the separately pinned CPU checkpoint do not complete a tutorial pair.
Hardware launch representability is a separate runtime contract. The old
`run-gfx942.sh` and protected scalar driver still select their existing
reduction profiles and do not qualify this new ABI.

Run the feature-isolated host tests through the normal binding driver:

```sh
cargo fe2o3 test --locked --manifest-path examples/workgroup_sync_v1/Cargo.toml \
  --no-default-features --features row-affine-u32-kernel --test row_affine
```

## Scoped atomic add

The second profile admits one coherent global `u32` atomic object, relaxed
ordering, system scope, and exactly 64 eligibility declarations. Eligible
lanes add once; ineligible lanes do not touch the object. Overflow is rejected
by host admission so the final value is an exact mathematical sum.

Its ordinary attributed Rust source is compiled from `src/scoped_atomic.rs`.
The signature uses `DeviceGlobalMutPtr<u32>` to state global address space
without pretending the concurrently shared object is a Rust slice. Generated
host bindings accept only a one-element initialized `u32` device region held
under an exclusive borrow; they expose neither a raw pointer nor a launch path.
The macro registration binds global address space, mutability, pointee type,
physical pointer layout, and exclusive alias admission.

## Evidence boundary

The 2026-09-25 integration check does not qualify the broad native LLVM gate:
its first gfx942 LDS reduction fails generic formal memory admission with an
inter-invocation conflict. The coverage described below remains an acceptance
requirement, not a claim that every current gate passes. Historical native
results retain their original source/compiler identities; row CPU results
grant neither native artifact nor GPU execution authority.

The checked CPU oracles and deterministic debug/release tests are usable now.
They fail before output mutation and reject missing or divergent barriers,
stale epochs, duplicate or wrong owners, invalid lane counts, incorrect atomic
address space, ordering, scope, target, eligibility, overflow, and substituted
outputs. Verus models initialization, convergence, epoch reuse, ownership,
exact integer sums, and atomic eligibility, with expected-negative mutations.

Both kernels are ordinary attributed Rust modules with source-level typed ABIs.
They enter the same feature-independent production transaction: authenticated
rustc collection, semantic MIR, ranked PLIRON, verified Kernel IR, composed
formal/ranked memory checks, target-bound gfx942 or gfx950 LLVM,
compiler-bound handoff, measured upstream LLVM target APIs plus in-process LLD,
and COV6 inspection. There is no workload-profile selector on this route, and
the compiler handoff grants no load or launch authority.

The ignored neutral-collective rustc driver requires the pinned nightly. It
authenticates the compiler-observed provider definition identities and
recomputed complete source-closure pin, then checks semantic MIR, ranked
PLIRON, generic LDS/tree/barrier KIR, target binding, and the LLVM route for
the three reduction and six canonical scan entries on both targets. The 18-entry
Bundle V5 CPU gate independently covers every scan family at every representative
extent. The separate
protected reduction driver requires the authority launcher and measured Worker
V3 and LLVM build identities. It starts again from the immutable reduction
sources for all three scalar profiles
and checks real Worker/finalizer output for the exact
256-byte static group segment, 288-byte complete kernarg ABI, required
`64x1x1` workgroup, COV6 descriptor, and deterministic two-run HSACO. It does
not dispatch the code object or grant load/launch authority. The scoped-atomic
profile and scan HSACO qualification are outside this protected driver.

This is bounded source-to-code-object evidence, not a compiler-refinement proof
or a claim of generalized memory safety, race freedom, reduction coverage, or
GPU execution. The closed target-neutral V1 reduction contract is supported
only on the pinned gfx942 and gfx950 production compiler profiles. This is
compiler-target evidence for both targets, not gfx950 execution evidence. The
currently qualified direct-KFD execution path is gfx942 only and must enter
through Worker V3 and the pure-Rust KFD runtime. The finalizer uses no COMGR and
no shell linker; it does not shell out to `clang`, `llc`, or `ld.lld`.

## Validation

```sh
cargo fe2o3 test --locked --all-targets \
  --manifest-path examples/workgroup_sync_v1/Cargo.toml
cargo fe2o3 test --release --locked --all-targets \
  --manifest-path examples/workgroup_sync_v1/Cargo.toml
cargo fe2o3 clippy --locked --all-targets \
  --manifest-path examples/workgroup_sync_v1/Cargo.toml -- -D warnings
VERUS=/absolute/path/to/pinned/verus \
  examples/workgroup_sync_v1/run-verus.sh
```

## Mixed Tile/SIMT CPU Diagnostic Example

The opt-in `mixed-tile-u32-kernel` feature selects
[`src/kernel_mixed_tile_u32.rs`](src/kernel_mixed_tile_u32.rs): an ordinary
`KernelContext` kernel with a masked three-element tile load, per-lane Rust
wrapping arithmetic, and a bounds-checked global output store. It is disabled by
default. At compiler `ae162efd2bc8df7a061108a126333c378ac980d9`, this ordinary
standalone manifest completed two fresh public V18 exports, two inventories,
52 independently checked CPU simulations and two complete 15-command JSONL
debugger sessions, with six host-oracle tests and ten refusal/unavailable
controls. No fixture source injection was used. This is bounded CPU observation,
not native/GPU execution or a completed SIMT/tile tutorial pair.

The [diagnostic qualification](../../docs/diagnostic-scoped-tile-v18.md)
records the exact source and result-summary hashes.
[Retained output and JSONL captures](https://github.com/harsh-nod/fe2o3-kernels/tree/dfa8fa27ca265309107b1c27bda8dd2528f4cbf3/examples/mixed-tile-cpu-v18)
use a 65-element boundary-value input and eight output elements. The separate
`1..=65` request below is an illustrative input, not that retained capture.

Blocked order assigns input positions `base + 3*lane + j`; striped order
assigns `base + 64*j + lane`. Each active position contributes `x*3+11`,
`y*5+13` or `z*7+17` modulo 2^32. No active position means no write.
Different orders generally produce different per-lane results. Each workgroup
revisits the input tile but writes to distinct global output indices.

[`src/mixed_tile_oracle.rs`](src/mixed_tile_oracle.rs) is an independent
u128 specification with per-element initialization state. Its host tests cover
literal output anchors, masked tails, wrapping arithmetic, shifted and
overflowing 64-bit bases, short outputs, two workgroups, and guard regions:

```sh
cargo test --locked --manifest-path examples/workgroup_sync_v1/Cargo.toml \
  --no-default-features --test mixed_tile
```

From the compiler workspace, build the public tools and export the same ordinary
example source twice. No fixture feature or source-injection environment is used.
Both orders intentionally share their Cargo target directory; a cached command
that does not publish a fresh canonical file is a failure, not an export.
Use the repository's `nightly-2026-04-03` toolchain. The V18 exporter pins
`-Copt-level=0 -Zmir-opt-level=0` with overflow checks enabled; source
acceptance checks and mutable execution-borrow copy refusal remain unchanged.

```sh
cargo build --locked -p rustc-codegen-fe2o3 \
  --bin fe2o3-export-sim --bin fe2o3-rustc-extract
cargo build --locked -p fe2o3-kir-sim-cli --bin fe2o3-kir-sim
cargo build --locked -p fe2o3-debug-cli --bin fe2o3-debug

repo="$PWD"
tools="$(realpath "${CARGO_TARGET_DIR:-target}/debug")"
out="$(mktemp -d)"
export_target="$repo/target/mixed-tile-cpu-export"

for order in blocked striped; do
  "$tools/fe2o3-export-sim" \
    --diagnostic-kir-v18 --diagnostic-tile-order "$order" \
    --crate fe2o3_workgroup_sync_v1 --target gfx942 \
    --target-dir "$export_target" --output "$out/$order.kir" \
    -- --manifest-path "$repo/examples/workgroup_sync_v1/Cargo.toml" \
    --no-default-features --features mixed-tile-u32-kernel
  "$tools/fe2o3-kir-sim" inspect \
    --diagnostic-kir-v18 "$out/$order.kir" \
    --output "$out/$order.inventory.json"
done
```

Inventory reports all actual canonical kernel IDs and ordered entry types. It
does not execute or establish simulator admission. Do not guess the kernel ID
from the Rust function name. This request generator checks the example's actual
ABI and binds each request to the measured ID. The Rust `usize` base is
represented by the exported `u64` parameter, not an `index` parameter.

```sh
for order in blocked striped; do
  python3 - "$out/$order.inventory.json" "$out/$order.kir" \
    "$out/$order.request.json" <<'PY'
import hashlib, json, pathlib, struct, sys

inventory_path, kir_path, request_path = map(pathlib.Path, sys.argv[1:])
inventory = json.loads(inventory_path.read_text())
canonical = kir_path.read_bytes()
assert inventory["schema"] == "fe2o3-kernel-inventory-v1"
assert inventory["authority"] == "observation_only"
assert inventory["simulated"] is False
assert inventory["simulator_admission"] == "not_checked"
assert inventory["kir"]["wire_version"] == 18
assert inventory["kir"]["raw_sha256"] == hashlib.sha256(canonical).hexdigest()
assert inventory["kir"]["canonical_bytes"] == len(canonical)
assert len(inventory["kernels"]) == 1
kernel = inventory["kernels"][0]
assert kernel["id"] and kernel["workgroup_size"] == [64, 1, 1]
assert kernel["request_abi"] == "entry_parameter_order"
assert kernel["results"] == []
parameters = kernel["parameters"]
assert [p["index"] for p in parameters] == [0, 1, 2]
for index, access in [(0, "read_only"), (2, "read_write")]:
    assert parameters[index]["type"] == {
        "kind": "slice", "address_space": "global", "access": access,
        "element": {"kind": "scalar", "type": "u32", "bits": 32},
    }
assert parameters[1]["type"] == {"kind": "scalar", "type": "u64", "bits": 64}
request = {
    "schema": "fe2o3-simulation-request-v1", "kernel": kernel["id"],
    "grid": [64, 1, 1], "workgroup": [64, 1, 1],
    "arguments": [
        {"kind": "buffer", "element": "u32", "access": "read_only",
         "alignment": 4, "bytes": "0x" + struct.pack("<65I", *range(1, 66)).hex()},
        {"kind": "scalar", "type": "u64", "bits": "0x0000000000000000"},
        {"kind": "buffer", "element": "u32", "access": "read_write",
         "alignment": 4, "bytes": "0x" + "00" * 256,
         "initialized": "0x" + "00" * 32},
    ],
}
with request_path.open("x") as output:
    json.dump(request, output)
    output.write("\n")
PY
  "$tools/fe2o3-kir-sim" \
    --diagnostic-kir-v18 "$out/$order.kir" \
    --request "$out/$order.request.json" --output "$out/$order.result.json"
done

"$tools/fe2o3-debug" sim --diagnostic-kir-v18 "$out/blocked.kir" \
  --request "$out/blocked.request.json" --protocol jsonl --wave-width 64
```

For the displayed input `1..=65` and base zero, the independent mathematical
oracle predicts first output `75` for blocked and `352` for striped. These are
expected oracle values, not observed CLI results.

Start the interactive JSONL session with:

```json
{"schema":"fe2o3-debug-request-v1","request_id":1,"expected_revision":0,"operation":"discover_capabilities"}
```

Use the returned session revision in subsequent requests. Step by operation,
inspect dispatch/workgroup/wave/lane scopes, inspect captured logical values,
and obtain allocation identities from captured pointers before `read_memory`.
The output begins uninitialized: inspection can distinguish a valid zero store
from an inactive lane's untouched bytes. Continue and inspect the final recorded
state against the selected order's oracle. See the
[debugger CLI protocol](../../crates/fe2o3-debug-cli/README.md) for request fields.
The linked retained sessions show exact completion, logical hierarchy and
pointer-derived allocation memory, initialization bits, unchanged canaries and
one reverse operation to the final captured snapshot. They do not claim source
stepping or physical-register inspection.

The committed regression was separately qualified at
`6d0bf3935e3a9d6b56b0c3a24889510214ed683e` and is selected by the production
`scripts/ci-local.sh rocm-compile` pipeline. It exports both orders from the
ordinary manifest, discovers each actual ABI, compares all 52 CPU cases against
an independent oracle and drives both complete JSONL sessions. Run it directly:

```sh
cargo test --locked -p rustc-codegen-fe2o3 \
  --test production_scoped_tile_cpu_driver_v1 \
  ordinary_mixed_tile_source_executes_public_cpu_cli_paths \
  -- --ignored --exact --test-threads=1
```

This regression requires the pinned toolchain and its preinstalled dependencies,
not GPU access. It retains a bounded evidence directory whose path is printed
by the driver. Test presence alone is not a successful run; retain the command
result and exact revision when recording a new qualification.

Raw V18 currently requires Wave64. Source maps, source stepping, physical
registers, and persisted schedule record/replay are unavailable on this route.
Canonical inventory is metadata-only; CPU simulation/debugging is
observation-only. Neither the gfx942 source target nor these CPU results grants
GPU execution, native artifact, protected compiler authentication, source
refinement, hardware timing, performance prediction, or cross-order equivalence.

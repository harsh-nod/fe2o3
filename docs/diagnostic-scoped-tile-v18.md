# Scoped Tile V18 Diagnostics

This opt-in path connects actual Rust accepted by the existing scoped tile
observer to raw canonical V18 CPU/debug input. It does not complete the native
compiler path, authenticate serialized source, or qualify a SIMT/tile tutorial
pair. Unsupported source remains an error; there is no older-IR fallback.

## Export

For an existing kernel crate using the admitted `KernelContext::with_workgroup`
and `MaskedTile1D<u32, L, E, _>::load_masked` subset:

```sh
fe2o3-export-sim --diagnostic-kir-v18 --diagnostic-tile-order blocked \
  --crate my_kernels --output /absolute/fresh/tile-v18.kir \
  --target gfx942 --target-dir /absolute/owned/export-target \
  -- --manifest-path /absolute/my-kernels/Cargo.toml --offline
```

Use the actual rustc crate name and an existing output parent. The output file
must not exist. The exporter and sibling `fe2o3-rustc-extract` must be built
together with the repository's pinned toolchain. Both diagnostic flags are
required; `striped` is the other admitted order. Bundle, V16/V17, region-origin,
ranked, native-output, and inherited diagnostic controls cannot be combined
with this route. The V18 source extraction profile uses `-Copt-level=0`,
`-Zmir-opt-level=0`, `-Zinline-mir=no`, `-Zmir-enable-passes=-JumpThreading`,
`-Zalways-encode-mir`, and `-Coverflow-checks=on`, plus the selected target
CPU/features. It does not override debug assertions. These target rustflags
apply to the selected crate and target dependencies in the Cargo extraction.
Other export formats retain their existing profiles.

The explicit V18 MIR level qualified the ordinary mixed-tile example described
below through the public CLI. This is evidence for that exact source and
profile, not a general reborrow-normalization result. Mutable execution-borrow
copy rejection and all source checks remain unchanged; unsupported source
still fails without a fallback.

The exporter runs real source collection, semantic MIR/SSA and the existing
borrowed scalar-candidate observer. It stages exact bytes while the source and
candidate owners and original resource ledger are live. It promotes the file
without clobbering only after exactly one source callback, one completed
candidate observation and the expected diagnostic-only terminal refusal.
Compiler continuation or a different error never counts as diagnostic success.
Unknown partial/replaced staging files are retained rather than deleting
foreign state; successful staging is removed without touching the output.

The declared `gfx942` or `gfx950` source profile is not GPU execution evidence.
Raw V18 retains its storage table and canonical identity, but contains no
exported source ownership, source-variable map, formal proof, protected compiler
receipt, native artifact, or load/launch authority.

## CPU and Debugger

Discover actual kernel IDs and ordered argument types before constructing a
request:

```sh
fe2o3-kir-sim inspect --diagnostic-kir-v18 /absolute/fresh/tile-v18.kir \
  --output /absolute/fresh/inventory.json
fe2o3-kir-sim --diagnostic-kir-v18 /absolute/fresh/tile-v18.kir \
  --request /absolute/request.json --output /absolute/fresh/result.json
fe2o3-debug sim --diagnostic-kir-v18 /absolute/fresh/tile-v18.kir \
  --request /absolute/request.json --protocol jsonl --wave-width 64
```

These are existing simulator/debugger engines, not a second CPU kernel body.
The input request must match the actual emitted kernel name, scalar ABI and
buffer access. On the admitted 64-bit source target, a `usize` base in this
path has the emitted `u64` ABI, not the simulator's separate `index` scalar.
Inventory is metadata-only: `simulator_admission=not_checked` does not
establish executability or additional launch requirements.

CPU execution permits supported scalarized operations with inert layout
metadata; executable storage-object operations remain explicit refusals.
Persisted schedules, exploration/reduction, source maps and runtime-observation
mode are not enabled for raw V18. The debugger exposes logical CPU observations,
not hardware registers or a physical wave trace.

Blocked and striped assign fragment components differently. Fragment-local
weighted computation or per-lane stores may therefore change whole-kernel
output. This selector does not prove that two distributions implement the same
algorithm. Source identities from separate rustc contexts must not be treated
as interchangeable merely because the source text matches.

## Qualification Boundary

At compiler `ae162efd2bc8df7a061108a126333c378ac980d9`, the ordinary
`examples/workgroup_sync_v1/Cargo.toml` manifest with the opt-in
`mixed-tile-u32-kernel` feature completed two fresh public exports, two
metadata inventories, 52 independently oracle-checked CPU simulations, and
two complete 15-command JSONL debugger sessions. Six independent host-oracle
tests and ten refusal or unavailable controls also passed. Both exports used
the same source path and Cargo target directory, without fixture source
injection. The selected source SHA-256 was
`e8aad7f11d63371e7d97e915dcbb5e35a6f44b885750a621696895f7d8810ee5`.

The retained batch is
`v18-public-example-cli-mir0-ae162efd2bc8df7a061108a126333c378ac980d9-20260929203358-887257`;
its summary SHA-256 is
`a24a67fa0fbfda70909a0747c086668bee131885f34e3026830f45d06c4bc738`.
The debugger sessions inspected dispatch/workgroup/logical-wave/lane scopes,
captured logical pointer identities, allocation-relative memory and byte
initialization, exact completion, and a reverse operation to the final retained
snapshot. Source stepping and physical-register inspection remained explicitly
unavailable. The example's `gfx942` source profile is not hardware execution.

These are bounded observations for the masked three-element `u32` tile load,
per-lane wrapping arithmetic and checked output source. They do not qualify
every accepted kernel, authenticate source-to-KIR refinement, establish
cross-order equivalence, or complete native SIMT/tile tutorial pairs. The two
orders retain separate canonical and schedule identities.

Focused source tests use the existing genuine-source harness, exact emitted
identity checks, retained nonempty layout metadata, and the independent masked
tile CPU/replay oracle. They cover both diagnostic orders and both declared
source targets without asserting cross-session source identity equality.

The relevant exact test parent is:

```text
production_rustc_driver_v1::checked_output_source_v1_tests::context_source_v29_tests::pending_source_tests::tile_export_tests::actual_scoped_tile_export_preserves_canonical_identity_and_cpu_oracle
```

It is ignored by default because it requires authentic AMD SDK dependencies and
the pinned rustc source toolchain. Source presence and an ignored test do not
establish successful execution. The integration owner must retain the exact
tested commit, selected child counts, results and separate CLI session evidence.
No native/GPU or full tutorial-pair result is implied by this diagnostic path.

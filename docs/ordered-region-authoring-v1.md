# Author one fixed-register instruction region

This draft implementation tutorial exercises real Rust source, one closed
instruction region, diagnostic CPU execution, and a separate final-machine
inspection. Ordinary tools now expose an explicit diagnostic path: the exporter
uses the valueless `--diagnostic-kir-v16` flag; the simulator and JSONL debugger
use `--diagnostic-kir-v16 PATH`. This is not a simulation-bundle format, protected
production admission, a supported source-to-GPU workflow, or a curriculum release.

Tracking remains [#280](https://github.com/harsh-nod/fe2o3/issues/280),
[#281](https://github.com/harsh-nod/fe2o3/issues/281), and
[#282](https://github.com/harsh-nod/fe2o3/issues/282). Their complete milestones
remain open; see the [implementation status](assembly-authoring-implementation-status.md)
and the earlier [typed-instruction slice](assembly-authoring-first-slice.md).

## The source contract

Use the complete [source fixture](../crates/rustc-codegen-fe2o3/tests/fixtures/production-extraction-device/src/ordered_region_v31.rs)
and its adjacent manifest/root module. This display shows its positive branch;
the actual fixture also contains the explicitly selected negative variants.

```rust
use fe2o3_device::{DisjointSlice, amdgpu_asm, amdgpu_ordered_region, kernel, thread};

#[kernel(typed, launch(required = [64, 1, 1], max = [64, 1, 1]))]
pub fn ordered_xor_add(mut output: DisjointSlice<u32>, a: u32, b: u32, c: u32) {
    let a = amdgpu_asm!(v_mov_b32(a));
    let value = amdgpu_ordered_region! {
        gfx942_xnack_off_wave64;
        scratch(32);
        out(33);
        in(34) = a;
        in(35) = b;
        in(36) = c;
        xor_add_u32_e32;
    };
    let index = thread::index_1d();
    if let Some(element) = output.get_mut(index) {
        *element = value;
    }
}
```

The macro accepts no assembly string. It denotes exactly these two instructions,
in this internal order, with a wrapping `u32` result:

```text
v_xor_b32_e32 v32, v34, v35
v_add_u32_e32 v33, v32, v36
result = (a ^ b).wrapping_add(c)
```

The five VGPR indices must be distinct literals in `0..64`. Data inputs and
result are exactly `u32`; the authenticated source marker receives five actual
MIR `u8` constants for the physical roles. Data expressions are evaluated once.
The marker has no silent host fallback: calling it on the host panics.

Only one unconditional region in one direct kernel root is admitted here, on
`gfx942:xnack-`, wave64, with required workgroup size `64x1x1`. It must precede
conditional source edges and cannot be re-entered through a loop. The old
`v_mov_b32` marker above demonstrates coexistence, not arbitrary pre-region
calls or memory accesses.

The region has **NoMemory**, reads EXEC, and declares no implicit register
writes. Its output is early-clobber and its scratch register is clobbered.
Internal instruction order is retained, including when the result is unused.
This is not a memory fence or a promise to fix the order of unrelated surrounding
operations. Register bindings are local to this operation; handles do not escape.

## Where it enters compilation

```text
actual Rust -> semantic MIR V31 -> same private source-owned canonical KIR V16
                                  |-> raw diagnostic file -> CPU sim / JSONL debug
                                  `-> opt-in retained-owner qualifier -> LLVM22
                                        -> test worker / LLD / static inspection
```

The release-active private continuation consumes the real collected transaction
through normal semantic import, middle-end and SSA construction, retaining the
original authenticated source, target and launch-descriptor bindings. The ordinary
diagnostic exporter and opt-in source qualifier share this sole owner construction.
Only the qualifier's additional observation callbacks remain `cfg(test)`. The
move-only V16 owner retains the verified decoded module and exact canonical bytes;
there is no replacement simulator graph or reinterpretation as a V12 owner.

Diagnostic export stops at a raw canonical file. Its consumers independently
admit those bytes for CPU observation; serialization does not retain the private
source-owner borrow, authenticate source, provide a source map, or grant proof,
artifact or production-resume authority. This path emits neither LLVM nor a GPU
binary. It does not change the ordinary protected compilation pipeline.

In the separate retained-owner LLVM qualification, LLVM IR is not bypassed.
The region becomes one `asm sideeffect` call with
constraints `=&{v33},{v34},{v35},{v36},~{v32}`. Ordinary surrounding Rust operations
remain ordinary LLVM operations. Compiler-owned boundary copies are allowed and
reported separately from the authored pair. The V16 renderer selects the existing
reviewed LLVM22 worker data layout at initial header generation; it does not edit
a captured LLVM file or relax the worker's target checks. Historical LLVM layout
contracts remain unchanged.

CPU simulation treats the pair as one logical operation. It checks wrapping
values and surrounding simulated memory, not per-instruction physical VGPRs,
EXEC contents, cycles, register pressure or hardware behavior. The separate
native observer assembles/links and inspects returned machine bytes through an
explicitly unauthenticated test transport. Hash joins establish consistency of
retained observations, not source authentication or protected artifact authority.

The [logical debugger exercise](ordered-region-debugger-v1.md) separates ordinary
raw-file debugging from the optional same-live-source-owner qualification. Both
distinguish the authored register plan from logical before/after values. Neither
exposes physical scratch or EXEC contents. The checked-in
[bounded debugger exercise](ordered-region-debugger-v1.md#reproduce-the-bounded-debugger-exercise)
automates current-owner inspection, independent expected values, stepping and
complete output-memory checks after the source export below.

## Use the ordinary diagnostic tools

Run on Linux from the compiler checkout with the installed `nightly-2026-04-03`
toolchain and the components in `rust-toolchain.toml`, including `rust-src` and
`rustc-dev`. These commands assume dependencies are already cached for `--offline`.
Serialize Cargo work and allow at least 40 GiB free plus actual host/target build
room. Start without custom FE2O3 extraction variables or Rust compiler wrappers;
do not set private extraction-output variables yourself. Replace the qualification
storage placeholder with an existing writable absolute directory.

Run selected tests first. The final build before export must be a normal build
of the backend library, four binaries and inspection example below. If tests run
later, repeat this normal build so export does not accidentally load a test-built
codegen shared library.

```sh
ordered_repo=$(pwd -P)
ordered_cli_run=$(mktemp -d -p /absolute/qualification-storage fe2o3-ordered-cli.XXXXXXXX)
export RUSTUP_TOOLCHAIN=nightly-2026-04-03
export RUSTC=$(rustup which --toolchain nightly-2026-04-03 rustc)
export CARGO=$(rustup which --toolchain nightly-2026-04-03 cargo)
export CARGO_BUILD_JOBS=2 CARGO_INCREMENTAL=0 CARGO_PROFILE_DEV_DEBUG=0
export CARGO_TARGET_DIR="$ordered_cli_run/host-target"
"$RUSTC" -vV

"$CARGO" build --locked --offline \
  -p rustc-codegen-fe2o3 -p fe2o3-kir-sim-cli -p fe2o3-debug-cli \
  --lib --bin fe2o3-rustc-extract --bin fe2o3-export-sim \
  --bin fe2o3-kir-sim --bin fe2o3-debug \
  --example inspect_diagnostic_ordered_region_v16

ordered_bin="$CARGO_TARGET_DIR/debug"
"$ordered_bin/fe2o3-export-sim" --diagnostic-kir-v16 \
  --crate fe2o3_production_extraction_fixture \
  --output "$ordered_cli_run/used.kir" --target gfx942 \
  --target-dir "$ordered_cli_run/device-target" -- \
  --manifest-path "$ordered_repo/crates/rustc-codegen-fe2o3/tests/fixtures/production-extraction-device/Cargo.toml" \
  -p fe2o3-production-extraction-fixture --lib --no-default-features \
  --features ordered-region-v31 --offline
```

Check that rustc reports release `1.96.0-nightly` and commit
`55e86c996809902e8bbad512cfb4d2c18be446d9`. A matching toolchain name alone is not
a compiler-closure attestation. The exporter uses its sibling extractor and
normal backend shared library, checks the source crate with locked Cargo
resolution, and publishes to a new path. Reusing `used.kir` refuses rather than
overwrites. Exporter `--diagnostic-kir-v16` takes no value, conflicts with every
`--bundle-version` selection, and supports only this closed gfx942 source profile.

### Supply the exact request and check every output byte

The kernel ABI is the output slice first, then `a`, `b`, `c`. This request uses
19, 23 and 42. A 264-byte backing contains a four-byte leading guard, a 64-u32
output view at byte offset 4, and a four-byte trailing guard. All bytes start as
`a5`, with every initialization bit clear. The guards must stay unchanged and
uninitialized. The logical result is `(19 ^ 23) + 42 = 46`.

```sh
node --input-type=module - "$ordered_cli_run" <<'JS'
import { writeFileSync } from 'node:fs';
import { join } from 'node:path';
const request = {
  schema: 'fe2o3-simulation-request-v1', kernel: 'ordered_xor_add',
  grid: [64, 1, 1], workgroup: [64, 1, 1],
  arguments: [
    { kind: 'buffer_view', backing: 1, element: 'u32', access: 'read_write',
      alignment: 4, byte_offset: 4, elements: 64 },
    { kind: 'scalar', type: 'u32', bits: '0x00000013' },
    { kind: 'scalar', type: 'u32', bits: '0x00000017' },
    { kind: 'scalar', type: 'u32', bits: '0x0000002a' },
  ],
  shared_buffers: [{ id: 1, element: 'u32', access: 'read_write', alignment: 4,
    bytes: '0x' + 'a5'.repeat(264), initialized: '0x' + '00'.repeat(33) }],
};
writeFileSync(join(process.argv[2], 'request.json'), JSON.stringify(request) + '\n',
  { flag: 'wx', mode: 0o600 });
JS

"$ordered_bin/fe2o3-kir-sim" --diagnostic-kir-v16 "$ordered_cli_run/used.kir" \
  --request "$ordered_cli_run/request.json" --output "$ordered_cli_run/result.json"

node --input-type=module - "$ordered_cli_run/result.json" <<'JS'
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
const result = JSON.parse(readFileSync(process.argv[2], 'utf8'));
assert.equal(result.status, 'ok');
assert.equal(result.authority, 'observation_only');
assert.equal(result.simulated, true);
assert.equal(result.hardware_observed, false);
assert.equal(result.counts.invocations_executed, 64);
assert.equal(result.counts.workgroups_visited, 1);
assert.equal(result.shared_buffers.length, 1);
assert.equal(result.shared_buffers[0].id, 1);
const expected = Buffer.alloc(264, 0xa5);
const value = Number(((19n ^ 23n) + 42n) & 0xffffffffn);
for (let lane = 0; lane < 64; lane++) expected.writeUInt32LE(value, 4 + 4 * lane);
const buffer = result.shared_buffers[0].buffer;
assert.equal(buffer.bytes, '0x' + expected.toString('hex'));
assert.equal(buffer.initialized, '0xf0' + 'ff'.repeat(31) + '0f');
console.log('64 logical outputs are 46; both guards are unchanged/uninitialized');
JS
```

The simulator's `kir.sha256` is a versioned, domain-separated canonical identity,
not the plain SHA-256 of the file. Compare it and the exact canonical byte length
with the exporter's report. Neither identifier authenticates source custody or
proves GPU equivalence.

### Inspect fresh identities before debugging

```sh
"$ordered_bin/examples/inspect_diagnostic_ordered_region_v16" \
  "$ordered_cli_run/used.kir" "$ordered_cli_run/request.json" \
  > "$ordered_cli_run/inspection.json"
```

Use a new output path. This example uses the shared hardened V16 loader and a
small CPU preflight; it does not execute the kernel. Its observation-only JSON
has `kind: diagnostic_ordered_region_inspection_example`, not a new registered
transport schema. `canonical` contains `wire_version`, `sha256` and `bytes`;
`coordinate` contains function, block and operation **ordinals**, with the exact
keys `function_ordinal`, `block_ordinal`, and `operation_ordinal`. `raw_block_id`
is separate. `input_value_ids` and `result_value_id` come from the current
verified owner, not a historical fixture's SSA numbering. Keep this report with
the same canonical file and request used for the [debugger exercise](ordered-region-debugger-v1.md).

`register_plan` and `declared_instruction_steps` describe the authored plan,
not observed physical contents. `declared_source_ids` are declarations, not
source authentication or a source map. Physical values, instruction microsteps,
register lifetime/final-allocation proof, proof/artifact/resume authority and
hardware execution remain false. The example deliberately accepts only one
kernel, one entry function and one region, with at most 128 blocks, 4,096
operations and 8,192 SSA values. Its 64 KiB canonical-size check is after shared
admission; a separate 64 MiB CPU-preflight resident limit and 8 KiB output cap
are not whole-process memory bounds.

### Edit supported source, then re-export

For the unused-result fixture, repeat the exporter command with feature
`ordered-region-unused-v31` and a fresh `unused.kir`. The surrounding kernel
stores `a = 19`, although the retained logical region result is still 46. Do not
reuse the expected-46 buffer check unchanged. The two exports have different
canonical identities despite sharing a source file.

On a separate concrete source copy with the normal supported manifest and pinned
dependency closure, change only the positive branch. An operand-order edit is:

```rust
in(34) = c; in(35) = b; in(36) = a; xor_add_u32_e32;
```

This gives `(42 ^ 23) + 19 = 80`. A separate register-plan edit preserving the
original data order is:

```rust
gfx942_xnack_off_wave64; scratch(40); out(41);
in(42) = a; in(43) = b; in(44) = c; xor_add_u32_e32;
```

That still gives 46. These are replacement lines inside the existing macro, not
standalone Rust programs. Re-export to a fresh file, re-run with a fresh result
path, and re-inspect current identities and SSA coordinates. Keep original
source/reports unchanged; do not mutate canonical bytes or relabel old exports.
Distinct package/crate names and source paths also contribute to isolated-copy
identities: this does not prove that only a register edit changed an identity.
CPU equality is not an equivalence proof or independent verification of final
physical allocation/encoding. The development edit-fixture generator is not a
shipped standalone reproduction tool.

All four source refusals below still apply. The diagnostic commands additionally
refuse incompatible bundle/target selections. The simulator rejects persisted
schedule recording, replay, exploration, reduction and their auxiliary options
before file access. The debugger rejects wave32, source-map overrides and
persisted schedule replay. Its existing diagnosis-V2 schema requires canonical
V7 input evidence, so raw V16 returns `unsupported_schema`, never a V7 relabel.
Logical stepping, SSA/memory views and resource queries are separate capabilities.

## Reproduce the six actual source callbacks

This optional compiler qualification retains the live source owner and adds test
observations beyond the ordinary diagnostic-file workflow above.

Run from the compiler implementation checkout on Linux with the already installed
`nightly-2026-04-03` toolchain and the components in `rust-toolchain.toml`, including
`rust-src` and `rustc-dev`. The commands below use an already populated dependency
cache (`--offline`); use the repository's normal setup before this exercise.
Serialize this run with other Cargo work. Reserve at least 40 GiB free plus room
for the host build; the fresh AMD dependency preparation itself is capped at
500 MiB. The harness is not a whole-build disk or RSS limiter.

Choose an existing, writable qualification directory outside the checkout on
that filesystem. Replace the example absolute path before running:

```sh
ordered_run=$(mktemp -d -p /absolute/qualification-storage fe2o3-ordered.XXXXXXXX)
ordered_rustc=$(rustup which --toolchain nightly-2026-04-03 rustc)
RUSTC="$ordered_rustc" CARGO_BUILD_JOBS=2 CARGO_INCREMENTAL=0 \
CARGO_PROFILE_DEV_DEBUG=0 RUST_TEST_THREADS=1 \
FE2O3_TEST_ORDERED_REGION_OUTPUT_V31="$ordered_run/source" \
cargo +nightly-2026-04-03 test -p rustc-codegen-fe2o3 --lib \
  --locked --offline actual_ordered_region_source_ladder -- \
  --ignored --exact \
  production_rustc_driver_v1::gfx942_ordered_region_qualification_v31_tests::actual_ordered_region_source_ladder \
  --nocapture
```

`source` must not exist. The parent prepares real target dependencies, then runs
six isolated rustc callbacks. Each child must execute exactly one test, not merely
return a zero exit status. Source files remain unchanged. Changing a fixture
requires rebuilding the harness; stale compiled-source bytes are rejected.

| Feature | Required observation |
| --- | --- |
| `ordered-region-v31` | One region, `(a ^ b).wrapping_add(c)` stored correctly |
| `ordered-region-unused-v31` | Region retained, despite storing `a` instead |
| `ordered-region-alias-v31` | Scratch/output alias rejected |
| `ordered-region-dynamic-v31` | Runtime `a as u8` physical binding rejected |
| `ordered-region-divergent-v31` | Region after a conditional source edge rejected |
| `ordered-region-wrong-launch-v31` | Required `32x1x1` workgroup rejected |

Both positives run six independent arithmetic cases, including overflow, across
64 logical invocations with unchanged leading/trailing canaries. The four negative
fixtures are valid Rust; they must fail at their specific source-profile boundary,
not because a process failed or some later operation was unsupported.

Read `source/observation.json`, each feature's invocation JSON and stdout/stderr,
and the positive directories' `canonical-v16.bin` and `observation.ll`. Preserve
the real Cargo artifact/metadata records and hashes. The four occurrence references
identify compiler-observed origins; they are not raw-source digests or proof receipts.

## Inspect the same LLVM bytes after native code generation

Use Node.js and the reviewed LLVM/LLD development package, CMake, a C++ compiler,
and zstd headers/library. The checked-in build script requires LLVM `22.0.0git`
and package claim
`rocm7.2.1-packages-sha256:eb02c62693d6697017195f0abf5ebcf7e58f60e4d2acf8356de2e944bceec540`.
Supply its existing package-identity file; do not invent a matching claim for
different binaries. The package claim is not a runtime-closure attestation.
Replace all input paths below with the reviewed local files:

```sh
node scripts/assembly-region-worker-prototype.mjs \
  --llvm-root /absolute/llvm-prefix \
  --llvm-build-id-file /absolute/reviewed-llvm-build-id.txt \
  --zstd-include-dir /absolute/zstd-include \
  --zstd-library /absolute/libzstd.so \
  --output "$ordered_run/native-build"
node scripts/assembly-region-source-machine-observation.mjs \
  --source-run "$ordered_run/source" \
  --native-build-run "$ordered_run/native-build" \
  --output "$ordered_run/machine"
```

Both output directories must be new. If needed, give the first script explicit
absolute `--cmake` and `--cxx` paths. The build preserves the original four native
fixture tests. The second script validates that build's measured inputs, joins
all six source callbacks to their retained records, and submits the two positive
LLVM files unchanged for O0 and O3 observation. It checks exact physical operands,
one unambiguous contiguous e32 pair, implicit EXEC reads, no implicit writes, and
sufficient descriptor VGPR capacity. Neither script launches a GPU kernel.

Read `machine/receipt.json` and the per-feature native reports. The qualified
development run found little-endian instruction bytes `2247402a` then `20494268`
in all four cases. The unused result did not eliminate the pair. Descriptor
encoded VGPR capacity was 56 at O0 and 40 at O3; the authored binding high-water
was 37, and the reported architected VGPR boundary was 40. These are different
quantities. Encoded capacity is not measured register usage, a lifetime/allocation
proof, occupancy, or a performance result. Whole-kernel instruction counts and
bytes may differ across optimization levels.

## Evidence scope and further work

The fresh `phase8-ordinary-source-r4` aggregate passed five ordinary source
exports, four expected source refusals, 30 CPU simulations and 15 ordinary
negative controls. Its `receipt.json` SHA-256 is
`441a757d093995210ea4b2f79cf082ca50ede0690a8fdd8d173ae140d0caf366`.
The associated `phase8-ordinary-debugger-r2` aggregate passed 30 ordinary JSONL
sessions: six requests for each of the original used/unused exports and three
isolated source-edit exports. Each session had 34 commands and a distinct
session/configuration identity, totaling 1,020 commands. Its receipt SHA-256 is
`82d40a95e9e3a6a277fa5fe88a9619212f5ba1f4f2dde82997764e02f2b6ce9f`.

The debugger client checked selected logical lane 0 before/after the atomic
region, reverse navigation and repeated forward navigation. It separately
checked all 64 output words, their initialization, both guards and all 64 exact
logical write-history ranges. This is not all-lane SSA inspection or physical
execution. Foreign-session token substitution was not tested by this client.
Five fresh inspector reports selected the current owner's SSA IDs and roster
coordinates and checked the declared register plans: `40..44` with high-water
45 for the register-plan edit, `32..36` with high-water 37 for the other four.
These are authored declarations, not physical register values or final code.

An old-SSA pilot and the first debugger batch with malformed stale-anchor
negative inputs remain retained failures. The client was corrected to submit
a valid old revision expecting `stale_revision`, then a current revision with
an old event expecting `invalid_cursor`. No compiler or debugger acceptance
predicate was changed to pass those controls. This ordinary CLI evidence is
separate from the private retained-source-owner qualification; neither grants
source authentication, compiler-closure, release, GPU or protected-admission
authority. The development qualification clients are not shipped wrapper tools.

The earlier `phase8-ordinary-source-r3` development aggregate passed five ordinary
source exports, four expected source refusals, 30 CPU simulations and 15 ordinary
negative controls. The isolated baseline, operand-swap and register-plan copies
produced 46, 80 and 46 respectively for the concrete request above; original source
was unchanged. Each accepted edit-fixture lock retained 109 unchanged external
dependency tuples. It remains historical working-build evidence, not a release
attestation or a substitute for the fresh source/debugger aggregates above.

The retained `phase8-ordinary-source-r2` aggregate failed at the isolated fixture's
host dependency preparation, despite its earlier original exports/refusals and
simulations passing. It is not a passing aggregate or a public minimal-manifest
recipe. The fresh r3 run used the corrected normal host dependency closure.

The earlier retained development observations are source run `ordered-region-source-v31-r4`
and machine join `ordered-region-source-machine-join-r1`. Their JSON SHA-256s are
respectively `b47c20d0ed8cfb819e8070cbf279b1a14852bb9dde91261d41746785138e9d44`
and `e54dee8f50eaf881398eb385f35558abacc39fa7d922b991755c28ae8dfff7d2`.
The final backend library suite passed 754 tests with seven explicit ignores;
the source ladder was separately selected and actually executed. These are
working-tree observations, not a retroactive commit, clean-checkout, compiler
closure or release attestation. Reproductions must keep their own records.

The earlier `ordered-region-source-v31-r3` run emitted the historical layout and
was correctly refused by native input validation. Its LLVM was not repaired in
place: `ordered-region-source-v31-r4` freshly emitted
the reviewed worker header. A failed native stage is not a passing machine case.

Unsupported profiles include SGPRs, register tuples/inouts/aliases, multiple regions,
helpers, loops around the region, divergent placement, partial waves, alternate
instruction sequences or encodings, authored memory/synchronization, matrix
instructions, gfx950, and whole-kernel assembly. General allocation/liveness,
physical debugger views, source-to-assembly promotion, reversible Rust regeneration,
persistent schedule recipes, functional proofs and protected final admission
remain separate work. Existing V6/V11 authoring/debugger commands do not accept
this V16 profile. None of the original milestone scope is replaced by this lesson.

The companion [kernel-author tutorial](https://github.com/harsh-nod/fe2o3-kernels/blob/codex/multilevel-resource-views-20260917/docs/ordered-region-authoring-v1.md)
is likewise a draft, with no lesson route or publication-pin change.

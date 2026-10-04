# Ordinary-source initializedness diagnostics and terminal replay

## Qualified scope — 2026-09-22

The bounded R3 regression passed on independently built canonical and mirror
compiler checkouts: fresh ordinary vecadd export, positive CPU result,
explicit-uninitialized-input diagnostic, and reverse/repeat debugger replay.
Each capture retained five normal command stages and 44 public debugger pairs;
the expected uninitialized simulation exits 1 while the complete harness exits
successfully. The 21 pure control groups are separate from those actual runs.

The [dated authoring/repeat/fault evidence record](evidence/authoring-repeat-fault-20260922.md#final-publication-qualification)
is the source for exact compiler versions/censuses, build and outer-supervisor
receipts, original artifact hashes, and retained failed attempts. Those pins
qualify only the recorded source/tool combination, not an arbitrary checkout.
No GPU execution, native code, general correctness proof, source authentication,
terminal snapshot, allocation reuse or broad V2 completion follows.

This exercise changes one initialized-bitmask bit in the CPU simulator's
explicit input model. Rust source and input bytes stay unchanged. It does not
change process memory or construct KIR/source maps.

### What the retained canonical session observed

| Stop | Event / revision | Observed state |
| --- | --- | --- |
| Initial forward checkpoint | 1 / 1 | Captured; complete allocation inventory |
| Fault, repeated twice | 22 / 2, 4, 6 | Fault/Failed; no captured terminal values |
| Prior load, then repeated prior load | 21 / 3, 5 | Source-resolved checkpoint, stack, SSA and memory |
| Old-revision SSA query | 21 / 5 unchanged | stale_revision; no session mutation |
| Continue at terminal | 22 / 7 | Completed/Failed, zero events advanced, still unavailable |
| Terminate | 22 / 8 | Clean session termination |

These are facts from this retained run, not constants copied into future
requests. Both prior-checkpoint observations selected the actual load at
canonical function/block/operation 0/6/2, with one frame, 13 SSA rows and three
complete memory windows totaling 56 bytes. Each paginated source V2 inspection
returned six rows across three pages: a and b were captured allocation-relative pointer
bindings; c, idx, i and out were not_represented. Missing source values were
not filled from SSA. An explicit frame-one/occurrence-one source anchor refines
the unframed checkpoint; it is not the same literal anchor or a dynamic
activation identity.

All terminal source queries returned checkpoint_not_captured; stack, SSA and
memory returned not_captured. The allocation observations use generation zero
only and establish no lifecycle transition or reuse. The source-binding records
and resource allocation generations are separate concepts.

The mirror independently passed the same bounded checks but has its own raw
bytes, identities and source-row order. In particular, the standalone raw KIR
block was 2 in the canonical result and 0 in the mirror; neither is the debugger
canonical block ordinal 6. Do not normalize those records or equate sessions.

The positive CPU run checked all four expected f32 output words, eight canary
bytes and unchanged inputs. It reported 256 scheduled slots, four actual
invocations/cooperative decisions, one workgroup, zero barriers and zero
delivered ordinary events. The exact step count remains an observed bounded
quantity, not a fixed acceptance constant.

### Retained failures, not successful fallbacks

The first canonical attempt was refused by the path guard because observation
and compiler-scratch outputs had different parents; no export ran. The next
attempt exported unchanged vecadd but used workgroup64: the simulator refused
preflight_workgroup_mismatch against the real KIR workgroup256. Neither attempt
is positive/fault execution evidence. R3 corrected the request profile and
repeated a fresh ordinary export; it did not change the source or relax
preflight. Failed logs and scratch remain retained in the dated evidence.

## Driver and unchanged fixture

The implementation and pure controls are
[resource-source-fault-replay-v2-smoke.mjs](../scripts/resource-source-fault-replay-v2-smoke.mjs)
and [its tests](../scripts/resource-source-fault-replay-v2-smoke.test.mjs).
The fixture is unchanged examples/vecadd/src/lib.rs plus its included
examples/vecadd/src/vecadd_body.rs and reviewed manifest/lock inputs. Its typed
vecadd profile requires exactly 256x1x1; do not substitute the 64-thread
instruction-program fixture contract.

Run the pure controls separately:

    node --test scripts/resource-source-fault-replay-v2-smoke.test.mjs

Importing the driver starts no capture. These controls use labeled synthetic
protocol documents in memory, never executable KIR, manufactured source maps or
real-capture substitutes. Keep the driver beside its existing reviewed helper
scripts: the normal Bundle V6 author-inspection validators, bounded JSON
parser and process transport. Do not substitute copied stale helpers or change
production owner roots to make this profile pass.

## Reproduce with matching normal tools

The dated qualification ran on mi350. Reproduce through the integrator's
deadline/process/resource supervisor. All six paths below must be absolute,
canonical paths; Cargo/rustc must be the explicit
pinned normal tools, not unreviewed shims. The observation and Cargo target
directories must both be absent, distinct siblings outside every source/tool
input tree. The driver creates them exclusively and never cleans either.

    node scripts/resource-source-fault-replay-v2-smoke.mjs \
      --repo /ABSOLUTE/APPROVED/COMPILER \
      --bin-dir /ABSOLUTE/FRESH/NORMAL/TOOLS/debug \
      --cargo /ABSOLUTE/PINNED/TOOLCHAIN/bin/cargo \
      --rustc /ABSOLUTE/PINNED/TOOLCHAIN/bin/rustc \
      --export-target /ABSOLUTE/TASK/RUNS/vecadd-export-target-r1 \
      --output /ABSOLUTE/TASK/RUNS/vecadd-fault-observation-r1

The first operation is the normal public exporter, with fresh target scratch:

    fe2o3-export-sim --crate fe2o3_vecadd \
      --output /ABSOLUTE/TASK/RUNS/vecadd-fault-observation-r1/vecadd-v6.fe2sim \
      --bundle-version 6 --target gfx942 \
      --target-dir /ABSOLUTE/TASK/RUNS/vecadd-export-target-r1 \
      -- --manifest-path /ABSOLUTE/APPROVED/COMPILER/examples/vecadd/Cargo.toml \
      --lib --offline

The existing exporter itself uses Cargo check --locked -Zbuild-std=core with the
GPU target and its ordinary extraction wrapper. Explicit --offline and
CARGO_NET_OFFLINE=true supplement that existing locked-build behavior; the
driver does not alter the lockfile. CARGO/RUSTC are explicit, incremental is off,
jobs is two, inherited FE2O3 overrides/compiler wrappers/target Rust flags are
removed, and LD_LIBRARY_PATH uses the pinned toolchain lib and selected bin dir.
The integrator must also pin/check the actual backend, extraction wrapper,
Cargo cache, rustc/sysroot/library closure, tool build receipts and source census
under its existing supervisor. The selected pins in this driver are not a
compiler-closure attestation.

Normal export refusal, source-map omission, an unsupported KIR version, or a
larger-than-bounded profile must remain a retained failure; there is no test-KIR,
historical bundle, altered Rust source, V17, or fabricated-source-map fallback.

## Fixed independent numerical/input contract

Both requests select vecadd, grid [4,1,1], workgroup [256,1,1], and three direct
global f32 buffers, alignment four. There are no shared buffers or alias views.

| Request-model buffer | Bytes / initial values | Initialization model |
| --- | --- | --- |
| A, read-only | 16 bytes: [1,2,4,8] | Positive 0xffff; diagnostic 0xfeff |
| B, read-only | 16 bytes: [.5,1.5,2.5,3.5] | 0xffff in both |
| Output, read-write | Four 0xa5a5a5a5 sentinel words plus bytes deadbeefcafebabe | 0xffffff in both |

The requests differ only in A's first initialization bit. Bit zero represents
byte zero in the simulator's packed initialized mask; bytes, element type,
access, geometry, kernel registration, and all other mask bits remain unchanged.
The output has six f32-sized slots but only four global invocations, so the
eight trailing bytes must remain untouched.

The independently written exact dyadic f32 oracle is [1.5,3.5,6.5,11.5],
little-endian bytes 0000c03f000060400000d04000003841. The positive standalone
simulation must exit zero, have empty stderr, match the inspected canonical KIR
digest/byte count, report four invocations, one workgroup, 256 scheduled slots,
four cooperative scheduling decisions and zero delivered events, retain both
input buffers exactly, produce all four words and preserve all eight canary
bytes. The entire selected success shape is
closed, including no-conflicts-observed and false hardware/authority flags.
This is a finite simulation observation, not a universal race/correctness proof.

These partial-grid counters come from the current scheduler, not an assumed
rounded invocation count: `crates/fe2o3-kir-sim/src/execute.rs` increments
`scheduled_slots` for all 256 local slots before excluding those outside
grid[4,1,1]. Only four invocation machines are created; the barrier-free vecadd
completes each after one `schedule.selected` call. `schedule.rs` defaults to
`workgroup_major_local_zyx_cooperative_v1` and records four decisions, one
workgroup and zero barrier releases. `model.rs::SimulationRequestV1::new`
disables event delivery, so the ordinary CLI result has `events_emitted: 0`.
Debugger capture is separate and does not change that standalone request.
The actual step total remains a positive integer within this driver's 65,536
selected-profile cap; no exact source/MIR step count is invented.

The result shape matches `fe2o3-kir-sim-cli/src/linux.rs::write_success` for
the actual command without `--race-evidence`: all ordinary mandatory fields
are required; unknown fields and extra authority claims refuse. The writer's
optional `race_assessment` is neither required nor accepted for this command.
The normal live inspection still supplies the exact canonical digest and byte
count join; the byte count is not inferred from the bundle file size.

The explicit-uninitialized standalone simulation must exit one, produce no
stdout, and emit the actual bounded execution_uninitialized_read error document
with stage execution and the real invocation and site. The current public
document does not expose a typed allocation/range for this error. Its message
is retained unchanged and is not parsed to invent that missing structure.

The standalone simulation and debugger are distinct executions with the exact
same pinned bundle and explicit-uninitialized request. Their observations are
not relabeled as one common captured event. In particular, the standalone
error's raw block ID is not equated with a debugger canonical block ordinal.

## Exact public debugger navigation and joins

The driver requests the existing logical wave width 32. Scope interpretation is
logical visualization, not hardware execution or a claim about physical waves.

1. Step forward one operation. Require a genuine captured, source-resolved
   checkpoint in logical lane/workitem zero; retain the original response.
2. Query the current allocation inventory using its exact snapshot anchor.
   Require exactly three complete global allocations of capacities 16,16,24,
   observed generation zero and existing lifetime/owner/base not-represented
   fields. Discover their actual ordinals; never assume ordinal == argument.
3. Continue within 65,536 events to the actual generic Fault/Failed terminal.
   Require snapshot unavailable/not_captured. At this exact terminal session,
   query stack, all SSA values, source V2 frame one, and a previously observed
   16-byte allocation window. All must explicitly refuse values.
4. Reverse one operation to the actual preceding captured checkpoint. Its
   canonical function/block/operation coordinate must select one load in the
   complete ordinary author-inspection roster. A complete one-frame stack must
   identify that same before-load next_operation, not the previous after-op.
5. At this same stopped session query the complete SSA values, exact allocation
   inventory, source V2 pages of two rows, and all three complete small memory
   windows. Every response retains its exact snapshot/session and original raw
   JSONL line. SSA must match the control snapshot. Memory must match the entire
   explicit-uninitialized request model by exact bytes, mask, access and
   capacity, with no output writes. This finite byte matching is not a guessed
   compiler argument/variable-to-allocation map.
6. Forward one operation returns to generic Fault/Failed with unavailable
   values again. Reverse one operation repeats the actual same prior event,
   source/KIR site, scope, stack, full SSA rows and memory. Revisions are new;
   original anchors are retained, not rewritten or made equal.
7. At the repeated prior checkpoint, an actual SSA query with the old revision
   must fail stale_revision and preserve every session field.
8. Forward one operation returns to the same uncaptured terminal. Continue once
   at that terminal must give Completed/Failed, not successful completion.
   Zero events advanced is expected because the visible stop changes while
   the cursor does not. Values remain unavailable. Finally terminate cleanly.

The actual first stop/event numbers, allocation ordinals, load coordinate,
source-variable rows and opaque cursor identities are discovered from the
current returned records. The driver never hardcodes an old transcript's IDs.
Request IDs in the new session are strictly increasing and the complete
request/response stream is retained and parsed without line reconstruction.

At the terminal, stack/SSA/memory must be unavailable with not_captured and no
state change. Source V2 checks source-map/variable availability before looking
up a captured checkpoint, so its truthful allowed refusal is one of
checkpoint_not_captured, source_map_v2_required, or variables_not_captured.
No terminal source values are accepted under any of these states.

At the preceding checkpoint, source rows are retained if queried successfully.
The explicit frame-one/occurrence-one snapshot is a refinement of the original
unframed checkpoint anchor, not literally the same anchor. Its frame identifies
static stack depth; occurrence one is not a dynamic activation identity.
A current source producer refusal stays a separate unavailable result, never
filled in from SSA. A queried empty list is retained as an empty list.
No name/value heuristic links source variables to SSA, arguments, or memory.
Rows marked unrepresented remain unrepresented.

Source locations retain the compiler's opaque map/file identities and byte
ranges. They are not advertised as authenticated source hashes. The unchanged
ordinary source files, bundle, selected compiler/tool scripts and full raw
transcript are hash-pinned independently; the integrator provides the full
source census.

## Producer limits this does not close

The simulator detects uninitializedness before emitting a successful read
event. Current public debug V2 structured diagnosis does not include this
failure kind, and the terminal fault has no captured operation stack/memory.
Thus the capture can establish:

- a typed standalone explicit-uninitialized-input diagnostic;
- generic Fault/Failed navigation on the same pinned input model;
- genuine earlier operation/source/SSA/allocation observations and replay;
- explicit terminal unavailability, unchanged-session refusals, and no fake
  terminal values.

It cannot establish a captured failed-read descriptor, allocation-relative
terminal bytes, source variables at the terminal, physical registers, an
allocation lifetime transition/reuse generation, or a dynamic frame activation.
Those owner-produced transport/lifecycle gaps remain separate work under
[#215](https://github.com/harsh-nod/fe2o3/issues/215) and
[#216](https://github.com/harsh-nod/fe2o3/issues/216). This observation route
changes neither producer nor schema and does not close broad
[#281 V2](https://github.com/harsh-nod/fe2o3/issues/281).

## Bounds, retention, and qualification ownership

Observation output is cooperatively limited to 64 MiB, including the <=4 MiB
bundle, bounded stage streams, requests, responses, observations and receipt.
The debugger has <=128 request/response pairs, <=64 KiB per JSONL line, <=256 KiB
requests, <=8 MiB responses, and <=64 KiB stderr. All finite records use bounded
JSON parsing. Source query pages are <=32 with <=64 total rows; SSA is <=64 rows;
the three windows total just 56 bytes. The underlying existing debugger capture
limits are unchanged; these selected-query limits do not reduce or replace
the producer's internal record/value/byte caps.

The complete capture has a cooperative five-minute deadline: exporter <=180s,
other stages <=30s each, debugger <=90s, reply <=15s, stream drain <=10s, all
clipped to the remaining overall deadline. No stage count exceeds 16.
The inherited process helper supervises each spawned process group; the live
debugger also runs in its own group and is killed on a bound/guard failure.
The integrator's outer supervisor must independently enforce the whole deadline
and descendant/resource closure, including any abnormal undrained subprocess.

Each stage stream is <=1 MiB. Selected file pins total <=2 GiB and all hashed
input reads, including final selected-pin/stage readback, total <=4 GiB. Files
are read through no-follow descriptors with regular-file/size, name/descriptor
identity and final metadata/hash checks. Node, Cargo/rustc, four normal tools,
six existing/imported scripts and unchanged ordinary source/manifest/lock inputs
are selected pins. This does not replace the outer source/toolchain census.

The Cargo target is a separate fresh sibling, **not part of the 64 MiB
observation allowance**. The integrator must explicitly reserve and monitor it
against the current whole-task storage envelope and retain it on failure. The driver
pins its initial directory inode/device/mode identity but does not claim to
hash every changing compiler scratch artifact. The same 40 GiB available disk
and 64 GiB available RAM floors remain. Jobs is two. No filesystem quota,
resource relaxation, old target reuse, successful fallback, or cleanup occurs.

New outputs are create-new only. Stage streams are retained after each command;
raw JSONL chunks are retained as they arrive. failure.json records a bounded
explanation, partial transcript warning, stderr prefix and retained compiler
scratch path. Existing directories are refused, not emptied. Success needs the
outer supervisor's zero-exit result, no failure.json, completed fresh receipt,
unchanged closure/source pins and
independent review. A mere receipt file is not an authorization/proof artifact.

This capture adds no site importer or published fault fixture. The companion
[ordinary-source tutorial](https://github.com/harsh-nod/fe2o3-kernels/blob/main/docs/ordinary-source-fault-replay-v1.md)
explains the recorded semantics without a new browser control. No Phase22
UI/browser rerun is claimed. A future read-only adapter needs its own exact
profile and qualification; do not weaken existing resource, helper or
seven-pair watchpoint importers to accept this full session.

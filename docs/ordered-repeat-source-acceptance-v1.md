# Bounded ordered-repeat normal-source acceptance

Validate a complete registered Rust kernel that uses `init` and `repeat(N)`
instruction generation through the normal exporter, inspector and CPU simulator.
The driver checks fresh source admission, declared instruction order,
complete output buffers and exact frontend refusals; it never constructs a
replacement executable KIR owner.

Independent canonical and mirror captures each passed four exports, 120 CPU
simulations and eight frontend refusals. Each capture records 136 stages and
406 selected file pins. The 23 pure control groups also passed in each fork.
See the [dated qualification evidence](evidence/authoring-repeat-fault-20260922.md#final-publication-qualification)
for receipt identities, build provenance and the limits of those observations.

Implementation and controls:

- [Source acceptance driver](../scripts/ordered-repeat-source-smoke.mjs)
- [Pure controls](../scripts/tests/ordered-repeat-source-smoke.test.mjs)

## Normal route and precise boundary

The new source syntax expands one initialization block plus a literal number of
copies into the existing ordered-program count/four-packed-words marker. The
driver does not construct KIR, invoke a private compiler test callback, insert a
replacement executable owner, or lower through an independent instruction graph.
It uses only the normal exporter, normal program inspector, and normal CPU
simulator on fresh complete registered Rust kernels.

The supported normal export is **raw diagnostic canonical KIR V17**, not a
simulation Bundle V6. The exporter makes these formats mutually exclusive. The
receipt therefore explicitly reports Bundle identity unavailable; it records:

- immutable original and generated complete-source file SHA-256;
- the actual live exporter's semantic MIR V32, retained source preflight,
  retained root/contract inventory, and canonical KIR V17 identities;
- exact raw KIR file bytes and SHA-256, kept separate from the domain-separated
  canonical identity;
- the normal inspector's one retained ordered-program occurrence, current
  coordinate, source reference IDs, full descriptor array, full ordered list of
  declared instructions and literal register plan;
- every actual simulator request and complete result stdout/stderr.

Retained inventory is a root/contract census, **not a generic body digest**.
Changing a repetition count must change source, raw KIR, canonical KIR, semantic
MIR, source preflight, and retained statement reference observations. No arbitrary
cross-count inventory inequality is asserted. Repeating the identical fifteen-
copy source at the identical manifest/source path with a fresh extraction target
must reproduce the entire export/inspection identity set exactly, including the
inventory. Failed actual identity comparisons are retained failures, never fixed
by deleting identity checks or inventing semantic IDs from preflight.

Only declared register bindings and whole-region before/after logical behavior
are observed. This lane does not generate LLVM or HSACO or decode native
instructions. The separate [LLVM observation guide](ordered-repeat-llvm-observation-v1.md)
covers four fresh ordinary lowerings of the retained KIR. Neither lane establishes
hardware execution, physical register values/microsteps, lifetime or ABI proof,
protected/ranked proof, production resume or compiler/source closure
authentication. This is a bounded M2 compile-time
generation slice, not closure of general M2, U2 proof invalidation, runtime loops,
author-controlled branches, scheduling, or helper clobber contracts.

## Prerequisites and normal command contract

Use a checkout containing the ordered-repeat macro and checked expansion helper,
run the package's repeat and existing flat-syntax checks, and build the **same
current** normal exporter, sibling extractor, program inspector, simulator and
compiler DSO. A stale tool directory must not be adopted merely because these
files exist. Record an independent frozen source, dependency, toolchain and
runtime input census before execution.

Use one canonical repository and its matching tools. Do not combine dependency
graphs from different shared Cargo targets or use a test binary as positive
publication authority. This runner's exporter calls use fresh per-export target
directories. No seed consumer, private DSO-loading host client, or native worker
is needed here.

The following files must be present in that checkout:

- crates/fe2o3-device/src/ordered_program_repeat_v1.rs
- its reviewed module/macro hook in crates/fe2o3-device/src/ordered_program.rs
- crates/fe2o3-device/tests/fixtures/ordered-repeat-v1/source-{1,2,15}.rs

The source hashes are deliberately fixed to the reviewed complete fixture bytes:

| Copies | Bytes | SHA-256 |
| --- | ---: | --- |
| 1 | 662 | fa7634a5a1bc841db4b2a8ed5240b0184dfce8c79259fad6e04e60c66f5d08af |
| 2 | 662 | a8bd4ddbb76a6e59871f06ce4b3ee958b7b24b6a7cfd95ca0e804c094e3351f1 |
| 15 | 663 | d1d3f3812f459be5583c377c2a1d1690a85b24abb483555ed53b6150453f55c3 |

All other bytes must match the one-copy source. The runner independently checks
the exact one-literal replacements, preserves all three originals, and copies
the existing assembly-authoring-v30 Cargo.lock unchanged. Its Cargo.toml is the
normal template with only the two dependency paths replaced exactly once by
canonical repository paths. The existing crate name stays unchanged; the new
source's explicitly declared kernel selector is ordered_repeat_u32. Function,
block, operation and SSA coordinates come from the normal inspector, never
hard-coded function ordinals.

Reproduction commands (replace all placeholder paths with the matching inputs):

~~~sh
node /CANONICAL_REPO/scripts/tests/ordered-repeat-source-smoke.test.mjs

node /CANONICAL_REPO/scripts/ordered-repeat-source-smoke.mjs \
  --repo /CANONICAL_REPO \
  --bin-dir /MATCHING_NORMAL_TOOL_DIRECTORY \
  --cargo /EXACT_RUST_TOOLCHAIN/bin/cargo \
  --rustc /EXACT_RUST_TOOLCHAIN/bin/rustc \
  --output /CURRENT_SCOPED_ROOT/logs/NEW_ORDERED_REPEAT_OUTPUT
~~~

The output leaf must not exist, and its canonical parent must exist. It must be
outside the repository and tool directory and must not contain any selected
input. All output writes use create-new, no overwrite. Failure leaves are kept;
the runner does not delete or roll back source or target directories.

Each normal export is:

~~~text
fe2o3-export-sim --diagnostic-kir-v17
  --crate fe2o3_assembly_authoring_v30_fixture
  --output NEW_OUTPUT/<label>.kir --target gfx942
  --target-dir NEW_OUTPUT/<label>-extraction
  -- --manifest-path NEW_OUTPUT/<variant>-source/Cargo.toml --lib --offline
~~~

The exporter itself forces cargo check --locked -Zbuild-std=core and the exact
AMDGPU target/feature profile; this driver does not duplicate or weaken --locked.
It supplies canonical CARGO/RUSTC, jobs2, incremental0, offline=true, color=never
and a matching toolchain-library/tool directory loader path, removing ambient
FE2O3_* variables, Rust flags and all rustc wrapper variants. Outer supervision
must still freeze Cargo configuration, cache, source and runtime dependencies;
selected pins are not a complete compiler closure or sandbox.

Normal inspection:

~~~text
fe2o3-program-inspect NEW_OUTPUT/<label>.kir NEW_OUTPUT/<label>-inspect-request.json
~~~

Normal simulation:

~~~text
fe2o3-kir-sim --diagnostic-kir-v17 NEW_OUTPUT/<label>.kir --request NEW_REQUEST.json
~~~

No emitter, native worker, launcher, device, private API or hand-written KIR is
used. The inspector's structural profile already rejects extra functions, roots
or ordered programs and validates that the single function is the kernel entry.

## Positive qualification and independent arithmetic

The ordered sources all declare scratch32/out33/input34,input35,input36, wave64
gfx942:xnack- and exact required/maximum 64x1x1 launch. Each executes:

~~~text
init { mov(out, input0); }
repeat(N) { add(out, out, input1); }
~~~

The expected flat source descriptors are mov=8, then N copies of add=201, then
zero padding to exactly sixteen descriptors. N1/N2/N15 must expose 2/3/16
instructions respectively, in the exact authored order, with the exact role
bindings. The high-water declaration is 37. These are source descriptors and
declared instruction text, **not native machine encodings or observed registers**.

Four exports are performed: one, two, fifteen, repeat. The repeat reuses the
fifteen source and manifest paths without modification but uses a new KIR path
and fresh extraction target. Every variant is inspected once with one full
64-lane workgroup request.

Each variant gets five independent scalar triples, lengths 0/1/65, and two
replays: 30 actual simulations per variant, **120 total**. Length 65 launches two
64-lane workgroups. The oracle uses BigInt(a) + BigInt(N)*BigInt(b), reduced
modulo2^32, not a descriptor interpreter or exported instruction evaluation.
Inputs include all-ones, wraparound, alternating bits, high-bit overflow, and
19/23/42. The third scalar remains part of the exact ABI request but is not
used by the independent formula.

Every complete backing starts at 0xa5 and every initialization bit starts clear.
The output view starts four bytes into that backing. All output words, every
backing byte, every initialized/uninitialized/padding bit, and both four-byte
canaries are checked. Even the zero-length case must preserve all eight canary
bytes and all initialization bits. Result identities, exact arguments, launch
counts, workgroup counts, the exact target profile, complete schedule coverage
and observation-only/no-hardware flags must agree. The validator follows
`crates/fe2o3-kir-sim-cli/src/linux.rs::write_success`: exact ordinary top-level
fields; exact seven count fields; exact schedule identity/transcript/coverage;
and the complete selected-run `no_conflicts_observed` record. Scheduled slots
and decisions equal the launched invocation count, workgroups match the request,
barrier releases and emitted events are zero. The actual step count must be a
positive integer within the normal CLI's 2^27-step budget; no unobserved MIR
step total is invented.

The writer's optional `race_assessment` exists only with `--race-evidence`.
This driver never supplies that option, so it neither requires nor accepts that
extra field. Unknown fields, including fabricated authority claims, refuse.
Selected-run conflict observations are not a general race proof. Schedule
transcript retention is existing simulator behavior, not a newly authored
schedule or runtime loop.

## Exact fresh frontend refusals

Eight additional fresh registered sources are derived by exactly one bounded
byte replacement in the complete one-copy kernel. The plain refuse-*.rs host
fixtures are **not** exported: their absent kernel would be an unrelated failure.

| Case | Exact intended cause |
| --- | --- |
| zero | const evaluation: ordered repeat count must be 1..15 |
| sixteen | same closed count guard, not an unchecked expanded loop |
| huge | usize64 maximum literal; same pre-multiplication count guard |
| expanded-seventeen | one init + 8 copies of two additions exceeds 16 |
| dynamic | literal-only macro syntax rejects repeat(a) |
| bad-init | initializing scratch does not initialize the first read of out |
| nested | nonnested macro grammar refuses an inner repetition |
| physical-alias | the existing semantic owner rejects scratch33/out33 aliasing |

The last is a genuine physical resource check in the existing source owner, not
a read-only destination spelling test renamed as register rejection. Huge-count
refusal demonstrates pre-arithmetic bounding, not an actual overflow execution.

Negative exports append --message-format=json as normal Cargo passthrough.
The validator requires:

1. Healthy bounded process transport, exact exporter exit 1, no signal, timeout,
   stream cap, spawn/pipe failure, low-resource interruption or ICE.
2. Exactly one failed terminal Cargo build-finished record and the exact outer
   Cargo exit 101 diagnostic; no published KIR leaf, including a dangling symlink.
3. For macro/const failures, one primary typed compiler-message error bound to
   the exact generated manifest, src/lib.rs and crate name. Const errors must be
   E0080 with the exact evaluation-panicked message; macro errors must have no
   code and the exact compile_error message. Rendered source quotations never
   establish the cause. Primary const-error spans may legitimately point to the
   actual checked device helper rather than the generated call site.
4. For physical alias, zero unrelated Rust errors and the exact first existing
   source-owner extraction diagnostic, including its full current prefix.
5. No successful export/semantic identity line, no unrelated extraction error,
   and no silent fallback or reuse of a prior good KIR.

Both retained source captures exercised these exact refusal causes. A future
frontend that emits a different message or cascade must leave a retained failure
for review, not silently broaden the accepted diagnostic. A timeout or missing
crate must never be accepted just because a desired phrase appears in a quoted
source or warning. Do not weaken normal compiler predicates.

## Bounds and custody

The driver makes exactly 136 sequential top-level subprocess calls on success:
4 exports +4 inspections +120 simulations +8 negative exports. No concurrency is
introduced; Cargo has jobs 2. Nested Cargo/rustc processes still need the outer
supervisor's process-group, aggregate resource and exact runtime controls.

- Export command timeout 300s; inspector/simulator timeout 60s.
- Cooperative whole-run deadline 19min; use a hard outer deadline of 20min,
  leaving one minute for stop/report/outer retention. The cooperative
  check is not a hard real-time guarantee; synchronous I/O can delay it.
- One MiB each stdout/stderr, bounded input transport, detached process group;
  timeout/cap/guard kills the group and cannot qualify a refusal.
- Source and raw KIR ≤64KiB; parsed JSON and each receipt ≤1MiB.
- At most 256 negative Cargo JSON messages, duplicate-key/UTF8/depth/collection
  checks inherited from the existing bounded parser.
- At most 512 selected file pins, each ≤512MiB and aggregate selected bytes≤2GiB.
  This is not an RSS or total build-storage limit.
- Rechecking each executable before/after each child rereads it. The fixed 136
  stages and 512MiB file cap bound that repeated input I/O by 136GiB, plus at
  most 4GiB for initial/final selected pins and bounded generated-stream reads;
  this is intentionally distinct from the 2GiB unique selected-byte budget.
- Existing disk-reserve guard plus 64GiB available-RAM reserve checked before work
  and cooperatively during children. Current task-root storage authority and
  all build-directory growth remain external to this runner.

Canonical, non-symlink, regular selected files are read via no-follow/nonblocking
file descriptors. Before/after device/inode/mode/link-count/size/mtime/ctime and
SHA-256 must agree, including the final named file identity. Node, Cargo, rustc,
the tool/extractor/DSO set, imported runner dependencies, selected production
source files, source fixtures/manifests/locks, generated sources, KIR, requests,
and all command stdout/stderr are retained/pinned. The directly invoked
executable is checked again before and after each child, and all selected pins
are reread before the success receipt. Original inputs are never rewritten.

The success receipt includes the complete ordered simulation summaries and raw
stream hashes/paths through stages plus a selected file ledger. The receipt does
not hash or authenticate itself; pin it independently in the outer run record.
Create-new failure.json preserves failure and completed observations where bounds/storage
permit. Neither receipt offers source authentication or turns a diagnostic file
into resume authority.

## Controls and qualification

The test leaf has 23 synthetic groups. It exercises exact source changes, all
eight registered-source mutations, independent wrapping arithmetic, full
descriptor/order/resource checks, 120 synthetic complete-buffer observations,
canaries/initialization drift, current identities, complete/repeated matrices,
inventory semantics, the complete actual no-race-result schema, required field
omissions, unknown authority/optional-field injection, exact counters/schedules
and conflict observations, exact diagnostic/custody binding, stale/quoted/unrelated
errors, timeout/signal/cap refusal, strict JSON, CLI paths, fixed stage bounds and
the inner/outer deadline margin.
These are pure validators and synthetic data constructors; they do not run
Cargo, rustc, exporters or simulation.

The [dated evidence](evidence/authoring-repeat-fault-20260922.md#final-publication-qualification) records the
passed canonical and mirror captures and their surrounding build/control gates.
For a new checkout, rerun package controls, build matching normal tools, execute
the complete capture and revalidate source/tool/raw-output pins under a fresh
outer resource census. A previous receipt cannot qualify changed inputs.

This finite source profile does not complete the general assembly-authoring
milestone, establish native correctness or supply protected proof authority.

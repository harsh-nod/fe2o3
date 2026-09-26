# Shared Domain Planner Development

Base: signed `7ff017392840356feae110e674a771b3b3a2fe6c`.
This packet qualifies a production-used pure reservation planner, not the
complete accounting hierarchy or HIP/HSA parity. Read-only agent reviews
checked production/error-order correspondence and proof-controller lifecycle;
the primary agent owns edits, qualification and integration.

## Production Change

The old domain adapter invoked the R70 batch planner separately for each
ancestor. R75 visits the same leaf-to-root facts in the same error order, scans
all member charges once at the leaf, derives their exact accepted aggregate,
then checks each parent with one vector reservation. All nineteen dimensions,
three occupied-record phases, count bounds, owner arithmetic, inactive plan
entries and independent per-member tokens remain explicit. Work changes from
`O(members * depth * 19)` to `O((members + depth) * 19)` with fixed local storage
and no new heap allocation or roster copy. Scalar calls skip aggregation.

The adapter preserves its earlier global prechecks, validates the actual path,
copies raw node facts under the existing mutex, calls the planner, checks exact
free slots and commits the staged counts/usage/owner. Raw fact extraction does
not validate a parent counter before a leaf error. Tests compare complete
success/error results against the frozen prior leaf-first algorithm, exercise
all dimensions and ancestors, arithmetic boundaries, competing failures,
profile/depth/count bounds, poisoned inactive padding and 4,096 mixed cases.
An independent u128 oracle checks scalar reserve/release so the differential
oracle does not merely agree through a shared arithmetic defect.

Actual adapter tests cover twelve competing-failure modes and full raw-state
snapshots, unchanged output tokens, success at depths one through four, exact
member charges/owner order, ancestor debits, sibling framing and independent
out-of-order releases. Poison and its retention anchor are deliberately outside
the failure snapshot equality. Fault injection is CPU-only internal corruption,
not demonstrated public-API reachability.

## Formal Scope

Rust and Verus include the same vector declarations, checked add/subtract and
domain declarations/planner bodies. The final authenticated campaign is
[`raw/campaign-3`](raw/campaign-3). Verus
`0.2026.08.09.92f466f`, `--no-cheating`, two verifier threads and unchanged
resource defaults produce **19 verified, zero errors**, both before and after
fifteen semantic body mutations. The proof establishes exact active-ancestor
usage, reserved counts, owner advancement, capacity bounds, zero inactive plan
suffix and the exact first error, including overflow and invalid headers.

Mutations omit/reverse ancestors, omit the last member or a record phase,
change header/record errors, corrupt the accepted sum or parent charge, skip
owner/count/usage commits, change scalar coordinates, and omit the final
coordinate. Each negative has exit 1, a complete nineteen-obligation result
roster and only recognized logical proof failures. Compiler errors, timeouts,
resource limits, crashes and unrelated diagnostics cannot count as rejection.

The source gate fixes fourteen production/wrapper/proof/build inputs and the
exact five-file executable proof include closure. A unique hook-free Rust
invocation and reviewed proof bytes prevent arbitrary executable hook
substitution. Inputs, controller, commands, process-group identities, exits and
logs are captured. Before/after checks authenticate the 190-file Verus release
closure (129,019,839 bytes). Nine controller tests exercise source/manifest
drift, missing inputs, exact positive/negative classification, mutation identity,
symlinks, interruption after PID publication, interruption during spawn handoff,
and an exited leader with a TERM-ignoring descendant. The Linux single-threaded
runner owns a new process group, blocks signals during handoff, adopts/reaps
same-group descendants and checks group absence after termination escalation.

The outer shell wrapper authenticates the captured Python checker and tests
before importing them. Its own reviewed source is an outer trust boundary,
bound by this signed source change and equal before/after receipts at SHA-256
`7c90ab7d9865be8e335e1fc5e45b41285b4bea40ec0ca9ef14e3c3b01f07b25b`.
Neither a self-check nor the raw packet alone establishes independent trust.

**Not proved:** actual State/path/key extraction, global record correspondence,
free-slot/bitmap/token commit, mutex/poison behavior, retirement, native costs or
disposal, whole-profile root construction, compiler/ISA correspondence or
performance. Adapter source identity is not adapter refinement. Trusted Verus,
Rust compilation and hardware remain external boundaries.

Development attempts 1-11 retain environment/syntax/incomplete-proof failures;
attempt 12 is an earlier positive. Campaign 1 correctly rejected an
omit-last-member mutation that also introduced an arithmetic proof error. The
mutation was fixed to remain arithmetically valid; the classifier was not
weakened. Campaign 2 passed with the previous controller. Campaign 3 qualifies
the subsequent reviewed process-cleanup changes and added lifecycle tests.
No earlier failure is counted as final success.

Reproduction requires the pinned Verus release and a fresh absolute output:

```sh
sh crates/fe2o3-runtime-model/verus/verify-resource-domain.sh \
  /absolute/new-output /absolute/pinned-verus-release/verus
```

## Rust Validation

[`validate.sh`](validate.sh) runs four packages with locked/offline dependencies,
two test threads, debug information disabled and incremental compilation off.
Every command returned; the driver exits 1 because the broad library group has
four failures. The per-group exits are in `raw/final-status.tsv`.

| Check | Result |
| --- | --- |
| All-feature libraries | Model 1,034 passed/19 ignored; accounting 59 passed; KFD 1,321 passed/one failed/296 construction cases filtered; runtime 1,476 passed/three failed/28 ignored. Exit 101 |
| Selected construction/custody tests, same feature unification | 67 KFD and two runtime passed; exit 0 |
| All-feature doctests | 104 passed: 27 model, three accounting, 28 KFD, 46 runtime; exit 0 |
| Strict Clippy, all features/targets | Passed; exit 0 |
| No-default-feature check and workspace formatting | Both passed; exit 0 |

These groups overlap. The four failures remain KFD
`target_debug_telemetry_v2::tests::credential_bound_channel_accepts_typed_failure_before_publication`
at `SocketAdmission`, and runtime
`authorized_execution::tests::cooperative_debug_telemetry_emits_only_bounded_logical_records`,
`authorized_execution::tests::failed_session_end_is_explicit_and_terminal`, and
`authorized_execution::tests::pre_native_telemetry_failure_is_returned_and_poisoned`
at `InspectSocket(EPERM)`. Socket validation is unchanged. This is not full CPU
qualification; ignored native tests were not run by this suite.

The initial source snapshot precedes broad validation. Its final recheck detects
only the three intentional controller/test/runner edits; every Rust input is
unchanged. The final source snapshot and all five test/benchmark ELF checksums
pass rechecks. These receipts identify the local run, not a complete signed
native replay closure. Raw initial/expanded tests are also retained.

## Planner Microbenchmark

One release-mode executable compares the frozen previous planner algorithm to
the new shared planner with identical facts, charges and result checks. It runs
depths 1-4 and batch sizes 1, 2, 16, 1,024 and 65,536, warms both paths and
alternates paired order over nine rounds. Per round, repetitions are
`clamp(32768 / members, 2, 32768)`; inputs/results pass through `black_box`.
[`summarize_benchmark.py`](summarize_benchmark.py) checks all 180 unique cells
and reports paired old/new ratios as well as per-path median nanoseconds.

| Depth | Members | Old median ns | New median ns | Median paired old/new |
| --- | --- | --- | --- | --- |
| 1 | 1 | 147.88 | 106.64 | 1.390 |
| 4 | 1 | 443.99 | 221.10 | 2.008 |
| 4 | 1,024 | 191,600.88 | 27,377.09 | 6.928 |
| 4 | 65,536 | 12,411,586.50 | 1,812,779.00 | 7.154 |

All twenty cells have lower new medians. The full table, paired ranges and raw
samples are retained. This is a local AMD Ryzen AI MAX+ PRO 395/WSL2 development
measurement, not an exclusively reserved CPU or MI300X result. The baseline
uses the prior algorithm with the currently shared scalar functions, not an
entire base-commit binary. Depth-one improvements also reflect code generation
and function boundaries; the full ratio cannot be attributed solely to fewer
ancestor scans. The measured region excludes actual mutex/State fact extraction,
arena checks, tokens, allocation, native execution and HIP/HSA. It neither
establishes whole-account scalar latency nor orders-of-magnitude runtime parity.

```sh
CARGO_TARGET_DIR=/absolute/owned-target CARGO_PROFILE_TEST_DEBUG=0 \
CARGO_PROFILE_DEV_DEBUG=0 CARGO_INCREMENTAL=0 \
cargo test -p fe2o3-runtime-model --release --lib --locked --offline \
  r75_planner_microbenchmark -- --ignored --nocapture --test-threads=1
```

## Access And Remaining Work

The final MI300X probe fails resolving `sharkmi300x-1`, before any shared-host
resource is created. No new native or matched HIP/HSA result is added. The
owned local target is
`/home/harsh/.codex-tmp/fe2o3-domain-planner-target-20260926-VPTL29nZ`;
after all jobs returned and source/binary rechecks passed, its 639,504 KiB were
removed with `rm -r`. Independent `ls` reports ENOENT and `test ! -e` exits 0.
Cleanup receipts are retained; no unrelated directory was removed.

Next requirements include production path/arena/transition/retirement refinement,
logical/native account composition and complete root/bootstrap/resource closure,
native hierarchy qualification and matched performance. Worker V3, generated
execution, device-language, multi-device, atomic/collective and profiling exit
gates remain independent. No A1/A2 or accepted lane checkpoint is promoted.

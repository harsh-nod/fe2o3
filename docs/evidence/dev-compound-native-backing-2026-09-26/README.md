# Compound Native Backing Development

Development above signed `6ceedffa9adace4125d3d7a3847852fbbaf16820`.
This packet concerns a four-level root/device/session/class accounting profile
and compound N1/N2 admission. It is not native qualification, hierarchy proof,
whole-process memory closure, A1/A2 acceptance or HIP/HSA parity.

The [contract](../../runtime-compound-native-backing-v1.md) defines the public
profile and unchanged acceptance boundaries.

## Coverage

The generic coordinator tests preserve the legacy depth-three limit and reject
a fifth level in the explicit class profile. They cover scalar and batch
failure atomicity at all four ancestors, all nineteen vector coordinates,
record and owner/node generation exhaustion, independent batch member release,
retained-node reaping, sibling contention, quarantine and corruption/poison.

Private registry tests cover exact bootstrap payload, domain exhaustion at
session and both class-creation stages, no publication of a partially created
new device parent, immutable canonical device limits, generation reuse, unused
admission lifetime and mixed-root/session token rejection. These are not
fabricated checked Linux devices or native constructor execution.

The production `SharedMemoryEngine<FakeBackend>` adapters cover combined
session records, separate class byte/record limits, independent host/device
byte coordinates, root/device/session pressure before backend effects,
all-or-none configuration and immutable replacement. Twenty-four mixed
disposal cells inject errors and panics at N1's seven and N2's five boundaries.
They retain original debit, close both class allocation paths, preserve unwind
payloads and keep the typed registry through owner destruction. Wrong N2 refund
identity also preserves registry/quarantine custody. Clean disposal and final
owner destruction free the root normally.

Two model sessions on one canonical device contend for its shared capacity;
uncertainty in one retains that capacity without quarantining the healthy
sibling's phase. This is not supported native same-GPU multi-Context execution.
The existing real foundation-loan/pool-retag test now covers local, N1-rooted and
compound profiles in both startup orders, including N2 final disposal. The
compound phase path uses one session snapshot instead of two leaf snapshots;
this reduces coordinator lock acquisitions by inspection, not by a measured
latency or throughput result.

Runtime tests exercise consumed policy refusing both local budgets and generated
startup failing before acquisition. Source-routing assertions check all three
startup paths and ordered XGMI intake; they are not successful native startup
tests. Move-only endpoint helpers test admission order, unused-token cleanup,
exact error/panic preservation and first-acquisition rejection. Existing
second-acquisition subprocess tests preserve the abort boundary. Successfully
registered canonical parents remain permanent when later peer admission fails.

## Validation

All Cargo build/check/test commands use locked/offline dependencies and the
owned target `/home/harsh/.codex-tmp/fe2o3-compound-target-20260926-rtN43Yww`,
with dev/test debug information disabled and incremental compilation disabled.
Tests use two threads. The reproducible final command roster is
[`validate.sh`](validate.sh); it records each actual exit code and continues
through failed groups without converting failures into passes.

Initial focused validation passed 37 tests. Expanded validation passed 40:
27 KFD, seven accounting and six runtime. These overlap later library runs.
The first broad run (`raw/final-libraries.log`, before the last review changes)
had 52 accounting passes, 1,319 KFD passes with one socket-admission failure and
296 filtered construction tests, and 1,475 runtime passes with four failures
and 28 ignores. Its exit code is 101. The runtime failures include three
socket-inspection EPERM errors and one formatting-sensitive source assertion.
`raw/xgmi-routing-diagnostic.log` isolates the latter: rustfmt split the receiver
from the method name. The assertion now checks the existing whitespace-normalized
source, without changing its required entry or admission-before-VM ordering.

Before the final review changes, strict Clippy and formatting passed, and all
67 selected construction/custody regressions passed. Their original logs are
retained as `final-clippy.log`, `final-fmt-check.log` and
`selected-construction.log`; these names do not indicate final-source coverage.
The post-review validation is separate and includes the phase optimization,
symmetric N2 pressure and shared-device model-session tests.

The known socket failures are KFD
`target_debug_telemetry_v2::tests::credential_bound_channel_accepts_typed_failure_before_publication`
at `SocketAdmission`, and runtime
`authorized_execution::tests::cooperative_debug_telemetry_emits_only_bounded_logical_records`,
`authorized_execution::tests::failed_session_end_is_explicit_and_terminal`, and
`authorized_execution::tests::pre_native_telemetry_failure_is_returned_and_poisoned`
at `InspectSocket(EPERM)`. Production socket checks are not weakened.

### Post-Review Results

| Check | Result | Raw log |
| --- | --- | --- |
| All-feature libraries, three packages, same construction exclusion | Accounting: 52 passed. KFD: 1,321 passed, one socket failure, 296 filtered. Runtime: 1,476 passed, three socket failures, 28 ignored. Exit 101 | `post-review-libraries.log` |
| Expanded focused selection, including all XGMI-budget tests | 49 passed: 29 KFD, seven accounting, 13 runtime. Exit 0; overlaps broad coverage | `post-review-focused.log` |
| Selected KFD construction/custody regressions | 67 passed, 1,551 filtered; exit 0; overlaps broad coverage | `post-review-construction.log` |
| Strict Clippy, all features and targets, all three crates | Passed; exit 0 | `post-review-clippy.log` |
| All-feature doctests, all three crates | 77 passed: 28 KFD, three accounting, 46 runtime; exit 0 | `post-review-doctests.log` |
| No-default-feature checking, all three crates | Passed; exit 0 | `post-review-no-default.log` |
| Workspace formatting check | Passed; exit 0 | `post-review-fmt.log` |

The corrected source assertion and worker startup/deadline tests pass in the
post-review broad run. Only the four socket failures named above remain there;
this is not full CPU qualification. Every validation process terminated. The
driver exits 1 because the broad library group failed; individual results are
preserved in `post-review-status.tsv`.
`post-review-sources.sha256` records the source/manifests before that run, with a
passing checksum recheck; it supersedes the earlier source snapshot for
final-source identity. These local identity receipts are not signed native
replay or independent refinement proofs.
Final source and test-ELF checksum checks pass. The three-package library/test
ELFs and the separately built KFD constructor-selection ELF have separate hash
receipts because their dependency-feature unification differs. The same frozen
Rust sources are used throughout. Source/documentation whitespace checking and
validation-script syntax checking pass; raw tool logs retain original whitespace.

## Access And Cleanup

`raw/mi300x-access.log` records hostname resolution failure before reaching the
shared host. `origin-access.log` and `upstream-access.log` record failed remote
observations. No remote resource was created, and no GPU or performance result
is claimed. After all validation processes terminated and source/binary identity
checks passed, the exact owned local target was removed with `rm -r`.
`cleanup-before.log` records 692,852 KiB of allocated storage; the absence log
records ENOENT afterward and `test ! -e` returned 0. No unrelated directory was
removed.

## Open Work

Logical/native root composition, other backing/image/metadata profiles,
complete bootstrap and terminal headroom, whole-roster pre-effect native
creation, hierarchy/registry/native correspondence proofs, signed native
replay, broader execution/overlap and matched HIP/HSA measurements remain open.
No A1/A2, MEM-DOM/MEM-5 or accepted lane checkpoint is promoted by this packet.

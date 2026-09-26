# Rooted Native N1 Development

Developed above signed `09cf5c9e9a150f4e290bf710ae2cb3acf69aa28e` on
`codex/r65-runtime-drain-versions`. The
[contract](../../runtime-native-root-admission-v1.md) defines this scoped native
adapter integration. It is not complete MEM-DOM/MEM-5 closure, new formal proof,
native qualification, A1/A2 acceptance or full parity.

## Coverage

Private registry tests cover canonical UID/PCI reuse across model generations,
conflicting aliases/limits, shared device/root ceilings, concurrent registration,
domain exhaustion, exact generation binding and unused-admission lifetime.
New-parent rollback is tested when the parent fits but its session leaf does not.
They do not fabricate a checked Linux device.

The actual `SharedMemoryEngine<FakeBackend>` N1 adapter is tested for root,
device and leaf byte/record exhaustion before backend effects, exact 4,100-byte
request to 8,192-byte backing extraction, disposal refund and retry, wrong
generation and immutable replacement. Fourteen disposal error/panic boundary
cells preserve ancestor debit through session destruction. Weak observations
prove native owners retain the typed registry after external root Drop, clean
final disposal releases it, and uncertain Drop/failed identity refund anchor
the registry and quarantined debit. Existing foundation-loan/pool-retag tests
also execute with rooted accounts in both startup orders.

Runtime tests cover consumed rooted mode rejecting local replacement and
failing closed without fallback. A generated metadata-only fixture invokes the
actual startup helper with consumed admission, retaining inert control and
returning terminal before native acquisition. Error classification distinguishes
capacity from invalid binding. Source assertions check constructor admission
and all three startup routes before checked-device consumption; KFD routing checks
place identity validation before VM identity/attempt creation. These assertions
are not hardware execution or constructor refinement proofs. Existing native
single-VM admission restrictions are unchanged.

## Validation

Commands use the owned target
`/home/harsh/.codex-tmp/fe2o3-n1-root-target-20260926-iwYWe5E7`, locked/offline
Cargo, `CARGO_PROFILE_DEV_DEBUG=0`, `CARGO_PROFILE_TEST_DEBUG=0` and
`CARGO_INCREMENTAL=0`. Broad library tests use two threads.

### Before Final Review

| Check | Result | Raw log |
| --- | --- | --- |
| All-feature libraries, excluding KFD primary-construction integration | Accounting: 45 passed. KFD: 1,307 passed, 1 failed, 296 filtered. Runtime: 1,471 passed, 4 failed, 28 ignored. Command exit 101 | `qualified-libraries.log` |
| Rooted N1 KFD filter against final binary | 14 passed, 1,590 filtered; exit 0 | `qualified-rooted-kfd.log` |
| Rooted N1 runtime filter against final binary | 2 passed, 1,501 filtered; exit 0 | `qualified-rooted-runtime.log` |
| Isolated worker startup-timeout recheck | 1 passed, 1,502 filtered; exit 0 | `worker-startup-recheck.log` |
| Strict Clippy, all features and targets, all three crates | Passed; exit 0 | `clippy-qualified.log` |
| All-feature doctests, all three crates | 77 passed: 3 accounting, 28 KFD, 46 runtime; exit 0 | `doctests.log` |
| No-default-feature checking, all three crates | Passed; exit 0 | `default-check.log` |
| Workspace formatting check | Passed; exit 0 | `fmt-check.log` |

The runtime failures are
`authorized_execution::tests::cooperative_debug_telemetry_emits_only_bounded_logical_records`,
`authorized_execution::tests::failed_session_end_is_explicit_and_terminal`, and
`authorized_execution::tests::pre_native_telemetry_failure_is_returned_and_poisoned`,
all at `InspectSocket(EPERM)`, plus
`worker::tests::expired_worker_wait_is_pending_without_publishing_a_request`
at `StartupTimeout`. The latter passes in isolation against the same binary;
the original failed run remains failed. KFD fails
`target_debug_telemetry_v2::tests::credential_bound_channel_accepts_typed_failure_before_publication`
with `SocketAdmission`. This is not full CPU qualification. Production socket
validation and worker deadlines were not weakened.

The library command was `cargo test -p fe2o3-resource-accounting -p fe2o3-kfd -p
fe2o3-runtime --all-features --locked --offline --lib --no-fail-fast --
--test-threads=2 --skip queue::live::construction_primary::integration_tests`.
Clippy uses the same three packages with `--all-features --all-targets --
-D warnings`; doctests use `--all-features --doc`; checking uses
`--no-default-features`. All Cargo build/check/test commands also use
`--locked --offline` and the profile settings above. Formatting uses
`cargo fmt --all -- --check`.

The initial focused filters run `rooted_n1` against the pre-review binaries
`debug/deps/fe2o3_kfd-60551c78c3ce1a5d` and
`debug/deps/fe2o3_runtime-f40ec65f6abc2ea1`, with two test threads.
The worker recheck uses its exact full name and one test thread. These overlap
the broad run; counts must not be added as independent tests.

The initial all-feature runtime check passed. The first focused test build
found a Rust 2024 opaque-return lifetime capture in the Weak observer helper;
the helper now explicitly captures no borrowed lifetime. That failed log is
retained rather than overwritten.

The initial Clippy run found two collapsible conditionals. They were fixed;
its failed log and the intermediate pre-final test/check logs remain here.
The table above predates the final generated-route review correction. That
review found generated adoption still using the independent-account constructor.
It now consumes the root-issued admission before taking the checked device and
initializes DATA through the same rooted session retained by queue construction.
The source assertion includes this third startup path, and the consumed-policy
fixture exercises its actual early refusal. Final post-review results are
recorded separately below; intermediate failures are not overwritten.

`raw/mi300x-access.log` and `raw/mi300x-final-access.log` record hostname
resolution failure before reaching the shared host. No remote resources were
created. No GPU or performance result is claimed from the CPU fixtures or
source assertions.

### Final Results

The generated-route correction passed its actual consumed-admission regression.
The first post-review run exposed a formatting-sensitive source assertion:
rustfmt split `self.admitted_device.take()` across lines. The assertion now
normalizes whitespace without changing its required ordering or rooted branch.
`post-review-libraries.log` retains that run's 1,473 runtime passes, four
failures (the assertion plus the three socket failures), and 28 ignores;
`post-review-routing-diagnostic.log` isolates the assertion failure.

| Check | Result | Raw log |
| --- | --- | --- |
| Final all-feature libraries, same three packages and exclusions | Accounting: 45 passed. KFD: 1,307 passed, 1 failed, 296 filtered. Runtime: 1,474 passed, 3 failed, 28 ignored. Exit 101 | `final-libraries.log` |
| Final rooted N1 filters | KFD: 14 passed. Runtime: 4 passed. Both exit 0; overlap broad library coverage | `final-rooted-kfd.log`, `final-rooted-runtime.log` |
| Selected KFD construction/custody regressions | 67 passed, 1,537 filtered; exit 0; overlaps broad KFD coverage | `qualified-construction.log` |
| Final strict Clippy, all features and targets, all three crates | Passed; exit 0 | `final-clippy.log` |
| Post-review all-feature doctests, all three crates | 77 passed; exit 0 | `post-review-doctests.log` |
| Post-review no-default-feature checking, all three crates | Passed; exit 0 | `post-review-default-check.log` |
| Final workspace formatting check | Passed; exit 0 | `final-fmt-check.log` |

Only the four socket failures listed above remain in the final library run.
The worker deadline test passes there. The doctests and no-default-feature
check cover final production code; the only later source change was whitespace
normalization in the unit-test assertion. This is still not full CPU, native,
formal or performance qualification.

The construction selection runs the KFD binary with filters
`initial_bind_cases`, `replacement_cases`, `same_engine_auxiliary_`,
`construction_auxiliary::tests`, `backing_constructor_forwarding`,
`persistent_owner_and_queue_layouts`, and `recycled_detach`, with two threads.
KFD source did not change during the generated-route correction. The recorded
construction ELF digest matches the final KFD ELF digest; this selection is
not a successful whole-KFD run.

`final-sources.sha256` records manifests and the changed Rust sources before
the final library command; both subsequent source checks pass.
`final-binaries.sha256` and its passing check identify the final three test
ELFs before deletion. `construction-binary.sha256` and its passing check
identify the selected constructor-test binary. Toolchain versions and the base
commit are recorded alongside these receipts. These are local development
identity checks, not a sealed native replay or independent refinement proof.
Source/documentation diff whitespace checking passes. Raw tool logs preserve
their original trailing blank lines and SSH carriage returns, which the
unfiltered Git whitespace check reports; those logs were not rewritten.

## Cleanup

After every build/check/test process terminated, the exact owned target above
was removed. `raw/cleanup-before.log` records 874,620 KiB of allocated storage;
`raw/cleanup-absence.log` records ENOENT afterward, and `test ! -e` returned 0.
No unrelated local directory was removed and no remote resource was created.

## Open Work

Hierarchy/registry/native correspondence proofs, signed native replay,
remaining backing/control/image/bootstrap integration, complete bounded-profile
construction, broader same-device/multi-device execution and matched HIP/HSA
measurements remain open. Accepted milestones are unchanged.

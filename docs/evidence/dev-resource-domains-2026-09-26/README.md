# Shared Resource Domain Development

Developed on `codex/r65-runtime-drain-versions`, based on
`5d142eb384038a6dbf8fab6a41d57731f1f9a9eb`. The
[domain contract](../../runtime-resource-domains-v1.md) describes the implemented
ledger and Context request-admission boundaries. This is not sealed native
qualification, new hierarchy proof, MEM-DOM closure, A1/A2 acceptance or parity.

## Coverage

Core tests exercise scalar and independent batch-member lifetimes across one
root, complete 19-coordinate rejection at each of three levels, sibling and
cross-parent exhaustion, record/depth/node-generation/owner-generation limits,
unchanged admission state on rejection, retained-parent lifetime and clean
generation-changing slot reuse. Concurrent siblings contend for the final
credit. Equal-looking foreign roots stay independent. Stale owner/wrong leaf,
corrupt child preflight and corrupt global record totals poison without refunds.
Rooted host tables retain exact leaf identity and the fixed baseline through
clone/disposal. Token-only lifetime keeps the root alive; abandoned retained
tokens anchor quarantine after all external handles disappear.

Context tests use real admission/release paths with `MockBackend`, including
multiple Contexts, all ancestor byte/record limits, immutable attachment,
foreign devices, invalid/full/deep parents followed by valid retry, definite
rejection, failed release, terminal/panicking allocation and disposal, and
repeated Context creation after quarantine. Backend call counts distinguish
pre-entry rejection from accepted custody. The allocation-roster test calls
the production `prepare_roster` helper used by generated preparation, but does
not execute an end-to-end generated native launch.

These are CPU fixtures. Only core fake-resource anchors are explicitly removed
after inspection; production/native quarantined custody is not recovered.
The tests inspect charged baseline and storage lifetimes; they do not constitute
a global allocator instrumentation proof. No native hardware or performance
claim follows from these tests.

## Validation

Commands use the owned target
`/home/harsh/.codex-tmp/fe2o3-domain-target-20260926-0XHJJ27l`,
`CARGO_PROFILE_DEV_DEBUG=0`, `CARGO_PROFILE_TEST_DEBUG=0`, and
`CARGO_INCREMENTAL=0`. Cargo uses `--locked --offline`; broad library tests use
two threads. The completed post-compaction checks are:

| Check | Result | Raw log |
| --- | --- | --- |
| Accounting and runtime, all-feature libraries | Accounting: 45 passed. Runtime: 1,470 passed, 3 failed, 28 ignored; command exit 101 | `account-runtime-compact-final.log` |
| KFD all-feature library, excluding primary-construction integration | 1,293 passed, 1 failed, 296 filtered; exit 101 | `kfd-compact-final.log` |
| Selected KFD construction and custody regressions | 67 passed, 1,523 filtered; exit 0; overlaps the broad subset | `construction-compact.log` |
| Strict Clippy, all features and targets, all three crates | Passed; exit 0 | `clippy-compact-final.log` |
| All-feature doctests, all three crates | 77 passed: 3 accounting, 28 KFD, 46 runtime; exit 0 | `doctests-compact.log` |
| No-default-feature checking, all three crates | Passed; exit 0 | `default-compact.log` |
| Workspace formatting check | Passed; exit 0 | `fmt-compact-check.log` |

The runtime failures are `cooperative_debug_telemetry_emits_only_bounded_logical_records`,
`failed_session_end_is_explicit_and_terminal`, and
`pre_native_telemetry_failure_is_returned_and_poisoned`, all at
`InspectSocket(EPERM)`. KFD fails
`credential_bound_channel_accepts_typed_failure_before_publication` with
`SocketAdmission`. These failures remain failures: this is not full CPU
qualification. Production socket validation was not weakened.

The library commands were `cargo test -p fe2o3-resource-accounting -p
fe2o3-runtime --all-features --lib -- --test-threads=2` and `cargo test -p
fe2o3-kfd --all-features --lib -- --test-threads=2 --skip
queue::live::construction_primary::integration_tests`. Clippy uses
`--all-features --all-targets -- -D warnings`; doctests use
`--all-features --doc`; checking uses `--no-default-features`. Each also uses
the locked/offline and profile settings above. Formatting uses
`cargo fmt --all -- --check`.

The construction selection runs the final KFD test binary with filters
`initial_bind_cases`, `replacement_cases`, `same_engine_auxiliary_`,
`construction_auxiliary::tests`, `backing_constructor_forwarding`,
`persistent_owner_and_queue_layouts`, and `recycled_detach`, with two test
threads. It is additional scoped coverage, not a successful whole-KFD run.

The initial shared-account suite passed 38 tests. The initial domain filter
passed ten core tests and nine runtime tests, including four new Context tests
and five existing tests with matching names. Coverage was expanded afterward;
these are overlapping, intermediate results, not final totals.

`raw/socket-probe.json` reproduces EPERM for socket inspection prerequisites.
`raw/mi300x-access.log` and `raw/mi300x-final-access.log` record hostname resolution
failure before reaching the shared host. No remote resources were created.

The first KFD regression run caught an inline-layout regression: expanded
account handles made `HostMetadataTableV1<u64>` exceed its existing 64-byte limit.
The threshold was not relaxed. Tokens now carry the root plus exact record
identity, with the leaf retained in that preallocated record; tables no longer
duplicate the account handle. The owned account getter and native callers were
updated together. Token-only lifetimes, recovered handles and poisoned-table
cloning have dedicated regressions. Earlier logs are retained as intermediate
evidence; qualification must use the post-compaction results.

## Cleanup

After all test/check processes completed, the exact owned target above was
removed. `raw/cleanup-before.log` records 748,172 KiB of allocated storage;
`raw/cleanup-absence.log` independently records ENOENT afterward, and
`test ! -e` returned exit 0. Retained logs remain here. No unrelated local
directory was removed and no remote resource was created.

## Open Work

Mandatory persistent-root construction, canonical physical-device parents,
native session/VM binding, complete Context/native bootstrap, root participation
for backing/images/scaled tables, charged batch output and remaining metadata,
closed terminal payloads and hierarchy proofs remain open. The fixed Rust arena
payload excludes allocator/Arc internals. Existing R67/R70 proofs do not verify
the new hierarchy, and a freely creatable accounting root is not a process-global
memory bound. Native qualification and matched HIP/HSA baselines remain required.

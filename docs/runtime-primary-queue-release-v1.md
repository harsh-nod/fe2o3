# R126 Primary Queue Release

## Development Status

R125 remains the last qualified Native checkpoint at the local CPU/test boundary.
R126 is in development: borrowed foundation restoration, four-resource cleanup,
retained Linux platform teardown and the ordinary primary runtime integration
are implemented in the development worktree. The production ordering driver now
also runs against genuinely constructed fixture parents. Remaining fault-injection
and qualification work is listed below. A1/A2, issue #182 and full HIP/HSA parity
remain incomplete.

The development parent is R125 commit
`cc3721a4f027fba19e86a63d564370285e719971`. Its archived evidence describes that
source, not these subsequent edits.

## Implemented Prerequisites

`QueueModelOwnershipV1::restore_foundation` borrows the real queue foundation
until complete validation and certificate revocation succeed. Only then does
it swap the two foundations and commit session ownership. Failure preserves
both owners and their ownership phase. `ComputeAqlQueueSessionV1` no longer
replaces its authentic foundation with an empty placeholder before validation;
its `foundation_in_engine` flag changes only after success.

`ControlCleanupCustodyV1` supports two one-shot borrowed phases. Successful
unmap reaches `CleanupStageV1::Unmapped` only after the model commit; disposal
requires this exact state and rejects failures and repeated attempts. Each
phase retains its original tokens, native outcomes and terminal disposal
receipts. `release_v1` still invokes both phases consecutively, preserving the
existing dispatch-control and coherent-data ordering and quarantine policy.

The eight new tests cover foundation ownership and loan-generation retention,
invalid restoration, exact split-control ordering, invalid reentry, projection
errors/panics, intervening revision exhaustion, a panicking poison callback,
and release-currentness failures. The split fixture uses existing dispatch
controls, not the four actual primary-queue resources.

## Published Prerequisite Validation

These results describe `f0214ed5e9785f883a0a838d11fcd3befdb1ebcd`, not the
subsequent retained-primary integration.

- Full GNU KFD/runtime library tests: 1,252/744 passed, zero failed/ignored/filtered.
- Musl KFD shared-memory tests: 263 passed, zero failed/ignored, 989 filtered.
- Strict Clippy for both crates, all features and all targets: passed.
- Two bounded read-only source reviews found no correctness blockers.

Commands:

```sh
cargo test --locked --offline -p fe2o3-kfd -p fe2o3-runtime --all-features --lib
cargo test --locked --offline -p fe2o3-kfd --all-features --target x86_64-unknown-linux-musl --lib shared_memory::tests
cargo clippy --locked --offline -p fe2o3-kfd -p fe2o3-runtime --all-features --all-targets -- -D warnings
```

These are local development checks, not a full R126 qualification campaign.
No native GPU, formal implementation, total-memory or HIP/HSA performance
claim follows. No SSH work was needed.

## Retained Primary Development

`QueueResourceCleanupCustodyV1` owns the genuine typed AQL ring, USERPTR control,
EOP and context-save tokens in four fixed slots. All four GPU unmaps precede
all four disposals. Each slot retains its original identity, call progress and
disposed receipt; failure prevents reentry. USERPTR preserves Free before CPU
unmap and has no separate native VA-release call. Tests cover every native call,
currentness boundary, model commit, partial-unmap outcome and all nine positions
in the eight-revision certificate budget. These lower fixtures do not establish
whole-queue geometry or parent integration.

`LinuxPrimaryTeardownCustodyV1` retains event/runtime request records and exact
per-call outcomes, writable/zeroed/protected/unmapped payload progress, lease
release, the doorbell and pending shadow completion. Error/panic settlement
occurs after any runtime gate guard has left scope. Tests use real anonymous
payload/doorbell mappings but scripted event/runtime ioctls and a local gate.
No Linux destroyed-event authority is reconstructed from a cloneable test ID.

`PrimaryQueueReleaseCustodyV1` retains the parent session, platform owner,
returned resource authority, cleanup roots and teardown arm. DESTROY_QUEUE
progress retains its original request, attempt and raw returned arguments/status
before model observation or closing currentness. The adapter's status does not
provide a retained kernel errno. The runtime reserves and installs a boxed owner
before effects; failures keep it rooted, block reuse and preserve backing usage
observations. Missing primary owners are errors, never legacy-fallback decisions.
Only full resource, dispatch and signal cleanup confirms the gate and authorizes
the destroyed observation. Dropping an unfinished public owner aborts.

The selected route is ordinary primary AqlSpecial Release without SDMA,
auxiliary, debug-runtime or persistent attachments. Other profiles retain their
existing paths; they remain requirements, not qualified by this implementation.

Focused GNU development checks pass: six four-resource tests, six platform
tests, three retained-DESTROY engine tests and three primary-owner negative/Drop
tests. A separate `queue::tests` filter passes 53 tests, including the three
retained-DESTROY cases. The primary negative fixture has no native owners; its
subprocesses test abort semantics, not successful constructed-parent teardown.
These checks are not a fresh full-suite, musl, native or formal qualification.

## Constructed Parent Development

The private `PrimaryReleaseStateV1` driver owns teardown ordering for both the
public Linux root and the completed-constructor fixture. Its memory adapters
forward to real borrowed foundation restoration, four-resource cleanup,
ordinary dispatch cleanup and coherent signal cleanup. Platform adapters only
provide platform operations; they cannot replace memory cleanup with success.

The fixture consumes the original `CompletedPrimaryV1` engine, certified
foundation, submission/dependency/completion owners, dispatch, typed resource
authority and platform owners. Constructor DESTROY remains forbidden unless
explicitly armed after successful construction. The no-dispatch success case
is never-prepared, not a post-pristine-abort queue.

Nine constructed-parent test functions cover:

- Success with and without dispatch, original identities, zero host/device
  account usage, empty record indexes and gate-inert completed fixture Drop.
- Two genuinely constructed parents sharing the original local runtime gate;
  releasing the first lease does not disable the second parent's runtime.
- Failed/indeterminate/malformed DESTROY output and an original native panic.
- Exact event, payload, runtime and doorbell error/panic prefixes, using real
  payload VM operations and canonical event state shared by fixture clones.
- Real foreign-VM foundation restoration rejection after publication removal,
  retaining both exact foundations, ownership watermark and outer authority.
- Representative native four-resource failures, exact two-phase ordering,
  USERPTR-specific disposal, completed prefixes and untouched record suffixes.
- Native dispatch-control and signal failures, retained account charges,
  original panic payloads and an unconfirmed outer teardown gate.
- Opening/post-DESTROY/post-doorbell native currentness failures and full-state
  one-shot retry comparisons, including native records, model and accounting.

The local platform uses simulated event/runtime outcomes and doorbell metadata;
it does not issue KFD ioctls or mint Linux destroyed-event/disabled-runtime
authority. Separate lower Linux tests exercise real doorbell mappings and
scripted syscalls. Test-only mapping disposal occurs after assertions and is
not used as a successful-release oracle. Payload disposal no longer rewrites
a possibly protected page after a failed teardown prefix.

The reviewed unsafe inventory includes the nine retained-Linux teardown blocks
and previously omitted, separately reviewed committed sites in generated host
arguments/macros, completion/graph tests and adapters, copy-only examples,
the test allocator and the local constructor fixture. Inventory acceptance is
not formal refinement or native-execution evidence.

## Current Development Checks

Fresh full GNU library regressions pass 1,279 KFD tests and 744 runtime tests,
with zero failures, ignored tests or filtered tests in either suite. Both runs
include the final full-state retry comparisons. These are the two crate-library
suites, not a workspace-wide qualification campaign.

The constructed-parent filter passes all nine tests on GNU and musl, with
1,270 tests filtered out on each. The musl Linux-platform filter separately
passes 28 tests, with 1,251 filtered out. These are focused checks, not a full
musl regression campaign.

Strict Clippy for both runtime crates with all features and targets passes.
Formatting and diff-whitespace checks pass. The unsafe-source policy suite
passes five tests; its explicit inventory-refresh maintenance test is ignored.
Two bounded read-only reviews found no remaining blocking source defect within
their scopes, including a follow-up confirmation of the full-state retry oracle.
These source reviews are not independent qualification-archive acceptance.

```sh
cargo test --locked --offline -p fe2o3-kfd -p fe2o3-runtime --all-features --lib
cargo test --locked --offline -p fe2o3-kfd --all-features --lib integration_tests::release_cases::
cargo test --locked --offline -p fe2o3-kfd --all-features --target x86_64-unknown-linux-musl --lib integration_tests::release_cases::
cargo test --locked --offline -p fe2o3-kfd --all-features --target x86_64-unknown-linux-musl --lib queue_linux::
cargo clippy --locked --offline -p fe2o3-kfd -p fe2o3-runtime --all-features --all-targets -- -D warnings
cargo test --locked --offline -p cargo-fe2o3 --test unsafe_source_policy
```

The earlier focused GNU run predates the final retry-oracle comparisons; the
full GNU regressions, focused musl run and strict Clippy include them. R125
remains the accepted Native milestone. No SSH or native GPU execution was used
here.

## Remaining Qualification

1. Extend constructed-parent coverage to publication-return commit failure,
   dispatch-data failures, late memory/model prefixes and the post-pristine-abort
   entry state. The exhaustive lower matrices do not substitute for these joins.
2. Add direct post-DESTROY model-observation rejection and successful public-root
   Drop coverage with a genuinely completed parent. The negative unfinished-root
   subprocess checks and completed generic fixture Drop have distinct scopes.
3. Verify runtime retention, account observations, slot reuse rejection and
   destruction-profile events through the actual retained parent path.
4. Run fresh GNU/musl regressions, source gates, compiled negatives, checker
   calibrations and independent evidence review before R126 acceptance.
5. Qualify applicable additional queue profiles, native GPU execution, formal
   correspondence, aggregate-memory behavior and matched HIP/HSA performance.

The initial ordinary-primary profile does not remove other queue profiles from
the final objective. Generated DATA-ADOPT, ISSUE, completion/readback/typed
replies, Stop/drain/graphs, resource proofs and production Context integration
remain subsequent requirements.

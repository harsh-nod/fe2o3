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

The initial selected route was ordinary primary AqlSpecial Release without SDMA,
auxiliary, debug-runtime or persistent attachments. Other profiles retain their
existing paths; they remain requirements, not qualified by this implementation.

At that initial revision, the runtime facade branch was not reached by a
successful ordinary public native workload. Native `allocate_v1` calls
`ensure_sdma_queue_v1`, which creates
the primary and immediately attaches directional SDMA. Shutdown trims free
buffers but does not detach that SDMA owner, so profile selection takes the
legacy path. Nonempty fixed-dispatch data requires those allocations; empty data
is rejected, and opening/stream/module-only workflows leave the queue absent.
Retained SDMA teardown or a genuinely supported compute-only allocation path is
therefore an integration prerequisite. The direct queue probe below does not
close this runtime-facade reachability gap. The directional integration and
public runtime probe below now close that packetless allocation/shutdown gap.

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

## Published Integration Checks

The following results describe `43746b71889cc03778dc1b945a7d86788e6b5ccb`,
before the failure-path extension below.

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

## Failure-Path Extension

Seven additional constructed-parent test functions extend the shared-driver
coverage to sixteen functions:

- All four original dispatch-data entries, covering initialized/uninitialized
  device and coherent host inputs, every applicable native disposal call and
  currentness boundary, with errors and original panic payloads.
- Both coherent host-data entries at unmap/release projection and commit
  boundaries. Native disposal, accounting refund and model commit are observed
  separately; a disposed owner must be a terminal receipt, not a usable token.
- All four queue resources at those same four model boundaries, with exact
  native call order, committed model prefix and untouched dispatch/signal state.
- Actual publication-return commit rejection after its successful projection:
  a test-only hook exhausts the original certificate revision before replacement.
  The original publication model and resource authority remain in the engine.
- Actual post-DESTROY model rejection: a test-only hook deliberately corrupts
  only the observation's queue key. The real projector rejects the absent key;
  the original queue stays DestroyPending with its exact successful native receipt.
  This is corrupted-input coverage, not a claim that valid inputs naturally fail.
- Release after the original prepared dispatch has completed lower pristine
  abort and disposal of its original returned data, through a genuine foundation
  loan/reclaim. This fixture does not exercise the concrete live detached ledger.

Late session-owned model hooks inject errors/panics at actual boundaries. They
do not mint a replacement queue certificate or mislabel an injector panic as
certificate exhaustion. Retry comparisons now include the ordinary active-data
receipt, remaining-data cursor/storage, queue model and authority-poison state.
Successful release additionally checks zero reserved, retained and quarantined
account records and unpoisoned host/device accounts.

The [packetless MI300X probe](evidence/dev-r126-primary-release-native-2026-09-16/README.md)
passed on one explicit device. A genuine no-dispatch primary queue completed
retained teardown and normal process exit after explicitly dropping the concrete
public release root. It submitted zero packets and performed zero MMIO stores.
The exact executable and private staging directory were removed after confirmed
process closure. This closes the no-dispatch concrete successful-Drop example,
not native failure retention, runtime-facade integration or R126 acceptance.

Final focused extension checks pass: sixteen constructed-parent tests each on
GNU and musl (1,270 filtered out in each run); GNU shared-memory 269/269,
dispatch-control cleanup 60/60, queue engine 53/53 and example CLI 4/4. Strict
Clippy for both runtime crates, all features/targets, and formatting/whitespace
checks pass. The unchanged unsafe-source inventory passes five policy tests;
the explicit inventory-refresh maintenance test remains ignored. These focused
suites are not a fresh full workspace GNU/musl
qualification run. Earlier compile/oracle/lint failures remain development
history, not passing evidence; the final runs include the receipt-variant and
full retry assertions.

```sh
cargo test --locked --offline -p fe2o3-kfd --all-features --lib integration_tests::release_cases::
cargo test --locked --offline -p fe2o3-kfd --all-features --target x86_64-unknown-linux-musl --lib integration_tests::release_cases::
cargo test --locked --offline -p fe2o3-kfd --all-features --lib shared_memory::tests
cargo test --locked --offline -p fe2o3-kfd --all-features --lib queue::dispatch_binding::control_release::
cargo test --locked --offline -p fe2o3-kfd --all-features --lib queue::tests::
cargo test --locked --offline -p fe2o3-kfd --features live-validation --example kfd-compute-aql-queue
```

## Retained Directional SDMA Integration

The development implementation now admits an ordinary primary with its original
directional SDMA set. `DirectionalSdmaReleaseCustodyV1` retains the original vector,
both queue identities, in-place DESTROY arguments and raw outcomes, original
doorbells and three-token cleanup roots. Whole-roster preflight precedes the first
native operation. Teardown remains H2D then D2H before primary destruction;
resource release follows permanent foundation restoration, primary cleanup and
shadow completion, and precedes dispatch/signals. Successful completion accounts
for eleven resources. No owner is popped during fallible teardown.

The three-token profile shares the fixed-order cleanup driver with the existing
four-token primary profile. It preserves completion/control/ring unmaps before
any disposal, USERPTR free-before-CPU-unmap, exact terminal receipts and inert
re-entry. Its certified lower path preflights six revisions; the integrated
permanently restored foundation is not given a replacement certificate.

New tests cover real same-session mapped token owners, real anonymous doorbell
unmaps, both directional destroy/doorbell/currentness/topology failure prefixes,
malformed DESTROY outputs including mutation followed by panic, and bad
second-owner admission before any effect. Lower tests sweep eleven native calls,
eighteen currentness boundaries, projection/commit boundaries and exact revision
headroom, with full native-record and account snapshots. Constructed-parent
tests also fail inside the last native disposal call of either SDMA owner,
retaining the real partially released root, sibling, primary signals and gate.
CPU queue identities and ioctl results remain scripted; these are not
hardware-created CPU-test queues or formal correspondence proofs.

An opt-in ignored runtime test follows public open, stream creation, host
allocation, release, stream destruction and shutdown. Its test-only observer
records the real post-trim selector without changing routing or injecting a
queue. It checks completed-root/backend Drop, reuse rejection and the absence of
false compute-lane profile events. The native execution result is recorded
in the [passing one-device receipt](evidence/dev-r126-directional-release-native-2026-09-16/README.md),
separately from CPU tests. This closes the packetless public allocation-path
reachability example, not native fault qualification. That directional-release
packet did not cover pool trimming before retained-primary-root installation;
the subsequent pool-trim integration below addresses that ownership gap.

Final development library regressions pass 1,297 KFD tests and 744 runtime tests
on both GNU and musl. Each local runtime run ignores only the opt-in hardware
test; its separate native execution passes. The 21 constructed-parent tests are
included in the full KFD runs and also pass focused runs on both targets. Strict
all-feature/all-target Clippy and the reviewed unsafe-source policy pass. Two
bounded read-only reviews found no new blocking correctness issue in the
extension. These are two-crate library regressions, not full-workspace or formal
qualification, and do not advance the accepted R125 checkpoint.

The [development receipt](evidence/dev-r126-directional-release-native-2026-09-16/README.md)
retains source/executable identities and raw logs. An earlier intermediate GNU
run failed five self-spawn tests because a concurrent build replaced its live
executable; all five failed with `ENOENT`. Fresh full runs used private frozen
copies and passed. The earlier failed run remains separate history, not passing
evidence. Source/documentation formatting checks pass; raw logs preserve their
original libtest whitespace.

## Retained Pool Trim

The queue now installs `SdmaPoolTrimCustodyV1` before removing any free buffer.
The original free vector retains the untouched suffix, and each active buffer
moves into borrowed `DataCleanupCustodyV1` before the model loan or native
cleanup. Reverse order and one loan/retake per buffer are unchanged. An active
disposed receipt remains rooted until both cleanup and retake succeed; only
then is the completed count advanced. Full success clears the root, allowing
later independent trims. Failure rejects retries and prevents either teardown
route from treating an empty free vector as a drained pool. Dropping a queue
with unfinished trim custody aborts rather than silently discarding its owners.

The SDMA-specific lower conversion preserves owner, generation, logical extent,
original storage/layout and the original content-certificate box. It neither
uses the certificate-invalidating dispatch bridge nor invents fixed-dispatch
initialization authority. Decomposition performs no allocation. Fresh rejected
trim attempts still close pool configuration; terminal retries leave the
activity ledger unchanged. No second backing-byte ledger is introduced.

Runtime shutdown now terminalizes trim errors and panics while retaining the
original queue and account observers, then resumes the original panic payload.
The shared driver retains normal retake-error priority and lower-panic priority.
Six new genuine constructed-parent tests cover mixed host/device first/middle/
last failures, native and currentness boundaries, host projection/commit joins,
opening rejection, actual loan-generation/reclaim rejection, cleanup/retake
error/panic combinations, exact metadata and refunds, inert retry, repeated
success and subsequent directional teardown. The fixture constructs its valid
free roster directly; it is not public recycle-admission evidence.

Additional tests preserve the original certificate-box identity with actual
initialized host bytes, check the concrete queue Drop guard in a subprocess,
and verify runtime terminal settlement and panic identity. The runtime callback
test has no native queue and does not establish native failure/accounting behavior.

GNU/musl each pass all 27 constructed-parent release tests, 274 shared-memory
tests, six SDMA-cleanup tests and 745 runtime tests (one hardware test ignored
locally). Drop, activity-latch and pool-wiring checks pass on both targets;
strict all-target Clippy and the unchanged unsafe-source policy also pass.
A fresh MI300X public allocation/shutdown probe confirms a nonempty one-buffer,
4096-byte pool before successful retained shutdown and backend Drop. See the
[pool-trim development receipt](evidence/dev-r126-pool-trim-2026-09-16/README.md).
These are focused KFD and full runtime library checks, not fresh full-KFD or
workspace regressions, formal proof, native fault qualification or performance
evidence. R125 remains the accepted checkpoint.

## Late Cleanup And Detached Ledger Joins

The constructed primary driver now covers all 24 queue-resource currentness
boundaries, all six signal currentness boundaries, and all eight model stages
for each of the four queue resources and the final signal, with errors and
panics separately. Oracles check original identities/layouts, untouched suffixes,
native calls and receipts, model prefixes, account refunds, retained VA, lower
phase, terminal parent, retained gate and inert retry. Final currentness can
retain a fully disposed signal without a refund; release-commit failure can
retain the refund while the model remains only unmapped. Signal
`UnmapPreflight`/`UnmapEvidence` errors preserve the lower phase, while panics
quarantine it.

The post-pristine-abort test now installs the actual returned continuation and
identity ledger on its constructed parent. It closes the abort model loan
before returning data, then uses the shared detached-data release driver with
one genuine loan/retake per original owner. Production and the constructed
parent share the unchanged borrowed primary-admission predicate. Every partial
ledger rejects teardown; the fully settled ledger admits it. Count/identity
disagreement, recycled-generation substitution and an out-of-range insertion
index reject without changing memory, native calls, owners, continuation or
gate state. Final teardown refunds all backing and preserves the settled ledger.
This joins genuine CPU fixture owners and production drivers; it is not public
Linux forwarding or native fault evidence.

GNU/musl each pass 308 selected KFD tests, including all 30 constructed-parent
release tests, and 745 runtime library tests with one hardware test ignored.
Strict all-target Clippy, formatting and the unchanged unsafe-source policy
pass. Two read-only reviews checked the shared admission and exact failure
oracles. See the [late-release development receipt](evidence/dev-r126-late-release-2026-09-16/README.md).
These results close the listed late CPU joins, not R126 qualification.

## Native Dispatch, Accounting And Read Observation

Two opt-in, process-isolated MI300X probes now pass through public runtime
workflows: allocation/pool return without dispatch, and the exact admitted
typed gfx942 vecadd launch with three 4 MiB HostVisible allocations. The latter
checks every byte of both preserved inputs and the output, observes the retained
primary selector, and checks configured host accounting before release and
before completed-root Drop. Both probes observe 532,480 retained bytes in three
records fall to zero bytes/records, with no reserved, retained or quarantined
records, unchanged budgets and no poison. Backend Drop completes.

This is auxiliary compute followed by retained bootstrap-primary teardown,
not primary-executed teardown. Allocation creates the bootstrap primary without
assigning an ordinary compute lane; the typed launch currently creates auxiliary
ordinal 1. The probe asserts this routing and one exact matching
queue-created/published/completed/destroyed trace. Its three public readbacks
must each have the correct allocation identity, offset and byte length.

The expanded workflow checks exposed two concrete runtime defects. Repeated shutdown could
re-enter SDMA teardown after retiring its queue; it is now inert after the
existing terminal/busy checks, preserving partial multi-device shutdown retries.
Successful SDMA-backed host reads omitted their profiler events; the public read
now records exactly one event after complete download and staging cleanup,
using returned bytes rather than a potentially stale shadow digest. CPU tests
cover both storage/content modes, full/partial/empty ranges, chunked success,
and pre/post-mutation failures with no false successful read event.

GNU and musl runtime suites each pass 750 tests, with the two native probes
ignored locally and then run explicitly on MI300X. Strict Clippy, formatting
and the unchanged unsafe-source gate pass. See the
[native-dispatch development receipt](evidence/dev-r126-native-dispatch-2026-09-16/README.md)
for exact source, raw traces, regressions, checksums and cleanup. These results
do not qualify native failure retention, primary-executed dispatch, other queue
profiles, formal correspondence or matched performance. R125 remains accepted;
R126, A1/A2 and #182 remain open.

Fresh bootstrap-primary adoption required a distinct initial binding path. Existing
rebind APIs require a genuine recycled predecessor or pristine-abort continuation;
neither can be fabricated for a fresh queue. The next development packet below
implements that path without changing either continuation contract.

## Initial Primary Binding

`bind_initial_fixed_dispatch_v1` now binds the first ordinary fixed batch on the
existing unused primary. A bounded, pre-reserved root holds each returned data
owner before the next initializer call, preparation through real model retake,
and the original parent on admitted failure or panic. The initializer's captures
are destroyed inside the unwind boundary before validation/install. A successful
binding alone assigns the runtime's logical compute lane. Subsequent streams
still create auxiliary lanes; existing recycled and pristine-abort rebind paths
are unchanged.

The constructed-parent matrix checks materialized and preparation prefixes,
model loan/retake failures and exact precedence, genuine generation exhaustion
and revision regression, initializer-destructor panic, invalid fresh-state
ledgers, size bounds, terminal re-entry, original owner identities, authenticated
foundation placement and generation, host/device accounting and native record
partitions. These scripted CPU failures are not native fault-injection evidence.

The native single-stream probe now observes actual primary ordinal 0 with no
auxiliary queue. The two-stream probe observes primary 0 and auxiliary 1, with
both launches flushed before explicit waits, six complete input/output checks,
distinct stream/queue attribution and 41 complete observed profile events. The
single-stream profile has 22 events. All configured host backing charges refund
on shutdown and repeated shutdown is inert. These are exact-artifact gfx942
success workflows, not general generated-kernel admission or proof of physical
overlap. See the [initial-binding receipt](evidence/dev-r126-initial-primary-binding-2026-09-16/README.md)
for source, raw results and the retained failed experiment.

The initial two-stream experiment allocated the second stream's buffers while
the first dispatch was pending and received the existing Busy rejection for
changing SDMA ownership. The passing probe preallocates both sets of buffers;
it does not remove that restriction. At that checkpoint, older NEW/AUXILIARY
materializers and REBOUND materialization still lost callback-local successful
owner prefixes on later error/panic, NEW retained its local memory only on ordinary
error, and same-shape resident overwrite could lose its removed roster on failure.
These reviewed gaps required retained runtime custody and integrated fault tests;
they were not repaired by the primary-only driver. R125 remains accepted and
R126/A1/A2/#182 remain open.

## Runtime Materialization Custody

The subsequent runtime packet roots the original specification vector and every
returned native owner before the next initializer call. NEW installs its genuine
memory session in `terminal_memory` before initialization and transfers it only
after complete success. REBOUND uses the same prefix driver. Resident overwrite
roots both original vectors before any borrowed write; errors and panics retain
successful writes, partial current-item mutation and untouched suffixes without
rollback or retry. Capture and metadata destructors run inside the unwind guard
before native-owner transfer. Original errors and panic payloads are preserved.

Fourteen production-driver CPU tests cover these custody boundaries, capacity
rejection, vector storage, callback/metadata destruction and partial writes. The
materializer adds only its output-vector reservation; the overwrite driver makes
no additional allocations in the measured success cases. These are driver-level
allocation counts, not native throughput measurements or a total-memory bound.
GNU and musl runtime suites each pass 764 tests with four opt-in hardware tests
ignored locally. Strict Clippy, formatting, no-default-feature compilation and the
unsafe-source policy pass.

Three process-isolated MI300X budget-rejection cases exercise the public runtime
AUXILIARY route while primary work stays logically pending. Budgets of 37/41/45
MiB reject initialization after exactly 0/1/2 returned 4 MiB native owners. The
tests inspect the actual retained typed prefixes, exact pre-retake account
charges, preserved first execution identity, retained second-stream pending
request and unchanged queue/publication records. A repeated public flush is inert.
These cases retain resources until process exit; they do not demonstrate a
successful terminal shutdown, refund, native ioctl failure or physical overlap.
Allocation, primary dispatch and preallocated two-stream success probes also
pass on the new binary. See the
[materialization receipt](evidence/dev-r126-runtime-materialization-2026-09-16/README.md).

Pending-compute allocation remains blocked. Review identified a prerequisite:
fresh allocation can return an owner through `with_live_queue_memory_model`
before model retake succeeds, losing the returned authority on retake failure.
Fix that custody boundary before separating existing-SDMA-owner admission from
actual SDMA creation. HostVisible and DeviceLocal initialization need separate
noninterference/currentness qualification; existing copy and persistent-compute
alias restrictions must remain intact. R125 remains accepted; this packet does
not accept R126 or close A1/A2/#182.

## Fresh SDMA Allocation Custody

Fresh host/device allocation now installs a bounded queue-owned slot before the
real model loan and native allocation. The operation returns only unit through
model retake; the mapped buffer leaves the slot and increments the outstanding
ledger only after successful retake. Device allocation reuses the existing
borrowed-map root, retaining the unmapped lease on map failure instead of passing
it through the old consuming map helper. Host transition failures retain their
actual lower transition/pending owner; a completed host allocation remains in the
queue slot if retake fails. No success-path heap wrapper is added by this driver.

Healthy size/alignment, backing-credit and opening rejections clear only the
empty slot and preserve retryability. Quarantined lower state, returned owners,
retake failures and panics retain custody and seal the parent. Original errors,
retake-error precedence and first-panic identity survive secondary poison panics.
Device extent validation remains inside the original loan/retake boundary, and
rejected SDMA activity still closes pool configuration. Initial binding, trim,
retained/legacy teardown and Drop cannot discard an unfinished allocation slot.

Twelve focused tests cover constructed directional parents with configured
accounts, allocation/release at lengths 1/17/4097, real backing-credit rejection
and retry, native/currentness errors and panics, malformed/partial mapping,
opening and retake failures, generation exhaustion, exact error precedence,
terminal reentry and process-isolated Drop. This is CPU fault-injection evidence,
not a native failure campaign or executable/formal correspondence. The existing
six MI300X success/AUX budget-rejection regressions pass on the new binary. See
the [allocation receipt](evidence/dev-r126-sdma-allocation-2026-09-16/README.md).
Full KFD GNU/musl regressions each pass 1,326 tests; runtime GNU/musl each pass
764 with four opt-in hardware tests ignored locally. Strict Clippy, formatting,
no-default-feature compilation and unsafe-source checks pass. These crate-level
checks do not replace the remaining full R126 qualification campaign.

At that packet, pending-compute allocation remained disabled. Runtime HostVisible initialization
and upload staging now retain across borrowed-write failure as described below;
directional promotion now roots its input through validation and unwind. Device
initialization additionally needs retained synchronous submit/wait/retire
boundaries. Recycle/release retention is described below. Runtime allocation
error conversion also still terminalizes healthy backing-credit rejection;
a typed disposition must preserve the lower contract.
Only after those prerequisites should borrowed owner-roster preflight
separate reuse of existing directional SDMA from idle-only queue creation. R125
remains accepted; R126/A1/A2/#182 and HIP/HSA parity remain incomplete.

## Runtime Host Write Retention

Fresh HostVisible initialization and transient upload staging keep their owner
outside the borrowed-write unwind boundary, including the lower error-to-string
conversion. On error or panic the original buffer moves into the existing
terminal SDMA slot before poison or diagnostics; failed staging is never passed
to consuming recycle. Indexed ordinary and authenticated host writes retain their
buffer in the allocation record and now seal the backend on panic as well as error.
Non-formatting terminalization is shared with existing error paths. A secondary
poison panic or panic-payload destructor cannot replace the first write panic.

Nine focused tests cover errors/panics before any copy, after a partial prefix and
after the whole write, distinguishable fresh-allocation bytes, exact owner IDs,
unchanged allocation/shadow/accounting commits, certificate invalidation, later
upload and indexed/authenticated chunks, inert terminal retries, public-context
behavior, successful cleanup and process-isolated terminal Drop. GNU/musl runtime
regressions each pass 773 tests with four hardware tests ignored locally. The six
unchanged MI300X allocation/primary/AUX regressions pass on the new binary.
The [host-write receipt](evidence/dev-r126-host-write-2026-09-16/README.md) separates
scripted partial-write faults from native regression success.

An unconfigured public Context does not immediately set its own terminal marker
after a caller catches a backend panic; the backend is already terminal and the
next valid backend call seals the Context without native work. This packet does
not change that facade policy, admit pending-compute allocation, qualify native
partial writes, add executable/formal correspondence or establish performance.
Directional promotion, demotion, recycle/release, synchronous-copy custody and
typed capacity disposition are now covered below. Native owner-roster preflight
and pending-compute qualification remain open.

## Directional Promotion Custody

Promotion installs the original device buffer in the queue before borrowing the
live model. Validation uses the same mapped-record, domain and physical-extent
checks as before; only unit crosses model retake. Healthy rejection returns the
exact retryable buffer, terminal returned errors carry process-teardown custody,
and panic leaves the buffer rooted in the poisoned queue. Retake errors retain
precedence over ordinary validation errors; the first panic survives later
retake/poison panics. Conversion preserves the outstanding debit and pool
generation. Its existing persistent-owner `Rc` and ledger allocations remain;
allocator failure is not qualified by this change.

The runtime roots input before driver selection and seals itself on lower
promotion unwind. Native failures cross the adapter as typed errors without
formatting. Returned custody is installed before the complete diagnostic is
formatted; diagnostic panic retains it and seals the backend. A healthy retryable
failure still follows existing recycling and rejection semantics after formatting.
Recycle/release and demotion retention are now covered below; copy phase
transitions remain separate unfixed unwind boundaries.

Constructed tests use actual fresh mapped leases, the original directional pair,
shared production mapping checks and real foundation loan/reclaim. Coverage
includes healthy retry/refund, padded physical extents, foreign mapping, model
revision regression, validation/retake/poison failure combinations, inert retries
and process-isolated Drop. Scripted runtime tests additionally check original
owner/neighbor identities, shadow allocation identity, typed diagnostic unwind,
credit quarantine and public-context terminal behavior. These are not native
failure injection or formal implementation correspondence. Validation is recorded
in the [promotion receipt](evidence/dev-r126-sdma-promotion-2026-09-16/README.md).

## SDMA Recycle And Release Custody

The shared recycle/release driver installs a matching-owner buffer before
admission, policy validation or native work. Cache reservation failure returns
the exact unchanged healthy buffer; successful caching advances its generation
and removes the outstanding debit once, after reserved insertion. Generation
overflow preserves the original host certificate. Configured pressure and
explicit release convert the retained input into borrowed data-cleanup custody,
which remains rooted through model retake. A completed backing refund is not
undone by a later retake failure, but the logical outstanding debit and completed
receipt remain retained until settlement succeeds. Retake errors outrank ordinary
cleanup errors; the original panic survives secondary retake/poison failures.

Foreign input returns unchanged. On a healthy root-free session the rejected
attempt still closes immutable pool configuration, including when SDMA is
disabled. Terminal or retained-root foreign calls are inert. A second
matching-owner input while the recycle root is occupied aborts: the existing
return type cannot represent terminal returned custody, and neither overwriting
the retained input nor falsely returning a healthy retry is valid. Allocation,
pool checkout/trim, queue release and Drop guard unfinished recycling.

Runtime input is retained before driver selection. Native failures cross the
adapter as typed errors; returned buffers are rooted before diagnostic formatting.
Indexed healthy recovery restores only the original-kind synchronous placeholder
and remains Quiescent. Transient recovery and ambiguous failure remain terminal.
Logical allocation removal and accounting/profile commits still occur only after
successful settlement. The scripted driver retains ambiguous/panicking input so
runtime tests inspect exact lower custody rather than a fake release.

Nine constructed tests plus a certificate-overflow unit test exercise cache and
disposal success, reservation retry, policy/admission rejection, cleanup/retake
and currentness failure matrices, foreign-attempt history, public guards and
process-isolated fail-closed behavior. Nine runtime tests cover exact owner and
neighbor identity, independent slot/kind validation, diagnostic unwind, public
Context error classes and credit quarantine, and inert terminal retries. The
public guard cases use engine-less shells carrying genuine fixture buffers.
GNU/musl each pass 1,344 KFD and 789 runtime tests, with six opt-in hardware tests
ignored locally. Eight isolated MI300X probes pass; the new zero-cache case
observes complete device backing disposal before trim and control-only host
backing before shutdown. See the
[development receipt](evidence/dev-r126-sdma-recycle-2026-09-16/README.md).
This is not formal correspondence, native recycler fault qualification or
R126 acceptance.

## Directional Demotion Custody

Demotion now retains the original directional allocation before the model loan.
Its borrowed admission order and mapped-memory validation are unchanged. Retake
settles before the existing quiescent conversion; successful demotion advances
the pool generation once without changing native backing or outstanding debit.
Healthy opening/validation or active-use rejection returns the exact allocation.
Retake errors take precedence over ordinary validation errors, and the first
panic leaves the allocation rooted through secondary retake/poison failures.
The new root participates in construction, enable/allocation/trim/release and
Drop guards, and suppresses foreign-recycle activity after unfinished demotion.

The runtime retains the device before selecting its driver and receives native
errors without formatting. Returned custody is rooted before diagnostics, and
healthy restoration requires the original DeviceLocal synchronous placeholder.
It reuses the original device box through a small reviewed allocation-splitting
helper, avoiding a new device-box allocation on successful demotion or retry
restoration. Error-message formatting may still allocate.
The helper transfers a unique initialized value and preserves the same allocation
as MaybeUninit storage; tests cover drop order, unwind, over-alignment and ZSTs.
Public release remains Quiescent on healthy demotion rejection because device
scrubbing may already have occurred. Accounting and release profiling commit last.

Eight constructed tests and nine new runtime tests cover retained native/ledger
identity, real foreign mapping rejection, active-use cancellation, diagnostic
unwind, original box/neighbor identity, authentic scrub invalidation, public
Context credit settlement and subprocess Drop. Public lower guard tests use
engine-less shells with genuine fixture mappings, not successful native parents.
Execution and its qualification scope are recorded in the
[development receipt](evidence/dev-r126-sdma-demotion-2026-09-16/README.md).
This does not accept R126 or qualify native demotion faults, formal correspondence
or matched HIP/HSA performance.

## Lower Synchronous Copy Custody

The ordinary directional single-copy path now roots its original allocation,
host buffer and typed use lease before opening currentness. Borrowed preparation
keeps the request through native callbacks; publication installs actual queue
records before writes. Completed data is rooted before retake, and returned
leases survive failed completion/quarantine transitions. Separate opening and
fused prepare/publish/wait loans, error precedence and session-only versus
process-wide poisoning are preserved without new success-path allocation.

Fourteen constructed tests cover real mapping/model transitions, callback and
backend failures, exact packet/owner identity, timeout and public reentry/Drop.
Eight isolated MI300X regression probes pass, including patterned device copies
in both directions. See the [development receipt](evidence/dev-r126-sdma-synchronous-2026-09-16/README.md)
for final regression status and limitations. This does not accept R126, A1/A2,
formal correspondence or HIP/HSA performance.

## Runtime Synchronous Copy And Readback Custody

The runtime now roots host staging before normalization and preserves the
original device-box shell across fused execution, typed diagnostics, completed
metadata checks and callback-free native frontier retirement. Restoration
requires the exact synchronous allocation slot before refilling the original
box. Busy and prepublication rejection settle staging cleanup first. Indexed
host access and transient readback retain owners through read/copy panic;
ordinary read errors preserve cleanup-error precedence.

Eighteen new tests cover exact bytes/owners, offset copies, healthy retry,
metadata and restoration-slot corruption, timeout/teardown, actual scripted
copy/retirement panic, diagnostic panic, partial read visibility and Context
credits. GNU/musl each pass 816 runtime tests with six ignored. The synchronous
core success path counts zero heap allocations in its scripted fixture; public
readback still allocates its intermediate byte slice. See the
[development receipt](evidence/dev-r126-runtime-synchronous-2026-09-16/README.md).
Fresh native validation is pending while the shared MI300X is busy. Typed
capacity disposition and warm pending-compute allocation are implemented below.
R125 is still the accepted CPU/test checkpoint.

## Typed SDMA Allocation Disposition

The lower fresh-allocation driver now preserves the original typed error and
supplies retry authority only for exact host-visible/device backing-credit
`Capacity` after successful model retake and complete owner settlement. A
nonterminal flag alone is insufficient. Other errors remain conservatively
classified for process teardown; legacy allocation APIs erase this added
classification without changing their original errors or state transitions.
Pooled APIs share the same checkout and fresh-allocation implementation.

Runtime allocation captures queue readiness before initialization. Proven
capacity rejection on an already established primary/SDMA route is `Rejected`;
if this call created queues, it is `Quiescent`. Host-side activity bookkeeping
and model revisions can still advance. At that packet, configured Context
admission refunded only `Rejected`; a cold `Quiescent` failure retained credit even
without an allocation handle. This conservative behavior is not full memory
parity. Errors after successful allocation, including recovered promotion and
zero-initialization followed by successful cleanup, are also `Quiescent`.

Staging uses the same typed helper. Existing public read/write and partial-chunk
boundaries preserve prior-effect classification and visible prefixes. Driver
selection, allocation and diagnostic panics seal the backend while preserving
original panic payloads and existing owners. No message matching or new unsafe
code is introduced.

Fourteen added tests cover genuine constructed allocation/accounting/model
transitions, denied settlement authority, public pool hit/miss compatibility,
Context credit behavior, exact terminal custody and chunk visibility. The
[development receipt](evidence/dev-r126-sdma-allocation-disposition-2026-09-16/README.md)
records validation and limitations. Scripted cold admission is not native queue
creation; engine-less pool coverage is not a native fresh-allocation test.
R126 acceptance, native pending-compute allocation, formal correspondence and
matched HIP/HSA performance remain open.

## Allocation During Pending Compute

The allocation path now distinguishes reusing established primary/directional
SDMA ownership from creating queues or enabling engines. The warm route no longer
rejects solely because compute is active. Readiness requires the actual owner
and enabled engines; a flag alone is insufficient. Cold active-compute requests
still return `Rejected(Busy)` before changing native ownership. Existing request
validation precedes that guard, and valid rejected attempts may consume an ID.

Fresh/pool allocation, full-foundation loan/retake, promotion and synchronous
zeroing retain their existing algorithms. No dispatch is polled, drained,
replaced or implicitly completed. Persistent device-zero publication still
requires the lower exact-currentness/disjointness predicate; the runtime shortcut
does not grant coexistence authority. Terminal requests remain sealed.

Seven new CPU tests run matrices over ordinary primary-only, auxiliary-only,
two active lanes, a third queued stream, and genuine Scripted persistent
prepared/published workflows. They compare original owner IDs/bytes/boxes,
allocation metadata/shadows, active/pending identities, recipes, retained
resources and lane leases. Warm capacity and cold-active Busy refund configured
Context credits; initialization errors/panics retain the partial Host owner and,
for DeviceLocal staging failure, its hidden indexed Device and charge. Context
quarantines that failed attempt's credits. Fixture disposal is not native cleanup.

Two ignored native probes extend the existing public vecadd workflow. They
require native allocations, unchanged real published-receipt commitments,
full original readback, zeroed new bytes and eventual backing-account refunds.
The new borrowed observer authenticates the exact lane/epoch using the existing
lower checker, adds an ordinary/lane digest domain, and neither reads completion
signals nor grants authority. The probes are not yet executed. Logical pending
custody would not establish physical GPU overlap even on a successful run.

See the [development receipt](evidence/dev-r126-pending-compute-allocation-2026-09-16/README.md).
Scripted persistent execution does not instantiate the lower native attachment
predicate. Ordinary prepared/completed states, three-binding and pipeline phases
still need dedicated integrated qualification; Scripted has no real pipeline
publication path. Formal correspondence, full R126 acceptance and matched
HIP/HSA benchmarks remain open.

## Allocation-Specific Settlement

`RuntimeBackendV1::allocate_with_outcome_v1` now supplies an additive
`Allocated`/`SettledNoOwner` outcome. Its default preserves legacy errors.
Context refunds only the explicit settled attempt's requested-byte/record token,
then returns the original diagnostic as `BackendQuiescent`. Queue infrastructure
and pool/model bookkeeping may remain; no requested owner or pending allocation
root may remain. Generic quiescence or an empty allocation index is insufficient.

Direct KFD grants this outcome only for cold typed backing-capacity rejection
from the lower allocation helper after settled model retake. Queue creation,
promotion, initialization and hidden-cleanup errors do not grant it. The
multi-device router forwards it without installing an allocation route, and
legacy allocation calls still return `Quiescent`. Worker V1/V4/V5 keep their
existing wire semantics and therefore still quarantine these failed attempts.

Nine added CPU tests cover exact diagnostic/panic identity, repeated refund and
retry, neighboring allocations and device isolation, direct multi-device routing,
later cleanup quarantine, canonical Worker dispatch and child-process accounting.
The [development receipt](evidence/dev-r126-allocation-settlement-2026-09-16/README.md)
records final-source validation separately from native, formal and performance
qualification. This closes the direct Context credit-recovery implementation
gap, not all allocation-failure recovery or full memory parity.

Two opt-in native probes now cover the intended cold/warm/retry sequence for
HostVisible and DeviceLocal allocation using deterministic session budgets.
Their source requires exact Context/backing refunds, native owners and divergent
native/shadow readback, zero-cache disposal and retained-root account observations.
They require explicit device and isolation acknowledgement and remain unexecuted.
See the [preparation receipt](evidence/dev-r126-native-cold-probes-2026-09-16/README.md);
these are compiled qualification candidates, not native acceptance or benchmarks.

## Remaining Qualification

Allocation settlement still needs native cold-admission/failure qualification,
formal implementation correspondence and a separately negotiated Worker protocol
extension before its stronger guarantee can cross the process boundary.

1. Extend the successful allocation, primary and two-stream dispatch native
   probes beyond the now-qualified AUX host-budget rejection. Qualify integrated
   NEW, REBOUND and resident-overwrite native failure paths, and allocation/SDMA
   ownership changes with pending compute after qualifying synchronous-copy
   custody and typed capacity disposition. Lower fresh-allocation outputs now
   remain rooted through model retake; the new warm runtime route does not by
   itself qualify native pending work.
   Corrupted-observation model rejection, scripted native errors and actual
   hardware outcomes retain distinct evidence scopes.
2. Qualify the new directional route through genuine public runtime workflows,
   including failure retention and additional account/assigned compute-lane
   destruction observations beyond the qualified success probes. A manually
   installed queue or scripted early shutdown return is not public-workflow
   evidence. Pending ordinary/XGMI/window
   owner matrices and remaining integrated resource/model joins need completion.
3. Qualify pool-trim failure retention and account observations through public
   native runtime workflows. Constructed-parent CPU matrices and the packetless
   native success probe do not substitute for these fault paths. The shared
   ordinary recycle/release driver now has its own retained root, but its native
   failure paths and other SDMA ownership transitions still require qualification.
4. Run fresh GNU/musl regressions, source gates, compiled negatives, checker
   calibrations and independent evidence review before R126 acceptance.
5. Qualify applicable additional queue profiles, native GPU execution, formal
   correspondence, aggregate-memory behavior and matched HIP/HSA performance.

The initial ordinary-primary profile does not remove other queue profiles from
the final objective. Generated DATA-ADOPT, ISSUE, completion/readback/typed
replies, Stop/drain/graphs, resource proofs and production Context integration
remain subsequent requirements.

## Directional SDMA Handoff

The handoff below motivated the now-implemented ordinary-primary plus Directional
SDMA packet. The legacy methods still used by deferred profiles have these
limitations: existing
`Gfx942SdmaQueueOwnerV1::destroy_queue` consumes its doorbell without retaining
the exact unmap outcome; `release_resources` consumes its three tokens, and
queue-set cleanup pops owners. Wrapping those consuming methods is insufficient.

The new path installs borrowed one-shot cleanup state under the public root before effects:
original owners, exact destroy request/outcome, doorbell progress, fixed
completion/control/ring cleanup tokens, terminal latch and completed prefixes.
Reuse existing vectors and indexed borrowed owners rather than popping after
effects. Preserve actual ordering:

1. Destroy directional H2D slot 1, then D2H slot 0, before primary destruction.
2. Complete primary platform teardown, foundation restoration, primary resource
   release and shadow completion.
3. Release directional H2D, then D2H resources. Within each owner, unmap all
   completion/control/ring tokens before releasing them in that same order;
   preserve USERPTR free-before-CPU-unmap behavior.
4. Release dispatch and primary signals; only then confirm the gate. The combined
   primary/two-directional-queue result accounts for eleven resources.

Required tests cover genuine mapped owners/doorbells, pending-record and malformed
roster rejection, raw destroy/doorbell outcomes, every cleanup/commit/currentness
prefix, certified lower-path six-revision headroom (without inventing a certificate
after permanent restoration), inert retries, native completed Drop, and actual
public runtime allocate/release/shutdown selection with accounting
and profiler events. ID-only fixtures cannot establish successful cleanup.
Striped, Generic, LogicalMux and terminal-creation profiles remain explicit later
requirements; this handoff implements none of them and adds no acceptance claim.

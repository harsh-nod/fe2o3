# Persistent Returned-Data Cleanup V1

Status: R119 locally accepted at the CPU/test boundary. This follows
[detached persistent-control cleanup](runtime-detached-persistent-control-cleanup-v1.md).
The contract below was reviewed by the read-only swarm on 2026-09-14.
Primary first implemented the candidate separately above accepted R117
`a07ec44309e214f2a8ef0e687e610c8e60a36224`, then integrated its exact four-file
delta above published R118B `8e2c8532cb60918de523c6cab1b861bcd61fc319`.
It is a new source cohort, not part of R118B acceptance. The
[464-artifact archive](evidence/local-r119-persistent-returned-data-cleanup-2026-09-15/README.md)
and both independent final reviews pass. No native, formal or performance
acceptance is added.

## Isolated Candidate

The isolated worktree is `/home/harsh/.codex-tmp/fe2o3-r119-persistent-data`,
branch `codex/r119-persistent-returned-data`. The candidate changes the existing
owner's consuming wrappers and shared control-release driver and adds fifteen
tests. It preserves the existing crate-private tuple signatures and uses explicit
Unprepared/Prepared/Returnable/Taken states. Output capacity is reserved before
disposal; normal cleanup errors return ordered data while retaining incomplete
controls and metadata, whereas panics retain data before resuming the payload.
The common driver performs one forward control pass and one ordered data
conversion, with no output allocation after disposal or authority cloning.

The isolated source map is
`b12435bb1aff37927a5d9b79e4871679476555d76a7efd62ea33635d463f764d`,
covering 5,690 non-document source identities. Formatting passes. The first
focused command compiled but selected zero tests because its filter was wrong;
`r119-persistent-focused.json` is retained as compile-only history. The corrected
`queue::dispatch_binding::control_release::tests::persistent::` filter passes
all fifteen tests with no failures or ignored tests, recorded in
`r119-persistent-focused-v2.json`. Both runs preserve their source endpoints and
closed process groups. Raw records remain under `/home/harsh/.codex-tmp`.

The full KFD library suite passes all 1,149 tests with no failures or ignored
tests in `r119-kfd-runtime.json`: the exact retained R117 roster of 1,134 tests
plus fifteen new tests. The cleanup namespace contains 46 passing tests,
31 retained plus fifteen new. Strict all-feature/all-target Clippy passes in
`r119-clippy.json`. Both records preserve the same source endpoints, runner pin,
predecessor chain and closed process groups. The library command took 963.768
seconds and Clippy 45.863 seconds; these are validation durations, not runtime
performance measurements.

At this isolated boundary, full packet qualification, including fresh integrated
GNU/musl and auxiliary gates, decisive mutations, restoration checks and
independent archive review, remained pending. Completed integrated qualification
is recorded below; the isolated checks do not substitute for it.
The fifteen tests comprise fourteen behavioral functions and one source-routing
guard; scripted lower-adapter coverage is not live Linux/KFD execution evidence.

The prospective mutation descriptors are reviewed and derive 37 executions
against 30 distinct production source variants, including four repeated-source
groups. Unique anchors match the unchanged isolated source. The current draft is
`/home/harsh/.codex-tmp/r119-persistent-mutations-v2.js`, SHA-256
`416c1c0f1e0dd75b87780c0d2fa88dd5ec07aa14f5876714fcbf978c6bd3098a`;
it retains the v1 descriptor as an input. The only v2 change labels the combined
projection/commit test's first decisive cell as projection failure, not actual
commit exhaustion. No Rust mutation or compiled negative ran in the isolated
cohort; the integrated campaign below executes all 37 descriptors.

Independent review found no assertion-under-lock or destructor-panic blocker.
Replacement output storage is allocated while the original buffer remains live,
taken active control stays mutable, and generation mutations change only the
persistent modes. These are construction checks, not observed failure oracles;
compiled failures, exact restoration and integrated qualification were still
required at that boundary.

## Integrated Qualification

The integrated source map is
`542110b3429161394392f24a398929aba37aa1d74950d0a86204adda2d0959d2`,
with 5,693 identities. All three modified files' parent blobs match R118B;
`control_release/persistent_tests.rs` is the sole added source file. Independent
source review found no integration or ownership blocker. The control/output
passes are O(controls + data), excluding the existing generation-slot scan and
lower-adapter costs. Deliberate failed-owner retention is not resource
reclamation or an aggregate-memory bound.

`r119-integrated-origin-v1.json` pins the original 24 isolated artifacts, four
promoted source hashes, both parents and a separately named runner. Original
records retain their isolated cwd, source map and clock contract. The new
runner's repository/parent/namespace changes are mechanical, preserving bounded
execution and owned-process cleanup.

Integrated formatting and focused suites pass: fifteen persistent tests, then
all 46 cleanup tests (31 retained plus fifteen new). Exact rosters, full source
endpoints, predecessor hashes, raw clocks and absent owned groups pass independent
review. These were preliminary checks, not by themselves packet qualification.
The fresh GNU run passes 2,795 tests with five ignored across the same 48
libtest targets plus one CSV target. KFD adds exactly fifteen tests (1,149
total); the 733-test runtime roster and every other target are unchanged.
Independent review verifies the closed record, exact source endpoints and
absent owned group. `r119-integrated-gnu-all.json` hashes to
`ebdaefb857430876d57e470cd467e33ae931b0bf795a7c40a0ea35d59730da3c`.
Its 1,233.671-second validation duration is not a runtime performance result.
The first fresh musl attempt, `r119-integrated-musl-all.json`, hit the unchanged
1,800-second outer deadline and is rejected history. The wrapper records exit
124, an observed SIGKILL and no remaining owned process-group members. Its
1,800.128-second duration includes a 3m16s compilation phase. Exactly three
completed harnesses passed 62 tests; KFD emitted 415 passing rows but no completed
summary. There were no failed-test rows. These partial results are not a full
musl pass, and the underlying cause of the longer duration is not established.
Independent review confirms the unchanged source map, clocks, runner and closed
group. The record hashes to
`45cc664330884b635e762122cd606e0d660e7596a9967eabce7a991d3d928052`;
the log hashes to
`54a51986daa7488c7f36dd0ae3ef6bcc5677b533df0c66a5c89a7d7df92d4160`.

The original history validator correctly rejects this timeout as a normal
prerequisite, recorded in `r119-integrated-history-reject-timeout-v1.json`.
Twenty-six independent freeze-boundary/input-validation tests pass in
`r119-integrated-preliminary-freeze-contracts-v1.json`. They cover seventeen
capture/output boundaries and nine helper-pin/TAP validation cases; they do not
constitute a source freeze. Start/end manifest hashes bind seventeen helper
inputs. At that preliminary boundary the full 24-case history suite and fresh
runner contracts had not run, and no frozen-source/environment outputs existed.

The complete retry, `r119-integrated-musl-retry1.json`, subsequently passes
2,795 tests with five ignored across 48 libtest targets and the separate CSV
target. It uses the unchanged 1,800-second deadline and environment, chaining
after the original timeout. Its 1,236.727-second duration includes 1,188.16
seconds in the 1,149-test KFD harness. Independent review verifies source,
complete rosters, raw clocks, predecessor and absent owned group. Record and
log hashes are respectively
`ed6e6824dbc1ad3b3af6a7210302198e3e37688f1207281846c046142b064ebf` and
`14ab2a1c5b49c091a46e01b6b53551622ec8fa2c85d59d3b6931e41d87cc03e7`.
No partial results from the first attempt supply this pass.

The immutable V2 helper cohort selects only that completed retry and separately
checks the rejected timeout's raw failure/cleanup semantics before its artifact
pins. Its manifest pins 27 inputs and hashes to
`f4155cd4e807a1fb95c259dc909b27ada2657d9904f0a895047cfd16a0dea7cb`.
The revised history suite passes 46 checks, exact history validation passes,
the runner suite passes nine checks with twenty fixture-artifact pins, and the
freeze/input suite passes 26 checks. Original V1 helpers and attempts remain
unchanged. The direct freeze captures 123 prior artifacts and all 5,693 source
identities; it was not wrapped by a runner whose open log could enter the capture.
`r119-integrated-environment.json` hashes to
`4a74a5a41e1ef8eaeb42116bbc022f8a906046a2ac7e922dd7cbffd00af28643` and
explicitly records `accepted: false`.

The fifteen remaining source gates and ten auxiliary checks pass. Their
closed records retain unchanged source maps and absent owned process groups.
The auxiliary proof-inventory check is not a fresh solver run. Exact campaign
validation by the final closed collector passes.

Core and lifecycle contracts pass 48 and 22 checks. The qualification harness
passes all 227 exact cases. Six frozen and restored suites each pass their
15/16/15/37/7/9 rosters. All 37 compiled production negatives reach their named
behavioral failure with normal Cargo exit 101, covering 30 distinct source maps
and four exact repeated-source groups. Every execution restores all 5,693
source identities. The 81-entry terminal chain and closed collector pass.

The archive contains 464 raw artifacts, including 213 historical inputs. Both
independent reviews and Primary's final read-only check match the exact captured
bytes, provenance, helper pins, source endpoints, clocks and process closure.
Its summary SHA-256 is
`ab764a35a311f62a644dc6d7808d25360a5f82b5f79d4c51a5bf2507345c036c`.
The original musl timeout remains rejected history; only the completed retry
supplies musl acceptance. Endpoint equality does not prove continuous source
immutability. These validation durations are not GPU runtime or HIP/HSA
performance measurements.

Integrated strict KFD all-feature/all-target Clippy also passes in
`r119-integrated-clippy.json`, preserving the same source map and closed owned
group. This is a separate preliminary branch after GNU, not a replacement for
the complete source-gate campaign.

Four unchanged compile-fail doctests move in `queue_dispatch_binding.rs`:
3049 -> 3022, 3059 -> 3032, 3067 -> 3040 and 3093 -> 3066. Qualification accounts
for these exact relocations and checks the complete unchanged fence bytes,
without allowing arbitrary roster changes. Independent derivation verifies all
30 production maps and repeated groups against the executed 37-case plan.
Its focused selector preserves Rust substring matching for `pristine_abort`.
Source/test, native, formal and performance acceptance remain separate.

## Boundary And Contract

Replace only DispatchResourceOwnerV1::release_persistent_data_before_publication,
release_persistent_data_after_recycle and shared release_persistent_data in
crates/fe2o3-kfd/src/queue_dispatch_binding.rs. Use the existing complete owner
in queue_dispatch_binding/control_release.rs, its borrowed control adapter and
narrow test helpers; add a separate persistent test module.

Generation rejection precedes cardinality rejection; both return empty data.
After successful validation, every control-cleanup Err returns the entire
original ordered data set. Success returns the same set and admitted generation.
Before-publication permits generation zero; after-recycle does not. Do not add
an Attached-state requirement absent from today's methods.

## Ownership Design

1. Add explicit persistent-before-publication and persistent-after-recycle
   modes; construct the complete root before generation validation.
2. Root initially empty Vec<Gfx942FixedDispatchDataV1> output storage and explicit
   readiness. Reserve capacity after generation/cardinality checks, before any
   disposal. Do not reserve ordinary returned-lease storage for these modes.
3. Reuse the one-shot kernarg-first, forward-code borrowed control driver.
   Preserve active custody until explicit Complete.
4. Persistent-only extraction may transfer data after cleanup success or Err
   when preflight completed and cleanup returned normally. Use
   dispatch_data_from_authority_v1 and preallocated
   storage, preserving initialization without reviving content descriptors.
   Never allocate after disposal or add a second cleanup loop.
5. Preserve tuple signatures. Early rejection retains the whole root and returns
   empty data. Later Err transfers all data and retains controls/metadata.
   Panic retains untransferred ownership before resuming the original payload.
6. Ordinary returning wrappers/extraction positively admit only their original
   two modes. Unit extraction is detached-only. Wrong wrappers reject before
   cleanup or transfer. Track readiness separately from output-vector emptiness.

Returned-on-error data is no longer owned by the retained control root. Preserve
this explicit split in tests and documentation, including valid zero-data input.
Retain original premises and metadata after transfer; borrow premises while
converting data. Do not reuse the existing interrupted-root assertion unchanged,
since it requires that all original data remain rooted.

Follow-up review freezes explicit persistent output states:
Unprepared -> Prepared(generation) -> Returnable(generation) -> Taken.
Prepared follows successful generation/cardinality/capacity preflight.
Returnable follows a normal return from the common cleanup driver, including
Err; a panic leaves Prepared and extraction must reject without mutation.
Taken latches before extraction, independently of data length and the existing
control-complete flag. A normal cleanup Err can return data without claiming
that controls completed. Later N4-L may retain Returnable across retake, but
outer settlement still decides whether output escapes. No second cleanup loop
is needed for this distinction.

## Decisive Tests

Use the accepted scripted lower adapter, not live cancellation injection, which
can bypass the real bridge. Cover generation precedence, cardinality, capacity,
zero data, cancelled history, exhausted next generation and exact five-variant
mixed data output on success/error. Check identities, order, initialization and
absence of revived descriptors. Cross every control position with native
error/panic, currentness, partial unmap, projection/actual commit rejection and
incomplete callback. Inspect exact active state, suffix, metadata, charges and
completed effects; data itself receives no disposal. Error transfers once,
panic retains, retry/wrong extraction never re-enters. Capacity must predate
disposal and survive success/error extraction. Use genuine single- and
three-binding persistent preparation fixtures. Keep R115/R117 regressions.

Use fourteen table-driven behavioral functions and one supplemental routing
guard: exact mixed-data success; generation-error precedence; cardinality/capacity
precedence; zero-data readiness; generation history/exhaustion; native error/panic
custody split; all eighteen currentness boundaries; partial unmap; projection and
actual commit failure; incomplete callbacks; wrong/repeated extraction; real
preparation; pre-disposal output capacity; and panic-root extraction rejection.
Exercise wrappers for error/panic output splits, not only the borrowed driver.

Reuse pristine_dispatch_fixture_v1(8), lower error/currentness/unmap/commit
injection and native/control-record snapshots. Advance genuine prepared owners
using their retained queue identities, without detaching data. Add explicit
persistent-mode cases to fixture generation transitions: the old fixture only
recycles Mode::AfterRecycle and would otherwise mask later failure assertions.

Freeze mutation anchors only after implementation. Independently test mode
selection, swallowed generation errors, preflight ordering/guards, constructor
data retention, length-based readiness, premature Returnable, missing normal-Err
eligibility, missing Taken, late/wrong output allocation, reversed/truncated or
misinitialized output, error/panic retention and each wrong-wrapper guard.
The decisive panic negative promotes Prepared to Returnable before cleanup and
must fail the panic-root extraction-rejection assertion. Reuse shared control
order/active-custody/retry/incomplete mutations against persistent assertions,
recording repeated source maps separately from distinct mutations.

## Subsequent Production Joins

The next lower packet combines ordinary `DispatchResourceOwnerV1::release`
with typed `SharedGttMemorySessionV1::release_fixed_dispatch_data` cleanup and
the initialized-device disposal bridge. Extend the existing full-owner root
and borrowed cleanup primitives. Preserve ordinary poison/vacant-slot checks
without importing persistent generation/cardinality checks or output allocation.
Keep kernarg, forward code and forward data order, retaining the active data
owner or disposal receipt and untouched suffix on error or panic. Device
disposal uses its existing native records/accounting, not invented coherent-host
model keys. Confirm disposal before later currentness/accounting failures;
never retry disposal or refund twice. Generic raw-token APIs and XGMI teardown
remain outside this lower packet.

Persistent N4-L must root custody outside the live model loan through retake.
Both existing callers are before-publication cancellation; no current caller
uses the true after-recycle branch. Retake error still dominates while returning
exact data; retake panic must retain the sole output owner.

Ordinary N4-L also needs outer custody for detach_recycled_fixed_dispatch_inner
and retained-control release. with_live_queue_memory_model can otherwise discard
successful output on a closing failure. Data disposal needs typed custody for
release_fixed_dispatch_data, mixed owner release and detached-data release
before committing a reusable hole. N4-Q must retain outputs across later signal
disposal, callbacks and backend extraction, and retain taken auxiliary lanes.

This packet feeds N5 DATA-ADOPT, but N5 still needs its applicable live/data/queue
routes. It does not need every unrelated teardown mode first. No new native,
formal, memory-bound or performance acceptance follows from this handoff.

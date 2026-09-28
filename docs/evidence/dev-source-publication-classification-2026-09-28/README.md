# Classified Native Source Publication

This packet qualifies the host receipt flow for native dependency-source
publication. It does not qualify GPU execution, native ordering, physical
overlap, formal refinement, performance, or HIP/HSA parity. A1/A2 and accepted
Native R125, Admission R118B C1/C2/C3 and Resources R116/V3 remain unchanged.

Source commit: `c97c385861d789224d02d6b826707c4986f7efd6` (SSH-signed).

## Validation

| Check | Result |
| --- | --- |
| Broad KFD all-features library, before test-only corrections | 1720 passed, 2 failed; exit 101 |
| Corrected KFD live-queue subset, including all 9 new source groups | 155 passed, 0 failed; 1567 filtered |
| Full runtime all-features library | 1818 passed, 0 failed, 30 hardware ignores |
| KFD/runtime doctests | 95 passed, 0 failed |
| Both crates, all-features/all-targets strict Clippy | Passed with `-D warnings` |
| KFD no-default-features production check | Passed |
| Package formatting, diff and frozen-source checks | Passed |

The complete raw logs, exact command outcomes, source patch and hashes,
executed-ELF hashes, toolchain identity and review notes are in
[raw.tar.gz](raw.tar.gz). The archive has an internal file checksum inventory;
its outer checksum is in [SHA256SUMS](SHA256SUMS). Executables are identified by
hash, not bundled. A fresh extraction and internal checksum check verify archive
integrity only, not a new execution of the tests.

## Implementation

`ComputeAqlQueueLaneDispatchV1` gains
`submit_fixed_dispatch_with_dependency_events_classified_v1`. The existing
`submit_fixed_dispatch_with_dependency_events_v1` remains and maps the classified
error back to the same underlying error API.

The private native recipe adapter binds retained native dispatch resources and
uses the existing native batch publisher. A shared, statically dispatched wrapper
owns source admission, acceptance reservation, recipe binding, completion/event
publication, rollback, batch/event wrapping, terminalization and panic handling.
CPU tests substitute only the recipe boundary and native publication callback;
they use the actual dispatch-generation, completion, event and reader owners.
No public recipe, packet, signal address or native callback injection is added.

- Persistent conflict, invalid count and ordinary binding refusal are rejected
  before publication. The public lane wrapper can reject persistent conflict
  even before entering source publication.
- Acceptance is reserved before recipe binding, preserving legacy identity burn
  ordering. Exhaustion is terminal. An already-poisoned acceptance owner retains
  its legacy no-effect rejection without newly poisoning the session.
- Signal capacity is retryable only after dispatch cancellation. Ring capacity
  is retryable only after complete event release, completion cancellation and
  dispatch cancellation, in that order.
- Retry refunds reusable capacity, not consumed acceptance epochs, dispatch
  generations, batch IDs or event IDs. Exact neighboring receipts remain live.
- Rollback failure, post-publication failure and uncertain native outcomes are
  terminal. Dispatch cancellation failure preserves `StaleDispatchGeneration`.
- Rust unwinds retain the original panic payload. The source wrapper poisons
  before unwinding; the lane envelope restores primary/AUX storage and poisons
  before resuming the panic. Returned bundles remain move-only retained custody.

The adapter introduces no additional publication loop or background worker.
Existing bounded per-packet binding and rollback remain in use. This is a
structural observation, not a measured performance improvement.

## Test Scope

Nine test groups cover both primary and AUX lanes where applicable:

1. N=3 complete dispatch-roster and per-event identity joins; partial event
   release refuses recycle at each remaining pinned slot; a genuine reader
   still prevents recycle after its event is released; final release resets
   exactly three slots.
2. Full signals reject before the native callback, preserve existing completion
   custody, cancel the reserved dispatch generation and permit later reuse.
3. Injected ring-full releases all attempted events/signals/dispatch capacity,
   burns identities, preserves live neighbors on both lanes and permits retry.
   Neighbor and retry receipts retire out of publication order.
4. Event release, completion cancellation, dispatch cancellation, typed native
   failure, completion publication, event binding, dispatch publication and Rust
   unwind failures never become retryable. Terminal fixtures do not manufacture
   healthy cleanup.
5. Public legacy/classified rejections agree; zero count does not consume an
   acceptance epoch, while recipe refusal does.
6. Acceptance exhaustion is terminal before recipe/native work.
7. Persistent and already-poisoned acceptance gates preserve legacy behavior.
8. A complete returned bundle deposited outside the lane callback survives a
   later callback panic without recycle or false logical success.
9. Scoped source-forwarding smoke checks tie the native adapter and both public
   methods to the tested wrapper. These checks are not executable refinement.

Signal observations and native return values are CPU-injected. The event-bind
negative control uses an invalid returned last-packet ID for N=3; the completion
cancel control injects an inconsistent slot phase. Neither supplies GPU evidence
or native authority. Ledger counts are checked separately because the existing
completion custody snapshot does not contain event/reader ledger contents.

## Remaining Integration

Existing R42/R45/R51 model roots do not prove this source wrapper's production
refinement. A successor must share actual control and exact rollback algorithms,
derive the live-event/slot-pin cardinality invariant, and prove successful retry
restores selected custody while preserving unrelated state and burned counters.
A Boolean rollback-success premise or a target-only 256-dependency bound cannot
substitute for the source operation and its 8192-packet bound.

Runtime `ActiveSubmissionV1` does not yet own these source events. The next
integration must retain an event alongside its actual batch before outer-close
settlement and preserve requested state through Prepared retries. Unused native
pins must be released after actual Ready, before recycle, independently of public
logical-event lifetime. Late and repeated logical events must not duplicate
native move-only authority.

Native target scheduling still needs separate physical completion and logical
settlement, retained dependency identities, and checked lane-local storage
transfer or shared native leases. A target may need physical progress to release
source pins while semantic reconciliation is pending; promoting signal completion
to logical success or waiting for source recycle before target progress is not a
valid solution. Existing explicit-success and overlap/version guards are unchanged.

Production protected Worker providers/proof artifacts, full Context composition,
GPU overlap, repeated scale/rebind, multi-device/distributed qualification and
matched HIP/HSA performance remain open. Issue #182 was freshly observed open in
this campaign; its captured body and update timestamp are included in raw output.

## Reproduction and Cleanup

The campaign uses `nightly-2026-04-03`, `CARGO_INCREMENTAL=0` and
`CARGO_BUILD_JOBS=2`, reusing the existing GNU debug cache. This is not a hermetic
clean-build claim. Source hashes are frozen for qualification. Initial compile
errors and both persistent-gate oracle failures are retained in raw logs. The
broad KFD run passed 1720 tests and failed two assertions: the primary/AUX oracle
and an existing source-wiring check that predated `OrdinaryQueueIoV1`. The latter
now checks both forwarding layers. Both corrections change tests only, with
unchanged production hashes. Corrective validation is recorded separately from
that non-green broad run; it is not presented as a fresh all-green full KFD run.

No MI300X jobs or directories were created. Five stale local host-test ELFs were
removed after exact-path ownership/open-handle review, recovering about 2.1 GiB.
Current binaries, dependency caches, historical evidence and unrelated work were
preserved.

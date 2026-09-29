# Complete Event Batch Release Refinement

Runtime/test/proof source: `c942a0fbfb136a5a30700013b62c3f1febf05ef5`.
Final campaign source: `9b59c1236f5055ac490e2f562aa0cf9d2fb45a2b`.
The latter changes only this campaign's checker and synthetic calibration.

This packet proves the complete raw host-ledger event-batch release algorithm,
conditional on two explicitly trusted standard-library contracts. It does not
close A1/A2, issue #182, source rollback, or HIP/HSA parity.

## Runtime And Proof

Production and Verus share the complete method body: readiness, event-ID scratch
reservation, ordered duplicate/active/live/nonzero-pin validation, multi-item
budget reservation and checking, exact ledger removal and pin decrement. The
mapped budget iterator is replaced by an indexed scan of the original roster.
Commit still consumes the actual Vec iterator. Scratch containers leave scope
before commit, as before. The empty/single-item path skips aggregate scratch.
There is no new allocation or arena scan; expected time and scratch remain O(N).
No measured speedup or physical allocation-call-count claim is made.

No valid-roster or global ledger-cardinality premise is assumed. The proof
derives bounds, authentication, distinct event IDs and sufficient aggregate pins
from the actual checks, including distinct events sharing a slot. A proof-only
trace observes the two real reservation results and records which stages were
reached. Its exact decision function preserves allocation-versus-validation
error precedence and rejects fabricated allocation refusals.

Every normal refusal returns the original non-Copy Vec with unchanged modeled
owner state. Success returns the event count, removes exactly the selected IDs
and decrements pins once per event, framing all other slots, readers, phases,
identities and counters. Mathematical counts impose no artificial roster cap.
A constructed alias witness discharges validity and checks both sufficient and
insufficient pin cases for either observed reservation outcome. This does not
establish allocator-backed success reachability under unconstrained contracts;
genuine CPU success paths provide separate concrete evidence.

## Explicit Trust Boundary

Pinned vstd lacks `HashSet::try_reserve` and `HashMap::try_reserve` specifications.
The 18-line supplement `completion_hash_reserve_contracts_v1.rs` contains exactly
two contents-preservation declarations, analogous to vstd's existing Vec contract.
They assume neither reservation succeeds, nor capacity, addresses, allocation
cost, or termination. No owner/roster authentication is assumed by them.

This campaign **does not pass `--no-cheating`**. That flag rejects the new
contracts, including when imported from a separate crate; those failed attempts
are retained. The final campaign explicitly omits it and pins the supplement's
exact SHA-256. A source guardrail rejects additional trust constructs and
nonallowlisted source/environment inputs across the four-file proof closure.
This textual guardrail is not equivalent to Verus's own no-cheating enforcement.
Historical controllers, classifiers, proofs and the pinned tool installation
are unchanged. Rust/compiler, vstd/Z3 and the identity/storage projection remain
trusted as in the preceding packets.

## Qualification

- Final signed campaign: **32/32 stages pass**. Opening, relocated and closing
  positives each report **28 verified obligations, zero errors**, whole-crate.
  The count includes ten derived Clone checks, not 28 runtime operations.
- **24 executable negative controls** fail logically. These cover ignored and
  fabricated reservation failures at both stages, readiness, identity/duplicate
  validation, bounds, aggregate checks/debits, commit effects, error/count values
  and substituted refusal custody.
- Signed source binding, exact four-file relocation, complete before/after
  continuity of 6198 source hashes and pinned 190-file tool closure pass.
  All 32 final process groups are reaped and independently absent.
- Final completion tests: **55 passed**. Three new groups cover six empty/single
  phase cases, eight late/ordered validation faults with exact spare-capacity Vec
  custody and retry, and mixed Bound/Published/Completed releases with interleaved
  aliases, surviving events/readers, neighboring slots and genuine cleanup.
- Broader KFD suite: **1415 passed, zero failures**, explicitly excluding
  **320 construction-primary tests**. This is not a full KFD-suite rerun.
- Full runtime suite: **1828 passed, zero failures, 30 hardware ignores**.
- **124 doctests**, strict all-feature/all-target Clippy, no-default-feature
  production checks, workspace/included-source formatting and diff checks pass.

The first signed campaign remains failed evidence. Its skip-live-validation
mutant produced a genuine bounds-precondition failure that the strict classifier
did not recognize. The final adapter adds only that exact logical diagnostic,
still requiring authenticated primary source spans and rejecting unknown,
warning-level, foreign-source, nested-internal-error and mixed resource/frontend
failures. Five synthetic calibration groups pass outside the 32 owned stages.

## Remaining Work

These proofs concern normal host-ledger returns, not allocator internals, physical
Vec address preservation, panic/unwind behavior, machine-code correspondence,
native currentness/publication, global pin cardinality or performance. CPU tests
inspect actual refusal Vec addresses/capacity, but do not inject system OOM.
The reservation trace covers its two annotated calls, not all allocator activity;
requested-size correctness and no-reallocation behavior remain source-reviewed,
not proved by the contents-only contracts.

Next is composition with bound and dispatch cancellation, preserving the actual
short-circuit cleanup prefixes and token-discarding terminal adapters. Full
record-to-rollback conservation also needs record freshness/pin increments and
native callback ledger frames. Native target scheduling, protected Worker/compiler
execution, physical overlap, lane-local storage, high-depth async ownership,
multi-GPU/distributed qualification and matched HIP/HSA performance remain open.
A1/A2 and accepted lane checkpoints are unchanged. No MI300X work or new GPU or
performance result was produced in this packet.

## Replay

`raw.tar.gz` contains CPU/static logs, source/tool/test-ELF hashes, development
failures, the failed and final campaigns, command notes and an open #182 snapshot.
Test ELFs are not archived; no hermetic-build claim is made. `SHA256SUMS` seals
this README and archive; the archive has its own file manifest.
A fresh restore matches the raw tree exactly and passes all 418 file hashes.

```sh
python3 -I -B crates/fe2o3-runtime-model/verus/check-completion-event-batch-release.py \
  --output /absolute/new/owned/output \
  --verus /absolute/pinned/verus-x86-linux/verus
```

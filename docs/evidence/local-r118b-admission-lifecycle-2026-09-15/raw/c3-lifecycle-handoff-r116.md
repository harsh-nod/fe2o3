# C3 Reply And Retained-Owner Lifecycle Handoff

Read-only swarm review on 2026-09-14 above accepted R116. No implementation,
test execution, solver or hardware acceptance is added.

Proposed module:
crates/fe2o3-runtime/src/async_engine/tests/owned_tests/preparation_tests/completion_tests.rs.
Declare it in preparation_tests.rs; narrowly expose reservation_tests::reserved
and adoption_tests::{reserved,activate,preparation} as pub(super). Reuse Harness,
LocalPayload, PreparationWakeCount, ready and join. Add only a cfg(test) mutable
accessor to the real ticket completion in generated_operation/reservation.rs:
pub(in crate::async_engine) fn completion_for_test_v1(&mut self)
    -> &mut RuntimeAsyncCommandFutureV1<()>.
pub(super) is insufficient for the sibling test module. Do not clone consumers
or fabricate a new producer. Exposing generic adoption preparation also requires
adoption_tests::RetireBackend to be pub(super), avoiding a private-bound mismatch.

Pass hold.stream() into the test retire_adoption hook. Extend MockState with
per-stream outcomes and (stream, actual_thread_id) attempts, retaining its scalar
fallback. The old hook ignores the hold and cannot distinguish A/B/C.
Add a read-only cfg(test) pub(crate) Context query in context/unpublished.rs
returning Option<Option<u64>> for missing/unheld/held stream identity. Read the
record without require_live so terminal-state errors cannot mask exact B/C holds.

## Tests

1. Reserve A/B with separate drop counters, wakers and real tickets. Call actual
   registry.discard_reserved(key) in both orders; only the selected owner stops
   and drops. Neighbor stays Pending with custody. Include abandoned observer.
   Credits stay charged until producer and consumer are both gone.
   Use Harness::new(2, 4, true); clone only each key. Selected completion is
   Err(EngineStopped); both credits remain while both tickets still exist and
   decrease as their final consumers drop.
2. Queue Stop and run real run_engine_context_v1, not Harness.command alone.
   Completion stops before payload disposal; disposal later occurs on owner thread.
   Destructure Harness to move its receiver into run_engine_context_v1 with None
   and its admission state. Before dispose_quiescent, registry length is two,
   Context is nonterminal and both payload drop counts remain zero.
3. Poll old/latest/latest wakers on A/B, then repeat Stop/progress. Old wakes zero,
   latest once per owner. The existing co1 sticky reply test remains the decisive
   first-result-wins oracle; wake counts alone cannot exclude overwritten values.
4. A wake records counters/order then panics; B is still notified and Context is
   nonterminal. Payloads remain until disposal. Production catches wake panics,
   so assertions inside wake are not decisive: record flags and assert outside.
5. Real prepare/reserve/adopt and activate A/B/C. Script retirement success,
   error-or-panic, then success; log each hold.stream. A retires/drops, B fails
   and Context becomes terminal, B/C stay retained. Repeated progress/retire/Stop
   never invokes C or retries B. Aggregate retirement count alone is insufficient.
   Add a read-only cfg(test) hold snapshot only if asserting exact hold identity.
   Use Harness::new(3, 8, true), register completion wakers before activation
   consumes tickets, then call one full advance to actually adopt all three.
   Check nonterminal state and capture three distinct holds before retirement.
   After retirement, require exact attempts [A,B], drops [1,0,0], length two,
   A unheld and unchanged B/C identities; preserve these on every repeated call.
6. Use start_with_config, ThreadBoundBackend and OwnerTrace for owned shutdown:
   RetainedUntilProcessExit, A dropped once, B/C never dropped, exactly two
   retirement attempts on owner thread.
   Wait with a deadline for all three adoption records before shutdown when
   claiming adopted-path coverage. Require worker_panicked == false and two
   retained reply cells. Record actual callback/drop thread IDs externally;
   OwnerTrace::record stores the expected thread after an assertion and is not
   an independent record of the observed thread.

## Prospective Negative Oracles

Omitted real producer transfer resolves the ticket prematurely through the
ReserveCommand reply destructor: initial Pending is the decisive oracle.
Omitted stop_reply leaves the ticket Pending. Keeping the old waker violates
latest-waker counts. Removing wake catch_unwind must be
distinguished by Context nonterminal state, not merely an outer caught panic.
Disposing in stop_observations violates pre-disposal drops. Continuing retirement
after B failure invokes forbidden C only with actual advancement: merely
deleting the failure return leaves the index on Retiring B and masks this mutant.
A decisive continue mutation replaces that failure return with index += 1.
Skipping hold release leaves A's hold.
Removing a retry guard alone may be masked by terminal and Retiring guards; do
not assume it is a decisive negative without an otherwise-admissible fixture.
Use an explicitly compound negative or separately exercise the actual local
retirement boundary before registry quarantine; do not claim two individual
guard obligations from a single public repeated-call test.

PreparationDriver's explicit Drop stop must precede payload drop. Omitting that
stop still resolves through the completion field destructor, but payload drops
first; record the payload drop count inside the wake callback and assert it
outside. Eventual EngineStopped alone cannot detect the ordering regression.
For deterministic owned A/B/C retirement, reserve all three before activation
and set polls_per_tick >= 3. Full active-roster rotation preserves insertion
order; no artificial scheduling barrier is needed.

This is host reply/unpublished-owner lifecycle coverage. Its completion channel
does not represent GPU success and grants no DATA/ISSUE/COMPLETE authority.

# Live Device Insertion Custody V1

R109/N3-L1 locally accepts the named CPU/shared-sequencer and concrete-facade
boundaries, with [retained evidence](evidence/local-r109-live-device-insertion-2026-09-12/README.md).
Seventeen source gates, ten auxiliary gates and eleven compiled behavioral
negatives pass. GNU/musl each pass 2,619 tests with five ignored; frozen/restored
insertion, initializer and model-loan suites pass 14/14, 11/11 and 4/4.
All 5,673 non-doc source identities match. No native, authenticated formal or
performance acceptance is implied by these local tests.

## Shared Sequence

`queue_live/data_insertion.rs` supplies one production-used sequencer for
initialized-device append, remembered-hole replacement and explicit insertion.
Public session methods and the selected-lane facade use that same sequencer.
It borrows the existing detached ledger rather than creating another ledger.

1. Require an unbound, releasable dispatch and a valid bounded identity ledger.
   Preserve full-capacity rejection before insertion-index rejection.
2. Select the explicit index, remembered hole, or append position. Fallibly
   reserve space for the maximum sixteen identities before native effects.
3. Keep the original boxed input and R107 initialization root outside the model
   loan. The Linux session forwarder passes that root to the original engine,
   model device and VM; only unit crosses the operation callback.
4. Settle the existing operation/retake envelope. A completed initialized owner
   remains in the external root until closing retake succeeds.
5. Borrow the completed identity, commit insertion/count/hole metadata in the
   reserved vector, then extract and return the original initialized owner.

Explicit insertion shifts occupied entries; it is not an overwrite. Successful
insertion clears a remembered hole. The public device initializer with no
explicit index uses that hole when present and appends otherwise. Coherent
replacement has a different required-hole contract and is not routed here.

There is no new copy algorithm, address exposure, per-call root heap allocation or
large inline engine field. The existing arbitrary-byte initialization hash,
copy, exact readback and GPU mapping remain unchanged. Capacity reservation
may allocate before effects; pointer/capacity assertions are not a global
allocator or aggregate-memory bound.

## Failure Custody

Returned pre-entry unbound, ledger, capacity, index and reservation errors do not
poison or transport the parent. An entered operation error marks the selected
owner terminal. In particular, source-validation or layout rejection after entry
is not retryable through that parent, even without native effects or a retained
initializer slot. Absence of a retained slot does not grant healthy retry.
A caught panic additionally
uses the existing dispatch-global poison policy. Opening errors, operation
errors, closing errors and panics retain their existing distinct precedence;
an ordinary opening error is not reclassified as a process-global panic.

An admitted root, including Complete after closing failure, is marked failed
and placed in the original engine's preallocated terminal-initialization slot.
It cannot restart initialization or yield a completed output. Source-only
failures do not occupy that slot. The lower allocator's configured quarantine
policy still applies independently of whether a native allocation began.

The selected-lane facade ORs the transport requirement before returning an error
or resuming a panic. A subsequent poisoned preflight rejection cannot clear it.
The existing facade custody envelope restores the selected auxiliary into its
original slot before retaining the whole parent, even if the caller swallows
the operation error or catches a later caller panic. The direct session wrapper
retains its parent before exposing the settled result.

## Verification Boundaries

Ten constructed-engine test functions exercise the production sequencer with
the actual lower initializer, original memory engine and shared model-loan
envelope. They cover ordered insertion, genuine release-created holes,
15-to-16 capacity, rejection/reservation, opening and closing failures,
currentness/native prefixes, partial maps, panic precedence and commit entry.
They check original source/lease identity, account domains and charges, exact
native arguments/progress, unrelated owners, no retry, and lane/platform custody.

Four additional functions exercise actual session/facade preflight and
missing-engine failure, plus source-routing guards. The concrete facade cases
check sticky transport, swallowed errors, caught/escaping caller panics and
restoration before the retained-parent callback. The guards connect both Linux
forwarders to the same root and engine and enforce extraction after the full
metadata commit.

These boundaries are deliberately separate:

- Constructed native leaves are scripted; they are not Linux GPU execution.
- Primary detached metadata is fixture-local. Auxiliary metadata belongs to
  the actual constructed lane. Later-ordinal tests relocate that same real
  auxiliary behind a vacancy; they do not construct an extra auxiliary beyond
  the supported compute-lane limit.
- The new composed matrix uses configured accounts. Prior lower initializer
  tests retain both configured and unconfigured coverage.
- Dynamic commit-entry fault injection demonstrates custody through retake and
  commit entry. Full commit-before-extraction ordering additionally relies on
  source guards/review, not an authenticated formal refinement proof.
- The concrete Linux session/facade tests stop at preflight or a missing engine;
  they do not establish native initialization success through that facade.
- The released-device accounting exception names only exact fully disposed
  identities; the original host, pending-allocation and aggregate checks remain.

## Remaining Work

N3-L2 needs coherent live insertion, including its required-hole replacement
semantics; N3-L3 needs explicitly uninitialized insertion. N4 lower/live/returning
cleanup and destruction, N5 generated DATA-ADOPT, Context admission/journals,
authenticated refinement, hardware reliability and matched performance retain
their own exits. See the [current swarm map](runtime-swarm-next-packets.md).
This device-only packet does not close A1/A2, issue #182 or HIP/HSA parity.

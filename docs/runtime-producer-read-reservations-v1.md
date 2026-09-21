# Producer-Bound Read Reservations V1

Status: concrete Rust model candidate. Not yet wired into RuntimeContext, Worker
transport or a native backend. Core reader/custody proofs are registered in the
Verus campaign; recent execution extensions have dedicated development checkers.
Production Rust/native refinement remains open.
Pending journaled producer-to-consumer requests still return `ContextReserved`.

## Ownership Model

`ContextProducerReadJournalV1` owns the existing stable-reader journal and a
separate bounded reservation arena. It exposes immutable inspection, but neither
mutable extraction nor `DerefMut`. Every allocation-mutating entry point remains
inside the wrapper. New writes, allocation retirement and Unknown-writer disposal
require the combined reader count to be zero. Producer settlement intentionally
changes protected allocation state while preserving the reservation lifecycle.

A reservation binds a complete allocation reference, device, extent, range,
producer writer reference, consumer identity, attempt epoch `E`, prior lineage
`L`, and fresh reservation incarnation. Admission requires `L < E`, the exact
pending producer, and a same-context submission consumer with a later local ID.
The caller must independently authenticate the event-to-producer relationship
and the backend's success-gated execution contract; model values are not native
authority or initialized-data evidence.

Reservations remain in their arena until exact consumer quiescence. They never
migrate into ordinary stable-version reads. Their status follows the protected
allocation without a settlement-time scan or allocation:

| Allocation observation | Reservation status |
| --- | --- |
| Exact pending producer, epoch `E`, lineage `L` | Pending |
| Exact Unknown producer, epoch `E`, lineage `L` | Unknown |
| No pending writer, epoch `E`, lineage `E` | Success |
| No pending writer, epoch `E`, lineage `L` | NoEffect |
| Any other identity or version | Invalid |

Only Pending admits a new producer-bound reservation. Success and NoEffect may
retire the producer's writer slot, so resolved reservations do not require that
old slot to remain occupied. Continuous allocation exclusion protects their
epoch/lineage relationship even if the slot is reused for another writer.
Release accepts every legitimate lifecycle status but authenticates the exact
consumer reference and inert quiescence premise. Release never implies that the
consumer received successful input.

## Bounds And Cost

Both arenas have `reads` preallocated slots but share a total limit of `reads`
active records. Thus the active-read budget is unchanged; allocated metadata is
larger than the original single-arena journal. Counts include both kinds of read
for writer admission, allocation retirement and Unknown-writer disposal.

Construction uses O(allocations + writers + reads) storage. Canonical acquisition
and release take O(k) for k requests and are unchanged-on-error, including caller
output. Lookup is O(1); producer settlement preserves the existing journal's
member-roster complexity and does not scan consumers. Ordinary stable-reader
header rejection precedence is preserved before shared-budget checks.

## Verification And Integration Gates

CPU tests cover all producer outcomes, arbitrary release/settlement ordering for
fan-out sizes one through four, independent producers in one roster, canonical
atomic admission/release, shared capacity, stable-reader coexistence, writer and
allocation slot reuse, incarnation exhaustion, unchanged storage identity, and
whole-roster mutation/disposal exclusion when a later member is protected.
These are executable tests, not a formal refinement or hardware qualification.
The [signed-source CPU qualification](evidence/dev-producer-read-model-cpu-2026-09-21/README.md)
records the GNU/musl model, doctest and runtime regression results.

`context_producer_read_invariant_v1.rs` adds 25 obligations to the 156 inherited
reader/journal obligations. It specifies exact arena partitions, live reference
identity/incarnation, per-allocation counts, and the shared active-read ceiling.
It proves logical construction, a two-consumer nonempty witness, and conditional
preservation under Success/NoEffect settlement and Pending-to-Unknown marking
(including idempotent Unknown). Settlement frames stable readers and the outer
reservation storage. Resolved status depends on the protected allocation, not
retention of the old writer/member slots. A retained reservation contributes a
positive combined reader count; concrete mutation rejection is not yet refined.

The dedicated pinned checker runs the whole importing crate before and after
12 invariant-sensitivity mutations, requiring each negative to fail exactly the
named postcondition. These mutations test the invariant, not implementations of
acquisition, release, or settlement. The checker authenticates the recursive
source/tool closure and binds solver inputs to captured pinned bytes.
The [signed-source custody qualification](evidence/dev-producer-read-custody-verus-2026-09-21/README.md)
records the accepted dedicated campaign and the full registered proof run,
including all 691 standalone negative files and portable evidence validation.

`context_producer_read_lifecycle_v1.rs` adds 74 obligations to the 181 inherited
obligations. Executable logical status, lookup, capacity and ordered preflight
functions match their exact decisions, including error precedence. Acquire and
release commit loops implement exact contents relations; rejection frames the
whole logical state and caller output. Separate wrappers prove arena, count,
incarnation and shared-budget preservation. Stable-reader wrapper operations
preserve the producer arena and shared limit. Combined-count arithmetic is
bounded, and the ordered unread guard rejects a retained reservation.

Nonempty executable witnesses cover late-item acquire/release rejection,
two-item round trips, release-capacity rejection, slot reuse and stale references,
and release in all four legitimate producer statuses. Settlement witnesses
instantiate the conditional journal relations; they do not refine production
settlement preflight. Release leaves the journal unchanged in every status.
The dedicated checker runs whole-crate positives before and after 19 executable
body mutations, with strict single-postcondition diagnostics. The exact custody
module header is authenticated separately; arbitrary imports remain forbidden.
The [signed-source lifecycle qualification](evidence/dev-producer-read-lifecycle-verus-2026-09-21/README.md)
records both whole-crate positive brackets, all 19 executable controls, and the
complete registered proof run with all 691 standalone negative files.

The linked historical archives retain their original source and obligation counts.
`context_producer_journal_issuance_v1.rs` composes the custody
predicate with exact `reserved_count` and successful-registration history. Its
logical constructor/register/Reserved-abort wrappers preserve the combined
invariant, reader storage and unrelated Pending/Unknown chains. A constructor-based
reuse witness covers rejection and stale identity; a separate directly initialized
mixed-state fixture covers all four reservation statuses during register/abort in
a retired producer slot. That fixture is not production reachability. The
[signed-source issuance qualification](evidence/dev-producer-journal-issuance-verus-2026-09-21/README.md)
records the 201 whole-crate obligations (181 inherited and 20 new), all 13
executable mutation controls, and the complete registered runner, including the
updated inherited proofs and all 691 standalone negative files.

The [signed-source enrollment qualification](evidence/dev-enrollment-execution-verus-2026-09-21/README.md)
connects complete logical batch admission, sorting/search, rollback and commit to
producer custody and issuance preservation: two 263/0 positives and 21 exact
262/1 controls. Its constructor/enroll/register witness reaches Reserved, not
Pending. Production still uses the standard-library sorting/search operations.

`context_version_journal_begin_v1.rs` adds a separate raw Begin executor with
exact ordered preflight, prestate-derived scratch plans, sequential member and
allocation commit, and complete unchanged-on-error contents. It has no valid-state
precondition: malformed free lists retain production's sequential overwrite
semantics rather than silently acquiring a uniqueness premise. A general theorem
preserves issuance history and the exact Reserved count. Executable constructor,
enrollment and registration traces reach both empty and two-member Pending writers;
repeat Begin rejects without mutation. This raw root alone does not establish
general pending-custody or producer-reader preservation across Begin.

Production Begin now calls a private immutable preflight under the same exclusive
borrow as staging/commit. Its guard order and indexed-access bound are unchanged.
An independent ranked-fault oracle covers 4,356 paired malformed states and compares
the full transaction and storage identity. Wrapper tests cover combined-reader
rejection before raw writer/roster faults. These tests and the parallel logical
executor are not a compiler-checked Rust correspondence proof.
The [Begin development packet](evidence/dev-begin-execution-2026-09-21/README.md)
retains whole-crate proof brackets, executable-body controls and CPU receipts.

`context_version_journal_begin_custody_v1.rs` composes that exact raw transition
with pending-chain custody, stable leases, producer reservations and issuance.
The successful preflight derives distinct selected allocation/member slots;
the ordered combined-reader guard proves both selected counts zero. No global-idle
premise is required: unrelated Pending/Unknown chains, live stable readers and all
four producer statuses are preserved, including resolved producer-slot reuse.
The executable wrapper runs the complete combined scan, complete stable scan,
then raw Begin, framing the whole producer state on every error. Its five shared
guard declarations are authenticated against pinned lifecycle bytes and reverified
in the importing type universe, without modifying the historical lifecycle root.
Constructor witnesses reach empty and two-member Pending writers through this
wrapper with empty reader arenas; a separate production regression has a stable
lease and all four producer statuses present together. That test is not a formal
constructor-derived witness for the mixed state.
The [Begin custody development packet](evidence/dev-begin-custody-2026-09-21/README.md)
retains two 321/0 whole-crate positives, eight exact 320/1 executable controls,
and 822 model unit tests plus 27 doctests. Its mandatory portable audit binds the
exact regenerated inputs and receipts to Git objects. Counts include 289 inherited
obligations and must not be summed across campaigns.

`context_version_journal_settlement_v1.rs` adds exact logical retained-header,
member, chain, return-capacity and settlement-preflight executors without a
valid-state precondition. It also executes Unknown marking and its issued wrapper,
including whole-state rejection and object-identical repeated Unknown marking.
General conditional settlement and Unknown relations now preserve issuance/history,
the exact Reserved count and all retained readers without a global-idle premise.
The constructor witness reaches Pending through Begin, exercises accepted/rejected
settlement preflight, and executes Unknown; it does not execute Success/NoEffect
or derive their settlement relation from staging/commit. Capacity arguments remain
observations, not proved bindings to live Rust `Vec::capacity()` values.
Production has a journal-private immutable settlement preflight under the same
exclusive borrow as commit. A 1,080-case fault matrix checks exact result/state,
storage identity and indexed-access precedence; mixed-reader tests cover all target
outcomes with all four unrelated producer statuses and a stable lease.
The [settlement admission development packet](evidence/dev-settlement-admission-2026-09-21/README.md)
retains two 338/0 whole-crate positives, ten exact 337/1 executable controls,
823 model unit tests and 27 doctests, with mandatory exact-source/receipt auditing.
Its 321 inherited obligations overlap earlier campaigns.

`context_version_journal_settlement_commit_v1.rs` now executes the complete raw
settlement transaction: exact preflight, prestate-derived scratch staging,
sequential Success/NoEffect commit, member/writer returns and scratch restoration.
It has no valid-state precondition and exactly frames rejection. It preserves
unreachable members, unrelated forged backlinks, malformed free prefixes and
unused scratch tails rather than introducing a global-validity premise.
`context_version_journal_settlement_custody_v1.rs` derives the existing settlement
relation from that execution under pending custody, then preserves issued history,
stable leases and producer reservations without global idleness. Complete retained
chain coverage and the historical backlink selector are justified by custody,
not by raw preflight alone. Constructor/enroll/register/Begin traces now execute
both outcomes for empty and two-member writers, including rejection and repeat
settlement framing; their reader arenas are empty.
The [settlement execution development packet](evidence/dev-settlement-commit-2026-09-21/README.md)
retains two 360/0 whole-crate positives, ten exact 359/1 executable controls,
825 model unit tests and 27 doctests, with exact offline source/receipt auditing.
New production regressions cover 34 admitted-state cases, including malformed
untouched state, and preserve all seven vector storage identities and the
touched-chain indexed-access bound.
The 338 inherited obligations overlap prior packets.

Settlement's scalar return-admission body is now shared directly between ordinary
Rust and Verus through `settlement_return_body.rs`: both checked additions and all
five logical/storage/scratch bounds execute from the same included source. The
private production adapter captures actual live vector lengths/capacities after
retained-chain validation and before the unchanged indexed scratch scan. It does
not allocate, mutate state, create public authority or change failing-guard order.
The [shared scalar storage development packet](evidence/dev-shared-settlement-storage-2026-09-21/README.md)
retains two 367/0 whole-crate positives, ten exact 366/1 controls, 826 unit tests
and 27 doctests, including 6,996 independent widened-arithmetic boundary cases.
Macro diagnostics are bound to the actual included body, invocation, definition
site and wrapper postcondition. Its 360 inherited obligations overlap the preceding
packet. This closes one scalar source-correspondence slice, not the full Rust
settlement gate: the live callsite is authenticated and tested, but the standard
library's capacity implementation, allocator, unwind and remaining production
header/chain/scratch/staging/commit bodies are not refined by that packet.

Retained writer/header checks, exact allocation lookup, member validation, bounded
chain traversal and Pending-to-Unknown execution now use the same executable
source in ordinary Rust and Verus through `retained_bodies.rs`. Raw proofs have no
journal-validity or global-idle premise and establish exact admission results,
unchanged-on-error modeled contents and content-identical repeated Unknown marking.
Issued wrappers preserve history and reader custody under their explicit invariant
precondition. Constructor witnesses cover empty and two-member writers with empty
reader arenas. Settlement composition uses the new shared admission path, but its
scratch scan, staging and commit remain separately implemented production/model
bodies. The [shared retained development packet](evidence/dev-shared-retained-2026-09-21/README.md)
retains two 382/0 whole-crate positives, fifteen 381/1 executable controls,
833 unit tests and 27 doctests. Thirteen controls fail exact postconditions;
two fail exact supporting assertions. Its 367 inherited obligations overlap prior
packets. The pinned identity syntax adapter supplies no executable loop annotations
in Rust; the Verus invocation supplies only fixed invariant/decreases clauses.
Source-consistent non-exit diagnostic spans and substituted adapters are rejected
by the evidence checker. Physical pointer/capacity preservation remains CPU-test
evidence, not a theorem from modeled-content equality.

Settlement scratch-prefix scanning and staging now also use shared executable
Rust/Verus bodies in `settlement_scratch_bodies.rs`. Scalar return admission still
precedes scanning, and complete preflight still precedes staging under one borrow.
The stage retains its existing weak readiness precondition: no global-validity,
issued-provenance, uniqueness, terminal-head or physical-spare-capacity premise
was added. Exact intermediate plans and non-scratch/tail framing are proved even
for admitted bounded repeated prefixes. The [shared scratch development packet](evidence/dev-shared-settlement-scratch-2026-09-21/README.md)
retains two 392/0 whole-crate positives and ten 391/1 executable controls: five exact
loop-invariant failures and five exact postcondition failures. Its 382 inherited
obligations overlap prior packets. All 840 unit tests and 27 doctests passed;
new direct staging tests inspect nonzero prior lineage before commit erases scratch,
linked slot/field identity, malformed untouched state, dirty tails, zero-count
identity, cyclic prefixes and exact access counts. Pointer/capacity identity remains
CPU evidence rather than a physical-storage theorem.

The final settlement commit now shares its executable production body as well.
`settlement_commit_body.rs` retains the actual scratch `take`, allocation `as_mut`,
in-place lineage/backlink mutations, member clear and both free-list appends.
Its unchanged raw logical contract permits repeated member/allocation destinations,
arbitrary untouched malformed state and an in-bounds writer slot without requiring
stored writer identity. Success preserves sequential last-write-wins semantics;
NoEffect preserves current allocation lineage. The [shared commit development packet](evidence/dev-shared-settlement-commit-2026-09-21/README.md)
retains two 398/0 whole-crate positives and ten 397/1 executable controls, with 392
inherited obligations overlapping earlier packets. All 847 unit tests and 27 doctests
passed. Direct regressions cover aliases, repeated returns, dirty tails, weak writer
state and exact `4 * count + 2` commit work. Physical storage/allocation and unwind
semantics remain separate obligations; this is not whole-wrapper refinement.

The ordinary Rust journal and the proof now also compile the same declaration
tokens. Lossless, independently anchored views preserve all five scalars, seven
ordered vector contents and their complete entry/reference fields without a
valid-state premise. Exact historical error embeddings leave
`StorageAllocationFailed` explicitly unrepresented. The existing shared explicit
key comparisons are verified against these actual value declarations. The
[journal representation development packet](evidence/dev-journal-representation-2026-09-21/README.md)
retains two 452/0 whole-crate positives, twelve explicitly scoped one-function
negative controls, 850 unit tests and 27 doctests. Its 398 inherited obligations
overlap earlier packets. Derived comparison traits, public status/evidence wrapper
semantics, operation preservation of the content relation and physical storage
remain distinct obligations; declaration/view correspondence is not full-runtime
refinement.

The next [production retained-admission packet](evidence/dev-journal-retained-execution-2026-09-21/README.md)
instantiates allocation/header/member lookup and bounded chain traversal on those
actual declarations. Unconditional contracts relate each shared body to a pure
sequence-view decision; independent bridges establish exact historical decision
correspondence without assuming an allocated historical journal exists. Raw
malformed-state behavior is preserved. Qualification records two 465/0 whole-crate
positives, ten scoped one-function negatives, 855 unit tests and 27 doctests.
Actual-typed mutation and full-wrapper correspondence remain open.

These are scoped logical/shared-body proofs with invariant-conditional custody composition,
not whole-wrapper verification.
Complete retained-chain coverage is preserved by logical enrollment and Begin;
settlement/issuance composition is now derived from the executable logical commit.
Physical Vec storage, fallible allocation, panic/unwind behavior and production
Rust correspondence for the remaining bodies remain separate. Release bounds use an observed
capacity argument, not a proved binding to Rust `Vec::capacity()`. The acquisition
induction advances a hypothetical incarnation prefix; the concrete logical loop
updates its watermark once at the end. It does not establish invariants at every
intermediate machine state or atomicity under panic/unwind.

Before enabling runtime admission, the remaining work is:

1. Finish production Rust correspondence for enrollment, Begin, settlement,
   Unknown marking and the proved logical admission/release and unread guards.
   Shared executable source now covers the settlement scalar return guard,
   retained-chain admission/Unknown transition and settlement scratch scan/staging/commit.
   Remaining bodies include construction/enrollment and sorting/search, Begin,
   stable/producer reader admission/release and unread guards.
   Core journal declarations and content views are now shared and related, but
   only the explicit comparisons and retained admission bodies have actual-typed
   operation correspondence so far. Actual-typed Unknown/settlement mutation,
   remaining type/trait/wrapper correspondence, physical storage and fallible-allocation
   contracts, and explicit normal/unwind semantics remain open before treating
   the logical lifecycle as full-wrapper refinement.
2. Retain an exact Context event-to-producer writer/member binding and a distinct
   producer-reader root, with preallocated capacity before backend entry.
   Include it in Context cleanup, generated-operation exclusion and usage.
3. Retain producer result custody independently of public event lifetime.
   Reconcile the producer through its own backend completion before publishing
   consumer success. Do not infer producer success from a consumer result.
4. Add an audited, default-false backend contract for success-gated dependencies
   and retained producer results. Generic event support and Worker negotiation
   do not imply this guarantee.
5. Drive bounded producer-first dependency progress from consumer-only owner
   registration, with shared deadlines, deduplicated work and cancellation/error
   custody. Do not require an undocumented separate producer registration.
6. Qualify the journal-enabled pending dataflow on native XGMI, including event
   release, producer-first and consumer-first observation, failures and cleanup;
   then run matched HIP/HSA comparisons before claiming performance gains.

This is immediate runtime/backend admission with producer-bound custody, not
host-deferred submission, journal bypass, or a claim of GPU-side dependency
packets. Wider typed-launch inputs and Worker transport remain separate
integration work under the full parity objective.

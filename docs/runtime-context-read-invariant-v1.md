# Context Reader Arena Invariant V1

V4-J4 development composes the exact pinned J3 contents transitions with a
reader-arena invariant. The separate [qualification archive](evidence/dev-v4j4-reader-invariant-2026-09-17/README.md)
records executed checks; this contract is not itself acceptance evidence.
Native R125 CPU/test, Admission R118B C1-C3 and Resources R116/V3 remain
the accepted checkpoints. A1/A2, #182 and HIP/HSA parity remain open.

## Invariant

The predicate inspects the actual journal and reader sequences, independently
of any transition relation. Free slots are distinct, in range, and exactly the
complement of occupied leases. Every allocation's reader count equals the
number of occupied leases targeting that allocation. Each occupied lease names
its physical slot, a valid same-context consumer, a nonzero incarnation below
the next watermark, and an exact currently valid allocation/range/version.
Live incarnations are distinct, including across different consumers.

The predicate permits overlapping readers, repeated consumers, arbitrary free
stack order and `next_incarnation == u64::MAX`. The latter is a valid exhausted
state, not permission to wrap or reuse an incarnation.

Acquisition derives J3's selected-free uniqueness premise from this invariant.
The proof uses prefix sequence algebra with a ghost prefix watermark: the actual
commit loop installs all leases before advancing its watermark. It does not
incorrectly claim the full invariant holds between those individual stores.
Release clears exactly the selected leases, subtracts their multiplicities and
pushes those slots, preserving every retained neighbor. Rejections are unchanged
states. These are contents properties, not statements about physical vector
capacity, allocation failure, unwind or authenticated quiescence.

The count consequences prove zero readers exactly when no retained lease targets
the allocation, and prove a positive count has a present, non-pending allocation.
Every live reference resolves to its exact request and blocks unread admission.

## Composition And History

The field-based journal frame permits changes only outside allocations with
positive reader counts, preserving reader storage, context and allocation-table
extent. It derives preservation without assuming that post-state read validation
succeeds. Existing executable register/abort wrappers discharge that frame and
retain their exact J1 issuance results. This proves their reader effects, not the
full writer/member invariant from an arbitrary reader-valid state.

`ReaderStepV1` binds acquisition, release, registration and abort to those exact
executable relations, including failures. A finite sequence of actual modeled
contents satisfying these relations preserves the invariant at every boundary.
Successful acquisition outputs are projected into consecutive incarnation
intervals; all other steps emit no incarnations. The trace theorem proves strict
ordering between all distinct minted references, including within one batch.
It cannot be instantiated with an unrelated numeric watermark trace alone.

The initial trace state is reader-valid with watermark 1 and may already contain
enrolled allocations. General enrollment, writer membership, settlement,
retirement and Unknown disposal are not variants of this proved trace yet.
Neither the trace theorem nor a frame premise proves those missing transitions.
Production Rust/native refinement and historical correspondence remain separate.

The concrete executable formal witness constructs an empty journal, explicitly
pops and installs one fixture allocation, acquires the same range for two
consumers, releases them in reverse order, and reuses the first slot with a new
incarnation. It checks last-reader exclusion and rejects the old reference.
Fixture enrollment is not a production enrollment proof; the witness is a
verified call chain, not a separately assembled ghost trace.

## Qualification Contract

The whole proof has 155 obligations: 127 inherited through the unchanged pinned
J3/J2/J1 sources and 28 new. The checker recursively authenticates and audits
that exact include closure. It adds one test-only executable invariant-preserving
subject, so its two campaign positives must report exactly 156 verified and zero
errors. Each of sixteen reversible invariant-sensitivity mutations must report
155 verified and one exact, source-located postcondition error.

These mutations corrupt free indices, counts, physical slot, consumer/allocation/
device identity, range, version, incarnation uniqueness, watermark or retained
allocation presence. The subject has only the invariant as its postcondition:
J3's stronger exact relation cannot mask a missing invariant check. These are
invariant-sensitivity tests, not mutations of production reader bodies; the
existing J3 executable-body campaign remains an independent integrated gate.
Compiler, timeout, unexpected or additional errors do not qualify as negatives.

## Executable Witnesses

The Rust whole-state inspector independently checks the complete partition,
counts, lease identities, live versions and incarnation uniqueness. A lifecycle
test retains readers on two allocations while a third passes through successful
settlement, no-effect settlement, Unknown disposal and slot reuse. It checks
reader exclusion, partial release, retirement and re-enrollment, and unchanged
reader-storage addresses/capacities after every transition. The existing
4,000-step multi-consumer trace also checks this whole-state invariant.

These Rust tests do not mechanically refine the Verus program. In particular,
concrete enrollment, writer membership, settlement, retirement and Unknown
disposal still need executable Verus correspondence. A field-level preservation
lemma cannot discharge those transition obligations merely by assuming its
frame premise.

Constructor-only traces contain no readable allocations. The explicit fixture
and real Rust enrollment tests keep nonempty behavior separate from the still
missing mechanical correspondence for production enrollment.

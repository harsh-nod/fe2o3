# Context Version Journal Settlement V1: Design Draft

Status: reviewed design decisions for V3, not a frozen implementation contract,
implemented API or qualification result. The parent is the locally accepted
R113/V2 [membership model](runtime-context-version-membership-v1.md).
See the [swarm map](runtime-swarm-next-packets.md). V3 contract freeze is next;
R113 acceptance does not qualify settlement or production integration.

## Boundary

Extend the executable model's retained writer/member state only. Do not add a
production Context consumer, allocator, ID map, authenticated receipt producer,
native completion path, recovery operation or cross-run reuse permission.

Proposed operations are `settle_success`, `settle_no_effect` and `mark_unknown`.
Success and NoEffect take a borrowed inert projection binding the complete
writer reference: slot, Context generation, local identity and writer kind.
These projections describe a model premise; freely constructing one supplies
no runtime authority. V5 will separately require private consuming authenticated
receipts, custody-preserving rejection and production sealing on invariant error.

Borrowed evidence remains unchanged on rejection. Success removes the exact
writer from the journal, making replay invalid even though the descriptive
projection remains in its caller's possession. Slot reuse cannot revive it:
the existing nonwrapping writer IDs and registration watermark still apply.

## Transitions

| Operation | Required State | Effect |
| --- | --- | --- |
| Success | Exact Pending writer, matching projection | Set each retained allocation's lineage to that member's admitted attempt epoch; clear exact backlinks; return the exact member slots and writer slot. |
| NoEffect | Exact Pending writer, matching projection | Preserve every prior lineage and already-burned attempt epoch; clear exact backlinks; return the exact member slots and writer slot. |
| Unknown | Exact Pending writer | Change only the writer phase, retaining key, head/count, all members, backlinks, epochs and lineage. |
| Repeated Unknown | Exact Unknown writer | Revalidate the touched retained chain, then succeed without mutation. |

Unknown is sticky. Success, NoEffect, Begin and pre-effect abort cannot release
or overwrite it. Recovery belongs to V7 and needs a separate contract. Allocation
lookup continues to identify the retained writer for Pending and Unknown;
writer lookup distinguishes those phases. Merely dropping a reference does not
release capacity or roll back an epoch.

An empty Pending writer consumes W capacity and no member capacity. Success or
NoEffect returns its writer slot only. Empty Unknown retains the writer slot.
None of these operations changes the registration watermark, enrollment,
allocation-free stack or Reserved-only count. They affect no disjoint writer.

## Validation Order

Before any scratch or persistent-state write, validate:

1. Exact writer reference and eligible phase, otherwise `InvalidReference`.
2. Success/NoEffect evidence equality, otherwise the proposed
   `SettlementEvidenceMismatch` error.
3. Retained header and complete touched chain, otherwise `InvalidState`.
4. For releasing settlements, checked return-stack headroom and sufficient
   vacant scratch cells, otherwise `InvalidState`.

The header has exactly `count` members, with `count <= A`; an empty chain has
no head. Starting at its head, inspect exactly `count` occupied, in-bounds nodes.
Every node must bind the exact writer and exact allocation reference, and its
allocation must point back to that member. Full allocation keys must increase
strictly; the final `next` must be None. This rejects repeated nodes, cycles,
truncation and overlong chains in the touched roster without scanning all arenas.

Each member's admitted epoch must equal its allocation's attempt epoch, and its
prior lineage must equal that allocation's current lineage. Require
`prior_lineage < attempt_epoch`. Do not require an epoch to equal prior lineage
plus one: earlier NoEffect attempts legitimately create gaps. V2 does not retain
the pre-Begin attempt epoch; exact increment correctness is a transition
preservation obligation, not a fact reconstructible from lineage alone.

Success/NoEffect need room to return members, not that many currently free
members. Use checked arithmetic for `free.len() + 1 <= W` and
`member_free.len() + count <= A`, also bounded by actual vector capacities.
Validate selected scratch cells as vacant. Unknown does not return slots or use
scratch, so it does not require release headroom or write a temporary plan.

## Commit And Complexity

Reuse the seven V2 vectors. `MemberEntryV1` retains the writer/allocation
references, prior lineage, admitted epoch and next link. `BeginMemberPlanV1`
retains the member slot, allocation reference, prior lineage and admitted epoch,
but no writer reference. These existing fields are sufficient for settlement;
no additional per-member field is currently required.

After complete validation, plan and commit under the same exclusive mutable
borrow. There must be no remaining allocation, callback, native operation or
fallible computation. Return members in canonical chain order for deterministic
tests, then return the writer slot. No half-settlement may become observable.

Each operation targets O(k) touched work, independent of unrelated A/W, with no
commit-time vector growth. Keep complete arena scans in test auditors, not the
transition. Fixed-k tests must pin the actual implementation's counted work;
source guards and counters alone are not authenticated complexity proofs.

## Invariant Assumptions

Touched checks cannot establish uniqueness of unrelated free-stack entries,
absence of unrelated orphaned members, or the truth of coordinated substitutions
to matching private fields. Global free/occupied partitions and free-stack
uniqueness remain initialization/preservation invariants, checked by a complete
test auditor and eventually V4 proofs. Do not promise resistance to arbitrary
hostile corruption of the model's private memory, or add O(A/W) scans to imply it.

NoEffect is an external premise at this model boundary. An ordinary backend
error alone does not authenticate that no effects occurred. Unknown is not an
authenticated quiescence or disposal receipt.

## Required Tests

- Independent reference transitions for Success, NoEffect and Unknown, with
  complete arena/free-list/backlink auditing after every accepted action.
- Complete rejection snapshots for first/middle/last touched corruption,
  foreign/stale references, evidence mismatches, malformed head/count, cycles,
  truncated/overlong chains, inadequate return headroom and occupied scratch.
- Success versus NoEffect lineage and burned epochs, including several NoEffect
  gaps, MAX exhaustion after settlement and unaffected older/disjoint writers.
- Evidence replay after writer-slot reuse; no aliasing through kind, Context,
  local identity, slot or allocation substitutions.
- Unknown idempotence after chain validation, retained capacity/backlinks,
  forbidden ordinary settlement/Begin/abort and empty-writer behavior.
- All seven storage pointers/capacities unchanged; fixed-k work across larger
  unrelated allocation and writer populations.
- Decisive compiled transition/ownership negatives, exact source restoration,
  applicable full regression/lint gates and independent evidence review.

Formal, production/native and performance qualification remain separate. V4
must authenticate named properties and their Rust/model correspondence; no test
inventory or model fixture can supply that evidence.

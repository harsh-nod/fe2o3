# Preparation policy and direct comparison qualification — 2026-09-26

This checkpoint adds shared preparation-allocation policy and a private,
resource-metered direct-comparison component. It does not complete the whole-source
argument producer, bounds factory, or public kernel-authoring route.

## Shared policy without changing ordinary allocation behavior

Private item macros share the existing reserve/push decision body between the
ordinary preparation adapter and a sealed assertion-resource adapter. The original
preparation methods expand to their exact original bodies: no new runtime wrapper
or closure is inserted into their existing caller graph. Checked length, fitting
capacity, work debit, reservation, post-reservation checks, and failure ordering
are preserved. Ordinary growth stays amortized; paid growth stays exact.

The assertion adapter adds strict-owner and sticky-denial admission. It does not
silently substitute the assertion evaluator's different vector-growth policy.
This new adapter is a building block; future callers must explicitly admit their
concrete generic and assertion frames before using it.

Thirteen controls cover both policies, exact work/storage boundaries, overflow,
foreign-ledger and prior-denial refusal, retained allocations and unwind cleanup.
The generic-wrapper proposal and an initially stale frame inventory are preserved
as historical drafts; this checkpoint uses the macro implementation and corrected
inventory.

## Direct comparison in source order

The new owner scans the actual function's blocks and statements in order and uses
the qualified uniform-operand helper for left then right operands. The ordinary
comparison loop is unchanged and retained independently as the decision oracle.
Earlier argument-slot, counter, operation and SSA mutations are not rolled back
when a later operand or destination check refuses.

Predicate candidates are attached to the owner before fallible allocation.
Duplicate predicates retain both physical payloads; a conflicting candidate stays
attached while the owner becomes terminal. Reservation failure does not discard
an already-owned candidate. Successful and failed owners cannot restart their
preparation pass. The caller must retain this owner and its original prefix
storage through postflight, then drop physical payloads before releasing credits.

Twenty-five controls compare the original loop and operand oracles, source order,
duplicate/conflict behavior, partial mutations, work/storage denial, source and
ledger mismatch, SSA refusal after reservation, and unwind retention.

## Accounting scope and remaining authenticity

Checked logical accounting has distinct typed rows for the selected model
accessors, including their receivers and actual return values. The accessor
correction has an independent per-row and constructor-formula control. No unrelated
row allowance substitutes for a missing call.

This is selected-source logical work/storage accounting, not a native stack, RSS,
allocator-internal or GPU performance proof. Standard-library comparison/iterator
implementation frames are outside this model. Bounded scalar predicate equality
does not establish general recursive heap equality.

A successful view is DATA, not an authenticated all-producer continuation.
Function and original budget/work identity checks do not authenticate a supplied
owned-credit counter or prove the provenance of all caller-supplied rosters.
The enclosing authentic factory must establish those joins and the full lifetime.

## Qualification

- 38 new controls passed: 13 policy and 25 direct comparison.
- 331 model and 2,916 backend tests passed; 189 backend tests remain ignored.
- Backend build and 83 JavaScript controls passed.
- Both ordinary compilation ladders passed. All 38 lossless observation bodies
  and 52 artifacts match the preceding uniform-operand checkpoint exactly.

Regression receipt: `f17410b02a30ff6d0d3e65891e12a012dcb06bbccca5a396127892cdd645a970`.
Normal-ladder receipt: `05ce1a13b5175ca3d3645716d51fdd55a4e5bbe378fbc9428e333b288b109895`.
Normal-comparison receipt: `eed65c7bf4f9f19e965d823a3fd11619eee8b713d5b563c6877701ebdb3f4687`.

## Remaining route

Lazy proof ownership, a joint source-ordered bounds driver, the remaining
induction/body/switch/launch producers, the final extent-count rule, authentic
all-producer handoff, and mandatory verification/normal continuation remain open.
No ordinary route is redirected to this private component by this checkpoint.
The proof-owner draft remains separately reviewed and is not included here.

No target dispatch, stopped-wave capture, edited-source promotion, new nominal
LLVM continuation, public capture gate or global compiler pin is supplied.
Accepted broad exits remain **M1/V1/V2/U1/U2/U3 (6/18)**.

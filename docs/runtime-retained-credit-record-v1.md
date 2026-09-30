# Retained Credit Record Predicate

The production `Record::matches_retained_charge` method and the Verus root
`retained_credit_record_v1.rs` invoke the same complete expression from
`fe2o3-resource-accounting/src/retained_charge_body.rs`. Extraction preserves
the existing borrowed, read-only implementation and its short-circuit order.
It changes no public accounting API or resource lifetime.

## Theorem

The returned boolean is true exactly when the requested owner is nonzero, the
record has that owner, its phase is `Retained`, and every one of the nineteen
resource-vector coordinates equals the expected charge. Zero charges remain
valid when the other conditions hold. Reserved, quarantined and vacant records
do not match. The method borrows the record and cannot change its contents.

The source checker pins the complete accounting Rust roster, explicit model and
build inputs, concrete Record/Phase schemas, vector declarations and actual
production invocation. Record really is Copy metadata in production; the proof
does not manufacture a Copy representation of an opaque resource owner.

The vector equality bridge uses the pinned vstd array indexing and structural
extensionality specifications. Rust derive lowering, Rust/Verus lowering and
those library specifications remain trusted compiler/library boundaries. The
root has no local `assume`, `admit` or external proof body and runs with
`--no-cheating`. This is not a compiler or machine-code correctness proof.

## Qualification

Signed candidate `7dc2fc256`, integrated at `5cf8266b6`, passes a twenty-stage
component campaign: three full seven-obligation positive runs, including an
exact relocated three-file closure, and eleven actual-body logical negatives.
The three positive counts overlap; they are not twenty-one distinct theorems.
Negatives remove or invert owner, phase or charge checks, compare only the first
or last coordinate, or reject valid records. Compiler errors and timeouts cannot
stand in for logical counterexamples.

Both 190-file verifier-release checks, the 6278 selected signed-source blob
binding, source/tool continuity and twenty fresh same-namespace process-group
closure checks pass. The earlier rejected proof attempt is preserved; its repair
adds the array-equality bridge without weakening the postcondition. Separate
pre-signing CPU qualification passes all 72 accounting library tests. All
fourteen accounting Rust files and their build inputs are byte-identical to the
signed candidate; only five proof/checker/manifest inputs changed after that CPU
run. This binding is not a fresh signed-candidate CPU execution. These results
do not constitute a new merged-runtime or native qualification.

The R75 compatibility repair adds the shared body to its source manifest and
updates reviewed source hashes. Its eight executable proof inputs, unit counts
and mutation policy are unchanged. Nine lightweight controls pass; this packet
does not claim a fresh R75 solver campaign.

## Remaining Boundary

This predicate does not establish Arc identity, successful locking, lock poison
handling, domain ancestry, ledger conservation, token ownership, session
liveness or freshness between observations. A matching record alone is not
Context allocation authority or an authenticated producer input. The next
refinement is the actual domain post-lock observation over checked ancestry,
slot and leaf identity. The enclosing account and runtime checks remain enabled
and keep their separate proof status.

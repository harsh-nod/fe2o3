# Production middle-end evidence V5

`ProductionMiddleEndEvidenceV5` is the canonical record emitted by new rustc
production projections. V4 remains immutable and decodable for compatibility;
V5 uses a distinct magic, version, domain, policy, limits, identity domain, and
Rust types.

## Bound facts

V5 serializes the exact clean order of all eight mandatory V2 passes:

1. tensor layout
2. memory bounds
3. atomic legality
4. race freedom
5. hierarchical ownership
6. barrier convergence
7. workgroup memory
8. semantic refinement

The record includes exact declared and proved counts for `TotalView` and
`CollectiveContributions`. A zero-count clean report is represented as absence
of a coverage proof, never as vacuous success. A collective contribution proof
establishes participation accounting only; it does not establish the operator,
identity, ordering, final value, or termination of a collective.

Six additional counters bind the semantic pass: declared and proved reference
equalities, output-effect refinements, and finite collective-value contracts.
The collective contracts describe target-neutral finite folds, bounded
recurrences, and permutation gathers with explicit domains, step bounds,
evaluation order, numerical policy, and coverage binding. Zero declarations
remain absence of evidence. V5 rejects any declared/proved mismatch.

V5 also binds every counter in `ProductionTypedSemanticObligationSummaryV2`.
While the owner-held PLIRON graph is live, construction independently walks the
retained typed ranked recipe and the actual
`SemanticExpressionCommitmentOp` sequence. Construction requires exact ordered
digest equality and stores the two counts plus a domain-separated digest of the
ordered commitments.

## Authority boundary

The typed reconciliation says that PLIRON retained the same canonical typed
expression commitments carried by MIR operator-congruence obligations. It does
not by itself say that every retained root participates in a proved obligation,
interpret arbitrary arithmetic, prove IEEE target values, justify lowering,
authenticate decoded bytes, prove total program correctness, or grant artifact,
publication, load, or launch authority. Every authority accessor on live and
inert V5 evidence remains false.

Strict decoding validates aggregate limits before allocation, all fixed fields,
pass order and clean status, authority bits, coverage equality, typed-summary
invariants, semantic obligation equality, commitment reconciliation, canonical
ranked IR, terminal identity, and byte-for-byte canonical re-encoding. Decoding
yields an inert value and does not recreate live producer custody.

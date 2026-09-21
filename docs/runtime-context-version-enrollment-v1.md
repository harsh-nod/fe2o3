# Context Batch Enrollment Prerequisites V1

This development packet addresses two executable portions of
`ContextVersionJournalV1::enroll_allocations`: its ordered admission prefix and
its final allocation-write/truncate loop. It does not verify the complete batch
operation or advance a native runtime acceptance milestone.

The proof includes the pinned J4 reader invariant, J3 commit, J2
preflight and J1 issuance sources. It introduces no authority, allocation-key
issuance mechanism or native operation.

## Admission Prefix

`enrollment_header_exec_v1` follows the production order through the guard on
the free-list length, before allocation replay search:

- Roster capacity and output length precede output-vacancy validation.
- Each entry checks both contexts before allocation ID, device ID and extent.
- Entry metadata is checked before its canonical-order comparison.
- Caller order determines the first failing entry.
- An empty batch returns successfully before checking free-list length.
- Every return preserves the complete caller output.

Canonical comparison is lexicographic by `context_generation`, then `local`,
matching the declaration order used by Rust's derived `Ord` for
`ContextAllocationKeyV1`.

Success means only that this prefix accepts. It does not establish available
allocation capacity, absence of replay, free-slot uniqueness, a final output
roster or permission to commit.

## Conditional Commit

`enrollment_output_bound_v1` states a sufficient subset of the middle-phase
properties: exact entry-to-output identities, reversed free-suffix selection,
in-range vacant selected allocation slots, distinct selected slots and the
remaining free-prefix length. No function in this packet claims that production
sorting or replay search establishes this predicate. It does not require
selected-versus-retained-prefix disjointness, absence of replay, canonical
admission or allocation-arena reachability. Reader preservation needs none of
those additional properties; the resulting allocation free partition is not
proved.

Under that predicate and the existing reader invariant,
`enrollment_commit_suffix_exec_v1` executes the production final loop: install
each entry with epoch and lineage zero and no pending member, then truncate the
allocation free stack. Its postcondition binds the exact recursive sequence of
allocation writes, the exact retained free prefix, all untouched journal fields
and unchanged reader storage contents.

`vacant_allocation_has_no_readers_v1` derives zero reader count from the J4
invariant and actual vacancy. Prefix induction then proves that none of these
writes modifies a positively read allocation. The J4 field-frame theorem derives
reader-invariant preservation; positive-reader framing is not assumed as a
commit precondition.

This conditional result neither proves the allocation/member arena invariant
nor connects the admission prefix to the commit suffix.

## Sorting Boundary

The installed pinned Verus library supplies ghost `Seq::sort_by` and a proof of
its ordering and multiset properties. It supplies no executable specification
for Rust slice `sort_unstable_by_key` or `binary_search_by_key`. A ghost sorting
theorem is not a refinement of those production library calls.

The production middle phase scans existing allocations with binary search,
temporarily writes and sorts output references by physical slot, detects both
selected-slot duplicates and overlap with the retained free prefix, restores
all-None output on rejection, and restores canonical key/slot associations from
the unchanged reversed free suffix in a linear pass.
Those operations, including the error restoration, still require executable
correspondence. This packet does not replace them with a different sorting
algorithm, repeated single enrollment, an assumed permutation oracle or an
external-body contract.

The final restoration formerly used a second sort. Its replacement makes that
step O(k), without changing the whole-operation asymptotic bound. CPU qualification
covers every permutation of five free slots and all six batch sizes, with an
existing allocation and distinct incoming metadata: 720 cases. This is a tested
optimization, not a formal refinement of the missing middle phase or a measured
performance claim.

Consequently there is no end-to-end enrollment-based reader witness yet. Full
membership, settlement, retirement and Unknown-disposal trace composition,
freshness of caller-supplied allocation identities, physical storage/capacity,
unwind behavior and production Rust/native refinement remain separate work.

## Qualification

The [2026-09-21 qualification](evidence/dev-enrollment-qualification-2026-09-21/README.md)
records 814 passing model tests, two intentionally ignored benchmark-style tests,
27 passing doctests, formatting and Clippy for the linear restoration. The
standalone prerequisite checker was refreshed for the additional inherited
reader-prefix lemma: both whole-crate positives passed with 169 obligations
(156 inherited, 13 local), and all seven controls reported 168 verified with
only the intended postcondition error. The proof scope remains unchanged.

The following results and archived counts are historical.
The standalone `verus/check-journal-enrollment.py` campaign on 2026-09-18 passed
two whole-crate positive runs with **168 verified, 0 errors** each: 155 inherited
obligations and 13 new obligations. Seven executable-body mutations each produced
exactly **167 verified, 1 error**, located at the intended postcondition:
foreign-context handling, allocation-error identity, shape-error identity,
empty-batch precedence, rejection output preservation, retained free prefix and
the unchanged writer watermark.

The actual command, run from the repository root, was:

```sh
python3 -B crates/fe2o3-runtime-model/verus/check-journal-enrollment.py \
  --verus /home/harsh/.local/opt/verus/verus --timeout 180 \
  --output /home/harsh/.codex-tmp/fe2o3-enrollment-prereq-20260918-qualified
```

The [portable evidence packet](evidence/dev-context-enrollment-prereq-2026-09-18/README.md)
preserves these machine-local receipts without changing their bytes. They retain
each case source, command, solver output,
process-group cleanup result, environment and before/after input identities.
The complete pinned Verus distribution passed closure checks before and after;
all 27 recorded input identities were unchanged. All eleven owned invocations
reported their process groups absent after completion. The archive's portable
audit checks historical manifest integrity separately from optional current
source matching; neither is a Rust/native correspondence campaign. An earlier
attempt stopped after two passing cases on an ambiguous mutation anchor; its
partial receipts remain separate and do not count as a successful campaign.
The preliminary checker bytes and controller terminal output were not retained;
its two cases receive offline reanalysis under the retained corrected checker.

The historical J1-J4 pins and campaigns remain unchanged. This focused checker
is not yet part of the shared verification runner. No Cargo or GPU test was run
for this packet.

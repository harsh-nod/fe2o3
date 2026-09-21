# Context Batch Enrollment V1

The current development candidate verifies a complete executable enrollment
operation over logical journal contents. It composes admission, replay search,
capacity and free-slot checks, temporary output sorting, rollback, canonical
restoration and commit. It preserves the combined producer, stable-reader,
pending-chain custody and writer-issuance invariant without an idle-state premise.
This is not production Rust correspondence or native runtime acceptance.

The root includes the pinned producer-journal issuance composition and its five
dependencies: 201 inherited obligations and 62 local obligations, for 263 in
each whole-crate positive run. Counts overlap importing campaigns and must not
be summed. It introduces no authority, allocation-key issuance mechanism or
native operation.

## Complete Logical Execution

`enrollment_journal_exec_v1` requires no reader or custody invariant. Its exact
decision relation covers malformed logical states as well as admitted ones:

- Roster capacity and output length precede output-vacancy validation.
- Each entry checks both contexts before allocation ID, device ID and extent.
- Entry metadata is checked before its canonical-order comparison.
- Caller order determines the first failing entry.
- An empty batch returns successfully before checking free-list length.
- The nonempty free-length guard precedes full-key replay, then capacity.
- Selected slots must be in range and vacant before writing temporary output.
- Duplicate selected slots and selected/retained-prefix collisions reject.
- Every rejection leaves the entire logical journal and caller output unchanged.
- Success restores the exact canonical key/reversed-free-suffix association,
  installs zero-epoch/zero-lineage entries, and retains the exact free prefix.

Canonical comparison is lexicographic by `context_generation`, then `local`,
matching the declaration order used by Rust's derived `Ord` for
`ContextAllocationKeyV1`. Replay compares that full key, not just the local ID.
Unrelated corruption in the retained free prefix is deliberately not rejected
unless it overlaps a selected slot; this preserves the existing API behavior.

The executable helpers implement a constant-extra-storage max-heapsort and two
lower-bound searches. Heap construction, hole repair, root maximality, sorted
suffix growth, complete-reference permutation, bounds and termination are
proved. No external sorting contract, permutation oracle or quadratic scan is
assumed. Equal-slot ordering is unobservable: duplicates reject and restore the
original all-None output. Success has unique physical slots.

## Custody And Issuance

`enrollment_issued_exec_v1` wraps the raw logical execution. Given the existing
combined invariant, it preserves allocation/free partition, all old occupied
entries, retained Pending/Unknown chains, stable-reader and producer reservation
storage, count equations, incarnation fields, combined reader budget, reserved
writer count, registration watermark and the same writer history.

Every previously valid producer request retains its exact status: Pending,
Unknown, Success or NoEffect. Resolved requests do not require their old writer
slot to remain live. These are final-state invariants; they are not asserted
during the allocation-write loop before the free stack is truncated.

The constructor witness actually calls construction, two-item enrollment,
replay rejection, a later enrollment with a lower unused key, and writer
registration. It checks exact output slots and metadata. This is not yet a
constructor-to-Pending witness, nor a nonempty enrollment witness starting with
all four active producer statuses; the preservation theorem is parameterized
over such valid prestates, not restricted to the constructor fixture.

There is no allocation-key watermark. Historical freshness after retirement
remains the caller's responsibility. No proof turns inert identities or modeled
completion facts into native authority.

## Production Boundary

The installed pinned Verus library supplies ghost `Seq::sort_by` and a proof of
its ordering and multiset properties. It supplies no executable specification
for Rust slice `sort_unstable_by_key` or `binary_search_by_key`. A ghost sorting
theorem is not a refinement of those production library calls.

Production still uses those standard-library operations. The new verified
heapsort/search helpers are in the executable Verus model only; they have not
replaced the Rust helpers. Source correspondence, differential CPU tests and
scoped cost measurements must precede that integration. This is not literal
same-source compilation or machine-code refinement.

The final restoration formerly used a second sort. Its replacement makes that
step O(k), without changing the whole-operation asymptotic bound. CPU qualification
covers every permutation of five free slots and all six batch sizes, with an
existing allocation and distinct incoming metadata: 720 cases. This is a tested
optimization, not a formal refinement of the production middle phase or a measured
performance claim.

Full begin-write, settlement, retirement and Unknown-disposal trace composition,
physical storage/capacity, fallible allocation, unwind behavior and production
Rust/native refinement remain separate work. The candidate does not advance a
native milestone or establish HIP/HSA parity or performance.

## Qualification

The standalone `check-journal-enrollment.py` now requires two whole-crate
263/0 positives and 21 body-only mutations, each with 262 verified and exactly
one intended postcondition failure. It authenticates the six inherited sources,
checker chain and complete pinned Verus distribution; captured pinned bytes
produce the solver inputs. Input identities are checked around every case.
Partial runs, other diagnostics, compiler failures and timeouts are rejected.
The local classifier self-test rejects 43 adverse results, and the source-audit
self-test rejects 15 adverse sources in addition to inherited self-tests.
This remains a standalone campaign, not part of the shared verification runner.

The [signed-source execution qualification](evidence/dev-enrollment-execution-verus-2026-09-21/README.md)
records both 263/0 positives and all 21 exact 262/1 controls on clean signed source
`97ea2f97a2473bb8ebc1944a8efff18866d3b72a`. Its portable validator reconstructs
source and mutations from Git, checks raw receipts and the packet manifest, and
passes 26 adverse-evidence tests. Development solver probes are excluded.

### Historical Prerequisites

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

The historical J1-J4 pins and archived campaigns retain their original scope.
No Cargo or GPU test was run for the 2026-09-18 prerequisite packet.

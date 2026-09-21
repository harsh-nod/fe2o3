# Raw Begin Execution Development

This packet records the raw Reserved-to-Pending logical transition. It does not
qualify pending producer-to-consumer runtime admission or advance a native milestone.
The implementation checkpoint is `27af75e2528502ae040f0b5bd46c55a0270c2696`.
`SOURCE` binds the same implementation and the final packet tools to Git objects.
The runs began before those source commits; this is not a clean-commit campaign.

## Scope

- Production now separates immutable preflight from staging/commit, under the
  same exclusive borrow. Error order, allocation behavior and indexed-access
  bounds are unchanged.
- The parallel logical executor proves exact raw preflight, prestate-derived
  plans, sequential commit and complete unchanged-on-error modeled contents.
  No valid-state precondition hides malformed arena behavior. Duplicate selected
  free slots have production's sequential overwrite semantics, not invented
  admission rejection or an implicit uniqueness premise.
- A general theorem preserves issuance history, writer occupancy and the exact
  Reserved count. Constructor/enroll/register/Begin witnesses reach empty and
  two-member Pending writers and reject repeated Begin without mutation.
- General pending-custody preservation, combined-reader preservation across
  Begin, settlement composition, Rust source correspondence, physical storage,
  fallible allocation and panic/unwind behavior remain open. A logical witness
  is not a production-to-machine refinement or a native admission authority.

## Retained Checks

`proof/` contains two whole-crate positives at **289 verified, 0 errors**:
263 inherited obligations and 26 local obligations. Counts overlap importing
campaigns and must not be summed. Eight executable-body controls must each fail
exactly its named postcondition with **288 verified, 1 error**:

| Control | Required rejection |
| --- | --- |
| `exact_context` | Missing raw allocation context check |
| `busy_precedence` | Wrong destination error identity |
| `member_capacity` | Wrong member-capacity error |
| `stage_plan` | Destroyed staged plan |
| `reserved_count` | Incorrect committed Reserved count |
| `watermark_frame` | Changed registration watermark |
| `error_frame` | Mutation on raw rejection |
| `success_result` | Wrong result after successful commit |

The checker pins inherited source/checker bytes, audits the exact candidate body,
authenticates generated solver inputs before/after each run, and freezes completed
case receipts. The whole pinned Verus closure is checked before and after:
190 files, 129019839 bytes, release `0.2026.08.09.92f466f`. Runs use default resource
limits, four solver threads, a 180-second deadline, and `--no-cheating`.
`selftest.py` rejects 28 adverse result/diagnostic changes using an actual negative.

`cargo/` records **821 unit tests passed, 2 ignored; 27 doctests passed**, formatting
and Clippy with warnings denied. The new ranked-fault oracle covers 4,356 paired
malformed states and exact successful/rejected snapshots including storage
identities. Other tests cover empty/partial rosters, unrelated Pending and Reserved
writers, and combined-reader precedence before raw writer/roster/destination faults.
The reused Cargo recorder is
`docs/evidence/dev-enrollment-transaction-2026-09-21/cargo-checks.py`; all its inputs,
including the external `context.rs` test fixture, are bound to `SOURCE`.

No shared proof runner, musl campaign, GPU test or performance comparison was run.
The checker is standalone, not registered in the shared runner. There are no new
assumptions, axioms or external bodies. No MI300X processes or files were created.

## Audit And Reproduce

The offline audit requires a Git repository containing the `SOURCE` objects, not
the original scratch directories or Verus installation:

```sh
python3 -B docs/evidence/dev-begin-execution-2026-09-21/audit.py --repo "$REPO"
python3 -B docs/evidence/dev-begin-execution-2026-09-21/selftest.py \
  --repo "$REPO" --proof docs/evidence/dev-begin-execution-2026-09-21/proof
```

To rerun the logical campaign, supply the pinned Verus installation and a new
output directory to `check.py --repo "$REPO" --verus "$VERUS" --output "$OUTPUT"`.
`SHA256SUMS` covers every packet file except itself. The auditor reconstructs
signed-source inputs, all mutations and complete file rosters, checks exact
whole-crate diagnostics and command receipts, and verifies Cargo input bindings
and terminal test summaries. It validates retained evidence; it does not rerun
the solver or establish native correctness.

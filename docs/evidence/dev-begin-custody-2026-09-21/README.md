# Begin Custody And Ordered Readers

This development packet composes raw Begin execution with pending-chain custody,
stable leases, producer reservations and writer issuance. It does not enable
pending producer-to-consumer runtime admission or advance a native milestone.
`SOURCE` binds the implementation, packet tools and CPU inputs to Git objects.
The runs began before that commit; this is not a clean-commit campaign.

## Scope

- General preservation covers an arbitrary issued producer journal after the
  complete ordered unread guards, without requiring global idleness. A successful
  raw preflight derives distinct selected allocation/member slots and a canonical
  chain; uniqueness is not silently added to the raw executor's contract.
- Begin preserves every unrelated Pending/Unknown chain, the exact member/free
  partition and allocation coordinates. Selected allocations have both reader
  counts zero. Existing stable leases and Pending, Success, NoEffect and Unknown
  producer reservations remain valid, including a resolved producer's descriptive
  writer slot being reused by the new writer.
- The executable wrapper performs the complete combined-reader scan, then the
  complete stable-reader scan, then raw Begin. Its exact result relation frames
  the entire producer contents on every rejection and preserves issuance/history,
  shared read budget and the reader arenas on success.
- Constructor/enroll/register traces reach empty and two-member Pending writers
  through the wrapper and reject repeated Begin without mutation. These traces
  start with empty reader arenas; they do not construct all four producer statuses.
  General preservation has no such empty-reader premise.

The immutable historical lifecycle root has a separate nominal type universe.
Five exact guard declarations are projected from its pinned bytes into a companion
module and reverified with the importing root's types. The checker authenticates
the entire projection, its module header and the lifecycle authority. Historical
qualification is not treated as verification of the new import.

## Retained Checks

`proof/` retains two whole-crate positives at **321 verified, 0 errors**, including
289 inherited raw-Begin obligations. Counts overlap previous importing campaigns
and must not be summed. Eight executable-body controls each require precisely the
named postcondition failure, **320 verified, 1 error**, with a normal failure exit:

| Control | Deliberate defect |
| --- | --- |
| `combined_busy` | Admit a combined-reader conflict |
| `combined_error` | Substitute the allocation lookup error |
| `final_descriptor` | Skip the final descriptor |
| `stable_busy` | Admit a stable-reader conflict |
| `writer_precedence` | Return a raw writer/preflight error before reader guards |
| `stable_precedence` | Return a stable-reader error before the combined scan |
| `guard_error_frame` | Corrupt the incarnation on a guard rejection |
| `result_substitution` | Report failure after a successful commit |

The pinned Verus release `0.2026.08.09.92f466f` uses unchanged default resource
limits, four solver threads, a 180-second deadline and `--no-cheating`. Full release
closure checks bracket the campaign: 190 files, 129019839 bytes. All generated and
inherited inputs, completed-case receipts and exact result diagnostics are bound
and checked. A timeout, assertion failure or unrelated diagnostic is not a passing
negative control. No assumptions, axioms or external bodies were added.

`cargo/` retains **822 unit tests passed, 2 ignored; 27 doctests passed**, formatting
and Clippy with warnings denied. The new production regression runs empty and
two-member Begin with a stable lease and all four producer reservation statuses
present together. It asserts the setup and exact writer transition, status/lookup
preservation, outer arena contents and storage identity, selected allocation state,
read budget, and unchanged-on-repeated-Begin rejection. No production code changed
in this checkpoint.

`selftest.py` rechecks an actual negative, rejects 28 adverse solver-result changes
and six unauthenticated source projections. The portable auditor reconstructs the
complete source/input roster and all mutations from Git objects, verifies exact
receipts and CPU source bindings, and checks the complete packet manifest.
That audit is required for acceptance: the live checker brackets hashes of staged
files; the auditor additionally compares their bytes with independently regenerated
expected inputs. Git binding is not a recorded clean-worktree or signature check.

## Limits And Next Gate

Settlement custody/issuance composition, exact settlement preflight/scratch
execution, production Rust correspondence, physical Vec capacities, fallible
allocation and panic/unwind behavior remain separate obligations. The logical
wrapper is not compiler-checked production-to-machine refinement.

Context producer/event/result binding, audited success-gated backend admission,
bounded producer-first progress, and journal-enabled native XGMI qualification
remain open. Pending producer consumers remain rejected by runtime admission.
No shared proof runner, musl campaign, GPU test, MI300X cleanup or matched HIP/HSA
performance test was run. This standalone checker is not registered in the shared
runner, and this packet establishes no native correctness or performance claim.

## Audit And Reproduce

Offline validation requires a Git repository containing the `SOURCE` objects,
not the original scratch paths or Verus installation:

```sh
python3 -B docs/evidence/dev-begin-custody-2026-09-21/audit.py --repo "$REPO"
python3 -B docs/evidence/dev-begin-custody-2026-09-21/selftest.py \
  --repo "$REPO" --proof docs/evidence/dev-begin-custody-2026-09-21/proof
```

Rerun the proof campaign with `check.py --repo "$REPO" --verus "$VERUS" --output
"$NEW_OUTPUT"`. CPU receipts use the unchanged recorder at
`docs/evidence/dev-enrollment-transaction-2026-09-21/cargo-checks.py`.
`SHA256SUMS` covers every packet file except itself. The auditor validates retained
evidence; it does not rerun the solver or close the broader runtime parity goal.

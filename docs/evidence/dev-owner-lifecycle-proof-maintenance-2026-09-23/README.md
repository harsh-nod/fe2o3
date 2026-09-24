# Owner Lifecycle Development and Proof Maintenance

Development log only. This is not a qualified evidence packet, a replacement for
the failed inspection campaign, or native runtime qualification.

## Lifecycle Scope

Eleven new Verus sources compose constructor-origin owner traces. Separate
actual-owner and logical relations cover fourteen mutation families, twelve
getters and eighteen query families. Constructor-origin induction derives
representation, invariants, result agreement and historical non-reissue; those
conclusions are not assumed by trace admission. Failed constructors admit no
continuation. A concrete thirteen-event reader witness includes producer
settlement, writer-slot reuse, retained producer lookup and stable-reader reuse.

Independent semantic review found no weakened original contracts or circular
premises. This is trace-contract composition, not an interpreter proof for
arbitrary executable calls. Machine-sized input and physical storage conditions
remain explicit. Allocator/unwind behavior, cross-owner identity freshness and
native pending-producer authorization are not established by this development.

## Source Changes

The initial whole lifecycle measurement reported 1,230 verified obligations and
three SMT resource-limit errors: `begin_new_chain_v1`,
`begin_surviving_chain_v1`, and `stable_release_live_witness_v1`.

The two Begin proofs now compose seven smaller helpers for chain links, selected
member projection, and membership transport. Their original signatures,
preconditions, and postconditions are unchanged. The selected-member helper
explicitly requires a representable member index. Its caller derives the length
bound from existing pending custody and the final-view frame; the original
theorem does not acquire an additional premise. Empty chains remain covered.

The stable-release witness keeps its no-precondition contract, exact executable
call sequence, and outcome assertions. Large logical invariants and transition
relations remain opaque where paired callee contracts suffice; the rejected-state
frame is revealed locally. Production Rust bodies are unchanged.

Current development source identities:

| File in `crates/fe2o3-runtime-model/verus` | SHA-256 |
| --- | --- |
| `context_version_journal_begin_custody_v1.rs` | `f0b984eccdc0b849af719a74c66a4e5b04c54febfb6cdb9ce6c09cfaa4dff747` |
| `context_stable_release_witnesses_v1.rs` | `f3746f456acc43222d433ca6497a20ab9fd3fcf9bfcaac2c862e7f69ce04af55` |

## Measurements

Local raw records and source snapshots are retained under
`/home/harsh/.codex-tmp/fe2o3-lifecycle-scopes-20260923-p5dG0KxA`.
The development runner permits exactly two inherited-file changes relative to
`dcebed7d2850700142e6c2477c61e2cf4a115d88`. It reconstructs the original Begin
file after removing the named helpers and restoring the two original proof
bodies, and checks unchanged original contracts and all other bytes. The
stable-release edit is an exact text transform. All remaining inherited inputs
must match the frozen inspection source roster.

- `release-1`: 1 verified, zero errors.
- `new-chain-6`: 4 verified, zero errors.
- `surviving-chain-5`: 5 verified, zero errors.
- `whole-2`: 1,240 verified, zero errors, entire crate; exit zero in 517.790
  seconds, with owned process-group absence. Both tool-closure endpoints pass
  and the before/after source, runner, tool and HEAD manifests are identical.
- `raw-1`: 384 verified, zero errors, entire raw-owner root.
- `inspection-1`: 1,179 verified, zero errors, entire inspection root.
- `constructor-1`: 1,156 verified, zero errors, entire constructor root; exit
  zero in 397.377 seconds, with owned process-group absence.

Each solver measurement stages and brackets 449 input files, HEAD, runner
identity, and three tool binaries. Both endpoints check the pinned 190-file
Verus distribution. Scoped runs use 600 seconds; the whole root uses 900 seconds.
Four threads, default SMT limits, and `--no-cheating` are unchanged. Earlier
failed attempts remain separate, including the opacity-placement frontend
failure and the selected-member helper's missing cast-bound assertion.

The three later regression measurements capture 448 inputs, excluding only the
candidate lifecycle checker so its authentication implementation can proceed
without changing measured proof inputs. Each checks both tool-closure endpoints
and unchanged source, runner, tool and HEAD identities. Their counts overlap the
full lifecycle root and must not be added as independent proof coverage.

Independent review found no weakened original contracts or circular helper
premises. Lightweight checks pass for the three unchanged contracts, thirteen
source-tamper controls, and the inherited/lifecycle trust policy over all 449
inputs. These are not solver-negative mutation results or campaign acceptance.
The complete root checks the seven added helpers together with the original
theorems and lifecycle witnesses. The earlier 1,230/3 attempt remains failed.

## Remaining Qualification

The candidate lifecycle checker now seals the reviewed source, definition,
contract and measured-count pins. It authenticates both inherited proof edits by
checking unchanged original contracts, the ten reviewed original/helper bodies,
and exact whole-file reconstruction outside those edits. Its root extension
admits exactly eight named whitespace removals plus the new modules/includes.

All fifteen inherited Python helpers are authenticated before the first import.
Authoritative Git reads use an absolute executable, a scrubbed environment and
disabled replacement refs. Historical inspection selftests run in a separate
isolated Python process against the exact frozen 437-file source closure, in the
outer recorder's owned process group, with an inner 300-second bound and exact
transcript checks. The historical checker and its authentication rules remain
unchanged. A concurrently hostile filesystem writer racing authentication and
path-based import is outside this frozen-worktree operating model.

Source-bound selftest records are retained under
`/home/harsh/.codex-tmp/fe2o3-lifecycle-recorder-20260923-PpISUvaR`.
`attempt-3` passes the complete recorder transcript with 449 unchanged inputs
and owned process-group absence. New controls cover eighteen helper byte/node
changes, five ambient Git configurations, an effective private replacement ref,
fifteen rehashed inherited-proof changes, nineteen rehashed lifecycle-policy
changes and six source/root/checker/roster changes. The 49 logical mutation
anchors are authenticated, not solver-qualified by this selftest. The final
constructor-count pin is recorded after this run from `constructor-1` above.

The first authentication attempt exposed the root whitespace mismatch. Recorder
`attempt-2` caught a positive-control fixture error: a present
`GIT_NO_REPLACE_OBJECTS=0` still disables replacement lookup. The corrected
positive control removes that variable only in its private fixture subprocess;
authoritative reads retain the restriction. Failed records and source snapshots
remain unchanged. Independent recorder re-review found no remaining launch
blocker after the import and Git-provenance corrections.

A fresh lifecycle campaign must run the full positive, negative, regression,
tool-closure and CPU inventory from signed source before publication as accepted
evidence. No old failed campaign may be resumed into acceptance by rewriting its
records or updating hashes.

This work concerns proof decomposition and reproducibility, not runtime speed.
Pending-producer native integration, physical ownership/unwind refinement,
A1/A2, HIP/HSA parity and matched performance qualification remain open.

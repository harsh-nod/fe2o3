# Settlement Execution And Issued Custody

This development packet connects successful raw settlement preflight to exact
logical scratch staging, sequential Success/NoEffect commit and the existing
issued-producer preservation theorem. `SOURCE` binds the implementation, packet
tools and CPU inputs to Git objects. Runs began before that commit, not from a
recorded clean/signed HEAD. Native pending-producer admission remains disabled.

## Established Scope

- The raw executor has no valid-state precondition. Complete ordered preflight
  precedes every mutation; errors preserve the entire logical object and exact
  result. Successful preflight derives the bounded live traversal and empty
  scratch prefix needed by staging and commit.
- Scratch plans come from the prestate reachable chain. Commit clears each plan,
  updates its exact allocation, clears its member and appends the member slot in
  order. Only then does it clear and return the writer. It preserves arbitrary
  free prefixes, unreachable members, unrelated allocation backlinks, scratch
  tails and all untargeted fields. Empty settlement releases only the writer.
- Sequential updates are specified as recursive prefixes, not a substituted
  global overwrite. Pointwise allocation normalization uses the common prestate
  attempt epoch and update idempotence, not an invented uniqueness premise.
- The bridge obtains complete retained-chain coverage from pending custody, then
  identifies the concrete traversal and derives `settle_chain_relation_v1`.
  Custody is also required to equate exact member allocation slots with the
  historical backlink selector. Raw preflight alone establishes neither fact.
- The issued wrapper preserves history, the exact Reserved count, stable leases,
  producer reservations and outer storage without a global-idle premise. It
  exactly frames rejection, and its successful relation follows from the commit
  executor rather than being supplied as a settlement premise.
- Constructor/enroll/register/Begin traces execute both settlement outcomes for
  empty and two-member writers, reject bad evidence and insufficient headroom,
  and reject repeated settlement without mutation. They have empty reader arenas;
  they do not demonstrate constructor-derived mixed reader/status reconciliation.

These are normal-return logical-execution proofs. Observed capacity arguments are
not proved bindings to Rust `Vec::capacity()`. Fallible allocation, physical vector
storage, intermediate-state/panic semantics and compiler-checked production Rust
correspondence remain open. Production settlement code is unchanged in this packet.

## Qualification

The whole importing crate passes **360 verified, 0 errors** before and after ten
executable-body controls, each requiring exactly **359 verified, 1 intended
postcondition error** and a normal failure exit. Counts include 338 inherited
obligations and overlap earlier packets; they must not be summed.

| Control | Deliberate defect |
| --- | --- |
| `stage_plan` | Remove a completed staged plan |
| `scratch_tail` | Clear an unselected scratch cell |
| `unlinked_member` | Clear a member during empty settlement |
| `writer_retained` | Fail to clear an empty writer |
| `writer_return` | Return the wrong writer slot |
| `free_prefix` | Corrupt an existing free-stack entry |
| `scalar_frame` | Reset the unrelated Reserved count |
| `wrong_outcome` | Invert the Success/NoEffect outcome |
| `rejection_frame` | Mutate the watermark on preflight rejection |
| `issued_result` | Substitute the issued wrapper's result |

Verus `0.2026.08.09.92f466f` runs with unchanged default resource limits, four
threads, a 180-second deadline and `--no-cheating`. Full tool closure checks
bracket the campaign: 190 files, 129019839 bytes. Every recursive source/checker
dependency is authenticated. Generated executable bytes match captured inputs,
are hashed before/after each solver, and are independently reconstructed by the
mandatory offline audit. Completed cases are frozen. No new assumption, axiom,
external body, weakened postcondition or increased verifier budget is used.

`cargo/` records **825 unit tests passed, 2 ignored; 27 doctests passed**, formatting
and Clippy with warnings denied. New regressions exercise 16 combinations of
unlinked-member, forged-backlink, duplicate-free-prefix and dirty-scratch-tail
faults across both outcomes, plus both empty-settlement outcomes with unrelated
allocation/member/scratch vectors cleared. All 34 cases compare exact snapshots,
all seven vector pointers/capacities and indexed-access counts (`9 * count + 3`).
These tests establish retained storage identity, not general absence of temporary
allocation or production-to-machine correspondence.

`selftest.py` rejects 29 altered actual solver diagnostics and five unauthenticated
source changes, including confusing the importing root with the mutated included
source. The mandatory offline audit reconstructs exact mutated inputs, rosters,
commands and source spans from Git, checks process-custody receipts and terminal
CPU summaries, and requires the complete packet manifest. It does not replay the
solver or establish an independently recorded clean-worktree campaign.

## Remaining Work

Bind physical capacities, fallible allocation and normal/unwind semantics to the
production Rust implementation. Integrate exact Context event/producer/member and
result custody, an audited default-false success-gated backend contract, and
bounded producer-first progress. Qualify journal-enabled pending dataflow on
native XGMI before matched HIP/HSA performance comparisons.

This standalone checker is not registered in the shared proof runner. No shared
runner campaign, musl campaign, GPU test or performance comparison is included.
No MI300X processes or files were created. Retained evidence does not complete
the broader runtime parity objective.

## Audit And Reproduce

```sh
python3 -B docs/evidence/dev-settlement-commit-2026-09-21/audit.py --repo "$REPO"
python3 -B docs/evidence/dev-settlement-commit-2026-09-21/selftest.py \
  --repo "$REPO" --proof docs/evidence/dev-settlement-commit-2026-09-21/proof
```

The offline audit needs the `SOURCE` Git objects, not scratch paths or the Verus
installation. Rerun `check.py --repo "$REPO" --verus "$VERUS" --output "$NEW_OUTPUT"`
for new solver receipts. CPU receipts use the unchanged recorder at
`docs/evidence/dev-enrollment-transaction-2026-09-21/cargo-checks.py`.
`SHA256SUMS` covers every packet file except itself.

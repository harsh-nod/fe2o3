# V3 Settlement Negative-Test Handoff

Status: prospective behavioral mutations, reviewed on 2026-09-14. None of these
mutations has been executed. Primary owns integration, mutation application,
compiled diagnostics, exact restoration and qualification.

Candidate worktree: `/home/harsh/.codex-tmp/fe2o3-v3-settlement`.
Publication parent: `0265025b96f25a7cb79c97b576bb25cb38b15df5`.
Current non-documentation source-map SHA256:
`51c996704992ad6f6a46965eeeeffb389c451d1ba051fbf7917a9fe3e490ddd0`.
Fresh positive: `v3candidate-lineage-settlement.json`, all 17 settlement tests
passed with unchanged source and a closed child/process group.

Production mutation file:
`crates/fe2o3-runtime-model/src/context_version_journal/settlement.rs`.
Behavioral test prefix: `context_version_journal::tests::settlement::`.
Use exact test filters; the supplemental structural guard is not a behavioral
mutation target. Reconfirm source anchors and assertion lines before freezing
an executable campaign. The public model premises are inert, not production
completion or NoEffect authority.

## Oracle Names

| Alias | Test Under The Prefix |
| --- | --- |
| S | settlements_return_exact_canonical_members_and_preserve_unrelated_storage |
| G | no_effect_burns_epochs_and_success_advances_lineage_across_gaps |
| U | unknown_is_sticky_revalidated_and_retains_empty_and_nonempty_writers |
| K | exact_reserved_writers_reject_settlement_and_unknown_before_evidence |
| F | full_reference_and_evidence_rejections_precede_retained_corruption |
| M | every_retained_member_rejects_corruption_before_any_scratch_write |
| L | retained_prior_lineage_mismatch_below_admitted_epoch_rejects_atomically |
| H | header_cardinality_and_noncanonical_chains_reject_atomically |
| B | rejected_release_cost_covers_each_scratch_cell_and_return_limit |
| D | reference_traces::independent_traces_preserve_disjoint_writers_and_lineage_below_watermark |

## Proposed Mutations

`disable(E)` means replacing the unique expression E with `(false && E)`.
Lines below refer to `settlement_tests.rs`, except D, which refers to
`settlement_reference_tests.rs`. Results and lines are predictions until an
actual compiled test reaches the named assertion.

| ID | Mutation | Predicted Oracle |
| --- | --- | --- |
| 1 | disable `key != writer.key` | F:236, wrong InvalidState instead of InvalidReference |
| 2 | Admit Reserved in retained_header as `(key, None, 0, false)` | K:207, Reserved settles |
| 3 | disable `evidence != writer` | F:241, wrong later chain error |
| 4 | disable `member.writer != writer` | M:310, foreign member writer accepted |
| 5 | Replace exact_allocation by read_allocation at the supplied slot | M:310, final foreign allocation key accepted |
| 6 | disable `allocation.pending_member != Some(slot)` | M:310, missing backlink accepted |
| 7 | disable `allocation.attempt_epoch != member.attempt_epoch` | M:310, substituted epoch accepted |
| 8 | disable `allocation.content_lineage != member.prior_lineage` | L:351, mismatched lineage below epoch accepted |
| 9 | disable `member.prior_lineage >= member.attempt_epoch` | M:310, equal lineage and epoch accepted |
| 10 | disable final `head.is_some()` | H:389, truncated subset released |
| 11 | disable `previous_key.is_some_and(\|key\| key >= member.allocation.key)` | H:389, reverse-key chain accepted |
| 12 | Route NoEffect through `settle_retained(..., true)` | G:140, NoEffect advances lineage |
| 13 | Before clearing backlink, set `allocation.attempt_epoch = plan.prior_lineage` when !success | G:140, burned epoch rolled back |
| 14 | Success writes prior_lineage rather than attempt_epoch | G:140, Success fails to advance lineage |
| 15 | Clear writer immediately after releasing retained_header preflight | F:245, rejected evidence destroys writer |
| 16 | Store a first-member scratch plan before releasing chain validation | M:311, rejected wrong writer leaves scratch changed |
| 17 | disable `writer_returns > self.free.capacity()` | B:673, physically short writer vector grows |
| 18 | disable `member_returns > self.member_free.capacity()` | B:673, physically short member vector grows |
| 19 | disable `writer_returns > self.writer_capacity` | B:673, logical writer ceiling exceeded |
| 20 | disable `member_returns > self.allocation_capacity` | B:673, logical member ceiling exceeded |
| 21 | disable `self.scratch[index].is_some()` | B:673, occupied scratch overwritten |
| 22 | Reverse only the commit loop consuming scratch plans | S:121, canonical member-return order changed |
| 23 | Push writer slot zero instead of writer.slot | S:121, wrong slot returned on empty settlement |
| 24 | Release calls `retained_header(writer, true)` | U:180, sticky Unknown released |
| 25 | Revalidate mark_unknown chain only when !unknown | M:310, corrupt repeated Unknown accepted |
| 26 | After final writer clear, clear `(writer.slot + 2) % writer_capacity` when count != 0 | D:385, disjoint Unknown writer destroyed |

Mutation 5 must preserve the Result type, for example:
`read_allocation(member.allocation.slot).ok_or(ContextVersionJournalErrorV1::InvalidAllocationReference)`.
Mutation 16 inserts this before the release's chain-validation call followed by
`let writer_returns`, not into mark_unknown:

```rust
if let Some(slot) = head {
    if let Some(member) = self.members[slot] {
        self.store_plan(0, BeginMemberPlanV1 {
            member_slot: slot,
            allocation: member.allocation,
            prior_lineage: member.prior_lineage,
            attempt_epoch: member.attempt_epoch,
        });
    }
}
```

## Acceptance Limits

Require successful compilation followed by the exact named behavioral assertion.
A compiler error, production bounds panic or unrelated source-guard failure is
not a replacement oracle. Retain failures and reject unexpected outcomes.

The lineage-isolation positive is now observed, not merely planned. Its member
prior and allocation lineage differ while both remain below admitted epoch 3,
so the separate epoch bound cannot mask omission of their equality check.

Canonical member order and the exact returned writer slot are observable.
Moving the final writer return before an otherwise infallible member commit is
not behaviorally distinguishable under the exclusive borrow; the supplemental
structural guard covers that textual ordering. Do not claim a decisive compiled
negative for it or claim these tests establish authenticated formal refinement.

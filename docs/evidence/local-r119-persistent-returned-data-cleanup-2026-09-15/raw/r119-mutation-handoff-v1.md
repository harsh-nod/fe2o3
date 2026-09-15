# R119 Prospective Mutation Handoff

Read-only review, 2026-09-15 UTC. No mutation in this handoff has been executed.
Candidate: `/home/harsh/.codex-tmp/fe2o3-r119-persistent-data`, accepted parent
`a07ec44309e214f2a8ef0e687e610c8e60a36224`, source map
`b12435bb1aff37927a5d9b79e4871679476555d76a7efd62ea33635d463f764d`.

The proposed matrix has 30 distinct production source variants and 37 executions.
Actual plans must pin complete unique source anchors and confirm each first
diagnostic. Successful compilation, one exact named failed test, the intended
behavioral assertion, closed processes and exact full-source restoration are
required. Nonzero exit alone is not acceptance. Fail-fast mutations do not
exercise every positive matrix cell.

`P` is `crates/fe2o3-kfd/src/queue_dispatch_binding/control_release.rs`.
`T` is its adjacent `control_release/persistent_tests.rs`; `H` is adjacent
`control_release/tests.rs`. Test aliases below use the exact prefix
`queue::dispatch_binding::control_release::tests::persistent::`.

| Alias | Test Suffix |
| --- | --- |
| A | persistent_success_preserves_exact_mixed_data_and_forward_cleanup |
| B | persistent_generation_errors_precede_cardinality_and_capacity |
| C | persistent_cardinality_and_capacity_reject_before_disposal |
| D | persistent_zero_data_uses_explicit_readiness_and_one_shot_transfer |
| E | persistent_native_errors_return_data_while_panics_retain_it_at_every_control |
| F | persistent_currentness_failures_preserve_all_eighteen_control_boundaries |
| G | persistent_partial_unmap_returns_data_without_losing_original_control |
| H | persistent_projection_and_actual_commit_failures_preserve_receipts_and_output_split |
| I | persistent_incomplete_callbacks_return_data_but_never_claim_control_completion |
| J | persistent_wrong_wrappers_and_repeated_extraction_reject_before_effects |
| K | persistent_genuine_single_and_three_binding_preparation_preserve_metadata_and_data |
| L | persistent_output_capacity_survives_success_and_error_extraction |
| M | persistent_panic_root_rejects_extraction_without_mutating_retained_custody |

All changes target P. Diagnostic lines are prospective macro invocation lines,
not expression continuation lines.

| ID | Change | Expected First Failure |
| --- | --- | --- |
| 01 | Constructor `data: owner.data` becomes an empty vector | A, H:255, changed original owner |
| 02 | Constructor `data_premises: owner.data_premises` becomes empty | A, H:255, constructor frame |
| 03 | Before-mode generation uses `returned_generation()` | A, T:205, unexpected ResourcePhase |
| 04 | After-mode generation uses `returning_destroy_generation()` | B, T:265, invalid generation reaches cardinality |
| 05 | Before-mode generation swallows error with `unwrap_or(0)` | B, T:265, generation error lost |
| 06 | Copy cardinality guard before generation selection | B, T:265, precedence changed |
| 07 | Delete cardinality guard | C, T:295, wrong rejection |
| 08 | Move persistent reservation/headroom block after control callbacks | C, T:312, preflight changed retained root |
| 09 | In persistent reservation block only, reserve/check ordinary `returned` | A, T:78, ordinary output capacity changed |
| 10 | After Taken, replace persistent output with a freshly allocated vector | L, T:907, original storage lost |
| 11 | Move Prepared-to-Returnable before control callbacks | M, T:935, panic root permits extraction |
| 12 | Propagate cleanup error before Prepared-to-Returnable | E, T:59, normal error loses data |
| 13 | Delete Taken assignment | A, T:87, repeated extraction accepted |
| 14 | Reject extraction when data is empty | D, T:336, valid zero-data transfer rejected |
| 15 | Reverse zipped extraction iterator | A, T:59, output order changed |
| 16 | Truncate zipped extraction iterator with `take(1)` | A, T:59, output count changed |
| 17 | Negate `premise.fully_initialized` during conversion | A, T:59, initialization changed |
| 18 | Revive initialized content descriptor during conversion | A, T:59, stale content authority revived |
| 19 | Drain premises instead of borrowing them during extraction | A, T:71, retained premises lost |
| 20 | Wrapper discards normal-error returned data | E, T:59, error output lost |
| 21 | Normal-error wrapper drops rather than retains root | E, T:35, retained root missing |
| 22 | Panic wrapper drops rather than retains root | E, T:35, retained root missing |
| 23 | Panic wrapper substitutes original panic payload | E, T:178, payload identity lost |
| 24 | Delete started latch | B, T:272, rejected admission is not one-shot |
| 25 | Reverse code cleanup with `next_back()` | K, T:874, real callback order changed |
| 26 | Take active control out of root before callback | E, T:135, active custody missing |
| 27 | Delete incomplete-control rejection | I, T:30, false successful cleanup |
| 28 | Delete mode guard only in persistent wrapper | J, T:746, prohibited callback panic |
| 29 | Delete mode guard only in returning wrapper | J, H:269, prohibited callback entered |
| 30 | Delete mode guard only in detached wrapper | J, H:269, prohibited callback entered |

## Exact Construction Notes

For 06, C is the complete cardinality guard at P:110-115. Insert C between the
unique `self.started = true;` and `let generation = match self.mode {`, leaving
the original C unchanged. Mutation 07 deletes only the original C.

For 08, R is the complete persistent reservation and headroom block at P:124-133,
starting at `self.persistent_returned.try_reserve_exact(capacity)` and ending
before the Prepared assignment. Delete R, retain Prepared, then insert R after
`let release = self.release_controls(memory);`, under
`if let PersistentOutputStateV1::Prepared(_) = self.persistent_output` with the
same data length and test-only capacity override. Pin full bytes; do not use an
unbounded textual replacement. For 09 replace `persistent_returned` with
`returned` only inside R.

For 11 move the complete three-line Prepared-to-Returnable block immediately
above `let release = self.release_controls(memory);`. For 12 move `release?;`
immediately below that call, before the transition.

For 18 replace the exact conversion push with the existing pristine-abort
conversion idiom: match `(authority, premise.initialized_content)`. A Device
authority with Some content goes through
`Gfx942InitializedDeviceMemoryV1::from_authenticated_full_transfer` and then
`Gfx942FixedDispatchDataV1::initialized`. A None descriptor uses the unchanged
conversion. Fixtures have matching Copy descriptors and extent. Compilation or
an unrelated conversion panic does not satisfy the intended content assertion.

For 26 replace the exact two lines borrowing active control and calling
`memory.release_control(active)?` with a local obtained by
`self.active_control.take().expect("rooted active control")` and
`memory.release_control(&mut active)?`, leaving completion checking/clearing intact.

## Repeated Source Variants

Execute 08 additionally against E and H, expecting T:113 because panic precedes
output reservation. Execute 11 additionally against D, expecting T:109 because
empty panic output becomes Returnable. Execute 12 additionally against D,
expecting T:121 because empty normal-error output remains Prepared. Execute 26
additionally against F, G and H, expecting T:135. These are seven extra test
executions against existing source maps, not seven distinct mutations.

## Exclusions

Extraction-mode guard omissions are masked by independent readiness/generation
guards and must not be claimed as decisive negatives. Swallowing reservation
errors is masked by physical headroom checking; deleting headroom alone depends
on the synthetic override and would need a separately labeled calibration.
Routing coverage is a source guard, not native execution. Output storage identity
shows reuse of that buffer, not absence of all possible temporary allocations.

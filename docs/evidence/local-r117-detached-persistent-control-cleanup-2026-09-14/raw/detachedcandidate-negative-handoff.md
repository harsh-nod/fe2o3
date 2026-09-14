# Detached-Control Mutation Handoff

Read-only source review on 2026-09-14. Not executed qualification.
Candidate: `/home/harsh/.codex-tmp/fe2o3-detached-control`, source map
`60adae0665fc2995e98e6f647e630c26fbbe4c7e6e20eb603fec05c1986a8d4d`.

All new mutations target
`crates/fe2o3-kfd/src/queue_dispatch_binding/control_release.rs`.
Require exactly one anchor within the stated enclosing arm/function; select a
full unique textual anchor before freezing. Each mutant must compile and fail
its exact named behavioral assertion, then restore every source identity.

Test prefix: `queue::dispatch_binding::control_release::tests::detached::`.
`D` is `control_release/detached_tests.rs`; `T` is `control_release/tests.rs`.

| Alias | Test Suffix |
| --- | --- |
| S | detached_success_preserves_separate_data_metadata_order_and_zero_return_storage |
| G | detached_generation_errors_precede_malformed_state_without_cleanup_entry |
| V | detached_generation_rejects_with_otherwise_valid_detached_state |
| C | detached_state_cardinality_and_generation_mismatch_retain_exact_owner |
| W | detached_wrapper_retains_root_before_error_and_original_panic_and_drops_only_complete |
| M | detached_and_returning_wrappers_reject_wrong_modes_without_losing_outputs |
| P | detached_real_preparation_preserves_populated_metadata_and_separate_native_data |

| ID | Scoped Mutation | Predicted Oracle |
| --- | --- | --- |
| D01 | Detached generation call: `returned_generation()?` to `returning_destroy_generation()?` | V, D:366, ResourcePhase; expected generation zero distinguishes fresh/cancelled-only state |
| D02 | Same call to `returned_generation().unwrap_or(expected_generation)` | G, D:219, Poisoned rather than ResourcePhase |
| D03 | Ignore the detached validator's Result, retaining the generation argument's `?` | C, D:267, Ordinary state incorrectly succeeds |
| D04 | Validator's expected-generation argument becomes `{ let _ = expected_generation; self.generation.returned_generation()? }` | C, D:267, case 5 ignores supplied generation 9 |
| D05 | Constructor's data becomes empty only for DetachedPersistent mode | G, T:225, constructor changed original owner |
| D06 | Detached arm reserves one returned slot before returning None | S, D:150, metadata/storage equality |
| D07 | Conversion branch additionally runs for DetachedPersistent mode | S, D:146, zipped drains lose retained premises even with empty data |
| D08 | Code cleanup changes `while let Some(code)` to `if let Some(code)` | P, D:973, exact control-entry order |
| D09 | Disable the unit wrapper's wrong-mode guard | M, D:840, retained-root equality |
| D10 | Disable the returning wrapper's detached-mode guard | M, D:872, retained-root equality |
| D11 | Unit wrapper's Ok(Err(error)) arm drops instead of retaining root | W, D:820, scenario 0 retention |
| D12 | Unit wrapper's Err(payload) arm drops instead of retaining root | W, D:820, scenario 2 retention after original panic payload check |

Retain all sixteen definitions/test identities from R115's archived
`r115-qualification-plan.js`. Their current oracle lines are:

| Mutation | Current Oracle |
| --- | --- |
| constructor-metadata-loss | T:225 |
| reverse-control-order | shared_memory/tests/pristine_abort/cleanup_tests.rs:303 |
| admit-unrecycled-mode | T:425 |
| swallow-generation-rejection | T:362 |
| skip-cardinality | T:395 |
| skip-return-capacity-check | T:403 |
| permit-cleanup-retry | T:245 |
| remove-active-custody | T:468 |
| accept-incomplete-callback | T:775 |
| extract-failed-output | T:250 |
| reverse-returned-authorities | T:317 |
| corrupt-returned-premise | T:318 |
| drop-error-root / drop-panic-root | T:811 |
| late-return-capacity | T:498 |
| truncate-returned-output | T:305 |

Adapt anchors for current Some-wrapped generation arms, scope generation
swallowing to AfterRecycle, scope retention edits to the returning wrapper,
and move the entire current `if let Some(generation)` reservation block for
late capacity. Failed-output extraction disables only `!self.complete`, not the
detached-mode disjunct.

Four additional detached regression targets reuse shared-driver mutations:
reverse-control-order at P D:973; remove-active-custody at
`detached_native_errors_and_panics_preserve_each_control_and_destructive_prefix`
T:468; permit-cleanup-retry at V T:245; accept-incomplete-callback at
`detached_incomplete_callback_retains_active_owner_and_cannot_retry_or_extract`
D:747.

Removing only the DetachedPersistent check in take_completed is not decisive:
returned_generation remains None and independently rejects extraction. Do not
count this edit or the supplemental routing guard as behavioral mutation proof.
These predictions do not establish native execution or formal correspondence.

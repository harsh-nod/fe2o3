# C1 Compiled-Negative Handoff For R118

Read-only review of the integrated four-observation source on 2026-09-14.
No negatives below have been executed or qualified. Source map:
`df2e27085ee920fdd07d2a6955503b1ecdcadc2d7c10c6fd4cec6768cecdadbb`.

Production anchors are in `crates/fe2o3-runtime/src/context.rs`. Assertion
locations are in `context/tests/submission_identity_tests.rs`. Exact filter
prefix: `context::tests::submission_identity_tests::`.

| Key | Test Suffix |
| --- | --- |
| M | every_ingress_rejects_each_coordinate_for_pending_and_event_completed_records |
| V | exact_tokens_reach_each_ingress_with_state_appropriate_results |
| C | cached_success_never_bypasses_any_identity_coordinate |
| L | released_cached_token_is_rejected_at_every_ingress |
| B | real_backend_id_reuse_does_not_revive_a_released_cached_token |
| U | destroyed_stream_keeps_identity_validation_and_retained_query_semantics |
| D | wait_validates_identity_before_deadline_but_drain_checks_expiration_first |
| T | actual_terminal_backend_failure_precedes_ingress_identity_except_pure_query |
| G | genuine_graph_reservation_preserves_query_and_public_ingress_precedence |

## Ingress Wiring

Create eight distinct mutations replacing checked record acquisition with:

```rust
*self.submissions.get(&submission.id).ok_or(RuntimeValidationErrorV1::UnknownSubmission)?
```

Scope each replacement to `poll_with_graph_access_v1`, `wait`, `on_completion`,
`release_submission_ref`, `record_event`, `cancel`, or `drain`. For
`query_submission`, return that record's `.status` inside the existing Ok.
Each fails M:505 on its selected ingress's Logical substitution: lookup accepts
the neighbor record. Release produces SubmissionPending rather than
UnknownSubmission; the others produce successful observations. Freeze unique
method-scoped anchors, not all matching acquisition statements globally.

## Identity, State And Precedence

| Mutation | Predicted First Oracle |
| --- | --- |
| In `submission_record`, remove explicit generation guard and canonicalize lookup to `RuntimeSubmissionIdV1::new(self.context_generation, submission.id.local)` | M:505, foreign generation accepted; explicitly compound |
| Replace its full-ID lookup with `.values().find(|r| r.backend_submission == submission.backend_submission)` | M:505 for Logical, separately B:505 for actual reused backend ID |
| Separately replace backend, stream or device comparison with false, retaining other OR terms | Three mutants, M:505 for selected coordinate |
| In Poll, after Context gate insert `if let Some(done) = submission.completion { return Ok(done); }` | C:505 and separately L:505 |
| In `live_submission_record`, remove stream lookup/device-consistency block but retain `submission_record` | U:505, exact destroyed-stream Poll succeeds instead of UnknownStream |
| Move Wait deadline construction before identity acquisition, retaining Context gate first | D:1004, InvalidDeadline beats malformed identity |
| Move Drain record acquisition before expiration check | D:1012, malformed identity beats expired deadline |
| Move Wait deadline construction before Context gate | T:535 and separately G:535 |
| Move Drain deadline check before Context gate | T:539 and separately G:539 |
| Disable terminal branch in `require_graph_access` | T:505, UnknownSubmission replaces ContextTerminal |
| Disable reservation-mismatch branch there | G:505, public ingress enters reserved Context |
| Reorder reservation mismatch before terminal there | G:505, ContextReserved replaces ContextTerminal |
| Add `self.require_live()?;` at start of `query_submission` | G:1161, retained query returns ContextReserved instead of Pending |
| Delete pending rejection in `release_submission_ref` | V:578, pending release succeeds |
| Separately omit logical-record removal or backend-ID-set removal after release | V:721 or V:722, stale owner remains registered |
| In completed `on_completion`, replace `completion_callback_panicked_v1(callback, record.status)` with `{ drop(callback); false }` | V:612, callback counts (0,1) instead of (1,1) |

## Actual Routing

Select the other retained valid native ID before the call:

```rust
self.submissions.values()
    .find(|r| r.backend_submission != original)
    .unwrap().backend_submission
```

Do not substitute an unknown native handle: that can trigger unrelated mock
diagnostics. Mutate Event's two arguments separately, using the other retained
stream's backend ID for its stream case.

| Argument-Only Mutation | V Assertion |
| --- | --- |
| Wrong backend Poll submission | 663, exact per-submission poll-map delta |
| Wrong backend Wait submission | 675, exact waited submission |
| Wrong backend Drain submission | 675 first, shared wait assertion precedes drain assertion |
| Wrong Event native submission | 731, exact Event pair |
| Wrong Event native stream, unchanged submission | 731, exact Event pair |
| Wrong Cancel native submission | 755, full snapshot includes cancellation argument |
| Replace Context Drain's `backend.drain_v1` with `backend.wait_v1` | 684, explicit Drain entry missing |

Fixed Option observations at actual mock Wait/Drain/Cancel/Event entries are
part of BackendSnapshot. Completed paths preserve prior observations. Poll's
per-ID deltas, callback placement and released-owner removal are independently
observed. No new allocation-heavy mock log is needed.

## Rejection Frames And Limits

Two more distinct mutations before Poll identity validation:

- Assign `submission.completion = Some(RuntimePollV1::Succeeded)`: M:509,
  supplied token changed despite rejection.
- Perform `let _ = self.backend.poll_v1(submission.backend_submission);`:
  M:514, retained/backend snapshot changed despite the correct error.

This proposed roster gives 36 distinct mutation sources and 40 executions when
the four shared sources are checked at both named oracles. Confirm that count
and the exact edit groups when freezing the executable plan.

Deleting only the generation guard is masked by full-ID hashing. Deleting only
the retained stream's device-consistency guard is unexercised by these valid
records. Wait-to-Drain substitution fails compilation under Wait's existing
generic bounds. None is a killed behavioral negative.

The 80/40 case counters prove traversal, not correctness independently. Require
successful compilation, one exact named selected test, the intended diagnostic,
and full source restoration. No source-guard, timeout, compile failure or
incidental fixture panic qualifies. This does not establish native routing,
private credit-account identity/storage, formal refinement or performance.

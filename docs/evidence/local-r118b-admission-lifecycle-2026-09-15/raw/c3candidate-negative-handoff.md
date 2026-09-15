# C3 Compiled-Negative Handoff

Read-only source review, 2026-09-14. These are predictions, not executed or
qualified negatives. Candidate map:
`b4d158f6b305f506d52c1ac0b6364adb1dee16b085dcc96f3f26307b931aa0e1`.
The integrated R118 test files are byte-identical.

Paths are relative to `crates/fe2o3-runtime/src/async_engine/` unless stated.
Assertions are in `tests/owned_tests/preparation_tests/completion_tests.rs`.
Use its full `async_engine::tests::owned_tests::preparation_tests::completion_tests::`
prefix with the selected suffix, `--exact`, and one test thread.

| Key | Test Suffix |
| --- | --- |
| I | reserved_completion_isolates_discard_order_and_abandoned_observers |
| S | reserved_stop_notifies_latest_wakers_before_disposal_and_contains_wake_panic |
| D | reserved_discard_wakes_before_payload_drop_even_when_wake_panics |
| R | unpublished_retirement_preserves_exact_failed_owner_and_untouched_neighbor |
| O | owned_shutdown_retires_in_order_and_retains_failed_and_unvisited_owners |

Scope repeated anchors to the named method and require exactly one replacement.

| ID | Mutation | Predicted First Behavioral Assertion |
| --- | --- | --- |
| 1 | `generated_operation.rs`, producer transfer: replace `self.completion = completion.take();` with `drop(completion.take());` | I:67, prematurely Ready instead of Pending |
| 2 | `generated_operation.rs`, `PreparationDriver::reject`: delete `stop_reply(&mut self.completion, None, error);` | S:197, missing latest-wake roster |
| 3 | `generated_operation.rs`, `PreparationDriver::drop`: delete only its completion `stop_reply` block | D:230, payload already dropped at wake |
| 4 | `operation.rs`, `stop_observations`: insert `self.dispose_quiescent();` immediately before final `panicked` expression | S:176, retained registry is empty |
| 5 | `owned.rs`: replace `core::mem::replace(&mut state.waker, new_waker.take())` with `{ if state.waker.is_none() { state.waker = new_waker.take(); } None }` | S:189, old waker fires |
| 6 | `owned.rs`, caught wake: replace entire catch block with `if let Some(waker) = waker { waker.wake(); }` | S:175, panic containment terminalizes Context |
| 7 | `owned.rs`, `RuntimeAsyncCommandFutureV1::drop`: delete `self.clear_waker();` | I:88, abandoned observer notified |
| 8 | `operation.rs`, matched parked-owner removal: replace `remove(index)` with `remove((index + 1) % self.parked.len())` | I:77, selected owner did not drop |
| 9 | At the same removal, first reject every other parked driver using its `reject(EngineStopped)` | I:88, untouched neighbor notified |
| 10 | `owned.rs`: insert `drop(state._permit.take());` immediately before `state.result = Some(result);` | I:89, reply credit returned before consumer drop |
| 11 | `generated_operation/adoption.rs`: delete `context.release_unpublished_hold_v1(&owner.hold)?;` | R:310, retired hold still occupied |
| 12 | `operation.rs`, successful retirement: replace `drop(entry);` following `self.retire_stream(entry.stream);` with `core::mem::forget(entry);` | R:306, retired carrier never drops |
| 13 | `generated_operation/adoption.rs`: delete `(self.adoption.as_ref().expect("admitted hooks").retire)(context, &owner.hold)?;` | R:317, no retirement attempts |
| 14 | `operation.rs`, retirement failure arm: replace `return;` following quarantine with `index += 1;` | R:317, C attempted after B failure |
| 15 | `owned.rs`, only inside `Ok((cleanup, native_failure))`: replace `core::mem::forget(operations);` with `drop(operations);` | O:414, failed and unvisited owners drop |
| 16 | `owned.rs`: delete `operations.retire_unpublished_v1(&mut context, usize::MAX);` | O:406, successful shutdown is retained |

ID 9's exact inserted loop is:

```rust
for (other, entry) in self.parked.iter_mut().enumerate() {
    if other != index {
        entry.driver.reject(RuntimeAsyncEngineCallErrorV1::EngineStopped);
    }
}
```

## Helper Calibration

These calibrate observability, not production correctness independently:

- `adoption_tests.rs`: replace `retire(&self.inner.state, stream)` with
  `let state = self.inner.state.clone(); thread::spawn(move || retire(&state, stream)).join().unwrap()`.
  O:431 must observe a callback thread different from the owner. Only shared
  mock state and stream cross threads, never the non-Send backend.
- Replace `payload.stream = Some(hold.stream());` with `payload.stream = None;`.
  R:318 must observe lost exact disposal-owner identity.

## Masking And Acceptance

Deleting only retirement's failure `return` stays on B and is masked by B's
Retiring guard. ID 14 deliberately advances to C. Individual registry-terminal
and driver-phase guard deletions do not prove retry coverage. An optional
compound deletion of both guards should fail R:317 after the first repetition
records `[A,B,B]`; label it combined-defense coverage, not two independent guards.

Reply field destruction still eventually yields EngineStopped if the explicit
Drop completion is removed. ID 3 therefore needs the before-payload-drop wake
observation. Existing CO1 distinct-value tests, not C3's identical repeated Stop
results, establish first-result-wins.

Require successful compilation, one exact selected test, its intended assertion
and complete source restoration. Compile failures, timeouts, source guards and
unrelated assertions do not qualify. Freeze exact source replacements and
diagnostic markers in the integrated plan before execution. No GPU completion,
native authority, formal correspondence or performance is established here.

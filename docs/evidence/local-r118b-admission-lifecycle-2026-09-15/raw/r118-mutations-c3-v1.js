// Prospective negatives; helper calibrations are not production mutations.
const base = 'crates/fe2o3-runtime/src/async_engine/';
const oracle_path = base + 'tests/owned_tests/preparation_tests/completion_tests.rs';
const prefix = 'async_engine::tests::owned_tests::preparation_tests::completion_tests::';
const tests = {
  I: prefix + 'reserved_completion_isolates_discard_order_and_abandoned_observers',
  S: prefix + 'reserved_stop_notifies_latest_wakers_before_disposal_and_contains_wake_panic',
  D: prefix + 'reserved_discard_wakes_before_payload_drop_even_when_wake_panics',
  R: prefix + 'unpublished_retirement_preserves_exact_failed_owner_and_untouched_neighbor',
  O: prefix + 'owned_shutdown_retires_in_order_and_retains_failed_and_unvisited_owners',
};
const G = 'generated_operation.rs';
const O = 'operation.rs';
const W = 'owned.rs';
const A = 'generated_operation/adoption.rs';
const H = 'tests/owned_tests/preparation_tests/adoption_tests.rs';
function patch(file, from, old, replacement = '') {
  if (!old || from.split(old).length !== 2) throw new Error('ambiguous inner replacement');
  return {path: base + file, edits: [[from, from.replace(old, replacement)]]};
}
const mutations = [];
function add(name, alias, line, patches, expected = 'assertion `left == right` failed', kind = 'production') {
  mutations.push({name: 'c3-' + name, kind, test: tests[alias], oracle_path,
    oracle_line: line, expected: [expected], patches});
}
add('omit-producer-transfer', 'I', 67, [patch(G,
  '        self.roster = Some(roster);\n        self.completion = completion.take();\n',
  'self.completion = completion.take();', 'drop(completion.take());')],
  'assertion failed: poll_completion(ticket.as_mut().unwrap(), waker).is_pending()');
add('omit-reject-completion-stop', 'S', 197, [patch(G,
  '        stop_reply(&mut self.reply, Some(&self.control), error);\n        stop_reply(&mut self.completion, None, error);\n        drop(self.prepare.take());\n',
  '        stop_reply(&mut self.completion, None, error);\n')]);
const dropStop = '        stop_reply(\n            &mut self.completion,\n            None,\n            RuntimeAsyncEngineCallErrorV1::EngineStopped,\n        );\n';
add('omit-drop-completion-stop', 'D', 230, [patch(G, dropStop, dropStop)]);
add('dispose-before-stop-observations-close', 'S', 176, [patch(O,
  '                panicked = true;\n            }\n        }\n        panicked\n',
  '        panicked\n', '        self.dispose_quiescent();\n        panicked\n')]);
add('retain-old-waker', 'S', 189, [patch(W,
  '            let old_waker = if result.is_none() {\n                core::mem::replace(&mut state.waker, new_waker.take())\n',
  'core::mem::replace(&mut state.waker, new_waker.take())',
  '{ if state.waker.is_none() { state.waker = new_waker.take(); } None }')],
  'assertion failed: old.lock().unwrap().is_empty()');
const wake = '        if let Some(waker) = waker\n            && let Err(payload) = catch_unwind(AssertUnwindSafe(|| waker.wake()))\n        {\n            core::mem::forget(payload);\n        }\n';
add('omit-wake-panic-containment', 'S', 175, [patch(W, wake, wake,
  '        if let Some(waker) = waker { waker.wake(); }\n')],
  'assertion failed: !context.is_terminal()');
add('retain-abandoned-waker', 'I', 88, [patch(W,
  'impl<R> Drop for RuntimeAsyncCommandFutureV1<R> {\n    fn drop(&mut self) {\n        self.clear_waker();\n    }\n}\n',
  '        self.clear_waker();\n')]);
const discard = '        drop(self.parked.remove(index).expect("matched parked owner"));\n        true\n';
add('discard-wrong-owner', 'I', 77, [patch(O, discard,
  'remove(index)', 'remove((index + 1) % self.parked.len())')]);
add('stop-neighbor-on-discard', 'I', 88, [patch(O, discard,
  '        drop(self.parked',
  '        for (other, entry) in self.parked.iter_mut().enumerate() {\n            if other != index {\n                entry.driver.reject(RuntimeAsyncEngineCallErrorV1::EngineStopped);\n            }\n        }\n        drop(self.parked')]);
add('release-reply-credit-before-consumption', 'I', 89, [patch(W,
  '            state.result = Some(result);\n            state.waker.take()\n',
  '            state.result', '            drop(state._permit.take());\n            state.result')]);
add('omit-unpublished-hold-release', 'R', 310, [patch(A,
  '        context.release_unpublished_hold_v1(&owner.hold)?;\n        owner.phase = PhaseV1::Retired;\n',
  '        context.release_unpublished_hold_v1(&owner.hold)?;\n')]);
add('forget-retired-owner', 'R', 306, [patch(O,
  '                    self.retire_stream(entry.stream);\n                    drop(entry);\n',
  'drop(entry);', 'core::mem::forget(entry);')]);
add('omit-retirement-callback', 'R', 317, [patch(A,
  '        owner.phase = PhaseV1::Retiring;\n        (self.adoption.as_ref().expect("admitted hooks").retire)(context, &owner.hold)?;\n',
  '        (self.adoption.as_ref().expect("admitted hooks").retire)(context, &owner.hold)?;\n')]);
add('continue-retirement-after-failure', 'R', 317, [patch(O,
  '                failed => {\n                    if let Err(payload) = failed {\n                        core::mem::forget(payload);\n                    }\n                    context.quarantine_after_async_command_panic_v1();\n                    return;\n',
  '                    return;\n', '                    index += 1;\n')]);
add('drop-failed-shutdown-owners', 'O', 414, [patch(W,
  '                    Ok((cleanup, native_failure)) => {\n                        core::mem::forget(operations);\n                        core::mem::forget(context);\n',
  'core::mem::forget(operations);', 'drop(operations);')]);
add('omit-shutdown-retirement', 'O', 406, [patch(W,
  '                    operations.retire_unpublished_v1(&mut context, usize::MAX);\n                    let cleanup = context.cleanup();\n',
  '                    operations.retire_unpublished_v1(&mut context, usize::MAX);\n')]);
add('remove-both-retirement-retry-guards', 'R', 317, [
  patch(O, '        if context.is_terminal() {\n            return;\n        }\n        let mut index = 0;\n',
    '        if context.is_terminal() {\n            return;\n        }\n'),
  patch(A, '        if !matches!(owner.phase, PhaseV1::Adopting | PhaseV1::Adopted) {\n            return Err(RuntimeValidationErrorV1::ContextTerminal.into());\n        }\n        owner.phase = PhaseV1::Retiring;\n',
    '        if !matches!(owner.phase, PhaseV1::Adopting | PhaseV1::Adopted) {\n            return Err(RuntimeValidationErrorV1::ContextTerminal.into());\n        }\n'),
], 'assertion `left == right` failed', 'production-combined-defense');
add('calibrate-wrong-callback-thread', 'O', 431, [patch(H,
  '        self.record("adoption_retire");\n        retire(&self.inner.state, stream)\n',
  '        retire(&self.inner.state, stream)\n',
  '        let state = self.inner.state.clone();\n        thread::spawn(move || retire(&state, stream)).join().unwrap()\n')],
  'assertion `left == right` failed', 'helper-calibration');
add('calibrate-lost-disposal-identity', 'R', 318, [patch(H,
  '            payload.stream = Some(hold.stream());\n            payload.state.lock().unwrap().adoption_order.push("adopt");\n',
  'payload.stream = Some(hold.stream());', 'payload.stream = None;')],
  'assertion `left == right` failed', 'helper-calibration');
module.exports = {tests, mutations};

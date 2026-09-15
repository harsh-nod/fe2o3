// Prospective scoped mutations only; this program never changes runtime source.
const fs = require('fs'), assert = require('assert'), crypto = require('crypto');
const root = '/home/harsh/.codex-tmp/';
const checkerHash = 'd83f5ab4a189ab67e52d1a938bfacf85d01081b8bdcd3646ca96023387892c4e';
const sha = bytes => crypto.createHash('sha256').update(bytes).digest('hex');
assert.strictEqual(sha(fs.readFileSync(root + 'r123-development-check-v1.js')), checkerHash);
const C = require(root + 'r123-development-check-v1.js');
const core = C.E.core;
assert.deepStrictEqual(C.identities(), C.map);
const base = 'crates/fe2o3-kfd/src/';
const sequence = base + 'queue_live/retained_control_release.rs';
const direct = base + 'queue_live/fixed_dispatch.rs';
const facade = base + 'queue_live.rs';
const constructed = base + 'queue_live/construction_auxiliary/integration_retained_control_tests.rs';
const publicTests = base + 'queue_live/rebind_tests/retained_control_release.rs';
const shellTests = base + 'queue_live/rebind_tests.rs';
const c = 'queue::live::construction_primary::integration_tests::auxiliary_cases::prefix_cases::retained_control_cases::';
const p = 'queue::live::rebind_tests::retained_control_release::';
const scopes = {
  [sequence]: {start: 'pub(in crate::queue) fn settle_retained_control_release_v1(', end: '\nimpl RetainedControlReleaseContextV1 for ComputeAqlQueueSessionV1 {'},
  [direct]: {start: '    pub fn release_retained_persistent_fixed_dispatch_control_v1(', end: '\n    pub(super) fn detach_recycled_fixed_dispatch_inner('},
  [facade]: {start: '    pub fn release_retained_persistent_fixed_dispatch_control_v1(', end: '\n    pub fn bind_fixed_dispatch<const N: usize>('},
};
function oracle(file, line, token, fragments) {
  const source = fs.readFileSync(C.repo + '/' + file, 'utf8');
  assert.strictEqual(sha(source), C.map[file]);
  const row = source.split('\n')[line - 1], offset = row.indexOf(token);
  assert(offset >= 0, 'declared assertion token');
  return {location: file + ':' + line + ':' + (offset + 1), fragments};
}
const opening = c + 'retained_control_exhausted_real_opening_restores_exact_owner_without_poison_or_retake';
const modelPanic = c + 'retained_control_lower_panic_survives_model_driver_poison_panic';
const values = [
  ['01-opening-terminal', sequence, 'terminal_on_error = entered;', 'terminal_on_error = true;', opening,
    oracle(constructed, 498, 'assert!', ['assertion failed: !released.transport'])],
  ['02-retake-error', sequence, 'retake?;', 'let _ = retake;',
    c + 'retained_control_retake_error_precedes_lower_error_but_never_lower_panic',
    oracle(constructed, 564, 'panic!', ['unexpected retained-control retake provenance'])],
  ['03-lower-panic', sequence, 'core::mem::forget(result);\n            Err(payload)', 'core::mem::forget(payload);\n            result', modelPanic,
    oracle(constructed, 986, 'assert_eq!', ['assertion `left == right` failed', 'left: None', 'right: Some(("N2 native panic", "free"))'])],
  ['04-poison-kind', sequence, 'context.poison(result.is_err())', 'context.poison(false)', modelPanic,
    oracle(constructed, 993, 'assert_eq!', ['assertion `left == right` failed', 'left: (1, false)', 'right: (1, true)'])],
  ['05-final-poison', sequence, 'if result.is_err() {\n                core::mem::forget(payload);', 'if result.is_err() {\n                result = Err(payload);',
    c + 'retained_control_final_poison_panic_stays_settled_and_preserves_existing_lower_panic',
    oracle(constructed, 778, 'assert_eq!', ['assertion `left == right` failed', 'left: None', 'right: Some(("N2 native panic", "free"))'])],
  ['06-original-restore', sequence, '*context.dispatch() = Some(dispatch);', 'core::mem::forget(dispatch);', opening,
    oracle(constructed, 505, 'assert!', ['opening rejection restores dispatch custody'])],
  ['07-direct-transport', direct, 'if settled.transport {', 'if false && settled.transport {',
    p + 'retained_control_direct_missing_engine_transports_parent_before_returning_error',
    oracle(shellTests, 187, 'assert!', ['assertion failed: session.completion_owner.0.is_none()'])],
  ['08-sticky-transport', facade, '*self.terminal_transport |= settled.transport;', '*self.terminal_transport = settled.transport;',
    p + 'retained_control_facade_restores_exact_selected_owner_before_sticky_parent_transport',
    oracle(publicTests, 155, 'assert!', ['retry cannot erase prior terminal transport'])],
  ['09-generation', sequence, '.validate_detached_persistent_control_release_v1(generation)?;', '.validate_detached_persistent_control_release_v1(generation).unwrap_or(());',
    c + 'retained_control_generation_rejection_precedes_currentness_and_loan',
    oracle(constructed, 906, 'assert!', ['preflight preserves original control custody'])],
  ['10-currentness', sequence, 'context.check_currentness()?;', 'context.check_currentness().unwrap_or(());',
    c + 'retained_control_currentness_failure_preserves_unconsumed_owner',
    oracle(constructed, 944, 'assert!', ['assertion failed: matches!(released.result,', 'MemorySessionError::Injected("currentness")'])],
];
const mutations = values.map(([id, file, from, to, test, primary]) => {
  assert(C.p.new_tests.kfd.includes(test));
  const original = fs.readFileSync(C.repo + '/' + file, 'utf8');
  assert.strictEqual(sha(original), C.map[file]);
  const patch = {path: file, scope: scopes[file], edits: [[from, to]]};
  const changed = core.mutatedSource(original, patch);
  const reverse = {...patch, edits: [[to, from]]};
  assert.strictEqual(core.mutatedSource(changed, reverse), original);
  const map = {...C.map, [file]: sha(changed)};
  const item = {id, kind: 'kfd', test, patch, expected_file_sha256: sha(changed),
    expected_source_map_sha256: sha(JSON.stringify(map)), oracle: primary, final_panic: 'primary'};
  if (id === '08-sticky-transport') {
    item.final_panic = 'caught_then_unwrap';
    item.following_panic = oracle(publicTests, 199, 'unwrap()', ['called `Result::unwrap()` on an `Err` value']);
  }
  return item;
});
assert.strictEqual(new Set(mutations.map(item => item.expected_source_map_sha256)).size, 10);
const plan = {
  accepted: false,
  scope: 'Ten predeclared R123 behavioral negatives; compile failures, aborts and unrelated assertions do not qualify.',
  source_plan: 'r123-development-full-plan-v1.json', source_plan_sha256: '049a06fa35d9933cba77c0717fbc769f8244f3d160cb541ba7f4d3efc2213ee7',
  checker: 'r123-development-check-v1.js', checker_sha256: checkerHash,
  source_parent: C.p.source_parent, source_map_sha256: C.p.source_map_sha256, source_count: C.p.source_count,
  runner: C.p.runner, runner_sha256: C.p.runner_sha256,
  test_command: ['cargo', '+nightly-2026-04-03', 'test', '--locked', '--offline', '--all-features', '-p', 'fe2o3-kfd', '--lib'],
  test_count: C.p.counts.kfd,
  diagnostic_path_aliases: {[constructed]: base + 'queue_live/construction_primary/../construction_auxiliary/integration_retained_control_tests.rs'},
  excluded_source_only_guards: ['completed-root acceptance is additionally guarded by the lower borrowed driver; deleting the final is_complete guard alone has no decisive reachable negative'],
  mutations,
};
assert.deepStrictEqual(C.identities(), C.map);
C.stable();
console.log(JSON.stringify(plan, null, 2));

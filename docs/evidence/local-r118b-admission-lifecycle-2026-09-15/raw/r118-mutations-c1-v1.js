// Prospective method-scoped negatives, not evidence of compiled test failures.
const path = 'crates/fe2o3-runtime/src/context.rs';
const oracle_path = 'crates/fe2o3-runtime/src/context/tests/submission_identity_tests.rs';
const prefix = 'context::tests::submission_identity_tests::';
const tests = Object.fromEntries(Object.entries({
  M: 'every_ingress_rejects_each_coordinate_for_pending_and_event_completed_records',
  V: 'exact_tokens_reach_each_ingress_with_state_appropriate_results',
  C: 'cached_success_never_bypasses_any_identity_coordinate',
  L: 'released_cached_token_is_rejected_at_every_ingress',
  B: 'real_backend_id_reuse_does_not_revive_a_released_cached_token',
  U: 'destroyed_stream_keeps_identity_validation_and_retained_query_semantics',
  D: 'wait_validates_identity_before_deadline_but_drain_checks_expiration_first',
  T: 'actual_terminal_backend_failure_precedes_ingress_identity_except_pure_query',
  G: 'genuine_graph_reservation_preserves_query_and_public_ingress_precedence',
}).map(([key, value]) => [key, prefix + value]));
const scopes = {
  gate: ['    fn require_graph_access(', '    fn submission_record<A>('],
  record: ['    fn submission_record<A>(', '    fn live_submission_record<A>('],
  live: ['    fn live_submission_record<A>(', '    fn backend_result<T>('],
  Poll: ['    pub(crate) fn poll_with_graph_access_v1<A>(', '    pub fn wait<A>('],
  Wait: ['    pub fn wait<A>(', '    pub fn query_submission<A>('],
  Query: ['    pub fn query_submission<A>(', '    pub fn query_stream('],
  Callback: ['    pub fn on_completion<A, F>(', '    pub const fn completion_callback_panic_count('],
  Release: ['    fn release_submission_ref<A>(', '    pub fn record_event<A>('],
  Event: ['    pub fn record_event<A>(', '    pub fn query_event('],
  Cancel: ['    pub fn cancel<A>(', '    pub fn drain<A>('],
  Drain: ['    pub fn drain<A>(', '\n}\n\nfn validate_byte_range('],
};
const mutations = [];
function add(name, method, alias, line, expected, edits) {
  const [start, end] = scopes[method];
  mutations.push({name: 'c1-' + name, kind: 'production', test: tests[alias],
    oracle_path, oracle_line: line, expected: [expected], patches: [{path, scope: {start, end}, edits}]});
}
const raw = '*self.submissions.get(&submission.id).ok_or(RuntimeValidationErrorV1::UnknownSubmission)?';
for (const [method, checked, diagnostic] of [
  ['Poll', 'live_submission_record', 'Poll: Ok(Poll(Pending))'],
  ['Wait', 'live_submission_record', 'Wait: Ok(Poll(Succeeded))'],
  ['Callback', 'submission_record', 'Callback: Ok(Registered)'],
  ['Release', 'submission_record', 'Release: Err(Validation(SubmissionPending))'],
  ['Event', 'live_submission_record', 'Event: Ok(Event('],
  ['Cancel', 'submission_record', 'Cancel: Ok(Cancel(TooLate))'],
  ['Drain', 'submission_record', 'Drain: Ok(Poll(Succeeded))'],
]) add('bypass-' + method.toLowerCase() + '-identity', method, 'M', 505, diagnostic, [[
  'self.' + checked + '(submission)?', raw,
]]);
add('bypass-query-identity', 'Query', 'M', 505, 'Query: Ok(Query(Pending))', [[
  'Ok(self.submission_record(submission)?.status)', 'Ok((' + raw + ').status)',
]]);
const generation = '        if submission.id.context_generation != self.context_generation {\n            return Err(RuntimeValidationErrorV1::UnknownSubmission);\n        }\n';
add('canonicalize-foreign-generation', 'record', 'M', 505, 'Poll: Ok(Poll(Pending))', [
  [generation, ''],
  ['.get(&submission.id)', '.get(&RuntimeSubmissionIdV1::new(self.context_generation, submission.id.local))'],
]);
const backendLookup = [['.get(&submission.id)',
  '.values().find(|record| record.backend_submission == submission.backend_submission)']];
add('lookup-by-native-id', 'record', 'M', 505, 'Poll: Ok(Poll(Pending))', backendLookup);
add('lookup-by-reused-native-id', 'record', 'B', 505, 'Poll: Ok(Poll(Pending))', backendLookup);
for (const [name, field] of [['backend', 'backend_submission'], ['stream', 'stream'], ['device', 'device']]) {
  const predicate = 'record.' + field + ' != submission.' + field;
  add('omit-' + name + '-coordinate', 'record', 'M', 505, 'Poll: Ok(Poll(Pending))', [[
    predicate, 'false && ' + predicate,
  ]]);
}
const cached = [['        self.require_graph_access(access)?;\n',
  '        self.require_graph_access(access)?;\n        if let Some(done) = submission.completion { return Ok(done); }\n']];
add('trust-cached-completion', 'Poll', 'C', 505, 'Poll: Ok(Poll(Succeeded))', cached);
add('trust-released-cached-completion', 'Poll', 'L', 505, 'Poll: Ok(Poll(Succeeded))', cached);
const stream = '        let stream = self\n            .streams\n            .get(&record.stream)\n            .ok_or(RuntimeValidationErrorV1::UnknownStream)?;\n        if stream.device != record.device {\n            return Err(RuntimeValidationErrorV1::WrongDevice);\n        }\n';
add('omit-live-stream-validation', 'live', 'U', 505, 'Poll: Ok(Poll(Failed { code: -3 }))', [[stream, '']]);
const context = '        self.require_live()?;\n';
const liveRecord = '        let record = self.live_submission_record(submission)?;\n';
const record = '        let record = self.submission_record(submission)?;\n';
const terminal = '        if record.status.is_terminal() {\n            return Ok(submission.observe_status(record.status));\n        }\n';
const deadline = '        let deadline = Instant::now()\n            .checked_add(timeout)\n            .ok_or(RuntimeValidationErrorV1::InvalidDeadline)?;\n';
const expiry = '        if deadline <= Instant::now() {\n            return Err(RuntimeValidationErrorV1::InvalidDeadline.into());\n        }\n';
add('wait-deadline-before-identity', 'Wait', 'D', 1004, 'assertion failed:', [[
  context + liveRecord + terminal + deadline, context + deadline + liveRecord + terminal,
]]);
add('drain-identity-before-expiry', 'Drain', 'D', 1012, 'assertion failed:', [[
  context + expiry + record, context + record + expiry,
]]);
const waitContext = [[context + liveRecord + terminal + deadline, deadline + context + liveRecord + terminal]];
add('wait-deadline-before-terminal-context', 'Wait', 'T', 535, 'assertion failed:', waitContext);
add('wait-deadline-before-reserved-context', 'Wait', 'G', 535, 'assertion failed:', waitContext);
const drainContext = [[context + expiry, expiry + context]];
add('drain-expiry-before-terminal-context', 'Drain', 'T', 539, 'assertion failed:', drainContext);
add('drain-expiry-before-reserved-context', 'Drain', 'G', 539, 'assertion failed:', drainContext);
add('omit-terminal-context-check', 'gate', 'T', 505, 'Poll: Err(Validation(UnknownSubmission))', [[
  'if self.terminal {', 'if false && self.terminal {',
]]);
add('omit-context-reservation-check', 'gate', 'G', 505, 'Poll: Ok(Poll(Pending))', [[
  'else if self.graph_reservation != access {', 'else if false && self.graph_reservation != access {',
]]);
add('reservation-before-terminal', 'gate', 'G', 505, 'Poll: Err(Validation(ContextReserved))', [[
  '        if self.terminal {\n            Err(RuntimeValidationErrorV1::ContextTerminal)\n        } else if self.graph_reservation != access {\n            Err(RuntimeValidationErrorV1::ContextReserved)\n',
  '        if self.graph_reservation != access {\n            Err(RuntimeValidationErrorV1::ContextReserved)\n        } else if self.terminal {\n            Err(RuntimeValidationErrorV1::ContextTerminal)\n',
]]);
add('require-live-for-pure-query', 'Query', 'G', 1161, 'assertion `left == right` failed', [[
  '        Ok(self.submission_record(submission)?.status)',
  context + '        Ok(self.submission_record(submission)?.status)',
]]);
add('release-pending-submission', 'Release', 'V', 578, 'assertion failed:', [[
  '        if !record.quiescent {\n            return Err(RuntimeValidationErrorV1::SubmissionPending.into());\n        }\n', '',
]]);
add('retain-released-logical-record', 'Release', 'V', 721, 'assertion failed:', [[
  '        self.submissions.remove(&submission.id);\n', '',
]]);
add('retain-released-native-id', 'Release', 'V', 722, 'assertion failed:', [[
  '        self.backend_submissions.remove(&record.backend_submission);\n', '',
]]);
add('drop-completed-callback', 'Callback', 'V', 612, 'assertion `left == right` failed', [[
  'completion_callback_panicked_v1(callback, record.status)', '{ drop(callback); false }',
]]);
const neighbor = original => '        let routed_submission = self.submissions.values().find(|candidate| candidate.backend_submission != ' + original + ').unwrap().backend_submission;\n';
for (const [method, call, line, diagnostic] of [
  ['Poll', 'poll_v1', 663, 'assertion `left == right` failed'],
  ['Wait', 'wait_v1', 675, 'Wait: exact backend wait submission'],
  ['Drain', 'drain_v1', 675, 'Drain: exact backend wait submission'],
  ['Cancel', 'cancel_v1', 755, 'assertion `left == right` failed'],
]) {
  const extra = method === 'Wait' || method === 'Drain' ? ', deadline' : '';
  const from = '        let result = self.backend.' + call + '(record.backend_submission' + extra + ');';
  const to = neighbor('record.backend_submission') +
    '        let result = self.backend.' + call + '(routed_submission' + extra + ');';
  add('route-' + method.toLowerCase() + '-to-neighbor', method, 'V', line, diagnostic, [[from, to]]);
}
const event = '        let result = self\n            .backend\n            .record_event_v1(stream.backend_stream, submission_record.backend_submission);';
add('route-event-to-neighbor-submission', 'Event', 'V', 731, 'exact backend event stream and submission', [[
  event, neighbor('submission_record.backend_submission') +
    '        let result = self\n            .backend\n            .record_event_v1(stream.backend_stream, routed_submission);',
]]);
add('route-event-to-neighbor-stream', 'Event', 'V', 731, 'exact backend event stream and submission', [[
  event, '        let routed_stream = self.streams.values().find(|candidate| candidate.backend_stream != stream.backend_stream).unwrap().backend_stream;\n' +
    '        let result = self\n            .backend\n            .record_event_v1(routed_stream, submission_record.backend_submission);',
]]);
add('route-drain-through-wait', 'Drain', 'V', 684, 'Drain: exact backend drain entry', [[
  'self.backend.drain_v1(record.backend_submission, deadline)',
  'self.backend.wait_v1(record.backend_submission, deadline)',
]]);
for (const [name, statement, line, diagnostic] of [
  ['mutate-rejected-token', 'submission.completion = Some(RuntimePollV1::Succeeded);', 509, 'supplied token changed'],
  ['poll-before-identity-validation', 'let _ = self.backend.poll_v1(submission.backend_submission);', 514, 'retained owners or backend changed'],
]) add(name, 'Poll', 'M', line, diagnostic, [[
  '        self.require_graph_access(access)?;\n',
  '        self.require_graph_access(access)?;\n        ' + statement + '\n',
]]);
module.exports = {tests, mutations};

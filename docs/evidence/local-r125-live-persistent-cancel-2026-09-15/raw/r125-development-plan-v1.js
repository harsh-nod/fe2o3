// Prospective CPU regression plan derived from committed R124, never fresh test output.
const fs = require('fs'), path = require('path'), cp = require('child_process');
const assert = require('assert'), crypto = require('crypto');
const root = '/home/harsh/.codex-tmp', repo = path.join(root, 'fe2o3-r61-execution');
const head = 'c171d915047daa5153c1f77253e15260e55da8c5';
const archive = 'docs/evidence/local-r124-live-recycled-detach-2026-09-15/raw/';
const hash = bytes => crypto.createHash('sha256').update(bytes).digest('hex');
const git = args => cp.execFileSync('git', args, {cwd: repo, maxBuffer: 64 * 1024 * 1024}).toString();
const committed = name => git(['show', head + ':' + archive + name]);
assert.strictEqual(git(['rev-parse', 'HEAD']).trim(), head);
const oldBytes = committed('r124-development-full-plan-v2.json');
assert.strictEqual(hash(oldBytes), 'e0d5f50276944d64e626c8f7dac4a06477fd1a290c6a396ef2a011dbb568a63d');
const old = JSON.parse(oldBytes), runner = 'r125-development-run-v2.js';
assert.strictEqual(hash(fs.readFileSync(path.join(root, runner))), '526871b5decf9694f091d7ddaac208af1f0e3397d6f67da5e02428185781753f');
const map = Object.fromEntries([...new Set(git(['ls-files', '--cached', '--others', '--exclude-standard', '-z']).split('\0'))]
  .filter(n => n && !n.startsWith('docs/')).sort().map(n => [n, hash(fs.readFileSync(path.join(repo, n)))]));
assert.strictEqual(Object.keys(map).length, 5709);
assert.strictEqual(hash(JSON.stringify(map)), '902a842fa783f96fcb96e44e03a9be07aea7302bc6ccd02949abaa1416f50833');
const declarations = [
  ['crates/fe2o3-kfd/src/queue_live/construction_auxiliary/integration_persistent_cancel_tests.rs',
    'queue::live::construction_primary::integration_tests::auxiliary_cases::prefix_cases::persistent_cancel_cases::', [
      'persistent_cancel_constructed_native_cleanup_preserves_exact_control_prefix_and_data',
      'persistent_cancel_constructed_restores_exact_initialized_cold_and_digest_free_inputs',
      'persistent_cancel_constructed_model_errors_and_panics_retain_complete_roster',
      'persistent_cancel_constructed_shape_rejection_precedes_native_restore_and_ledger',
      'persistent_cancel_constructed_restore_and_cancel_prefixes_retain_failed_leases',
      'persistent_cancel_constructed_retake_error_or_panic_overrides_normal_lower_error',
      'persistent_cancel_constructed_lower_cleanup_retains_data_and_primary_panic',
      'persistent_cancel_constructed_output_and_ledger_panics_keep_all_restored_owners',
      'persistent_cancel_constructed_three_preflights_reject_before_changing_any_prefix',
      'persistent_cancel_constructed_single_returned_initialization_is_not_downgraded',
    ]],
  ['crates/fe2o3-kfd/src/queue_live/rebind_tests/persistent_cancel.rs',
    'queue::live::rebind_tests::persistent_cancel::', [
      'persistent_cancel_public_terminal_reentry_preserves_complete_cancellation_custody',
      'persistent_cancel_public_terminal_ingress_preserves_preexisting_detached_custody',
      'persistent_cancel_public_wrong_state_preserves_exact_published_lease',
      'persistent_cancel_public_terminal_wrong_shape_or_generation_returns_exact_receipt',
    ]],
  ['crates/fe2o3-kfd/src/queue_dispatch_binding/control_release/persistent_tests.rs',
    'queue::dispatch_binding::control_release::tests::persistent::', [
      'persistent_cancellation_inherits_detached_continuation_without_minting_recycle_history',
    ]],
];
const additions = declarations.flatMap(([file, prefix, names]) => {
  const text = fs.readFileSync(path.join(repo, file), 'utf8');
  for (const name of names) assert.strictEqual(text.split('#[test]\nfn ' + name + '()').length, 2, name);
  return names.map(name => prefix + name);
}).sort();
assert.strictEqual(additions.length, 15);
assert.strictEqual(new Set(additions).size, 15);
const newer = text => text.replaceAll('r124-development-', 'r125-development-');
const fullRuns = old.full_runs.map(spec => ({
  name: 'r125-development-' + spec.kind + '-all-v2', kind: spec.kind,
  predecessor: spec.kind === 'gnu' ? null : 'r125-development-gnu-check-v1.json',
  command: spec.command, runner, deadline_ms: 3600000,
  accepted_log: archive + spec.name + '.log',
  accepted_log_sha256: hash(committed(spec.name + '.log')),
}));
let previous = 'r125-development-full-check-v1.json';
const gates = old.gates.map(spec => {
  const next = JSON.parse(newer(JSON.stringify(spec)));
  next.accepted_log = archive + spec.run + '.log';
  next.accepted_log_sha256 = hash(committed(spec.run + '.log'));
  next.predecessor = previous;
  previous = next.run + '.json';
  return next;
});
assert.strictEqual(gates.length, 25);
for (const [name, digest] of Object.entries(old.parser_inputs))
  assert.strictEqual(hash(fs.readFileSync(path.join(root, name))), digest);
const E = require('./r119-integrated-qualification-evidence-v1.js');
const changed = new Set(git(['diff', '--name-only', '-z']).split('\0'));
function fence(text, line) {
  const rows = text.split('\n'), start = line - 1;
  assert(/^\s*\/\/\/ \x60{3}/.test(rows[start]), 'doc fence at declared line');
  let end = start + 1;
  while (end < rows.length && !/^\s*\/\/\/ \x60{3}\s*$/.test(rows[end])) end++;
  assert(end < rows.length);
  return rows.slice(start, end + 1).join('\n');
}
const relocations = E.passing(committed('r124-development-gate-gnu-docs-v1.log')).flatMap(name => {
  const match = /^(crates\/[^ ]+\.rs) - .+ \(line (\d+)\)$/.exec(name);
  if (!match || !changed.has(match[1])) return [];
  const file = match[1], from = Number(match[2]);
  const block = fence(git(['show', head + ':' + file]), from);
  const rows = fs.readFileSync(path.join(repo, file), 'utf8').split('\n');
  const width = block.split('\n').length;
  const positions = rows.flatMap((_, index) => rows.slice(index, index + width).join('\n') === block ? [index + 1] : []);
  assert.strictEqual(positions.length, 1, 'unique byte-identical fence: ' + name);
  return positions[0] === from ? [] : [{path: file, from, to: positions[0], fence_sha256: hash(block)}];
});
const attempted = ['r125-development-gnu-all-v1.json', 'r125-development-gnu-all-v1.log',
  'r125-development-gnu-all-v1-source.json', 'r125-development-gnu-all-v1-source-after.json',
  'r125-development-run-v1.js'];
const attempt = JSON.parse(fs.readFileSync(path.join(root, attempted[0])));
assert.strictEqual(attempt.signal, 'SIGTERM');
assert.strictEqual(attempt.child_closed, true);
assert.deepStrictEqual(attempt.process_group_cleanup.close.live_members, []);
const plan = {
  accepted: false,
  scope: 'R125 CPU/runtime regression only; mutation, native, formal, aggregate-memory and performance acceptance remain separate.',
  source_parent: head, accepted_commit: head,
  source_map: 'r125-development-gnu-all-v2-source.json',
  source_map_sha256: hash(JSON.stringify(map)), source_count: Object.keys(map).length,
  admitted_boot: '74856d82-79a2-4377-b6c4-aaf01159fd19',
  runner, runner_sha256: hash(fs.readFileSync(path.join(root, runner))),
  runners: {[runner]: hash(fs.readFileSync(path.join(root, runner)))},
  parser_inputs: old.parser_inputs,
  accepted_inputs: {
    [archive + 'r124-development-full-plan-v2.json']: hash(oldBytes),
    [archive + 'r124-development-gate-check-v2.js']: hash(committed('r124-development-gate-check-v2.js')),
  },
  counts: {...old.counts, baseline_full: old.counts.full, full: old.counts.full + additions.length,
    baseline_kfd: old.counts.kfd, kfd: old.counts.kfd + additions.length,
    baseline_runtime: old.counts.runtime},
  new_tests: {kfd: additions, runtime: []},
  full_runs: fullRuns, gates, doc_relocations: relocations, auxiliary_counts: old.auxiliary_counts,
  normalization_contract: {
    standalone_cache_wait: old.normalization_contract.standalone_cache_wait,
    linux_split: 'Committed R124 is the baseline; retain only the existing exact Linux split-row normalization.',
    docs: 'Only declared complete byte-identical fence relocations are permitted.',
  },
  superseded_attempts: {
    classification: 'GNU v1 intentionally interrupted before adding reviewed public re-entry/mixed-state coverage; not qualifying.',
    inputs: Object.fromEntries(attempted.map(name => [name, hash(fs.readFileSync(path.join(root, name)))])),
  },
  acceptance: 'Require exact source endpoints, commands, environment, runner, deadlines, normal child and current-boot group closure, predecessor hashes, complete executable/test/ignored/summary rosters plus the declared 15 KFD tests, and all 25 leaves. This plan does not accept R125 or establish native execution, formal correspondence or HIP/HSA parity.',
};
fs.writeFileSync(path.join(root, 'r125-development-full-plan-v1.json'), JSON.stringify(plan, null, 2) + '\n', {flag: 'wx'});
console.log(JSON.stringify({plan_sha256: hash(JSON.stringify(plan, null, 2) + '\n'), source_count: plan.source_count,
  expected_full_tests: plan.counts.full, new_tests: additions.length, doc_relocations: relocations}));

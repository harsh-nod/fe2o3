// Freeze a prospective R124 plan against committed R123, not its older inputs.
const fs = require('fs'), cp = require('child_process'), crypto = require('crypto'), assert = require('assert');
const root = '/home/harsh/.codex-tmp/';
const repo = root + 'fe2o3-r61-execution';
const head = '948d1f2637ef08061aabf42485929869e3ed5dbc';
const raw = 'docs/evidence/local-r123-live-retained-control-release-2026-09-15/raw/';
const sourceHash = 'a633edf685c4cbe3a24b1500a6a086ba36d39bec9b042abb5f88c52e4024cc24';
const hash = bytes => crypto.createHash('sha256').update(bytes).digest('hex');
const bytes = name => fs.readFileSync(root + name);
const read = name => JSON.parse(bytes(name));
const git = args => cp.execFileSync('git', args, {cwd: repo, maxBuffer: 64 * 1024 * 1024}).toString();
const committed = name => git(['show', head + ':' + name]);
assert.strictEqual(git(['rev-parse', 'HEAD']).trim(), head);
const old = JSON.parse(committed(raw + 'r123-development-full-plan-v1.json'));
const names = text => [...text.matchAll(/^test (.+) \.\.\. ok$/gm)].map(match => match[1]);
const baseline = committed(raw + 'r123-development-gnu-all-v1.log');
const existing = new Set(names(baseline));
assert.strictEqual(names(baseline).length, 2865);
const regressionLog = bytes('r124-development-regression-02.log').toString();
const observed = names(regressionLog);
assert.strictEqual(observed.length, 158);
assert.strictEqual(new Set(observed).size, 158);
assert(/^test result: ok\. 158 passed; 0 failed; 0 ignored; 0 measured; 1068 filtered out;/m.test(regressionLog));
const added = observed.filter(name => !existing.has(name)).sort();
assert.strictEqual(added.length, 17);
assert.strictEqual(added.filter(name => name.includes('::recycled_detach_cases::')).length, 13);
assert.strictEqual(added.filter(name => name.includes('::rebind_tests::recycled_detach::')).length, 4);
const sourceName = 'r124-development-regression-02-source.json';
const source = read(sourceName);
assert.strictEqual(hash(JSON.stringify(source)), sourceHash);
assert.strictEqual(Object.keys(source).length, 5706);
const current = Object.fromEntries([...new Set(git(['ls-files', '--cached', '--others', '--exclude-standard', '-z']).split('\0'))]
  .filter(name => name && !name.startsWith('docs/')).sort().map(name => [name, hash(fs.readFileSync(repo + '/' + name))]));
assert.deepStrictEqual(current, source);
const rename = text => text.replaceAll('r123-development-', 'r124-development-');
const full = old.full_runs.map(spec => ({...spec,
  name: rename(spec.name),
  predecessor: spec.kind === 'gnu' ? 'r124-development-clippy-02.json' : 'r124-development-gnu-all-v1.json',
  accepted_log: raw + spec.name + '.log',
  accepted_log_sha256: hash(committed(raw + spec.name + '.log')),
  deadline_ms: 3600000,
}));
assert.deepStrictEqual(full.map(spec => spec.accepted_log_sha256), [
  '70feffc934b4f0a05d0d7fc9ae4a58fc317d13ea5625c6e4dbcf6c70e704ea7b',
  '453c3fc2661d35cbfbc27c9a215d1e7b8ea9cc7170f19ffe7fd82ce7ae0e47d4',
]);
const gates = old.gates.map(spec => ({...spec,
  run: rename(spec.run), predecessor: rename(spec.predecessor), command: spec.command.map(rename),
  ...(spec.stderr_log ? {stderr_log: rename(spec.stderr_log)} : {}),
  accepted_log: raw + spec.run + '.log',
  accepted_log_sha256: hash(committed(raw + spec.run + '.log')),
  deadline_ms: 1800000,
}));
assert.strictEqual(gates.length, 25);
const anchors = ['r124-development-regression-02', 'r124-development-clippy-02'].map(name => {
  const record = read(name + '.json');
  assert.strictEqual(record.returncode, 0);
  assert.strictEqual(record.child_returncode, 0);
  assert.strictEqual(record.child_closed, true);
  assert.strictEqual(record.timed_out, false);
  assert.strictEqual(record.source_head, head);
  assert.strictEqual(record.source_map_sha256, sourceHash);
  assert.strictEqual(record.source_unchanged, true);
  assert.deepStrictEqual(read(name + '-source.json'), source);
  assert.deepStrictEqual(read(name + '-source-after.json'), source);
  return {name, command: record.command, predecessor: record.clock.predecessor.name,
    runner: record.runner.name, record_sha256: hash(bytes(name + '.json')),
    log_sha256: hash(bytes(name + '.log')), passed: name.includes('regression') ? 158 : null};
});
const docRelocations = [
  ['queue_dispatch_binding.rs', 3018, 3043, '4f33eb779ccdbf31d318b8918a17666b1be38fa2326e1d47fd39f4d10a9aa6b0'],
  ['queue_dispatch_binding.rs', 3028, 3053, '5b9c98efbe78d001d3b2d74c4b83f6ffee64484eaad1eefb3cf6511f5f41d46c'],
  ['queue_dispatch_binding.rs', 3036, 3061, '6c490849d07d33cbc9e88cc727c5a38ee7bfed15843d9e67258fa69a872b50ef'],
  ['queue_dispatch_binding.rs', 3062, 3087, '62ae8451926209736f88f8bdc599ef548c2cd4c68b44809b3d557cf7004ab9a1'],
  ['queue_live.rs', 3823, 3825, '7292b71766209846e2100e19e1a5940936f39d3cbddd88859947f704dfa43c91'],
  ['queue_live.rs', 3878, 3880, 'a4ab9bceea27b3963d9e0edbea46d56643788ea7d44fa9ebf5a3f828e297eb0c'],
  ['queue_live.rs', 4362, 4364, 'a06c5972edc54f8bcb05059c11847a63bdb2a86af60c3d8faad20cb2a9647715'],
].map(([name, from, to, fence_sha256]) => ({path: 'crates/fe2o3-kfd/src/' + name, from, to, fence_sha256}));
const acceptedNames = ['r123-development-full-plan-v1.json', 'r123-development-gate-check-v2.js'];
const plan = {
  accepted: false,
  scope: 'R124 CPU/runtime compatibility only; mutations, archive, native, formal and performance acceptance remain separate.',
  source_parent: head, source_map: sourceName, source_map_sha256: sourceHash, source_count: 5706,
  runner: 'r124-development-run-v2.js', runner_sha256: hash(bytes('r124-development-run-v2.js')),
  runners: Object.fromEntries(['r124-development-run-v1.js', 'r124-development-run-v2.js'].map(name => [name, hash(bytes(name))])),
  parser_inputs: old.parser_inputs,
  accepted_commit: head,
  accepted_inputs: Object.fromEntries(acceptedNames.map(name => [raw + name, hash(committed(raw + name))])),
  counts: {baseline_full: 2865, full: 2882, baseline_kfd: 1209, kfd: 1226, baseline_runtime: 743, runtime: 743, ignored: 5, harnesses: 48, csv_targets: 1},
  new_tests: {kfd: added, runtime: []}, anchors, anchor_test_count: 158, full_runs: full, gates,
  doc_relocations: docRelocations,
  auxiliary_counts: old.auxiliary_counts,
  normalization_contract: {...old.normalization_contract,
    linux_split: 'Actual baseline is committed R123. The separately pinned historical R119 split-row transcript is only a parser calibration fixture.',
    docs: 'Only the seven declared relocations of complete byte-identical fences are permitted.'},
  acceptance: 'Require exact commands, declared runners/deadlines, unchanged source endpoints, valid clock/predecessor identities, normal child and process-group closure, exact accepted executable/test/ignored rosters plus only the declared seventeen KFD tests, and all summary fields. Permit only byte-identical doctest fence relocation and the existing narrow Linux/cache-wait transcript rules. This plan does not accept the packet or establish hardware, formal correspondence, aggregate memory or performance parity.',
};
const output = root + 'r124-development-full-plan-v1.json';
fs.writeFileSync(output, JSON.stringify(plan, null, 2) + '\n', {flag: 'wx'});
console.log(JSON.stringify({output, sha256: hash(fs.readFileSync(output)), accepted: false, expected_counts: plan.counts}));

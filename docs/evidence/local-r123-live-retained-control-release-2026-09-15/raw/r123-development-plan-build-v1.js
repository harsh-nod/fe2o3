// Emit a prospective plan from pinned accepted logs and the completed development cohort.
const fs = require('fs'), cp = require('child_process'), crypto = require('crypto'), assert = require('assert');
const root = '/home/harsh/.codex-tmp/';
const repo = root + 'fe2o3-r61-execution';
const head = 'd1c87064bd10e513f5aad328e0b3445bd78c296a';
const raw = 'docs/evidence/local-r122-live-data-release-2026-09-15/raw/';
const sourceHash = '8380746325d6717df78a608eb5be8f3ff3c48e42f32eec19aba2ab0677fdcb08';
const hash = bytes => crypto.createHash('sha256').update(bytes).digest('hex');
const bytes = name => fs.readFileSync(root + name);
const read = name => JSON.parse(bytes(name));
const git = name => cp.execFileSync('git', ['show', head + ':' + name], {cwd: repo, maxBuffer: 64 * 1024 * 1024}).toString();
const old = read('r122-development-gates-v2.json');
assert.strictEqual(bytes('r122-development-gates-v2.json').toString(), git(raw + 'r122-development-gates-v2.json'));
const mutations = read('r122-development-mutations-v1.json');
assert.strictEqual(bytes('r122-development-mutations-v1.json').toString(), git(raw + 'r122-development-mutations-v1.json'));
const names = text => [...text.matchAll(/^test (.+) \.\.\. ok$/gm)].map(match => match[1]);
const existing = new Set(names(git(raw + 'r122-development-gnu-all-v1.log')));
const added = names(bytes('r123-development-focused-05.log').toString()).filter(name => !existing.has(name)).sort();
assert.strictEqual(added.length, 19);
assert.strictEqual(added.filter(name => name.includes('::retained_control_cases::')).length, 14);
assert.strictEqual(added.filter(name => name.includes('::rebind_tests::retained_control_release::')).length, 4);
assert.strictEqual(added.filter(name => name === 'queue::tests::retained_control_revision_reseal_authentication_rejects_identity_and_seal_corruption').length, 1);
const source = read('r123-development-focused-05-source.json');
assert.strictEqual(hash(JSON.stringify(source)), sourceHash);
assert.strictEqual(Object.keys(source).length, 5703);
const rename = text => text.replaceAll('r122-development-', 'r123-development-');
const full = old.full_runs.map(spec => ({...spec,
  name: rename(spec.name),
  predecessor: spec.kind === 'gnu' ? 'r123-development-r122-regression-01.json' : 'r123-development-gnu-all-v1.json',
  accepted_log: raw + spec.name + '.log',
  accepted_log_sha256: hash(git(raw + spec.name + '.log')),
  deadline_ms: 3600000,
}));
const gates = old.gates.map(spec => ({...spec,
  run: rename(spec.run), predecessor: rename(spec.predecessor), command: spec.command.map(rename),
  ...(spec.stderr_log ? {stderr_log: rename(spec.stderr_log)} : {}),
  accepted_log: raw + spec.run + '.log',
  accepted_log_sha256: hash(git(raw + spec.run + '.log')),
  deadline_ms: 1800000,
}));
const anchors = ['r123-development-clippy-02', 'r123-development-focused-05', 'r123-development-r122-regression-01'].map(name => {
  const record = read(name + '.json');
  assert.strictEqual(record.returncode, 0);
  assert.strictEqual(record.source_map_sha256, sourceHash);
  return {name, command: record.command, predecessor: record.clock.predecessor.name,
    runner: record.runner.name, record_sha256: hash(bytes(name + '.json')),
    passed: name.includes('focused') ? 128 : name.includes('regression') ? 17 : null};
});
const plan = {
  accepted: false,
  scope: 'R123 CPU/runtime compatibility only; mutations, archive, native, formal and performance acceptance remain separate.',
  source_parent: head, source_map: 'r123-development-focused-05-source.json', source_map_sha256: sourceHash, source_count: 5703,
  runner: 'r123-development-run-v2.js', runner_sha256: hash(bytes('r123-development-run-v2.js')),
  runners: Object.fromEntries(['r123-development-run-v1.js', 'r123-development-run-v2.js'].map(name => [name, hash(bytes(name))])),
  parser_inputs: mutations.parser_inputs,
  accepted_commit: head,
  accepted_inputs: {[raw + 'r122-development-gates-v2.json']: hash(git(raw + 'r122-development-gates-v2.json'))},
  counts: {baseline_full: 2846, full: 2865, baseline_kfd: 1190, kfd: 1209, baseline_runtime: 743, runtime: 743, ignored: 5, harnesses: 48, csv_targets: 1},
  new_tests: {kfd: added, runtime: []}, anchors, full_runs: full, gates,
  doc_relocations: [
    {path: 'crates/fe2o3-kfd/src/queue_live.rs', from: 3821, to: 3823},
    {path: 'crates/fe2o3-kfd/src/queue_live.rs', from: 3876, to: 3878},
    {path: 'crates/fe2o3-kfd/src/queue_live.rs', from: 4360, to: 4362},
  ],
  auxiliary_counts: old.auxiliary_counts,
  normalization_contract: {...old.normalization_contract,
    linux_split: 'Actual baseline is committed R122. The separately pinned historical R119 split-row transcript is only a parser calibration fixture.'},
  acceptance: 'Require exact commands, declared runners/deadlines, unchanged source endpoints, valid clock/predecessor identities, normal child and process-group closure, exact accepted executable/test/ignored rosters plus only the declared nineteen KFD tests, and all summary fields. Permit only byte-identical doctest fence relocation and the existing narrow Linux/cache-wait transcript rules. This plan does not accept the packet or establish hardware, formal correspondence, aggregate memory or performance parity.',
};
console.log(JSON.stringify(plan, null, 2));

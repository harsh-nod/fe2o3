const fs = require('fs');
const path = require('path');
const crypto = require('crypto');
const assert = require('assert');
const cp = require('child_process');
const root = '/home/harsh/.codex-tmp';
const repo = path.join(root, 'fe2o3-r61-execution');
const dest = path.join(repo, 'docs/evidence/local-r96-auxiliary-shared-engine-2026-09-11');
const read = name => JSON.parse(fs.readFileSync(path.join(root, name), 'utf8'));
const hash = bytes => crypto.createHash('sha256').update(bytes).digest('hex');
const baseline = read('r96-frozen-source.json');
const planningParent = '10902ca32a853448f79b59cfcf22072e3cdd9325';
assert.strictEqual(cp.execFileSync('git', ['rev-parse', 'HEAD'], {cwd: repo}).toString().trim(), planningParent);
const source = read('r96-final-source-inputs.json');
assert.deepStrictEqual(source, baseline);
const paths = cp.execFileSync('git', ['ls-files', '--cached', '--others', '--exclude-standard', '-z'], {cwd: repo}).toString().split('\0').filter(p => p && !p.startsWith('docs/'));
const current = Object.fromEntries([...new Set(paths)].sort().map(p => [p, hash(fs.readFileSync(path.join(repo,p)))]));
assert.deepStrictEqual(current, baseline, 'final non-documentation source must match tested source');
const gates = read('r96-final-source-gate.json');
const auxiliary = read('r96-auxiliary-results.json');
assert.deepStrictEqual(read('r96-auxiliary-source-inputs.json'), baseline);
assert.deepStrictEqual(read('r96-auxiliary-source-after.json'), baseline);
assert.strictEqual(gates.length, 17);
assert.strictEqual(auxiliary.length, 12);
for (const row of [...gates, ...auxiliary]) assert.strictEqual(row.returncode, 0, row.name);
for (const name of ['r96-frozen', 'r96-format-final']) {
  const result = read(name+'.json');
  assert.strictEqual(result.returncode, 0);
  assert.strictEqual(result.source_unchanged, true);
  assert.deepStrictEqual(read(name+'-source.json'), baseline);
}
const mutations = read('r96-mutations.json');
assert.deepStrictEqual(mutations.map(m => m.name), [
  'v2-completion-owner', 'v2-closing-currentness', 'v2-installed-generation',
  'v2-original-sdma-roster',
]);
for (const m of mutations) {
  const name = 'r96-mut-'+m.name;
  const run = read(name+'.json');
  assert.strictEqual(run.returncode, 101, name);
  assert.strictEqual(run.source_unchanged, true);
  assert.strictEqual(run.cwd, repo);
  assert.deepStrictEqual(run.command, ['cargo', '+nightly-2026-04-03', 'test',
    '--locked', '--offline', '-p', 'fe2o3-kfd', '--all-features', '--lib',
    m.test, '--', '--exact']);
  const inputs = read(name+'-source.json');
  assert.deepStrictEqual(Object.keys(inputs).sort(), Object.keys(baseline).sort());
  assert.deepStrictEqual(Object.keys(baseline).filter(p => inputs[p] !== baseline[p]), [m.path]);
  let candidate = fs.readFileSync(path.join(repo,m.path), 'utf8');
  for (const [before,after] of m.edits) {
    assert.strictEqual(candidate.split(before).length, 2);
    candidate = candidate.replace(before,after);
  }
  assert.strictEqual(hash(candidate), inputs[m.path]);
  const log = fs.readFileSync(run.log,'utf8');
  for (const marker of ['Finished `test` profile', 'Running unittests', 'running 1 test', 'test '+m.test+' ... FAILED', m.expected, '0 passed; 1 failed']) assert(log.includes(marker), name+': '+marker);
}
const totals = logfile => {
  const text = fs.readFileSync(logfile,'utf8');
  const matches = [...text.matchAll(/test result: ok\. (\d+) passed; (\d+) failed; (\d+) ignored; (\d+) measured; (\d+) filtered out/g)];
  return matches.reduce((a,m) => ({harnesses:a.harnesses+1, passed:a.passed+Number(m[1]), failed:a.failed+Number(m[2]), ignored:a.ignored+Number(m[3])}), {harnesses:0,passed:0,failed:0,ignored:0});
};
const summary = {
  source_parent: 'da90a0038c6ec4c697faf0fbe93d597e9fa36e1a',
  planning_parent: planningParent,
  scope: 'NATIVE-2B.5A production-used phase composition; not full NATIVE-2B.5',
  source_identities: Object.keys(baseline).length,
  source_gates: gates.length,
  auxiliary_checks: auxiliary.length,
  compiled_negative_mutations: mutations.length,
  mutation_command_identity_checked: true,
  preliminary_runs: ['r96-focused', 'r96-clippy-preflight', 'r96-accepted',
    'r96-final-freeze', 'r96-format-preflight', 'r96-mut-completion-owner',
    'r96-mut-closing-currentness', 'r96-mut-installed-generation',
    'r96-mut-original-sdma-roster'],
  tests: Object.fromEntries([...gates,...auxiliary].filter(r => r.command.includes('test')).map(r => [r.name,totals(r.log)])),
  live_kfd: false,
  solver_rerun: false,
  performance_measurement: false,
};
assert.strictEqual(summary.tests['gnu-tests'].failed, 0);
assert.deepStrictEqual(summary.tests['gnu-tests'], summary.tests['musl-tests']);
const previous = JSON.parse(fs.readFileSync(path.join(repo,
  'docs/evidence/local-r95-auxiliary-custody-2026-09-11/test-summary.json'), 'utf8'));
const increments = {'gnu-tests': 3, 'musl-tests': 3, 'auxiliary-construction': 1, primary: 2};
for (const [name, counts] of Object.entries(summary.tests)) {
  assert(previous[name], 'known test suite: '+name);
  assert.deepStrictEqual(counts, {...previous[name], passed: previous[name].passed+(increments[name] || 0)}, name);
}
assert.deepStrictEqual(totals(path.join(root, 'r96-frozen.log')),
  {harnesses: 1, passed: 41, failed: 0, ignored: 0});
const newTests = [
  'queue::live::construction_primary::integration_tests::auxiliary_cases::same_engine_auxiliary_success_preserves_primary_and_exact_owner_partition',
  'queue::live::construction_primary::integration_tests::auxiliary_cases::same_engine_auxiliary_late_failures_retain_both_queues_without_installation',
  'queue::live::construction_auxiliary::tests::auxiliary_production_glue_uses_shared_phases_and_original_target_after_retake',
];
for (const name of ['r96-frozen', 'r96-final-gnu-tests', 'r96-final-musl-tests']) {
  const log = fs.readFileSync(path.join(root, name+'.log'), 'utf8');
  for (const test of newTests) assert(log.includes('test '+test+' ... ok'), name+': '+test);
}
fs.mkdirSync(dest);
fs.mkdirSync(path.join(dest,'raw'));
for (const name of fs.readdirSync(root).filter(p => /^r96-.*\.(json|log|py|js)$/.test(p))) {
  fs.copyFileSync(path.join(root,name),path.join(dest,'raw',name),fs.constants.COPYFILE_EXCL);
}
fs.writeFileSync(path.join(dest,'test-summary.json'),JSON.stringify(summary,null,2)+'\n',{flag:'wx'});
console.log(JSON.stringify(summary,null,2));

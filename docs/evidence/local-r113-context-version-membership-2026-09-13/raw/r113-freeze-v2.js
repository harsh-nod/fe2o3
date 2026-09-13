const fs = require('fs');
const path = require('path');
const cp = require('child_process');
const crypto = require('crypto');
const assert = require('assert');
const root = '/home/harsh/.codex-tmp';
const repo = path.join(root, 'fe2o3-r61-execution');
const execute = command => cp.execFileSync(command[0], command.slice(1), {cwd: repo, maxBuffer: 64 * 1024 * 1024}).toString();
const hash = bytes => crypto.createHash('sha256').update(bytes).digest('hex');
const parent = 'a2feef229758381b66f962ad8be2b87843eb33a3';
const accepted = parent;
assert.strictEqual(execute(['git', 'rev-parse', 'HEAD']).trim(), parent);
const inventory = () => [...new Set(execute(['git', 'ls-files', '--cached', '--others', '--exclude-standard', '-z'])
  .split('\0').filter(p => p && !p.startsWith('docs/')))].sort();
const paths = inventory();
assert.strictEqual(paths.length, 5680);
const frozen = Object.fromEntries(paths.map(p => [p, hash(fs.readFileSync(path.join(repo, p)))]));
const commands = [
  ['cargo', '+nightly-2026-04-03', '--version', '--verbose'],
  ['rustc', '+nightly-2026-04-03', '--version', '--verbose'],
  ['python3', '-VV'], ['node', '--version'], ['git', '--version'], ['uname', '-srmo'],
];
const environment = {
  publication_parent: parent, accepted_runtime_checkpoint: accepted,
  recorded_phase: 'Before frozen/full/auxiliary/mutation gates; preliminary failures preserved. No native or load qualification.',
  records: commands.map(command => ({command, output: execute(command)})),
  binaries: ['cargo', 'rustc'].map(name => {
    const binary = execute(['rustup', 'which', '--toolchain', 'nightly-2026-04-03', name]).trim();
    return {name, path: binary, sha256: hash(fs.readFileSync(binary))};
  }),
  runners: ['r113-run.js', 'r113-run-v2.js', 'r113-source-gate.py', 'r113-auxiliary-gates.py',
    'r113-freeze.js', 'r113-freeze-v2.js', 'r113-runner-tests.js', 'r113-freeze-tests.js']
    .map(name => ({name, sha256: hash(fs.readFileSync(path.join(root, name)))})),
  environment_overrides: {CARGO_BUILD_JOBS: '4', CARGO_INCREMENTAL: '0', RUST_TEST_THREADS: '4', PYTHONDONTWRITEBYTECODE: '1'},
  environment_removed: ['XDG_RUNTIME_DIR'], test_deadlines_changed: false, hardware_qualification: false, solver_rerun: false,
  campaign_envelope: {previous_deadline_ms: 7200000, deadline_ms: 7200000, individual_gate_deadline_ms: 1800000, reason: 'Retain R112\'s serialized campaign envelope; individual gate and test deadlines are unchanged.'},
  ambient_stack: {RUST_MIN_STACK: process.env.RUST_MIN_STACK ?? null,
    command: ['bash', '-c', 'ulimit -s'], output: execute(['bash', '-c', 'ulimit -s'])},
  isolated_artifacts: fs.readdirSync(root).filter(name => /^v2-membership-.*\.(json|log|js)$/.test(name)).sort()
    .map(name => ({name, sha256: hash(fs.readFileSync(path.join(root, name)))})),
  prior_artifacts: fs.readdirSync(root).filter(name => /^r113-.*\.(json|log)$/.test(name)).sort()
    .map(name => ({name, sha256: hash(fs.readFileSync(path.join(root, name)))})),
};
const previous = JSON.parse(fs.readFileSync(path.join(repo,
  'docs/evidence/local-r112-uninitialized-device-insertion-2026-09-13/raw/r112-environment.json')));
assert.deepStrictEqual(environment.binaries, previous.binaries);
const command = ['gh', 'issue', 'view', '182', '--repo', 'harsh-nod/fe2o3', '--json', 'number,title,state,updatedAt,url'];
const issue = {observed_at: new Date().toISOString(), command, issue: JSON.parse(execute(command))};
const previousSource = JSON.parse(fs.readFileSync(path.join(repo,
  'docs/evidence/local-r112-uninitialized-device-insertion-2026-09-13/raw/r112-frozen-source.json')));
environment.source_delta = paths.filter(name => frozen[name] !== previousSource[name]);
assert.deepStrictEqual(environment.source_delta, [
  'crates/fe2o3-runtime-model/src/context_version_journal.rs',
  'crates/fe2o3-runtime-model/src/context_version_journal/membership_tests.rs',
  'crates/fe2o3-runtime-model/src/context_version_journal/tests.rs',
]);
assert.deepStrictEqual(Object.keys(previousSource).filter(name => !Object.hasOwn(frozen, name)), []);
const refreshedPaths = inventory();
assert.deepStrictEqual(refreshedPaths, paths, 'complete source inventory remains unchanged');
assert.deepStrictEqual(Object.fromEntries(refreshedPaths.map(p => [p, hash(fs.readFileSync(path.join(repo, p)))])), frozen);
assert.strictEqual(execute(['git', 'rev-parse', 'HEAD']).trim(), parent);
environment.observation = {utc_ms: Date.now(), monotonic_ns: process.hrtime.bigint().toString(),
  boot_id: fs.readFileSync('/proc/sys/kernel/random/boot_id', 'utf8').trim(),
  clock_source: 'node-process-hrtime-linux-monotonic'};
environment.recorded_at = new Date(environment.observation.utc_ms).toISOString();
for (const [name, value] of [['frozen-source', frozen], ['environment', environment], ['issue-status', issue]]) {
  fs.writeFileSync(path.join(root, 'r113-' + name + '.json'), JSON.stringify(value, null, 2) + '\n', {flag: 'wx'});
}
console.log('Frozen ' + paths.length + ' source identities; tools, runners, preliminary bytes and issue status retained');


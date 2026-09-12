const fs = require('fs');
const path = require('path');
const cp = require('child_process');
const crypto = require('crypto');
const assert = require('assert');
const root = '/home/harsh/.codex-tmp';
const repo = path.join(root, 'fe2o3-r61-execution');
const execute = command => cp.execFileSync(command[0], command.slice(1), {cwd: repo}).toString();
const hash = bytes => crypto.createHash('sha256').update(bytes).digest('hex');
const parent = '5ba89a0e642cf02effc8e462e33be93b19aec1c2';
const acceptedRuntimeCheckpoint = 'ad6f71304ddb0f41cc102f9e6ecabd7ed7840efe';
assert.strictEqual(execute(['git', 'rev-parse', 'HEAD']).trim(), parent);
assert.strictEqual(execute(['git', 'rev-parse', parent + '^']).trim(), acceptedRuntimeCheckpoint);
assert.deepStrictEqual(execute(['git', 'diff', '--name-only', acceptedRuntimeCheckpoint, parent])
  .trim().split('\n'), ['docs/runtime-a1-a2-swarm-current.md']);
const names = [...new Set(execute(['git', 'ls-files', '--cached', '--others', '--exclude-standard', '-z'])
  .split('\0').filter(p => p && !p.startsWith('docs/')))].sort();
const frozen = Object.fromEntries(names.map(p => [p, hash(fs.readFileSync(path.join(repo, p)))]));
const commands = [
  ['cargo', '+nightly-2026-04-03', '--version', '--verbose'],
  ['rustc', '+nightly-2026-04-03', '--version', '--verbose'],
  ['python3', '-VV'], ['node', '--version'], ['git', '--version'], ['uname', '-srmo'],
];
const environment = {
  recorded_at: new Date().toISOString(),
  recorded_phase: 'Before frozen/full/auxiliary/mutation campaigns; after preliminary focused tests. No host-load qualification.',
  publication_parent: parent, accepted_runtime_checkpoint: acceptedRuntimeCheckpoint,
  records: commands.map(command => ({command, output: execute(command)})),
  binaries: ['cargo', 'rustc'].map(name => {
    const binary = execute(['rustup', 'which', '--toolchain', 'nightly-2026-04-03', name]).trim();
    return {name, path: binary, sha256: hash(fs.readFileSync(binary))};
  }),
  runners: ['r107-run.js', 'r107-source-gate.py', 'r107-auxiliary-gates.py', 'r107-freeze.js']
    .map(name => ({name, sha256: hash(fs.readFileSync(path.join(root, name)))})),
  environment_overrides: {CARGO_BUILD_JOBS: '4', CARGO_INCREMENTAL: '0', RUST_TEST_THREADS: '4', PYTHONDONTWRITEBYTECODE: '1'},
  environment_removed: ['XDG_RUNTIME_DIR'],
  test_deadlines_changed: false, hardware_qualification: false, solver_rerun: false,
};
const previous = JSON.parse(fs.readFileSync(path.join(repo,
  'docs/evidence/local-r106-coherent-initialization-custody-2026-09-12/raw/r106-environment.json')));
assert.deepStrictEqual(environment.binaries, previous.binaries);
const command = ['gh', 'issue', 'view', '182', '--repo', 'harsh-nod/fe2o3', '--json', 'number,title,state,updatedAt,url'];
const issue = {observed_at: new Date().toISOString(), command, issue: JSON.parse(execute(command))};
assert.strictEqual(issue.issue.number, 182);
for (const [name, value] of [['frozen-source', frozen], ['environment', environment], ['issue-status', issue]]) {
  fs.writeFileSync(path.join(root, 'r107-' + name + '.json'), JSON.stringify(value, null, 2) + '\n', {flag: 'wx'});
}
console.log('Frozen ' + names.length + ' source identities and recorded tools/runners/issue status');

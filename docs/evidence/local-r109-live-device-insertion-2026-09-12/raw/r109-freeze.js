const fs = require('fs');
const path = require('path');
const cp = require('child_process');
const crypto = require('crypto');
const assert = require('assert');
const root = '/home/harsh/.codex-tmp';
const repo = path.join(root, 'fe2o3-r61-execution');
const execute = command => cp.execFileSync(command[0], command.slice(1), {cwd: repo}).toString();
const hash = bytes => crypto.createHash('sha256').update(bytes).digest('hex');
const parent = '17d0be5476a4f71c473a619903c62aa316a96fc7';
const accepted = '9171bd68d920681e524d8eda5a29053a8fd8ae88';
assert.strictEqual(execute(['git', 'rev-parse', 'HEAD']).trim(), parent);
assert.strictEqual(execute(['git', 'rev-parse', parent + '^']).trim(), accepted);
assert.deepStrictEqual(execute(['git', 'diff', '--name-only', accepted, parent]).trim().split('\n'),
  ['docs/runtime-a1-a2-swarm-current.md', 'docs/runtime-swarm-next-packets.md']);
const paths = [...new Set(execute(['git', 'ls-files', '--cached', '--others', '--exclude-standard', '-z'])
  .split('\0').filter(p => p && !p.startsWith('docs/')))].sort();
assert.strictEqual(paths.length, 5673);
const frozen = Object.fromEntries(paths.map(p => [p, hash(fs.readFileSync(path.join(repo, p)))]));
const commands = [
  ['cargo', '+nightly-2026-04-03', '--version', '--verbose'],
  ['rustc', '+nightly-2026-04-03', '--version', '--verbose'],
  ['python3', '-VV'], ['node', '--version'], ['git', '--version'], ['uname', '-srmo'],
];
const environment = {
  recorded_at: new Date().toISOString(), publication_parent: parent, accepted_runtime_checkpoint: accepted,
  recorded_phase: 'Before frozen/full/auxiliary/mutation gates; preliminary failures preserved. No native or load qualification.',
  records: commands.map(command => ({command, output: execute(command)})),
  binaries: ['cargo', 'rustc'].map(name => {
    const binary = execute(['rustup', 'which', '--toolchain', 'nightly-2026-04-03', name]).trim();
    return {name, path: binary, sha256: hash(fs.readFileSync(binary))};
  }),
  runners: ['r109-run.js', 'r109-source-gate.py', 'r109-auxiliary-gates.py', 'r109-freeze.js']
    .map(name => ({name, sha256: hash(fs.readFileSync(path.join(root, name)))})),
  environment_overrides: {CARGO_BUILD_JOBS: '4', CARGO_INCREMENTAL: '0', RUST_TEST_THREADS: '4', PYTHONDONTWRITEBYTECODE: '1'},
  environment_removed: ['XDG_RUNTIME_DIR'], test_deadlines_changed: false, hardware_qualification: false, solver_rerun: false,
  ambient_stack: {RUST_MIN_STACK: process.env.RUST_MIN_STACK ?? null,
    command: ['bash', '-c', 'ulimit -s'], output: execute(['bash', '-c', 'ulimit -s'])},
  prior_artifacts: fs.readdirSync(root).filter(name => /^r109-.*\.(json|log)$/.test(name)).sort()
    .map(name => ({name, sha256: hash(fs.readFileSync(path.join(root, name)))})),
};
const previous = JSON.parse(fs.readFileSync(path.join(repo,
  'docs/evidence/local-r108-context-writer-issuance-2026-09-12/raw/r108-environment.json')));
assert.deepStrictEqual(environment.binaries, previous.binaries);
const command = ['gh', 'issue', 'view', '182', '--repo', 'harsh-nod/fe2o3', '--json', 'number,title,state,updatedAt,url'];
const issue = {observed_at: new Date().toISOString(), command, issue: JSON.parse(execute(command))};
for (const [name, value] of [['frozen-source', frozen], ['environment', environment], ['issue-status', issue]]) {
  fs.writeFileSync(path.join(root, 'r109-' + name + '.json'), JSON.stringify(value, null, 2) + '\n', {flag: 'wx'});
}
console.log('Frozen ' + paths.length + ' source identities; tools, runners, preliminary bytes and issue status retained');

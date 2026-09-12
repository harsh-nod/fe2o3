const fs = require('fs');
const path = require('path');
const cp = require('child_process');
const crypto = require('crypto');
const assert = require('assert');
const root = '/home/harsh/.codex-tmp';
const repo = path.join(root, 'fe2o3-r61-execution');
const execute = command => cp.execFileSync(command[0], command.slice(1), {cwd: repo}).toString();
const hash = bytes => crypto.createHash('sha256').update(bytes).digest('hex');
const parent = '5cdeedd8290fac0bf01ca53b01cc12e828a2e20e';
const tested = '0bb5d1d35c89336f40e2ee311240e66f659fc323';
const accepted = '61a1479348ec3b744a8881108e059f540f324364';
assert.strictEqual(execute(['git', 'rev-parse', 'HEAD']).trim(), parent);
const planningChanges = execute(['git', 'diff', '--name-only', accepted, parent])
  .trim().split('\n').filter(Boolean);
assert(planningChanges.length && planningChanges.every(p => p.startsWith('docs/')));
const commands = [
  ['cargo', '+nightly-2026-04-03', '--version', '--verbose'],
  ['rustc', '+nightly-2026-04-03', '--version', '--verbose'],
  ['python3', '-VV'], ['node', '--version'], ['git', '--version'], ['uname', '-srmo'],
];
const environment = {
  recorded_at: new Date().toISOString(),
  recorded_phase: 'After full/auxiliary campaigns; before behavioral mutations. Not a retrospective host-load observation.',
  publication_parent: parent, tested_source_checkpoint: tested, accepted_runtime_checkpoint: accepted,
  documentation_only_parent_changes: planningChanges,
  records: commands.map(command => ({command, output: execute(command)})),
  binaries: ['cargo', 'rustc'].map(name => {
    const binary = execute(['rustup', 'which', '--toolchain', 'nightly-2026-04-03', name]).trim();
    return {name, path: binary, sha256: hash(fs.readFileSync(binary))};
  }),
  environment_overrides: {CARGO_BUILD_JOBS: '4', CARGO_INCREMENTAL: '0', RUST_TEST_THREADS: '4', PYTHONDONTWRITEBYTECODE: '1'},
  environment_removed: ['XDG_RUNTIME_DIR'],
  test_deadlines_changed: false, hardware_qualification: false, solver_rerun: false,
};
fs.writeFileSync(path.join(root, 'r104-environment.json'), JSON.stringify(environment, null, 2) + '\n', {flag: 'wx'});
const command = ['gh', 'issue', 'view', '182', '--repo', 'harsh-nod/fe2o3', '--json', 'number,title,state,updatedAt,url'];
const issue = {observed_at: new Date().toISOString(), command, issue: JSON.parse(execute(command))};
fs.writeFileSync(path.join(root, 'r104-issue-status.json'), JSON.stringify(issue, null, 2) + '\n', {flag: 'wx'});
console.log('Recorded environment/tool identities and current issue status; no hardware or solver run');

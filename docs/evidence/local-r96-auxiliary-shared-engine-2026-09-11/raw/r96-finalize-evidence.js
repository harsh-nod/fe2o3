const fs = require('node:fs');
const path = require('node:path');
const crypto = require('node:crypto');
const assert = require('node:assert/strict');
const cp = require('node:child_process');
const root = '/home/harsh/.codex-tmp/fe2o3-r61-execution';
const dest = path.join(root, 'docs/evidence/local-r96-auxiliary-shared-engine-2026-09-11');
const hash = bytes => crypto.createHash('sha256').update(bytes).digest('hex');
const baseline = JSON.parse(fs.readFileSync(path.join(dest, 'raw/r96-frozen-source.json')));
const currentPaths = cp.execFileSync('git', ['ls-files', '--cached', '--others',
  '--exclude-standard', '-z'], {cwd: root}).toString().split('\0')
  .filter(name => name && !name.startsWith('docs/'));
assert.deepEqual([...new Set(currentPaths)].sort(), Object.keys(baseline).sort());
const sourceLines = [];
for (const name of Object.keys(baseline).sort()) {
  const digest = baseline[name];
  assert.equal(hash(fs.readFileSync(path.join(root, name))), digest, name);
  sourceLines.push(digest+'  '+name);
}
fs.writeFileSync(path.join(dest, 'source-files.sha256'), sourceLines.join('\n')+'\n', {flag: 'wx'});
function files(directory, prefix = '') {
  return fs.readdirSync(directory, {withFileTypes: true}).flatMap(entry => {
    const name = path.join(prefix, entry.name);
    return entry.isDirectory() ? files(path.join(directory, entry.name), name) : [name];
  }).sort();
}
assert(fs.existsSync(path.join(dest, 'README.md')));
const retained = files(dest).filter(name => name !== 'retained-files.sha256');
fs.writeFileSync(path.join(dest, 'retained-files.sha256'), retained.map(name =>
  hash(fs.readFileSync(path.join(dest, name)))+'  '+name).join('\n')+'\n', {flag: 'wx'});
console.log(JSON.stringify({source_identities: sourceLines.length, retained_files: retained.length}));

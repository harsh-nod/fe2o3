const fs = require('fs');
const path = require('path');
const crypto = require('crypto');
const cp = require('child_process');
const assert = require('assert');
const repo = '/home/harsh/.codex-tmp/fe2o3-r61-execution';
const evidence = 'docs/evidence/local-r101-auxiliary-recovery-currentness-2026-09-11';
const root = path.join(repo, evidence);
const mode = process.argv[2];
assert(['manifest', 'staged'].includes(mode));
const git = args => cp.execFileSync('git', args, {cwd: repo, maxBuffer: 16 * 1024 * 1024});
assert.strictEqual(git(['rev-parse', 'HEAD']).toString().trim(),
  '4424f4607d8a64677556b32713a74b1ca5c6557a');
const hash = bytes => crypto.createHash('sha256').update(bytes).digest('hex');
const baseline = JSON.parse(fs.readFileSync(path.join(root, 'raw/r101-frozen-source.json')));
const files = [...new Set(git(['ls-files', '--cached', '--others', '--exclude-standard', '-z'])
  .toString().split('\0').filter(p => p && !p.startsWith('docs/')))].sort();
const current = Object.fromEntries(files.map(p => [p, hash(fs.readFileSync(path.join(repo, p)))]));
assert.deepStrictEqual(current, baseline);
const docs = ['docs/runtime-a1-a2-swarm-current.md', 'docs/runtime-a1-a2-next-wave.md',
  'docs/runtime-a1-a2-swarm-dispatch-r83.md', 'docs/runtime-auxiliary-queue-construction-custody-v1.md',
  evidence + '/README.md'];
let links = 0;
for (const doc of docs) {
  const content = fs.readFileSync(path.join(repo, doc), 'utf8');
  for (const match of content.matchAll(/\]\(([^)]+)\)/g)) {
    const target = match[1].split('#')[0];
    if (!target || /^[a-z]+:/i.test(target)) continue;
    assert(fs.existsSync(path.resolve(repo, path.dirname(doc), target)), doc + ': ' + target);
    links++;
  }
}
function walk(dir) {
  return fs.readdirSync(path.join(root, dir), {withFileTypes: true}).flatMap(entry => {
    const file = path.join(dir, entry.name);
    assert(!entry.isSymbolicLink(), file);
    return entry.isDirectory() ? walk(file) : [file];
  });
}
const retained = walk('').filter(p => p !== 'retained-files.sha256').sort();
const manifest = retained.map(p => hash(fs.readFileSync(path.join(root, p))) + '  ' + p + '\n').join('');
const manifestPath = path.join(root, 'retained-files.sha256');
if (mode === 'manifest') fs.writeFileSync(manifestPath, manifest, {flag: 'wx'});
assert.strictEqual(fs.readFileSync(manifestPath, 'utf8'), manifest);
for (const file of retained.filter(p => p.startsWith('raw/'))) {
  assert.deepStrictEqual(fs.readFileSync(path.join(root, file)),
    fs.readFileSync(path.join('/home/harsh/.codex-tmp', path.basename(file))), file);
}
if (mode === 'staged') {
  assert.strictEqual(git(['diff', '--name-only']).toString(), '');
  assert.strictEqual(git(['ls-files', '--others', '--exclude-standard']).toString(), '');
  const changed = git(['diff', '--cached', '--name-only', '-z']).toString().split('\0').filter(Boolean);
  const summary = JSON.parse(fs.readFileSync(path.join(root, 'test-summary.json')));
  const expected = [...new Set([...summary.changed_test_files, ...docs,
    ...retained.map(p => path.join(evidence, p)), path.join(evidence, 'retained-files.sha256')])].sort();
  assert.deepStrictEqual(changed.slice().sort(), expected);
  for (const file of changed) assert.deepStrictEqual(git(['show', ':' + file]),
    fs.readFileSync(path.join(repo, file)), file);
  console.log('PASS: ' + changed.length + ' staged files match working bytes');
}
console.log('PASS: ' + files.length + ' frozen source hashes, ' + links +
  ' local documentation paths and ' + retained.length + ' retained file hashes');

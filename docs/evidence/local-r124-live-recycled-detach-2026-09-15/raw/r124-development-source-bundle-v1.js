// Generate an immutable source snapshot; this does not execute or qualify runtime behavior.
const fs = require('fs'), path = require('path'), assert = require('assert'), crypto = require('crypto');
const root = '/home/harsh/.codex-tmp/', helper = root + 'r124-development-check-v1.js';
const helperHash = '1caef52ee3c10a2d8efd49d82debadfbe227822fb575fc88ea32f99cd0bb82ad';
assert.strictEqual(crypto.createHash('sha256').update(fs.readFileSync(helper)).digest('hex'), helperHash);
const C = require(helper);
C.pin(helper, helperHash); C.bytes(__filename);
assert.deepStrictEqual(C.identities(), C.map);
const baselinePath = 'docs/evidence/local-r123-live-retained-control-release-2026-09-15/raw/r123-development-focused-05-source.json';
const baseline = JSON.parse(C.git(['show', C.p.source_parent + ':' + baselinePath]));
assert.strictEqual(C.hash(JSON.stringify(baseline)), '8380746325d6717df78a608eb5be8f3ff3c48e42f32eec19aba2ab0677fdcb08');
assert.strictEqual(Object.keys(baseline).length, 5703);
assert(Object.keys(baseline).every(file => Object.hasOwn(C.map, file)), 'no removed source identities');
const files = Object.keys(C.map).filter(file => C.map[file] !== baseline[file]).map(file => {
  assert(file.startsWith('crates/') && file.endsWith('.rs'));
  const text = C.bytes(path.join(C.repo, file)).toString();
  assert.strictEqual(C.hash(text), C.map[file]);
  const previous = baseline[file] || null;
  if (previous) assert.strictEqual(C.hash(C.git(['show', C.p.source_parent + ':' + file])), previous);
  return {path: file, previous_sha256: previous, sha256: C.map[file], text};
});
assert.strictEqual(files.length, 12);
assert.strictEqual(files.filter(file => file.previous_sha256 === null).length, 3);
const diff = [...new Set([
  ...C.git(['diff', '--name-only', '-z', 'HEAD']).split('\0'),
  ...C.git(['ls-files', '--others', '--exclude-standard', '-z']).split('\0'),
])].filter(file => file && !file.startsWith('docs/')).sort();
assert.deepStrictEqual(diff, files.map(file => file.path));
const bundle = {development_only: true, source_parent: C.p.source_parent,
  source_map_sha256: C.p.source_map_sha256, source_identities: C.p.source_count,
  accepted_baseline: {commit: C.p.source_parent, path: baselinePath,
    source_map_sha256: C.hash(JSON.stringify(baseline))}, files};
assert.deepStrictEqual(C.identities(), C.map); C.stable();
const output = root + 'r124-development-source-bundle-v1.json', encoded = JSON.stringify(bundle, null, 2) + '\n';
fs.writeFileSync(output, encoded, {flag: 'wx'});
assert.strictEqual(fs.readFileSync(output, 'utf8'), encoded);
assert.deepStrictEqual(C.identities(), C.map); C.stable();
console.log(JSON.stringify({source_snapshot_only: true, output, files: files.length,
  new_files: 3, sha256: C.hash(encoded), source_map_sha256: C.p.source_map_sha256}));

const fs = require('fs');
const path = require('path');
const assert = require('assert');
const p = require('./r115-qualification-plan.js');
const e = require('./r115-qualification-evidence.js');
const map = e.identities();
const original = p.productionPaths.map(source => ({path: source,
  text: fs.readFileSync(path.join(e.repo, source), 'utf8')}));
assert.strictEqual(Object.keys(map).length, p.sourceCount);
assert.strictEqual(e.hash(JSON.stringify(map)), 'cb4cbb046fa2394f900723ce8bbf6a37a26624fd6114026c9f29fbe576a9eeab');
const old = JSON.parse(e.committed('raw/r114-frozen-source.json'));
assert.deepStrictEqual([...new Set([...Object.keys(old), ...Object.keys(map)])].filter(n => old[n] !== map[n]).sort(), p.sourceDelta);
assert.deepStrictEqual(Object.keys(map).filter(n => !Object.hasOwn(old, n)).sort(), p.addedSource);
assert.strictEqual(p.newTests.length, 15);
const testSource = fs.readFileSync(path.join(e.repo, p.addedSource[1]), 'utf8');
assert.deepStrictEqual([...testSource.matchAll(/^fn (returning_\w+)\(\)/gm)].map(m => 'queue::dispatch_binding::control_release::tests::' + m[1]).sort(), p.newTests);
assert.strictEqual(Object.keys(e.expectedEntries(map, original)).length, 37);
for (const mutation of p.mutations) {
  e.mutationSource(map, original, mutation);
  assert(p.allowedMutationTests.includes(mutation.test));
  assert.notStrictEqual(mutation.test, p.sourceGuard);
  const line = fs.readFileSync(path.join(e.repo, mutation.oracle_path), 'utf8').split('\n')[mutation.oracle_line - 1];
  assert(/assert!|assert_eq!|\.expect\(/.test(line), mutation.name + ': expected lexical oracle');
}
for (const helper of new Set([...p.helpers, ...p.freezeHelpers])) e.bytes(helper);
const {docNames} = require('./r115-qualification-collect.js');
for (const target of ['gnu', 'musl']) {
  const old = e.passing(e.committed('raw/r114-final-' + target + '-docs.log'));
  assert.strictEqual(old.filter(n => !docNames(old).includes(n)).length, p.docRelocations.length);
}
console.log('PASS source inventory, test roster, 16 unique mutation maps, oracle lines, helper presence and six unchanged doctest relocations');

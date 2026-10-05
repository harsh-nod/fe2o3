// SPDX-License-Identifier: GPL-3.0-or-later
// Synthetic byte transformations only; no production source or native authority.
import test from 'node:test';
import assert from 'node:assert/strict';
import crypto from 'node:crypto';
import {SourceAccount} from './source-account.mjs';
import {verifyTransformPair} from './offset-transforms.mjs';

const pin = b => ({bytes: b.length, sha256: crypto.createHash('sha256').update(b).digest('hex')});
function fixture() {
  return {before: Buffer.from('abc\nfoo\nz\n'), after: Buffer.from('abc\nLONG\nz\n'),
    changes: [{beforeOffset: 4, afterOffset: 4, before: Buffer.from('foo'), after: Buffer.from('LONG')}]};
}
function verify(a, f) { return verifyTransformPair(a, f.before, f.after, pin(f.before), pin(f.after), f.changes); }
test('exact forward inverse equality and paired accounting', () => {
  const f = fixture(), a = new SourceAccount(), r = verify(a, f);
  assert.deepEqual(r, {beforeBytes: 10, afterBytes: 11, changes: 1, expansionBytes: 21, logicalWork: 448});
  assert.equal(a.usage().expansionBytes, 21);
  assert.equal(a.usage().patternBytes, 7);
  assert.equal(a.usage().hunks, 1);
  assert.equal(a.usage().work, 448);
});
test('UTF8 offsets are bytes and preserve multibyte surrounding text', () => {
  const f = {before: Buffer.from('πx\n'), after: Buffer.from('π🚀\n'),
    changes: [{beforeOffset: 2, afterOffset: 2, before: Buffer.from('x'), after: Buffer.from('🚀')}]};
  assert.equal(verify(new SourceAccount(), f).expansionBytes, 11);
  f.changes[0].beforeOffset = 1;
  assert.throws(() => verify(new SourceAccount(), f));
});
test('insertion deletion and unchanged runs share derived inverse offsets', () => {
  const f = {before: Buffer.from('abcdefghij'), after: Buffer.from('aXYbcdehij'), changes: [
    {beforeOffset: 1, afterOffset: 1, before: Buffer.alloc(0), after: Buffer.from('XY')},
    {beforeOffset: 5, afterOffset: 7, before: Buffer.from('fg'), after: Buffer.alloc(0)},
  ]};
  assert.equal(verify(new SourceAccount(), f).changes, 2);
  f.changes[1].afterOffset = 6;
  assert.throws(() => verify(new SourceAccount(), f), /inverse offset/);
});
test('explicit offset preserves the other identical textual spelling', () => {
  const f = {before: Buffer.from('aa-aa'), after: Buffer.from('aa-BB'), changes: [
    {beforeOffset: 3, afterOffset: 3, before: Buffer.from('aa'), after: Buffer.from('BB')},
  ]};
  verify(new SourceAccount(), f);
  f.changes[0].beforeOffset = f.changes[0].afterOffset = 0;
  assert.throws(() => verify(new SourceAccount(), f));
});
test('overlap reorder duplicate and out-of-source ranges refuse', () => {
  const f = fixture();
  for (const changes of [[f.changes[0], f.changes[0]], [{...f.changes[0], beforeOffset: 99, afterOffset: 99}],
    [{...f.changes[0], beforeOffset: Number.MAX_SAFE_INTEGER, afterOffset: Number.MAX_SAFE_INTEGER}],
    [{...f.changes[0], beforeOffset: 0.5}]]) {
    assert.throws(() => verify(new SourceAccount(), {...f, changes}));
  }
});
test('wrong fragments are rejected before constructing outputs', () => {
  for (const side of ['before', 'after']) {
    const f = fixture();
    f.changes[0][side] = Buffer.alloc(f.changes[0][side].length, 120);
    assert.throws(() => verify(new SourceAccount(), f), /fragment/);
  }
});
test('equal extents and locally correct fragments cannot hide a changed tail', () => {
  const f = fixture();
  f.after = Buffer.from('abc\nLONG\nY\n');
  assert.throws(() => verify(new SourceAccount(), f), /full forward equality/);
});
test('exact and one-short work credit with sticky refusal', () => {
  verify(new SourceAccount({work: 448}), fixture());
  const a = new SourceAccount({work: 447});
  assert.throws(() => verify(a, fixture()), /work/);
  assert.equal(a.usage().expansionBytes, 0);
  assert.equal(a.usage().patternBytes, 0);
  assert.equal(a.usage().work, 0);
  assert.equal(a.usage().hunks, 1);
  assert.throws(() => verify(a, fixture()));
});
test('exact and one-short output and decoded-pattern caps', () => {
  verify(new SourceAccount({expansionBytes: 21, patternBytes: 7}), fixture());
  for (const limits of [{expansionBytes: 20}, {patternBytes: 6}]) {
    const a = new SourceAccount(limits);
    assert.throws(() => verify(a, fixture()));
    assert.equal(a.usage().expansionBytes, 0);
    assert.equal(a.usage().work, 0);
  }
});
test('hunk debit precedes descriptor access', () => {
  const f = fixture(), a = new SourceAccount({hunks: 0});
  const trap = {get before() { throw new Error('descriptor accessed before refusal'); }};
  assert.throws(() => verify(a, {...f, changes: [trap]}), /hunks/);
  assert.equal(a.usage().hunks, 0);
});
test('multiple files share cumulative expansion work and failure', () => {
  const a = new SourceAccount({expansionBytes: 42, work: 896});
  verify(a, fixture());
  verify(a, fixture());
  assert.equal(a.usage().expansionBytes, 42);
  assert.equal(a.usage().work, 896);
  assert.throws(() => verify(a, fixture()));
  assert.equal(a.usage().expansionBytes, 42);
  assert.equal(a.usage().failed, true);
});
test('exact input pins and nonempty closed change descriptors are required', () => {
  const f = fixture();
  assert.throws(() => verifyTransformPair(new SourceAccount(), f.before, f.after,
    {...pin(f.before), sha256: '0'.repeat(64)}, pin(f.after), f.changes), /digest/);
  for (const changes of [[], [{...f.changes[0], extra: true}],
    [{beforeOffset: 0, afterOffset: 0, before: Buffer.alloc(0), after: Buffer.alloc(0)}]]) {
    assert.throws(() => verify(new SourceAccount(), {...f, changes}));
  }
});

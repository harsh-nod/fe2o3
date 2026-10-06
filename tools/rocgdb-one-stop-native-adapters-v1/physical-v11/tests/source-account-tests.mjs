// SPDX-License-Identifier: GPL-3.0-or-later
// Synthetic account controls, not source or native admission evidence.
import test from 'node:test';
import assert from 'node:assert/strict';
import {SourceAccount, CAPS, add, multiply} from './source-account.mjs';

test('v5 exact caps and cumulative envelope arithmetic', () => {
  assert.equal(CAPS.selectedBytes, 2240 * 1024);
  assert.equal(CAPS.perMetadata, 64 * 1024);
  assert.equal(CAPS.roles, CAPS.selectedRows + CAPS.metadataRoles + CAPS.auxiliaryRoles + CAPS.apiRoles);
  const reads = CAPS.selectedBytes + CAPS.metadataBytes + CAPS.auxiliaryBytes + CAPS.apiBytes + CAPS.roles;
  assert.equal(reads, 5096568);
  assert(reads <= CAPS.requestedBytes);
  assert.equal(CAPS.expansionBytes, CAPS.selectedBytes + 2144 * 1024);
  const work = 3 * (CAPS.selectedBytes + CAPS.auxiliaryBytes + CAPS.apiBytes) +
    12 * CAPS.metadataBytes + 2 * CAPS.selectedBytes +
    8 * (2 * CAPS.expansionBytes + 2 * CAPS.patternBytes) + CAPS.reportBytes;
  assert.equal(work, 97348584);
  assert(work <= CAPS.work);
});
test('limits cannot be widened or made unsafe and zero lower limits work', () => {
  for (const lower of [{work: CAPS.work + 1}, {extra: 1}, {work: -1}, {work: 1.5}, {work: '1'}, {work: Infinity}])
    assert.throws(() => new SourceAccount(lower));
  const a = new SourceAccount({work: 0});
  a.charge({work: 0});
  assert.throws(() => a.charge({work: 1}));
});
test('multi-field refusal is atomic and the first error is sticky', () => {
  const a = new SourceAccount({work: 5, expansionBytes: 10});
  a.charge({work: 3});
  let first;
  try { a.charge({work: 3, expansionBytes: 1}); } catch (error) { first = error; }
  assert(first instanceof Error);
  assert.equal(a.usage().work, 3);
  assert.equal(a.usage().expansionBytes, 0);
  let ran = false;
  assert.throws(() => a.run(() => { ran = true; }), error => error === first);
  assert.equal(ran, false);
  assert.throws(() => a.charge({work: 0}), error => error === first);
});
test('all read categories share one account and metadata sublimits', () => {
  const a = new SourceAccount();
  for (const kind of ['selected', 'manifest', 'delta', 'metadata', 'auxiliary', 'api'])
    a.admitRead({id: kind, kind}, 2);
  const u = a.usage();
  assert.equal(u.roles, 6);
  assert.equal(u.requestedBytes, 18);
  assert.equal(u.readCalls, 12);
  assert.equal(u.metadataBytes, 6);
  assert.equal(u.metadataRoles, 3);
  assert.equal(u.manifestBytes, 2);
  assert.equal(u.deltaBytes, 2);
  assert.equal(u.auxiliaryMetadataBytes, 2);
  assert.equal(u.work, 90);
});
test('metadata per-file and cumulative partition limits do not reset', () => {
  assert.throws(() => new SourceAccount().admitRead({id: 'large', kind: 'metadata'}, CAPS.perMetadata + 1));
  const a = new SourceAccount({deltaBytes: 3});
  a.admitRead({id: 'first', kind: 'delta'}, 2);
  assert.throws(() => a.admitRead({id: 'second', kind: 'delta'}, 2));
  assert.equal(a.usage().metadataBytes, 2);
  assert.equal(a.usage().roles, 1);
});
test('read roles cannot be reused and API roles remain singular', () => {
  const a = new SourceAccount();
  a.admitRead({id: 'same', kind: 'selected'}, 1);
  assert.throws(() => a.admitRead({id: 'same', kind: 'auxiliary'}, 1));
  const b = new SourceAccount();
  b.admitRead({id: 'api1', kind: 'api'}, 1);
  assert.throws(() => b.admitRead({id: 'api2', kind: 'api'}, 1));
});
test('row and metadata-role counts are independent bounded work dimensions', () => {
  const a = new SourceAccount();
  for (let i = 0; i < 63; i++) a.admitRead({id: 'source/' + i, kind: 'selected'}, 1);
  assert.throws(() => a.admitRead({id: 'source/63', kind: 'selected'}, 1));
  const b = new SourceAccount();
  for (let i = 0; i < 16; i++) b.admitRead({id: 'metadata/' + i, kind: 'metadata'}, 1);
  assert.throws(() => b.admitRead({id: 'metadata/16', kind: 'metadata'}, 1));
});
test('read requests and both calls are prepaid exactly once', () => {
  const a = new SourceAccount({requestedBytes: 4, readCalls: 2, work: 9});
  a.admitRead({id: 'exact', kind: 'selected'}, 3);
  assert.equal(a.usage().requestedBytes, 4);
  assert.equal(a.usage().readCalls, 2);
  assert.equal(a.usage().work, 9);
  for (const lower of [{requestedBytes: 3}, {readCalls: 1}, {work: 8}]) {
    const b = new SourceAccount(lower);
    assert.throws(() => b.admitRead({id: 'short', kind: 'selected'}, 3));
    assert.equal(b.usage().roles, 0);
  }
});
test('every cumulative counter supports exact and one-more sticky refusal', () => {
  for (const [key, cap] of Object.entries(CAPS).filter(([k]) => !k.startsWith('per'))) {
    const a = new SourceAccount();
    a.charge({[key]: cap});
    a.charge({[key]: 0});
    assert.throws(() => a.charge({[key]: 1}));
    assert.equal(a.usage()[key], cap);
    assert.equal(a.usage().failed, true);
  }
});
test('safe arithmetic rejects overflow and nonnumeric operands', () => {
  assert.equal(add(Number.MAX_SAFE_INTEGER, 0), Number.MAX_SAFE_INTEGER);
  assert.equal(multiply(0, Number.MAX_SAFE_INTEGER), 0);
  for (const fn of [() => add(Number.MAX_SAFE_INTEGER, 1), () => multiply(Number.MAX_SAFE_INTEGER, 2),
    () => add('1', 2), () => multiply(-1, 2), () => add(NaN, 0)]) assert.throws(fn);
});
test('malformed roles and charges refuse without accepted credit', () => {
  for (const role of [{id: 'a', kind: 'unknown'}, {id: '../x', kind: 'selected', extra: true},
    {id: '', kind: 'selected'}, {id: 'a\n', kind: 'selected'}]) {
    const a = new SourceAccount();
    assert.throws(() => a.admitRead(role, 1));
    assert.equal(a.usage().roles, 0);
    assert.equal(a.usage().failed, true);
  }
  assert.throws(() => new SourceAccount().charge({foreign: 1}));
});

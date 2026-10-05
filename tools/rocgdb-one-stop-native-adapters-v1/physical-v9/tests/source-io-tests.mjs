// SPDX-License-Identifier: GPL-3.0-or-later
// Inert filesystem port only; these tests open no actual source/native files.
import test from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import crypto from 'node:crypto';
import {SourceAccount} from './source-account.mjs';
import {readPinned} from './source-io.mjs';

const pathname = '/inert/source';
const role = {id: 'fixture', kind: 'selected'};
const pin = b => ({bytes: b.length, sha256: crypto.createHash('sha256').update(b).digest('hex')});
function port(bytes, options = {}) {
  const calls = {realpath: 0, open: 0, read: [], fstat: 0, stat: 0, close: 0};
  const record = () => ({dev: 1n, ino: 2n, size: BigInt(bytes.length), mode: 33188n,
    nlink: 1n, mtimeNs: 3n, ctimeNs: 4n, isFile: () => true});
  const io = {
    realpathSync(name) {
      assert.equal(name, pathname);
      calls.realpath++;
      return options.realpath?.(calls.realpath) ?? name;
    },
    openSync(name, flags) {
      calls.open++;
      assert.equal(name, pathname);
      assert(flags & fs.constants.O_NOFOLLOW);
      assert(flags & fs.constants.O_NONBLOCK);
      options.beforeOpen?.();
      if (options.openError) throw options.openError;
      return 7;
    },
    fstatSync(fd, arg) {
      assert.equal(fd, 7);
      assert.deepEqual(arg, {bigint: true});
      return options.fstat?.(++calls.fstat, record()) ?? record();
    },
    statSync(name, arg) {
      assert.equal(name, pathname);
      assert.deepEqual(arg, {bigint: true});
      calls.stat++;
      return options.stat?.(record()) ?? record();
    },
    readSync(fd, out, offset, length, position) {
      assert.equal(fd, 7);
      calls.read.push({length, position});
      if (options.readError) throw options.readError;
      if (position === 0) {
        const count = options.short ? length - 1 : length;
        bytes.copy(out, offset, 0, count);
        return count;
      }
      assert.equal(position, bytes.length);
      return options.growth ? 1 : 0;
    },
    closeSync(fd) {
      assert.equal(fd, 7);
      calls.close++;
      if (options.closeError) throw options.closeError;
    },
  };
  return {io, calls};
}
test('exact payload and one EOF are prepaid before opening', () => {
  const b = Buffer.from('abc');
  const a = new SourceAccount();
  const p = port(b, {beforeOpen() {
    assert.equal(a.usage().requestedBytes, 4);
    assert.equal(a.usage().readCalls, 2);
    assert.equal(a.usage().work, 9);
  }});
  const r = readPinned(a, pathname, pin(b), role, p.io);
  assert(r.bytes.equals(b));
  assert.equal(r.text, 'abc');
  assert.deepEqual(p.calls.read, [{length: 3, position: 0}, {length: 1, position: 3}]);
  assert.equal(p.calls.close, 1);
});
test('strict UTF8 preserves a BOM and refuses invalid encoding', () => {
  const b = Buffer.from('\ufeffπ\n');
  assert.equal(readPinned(new SourceAccount(), pathname, pin(b), role, port(b).io).text, '\ufeffπ\n');
  const bad = Buffer.from([255]);
  const p = port(bad);
  assert.throws(() => readPinned(new SourceAccount(), pathname, pin(bad), role, p.io));
  assert.equal(p.calls.close, 1);
});
test('short payload fails once with no retry or EOF read', () => {
  const b = Buffer.from('abc'), a = new SourceAccount(), p = port(b, {short: true});
  assert.throws(() => readPinned(a, pathname, pin(b), role, p.io), /short payload/);
  assert.equal(p.calls.read.length, 1);
  assert.equal(p.calls.close, 1);
  assert.equal(a.usage().requestedBytes, 4);
  assert.equal(a.usage().failed, true);
});
test('growth at EOF is refused and the descriptor is closed', () => {
  const b = Buffer.from('abc'), p = port(b, {growth: true});
  assert.throws(() => readPinned(new SourceAccount(), pathname, pin(b), role, p.io), /EOF/);
  assert.equal(p.calls.read.length, 2);
  assert.equal(p.calls.close, 1);
});
test('extent mismatch refuses before payload allocation and reading', () => {
  const b = Buffer.from('abc'), p = port(b, {fstat(_n, s) { return {...s, size: 4n}; }});
  assert.throws(() => readPinned(new SourceAccount(), pathname, pin(b), role, p.io), /extent/);
  assert.equal(p.calls.read.length, 0);
  assert.equal(p.calls.close, 1);
});
test('every descriptor and named identity field is checked', () => {
  const b = Buffer.from('abc');
  for (const key of ['dev', 'ino', 'size', 'mode', 'nlink', 'mtimeNs', 'ctimeNs']) {
    const mutate = s => ({...s, [key]: s[key] + 1n});
    for (const option of [{fstat(n, s) { return n === 2 ? mutate(s) : s; }}, {stat: mutate}]) {
      const p = port(b, option);
      assert.throws(() => readPinned(new SourceAccount(), pathname, pin(b), role, p.io), /identity/);
      assert.equal(p.calls.close, 1);
    }
  }
});
test('canonical path substitutions before and after reading refuse', () => {
  const b = Buffer.from('abc');
  for (const when of [1, 2]) {
    const p = port(b, {realpath(n) { return n === when ? '/foreign' : pathname; }});
    assert.throws(() => readPinned(new SourceAccount(), pathname, pin(b), role, p.io), /canonical/);
    assert.equal(p.calls.open, when === 1 ? 0 : 1);
    assert.equal(p.calls.close, when === 1 ? 0 : 1);
  }
});
test('digest substitution and nonregular or inexact metadata refuse', () => {
  const b = Buffer.from('abc');
  assert.throws(() => readPinned(new SourceAccount(), pathname, {...pin(b), sha256: '0'.repeat(64)}, role, port(b).io), /digest/);
  for (const option of [{fstat(_n, s) { return {...s, isFile: () => false}; }},
    {stat(s) { return {...s, ino: 2}; }}]) {
    const p = port(b, option);
    assert.throws(() => readPinned(new SourceAccount(), pathname, pin(b), role, p.io));
    assert.equal(p.calls.close, 1);
  }
});
test('one-short credit refuses before any filesystem operation', () => {
  const b = Buffer.from('abc');
  for (const limits of [{requestedBytes: 3}, {readCalls: 1}, {work: 8}, {selectedBytes: 2}]) {
    const p = port(b), a = new SourceAccount(limits);
    assert.throws(() => readPinned(a, pathname, pin(b), role, p.io));
    assert.equal(p.calls.realpath, 0);
    assert.equal(p.calls.open, 0);
    assert.equal(p.calls.read.length, 0);
  }
});
test('read failure remains the first error when close also fails', () => {
  const b = Buffer.from('abc'), first = new Error('EINTR fixture'), a = new SourceAccount();
  const p = port(b, {readError: first, closeError: new Error('close fixture')});
  assert.throws(() => readPinned(a, pathname, pin(b), role, p.io), error => error === first);
  assert.equal(p.calls.read.length, 1);
  assert.equal(p.calls.close, 1);
  assert.throws(() => readPinned(a, pathname, pin(b), {id: 'other', kind: 'selected'}, p.io), error => error === first);
  assert.equal(p.calls.open, 1);
});
test('open and close errors are sticky without leaking a descriptor', () => {
  const b = Buffer.from('abc');
  const open = port(b, {openError: new Error('nofollow fixture')});
  const a = new SourceAccount();
  assert.throws(() => readPinned(a, pathname, pin(b), role, open.io), /nofollow fixture/);
  assert.equal(open.calls.close, 0);
  const close = port(b, {closeError: new Error('close fixture')});
  const c = new SourceAccount();
  assert.throws(() => readPinned(c, pathname, pin(b), role, close.io), /close fixture/);
  assert.equal(c.usage().failed, true);
});
test('invalid pin and path shape refuse before IO', () => {
  const b = Buffer.from('abc');
  for (const file of ['', 'relative', '/x\0y', '/' + 'x'.repeat(4096)]) {
    const p = port(b);
    assert.throws(() => readPinned(new SourceAccount(), file, pin(b), role, p.io));
    assert.equal(p.calls.open, 0);
  }
  for (const bad of [{...pin(b), bytes: 0}, {...pin(b), extra: true}, {...pin(b), sha256: 'A'.repeat(64)}])
    assert.throws(() => readPinned(new SourceAccount(), pathname, bad, role, port(b).io));
});

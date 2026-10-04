// SPDX-License-Identifier: GPL-3.0-or-later
// V5 exact-read policy: short reads refuse, never retry with refreshed credit.
import fs from 'node:fs';
import path from 'node:path';
import crypto from 'node:crypto';
import assert from 'node:assert/strict';
import {TextDecoder} from 'node:util';

const IDENTITY = Object.freeze(['dev', 'ino', 'size', 'mode', 'nlink', 'mtimeNs', 'ctimeNs']);
function same(a, b) {
  assert(a.isFile() && b.isFile(), 'regular file required');
  for (const key of IDENTITY) {
    assert(typeof a[key] === 'bigint' && typeof b[key] === 'bigint', 'exact file metadata');
    assert.equal(a[key], b[key], 'file identity changed: ' + key);
  }
}
function expected(spec) {
  assert(spec && typeof spec === 'object' && !Array.isArray(spec), 'pin object');
  assert.deepEqual(Object.keys(spec).sort(), ['bytes', 'sha256']);
  assert(Number.isSafeInteger(spec.bytes) && spec.bytes > 0, 'positive exact extent');
  assert(typeof spec.sha256 === 'string' && /^[0-9a-f]{64}$/.test(spec.sha256), 'SHA256 pin');
}
export function readPinned(account, filename, spec, role, io = fs) {
  // The IO port is injectable only for inert controls. Public entrypoints must
  // use the default fs port and a role/pin from their closed trusted manifest.
  return account.run(() => {
    expected(spec);
    assert(typeof filename === 'string' && filename.length > 1 && filename.length <= 4096 &&
      path.isAbsolute(filename) && !/[\x00-\x1f\x7f]/.test(filename), 'canonical absolute path');
    account.admitRead(role, spec.bytes);
    assert.equal(io.realpathSync(filename), filename, 'canonical path required');
    let fd;
    try {
      fd = io.openSync(filename, fs.constants.O_RDONLY | fs.constants.O_NOFOLLOW | fs.constants.O_NONBLOCK);
      const before = io.fstatSync(fd, {bigint: true});
      assert(before.isFile(), 'regular file required');
      assert.equal(before.size, BigInt(spec.bytes), 'declared file extent changed');
      // Extents, both requested-read slots and hash/decode work are prepaid.
      const bytes = Buffer.alloc(spec.bytes);
      const eof = Buffer.alloc(1);
      const got = io.readSync(fd, bytes, 0, bytes.length, 0);
      assert.equal(got, bytes.length, 'short payload read refused');
      assert.equal(io.readSync(fd, eof, 0, 1, bytes.length), 0, 'growth or missing EOF');
      same(before, io.fstatSync(fd, {bigint: true}));
      same(before, io.statSync(filename, {bigint: true}));
      assert.equal(io.realpathSync(filename), filename, 'canonical path changed');
      assert.equal(crypto.createHash('sha256').update(bytes).digest('hex'), spec.sha256, 'payload digest mismatch');
      const text = new TextDecoder('utf-8', {fatal: true, ignoreBOM: true}).decode(bytes);
      return Object.freeze({bytes, text});
    } catch (error) {
      throw account.refuse(error);
    } finally {
      if (fd !== undefined) {
        try { io.closeSync(fd); } catch (error) { throw account.refuse(error); }
      }
    }
  });
}

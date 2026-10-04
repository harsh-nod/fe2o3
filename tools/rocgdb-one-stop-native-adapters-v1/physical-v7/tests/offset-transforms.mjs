// SPDX-License-Identifier: GPL-3.0-or-later
// One forward and inverse reconstruction per file; no repeated full-file replace.
import assert from 'node:assert/strict';
import crypto from 'node:crypto';
import {add, multiply} from './source-account.mjs';

const digest = bytes => crypto.createHash('sha256').update(bytes).digest('hex');
function pin(bytes, spec) {
  assert(spec && typeof spec === 'object' && !Array.isArray(spec), 'transform pin');
  assert.deepEqual(Object.keys(spec).sort(), ['bytes', 'sha256']);
  assert.equal(bytes.length, spec.bytes, 'transform exact extent');
  assert(typeof spec.sha256 === 'string' && /^[0-9a-f]{64}$/.test(spec.sha256), 'transform SHA256');
  assert.equal(digest(bytes), spec.sha256, 'transform digest');
}
function reconstruct(input, changes, direction, outputBytes) {
  const output = Buffer.alloc(outputBytes);
  let from = 0, to = 0;
  for (const c of changes) {
    const offset = direction === 'forward' ? c.beforeOffset : c.afterOffset;
    const before = direction === 'forward' ? c.before : c.after;
    const after = direction === 'forward' ? c.after : c.before;
    const end = offset + before.length;
    const unchanged = offset - from;
    input.copy(output, to, from, offset);
    to += unchanged;
    after.copy(output, to);
    to += after.length;
    from = end;
  }
  input.copy(output, to, from);
  assert.equal(to + input.length - from, outputBytes, 'reconstruction exact extent');
  return output;
}
export function verifyTransformPair(account, before, after, beforePin, afterPin, changes) {
  // before/after and decoded fragment Buffers are caller-owned pinned inputs.
  // The caller must debit their reads/metadata decoding on this same account.
  // This primitive does not make a raw report or caller-made pin authoritative.
  return account.run(() => {
    assert(Buffer.isBuffer(before) && Buffer.isBuffer(after), 'source byte inputs');
    assert(before.length > 0 && after.length > 0 &&
      before.length <= account.limits().perFile && after.length <= account.limits().perFile, 'source extent');
    assert(Array.isArray(changes) && changes.length > 0, 'nonempty change roster');
    // Bound descriptor work before walking it. There is no refund on failure.
    account.charge({hunks: changes.length});
    let patterns = 0, oldEnd = 0, newEnd = 0, oldStart = -1;
    for (const c of changes) {
      assert(c && typeof c === 'object' && !Array.isArray(c), 'range object');
      assert.deepEqual(Object.keys(c).sort(), ['after', 'afterOffset', 'before', 'beforeOffset']);
      assert(Buffer.isBuffer(c.before) && Buffer.isBuffer(c.after), 'decoded byte fragments');
      assert(Number.isSafeInteger(c.beforeOffset) && c.beforeOffset >= 0 &&
        Number.isSafeInteger(c.afterOffset) && c.afterOffset >= 0, 'safe range offsets');
      assert(c.before.length + c.after.length > 0, 'empty change');
      assert(c.beforeOffset > oldStart && c.beforeOffset >= oldEnd &&
        c.afterOffset >= newEnd, 'ordered nonoverlapping ranges');
      assert.equal(c.beforeOffset - oldEnd, c.afterOffset - newEnd, 'derived inverse offset');
      oldEnd = add(c.beforeOffset, c.before.length);
      newEnd = add(c.afterOffset, c.after.length);
      assert(oldEnd <= before.length && newEnd <= after.length, 'range inside source');
      oldStart = c.beforeOffset;
      patterns = add(patterns, add(c.before.length, c.after.length));
    }
    assert.equal(before.length - oldEnd, after.length - newEnd, 'unchanged tail extent');
    // Two directions consume both input/output extents and both pattern sets.
    const extent = add(before.length, after.length);
    const work = multiply(8, add(multiply(2, extent), multiply(2, patterns)));
    account.charge({expansionBytes: extent, patternBytes: patterns, work});
    pin(before, beforePin);
    pin(after, afterPin);
    for (const c of changes) {
      assert(before.subarray(c.beforeOffset, c.beforeOffset + c.before.length).equals(c.before), 'exact before fragment');
      assert(after.subarray(c.afterOffset, c.afterOffset + c.after.length).equals(c.after), 'exact after fragment');
    }
    const forward = reconstruct(before, changes, 'forward', after.length);
    assert(forward.equals(after), 'full forward equality');
    assert.equal(digest(forward), afterPin.sha256, 'forward postimage pin');
    const inverse = reconstruct(after, changes, 'inverse', before.length);
    assert(inverse.equals(before), 'full inverse equality');
    assert.equal(digest(inverse), beforePin.sha256, 'inverse preimage pin');
    return Object.freeze({beforeBytes: before.length, afterBytes: after.length,
      changes: changes.length, expansionBytes: extent, logicalWork: work});
  });
}

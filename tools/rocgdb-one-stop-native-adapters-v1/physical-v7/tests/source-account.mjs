// SPDX-License-Identifier: GPL-3.0-or-later
// V5-only cumulative logical accounting. Not a Node heap/RSS or native budget.
import assert from 'node:assert/strict';

export const CAPS = Object.freeze({
  perFile: 524288, perMetadata: 65536,
  selectedBytes: 2293760, selectedRows: 63,
  metadataBytes: 393216, metadataRoles: 16,
  manifestBytes: 131072, deltaBytes: 131072, auxiliaryMetadataBytes: 131072,
  auxiliaryBytes: 2097152, auxiliaryRoles: 48,
  apiBytes: 312312, apiRoles: 1,
  roles: 128, requestedBytes: 5242880, readCalls: 256,
  expansionBytes: 4489216, patternBytes: 131072, hunks: 128,
  work: 100663296, reportBytes: 8192,
});
const COUNTERS = Object.freeze(Object.keys(CAPS).filter(k => !k.startsWith('per')));
const META = Object.freeze({
  manifest: 'manifestBytes', delta: 'deltaBytes', metadata: 'auxiliaryMetadataBytes',
});
export function add(a, b) {
  assert(Number.isSafeInteger(a) && a >= 0 && Number.isSafeInteger(b) && b >= 0, 'nonnegative safe operands');
  const n = a + b;
  assert(Number.isSafeInteger(n), 'integer addition overflow');
  return n;
}
export function multiply(a, b) {
  assert(Number.isSafeInteger(a) && a >= 0 && Number.isSafeInteger(b) && b >= 0, 'nonnegative safe operands');
  const n = a * b;
  assert(Number.isSafeInteger(n), 'integer multiplication overflow');
  return n;
}
export class SourceAccount {
  #limits;
  #used = Object.fromEntries(COUNTERS.map(k => [k, 0]));
  #roles = new Set();
  #failure = null;
  constructor(lower = {}) {
    assert(lower && typeof lower === 'object' && !Array.isArray(lower), 'limit object');
    for (const [key, value] of Object.entries(lower)) {
      assert(Object.hasOwn(CAPS, key), 'unknown limit');
      assert(Number.isSafeInteger(value) && value >= 0 && value <= CAPS[key], 'limits may only decrease');
    }
    this.#limits = Object.freeze({...CAPS, ...lower});
  }
  refuse(error) {
    if (this.#failure === null) this.#failure = error instanceof Error ? error : new Error('source account refused');
    return this.#failure;
  }
  run(fn) {
    if (this.#failure !== null) throw this.#failure;
    try { return fn(); } catch (error) { throw this.refuse(error); }
  }
  charge(charges) {
    return this.run(() => {
      assert(charges && typeof charges === 'object' && !Array.isArray(charges), 'charge object');
      // Check the whole fixed-field debit before updating any accepted credit.
      for (const [key, value] of Object.entries(charges)) {
        assert(COUNTERS.includes(key), 'unknown charge');
        assert(add(this.#used[key], value) <= this.#limits[key], 'source account limit: ' + key);
      }
      for (const [key, value] of Object.entries(charges)) this.#used[key] += value;
    });
  }
  admitRead(role, bytes) {
    return this.run(() => {
      assert(role && typeof role === 'object' && !Array.isArray(role), 'read role');
      assert.deepEqual(Object.keys(role).sort(), ['id', 'kind']);
      assert(typeof role.id === 'string' && role.id.length > 0 && role.id.length <= 256 && /^[a-zA-Z0-9._/-]+$/.test(role.id), 'bounded role identity');
      assert(!this.#roles.has(role.id), 'read role already spent; cache outside the reader');
      assert(Number.isSafeInteger(bytes) && bytes > 0 && bytes <= this.#limits.perFile, 'file extent');
      const charges = {roles: 1, requestedBytes: add(bytes, 1), readCalls: 2};
      if (Object.hasOwn(META, role.kind)) {
        assert(bytes <= this.#limits.perMetadata, 'metadata per-file limit');
        charges.metadataBytes = bytes;
        charges.metadataRoles = 1;
        charges[META[role.kind]] = bytes;
        charges.work = multiply(bytes, 12);
      } else {
        assert(['selected', 'auxiliary', 'api'].includes(role.kind), 'closed read kind');
        const key = {selected: 'selected', auxiliary: 'auxiliary', api: 'api'}[role.kind];
        charges[key + 'Bytes'] = bytes;
        charges[key === 'selected' ? 'selectedRows' : key + 'Roles'] = 1;
        charges.work = multiply(bytes, 3);
      }
      this.charge(charges);
      // This role and both read slots remain spent after a short read or error.
      this.#roles.add(role.id);
    });
  }
  limits() { return this.#limits; }
  usage() { return Object.freeze({...this.#used, failed: this.#failure !== null}); }
}

// SPDX-License-Identifier: GPL-3.0-or-later
// One closed v9 command account. Ancestor validators are never invoked.
import fs from 'node:fs';
import path from 'node:path';
import {fileURLToPath} from 'node:url';
import assert from 'node:assert/strict';
import crypto from 'node:crypto';
import {SourceAccount, CAPS, add} from './source-account.mjs';
import {readPinned} from './source-io.mjs';
import {verifyTransformPair} from './offset-transforms.mjs';
import {CONTRACT_PIN, PATHS, EXPECTED_CHANGED} from './contract-pin.mjs';

export const STAGE = 'physical-paired-completion-resume-disabled-v9';
const own = name => fileURLToPath(new URL('../' + name, import.meta.url));
const digest = value => crypto.createHash('sha256').update(value).digest('hex');
const pin = r => ({bytes: r.bytes, sha256: r.sha256});
const equal = (a, b) => assert.deepEqual(a, b);
const keys = (v, wanted) => {
  assert(v && typeof v === 'object' && !Array.isArray(v), 'closed object');
  equal(Object.keys(v).sort(), wanted.slice().sort());
};
function relative(p) {
  assert(typeof p === 'string' && p.length > 0 && p.length <= 192 &&
    /^[a-zA-Z0-9._/-]+$/.test(p) && !p.startsWith('/') &&
    p.split('/').every(s => s.length > 0 && s !== '.' && s !== '..'), 'closed relative path');
}
function row(r, expectedPath) {
  keys(r, ['path', 'bytes', 'sha256']);
  relative(r.path);
  if (expectedPath !== undefined) assert.equal(r.path, expectedPath);
  assert(Number.isSafeInteger(r.bytes) && r.bytes > 0 && r.bytes <= CAPS.perFile, 'row extent');
  assert(typeof r.sha256 === 'string' && /^[0-9a-f]{64}$/.test(r.sha256), 'row digest');
}
function canonical(read) {
  // readPinned prepaid 12*encodedBytes for metadata before its read/decoding.
  const v = JSON.parse(read.text);
  assert.equal(JSON.stringify(v, null, 2) + '\n', read.text, 'canonical JSON metadata');
  return v;
}
function bootstrapSource() {
  // The pinned metadata's decoding/validation credit covers this bounded encoding.
  return '// SPDX-License-Identifier: GPL-3.0-or-later\n' +
    '// Generated exact manifest anchor; payload read is charged by SourceSession.\n' +
    'export const CONTRACT_PIN = Object.freeze(' + JSON.stringify(CONTRACT_PIN) + ');\n' +
    'export const PATHS = Object.freeze(' + JSON.stringify(PATHS) + ');\n' +
    'export const EXPECTED_CHANGED = Object.freeze(' + JSON.stringify(EXPECTED_CHANGED) + ');\n';
}
function freeze(value) {
  if (value && typeof value === 'object') {
    for (const child of Object.values(value)) freeze(child);
    Object.freeze(value);
  }
  return value;
}
export function validateManifest(v) {
  // Production supplies only an exact hash-checked, bounded canonical document.
  // This exposed predicate supports synthetic mutations, not arbitrary input admission.
  keys(v, ['schema', 'license', 'upstream', 'stage', 'status',
    'activation_available', 'capture_available', 'publication_available',
    'source_checks_are_native_authority', 'caps', 'native_caps', 'predecessor',
    'selected', 'changed', 'payloads', 'bootstrap', 'patch', 'series', 'external_api_header',
    'fixture_contracts', 'source_scope']);
  assert.equal(v.schema, 'fe2o3-rocgdb-one-stop-physical-v9-source-v1');
  assert.equal(v.license, 'GPL-3.0-or-later');
  equal(v.upstream, {repository: 'https://github.com/ROCm/ROCgdb', commit: '48b1d324e389d2ed5e19822d377ff9050770233d'});
  assert.equal(v.stage, STAGE);
  assert.equal(v.status, 'provisional-reviewed-source-inputs-not-native-qualified');
  for (const k of ['activation_available', 'capture_available', 'publication_available', 'source_checks_are_native_authority'])
    assert.equal(v[k], false, 'disabled source contract');
  equal(v.caps, CAPS);
  equal(v.native_caps, {logical_bytes: 65536, api_calls: 192, read_bytes: 65536,
    read_calls: 256, work: 131072, rows: 16, events: 64, callbacks: 8,
    client_output_bytes: 128, file_requested_bytes: 8324130,
    file_observed_bytes: 8324128, file_probe_bytes: 2, file_rounds: 8324224,
    proc_requested_bytes: 16384});
  keys(v.predecessor, ['directory', 'stage', 'manifest']);
  assert.equal(v.predecessor.directory, '../physical-v8');
  assert.equal(v.predecessor.stage, 'physical-scalar-checkpoint-presentation-disabled-v8');
  row(v.predecessor.manifest, 'source-manifest.json');
  assert(v.predecessor.manifest.bytes <= CAPS.perMetadata);
  keys(v.selected, ['bytes', 'files']);
  assert(Array.isArray(v.selected.files) && v.selected.files.length === 63);
  assert.equal(PATHS.length, 63);
  let selected = 0;
  v.selected.files.forEach((r, i) => { row(r, PATHS[i]); selected = add(selected, r.bytes); });
  assert.equal(selected, v.selected.bytes);
  assert(selected <= CAPS.selectedBytes, 'combined selected source cap');
  assert(Array.isArray(v.changed));
  equal(v.changed.map(c => c.source), EXPECTED_CHANGED);
  const sources = new Map(v.selected.files.map(r => [r.path, r]));
  const payloads = new Map();
  const sums = {auxiliary: 0, delta: 0, metadata: 0};
  const roles = {auxiliary: 0, delta: 0, metadata: 0};
  assert(Array.isArray(v.payloads) && v.payloads.length <= CAPS.auxiliaryRoles + CAPS.metadataRoles - 2);
  for (const r of v.payloads) {
    keys(r, ['path', 'bytes', 'sha256', 'kind']);
    row({path: r.path, ...pin(r)});
    assert(['auxiliary', 'delta', 'metadata'].includes(r.kind), 'package role kind');
    assert(!payloads.has(r.path), 'unique package role');
    payloads.set(r.path, r);
    sums[r.kind] = add(sums[r.kind], r.bytes);
    roles[r.kind]++;
    if (r.kind !== 'auxiliary') assert(r.bytes <= CAPS.perMetadata);
  }
  keys(v.bootstrap, ['path', 'bytes']);
  assert.equal(v.bootstrap.path, 'tests/contract-pin.mjs');
  assert(!payloads.has(v.bootstrap.path), 'bootstrap has one distinct counted role');
  assert.equal(v.bootstrap.bytes, Buffer.byteLength(bootstrapSource()));
  sums.auxiliary = add(sums.auxiliary, v.bootstrap.bytes);
  roles.auxiliary++;
  assert(roles.auxiliary <= CAPS.auxiliaryRoles && roles.delta + roles.metadata + 2 <= CAPS.metadataRoles);
  assert(sums.auxiliary <= CAPS.auxiliaryBytes && sums.delta <= CAPS.deltaBytes &&
    sums.metadata <= CAPS.auxiliaryMetadataBytes);
  assert(CONTRACT_PIN.bytes + v.predecessor.manifest.bytes <= CAPS.manifestBytes);
  assert(CONTRACT_PIN.bytes + v.predecessor.manifest.bytes + sums.delta + sums.metadata <= CAPS.metadataBytes);
  const requirePayload = (p, kind) => {
    relative(p);
    const r = payloads.get(p);
    assert(r && r.kind === kind, 'exact package payload role');
    return r;
  };
  for (const c of v.changed) {
    keys(c, ['source', 'before', 'after', 'preimage', 'postimage', 'delta']);
    row(c.before, c.source);
    row(c.after, c.source);
    equal(c.after, sources.get(c.source));
    equal(pin(requirePayload(c.preimage, 'auxiliary')), pin(c.before));
    equal(pin(requirePayload(c.postimage, 'auxiliary')), pin(c.after));
    requirePayload(c.delta, 'delta');
  }
  requirePayload(v.patch, 'auxiliary');
  requirePayload(v.series, 'auxiliary');
  assert(Array.isArray(v.fixture_contracts));
  for (const p of v.fixture_contracts) requirePayload(p, 'metadata');
  row(v.external_api_header, 'amd-dbgapi/amd-dbgapi.h');
  assert.equal(v.external_api_header.bytes, CAPS.apiBytes);
  assert.equal(v.external_api_header.sha256, '2d0f9629299ecd8c0e72ff292f127573db499a3ab559ebe40ae66d366bde176a');
  assert.equal(v.source_scope, 'exact selected source and declared package payloads only; no whole checkout, build, native or milestone authority');
  const encoded = JSON.stringify(v, null, 2) + '\n';
  assert.equal(Buffer.byteLength(encoded), CONTRACT_PIN.bytes);
  assert.equal(digest(encoded), CONTRACT_PIN.sha256, 'exact versioned manifest');
  return v;
}
export function falseGates(files) {
  for (const [leaf, fn] of [
    ['gdb/amd-dbgapi-one-stop-activation-v1.h', 'selection_available'],
    ['gdb/amd-dbgapi-one-stop-snapshot-v1.h', 'snapshot_capture_available'],
    ['gdb/amd-dbgapi-one-stop-publication-v2.h', 'snapshot_publication_available'],
  ]) {
    const text = files.get(leaf)?.text;
    const needle = fn + ' () noexcept { return false; }';
    assert.equal(typeof text, 'string', 'required disabled gate source');
    const first = text.indexOf(needle);
    assert(first >= 0 && text.indexOf(needle, first + needle.length) === -1, 'one literal false gate');
  }
}
function rootPath(root) {
  assert(typeof root === 'string' && root.length > 1 && root.length <= 4096 &&
    path.isAbsolute(root) && !/[\x00-\x1f\x7f]/.test(root), 'explicit source root');
  assert.equal(fs.realpathSync(root), root, 'canonical source root');
}
export class SourceSession {
  #account;
  #manifest;
  #package = new Map();
  #source = new Map();
  #sourceAccepted = false;
  #apiAccepted = false;
  #reported = false;
  constructor(lower = {}) {
    this.#account = new SourceAccount(lower);
    this.#run(() => {
      const ownRead = readPinned(this.#account, own('source-manifest.json'), CONTRACT_PIN,
        {id: 'manifest/self', kind: 'manifest'});
      this.#manifest = freeze(validateManifest(canonical(ownRead)));
      const m = this.#manifest;
      const anchor = bootstrapSource();
      this.#package.set(m.bootstrap.path, readPinned(this.#account, own(m.bootstrap.path),
        {bytes: m.bootstrap.bytes, sha256: digest(anchor)}, {id: 'package/bootstrap', kind: 'auxiliary'}));
      const parent = canonical(readPinned(this.#account, own('../physical-v8/source-manifest.json'),
        pin(m.predecessor.manifest), {id: 'manifest/parent', kind: 'manifest'}));
      assert.equal(parent.schema, 'fe2o3-rocgdb-one-stop-physical-v8-source-v1');
      assert.equal(parent.caps.selectedBytes, 2293760);
      assert.equal(parent.selected.files.length, 63);
      const old = new Map(parent.selected.files.map(r => [r.path, r]));
      const changes = new Map(m.changed.map(c => [c.source, c]));
      for (const r of m.selected.files) {
        const c = changes.get(r.path);
        if (c) equal(c.before, old.get(r.path));
        else equal(r, old.get(r.path));
      }
      for (const r of m.payloads) {
        this.#package.set(r.path, readPinned(this.#account, own(r.path), pin(r),
          {id: 'package/' + r.path, kind: r.kind}));
      }
      assert.equal(this.#package.get(m.series).text, path.posix.basename(m.patch) + '\n');
      this.#verifyChanges();
    });
  }
  #run(fn) {
    try { return this.#account.run(fn); }
    catch (error) {
      this.#source.clear();
      this.#package.clear();
      this.#manifest = undefined;
      this.#sourceAccepted = false;
      this.#apiAccepted = false;
      throw error;
    }
  }
  #verifyChanges() {
    let decoded = 0;
    for (const c of this.#manifest.changed) {
      const v = canonical(this.#package.get(c.delta));
      keys(v, ['schema', 'path', 'before', 'after', 'changes']);
      assert.equal(v.schema, 'fe2o3-disabled-physical-v9-byte-ranges-v1');
      assert.equal(v.path, c.source);
      equal(v.before, pin(c.before));
      equal(v.after, pin(c.after));
      assert(Array.isArray(v.changes) && v.changes.length > 0 && v.changes.length <= CAPS.hunks);
      // Metadata's 12*encodedBytes payment already covers descriptor decoding.
      // Check the shared decoded-byte derivation before each Buffer allocation.
      const changes = [];
      for (const x of v.changes) {
        keys(x, ['beforeOffset', 'afterOffset', 'before', 'after']);
        assert(typeof x.before === 'string' && typeof x.after === 'string', 'UTF8 fragments');
        const n = add(Buffer.byteLength(x.before), Buffer.byteLength(x.after));
        decoded = add(decoded, n);
        assert(decoded <= this.#account.limits().patternBytes, 'shared decoded-fragment bound');
        changes.push({beforeOffset: x.beforeOffset, afterOffset: x.afterOffset,
          before: Buffer.from(x.before), after: Buffer.from(x.after)});
      }
      verifyTransformPair(this.#account, this.#package.get(c.preimage).bytes,
        this.#package.get(c.postimage).bytes, pin(c.before), pin(c.after), changes);
    }
    assert.equal(this.#account.usage().patternBytes, decoded);
  }
  manifest() { return this.#run(() => this.#manifest); }
  packageText(name) {
    return this.#run(() => {
      assert(this.#package.has(name), 'closed package payload only');
      return this.#package.get(name).text;
    });
  }
  verifyStage(root, stage) {
    return this.#run(() => {
      assert(!this.#sourceAccepted, 'selected stage is single-use');
      assert.equal(stage, STAGE, 'one closed explicit v9 stage');
      rootPath(root);
      for (const r of this.#manifest.selected.files) {
        this.#source.set(r.path, readPinned(this.#account, path.join(root, r.path), pin(r),
          {id: 'source/' + r.path, kind: 'selected'}));
      }
      this.#account.charge({work: 2 * this.#manifest.selected.bytes});
      falseGates(this.#source);
      this.#sourceAccepted = true;
      return this;
    });
  }
  sourceText(name) {
    return this.#run(() => {
      assert(this.#sourceAccepted && this.#source.has(name), 'complete selected stage required');
      return this.#source.get(name).text;
    });
  }
  verifyApi(filename) {
    return this.#run(() => {
      assert(!this.#apiAccepted, 'API role is single-use');
      readPinned(this.#account, filename, pin(this.#manifest.external_api_header),
        {id: 'external/api', kind: 'api'});
      this.#apiAccepted = true;
      return this;
    });
  }
  report() {
    return this.#run(() => {
      assert(!this.#reported && (this.#sourceAccepted || this.#apiAccepted), 'a complete requested check is required');
      this.#account.charge({reportBytes: CAPS.reportBytes, work: CAPS.reportBytes});
      const result = {
        schema: 'fe2o3-rocgdb-one-stop-physical-v9-source-check-v1', stage: STAGE,
        status: this.#manifest.status, package_checked: true,
        selected_stage_checked: this.#sourceAccepted, selected_files: this.#sourceAccepted ? 63 : 0,
        selected_bytes: this.#sourceAccepted ? this.#manifest.selected.bytes : 0,
        api_header_checked: this.#apiAccepted, accounting_reservations: this.#account.usage(),
        activation_available: false, capture_available: false, publication_available: false,
        complete_checkout_verified: false, built_debugger_verified: false,
        native_authority: false, milestone_complete: false,
      };
      const text = JSON.stringify(result) + '\n';
      assert(Buffer.byteLength(text) <= CAPS.reportBytes, 'bounded report');
      this.#reported = true;
      return text;
    });
  }
  usage() { return this.#account.usage(); }
}

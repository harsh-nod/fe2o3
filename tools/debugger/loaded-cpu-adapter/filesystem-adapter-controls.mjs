// Explicit, separately gated real-filesystem controls. Never reads historical host inputs.
import test from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import {createHash} from 'node:crypto';
import {runFilesystemAdapter} from './adapter-fs.mjs';
import {deriveReadBudget} from '../loaded-input-reader/reader-protocol.mjs';

const root = process.env.FE2O3_ADAPTER_FS_TEST_ROOT;
assert.ok(typeof root === 'string' && path.isAbsolute(root) && root !== '/',
  'FE2O3_ADAPTER_FS_TEST_ROOT must name an explicit private empty directory');
assert.equal(fs.realpathSync(root), root);
const fields = ['dev', 'ino', 'size', 'mode', 'mtimeNs', 'ctimeNs'];
const identity = s => fields.map(k => String(s[k]));
const rootIdentityText = process.env.FE2O3_ADAPTER_FS_TEST_ROOT_IDENTITY;
assert.ok(typeof rootIdentityText === 'string' && rootIdentityText.length <= 1024,
  'FE2O3_ADAPTER_FS_TEST_ROOT_IDENTITY must bind all six initial stat fields');
const expectedRootIdentity = JSON.parse(rootIdentityText);
assert.ok(Array.isArray(expectedRootIdentity) && expectedRootIdentity.length === 6
  && expectedRootIdentity.every(x => typeof x === 'string' && /^\d+$/.test(x)));
const rootStat = fs.lstatSync(root, {bigint: true});
assert.ok(rootStat.isDirectory() && (rootStat.mode & 0o777n) === 0o700n);
assert.equal(rootStat.uid, BigInt(process.getuid()));
assert.deepEqual(identity(rootStat), expectedRootIdentity);
assert.deepEqual(fs.readdirSync(root), []);
const sha = b => createHash('sha256').update(b).digest('hex');
const encode = v => Buffer.from(JSON.stringify(v));
const byteCap = 512 * 1024;
const reports = [];
function fixture(label, {existingFinal = false, existingTemporary = false} = {}) {
  const dir = fs.mkdtempSync(path.join(root, label + '-'));
  fs.chmodSync(dir, 0o700);
  const input = path.join(dir, 'input'), output = path.join(dir, 'output');
  fs.mkdirSync(input, {mode: 0o700});
  fs.mkdirSync(output, {mode: 0o700});
  const body = Buffer.alloc(65537, 7), target = path.join(input, 'target.bin');
  fs.writeFileSync(target, body, {flag: 'wx', mode: 0o600});
  const empty = path.join(input, 'empty.bin');
  fs.writeFileSync(empty, Buffer.alloc(0), {flag: 'wx', mode: 0o600});
  const alias = path.join(input, 'alias.bin');
  fs.symlinkSync('target.bin', alias);
  const row = (name, bytes, duty, resolved = name) => {
    const s = fs.statSync(resolved, {bigint: true});
    return {path: name, kind: 'readable', pin: {path: name, bytes: bytes.length, sha256: sha(bytes)},
      resolved, identity: identity(s), ownership: {uid: String(s.uid), gid: String(s.gid)},
      labels: {prior: [], loaded: [], duties: [duty], extras: []}, cap: bytes.length};
  };
  const targetRow = row(target, body, 'target'), aliasRow = row(alias, body, 'alias', target);
  const absent = path.join(input, 'missing', 'child');
  const entries = [targetRow, aliasRow, row(empty, Buffer.alloc(0), 'empty'),
    {path: absent, kind: 'absence-observation', pin: null, resolved: absent,
      identity: null, ownership: null, labels: {prior: [], loaded: [], duties: ['absence'], extras: []}, cap: 0}];
  const aliases = [{pin: aliasRow.pin, resolved: target, link_text: 'target.bin',
    target_pin: targetRow.pin, target_selected: true}];
  const protocol = passes => ({schema: 'fe2o3-loaded-read-protocol-v1', entries, aliases, passes,
    budget: deriveReadBudget(entries, aliases, passes), selection_digest: 'a'.repeat(64),
    custody: {qualified_historical_input_bytes: false, operational_roster_complete: false,
      root_cap_change_approved: false, execution_authority: false}});
  const plan = {schema: 'fe2o3-cpu-loaded-adapter-plan-v1', graph_protocol: protocol(1),
    historical_protocol: protocol(2), named_cap: entries.length,
    phase_milliseconds: {precheck: 10000, historical: 10000, postcheck: 10000}};
  const sentinel = Buffer.from('existing output must survive\n');
  if (existingFinal) fs.writeFileSync(path.join(output, 'observation.json'), sentinel, {flag: 'wx', mode: 0o600});
  if (existingTemporary) fs.writeFileSync(path.join(output, 'observation.pending'), sentinel, {flag: 'wx', mode: 0o600});
  const spec = {schema: 'fe2o3-exclusive-adapter-evidence-v1', directory: output,
    directory_identity: identity(fs.lstatSync(output, {bigint: true})),
    temporary_name: 'observation.pending', final_name: 'observation.json', cap_bytes: byteCap};
  const now = Date.now();
  const policy = {schema: 'fe2o3-cpu-reader-guard-policy-v1', not_before_utc_ms: now - 1000,
    expires_utc_ms: now + 60000, max_elapsed_ms: 30000, max_rss_bytes: 512 * 1024 * 1024,
    min_free_bytes: 40 * 1024 ** 3, min_available_ram_bytes: 64 * 1024 ** 3,
    resource_interval_ms: 1000, max_resource_probes: 64};
  return {dir, input, output, plan, spec, policy, sentinel,
    run: () => runFilesystemAdapter(encode(plan), encode(policy), encode(spec), {resource_root: root})};
}
function readExact(name, cap) {
  const before = fs.lstatSync(name, {bigint: true});
  assert.ok(before.isFile() && before.size <= BigInt(cap));
  const fd = fs.openSync(name, fs.constants.O_RDONLY | fs.constants.O_NOFOLLOW | fs.constants.O_NONBLOCK);
  try {
    assert.deepEqual(identity(fs.fstatSync(fd, {bigint: true})), identity(before));
    const body = Buffer.alloc(Number(before.size));
    for (let at = 0; at < body.length;) {
      const take = Math.min(65536, body.length - at);
      assert.equal(fs.readSync(fd, body, at, take, null), take);
      at += take;
    }
    assert.equal(fs.readSync(fd, Buffer.alloc(1), 0, 1, null), 0);
    assert.deepEqual(identity(fs.fstatSync(fd, {bigint: true})), identity(before));
    assert.deepEqual(identity(fs.lstatSync(name, {bigint: true})), identity(before));
    return body;
  } finally { fs.closeSync(fd); }
}
function remember(x, result) {
  reports.push({directory: x.dir, status: result.status, first_failure: result.first_failure,
    durable_evidence_available: result.summary.durable_evidence_available,
    evidence_pin: result.summary.evidence_pin ?? null});
}
function acceptedEvidence(x, result) {
  assert.equal(result.publication.publication_accepted, true);
  assert.equal(result.publication.descriptor.live_fd_possible, false);
  assert.equal(result.summary.resource_probe_observation.live_fd_possible, false);
  assert.equal(result.guard_observation_after_publication.denied, false);
  const pin = result.summary.evidence_pin, body = readExact(pin.path, byteCap);
  assert.equal(body.length, pin.bytes);
  assert.equal(sha(body), pin.sha256);
  const temporary = path.join(x.output, x.spec.temporary_name);
  assert.deepEqual(readExact(temporary, byteCap), body);
  assert.equal(fs.statSync(temporary).ino, fs.statSync(pin.path).ino);
  const evidence = JSON.parse(body);
  assert.deepEqual(evidence.reader_record, result.record);
  assert.match(evidence.publication_state, /intent only/);
  assert.equal(result.summary.qualified, false);
  assert.equal(result.summary.native_authority, false);
  assert.equal(result.summary.child_processes_started, 0);
  return evidence;
}
test('real files, selected alias target, empty file and absence complete all four passes', () => {
  const x = fixture('complete'), result = x.run();
  remember(x, result);
  assert.equal(result.status, 'read-and-publication-observed');
  assert.deepEqual(result.record.phases.map(p => p.status), ['completed', 'completed', 'completed']);
  assert.deepEqual(result.record.phases.map(p => p.result.counts.completed_passes), [1, 2, 1]);
  assert.equal(result.record.shared_accounting.requested_content_bytes, 4 * (65538 * 2 + 1));
  acceptedEvidence(x, result);
});
test('wrong whole-file hash stays primary and durable failure evidence is read back', () => {
  const x = fixture('wrong-hash');
  x.plan.graph_protocol.entries[0].pin.sha256 = '0'.repeat(64);
  x.plan.graph_protocol.entries[1].pin.sha256 = '0'.repeat(64);
  // The target alias rule must agree structurally; the immutable file itself is unchanged.
  x.plan.graph_protocol.aliases[0].target_pin.sha256 = '0'.repeat(64);
  const result = x.run();
  remember(x, result);
  assert.equal(result.status, 'failed');
  assert.equal(result.first_failure.code, 'whole-content-hash-mismatch');
  assert.deepEqual(result.record.phases.map(p => p.status), ['failed', 'not-started', 'not-started']);
  acceptedEvidence(x, result);
});
test('existing final name is never overwritten and new temporary evidence remains', () => {
  const x = fixture('existing-final', {existingFinal: true}), result = x.run();
  remember(x, result);
  assert.equal(result.status, 'failed');
  assert.equal(result.record.status, 'read-protocol-completed');
  assert.equal(result.first_failure.source, 'evidence-publication');
  assert.equal(result.publication.first_error.code, 'EEXIST');
  assert.equal(result.publication.publication_accepted, false);
  assert.deepEqual(readExact(path.join(x.output, x.spec.final_name), byteCap), x.sentinel);
  const retained = readExact(path.join(x.output, x.spec.temporary_name), byteCap);
  assert.equal(retained.length, result.publication.submitted_bytes);
  assert.equal(sha(retained), result.publication.submitted_sha256);
  assert.deepEqual(JSON.parse(retained).reader_record, result.record);
});
test('existing temporary name is never overwritten and no final name appears', () => {
  const x = fixture('existing-temporary', {existingTemporary: true}), result = x.run();
  remember(x, result);
  assert.equal(result.publication.first_error.code, 'EEXIST');
  assert.equal(result.publication.publication_accepted, false);
  assert.deepEqual(readExact(path.join(x.output, x.spec.temporary_name), byteCap), x.sentinel);
  assert.equal(fs.existsSync(path.join(x.output, x.spec.final_name)), false);
});
test('expired finite policy denies before input dispatch and creates no evidence', () => {
  const x = fixture('expired');
  x.policy.expires_utc_ms = x.policy.not_before_utc_ms + 1;
  const result = x.run();
  remember(x, result);
  assert.equal(result.status, 'failed');
  assert.equal(result.summary.resource_probe_observation.probes, 0);
  assert.deepEqual(result.record.phases.map(p => p.status), ['failed', 'not-started', 'not-started']);
  assert.equal(result.record.phases[0].provider_observation.counts.provider_invocations, 0);
  assert.equal(result.publication.provider_invocations, 0);
  assert.equal(result.guard_observation_after_publication.denied, true);
  assert.deepEqual(fs.readdirSync(x.output), []);
});
test('oversized evidence refusal happens before writer creation', () => {
  const x = fixture('oversized');
  x.spec.cap_bytes = 4096;
  const result = x.run();
  remember(x, result);
  assert.equal(result.record.status, 'read-protocol-completed');
  assert.equal(result.publication.status, 'admission-refused');
  assert.equal(result.publication.first_error.message, 'adapter evidence writer: serialized evidence byte cap');
  assert.deepEqual(fs.readdirSync(x.output), []);
});
test('closed-plan admission refusal creates no evidence', () => {
  const x = fixture('invalid-plan');
  x.plan.unrecognized = true;
  const result = x.run();
  remember(x, result);
  assert.equal(result.status, 'admission-refused');
  assert.equal(result.record, null);
  assert.equal(result.publication, null);
  assert.deepEqual(fs.readdirSync(x.output), []);
});
test.after(() => {
  // Leave exact test-owned outputs available for root inspection; never recursively delete.
  process.stdout.write('filesystem-adapter-retained-fixtures ' + JSON.stringify(reports) + '\n');
});

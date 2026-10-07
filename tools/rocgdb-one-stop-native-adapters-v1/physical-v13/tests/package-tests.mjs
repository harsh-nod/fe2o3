// SPDX-License-Identifier: GPL-3.0-or-later
// Source/package controls. Each refusal case is an independent synthetic command.
// No test here builds GDB, obtains a native owner, or runs either C++ fixture.
import test from 'node:test';
import assert from 'node:assert/strict';
import {SourceSession, validateManifest, falseGates, STAGE} from './source-files.mjs';
import {CAPS} from './source-account.mjs';
import {CONTRACT_PIN} from './contract-pin.mjs';

const session = new SourceSession();
const manifest = session.manifest();
const mutate = fn => { const value = structuredClone(manifest); fn(value); return value; };
const reject = fn => assert.throws(() => validateManifest(mutate(fn)), Error);
test('closed 63-row source includes all 38 stock rows and preserves disabled caps', () => {
  assert.equal(validateManifest(manifest), manifest);
  assert.equal(manifest.selected.files.length, 63);
  const custom = manifest.selected.files.filter(r => /one-stop|runtime-observation|stopped-wave/.test(r.path));
  assert.equal(custom.length, 25);
  assert.equal(manifest.selected.files.length - custom.length, 38);
  assert.equal(manifest.selected.bytes, 2272167);
  assert.equal(CAPS.selectedBytes - manifest.selected.bytes, 21593);
  assert.equal(manifest.predecessor.stage, 'physical-proc-record-work-disabled-v12');
  assert.equal(manifest.status, 'provisional-reviewed-source-inputs-not-native-qualified');
});
test('six current leaves retain exact full forward/inverse source pins', () => {
  assert.equal(manifest.changed.length, 6);
  assert.equal(session.usage().hunks, 6);
  assert.equal(session.usage().patternBytes, manifest.changed.flatMap(r=>JSON.parse(session.packageText(r.delta)).changes).reduce((n,r)=>n+Buffer.byteLength(r.before)+Buffer.byteLength(r.after),0));
  assert.equal(session.usage().expansionBytes, manifest.changed.reduce((n,r)=>n+r.before.bytes+r.after.bytes,0));
});
test('one account charges every package and anchor role without ancestor recursion', () => {
  const u = session.usage();
  assert.equal(u.roles, manifest.payloads.length + 3); // own, parent, anchor
  assert.equal(u.readCalls, u.roles * 2);
  assert.equal(u.requestedBytes, CONTRACT_PIN.bytes + manifest.predecessor.manifest.bytes +
    manifest.bootstrap.bytes + manifest.payloads.reduce((n, r) => n + r.bytes, 0) + u.roles);
  assert.equal(u.metadataRoles, 16);
  assert.equal(u.failed, false);
  assert.equal(u.selectedRows, 0);
  assert.equal(u.apiRoles, 0);
  assert.equal(typeof session.packageText(manifest.bootstrap.path), 'string');
});
test('complete combined source API and report envelope fits unchanged approved bounds', () => {
  const u = session.usage();
  const roles = u.roles + 63 + 1;
  assert(roles <= CAPS.roles);
  assert(u.readCalls + 2 * 64 <= CAPS.readCalls);
  assert(u.requestedBytes + manifest.selected.bytes + 63 + CAPS.apiBytes + 1 <= CAPS.requestedBytes);
  assert(u.work + 5 * manifest.selected.bytes + 3 * CAPS.apiBytes + CAPS.reportBytes <= CAPS.work);
  assert.equal(CAPS.metadataBytes, 393216);
  assert.equal(CAPS.expansionBytes, 4489216);
});
test('exact construction reservations accept while each cumulative one-short refuses', () => {
  const u = session.usage();
  const names = ['roles', 'readCalls', 'requestedBytes', 'metadataBytes', 'metadataRoles',
    'manifestBytes', 'deltaBytes', 'auxiliaryMetadataBytes', 'auxiliaryBytes',
    'auxiliaryRoles', 'expansionBytes', 'patternBytes', 'hunks', 'work'];
  for (const name of names) {
    assert(u[name] > 0);
    assert.equal(new SourceSession({[name]: u[name]}).usage()[name], u[name]);
    assert.throws(() => new SourceSession({[name]: u[name] - 1}), Error, name);
  }
});
test('no caller can raise a limit or reset a command to an unknown role', () => {
  assert.throws(() => new SourceSession({work: CAPS.work + 1}), Error);
  assert.throws(() => new SourceSession({reset: 1}), Error);
});
test('closed manifest rejects omitted duplicated reordered or foreign selected rows', () => {
  reject(v => v.selected.files.pop());
  reject(v => v.selected.files[1] = v.selected.files[0]);
  reject(v => v.selected.files.reverse());
  reject(v => v.selected.files[0].path = 'gdb/foreign.h');
});
test('closed manifest rejects changed source digests extents and widened accounting', () => {
  reject(v => v.selected.files[0].sha256 = '0'.repeat(64));
  reject(v => v.selected.bytes++);
  reject(v => v.caps.selectedBytes++);
  reject(v => v.native_caps.logical_bytes++);
  reject(v => v.native_caps.work++);
});
test('all public authority gates are required false', () => {
  for (const key of ['activation_available', 'capture_available', 'publication_available',
    'source_checks_are_native_authority']) reject(v => v[key] = true);
  reject(v => v.native_authority = true);
});
test('stage predecessor external API and provisional status are exact', () => {
  reject(v => v.stage = 'physical-paired-completion-resume-disabled-v9');
  reject(v => v.predecessor.directory = '../physical-v3');
  reject(v => v.predecessor.manifest.sha256 = '0'.repeat(64));
  reject(v => v.external_api_header.bytes--);
  reject(v => v.status = 'native-qualified');
});
test('closed package roles reject duplicates traversal missing deltas and anchor omission', () => {
  reject(v => v.payloads.push(v.payloads[0]));
  reject(v => v.payloads[0].path = '../outside');
  reject(v => v.payloads.splice(v.payloads.findIndex(r => r.kind === 'delta'), 1));
  reject(v => v.bootstrap.bytes--);
  reject(v => v.bootstrap.path = 'tests/other.mjs');
});
test('manifest and all nested rosters are immutable after admission', () => {
  assert(Object.isFrozen(manifest) && Object.isFrozen(manifest.selected.files));
  assert.throws(() => manifest.selected.files.pop(), TypeError);
  assert.throws(() => manifest.changed[0].after.bytes++, TypeError);
});
test('cached reads do not grant new credits or permit hidden auxiliary files', () => {
  const s = new SourceSession(), before = s.usage();
  s.packageText(manifest.patch);
  assert.deepEqual(s.usage(), before);
  let first;
  try { s.packageText('../physical-v9/tests/source-files.mjs'); } catch (e) { first = e; }
  assert(first instanceof Error);
  assert.throws(() => s.packageText(manifest.patch), e => e === first);
  assert.throws(() => s.manifest(), e => e === first);
  assert.equal(s.usage().failed, true);
});
test('stage mismatch refuses before selected reads and remains sticky', () => {
  const s = new SourceSession(), before = s.usage();
  let first;
  try { s.verifyStage('/must-not-be-read', 'other-stage'); } catch (e) { first = e; }
  assert(first instanceof Error);
  assert.equal(s.usage().selectedRows, 0);
  assert.equal(s.usage().readCalls, before.readCalls);
  assert.throws(() => s.verifyStage('/must-not-be-read', STAGE), e => e === first);
});
test('no source text or success report escapes an incomplete selected check', () => {
  const a = new SourceSession(), b = new SourceSession();
  assert.throws(() => a.sourceText(manifest.selected.files[0].path), Error);
  assert.throws(() => a.manifest(), Error);
  assert.throws(() => b.report(), Error);
  assert.equal(b.usage().reportBytes, 0);
  assert.equal(b.usage().failed, true);
});
test('literal false-gate scan rejects absent enabled and duplicate spellings', () => {
  const names = [
    ['gdb/amd-dbgapi-one-stop-activation-v1.h', 'selection_available'],
    ['gdb/amd-dbgapi-one-stop-snapshot-v1.h', 'snapshot_capture_available'],
    ['gdb/amd-dbgapi-one-stop-publication-v2.h', 'snapshot_publication_available'],
  ];
  const make = () => new Map(names.map(([p, f]) => [p, {text: f + ' () noexcept { return false; }'}]));
  falseGates(make());
  for (const [p] of names) {
    const absent = make(); absent.delete(p); assert.throws(() => falseGates(absent), Error);
    const enabled = make(); enabled.get(p).text = enabled.get(p).text.replace('false', 'true');
    assert.throws(() => falseGates(enabled), Error);
    const duplicate = make(); duplicate.get(p).text += duplicate.get(p).text;
    assert.throws(() => falseGates(duplicate), Error);
  }
});

test('source verifier usage advertises the exact successor stage',()=>{assert(session.packageText('verify-source.mjs').includes(STAGE));});

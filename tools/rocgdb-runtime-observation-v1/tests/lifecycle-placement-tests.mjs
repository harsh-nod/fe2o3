// SPDX-License-Identifier: GPL-3.0-or-later
// Pure source placement/mutation controls. No GDB execution or exit proof.
import assert from 'node:assert/strict';
import test from 'node:test';
import {fileURLToPath} from 'node:url';
import {finalSource,manifest,readBounded,checkedText} from './source-files.mjs';

const original = finalSource()['gdb/amd-dbgapi-target.c'];
const priorFixture = JSON.parse(readBounded(fileURLToPath(new URL('./lifecycle-preimage.json',import.meta.url)),16384).toString('utf8'));
assert.equal(priorFixture.schema,'fe2o3-lifecycle-preimage-fixture-v1');
assert(Array.isArray(priorFixture.replacements)&&priorFixture.replacements.length===4);
let previous=original;
for(const row of priorFixture.replacements.slice().reverse()){
 assert.deepEqual(Object.keys(row).sort(),['after','before']);
 for(const key of ['before','after'])assert(typeof row[key]==='string'&&row[key].length>0&&row[key].length<=4096);
 assert.equal(previous.split(row.after).length,2);previous=previous.replace(row.after,row.before);
}
const historical=manifest().stages[2].files.find(r=>r.path==='gdb/amd-dbgapi-target.c');
previous=checkedText(historical,Buffer.from(previous));
const detach = '  invalidate_runtime_observation (runtime_obs::reason::detach);';
const exit = '  invalidate_runtime_observation (runtime_obs::reason::inferior_exit);';
const exec = '  invalidate_runtime_observation (runtime_obs::reason::inferior_exec);';
function section(source, from, to) {
  const start = source.indexOf(from);
  assert.ok(start >= 0 && source.indexOf(from, start + from.length) === -1, from);
  const end = source.indexOf(to, start + from.length);
  assert.ok(end > start, to);
  return source.slice(start, end);
}
const names = {
  helper: ['\nstatic void\ndetach_amd_dbgapi (inferior *inf)', '\nvoid\namd_dbgapi_target::mourn_inferior'],
  mourn: ['\nvoid\namd_dbgapi_target::mourn_inferior ()', '\nvoid\namd_dbgapi_target::detach'],
  detach: ['\nvoid\namd_dbgapi_target::detach (inferior *inf, int from_tty)', '\nvoid\namd_dbgapi_target::fetch_registers'],
  pre: ['\nstatic void\namd_dbgapi_inferior_pre_detach (inferior *inf)', '\n/* client_process_get_info callback.  */'],
  exit: ['\nstatic void\namd_dbgapi_inferior_exited (inferior *inf)', '\n/* inferior_pre_detach observer.  */'],
  exec: ['\nstatic void\namd_dbgapi_inferior_execd (inferior *exec_inf, inferior *follow_inf)', '\n/* inferior_forked observer.  */'],
};
function ordered(value, marks) {
  let prior = -1;
  for (const mark of marks) {
    const at = value.indexOf(mark);
    assert.ok(at > prior && value.indexOf(mark, at + mark.length) === -1, mark);
    prior = at;
  }
}
function check(source) {
  const body = Object.fromEntries(Object.entries(names).map(([key, pair]) => [key, section(source, ...pair)]));
  assert.ok(!body.helper.includes('invalidate_runtime_observation'));
  assert.equal((source.match(/\n[ \t]+detach_amd_dbgapi \(/g) ?? []).length, 5);
  assert.equal(source.split(detach).length - 1, 2);
  assert.equal(source.split(exit).length - 1, 3); // mourn, exit observer, removed observer
  ordered(body.mourn, [exit, 'detach_amd_dbgapi (current_inferior ());', 'beneath ()->mourn_inferior ();']);
  ordered(body.detach, [detach, 'remove_breakpoints_inf (inf);', 'detach_amd_dbgapi (inf);', 'beneath ()->detach (inf, from_tty);']);
  ordered(body.pre, [detach, 'if (!inf->target_is_pushed (&the_amd_dbgapi_target))', 'detach_amd_dbgapi (inf);']);
  ordered(body.exit, [exit, 'detach_amd_dbgapi (inf);']);
  ordered(body.exec, [exec, 'detach_amd_dbgapi (exec_inf);']);
}
function changePart(key, transform) {
  const old = section(original, ...names[key]);
  const changed = transform(old);
  assert.notEqual(changed, old, 'mutation must change the named body');
  return original.replace(old, changed);
}
function refuseMutation(makeCandidate) {
  // Baseline validity is required independently for EVERY negative control.
  // An unrelated missing source boundary may not satisfy assert.throws.
  check(original);
  const candidate = makeCandidate();
  assert.notEqual(candidate, original, 'negative control must change its input');
  assert.throws(() => check(candidate));
}
function reject(key, transform) { refuseMutation(() => changePart(key, transform)); }

test('lifecycle callers classify before teardown; helper does not classify', () => check(original));
test('previous blanket-detach source is refused', () => refuseMutation(() => previous));
test('blanket detach in internal helper refuses', () => reject('helper', s => s.replace('  info->runtime_state', detach + '\n  info->runtime_state')));
test('blanket exit in internal helper refuses', () => reject('helper', s => s.replace('  info->runtime_state', exit + '\n  info->runtime_state')));
test('missing mourn invalidation refuses', () => reject('mourn', s => s.replace(exit, '')));
test('mourn invalidation after library teardown refuses', () => reject('mourn', s => s.replace(exit, '').replace('  beneath ()', exit + '\n  beneath ()')));
test('mourn mislabeled detach refuses', () => reject('mourn', s => s.replace(exit, detach)));
test('missing direct GDB detach invalidation refuses', () => reject('detach', s => s.replace(detach, '')));
test('GDB detach invalidation after breakpoint removal refuses', () => reject('detach', s => s.replace(detach, '').replace('  detach_amd_dbgapi', detach + '\n  detach_amd_dbgapi')));
test('GDB detach mislabeled exit refuses', () => reject('detach', s => s.replace(detach, exit)));
test('missing pre-detach invalidation refuses', () => reject('pre', s => s.replace(detach, '')));
test('unpushed-only pre-detach invalidation refuses', () => reject('pre', s => s.replace(detach, '').replace('    detach_amd_dbgapi', '  ' + detach + '\n    detach_amd_dbgapi')));
test('pre-detach mislabeled exit refuses', () => reject('pre', s => s.replace(detach, exit)));
test('existing exit observer moved after library teardown refuses', () => reject('exit', s => s.replace(exit, '').replace('  detach_amd_dbgapi (inf);', '  detach_amd_dbgapi (inf);\n' + exit)));
test('existing exec cause removal refuses', () => reject('exec', s => s.replace(exec, '')));
test('unclassified extra teardown caller refuses', () => refuseMutation(() => original + '\nvoid extra_caller () {\n  detach_amd_dbgapi (nullptr);\n}\n'));
test('duplicate mourn invalidation refuses', () => reject('mourn', s => s.replace(exit, exit + '\n' + exit)));

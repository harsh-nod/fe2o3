// SPDX-License-Identifier: GPL-3.0-or-later
// Requires an explicit exact final-false source projection. Text placement is not ABI/native proof.
import test from 'node:test';
import assert from 'node:assert/strict';
import crypto from 'node:crypto';
import {fileURLToPath} from 'node:url';
import {finalSource,manifest,packageText,checkedText,readBounded,MAX_FILE,falseGates} from './source-files.mjs';
const files=finalSource(),spec=manifest();
const sha=b=>crypto.createHash('sha256').update(b).digest('hex');
function slice(source,start,end){assert.equal(source.split(start).length,2);assert.equal(source.split(end).length,2);const a=source.indexOf(start),b=source.indexOf(end);assert(b>a);return source.slice(a,b);}
test('actual helper CPU include is exact joined selected-source extraction',()=>{
 const x=JSON.parse(packageText('tests/helper-extraction.json'));let combined=x.prefix;
 for(const part of x.parts){
  const source=files[part.source];
  const body=part.lines?source.split('\n').slice(part.lines[0]-1,part.lines[1]).join('\n')+'\n':slice(source,part.start,part.end);
  assert.equal(Buffer.byteLength(body),part.bytes);assert.equal(sha(body),part.sha256);combined+=body;
 }
 assert.equal(combined,packageText('tests/actual-maintenance-bodies.inc'));
 assert.equal(sha(files['gdb/amd-dbgapi-owned-one-stop-v1.h']),x.owner_core.sha256);
 assert.equal(sha(files['gdbsupport/gdb_ref_ptr.h']),x.ref_ptr.sha256);
});
test('all 16 runtime edits invert to exact unchanged public v2 preimages',()=>{
 const t=JSON.parse(packageText('ordinary-transforms.json'));assert.equal(t.operations.length,16);
 const changed=spec.patches[0].changes;
 for(const row of changed){
  const name=row.path.slice(4),ops=t.operations.filter(op=>op.file===name);assert(ops.length>0);
  const parentPath=fileURLToPath(new URL('../../physical-v2/src/'+name,import.meta.url));
  const before=checkedText({...row.preimage,path:row.path},readBounded(parentPath,MAX_FILE));
  let after=before;for(const op of ops){assert.equal(after.split(op.before).length,2);after=after.replace(op.before,op.after);}
  assert.equal(after,files[row.path]);
  let reverse=after;for(const op of [...ops].reverse()){assert.equal(reverse.split(op.after).length,2);reverse=reverse.replace(op.after,op.before);}
  assert.equal(reverse,before);
 }
 assert(t.operations.every(op=>changed.some(r=>r.path==='gdb/'+op.file)));
});
test('actual source false gates and existing native caps are unchanged',()=>{
 falseGates(files);
 const header=files['gdb/amd-dbgapi-one-stop-native-v1.h'],core=files['gdb/amd-dbgapi-owned-one-stop-v1.h'];
 assert(header.includes('entry_maintenance_scalar_scratch = 256;'));
 assert(header.includes('target_ops_ref m_entry_process_ref,m_entry_top_ref;'));
 assert(header.includes('entry_epoch m_entry_epoch=entry_epoch::unused;'));
 const x=JSON.parse(packageText('tests/helper-extraction.json'));
 assert.equal(sha(core),x.owner_core.sha256); // Whole unchanged source, not guessed cap text.
});
test('actual lifetime context retains stack ownership across ordinary release',()=>{
 const target=files['gdb/target.c'],header=files['gdb/target.h'],ref=files['gdbsupport/gdb_ref_ptr.h'];
 assert(target.includes('target_ops_ref::new_reference'));assert(target.includes('target_stack::unpush'));
 assert(header.includes('target_ops_ref'));assert(header.includes('target_ops_ref_policy'));
 assert(ref.includes('new_reference'));assert(ref.includes('reset'));
 assert(files['gdbsupport/refcounted-object.h'].includes('decref'));
 assert(files['gdb/process-stratum-target.h'].includes('has_resumed_with_pending_wait_status'));
});
test('retirement stays monotonic and cannot release during the existing effect',()=>{
 const source=files['gdb/amd-dbgapi-one-stop-native-resume-v1.inc'];
 const retire=slice(source,'void native_adapter::retire_entry_maintenance () noexcept {','void native_adapter::infrun_begin');
 assert(retire.includes('m_entry_epoch=entry_epoch::retired'));
 assert(retire.includes('!m_entry_maintenance_inflight && !m_generic_commit'));
 assert(retire.includes('target_is_pushed (m_entry_process_ref.get ())'));
 assert(retire.includes('target_is_pushed (m_entry_top_ref.get ())'));
 assert(retire.includes('m_entry_top_ref.reset (nullptr);'));
 assert(retire.includes('m_entry_process_ref.reset (nullptr);'));
});

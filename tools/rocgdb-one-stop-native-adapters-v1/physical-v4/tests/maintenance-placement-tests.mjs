// SPDX-License-Identifier: GPL-3.0-or-later
// Source-only placement. Explicit final-false source; no debugger or native effect.
import test from 'node:test';
import assert from 'node:assert/strict';
import crypto from 'node:crypto';
import {fileURLToPath} from 'node:url';
import {finalSource,manifest,packageText,checkedText,readBounded,MAX_FILE,falseGates} from './source-files.mjs';
const files=finalSource(),spec=manifest();
const sha=b=>crypto.createHash('sha256').update(b).digest('hex');
function slice(source,start,end){assert.equal(source.split(start).length,2);assert.equal(source.split(end).length,2);const a=source.indexOf(start),b=source.indexOf(end);assert(b>a);return source.slice(a,b);}
const transforms=JSON.parse(packageText('ordinary-transforms.json'));
function prior(name,stage){
 let result=files['gdb/'+name];
 for(const op of [...transforms.operations].reverse()){
  if(op.file!==name || stage==='diagnostic'&&op.stage==='diagnostic')continue;
  assert.equal(result.split(op.after).length,2);result=result.replace(op.after,op.before);
 }
 return result;
}
test('40 runtime edits join and invert all five immutable v3 preimages',()=>{
 assert.equal(transforms.operations.length,40);assert.equal(transforms.operations.filter(x=>x.stage==='diagnostic').length,13);
 assert.equal(transforms.operations.filter(x=>x.stage==='loaded').length,21);
 assert.equal(transforms.operations.filter(x=>x.stage==='finish-event-context').length,5);
 for(const row of spec.patches[0].changes){
  const name=row.path.slice(4),p=name==='amd-dbgapi-owned-one-stop-v1.h'?'../../src/'+name:'../../physical-v3/src/'+name;
  const before=checkedText(row.preimage,readBounded(fileURLToPath(new URL(p,import.meta.url)),MAX_FILE));
  let after=before;
  for(const op of transforms.operations.filter(x=>x.file===name)){assert.equal(after.split(op.before).length,2);after=after.replace(op.before,op.after);}
  assert.equal(after,files[row.path]);assert.equal(prior(name,'v3'),before);
 }
 assert(transforms.operations.every(x=>spec.patches[0].changes.some(r=>r.path==='gdb/'+x.file)));
});
test('actual helper fixture extracts selected final and exact inverse diagnostic source',()=>{
 const x=JSON.parse(packageText('tests/helper-extraction.json'));let combined=x.prefix;
 for(const p of x.parts){
  const source=p.stage==='diagnostic'?prior(p.source.slice(4),'diagnostic'):files[p.source];
  let body=p.lines?source.split('\n').slice(p.lines[0]-1,p.lines[1]).join('\n')+'\n':slice(source,p.start,p.end);
  if(p.rename){assert.equal(body.split(p.rename.from).length,2);body=body.replace(p.rename.from,p.rename.to);}
  assert.equal(Buffer.byteLength(body),p.bytes);assert.equal(sha(body),p.sha256);combined+=body;
 }
 assert.equal(combined,packageText('tests/actual-maintenance-bodies.inc'));
 assert.equal(sha(files[x.owner_core.source]),x.owner_core.sha256);
 assert.equal(sha(files[x.ref_ptr.source]),x.ref_ptr.sha256);
 const scratch=packageText('tests/loaded-scratch.inc');
 assert(files['gdb/amd-dbgapi-one-stop-native-v1.h'].includes(scratch));
});
test('all three disabled gates and unchanged native budget declarations remain selected',()=>{
 falseGates(files);
 const h=files['gdb/amd-dbgapi-one-stop-native-v1.h'],o=files['gdb/amd-dbgapi-owned-one-stop-v1.h'];
 assert(h.includes('loaded_maintenance_scalar_scratch = sizeof (loaded_maintenance_scratch);'));
 assert(h.includes('+ entry_maintenance_scalar_scratch + loaded_maintenance_scalar_scratch'));
 assert(h.includes('sizeof (native_adapter) + 4096 + 1024 + 256 + sizeof (loaded_maintenance_scratch)'));
 const before=prior('amd-dbgapi-owned-one-stop-v1.h','v3');
 const limits=s=>{const a=s.indexOf('struct limits {'),b=s.indexOf('\n};',a);assert(a>=0&&b>a);return s.slice(a,b+3);};
 assert.equal(limits(o),limits(before));
});
test('first loaded ACK ordering retains original retire owner update and flush',()=>{
 const body=slice(files['gdb/amd-dbgapi-one-stop-native-events-v1.inc'],'void native_adapter::runtime_ack','void native_adapter::objects_ack');
 const expected=['prepare_loaded_maintenance (actual);','retire_entry_maintenance ();','m_owner.runtime_loaded','flush ();','arm_loaded_maintenance ();'];
 let at=-1;for(const marker of expected){const next=body.indexOf(marker,at+1);assert(next>at,marker);at=next;}
 assert.equal((body.match(/m_owner\.runtime_loaded/g)||[]).length,1);
 assert.equal((body.match(/flush \(\);/g)||[]).length,1);
});
test('selected strong target ownership remains source-bound and monotonic',()=>{
 const h=files['gdb/amd-dbgapi-one-stop-native-v1.h'],s=files['gdb/amd-dbgapi-one-stop-native-resume-v1.inc'];
 assert(h.includes('target_ops_ref m_loaded_process_ref,m_loaded_base_ref,m_loaded_top_ref;'));
 assert(h.includes('loaded_epoch m_loaded_epoch=loaded_epoch::unused;'));
 const retirement=slice(s,'void native_adapter::retire_loaded_maintenance () noexcept {','void native_adapter::infrun_begin');
 assert(retirement.includes('loaded_epoch::retired'));
 assert(s.includes('target_ops_ref::new_reference'));
 assert(files['gdb/target.c'].includes('target_stack::unpush'));
 assert(files['gdbsupport/gdb_ref_ptr.h'].includes('new_reference'));
 assert(files['gdbsupport/refcounted-object.h'].includes('decref'));
 assert(files['gdb/process-stratum-target.h'].includes('has_resumed_with_pending_wait_status'));
});
test('diagnostic tags append without changing first-poison owner predicates',()=>{
 const old=prior('amd-dbgapi-owned-one-stop-v1.h','diagnostic'),current=files['gdb/amd-dbgapi-owned-one-stop-v1.h'];
 const ops=transforms.operations.filter(x=>x.file==='amd-dbgapi-owned-one-stop-v1.h'&&x.stage==='loaded');
 assert.equal(ops.length,1);assert.equal(old.replace(ops[0].before,ops[0].after),current);
 for(const n of [15,16,17,18,19,20,21,22])assert(current.includes('= '+n));
});
test('public first-poison fixture binds exact old owner and two include-only edits',()=>{
 const x=JSON.parse(packageText('tests/helper-extraction.json'));let text=packageText('tests/first-poison-controls.cc');
 for(const op of [...x.first_poison.include_only_transforms].reverse()){assert.equal(text.split(op.after).length,2);text=text.replace(op.after,op.before);}
 assert.equal(Buffer.byteLength(text),x.first_poison.private_fixture.bytes);assert.equal(sha(text),x.first_poison.private_fixture.sha256);
 const old=readBounded(fileURLToPath(new URL('../../src/amd-dbgapi-owned-one-stop-v1.h',import.meta.url)),MAX_FILE);
 assert.equal(old.length,x.first_poison.old_owner.bytes);assert.equal(sha(old),x.first_poison.old_owner.sha256);
});

function debugTypeAnchor(header,scratch){
 const x=JSON.parse(packageText('tests/helper-extraction.json')).scratch;
 checkedText(x.current_header,Buffer.from(header));checkedText(x.extraction,Buffer.from(scratch));
 assert.equal(header.split(x.anchor.after).length,2,'one exact namespace-private debug-type anchor');
 const before=header.replace(x.anchor.after,()=>x.anchor.before);
 checkedText(x.prior_header,Buffer.from(before));
 assert.equal(before.split(x.anchor.before).length,2);assert.equal(before.replace(x.anchor.before,()=>x.anchor.after),header);
 assert.equal(slice(header,x.start,x.end),scratch,'complete scratch and anchor extraction');
 assert.equal(header.slice(header.indexOf(x.end)),before.slice(before.indexOf(x.end)),'native_adapter suffix unchanged');
 assert.equal((header.match(/loaded_maintenance_debug_type\s*\(/g)||[]).length,1,'definition only, no source call');
 assert.equal(Buffer.byteLength(header)-Buffer.byteLength(before),213);
 return before;
}
test('debug-type anchor is exact noncalled const identity and whole CPU extraction',()=>{
 const header=files['gdb/amd-dbgapi-one-stop-native-v1.h'],scratch=packageText('tests/loaded-scratch.inc');
 const before=debugTypeAnchor(header,scratch),x=JSON.parse(packageText('tests/helper-extraction.json')).scratch;
 assert.equal(x.anchor.bytes,213);assert.equal(x.anchor.calls_added,0);assert.equal(x.anchor.objects_added,0);
 assert.equal(x.actual_gdb_layout,null);assert.equal(x.native_authority,false);
 assert(header.includes('[[gnu::used]] static const loaded_maintenance_scratch *\nloaded_maintenance_debug_type (const loaded_maintenance_scratch *p) noexcept\n{ return p; }\n'));
 assert.equal(sha(before),x.prior_header.sha256);
});
test('missing duplicated mutable or changed-return debug anchor refuses',()=>{
 const header=files['gdb/amd-dbgapi-one-stop-native-v1.h'],scratch=packageText('tests/loaded-scratch.inc'),x=JSON.parse(packageText('tests/helper-extraction.json')).scratch;
 for(const replacement of [x.anchor.before,x.anchor.after+x.anchor.after,x.anchor.after.replace('static const loaded_maintenance_scratch *','static loaded_maintenance_scratch *'),x.anchor.after.replace('return p;','return nullptr;')]){
  const bad=header.replace(x.anchor.after,()=>replacement);assert.notEqual(bad,header);assert.throws(()=>debugTypeAnchor(bad,scratch));
 }
});
test('old truncated foreign or extended CPU scratch cannot bypass anchor binding',()=>{
 const header=files['gdb/amd-dbgapi-one-stop-native-v1.h'],scratch=packageText('tests/loaded-scratch.inc');
 for(const bad of [scratch.slice(1),scratch+'\n',scratch.replace('return p;','return nullptr;'),scratch.slice(0,scratch.indexOf('// Debug-type observability'))]){
  assert.notEqual(bad,scratch);assert.throws(()=>debugTypeAnchor(header,bad));
 }
});
test('anchor keeps all34 prior transforms before five additive context edits',()=>{
 const anchors=transforms.operations.filter(x=>x.stage==='observability');assert.equal(anchors.length,1);
 assert.equal(transforms.operations[34],anchors[0]);assert.equal(anchors[0].file,'amd-dbgapi-one-stop-native-v1.h');assert.equal(anchors[0].count,1);
 assert.deepEqual(transforms.operations.slice(0,34).map(x=>x.stage),[...Array(13).fill('diagnostic'),...Array(21).fill('loaded')]);
 assert.deepEqual(transforms.operations.slice(35).map(x=>x.stage),Array(5).fill('finish-event-context'));
 const expected=JSON.parse(packageText('tests/finish-event-context-extraction.json')).unchanged_first35_transforms_sha256;
 assert.equal(sha(JSON.stringify(transforms.operations.slice(0,35))),expected);
 const x=JSON.parse(packageText('tests/helper-extraction.json')).scratch;assert.equal(anchors[0].before,x.anchor.before);assert.equal(anchors[0].after,x.anchor.after);
 falseGates(files);for(const key of ['activation_available','capture_available','publication_available','source_checks_are_native_authority'])assert.equal(spec[key],false);
});

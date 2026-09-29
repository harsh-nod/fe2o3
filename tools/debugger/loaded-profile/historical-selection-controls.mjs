import {readLoadedReviewFixtures} from './loaded-fixture-reader.mjs';
import test from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import crypto from 'node:crypto';
import {BINDING} from './loaded-selection-binding.mjs';
import {PROFILE} from './loaded-profile-binding.mjs';
import {reviewLoadedStartup} from './loaded-profile.mjs';
import {planLoadedSelection,planUnqualifiedLoadedSelection} from './loaded-selection.mjs';
const {profile_records,selector_records}=readLoadedReviewFixtures();
const by=new Map(profile_records.map(r=>[r.role,r.bytes])),sb=new Map(selector_records.map(r=>[r.role,r.bytes]));
const text=sb.get('selector-data').toString('utf8'),selector=JSON.parse(text.slice(BINDING.data_prefix.length,-BINDING.data_suffix.length));
const gate=JSON.parse(by.get(PROFILE.valueRoles.gate)),request=JSON.parse(by.get(PROFILE.valueRoles.gateRequest));
const profile=reviewLoadedStartup(profile_records);
const fixture={selector,prior:request.inputs,before:gate.inputs_before,after:gate.inputs_after,profile};
const clone=()=>structuredClone(fixture),encode=v=>Buffer.from(JSON.stringify(v)),run=v=>planUnqualifiedLoadedSelection(encode(v));
const overlap=v=>v.profile.rows.find(r=>v.prior.some(p=>p.path===r.observation.path));
const absent=v=>v.profile.rows.find(r=>!r.observation.exists);
const fresh=v=>v.profile.rows.find(r=>r.observation.exists&&!v.prior.some(p=>p.path===r.observation.path));
test('qualified complete historical inputs retain 879 duties and all 1217 named role labels',()=>{
 const out=planLoadedSelection({profile_records,selector_records});
 assert.equal(out.qualified_retained_input_bytes,true);
 assert.equal(out.duties.length,879);assert.equal(out.extra_roles.length,453);assert.equal(out.historical_source_transfers.length,141);
 assert.deepEqual(out.prior_input_roles.map(r=>r.pin),request.inputs);
 for(let i=0;i<selector.duties.length;i++){
  const a=selector.duties[i],b=out.duties[i];assert.equal(b.original_index,a.original_index);assert.deepEqual(b.old_pin,a.old_pin);assert.deepEqual(b.roles,a.roles);assert.equal(b.method,a.method);assert.equal(b.targets.length,a.targets.length);
  for(const t of b.targets)assert.deepEqual(t.pin,request.inputs.find(p=>p.path===t.path));
 }
 assert.equal(out.entries.length,1173);assert.equal(out.entries.filter(r=>r.kind==='readable').length,1150);
 assert.equal(out.entries.reduce((n,r)=>n+r.prior_input_roles.length,0),1024);assert.equal(out.entries.reduce((n,r)=>n+r.loaded_named_roles.length,0),193);
 assert.equal(out.counts.independent_prior_plus_loaded_labels,1217);
 assert.equal(out.named_alias_roles.length,3);assert.equal(out.runtime_slots.length,45);
 for(const index of [770,771,772,773]){const r=out.duties.find(x=>x.original_index===index);assert.equal(r.targets.length,2);assert.ok(r.targets.some(t=>eqPin(t.pin,r.old_pin)));}
 assert.equal(out.failed_attempt.full_gate_passed,false);
 for(const key of ['execution_authority','physical_observation_performed','successor_graph_active','runtime_selection','native_acceptance','root_cap_change_approved'])assert.equal(out[key],false);
 assert.equal(out.lease,null);assert.equal(out.read_plan.final_operational_named_count,null);
});
function eqPin(a,b){return a.path===b.path&&a.bytes===b.bytes&&a.sha256===b.sha256;}
test('absence observations remain metadata roles, not empty readable files',()=>{
 const out=run(clone()),abs=out.entries.filter(r=>r.kind==='absence-observation');
 assert.equal(abs.length,23);for(const r of abs){assert.equal(r.pin,null);assert.equal(r.identity,null);assert.equal(r.prior_input_roles.length,0);assert.equal(r.loaded_named_roles.length,1);}
 assert.equal(out.read_plan.absence_content_reads,0);assert.equal(out.read_plan.absence_checks_per_pass,23);
});
test('one pass has checked payload, EOF and exact-or-refuse chunk call admissions',()=>{
 const out=run(clone()),p=out.read_plan;
 assert.deepEqual(p.one_pass,{readable_paths:1150,payload_bytes:1817924086,eof_reserved_bytes:1150,reserved_bytes:1817925236,read_call_cap:29748});
 assert.equal(p.prior.payload_bytes,1798580929);assert.equal(p.new_present.payload_bytes,19343157);
 assert.equal(p.prior.readable_paths,1024);assert.equal(p.new_present.readable_paths,126);
 assert.equal(p.two_pass.payload_bytes,3635848172);assert.equal(p.two_pass.reserved_bytes,3635850472);assert.equal(p.two_pass.read_call_cap,59496);
 assert.equal(p.inner_512MiB_artifact_cap_unchanged,true);assert.equal(p.final_integration_closure_complete,false);
 assert.equal(p.metadata_syscalls_metered,false);assert.equal(p.actual_module_loader_io_metered,false);
});
test('named aliases and independently named canonical targets are not merged',()=>{
 const out=run(clone());
 for(const a of out.named_alias_roles){assert.ok(out.entries.some(r=>r.path===a.path));if(out.prior_input_roles.some(r=>r.pin.path===a.resolved))assert.ok(out.entries.some(r=>r.path===a.resolved&&r.path!==a.path));}
 const s=out.named_alias_roles.find(r=>r.path.endsWith('/sitecustomize.py'));assert.equal(s.loaded_named_roles.length,1);assert.equal(s.prior_input_roles.length,1);
 assert.equal(out.entries.some(r=>r.path===s.resolved),false);
});
test('synthetic output is deterministic, immutable, unqualified and inactive',()=>{
 const v=clone(),a=run(v),b=run(clone());assert.deepEqual(a,b);assert.equal(a.qualified_retained_input_bytes,false);
 const saved=JSON.stringify(a);v.prior.length=0;v.profile.rows.length=0;assert.equal(JSON.stringify(a),saved);
 assert.throws(()=>a.duties.pop());assert.ok(Object.isFrozen(a.entries[0].duty_targets));
});
const mutations=[
 ['missing prior pin',v=>v.prior.pop()],
 ['extra prior pin',v=>v.prior.push(v.prior[0])],
 ['duplicate prior named path',v=>{v.prior[1]=v.prior[0];v.before[1]=v.before[0];v.after[1]=v.after[0];}],
 ['prior identity drift',v=>v.after[0].identity[1]='999'],
 ['prior observed content mismatch',v=>{v.before[0].sha256='0'.repeat(64);v.after[0].sha256='0'.repeat(64);}],
 ['prior size overflow',v=>v.prior[0].bytes=Number.MAX_SAFE_INTEGER],
 ['prior identity short',v=>{v.before[0].identity.pop();v.after[0].identity.pop();}],
 ['prior nonregular mode',v=>{v.before[0].identity[3]='16877';v.after[0].identity[3]='16877';}],
 ['prior named dot path',v=>v.prior[0].path='/tmp/../bad'],
 ['lost original inventory',v=>v.selector.original.pop()],
 ['original index reorder',v=>v.selector.original[0].index=1],
 ['lost original transfer',v=>v.selector.transfers.pop()],
 ['duplicate transfer',v=>v.selector.transfers[1]=v.selector.transfers[0]],
 ['transfer old pin changed',v=>v.selector.transfers[0].old_pin.sha256='0'.repeat(64)],
 ['lost retained duty',v=>v.selector.duties.pop()],
 ['duplicate retained duty',v=>v.selector.duties[1]=v.selector.duties[0]],
 ['transfer reintroduced as retained',v=>v.selector.duties[0].original_index=v.selector.transfers[0].original_index],
 ['old duty identity changed',v=>v.selector.duties[0].old_pin.sha256='0'.repeat(64)],
 ['empty duty targets',v=>v.selector.duties[0].targets=[]],
 ['target omitted from original selection',v=>v.selector.duties[0].targets[0].path='/missing/closed-role'],
 ['known target pin changed',v=>v.selector.duties[0].targets[0].pin.sha256='0'.repeat(64)],
 ['inherited cap lowered below existing size',v=>v.selector.duties[0].targets[0].cap=0],
 ['extra role omitted',v=>v.selector.extras.pop()],
 ['runtime slot omitted',v=>v.selector.runtime_slots.pop()],
 ['runtime slot reordered',v=>v.selector.runtime_slots[0].slot=1],
 ['runtime path omitted',v=>v.selector.runtime_slots[0].path='/missing/runtime-role'],
 ['nonce role changed',v=>v.selector.benign_modes[0]='other'],
 ['actual nonce falsely historical',v=>v.selector.historical_nonces.push(BINDING.benign_nonces.normal)],
 ['selector count widened',v=>v.selector.expected_paths=1024],
 ['materializer own role changed',v=>{const x=BINDING.materializer_own_pins[0],i=v.prior.findIndex(p=>p.path===x.path);v.prior[i].sha256='0'.repeat(64);v.before[i].sha256='0'.repeat(64);v.after[i].sha256='0'.repeat(64);}],
 ['loaded role omitted',v=>v.profile.rows.pop()],
 ['loaded role duplicated',v=>v.profile.rows[1]=v.profile.rows[0]],
 ['loaded same-path hash conflict',v=>overlap(v).observation.sha256='0'.repeat(64)],
 ['loaded same-path size conflict',v=>overlap(v).observation.bytes++],
 ['loaded same-path inode conflict',v=>overlap(v).observation.ino='999'],
 ['loaded same-path timestamp conflict',v=>overlap(v).observation.mtime_ns='999'],
 ['loaded same-path realpath conflict',v=>overlap(v).observation.realpath='/some/other/named-target'],
 ['same-path absence conflicts with existing readable role',v=>{const r=overlap(v),p=r.observation.path;r.observation={path:p,realpath:p,exists:false};}],
 ['absent role given fake file bytes',v=>absent(v).observation.bytes=0],
 ['absent role given fake digest',v=>absent(v).observation.sha256='0'.repeat(64)],
 ['absent alias invented',v=>absent(v).observation.realpath='/invented/alias-target'],
 ['absent non-cache role',v=>absent(v).references[0].field='file'],
 ['unreferenced loaded role',v=>fresh(v).references=[]],
 ['cache execution falsely inferred',v=>fresh(v).cache_execution_provenance=true],
 ['new file bytes beyond original loaded cap',v=>fresh(v).observation.bytes=256*1024*1024+1],
 ['new alias invented',v=>fresh(v).observation.realpath='/invented/new-alias'],
 ['profile became live',v=>v.profile.live_activation=true],
 ['profile old duties changed',v=>v.profile.historical_duties_changed=true],
 ['profile physical reread invented',v=>v.profile.physical_files_reread=true],
 ['profile census silently narrowed',v=>v.profile.counts.files--],
 ['profile runtime acceptance promoted',v=>v.profile.limitations.runtime_acceptance=true],
 ['profile import completeness promoted',v=>v.profile.limitations.complete_import_history=true],
 ['profile cache execution promoted',v=>v.profile.limitations.cache_execution_provenance=true],
 ['profile physical capture promoted',v=>v.profile.limitations.physical_capture=true],
 ['profile authority invented',v=>v.profile.source_authority='root'],
 ['unknown input field',v=>v.extra=true],
];
for(const index of [770,771,772,773])mutations.push(['mixed historical pair '+index+' removed',v=>v.selector.duties.find(r=>r.original_index===index).targets.pop()]);
for(const [name,mutate]of mutations)test('refuses '+name,()=>{const v=clone();mutate(v);assert.throws(()=>run(v));});
for(let i=0;i<BINDING.selector_records.length;i++)test('whole selector input '+i+' pin refuses mutation',()=>{
 const xs=selector_records.map(r=>({role:r.role,bytes:Buffer.from(r.bytes)}));xs[i].bytes[0]^=1;
 assert.throws(()=>planLoadedSelection({profile_records,selector_records:xs}),/whole input pin/);
});
for(const [name,change]of [
 ['missing selector record',x=>x.selector_records.pop()],
 ['reordered selector records',x=>[x.selector_records[0],x.selector_records[1]]=[x.selector_records[1],x.selector_records[0]]],
 ['extra selector record',x=>x.selector_records.push(x.selector_records[0])],
 ['missing profile record',x=>x.profile_records.pop()],
 ['wrong selector Buffer length',x=>x.selector_records[0]={role:x.selector_records[0].role,bytes:Buffer.alloc(0)}],
 ['unclosed input',x=>x.extra=1],
])test('qualified entry refuses '+name,()=>{
 const input={profile_records:[...profile_records],selector_records:[...selector_records]};change(input);assert.throws(()=>planLoadedSelection(input));
});
test('bounded parser rejects non-Buffer, invalid UTF8, huge or malformed JSON',()=>{
 for(const x of [null,{},Buffer.alloc(0),Buffer.from([0xff]),Buffer.from('{'),Buffer.alloc(32*1024*1024+1)])assert.throws(()=>planUnqualifiedLoadedSelection(x));
});
test('bounded parser rejects excessive depth and prototype keys',()=>{
 let v={};for(let i=0;i<66;i++)v={next:v};assert.throws(()=>planUnqualifiedLoadedSelection(encode(v)));
 assert.throws(()=>planUnqualifiedLoadedSelection(Buffer.from('{"__proto__":{}}')));
});

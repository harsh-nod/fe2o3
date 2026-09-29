import {readLoadedReviewFixtures} from '../loaded-profile/loaded-fixture-reader.mjs';
import test from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import crypto from 'node:crypto';
import {PROFILE} from '../loaded-profile/loaded-profile-binding.mjs';
import {BINDING} from '../loaded-profile/loaded-selection-binding.mjs';
import {buildHistoricalReadProtocol} from './reader-plan.mjs';
import {EXACT_ALIASES} from './reader-aliases.mjs';
import {filesystemProvider} from './reader-fs.mjs';
const {profile_records,selector_records}=readLoadedReviewFixtures();
const prepared=buildHistoricalReadProtocol({profile_records,selector_records});
test('complete historical planner bridge preserves every original and loaded label',()=>{
 const {protocol:p,historical_plan:h}=prepared;assert.equal(p.entries.length,1173);assert.equal(h.duties.length,879);assert.equal(h.loaded_named_roles.length,193);
 assert.equal(p.entries.reduce((n,r)=>n+r.labels.prior.length,0),1024);assert.equal(p.entries.reduce((n,r)=>n+r.labels.loaded.length,0),193);
 for(let i=0;i<p.entries.length;i++){
  const a=p.entries[i],b=h.entries[i];assert.deepEqual(a.pin,b.pin);assert.deepEqual(a.identity,b.identity);
  assert.deepEqual(a.labels.duties,b.duty_targets.map(x=>'duty:'+x.original_index+':target:'+x.target_index));
  assert.deepEqual(a.labels.extras,b.extra_roles.map(x=>'extra:'+x.index));
 }
 assert.equal(h.extra_roles.length,453);assert.equal(h.runtime_slots.length,45);assert.equal(h.original_materializer_roles.length,3);
 for(const index of [770,771,772,773])assert.equal(h.duties.find(r=>r.original_index===index).targets.length,2);
});
test('every inherited duty/member cap and loaded ownership survives the bridge',()=>{
 const {protocol:p,historical_plan:h}=prepared;
 for(let i=0;i<p.entries.length;i++){
  const a=p.entries[i],b=h.entries[i];
  if(a.kind==='readable'){
   assert.ok(a.pin.bytes<=a.cap);for(const x of b.duty_targets.concat(b.extra_roles))assert.ok(a.cap<=x.cap);
   if(b.loaded_named_roles.length){assert.ok(a.cap<=256*1024*1024);for(const j of b.loaded_named_roles){const f=h.loaded_named_roles[j].observation;assert.deepEqual(a.ownership,{uid:String(f.uid),gid:String(f.gid)});}}
  }else{assert.equal(a.cap,0);assert.equal(a.pin,null);assert.equal(a.ownership,null);assert.equal(a.identity,null);}
 }
});
test('source-bound three alias rules are exact and keep independent target dispositions',()=>{
 assert.equal(EXACT_ALIASES.length,3);assert.deepEqual(EXACT_ALIASES.map(r=>r.link_text),['rustup','codex','/etc/python3.12/sitecustomize.py']);
 const names=new Set(prepared.protocol.entries.map(r=>r.path));
 for(const a of EXACT_ALIASES){assert.equal(names.has(a.resolved),a.target_selected);assert.ok(names.has(a.pin.path));}
 assert.equal(names.has('/etc/python3.12/sitecustomize.py'),false);
});
test('independent BigInt complete budget includes metadata but excludes zero-file absence content',()=>{
 const p=prepared.protocol;let payload=0n,content=0n,metadata=0n,readable=0n,absent=0n;
 for(const r of p.entries){if(r.kind==='readable'){readable++;payload+=BigInt(r.pin.bytes);content+=(BigInt(r.pin.bytes)+65535n)/65536n+1n;metadata+=p.aliases.some(a=>a.pin.path===r.path)?19n:10n;}else{absent++;const prefixes=BigInt(r.path.split('/').length);metadata+=2n*(2n*prefixes+1n);}}
 assert.equal(readable,1150n);assert.equal(absent,23n);assert.equal(payload,1817924086n);assert.equal(content,29748n);
 assert.equal(BigInt(p.budget.total.reserved_bytes),2n*(payload+readable));assert.equal(BigInt(p.budget.total.content_calls),2n*content);assert.equal(BigInt(p.budget.total.metadata_calls),2n*metadata);
 assert.equal(p.budget.scratch_bytes,65537);assert.equal(p.passes,2);
});
test('qualified historical bytes never approve complete operational scope or cap',()=>{
 assert.equal(prepared.protocol.custody.qualified_historical_input_bytes,true);
 for(const k of ['operational_roster_complete','root_cap_change_approved','execution_authority'])assert.equal(prepared.protocol.custody[k],false);
 assert.equal(prepared.complete_operational_count,null);assert.equal(prepared.complete_operational_budget,null);assert.equal(prepared.live_activation,false);
 assert.ok(Object.isFrozen(prepared.protocol.entries[0].labels));assert.equal(prepared.historical_plan.read_plan.final_integration_closure_complete,false);
});
for(let i=0;i<4;i++)test('bridge cannot replace qualified selector Buffer '+i,()=>{
 const changed=selector_records.map(r=>({role:r.role,bytes:Buffer.from(r.bytes)}));changed[i].bytes[0]^=1;
 assert.throws(()=>buildHistoricalReadProtocol({profile_records,selector_records:changed}));
});
test('filesystem provider construction does not itself issue a read or create a CLI',()=>{
 const provider=filesystemProvider();assert.deepEqual(Object.keys(provider),['lstat','realpath','readlink','open','fstat','read','close']);assert.ok(Object.isFrozen(provider));
});

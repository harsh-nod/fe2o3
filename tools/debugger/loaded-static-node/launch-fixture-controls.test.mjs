// Pure preparation controls only. Fake source identities are never filesystem observations.
import test from 'node:test';
import assert from 'node:assert/strict';
import {buildSyntheticLaunchRequest,bindSyntheticBootstrap,verifySyntheticLaunchResult} from './launch-fixture.mjs';
import {staticModuleContext} from './launch-module-map.mjs';
import {OUTPUT_ROLES,sha} from './launch-model.mjs';
const json=v=>Buffer.from(JSON.stringify(v)),id=(n,i)=>['1',String(i),String(n),'33152','1','1'],ownership={uid:'10',gid:'10'};
const policy=()=>({schema:'fe2o3-cpu-reader-guard-policy-v1',not_before_utc_ms:1000,expires_utc_ms:100000,max_elapsed_ms:1000,max_rss_bytes:536870912,min_free_bytes:1,min_available_ram_bytes:1,resource_interval_ms:1000,max_resource_probes:4});
function sample(){
 const c=staticModuleContext('/synthetic/node'),paths=[c.runtime_path,...c.module_paths];
 const source_claims=paths.map((path,i)=>({id:'src:'+i,path,kind:'readable',pin:{path,bytes:1,sha256:sha(Buffer.from('s'))},resolved:path,identity:id(1,i+1),ownership,roles:['source:'+i]}));
 const directory='/fresh/inputs',row=(name,n,i)=>{const path=directory+'/'+name;return {path,kind:'readable',pin:{path,bytes:n,sha256:sha(Buffer.alloc(n,42))},resolved:path,identity:id(n,i),ownership,labels:{prior:[],loaded:[],duties:['fixture:'+name],extras:[]},cap:n};};
 const target=row('target.bin',65537,100),alias={...structuredClone(target),path:directory+'/alias.bin',pin:{...target.pin,path:directory+'/alias.bin'},labels:{prior:[],loaded:[],duties:['fixture:alias'],extras:[]}};
 const empty=row('empty.bin',0,101),absent={path:directory+'/absent.bin',kind:'absence-observation',pin:null,resolved:directory+'/absent.bin',identity:null,ownership:null,labels:{prior:[],loaded:[],duties:['fixture:absence'],extras:[]},cap:0};
 return {schema:'fe2o3-static-launch-synthetic-input-v1',label:'synthetic-launch',runtime_path:c.runtime_path,source_claims,fixture_directory:directory,fixture_entries:[target,alias,empty,absent],aliases:[{pin:alias.pin,resolved:target.path,link_text:'target.bin',target_pin:target.pin,target_selected:true}],outputs:OUTPUT_ROLES.map((role,i)=>({role,path:'/fresh/output/'+['pending','final','receipt','stdout','stderr','readback'][i],cap_bytes:1048576})),output_directory_identity:['1','200','0','16832','1','1'],resource_root:'/fresh',adapter_policy:policy(),bootstrap_policy:policy(),report_policy:policy(),external_terminal_pins:['/root/closure','/root/scope'].map(path=>({path,bytes:1,sha256:sha(Buffer.from('t'))})),phase_milliseconds:{precheck:1000,historical:1000,postcheck:1000}};
}
function bound(q=sample()){
 const built=buildSyntheticLaunchRequest(json(q)),request=built.request_bytes,binding={request:{pin:{path:'/fresh/request',bytes:request.length,sha256:sha(request)},identity:id(request.length,500),ownership},summary_descriptor:{fd:1,kind:'file',identity:id(0,501),ownership}};
 const bytes=bindSyntheticBootstrap(request,built.bootstrap_template_bytes,json(binding));return{q,built,binding,bytes};
}
test('fresh fixture builder binds the exact static sources and four distinct synthetic roles without IO',()=>{
 const {built,bytes}=bound(),boot=JSON.parse(bytes),expected=JSON.parse(built.expected_bytes);
 assert.equal(built.actual_io_performed,false);assert.equal(built.qualified,false);assert.equal(expected.fixture_paths.length,4);
 assert.equal(expected.historical_budget.total.completed_passes,undefined);assert.equal(expected.historical_budget.passes,2);
 assert.equal(expected.historical_budget.one_pass.readable_paths,3);assert.equal(expected.historical_budget.one_pass.absence_observations,1);
 assert.equal(boot.named_cap,built.context.module_paths.length+5);assert.equal(boot.outputs.length,6);
});
test('fresh fixture builder refuses missing or extra static source paths',()=>{
 for(const mutate of [q=>q.source_claims.pop(),q=>q.source_claims[0].path='/unselected/runtime',q=>q.source_claims.push(structuredClone(q.source_claims[0]))]){const q=sample();mutate(q);assert.throws(()=>buildSyntheticLaunchRequest(json(q)));}
});
test('fixed four fresh names cannot be replaced by a historical path',()=>{
 const q=sample();q.fixture_entries[0].path='/historical/kernel';assert.throws(()=>buildSyntheticLaunchRequest(json(q)),/fixed fresh fixture names/);
});
test('synthetic alias policy and independently selected target remain exact',()=>{
 const q=sample();q.aliases[0].target_selected=false;assert.throws(()=>buildSyntheticLaunchRequest(json(q)),/one exact relative alias/);
});
test('observed empty file cannot replace the required bounded absence',()=>{
 const q=sample();q.fixture_entries[3]={...structuredClone(q.fixture_entries[2]),path:q.fixture_directory+'/absent.bin'};assert.throws(()=>buildSyntheticLaunchRequest(json(q)),/bounded absence/);
});
test('bootstrap binding requires the externally observed complete request pin',()=>{
 const {built,binding}=bound();binding.request.pin.sha256='0'.repeat(64);assert.throws(()=>bindSyntheticBootstrap(built.request_bytes,built.bootstrap_template_bytes,json(binding)),/root-observed request whole pin/);
});
test('synthetic readback never accepts a process failure as successful publication',()=>{
 const {built}=bound();assert.throws(()=>verifySyntheticLaunchResult(json({}),json({}),json({}),built.expected_bytes,1),/external process exit/);
});
test('synthetic readback never treats command intent without complete evidence as success',()=>{
 const {built}=bound();assert.throws(()=>verifySyntheticLaunchResult(json({schema:'fe2o3-static-node-command-observation-v1'}),json({}),json({}),built.expected_bytes,0),/complete successful command intent/);
});

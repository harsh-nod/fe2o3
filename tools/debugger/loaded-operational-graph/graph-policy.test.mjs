import test from 'node:test';
import assert from 'node:assert/strict';
import {compileOperationalGraph,decodeGraphJSON,checkedAdd,GRAPH_LIMITS} from './loaded-operational-graph.mjs';
import {exactReadEnvelope,proposeResourcePolicy} from './loaded-operational-policy.mjs';
const raw=x=>Buffer.from(JSON.stringify(x));
const claim=(id='a',path='/data/a',bytes=3)=>({id,path,kind:'readable',pin:{path,bytes,sha256:'a'.repeat(64)},resolved:path,identity:['1','2',String(bytes),'33188','3','4'],ownership:{uid:'5',gid:'6'},roles:['role:'+id]});
const input=()=>({schema:'fe2o3-loaded-operational-graph-input-v1',claims:[claim()],aliases:[],imports:[],unresolved:[],outputs:[]});
test('one explicit file remains inactive and individually charged',()=>{const g=compileOperationalGraph(raw(input()));assert.equal(g.counts.named,1);assert.equal(g.counts.payload_bytes,3);for(const k of ['complete_operational_graph','qualified','execution_authority','root_cap_change_approved','native_authority','actual_paths_observed'])assert.equal(g[k],false);});
test('equal same-name claims retain both roles without two content names',()=>{const x=input();x.claims.push(claim('b'));const g=compileOperationalGraph(raw(x));assert.equal(g.counts.named,1);assert.equal(g.counts.claims,2);assert.deepEqual(g.entries[0].roles,['role:a','role:b']);});
test('identical payload under separate names pays twice',()=>{const x=input();x.claims.push(claim('b','/data/b'));const g=compileOperationalGraph(raw(x));assert.equal(g.counts.named,2);assert.equal(g.counts.payload_bytes,6);});
test('caller JSON is copied and output recursively frozen',()=>{const b=raw(input()),g=compileOperationalGraph(b);b.fill(0);assert.equal(g.entries[0].pin.bytes,3);assert.throws(()=>g.entries[0].roles.push('changed'));});
test('only full bounded UTF8 Buffers admitted',()=>{for(const b of ['',{},Buffer.alloc(0),Buffer.from([0xff]),Buffer.alloc(GRAPH_LIMITS.json_bytes+1)])assert.throws(()=>decodeGraphJSON(b));});
for(const [name,edit]of [
 ['unknown key',x=>x.surprise=true],['wrong schema',x=>x.schema='other'],['duplicate claim ID',x=>x.claims.push(claim())],
 ['pin conflict',x=>{x.claims.push(claim('b'));x.claims[1].pin.sha256='b'.repeat(64);}],
 ['identity conflict',x=>{x.claims.push(claim('b'));x.claims[1].identity[1]='99';}],
 ['owner conflict',x=>{x.claims.push(claim('b'));x.claims[1].ownership.uid='99';}],
 ['resolution conflict',x=>{x.claims.push(claim('b'));x.claims[1].resolved='/data/other';}],
 ['noncanonical name',x=>x.claims[0].path='/data/../a'],
 ['relative name',x=>x.claims[0].path='data/a'],
 ['oversized member',x=>x.claims[0].pin.bytes=GRAPH_LIMITS.member_bytes+1],
 ['negative member',x=>x.claims[0].pin.bytes=-1],
 ['bad pin digest',x=>x.claims[0].pin.sha256='a'.repeat(63)],
 ['wrong identity size',x=>x.claims[0].identity[2]='4'],
 ['nonregular mode',x=>x.claims[0].identity[3]='41471'],
 ['duplicate role in claim',x=>x.claims[0].roles.push('role:a')],
 ['same role on another name',x=>{x.claims.push(claim('b','/data/b'));x.claims[1].roles=['role:a'];}],
 ['undeclared alias',x=>x.claims[0].resolved='/data/target'],
 ['unknown import endpoint',x=>x.imports=[{from:'/data/a',specifier:'./b',to:'/data/b'}]],
 ['escaping import mapping',x=>{x.claims.push(claim('b','/data/b'));x.imports=[{from:'/data/a',specifier:'./c',to:'/data/b'}];}],
 ['unbounded unresolved text',x=>x.unresolved=[{role:'policy',stage:'before-read',reason:'a'.repeat(4097)}]],
 ['unknown unresolved stage',x=>x.unresolved=[{role:'policy',stage:'during-read',reason:'missing'}]],
 ['output aliases input',x=>x.outputs=[{role:'receipt',path:'/data/a',cap_bytes:1}]],
 ['duplicate output role',x=>x.outputs=[{role:'receipt',path:null,cap_bytes:1},{role:'receipt',path:null,cap_bytes:1}]],
 ['zero output cap',x=>x.outputs=[{role:'receipt',path:null,cap_bytes:0}]]
])test('refuses '+name,()=>{const x=input();edit(x);assert.throws(()=>compileOperationalGraph(raw(x)));});
test('unknown identity remains an explicit unresolved slot',()=>{const x=input();x.claims[0].identity=null;assert.equal(compileOperationalGraph(raw(x)).counts.identity_slots_unresolved,1);});
test('a consistent later claim may define a formerly unknown identity',()=>{const x=input();x.claims[0].identity=null;x.claims.push(claim('b'));assert.equal(compileOperationalGraph(raw(x)).counts.identity_slots_unresolved,0);});
test('absence is not a zero-byte readable file',()=>{const x=input();x.claims.push({id:'missing',path:'/data/missing',kind:'absence-observation',pin:null,resolved:'/data/missing',identity:null,ownership:null,roles:['absence:0']});const g=compileOperationalGraph(raw(x));assert.equal(g.counts.absence,1);assert.equal(g.counts.readable,1);const e=exactReadEnvelope(g.entries,[],2);assert.equal(e.total.reserved_bytes,8);assert.equal(e.total.content_calls,4);assert.equal(e.total.absence,2);});
test('absence carrying a file pin is rejected',()=>{const x=input();x.claims[0].kind='absence-observation';assert.throws(()=>compileOperationalGraph(raw(x)));});
test('alias target has an independent named charge',()=>{const x=input();x.claims.push(claim('target','/data/target'));x.claims[0].resolved='/data/target';x.aliases=[{pin:x.claims[0].pin,resolved:'/data/target',link_text:'target',target_pin:x.claims[1].pin,target_selected:true}];const g=compileOperationalGraph(raw(x));assert.equal(g.counts.readable,2);assert.equal(exactReadEnvelope(g.entries,g.aliases,1).one_pass.metadata_provider_calls,29);});
test('false independent target declaration refuses alias',()=>{const x=input();x.claims[0].resolved='/data/target';x.aliases=[{pin:x.claims[0].pin,resolved:'/data/target',link_text:'target',target_pin:{...x.claims[0].pin,path:'/data/target'},target_selected:true}];assert.throws(()=>compileOperationalGraph(raw(x)));});
test('builtin import endpoint binds a named runtime binary',()=>{const x=input();x.claims.push(claim('node','/tools/node'));x.imports=[{from:'/data/a',specifier:'node:crypto',to:'/tools/node'}];assert.equal(compileOperationalGraph(raw(x)).counts.imports,1);});
test('exact relative imports are retained as edges',()=>{const x=input();x.claims.push(claim('b','/data/b'));x.imports=[{from:'/data/a',specifier:'./b',to:'/data/b'}];assert.deepEqual(compileOperationalGraph(raw(x)).imports,x.imports);});
test('exact absolute source import is retained',()=>{const x=input();x.claims.push(claim('b','/data/b'));x.imports=[{from:'/data/a',specifier:'/data/b',to:'/data/b'}];assert.equal(compileOperationalGraph(raw(x)).counts.imports,1);});
test('duplicate import edges are not silently erased',()=>{const x=input();x.imports=[{from:'/data/a',specifier:'node:fs',to:'/data/a'},{from:'/data/a',specifier:'node:fs',to:'/data/a'}];assert.throws(()=>compileOperationalGraph(raw(x)));});
test('64KiB chunks include a separately charged EOF',()=>{for(const [bytes,calls]of [[0,1],[1,2],[65536,2],[65537,3]]){const e=exactReadEnvelope([claim('a','/data/a',bytes)],[],1);assert.equal(e.one_pass.content_calls,calls);assert.equal(e.one_pass.reserved_bytes,bytes+1);}});
test('checked arithmetic rejects fractional negative and overflowing sums',()=>{for(const pair of [[-1,1],[0.5,1],[Number.MAX_SAFE_INTEGER,1]])assert.throws(()=>checkedAdd(...pair));assert.equal(checkedAdd(0,0),0);});
test('read envelope pass count is bounded',()=>{for(const n of [0,5,-1,1.5])assert.throws(()=>exactReadEnvelope([],[],n));});
test('policy refuses an incomplete historical name census',()=>assert.throws(()=>proposeResourcePolicy(raw(input()),{fixture_manifest_bytes:1,historical_paths:['/data/a']})));
test('output slots can be finite but remain unbound',()=>{const x=input();x.outputs=[{role:'receipt',path:null,cap_bytes:8388608}];assert.equal(compileOperationalGraph(raw(x)).outputs[0].path,null);});

import test from 'node:test';
import assert from 'node:assert/strict';
import {reviewLoadedStartup,reviewUnqualifiedStartup} from './loaded-profile.mjs';
import {planLoadedSelection,planUnqualifiedLoadedSelection} from './loaded-selection.mjs';
import {decodeFixtureManifest,requiredFixtureManifestPath,FIXTURE_ROLES,MAX_FIXTURE_MANIFEST_BYTES,MAX_FIXTURE_RESERVED_BYTES,MAX_FIXTURE_CONTENT_CALLS} from './loaded-fixture-manifest.mjs';
const encode=v=>Buffer.from(JSON.stringify(v));
const fixtureManifest=()=>({schema:'fe2o3-loaded-review-fixtures-v1',files:FIXTURE_ROLES.map((role,i)=>({role,path:'/explicit-fixtures/role-'+i}))});
// These are portable API/admission controls, not historical startup semantic coverage.
test('four public review/planning entries are available without fixtures',()=>{
 for(const f of [reviewLoadedStartup,reviewUnqualifiedStartup,planLoadedSelection,planUnqualifiedLoadedSelection])assert.equal(typeof f,'function');
});
test('qualified profile cannot accept missing complete historical inputs',()=>{
 for(const v of [null,{},[],Array(71).fill({}),Array(73).fill({})])assert.throws(()=>reviewLoadedStartup(v));
});
test('qualified planner cannot accept missing complete historical inputs',()=>{
 for(const v of [null,{},[],{profile_records:[],selector_records:[]},{profile_records:[],selector_records:[],permission:true}])assert.throws(()=>planLoadedSelection(v));
});
test('profile raw JSON boundary rejects malformed non-Buffer invalid UTF8 and oversized input',()=>{
 for(const v of [null,{},Buffer.alloc(0),Buffer.from([255]),Buffer.from('{'),Buffer.from('null'),Buffer.alloc(16*1024*1024+1)])assert.throws(()=>reviewUnqualifiedStartup(v));
});
test('planner raw JSON boundary rejects malformed non-Buffer invalid UTF8 and oversized input',()=>{
 for(const v of [null,{},Buffer.alloc(0),Buffer.from([255]),Buffer.from('{'),Buffer.from('null'),Buffer.alloc(32*1024*1024+1)])assert.throws(()=>planUnqualifiedLoadedSelection(v));
});
test('public JSON parsers refuse dangerous keys and excessive nesting',()=>{
 let v={};for(let i=0;i<66;i++)v={next:v};
 for(const f of [reviewUnqualifiedStartup,planUnqualifiedLoadedSelection]){assert.throws(()=>f(Buffer.from('{"__proto__":{}}')));assert.throws(()=>f(encode(v)));}
});
test('explicit fixture manifest is data-only and binds immutable expected pins',()=>{
 const v=fixtureManifest(),p=decodeFixtureManifest(encode(v));
 assert.equal(p.files.length,76);assert.equal(p.profile_count,72);assert.equal(p.selector_count,4);
 assert.equal(p.payload_bytes,13854356);assert.equal(p.reserved_bytes,13854432);assert.equal(p.content_calls,347);
 assert.equal(p.historical_paths_used_as_io_defaults,false);
 assert.equal(p.files[0].path,v.files[0].path);assert.match(p.files[0].expected.sha256,/^[a-f0-9]{64}$/);
 assert.ok(Object.isFrozen(p.files[0].expected));assert.ok(Object.isFrozen(FIXTURE_ROLES));
 v.files[0].path='/caller-mutated';assert.notEqual(p.files[0].path,v.files[0].path);
 assert.ok(p.reserved_bytes+MAX_FIXTURE_MANIFEST_BYTES+1<MAX_FIXTURE_RESERVED_BYTES);
 assert.ok(p.content_calls+2<MAX_FIXTURE_CONTENT_CALLS);
});
test('fixture role list is exactly 76 ordered unique provenance roles',()=>{
 assert.equal(FIXTURE_ROLES.length,76);assert.equal(new Set(FIXTURE_ROLES).size,76);assert.throws(()=>FIXTURE_ROLES.pop());
});
test('missing fixture environment variable is an error rather than fallback or skip',()=>{
 for(const v of [undefined,null,'',0,false])assert.throws(()=>requiredFixtureManifestPath(v),/required; no fallback or skip/);
 assert.equal(requiredFixtureManifestPath('/fixtures/manifest.json'),'/fixtures/manifest.json');
});
const pathFailures=['relative.json','/fixtures/../file','/fixtures/./file','/fixtures//file','/fixtures/file/','/fixtures/\\file','/fixtures/\0file','/fixtures/file (deleted)','/'+ 'a'.repeat(4096)];
for(const value of pathFailures)test('fixture path admission rejects '+JSON.stringify(value.slice(0,80)),()=>assert.throws(()=>requiredFixtureManifestPath(value)));
const mutations=[
 ['wrong schema',v=>v.schema='other'],
 ['missing role',v=>v.files.pop()],
 ['extra role',v=>v.files.push(v.files[0])],
 ['duplicate role',v=>v.files[1].role=v.files[0].role],
 ['reordered roles',v=>[v.files[0],v.files[1]]=[v.files[1],v.files[0]]],
 ['duplicate physical name',v=>v.files[1].path=v.files[0].path],
 ['caller-supplied digest',v=>v.files[0].sha256='0'.repeat(64)],
 ['caller-supplied bytes',v=>v.files[0].bytes=0],
 ['historical copy fallback field',v=>v.files[0].copy='/old/host/path'],
 ['unknown root field',v=>v.copy_from_historical_binding=true],
 ['null path',v=>v.files[0].path=null],
 ['unknown role',v=>v.files[0].role='foreign'],
 ['row array',v=>v.files[0]=[]],
];
for(const [name,mutate]of mutations)test('fixture decoder refuses '+name,()=>{const v=fixtureManifest();mutate(v);assert.throws(()=>decodeFixtureManifest(encode(v)));});
test('fixture decoder enforces closed finite JSON byte admission',()=>{
 for(const v of [null,{},Buffer.alloc(0),Buffer.from([255]),Buffer.from('{'),Buffer.from('null'),Buffer.alloc(MAX_FIXTURE_MANIFEST_BYTES+1)])assert.throws(()=>decodeFixtureManifest(v));
});

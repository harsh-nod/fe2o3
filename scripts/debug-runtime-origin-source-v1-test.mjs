// Synthetic controls only. No compiler export, real source claim, or GPU execution.
import test from 'node:test';
import assert from 'node:assert/strict';
import { EventEmitter } from 'node:events';
import { SOURCE,CRATE,LIMITS,sha,validateCensus,validateObserver,validateRows,
  validateAttribution,manifest,json,exportArguments } from './debug-runtime-origin-source-v1-data.mjs';
import { parseArguments } from './debug-runtime-origin-source-v1-smoke.mjs';
import { runRuntimeOriginCommand } from './debug-runtime-origin-source-v1-process.mjs';

const topology={entry:0,helper:1,call:[0,17,0],call_authoring_coordinate:[0,0,0],cycle:[17],
  helper_sites:[[1,99,0],[1,99,1],[1,99,2]],
  helper_authoring_coordinates:[[1,0,0],[1,0,1],[1,0,2]]};
function rows(trips,expected) {
  const output=[];
  for(let lane=0;lane<4;lane++) {
    const push=(site,kind,activation,attempt,allocation=0,offset=0,bits=0)=>
      output.push([output.length,lane,lane,[...site],kind,activation,attempt,allocation,offset,bits]);
    for(let trip=0;trip<trips;trip++) {
      push(topology.call,0,1,trip+1);
      topology.helper_sites.forEach((site,i)=>{
        push(site,0,trip+2,i+1);push(site,1,trip+2,i+1);
      });
      push(topology.call,1,1,trip+1);
    }
    const site=[0,42,0];
    push(site,0,1,trips+1);push(site,2,1,trips+1,7,4+4*lane,expected);push(site,1,1,trips+1);
  }
  return output;
}
function report() {
  const cases=[];
  for(const [i,rounds] of [0,1,3].entries()) for(const schedule of ['canonical','seeded_71']) {
    const expected=[0xabcd1234,0x479e,0x479d][i],bytes=Buffer.alloc(24),recordRows=rows(rounds,expected);
    [0xdeadbeef,expected,expected,expected,expected,0xcafebabe].forEach((v,i)=>bytes.writeUInt32LE(v,i*4));
    cases.push({rounds,schedule,expected_word:expected,invocations:4,steps:100,output_bytes:[...bytes],
      full_execution_equal:true,compact_legacy_records_equal:true,
      observation:{records:recordRows.length,call_attempts:rounds*4,helper_activations:rounds*4,
        global_writes:4,rows:recordRows}});
  }
  return {schema:'task-runtime-origin-source-observer-v1',status:'passed',bundle_sha256:'a'.repeat(64),
    target:'gfx942:xnack-',bundle_identity:'b'.repeat(64),canonical_kir_sha256:'c'.repeat(64),
    canonical_kir_digest:'f'.repeat(64),
    kernel_abi_identity:'d'.repeat(64),semantic_mir_identity:'e'.repeat(64),canonical_kir_bytes:4096,
    production_kir_identity:'synthetic_test_only',source_authenticated:false,hardware_observed:false,
    compiler_resume_authority:false,topology:structuredClone(topology),cases,
    contextual_runs:6,opt_out_runs:6,helper_activations:32};
}
function census() {
  return {schema:'fe2o3-diagnostic-source-census-v1',diagnosticOnly:true,qualified:false,
    authenticatesCompilerExecution:false,extractionSucceeded:true,runId:'test-only',
    extractionMode:{kind:'simulation-bundle',version:6},workingDirectory:'/tmp/source',
    arguments:['--crate-name',CRATE,'src/lib.rs'],selection:{status:'available',value:{
      target:'gfx942:xnack-',functions:[0,1].map(i=>({role:i?'internal-helper':'kernel-entry',
        exportName:i?null:'loop_helper',functionIdentity:String(i+1).repeat(64),
        definitionIdentity:'b'.repeat(64),monomorphizationIdentity:'c'.repeat(64)})),
      files:[{identity:'f'.repeat(64),compiledSourceHash:'synthetic-only',
        originalSha256:sha(Buffer.from(SOURCE)),originalBytes:Buffer.byteLength(SOURCE),
        normalizedBytes:Buffer.byteLength(SOURCE)}]}}};
}

test('synthetic positive observer checks rows, zero/single/repeated calls, and aggregate32',()=>{
  validateObserver(report(),'a'.repeat(64));
});
test('independent row controls reject site/token/order/count/memory substitutions',()=>{
  const changes=[
    r=>r[1][3]=[1,100,0],r=>r[1][5]=1,r=>r[2][6]=2,r=>r[1][2]=4,
    r=>r.pop(),r=>r.find(v=>v[4]===2)[8]=0,r=>r.find(v=>v[4]===2)[9]^=1,
    r=>r[1][5]=0,r=>r[1][0]=0,r=>r[1][3][0]=99,
  ];
  for(const change of changes) {
    const sample=rows(1,0x479e);change(sample);
    assert.throws(()=>validateRows(sample,topology,1,0x479e));
  }
  assert.throws(()=>validateRows(Array(4097).fill(rows(1,0x479e)[0]),topology,1,0x479e));
});
test('observer rejects stale bundle, duplicated case, false authority and changed canaries',()=>{
  for(const mutate of [
    r=>r.bundle_sha256='0'.repeat(64),r=>r.cases[2]=structuredClone(r.cases[0]),
    r=>r.source_authenticated=true,r=>r.cases[0].output_bytes[0]^=1,
    r=>r.cases[0].output_bytes[23]^=1,r=>r.helper_activations=64,
    r=>r.cases[4].observation.helper_activations=4,r=>r.topology.cycle=[],
    r=>r.cases[0].full_execution_equal=false,
  ]) {
    const sample=report();mutate(sample);assert.throws(()=>validateObserver(sample,'a'.repeat(64)));
  }
});
test('same-run census requires actual exact bytes, call-capable roster and intended source',()=>{
  const source=Buffer.from(SOURCE);
  assert.equal(validateCensus(census(),'test-only','/tmp/source/src/lib.rs',source),'f'.repeat(64));
  for(const mutate of [
    c=>c.runId='old',c=>c.extractionSucceeded=false,c=>c.selection.status='unavailable',
    c=>c.arguments[1]='wrong',c=>c.arguments[2]='/tmp/other.rs',
    c=>c.selection.value.files[0].originalSha256='0'.repeat(64),
    c=>c.selection.value.files[0].normalizedBytes--,
    c=>c.selection.value.functions.pop(),
    c=>c.selection.value.functions[1].functionIdentity=c.selection.value.functions[0].functionIdentity,
  ]) {
    const sample=census();mutate(sample);
    assert.throws(()=>validateCensus(sample,'test-only','/tmp/source/src/lib.rs',source));
  }
});
test('nonpositional runtime block IDs join exact authoring roster coordinates, never names',()=>{
  const r=report(),source=Buffer.from(SOURCE);
  const ops=[topology.call_authoring_coordinate,...topology.helper_authoring_coordinates].map((site,i)=>({
    coordinate:{function:site[0],block:site[1],operation:site[2]},kind:i?'binary':'call',
    mnemonic:null,inline_assembly_source:null,source_spans:[{file_identity:'f'.repeat(64),
      byte_start:'0',byte_end:'10'}]}));
  const summary={target:r.target,canonical_kir_version:11,bundle_identity:r.bundle_identity,
    canonical_kir_digest:r.canonical_kir_digest,operation_count:ops.length,
    authority:{source_authenticated:false,grants_production_resume:false}};
  validateAttribution(r,summary,ops,'f'.repeat(64),source);
  // Raw byte SHA256 and the canonical model's domain-separated identity are
  // different contracts; one must never be substituted for the other.
  assert.throws(()=>validateAttribution(r,
    {...summary,canonical_kir_digest:r.canonical_kir_sha256},ops,'f'.repeat(64),source));
  for(const mutate of [
    t=>t.call_authoring_coordinate=[...t.call],
    t=>t.helper_authoring_coordinates=structuredClone(t.helper_sites),
    t=>t.call=[...t.call_authoring_coordinate],
    t=>t.call_authoring_coordinate[2]++,
    t=>t.helper_authoring_coordinates[1]=[...t.helper_authoring_coordinates[0]],
    t=>delete t.call_authoring_coordinate,
  ]) {
    const sample=report();mutate(sample.topology);
    assert.throws(()=>validateAttribution(sample,summary,ops,'f'.repeat(64),source));
  }
  for(const mutate of [
    o=>o[0].source_spans[0].file_identity='0'.repeat(64),
    o=>o[0].source_spans[0].byte_end='999999',
    o=>o[0].kind='binary',o=>o[1].source_spans=[],o=>o.push(o[0]),
    o=>o[0].coordinate.block=topology.call[1],
    o=>o[1].coordinate.block=topology.helper_sites[0][1],
  ]) {
    const sample=structuredClone(ops);mutate(sample);
    assert.throws(()=>validateAttribution(r,summary,sample,'f'.repeat(64),source));
  }
});
function options() {
  return ['--output','/tmp/origin-output','--bin-dir','/tmp/bin','--observer','/tmp/observer',
    '--rustc','/tmp/toolchain/bin/rustc','--cargo','/tmp/toolchain/bin/cargo',
    '--rustc-driver','/tmp/toolchain/lib/librustc_driver-abc.so','--cargo-home','/tmp/cargo',
    '--cache-root','/tmp/cache-a','--secondary-cache-root','/tmp/cache-b'];
}
test('CLI requires distinct resolved-syntax paths and exact explicit options',()=>{
  assert.equal(parseArguments(options()).output,'/tmp/origin-output');
  for(const mutate of [
    a=>a.pop(),a=>a[0]='--unknown',a=>a[1]='relative',a=>a[3]='bad\npath',
    a=>a[a.length-1]='/tmp/cache-a',a=>a[1]='/tmp/cache-a/subdir',
    a=>a[12]='--output',a=>a[11]='/different/driver.so',
  ]) {const a=options();mutate(a);assert.throws(()=>parseArguments(a));}
  assert.match(manifest('/tmp/repo'),/fe2o3-runtime-origin-source-v1-fixture/);
  assert.match(SOURCE,/#\[inline\(never\)\]/);
  assert.match(SOURCE,/control_flow\(loop_bounds\(3\)\)/);
  assert.match(SOURCE,/let trips = rounds % 4;/);
  // Exact nonzero constant remainder is an admitted uniform-index expression.
  // It preserves the separately implemented bit-mask oracle for every u32.
  for(const rounds of [0,1,2,3,4,15,16,0x7fffffff,0xfffffffe,0xffffffff])
    assert.equal(rounds % 4,rounds & 3);
  assert.match(SOURCE,/while iteration < trips/);
  assert.match(SOURCE,/iteration \+= 1;/);
  assert.doesNotMatch(SOURCE,/iteration\.wrapping_add/);
  assert.doesNotMatch(SOURCE,/amdgpu_asm!/);
  const exported=exportArguments({output:'/tmp/output',target:'/tmp/target'});
  assert.equal(exported.filter(arg=>arg==='--offline').length,1);
  // The exporter itself supplies --locked to Cargo; duplicates are refused.
  assert.equal(exported.filter(arg=>arg==='--locked').length,0);
});
test('bounded JSON parser rejects duplicate or oversized evidence',()=>{
  assert.throws(()=>json(Buffer.from('{"a":1,"a":2}'),LIMITS.report));
  assert.throws(()=>json(Buffer.alloc(33,32),32));
});

function mock(mode) {
  let child,kills=0,writes=0;
  const spawnImpl=()=>{
    child=new EventEmitter();child.pid=77;child.stdout=new EventEmitter();child.stderr=new EventEmitter();
    child.stdin=new EventEmitter();child.stdin.destroy=()=>{};
    child.stdin.write=()=>{writes++;if(mode==='drain'&&writes===1) {
      queueMicrotask(()=>child.stdin.emit('drain'));return false;}return true;};
    child.stdin.end=()=>{if(mode==='timeout')return;
      queueMicrotask(()=>{if(mode==='cap')child.stdout.emit('data',Buffer.alloc(33));
        else child.stdout.emit('data',Buffer.from('ok'));child.emit('close',mode==='exit'?9:0,null);});};
    return child;
  };
  const killImpl=(pid,signal)=>{assert.equal(pid,-77);assert.equal(signal,'SIGKILL');kills++;
    queueMicrotask(()=>child.emit('close',null,signal));};
  return {spawnImpl,killImpl,state:()=>({kills,writes})};
}
const command={executable:'/unused/compiler',args:[],cwd:'/tmp',env:{},timeoutMs:20,outputCap:32};
test('mock stdin drains and successful command captures output without a real process',async()=>{
  const child=mock('drain');
  const result=await runRuntimeOriginCommand({...command,input:Buffer.alloc(17000)},child);
  assert.equal(result.code,0);assert.equal(result.reason,null);assert.equal(result.stdout.toString(),'ok');
  assert.equal(child.state().writes,2);
});
test('mock timeout/output cap/nonzero/spawn failure never becomes a passed command',async()=>{
  for(const mode of ['timeout','cap','exit']) {
    const child=mock(mode),result=await runRuntimeOriginCommand(command,child);
    assert.ok(result.code!==0||result.reason!==null);
    if(mode!=='exit')assert.equal(child.state().kills,1);
  }
  const result=await runRuntimeOriginCommand(command,{spawnImpl:()=>{throw new Error('missing');}});
  assert.equal(result.code,null);assert.match(result.reason,/setup/);
  assert.throws(()=>runRuntimeOriginCommand({...command,outputCap:LIMITS.stream+1}));
});

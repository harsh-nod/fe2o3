// Private bounded acceptance data; no compiler/debugger wire or source authority.
import crypto from 'node:crypto';
import path from 'node:path';
import { parseBoundedJson } from './ordered-program-source-native.mjs';
export const SOURCE = "#![no_std]\n\nuse fe2o3_device::{DisjointSlice, kernel, thread};\n\n#[inline(never)]\nfn mix(value: u32, salt: u32) -> u32 {\n    (value ^ salt) & 0xffff\n}\n\n/// Acceptance fixture, not an alternate frontend or authored-assembly kernel.\n#[kernel(typed, launch(required = [64, 1, 1], max = [64, 1, 1]), control_flow(loop_bounds(3)))]\npub fn loop_helper(mut out: DisjointSlice<u32>, seed: u32, salt: u32, rounds: u32) {\n    let trips = rounds % 4;\n    let mut iteration = 0_u32;\n    let mut value = seed;\n    while iteration < trips {\n        value = mix(value, salt ^ iteration);\n        iteration += 1;\n    }\n    let index = thread::index_1d();\n    if let Some(output) = out.get_mut(index) {\n        *output = value;\n    }\n}\n";
export const CRATE = 'fe2o3_runtime_origin_source_v1_fixture';
export const RUSTC_COMMIT = '55e86c996809902e8bbad512cfb4d2c18be446d9';
export const LIMITS = Object.freeze({ source:16384, artifact:65536, census:262144,
  bundle:262144, report:524288, retained:1048576, capture:4194304, operations:256,
  stream:1048576, export_ms:300000, observer_ms:60000, jobs:2,
  free_bytes:String(40n*1024n**3n), ram_bytes:String(64n*1024n**3n),
  cache_bytes:String(20n*1024n**3n) });
export const sha = bytes => crypto.createHash('sha256').update(bytes).digest('hex');
export const demand = (ok,message) => { if (!ok) throw new Error(message); };
export const same = (a,b,message) => demand(JSON.stringify(a)===JSON.stringify(b),message);
export const json = (bytes,cap) => parseBoundedJson(bytes,cap);
export const artifact = bytes => ({bytes:bytes.length,sha256:sha(bytes),
  utf8:new TextDecoder('utf-8',{fatal:true}).decode(bytes)});
export function uint(value,max,message) {
  demand(Number.isSafeInteger(value)&&value>=0&&value<=max,message); return value;
}
export function digest(value) {
  demand(typeof value==='string'&&/^[0-9a-f]{64}$/.test(value),'digest');
}
export function absolute(value) {
  demand(typeof value==='string'&&value.length<=4096&&path.isAbsolute(value)
    &&path.resolve(value)===value&&!/[\x00-\x1f\x7f]/.test(value),'absolute bounded path'); return value;
}
export function exportArguments(c) {
  return ['--crate',CRATE,'--output',path.join(c.output,'loop-helper-v6.fe2sim'),
    '--bundle-version','6','--target','gfx942','--target-dir',c.target,
    '--','--manifest-path',path.join(c.output,'source/Cargo.toml'),'--lib','--offline'];
}
export function manifest(repo) {
  return '[package]\nname = "fe2o3-runtime-origin-source-v1-fixture"\nversion = "0.0.0"\n'
    +'edition = "2024"\npublish = false\n\n[workspace]\n\n[dependencies]\n'
    +'fe2o3-device = { path = '+JSON.stringify(path.join(repo,'crates/fe2o3-device'))+' }\n'
    +'\n[target.\'cfg(not(target_arch = "amdgpu"))\'.dependencies]\n'
    +'fe2o3-host = { path = '+JSON.stringify(path.join(repo,'crates/fe2o3-host'))+' }\n'
    +'\n[lib]\nname = "'+CRATE+'"\npath = "src/lib.rs"\n';
}
export function validateCensus(census,runId,sourceFile,source) {
  demand(census.schema==='fe2o3-diagnostic-source-census-v1'
    &&census.diagnosticOnly===true&&census.qualified===false
    &&census.authenticatesCompilerExecution===false&&census.extractionSucceeded===true
    &&census.runId===runId,'same-run successful diagnostic census');
  same(census.extractionMode,{kind:'simulation-bundle',version:6},'census extraction mode');
  absolute(census.workingDirectory);
  demand(Array.isArray(census.arguments)&&census.arguments.length<=128
    &&census.arguments.every(arg=>typeof arg==='string'&&arg.length<=4096),'census argument bound');
  const flags=census.arguments.flatMap((arg,i)=>arg==='--crate-name'?[i]:[]);
  demand(flags.length===1&&census.arguments[flags[0]+1]===CRATE,'census exact crate');
  const sources=census.arguments.filter(arg=>!arg.startsWith('-')&&arg.endsWith('.rs'));
  demand(sources.length===1&&path.resolve(census.workingDirectory,sources[0])===sourceFile,
    'census exact source argument');
  demand(census.selection?.status==='available','census selection unavailable');
  const selected=census.selection.value;
  demand(selected?.target==='gfx942:xnack-'&&Array.isArray(selected.functions)
    &&selected.functions.length>=2&&selected.functions.length<=8,'census target/function bound');
  demand(selected.functions.filter(fn=>fn.role==='kernel-entry'&&fn.exportName==='loop_helper').length===1,
    'one census kernel entry');
  for(const fn of selected.functions) for(const key of
    ['functionIdentity','definitionIdentity','monomorphizationIdentity']) digest(fn[key]);
  demand(new Set(selected.functions.map(fn=>fn.functionIdentity)).size===selected.functions.length,
    'duplicate census function identity');
  demand(Array.isArray(selected.files)&&selected.files.length===1,'one exact retained source file');
  const file=selected.files[0]; digest(file.identity);
  demand(file.originalSha256===sha(source)&&file.originalBytes===source.length
    &&file.normalizedBytes===source.length,'census source byte join');
  demand(typeof file.compiledSourceHash==='string'&&file.compiledSourceHash.length>0
    &&file.compiledSourceHash.length<=128,'census compiled hash');
  return file.identity;
}
export function validateRows(rows,t,trips,expected) {
  demand(Array.isArray(rows)&&rows.length>0&&rows.length<=4096,'compact row cap');
  const pending=new Map(),last=new Map(),helpers=new Map(),calls=[],writes=new Set();
  const helperSites=new Set(t.helper_sites.map(site=>site.join(':')));
  let allocation=null;
  rows.forEach((r,i)=>{
    demand(Array.isArray(r)&&r.length===10&&r[0]===i&&Array.isArray(r[3])&&r[3].length===3,'row exact shape/ordinal');
    for(const n of [r[0],r[1],r[5],r[6]]) uint(n,4097,'row bounded counter');
    uint(r[2],3,'row full invocation index');uint(r[4],2,'row kind');
    r[3].forEach(n=>uint(n,0xffffffff,'row static coordinate'));
    uint(r[7],16,'allocation');uint(r[8],24,'memory offset');uint(r[9],0xffffffff,'memory scalar');
    demand(r[5]>0&&r[6]>0,'row nonzero origin');
    const site=r[3].join(':'),scope=r[2]+':'+r[5],key=scope+':'+r[6];
    if(r[3][0]===t.entry) demand(r[5]===1,'row root activation');
    if(r[4]!==2) demand(r[7]===0&&r[8]===0&&r[9]===0,'checkpoint has no fabricated memory');
    if(r[4]===0) {
      demand(r[6]===(last.get(scope)??0)+1&&!pending.has(key),'duplicate/skipped attempt');
      pending.set(key,{site,before:i});last.set(scope,r[6]);
    } else if(r[4]===1) {
      const before=pending.get(key);demand(before?.site===site&&before.before<i,'missing/wrong after');
      pending.delete(key);
      if(site===t.call.join(':')) calls.push({lane:r[2],before:before.before,after:i});
    } else {
      const before=pending.get(key);
      demand(before?.site===site&&before.before<i&&r[7]>0&&r[8]===4+4*r[2]
        &&r[9]===expected&&!writes.has(r[2]),'memory origin/output bounds');
      demand(allocation===null||allocation===r[7],'output allocation changed');
      allocation=r[7];writes.add(r[2]);
    }
    if(r[3][0]===t.helper) {
      demand(r[5]!==1&&helperSites.has(site),'helper static/dynamic join');
      const interval=helpers.get(scope)??{lane:r[2],first:i,last:i};
      interval.last=i;helpers.set(scope,interval);
    }
  });
  demand(pending.size===0&&writes.size===4,'unclosed attempts/missing writes');
  for(let lane=0;lane<4;lane++) {
    const localCalls=calls.filter(r=>r.lane===lane),localHelpers=[...helpers.values()].filter(r=>r.lane===lane);
    demand(localCalls.length===trips&&localHelpers.length===trips,'per-invocation actual helper/call count');
    for(const h of localHelpers) demand(localCalls.filter(c=>c.before<h.first&&h.last<c.after).length===1,
      'helper needs exact caller interval');
    for(const c of localCalls) demand(localHelpers.filter(h=>c.before<h.first&&h.last<c.after).length===1,
      'caller needs one helper activation');
  }
  return {calls:calls.length,helpers:helpers.size,writes:writes.size};
}
function validateTopology(t) {
  uint(t?.entry,7,'entry ordinal'); uint(t?.helper,7,'helper ordinal');
  demand(t.entry!==t.helper,'distinct retained functions');
  const coordinate=(site,maxBlock,label)=>{
    demand(Array.isArray(site)&&site.length===3,label);
    uint(site[0],7,label);uint(site[1],maxBlock,label);uint(site[2],255,label);
  };
  coordinate(t.call,0xffffffff,'runtime call site');
  coordinate(t.call_authoring_coordinate,63,'authoring call roster coordinate');
  demand(t.call[0]===t.entry&&t.call_authoring_coordinate[0]===t.entry
    &&t.call[2]===t.call_authoring_coordinate[2],'call coordinate domain join');
  demand(Array.isArray(t.cycle)&&t.cycle.length>0&&t.cycle.length<=64
    &&new Set(t.cycle).size===t.cycle.length&&t.cycle.includes(t.call[1]),'retained cycle');
  t.cycle.forEach(n=>uint(n,0xffffffff,'runtime cycle block ID'));
  demand(Array.isArray(t.helper_sites)&&t.helper_sites.length===3
    &&Array.isArray(t.helper_authoring_coordinates)&&t.helper_authoring_coordinates.length===3,
    'retained pure helper sites/roster coordinates');
  t.helper_sites.forEach((site,i)=>{
    coordinate(site,0xffffffff,'runtime helper site');
    coordinate(t.helper_authoring_coordinates[i],63,'authoring helper roster coordinate');
    demand(site[0]===t.helper&&site[1]===t.helper_sites[0][1]&&site[2]===i,
      'single-block runtime helper roster');
    same(t.helper_authoring_coordinates[i],[t.helper,0,i],'single-block authoring helper roster');
  });
  return t;
}
export function validateObserver(report,bundleSha) {
  demand(report.schema==='task-runtime-origin-source-observer-v1'&&report.status==='passed'
    &&report.bundle_sha256===bundleSha&&report.target==='gfx942:xnack-','exact observer bundle');
  for(const field of ['bundle_identity','canonical_kir_sha256','canonical_kir_digest','kernel_abi_identity','semantic_mir_identity'])
    digest(report[field]);
  uint(report.canonical_kir_bytes,LIMITS.bundle,'canonical bytes');
  demand(typeof report.production_kir_identity==='string'&&report.production_kir_identity.length<=512,
    'actual production lineage retained');
  demand(report.source_authenticated===false&&report.hardware_observed===false
    &&report.compiler_resume_authority===false,'observer authority');
  const t=validateTopology(report.topology);
  demand(Array.isArray(report.cases)&&report.cases.length===6,'six actual contextual cases');
  let total=0;
  report.cases.forEach((row,i)=>{
    const rounds=[0,1,3][Math.floor(i/2)],expected=[0xabcd1234,0x479e,0x479d][Math.floor(i/2)];
    demand(row.rounds===rounds&&row.schedule===(i%2?'seeded_71':'canonical')
      &&row.expected_word===expected&&row.invocations===4&&row.full_execution_equal===true
      &&row.compact_legacy_records_equal===true,'exact case/oracle/comparison');
    uint(row.steps,4096,'steps');
    const bytes=Buffer.alloc(24); [0xdeadbeef,expected,expected,expected,expected,0xcafebabe]
      .forEach((word,j)=>bytes.writeUInt32LE(word,j*4));
    same(row.output_bytes,[...bytes],'output and both canaries');
    const o=row.observation;
    demand(o.call_attempts===rounds*4&&o.helper_activations===rounds*4&&o.global_writes===4,
      'measured helper/call/write count');
    uint(o.records,4096,'record cap');
    demand(Array.isArray(o.rows)&&o.rows.length===o.records&&o.rows.length>0,'retained compact records');
    const checked=validateRows(o.rows,t,rounds,expected);
    demand(checked.calls===o.call_attempts&&checked.helpers===o.helper_activations
      &&checked.writes===o.global_writes,'row-derived counts');
    total+=o.helper_activations;
  });
  demand(report.contextual_runs===6&&report.opt_out_runs===6&&report.helper_activations===total&&total===32,
    'checked contextual run/count aggregate');
}
export function validateAttribution(report,summary,operations,fileIdentity,source) {
  const topology=validateTopology(report.topology);
  demand(summary.target==='gfx942:xnack-'&&summary.canonical_kir_version===11
    &&summary.bundle_identity===report.bundle_identity
    &&summary.canonical_kir_digest===report.canonical_kir_digest,'inspect/observer identity join');
  demand(summary.authority?.source_authenticated===false
    &&summary.authority?.grants_production_resume===false,'inspect authority');
  demand(operations.length===summary.operation_count&&operations.length<=LIMITS.operations,'complete operation count');
  const coordinates=new Map();
  for(const op of operations) {
    const coordinate=[op.coordinate.function,op.coordinate.block,op.coordinate.operation];
    coordinate.forEach(v=>uint(v,0xffffffff,'operation coordinate'));
    const key=coordinate.join(':'); demand(!coordinates.has(key),'duplicate coordinate');
    demand(op.mnemonic===null&&op.inline_assembly_source===null&&op.kind!=='inline_assembly',
      'ordinary source cannot be relabeled authored ISA');
    coordinates.set(key,op);
  }
  for(const site of [topology.call_authoring_coordinate,...topology.helper_authoring_coordinates]) {
    const op=coordinates.get(site.join(':')); demand(op,'selected topology operation absent');
    if(site===topology.call_authoring_coordinate) demand(op.kind==='call','selected retained call kind');
    demand(Array.isArray(op.source_spans)&&op.source_spans.length>0&&op.source_spans.length<=16,
      'selected source attribution unavailable');
    for(const span of op.source_spans) {
      demand(span.file_identity===fileIdentity,'selected span exact file-ID join');
      demand(typeof span.byte_start==='string'&&/^(0|[1-9][0-9]{0,8})$/.test(span.byte_start)
        &&typeof span.byte_end==='string'&&/^(0|[1-9][0-9]{0,8})$/.test(span.byte_end),'span byte syntax');
      const start=Number(span.byte_start),end=Number(span.byte_end);
      demand(start<=end&&end<=source.length,'span byte bounds');
      for(const offset of [start,end]) demand(offset===source.length||(source[offset]&0xc0)!==0x80,
        'span UTF8 boundary');
    }
  }
}

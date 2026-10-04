// Bounded ordinary-source acceptance only; no source authentication or GPU authority.
import path from 'node:path';
import { demand,same,uint,digest,absolute,sha,CRATE } from './debug-runtime-origin-source-v1-data.mjs';
export { SOURCE,CRATE,RUSTC_COMMIT,demand,same,uint,digest,absolute,sha,json,artifact,manifest,
  validateCensus,validateAttribution } from './debug-runtime-origin-source-v1-data.mjs';
export const WG_CRATE='fe2o3_production_ranked_bounds_fixture';
export const WG_SOURCE='crates/rustc-codegen-fe2o3/tests/fixtures/production-ranked-bounds-device/src/lib.rs';
export const WG_MANIFEST='crates/rustc-codegen-fe2o3/tests/fixtures/production-ranked-bounds-device/Cargo.toml';
export const PROFILE=Object.freeze({records:65536,origin_bytes:4194304,frame_rows:131072,frame_bytes:25165824,
  lifecycle_transitions:8192,lifecycle_bytes:8388608,lifecycle_validation_work:1000000,reuse_bytes:8192});
export const LIMITS=Object.freeze({source:65536,artifact:262144,census:262144,bundle:262144,
  report:1048576,retained:8388608,capture:16777216,operations:512,stream:1048576,
  export_ms:300000,observer_ms:120000,jobs:2,free_bytes:String(40n*1024n**3n),
  ram_bytes:String(64n*1024n**3n),cache_bytes:String(20n*1024n**3n)});
export function u64(value,nonzero=false) {
  demand(typeof value==='string'&&/^(0|[1-9][0-9]{0,19})$/.test(value),'decimal u64');
  const n=BigInt(value);demand(n<=18446744073709551615n&&(!nonzero||n>0n),'bounded nonzero u64');return n;
}
const triple=(value,max,message)=>{demand(Array.isArray(value)&&value.length===3,message);
  value.forEach(v=>uint(v,max,message));return value;};
export function invocation(value,grid=4) {
  triple(value?.global,grid-1,'full global invocation');same(value.global.slice(1),[0,0],'1D global');
  same(value.workgroup,[Math.floor(value.global[0]/64),0,0],'full workgroup');
  same(value.local,[value.global[0]%64,0,0],'full local invocation');
  same(value.workgroup_size,[64,1,1],'actual workgroup size');
  same(value.workgroup_count,[Math.ceil(grid/64),1,1],'actual workgroup count');
  same(value.launch_extent,[grid,1,1],'actual launch extent');return value;
}
export function site(value) {return triple(value,0xffffffff,'static operation coordinate');}
function values(rows) {
  demand(Array.isArray(rows)&&rows.length<=256,'bounded captured SSA roster');
  const seen=new Set();for(const row of rows) {
    demand(Array.isArray(row)&&row.length===2,'SSA row');uint(row[0],0xffffffff,'SSA value ID');
    demand(!seen.has(row[0]),'duplicate SSA value');seen.add(row[0]);const v=row[1];
    if(v?.kind==='scalar') demand(typeof v.type==='string'&&v.type.length<=16
      &&typeof v.bits==='string'&&/^[0-9a-f]{32}$/.test(v.bits),'scalar bits');
    else if(v?.kind==='pointer'||v?.kind==='slice') {
      u64(v.allocation,true);uint(v.offset,16384,'SSA pointer offset');
      demand(typeof v.element==='string'&&typeof v.access==='string','SSA pointer type');
      if(v.kind==='pointer') {uint(v.lower,16384,'pointer lower');uint(v.upper,16384,'pointer upper');
        demand(v.lower<=v.offset&&v.offset<=v.upper,'pointer bounds');}
      else {uint(v.elements,4096,'slice elements');uint(v.bytes,16384,'slice bytes');}
    } else demand(false,'uncaptured or unsupported SSA value');
  }
}
export function checkpoint(value,grid=4) {
  uint(value?.record,PROFILE.records-1,'retained record');uint(value.ordinal,262144,'record ordinal');
  u64(value.capture_instance,true);invocation(value.invocation,grid);site(value.site);
  demand(value.phase==='before'||value.phase==='after','checkpoint phase');
  u64(value.origin?.activation,true);u64(value.origin?.attempt,true);
  demand(Array.isArray(value.frames)&&value.frames.length>=1&&value.frames.length<=8,'actual frame roster');
  const seen=new Set();
  value.frames.forEach((f,depth)=>{
    demand(f.depth===depth,'ordered frame depth');uint(f.function,7,'frame function');uint(f.block,0xffffffff,'frame block');
    if(f.next_operation!==null) uint(f.next_operation,511,'next operation');
    u64(f.activation,true);demand(!seen.has(f.activation),'duplicate active frame identity');seen.add(f.activation);
    values(f.ssa);
    if(depth===0) demand(f.parent?.kind==='root','root parent');
    else {
      const p=f.parent,caller=value.frames[depth-1];
      demand(p?.kind==='caller'&&p.activation===caller.activation,'actual caller activation');
      u64(p.attempt,true);site(p.site);
      demand(caller.operation?.kind==='suspended'&&caller.operation.attempt===p.attempt,'actual suspended caller attempt');
      same(p.site,caller.operation.site,'actual caller site');
    }
    if(f.operation?.kind!=='ready') {
      demand(f.operation?.kind==='active'||f.operation?.kind==='suspended','frame operation state');
      u64(f.operation.attempt,true);site(f.operation.site);
      demand(f.operation.site[0]===f.function,'frame operation function');
    }
  });
  const current=value.frames.at(-1);
  demand(current.activation===value.origin.activation&&current.operation.kind==='active'
    &&current.operation.attempt===value.origin.attempt,'exact current operation origin');
  same(current.operation.site,value.site,'record/current operation site');
  return value;
}
export function sameCheckpoint(a,b,label) {same(a,b,label);}
function usage(value) {
  demand(value&&value.origins&&value.frames&&value.allocations,'metadata usage');
  const o=value.origins,f=value.frames,a=value.allocations;
  uint(o.rows,PROFILE.records,'origin rows');uint(o.capacity,PROFILE.records,'origin capacity');
  uint(o.bytes,PROFILE.origin_bytes,'origin retained bytes');demand(o.rows<=o.capacity,'origin capacity accounting');
  uint(f.records,PROFILE.records,'frame record indexes');uint(f.rows,PROFILE.frame_rows,'frame rows');
  uint(f.record_capacity,PROFILE.records,'frame index capacity');uint(f.frame_capacity,PROFILE.frame_rows,'frame row capacity');
  uint(f.bytes,PROFILE.frame_bytes,'frame byte charge');demand(f.records<=f.record_capacity&&f.rows<=f.frame_capacity,'frame capacity charge');
  uint(a.records,PROFILE.records,'allocation record indexes');uint(a.transitions,8192,'transition rows');
  uint(a.record_capacity,PROFILE.records,'allocation index capacity');uint(a.transition_capacity,8192,'transition capacity');
  uint(a.bytes,PROFILE.lifecycle_bytes,'lifecycle byte charge');
  demand(a.validation_work_limit===PROFILE.lifecycle_validation_work,'validation allowance');
  uint(a.validation_work_used,a.validation_work_limit,'validation work used');
  demand(a.records<=a.record_capacity&&a.transitions<=a.transition_capacity,'allocation capacity charge');
  uint(value.fixed_owner_bytes,1048576,'fixed owner charge');
}
function output(row,count,word) {
  const expected=Buffer.alloc((count+2)*4);
  [0xdeadbeef,...Array(count).fill(word),0xcafebabe].forEach((n,i)=>expected.writeUInt32LE(n,4*i));
  same(row.output_bytes,[...expected],'exact output and both guards');
  same(row.initialized,Array(expected.length).fill(true),'all output initialization bytes');
  demand(row.reuse_off_execution_equal===true&&row.reuse_off_legacy_equal===true,'full reuse off/on equality');
  uint(row.records,PROFILE.records,'record bound');demand(row.records>0,'nonempty actual transcript');
}
function base(report,mode,bundleSha) {
  demand(report?.schema==='task-runtime-observations-source-observer-v1'&&report.status==='passed'
    &&report.mode===mode,'closed public observer schema/mode');
  demand(report.profile&&Object.keys(report.profile).length===Object.keys(PROFILE).length
    &&Object.entries(PROFILE).every(([key,value])=>report.profile[key]===value),'exact source capture profile');
  for(const holder of [report,report.result]) demand(holder?.source_authenticated===false
    &&holder.hardware_observed===false&&holder.compiler_resume_authority===false,'authority refusal');
  const r=report.result;demand(r.bundle_sha256===bundleSha&&r.target==='gfx942:xnack-','bundle/target join');
  for(const field of ['bundle_sha256','bundle_identity','canonical_kir_sha256','canonical_kir_digest']) digest(r[field]);
  uint(r.canonical_kir_bytes,LIMITS.bundle,'bounded canonical bytes');return r;
}
export function validateLoop(report,bundleSha) {
  const r=base(report,'loop',bundleSha),t=r.topology;
  site(t.call);demand(Array.isArray(t.helper_sites)&&t.helper_sites.length===3,'retained helper operations');
  demand(Array.isArray(r.cases)&&r.cases.length===6,'six source loop cases');
  let total=0;
  r.cases.forEach((row,i)=>{
    const rounds=[0,1,3][Math.floor(i/2)],expected=[0xabcd1234,0x479e,0x479d][Math.floor(i/2)];
    demand(row.rounds===rounds&&row.schedule===(i%2?'seeded_71':'canonical')&&row.expected_word===expected,'loop oracle');
    output(row,4,expected);usage(row.usage);const o=row.observations,n=row.navigation;
    demand(o?.helper_activations===rounds*4&&o.call_attempts===rounds*4
      &&o.checked_helper_operations===rounds*12&&o.writes===4,'actual loop count');
    uint(o.pair_scan_work,1000000,'pair scan bound');
    demand(Array.isArray(o.witnesses)&&o.witnesses.length===rounds*4,'one witness per actual helper activation');
    const keys=new Set(),perLane=[0,0,0,0];
    for(const witness of o.witnesses) {
      const before=checkpoint(witness.before),after=checkpoint(witness.after);
      demand(before.phase==='before'&&after.phase==='after'&&after.record>before.record,'actual before/after pair');
      same(before.origin,after.origin,'paired activation/attempt');same(before.site,after.site,'paired site');
      same(before.invocation,after.invocation,'paired full invocation');
      demand(before.capture_instance===after.capture_instance&&before.frames.length===2&&after.frames.length===2,'same capture actual helper');
      demand(before.site[0]===t.helper&&t.helper_sites.some(s=>JSON.stringify(s)===JSON.stringify(before.site)),'actual retained helper site');
      same(before.frames[0].ssa,after.frames[0].ssa,'suspended caller SSA unchanged');
      demand(JSON.stringify(before.frames[1].ssa)!==JSON.stringify(after.frames[1].ssa),'child SSA changed');
      demand(witness.caller_ssa_unchanged===true&&witness.child_ssa_changed===true,'SSA observation assertions');
      const key=JSON.stringify(before.invocation)+':'+before.origin.activation;
      demand(!keys.has(key),'repeated activation silently rebound');keys.add(key);perLane[before.invocation.global[0]]++;
    }
    same(perLane,Array(4).fill(rounds),'helper visits per full invocation');
    if(rounds===0) demand(n.checks===0&&n.reason==='no helper activation for zero rounds','zero-round no invented helper');
    else {
      demand(n.checks===(rounds===3?8:6),'exact navigation controls');
      for(const name of ['first','completed','caller_before','caller_after']) checkpoint(n[name]);
      sameCheckpoint(n.first,o.witnesses[0].before,'navigation first source witness');
      sameCheckpoint(n.completed,o.witnesses[0].after,'navigation completed source witness');
      demand(n.caller_before.frames.length===1&&n.caller_after.frames.length===1,'retired callee not current');
      same(n.caller_before.origin,n.caller_after.origin,'actual caller attempt');
      same(n.caller_before.site,t.call,'caller before actual call');same(n.caller_after.site,t.call,'caller after actual call');
      demand(n.old_activation_absent_at_later_call===(rounds===3),'old activation expiry');
      if(rounds===3) {uint(n.later_record,row.records-1,'later repeated call');demand(n.later_record>n.first.record,'later actual call');}
      else demand(n.later_record===null,'single call has no later activation');
      uint(n.replay_work_used,1000000000,'bounded navigation work');
    }
    total+=o.helper_activations;
  });
  demand(r.reuse_on_runs===6&&r.reuse_off_runs===6&&r.helper_activations===32&&total===32,'loop run/count aggregate');
  return r;
}
function identity(v) {for(const key of ['allocation','storage_slot','generation'])u64(v?.[key],true);return v;}
function scope(v,group) {
  demand(v?.kind==='workgroup','actual workgroup allocation scope');
  same(v.coordinate,[group,0,0],'workgroup allocation coordinate');same(v.size,[64,1,1],'scope workgroup size');
  same(v.count,[2,1,1],'scope workgroup count');same(v.launch,[128,1,1],'scope launch extent');
}
export function validateReuse(o) {
  demand(Array.isArray(o?.transitions_at_last_record)&&o.transitions_at_last_record.length>0
    &&o.transitions_at_last_record.length<=8192,'literal bounded transition prefix');
  const rows=o.transitions_at_last_record;rows.forEach((r,i)=>{demand(u64(r.sequence,true)===BigInt(i+1),'exact transition prefix');identity(r.identity);});
  const creates=rows.filter(r=>r.kind?.kind==='create'&&r.space==='Workgroup');
  demand(creates.length===2,'two real workgroup creates');const [a,b]=creates;
  scope(a.scope,0);scope(b.scope,1);
  for(const row of [a,b]) demand(row.access==='ReadWrite'&&row.alignment===4&&row.byte_len===256,'exact LDS shape');
  demand(a.identity.allocation!==b.identity.allocation&&a.identity.storage_slot===b.identity.storage_slot
    &&a.identity.generation==='1'&&b.identity.generation==='2'
    &&a.kind.previous_allocation===null&&b.kind.previous_allocation===a.identity.allocation,'fresh semantic identity reused real slot');
  const releases=rows.filter(r=>r.kind?.kind==='release'&&r.identity.allocation===a.identity.allocation);
  demand(releases.length===1,'one actual first-workgroup release');const release=releases[0];
  demand(u64(a.sequence)<u64(release.sequence)&&u64(release.sequence)<u64(b.sequence),'release precedes reuse');
  for(const key of ['identity','scope','space','access','alignment','byte_len','creation_site'])
    same(release[key],a[key],'release matches entire actual descriptor');
  for(const [snap,create,group] of [[o.first,a,0],[o.second,b,1]]) {
    uint(snap?.record,PROFILE.records-1,'resource record');u64(snap.capture_instance,true);
    invocation(snap.invocation,128);demand(snap.invocation.workgroup[0]===group,'resource exact invocation scope');
    site(snap.site);identity(snap.identity);same(snap.identity,create.identity,'resource descriptor identity');
    same(snap.scope,create.scope,'resource descriptor scope');u64(snap.through_sequence,true);
    demand(u64(snap.through_sequence)>=u64(create.sequence),'checkpoint watermark includes creation');
    same(snap.bytes,Array(256).fill(0),'actual new/reused storage reset bytes');
    same(snap.initialized,Array(256).fill(false),'actual new/reused storage reset initialization');
  }
  demand(o.first.record<o.second.record&&o.first.capture_instance===o.second.capture_instance,'historical same owner ordering');
  demand(o.old_allocation_refused_at_second===true&&o.future_allocation_refused_at_first===true
    &&o.historical_seek_repeat_checks===4&&o.terminal_release_claimed===false,'no stale alias or invented terminal release');
  uint(o.replay_work_used,1000000000,'allocation navigation work');usage(o.usage);
}
export function validateWorkgroup(report,bundleSha) {
  const r=base(report,'workgroup',bundleSha);
  demand(r.topology?.element==='U32'&&r.topology.elements===64&&r.topology.alignment===4
    &&r.topology.private_alloca===false,'actual source LDS, not private Alloca');
  uint(r.topology.operations,512,'source operation count');site(r.topology.lds.runtime_site);site(r.topology.lds.authoring_coordinate);
  demand(Array.isArray(r.cases)&&r.cases.length===2&&r.reuse_on_runs===2&&r.reuse_off_runs===2,'two source workgroup schedules');
  r.cases.forEach((row,i)=>{demand(row.schedule===(i?'seeded_71':'canonical')&&row.input===2&&row.expected_word===128,'workgroup oracle');
    same(row.grid,[128,1,1],'grid');same(row.workgroup,[64,1,1],'workgroup');output(row,128,128);validateReuse(row.observations);});
  return r;
}
export function validateWorkgroupCensus(census,runId,sourceFile,source) {
  demand(census.schema==='fe2o3-diagnostic-source-census-v1'&&census.diagnosticOnly===true
    &&census.qualified===false&&census.authenticatesCompilerExecution===false
    &&census.extractionSucceeded===true&&census.runId===runId,'same-run successful WG diagnostic census');
  same(census.extractionMode,{kind:'simulation-bundle',version:5},'WG census extraction mode');absolute(census.workingDirectory);
  demand(Array.isArray(census.arguments)&&census.arguments.length<=128
    &&census.arguments.every(a=>typeof a==='string'&&a.length<=4096),'census argument cap');
  const indexes=census.arguments.flatMap((a,i)=>a==='--crate-name'?[i]:[]);
  demand(indexes.length===1&&census.arguments[indexes[0]+1]===WG_CRATE,'WG census exact crate');
  const sources=census.arguments.filter(a=>!a.startsWith('-')&&a.endsWith('.rs'));
  demand(sources.length===1&&path.resolve(census.workingDirectory,sources[0])===sourceFile,'WG exact source argument');
  const selected=census.selection?.value;
  demand(census.selection?.status==='available'&&selected?.target==='gfx942:xnack-'
    &&Array.isArray(selected.functions)&&selected.functions.length>0&&selected.functions.length<=8,'WG census selection');
  demand(selected.functions.filter(f=>f.role==='kernel-entry'&&f.exportName==='workgroup_reduce_u32').length===1,'WG entry');
  for(const f of selected.functions)for(const key of ['functionIdentity','definitionIdentity','monomorphizationIdentity'])digest(f[key]);
  demand(Array.isArray(selected.files)&&selected.files.length>0&&selected.files.length<=32,'WG source file roster');
  const matches=selected.files.filter(f=>f.originalSha256===sha(source)&&f.originalBytes===source.length&&f.normalizedBytes===source.length);
  demand(matches.length===1,'WG exact original source byte join');digest(matches[0].identity);return matches[0].identity;
}
export function validateWorkgroupAttribution(report,summary,operations) {
  demand(summary.target==='gfx942:xnack-'&&summary.canonical_kir_version===10
    &&summary.bundle_identity===report.bundle_identity&&summary.canonical_kir_digest===report.canonical_kir_digest,'WG inspect identity join');
  demand(summary.authority?.source_authenticated===false&&summary.authority?.grants_production_resume===false,'WG inspect authority');
  demand(operations.length===summary.operation_count&&operations.length<=512,'WG complete operation roster');
  const coords=new Map();for(const op of operations) {
    const key=[op.coordinate.function,op.coordinate.block,op.coordinate.operation];site(key);
    demand(!coords.has(key.join(':'))&&op.mnemonic===null&&op.inline_assembly_source===null&&op.kind!=='inline_assembly','ordinary unique WG operation');
    coords.set(key.join(':'),op);
  }
  const actual=coords.get(report.topology.lds.authoring_coordinate.join(':'));
  demand(actual&&actual.kind==='workgroup_memory','source LDS coordinate join');
  // The export/census binds source input; this does not invent source-variable-to-SSA ownership.
}
export function requestDocument(mode,rounds=0) {
  demand(mode==='loop'||mode==='workgroup','request mode');demand([0,1,3].includes(rounds),'request rounds');
  const count=mode==='loop'?4:128,bytes=Buffer.alloc((count+2)*4);
  [0xdeadbeef,...Array(count).fill(0xa5a5a5a5),0xcafebabe].forEach((n,i)=>bytes.writeUInt32LE(n,i*4));
  const view={kind:'buffer_view',backing:0,element:'u32',access:'read_write',alignment:4,byte_offset:4,elements:count};
  const scalar=n=>({kind:'scalar',type:'u32',bits:'0x'+n.toString(16).padStart(8,'0')});
  return {schema:'fe2o3-simulation-request-v1',kernel:mode==='loop'?'loop_helper':'workgroup_reduce_u32',
    grid:[count,1,1],workgroup:[64,1,1],arguments:mode==='loop'?[view,scalar(0xabcd1234),scalar(0x0f0f55aa),scalar(rounds)]:[scalar(2),view],
    shared_buffers:[{id:0,element:'u32',access:'read_write',alignment:4,bytes:'0x'+bytes.toString('hex')}]};
}
export function exportArguments(c,mode) {
  const loop=mode==='loop';demand(loop||mode==='workgroup','export mode');
  return ['--crate',loop?CRATE:WG_CRATE,'--output',path.join(c.output,loop?'loop-helper-v6.fe2sim':'workgroup-reduce-v5.fe2sim'),
    '--bundle-version',loop?'6':'5','--target','gfx942','--target-dir',loop?c.target:c.workgroup_target,
    '--','--manifest-path',loop?path.join(c.output,'source/Cargo.toml'):path.join(c.repo,WG_MANIFEST),
    '--lib',...(loop?[]:['--features','workgroup_reduce_u32']),'--offline'];
}

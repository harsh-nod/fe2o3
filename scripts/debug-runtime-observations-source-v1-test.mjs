// Pure acceptance-validator controls. Mock reports are never source/runtime qualification.
import test from 'node:test';
import assert from 'node:assert/strict';
import { checkpoint,u64,validateReuse,validateWorkgroup,validateLoop,requestDocument,exportArguments,
  validateWorkgroupCensus,validateWorkgroupAttribution,PROFILE,WG_CRATE,sha } from './debug-runtime-observations-source-v1-data.mjs';
import { parseArguments } from './debug-runtime-observations-source-v1-smoke.mjs';
import { admissionInput,admissionArguments,validateAdmission } from './debug-runtime-observations-source-v1-admission.mjs';
import { validateNativeWorkgroupInspection } from './debug-runtime-observations-source-v1-native-v5.mjs';
const clone=v=>structuredClone(v);
const inv=(lane=0,grid=4)=>({global:[lane,0,0],workgroup:[Math.floor(lane/64),0,0],local:[lane%64,0,0],
  workgroup_size:[64,1,1],workgroup_count:[Math.ceil(grid/64),1,1],launch_extent:[grid,1,1]});
const scalar=n=>({kind:'scalar',type:'U32',bits:n.toString(16).padStart(32,'0')});
function mark(lane=0,activation='2',record=10,after=false) {
  return {record,ordinal:record,capture_instance:'7',invocation:inv(lane),site:[1,0,0],phase:after?'after':'before',
    origin:{activation,attempt:'1'},frames:[
      {depth:0,function:0,block:0,next_operation:1,activation:'1',operation:{kind:'suspended',attempt:'4',site:[0,0,0]},
        parent:{kind:'root'},ssa:[[0,scalar(7)]]},
      {depth:1,function:1,block:0,next_operation:after?1:0,activation,operation:{kind:'active',attempt:'1',site:[1,0,0]},
        parent:{kind:'caller',activation:'1',attempt:'4',site:[0,0,0]},ssa:after?[[0,scalar(1)],[1,scalar(2)]]:[[0,scalar(1)]]}]};
}
const usage=()=>({origins:{rows:20,capacity:32,bytes:2048},
  frames:{records:20,rows:30,record_capacity:32,frame_capacity:64,bytes:4096},
  allocations:{records:20,transitions:4,record_capacity:32,transition_capacity:8,bytes:4096,
    validation_work_limit:1000000,validation_work_used:6},fixed_owner_bytes:1024});
const scope=group=>({kind:'workgroup',coordinate:[group,0,0],size:[64,1,1],count:[2,1,1],launch:[128,1,1]});
function reuse() {
  const a={sequence:'1',identity:{allocation:'2',storage_slot:'2',generation:'1'},kind:{kind:'create',previous_allocation:null},
    space:'Workgroup',access:'ReadWrite',alignment:4,byte_len:256,scope:scope(0),creation_site:[0,0,1]};
  const release={...clone(a),sequence:'2',kind:{kind:'release'}};
  const b={...clone(a),sequence:'3',identity:{allocation:'3',storage_slot:'2',generation:'2'},
    kind:{kind:'create',previous_allocation:'2'},scope:scope(1)};
  const snapshot=(create,record,lane)=>({record,capture_instance:'8',invocation:inv(lane,128),site:[0,0,1],
    through_sequence:create.sequence,identity:clone(create.identity),scope:clone(create.scope),
    bytes:Array(256).fill(0),initialized:Array(256).fill(false)});
  return {transitions_at_last_record:[a,release,b],first:snapshot(a,2,0),second:snapshot(b,40,64),
    old_allocation_refused_at_second:true,future_allocation_refused_at_first:true,
    historical_seek_repeat_checks:4,replay_work_used:500,usage:usage(),terminal_release_claimed:false};
}
function outputs(count,word) {
  const bytes=Buffer.alloc((count+2)*4);
  [0xdeadbeef,...Array(count).fill(word),0xcafebabe].forEach((n,i)=>bytes.writeUInt32LE(n,i*4));
  return {output_bytes:[...bytes],initialized:Array(bytes.length).fill(true),reuse_off_execution_equal:true,
    reuse_off_legacy_equal:true,records:300};
}
const flags={source_authenticated:false,hardware_observed:false,compiler_resume_authority:false};
const digest='1'.repeat(64);
function base(mode,result) {
  return {schema:'task-runtime-observations-source-observer-v1',status:'passed',mode,profile:clone(PROFILE),...flags,
    result:{bundle_sha256:digest,bundle_identity:digest,canonical_kir_sha256:digest,canonical_kir_digest:digest,
      canonical_kir_bytes:400,target:'gfx942:xnack-',...flags,...result}};
}
function workgroup() {
  return base('workgroup',{topology:{operations:10,element:'U32',elements:64,alignment:4,private_alloca:false,
    lds:{runtime_site:[0,99,1],authoring_coordinate:[0,0,1]}},reuse_on_runs:2,reuse_off_runs:2,
    cases:['canonical','seeded_71'].map(schedule=>({schedule,grid:[128,1,1],workgroup:[64,1,1],input:2,
      expected_word:128,...outputs(128,128),observations:reuse()}))});
}
function loop() {
  const cases=[];
  for(const rounds of [0,1,3])for(const schedule of ['canonical','seeded_71']) {
    const witnesses=[];
    for(let lane=0;lane<4;lane++)for(let trip=0;trip<rounds;trip++) {
      const record=10+lane*40+trip*10;
      witnesses.push({before:mark(lane,String(trip+2),record),after:mark(lane,String(trip+2),record+1,true),
        caller_ssa_unchanged:true,child_ssa_changed:true});
    }
    let navigation={checks:0,reason:'no helper activation for zero rounds'};
    if(rounds) {
      const caller=after=>({record:after?19:9,ordinal:after?19:9,capture_instance:'7',invocation:inv(),site:[0,0,0],
        phase:after?'after':'before',origin:{activation:'1',attempt:'4'},frames:[
          {depth:0,function:0,block:0,next_operation:after?1:0,activation:'1',
            operation:{kind:'active',attempt:'4',site:[0,0,0]},parent:{kind:'root'},ssa:[[0,scalar(7)]]}]});
      navigation={checks:rounds===3?8:6,first:witnesses[0].before,completed:witnesses[0].after,
        caller_before:caller(false),caller_after:caller(true),old_activation_absent_at_later_call:rounds===3,
        later_record:rounds===3?20:null,replay_work_used:99};
    }
    const expected={0:0xabcd1234,1:0x479e,3:0x479d}[rounds];
    cases.push({rounds,schedule,expected_word:expected,...outputs(4,expected),usage:usage(),
      observations:{helper_activations:rounds*4,call_attempts:rounds*4,checked_helper_operations:rounds*12,
        writes:4,witnesses,pair_scan_work:50},navigation});
  }
  return base('loop',{topology:{entry:0,helper:1,call:[0,0,0],helper_sites:[[1,0,0],[1,0,1],[1,0,2]]},
    cases,reuse_on_runs:6,reuse_off_runs:6,helper_activations:32});
}
test('bounded decimal identities preserve values above JS safe integer',()=>{
  assert.equal(u64('18446744073709551615'),18446744073709551615n);
  for(const v of ['01','-1','18446744073709551616',1,null,'1e3'])assert.throws(()=>u64(v));
  assert.throws(()=>u64('0',true));
});
test('actual suspended caller/child mock validates; depth alone never selects identity',()=>{
  checkpoint(mark());
  for(const mutate of [
    c=>c.frames[1].parent.activation='9',c=>c.frames[0].operation.attempt='9',
    c=>c.frames[1].parent.site=[0,0,9],c=>c.frames[1].activation='1',
    c=>c.origin.activation='9',c=>c.frames[1].operation.kind='ready',
    c=>c.invocation.launch_extent=[64,1,1],c=>c.frames[1].ssa.push(c.frames[1].ssa[0]),
  ]){const value=mark();mutate(value);assert.throws(()=>checkpoint(value));}
});
test('reuse mock requires actual release, fresh semantic identity, same slot, next generation',()=>{
  validateReuse(reuse());
  for(const mutate of [
    o=>o.transitions_at_last_record.splice(1,1),o=>o.transitions_at_last_record[2].identity.allocation='2',
    o=>o.transitions_at_last_record[2].identity.storage_slot='3',o=>o.transitions_at_last_record[2].identity.generation='1',
    o=>o.transitions_at_last_record[2].kind.previous_allocation='7',o=>o.transitions_at_last_record[1].byte_len=128,
    o=>o.second.initialized[0]=true,o=>o.second.bytes[0]=1,o=>o.second.scope.coordinate=[0,0,0],
    o=>o.first.capture_instance='100',o=>o.terminal_release_claimed=true,o=>o.old_allocation_refused_at_second=false,
    o=>o.usage.allocations.validation_work_used=1000001,
  ]){const value=reuse();mutate(value);assert.throws(()=>validateReuse(value));}
});
test('complete mocked WG report validators are deterministic, not compiler execution evidence',()=>{
  validateWorkgroup(workgroup(),digest);
  for(const mutate of [
    r=>r.source_authenticated=true,r=>r.result.hardware_observed=true,r=>r.result.topology.private_alloca=true,
    r=>r.profile.reuse_bytes=16384,r=>r.result.cases[0].output_bytes[0]=0,
    r=>r.result.cases[1].initialized[519]=false,r=>r.result.cases[0].reuse_off_legacy_equal=false,
    r=>r.result.bundle_sha256='2'.repeat(64),r=>r.result.cases.pop(),
  ]){const value=workgroup();mutate(value);assert.throws(()=>validateWorkgroup(value,digest));}
});
test('complete mocked loop report requires actual visits, paired origins, SSA and history',()=>{
  validateLoop(loop(),digest);
  for(const mutate of [
    r=>r.result.cases[0].observations.helper_activations=1,
    r=>r.result.cases[2].observations.witnesses[0].after.origin.attempt='8',
    r=>r.result.cases[2].observations.witnesses[0].after.frames[0].ssa[0][1]=scalar(42),
    r=>r.result.cases[2].observations.witnesses[0].after.frames[1].ssa=
      clone(r.result.cases[2].observations.witnesses[0].before.frames[1].ssa),
    r=>r.result.cases[4].navigation.old_activation_absent_at_later_call=false,
    r=>r.result.cases[4].observations.witnesses[1]=clone(r.result.cases[4].observations.witnesses[0]),
    r=>r.result.cases[3].navigation.completed.origin.activation='20',
  ]){const value=loop();mutate(value);assert.throws(()=>validateLoop(value,digest));}
});
test('CLI request exact preimages agree with Rust harness; output slices exclude both guards',()=>{
  for(const mode of ['loop','workgroup']) {
    const doc=requestDocument(mode,3),view=doc.arguments[mode==='loop'?0:1];
    assert.equal(view.byte_offset,4);assert.equal(view.elements,mode==='loop'?4:128);
    const bytes=Buffer.from(doc.shared_buffers[0].bytes.slice(2),'hex');
    assert.equal(bytes.readUInt32LE(0),0xdeadbeef);assert.equal(bytes.readUInt32LE(bytes.length-4),0xcafebabe);
    assert.ok(bytes.subarray(4,-4).every(b=>b===0xa5));
    for(const arg of doc.arguments.filter(arg=>arg.kind==='scalar')) {
      assert.deepEqual(Object.keys(arg).sort(),['bits','kind','type']);
      assert.equal(arg.type,'u32');assert.match(arg.bits,/^0x[0-9a-f]{8}$/);
    }
    if(mode==='loop')assert.equal(doc.arguments[3].bits,'0x00000003');
    else assert.equal(doc.arguments[0].bits,'0x00000002');
  }
  assert.throws(()=>requestDocument('synthetic'));assert.throws(()=>requestDocument('loop',2));
});
test('normal exporter selects exact fixture and independent fresh targets',()=>{
  const c={repo:'/repo',output:'/capture',target:'/cache/loop',workgroup_target:'/cache2/wg'};
  const loopArgs=exportArguments(c,'loop'),wgArgs=exportArguments(c,'workgroup');
  assert.equal(loopArgs[loopArgs.indexOf('--bundle-version')+1],'6');
  assert.equal(wgArgs[wgArgs.indexOf('--bundle-version')+1],'5');
  assert.equal(wgArgs[wgArgs.indexOf('--crate')+1],WG_CRATE);
  assert.equal(wgArgs[wgArgs.indexOf('--features')+1],'workgroup_reduce_u32');
  assert.equal(wgArgs[wgArgs.indexOf('--target-dir')+1],'/cache2/wg');
});
function census() {
  const source=Buffer.from('ordinary source\n');
  return {source,value:{schema:'fe2o3-diagnostic-source-census-v1',diagnosticOnly:true,qualified:false,
    authenticatesCompilerExecution:false,extractionSucceeded:true,runId:'3'.repeat(64),
    extractionMode:{kind:'simulation-bundle',version:5},workingDirectory:'/repo',
    arguments:['--crate-name',WG_CRATE,'fixture.rs'],selection:{status:'available',value:{
      target:'gfx942:xnack-',functions:[{role:'kernel-entry',exportName:'workgroup_reduce_u32',
        functionIdentity:digest,definitionIdentity:digest,monomorphizationIdentity:digest}],
      files:[{identity:digest,originalSha256:sha(source),originalBytes:source.length,normalizedBytes:source.length}]}}}};
}
test('WG source census refuses stale run, wrong bytes, target, fixture, authority or extraction mode',()=>{
  const c=census();assert.equal(validateWorkgroupCensus(c.value,'3'.repeat(64),'/repo/fixture.rs',c.source),digest);
  for(const mutate of [
    c=>c.runId='4'.repeat(64),c=>c.authenticatesCompilerExecution=true,c=>c.extractionSucceeded=false,
    c=>c.extractionMode.version=6,c=>c.arguments[2]='other.rs',
    c=>c.selection.value.files[0].originalSha256='4'.repeat(64),c=>c.selection.value.target='gfx950:xnack-',
  ]){const v=census();mutate(v.value);
    assert.throws(()=>validateWorkgroupCensus(v.value,'3'.repeat(64),'/repo/fixture.rs',v.source));}
});
test('WG inspection uses authoring roster coordinate, not runtime BlockId',()=>{
  const report=workgroup().result,summary={target:'gfx942:xnack-',canonical_kir_version:10,
    bundle_identity:digest,canonical_kir_digest:digest,operation_count:1,
    authority:{source_authenticated:false,grants_production_resume:false}};
  const ops=[{coordinate:{function:0,block:0,operation:1},kind:'workgroup_memory',
    mnemonic:null,inline_assembly_source:null}];
  validateWorkgroupAttribution(report,summary,ops);
  ops[0].coordinate.block=99;assert.throws(()=>validateWorkgroupAttribution(report,summary,ops));
});

function admissionRows() {
  const names=['hierarchy_inspection','kir_sites','source_sites','call_stack','breakpoints','watchpoints',
    'forward_step','reverse_step','pause','deterministic_replay','kir_ssa_values','source_variable_values',
    'register_values','allocation_relative_memory','semantic_trace','hardware_wave_state','kfd_dispatch_control'];
  const caps=names.map(name=>['register_values','hardware_wave_state','kfd_dispatch_control'].includes(name)
    ?{name,availability:'unavailable',reason:'not_represented'}:{name,availability:'available'});
  return ['discover_capabilities','get_state','terminate'].map((operation,i)=>({
    schema:'fe2o3-debug-response-v1',status:'ok',request_id:i+1,operation,session:{
      backend:'cpu_kir_simulator',execution_kind:'cpu_kir_simulation',state:i===2?'terminated':'stopped',
      revision:i===2?1:0,configuration_identity:digest,cursor:{configuration_identity:digest,
        event_sequence:0,state_revision:i===2?1:0},simulated:true,hardware_observed:false,performance_prediction:false},
    result:i===0?{result:'capabilities',capabilities:caps}:i===1?
      {result:'state',snapshot:{status:'unavailable',reason:'not_captured'}}:{result:'terminated'}}));
}
const admissionBytes=rows=>Buffer.from(rows.map(row=>JSON.stringify(row)).join('\n')+'\n');
test('real CLI admission handshake is closed, correlated, read-only until termination',()=>{
  const commands=admissionInput().toString().trimEnd().split('\n').map(line=>JSON.parse(line));
  assert.deepEqual(commands.map(row=>row.operation),['discover_capabilities','get_state','terminate']);
  assert.ok(commands.every(row=>row.expected_revision===0));
  assert.equal(validateAdmission(admissionBytes(admissionRows())).terminated,true);
  assert.ok(admissionArguments('workgroup','/bundle','/request').includes('--bundle-v5'));
  for(const mutate of [
    rows=>rows.pop(),rows=>rows.push(rows[2]),rows=>rows[0].request_id=2,
    rows=>rows[1].session.revision=1,rows=>rows[1].session.cursor.event_sequence=1,
    rows=>rows[1].session.configuration_identity='2'.repeat(64),rows=>rows[2].session.state='stopped',
    rows=>rows[2].session.revision=0,rows=>rows[0].session.hardware_observed=true,
    rows=>rows[1].result.snapshot={status:'captured',snapshot:{}},rows=>rows[0].extra=true,
    rows=>rows[0].result.capabilities[0].name='invented',rows=>rows[1].status='error',
  ]){const rows=admissionRows();mutate(rows);assert.throws(()=>validateAdmission(admissionBytes(rows)));}
});


test('native V5 census joins exact bytes, runtime LDS, roster coordinates and same-run source file',()=>{
  const source=Buffer.from('ordinary LDS source\n'),report=workgroup().result;
  report.topology.operations=1;
  const inspection={schema:'task-runtime-workgroup-native-v5-inspection-v1',status:'passed',
    bundle_version:5,canonical_kir_version:10,target:report.target,...flags,
    bundle_sha256:digest,bundle_identity:digest,canonical_kir_digest:digest,canonical_kir_sha256:digest,
    canonical_kir_bytes:report.canonical_kir_bytes,source_map_identity:digest,source_map_sha256:digest,
    bundle_subject_identity:digest,operation_count:1,authority:{source_authenticated:false,grants_production_resume:false},
    files:[{identity:digest,bytes:source.length,display_path:'src/lib.rs'}],
    operations:[{coordinate:{function:0,block:0,operation:1},runtime_site:[0,99,1],function_name:'workgroup_reduce_u32',
      kind:'workgroup_memory',mnemonic:null,inline_assembly_source:null,
      source_spans:[{file_identity:digest,byte_start:'0',byte_end:String(source.length)}]}]};
  validateNativeWorkgroupInspection(report,inspection,digest,source);
  for(const mutate of [
    i=>i.bundle_version=6,i=>i.canonical_kir_version=11,i=>i.bundle_sha256='2'.repeat(64),
    i=>i.operations[0].runtime_site=[0,0,1],i=>i.operations[0].coordinate.block=99,
    i=>i.files[0].identity='2'.repeat(64),i=>i.files[0].bytes++,
    i=>i.operations[0].source_spans=[],i=>i.operations[0].source_spans[0].byte_end='99999',
    i=>i.operations=[],i=>i.operations[0].kind='inline_assembly',i=>i.source_authenticated=true,
  ]){const value=clone(inspection);mutate(value);
    assert.throws(()=>validateNativeWorkgroupInspection(report,value,digest,source));}
});

test('runner refuses missing explicit options before any process or filesystem write',()=>{
  assert.throws(()=>parseArguments([]));assert.throws(()=>parseArguments(['--output','/tmp/test']));
});

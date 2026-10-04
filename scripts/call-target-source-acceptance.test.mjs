// Pure synthetic controls only. No filesystem reads, child processes, compiler or simulator.
import test from 'node:test';
import assert from 'node:assert/strict';
import { AUTHORITY, LIMITS, callSelector, observeCallEdges, parseOptions, validateCallSourceSlices,
  validateCallTarget, validatePriorReceipt, validatePriorSimulationRequests, validateRefusal } from './call-target-source-acceptance.mjs';
import { INPUTS, LABELS, LENGTHS, LIMITS as SOURCE_LIMITS, inspectInstances, oracle, sha256,
  request, validatePage } from './const-u32-helper-source-smoke.mjs';
const clone=structuredClone,H=text=>sha256(Buffer.from('synthetic-only:'+text));
const v=(value,ty='Scalar(U32)')=>({value,ty});
const span={file_identity:H('file'),display_path:'src/lib.rs',byte_start:'1',byte_end:'4',line:1,column:2};
function op(fn,index,kind,detail,inputs,results) {
  return {coordinate:{function:fn,block:0,operation:index},function_name:'opaque-function-'+fn,
    kind,semantic_detail:detail,mnemonic:null,inline_assembly_source:null,inputs,results,
    local_memory_effects:[],complete_local_effect_summary:true,convergence:'not_analyzed',traps:'not_analyzed',
    physical_resources:'unavailable_logical_canonical_stage',source_binding:'bundle_content_bound_not_authenticated',
    source_spans:[clone(span)],materialization:'unavailable_unsupported_operation_or_contract'};
}
function summary(label,operations) {
  const profile=label==='repeat'?'edited512':label;
  return {schema:'fe2o3-multilevel-authoring-observation-v1',authority:clone(AUTHORITY),
    bundle_identity:H(profile+'bundle'),bundle_subject_identity:H(profile+'subject'),
    canonical_kir_version:11,canonical_kir_digest:H(profile+'kir'),canonical_kir_bytes:'256',
    target:'gfx942:xnack-',source_map_identity:H(profile+'map'),semantic_mir_identity:H(profile+'semantic'),
    rustc_identity_inventory_receipt_sha256:H(profile+'inventory'),rustc_identity_inventory_receipt_bytes:'128',
    rustc_preflight_plan_receipt_sha256:H(profile+'preflight'),rustc_preflight_plan_receipt_bytes:'128',
    compiler_policy_identity:'unavailable_in_v6',final_artifact_identity:'unavailable_extraction_precedes_final_artifact',
    operation_count:operations.length,eliminated_source_span_count:0,capabilities:[]};
}
function fixture(label='default256') {
  // Nonzero, nonadjacent function ordinals and opaque IDs forbid index/name assumptions.
  const caller=37, helpers=label==='two'?[91,103]:[91];
  const operations=[op(caller,0,'binary','BitXor',[v(10),v(11)],[v(12)]),
    op(caller,1,'constant','U32(255)',[],[v(13)]),
    op(caller,2,'binary','BitAnd',[v(12),v(13)],[v(14)])];
  const bits=label==='two'?[256,512]:[label==='default256'||label==='baseline'?256:512];
  if(label==='baseline') {
    operations.push(op(caller,3,'constant','U32(256)',[],[v(15)]),
      op(caller,4,'binary','BitOr',[v(14),v(15)],[v(16)]),
      op(caller,5,'store',null,[v(99,'Pointer(Global)'),v(16)],[]));
  } else {
    operations.push(op(caller,3,'call',null,[v(14)],[v(16)]));
    if(label==='two') operations.push(op(caller,4,'call',null,[v(10)],[v(17)]),
      op(caller,5,'binary','BitXor',[v(16),v(17)],[v(18)]));
    operations.push(op(caller,label==='two'?6:4,'store',null,[v(99,'Pointer(Global)'),v(label==='two'?18:16)],[]));
    for(let index=0;index<helpers.length;index++) {
      const constant=op(helpers[index],0,'constant','U32('+bits[index]+')',[],[v(3)]);
      const assembly=op(helpers[index],1,'inline_assembly',null,[v(0),v(3)],[v(4)]);
      assembly.mnemonic='v_or_b32';assembly.inline_assembly_source={frontend_unit:H((label==='repeat'?'edited512':label)+'unit'),
        function:H('instance'+bits[index]),contract:H('contract'),statement:H('statement'+bits[index]),
        authority:'inert_references_not_source_authentication'};
      operations.push(constant,assembly);
    }
  }
  const identity=summary(label,operations),calls=operations.filter(item=>item.kind==='call');
  const reports=calls.map((call,index)=>({schema:'fe2o3-authoring-call-target-v1',authority:clone(AUTHORITY),
    selector:callSelector(identity,call),call:clone(call),
    caller:{function:caller,function_id:'opaque-function-'+caller,role:'kernel_entry',
      kernel_registrations:[{kernel:7,kernel_id:'launch-profile',entry_function_id:'opaque-function-'+caller}]},
    callee:{function:helpers[index],function_id:'opaque-function-'+helpers[index],role:'internal_helper'},
    arguments:call.inputs.map((input,position)=>({position,operand:clone(input),formal:v(position)})),
    results:call.results.map((result,position)=>({position,result:clone(result),signature_type:result.ty})),
    correspondence:'exact_retained_call_operand_to_formal_position',transitive_helper_closure:'not_traversed',
    dynamic_invocation:'unavailable_static_call_site_only',physical_abi:'unavailable_logical_canonical_call'}));
  return {label,summary:identity,operations,calls,reports};
}
function observe(data) {return observeCallEdges(data.label,data.summary,data.operations,data.reports,'launch-profile');}
function resetReports(data) {
  data.summary.operation_count=data.operations.length;
  data.calls=data.operations.filter(item=>item.kind==='call');
  for(let i=0;i<data.reports.length&&i<data.calls.length;i++) {
    data.reports[i].call=clone(data.calls[i]);data.reports[i].selector=callSelector(data.summary,data.calls[i]);
  }
}
function priorReceipt() {
  const variants=LABELS.map(label=>{
    const data=fixture(label),profile=label==='repeat'?'edited512':label;
    return {label,source_sha256:H(profile+'source'),bundle_file_sha256:H(profile+'bytes'),bundle_bytes:128,
      summary:data.summary,instances:inspectInstances(label,data.summary,data.operations),
      simulations:INPUTS.flatMap((inputs,input)=>LENGTHS.flatMap(elements=>[0,1].map(replay=>({
        input,elements,replay,expected_word:oracle(label,inputs),output_words:elements,backing_bytes:8+4*elements,guard_bytes:8})))),
      exact_kernel_to_helper_call_edges:'unavailable_in_current_public_operation_projection'};
  });
  const stages=[];
  function stage(label,code=0) {
    stages.push({label,executable:'/synthetic-only/tool',args:['synthetic-only'],stdin_bytes:0,stdin_sha256:H('empty'),
      code,signal:null,reason:null,elapsed_ms:1,stdout_bytes:0,stdout_sha256:H('empty'),stderr_bytes:0,stderr_sha256:H('empty')});
  }
  for(const label of LABELS) {
    stage(label+'-export');stage(label+'-inspect');stage(label+'-operations-0');
    for(let input=0;input<INPUTS.length;input++)for(const elements of LENGTHS)for(const replay of [0,1])
      stage(label+'-case-'+input+'-length-'+elements+'-replay-'+replay);
  }
  stage('materialize');
  for(const name of ['constant-argv','constant-selector','stale-canonical'])stage('refuse-'+name,1);
  return {schema:'task-const-u32-source-acceptance-v1',status:'passed',
    origin:'current_ordinary_source_normal_bundle_v6_and_author_cli',authority:'observation_only',
    fixture:'crates/rustc-codegen-fe2o3/tests/fixtures/ordinary-bitwise-promotion-v1',
    original_source_sha256:variants[0].source_sha256,selector:callSelector(variants[0].summary,fixture('baseline').operations[4]),
    materialized_helper_sha256:H('helper'),actual_exports:5,whole_kernel_simulations:150,exact_cli_refusals:3,variants,stages,
    retained_file_pins:[{path:'/synthetic-only/tool',bytes:1,sha256:H('tool'),device:'1',inode:'2',mode:'33261',nlink:'1',mtime_ns:'3',ctime_ns:'4'}],
    retained_pin_bytes:1,distinct_scalar_function_instances:'observed_typed_u32_constants_and_inert_function_references',
    exact_kernel_to_helper_call_edges:'unavailable_in_current_public_operation_projection',
    source_authentication:false,compiler_closure_attestation:false,protected_proof:false,production_resume:false,
    hardware_observed:false,native_qualified:false,physical_register_or_helper_abi_qualified:false,milestone_completion:false,
    limits:clone(SOURCE_LIMITS),task_resource_accounting:'external_current_scope_supervisor_required'};
  }
const result=(stderr)=>({code:1,signal:null,reason:null,stdout:Buffer.alloc(0),stderr:Buffer.from(stderr)});
const help=Buffer.from('usage: fe2o3-author inspect\n       fe2o3-author call-target --selector JSON\nRead exact bytes.\n');
function options() {
  return ['--repo','/synthetic/repo','--author','/synthetic/bin/author','--library-dir','/synthetic/lib',
    '--capture','/synthetic/capture','--receipt-bytes','128','--receipt-sha256',H('receipt'),
    '--mode','current_tool_capture','--output','/synthetic/new-output'];
}

test('exact scalar specialization edges use discovered nonzero ordinals and opaque same-owner IDs',()=>{
  for(const label of LABELS) {
    const data=fixture(label),before=clone(data),seen=observe(data);
    assert.deepEqual(data,before);assert.equal(seen.caller_function,37);
    assert.equal(seen.edges.length,label==='baseline'?0:label==='two'?2:1);
    assert.equal(seen.registered_kernel_relation,label==='baseline'?'unavailable_in_current_public_observation':'exact_same_owner_kernel_registration_to_caller');
    assert.equal(seen.transitive_helper_closure,'not_traversed');
    assert.equal(seen.body_return_mapping,'unavailable_not_projected');
    assert.equal(seen.helper_call_operations,0);
  }
});
test('repeat preserves exact reports and static edges without claiming a dynamic occurrence',()=>{
  const edited=fixture('edited512'),repeat=fixture('repeat');
  assert.deepEqual(edited.reports,repeat.reports);assert.deepEqual(observe(edited),observe(repeat));
  assert.equal(edited.reports[0].dynamic_invocation,'unavailable_static_call_site_only');
});
test('positional projection preserves duplicate operands but not duplicate formal definitions',()=>{
  const data=fixture(),call=clone(data.calls[0]),report=clone(data.reports[0]);
  call.inputs.push(clone(call.inputs[0]));report.call=clone(call);
  report.arguments.push({position:1,operand:clone(call.inputs[1]),formal:v(1)});
  assert.equal(validateCallTarget(report,data.summary,call).arguments.length,2);
  assert.equal(report.arguments[0].operand.value,report.arguments[1].operand.value);
  report.arguments[1].formal.value=0;
  assert.throws(()=>validateCallTarget(report,data.summary,call),/distinct formal/u);
});
test('report identity, same-owner call, types, arity and authority cannot drift',()=>{
  const data=fixture();
  for(const mutate of [
    r=>r.selector.bundle_identity=H('other'),r=>r.selector.canonical_kir_digest=H('other'),
    r=>r.selector.target='gfx950:xnack-',r=>r.selector.operations[0].function=0,
    r=>r.call.inputs[0].value++,r=>r.arguments[0].operand.value++,r=>r.arguments[0].formal.ty='Scalar(U64)',
    r=>r.arguments[0].position=1,r=>r.arguments=[],r=>r.results[0].result.value++,
    r=>r.results[0].signature_type='Scalar(I32)',r=>r.results.push(clone(r.results[0])),
    r=>r.callee.role='kernel_entry',r=>r.authority.grants_proof_authority=true,
    r=>r.dynamic_invocation='1',r=>r.physical_abi='v0',r=>r.caller={function:0},
  ]) {
    const report=clone(data.reports[0]);mutate(report);
    assert.throws(()=>validateCallTarget(report,data.summary,data.calls[0]));
  }
});
test('renamed helper targets and unrelated function ordinals are not accepted by display similarity',()=>{
  for(const mutate of [d=>d.reports[0].callee.function=103,
    d=>d.reports[0].callee.function_id+='-same-prefix',
    d=>d.operations.at(-1).function_name+='-different',
    d=>d.reports[0].arguments[0].formal.value=1]) {
    const data=fixture();mutate(data);assert.throws(()=>observe(data));
  }
});
test('partial call census, extra helpers and nested helper calls refuse',()=>{
  const missing=fixture('two');missing.reports.pop();assert.throws(()=>observe(missing));
  const nested=fixture();nested.operations.push(op(91,2,'call',null,[v(0)],[v(9)]));
  nested.summary.operation_count++;nested.reports.push(clone(nested.reports[0]));
  assert.throws(()=>observe(nested),/nested Call/u);
  const unrelated=fixture();unrelated.operations.push(op(120,0,'constant','U32(1)',[],[v(0)]));
  unrelated.summary.operation_count++;assert.throws(()=>observe(unrelated),/unrelated operation-bearing/u);
  const extra=fixture();extra.operations.splice(4,0,op(37,4,'call',null,[v(14)],[v(17)]));
  extra.operations[5].coordinate.operation=5;extra.summary.operation_count++;extra.reports.push(clone(extra.reports[0]));
  assert.throws(()=>observe(extra),/static Call count/u);
});
test('swapped const instances and caller operand/result paths cannot pass the finite source profile',()=>{
  for(const mutate of [
    d=>{[d.reports[0].callee,d.reports[1].callee]=[d.reports[1].callee,d.reports[0].callee];},
    d=>{d.calls[0].inputs[0]=v(10);d.reports[0].arguments[0].operand=v(10);resetReports(d);},
    d=>{d.calls[1].inputs[0]=v(11);d.reports[1].arguments[0].operand=v(11);resetReports(d);},
    d=>{d.operations.find(o=>o.kind==='store').inputs[1]=v(16);},
  ]) {
    const data=fixture('two');mutate(data);assert.throws(()=>observe(data));
  }
});
test('actual typed constants cannot be inferred through aliases, later or foreign block definitions',()=>{
  for(const mutate of [
    d=>d.operations.find(o=>o.coordinate.function===91&&o.kind==='constant').semantic_detail='I32(256)',
    d=>d.operations.find(o=>o.coordinate.function===91&&o.kind==='constant').kind='cast',
    d=>d.operations.find(o=>o.coordinate.function===91&&o.kind==='constant').coordinate.block=1,
    d=>d.operations.find(o=>o.coordinate.function===91&&o.kind==='constant').coordinate.operation=2,
    d=>d.operations.find(o=>o.coordinate.function===91&&o.kind==='constant').semantic_detail='U32(512)',
  ]) {
    const data=fixture();mutate(data);assert.throws(()=>observe(data));
  }
});
test('source spans preserve exact caller/helper owner identity and actual source substring bytes',()=>{
  const data=fixture('two'),source=Buffer.from('header\nspecialized_or::<256u32>(low) ^ specialized_or::<512u32>(a);\n');
  for(let i=0;i<data.calls.length;i++) {
    const expr='specialized_or::<'+(i?512:256)+'u32>('+(i?'a':'low')+')';
    const start=source.indexOf(expr),prefix=source.subarray(0,start).toString();
    data.calls[i].source_spans=[{...clone(span),byte_start:String(start),byte_end:String(start+Buffer.byteLength(expr)),
      line:prefix.split('\n').length,column:prefix.split('\n').at(-1).length+1}];
  }
  resetReports(data);const seen=observe(data);
  assert.equal(validateCallSourceSlices('two',source,seen).call_spans_checked,2);
  for(const mutate of [
    o=>o.edges[0].call_source_spans[0].byte_start='0',
    o=>o.edges[1].call_source_spans[0].display_path='different.rs',
    o=>o.edges[0].call_source_spans[0].line++,
    o=>o.edges[1].call_source_spans[0].column++,
  ]) {const bad=clone(seen);mutate(bad);assert.throws(()=>validateCallSourceSlices('two',source,bad));}
  const changed=Buffer.from(source);changed[source.indexOf('256')]=57;
  assert.throws(()=>validateCallSourceSlices('two',changed,seen));
  const bad=fixture();bad.calls[0].source_spans=[];resetReports(bad);assert.throws(()=>observe(bad));
});
test('same exact call shape may legitimately contain no argument or result without invented return values',()=>{
  const data=fixture(),call=clone(data.calls[0]),report=clone(data.reports[0]);
  call.inputs=[];call.results=[];report.call=clone(call);report.arguments=[];report.results=[];
  assert.equal(validateCallTarget(report,data.summary,call).results.length,0);
});
test('fixed operation pages reject coherent shortened pages before deriving a helper roster',()=>{
  const data=fixture(),page={authority:clone(AUTHORITY),bundle_identity:data.summary.bundle_identity,
    canonical_kir_digest:data.summary.canonical_kir_digest,target:data.summary.target,start:0,next_start:null,
    total_operations:data.operations.length,operations:clone(data.operations)};
  assert.equal(validatePage(page,data.summary,0),data.operations.length);
  page.operations=page.operations.slice(0,2);page.next_start=2;
  assert.throws(()=>validatePage(page,data.summary,0),/exact fixed-limit/u);
});
test('historical source receipt remains its original schema with explicit unavailable old edge claim',()=>{
  const receipt=priorReceipt(),before=clone(receipt),checked=validatePriorReceipt(receipt);
  assert.deepEqual(receipt,before);assert.equal(checked.stages.size,169);assert.equal(checked.pins.size,1);
  assert.equal(receipt.exact_kernel_to_helper_call_edges,'unavailable_in_current_public_operation_projection');
});
test('source receipt counts, exact prior oracle metadata, repeated source identities and flags are checked',()=>{
  for(const mutate of [
    r=>r.schema='relabelled-v2',r=>r.whole_kernel_simulations=151,
    r=>r.exact_kernel_to_helper_call_edges='newly-proved',r=>r.hardware_observed=true,
    r=>r.variants[2].simulations[0].expected_word=256,r=>r.variants[3].bundle_file_sha256=H('changed-repeat'),
    r=>r.variants[3].summary.semantic_mir_identity=H('changed-repeat'),
    r=>r.retained_pin_bytes++,r=>r.retained_file_pins.push(clone(r.retained_file_pins[0])),
    r=>r.stages[0].reason='timeout',r=>r.stages[0].code=1,r=>r.stages[0].signal='SIGKILL',
    r=>r.stages.push(clone(r.stages[0])),r=>r.extra_authority=true,
  ]) {const receipt=priorReceipt();mutate(receipt);assert.throws(()=>validatePriorReceipt(receipt));}
});
test('each typed refusal requires exact relevant diagnostics, empty stdout and healthy transport',()=>{
  const messages={stale_bundle:'stale or malformed exact V6 bundle identity',
    stale_canonical:'stale or malformed exact canonical V11 identity',
    target:'selector target differs from the exact bundle target',not_call:'selected canonical operation is not a direct Call'};
  for(const [kind,message] of Object.entries(messages)) {
    const good=result('fe2o3-author: '+message+'\n');validateRefusal(good,kind);
    for(const mutate of [r=>r.code=0,r=>r.reason='stdin:EPIPE',r=>r.signal='SIGKILL',
      r=>r.stdout=Buffer.from('{}'),r=>r.stderr=Buffer.from('unrelated error\n'),
      r=>r.stderr=Buffer.from('unrelated error\nfe2o3-author: '+message+'\n')]) {
      const bad={...good};mutate(bad);assert.throws(()=>validateRefusal(bad,kind));
    }
  }
});
test('caller-injected callee and argv overrides cannot pass on unrelated parsing failures',()=>{
  validateRefusal(result('fe2o3-author: invalid selector: unknown field '+String.fromCharCode(96)+'callee'+String.fromCharCode(96)+
    ', expected one of '+['bundle_identity','canonical_kir_digest','target','operations'].map(x=>String.fromCharCode(96)+x+String.fromCharCode(96)).join(', ')+
    ' at line 1 column 312\n'),'selector_callee');
  assert.throws(()=>validateRefusal(result('fe2o3-author: invalid selector: unexpected EOF\n'),'selector_callee'));
  validateRefusal(result('fe2o3-author: '+help.toString()),'argv_callee',help);
  assert.throws(()=>validateRefusal(result('fe2o3-author: '+help.toString()+'unrelated\n'),'argv_callee',help));
  assert.throws(()=>validateRefusal(result('fe2o3-author: '+help.toString()),'argv_callee'));
});
test('CLI paths, modes, input/output separation and receipt pins are bounded before I/O',()=>{
  const parsed=parseOptions(options());assert.ok(Object.isFrozen(parsed));assert.equal(parsed['receipt-bytes'],128);
  for(const mutate of [
    a=>a.push('--extra','x'),a=>a[1]='relative',a=>a[15]='/synthetic/capture/child',
    a=>a[15]='/synthetic',a=>a[9]='00128',a=>a[9]=String(2**20+1),
    a=>a[11]='0'.repeat(64),a=>a[13]='fresh-by-status-only',a=>a[2]='--repo',
  ]) {const args=options();mutate(args);assert.throws(()=>parseOptions(args));}
  assert.equal(LIMITS.stages,64);assert.equal(LIMITS.command_ms,30000);assert.equal(LIMITS.output_bytes,64*1024**2);
});


test('caller registration uses exact ID and role, preserves multiple launch IDs and never assumes ordinal zero',()=>{
  const data=fixture('two');
  for(const report of data.reports) report.caller.kernel_registrations.push({
    kernel:12,kernel_id:'different-launch-id',entry_function_id:report.caller.function_id});
  const seen=observe(data);
  assert.equal(seen.caller.function,37);assert.equal(seen.caller_role,'kernel_entry');
  assert.equal(seen.requested_kernel_id,'launch-profile');
  assert.deepEqual(seen.caller.kernel_registrations.map(row=>row.kernel),[7,12]);
  assert.deepEqual(seen.caller.kernel_registrations.map(row=>row.kernel_id),['launch-profile','different-launch-id']);
  for(const mutate of [
    r=>r.caller.function=0,r=>r.caller.function_id='looks-like-kernel-entry',
    r=>r.caller.kernel_registrations=[],r=>r.caller.kernel_registrations[0].entry_function_id='other-caller',
    r=>r.caller.kernel_registrations[1].kernel=7,
    r=>r.caller.kernel_registrations[1].kernel_id='launch-profile',
    r=>r.caller.kernel_registrations.reverse(),r=>r.caller.extra_authority=true,
    r=>r.caller.role='external_import',r=>r.caller.role='device_ffi_export',
  ]) {
    const report=clone(data.reports[0]);mutate(report);
    assert.throws(()=>validateCallTarget(report,data.summary,data.calls[0]));
  }
  const unmatched=clone(data);for(const report of unmatched.reports)
    report.caller.kernel_registrations[0].kernel_id='same-looking-but-wrong-launch';
  assert.throws(()=>observe(unmatched),/simulator-request KernelId/u);
  const divergent=clone(data);divergent.reports[1].caller.kernel_registrations.pop();
  assert.throws(()=>observe(divergent),/one exact caller registration roster/u);
});
test('generic non-kernel callers remain valid observations but cannot satisfy the kernel-entry acceptance profile',()=>{
  for(const role of ['internal_helper']) {
    const data=fixture();data.reports[0].caller.role=role;data.reports[0].caller.kernel_registrations=[];
    validateCallTarget(data.reports[0],data.summary,data.calls[0]);
    assert.throws(()=>observe(data),/actual kernel-entry caller/u);
  }
  const data=fixture();data.reports[0].caller.kernel_registrations=Array.from({length:64},(_,index)=>({
    kernel:index,kernel_id:index===63?'launch-profile':'other-'+index,entry_function_id:'opaque-function-37'}));
  assert.equal(observe(data).caller.kernel_registrations.length,64);
  data.reports[0].caller.kernel_registrations.push({kernel:64,kernel_id:'extra',entry_function_id:'opaque-function-37'});
  assert.throws(()=>observe(data));
});
function priorRequestRecords(label,identity,capture='/synthetic/capture') {
  return INPUTS.flatMap((inputs,input)=>LENGTHS.map(elements=>{
    const query=request(inputs,elements),name=label+'-case-'+input+'-length-'+elements;
    const bytes=Buffer.alloc(8+4*elements,0xa5),initialized=Buffer.alloc(Math.ceil(bytes.length/8));
    for(let index=0;index<elements;index++)bytes.writeUInt32LE(oracle(label,inputs),4+4*index);
    for(let offset=4;offset<4+4*elements;offset++)initialized[offset>>3]|=1<<(offset&7);
    const result={schema:'fe2o3-simulation-result-v1',status:'ok',authority:'observation_only',simulated:true,
      hardware_observed:false,hardware_validation:false,performance_prediction:false,
      kir:{sha256:identity.canonical_kir_digest,canonical_bytes:Number(identity.canonical_kir_bytes)},
      arguments:clone(query.arguments),counts:{arguments:3,shared_buffers:1,invocations_executed:query.grid[0],workgroups_visited:query.grid[0]/64},
      target_profile:{identity:'amdgpu_64_little_endian_v1',index_bits:64},
      schedule:{coverage:{complete:true},transcript_sha256:H('synthetic-transcript')},
      shared_buffers:[{id:1,buffer:{element:'u32',access:'read_write',alignment:4,bytes:'0x'+bytes.toString('hex'),
        initialized:'0x'+initialized.toString('hex')}}]};
    const resultBytes=Buffer.from(JSON.stringify(result));
    return {input,elements,request_bytes:Buffer.from(JSON.stringify(query)),replays:[0,1].map(replay=>({
      stage:{label:name+'-replay-'+replay,executable:'/synthetic-only/simulator',
        args:['--bundle-v6',capture+'/'+label+'.fe2sim','--request',capture+'/'+name+'.request.json'],
        stdin_bytes:0,stdin_sha256:sha256(Buffer.alloc(0)),code:0,signal:null,reason:null,elapsed_ms:1,
        stdout_bytes:resultBytes.length,stdout_sha256:sha256(resultBytes),stderr_bytes:0,stderr_sha256:sha256(Buffer.alloc(0))},
      result_bytes:Buffer.from(resultBytes)}))};
  }));
}
function replaceRetainedResult(observation,mutate) {
  const result=JSON.parse(observation.result_bytes.toString());mutate(result);
  observation.result_bytes=Buffer.from(JSON.stringify(result));
  observation.stage.stdout_bytes=observation.result_bytes.length;
  observation.stage.stdout_sha256=sha256(observation.result_bytes);
}
test('retained request/result bytes revalidate all 150 original complete-buffer oracles without simulator execution',()=>{
  let count=0;
  for(const label of LABELS) {
    const data=fixture(label),records=priorRequestRecords(label,data.summary);
    const seen=validatePriorSimulationRequests(label,data.summary,records,'/synthetic/capture');
    assert.equal(seen.kernel_id,request(INPUTS[0],0).kernel);
    assert.equal(seen.request_records,15);assert.equal(seen.simulations_revalidated,30);
    assert.equal(seen.simulations_executed,0);count+=seen.simulations_revalidated;
    for(const item of seen.simulations)assert.equal(item.expected_word,oracle(label,INPUTS[item.input]));
  }
  assert.equal(count,150);
});
test('retained simulator paths, kernel requests, case census, KIR, canaries and raw-byte identity cannot drift',()=>{
  const data=fixture('two'),identity=data.summary;
  for(const mutate of [
    records=>records.pop(),records=>records.push(records[0]),
    records=>records[1].input=4,records=>records[0].replays.pop(),
    records=>records[0].replays[0].stage.args[1]='/synthetic/capture/default256.fe2sim',
    records=>records[0].replays[0].stage.args[3]='/synthetic/capture/other.request.json',
    records=>records[0].replays[0].stage.label+='-different',
    records=>records[0].replays[0].stage.reason='timeout',
    records=>records[0].replays[0].stage.stdout_sha256=H('wrong'),
    records=>{const query=JSON.parse(records[0].request_bytes.toString());query.kernel='guessed-caller-name';
      records[0].request_bytes=Buffer.from(JSON.stringify(query));},
    records=>replaceRetainedResult(records[0].replays[0],r=>r.kir.sha256=H('stale')),
    records=>replaceRetainedResult(records[0].replays[0],r=>r.hardware_observed=true),
    records=>replaceRetainedResult(records[0].replays[0],r=>r.shared_buffers[0].buffer.bytes='0x00'+r.shared_buffers[0].buffer.bytes.slice(4)),
    records=>replaceRetainedResult(records[1].replays[0],r=>r.shared_buffers[0].buffer.initialized='0x00'),
    records=>replaceRetainedResult(records[2].replays[1],r=>r.shared_buffers[0].buffer.bytes='0x'+
      'a5'.repeat(4)+'00000000'+r.shared_buffers[0].buffer.bytes.slice(18)),
  ]) {
    const records=priorRequestRecords('two',identity);mutate(records);
    assert.throws(()=>validatePriorSimulationRequests('two',identity,records,'/synthetic/capture'));
  }
});

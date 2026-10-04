// Synthetic pure controls only; no retained-source/compiler/native execution claim.
import test from 'node:test';
import assert from 'node:assert/strict';
import { REPORT_KEYS, ORIGIN_BYTES_V1, originOutputPathV1, repeatExportArgumentsV1,
  validateRepeatExportProfileV1, validateRepeatOriginV1 } from '../ordered-repeat-origin-v1.mjs';
import { LABELS, REFUSALS, options, LIMITS } from '../ordered-repeat-source-smoke.mjs';
import { stageLabels } from '../ordered-repeat-llvm-observation.mjs';
const H=byte=>byte.repeat(64);
function fixture(){
  const inspection={declared_target:'gfx942:xnack-',declared_wave_width:64,
    canonical:{sha256:H('1'),bytes:12},coordinate:{function_ordinal:0,block_ordinal:1,operation_ordinal:2},
    raw_block_id:9,declared_source_ids:{frontend_unit:H('2'),function:H('3'),contract:H('4'),statement:H('5')},
    declared_program:{count:2,descriptors:[8,201,...Array(14).fill(0)]},
    register_plan:{scratch:32,output:33,inputs:[34,35,36]}};
  const value=Object.fromEntries(REPORT_KEYS.map(key=>[key,null]));
  Object.assign(value,{schema:'fe2o3-diagnostic-ordered-program-origin-v1',diagnostic_only:true,
    authenticates_source:false,authenticates_compiler_execution:false,grants_proof_resume_artifact_launch_authority:false,
    stage:'pre_ranked_diagnostic',target:'gfx942:xnack-',wave_width:64,canonical_version:17,
    canonical_sha256:H('1'),canonical_bytes:12,semantic_version:32,semantic_sha256:H('6'),
    source_inventory_sha256:H('7'),source_preflight_sha256:H('8'),root_function_sha256:H('3'),
    root_monomorphization_sha256:H('9'),rustc_mir_body_sha256:H('a'),rustc_mir_block:4,
    semantic_block_identity:H('b'),semantic_function:0,semantic_block:2,kir_roster_coordinate:[0,1,2],
    kir_raw_block:9,declared_source_ids:structuredClone(inspection.declared_source_ids),
    origin_association:'retained_semantic_correspondence_and_live_rustc_block_identity',
    origin_scope:'whole_ordered_region',
    expansion:{file_identity:H('c'),byte_start:2,byte_end:6,line_start:1,column_start:2,line_end:1,column_end:6},
    call_site:{file_identity:H('d'),byte_start:10,byte_end:15,line_start:3,column_start:0,line_end:3,column_end:5},
    expansion_chain_sha256:H('e'),expansion_depth:2,
    macro_expansion_frames:'unavailable_only_digest_and_depth_retained',
    fine_step_origins:'unavailable_flat_descriptor_program',
    declared_instructions:[8,201].map((descriptor,ordinal)=>({ordinal,descriptor,source_association:'whole_ordered_region_only'})),
    declared_register_roles:{scratch:32,output:33,inputs:[34,35,36]},
    compiler_policy_identity:'unavailable_in_this_diagnostic',source_map_identity:'unavailable_no_debug_map_exported',
    edit_epoch:'unavailable',schedule_identity:'unavailable',final_artifact:'unavailable_no_native_compilation',
    physical_register_values:'unavailable',physical_register_lifetimes:'unavailable',
    limits:{source_reobservation_work:1048576,source_reobservation_work_used:8,maximum_expansion_depth:256,
      report_bytes:16384,rustc_internal_allocations_accounted:false}});
  const exported={canonical_sha256:value.canonical_sha256,canonical_bytes:value.canonical_bytes,
    retained_source_inventory:value.source_inventory_sha256,retained_source_preflight:value.source_preflight_sha256,
    semantic_identity:value.semantic_sha256};
  return {value,inspection,exported,raw:Buffer.alloc(12)};
}

const bytes = value => Buffer.from(JSON.stringify(value));
const validate = f => validateRepeatOriginV1(bytes(f.value),f.exported,f.inspection,f.raw);
function capture(selected='legacy') {
  const directory='/capture';
  const variants=LABELS.map((label,index)=>({label,source_path:directory+'/'+LABELS[Math.min(index,2)]+'-source/src/lib.rs'}));
  const refusals=REFUSALS.map(item=>({label:item.label,source_path:directory+'/refuse-'+item.label+'-source/src/lib.rs'}));
  const stages=stageLabels().map(label=>({label,args:[]}));
  for(const value of variants) stages.find(stage=>stage.label===value.label+'-export').args=
    repeatExportArgumentsV1(selected,directory,value.label,value.source_path);
  for(const value of refusals) stages.find(stage=>stage.label==='refuse-'+value.label+'-export').args=
    repeatExportArgumentsV1(selected,directory,'refuse-'+value.label,value.source_path,true);
  const retained_file_pins=selected==='legacy'?[]:LABELS.map(label=>({
    path:originOutputPathV1(directory,label),bytes:100,sha256:H('f')}));
  return {variants,refusals,stages,retained_file_pins};
}
test('CLI preserves exact default options and only accepts explicit v1 selection',()=>{
  const old={repo:'/repo','bin-dir':'/tools',cargo:'/cargo',rustc:'/rustc',output:'/capture'};
  const args=Object.entries(old).flatMap(([key,value])=>['--'+key,value]);
  assert.deepEqual(options(args),old);
  assert.deepEqual(options([...args,'--ordered-origin','v1']),{...old,'ordered-origin':'v1'});
  for(const extra of [['--ordered-origin','v2'],['--ordered-origin',''],['--ordered-origin'],
    ['--origin','v1'],['--ordered-origin','v1','--ordered-origin','v1']])
    assert.throws(()=>options([...args,...extra]));
  assert.throws(()=>options([...args.slice(2),'--ordered-origin','v1']));
  assert.equal(LIMITS.stages,136);assert.equal(LIMITS.pins,512);assert.equal(ORIGIN_BYTES_V1,16384);
});
test('two exact profiles preserve all eight negative command vectors and old positive bytes',()=>{
  const legacy=['--diagnostic-kir-v17','--crate','fe2o3_assembly_authoring_v30_fixture',
    '--output','/capture/one.kir','--target','gfx942','--target-dir','/capture/one-extraction',
    '--','--manifest-path','/capture/one-source/Cargo.toml','--lib','--offline'];
  assert.deepEqual(repeatExportArgumentsV1('legacy','/capture','one','/capture/one-source/src/lib.rs'),legacy);
  assert.deepEqual(repeatExportArgumentsV1('origin-v1','/capture','one','/capture/one-source/src/lib.rs'),
    [...legacy.slice(0,9),'--diagnostic-ordered-origin-v1','/capture/one.origin.json',...legacy.slice(9)]);
  for(const selected of ['legacy','origin-v1']) assert.equal(validateRepeatExportProfileV1(capture(selected),'/capture'),selected);
  for(const item of REFUSALS) {
    const label='refuse-'+item.label,source='/capture/'+label+'-source/src/lib.rs';
    const a=repeatExportArgumentsV1('legacy','/capture',label,source,true);
    assert.deepEqual(repeatExportArgumentsV1('origin-v1','/capture',label,source,true),a);
    assert.equal(a.includes('--diagnostic-ordered-origin-v1'),false);
    assert.deepEqual(a.slice(-1),['--message-format=json']);
  }
});
const commandMutations=[
  c=>c.stages.find(x=>x.label==='two-export').args.splice(9,2),
  c=>c.stages.find(x=>x.label==='one-export').args.splice(9,2),
  c=>c.stages.find(x=>x.label==='one-export').args.push('--extra'),
  c=>c.stages.find(x=>x.label==='one-export').args.splice(9,0,'--diagnostic-ordered-origin-v1','/capture/one.origin.json'),
  c=>c.stages.find(x=>x.label==='two-export').args[10]='/capture/one.origin.json',
  c=>c.stages.find(x=>x.label==='one-export').args[9]='--diagnostic-ordered-origin-v2',
  c=>c.stages.find(x=>x.label==='one-export').args[6]='gfx950',
  c=>c.stages.find(x=>x.label==='one-export').args.splice(9,2,'/capture/one.origin.json','--diagnostic-ordered-origin-v1'),
  c=>c.stages.find(x=>x.label==='refuse-zero-export').args.splice(9,0,'--diagnostic-ordered-origin-v1','/capture/refuse-zero.origin.json'),
  c=>c.stages[1]=structuredClone(c.stages[0]),
  c=>c.stages.pop(),
  c=>c.variants.reverse(),
  c=>c.variants[3].source_path='/capture/repeat-source/src/lib.rs',
  c=>c.retained_file_pins.pop(),
  c=>c.retained_file_pins[0].bytes=16385,
  c=>c.retained_file_pins[0].bytes=0,
  c=>c.retained_file_pins[0].sha256=H('0'),
  c=>c.retained_file_pins.push(structuredClone(c.retained_file_pins[0])),
  c=>c.retained_file_pins.push({path:'/capture/refuse-zero.origin.json',bytes:100,sha256:H('f')}),
];
for(const [index,mutate]of commandMutations.entries())test('closed profile refuses argv/sidecar mutation '+index,()=>{
  const c=capture('origin-v1');mutate(c);assert.throws(()=>validateRepeatExportProfileV1(c,'/capture'));
});
test('legacy profile does not admit origin pins or unsupported labels/paths/profiles',()=>{
  const c=capture();c.retained_file_pins.push({path:'/capture/one.origin.json',bytes:1,sha256:H('f')});
  assert.throws(()=>validateRepeatExportProfileV1(c,'/capture'));
  for(const selected of ['v1','origin-v2','',null])assert.throws(()=>repeatExportArgumentsV1(selected,'/capture','one','/capture/one-source/src/lib.rs'));
  for(const label of ['../one','one/other','five','refuse-other'])assert.throws(()=>originOutputPathV1('/capture',label));
  assert.throws(()=>originOutputPathV1('/capture/../capture','one'));
});
test('strict report join retains whole-region spans and compares all live exporter identities',()=>{
  const f=fixture(),before=structuredClone(f.value);
  assert.deepEqual(validate(f),before);assert.deepEqual(f.value,before);
  for(const field of ['canonical_sha256','semantic_identity','retained_source_inventory','retained_source_preflight']){
    const changed=fixture();changed.exported[field]=H('f');assert.throws(()=>validate(changed));
  }
  const changed=fixture();changed.exported.canonical_bytes++;assert.throws(()=>validate(changed));
  f.value.expansion_depth=0;assert.equal(validate(f).expansion_depth,0);
});
const reportMutations=[
  v=>{v.canonical_sha256=H('2');},v=>{v.canonical_bytes++;},v=>{v.canonical_version=16;},
  v=>{v.kir_roster_coordinate[1]++;},v=>{v.kir_raw_block++;},
  v=>{v.declared_source_ids.statement=H('f');},v=>{v.root_function_sha256=H('f');},
  v=>{v.declared_instructions[1].descriptor++;},v=>{v.declared_instructions[1].source_association='exact_microstep';},
  v=>{v.declared_register_roles.inputs[0]++;},v=>{v.expansion_depth=257;},
  v=>{v.call_site.file_identity='bad';},v=>{v.call_site.byte_end=0;},
  v=>{v.authenticates_source=true;},v=>{v.authenticates_compiler_execution=true;},
  v=>{v.grants_proof_resume_artifact_launch_authority=true;},v=>{v.physical_register_lifetimes='available';},
  v=>{v.fine_step_origins='available';},v=>{v.macro_expansion_frames=[];},
  v=>{v.limits.source_reobservation_work_used=1048577;},v=>{v.extra='unknown';},
  v=>{v.target='gfx950';},v=>{v.wave_width=32;},v=>{v.semantic_sha256=H('0');},
];
for(const [index,mutate]of reportMutations.entries())test('exact report refuses identity/authority mutation '+index,()=>{
  const f=fixture();mutate(f.value);assert.throws(()=>validate(f));
});
test('report codec refuses malformed UTF8, BOM, duplicate keys, trailing data and exact overbound',()=>{
  const f=fixture(),raw=bytes(f.value),run=input=>validateRepeatOriginV1(input,f.exported,f.inspection,f.raw);
  assert.deepEqual(run(Buffer.concat([raw,Buffer.alloc(ORIGIN_BYTES_V1-raw.length,0x20)])),f.value);
  for(const input of [Buffer.alloc(0),Buffer.alloc(ORIGIN_BYTES_V1+1,0x20),
    Buffer.concat([Buffer.from([0xef,0xbb,0xbf]),raw]),Buffer.concat([raw,Buffer.from([0xff])]),
    Buffer.concat([raw,Buffer.from('{}')]),Buffer.from('{"schema":"bad",'+raw.toString().slice(1))])
    assert.throws(()=>run(input));
  assert.throws(()=>validateRepeatOriginV1(raw,f.exported,f.inspection,Buffer.alloc(11)));
  assert.throws(()=>validateRepeatOriginV1(raw.toString(),f.exported,f.inspection,f.raw));
});
test('fresh same-source repeat may share every report identity; changed-count origins must not join',()=>{
  const first=fixture(),again=fixture();again.value.limits.source_reobservation_work_used++;
  assert.equal(validate(first).canonical_sha256,validate(again).canonical_sha256);
  const changed=fixture();changed.inspection.declared_program.count=3;
  changed.inspection.declared_program.descriptors[2]=201;
  assert.throws(()=>validateRepeatOriginV1(bytes(first.value),changed.exported,changed.inspection,changed.raw));
});

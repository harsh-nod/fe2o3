import test from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import {fileURLToPath} from 'node:url';
import {inspect,request,publication,admission,packageManifest,cargoArgs,cargoEnv,
  STAGES,FLAGS,LIMITS,CRATE,FIXTURE_SHA,sha} from './bf16-source-workflow/protocol.mjs';
import {cleanTerminal} from './bf16-source-workflow/io.mjs';
import {options} from './bf16-source-workflow.mjs';

// Synthetic transport values only. No compiler admission is fabricated.
const source=fs.readFileSync(new URL('../crates/rustc-codegen-fe2o3/tests/fixtures/tiled-region-inspection-v1/src/lib.rs',import.meta.url));
const digest='a'.repeat(64),other='b'.repeat(64);
const selection={semantic_sha256:digest,canonical_sha256:other,mir_sha256:'c'.repeat(64),original_sha256:FIXTURE_SHA};
function action() {return {schema:'fe2o3-bf16-tile-source-action-v1',mode:'inspect',status:'inspected',
  actual_rustc_callback:true,target:'gfx942:xnack-',wave_width:64,
  canonical_digest_domain:'compiler_identity_digest_not_sha256_serialized_bytes',source_postflight_ok:true,
  selection:{...selection,original_bytes:3950,publication_eligibility:'rechecked_by_explicit_source_action'},
  requested_selection:null,request_sha256:null,publication:null,publication_effect:'not_attempted',publication_error:null,
  selection_is_compiler_custody:false,candidate_compiled:false,simulation_performed:false,
  normal_ranked_admission_performed:false,native_execution:false,hardware_observed:false,
  grants_artifact_or_launch_authority:false,memory_measurement:'unavailable',candidate_requires_fresh_frontend:true};}
const candidate=Buffer.from('inert synthetic candidate\n'),stat={dev:9n,ino:9007199254740993n};
function published(order='identity') {
  const req=request(selection,order),r=action();
  Object.assign(r,{mode:'promote',status:'candidate_created',selection:null,
    requested_selection:{...selection},request_sha256:sha(req),publication_effect:'may_have_created_candidate'});
  r.publication={original_sha256:FIXTURE_SHA,candidate_sha256:sha(candidate),original_bytes:3950,
    candidate_bytes:candidate.length,candidate_device:String(stat.dev),candidate_inode:String(stat.ino),
    return_order:order,created_new:true,original_overwritten:false,fresh_compilation_required:true,
    fresh_compilation_observed:false,grants_compiler_or_launch_authority:false};
  return {r,req};
}
function admitted(order='identity') {return {
  schema:'fe2o3-bf16-generated-source-admission-v1',mode:'inspect_generated_source',
  status:'nominal_source_admitted_normal_ranked_refused',target:'gfx942:xnack-',wave_width:64,
  actual_rustc_callback:true,source_postflight_ok:true,source_admitted:true,
  nominal_pre_ranked_materialized:true,normal_ranked_attempted:true,normal_ranked_admitted:false,
  normal_refusal:'BF16 nominal source-ranked projection',
  admission:{source_sha256:sha(candidate),source_bytes:candidate.length,semantic_sha256:digest,
    root_mir_sha256:digest,helper_mir_sha256:other,helper_source_signature_sha256:digest,
    helper_fn_abi_sha256:digest,canonical_identity:other,root:'0',helper:'1',call_block:'2',
    return_permutation:order==='identity'?[0,1,2,3]:[1,0,2,3],copied_facts_are_source_authority:false},
  canonical_digest_domain:'compiler_identity_digest_not_sha256_serialized_bytes',
  source_writes_performed:false,simulation_performed:false,artifact_emitted:false,native_execution:false,
  hardware_observed:false,grants_artifact_or_launch_authority:false,memory_measurement:'unavailable'};}
const fact={sha256:sha(candidate),bytes:candidate.length,device:String(stat.dev),inode:String(stat.ino)};
const terminal={reason:null,exit_code:0,exit_signal:null,close_code:0,close_signal:null,
  stdout_end:true,stderr_end:true,drain_abandoned:false};

test('closed five Cargo actions and fresh candidate packages',()=>{
  assert.equal(STAGES.length,5);assert.deepEqual(STAGES.map(s=>s.mode),['inspect','publish','publish','admit','admit']);
  assert.deepEqual(STAGES.slice(3).map(s=>s.package),['original/identity','original/swap01']);
});
test('exact original fixture selection',()=>{assert.deepEqual(inspect(action(),source),selection);});
test('changed source, missing selector and extra field refuse',()=>{
  assert.throws(()=>inspect(action(),Buffer.from(source.toString().replace('Separate','Changed'))));
  const r=action();delete r.selection.mir_sha256;assert.throws(()=>inspect(r,source));
  const extra=action();extra.unexpected=true;assert.throws(()=>inspect(extra,source));
});
test('inspection refuses every production/authority claim',()=>{
  for(const k of ['candidate_compiled','simulation_performed','normal_ranked_admission_performed',
    'native_execution','hardware_observed','grants_artifact_or_launch_authority','selection_is_compiler_custody']) {
    const r=action();r[k]=true;assert.throws(()=>inspect(r,source),Error,k);
  }
});
test('requests are closed selectors with cwd-relative new paths',()=>{
  for(const order of ['identity','swap01']){
    const r=JSON.parse(request(selection,order));
    assert.equal(Object.keys(r).length,9);assert.equal(r.original_path,'src/lib.rs');
    assert.equal(r.candidate_path,order+'/src/lib.rs');assert.equal(r.helper_name,'__fe2o3_bf16_tile_'+order);
    assert.equal(r.return_order,order);assert.ok(request(selection,order).length<=8192);
  }
  assert.throws(()=>request(selection,'swap23'));
  assert.throws(()=>request({...selection,mir_sha256:'0'.repeat(64)},'identity'));
});
test('publication binds full bytes, lossless inode, original selectors and exact request',()=>{
  const {r,req}=published();assert.deepEqual(publication(r,selection,req,'identity',candidate,stat),fact);
});
test('publication refuses altered request, source and inode',()=>{
  const {r,req}=published();
  assert.throws(()=>publication(r,selection,Buffer.concat([req,Buffer.from(' ')]),'identity',candidate,stat));
  assert.throws(()=>publication(r,selection,req,'identity',Buffer.from('different'),stat));
  assert.throws(()=>publication(r,selection,req,'identity',candidate,{...stat,ino:stat.ino+1n}));
});
test('publication refuses stale selectors, wrong order or overwrite',()=>{
  for(const field of ['semantic_sha256','canonical_sha256','mir_sha256','original_sha256']){
    const {r,req}=published();r.requested_selection[field]='f'.repeat(64);
    assert.throws(()=>publication(r,selection,req,'identity',candidate,stat));
  }
  const {r,req}=published();r.publication.original_overwritten=true;
  assert.throws(()=>publication(r,selection,req,'identity',candidate,stat));
  const x=published('swap01');assert.throws(()=>publication(x.r,selection,x.req,'identity',candidate,stat));
});
test('fresh admission supports both exact requested return orders',()=>{
  for(const order of ['identity','swap01'])assert.equal(admission(admitted(order),order,candidate,fact,stat).normal_ranked_admitted,false);
});
test('fresh admission refuses opposite permutation and changed candidate identity',()=>{
  assert.throws(()=>admission(admitted('identity'),'swap01',candidate,fact,stat));
  assert.throws(()=>admission(admitted(),'identity',candidate,{...fact,sha256:other},stat));
  assert.throws(()=>admission(admitted(),'identity',candidate,fact,{...stat,ino:1n}));
});
test('fresh admission refuses fake normal success, broad claims and wrong typed-refusal label',()=>{
  for(const key of ['normal_ranked_admitted','source_writes_performed','simulation_performed','artifact_emitted',
    'native_execution','hardware_observed','grants_artifact_or_launch_authority']){
    const r=admitted();r[key]=true;assert.throws(()=>admission(r,'identity',candidate,fact,stat));
  }
  const r=admitted();r.normal_refusal='same words on a different boundary';
  assert.throws(()=>admission(r,'identity',candidate,fact,stat));
});
test('copied admission IDs/digests remain closed bounded and nonauthority',()=>{
  for(const value of ['01','-1','4294967296',1]){
    const r=admitted();r.admission.root=value;assert.throws(()=>admission(r,'identity',candidate,fact,stat));
  }
  const r=admitted();r.admission.copied_facts_are_source_authority=true;
  assert.throws(()=>admission(r,'identity',candidate,fact,stat));
  const missing=admitted();delete missing.admission.canonical_identity;
  assert.throws(()=>admission(missing,'identity',candidate,fact,stat));
});
test('new manifests only replace the two frozen relative dependencies',()=>{
  const template='[workspace]\na = "../../../../fe2o3-device"\nb = "../../../../fe2o3-host"\n';
  assert.equal(packageManifest(template,'/repo').toString(),'[workspace]\na = "/repo/crates/fe2o3-device"\nb = "/repo/crates/fe2o3-host"\n');
  assert.throws(()=>packageManifest(template+'a = "../../../../fe2o3-device"\n','/repo'));
  assert.throws(()=>packageManifest(template,'/bad"path'));
});
test('Cargo command uses actual check and build-std, no metadata/source override',()=>{
  const args=cargoArgs('/work/original/Cargo.toml','/work/target');
  assert.equal(args[0],'check');assert.ok(args.includes('-Zbuild-std=core'));
  assert.ok(args.includes('--locked')&&args.includes('--offline'));
  assert.equal(args.filter(x=>x.includes('metadata')).length,0);
});
test('Cargo environment never injects package or crate-binding authority',()=>{
  const env=cargoEnv({PATH:'/bin',CARGO_PRIMARY_PACKAGE:'fake',CARGO_PKG_NAME:'fake',
    FE2O3_CRATE_BINDING_ID_V1:'fake',FE2O3_CARGO_METADATA_BUILD_OBSERVATION_V2:'fake',
    RUSTC_WORKSPACE_WRAPPER:'/bad',RUSTFLAGS:'bad',FE2O3_EXTRACT_GFX942_LLVM_PATH_V1:'bad'},
    {rustc:'/rustc',extractor:'/extractor'},STAGES[0],'/out');
  for(const key of ['CARGO_PRIMARY_PACKAGE','CARGO_PKG_NAME','FE2O3_CRATE_BINDING_ID_V1',
    'FE2O3_CARGO_METADATA_BUILD_OBSERVATION_V2','RUSTC_WORKSPACE_WRAPPER','RUSTFLAGS',
    'FE2O3_EXTRACT_GFX942_LLVM_PATH_V1'])assert.equal(env[key],undefined);
  assert.equal(env.RUSTC_WRAPPER,'/extractor');assert.equal(env.FE2O3_EXTRACT_CRATE_V1,CRATE);
  assert.equal(env.CARGO_ENCODED_RUSTFLAGS,FLAGS.join('\x1f'));
});
test('only publication gets mutation opt-in; fresh mode is exclusive',()=>{
  const tools={rustc:'/r',extractor:'/e'};
  const pub=cargoEnv({},tools,STAGES[1],'/out','/request');
  assert.equal(pub.FE2O3_EXTRACT_BF16_TILE_PROMOTION_REQUEST_V1,'/request');
  const fresh=cargoEnv({},tools,STAGES[3],'/fresh');
  assert.equal(fresh.FE2O3_EXTRACT_BF16_TILE_SOURCE_DIRECTORY_V1,undefined);
  assert.equal(fresh.FE2O3_EXTRACT_BF16_TILE_PROMOTION_REQUEST_V1,undefined);
  assert.equal(fresh.FE2O3_EXTRACT_BF16_GENERATED_SOURCE_DIRECTORY_V1,'/fresh');
});
test('success requires clean exit close drain and no resource refusal',()=>{
  assert.ok(cleanTerminal(terminal));
  for(const [key,value]of [['reason','cap'],['exit_code',1],['close_code',1],['exit_signal','SIGKILL'],
    ['close_signal','SIGTERM'],['stdout_end',false],['stderr_end',false],['drain_abandoned',true]]){
    assert.equal(cleanTerminal({...terminal,[key]:value}),false,key);
  }
});
test('CLI options are exact nonduplicated bounded absolute fields',()=>{
  const args=['--repo','/r','--extractor','/e','--cargo','/c','--rustc','/u','--work','/w','--deadline','2026-10-01T20:00:00.000Z'];
  assert.equal(options(args).work,'/w');assert.throws(()=>options([...args,'--extra','x']));
  const repeated=[...args];repeated[2]='--repo';assert.throws(()=>options(repeated));
  const relative=[...args];relative[9]='relative';assert.throws(()=>options(relative));
});
test('failure cleanup retains referenced kill timer and avoids forced process exit',()=>{
  const main=fs.readFileSync(new URL('./bf16-source-workflow.mjs',import.meta.url),'utf8');
  const io=fs.readFileSync(new URL('./bf16-source-workflow/io.mjs',import.meta.url),'utf8');
  assert.ok(main.includes('process.exitCode=1'));
  assert.ok(!main.includes('process.exit('));
  assert.ok(!io.includes('clearTimeout(killTimer)'));
  assert.ok(io.includes("killTimer=setTimeout(()=>signal('SIGKILL'),1000)"));
  assert.ok(io.includes("name+'.terminal.json'"));
});
test('legacy caps stay explicit and failure/stream reservations are separate',()=>{
  assert.equal(LIMITS.stream,8*1024*1024);assert.equal(LIMITS.target,500*1024*1024);
  assert.equal(LIMITS.evidence,128*1024*1024);assert.equal(LIMITS.stageMs,300000);
  assert.equal(LIMITS.totalMs,1800000);assert.equal(LIMITS.report,16384);
});

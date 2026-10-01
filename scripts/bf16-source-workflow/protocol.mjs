// A closed tutorial workflow, not a report-authentication or compiler authority API.
import {createHash} from 'node:crypto';
export const MiB = 1024 * 1024;
export const LIMITS = Object.freeze({source:65536, report:16384, request:8192,
  stream:8*MiB, evidence:128*MiB, target:500*MiB, entries:100000, depth:32,
  stageMs:300000, totalMs:1800000, readBytes:64*1024*MiB});
export const CRATE = 'fe2o3_tiled_region_inspection_v1_fixture';
export const FIXTURE = 'crates/rustc-codegen-fe2o3/tests/fixtures/tiled-region-inspection-v1';
export const FIXTURE_SHA = 'fcb26135ad4f931bb8dda63d631639a34dd8461e7801c22a1f6a383e0e735a3e';
export const FLAGS = Object.freeze(['-Ctarget-cpu=gfx942',
  '-Ctarget-feature=-xnack,+wavefrontsize64,-wavefrontsize32', '-Cpanic=abort',
  '-Copt-level=3', '-Cembed-bitcode=no', '-Cdebug-assertions=off',
  '-Coverflow-checks=on', '-Zalways-encode-mir', '-Zunstable-options']);
export const STAGES = Object.freeze([
  {name:'inspect',package:'original',mode:'inspect'},
  {name:'publish-identity',package:'original',mode:'publish',order:'identity'},
  {name:'publish-swap01',package:'original',mode:'publish',order:'swap01'},
  {name:'admit-identity',package:'original/identity',mode:'admit',order:'identity'},
  {name:'admit-swap01',package:'original/swap01',mode:'admit',order:'swap01'},
]);
export function check(ok, message) { if (!ok) throw new Error(message); }
export function sha(bytes) { return createHash('sha256').update(bytes).digest('hex'); }
export function digest(s) {
  check(typeof s==='string' && /^[0-9a-f]{64}$/.test(s) && s!=='0'.repeat(64), 'digest');
  return s;
}
function exact(o, fields) {
  check(o && typeof o==='object' && !Array.isArray(o), 'object');
  check(JSON.stringify(Object.keys(o).sort())===JSON.stringify(fields.split(' ').sort()), 'closed fields');
}
function decimal(s, max=0xffffffffffffffffn) {
  check(typeof s==='string' && /^(0|[1-9][0-9]*)$/.test(s) && s.length<=20 && BigInt(s)<=max, 'decimal');
}
function noClaims(r, fields) { for(const key of fields.split(' ')) check(r[key]===false, key); }
const DOMAIN = 'compiler_identity_digest_not_sha256_serialized_bytes';
const actionKeys = 'schema mode status actual_rustc_callback target wave_width canonical_digest_domain source_postflight_ok selection requested_selection request_sha256 publication publication_effect publication_error selection_is_compiler_custody candidate_compiled simulation_performed normal_ranked_admission_performed native_execution hardware_observed grants_artifact_or_launch_authority memory_measurement candidate_requires_fresh_frontend';
function action(r) {
  exact(r, actionKeys);
  check(r.schema==='fe2o3-bf16-tile-source-action-v1' && r.actual_rustc_callback===true &&
    r.target==='gfx942:xnack-' && r.wave_width===64 && r.canonical_digest_domain===DOMAIN &&
    r.source_postflight_ok===true && r.memory_measurement==='unavailable' &&
    r.candidate_requires_fresh_frontend===true, 'action profile');
  noClaims(r, 'selection_is_compiler_custody candidate_compiled simulation_performed normal_ranked_admission_performed native_execution hardware_observed grants_artifact_or_launch_authority');
}
const selectors = ['semantic_sha256','canonical_sha256','mir_sha256','original_sha256'];
export function inspect(r, original) {
  action(r);
  check(r.mode==='inspect' && r.status==='inspected' && r.publication_effect==='not_attempted' &&
    r.requested_selection===null && r.request_sha256===null && r.publication===null &&
    r.publication_error===null, 'inspection completion');
  exact(r.selection, selectors.join(' ')+' original_bytes publication_eligibility');
  selectors.forEach(k=>digest(r.selection[k]));
  check(original.length===3950 && sha(original)===FIXTURE_SHA &&
    r.selection.original_sha256===sha(original) && r.selection.original_bytes===original.length &&
    r.selection.publication_eligibility==='rechecked_by_explicit_source_action', 'original selection');
  return Object.fromEntries(selectors.map(k=>[k,r.selection[k]]));
}
export function request(selection, order) {
  check(order==='identity'||order==='swap01','order');
  exact(selection,selectors.join(' ')); selectors.forEach(k=>digest(selection[k]));
  const r={schema:'fe2o3-bf16-tile-source-promotion-request-v1',...selection,
    original_path:'src/lib.rs',candidate_path:order+'/src/lib.rs',
    helper_name:'__fe2o3_bf16_tile_'+order,return_order:order};
  const bytes=Buffer.from(JSON.stringify(r)+'\n');
  check(bytes.length<=LIMITS.request,'request cap'); return bytes;
}
export function publication(r, selection, req, order, candidate, stat) {
  action(r);
  check(r.mode==='promote' && r.status==='candidate_created' && r.selection===null &&
    r.publication_effect==='may_have_created_candidate' && r.publication_error===null &&
    r.request_sha256===sha(req), 'publication completion');
  exact(r.requested_selection,selectors.join(' '));
  for(const key of selectors) check(r.requested_selection[key]===selection[key], 'requested selection');
  const p=r.publication;
  exact(p,'original_sha256 candidate_sha256 original_bytes candidate_bytes candidate_device candidate_inode return_order created_new original_overwritten fresh_compilation_required fresh_compilation_observed grants_compiler_or_launch_authority');
  decimal(p.candidate_device); decimal(p.candidate_inode);
  check(p.original_sha256===selection.original_sha256 && p.original_bytes===3950 &&
    candidate.length>0 && candidate.length<=LIMITS.source && p.candidate_bytes===candidate.length &&
    p.candidate_sha256===sha(candidate) && p.return_order===order && p.created_new===true &&
    p.original_overwritten===false && p.fresh_compilation_required===true &&
    p.fresh_compilation_observed===false && p.grants_compiler_or_launch_authority===false &&
    p.candidate_device===String(stat.dev) && p.candidate_inode===String(stat.ino), 'published file join');
  return {sha256:sha(candidate),bytes:candidate.length,device:String(stat.dev),inode:String(stat.ino)};
}
export function admission(r, order, candidate, publicationFact, stat) {
  exact(r,'schema mode status target wave_width actual_rustc_callback source_postflight_ok source_admitted nominal_pre_ranked_materialized normal_ranked_attempted normal_ranked_admitted normal_refusal admission canonical_digest_domain source_writes_performed simulation_performed artifact_emitted native_execution hardware_observed grants_artifact_or_launch_authority memory_measurement');
  check(r.schema==='fe2o3-bf16-generated-source-admission-v1' && r.mode==='inspect_generated_source' &&
    r.status==='nominal_source_admitted_normal_ranked_refused' && r.target==='gfx942:xnack-' &&
    r.wave_width===64 && r.canonical_digest_domain===DOMAIN && r.memory_measurement==='unavailable' &&
    r.actual_rustc_callback===true && r.source_postflight_ok===true && r.source_admitted===true &&
    r.nominal_pre_ranked_materialized===true && r.normal_ranked_attempted===true &&
    r.normal_refusal==='BF16 nominal source-ranked projection', 'admission completion');
  noClaims(r,'normal_ranked_admitted source_writes_performed simulation_performed artifact_emitted native_execution hardware_observed grants_artifact_or_launch_authority');
  const a=r.admission;
  exact(a,'source_sha256 source_bytes semantic_sha256 root_mir_sha256 helper_mir_sha256 helper_source_signature_sha256 helper_fn_abi_sha256 canonical_identity root helper call_block return_permutation copied_facts_are_source_authority');
  for(const k of ['source_sha256','semantic_sha256','root_mir_sha256','helper_mir_sha256',
    'helper_source_signature_sha256','helper_fn_abi_sha256','canonical_identity']) digest(a[k]);
  for(const k of ['root','helper','call_block']) decimal(a[k],0xffffffffn);
  check(order==='identity'||order==='swap01','order');
  check(a.root!==a.helper && a.copied_facts_are_source_authority===false &&
    JSON.stringify(a.return_permutation)===JSON.stringify(order==='identity'?[0,1,2,3]:[1,0,2,3]) &&
    a.source_sha256===sha(candidate) && a.source_sha256===publicationFact.sha256 &&
    a.source_bytes===candidate.length && a.source_bytes===publicationFact.bytes &&
    String(stat.dev)===publicationFact.device && String(stat.ino)===publicationFact.inode, 'fresh source join');
  return {source_sha256:a.source_sha256,canonical_identity:a.canonical_identity,
    return_permutation:a.return_permutation,normal_ranked_admitted:false};
}
export function packageManifest(template, repo) {
  check(typeof repo==='string' && repo.startsWith('/') && !/[\x00-\x1f"\\]/.test(repo), 'repository path');
  let result=template;
  for(const name of ['fe2o3-device','fe2o3-host']) {
    const old='"../../../../'+name+'"';
    check(result.split(old).length===2,'fixture dependency');
    result=result.replace(old,JSON.stringify(repo+'/crates/'+name));
  }
  check(result.includes('[workspace]') && Buffer.byteLength(result)<=8192,'manifest');
  return Buffer.from(result);
}
export function cargoArgs(manifest, target) {
  return ['check','--manifest-path',manifest,'--lib','--release','--target','amdgcn-amd-amdhsa',
    '--target-dir',target,'--jobs','1','--locked','--offline','-Zbuild-std=core'];
}
export function cargoEnv(inherited, tools, stage, output, requestPath) {
  const env={};
  // Never forward package identity, crate binding or pre-existing output modes.
  for(const key of ['PATH','HOME','CARGO_HOME','RUSTUP_HOME','LD_LIBRARY_PATH','TMPDIR']) {
    if(inherited[key]!==undefined) env[key]=inherited[key];
  }
  Object.assign(env,{LC_ALL:'C',CARGO_INCREMENTAL:'0',RUSTC:tools.rustc,
    RUSTC_WRAPPER:tools.extractor,CARGO_ENCODED_RUSTFLAGS:FLAGS.join('\x1f'),
    FE2O3_EXTRACT_CRATE_V1:CRATE});
  if(stage.mode==='admit') env.FE2O3_EXTRACT_BF16_GENERATED_SOURCE_DIRECTORY_V1=output;
  else {
    env.FE2O3_EXTRACT_BF16_TILE_SOURCE_DIRECTORY_V1=output;
    if(stage.mode==='publish') env.FE2O3_EXTRACT_BF16_TILE_PROMOTION_REQUEST_V1=requestPath;
  }
  return env;
}

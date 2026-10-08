// Synthetic parser controls only. Never execution evidence.
import test from 'node:test';
import assert from 'node:assert/strict';
import crypto from 'node:crypto';
import {canonical,parseOrdinary,parsePair} from './recipe-outcome-series-v1.mjs';
const O='FE2O3_RECIPE_OUTCOME_V1 ', R='FE2O3_RECIPE_OUTCOME_SERIES_V1 ';
const digest=s=>crypto.createHash('sha256').update(s).digest('hex');
const copy=x=>JSON.parse(JSON.stringify(x));
function fixture(workload='exact_revision_refusal') {
  const binding={workload,source:'src/renamed.rs',current_source_sha256:'22'.repeat(32),
    origin_source_sha256:'11'.repeat(32),origin_normal_sha256:'33'.repeat(32),
    origin_source_initializer:[1,2,3,4],recipe_sha256:'44'.repeat(32),instance_axes:Array.from({length:5},()=>Array(32).fill(5))};
  const e={source_sha256:Array(32).fill(34),source_initializer:[5,6,7,8],semantic_sha256:Array(32).fill(6),
    instance_axes:binding.instance_axes,original:{},input:{},output:{},requested_order:'reverse_ready',
    requested_relation:'or_before_xor',strength:'exact',source_binding_mode:'rebind_current',actual_relation:'or_before_xor',
    constraint_outcome:{status:'honored',relation:'or_before_xor'},region:[0,0,0,0],output_result_order:[1,0,2],
    prefix_execution_bytes:[],transition_sha256:[],transition_bytes:0,transition_rows:[],fresh_formal_counts:[],
    llvm_sha256:[],descriptor_sha256:[],recipe_sha256:Array(32).fill(68),canonical_work:0,canonical_peak_storage:0,
    created:false,descriptor_producer:'source-local-order-policy6-v1/gfx942',composition:'source-local-order-policy6-v1',grants_authority:false};
  const outcome={schema:'fe2o3-recipe-outcome-v1',binding,callback_count:1,compiler_callback_count:1,
    status:workload==='checked_rebind'?'accepted':'refused',
    normal_utf8:workload==='checked_rebind'?canonical({llvm:'inert parser control',created_recipe:null,evidence:e,grants_artifact_or_launch_authority:false})+'\n':null,
    failure:workload==='checked_rebind'?null:{phase:'RecipeBinding',diagnostic:'local-order recipe source revision changed',compiler_fatal:false},grants_authority:false};
  const bytes=canonical(outcome)+'\n', hash=digest(bytes), count=Buffer.byteLength(bytes);
  const ordinary={kind:'ordinary_complete',schema:outcome.schema,binding,outcome_bytes:count,outcome_sha256:hash,
    elapsed_ns:null,statistics:null,retained_logical_bytes:null,grants_authority:false};
  const plan={kind:'plan',schema:outcome.schema,binding,calls:35,calibration:5,measured:30,
    upfront_selected_input_reservations:{calls:35,maximum_selected_retained_source_read_bytes:110100585,maximum_selected_recipe_read_bytes:860265,maximum_additional_capture_path_read_bytes:2293795},
    additional_origin_read_bytes:1572867,additional_ordinary_oracle_read_bytes:3170307,callback_stopping_seconds:60,external_deadline_required:true,
    ordinary_oracle_sha256:hash,ordinary_oracle_bytes:count,retained_logical_bytes:null,storage_walks_in_timer:false,grants_authority:false};
  const samples=Array.from({length:35},(_,i)=>({kind:'sample',sample:{ordinal:i+1,calibration:i<5,elapsed_ns:i<5?999999: i-4,
    outcome_bytes:count,outcome_sha256:hash,exact_ordinary_oracle_equal:true}}));
  const complete={kind:'series_complete',schema:outcome.schema,binding,callback_count:1,raw_samples:35,calibration:5,measured:30,
    statistics:{p50_ns:15,p95_ns:29,max_ns:30},percentile_method:'nearest_rank_30_rank15_rank29_rank30',
    ordinary_oracle_sha256:hash,ordinary_oracle_bytes:count,all35_exact_ordinary_oracle_equal:true,frontend_reused:true,admitted_owner_reused:false,
    scope:'fresh_transaction_creation_through_original_consuming_recipe_return',companion_serialization_hash_equality_in_timer:false,
    api_internal_checks_in_timer:true,retained_logical_bytes:null,peak_heap_measured:false,rss_measured:false,budget_accepted:false,grants_authority:false};
  return {outcome,ordinary,rows:[plan,...samples,complete]};
}
function streams(f) {
  const stdout='test child ... \n'+O+canonical(f.outcome)+'\ntest result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s\n';
  return [stdout,R+canonical(f.ordinary)+'\n',stdout,f.rows.map(r=>R+canonical(r)+'\n').join('')];
}
function parse(f) { return parsePair(...streams(f)); }
for(const workload of ['checked_rebind','exact_revision_refusal']) test('complete '+workload+' is consistency only',()=>{
  const value=parse(fixture(workload)); assert.equal(value.raw_samples,35); assert.equal(value.statistics.p95_ns,29);
  assert.equal(value.execution_authenticated,false); assert.equal(value.budget_accepted,false);
  assert.equal(value.retained_logical_bytes,null); assert.equal(value.grants_authority,false);
});
test('ordinary refusal is retained as failure not accepted output',()=>{
  const f=fixture(); const [out,err]=streams(f);
  assert.equal(parseOrdinary(out,err).outcome.value.status,'refused');
});
const mutations={
  'drop sample':f=>f.rows.splice(12,1),
  'duplicate sample':f=>f.rows.splice(12,0,copy(f.rows[12])),
  'reorder sample':f=>{[f.rows[7],f.rows[8]]=[f.rows[8],f.rows[7]];},
  'calibration leaked':f=>f.rows[6].sample.calibration=true,
  'wrong ordinal':f=>f.rows[6].sample.ordinal=5,
  'wrong oracle hash':f=>f.rows[6].sample.outcome_sha256='00'.repeat(32),
  'wrong oracle bytes':f=>f.rows[6].sample.outcome_bytes++,
  'oracle not equal':f=>f.rows[6].sample.exact_ordinary_oracle_equal=false,
  'incomplete report':f=>f.rows[36]={kind:'incomplete'},
  'statistics fabricated':f=>f.rows[36].statistics.p95_ns=28,
  'origin/current not changed':f=>f.outcome.binding.current_source_sha256=f.outcome.binding.origin_source_sha256,
  'wrong refusal phase':f=>f.outcome.failure.phase='Frontend',
  'wrong refusal diagnostic':f=>f.outcome.failure.diagnostic='different refusal',
  'compiler fatal':f=>f.outcome.failure.compiler_fatal=true,
  'wrong callback count':f=>f.outcome.compiler_callback_count=2,
  'unexpected successful refusal':f=>f.outcome.status='accepted',
  'complete overclaims authority':f=>f.rows[36].grants_authority=true,
  'complete overclaims budget':f=>f.rows[36].budget_accepted=true,
  'retained memory invented':f=>f.rows[36].retained_logical_bytes=0,
  'expanded call cap':f=>f.rows[0].calls=36,
  'old schema':f=>f.outcome.schema='fe2o3-recipe-series-input-v1',
  'extra field':f=>f.rows[0].resume_owner={},
};
for(const [name,mutate] of Object.entries(mutations)) test('reject '+name,()=>{
  const f=fixture(); mutate(f); assert.throws(()=>parse(f));
});
test('ordinary and series cannot select different full byte oracles',()=>{
  const a=streams(fixture()),b=streams(fixture('checked_rebind'));
  assert.throws(()=>parsePair(a[0],a[1],b[2],b[3]));
});
test('duplicate JSON fields rejected even if same values',()=>{
  const args=streams(fixture());
  args[3]=args[3].replace('"calls":35','"calls":35,"calls":35');
  assert.throws(()=>parsePair(...args));
});
test('old positive-series records cannot qualify a new workload',()=>{
  const args=streams(fixture()); args[3]+='FE2O3_RECIPE_SERIES_V1 {}\n';
  assert.throws(()=>parsePair(...args));
});
test('partial stream and missing successful test terminal refuse',()=>{
  const args=streams(fixture()); args[2]=args[2].replace('test result: ok.','test result: FAILED.');
  assert.throws(()=>parsePair(...args));
});
test('fractional negative unsafe elapsed values refuse',()=>{
  for(const value of [-1,1.5,Number.MAX_SAFE_INTEGER+1]) {
    const f=fixture(); f.rows[7].sample.elapsed_ns=value; assert.throws(()=>parse(f));
  }
});
test('rebind cannot reuse original initializer or recipe/source/instance',()=>{
  for(const key of ['source_initializer','source_sha256','recipe_sha256','instance_axes']) {
    const f=fixture('checked_rebind'), normal=JSON.parse(f.outcome.normal_utf8);
    normal.evidence[key]=key==='source_initializer'?f.outcome.binding.origin_source_initializer:
      key==='instance_axes'?Array.from({length:5},()=>Array(32).fill(0)):Array(32).fill(0);
    f.outcome.normal_utf8=canonical(normal)+'\n'; assert.throws(()=>parse(f));
  }
});

test('conflicting or repeated terminals cannot be hidden after a success',()=>{
  for (const extra of ['test result: FAILED. 0 passed; 1 failed;\n','test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s\n']) {
    const args=streams(fixture()); args[2]+=extra; assert.throws(()=>parsePair(...args));
  }
});
test('truncated non-LF streams never acquire a synthesized final delimiter',()=>{
  for (let i=0;i<4;i++) {
    const args=streams(fixture()); args[i]=args[i].slice(0,-1); assert.throws(()=>parsePair(...args));
  }
});

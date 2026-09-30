// Fixed synchronous orchestration. Injected functions are trusted harness APIs, never request fields.
import {boundedError} from '../loaded-cpu-adapter/adapter-guard.mjs';
import {encodeBoundedEvidence} from '../loaded-cpu-adapter/adapter-writer.mjs';
import {admitLaunchBootstrap,admitStaticLaunchRequest,EXTERNAL_OBLIGATIONS,sha} from './launch-model.mjs';
import {readPinnedRequest} from './launch-reader.mjs';
const check=(v,m)=>{if(!v)throw Error('static Node launch dispatch: '+m);};
/** Defer a separately scoped guard's elapsed origin until that stage actually starts. */
export function deferLaunchGuard(factory){
 check(typeof factory==='function','explicit deferred guard factory');
 let started=false,guard=null,failed=false,firstFailure;
 const deferred=()=>{
  if(failed)throw firstFailure;
  if(!started){
   started=true;
   try{guard=factory();check(typeof guard==='function'&&typeof guard.snapshot==='function'&&typeof guard.resourceSnapshot==='function','complete deferred guard');}
   catch(e){failed=true;firstFailure=e;throw e;}
  }
  return guard();
 };
 deferred.snapshot=()=>guard&&!failed?guard.snapshot():{started,construction_failed:failed,first_failure:failed?boundedError(firstFailure):null};
 deferred.resourceSnapshot=()=>guard&&!failed?guard.resourceSnapshot():null;
 return deferred;
}
export function executeStaticLaunch(bootstrapBytes,{requestProvider,bootstrapGuard,reportGuard,context,invokeAdapter,summarySink}){
 // Bootstrap admission precedes even request-provider or summary-sink access.
 const boot=admitLaunchBootstrap(bootstrapBytes);
 check(typeof bootstrapGuard==='function'&&typeof reportGuard==='function'&&typeof invokeAdapter==='function'&&['before','write','after'].every(k=>typeof summarySink?.[k]==='function'),'explicit fixed harness APIs');
 const observation={schema:'fe2o3-static-node-command-observation-v1',label:boot.binding.label,status:'failed',
  first_failure:null,request_read:null,request_admission:'not-started',adapter_dispatch:'not-started',adapter_summary:null,accounting:null,
  output_roles:boot.binding.outputs,external_terminal_pins:boot.binding.external_terminal_pins,
  obligations:EXTERNAL_OBLIGATIONS.map(role=>({role,closed_by_this_component:false})),
  authority:false,qualified:false,native_authority:false,child_processes_started:0,
  summary_publication:'intent only; consult process exit, outer capture and complete root readback',
  actual_loader_io_metered:false,outer_supervisor_io_metered:false,
  in_memory_failure_requires_process_survival:true,external_kill_can_prevent_reporting:true};
 const transfer={schema:'fe2o3-static-node-summary-transfer-v1',status:'not-started',cap_bytes:boot.binding.outputs[3].cap_bytes,
  submitted_bytes:0,submitted_sha256:null,attempted_bytes:0,returned_bytes:0,call_cap:0,calls:0,provider_invocations:0,metadata_cap:2,metadata_calls:0,metadata_invocations:0,endpoint_before:null,endpoint_after:null,first_error:null,
  complete_write_observed:false,durable_publication_proved:false,partial_capture_may_remain:false};
 let failed=false,adapterResult=null;
 const remember=(stage,e)=>{const v={stage,error:boundedError(e)};if(!failed){failed=true;observation.first_failure=v;}return v;};
 try{
  const read=readPinnedRequest(boot.request_spec_bytes,requestProvider,{guard:bootstrapGuard});observation.request_read=read.observation;
  if(read.bytes===null){failed=true;observation.first_failure={stage:'request-read',detail:read.observation.first_failure};}
  else{
   observation.request_admission='started';
   const admitted=admitStaticLaunchRequest(read.bytes,bootstrapBytes,context);
   observation.accounting=admitted.accounting;observation.request_admission='completed';
   // No request-controlled command, import, environment or callback is dispatched.
   bootstrapGuard();observation.adapter_dispatch='started';
   adapterResult=invokeAdapter(admitted.plan_bytes,admitted.adapter_policy_bytes,admitted.evidence_spec_bytes,{resource_root:admitted.resource_root});
   check(adapterResult&&typeof adapterResult==='object'&&adapterResult.summary?.schema==='fe2o3-cpu-adapter-command-result-v1','fixed adapter response');
   observation.adapter_summary=adapterResult.summary;observation.adapter_dispatch='returned';
   if(adapterResult.status!=='read-and-publication-observed'){
    failed=true;observation.first_failure={stage:'adapter',detail:adapterResult.first_failure??{missing_failure_detail:true}};
   }
   observation.status=failed?'failed':'adapter-returned';
  }
 }catch(e){remember(observation.adapter_dispatch==='started'?'adapter-dispatch':observation.request_admission==='started'?'request-admission':'bootstrap-or-request',e);if(observation.request_admission==='started')observation.request_admission='failed';}
 try{
  // Capture after the final pre-dispatch check, including a retained refusal or last resource probe.
  observation.bootstrap_guard_observation=typeof bootstrapGuard.snapshot==='function'?bootstrapGuard.snapshot():null;
  observation.bootstrap_resource_observation=typeof bootstrapGuard.resourceSnapshot==='function'?bootstrapGuard.resourceSnapshot():null;
 }catch(e){remember('bootstrap-final-observation',e);}
 try{
  // A separately bound finite report policy may allow failure diagnostics after an inner elapsed-time refusal.
  // It cannot revive an expired UTC scope or authorize another selected-input read.
  reportGuard();
  observation.report_guard_before_emission=typeof reportGuard.snapshot==='function'?reportGuard.snapshot():null;
  observation.report_resources_before_emission=typeof reportGuard.resourceSnapshot==='function'?reportGuard.resourceSnapshot():null;
  const body=encodeBoundedEvidence(observation,transfer.cap_bytes);
  transfer.submitted_bytes=body.length;transfer.submitted_sha256=sha(body);transfer.call_cap=Math.ceil(body.length/65536);transfer.status='started';
  transfer.metadata_calls++;reportGuard();transfer.metadata_invocations++;transfer.endpoint_before=summarySink.before(boot.binding.summary_descriptor);
  for(let offset=0;offset<body.length;){
   const take=Math.min(65536,body.length-offset);
   check(transfer.calls<transfer.call_cap&&transfer.attempted_bytes<=transfer.cap_bytes-take,'summary reservation ceiling');
   transfer.calls++;transfer.attempted_bytes+=take;reportGuard();transfer.provider_invocations++;
   // A throwing provider need not report whether a prefix reached the inherited stream.
   transfer.partial_capture_may_remain=true;
   const n=summarySink.write(body,offset,take);
   check(Number.isInteger(n)&&n>=0&&n<=take,'summary write result');transfer.returned_bytes+=n;
   check(n===take,'short summary write without retry');offset+=take;
  }
  transfer.metadata_calls++;reportGuard();transfer.metadata_invocations++;transfer.endpoint_after=summarySink.after(boot.binding.summary_descriptor,transfer.returned_bytes);
  reportGuard();transfer.status='complete-write-observed';transfer.complete_write_observed=true;
 }catch(e){transfer.status='failed';transfer.first_error=boundedError(e);remember('summary-transfer',e);}
 observation.status=!failed&&transfer.complete_write_observed?'adapter-and-summary-observed':'failed';
 return {observation,summary_transfer:transfer,adapter_result:adapterResult,
  exit_code:observation.status==='adapter-and-summary-observed'?0:1,
  output_observation_is_not_root_readback:true,global_io_bound_proved:false};
}

// CPU-only injected adapter over the qualified bounded reader. No provider is constructed or called on import.
import {createHash} from 'node:crypto';
import {isDeepStrictEqual as equal} from 'node:util';
import {readUnqualifiedProtocol,deriveReadBudget,LoadedReaderRefusal} from '../loaded-input-reader/reader-protocol.mjs';
import {boundedError} from './adapter-guard.mjs';
const check=(v,m)=>{if(!v)throw Error('CPU loaded adapter: '+m);};
const freeze=v=>{if(v&&typeof v==='object'){for(const x of Object.values(v))freeze(x);Object.freeze(v);}return v;};
const keys=(v,n)=>check(v&&typeof v==='object'&&!Array.isArray(v)&&equal(Object.keys(v).sort(),n.slice().sort()),'closed plan keys');
const sha=b=>createHash('sha256').update(b).digest('hex');
const add=(a,b)=>{check(Number.isSafeInteger(a)&&a>=0&&Number.isSafeInteger(b)&&b>=0&&Number.isSafeInteger(a+b),'checked budget');return a+b;};
function admitProtocolWithoutIO(protocol){
 const marker='CPU-adapter-pure-admission-sentinel',forbidden=()=>{throw Error('pure admission unexpectedly reached a provider');};
 const provider=Object.fromEntries(['lstat','realpath','readlink','open','fstat','read','close'].map(k=>[k,forbidden]));
 try{readUnqualifiedProtocol(protocol,provider,{guard:()=>{throw Error(marker);},now:()=>0,milliseconds:1});throw Error('qualified protocol unexpectedly returned during pure admission');}
 catch(e){check(e instanceof LoadedReaderRefusal&&e.message==='loaded reader refused: '+marker&&e.accounting.metadata_calls===0&&e.accounting.content_calls===0&&e.accounting.opened===0&&e.accounting.completed_passes===0,'complete qualified protocol admission: '+String(e.message));}
}
function admit(raw){
 check(Buffer.isBuffer(raw)&&raw.length>0&&raw.length<=32*1024*1024,'complete bounded plan Buffer');
 const text=raw.toString('utf8');check(Buffer.from(text).equals(raw),'plan UTF8');const p=JSON.parse(text);
 keys(p,['schema','graph_protocol','historical_protocol','named_cap','phase_milliseconds']);
 check(p.schema==='fe2o3-cpu-loaded-adapter-plan-v1','plan schema');
 const g=p.graph_protocol,h=p.historical_protocol;
 check(g?.schema==='fe2o3-loaded-read-protocol-v1'&&h?.schema===g.schema&&g.passes===1&&h.passes===2,'one/two/one protocol phases');
 check(Array.isArray(g.entries)&&g.entries.length>0&&g.entries.length<=4096&&p.named_cap===g.entries.length,'exact externally selected named cap');
 check(Array.isArray(h.entries)&&h.entries.length>0&&h.entries.length<=g.entries.length,'historical subset');
 check(equal(g.aliases,h.aliases),'historical alias policies unchanged');
 for(const protocol of [g,h]){
  check(Buffer.byteLength(JSON.stringify(protocol))<=8*1024*1024,'qualified protocol byte bound');
  keys(protocol.custody,['qualified_historical_input_bytes','operational_roster_complete','root_cap_change_approved','execution_authority']);
  check(typeof protocol.custody.qualified_historical_input_bytes==='boolean','historical custody flag');
  for(const k of ['operational_roster_complete','root_cap_change_approved','execution_authority'])check(protocol.custody[k]===false,'no self-granted authority');
  check(equal(protocol.budget,deriveReadBudget(protocol.entries,protocol.aliases,protocol.passes)),'exact independently derived phase budget');
  // Exercise the unchanged qualified parser behind an immediately denying pure guard.
  // No operational provider is passed or reached, and no reader phase has started.
  admitProtocolWithoutIO(protocol);
 }
 const byName=new Map(g.entries.map(r=>[r.path,r]));check(byName.size===g.entries.length,'unique graph names');
 for(const r of h.entries){
  const full=byName.get(r.path);check(full,'historical path omitted');
  for(const k of ['kind','pin','resolved','identity'])check(equal(full[k],r[k]),'historical '+k+' replaced');
  check(r.ownership===null||equal(full.ownership,r.ownership),'historical ownership omitted or replaced');
  check(Number.isSafeInteger(full.cap)&&full.cap>=0&&full.cap<=r.cap,'inherited member cap');
  for(const kind of ['prior','loaded','duties','extras'])check(Array.isArray(full.labels?.[kind])&&r.labels[kind].every(role=>full.labels[kind].includes(role)),'historical label omitted');
 }
 keys(p.phase_milliseconds,['precheck','historical','postcheck']);
 for(const n of Object.values(p.phase_milliseconds))check(Number.isInteger(n)&&n>0&&n<=240000,'phase deadline');
 return freeze(p);
}
function diagnosticStat(v){
 if(!v||!['file','directory','symlink'].includes(v.kind)||!equal(Object.keys(v).sort(),['kind','identity','uid','gid'].sort())||!Array.isArray(v.identity)||v.identity.length!==6||![...v.identity,v.uid,v.gid].every(s=>typeof s==='string'&&/^(0|[1-9][0-9]{0,24})$/.test(s))||(BigInt(v.identity[3])&0o170000n)!=={file:0o100000n,directory:0o040000n,symlink:0o120000n}[v.kind])return{invalid_stat_shape:true};
 return{kind:v.kind,identity:v.identity.slice(),uid:typeof v.uid==='string'?v.uid.slice(0,25):null,gid:typeof v.gid==='string'?v.gid.slice(0,25):null};
}
function instrument(protocol,provider,guard,phase,shared,remember){
 const readable=protocol.entries.filter(r=>r.kind==='readable'),byName=new Map(protocol.entries.map(r=>[r.path,r])),fds=new Map(),namedStats=new Map(),aliasRules=new Map(protocol.aliases.map(a=>[a.pin.path,a]));
 let opens=0,lastClosed=null,lastOperation=null,lastProviderError=null;
 const counts={metadata_attempts:0,content_attempts:0,requested_content_bytes:0,returned_content_bytes:0,provider_invocations:0,open_returned:0,close_attempts:0,closed:0};
 const describe=f=>f?{entry_path:f.entry.path,pin:f.entry.pin,expected_identity:f.entry.identity,returned_content_bytes:f.returned,digest_of_returned_prefix:f.digest??f.hash.copy().digest('hex'),eof_observed:f.eof,last_stat:f.lastStat}:null;
 function fail(code,context){remember({source:'provider-observation',code,phase,...context});}
 function dispatch(method,args,cleanup=false){
  const fd=method==='fstat'||method==='read'||method==='close'?args[0]:null;
  let entry=fd!==null?fds.get(fd)?.entry:byName.get(args[0]);
  if(method==='open')entry=readable[opens%readable.length];
  const operation={method,entry_path:entry?.path??null,named_path:typeof args[0]==='string'?args[0]:entry?.resolved??null,descriptor:fd,invoked:false,observed:null};
  lastOperation=operation;
  if(method==='read'){
   const n=args[3];check(Number.isSafeInteger(n)&&n>=0&&n<=65536,'content request');
   check(counts.content_attempts<protocol.budget.total.content_calls&&shared.content_attempts<shared.content_cap,'content call ceiling');
   check(counts.requested_content_bytes<=protocol.budget.total.reserved_bytes-n&&shared.requested_content_bytes<=shared.byte_cap-n,'content byte ceiling');
   counts.content_attempts++;shared.content_attempts++;counts.requested_content_bytes+=n;shared.requested_content_bytes+=n;
  }else{
   if(!cleanup)check(counts.metadata_attempts<protocol.budget.total.metadata_calls&&shared.metadata_attempts<shared.metadata_cap,'metadata ceiling');
   counts.metadata_attempts++;shared.metadata_attempts++;
  }
  if(cleanup){check(fds.has(fd),'cleanup only owned descriptor');counts.close_attempts++;}
  else guard();
  if(method==='open')check(entry&&args[0]===entry.resolved,'ordered explicit graph opening');
  operation.invoked=true;counts.provider_invocations++;
  let value;
  try{value=provider[method](...args);}catch(e){operation.error=boundedError(e);lastProviderError=JSON.parse(JSON.stringify(operation));const expectedAbsence=method==='lstat'&&e?.code==='ENOENT'&&entry?.kind!=='readable'&&![...fds.values()].some(f=>f.entry.resolved===args[0])&&protocol.entries.some(r=>r.kind==='absence-observation'&&(r.path===args[0]||r.path.startsWith(args[0]+'/')||args[0]==='/'));if(!expectedAbsence)remember({source:'provider',code:'provider-'+method+'-failed',phase,operation:lastProviderError});throw e;}
  if(method==='open'){
   check(Number.isSafeInteger(value)&&value>=0&&!fds.has(value),'owned descriptor result');
   fds.set(value,{entry,hash:createHash('sha256'),returned:0,digest:null,eof:false,lastStat:null});opens++;counts.open_returned++;operation.observed={descriptor:value};
  }else if(method==='read'){
   if(!(Number.isSafeInteger(value)&&value>=0&&value<=args[3]))fail('invalid-content-result',{entry_path:entry?.path??null,returned_type:typeof value,returned_number:typeof value==='number'&&Number.isFinite(value)?value:null});
   check(Number.isSafeInteger(value)&&value>=0&&value<=args[3],'content result');const state=fds.get(fd);check(state,'read owned descriptor');
   if(value){state.hash.update(args[1].subarray(args[2],args[2]+value));state.returned=add(state.returned,value);counts.returned_content_bytes=add(counts.returned_content_bytes,value);}
   operation.observed={returned_bytes:value,requested_bytes:args[3]};
   if(args[3]===1&&state.returned>=entry.pin.bytes){
    if(value===0&&state.returned===entry.pin.bytes){state.eof=true;state.digest=state.hash.copy().digest('hex');if(state.digest!==entry.pin.sha256)fail('whole-content-hash-mismatch',{entry_path:entry.path,expected_pin:entry.pin,observed_sha256:state.digest});}
    else if(value!==0&&state.returned>entry.pin.bytes)fail('EOF-growth',{entry_path:entry.path,expected_pin:entry.pin,returned_bytes:state.returned});
   }
   if(value!==args[3]&&!(args[3]===1&&value===0&&state.returned===entry.pin.bytes))fail('short-read',{entry_path:entry.path,expected_pin:entry.pin,requested_bytes:args[3],returned_bytes:value});
  }else if(method==='lstat'||method==='fstat'){
   const observed=diagnosticStat(value);operation.observed=observed;if(observed.invalid_stat_shape)fail('invalid-stat-shape',{entry_path:entry?.path??null,named_path:operation.named_path});
   const expectedKind=method==='fstat'?'file':entry?.kind==='readable'?(aliasRules.has(args[0])?'symlink':'file'):undefined;if(expectedKind&&observed.kind!==expectedKind)fail('metadata-kind-mismatch',{entry_path:entry?.path??null,named_path:operation.named_path,expected_kind:expectedKind,observed_stat:observed});
   if(method==='lstat'&&!observed.invalid_stat_shape){const old=namedStats.get(args[0]);if(old&&!equal(old,observed))fail('named-metadata-drift',{entry_path:entry?.path??null,named_path:args[0],before:old,after:observed});else if(!old)namedStats.set(args[0],observed);}
   if(fd!==null&&fds.has(fd)){const old=fds.get(fd).lastStat,target=namedStats.get(entry.resolved);if(old===null&&target&&!equal(target,observed))fail('named-descriptor-admission-mismatch',{entry_path:entry.path,named_path:entry.resolved,named_target_stat:target,descriptor_stat:observed});if(old&&!equal(old,observed))fail('descriptor-metadata-drift',{entry_path:entry.path,before:old,after:observed});fds.get(fd).lastStat=observed;}
   if(entry?.kind==='readable'&&observed.kind==='file'&&(!equal(observed.identity,entry.identity)||entry.ownership!==null&&!equal({uid:observed.uid,gid:observed.gid},entry.ownership)))
    fail('historical-input-identity-mismatch',{entry_path:entry.path,expected_pin:entry.pin,expected_identity:entry.identity,expected_ownership:entry.ownership,observed_stat:observed});
  }else if(method==='close'){
   lastClosed=describe(fds.get(fd));fds.delete(fd);counts.closed++;operation.observed={closed:true};
  }else{operation.observed=typeof value==='string'?value.slice(0,4096):null;
   const expected=method==='realpath'?(entry?.resolved??args[0]):method==='readlink'?aliasRules.get(args[0])?.link_text:undefined;
   if(expected!==undefined&&value!==expected)fail(method+'-mismatch',{entry_path:entry?.path??null,named_path:args[0],expected,observed:operation.observed});
  }
  return value;
 }
 const wrapped=Object.fromEntries(['lstat','realpath','readlink','open','fstat','read','close'].map(method=>[method,(...args)=>dispatch(method,args,method==='close')]));
 return{provider:wrapped,snapshot:()=>({counts:{...counts},last_operation:lastOperation===null?null:JSON.parse(JSON.stringify(lastOperation)),last_closed:lastClosed,last_provider_error:lastProviderError,live_descriptors:[...fds].map(([descriptor,state])=>({descriptor,...describe(state)})),partial_phase_observation_arrays_available:false})};
}
export function executeAdapterPlan(raw,{provider,guard,now}){
 const plan=admit(raw);check(provider&&typeof provider==='object'&&typeof guard==='function'&&typeof now==='function','explicit provider guard and clock');
 for(const method of ['lstat','realpath','readlink','open','fstat','read','close'])check(typeof provider[method]==='function','provider operation');
 const phases=[['precheck',plan.graph_protocol],['historical',plan.historical_protocol],['postcheck',plan.graph_protocol]].map(([name,protocol])=>({name,status:'not-started',milliseconds:plan.phase_milliseconds[name],budget:protocol.budget,result:null,failure:null,provider_observation:null}));
 const shared={byte_cap:0,content_cap:0,metadata_cap:0,requested_content_bytes:0,content_attempts:0,metadata_attempts:0};
 for(const phase of phases){shared.byte_cap=add(shared.byte_cap,phase.budget.total.reserved_bytes);shared.content_cap=add(shared.content_cap,phase.budget.total.content_calls);shared.metadata_cap=add(shared.metadata_cap,phase.budget.total.metadata_calls);}
 const record={schema:'fe2o3-cpu-loaded-adapter-observation-v1',plan_sha256:sha(raw),status:'failed',first_failure:null,phases,shared_accounting:shared,guard_observation:null,
  input_named_cap:plan.named_cap,root_cap_approval_inferred:false,qualified:false,native_authority:false,gpu_dispatch:false,physical_capture:false,child_processes_started:0,
  source_currentness_qualified:false,global_writer_exclusion:false,durable_evidence_published:false,external_kill_can_prevent_durable_reporting:true};
 const remember=f=>{if(record.first_failure===null)record.first_failure=JSON.parse(JSON.stringify(f));};
 let currentPhase='precheck';
 const guarded=()=>{try{guard();}catch(e){remember({source:'guard',code:'guard-refused',phase:currentPhase,error:boundedError(e)});throw e;}};
 for(let i=0;i<phases.length;i++){
  const phase=phases[i],protocol=i===1?plan.historical_protocol:plan.graph_protocol;currentPhase=phase.name;phase.status='started';
  const tracked=instrument(protocol,provider,guarded,phase.name,shared,remember);
  let phaseStart=null,previousPhaseTime=null;
  const phaseNow=()=>{
   let value;try{value=now();}catch(e){remember({source:'phase-clock',code:'phase-clock-observer-failed',phase:phase.name,error:boundedError(e)});throw e;}
   const bad=!Number.isFinite(value)||(previousPhaseTime!==null&&value<previousPhaseTime)||(phaseStart!==null&&value-phaseStart>=phase.milliseconds);
   if(bad)remember({source:'phase-clock',code:'phase-clock-refused',phase:phase.name,observed_time:Number.isFinite(value)?value:null,previous_time:previousPhaseTime,start_time:phaseStart,milliseconds:phase.milliseconds});
   if(phaseStart===null&&Number.isFinite(value))phaseStart=value;if(Number.isFinite(value))previousPhaseTime=value;return value;
  };
  try{
   phase.result=readUnqualifiedProtocol(protocol,tracked.provider,{guard:guarded,now:phaseNow,milliseconds:phase.milliseconds});
   check(phase.result.io_protocol_completed===true&&phase.result.qualified===false,'bounded reader completion');
   // Observed safety discrepancies cannot be laundered by a later successful return.
   check(record.first_failure===null,'prior retained safety refusal');phase.status='completed';
  }catch(e){
   remember({source:'reader',code:'reader-refused',phase:phase.name,error:boundedError(e),provider_context:tracked.snapshot()});
   phase.failure={error:boundedError(e),reader_accounting:e?.accounting??null};phase.status='failed';phase.provider_observation=tracked.snapshot();break;
  }
  phase.provider_observation=tracked.snapshot();
 }
 if(phases.every(p=>p.status==='completed')&&record.first_failure===null)record.status='read-protocol-completed';
 try{if(typeof guard.snapshot==='function')record.guard_observation=guard.snapshot();}catch(e){remember({source:'guard-reporting',code:'guard-snapshot-failed',phase:currentPhase,error:boundedError(e)});record.status='failed';}
 return freeze(record);
}

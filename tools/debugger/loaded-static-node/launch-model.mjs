// Closed static-launch data admission. No filesystem, environment or process discovery.
import {createHash} from 'node:crypto';
import {isDeepStrictEqual as equal} from 'node:util';
import {compileOperationalGraph,checkedAdd} from '../loaded-operational-graph/loaded-operational-graph.mjs';
import {exactReadEnvelope} from '../loaded-operational-graph/loaded-operational-policy.mjs';
import {createFiniteGuard} from '../loaded-cpu-adapter/adapter-guard.mjs';
import {executeAdapterPlan} from '../loaded-cpu-adapter/adapter-protocol.mjs';
import {admitEvidenceSpec} from '../loaded-cpu-adapter/adapter-writer.mjs';
export const LAUNCH_LIMITS=Object.freeze({bootstrap_bytes:65536,request_bytes:72*1024*1024,graph_bytes:16*1024*1024,plan_bytes:32*1024*1024,policy_bytes:16384,named_inputs:4096,terminal_inputs:64,bootstrap_ms:30000,report_ms:5000,summary_bytes:8*1024*1024});
export const OUTPUT_ROLES=Object.freeze(['reader-observation-temporary','reader-observation-final','outer-receipt','outer-stdout','outer-stderr','root-readback']);
export const EXTERNAL_OBLIGATIONS=Object.freeze(['packet-terminal-source-closure','supervisor-adapter-source','source-selection-policy','root-cap-policy','root-resource-policy','root-currentness-guard-source','fresh-coordinate-and-currentness-evidence','fresh-independent-source-review','fresh-cpu-qualification-evidence','fresh-operational-request','staged-output-names','operational-reader-observation','operational-supervisor-receipt-and-streams','root-complete-readback']);
const check=(v,m)=>{if(!v)throw Error('static Node launch: '+m);};
const freeze=v=>{if(v&&typeof v==='object'&&!Buffer.isBuffer(v)){for(const x of Object.values(v))freeze(x);Object.freeze(v);}return v;};
export const sha=b=>createHash('sha256').update(b).digest('hex');
function keys(x,ks){check(x&&typeof x==='object'&&!Array.isArray(x)&&equal(Object.keys(x).sort(),ks.slice().sort()),'closed keys');}
function uint(v,max){check(Number.isSafeInteger(v)&&v>=0&&v<=max,'integer bound');return v;}
function name(s){check(typeof s==='string'&&Buffer.byteLength(s)>1&&Buffer.byteLength(s)<=4096&&s.startsWith('/')&&!/[\\\x00-\x1f\x7f]/.test(s)&&!s.includes(' (deleted)')&&!s.split('/').slice(1).some(p=>!p||p==='.'||p==='..')&&s.split('/').length<=129,'literal absolute name');return s;}
function digest(s){check(typeof s==='string'&&/^[a-f0-9]{64}$/.test(s),'whole SHA256');return s;}
function decimal(s){check(typeof s==='string'&&/^(0|[1-9][0-9]{0,24})$/.test(s),'decimal identity');return s;}
function pin(x,max=512*1024*1024){keys(x,['path','bytes','sha256']);name(x.path);uint(x.bytes,max);digest(x.sha256);return x;}
function bytePin(x,max){keys(x,['bytes','sha256']);uint(x.bytes,max);check(x.bytes>0,'nonempty payload');digest(x.sha256);return x;}
function json(raw,max){check(Buffer.isBuffer(raw)&&raw.length>0&&raw.length<=max,'complete bounded UTF8 Buffer');const s=raw.toString('utf8');check(Buffer.from(s).equals(raw),'exact UTF8');return JSON.parse(s);}
function part(p,max){keys(p,['bytes','sha256','base64']);bytePin({bytes:p.bytes,sha256:p.sha256},max);check(typeof p.base64==='string'&&p.base64.length===4*Math.ceil(p.bytes/3),'bounded base64 length');const raw=Buffer.from(p.base64,'base64');check(raw.length===p.bytes&&raw.toString('base64')===p.base64&&sha(raw)===p.sha256,'canonical base64 and whole pin');return raw;}
function policy(raw){const inert=()=>{throw Error('policy admission must not observe resources or clocks');};return createFiniteGuard(raw,{monotonicNow:inert,utcNow:inert,resources:inert}).snapshot().policy;}
function requestSpec(x){
 keys(x,['pin','identity','ownership']);pin(x.pin,LAUNCH_LIMITS.request_bytes);check(x.pin.bytes>0,'nonempty request');
 check(Array.isArray(x.identity)&&x.identity.length===6,'six request identity fields');x.identity.forEach(decimal);
 check(x.identity[2]===String(x.pin.bytes)&&(BigInt(x.identity[3])&0o170000n)===0o100000n,'request regular size/mode');
 keys(x.ownership,['uid','gid']);decimal(x.ownership.uid);decimal(x.ownership.gid);return x;
}
export function admitRequestSpec(raw){return freeze(requestSpec(json(raw,16384)));}
export function admitLaunchBootstrap(raw){
 const b=json(raw,LAUNCH_LIMITS.bootstrap_bytes);
 keys(b,['schema','label','request','resource_root','bootstrap_policy','report_policy','adapter_policy_pin','historical_protocol_sha256','named_cap','runtime_pin','entry_path','outputs','external_terminal_pins','declared_input_union_cap','summary_descriptor']);
 check(b.schema==='fe2o3-static-node-bootstrap-v1','bootstrap schema');
 check(typeof b.label==='string'&&/^[a-z][a-z0-9-]{0,99}$/.test(b.label),'literal label');
 requestSpec(b.request);name(b.resource_root);name(b.entry_path);pin(b.runtime_pin);check(b.runtime_pin.bytes>0,'nonempty runtime');digest(b.historical_protocol_sha256);
 uint(b.declared_input_union_cap,8192);check(b.declared_input_union_cap>0,'explicit complete declared-input union cap');
 keys(b.summary_descriptor,['fd','kind','identity','ownership']);const endpoint=b.summary_descriptor;check(endpoint.fd===1&&['file','fifo'].includes(endpoint.kind),'fixed stdout descriptor');check(Array.isArray(endpoint.identity)&&endpoint.identity.length===6,'six stdout identity fields');endpoint.identity.forEach(decimal);keys(endpoint.ownership,['uid','gid']);decimal(endpoint.ownership.uid);decimal(endpoint.ownership.gid);check((BigInt(endpoint.identity[3])&0o170000n)===(endpoint.kind==='file'?0o100000n:0o010000n)&&endpoint.identity[2]==='0','initial empty regular capture or FIFO');
 uint(b.named_cap,LAUNCH_LIMITS.named_inputs);check(b.named_cap>0,'explicit named cap');
 bytePin(b.adapter_policy_pin,LAUNCH_LIMITS.policy_bytes);
 const bootstrap_policy_bytes=part(b.bootstrap_policy,LAUNCH_LIMITS.policy_bytes),report_policy_bytes=part(b.report_policy,LAUNCH_LIMITS.policy_bytes);
 const bp=policy(bootstrap_policy_bytes),rp=policy(report_policy_bytes);check(bp.max_elapsed_ms<=LAUNCH_LIMITS.bootstrap_ms&&rp.max_elapsed_ms<=LAUNCH_LIMITS.report_ms,'bootstrap/report deadlines');
 check(bp.not_before_utc_ms===rp.not_before_utc_ms&&bp.expires_utc_ms===rp.expires_utc_ms,'one externally bound UTC scope');
 check(Array.isArray(b.outputs)&&b.outputs.length===6,'six exact output roles');const outputNames=new Set();let outputCap=0;
 for(let i=0;i<6;i++){const o=b.outputs[i];keys(o,['role','path','cap_bytes']);check(o.role===OUTPUT_ROLES[i],'ordered output roles');name(o.path);uint(o.cap_bytes,i<2?64*1024*1024:LAUNCH_LIMITS.summary_bytes);check(o.cap_bytes>0&&!outputNames.has(o.path),'distinct bounded output names');outputNames.add(o.path);outputCap=checkedAdd(outputCap,o.cap_bytes);}
 check(b.outputs[0].cap_bytes===b.outputs[1].cap_bytes,'two full evidence reservations');
 check(Array.isArray(b.external_terminal_pins)&&b.external_terminal_pins.length>=2&&b.external_terminal_pins.length<=LAUNCH_LIMITS.terminal_inputs,'explicit finite external terminal inputs');
 const terminalNames=new Set();let terminalBytes=0;
 for(const p of b.external_terminal_pins){pin(p);check(!terminalNames.has(p.path)&&p.path!==b.request.pin.path&&!outputNames.has(p.path),'distinct external terminals, not request or output');terminalNames.add(p.path);terminalBytes=checkedAdd(terminalBytes,p.bytes);}
 for(const p of [b.request.pin.path,b.runtime_pin.path,b.entry_path])check(!outputNames.has(p),'output/input collision');
 return freeze({binding:b,bootstrap_policy_bytes,report_policy_bytes,request_spec_bytes:Buffer.from(JSON.stringify(b.request)),bootstrap_policy:bp,report_policy:rp,output_cap_bytes:outputCap,external_terminal_payload_bytes:terminalBytes});
}
function purePlan(raw){
 const data=json(raw,LAUNCH_LIMITS.plan_bytes);let calls=0;const provider=Object.fromEntries(['lstat','realpath','readlink','open','fstat','read','close'].map(k=>[k,()=>{calls++;throw Error('unexpected admission provider '+k);}]));
 const marker='static-launch-pure-admission',probe=executeAdapterPlan(raw,{provider,guard:()=>{throw Error(marker);},now:()=>0});
 check(calls===0&&probe.first_failure?.source==='guard'&&probe.first_failure.error.message===marker,'complete pure adapter admission');
 return {data,envelope:probe.shared_accounting};
}
export function admitStaticLaunchRequest(raw,bootstrapBytes,context){
 const b=admitLaunchBootstrap(bootstrapBytes);
 check(Buffer.isBuffer(raw)&&raw.length===b.binding.request.pin.bytes&&sha(raw)===b.binding.request.pin.sha256,'externally pinned request Buffer');
 const x=json(raw,LAUNCH_LIMITS.request_bytes);
 keys(x,['schema','label','graph','plan','adapter_policy','evidence_spec']);check(x.schema==='fe2o3-static-node-adapter-request-v1'&&x.label===b.binding.label,'request schema and label');
 const graph_bytes=part(x.graph,LAUNCH_LIMITS.graph_bytes),plan_bytes=part(x.plan,LAUNCH_LIMITS.plan_bytes),adapter_policy_bytes=part(x.adapter_policy,LAUNCH_LIMITS.policy_bytes),evidence_spec_bytes=part(x.evidence_spec,16384);
 check(equal({bytes:adapter_policy_bytes.length,sha256:sha(adapter_policy_bytes)},b.binding.adapter_policy_pin),'externally pinned adapter policy');
 const ap=policy(adapter_policy_bytes),bp=b.bootstrap_policy;
 check(ap.not_before_utc_ms===bp.not_before_utc_ms&&ap.expires_utc_ms===bp.expires_utc_ms,'adapter UTC scope');
 const graph=compileOperationalGraph(graph_bytes),{data:plan,envelope}=purePlan(plan_bytes),spec=admitEvidenceSpec(evidence_spec_bytes);
 check(graph.entries.length===b.binding.named_cap&&plan.named_cap===b.binding.named_cap,'exact individually named input cap');
 check(equal(graph.outputs,b.binding.outputs),'six graph output bindings');check(equal(graph.aliases,plan.graph_protocol.aliases),'exact graph aliases');
 check(sha(Buffer.from(JSON.stringify(plan.historical_protocol)))===b.binding.historical_protocol_sha256,'immutable historical protocol digest');
 const byName=new Map(graph.entries.map(r=>[r.path,r])),contentByName=new Map();
 function contentObligation(path,kind,pin,message){const value={kind,pin},old=contentByName.get(path);check(!old||equal(old,value),message);if(!old)contentByName.set(path,value);}
 for(const row of graph.entries)contentObligation(row.path,row.kind,row.pin,'conflicting named content/kind obligation');
 for(const alias of graph.aliases)contentObligation(alias.target_pin.path,'readable',alias.target_pin,'conflicting alias target content/kind obligation');
 for(const terminal of b.binding.external_terminal_pins)contentObligation(terminal.path,'readable',terminal,'external terminal versus graph pin conflict');
 // Request overlap would introduce an unnecessary self-referential whole-pin cycle; refuse it explicitly.
 check(!contentByName.has(b.binding.request.pin.path),'request name overlaps graph name or alias target');
 const inputNames=new Set([...contentByName.keys(),b.binding.request.pin.path]);
 for(let i=0;i<graph.entries.length;i++){
  const g=graph.entries[i],p=plan.graph_protocol.entries[i];check(p&&['path','kind','pin','resolved','identity','ownership'].every(k=>equal(g[k],p[k])),'complete graph/reader named identity binding');
  const labels=Object.values(p.labels).flat();for(const role of g.roles)check(labels.includes(role),'every graph role retained');
 }
 check(inputNames.size===b.binding.declared_input_union_cap,'exact declared input union, including aliases and external terminals');
 for(const o of b.binding.outputs)check(!inputNames.has(o.path),'no output/input or independently named alias-target collision');
 check(spec.directory+'/'+spec.temporary_name===b.binding.outputs[0].path&&spec.directory+'/'+spec.final_name===b.binding.outputs[1].path&&spec.cap_bytes===b.binding.outputs[0].cap_bytes,'exact evidence names and cap');
 keys(context,['runtime_path','entry_path','module_paths','import_edges']);check(context.runtime_path===b.binding.runtime_pin.path&&context.entry_path===b.binding.entry_path,'fixed runtime and entry');
 check(Array.isArray(context.module_paths)&&context.module_paths.length>0&&context.module_paths.length<=128&&new Set(context.module_paths).size===context.module_paths.length&&context.module_paths.includes(context.entry_path),'finite static module closure');
 const runtime=byName.get(context.runtime_path);check(runtime?.kind==='readable'&&equal(runtime.pin,b.binding.runtime_pin),'individually pinned runtime');
 for(const p of context.module_paths){name(p);check(byName.get(p)?.kind==='readable','individually pinned actual static module');}
 check(Array.isArray(context.import_edges)&&context.import_edges.length<=512,'finite static import edges');
 for(const edge of context.import_edges)check(graph.imports.some(e=>equal(e,edge)),'every actual static import edge declared');
 const precheck=exactReadEnvelope(plan.graph_protocol.entries,graph.aliases,1),historical=exactReadEnvelope(plan.historical_protocol.entries,plan.historical_protocol.aliases,2);
 const request={payload_bytes:b.binding.request.pin.bytes,reserved_bytes:checkedAdd(b.binding.request.pin.bytes,1),content_calls:Math.ceil(b.binding.request.pin.bytes/65536)+1,metadata_calls:10,passes:1};
 const controls={reserved_bytes:checkedAdd(request.reserved_bytes,envelope.byte_cap),content_calls:checkedAdd(request.content_calls,envelope.content_cap),metadata_provider_calls:checkedAdd(request.metadata_calls,envelope.metadata_cap)};
 const resources={bootstrap_max_probes:bp.max_resource_probes,adapter_max_probes:ap.max_resource_probes,report_max_probes:b.report_policy.max_resource_probes};
 const probes=Object.values(resources).reduce(checkedAdd,0);resources.total_max_probes=probes;resources.requested_proc_bytes=probes*65537;resources.content_calls=probes*2;resources.statfs_calls=probes;resources.open_attempts=probes;resources.close_attempts=probes;
 return Object.freeze({bootstrap:b,graph,plan_bytes,adapter_policy_bytes,evidence_spec_bytes,resource_root:b.binding.resource_root,
  accounting:freeze({request,phases:{precheck,historical,postcheck:precheck},controlled_reads:controls,resources,
   total_stage_elapsed_ceiling_ms:bp.max_elapsed_ms+ap.max_elapsed_ms+b.report_policy.max_elapsed_ms,
   summary_emission:{cap_bytes:b.binding.outputs[3].cap_bytes,maximum_write_calls:Math.ceil(b.binding.outputs[3].cap_bytes/65536),maximum_requested_bytes:b.binding.outputs[3].cap_bytes,descriptor_metadata_calls:2,no_retry:true,durability_proved:false},
   application_new_owned_descriptor_ceiling:4,inherited_stdout_not_closed_here:true,loader_descriptor_use_excluded:true,
   evidence_named_reservations:b.binding.outputs.slice(0,2),six_output_roles:b.binding.outputs,total_named_output_cap:b.output_cap_bytes,
   static_loader:{module_paths:[...context.module_paths],import_edges:context.import_edges,runtime_pin:b.binding.runtime_pin,actual_io_metered:false,external_preexecution_pin_required:true},
   external_terminal_inputs:b.binding.external_terminal_pins,external_terminal_payload_bytes:b.external_terminal_payload_bytes,declared_input_union_count:inputNames.size,declared_input_union_names:[...inputNames],actual_loader_names_complete:false,
   module_loader_resource_supervisor_and_root_readback_not_in_controlled_read_subtotal:true,qualified:false,global_io_bound_proved:false}),
  obligations:EXTERNAL_OBLIGATIONS.map(role=>({role,closed_by_this_component:false})),execution_authority:false,qualified:false,native_authority:false});
}

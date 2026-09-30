// Pure fresh-fixture preparation and readback assertions. No filesystem or environment IO.
import {isDeepStrictEqual as equal} from 'node:util';
import {compileOperationalGraph} from '../loaded-operational-graph/loaded-operational-graph.mjs';
import {deriveReadBudget} from '../loaded-input-reader/reader-protocol.mjs';
import {admitLaunchBootstrap,admitStaticLaunchRequest,EXTERNAL_OBLIGATIONS,OUTPUT_ROLES,sha} from './launch-model.mjs';
import {staticModuleContext} from './launch-module-map.mjs';
const check=(v,m)=>{if(!v)throw Error('static launch synthetic fixture: '+m);};
const json=v=>Buffer.from(JSON.stringify(v));
const part=b=>({bytes:b.length,sha256:sha(b),base64:b.toString('base64')});
function decode(b,max){check(Buffer.isBuffer(b)&&b.length>0&&b.length<=max,'bounded explicit Buffer');const s=b.toString('utf8');check(Buffer.from(s).equals(b),'UTF8');return JSON.parse(s);}
function keys(x,k){check(x&&typeof x==='object'&&!Array.isArray(x)&&equal(Object.keys(x).sort(),k.slice().sort()),'closed keys');}
function protocol(entries,aliases,passes,selection_digest){return {schema:'fe2o3-loaded-read-protocol-v1',entries,aliases,passes,budget:deriveReadBudget(entries,aliases,passes),selection_digest,custody:{qualified_historical_input_bytes:false,operational_roster_complete:false,root_cap_change_approved:false,execution_authority:false}};}
/**
 * Root creates and observes fresh inputs before calling this pure builder.
 * Source claims are complete root-observed metadata, not self-pins discovered by this module.
 */
export function buildSyntheticLaunchRequest(inputBytes){
 const x=decode(inputBytes,16*1024*1024);
 keys(x,['schema','label','runtime_path','source_claims','fixture_directory','fixture_entries','aliases','outputs','output_directory_identity','resource_root','adapter_policy','bootstrap_policy','report_policy','external_terminal_pins','phase_milliseconds']);
 check(x.schema==='fe2o3-static-launch-synthetic-input-v1','schema');
 const context=staticModuleContext(x.runtime_path),sourceNames=[x.runtime_path,...context.module_paths];
 check(Array.isArray(x.source_claims)&&x.source_claims.length===sourceNames.length&&new Set(x.source_claims.map(c=>c.path)).size===sourceNames.length,'one claim per actual source/runtime');
 check(sourceNames.every(p=>x.source_claims.some(c=>c.path===p&&c.kind==='readable'&&c.resolved===p)),'exact canonical static source closure');
 check(typeof x.fixture_directory==='string'&&x.fixture_directory.startsWith('/')&&!x.fixture_directory.endsWith('/'),'explicit fresh fixture directory');
 check(Array.isArray(x.fixture_entries)&&x.fixture_entries.length===4,'four synthetic roles');
 const names=['target.bin','alias.bin','empty.bin','absent.bin'];
 for(let i=0;i<4;i++)check(x.fixture_entries[i].path===x.fixture_directory+'/'+names[i],'fixed fresh fixture names');
 const [target,alias,empty,absent]=x.fixture_entries;
 check(target.kind==='readable'&&target.pin.bytes>65536&&target.pin.bytes<=131072&&target.resolved===target.path,'two-chunk target');
 check(alias.kind==='readable'&&alias.resolved===target.path&&equal(alias.pin,{...target.pin,path:alias.path})&&equal(alias.identity,target.identity),'exact independently named alias target');
 check(empty.kind==='readable'&&empty.pin.bytes===0&&empty.pin.sha256===sha(Buffer.alloc(0))&&empty.resolved===empty.path,'real observed empty file');
 check(absent.kind==='absence-observation','bounded absence, not empty file');
 check(Array.isArray(x.aliases)&&x.aliases.length===1&&equal(x.aliases[0],{pin:alias.pin,resolved:target.path,link_text:'target.bin',target_pin:target.pin,target_selected:true}),'one exact relative alias');
 check(x.outputs.length===6&&x.outputs.every((o,i)=>o.role===OUTPUT_ROLES[i]),'six ordered output roles');
 const fixtureClaims=x.fixture_entries.map((r,i)=>({id:'synthetic-fixture:'+i,path:r.path,kind:r.kind,pin:r.pin,resolved:r.resolved,identity:r.identity,ownership:r.ownership,roles:Object.values(r.labels).flat()}));
 const graphInput={schema:'fe2o3-loaded-operational-graph-input-v1',claims:[...x.source_claims,...fixtureClaims],aliases:x.aliases,imports:context.import_edges,
  unresolved:EXTERNAL_OBLIGATIONS.map((role,i)=>({role,stage:i<11?'before-read':'after-read',reason:'synthetic qualification alone does not close actual historical operational evidence'})),outputs:x.outputs};
 const graphBytes=json(graphInput),graph=compileOperationalGraph(graphBytes);
 const byFixture=new Map(x.fixture_entries.map(r=>[r.path,r]));
 const entries=graph.entries.map(r=>byFixture.get(r.path)??{path:r.path,kind:r.kind,pin:r.pin,resolved:r.resolved,identity:r.identity,ownership:r.ownership,labels:{prior:[],loaded:[],duties:[],extras:r.roles},cap:r.pin.bytes});
 const selection=sha(graphBytes),historical=protocol(x.fixture_entries,x.aliases,2,selection);
 const plan={schema:'fe2o3-cpu-loaded-adapter-plan-v1',graph_protocol:protocol(entries,x.aliases,1,selection),historical_protocol:historical,named_cap:entries.length,phase_milliseconds:x.phase_milliseconds};
 const dir=x.outputs[0].path.slice(0,x.outputs[0].path.lastIndexOf('/'));
 check(x.outputs[1].path.startsWith(dir+'/')&&!x.outputs[1].path.slice(dir.length+1).includes('/'),'one explicit evidence directory');
 const spec={schema:'fe2o3-exclusive-adapter-evidence-v1',directory:dir,directory_identity:x.output_directory_identity,temporary_name:x.outputs[0].path.slice(dir.length+1),final_name:x.outputs[1].path.slice(dir.length+1),cap_bytes:x.outputs[0].cap_bytes};
 const requestBytes=json({schema:'fe2o3-static-node-adapter-request-v1',label:x.label,graph:part(graphBytes),plan:part(json(plan)),adapter_policy:part(json(x.adapter_policy)),evidence_spec:part(json(spec))});
 const template={schema:'fe2o3-static-node-bootstrap-v1',label:x.label,request:null,resource_root:x.resource_root,bootstrap_policy:part(json(x.bootstrap_policy)),report_policy:part(json(x.report_policy)),adapter_policy_pin:{bytes:json(x.adapter_policy).length,sha256:sha(json(x.adapter_policy))},
  historical_protocol_sha256:sha(json(historical)),named_cap:entries.length,runtime_pin:x.source_claims.find(c=>c.path===x.runtime_path).pin,entry_path:context.entry_path,outputs:x.outputs,external_terminal_pins:x.external_terminal_pins,declared_input_union_cap:null,summary_descriptor:null};
 return {request_bytes:requestBytes,bootstrap_template_bytes:json(template),expected_bytes:json({schema:'fe2o3-static-launch-synthetic-expected-v1',label:x.label,graph_named_count:entries.length,fixture_paths:x.fixture_entries.map(r=>r.path),historical_budget:historical.budget,plan_sha256:sha(json(plan)),outputs:x.outputs}),context,actual_io_performed:false,qualified:false};
}
/** Call only after root exclusively writes/identifies the request and pre-opens stdout. */
export function bindSyntheticBootstrap(requestBytes,templateBytes,bindingBytes){
 const t=decode(templateBytes,65536),binding=decode(bindingBytes,16384);
 keys(binding,['request','summary_descriptor']);
 check(t.request===null&&t.summary_descriptor===null&&t.declared_input_union_cap===null,'unbound template');
 check(binding.request.pin.bytes===requestBytes.length&&binding.request.pin.sha256===sha(requestBytes),'root-observed request whole pin');
 const request=decode(requestBytes,72*1024*1024),graph=compileOperationalGraph(Buffer.from(request.graph.base64,'base64'));
 t.request=binding.request;t.summary_descriptor=binding.summary_descriptor;
 t.declared_input_union_cap=new Set([...graph.entries.map(r=>r.path),...graph.entries.map(r=>r.resolved),t.request.pin.path,...t.external_terminal_pins.map(p=>p.path)]).size;
 const bytes=json(t);admitLaunchBootstrap(bytes);admitStaticLaunchRequest(requestBytes,bytes,staticModuleContext(t.runtime_pin.path));
 return bytes;
}
/** Complete root-read Buffers and an independently observed process exit are required. */
export function verifySyntheticLaunchResult(stdoutBytes,evidenceBytes,temporaryBytes,expectedBytes,exitCode){
 const expected=decode(expectedBytes,65536),summary=decode(stdoutBytes,8*1024*1024),evidence=decode(evidenceBytes,64*1024*1024);
 keys(expected,['schema','label','graph_named_count','fixture_paths','historical_budget','plan_sha256','outputs']);
 check(expected.schema==='fe2o3-static-launch-synthetic-expected-v1'&&exitCode===0,'expected schema and external process exit');
 check(summary.schema==='fe2o3-static-node-command-observation-v1'&&summary.label===expected.label&&summary.status==='adapter-returned'&&summary.first_failure===null,'complete successful command intent');
 check(summary.request_read?.status==='complete-request-observed'&&summary.request_admission==='completed'&&summary.adapter_dispatch==='returned','request to adapter seam');
 check(summary.adapter_summary?.status==='read-and-publication-observed'&&summary.adapter_summary.first_failure===null,'adapter and publication observed');
 check(equal(summary.output_roles,expected.outputs),'six exact outputs');
 check(summary.qualified===false&&summary.authority===false&&summary.native_authority===false&&summary.child_processes_started===0,'no promoted authority');
 check(equal(summary.obligations,EXTERNAL_OBLIGATIONS.map(role=>({role,closed_by_this_component:false}))),'all fourteen obligations remain external');
 check(Buffer.isBuffer(temporaryBytes)&&temporaryBytes.equals(evidenceBytes),'retained complete temporary equals final bytes');
 check(equal(summary.adapter_summary.evidence_pin,{path:expected.outputs[1].path,bytes:evidenceBytes.length,sha256:sha(evidenceBytes)}),'complete evidence pin');
 check(evidence.publication_state==='intent only; consult actual command-result and root readback','record remains intent');
 const r=evidence.reader_record;check(r?.schema==='fe2o3-cpu-loaded-adapter-observation-v1'&&r.status==='read-protocol-completed'&&r.first_failure===null&&r.plan_sha256===expected.plan_sha256&&r.input_named_cap===expected.graph_named_count,'complete selected plan record');
 check(equal(r.phases.map(p=>[p.name,p.status]),[['precheck','completed'],['historical','completed'],['postcheck','completed']]),'three completed phases');
 check(equal(r.phases[1].budget,expected.historical_budget),'exact synthetic two-pass budget');
 check(r.phases[1].result.counts.completed_passes===2&&equal(r.phases[1].result.passes[0].map(p=>p.kind==='readable'?p.pin.path:p.path),expected.fixture_paths),'four distinct fixture roles');
 check(r.phases[1].result.counts.completed_absence===2&&r.phases[1].result.counts.completed_readable===6,'absence and empty file separately observed');
 check(r.phases.every(p=>p.result.counts.opened===p.result.counts.closed&&p.result.counts.content_calls===p.budget.total.content_calls&&p.result.counts.attempted_content_bytes===p.budget.total.reserved_bytes),'complete bounded phases and cleanup');
 return {synthetic_request_to_adapter_to_bounded_report_observed:true,root_supplied_exit:exitCode,qualified:false,historical_operation_activated:false,native_authority:false,global_io_bound_proved:false};
}

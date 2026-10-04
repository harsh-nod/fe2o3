// Fixture-free only: all file and stream providers below are in-memory doubles.
import test from 'node:test';
import assert from 'node:assert/strict';
import {admitLaunchBootstrap,admitRequestSpec,admitStaticLaunchRequest,EXTERNAL_OBLIGATIONS,LAUNCH_LIMITS,OUTPUT_ROLES,sha} from './launch-model.mjs';
import {readPinnedRequest} from './launch-reader.mjs';
import {executeStaticLaunch,deferLaunchGuard} from './launch-run.mjs';
import {deriveReadBudget} from '../loaded-input-reader/reader-protocol.mjs';
const json=v=>Buffer.from(JSON.stringify(v));
const part=b=>({bytes:b.length,sha256:sha(b),base64:b.toString('base64')});
const id=(bytes,ino=1,mode=33152)=>['1',String(ino),String(bytes),String(mode),'1','1'];
const ownership={uid:'10',gid:'10'};
const policy=(ms=2000)=>({schema:'fe2o3-cpu-reader-guard-policy-v1',not_before_utc_ms:1000,expires_utc_ms:100000,max_elapsed_ms:ms,max_rss_bytes:536870912,min_free_bytes:1,min_available_ram_bytes:1,resource_interval_ms:1000,max_resource_probes:4});
function specimen(){
 const rows=['/unit/node','/unit/main.mjs','/unit/kernel-input'].map((path,i)=>({path,kind:'readable',pin:{path,bytes:1,sha256:sha(Buffer.from('x'))},resolved:path,identity:id(1,i+1),ownership:{...ownership},labels:{prior:[],loaded:[],duties:['role:'+i],extras:[]},cap:1}));
 const protocol=(entries,passes)=>({schema:'fe2o3-loaded-read-protocol-v1',entries,aliases:[],passes,budget:deriveReadBudget(entries,[],passes),selection_digest:'a'.repeat(64),custody:{qualified_historical_input_bytes:false,operational_roster_complete:false,root_cap_change_approved:false,execution_authority:false}});
 const plan={schema:'fe2o3-cpu-loaded-adapter-plan-v1',graph_protocol:protocol(rows,1),historical_protocol:protocol([structuredClone(rows[2])],2),named_cap:3,phase_milliseconds:{precheck:1000,historical:1000,postcheck:1000}};
 const outputs=OUTPUT_ROLES.map((role,i)=>({role,path:'/output/'+['observation.pending','observation.json','receipt.json','stdout','stderr','readback.json'][i],cap_bytes:65536}));
 const graph={schema:'fe2o3-loaded-operational-graph-input-v1',claims:rows.map((r,i)=>({id:'claim:'+i,path:r.path,kind:r.kind,pin:r.pin,resolved:r.resolved,identity:r.identity,ownership:r.ownership,roles:r.labels.duties})),aliases:[],imports:[{from:rows[1].path,specifier:'node:crypto',to:rows[0].path}],unresolved:[{role:'fresh-root-evidence',stage:'before-read',reason:'not provided by synthetic fixture'}],outputs};
 const spec={schema:'fe2o3-exclusive-adapter-evidence-v1',directory:'/output',directory_identity:id(0,99,16832),temporary_name:'observation.pending',final_name:'observation.json',cap_bytes:65536};
 const p=policy(),request={schema:'fe2o3-static-node-adapter-request-v1',label:'unit-launch',graph:null,plan:null,adapter_policy:null,evidence_spec:null};
 const boot={schema:'fe2o3-static-node-bootstrap-v1',label:request.label,request:null,resource_root:'/unit/resource-root',bootstrap_policy:part(json(policy(1000))),report_policy:part(json(policy(1000))),adapter_policy_pin:{bytes:json(p).length,sha256:sha(json(p))},historical_protocol_sha256:sha(json(plan.historical_protocol)),named_cap:3,runtime_pin:rows[0].pin,entry_path:rows[1].path,outputs,external_terminal_pins:['/terminal/closure','/terminal/currentness'].map(path=>({path,bytes:1,sha256:sha(Buffer.from('t'))})),declared_input_union_cap:6,summary_descriptor:{fd:1,kind:'file',identity:id(0,77),ownership:{...ownership}}};
 const q={rows,plan,graph,spec,policy:p,request,boot,context:{runtime_path:rows[0].path,entry_path:rows[1].path,module_paths:[rows[1].path],import_edges:structuredClone(graph.imports)}};
 q.seal=()=>{request.graph=part(json(graph));request.plan=part(json(plan));request.adapter_policy=part(json(p));request.evidence_spec=part(json(spec));q.raw=json(request);boot.request={pin:{path:'/unit/request.json',bytes:q.raw.length,sha256:sha(q.raw)},identity:id(q.raw.length,40),ownership:{...ownership}};};
 q.bootstrap=()=>json(boot);q.admit=()=>admitStaticLaunchRequest(q.raw,q.bootstrap(),q.context);q.seal();return q;
}
function memory(body,spec=null){
 const s=spec??{pin:{path:'/unit/request.json',bytes:body.length,sha256:sha(body)},identity:id(body.length,40),ownership:{...ownership}};
 const trace=[];let at=0;
 const observed=()=>({kind:'file',identity:[...s.identity],uid:s.ownership.uid,gid:s.ownership.gid});
 const provider={
  lstat:p=>{trace.push('lstat');assert.equal(p,s.pin.path);return observed();},
  realpath:p=>{trace.push('realpath');return p;},
  open:p=>{trace.push('open');assert.equal(p,s.pin.path);at=0;return 7;},
  fstat:fd=>{trace.push('fstat');assert.equal(fd,7);return observed();},
  read:(fd,b,offset,length)=>{trace.push('read:'+length);assert.equal(fd,7);const n=Math.min(length,body.length-at);body.copy(b,offset,at,at+n);at+=n;return n;},
  close:fd=>{trace.push('close');assert.equal(fd,7);},
 };
 return {spec:s,provider,trace,run:guard=>readPinnedRequest(json(s),provider,{guard:guard??(()=>{})})};
}
function harness(q=specimen()){
 const mem=memory(q.raw,q.boot.request),writes=[],calls={adapter:0,before:0,after:0};
 const h={q,mem,writes,calls,bootstrapGuard:()=>{},reportGuard:()=>{},context:q.context,
  invokeAdapter:(plan,p,s,o)=>{calls.adapter++;assert(Buffer.isBuffer(plan)&&Buffer.isBuffer(p)&&Buffer.isBuffer(s));assert.equal(o.resource_root,q.boot.resource_root);return {status:'read-and-publication-observed',first_failure:null,summary:{schema:'fe2o3-cpu-adapter-command-result-v1',status:'read-and-publication-observed',qualified:false,native_authority:false,durable_evidence_available:false}};},
  summarySink:{before:s=>{calls.before++;return s;},write:(b,o,n)=>{writes.push(Buffer.from(b.subarray(o,o+n)));return n;},after:s=>{calls.after++;return s;}},
 };
 h.run=()=>executeStaticLaunch(q.bootstrap(),{requestProvider:mem.provider,bootstrapGuard:h.bootstrapGuard,reportGuard:h.reportGuard,context:h.context,invokeAdapter:h.invokeAdapter,summarySink:h.summarySink});return h;
}
test('complete explicit Buffer admission retains separate domains and exact union',()=>{
 const q=specimen(),a=q.admit();assert.equal(a.accounting.declared_input_union_count,6);assert.equal(a.accounting.request.metadata_calls,10);
 assert.equal(a.accounting.controlled_reads.reserved_bytes,q.raw.length+1+16);
 assert.equal(a.accounting.phases.historical.passes,2);assert.equal(a.accounting.resources.total_max_probes,12);
 assert.equal(a.accounting.summary_emission.descriptor_metadata_calls,2);assert.equal(a.execution_authority,false);
});
test('bootstrap rejects missing non-Buffer invalid UTF8 and oversized JSON',()=>{
 for(const raw of [null,'{}',Buffer.from([255]),Buffer.alloc(65537)])assert.throws(()=>admitLaunchBootstrap(raw));
});
test('bootstrap never admits command environment or extra fields',()=>{
 for(const field of ['command','environment','import','scope_approval']){const q=specimen();q.boot[field]=true;assert.throws(()=>admitLaunchBootstrap(q.bootstrap()),/closed keys/);}
});
test('six role ordering identity and finite reservations are mandatory',()=>{
 for(const mutate of [b=>b.outputs.pop(),b=>b.outputs.reverse(),b=>b.outputs[1].path=b.outputs[0].path,b=>b.outputs[3].cap_bytes=0,b=>b.outputs[3].cap_bytes=8388609,b=>b.outputs[0].cap_bytes--]){const q=specimen();mutate(q.boot);assert.throws(()=>admitLaunchBootstrap(q.bootstrap()));}
});
test('external terminals never alias each other the request or output',()=>{
 for(const path of ['/unit/request.json','/output/stdout','/terminal/closure']){const q=specimen();q.boot.external_terminal_pins[1].path=path;assert.throws(()=>admitLaunchBootstrap(q.bootstrap()));}
});
test('bootstrap request cannot supply placeholder identity ownership or empty contents',()=>{
 for(const mutate of [s=>s.identity=null,s=>s.identity[2]='1',s=>s.ownership=null,s=>s.identity[3]='16832',s=>s.pin.bytes=0]){const q=specimen();mutate(q.boot.request);assert.throws(()=>admitRequestSpec(json(q.boot.request)));}
});
test('bootstrap and report policies share an external finite scope without clock IO',()=>{
 const q=specimen();assert.doesNotThrow(()=>admitLaunchBootstrap(q.bootstrap()));
 for(const mutate of [p=>p.expires_utc_ms++,p=>p.max_elapsed_ms=5001]){const z=specimen(),p=policy(1000);mutate(p);z.boot.report_policy=part(json(p));assert.throws(()=>admitLaunchBootstrap(z.bootstrap()));}
});
test('stdout requires a fixed initially empty file or FIFO descriptor identity',()=>{
 for(const mutate of [s=>s.fd=2,s=>s.kind='socket',s=>s.identity[2]='1',s=>s.ownership.uid='-1',s=>s.identity[3]='16832']){const q=specimen();mutate(q.boot.summary_descriptor);assert.throws(()=>admitLaunchBootstrap(q.bootstrap()));}
 const q=specimen();q.boot.summary_descriptor.kind='fifo';q.boot.summary_descriptor.identity[3]=String(0o10600);assert.doesNotThrow(()=>admitLaunchBootstrap(q.bootstrap()));
});
test('request whole pin is checked before payload interpretation',()=>{
 const q=specimen();q.raw=Buffer.concat([q.raw,Buffer.from(' ')]);assert.throws(q.admit,/externally pinned request/);
});
test('request cannot select extra command import or environment',()=>{
 for(const k of ['command','import','environment']){const q=specimen();q.request[k]={};q.raw=json(q.request);q.boot.request.pin.bytes=q.raw.length;q.boot.request.identity[2]=String(q.raw.length);q.boot.request.pin.sha256=sha(q.raw);assert.throws(q.admit,/closed keys/);}
});
test('payloads require exact canonical base64 full length and whole SHA',()=>{
 for(const mutate of [p=>p.base64+=' ',p=>p.sha256='0'.repeat(64),p=>p.bytes--]){const q=specimen();mutate(q.request.graph);q.raw=json(q.request);q.boot.request.pin.bytes=q.raw.length;q.boot.request.identity[2]=String(q.raw.length);q.boot.request.pin.sha256=sha(q.raw);assert.throws(q.admit);}
});
test('adapter policy remains pinned outside the request',()=>{
 const q=specimen();q.policy.max_rss_bytes++;q.seal();assert.throws(q.admit,/externally pinned adapter policy/);
});
test('historical content identity cannot silently refresh',()=>{
 const q=specimen();q.plan.historical_protocol.selection_digest='b'.repeat(64);q.seal();assert.throws(q.admit,/immutable historical protocol digest/);
});
test('exact named graph cap and inclusive union cap both matter',()=>{
 for(const key of ['named_cap','declared_input_union_cap']){const q=specimen();q.boot[key]++;assert.throws(q.admit);}
});
test('whole graph claim identity must match its qualified reader entry',()=>{
 const q=specimen();q.graph.claims[0].identity=[...q.rows[0].identity];q.graph.claims[0].identity[1]='900';q.seal();assert.throws(q.admit,/complete graph\/reader/);
});
test('no graph duty is discarded by the reader projection',()=>{
 const q=specimen();q.graph.claims[0].roles=['unpaid-role'];q.seal();assert.throws(q.admit,/every graph role retained/);
});
test('actual static modules imports and runtime are individually declared',()=>{
 for(const mutate of [c=>c.module_paths.push('/unit/missing.mjs'),c=>c.import_edges.push({from:'/unit/main.mjs',specifier:'node:fs',to:'/unit/node'}),c=>c.runtime_path='/unit/other-node']){const q=specimen();mutate(q.context);assert.throws(q.admit);}
});
test('evidence names and caps must match the exact six output bindings',()=>{
 for(const mutate of [s=>s.final_name='other.json',s=>s.cap_bytes--]){const q=specimen();mutate(q.spec);q.seal();assert.throws(q.admit,/exact evidence names and cap/);}
});
test('external terminal versus selected graph content conflicts refuse',()=>{
 const q=specimen();q.boot.external_terminal_pins[0]={path:'/unit/kernel-input',bytes:1,sha256:'f'.repeat(64)};q.boot.declared_input_union_cap=5;assert.throws(q.admit,/terminal versus graph pin conflict/);
});
test('unresolved source loader and all fourteen obligations stay unqualified',()=>{
 const a=specimen().admit();assert.equal(a.obligations.length,14);assert.deepEqual(a.obligations.map(x=>x.role),EXTERNAL_OBLIGATIONS);assert(a.obligations.every(x=>x.closed_by_this_component===false));
 assert.equal(a.accounting.static_loader.actual_io_metered,false);assert.equal(a.accounting.actual_loader_names_complete,false);assert.equal(a.qualified,false);assert.equal(a.graph.complete_operational_graph,false);
});
test('request reader consumes all bytes exact64KiB plus EOF and ten metadata calls',()=>{
 const m=memory(Buffer.alloc(65537,7)),r=m.run();assert.equal(r.observation.status,'complete-request-observed');assert.deepEqual(r.bytes,Buffer.alloc(65537,7));
 assert.deepEqual(m.trace.filter(x=>x.startsWith('read:')),['read:65536','read:1','read:1']);assert.equal(r.observation.counts.metadata_calls,10);assert.equal(r.observation.counts.closed,1);
});
test('request read short result is refused once without retry or exposing partial bytes',()=>{
 const m=memory(Buffer.alloc(7,7));m.provider.read=()=>6;const r=m.run();assert.equal(r.bytes,null);assert.match(r.observation.first_failure.error.message,/short read/);assert.equal(r.observation.counts.content_calls,1);assert.equal(m.trace.filter(x=>x==='close').length,1);
});
test('request EOF growth is charged and refused',()=>{
 const m=memory(Buffer.from('x')),read=m.provider.read;let n=0;m.provider.read=(...args)=>++n===2?1:read(...args);
 const r=m.run();assert.match(r.observation.first_failure.error.message,/EOF growth/);assert.equal(r.observation.counts.attempted_content_bytes,2);assert.equal(r.bytes,null);
});
test('wrong request hash stays primary when cleanup close also fails',()=>{
 const m=memory(Buffer.from('x'));m.spec.pin.sha256='0'.repeat(64);m.provider.close=()=>{throw Error('late close');};const r=m.run();
 assert.match(r.observation.first_failure.error.message,/whole request hash/);assert.equal(r.observation.cleanup_errors[0].error.message,'late close');assert.equal(r.observation.descriptor.live_fd_possible,true);
});
test('named owner and initial descriptor disagreement precede close errors',()=>{
 const m=memory(Buffer.from('x')),fstat=m.provider.fstat;m.provider.fstat=fd=>({...fstat(fd),uid:'11'});m.provider.close=()=>{throw Error('close');};
 const r=m.run();assert.match(r.observation.first_failure.error.message,/initial named\/descriptor/);assert.equal(r.observation.cleanup_errors.length,1);
});
test('request named and realpath admission refuse before open',()=>{
 for(const kind of ['named','realpath']){const m=memory(Buffer.from('x'));if(kind==='named')m.provider.lstat=()=>({kind:'directory',identity:id(1,40,16832),uid:'10',gid:'10'});else m.provider.realpath=()=>'/elsewhere';const r=m.run();assert.equal(r.bytes,null);assert(!m.trace.includes('open'));}
});
test('request post-close name drift refuses despite complete earlier hash',()=>{
 const m=memory(Buffer.from('x')),ls=m.provider.lstat;let n=0;m.provider.lstat=p=>{const s=ls(p);if(++n===3)s.identity[1]='999';return s;};
 const r=m.run();assert.equal(r.bytes,null);assert.match(r.observation.first_failure.error.message,/post-close named/);assert.equal(r.observation.counts.closed,1);
});
test('guard denial preserves attempted metadata debit without provider invocation',()=>{
 const m=memory(Buffer.from('x'));let n=0;const r=m.run(()=>{if(++n===2)throw Error('expired');});
 assert.equal(r.observation.counts.metadata_calls,1);assert.equal(r.observation.counts.provider_invocations,0);assert.equal(r.observation.first_failure.error.message,'expired');
});
test('guard expiry after content still permits exactly one owned close',()=>{
 const m=memory(Buffer.from('x')),read=m.provider.read;let expired=false;m.provider.read=(...a)=>{const n=read(...a);expired=true;return n;};
 const r=m.run(()=>{if(expired)throw Error('scope ended');});assert.equal(r.observation.counts.close_attempts,1);assert.equal(r.observation.counts.closed,1);assert.equal(r.observation.counts.content_calls,2);assert.equal(r.observation.counts.attempted_content_bytes,2);
});
test('throw-null read remains primary rather than doubling as an unset sentinel',()=>{
 const m=memory(Buffer.from('x'));m.provider.read=()=>{throw null;};m.provider.close=()=>{throw Error('secondary');};
 const r=m.run();assert.equal(r.observation.first_failure.error.message,'null');assert.equal(r.observation.cleanup_errors[0].error.message,'secondary');
});
test('expired post-close guard allows no fresh final metadata call',()=>{
 const m=memory(Buffer.from('x')),close=m.provider.close;let denied=false;m.provider.close=fd=>{close(fd);denied=true;};
 const r=m.run(()=>{if(denied)throw Error('after close');});assert.equal(r.bytes,null);assert.equal(r.observation.counts.closed,1);assert.equal(m.trace.filter(x=>x==='lstat').length,2);
});
test('request byte cap and complete spec schema refuse before callbacks',()=>{
 const m=memory(Buffer.from('x'));m.spec.pin.bytes=LAUNCH_LIMITS.request_bytes+1;m.spec.identity[2]=String(m.spec.pin.bytes);assert.throws(()=>m.run());assert.equal(m.trace.length,0);
});
test('static dispatch invokes exactly one adapter then one bounded summary transfer',()=>{
 const h=harness(),r=h.run();assert.equal(r.exit_code,0);assert.equal(h.calls.adapter,1);assert.equal(h.calls.before,1);assert.equal(h.calls.after,1);
 const body=Buffer.concat(h.writes);assert.equal(body.length,r.summary_transfer.submitted_bytes);assert.equal(sha(body),r.summary_transfer.submitted_sha256);
 assert.equal(JSON.parse(body).status,'adapter-returned');assert.match(JSON.parse(body).summary_publication,/intent only/);assert.equal(r.summary_transfer.durable_publication_proved,false);
});
test('admission failure never invokes adapter but has bounded failure summary',()=>{
 const q=specimen();q.graph.claims[0].roles=['lost'];q.seal();const h=harness(q),r=h.run();assert.equal(r.exit_code,1);assert.equal(h.calls.adapter,0);assert.equal(r.observation.request_admission,'failed');assert.equal(r.observation.adapter_dispatch,'not-started');assert.equal(r.summary_transfer.complete_write_observed,true);
});
test('request failure skips both plan admission and adapter dispatch',()=>{
 const h=harness();h.q.boot.request.pin.sha256='0'.repeat(64);const r=h.run();assert.equal(h.calls.adapter,0);assert.equal(r.observation.request_admission,'not-started');assert.equal(r.observation.first_failure.stage,'request-read');
});
test('malformed bootstrap admits no request or summary provider effects',()=>{
 const h=harness();h.q.boot.command=['arbitrary'];assert.throws(h.run);assert.equal(h.mem.trace.length,0);assert.equal(h.writes.length,0);assert.equal(h.calls.adapter,0);
});
test('short stdout write refuses without retry while retaining partial custody',()=>{
 const h=harness();h.summarySink.write=(b,o,n)=>n-1;const r=h.run();assert.equal(r.exit_code,1);assert.equal(r.summary_transfer.calls,1);assert.equal(r.summary_transfer.partial_capture_may_remain,true);assert.equal(h.calls.after,0);assert.match(r.summary_transfer.first_error.message,/short summary write/);
});
test('stdout descriptor precheck failure prevents all writes',()=>{
 const h=harness();h.summarySink.before=()=>{throw Error('endpoint changed');};const r=h.run();assert.equal(r.summary_transfer.metadata_calls,1);assert.equal(r.summary_transfer.provider_invocations,0);assert.equal(h.writes.length,0);assert.equal(r.observation.first_failure.stage,'summary-transfer');
});
test('stdout descriptor postcheck failure preserves the complete but unaccepted prefix',()=>{
 const h=harness();h.summarySink.after=()=>{throw Error('post-write identity drift');};const r=h.run();assert.equal(r.summary_transfer.returned_bytes,r.summary_transfer.submitted_bytes);assert.equal(r.summary_transfer.complete_write_observed,false);assert.equal(r.exit_code,1);
});
test('request failure remains first when the summary sink fails too',()=>{
 const h=harness();h.q.boot.request.pin.sha256='0'.repeat(64);h.summarySink.write=()=>{throw Error('sink error');};const r=h.run();assert.equal(r.observation.first_failure.stage,'request-read');assert.equal(r.summary_transfer.first_error.message,'sink error');
});
test('expired reporting guard forbids even endpoint metadata and summary writes',()=>{
 const h=harness();h.reportGuard=()=>{throw Error('report scope ended');};const r=h.run();assert.equal(r.summary_transfer.metadata_invocations,0);assert.equal(r.summary_transfer.provider_invocations,0);assert.equal(h.writes.length,0);assert.equal(r.exit_code,1);
});
test('final report currentness is checked after the last metadata observation',()=>{
 const h=harness();let expired=false;h.summarySink.after=()=>{expired=true;return {};};h.reportGuard=()=>{if(expired)throw Error('post-cleanup deadline');};
 const r=h.run();assert.equal(r.summary_transfer.returned_bytes,r.summary_transfer.submitted_bytes);assert.equal(r.summary_transfer.complete_write_observed,false);assert.equal(r.exit_code,1);
});
test('throw-null adapter dispatch is diagnosed without invoking request-supplied code',()=>{
 const h=harness();h.invokeAdapter=()=>{throw null;};const r=h.run();assert.equal(r.observation.first_failure.stage,'adapter-dispatch');assert.equal(r.observation.first_failure.error.message,'null');assert.equal(r.summary_transfer.complete_write_observed,true);
});
test('summary structural overflow refuses before endpoint admission or stream writes',()=>{
 const q=specimen();q.boot.outputs[3].cap_bytes=1;q.seal();const h=harness(q),r=h.run();assert.equal(r.summary_transfer.submitted_bytes,0);assert.equal(h.calls.before,0);assert.equal(h.writes.length,0);assert.equal(r.exit_code,1);
});
test('invalid injected adapter result cannot become launch success',()=>{
 const h=harness();h.invokeAdapter=()=>null;const r=h.run();assert.equal(r.exit_code,1);assert.match(r.observation.first_failure.error.message,/fixed adapter response/);
});
test('throwing stdout provider preserves unknown partial capture custody',()=>{
 const h=harness();h.summarySink.write=()=>{throw Error('write failed after unknown transfer');};const r=h.run();
 assert.equal(r.summary_transfer.provider_invocations,1);assert.equal(r.summary_transfer.returned_bytes,0);assert.equal(r.summary_transfer.partial_capture_may_remain,true);assert.equal(r.exit_code,1);
});
test('separate reporting elapsed origin starts only at its first check',()=>{
 let made=0,checked=0;const guard=deferLaunchGuard(()=>{made++;const g=()=>{checked++;};g.snapshot=()=>({checks:checked});g.resourceSnapshot=()=>({probes:checked});return g;});
 assert.deepEqual(guard.snapshot(),{started:false,construction_failed:false,first_failure:null});assert.equal(guard.resourceSnapshot(),null);assert.equal(made,0);
 guard();guard();assert.equal(made,1);assert.equal(checked,2);assert.deepEqual(guard.resourceSnapshot(),{probes:2});assert.deepEqual(guard.snapshot(),{checks:2});
});
test('throw-null deferred guard construction stays terminal without repeated factory calls',()=>{
 let made=0;const guard=deferLaunchGuard(()=>{made++;throw null;});for(let i=0;i<2;i++){let caught=false;try{guard();}catch(e){caught=true;assert.equal(e,null);}assert.equal(caught,true);}
 assert.equal(made,1);assert.equal(guard.snapshot().construction_failed,true);assert.equal(guard.snapshot().first_failure.message,'null');assert.equal(guard.resourceSnapshot(),null);
});
test('captured bootstrap ledger includes the final pre-dispatch check',()=>{
 const h=harness();let checks=0;const g=()=>{checks++;};g.snapshot=()=>({checks});g.resourceSnapshot=()=>({attempts:checks});h.bootstrapGuard=g;
 const r=h.run(),captured=JSON.parse(Buffer.concat(h.writes));assert.equal(r.exit_code,0);assert.equal(h.calls.adapter,1);
 assert.equal(captured.bootstrap_guard_observation.checks,checks);assert.equal(captured.bootstrap_resource_observation.attempts,checks);
});
test('final bootstrap refusal retains the last ledger and prevents adapter dispatch',()=>{
 const h=harness();let checks=0,afterBrackets=0;const g=()=>{checks++;if(h.mem.trace.filter(x=>x==='realpath').length===3&&++afterBrackets===2)throw Error('final bootstrap scope refusal');};g.snapshot=()=>({checks,denied:afterBrackets===2});g.resourceSnapshot=()=>({attempts:checks});h.bootstrapGuard=g;
 const r=h.run(),captured=JSON.parse(Buffer.concat(h.writes));assert.equal(r.exit_code,1);assert.equal(h.calls.adapter,0);assert.equal(captured.adapter_dispatch,'not-started');
 assert.equal(captured.bootstrap_guard_observation.checks,checks);assert.equal(captured.bootstrap_resource_observation.attempts,checks);assert.equal(captured.bootstrap_guard_observation.denied,true);
 assert.match(captured.first_failure.error.message,/final bootstrap scope refusal/);
});
function unselectedAliasSpecimen(){
 const q=specimen(),row=q.rows[2];row.resolved='/unit/unselected-target';q.graph.claims[2].resolved=row.resolved;
 const alias={pin:row.pin,resolved:row.resolved,link_text:'unselected-target',target_pin:{...row.pin,path:row.resolved},target_selected:false};
 q.graph.aliases=[alias];q.plan.graph_protocol.aliases=[alias];q.plan.graph_protocol.budget=deriveReadBudget(q.rows,[alias],1);
 q.plan.historical_protocol.entries=[structuredClone(row)];q.plan.historical_protocol.aliases=[alias];q.plan.historical_protocol.budget=deriveReadBudget(q.plan.historical_protocol.entries,[alias],2);
 q.boot.historical_protocol_sha256=sha(json(q.plan.historical_protocol));q.boot.external_terminal_pins[0]=structuredClone(alias.target_pin);q.seal();return q;
}
test('terminal pins must agree with an independently unselected alias target',()=>{
 const q=unselectedAliasSpecimen();assert.doesNotThrow(q.admit);q.boot.external_terminal_pins[0].sha256='0'.repeat(64);
 assert.throws(q.admit,/external terminal versus graph pin conflict/);
});
test('request name cannot overlap either a selected input or an unselected alias target',()=>{
 const q=specimen();q.boot.request.pin.path='/unit/kernel-input';assert.throws(q.admit,/request name overlaps/);
 const a=unselectedAliasSpecimen();a.boot.external_terminal_pins[0]={path:'/terminal/closure',bytes:1,sha256:sha(Buffer.from('t'))};a.boot.request.pin.path='/unit/unselected-target';
 assert.throws(a.admit,/request name overlaps/);
});
test('two unselected aliases cannot demand conflicting whole content at one named target',()=>{
 const q=unselectedAliasSpecimen(),row=structuredClone(q.rows[2]);row.path='/unit/second-alias';row.pin={...row.pin,path:row.path,sha256:'b'.repeat(64)};row.labels.duties=['role:second-alias'];
 const alias={pin:row.pin,resolved:row.resolved,link_text:'unselected-target',target_pin:{...row.pin,path:row.resolved},target_selected:false};
 q.rows.push(row);q.graph.claims.push({id:'claim:second-alias',path:row.path,kind:row.kind,pin:row.pin,resolved:row.resolved,identity:row.identity,ownership:row.ownership,roles:row.labels.duties});
 q.graph.aliases.push(alias);q.plan.graph_protocol.aliases=q.graph.aliases;q.plan.historical_protocol.aliases=q.graph.aliases;q.plan.historical_protocol.entries.push(structuredClone(row));
 q.plan.graph_protocol.budget=deriveReadBudget(q.rows,q.graph.aliases,1);q.plan.historical_protocol.budget=deriveReadBudget(q.plan.historical_protocol.entries,q.graph.aliases,2);
 q.plan.named_cap=4;q.boot.named_cap=4;q.boot.declared_input_union_cap=7;q.boot.historical_protocol_sha256=sha(json(q.plan.historical_protocol));q.seal();
 assert.throws(q.admit,/conflicting alias target content\/kind obligation/);
});

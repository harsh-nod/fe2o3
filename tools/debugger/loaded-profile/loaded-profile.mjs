// Historical loaded-file review only. No filesystem/process/native operations.
import crypto from 'node:crypto';
import {isDeepStrictEqual as eq} from 'node:util';
import {PROFILE} from './loaded-profile-binding.mjs';
import {validateObservation, parseMaps} from './loaded-observation-values.mjs';
import {validateTransport, validateStartupStderr} from './loaded-mi2-values.mjs';
const fail=m=>{throw Error('loaded-file review: '+m)};
const sha=b=>crypto.createHash('sha256').update(b).digest('hex');
const json=v=>JSON.stringify(v);
const freeze=v=>{if(v&&typeof v==='object'){for(const x of Object.values(v))freeze(x);Object.freeze(v);}return v;};
const pinOf=(p,b)=>({path:p,bytes:b.length,sha256:sha(b)});
const falseNames=['debugger_acceptance','runtime_acceptance','runtime_selection','physical_capture','gpu_dispatch','complete_import_history','cache_execution_provenance','strong_isolation'];
function keys(v,n){if(!v||typeof v!=='object'||Array.isArray(v)||!eq(Object.keys(v).sort(),n.slice().sort()))fail('closed keys')}
function uint(v,max=Number.MAX_SAFE_INTEGER){if(!Number.isSafeInteger(v)||v<0||v>max)fail('integer bound')}
function same(a,b,m){if(!eq(a,b))fail(m)}
function count(v,n,m){if(!Array.isArray(v)||v.length!==n)fail(m)}
function boundedJSON(b,cap=16*1024*1024){
 if(!Buffer.isBuffer(b)||b.length===0||b.length>cap)fail('JSON byte bound');
 const text=b.toString('utf8');if(!Buffer.from(text).equals(b))fail('JSON UTF8');
 const v=JSON.parse(text);let nodes=0;
 const visit=(x,depth)=>{
  if(++nodes>262144||depth>64)fail('JSON tree bound');
  if(typeof x==='number'&&!Number.isSafeInteger(x)&&!Number.isFinite(x))fail('JSON number');
  if(typeof x==='string'&&(x.length>8*1024*1024||x.includes('\0')))fail('JSON string bound');
  if(x&&typeof x==='object'){
   if(Array.isArray(x)){if(x.length>8192)fail('JSON array bound');for(const y of x)visit(y,depth+1);}
   else{const ks=Object.keys(x);if(ks.length>1024)fail('JSON key count');for(const k of ks){if(k.length>4096||['__proto__','prototype','constructor'].includes(k))fail('JSON key');visit(x[k],depth+1);}}
  }
 };visit(v,0);return v;
}
function plainPath(p){
 if(typeof p!=='string'||p.length>4096||!p.startsWith('/')||p.includes('\\')||p.includes('\0')||p.includes(' (deleted)')||p.split('/').slice(1).some(s=>!s||s==='.'||s==='..'))fail('path');
 return p;
}
function canonicalText(b,cap){if(!Buffer.isBuffer(b)||b.length>cap)fail('text bytes');const s=b.toString('utf8');if(!Buffer.from(s).equals(b))fail('text UTF8');return s;}
function readAccounting(value,total,n,limit){
 keys(value,['limit_bytes','reserved_bytes','actual_bytes','files']);
 same(value,{limit_bytes:limit,reserved_bytes:total+n,actual_bytes:total,files:n},'physical read accounting');
}
function snapshotFiles(rows,expectedPaths){
 count(rows,expectedPaths.length,'file roster count');const by=new Map();let total=0,present=0;
 for(let i=0;i<rows.length;i++){
  const f=rows[i];plainPath(f.path);plainPath(f.realpath);
  if(f.path!==expectedPaths[i]||by.has(f.path))fail('full ordered file roster');by.set(f.path,f);
  if(f.exists===false){keys(f,['path','realpath','exists']);continue;}
  if(f.exists!==true)fail('file existence');
  keys(f,['path','realpath','exists','bytes','sha256','dev','ino','mode','uid','gid','mtime_ns','ctime_ns','elf']);
  uint(f.bytes,256*1024*1024);total+=f.bytes;present++;
  if(total>1024*1024*1024||!/^[0-9a-f]{64}$/.test(f.sha256)||typeof f.elf!=='boolean')fail('file pin');
  for(const n of ['dev','ino','mtime_ns','ctime_ns'])if(typeof f[n]!=='string'||!/^(0|[1-9][0-9]{0,24})$/.test(f[n]))fail('file identity');
  for(const n of ['mode','uid','gid'])uint(f[n]);
  if((f.mode&0o170000)!==0o100000)fail('file not regular');
 }
 return{by,total,present};
}
function sourcePinJoins(v){
 same(v.sourceBefore,v.sourceAfter,'pre/post source drift');
 same(v.sourceBefore.build,v.buildPins,'complete build/source roster');
 same(v.sourceBefore.debugger,PROFILE.artifact.debugger,'actual debugger pin');
 count(v.buildPins.files,45,'build pin roles');const by=new Map();
 for(const p of v.buildPins.files){if(by.has(p.path))fail('duplicate source pin');by.set(p.path,p);}
 for(const p of PROFILE.sourcePins)same(by.get(p.path),p,'actual source input not in build roster');
 same(v.artifactsBefore,v.artifactsAfter,'pre/post artifact drift');
 const a=v.artifactsBefore;
 const artifactBytes=a.snapshots.reduce((n,r)=>n+r.pin.bytes,0)+a.data.files.reduce((n,r)=>n+r.bytes,0)+a.data.sources.reduce((n,r)=>n+r.pin.bytes,0);
 readAccounting(a.read_accounting,artifactBytes,a.snapshots.length+a.data.files.length+a.data.sources.length,512*1024*1024);
 count(a.data.files,78,'generated data roster');count(a.data.sources,54,'source Python roster');count(a.data.directories,9,'data directory roster');
 same(a.data.files.slice().sort((x,y)=>x.path<y.path?-1:x.path>y.path?1:0),PROFILE.artifact.data_files.slice().sort((x,y)=>x.path<y.path?-1:x.path>y.path?1:0),'all generated data pins');
 const py=PROFILE.artifact.data_files.filter(p=>p.path.startsWith('python/'));
 for(let i=0;i<py.length;i++){
  const p=py[i],source=a.data.sources[i];
  same(source.pin,{path:PROFILE.build+'/source/gdb/python/lib/gdb/'+p.path.slice('python/gdb/'.length),bytes:p.bytes,sha256:p.sha256},'source/generated Python equality');
 }
 if(a.debugger_qualified!==false||a.historical_whole_family_cleanup_proved!==false||a.historical_builder_pid!==null)fail('artifact authority promotion');
}
function gateJoins(v){
 const g=v.gate,r=v.gateRequest;
 if(g.schema!=='task-phase28-command-observation-v1'||g.status!=='command-passed'||g.authority!=='none'||g.production_qualified!==false||g.hardware_observed!==false||g.source_authenticated!==false)fail('outer gate domain');
 same(g.configuration,r,'complete gate request');same(g.request,PROFILE.pins.gateRequest,'whole gate request pin');
 same(r.command,v.coordination.command,'exact coordinated command');keys(r.environment,['FE2O3_ROOT_STARTUP_COORDINATION_SHA256']);
 if(r.environment.FE2O3_ROOT_STARTUP_COORDINATION_SHA256!==PROFILE.pins.coordination.sha256)fail('coordination digest');
 const issued=Date.parse(v.coordination.issued_at_utc),started=Date.parse(g.started),finished=Date.parse(g.finished),end=Date.parse(v.coordination.end_utc);
 if(![issued,started,finished,end].every(Number.isFinite)||issued>started||started>finished||finished>end)fail('historical coordination window');
 same(g.source_before,g.source_after,'source identity drift');same(g.source_before,r.source,'request/source identity');same(g.source_before,v.readback.source,'root source readback');
 same(g.tools_before,g.tools_after,'tool identity drift');count(g.tools_before,16,'complete tools');
 count(r.inputs,1024,'original 1024 duties');count(g.inputs_before,1024,'observed 1024 before');count(g.inputs_after,1024,'observed 1024 after');
 same(g.inputs_before,g.inputs_after,'all selected identities stable');const seen=new Set();
 for(let i=0;i<r.inputs.length;i++){const p=r.inputs[i],f=g.inputs_before[i];if(seen.has(p.path))fail('duplicate original duty');seen.add(p.path);same({path:f.path,bytes:f.bytes,sha256:f.sha256},p,'exact original selected pin');}
 if(g.errors.length!==0||g.command.code!==0||g.command.signal!==null||g.command.reason!==null||!eq(g.command.direct_exit,{code:0,signal:null})||!eq(g.command.close_event,{code:0,signal:null})||g.command.drain_abandoned!==false||g.command.signals.length!==0)fail('outer command completion');
 const streams=g.command.streams;
 for(const [key,raw]of [['stdout',v.gateStdout],['stderr',v.gateStderr]]){
  const b=Buffer.from(raw);const s=streams[key];
  if(s.bytes!==b.length||s.sha256!==sha(b))fail('outer whole stream');
 }
 if(v.gateStderr!=='')fail('outer stderr');
 const lines=v.gateStdout.split('\n');if(lines.pop()!==''||lines.length!==2)fail('outer output EOF/count');
 same(JSON.parse(lines[0]),v.outer,'outer stdout value');
 same(JSON.parse(lines[1]),{schema:'fe2o3-one-stop-mi2-startup-invocation-complete-v1',action:'custom-debugger-startup',historical_whole_family_cleanup_proved:false,runtime_acceptance:false,physical_capture:false},'invocation completion');
 same(v.readback.gate,PROFILE.pins.gate,'complete outer gate pin');
 same(v.readback.records,PROFILE.innerRecordPins,'complete retained inner record roster');
 for(const p of PROFILE.sourcePins)same(r.inputs.find(x=>x.path===p.path),p,'reviewed source selected by gate');
 same(g.invocation,{executable:PROFILE.node,argv:[PROFILE.runner,r.label,PROFILE.pins.gateRequest.sha256],cwd:'/home/harmenon',outer_wrapper_observed:false},'outer invocation identity');
 if(g.observation_limits.descendant_quiescence_proved!==false||g.observation_limits.transitive_build_attestation!==false||g.observation_limits.failure_post_observations_guaranteed!==false)fail('outer gate authority promotion');
}
function ownershipJoins(v){
 const {outer:o,ready:r,startup:s,family:f,readback:b,coordination:c,marker:m}=v;
 if(o.schema!=='fe2o3-noqueue-family-outer-v1'||o.mode!=='custom-debugger-startup'||o.error!==null)fail('outer startup mode');
 for(const n of ['exact_service_cgroup_empty','inner_cleanup_complete','startup_owner_joined','release_written','accepted','manager_cleanup_not_child_reaping','startup_observation_complete'])if(o[n]!==true)fail('outer completed owner');
 for(const n of ['outer_cleanup_deadline_expired','pre_release_service_cleanup_observed','strong_isolation','debugger_acceptance_claimed','physical_capture'])if(o[n]!==false)fail('outer forbidden claim');
 if(!/^[0-9a-f]{32}$/.test(o.nonce)||o.nonce!==b.nonce||m.nonce!==o.nonce||f.nonce!==o.nonce||o.dir!==b.dir||o.unit!==b.unit)fail('actual attempt identity');
 for(const n of ['pid','start_ticks','invocation_id','control_group','request_sha256'])same(r[n],s[n],'ready/startup owner');
 if(r.request_sha256!==sha(Buffer.from(v.requestText))||o.request_sha256!==r.request_sha256||f.request_sha256!==r.request_sha256||r.invocation_id!==f.invocation_id)fail('request owner digest');
 same(v.releaseText,r.invocation_id+'\n'+r.control_group+'\n'+r.request_sha256+'\n','release owner');
 const expectedRequest=['fe2o3-one-stop-mi2-startup-family-request-v1','custom-debugger-startup',o.nonce,PROFILE.artifact.debugger.sha256,PROFILE.collector.sha256,'no-inferior-no-attach-no-dispatch',''].join('\n');
 same(v.requestText,expectedRequest,'fixed no-inferior request');
 if(!r.control_group.endsWith('/'+o.unit)||r.workload_started!==false||s.workload_started!==false||s.environment_admitted!==false)fail('startup rendezvous');
 for(const manager of [v.managerStart,v.managerJoined,v.managerBefore]){
  if(manager.Id!==o.unit||manager.InvocationID!==r.invocation_id||manager.MainPID!==String(r.pid)||manager.ControlGroup!==r.control_group||manager.Delegate!=='yes'||manager.DelegateSubgroup!=='supervisor'||manager.MemoryMax!=='2147483648'||manager.TasksMax!=='256'||manager.KillMode!=='control-group'||manager.SendSIGKILL!=='yes'||manager.RuntimeMaxUSec!=='2min'||manager.TimeoutStopUSec!=='10s')fail('supervisor manager join');
  const prefix='{ path='+PROFILE.tools+'/supervisor ; argv[]='+PROFILE.tools+'/supervisor custom-debugger-startup '+o.nonce+' '+r.request_sha256+' ; ignore_errors=no ; ';
  if(!manager.ExecStart.startsWith(prefix)||(manager.ExecStart.match(/\{/g)||[]).length!==1)fail('manager exact executable');
 }
 if(v.managerAfter.Id!==o.unit||v.managerAfter.LoadState!=='not-found'||v.managerAfter.ActiveState!=='inactive'||v.managerAfter.SubState!=='dead'||v.managerAfter.MainPID!=='0'||v.managerAfter.ControlGroup!=='')fail('manager cleanup observation');
 for(const name of ['startup_only','no_inferior_or_attach_or_dispatch','owner_release_cleanup_join','current_cgroup_absent','manager_cleanup_not_child_reaping','limits_unchanged','root_ssh_quiet_retry'])if(b[name]!==true)fail('root readback limitations');
 for(const name of ['physical_capture','runtime_acceptance','global_writer_exclusion','historical_cleanup'])if(b[name]!==false)fail('root readback promotion');
 if(c.physical_capture!==false||c.global_writer_exclusion!==false||c.historical_cleanup!==false||c.prior_receipts_grant_authority!==false||c.all_1024_existing_roles_preserved!==true)fail('coordination authority');
 same(b.first_failure_preserved,PROFILE.pins.failure,'preserved failed attempt');same(c.failed_attempt_readback,PROFILE.pins.failure,'coordination predecessor');
 if(v.failure.full_gate_passed!==false||v.failure.post_census_failed!==true||v.failure.loaded_summary_retained_but_not_accepted!==true||v.failure.physical_capture!==false||v.failure.gate_after_census_absent!==true)fail('failed attempt promoted');
 if(v.failure.post_census_reason!=='stopped-wave current census: fixed privileged helper refused Bounds(InspectedByteCap)')fail('failure cause changed');
 same(m.outer_sha256,PROFILE.pins.outer.sha256,'marker outer');same(m.summary_sha256,PROFILE.pins.summary.sha256,'marker summary');same(m.transport_sha256,PROFILE.pins.transport.sha256,'marker transport');same(m.build_pins_sha256,PROFILE.pins.buildPins.sha256,'marker complete build bytes');
 for(const n of ['startup_only','no_inferior_command','closure_review_required'])if(m[n]!==true)fail('marker scope');
 for(const n of ['debugger_acceptance','runtime_acceptance','runtime_selection','physical_capture','complete_import_history'])if(m[n]!==false)fail('marker promotion');
}
function analyse(v,qualified){
 keys(v,PROFILE.valueKeys);
 sourcePinJoins(v);gateJoins(v);ownershipJoins(v);
 const observation=v.observation,artifacts=PROFILE.artifact;
 const summary=validateObservation(observation,v.family,v.ready,artifacts);
 const transport=validateTransport(Buffer.from(v.controllerStdout),Buffer.from(v.collectorFrame),v.family.mi2_startup);
 validateStartupStderr(Buffer.from(v.controllerStderr));
 same(transport,v.transport,'full MI2 transport');same(transport,v.summary.mi2_transport,'summary MI2 transport');
 const prefix='FE2O3_CUSTOM_STARTUP_V1 ';
 if(!v.collectorFrame.startsWith(prefix)||!v.collectorFrame.endsWith('\n')||v.collectorFrame.indexOf('\n')!==v.collectorFrame.length-1)fail('frame');
 same(JSON.parse(v.collectorFrame.slice(prefix.length,-1)),observation,'complete collector/observation equality');
 const loaded=snapshotFiles(observation.files,PROFILE.observedPaths);
 same(v.post.files,observation.files,'all post observed identities');same(v.post.bytes_hashed,loaded.total,'post bytes');
 if(v.post.schema!=='fe2o3-debugger-post-observed-files-v1'||v.post.not_prelaunch_admission!==true)fail('post is not prelaunch');
 readAccounting(v.post.read_accounting,loaded.total,loaded.present,1024*1024*1024);
 same(v.candidatesBefore,v.candidatesAfter,'static candidate and cache drift');
 const stat=snapshotFiles(v.candidatesBefore.files,PROFILE.candidatePaths);
 if(v.candidatesBefore.schema!=='fe2o3-debugger-static-candidate-snapshot-v1'||v.candidatesBefore.static_not_loaded!==true)fail('static not loaded');
 for(const p of artifacts.static_candidates.files){const f=stat.by.get(p.path);if(!f?.exists||f.realpath!==p.realpath||f.bytes!==p.bytes||f.sha256!==p.sha256)fail('source-bound static candidate');}
 same(v.candidatesBefore.bytes_hashed,stat.total,'static bytes');readAccounting(v.candidatesBefore.read_accounting,stat.total,stat.present,1024*1024*1024);
 const known=new Set(v.candidatesBefore.files.filter(f=>f.exists).map(f=>f.realpath));
 const observed=new Set(observation.files.filter(f=>f.exists).map(f=>f.realpath));
 summary.mi2_transport=transport;
 summary.newly_observed_files=observation.files.filter(f=>f.exists&&!known.has(f.realpath)).map(f=>f.path);
 summary.static_candidates_not_observed=v.candidatesBefore.files.filter(f=>f.exists&&!observed.has(f.realpath)).map(f=>f.path);
 summary.mi2_startup_not_runtime_acceptance=true;summary.system_preload_absent_before_and_after=true;
 same(summary,v.summary,'complete derived summary');
 const refs=new Map(observation.files.map(f=>[f.path,[]]));
 const maps={};
 for(const phase of ['initial','collection','final']){
  const rows=parseMaps(observation[phase+'_maps']);maps[phase]={text_sha256:sha(Buffer.from(observation[phase+'_maps'])),rows};
  rows.forEach((r,index)=>{if(r.path?.startsWith('/'))refs.get(r.path).push({kind:'map',phase,index,range:r.range,permissions:r.permissions,offset:r.offset,dev:r.dev,ino:r.ino});});
 }
 const moduleRows=[];
 for(const [phase,rows]of [['initial',observation.initial_modules],['collection',observation.collection_modules]]){
  for(const row of rows){
   moduleRows.push({phase,...row});
   if(row.kind!=='module')continue;
   for(const field of ['file','origin','cached'])if(row[field]?.startsWith('/'))refs.get(row[field]).push({kind:'module',phase,name:row.name,field});
   if(row.cached!==null){
    if(!row.file?.endsWith('.py'))fail('cache without source lineage');
    const slash=row.file.lastIndexOf('/'),base=row.file.slice(slash+1,-3),prefix=row.file.slice(0,slash)+'/__pycache__/'+base+'.cpython-312';
    if(![row.file+'c',prefix+'.pyc',prefix+'.opt-1.pyc',prefix+'.opt-2.pyc'].includes(row.cached))fail('cache lineage');
   }
   if(row.package_paths!==null&&row.file!==null&&(!row.file.endsWith('/__init__.py')||!eq(row.package_paths,[row.file.slice(0,row.file.lastIndexOf('/'))])))fail('package metadata lineage');
  }
 }
 const aliases=observation.files.filter(f=>f.path!==f.realpath);
 same(aliases,PROFILE.aliasRows,'closed actual named alias identities');
 const groups=new Map(),rows=[];
 for(const f of observation.files){
  const references=refs.get(f.path);if(!references.length)fail('silent file omission');
  if(!f.exists&&references.some(r=>r.kind!=='module'||r.field!=='cached'))fail('absent non-cache dependency');
  if(groups.has(f.realpath)){
   const prior=groups.get(f.realpath);const strip=x=>Object.fromEntries(Object.entries(x).filter(([k])=>k!=='path'));
   same(strip(prior),strip(f),'canonical alias file identity');
  }else groups.set(f.realpath,f);
  rows.push({observation:f,references,classification:f.exists?(f.elf?'mapped-or-referenced-ELF':f.path.endsWith('.pyc')?'cache-path-presence-not-execution':f.path.endsWith('.py')?'module-source-metadata-not-execution':'referenced-data'):'absent-cache-path',
   appeared_in_initial_snapshot:references.some(r=>r.phase==='initial'),collector_phase_only:references.every(r=>r.phase!=='initial'),reviewed_static_canonical_candidate:known.has(f.realpath),cache_execution_provenance:false});
 }
 const counts={files:rows.length,present:loaded.present,absent:rows.length-loaded.present,bytes:loaded.total,initial_modules:observation.initial_modules.length,mapped_elf_paths:summary.mapped_elf_paths.length,newly_observed_files:summary.newly_observed_files.length,static_candidates_not_observed:summary.static_candidates_not_observed.length};
 same(counts,PROFILE.counts,'complete observed census');same({...counts,full_payloads_and_file_identity_revalidated:true,closure_review_required:true,complete_import_history:false,cache_execution_provenance:false},v.readback.loaded,'root full loaded readback');
 for(let i=0;i<2;i++){
  const c=[v.censusBefore,v.censusAfter][i],r=v.readback.censuses[i];
  if(c.schema!=='fe2o3-one-stop-mi2-current-census-v1'||c.uid!==9661||c.sampled_only!==true||c.root_exclusive_writer_lease_required!==true||c.historical_builder_pid!==null)fail('current census domain');
  for(const n of ['historical_disappearance_proved','whole_family_cleanup_proved','writer_exclusion_proved'])if(c[n]!==false)fail('census promotion');
  count(c.rows,9,'census rows');const pids=new Set();
  for(const row of c.rows){keys(row,['pid','uid','start','end','complete','references']);if(pids.has(row.pid)||row.uid!==9661||row.start!==row.end||row.complete!==true||row.references!==0)fail('census stable no-reference rows');pids.add(row.pid);}
  if(c.inspected_bytes!==r.bytes||c.inspected_fd_links!==r.fds||c.elapsed_ms!==r.elapsed_ms||r.attempts!==1||r.rows!==9||r.bytes>64*1024*1024)fail('current census readback');
  const p=c.privileged_reader;
  if(p.schema!=='fe2o3-one-stop-mi2-privileged-proc-reader-v2'||p.caller_pid!==c.self_pid||p.caller_start_ticks!==v.gate.command.leader_start_ticks||p.attempts!==1||p.verified_sudo_intermediaries!==1||p.contents_disclosed!==false||p.counters_include_discarded_attempts!==true||p.whole_helper_family_cleanup_proved!==false)fail('privileged reader limitations');
  count(p.fixed_files,3,'census fixed tools');for(const f of p.fixed_files)same(v.buildPins.files.find(x=>x.path===f.path),f,'census tool/build pin');
 }
 return freeze({schema:'fe2o3-historical-startup-loaded-profile-v1',qualified_retained_input_bytes:qualified,semantic_review_complete:true,physical_files_reread:false,source_authority:'none',startup_only:true,live_activation:false,original_selected_roles:1024,historical_duties_changed:false,counts,rows,modules:moduleRows,maps,collector_added_modules:summary.collector_added_modules,static_candidates_not_observed:summary.static_candidates_not_observed,non_file_module_declarations:moduleRows.filter(r=>r.kind!=='module'||r.file===null),named_aliases:aliases,failed_attempt:v.failure,passed_root_readback:v.readback,limitations:{complete_import_history:false,cache_execution_provenance:false,runtime_acceptance:false,runtime_selection:false,physical_capture:false,gpu_dispatch:false,debugger_acceptance:false,global_writer_exclusion:false,historical_cleanup:false,loaded_profile_not_execution_permission:true}});
}
function values(raw){
 const v={};for(const [name,role]of Object.entries(PROFILE.valueRoles)){const b=raw.get(role);v[name]=PROFILE.textRoles.includes(name)?canonicalText(b,20*1024*1024):boundedJSON(b);}
 return v;
}
/** Exact reviewed Buffers only; caller owns external reading/custody. No path is opened here. */
export function reviewLoadedStartup(records){
 count(records,PROFILE.records.length,'complete input count');let total=0;const raw=new Map();
 for(let i=0;i<records.length;i++){
  const r=records[i],p=PROFILE.records[i];keys(r,['role','bytes']);
  if(r.role!==p.role||!Buffer.isBuffer(r.bytes)||r.bytes.length!==p.bytes||r.bytes.length>8*1024*1024)fail('complete input role/size');
  total+=r.bytes.length;if(total>32*1024*1024)fail('input aggregate');
  const owned=Buffer.from(r.bytes); // Private complete snapshot: no caller/shared-buffer alias survives admission.
  if(sha(owned)!==p.sha256)fail('whole input pin');raw.set(r.role,owned);
 }
 const v=values(raw);return analyse(v,true);
}
/** Synthetic/pure controls only: this can never return qualified input-byte status. */
export function reviewUnqualifiedStartup(bytes){return analyse(boundedJSON(bytes),false);}

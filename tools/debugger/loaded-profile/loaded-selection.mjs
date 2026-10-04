// Pure historical selection proposal. No filesystem, process, clock, or native calls.
import {createHash} from 'node:crypto';
import {isDeepStrictEqual as eq} from 'node:util';
import {reviewLoadedStartup} from './loaded-profile.mjs';
import {PROFILE} from './loaded-profile-binding.mjs';
import {BINDING} from './loaded-selection-binding.mjs';
const fail=m=>{throw Error('loaded selection: '+m);};
const sha=b=>createHash('sha256').update(b).digest('hex');
const freeze=x=>{if(x&&typeof x==='object'){for(const y of Object.values(x))freeze(y);Object.freeze(x);}return x;};
const same=(a,b,m)=>{if(!eq(a,b))fail(m);};
function keys(x,ks){if(!x||typeof x!=='object'||Array.isArray(x)||!eq(Object.keys(x).sort(),ks.slice().sort()))fail('closed keys');}
function count(xs,n,label){if(!Array.isArray(xs)||xs.length!==n)fail(label);}
function integer(n,max=Number.MAX_SAFE_INTEGER){if(!Number.isSafeInteger(n)||n<0||n>max)fail('checked integer');return n;}
function add(a,b){return integer(integer(a)+integer(b));}
function path(p){if(typeof p!=='string'||p.length>4096||!p.startsWith('/')||p.includes('\\')||p.includes('\0')||p.includes(' (deleted)')||p.split('/').slice(1).some(x=>!x||x==='.'||x==='..'))fail('named path');return p;}
function pin(p){keys(p,['path','bytes','sha256']);path(p.path);integer(p.bytes,512*1024*1024);if(!/^[a-f0-9]{64}$/.test(p.sha256))fail('digest');return p;}
function boundedJSON(bytes,limit=32*1024*1024){
 if(!Buffer.isBuffer(bytes)||!bytes.length||bytes.length>limit)fail('JSON bytes');
 const text=bytes.toString('utf8');if(!Buffer.from(text).equals(bytes))fail('UTF8');
 const value=JSON.parse(text);let nodes=0;
 function visit(x,d){
  if(++nodes>524288||d>64)fail('JSON tree bound');
  if(typeof x==='number'&&!Number.isFinite(x))fail('JSON number');
  if(typeof x==='string'&&(x.length>8*1024*1024||x.includes('\0')))fail('JSON string bound');
  if(x&&typeof x==='object'){
   if(Array.isArray(x)){if(x.length>8192)fail('JSON array bound');}
   else if(Object.keys(x).length>1024)fail('JSON object bound');
   for(const [k,y]of Object.entries(x)){if(k.length>4096||['__proto__','prototype','constructor'].includes(k))fail('JSON key');visit(y,d+1);}
  }
 }
 visit(value,0);return value;
}
function physicalIdentity(row){
 path(row.path);path(row.resolved);integer(row.bytes,512*1024*1024);
 if(!/^[a-f0-9]{64}$/.test(row.sha256))fail('observed digest');
 count(row.identity,6,'full prior identity');
 for(const x of row.identity)if(typeof x!=='string'||!/^(0|[1-9][0-9]{0,24})$/.test(x))fail('prior identity scalar');
 if(row.identity[2]!==String(row.bytes))fail('prior identity size');
 integer(row.mode,0o7777);
 const fullMode=Number(row.identity[3]);integer(fullMode);
 if((fullMode&0o170000)!==0o100000||(fullMode&0o7777)!==row.mode)fail('prior regular mode');
 return row.identity;
}
function loadedIdentity(f){
 path(f.path);path(f.realpath);
 if(f.exists===false){keys(f,['path','realpath','exists']);if(f.path!==f.realpath)fail('absent alias');return null;}
 if(f.exists!==true)fail('loaded existence');
 keys(f,['path','realpath','exists','bytes','sha256','dev','ino','mode','uid','gid','mtime_ns','ctime_ns','elf']);
 integer(f.bytes,256*1024*1024);integer(f.mode);integer(f.uid);integer(f.gid);
 if(!/^[a-f0-9]{64}$/.test(f.sha256)||(f.mode&0o170000)!==0o100000||typeof f.elf!=='boolean')fail('loaded regular pin');
 for(const k of ['dev','ino','mtime_ns','ctime_ns'])if(typeof f[k]!=='string'||!/^(0|[1-9][0-9]{0,24})$/.test(f[k]))fail('loaded identity scalar');
 return [f.dev,f.ino,String(f.bytes),String(f.mode),f.mtime_ns,f.ctime_ns];
}
function concretePaths(data,prior){
 same(data.benign_modes,BINDING.benign_modes,'closed benign modes');
 const targets=data.duties.flatMap(r=>r.targets).concat(data.extras.map(r=>r.target)),nonces={};
 for(const mode of data.benign_modes){
  const marker='@'+mode+'@',template=targets.find(t=>t.path.includes(marker));if(!template)fail('missing nonce role');
  const pieces=template.path.split(marker);count(pieces,2,'single nonce marker');
  const [pre,post]=pieces,found=prior.filter(p=>p.path.startsWith(pre)&&p.path.endsWith(post)).map(p=>p.path.slice(pre.length,p.path.length-post.length));
  count(found,1,'unique original nonce path');const n=found[0];
  if(!/^[a-f0-9]{32}$/.test(n)||data.historical_nonces.includes(n)||Object.values(nonces).includes(n))fail('historical or duplicate nonce');
  nonces[mode]=n;
 }
 same(nonces,BINDING.benign_nonces,'recorded benign nonces');
 return {nonces,resolve:p=>{
  const out=p.replace(/@([^@]+)@/g,(_,mode)=>{if(!Object.hasOwn(nonces,mode))fail('unknown nonce role');return nonces[mode];});
  if(out.includes('@'))fail('unresolved nonce');return path(out);
 }};
}
function analyse(v,qualified){
 keys(v,['selector','prior','before','after','profile']);
 const {selector:data,prior,before,after,profile}=v;
 count(prior,1024,'all original selected inputs');count(before,1024,'all original before identities');count(after,1024,'all original after identities');
 same(before,after,'original physical identity drift');
 if(profile.schema!=='fe2o3-historical-startup-loaded-profile-v1'||profile.semantic_review_complete!==true||profile.startup_only!==true||profile.live_activation!==false||profile.historical_duties_changed!==false||profile.physical_files_reread!==false||profile.source_authority!=='none')fail('review profile domain');
 for(const k of ['complete_import_history','cache_execution_provenance','runtime_acceptance','runtime_selection','physical_capture','gpu_dispatch','debugger_acceptance','global_writer_exclusion','historical_cleanup'])if(profile.limitations[k]!==false)fail('profile authority promotion');
 if(qualified&&profile.qualified_retained_input_bytes!==true)fail('qualified profile bytes required');
 count(profile.rows,193,'all loaded named roles');same(profile.counts,BINDING.profile_counts,'review profile census');
 const entries=new Map(),priorRoles=[];
 for(let i=0;i<prior.length;i++){
  const p=pin(prior[i]),o=before[i];physicalIdentity(o);
  same({path:o.path,bytes:o.bytes,sha256:o.sha256},p,'prior observed pin order');
  if(entries.has(p.path))fail('duplicate prior named path');
  const role={kind:'prior-selected-input',index:i,pin:p,observation:o};priorRoles.push(role);
  entries.set(p.path,{path:p.path,kind:'readable',pin:p,resolved:o.resolved,identity:o.identity,prior_input_roles:[i],loaded_named_roles:[],duty_targets:[],extra_roles:[]});
 }
 count(data.original,1020,'original duty inventory');count(data.transfers,141,'original approved source transfers');count(data.duties,879,'all retained duties');count(data.extras,453,'all original extra roles');
 for(let i=0;i<data.original.length;i++)if(data.original[i].index!==i)fail('original inventory order');
 const removed=new Set();for(const row of data.transfers){integer(row.original_index,1019);if(removed.has(row.original_index))fail('duplicate transfer');removed.add(row.original_index);same(row.old_pin,data.original[row.original_index].old_pin,'transfer original identity');}
 const {nonces,resolve}=concretePaths(data,prior),covered=new Set(),seen=new Set(),duties=[],extras=[];
 function target(t,label){
  keys(t,['path','pin','cap']);integer(t.cap,512*1024*1024);
  const p=resolve(t.path),entry=entries.get(p);if(!entry)fail('prior duty target omitted');
  if(entry.pin.bytes>t.cap)fail('inherited target cap');
  if(t.pin!==null)same(pin(t.pin),entry.pin,'source-owned target pin');
  covered.add(p);return {path:p,pin:entry.pin,cap:t.cap,label};
 }
 for(const row of data.duties){
  integer(row.original_index,1019);if(removed.has(row.original_index)||seen.has(row.original_index))fail('transferred or duplicate retained duty');seen.add(row.original_index);
  same(row.old_pin,data.original[row.original_index].old_pin,'retained original pin');
  if(!Array.isArray(row.targets)||!row.targets.length||row.targets.length>4)fail('retained target count');
  const targets=row.targets.map((t,j)=>{const x=target(t,'original:'+row.original_index+':target:'+j);entries.get(x.path).duty_targets.push({original_index:row.original_index,target_index:j,cap:x.cap});return x;});
  if([770,771,772,773].includes(row.original_index)&&(targets.length!==2||!targets.some(t=>eq(t.pin,row.old_pin))))fail('mixed historical configuration duty lost');
  duties.push({...row,targets});
 }
 if(seen.size+removed.size!==1020)fail('complete original partition');
 for(let i=0;i<data.extras.length;i++){
  const row=data.extras[i],t=target(row.target,'extra:'+i);entries.get(t.path).extra_roles.push({index:i,role:row.role,cap:t.cap});extras.push({index:i,role:row.role,target:t});
 }
 if(covered.size!==1021||data.expected_paths!==1021)fail('unchanged selector path census');
 const own=prior.filter(p=>!covered.has(p.path));same(own,BINDING.materializer_own_pins,'three original materializer roles');
 count(data.runtime_slots,45,'all original runtime slots');
 const slots=data.runtime_slots.map((r,i)=>{if(r.slot!==i)fail('runtime slot order');const p=resolve(r.path);if(!entries.has(p))fail('runtime slot omitted');return{slot:i,pin:entries.get(p).pin};});
 let present=0,absent=0,overlap=0,newPresent=0,loadedBytes=0;const loadedRoles=[],loadedSeen=new Set();
 for(let i=0;i<profile.rows.length;i++){
  const row=profile.rows[i],f=row.observation,id=loadedIdentity(f);
  if(loadedSeen.has(f.path))fail('duplicate loaded named role');loadedSeen.add(f.path);
  if(!Array.isArray(row.references)||!row.references.length||row.cache_execution_provenance!==false)fail('loaded references/provenance');
  if(f.exists){present++;loadedBytes=add(loadedBytes,f.bytes);}
  else{absent++;if(row.references.some(r=>r.kind!=='module'||r.field!=='cached'))fail('absence is not a cache metadata observation');}
  const priorEntry=entries.get(f.path);
  if(priorEntry){
   overlap++;if(!f.exists)fail('same-path readable/absent conflict');
   same(priorEntry.pin,{path:f.path,bytes:f.bytes,sha256:f.sha256},'same-path content conflict');
   same(priorEntry.resolved,f.realpath,'same-path alias conflict');same(priorEntry.identity,id,'same-path full identity conflict');
  }else{
   if(f.exists)newPresent++;
   entries.set(f.path,{path:f.path,kind:f.exists?'readable':'absence-observation',pin:f.exists?{path:f.path,bytes:f.bytes,sha256:f.sha256}:null,resolved:f.realpath,identity:id,prior_input_roles:[],loaded_named_roles:[],duty_targets:[],extra_roles:[]});
  }
  entries.get(f.path).loaded_named_roles.push(i);loadedRoles.push({index:i,...row});
 }
 same({present,absent,loadedBytes,overlap,newPresent},{present:170,absent:23,loadedBytes:398626885,overlap:44,newPresent:126},'complete role intersection census');
 const rows=[...entries.values()],readable=rows.filter(r=>r.kind==='readable'),missing=rows.filter(r=>r.kind==='absence-observation');
 const aliases=rows.filter(r=>r.path!==r.resolved).map(r=>({path:r.path,resolved:r.resolved,pin:r.pin,identity:r.identity,prior_input_roles:r.prior_input_roles,loaded_named_roles:r.loaded_named_roles}));
 same(aliases.map(r=>({path:r.path,resolved:r.resolved,pin:r.pin})),BINDING.aliases,'exact existing named alias roster');
 // Aliases are not canonicalized or coalesced. An independently named target is independently paid.
 const budget=xs=>{
  let payload=0,calls=0;for(const r of xs){payload=add(payload,r.pin.bytes);calls=add(calls,Math.ceil(r.pin.bytes/65536)+1);}
  return{readable_paths:xs.length,payload_bytes:payload,eof_reserved_bytes:xs.length,reserved_bytes:add(payload,xs.length),read_call_cap:calls};
 };
 const priorBudget=budget(readable.filter(r=>r.prior_input_roles.length)),newBudget=budget(readable.filter(r=>!r.prior_input_roles.length)),totalBudget=budget(readable);
 same(totalBudget,BINDING.union_budget,'exact union arithmetic');
 same({paths:rows.length,readable:readable.length,absent:missing.length,prior:priorRoles.length,loaded:loadedRoles.length,overlap},{paths:1173,readable:1150,absent:23,prior:1024,loaded:193,overlap:44},'exact prospective union');
 const twice=Object.fromEntries(Object.entries(totalBudget).map(([k,n])=>[k,add(n,n)]));
 return freeze({schema:'fe2o3-inactive-loaded-input-selection-plan-v1',qualified_retained_input_bytes:qualified,original_selector_unchanged:true,source_authority:'none',execution_authority:false,physical_observation_performed:false,successor_graph_active:false,runtime_selection:false,native_acceptance:false,lease:null,original_duties:1020,preserved_duties:879,historical_source_transfers:data.transfers,same_role_source_changes:0,prior_input_roles:priorRoles,duties,extra_roles:extras,original_materializer_roles:own,runtime_slots:slots,benign_nonces:nonces,loaded_named_roles:loadedRoles,entries:rows,named_alias_roles:aliases,counts:{prior_input_identities:1024,loaded_named_roles:193,independent_prior_plus_loaded_labels:1217,same_path_shared_reads:44,readable_paths:1150,absence_observations:23,distinct_named_paths:1173,old_named_path_cap:1024,excess_named_paths:149,excess_readable_paths:126},read_plan:{domain:'prospective data-plane only; no read performed',chunk_bytes:65536,policy:'one exact-length read per planned chunk or refuse; one EOF-byte read; no retry',prior:priorBudget,new_present:newBudget,one_pass:totalBudget,two_pass:twice,absence_checks_per_pass:23,absence_content_reads:0,metadata_syscalls_metered:false,actual_module_loader_io_metered:false,inner_512MiB_artifact_cap_unchanged:true,final_integration_closure_complete:false,final_operational_named_count:null,final_operational_budget:null,unresolved_additional_roles:['new planner/profile source and import closure','integration reader/driver/policy and qualification evidence','actual absence and alias metadata reader custody']},profile_limits:profile.limitations,failed_attempt:profile.failed_attempt,gate_change_required:true,root_cap_change_approved:false,no_serialization_exemption:true});
}
/** Exact complete retained inputs only; this function performs no physical file reads. */
export function planLoadedSelection(input){
 keys(input,['profile_records','selector_records']);count(input.profile_records,PROFILE.records.length,'all profile inputs');count(input.selector_records,BINDING.selector_records.length,'all selector inputs');
 let total=0;
 const snapshot=(rows,pins)=>rows.map((r,i)=>{
  keys(r,['role','bytes']);const p=pins[i];if(r.role!==p.role||!Buffer.isBuffer(r.bytes)||r.bytes.length!==p.bytes||r.bytes.length>8*1024*1024)fail('complete Buffer role/size');
  total=add(total,r.bytes.length);if(total>32*1024*1024)fail('complete Buffer aggregate');
  const owned=Buffer.from(r.bytes);if(sha(owned)!==p.sha256)fail('whole input pin');return{role:r.role,bytes:owned};
 });
 const profiles=snapshot(input.profile_records,PROFILE.records),selectors=snapshot(input.selector_records,BINDING.selector_records);
 const profile=reviewLoadedStartup(profiles),by=new Map(selectors.map(r=>[r.role,r.bytes])),text=by.get('selector-data').toString('utf8');
 if(!text.startsWith(BINDING.data_prefix)||!text.endsWith(BINDING.data_suffix))fail('exact inert DATA envelope');
 const data=boundedJSON(Buffer.from(text.slice(BINDING.data_prefix.length,-BINDING.data_suffix.length)));
 const manifest=boundedJSON(by.get('selector-manifest'));same(manifest.files,BINDING.selector_records.filter(r=>r.role!=='selector-manifest').map(r=>({path:r.path,bytes:r.bytes,sha256:r.sha256})),'unchanged selector manifest files');
 const raw=new Map(profiles.map(r=>[r.role,r.bytes]));
 const request=boundedJSON(raw.get(PROFILE.valueRoles.gateRequest)),gate=boundedJSON(raw.get(PROFILE.valueRoles.gate));
 for(const p of BINDING.selector_records)same(request.inputs.find(x=>x.path===p.path),{path:p.path,bytes:p.bytes,sha256:p.sha256},'exact original selector selected pin');
 return analyse({selector:data,prior:request.inputs,before:gate.inputs_before,after:gate.inputs_after,profile},true);
}
/** Synthetic controls only. A shape-valid proposal here is never byte-qualified or executable. */
export function planUnqualifiedLoadedSelection(bytes){return analyse(boundedJSON(bytes),false);}

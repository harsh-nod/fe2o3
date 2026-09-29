// Explicit injected-I/O protocol only. Importing performs no filesystem operations.
import {createHash} from 'node:crypto';
import {isDeepStrictEqual as equal} from 'node:util';
const fail=m=>{throw Error('loaded reader: '+m);};
const check=(v,m)=>{if(!v)fail(m);};
const freeze=v=>{if(v&&typeof v==='object'){for(const x of Object.values(v))freeze(x);Object.freeze(v);}return v;};
const sha=b=>createHash('sha256').update(b).digest('hex');
function uint(n,max=Number.MAX_SAFE_INTEGER){check(Number.isSafeInteger(n)&&n>=0&&n<=max,'integer bound');return n;}
function add(a,b){return uint(uint(a)+uint(b));}
function keys(v,n){check(v&&typeof v==='object'&&!Array.isArray(v)&&equal(Object.keys(v).sort(),n.slice().sort()),'closed keys');}
function path(v){check(typeof v==='string'&&Buffer.byteLength(v)<=4096&&v.startsWith('/')&&!v.includes('\\')&&!v.includes('\0')&&!v.includes(' (deleted)')&&!v.split('/').slice(1).some(x=>!x||x==='.'||x==='..'),'named path');return v;}
function decimal(v){check(typeof v==='string'&&/^(0|[1-9][0-9]{0,24})$/.test(v),'identity decimal');return v;}
function pin(p){keys(p,['path','bytes','sha256']);path(p.path);uint(p.bytes,512*1024*1024);check(/^[a-f0-9]{64}$/.test(p.sha256),'digest');return p;}
function identity(v){check(Array.isArray(v)&&v.length===6,'six identity fields');v.forEach(decimal);return v;}
function fullStat(v,kind){
 keys(v,['kind','identity','uid','gid']);check(['file','directory','symlink'].includes(v.kind)&&(!kind||v.kind===kind),'file type');
 identity(v.identity);decimal(v.uid);decimal(v.gid);
 const mode=BigInt(v.identity[3])&0o170000n;
 check(mode==={file:0o100000n,directory:0o040000n,symlink:0o120000n}[v.kind],'full mode/type');
 return {kind:v.kind,identity:[...v.identity],uid:v.uid,gid:v.gid};
}
class MetadataFailure extends Error{constructor(operation,cause){super('metadata provider '+operation+' failed');this.operation=operation;this.cause=cause;}}
function same(a,b,label){check(equal(a,b),label);}
function prefixes(p){const parts=p.split('/').slice(1),out=['/'];let sofar='';for(const part of parts){sofar+='/'+part;out.push(sofar);}check(out.length<=129,'component bound');return out;}
function labels(v){
 keys(v,['prior','loaded','duties','extras']);
 for(const k of Object.keys(v)){check(Array.isArray(v[k])&&v[k].length<=4096,'role bound');for(const r of v[k]){check(typeof r==='string'&&r.length<=128&&!r.includes('\0'),'role label');}}
 check(v.prior.length+v.loaded.length+v.duties.length+v.extras.length>0,'unlabelled path');
}
export function deriveReadBudget(entries,aliases,passes=2){
 uint(passes,2);check(passes>0,'positive pass count');
 let payload=0,calls=0,metadata=0,readable=0,absent=0;
 const aliasNames=new Set(aliases.map(x=>x.pin.path));
 for(const r of entries){
  if(r.kind==='readable'){
   readable++;payload=add(payload,r.pin.bytes);calls=add(calls,Math.ceil(r.pin.bytes/65536)+1);
   // Initial + descriptor/content bracket + close, followed by final named revalidation.
   metadata=add(metadata,aliasNames.has(r.path)?19:10);
  }else{
   absent++;const n=prefixes(r.path).length;
   // Each bounded walk: lstat + realpath per existing prefix and one ENOENT.
   // Two walks per pass; no content/EOF read for absence.
   metadata=add(metadata,2*(2*n+1));
  }
 }
 const one={readable_paths:readable,absence_observations:absent,payload_bytes:payload,reserved_bytes:add(payload,readable),content_calls:calls,metadata_calls:metadata};
 return {passes,one_pass:one,total:Object.fromEntries(Object.entries(one).map(([k,n])=>[k,uint(n*passes)])),chunk_bytes:65536,scratch_bytes:65537};
}
function admit(input){
 // Bounds are logical source-policy bounds, not a V8 heap or process-I/O theorem.
 const text=JSON.stringify(input);check(typeof text==='string'&&Buffer.byteLength(text)<=8*1024*1024,'protocol byte bound');
 const p=JSON.parse(text);
 keys(p,['schema','entries','aliases','passes','budget','selection_digest','custody']);
 check(p.schema==='fe2o3-loaded-read-protocol-v1','protocol schema');
 check(Array.isArray(p.entries)&&p.entries.length>0&&p.entries.length<=4096,'entry bound');
 check(Array.isArray(p.aliases)&&p.aliases.length<=3,'alias bound');
 check(/^[a-f0-9]{64}$/.test(p.selection_digest),'selection digest');
 keys(p.custody,['qualified_historical_input_bytes','operational_roster_complete','root_cap_change_approved','execution_authority']);
 check(typeof p.custody.qualified_historical_input_bytes==='boolean','historical qualification flag');
 for(const n of ['operational_roster_complete','root_cap_change_approved','execution_authority'])check(p.custody[n]===false,'authority promotion');
 const names=new Map();
 for(const r of p.entries){
  keys(r,['path','kind','pin','resolved','identity','ownership','labels','cap']);path(r.path);path(r.resolved);labels(r.labels);
  check(!names.has(r.path),'duplicate named entry');names.set(r.path,r);
  uint(r.cap,512*1024*1024);
  if(r.kind==='readable'){
   pin(r.pin);same(r.pin.path,r.path,'pin path');check(r.pin.bytes<=r.cap,'individual cap');identity(r.identity);
   same(r.identity[2],String(r.pin.bytes),'historical size');check((BigInt(r.identity[3])&0o170000n)===0o100000n,'historical regular mode');
   if(r.ownership!==null){keys(r.ownership,['uid','gid']);decimal(r.ownership.uid);decimal(r.ownership.gid);}
  }else{
   check(r.kind==='absence-observation','entry kind');same(r.path,r.resolved,'absence alias');
   check(r.pin===null&&r.identity===null&&r.ownership===null&&r.cap===0,'absence is not a file');
  }
  prefixes(r.path);
 }
 const aliases=new Map();
 for(const a of p.aliases){
  keys(a,['pin','resolved','link_text','target_pin','target_selected']);pin(a.pin);pin(a.target_pin);path(a.resolved);
  check(typeof a.link_text==='string'&&Buffer.byteLength(a.link_text)<=4096&&!a.link_text.includes('\0'),'link text bound');
  check(typeof a.target_selected==='boolean'&&!aliases.has(a.pin.path),'alias declaration');
  const r=names.get(a.pin.path);check(r&&r.kind==='readable','selected named alias');
  same(r.pin,a.pin,'alias pin');same(r.resolved,a.resolved,'alias target');
  same(a.target_pin,{...a.pin,path:a.resolved},'whole target pin');check(a.resolved!==a.pin.path,'alias must differ');
  same(names.has(a.resolved),a.target_selected,'independent target duty');
  if(a.target_selected)same(names.get(a.resolved).pin,a.target_pin,'independently paid target pin');
  aliases.set(a.pin.path,a);
 }
 for(const r of p.entries)check((r.path!==r.resolved)===aliases.has(r.path),'no undeclared alias');
 same(p.budget,deriveReadBudget(p.entries,p.aliases,p.passes),'derived full budget');
 return {p:freeze(p),aliases};
}
export class LoadedReaderRefusal extends Error{
 constructor(cause,accounting){super('loaded reader refused: '+String(cause?.message??cause).slice(0,512));this.name='LoadedReaderRefusal';this.accounting=freeze(accounting);}
}
/**
 * Always unqualified injected-I/O protocol. Provider methods perform one operation.
 * The caller owns provider custody and external finite scheduling; this is not a lease.
 * No callback runs between the two complete passes, and no file contents are retained.
 */
export function readUnqualifiedProtocol(input,provider,{guard,now,milliseconds=240000}){
 const {p,aliases}=admit(input);
 check(provider&&typeof provider==='object','provider');
 for(const k of ['lstat','realpath','readlink','open','fstat','read','close'])check(typeof provider[k]==='function','provider operation');
 check(typeof guard==='function'&&typeof now==='function','explicit external guard and clock');
 uint(milliseconds,240000);check(milliseconds>0,'finite deadline');
 const start=now();check(Number.isFinite(start),'initial clock');let previous=start;
 const counts={reserved_bytes:p.budget.total.reserved_bytes,reserved_content_calls:p.budget.total.content_calls,reserved_metadata_calls:p.budget.total.metadata_calls,attempted_content_bytes:0,returned_content_bytes:0,content_calls:0,metadata_calls:0,opened:0,close_attempts:0,closed:0,completed_passes:0,completed_readable:0,completed_absence:0};
 const scratch=Buffer.alloc(65536),eof=Buffer.alloc(1),passes=[];
 function clock(){guard();const t=now();check(Number.isFinite(t)&&t>=previous&&t-start<milliseconds,'deadline or clock regression');previous=t;}
 function metadata(name,...args){
  clock();check(counts.metadata_calls<p.budget.total.metadata_calls,'metadata call budget');counts.metadata_calls++;try{return provider[name](...args);}catch(e){throw new MetadataFailure(name,e);}
 }
 function content(fd,buffer,length){
  clock();check(counts.content_calls<p.budget.total.content_calls,'content call budget');
  check(counts.attempted_content_bytes<=p.budget.total.reserved_bytes-length,'content byte budget');
  counts.content_calls++;counts.attempted_content_bytes+=length;
  const n=provider.read(fd,buffer,0,length);uint(n,length);counts.returned_content_bytes=add(counts.returned_content_bytes,n);return n;
 }
 function closeOwned(fd){
  // Cleanup must run even after deadline/guard refusal. It never retries.
  counts.metadata_calls++;counts.close_attempts++;
  provider.close(fd);counts.closed++;
  check(counts.metadata_calls<=p.budget.total.metadata_calls,'cleanup metadata budget');
 }
 function targetStat(r,stat){fullStat(stat,'file');same(stat.identity,r.identity,'historical target identity');if(r.ownership)same({uid:stat.uid,gid:stat.gid},r.ownership,'loaded owner identity');return stat;}
 function resolution(r,a){
  const named=fullStat(metadata('lstat',r.path),a?'symlink':'file');
  same(metadata('realpath',r.path),r.resolved,'named realpath');
  if(!a)return{named,target:named,link_text:null};
  const link=metadata('readlink',r.path);same(link,a.link_text,'exact named link text');
  const target=fullStat(metadata('lstat',r.resolved),'file');same(metadata('realpath',r.resolved),r.resolved,'canonical target');
  return{named,target,link_text:link};
 }
 function revalidate(r,a,before){
  const after=resolution(r,a);same(after,before,'named/target identity or alias drift');
 }
 function absence(r){
  const parts=prefixes(r.path),ancestors=[];
  for(let i=0;i<parts.length;i++){
   const name=parts[i];let st;
   try{st=metadata('lstat',name);}catch(e){
    // No ENOTDIR, permissions, dangling links or ambiguous errors become absence.
    if(!(e instanceof MetadataFailure)||e.operation!=='lstat'||e.cause?.code!=='ENOENT')throw e;
    check(i>0,'root cannot be absent');
    return{kind:'absence-observation',path:r.path,missing_component:name,existing_ancestors:ancestors,content_read:false};
   }
   st=fullStat(st,'directory');
   same(metadata('realpath',name),name,'absence ancestor canonical');
   ancestors.push({path:name,stat:st});
  }
  fail('expected absent path exists');
 }
 function readable(r){
  const a=aliases.get(r.path),before=resolution(r,a);targetStat(r,before.target);let fd=null,result;
  try{
   fd=metadata('open',r.resolved);uint(fd);counts.opened++;
   const first=fullStat(metadata('fstat',fd),'file');same(first,before.target,'named/descriptor admission');targetStat(r,first);
   const hash=createHash('sha256');let used=0;
   while(used<r.pin.bytes){const n=Math.min(65536,r.pin.bytes-used);check(content(fd,scratch,n)===n,'short read without retry');hash.update(scratch.subarray(0,n));used+=n;}
   check(content(fd,eof,1)===0,'EOF growth');same(hash.digest('hex'),r.pin.sha256,'whole content hash');
   const after=fullStat(metadata('fstat',fd),'file');same(after,first,'descriptor drift');revalidate(r,a,before);
   result={kind:'readable',pin:r.pin,resolved:r.resolved,stat:first,named:before.named,link_text:before.link_text,labels:r.labels};
  }finally{if(fd!==null)closeOwned(fd);}
  counts.completed_readable++;return{result,before};
 }
 try{
  clock();
  for(let pass=0;pass<p.passes;pass++){
   const observations=[],checks=[];
   for(const r of p.entries){
    if(r.kind==='readable'){const x=readable(r);observations.push(x.result);checks.push(x.before);}
    else{const x=absence(r);observations.push(x);checks.push(x);counts.completed_absence++;}
   }
   // Recheck every name after the full pass; early files cannot silently drift while later files are read.
   for(let i=0;i<p.entries.length;i++){
    const r=p.entries[i];
    if(r.kind==='readable')revalidate(r,aliases.get(r.path),checks[i]);
    else same(absence(r),checks[i],'absence/ancestor changed during pass');
   }
   if(pass)same(observations,passes[0],'full two-pass observation mismatch');
   passes.push(observations);counts.completed_passes++;clock();
  }
  same(counts.content_calls,p.budget.total.content_calls,'complete planned content calls');
  same(counts.attempted_content_bytes,p.budget.total.reserved_bytes,'complete planned inclusive bytes');
  same(counts.returned_content_bytes,p.budget.total.payload_bytes,'complete planned payload');
  same(counts.opened,counts.closed,'all descriptors closed');clock();
  return freeze({schema:'fe2o3-loaded-reader-protocol-observation-v1',provider_identity:'caller-supplied-unqualified',io_protocol_completed:true,qualified:false,execution_authority:false,native_acceptance:false,gpu_dispatch:false,physical_capture:false,operational_roster_complete:false,root_cap_change_approved:false,selection_digest:p.selection_digest,entry_count:p.entries.length,counts,passes,observation_interval_only:true,global_writer_exclusion:false,complete_import_history:false,cache_execution_provenance:false,module_loader_io_metered:false,syscall_counters_are_provider_calls:true,scratch_bytes:65537});
 }catch(e){throw new LoadedReaderRefusal(e,{...counts,accepted:false,live_fd_possible:counts.opened!==counts.closed,all_prior_debits_retained:true});}
}

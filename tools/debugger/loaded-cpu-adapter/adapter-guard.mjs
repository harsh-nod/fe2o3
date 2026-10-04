// Finite CPU guard. No default clocks/resources and no import-time IO.
import {createHash} from 'node:crypto';
const equalKeys=(v,keys)=>v&&typeof v==='object'&&!Array.isArray(v)&&JSON.stringify(Object.keys(v).sort())===JSON.stringify(keys.slice().sort());
const check=(v,m)=>{if(!v)throw Error('adapter guard policy: '+m);};
const freeze=v=>{if(v&&typeof v==='object'){for(const x of Object.values(v))freeze(x);Object.freeze(v);}return v;};
export function boundedError(e){
 const message=String(e?.message??e),raw=Buffer.from(message),limit=16384;
 return freeze({name:String(e?.name??'Error').slice(0,128),code:typeof e?.code==='string'?e.code.slice(0,128):null,message:raw.length<=limit?message:raw.subarray(0,limit).toString('utf8'),message_bytes:raw.length,message_sha256:createHash('sha256').update(raw).digest('hex'),message_truncated:raw.length>limit});
}
export function createFiniteGuard(raw,{monotonicNow,utcNow,resources}){
 check(Buffer.isBuffer(raw)&&raw.length>0&&raw.length<=16384,'bounded explicit JSON policy');
 const text=raw.toString('utf8');check(Buffer.from(text).equals(raw),'UTF8');const p=JSON.parse(text);
 check(equalKeys(p,['schema','not_before_utc_ms','expires_utc_ms','max_elapsed_ms','max_rss_bytes','min_free_bytes','min_available_ram_bytes','resource_interval_ms','max_resource_probes']),'closed keys');
 check(p.schema==='fe2o3-cpu-reader-guard-policy-v1','schema');
 for(const k of Object.keys(p).filter(k=>k!=='schema'))check(Number.isSafeInteger(p[k])&&p[k]>=0,'integer '+k);
 check(p.expires_utc_ms>p.not_before_utc_ms&&p.expires_utc_ms-p.not_before_utc_ms<=24*60*60*1000,'finite scope');
 check(p.max_elapsed_ms>0&&p.max_elapsed_ms<=720000&&p.max_rss_bytes>0,'finite process bounds');
 check(p.resource_interval_ms>0&&p.resource_interval_ms<=1000&&p.max_resource_probes>0&&p.max_resource_probes<=1024,'bounded resource sampling');
 for(const f of [monotonicNow,utcNow,resources])check(typeof f==='function','explicit observers');
 const state={checks:0,resource_probes:0,start_monotonic:null,last_monotonic:null,last_utc:null,last_resource_monotonic:null,last_resources:null,first_failure:null,denied:false};
 function deny(code,detail=null){if(!state.first_failure)state.first_failure={code,detail,check:state.checks,monotonic:state.last_monotonic,utc:state.last_utc};state.denied=true;const e=new Error('CPU adapter guard refused: '+state.first_failure.code);e.code='ADAPTER_GUARD_REFUSED';throw e;}
 function guard(){
  if(state.denied)return deny(state.first_failure.code);
  if(++state.checks>2000000)return deny('guard-check-cap');
  let mono,utc;try{mono=monotonicNow();utc=utcNow();}catch(e){return deny('clock-observer-failure',boundedError(e));}
  if(!Number.isFinite(mono)||!Number.isSafeInteger(utc)||mono<0||utc<0)return deny('invalid-clock');
  if(state.last_monotonic!==null&&(mono<state.last_monotonic||utc<state.last_utc))return deny('clock-regression');
  state.last_monotonic=mono;state.last_utc=utc;
  if(state.start_monotonic===null)state.start_monotonic=mono;
  if(utc<p.not_before_utc_ms||utc>=p.expires_utc_ms)return deny('scope-not-current');
  if(mono-state.start_monotonic>=p.max_elapsed_ms)return deny('global-deadline');
  if(state.last_resource_monotonic===null||mono-state.last_resource_monotonic>=p.resource_interval_ms){
   if(state.resource_probes>=p.max_resource_probes)return deny('resource-probe-cap');
   state.resource_probes++;let r;
   try{r=resources();}catch(e){return deny('resource-observer-failure',boundedError(e));}
   if(!equalKeys(r,['rss_bytes','free_bytes','available_ram_bytes'])||!Object.values(r).every(v=>Number.isSafeInteger(v)&&v>=0))return deny('resource-shape');
   state.last_resources={...r};state.last_resource_monotonic=mono;
   if(r.rss_bytes>p.max_rss_bytes||r.free_bytes<p.min_free_bytes||r.available_ram_bytes<p.min_available_ram_bytes)return deny('resource-bound');
   let afterMono,afterUtc;try{afterMono=monotonicNow();afterUtc=utcNow();}catch(e){return deny('clock-observer-failure',boundedError(e));}
   if(!Number.isFinite(afterMono)||!Number.isSafeInteger(afterUtc)||afterMono<mono||afterUtc<utc)return deny('clock-regression-after-resource-probe');
   state.last_monotonic=afterMono;state.last_utc=afterUtc;
   if(afterUtc>=p.expires_utc_ms||afterMono-state.start_monotonic>=p.max_elapsed_ms)return deny('deadline-after-resource-probe');
  }
 }
 guard.snapshot=()=>freeze(JSON.parse(JSON.stringify({schema:'fe2o3-cpu-reader-guard-observation-v1',policy:p,...state,scope_approval_inferred:false,external_process_deadline_required:true,synchronous_probe_preemption:false})));
 return guard;
}

// Explicit sampled resource domain, separate from immutable selected-file reads.
export function createBoundedResourceObserver({resource_root,scope,started,monotonicNow,utcNow,provider}){
 check(typeof resource_root==='string'&&Buffer.byteLength(resource_root)<=4096&&resource_root.startsWith('/')&&resource_root!=='/'&&!/[\\\x00-\x1f\x7f]/.test(resource_root)&&!resource_root.split('/').slice(1).some(x=>!x||x==='.'||x==='..'),'explicit resource root');
 check(Number.isFinite(started)&&started>=0&&typeof monotonicNow==='function'&&typeof utcNow==='function','resource clocks');
 check(scope&&Number.isInteger(scope.max_resource_probes)&&scope.max_resource_probes>0&&scope.max_resource_probes<=1024,'resource probe policy');
 for(const name of ['statfs','open','read','close','rss'])check(typeof provider?.[name]==='function','resource provider '+name);
 const names=['statfs','open','read','rss','close'],ledger={probes:0,attempted:Object.fromEntries(names.map(n=>[n,0])),invoked:Object.fromEntries(names.map(n=>[n,0])),opened:0,close_attempts:0,closed:0,requested_bytes:0,returned_bytes:0,content_calls:0,first_failure:null,cleanup_errors:[],domain:'separate sampled resource IO; not selected-input provider counters'};
 let firstError=null,failed=false,operation=null;
 function remember(e){if(!failed){failed=true;firstError=e;ledger.first_failure={operation,error:boundedError(e)};}}
 function current(){const utc=utcNow(),mono=monotonicNow();check(Number.isSafeInteger(utc)&&Number.isFinite(mono)&&mono>=started&&utc>=scope.not_before_utc_ms&&utc<scope.expires_utc_ms&&mono-started<scope.max_elapsed_ms,'resource-probe deadline');}
 const dispatch=(name,...args)=>{operation=name;ledger.attempted[name]++;current();ledger.invoked[name]++;return provider[name](...args);};
 function observe(){
  if(failed)throw firstError;
  let fd=null,available=null,free=null,rss=null;
  try{
   operation='probe-admission';check(ledger.probes<scope.max_resource_probes,'resource probe cap');ledger.probes++;
   const disk=dispatch('statfs',resource_root);check(disk&&typeof disk.bavail==='bigint'&&typeof disk.bsize==='bigint'&&disk.bavail>=0n&&disk.bsize>=0n,'statfs integers');
   free=Number(disk.bavail*disk.bsize);check(Number.isSafeInteger(free),'free disk integer');
   const opened=dispatch('open','/proc/meminfo');check(Number.isSafeInteger(opened)&&opened>=0,'resource descriptor');fd=opened;ledger.opened++;
   const raw=Buffer.alloc(65536),eof=Buffer.alloc(1);
   ledger.requested_bytes+=65536;ledger.content_calls++;const n=dispatch('read',fd,raw,0,65536);
   check(Number.isSafeInteger(n)&&n>0&&n<=65536,'bounded meminfo record');ledger.returned_bytes+=n;
   ledger.requested_bytes++;ledger.content_calls++;const more=dispatch('read',fd,eof,0,1);
   check(Number.isSafeInteger(more)&&more>=0&&more<=1,'resource EOF result');ledger.returned_bytes+=more;check(more===0,'complete meminfo EOF');
   operation='meminfo-parse';const matches=[...raw.subarray(0,n).toString('ascii').matchAll(/^MemAvailable:[ \t]+([0-9]+) kB$/gm)];check(matches.length===1,'unique MemAvailable');
   available=Number(BigInt(matches[0][1])*1024n);check(Number.isSafeInteger(available),'available RAM integer');
   rss=dispatch('rss');check(Number.isSafeInteger(rss)&&rss>=0,'RSS integer');
  }catch(e){remember(e);}
  finally{
   if(fd!==null){operation='close';ledger.attempted.close++;ledger.invoked.close++;ledger.close_attempts++;try{provider.close(fd);ledger.closed++;}catch(e){ledger.cleanup_errors.push(boundedError(e));remember(e);}}
  }
  if(!failed){operation='post-cleanup-currentness';try{current();}catch(e){remember(e);}}
  if(failed)throw firstError;
  return{rss_bytes:rss,free_bytes:free,available_ram_bytes:available};
 }
 observe.snapshot=()=>freeze(JSON.parse(JSON.stringify({...ledger,live_fd_possible:ledger.opened!==ledger.closed,cleanup_without_fresh_read_authority:true})));
 return observe;
}

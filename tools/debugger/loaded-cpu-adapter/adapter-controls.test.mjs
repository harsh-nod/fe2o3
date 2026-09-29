// Fixture-free injected controls only: no filesystem provider, historical fixture or operational path is read.
import test from 'node:test';
import assert from 'node:assert/strict';
import {createHash} from 'node:crypto';
import {deriveReadBudget} from '../loaded-input-reader/reader-protocol.mjs';
import {executeAdapterPlan} from './adapter-protocol.mjs';
import {createFiniteGuard,boundedError,createBoundedResourceObserver} from './adapter-guard.mjs';
import {publishExclusiveEvidence,admitEvidenceSpec,encodeBoundedEvidence} from './adapter-writer.mjs';
const sha=b=>createHash('sha256').update(b).digest('hex');
const copy=v=>structuredClone(v),encode=v=>Buffer.from(JSON.stringify(v));
const ident=(bytes,ino='2',mode='33152')=>['1',ino,String(bytes),mode,'10','10'];
const labels=()=>({prior:['prior:0'],loaded:[],duties:['duty:0'],extras:[]});
const file=(path='/fixture/a',body=Buffer.from('abcd'))=>({path,kind:'readable',pin:{path,bytes:body.length,sha256:sha(body)},resolved:path,identity:ident(body.length),ownership:null,labels:labels(),cap:body.length});
const absent=path=>({path,kind:'absence-observation',pin:null,resolved:path,identity:null,ownership:null,labels:{prior:[],loaded:[],duties:['absence'],extras:[]},cap:0});
const protocol=(entries,passes,aliases=[])=>({schema:'fe2o3-loaded-read-protocol-v1',entries:copy(entries),aliases:copy(aliases),passes,budget:deriveReadBudget(entries,aliases,passes),selection_digest:'a'.repeat(64),custody:{qualified_historical_input_bytes:false,operational_roster_complete:false,root_cap_change_approved:false,execution_authority:false}});
function plan(entries=[file()],historical=entries,aliases=[]){return{schema:'fe2o3-cpu-loaded-adapter-plan-v1',graph_protocol:protocol(entries,1,aliases),historical_protocol:protocol(historical,2,aliases),named_cap:entries.length,phase_milliseconds:{precheck:1000,historical:1000,postcheck:1000}};}
function memory(p,bodies=new Map([['/fixture/a',Buffer.from('abcd')]]),hook=()=>undefined){
 const entries=p.graph_protocol.entries,byName=new Map(entries.map(r=>[r.path,r])),aliases=new Map(p.graph_protocol.aliases.map(a=>[a.pin.path,a])),fds=new Map(),calls=[],counts={};let next=10;
 const missing=()=>{const e=Error('synthetic missing');e.code='ENOENT';throw e;};
 const lookup=name=>{
  if(aliases.has(name))return{kind:'symlink',identity:ident(aliases.get(name).link_text.length,'8','41471'),uid:'1',gid:'1'};
  const e=byName.get(name)??entries.find(r=>r.resolved===name&&r.kind==='readable');
  if(e?.kind==='readable')return{kind:'file',identity:copy(e.identity),uid:e.ownership?.uid??'1',gid:e.ownership?.gid??'1'};
  if(e?.kind==='absence-observation')return missing();
  if(name==='/'||entries.some(r=>r.path.startsWith(name+'/')))return{kind:'directory',identity:ident(0,'1','16877'),uid:'1',gid:'1'};
  return missing();
 };
 const methods={
  lstat:lookup,realpath:name=>byName.get(name)?.resolved??name,readlink:name=>aliases.get(name)?.link_text,
  open:name=>{const fd=next++;fds.set(fd,{name,at:0});return fd;},
  fstat:fd=>lookup(fds.get(fd).name),
  read:(fd,buffer,offset,length)=>{const f=fds.get(fd),body=bodies.get(f.name)??Buffer.alloc(0),n=Math.min(length,Math.max(0,body.length-f.at));body.copy(buffer,offset,f.at,f.at+n);f.at+=n;return n;},
  close:fd=>{assert.ok(fds.has(fd));fds.delete(fd);},
 };
 const provider=Object.fromEntries(Object.entries(methods).map(([name,fn])=>[name,(...args)=>{counts[name]=(counts[name]??0)+1;const call={name,args,index:counts[name]};calls.push(call);return hook(call,()=>fn(...args),{fds,calls,counts})??fn(...args);}]));
 return{provider,calls,counts,fds};
}
function run(p=plan(),{hook,bodies,guard=()=>{},now=()=>0}={}){const m=memory(p,bodies,hook);return{record:executeAdapterPlan(encode(p),{provider:m.provider,guard,now}),m};}
const states=r=>r.phases.map(p=>p.status);
const guardPolicy=()=>({schema:'fe2o3-cpu-reader-guard-policy-v1',not_before_utc_ms:100,expires_utc_ms:10000,max_elapsed_ms:1000,max_rss_bytes:100,min_free_bytes:10,min_available_ram_bytes:10,resource_interval_ms:10,max_resource_probes:10});
function guardFixture(change=()=>{}){let mono=0,utc=101;const policy=guardPolicy(),resources={rss_bytes:50,free_bytes:100,available_ram_bytes:100};change(policy,resources);const guard=createFiniteGuard(encode(policy),{monotonicNow:()=>mono,utcNow:()=>utc,resources:()=>resources});return{guard,policy,resources,set:(m,u=101)=>{mono=m;utc=u;}};}
const syncOK=()=>({opened:true,synced:true,close_attempts:1,closed:true,live_fd_possible:false,first_error:null,cleanup_errors:[]});
const directoryStat=()=>({kind:'directory',identity:ident(0,'50','16832'),uid:'1',gid:'1'});
const writerSpec=()=>({schema:'fe2o3-exclusive-adapter-evidence-v1',directory:'/new-output',directory_identity:directoryStat().identity,temporary_name:'observation.pending',final_name:'observation.json',cap_bytes:200000});
function memoryWriter(hook=()=>undefined){
 const files=new Map(),fds=new Map(),calls=[],counts={};let next=20;
 const eexist=()=>{const e=Error('synthetic existing destination');e.code='EEXIST';throw e;};
 const fileStat=f=>({kind:'file',identity:ident(f.body.length,f.ino),uid:'1',gid:'1'});
 const methods={
  directory:()=>directoryStat(),
  openExclusive:path=>{if(files.has(path))return eexist();const f={body:Buffer.alloc(0),ino:String(next)},fd=next++;files.set(path,f);fds.set(fd,f);return fd;},
  fstat:fd=>fileStat(fds.get(fd)),
  write:(fd,b,at,n)=>{const f=fds.get(fd);f.body=Buffer.concat([f.body,b.subarray(at,at+n)]);return n;},
  fsync:()=>true,close:fd=>{fds.delete(fd);return true;},
  linkExclusive:(tmp,final)=>{if(files.has(final))return eexist();files.set(final,files.get(tmp));return true;},
  lstat:path=>fileStat(files.get(path)),fsyncDirectory:syncOK,
 };
 const provider=Object.fromEntries(Object.entries(methods).map(([name,fn])=>[name,(...args)=>{counts[name]=(counts[name]??0)+1;const call={name,args,index:counts[name]};calls.push(call);return hook(call,()=>fn(...args),{files,fds,calls,counts})??fn(...args);}]));
 return{provider,files,fds,calls,counts};
}
function write(body=Buffer.from('report'),{hook,guard=()=>{},spec=writerSpec()}={}){const m=memoryWriter(hook);return{result:publishExclusiveEvidence(body,encode(spec),m.provider,{guard}),m};}

test('complete one/two/one sequence preserves distinct phase and aggregate ceilings',()=>{
 const {record:r,m}=run();assert.equal(r.status,'read-protocol-completed');assert.deepEqual(states(r),['completed','completed','completed']);
 assert.deepEqual(r.phases.map(x=>x.result.counts.completed_passes),[1,2,1]);assert.equal(r.shared_accounting.byte_cap,20);assert.equal(r.shared_accounting.requested_content_bytes,20);assert.equal(r.shared_accounting.content_cap,8);assert.equal(m.counts.open,4);assert.equal(m.counts.close,4);assert.equal(r.qualified,false);assert.equal(r.native_authority,false);assert.equal(r.root_cap_approval_inferred,false);assert.equal(r.child_processes_started,0);assert.ok(Object.isFrozen(r.phases));
});
test('graph-only extra name is charged only in outer phases',()=>{
 const a=file(),b=file('/fixture/b',Buffer.from('xy'));b.labels={prior:[],loaded:[],duties:[],extras:['extra']};const p=plan([a,b],[a]),bodies=new Map([['/fixture/a',Buffer.from('abcd')],['/fixture/b',Buffer.from('xy')]]);
 const {record:r,m}=run(p,{bodies});assert.equal(r.status,'read-protocol-completed');assert.equal(m.counts.open,6);assert.equal(r.shared_accounting.byte_cap,26);assert.equal(r.phases[1].budget.total.readable_paths,2);
});
test('exact64KiB chunks and EOF pay all four passes',()=>{
 const body=Buffer.alloc(65537,7),p=plan([file('/fixture/a',body)]);const {record:r,m}=run(p,{bodies:new Map([['/fixture/a',body]])});assert.equal(r.status,'read-protocol-completed');assert.deepEqual(m.calls.filter(c=>c.name==='read').map(c=>c.args[3]),Array(4).fill([65536,1,1]).flat());assert.equal(r.shared_accounting.requested_content_bytes,4*65538);
});
test('observed empty regular file still pays EOF and remains distinct from absence',()=>{
 const p=plan([file('/fixture/a',Buffer.alloc(0)),absent('/fixture/missing')]);const {record:r}=run(p,{bodies:new Map([['/fixture/a',Buffer.alloc(0)]])});assert.equal(r.status,'read-protocol-completed');assert.equal(r.shared_accounting.content_attempts,4);assert.equal(r.phases[0].result.counts.completed_absence,1);
});
test('bounded expected ENOENT absence does not become a retained provider failure',()=>{
 const p=plan([file(),absent('/fixture/missing/deep')]);const {record:r}=run(p);assert.equal(r.status,'read-protocol-completed');assert.equal(r.first_failure,null);assert.equal(r.phases[1].result.counts.completed_absence,2);
});
test('ENOTDIR is never accepted as absence',()=>{
 const p=plan([file(),absent('/fixture/missing')]);const {record:r}=run(p,{hook:c=>{if(c.name==='lstat'&&c.args[0]==='/fixture/missing'){const e=Error('not a directory');e.code='ENOTDIR';throw e;}}});assert.equal(r.status,'failed');assert.equal(r.first_failure.code,'provider-lstat-failed');assert.deepEqual(states(r),['failed','not-started','not-started']);
});
test('an existing expected-absent path refuses without inventing zero-byte content',()=>{
 const p=plan([absent('/fixture/missing')]);const {record:r,m}=run(p,{hook:c=>c.name==='lstat'?directoryStat():undefined});assert.equal(r.status,'failed');assert.equal(m.counts.read??0,0);assert.equal(r.phases[0].budget.total.reserved_bytes,0);
});
test('alias and independently named target are both read and charged',()=>{
 const body=Buffer.from('abcd'),a=file('/fixture/alias',body),t=file('/fixture/target',body);a.resolved=t.path;t.labels={prior:[],loaded:[],duties:['target'],extras:[]};
 const aliases=[{pin:a.pin,resolved:t.path,link_text:'target',target_pin:t.pin,target_selected:true}],p=plan([a,t],[a,t],aliases);
 const {record:r,m}=run(p,{bodies:new Map([[t.path,body]])});assert.equal(r.status,'read-protocol-completed');assert.equal(m.counts.open,8);assert.equal(r.shared_accounting.byte_cap,40);
});
test('changed alias literal refuses exact path before opening',()=>{
 const a=file('/fixture/alias'),t=file('/fixture/target');a.resolved=t.path;const aliases=[{pin:a.pin,resolved:t.path,link_text:'target',target_pin:t.pin,target_selected:true}];
 const {record:r,m}=run(plan([a,t],[a,t],aliases),{hook:c=>c.name==='readlink'?'different':undefined});assert.equal(r.first_failure.code,'readlink-mismatch');assert.equal(r.first_failure.entry_path,a.path);assert.equal(m.counts.open??0,0);
});
test('stale target identity gives exact precheck path and immutable expected pin',()=>{
 const p=plan(),expected=copy(p.graph_protocol.entries[0].pin);const {record:r,m}=run(p,{hook:(c,normal)=>{if(c.name==='lstat'&&c.args[0]==='/fixture/a'){const s=normal();s.identity[2]='9';return s;}}});
 assert.equal(r.first_failure.code,'historical-input-identity-mismatch');assert.equal(r.first_failure.entry_path,'/fixture/a');assert.deepEqual(r.first_failure.expected_pin,expected);assert.equal(r.first_failure.phase,'precheck');assert.equal(m.counts.open??0,0);assert.deepEqual(states(r),['failed','not-started','not-started']);assert.equal(r.phases[2].budget.total.reserved_bytes,5);
});
test('same-size wrong content retains full observed hash after cleanup',()=>{
 const {record:r,m}=run(plan(),{bodies:new Map([['/fixture/a',Buffer.from('wxyz')]])});assert.equal(r.first_failure.code,'whole-content-hash-mismatch');assert.equal(r.first_failure.observed_sha256,sha(Buffer.from('wxyz')));assert.equal(m.counts.close,1);assert.equal(r.phases[0].provider_observation.last_closed.returned_content_bytes,4);
});
test('short read is not retried and attempted byte debit remains',()=>{
 const {record:r,m}=run(plan(),{hook:(c,normal)=>{if(c.name==='read'&&c.index===1){normal();return 2;}}});assert.equal(r.first_failure.code,'short-read');assert.equal(m.counts.read,1);assert.equal(r.shared_accounting.requested_content_bytes,4);assert.equal(r.phases[0].provider_observation.counts.returned_content_bytes,2);assert.equal(m.counts.close,1);
});
test('EOF growth refuses and accounts for the returned extra byte',()=>{
 const {record:r}=run(plan(),{bodies:new Map([['/fixture/a',Buffer.from('abcde')]])});assert.equal(r.first_failure.code,'EOF-growth');assert.equal(r.phases[0].provider_observation.counts.returned_content_bytes,5);
});
test('invalid content result is retained even if close also fails',()=>{
 const {record:r}=run(plan(),{hook:c=>{if(c.name==='read')return -1;if(c.name==='close')throw Error('late close');}});assert.equal(r.first_failure.code,'invalid-content-result');assert.equal(r.phases[0].provider_observation.live_descriptors.length,1);
});
test('content mismatch has precedence over a subsequent close exception',()=>{
 const {record:r}=run(plan(),{bodies:new Map([['/fixture/a',Buffer.from('wxyz')]]),hook:c=>{if(c.name==='close')throw Error('close failed');}});
 assert.equal(r.first_failure.code,'whole-content-hash-mismatch');assert.equal(r.phases[0].provider_observation.last_provider_error.method,'close');assert.equal(r.phases[0].failure.reader_accounting.live_fd_possible,true);
});
test('descriptor drift is retained with both stats',()=>{
 const {record:r}=run(plan(),{hook:(c,normal)=>{if(c.name==='fstat'&&c.index===2){const s=normal();s.identity[4]='11';return s;}}});assert.equal(r.first_failure.code,'descriptor-metadata-drift');assert.equal(r.first_failure.before.identity[4],'10');assert.equal(r.first_failure.after.identity[4],'11');
});
test('late named drift cannot be hidden by descriptor cleanup',()=>{
 const {record:r}=run(plan(),{hook:(c,normal)=>{if(c.name==='lstat'&&c.index===2){const s=normal();s.identity[4]='11';return s;}if(c.name==='close')throw Error('close failed');}});assert.equal(r.first_failure.code,'named-metadata-drift');
});
test('wrong descriptor kind is retained before later cleanup failure',()=>{
 const {record:r}=run(plan(),{hook:c=>{if(c.name==='fstat')return directoryStat();if(c.name==='close')throw Error('late close');}});assert.equal(r.first_failure.code,'metadata-kind-mismatch');
});
test('guard expiry after open permits exactly owned cleanup and no read',()=>{
 let expired=false;const guard=()=>{if(expired)throw Error('expired');};
 const {record:r,m}=run(plan(),{guard,hook:(c,normal)=>{if(c.name==='open'){const fd=normal();expired=true;return fd;}}});assert.equal(r.first_failure.source,'guard');assert.equal(m.counts.close,1);assert.equal(m.counts.read??0,0);assert.deepEqual(states(r),['failed','not-started','not-started']);
});
test('failure in historical phase leaves precheck complete and postcheck not-started',()=>{
 const {record:r}=run(plan(),{hook:(c,normal)=>{if(c.name==='lstat'&&c.index===4){const s=normal();s.identity[5]='12';return s;}}});assert.deepEqual(states(r),['completed','failed','not-started']);assert.equal(r.first_failure.phase,'historical');assert.equal(r.phases[2].budget.total.content_calls,2);
});
test('failure in postcheck does not erase completed historical observations',()=>{
 const {record:r}=run(plan(),{hook:(c,normal)=>{if(c.name==='lstat'&&c.index===10){const s=normal();s.identity[5]='12';return s;}}});assert.deepEqual(states(r),['completed','completed','failed']);assert.equal(r.first_failure.phase,'postcheck');assert.equal(r.phases[1].result.counts.completed_passes,2);
});
test('phase deadline refuses without claiming later phases started',()=>{
 let clock=0;const {record:r}=run(plan(),{now:()=>clock++,hook:c=>{if(c.name==='open')clock=1001;}});assert.equal(r.status,'failed');assert.deepEqual(states(r),['failed','not-started','not-started']);
});
test('plan admission refuses a Buffer with invalid UTF8 before provider calls',()=>{
 const m=memory(plan());assert.throws(()=>executeAdapterPlan(Buffer.from([0xff]),{provider:m.provider,guard:()=>{},now:()=>0}));assert.equal(m.calls.length,0);
});
for(const [name,change] of [
 ['unknown root key',p=>{p.approved=true;}],['wrong named cap',p=>{p.named_cap++;}],
 ['historical identity replacement',p=>{p.graph_protocol.entries[0].identity[4]='99';}],
 ['historical label omission',p=>{p.graph_protocol.entries[0].labels.duties=[];}],
 ['historical member cap increase',p=>{p.graph_protocol.entries[0].cap++;}],
 ['alias-policy replacement',p=>{p.historical_protocol.aliases=[{}];}],
 ['authority promotion',p=>{p.graph_protocol.custody.execution_authority=true;}],
 ['phase deadline overflow',p=>{p.phase_milliseconds.postcheck=240001;}],
 ['budget understatement',p=>{p.graph_protocol.budget.total.content_calls--;}],
 ['wrong historical passes',p=>{p.historical_protocol.passes=1;}],
 ['historical protocol unknown key',p=>{p.historical_protocol.unapproved=true;}],
 ['graph full-mode mismatch',p=>{p.graph_protocol.entries[0].identity[3]='0';p.historical_protocol.entries[0].identity[3]='0';}],
]){
 test('admission refuses '+name+' without provider IO',()=>{const p=plan();change(p);const m=memory(plan());assert.throws(()=>executeAdapterPlan(encode(p),{provider:m.provider,guard:()=>{},now:()=>0}));assert.equal(m.calls.length,0);});
}
test('finite guard snapshots explicit policy without approving authority',()=>{const x=guardFixture();x.guard();const s=x.guard.snapshot();assert.equal(s.resource_probes,1);assert.equal(s.denied,false);assert.equal(s.scope_approval_inferred,false);assert.equal(s.external_process_deadline_required,true);assert.ok(Object.isFrozen(s.policy));});
test('expired guard is sticky even if caller later restores clocks',()=>{const x=guardFixture();x.guard();x.set(1,10000);assert.throws(x.guard);x.set(2,102);assert.throws(x.guard);assert.equal(x.guard.snapshot().first_failure.code,'scope-not-current');});
test('monotonic or UTC regression is refused',()=>{const x=guardFixture();x.set(3,103);x.guard();x.set(2,104);assert.throws(x.guard);assert.equal(x.guard.snapshot().first_failure.code,'clock-regression');});
test('global monotonic deadline is finite',()=>{const x=guardFixture();x.guard();x.set(1000);assert.throws(x.guard);assert.equal(x.guard.snapshot().first_failure.code,'global-deadline');});
test('resource bounds retain observation and stop caller IO',()=>{const x=guardFixture((p,r)=>{r.free_bytes=9;});assert.throws(x.guard);assert.equal(x.guard.snapshot().first_failure.code,'resource-bound');assert.equal(x.guard.snapshot().last_resources.free_bytes,9);});
test('resource probe cap is explicit and never silently increased',()=>{const x=guardFixture(p=>{p.max_resource_probes=1;});x.guard();x.set(10);assert.throws(x.guard);assert.equal(x.guard.snapshot().resource_probes,1);assert.equal(x.guard.snapshot().first_failure.code,'resource-probe-cap');});
test('a blocking resource observer cannot authorize IO after elapsed expiry',()=>{let t=0;const g=createFiniteGuard(encode(guardPolicy()),{monotonicNow:()=>t,utcNow:()=>101,resources:()=>{t=1000;return{rss_bytes:1,free_bytes:100,available_ram_bytes:100};}});assert.throws(g);assert.equal(g.snapshot().first_failure.code,'deadline-after-resource-probe');});
test('guard rejects unknown policy keys and missing observers before resources',()=>{let calls=0;const p=guardPolicy();p.approved=true;assert.throws(()=>createFiniteGuard(encode(p),{monotonicNow:()=>0,utcNow:()=>101,resources:()=>{calls++;}}));assert.equal(calls,0);assert.throws(()=>createFiniteGuard(encode(guardPolicy()),{}));});
test('bounded errors identify full-message digest and explicit truncation',()=>{const e=boundedError(Error('x'.repeat(20000)));assert.equal(e.message_bytes,20000);assert.equal(e.message_sha256,sha(Buffer.alloc(20000,120)));assert.equal(e.message_truncated,true);assert.ok(Buffer.byteLength(e.message)<=16386);});
test('writer publishes no-replace hard-link custody and retains temporary',()=>{
 const {result:r,m}=write();assert.equal(r.publication_accepted,true);assert.equal(r.final.linked,true);assert.equal(r.temporary.retained,true);assert.equal(r.no_automatic_unlink,true);assert.equal(r.descriptor.closed,true);assert.equal(r.directory_sync.closed,true);assert.equal(r.provider_calls,r.provider_call_cap);assert.equal(m.files.get('/new-output/observation.json'),m.files.get('/new-output/observation.pending'));assert.equal(r.two_named_output_reservations,400000);
});
test('writer exact64KiB chunks do not retry or exceed selected cap',()=>{const body=Buffer.alloc(65537,1),{result:r,m}=write(body);assert.equal(r.publication_accepted,true);assert.deepEqual(m.calls.filter(c=>c.name==='write').map(c=>c.args[3]),[65536,1]);assert.equal(r.requested_write_bytes,65537);assert.equal(r.returned_write_bytes,65537);});
test('raced existing final destination is not overwritten and temporary is retained',()=>{
 const other={body:Buffer.from('other'),ino:'99'};const {result:r,m}=write(Buffer.from('report'),{hook:(c,normal,s)=>{if(c.name==='linkExclusive'){s.files.set(c.args[1],other);return normal();}}});assert.equal(r.publication_accepted,false);assert.equal(r.first_error.code,'EEXIST');assert.equal(m.files.get('/new-output/observation.json'),other);assert.equal(r.temporary.retained,true);assert.equal(r.descriptor.closed,true);
});
test('pre-existing temporary destination refuses before any write',()=>{
 const {result:r,m}=write(Buffer.from('report'),{hook:(c,normal,s)=>{if(c.name==='openExclusive'){s.files.set(c.args[0],{body:Buffer.from('prior'),ino:'88'});return normal();}}});assert.equal(r.first_error.code,'EEXIST');assert.equal(r.temporary.created,false);assert.equal(m.counts.write??0,0);assert.equal(r.descriptor.close_attempts,0);
});
test('short write retains attempted debit and partial-prefix custody without retry',()=>{
 const {result:r,m}=write(Buffer.from('report'),{hook:(c,normal,s)=>{if(c.name==='write'){const n=2,f=s.fds.get(c.args[0]);f.body=Buffer.from(c.args[1].subarray(c.args[2],c.args[2]+n));return n;}}});assert.equal(r.publication_accepted,false);assert.match(r.first_error.message,/short write/);assert.equal(r.write_calls,1);assert.equal(r.requested_write_bytes,6);assert.equal(r.returned_write_bytes,2);assert.equal(r.temporary.submitted_prefix_sha256,sha(Buffer.from('re')));assert.equal(m.counts.linkExclusive??0,0);
});
test('writer guard expiry after owned open allows close but no fstat or write',()=>{
 let expired=false;const guard=()=>{if(expired)throw Error('expired');};const {result:r,m}=write(Buffer.from('report'),{guard,hook:(c,normal)=>{if(c.name==='openExclusive'){const fd=normal();expired=true;return fd;}}});assert.equal(r.publication_accepted,false);assert.equal(r.descriptor.closed,true);assert.equal(m.counts.close,1);assert.equal(m.counts.fstat??0,0);assert.equal(m.counts.write??0,0);assert.ok(r.provider_calls>r.provider_invocations);
});
test('writer first failure survives cleanup failure with live descriptor warning',()=>{
 const {result:r}=write(Buffer.from('report'),{hook:c=>{if(c.name==='write')throw Error('first write fault');if(c.name==='close')throw Error('later close fault');}});assert.match(r.first_error.message,/first write fault/);assert.equal(r.cleanup_errors.length,1);assert.equal(r.descriptor.live_fd_possible,true);assert.equal(r.temporary.partial_or_complete_name_may_remain,true);
});
test('failed directory descriptor cleanup is reported and forbids accepted publication',()=>{
 const {result:r}=write(Buffer.from('report'),{hook:c=>c.name==='fsyncDirectory'?{opened:true,synced:true,close_attempts:1,closed:false,live_fd_possible:true,first_error:boundedError(Error('directory close')),cleanup_errors:[boundedError(Error('directory close'))]}:undefined});assert.equal(r.final.linked,true);assert.equal(r.publication_accepted,false);assert.equal(r.directory_sync.live_fd_possible,true);assert.equal(r.descriptor.closed,true);
});
test('output directory replacement before commit refuses with retained temp',()=>{
 const {result:r,m}=write(Buffer.from('report'),{hook:c=>{if(c.name==='directory'&&c.index===2){const s=directoryStat();s.identity[1]='99';return s;}}});assert.equal(r.publication_accepted,false);assert.equal(m.counts.linkExclusive??0,0);assert.equal(r.temporary.created,true);
});
test('final named inode mismatch refuses after link and reports possible final custody',()=>{
 const {result:r}=write(Buffer.from('report'),{hook:(c,normal)=>{if(c.name==='lstat'){const s=normal();s.identity[1]='99';return s;}}});assert.equal(r.publication_accepted,false);assert.equal(r.final.linked,true);assert.match(r.first_error.message,/committed named/);
});
test('body cap refusal happens before any writer provider call',()=>{
 const m=memoryWriter(),s=writerSpec();s.cap_bytes=1;assert.throws(()=>publishExclusiveEvidence(Buffer.from('two'),encode(s),m.provider,{guard:()=>{}}));assert.equal(m.calls.length,0);
});
test('writer specification rejects aliases identical names and authority fields',()=>{
 for(const mutate of [s=>{s.directory='/new-output/../alias';},s=>{s.final_name=s.temporary_name;},s=>{s.approved=true;},s=>{s.cap_bytes=64*1024*1024+1;},s=>{s.temporary_name='../escape';}]){const s=writerSpec();mutate(s);assert.throws(()=>admitEvidenceSpec(encode(s)));}
});
test('already-denied writer guard creates no output and retains its attempted call',()=>{
 const {result:r,m}=write(Buffer.from('report'),{guard:()=>{throw Error('already expired');}});assert.equal(m.calls.length,0);assert.equal(r.provider_calls,1);assert.equal(r.provider_invocations,0);assert.equal(r.temporary.created,false);assert.equal(r.publication_accepted,false);
});

test('bounded evidence serializer preserves JSON values and newline with exact cap',()=>{const value={text:'hello',n:2,list:[null,true,false]},expected=Buffer.from(JSON.stringify(value)+'\n');const encoded=encodeBoundedEvidence(value,expected.length);assert.deepEqual(encoded,expected);assert.deepEqual(JSON.parse(encoded),value);});
test('one-byte serialization cap shortfall refuses before publication',()=>{const value={a:'abc'},bytes=Buffer.byteLength(JSON.stringify(value)+'\n');assert.throws(()=>encodeBoundedEvidence(value,bytes-1),/byte cap/);});
test('evidence serializer refuses cycles accessors and unsupported values',()=>{const cyclic={};cyclic.self=cyclic;assert.throws(()=>encodeBoundedEvidence(cyclic,1000),/cyclic/);let accessed=false;const access={get bad(){accessed=true;return 1;}};assert.throws(()=>encodeBoundedEvidence(access,1000),/accessors/);assert.equal(accessed,false);for(const value of [undefined,NaN,Infinity,1n,Buffer.from('data')])assert.throws(()=>encodeBoundedEvidence(value,1000));});
test('evidence serializer refuses excessive nesting independently of byte cap',()=>{let value=null;for(let n=0;n<66;n++)value=[value];assert.throws(()=>encodeBoundedEvidence(value,10000),/structural/);});
test('serializer accepts bounded shared acyclic data without alias elision',()=>{const shared={x:1},value=[shared,shared];assert.equal(encodeBoundedEvidence(value,100).toString(),JSON.stringify(value)+'\n');});

test('phase deadline remains primary when owned descriptor close also fails',()=>{
 let clock=0;const {record:r}=run(plan(),{now:()=>clock,hook:c=>{if(c.name==='open')clock=1000;if(c.name==='close')throw Error('secondary close');}});
 assert.equal(r.first_failure.source,'phase-clock');assert.equal(r.first_failure.code,'phase-clock-refused');assert.equal(r.first_failure.phase,'precheck');assert.match(r.phases[0].provider_observation.last_provider_error.error.message,/secondary close/);assert.equal(r.phases[0].failure.reader_accounting.live_fd_possible,true);
});
test('phase clock regression remains primary when cleanup fails',()=>{
 let clock=10;const {record:r}=run(plan(),{now:()=>clock,hook:c=>{if(c.name==='open')clock=9;if(c.name==='close')throw Error('secondary close');}});
 assert.equal(r.first_failure.source,'phase-clock');assert.equal(r.first_failure.observed_time,9);assert.equal(r.first_failure.previous_time,10);
});
test('invalid phase clock remains primary when cleanup fails',()=>{
 let clock=0;const {record:r}=run(plan(),{now:()=>clock,hook:c=>{if(c.name==='open')clock=NaN;if(c.name==='close')throw Error('secondary close');}});
 assert.equal(r.first_failure.source,'phase-clock');assert.equal(r.first_failure.observed_time,null);assert.equal(r.phases[0].provider_observation.counts.close_attempts,1);
});
test('directory sync guard refusal stays primary through directory and file cleanup errors',()=>{
 const primary=boundedError(Object.assign(Error('first directory guard'),{code:'ADAPTER_GUARD_REFUSED'})),secondary=boundedError(Error('directory close fault'));
 const {result:r}=write(Buffer.from('report'),{hook:c=>{if(c.name==='fsyncDirectory')return{opened:true,synced:false,close_attempts:1,closed:false,live_fd_possible:true,first_error:primary,cleanup_errors:[secondary]};if(c.name==='close')throw Error('file close fault');}});
 assert.deepEqual(r.first_error,primary);assert.equal(r.cleanup_errors.length,2);assert.equal(r.directory_sync.live_fd_possible,true);assert.equal(r.descriptor.live_fd_possible,true);assert.equal(r.publication_accepted,false);
});
test('scope expiry during successful close prevents publication acceptance',()=>{
 let expired=false;const {result:r,m}=write(Buffer.from('report'),{guard:()=>{if(expired)throw Error('expired after close');},hook:(c,normal)=>{if(c.name==='close'){const value=normal();expired=true;return value;}}});
 assert.equal(r.descriptor.closed,true);assert.equal(r.final.linked,true);assert.equal(r.post_cleanup_guard_checked,true);assert.equal(r.publication_accepted,false);assert.match(r.first_error.message,/expired after close/);assert.equal(m.counts.close,1);
});
function resourceFixture(hook=()=>undefined){
 let mono=0,utc=101,reads=0;const calls=[],record=Buffer.from('MemAvailable: 123 kB\n');
 const methods={statfs:()=>({bavail:100n,bsize:4n}),open:()=>40,read:(fd,b,o,n)=>{if(reads++===0){record.copy(b,o);return record.length;}return 0;},close:()=>true,rss:()=>20};
 const provider=Object.fromEntries(Object.entries(methods).map(([name,fn])=>[name,(...args)=>{calls.push(name);return hook(name,args,()=>fn(...args),()=>{mono=1000;})??fn(...args);}]));
 const probe=createBoundedResourceObserver({resource_root:'/resources',scope:guardPolicy(),started:0,monotonicNow:()=>mono,utcNow:()=>utc,provider});
 return{probe,calls,set:(m,u=101)=>{mono=m;utc=u;}};
}
test('resource domain separately reserves complete bounded record EOF and owned cleanup',()=>{
 const x=resourceFixture();assert.deepEqual(x.probe(),{rss_bytes:20,free_bytes:400,available_ram_bytes:125952});const s=x.probe.snapshot();assert.equal(s.requested_bytes,65537);assert.equal(s.content_calls,2);assert.equal(s.attempted.statfs,1);assert.equal(s.invoked.statfs,1);assert.equal(s.closed,1);assert.equal(s.first_failure,null);
});
test('early statfs fault retains attempted and invoked resource calls with no open',()=>{
 const x=resourceFixture(name=>{if(name==='statfs')throw Error('statfs failure');});assert.throws(x.probe,/statfs failure/);const s=x.probe.snapshot();assert.equal(s.attempted.statfs,1);assert.equal(s.invoked.statfs,1);assert.equal(s.attempted.open,0);assert.equal(s.first_failure.operation,'statfs');
});
test('resource open fault is retained without invented descriptor cleanup',()=>{
 const x=resourceFixture(name=>{if(name==='open')throw Error('open failure');});assert.throws(x.probe,/open failure/);const s=x.probe.snapshot();assert.equal(s.attempted.open,1);assert.equal(s.invoked.open,1);assert.equal(s.opened,0);assert.equal(s.close_attempts,0);assert.equal(s.first_failure.operation,'open');
});
test('resource read fault retains requested debit and stays primary over cleanup fault',()=>{
 const x=resourceFixture(name=>{if(name==='read')throw Error('first resource read');if(name==='close')throw Error('secondary resource close');});assert.throws(x.probe,/first resource read/);const s=x.probe.snapshot();assert.equal(s.requested_bytes,65536);assert.equal(s.content_calls,1);assert.equal(s.first_failure.operation,'read');assert.equal(s.cleanup_errors.length,1);assert.equal(s.live_fd_possible,true);
});
test('expiry before resource dispatch records attempted but not invoked statfs',()=>{
 const x=resourceFixture();x.set(1000);assert.throws(x.probe,/deadline/);const s=x.probe.snapshot();assert.equal(s.attempted.statfs,1);assert.equal(s.invoked.statfs,0);assert.equal(s.probes,1);assert.equal(s.first_failure.operation,'statfs');assert.equal(x.calls.length,0);
});
test('expiry during resource cleanup is observed after unconditional close',()=>{
 const x=resourceFixture((name,args,normal,expire)=>{if(name==='close'){const value=normal();expire();return value;}});assert.throws(x.probe,/deadline/);const s=x.probe.snapshot();assert.equal(s.closed,1);assert.equal(s.first_failure.operation,'post-cleanup-currentness');assert.equal(s.live_fd_possible,false);
});

test('full initial named and descriptor ownership mismatch stays primary through close failure',()=>{
 const {record:r}=run(plan(),{hook:(c,normal)=>{if(c.name==='fstat'&&c.index===1){const s=normal();s.uid='2';return s;}if(c.name==='close')throw Error('secondary admission close');}});
 assert.equal(r.first_failure.code,'named-descriptor-admission-mismatch');assert.equal(r.first_failure.named_target_stat.uid,'1');assert.equal(r.first_failure.descriptor_stat.uid,'2');assert.equal(r.first_failure.entry_path,'/fixture/a');assert.match(r.phases[0].provider_observation.last_provider_error.error.message,/secondary admission close/);
});
test('unselected alias target full admission mismatch stays primary through close failure',()=>{
 const a=file('/fixture/alias');a.resolved='/fixture/unselected-target';const aliases=[{pin:a.pin,resolved:a.resolved,link_text:'unselected-target',target_pin:{...a.pin,path:a.resolved},target_selected:false}],p=plan([a],[a],aliases);
 const {record:r}=run(p,{bodies:new Map([[a.resolved,Buffer.from('abcd')]]),hook:(c,normal)=>{if(c.name==='fstat'&&c.index===1){const s=normal();s.gid='2';return s;}if(c.name==='close')throw Error('secondary alias close');}});
 assert.equal(r.first_failure.code,'named-descriptor-admission-mismatch');assert.equal(r.first_failure.entry_path,a.path);assert.equal(r.first_failure.named_path,a.resolved);assert.equal(r.first_failure.named_target_stat.gid,'1');assert.equal(r.first_failure.descriptor_stat.gid,'2');assert.equal(r.phases[0].provider_observation.counts.close_attempts,1);
});

test('resource provider throw null is retained and terminal without additional calls',()=>{
 const x=resourceFixture(name=>{if(name==='statfs')throw null;});
 for(let n=0;n<2;n++){let caught=false;try{x.probe();}catch(e){caught=true;assert.equal(e,null);}assert.equal(caught,true);}
 const s=x.probe.snapshot();assert.equal(s.first_failure.operation,'statfs');assert.equal(s.first_failure.error.message,'null');assert.equal(s.probes,1);assert.equal(s.attempted.statfs,1);assert.equal(s.invoked.statfs,1);assert.deepEqual(x.calls,['statfs']);
});

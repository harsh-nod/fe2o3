// Injected exclusive writer. Importing does no IO. Temporary custody is retained, never raced-unlinked.
import {createHash} from 'node:crypto';
import {isDeepStrictEqual as equal} from 'node:util';
import {boundedError} from './adapter-guard.mjs';
const check=(v,m)=>{if(!v)throw Error('adapter evidence writer: '+m);};
const freeze=v=>{if(v&&typeof v==='object'){for(const x of Object.values(v))freeze(x);Object.freeze(v);}return v;};
const sha=b=>createHash('sha256').update(b).digest('hex');
const core=s=>({kind:s.kind,dev:s.identity[0],ino:s.identity[1],mode:s.identity[3],uid:s.uid,gid:s.gid});
function stat(s,kind){check(s&&s.kind===kind&&Array.isArray(s.identity)&&s.identity.length===6&&s.identity.every(x=>typeof x==='string'&&/^(0|[1-9][0-9]{0,24})$/.test(x))&&[s.uid,s.gid].every(x=>typeof x==='string'&&/^(0|[1-9][0-9]{0,24})$/.test(x))&&(BigInt(s.identity[3])&0o170000n)==={file:0o100000n,directory:0o040000n}[kind],'stat shape');return JSON.parse(JSON.stringify(s));}
export function admitEvidenceSpec(specBytes,bodyBytes=0){
 check(Number.isSafeInteger(bodyBytes)&&bodyBytes>=0&&Buffer.isBuffer(specBytes)&&specBytes.length>0&&specBytes.length<=16384,'bounded specification Buffer');
 const text=specBytes.toString('utf8');check(Buffer.from(text).equals(specBytes),'UTF8');const s=JSON.parse(text);
 check(s&&typeof s==='object'&&!Array.isArray(s)&&equal(Object.keys(s).sort(),['schema','directory','directory_identity','temporary_name','final_name','cap_bytes'].sort()),'closed specification');
 check(s.schema==='fe2o3-exclusive-adapter-evidence-v1','schema');
 check(typeof s.directory==='string'&&s.directory.length<=4096&&s.directory.startsWith('/')&&s.directory!=='/'&&!/[\\\x00-\x1f\x7f]/.test(s.directory)&&!s.directory.split('/').slice(1).some(x=>!x||x==='.'||x==='..'),'literal directory');
 check(Array.isArray(s.directory_identity)&&s.directory_identity.length===6&&s.directory_identity.every(x=>typeof x==='string'&&/^(0|[1-9][0-9]{0,24})$/.test(x)),'explicit directory identity');
 for(const name of [s.temporary_name,s.final_name])check(typeof name==='string'&&/^[A-Za-z0-9][A-Za-z0-9._-]{0,95}$/.test(name)&&name!=='.'&&name!=='..','literal new leaf');
 check(s.temporary_name!==s.final_name,'separate names');check(Number.isSafeInteger(s.cap_bytes)&&s.cap_bytes>0&&s.cap_bytes<=64*1024*1024&&bodyBytes<=s.cap_bytes,'body cap');
 return freeze(s);
}
export function encodeBoundedEvidence(value,capBytes){
 check(Number.isSafeInteger(capBytes)&&capBytes>0&&capBytes<=64*1024*1024,'serialization cap');
 const output=Buffer.allocUnsafe(capBytes),ancestors=new Set();let used=0,nodes=0;
 const emit=text=>{const n=Buffer.byteLength(text);check(used<=capBytes-n,'serialized evidence byte cap');output.write(text,used,n,'utf8');used+=n;};
 function walk(v,depth){
  check(++nodes<=2000000&&depth<=64,'serialization structural bounds');
  if(v===null){emit('null');return;}
  if(typeof v==='string'){check(v.length<=capBytes,'string bound');emit(JSON.stringify(v));return;}
  if(typeof v==='boolean'){emit(v?'true':'false');return;}
  if(typeof v==='number'){check(Number.isFinite(v),'finite JSON number');emit(JSON.stringify(v));return;}
  check(v&&typeof v==='object'&&(Array.isArray(v)||Object.getPrototypeOf(v)===Object.prototype||Object.getPrototypeOf(v)===null),'plain JSON data only');
  check(!ancestors.has(v),'cyclic evidence');ancestors.add(v);
  if(Array.isArray(v)){check(v.length<=2000000,'array bound');emit('[');for(let i=0;i<v.length;i++){if(i)emit(',');const descriptor=Object.getOwnPropertyDescriptor(v,String(i));check(descriptor&&Object.hasOwn(descriptor,'value'),'no evidence array accessors or holes');walk(descriptor.value,depth+1);}emit(']');}
  else{const names=Object.keys(v);check(names.length<=2000000,'object bound');emit('{');for(let i=0;i<names.length;i++){const key=names[i],descriptor=Object.getOwnPropertyDescriptor(v,key);check(descriptor&&Object.hasOwn(descriptor,'value'),'no evidence accessors');check(key.length<=capBytes,'key bound');if(i)emit(',');emit(JSON.stringify(key));emit(':');walk(descriptor.value,depth+1);}emit('}');}
  ancestors.delete(v);
 }
 walk(value,0);emit('\n');return Buffer.from(output.subarray(0,used));
}
export function publishExclusiveEvidence(bytes,specBytes,provider,{guard}){
 check(Buffer.isBuffer(bytes),'complete body Buffer');const s=admitEvidenceSpec(specBytes,bytes.length);
 check(typeof guard==='function','explicit finite guard');for(const n of ['directory','openExclusive','fstat','write','fsync','close','linkExclusive','lstat','fsyncDirectory'])check(typeof provider?.[n]==='function','writer provider '+n);
 const body=Buffer.from(bytes),temporary=s.directory+'/'+s.temporary_name,final=s.directory+'/'+s.final_name;
 const result={schema:'fe2o3-exclusive-adapter-evidence-observation-v1',status:'failed',first_error:null,cleanup_errors:[],submitted_bytes:body.length,submitted_sha256:sha(body),
  requested_write_bytes:0,returned_write_bytes:0,write_calls:0,provider_calls:0,provider_invocations:0,provider_call_cap:Math.ceil(body.length/65536)+12,
  temporary:{path:temporary,created:false,retained:false,last_observed_stat:null},final:{path:final,linked:false,last_observed_stat:null},
  descriptor:{opened:false,close_attempts:0,closed:false,live_fd_possible:false},directory_synced:false,directory_sync:null,
  two_named_output_reservations:s.cap_bytes*2,atomic_no_replace_commit:true,existing_destination_overwritten:false,
  content_reread:false,writer_exclusion_proved:false,publication_accepted:false};
 let fd=null,beforeDirectory=null,lastFile=null;
 const call=(method,...args)=>{check(result.provider_calls<result.provider_call_cap-1,'provider call cap with close reservation');result.provider_calls++;guard();result.provider_invocations++;return provider[method](...args);};
 try{
  beforeDirectory=stat(call('directory',s.directory),'directory');check(equal(beforeDirectory.identity,s.directory_identity),'bound output directory changed');
  fd=call('openExclusive',temporary);check(Number.isSafeInteger(fd)&&fd>=0,'descriptor');result.descriptor.opened=true;result.descriptor.live_fd_possible=true;result.temporary.created=true;result.temporary.retained=true;
  const first=stat(call('fstat',fd),'file');check(first.identity[2]==='0','exclusive new file starts empty');result.temporary.last_observed_stat=first;
  for(let at=0;at<body.length;){
   const take=Math.min(65536,body.length-at);check(result.requested_write_bytes<=s.cap_bytes-take,'write byte cap');
   result.requested_write_bytes+=take;result.write_calls++;
   // One exact write or refusal, never a retry that could hide a short write.
   const n=call('write',fd,body,at,take);check(Number.isSafeInteger(n)&&n>=0&&n<=take,'write result');
   result.returned_write_bytes+=n;check(n===take,'short write without retry');at+=take;
  }
  call('fsync',fd);lastFile=stat(call('fstat',fd),'file');check(equal(core(lastFile),core(first))&&lastFile.identity[2]===String(body.length),'descriptor changed during write');
  result.temporary.last_observed_stat=lastFile;
  check(equal(core(stat(call('directory',s.directory),'directory')),core(beforeDirectory)),'output directory replaced');
  call('linkExclusive',temporary,final);result.final.linked=true;
  const descriptor=stat(call('fstat',fd),'file'),named=stat(call('lstat',final),'file');
  check(equal(descriptor,named)&&equal(core(descriptor),core(first))&&descriptor.identity[2]===String(body.length),'committed named/descriptor identity');
  result.final.last_observed_stat=named;result.temporary.last_observed_stat=descriptor;
  const sync=call('fsyncDirectory',s.directory);check(sync&&equal(Object.keys(sync).sort(),['opened','synced','close_attempts','closed','live_fd_possible','first_error','cleanup_errors'].sort()),'directory sync observation');result.directory_sync=JSON.parse(JSON.stringify(sync));if(!(sync.opened===true&&sync.synced===true&&sync.close_attempts===1&&sync.closed===true&&sync.live_fd_possible===false&&sync.first_error===null&&Array.isArray(sync.cleanup_errors)&&sync.cleanup_errors.length===0)){result.first_error=sync.first_error??boundedError(Error('directory sync/cleanup failed'));if(Array.isArray(sync.cleanup_errors))result.cleanup_errors.push(...sync.cleanup_errors);throw Error('directory sync/cleanup failed');}result.directory_synced=true;
  check(equal(core(stat(call('directory',s.directory),'directory')),core(beforeDirectory)),'final output directory replaced');
  guard();result.status='published-observed';
 }catch(e){if(result.first_error===null)result.first_error=boundedError(e);}
 finally{
  if(fd!==null){
   // Already-owned descriptor cleanup only; no expired guard can authorize another read/write.
   result.provider_calls++;result.descriptor.close_attempts++;
   try{result.provider_invocations++;provider.close(fd);result.descriptor.closed=true;result.descriptor.live_fd_possible=false;}catch(e){const error=boundedError(e);result.cleanup_errors.push(error);if(!result.first_error)result.first_error=error;result.status='failed';}
  }
 }
 result.post_cleanup_guard_checked=false;
 if(result.status==='published-observed'&&result.first_error===null&&result.descriptor.closed){result.post_cleanup_guard_checked=true;try{guard();}catch(e){result.first_error=boundedError(e);result.status='failed';}}
 if(result.status==='published-observed'&&result.first_error===null&&result.descriptor.closed)result.publication_accepted=true;
 else result.status='failed';
 result.temporary.submitted_prefix_bytes=result.returned_write_bytes;result.temporary.submitted_prefix_sha256=sha(body.subarray(0,result.returned_write_bytes));
 result.temporary.partial_or_complete_name_may_remain=result.temporary.created;
 result.output_directory_bracketing_only=true;result.atomic_ancestor_path_exclusion=false;result.displaced_name_custody_possible=result.temporary.created;result.root_private_output_directory_policy_required=true;
 result.counter_semantics='provider_calls and write_calls retain attempted reservations; provider_invocations counts actual calls';result.no_automatic_unlink=true;result.external_kill_can_prevent_reporting=true;
 return freeze(result);
}

// Exact named request bootstrap only. No IO on import or construction.
import {createHash} from 'node:crypto';
import {isDeepStrictEqual as equal} from 'node:util';
import {boundedError} from '../loaded-cpu-adapter/adapter-guard.mjs';
import {admitRequestSpec} from './launch-model.mjs';
const frozen=v=>{if(v&&typeof v==='object'){for(const x of Object.values(v))frozen(x);Object.freeze(v);}return v;};
const digest=b=>createHash('sha256').update(b).digest('hex');
const check=(v,m)=>{if(!v)throw Error('static Node request reader: '+m);};
function stat(v){
 check(v&&typeof v==='object'&&equal(Object.keys(v).sort(),['kind','identity','uid','gid'].sort()),'closed stat');
 check(v.kind==='file'&&Array.isArray(v.identity)&&v.identity.length===6,'regular stat');
 for(const n of [...v.identity,v.uid,v.gid])check(typeof n==='string'&&/^(0|[1-9][0-9]{0,24})$/.test(n),'decimal stat');
 check((BigInt(v.identity[3])&0o170000n)===0o100000n,'full regular mode');
 return JSON.parse(JSON.stringify(v));
}
export function readPinnedRequest(specBytes,provider,{guard}){
 const s=admitRequestSpec(specBytes);
 check(provider&&typeof guard==='function','explicit provider and guard');
 for(const k of ['lstat','realpath','open','fstat','read','close'])check(typeof provider[k]==='function','provider '+k);
 const contentCap=Math.ceil(s.pin.bytes/65536)+1,metadataCap=10;
 const r={schema:'fe2o3-static-node-request-read-v1',status:'failed',expected:s,
  ceiling:{payload_bytes:s.pin.bytes,reserved_bytes:s.pin.bytes+1,content_calls:contentCap,metadata_calls:metadataCap,chunk_bytes:65536,eof_bytes:1},
  counts:{attempted_content_bytes:0,returned_content_bytes:0,content_calls:0,metadata_calls:0,provider_invocations:0,opened:0,close_attempts:0,closed:0},
  descriptor:{live_fd_possible:false,invalid_open_result:false},first_failure:null,cleanup_errors:[],last_operation:null,
  qualified:false,execution_authority:false,global_io_bound_proved:false,external_kill_can_prevent_reporting:true};
 let failed=false,fd=null,body=null;
 const remember=(e,operation)=>{const error={operation,error:boundedError(e)};if(!failed){failed=true;r.first_failure=error;}return error;};
 function call(method,...args){
  r.last_operation=method;
  if(method==='read'){check(r.counts.content_calls<contentCap,'content-call ceiling');r.counts.content_calls++;check(r.counts.attempted_content_bytes<=s.pin.bytes+1-args[3],'inclusive byte ceiling');r.counts.attempted_content_bytes+=args[3];}
  else{check(r.counts.metadata_calls<metadataCap-1,'metadata ceiling with close reservation');r.counts.metadata_calls++;}
  guard();r.counts.provider_invocations++;return provider[method](...args);
 }
 const wanted={kind:'file',identity:s.identity,uid:s.ownership.uid,gid:s.ownership.gid};
 try{
  guard();body=Buffer.alloc(s.pin.bytes);
  check(equal(stat(call('lstat',s.pin.path)),wanted),'named admission identity/owner');
  check(call('realpath',s.pin.path)===s.pin.path,'canonical request name');
  const opened=call('open',s.pin.path);
  if(!Number.isSafeInteger(opened)||opened<0){r.descriptor.invalid_open_result=true;throw Error('static Node request reader: invalid open descriptor');}
  fd=opened;r.counts.opened++;r.descriptor.live_fd_possible=true;
  check(equal(stat(call('fstat',fd)),wanted),'initial named/descriptor identity/owner');
  for(let offset=0;offset<body.length;){
   const n=Math.min(65536,body.length-offset),got=call('read',fd,body,offset,n);
   check(Number.isInteger(got)&&got>=0&&got<=n,'read result');r.counts.returned_content_bytes+=got;
   check(got===n,'short read without retry');offset+=n;
  }
  const eof=call('read',fd,Buffer.alloc(1),0,1);
  check(Number.isInteger(eof)&&eof>=0&&eof<=1,'EOF result');r.counts.returned_content_bytes+=eof;check(eof===0,'EOF growth');
  check(digest(body)===s.pin.sha256,'whole request hash');
  check(equal(stat(call('fstat',fd)),wanted),'descriptor drift');
  check(equal(stat(call('lstat',s.pin.path)),wanted),'named drift');
  check(call('realpath',s.pin.path)===s.pin.path,'resolution drift');
 }catch(e){remember(e,r.last_operation??'bootstrap-guard-or-allocation');}
 finally{
  if(fd!==null){
   r.last_operation='close';r.counts.metadata_calls++;r.counts.close_attempts++;
   try{r.counts.provider_invocations++;provider.close(fd);r.counts.closed++;r.descriptor.live_fd_possible=false;check(r.counts.metadata_calls<=metadataCap,'cleanup metadata ceiling');}
   catch(e){const error=remember(e,'close');r.cleanup_errors.push(error);}
  }
 }
 if(!failed){
  try{
   // No remaining owned close is needed; the two final named operations use the full ceiling.
   for(const method of ['lstat','realpath']){
    r.last_operation=method;check(r.counts.metadata_calls<metadataCap,'final metadata ceiling');r.counts.metadata_calls++;guard();r.counts.provider_invocations++;
    const value=provider[method](s.pin.path);
    check(method==='lstat'?equal(stat(value),wanted):value===s.pin.path,'post-close named identity/resolution');
   }
   guard();r.status='complete-request-observed';
  }catch(e){remember(e,r.last_operation??'post-close-guard');}
 }
 r.counters_are_attempted_reservations=true;r.cleanup_only_after_denial=true;
 // A complete private Buffer is available only after whole hash, EOF, cleanup and final brackets.
 return Object.freeze({bytes:failed?null:Buffer.from(body),observation:frozen(r)});
}

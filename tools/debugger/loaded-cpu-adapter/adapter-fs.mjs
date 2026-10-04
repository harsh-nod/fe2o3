// Explicit CPU filesystem binding. Importing or constructing providers performs no IO and starts no child.
import fs from 'node:fs';
import {performance} from 'node:perf_hooks';
import {createHash} from 'node:crypto';
import {filesystemProvider} from '../loaded-input-reader/reader-fs.mjs';
import {executeAdapterPlan} from './adapter-protocol.mjs';
import {createFiniteGuard,boundedError,createBoundedResourceObserver} from './adapter-guard.mjs';
import {publishExclusiveEvidence,admitEvidenceSpec,encodeBoundedEvidence} from './adapter-writer.mjs';
const check=(v,m)=>{if(!v)throw Error('CPU adapter filesystem binding: '+m);};
const fields=['dev','ino','size','mode','mtimeNs','ctimeNs'];
const stat=s=>({kind:s.isFile()?'file':s.isDirectory()?'directory':s.isSymbolicLink()?'symlink':'other',identity:fields.map(k=>String(s[k])),uid:String(s.uid),gid:String(s.gid)});
function canonicalName(p){check(typeof p==='string'&&Buffer.byteLength(p)<=4096&&p.startsWith('/')&&p!=='/'&&!/[\\\x00-\x1f\x7f]/.test(p)&&!p.split('/').slice(1).some(x=>!x||x==='.'||x==='..'),'explicit canonical path');return p;}
export function filesystemEvidenceProvider({guard}){
 check(typeof guard==='function','explicit writer guard');
 return Object.freeze({
  directory:p=>{guard();const s=fs.lstatSync(p,{bigint:true});check(s.isDirectory(),'output directory type');guard();check(fs.realpathSync(p)===p,'canonical output directory');return stat(s);},
  openExclusive:p=>fs.openSync(p,fs.constants.O_WRONLY|fs.constants.O_CREAT|fs.constants.O_EXCL|fs.constants.O_NOFOLLOW|fs.constants.O_NONBLOCK,0o600),
  fstat:fd=>stat(fs.fstatSync(fd,{bigint:true})),
  write:(fd,body,offset,length)=>fs.writeSync(fd,body,offset,length,null),
  fsync:fd=>fs.fsyncSync(fd),
  close:fd=>fs.closeSync(fd),
  // linkSync atomically refuses an existing destination; ordinary overwrite-capable rename is never used.
  linkExclusive:(temporary,final)=>fs.linkSync(temporary,final),
  lstat:p=>stat(fs.lstatSync(p,{bigint:true})),
  fsyncDirectory:p=>{
   const r={opened:false,synced:false,close_attempts:0,closed:false,live_fd_possible:false,first_error:null,cleanup_errors:[]};let fd=null;
   try{guard();fd=fs.openSync(p,fs.constants.O_RDONLY|fs.constants.O_DIRECTORY|fs.constants.O_NOFOLLOW|fs.constants.O_NONBLOCK);r.opened=true;r.live_fd_possible=true;guard();fs.fsyncSync(fd);r.synced=true;}
   catch(e){r.first_error=boundedError(e);}finally{if(fd!==null){r.close_attempts++;try{fs.closeSync(fd);r.closed=true;r.live_fd_possible=false;}catch(e){const error=boundedError(e);r.cleanup_errors.push(error);if(r.first_error===null)r.first_error=error;}}}return r;
  },
 });
}
/**
 * Root supplies a complete plan Buffer, already-bound finite policy and explicit new output names.
 * This entry never reads an implicit request path, renews a scope, changes a cap, or launches a child.
 * No in-memory report is promised after process death; root must inspect actual durable evidence.
 */
export function runFilesystemAdapter(planBytes,guardPolicyBytes,writerSpecBytes,{resource_root}){
 check(Buffer.isBuffer(planBytes)&&Buffer.isBuffer(guardPolicyBytes)&&Buffer.isBuffer(writerSpecBytes),'explicit complete Buffers');
 check(guardPolicyBytes.length>0&&guardPolicyBytes.length<=16384,'bounded guard policy');const writerSpec=admitEvidenceSpec(writerSpecBytes,0);
 canonicalName(resource_root);const started=performance.now();
 // The policy is admitted by createFiniteGuard before any resource callback is invoked.
 let scope;try{scope=JSON.parse(guardPolicyBytes.toString('utf8'));}catch(e){throw Error('CPU adapter filesystem binding: policy JSON');}
 const resourceProbe=createBoundedResourceObserver({resource_root,scope,started,monotonicNow:()=>performance.now(),utcNow:()=>Date.now(),provider:{
  statfs:p=>fs.statfsSync(p,{bigint:true}),
  open:p=>fs.openSync(p,fs.constants.O_RDONLY|fs.constants.O_NOFOLLOW|fs.constants.O_NONBLOCK),
  read:(fd,b,offset,length)=>fs.readSync(fd,b,offset,length,null),
  close:fd=>fs.closeSync(fd),
  rss:()=>process.memoryUsage().rss,
 }});
 const guard=createFiniteGuard(guardPolicyBytes,{monotonicNow:()=>performance.now(),utcNow:()=>Date.now(),resources:resourceProbe});
 let record;
 try{record=executeAdapterPlan(planBytes,{provider:filesystemProvider(),guard,now:()=>performance.now()});}
 catch(e){
  // Admission failure occurs before any operational provider call. No output IO is attempted.
  return Object.freeze({status:'admission-refused',record:null,publication:null,first_failure:{source:'plan-admission',error:boundedError(e)},summary:{schema:'fe2o3-cpu-adapter-command-result-v1',status:'admission-refused',error:boundedError(e),durable_evidence_available:false,in_memory_result_requires_process_survival:true}});
 }
 let body=null,publication;
 try{body=encodeBoundedEvidence({reader_record:record,resource_probe_observation:resourceProbe.snapshot(),publication_state:'intent only; consult actual command-result and root readback'},writerSpec.cap_bytes);publication=publishExclusiveEvidence(body,writerSpecBytes,filesystemEvidenceProvider({guard}),{guard});}
 catch(e){publication={status:'admission-refused',first_error:boundedError(e),publication_accepted:false,temporary:null,final:null};}
 const guard_observation_after_publication=guard.snapshot();
 const first_failure=record.first_failure??(publication.publication_accepted?null:{source:'evidence-publication',error:publication.first_error});
 const status=record.status==='read-protocol-completed'&&publication.publication_accepted?'read-and-publication-observed':'failed';
 const summary={schema:'fe2o3-cpu-adapter-command-result-v1',status,first_failure,phase_states:record.phases.map(p=>({name:p.name,status:p.status,budget:p.budget})),shared_accounting:record.shared_accounting,
  publication,guard_observation_after_publication,resource_probe_observation:resourceProbe.snapshot(),durable_evidence_available:publication.publication_accepted,
  evidence_pin:publication.publication_accepted?{path:publication.final.path,bytes:body.length,sha256:createHash('sha256').update(body).digest('hex')}:null,
  qualified:false,native_authority:false,child_processes_started:0,in_memory_result_requires_process_survival:true,external_timeout_or_kill_can_prevent_reporting:true};
 return Object.freeze({status,record,publication,first_failure,guard_observation_after_publication,summary});
}

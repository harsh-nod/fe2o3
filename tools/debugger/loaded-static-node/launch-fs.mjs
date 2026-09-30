// Explicit filesystem binding for a single static CPU adapter dispatch. No IO on import.
import fs from 'node:fs';
import {performance} from 'node:perf_hooks';
import {isDeepStrictEqual as equal} from 'node:util';
import {fileURLToPath} from 'node:url';
import {admitLaunchBootstrap} from './launch-model.mjs';
import {executeStaticLaunch,deferLaunchGuard} from './launch-run.mjs';
import {staticModuleContext} from './launch-module-map.mjs';
import {filesystemProvider} from '../loaded-input-reader/reader-fs.mjs';
import {runFilesystemAdapter} from '../loaded-cpu-adapter/adapter-fs.mjs';
import {createFiniteGuard,createBoundedResourceObserver} from '../loaded-cpu-adapter/adapter-guard.mjs';
export const STATIC_ENVIRONMENT=Object.freeze({LANG:'C',LC_ALL:'C',PATH:'/usr/bin:/bin'});
const check=(v,m)=>{if(!v)throw Error('static Node filesystem launch: '+m);};
const fields=['dev','ino','size','mode','mtimeNs','ctimeNs'];
const endpoint=s=>({kind:s.isFile()?'file':s.isFIFO()?'fifo':'other',identity:fields.map(k=>String(s[k])),ownership:{uid:String(s.uid),gid:String(s.gid)}});
const core=s=>({kind:s.kind,dev:s.identity[0],ino:s.identity[1],mode:s.identity[3],ownership:s.ownership});
function guardFor(raw,resourceRoot){
 const scope=JSON.parse(raw.toString('utf8')),started=performance.now();
 const resources=createBoundedResourceObserver({resource_root:resourceRoot,scope,started,monotonicNow:()=>performance.now(),utcNow:()=>Date.now(),provider:{
  statfs:p=>fs.statfsSync(p,{bigint:true}),
  open:p=>fs.openSync(p,fs.constants.O_RDONLY|fs.constants.O_NOFOLLOW|fs.constants.O_NONBLOCK),
  read:(fd,b,offset,length)=>fs.readSync(fd,b,offset,length,null),
  close:fd=>fs.closeSync(fd),
  rss:()=>process.memoryUsage().rss,
 }});
 const guard=createFiniteGuard(raw,{monotonicNow:()=>performance.now(),utcNow:()=>Date.now(),resources});
 guard.resourceSnapshot=resources.snapshot;return guard;
}
export function runStaticNodeFilesystem(bootstrapBytes){
 const admitted=admitLaunchBootstrap(bootstrapBytes),b=admitted.binding;
 const entry=fileURLToPath(new URL('./launch-main.mjs',import.meta.url));
 // These checks observe launch configuration; they cannot undo pre-entry loader effects.
 check(process.execPath===b.runtime_pin.path&&entry===b.entry_path&&process.argv[1]===entry,'fixed actual runtime and entry');
 check(process.execArgv.length===0&&process.argv.length===3,'no mutable Node flags or extra arguments');
 check(equal({...process.env},STATIC_ENVIRONMENT),'explicit cleared environment; no fallback or loader variables');
 const bootstrapGuard=guardFor(admitted.bootstrap_policy_bytes,b.resource_root),reportGuard=deferLaunchGuard(()=>guardFor(admitted.report_policy_bytes,b.resource_root));
 const summarySink={
  before:spec=>{
   const observed=endpoint(fs.fstatSync(1,{bigint:true}));
   check(equal(observed,{kind:spec.kind,identity:spec.identity,ownership:spec.ownership}),'inherited stdout identity');
   return observed;
  },
  write:(body,offset,length)=>fs.writeSync(1,body,offset,length,null),
  after:(spec,bytes)=>{
   const observed=endpoint(fs.fstatSync(1,{bigint:true}));
   check(equal(core(observed),core(spec)),'inherited stdout identity after write');
   if(spec.kind==='file')check(observed.identity[2]===String(bytes),'complete regular stdout size');
   return observed;
  },
 };
 const result=executeStaticLaunch(bootstrapBytes,{requestProvider:filesystemProvider(),bootstrapGuard,reportGuard,
  context:staticModuleContext(b.runtime_pin.path),invokeAdapter:runFilesystemAdapter,summarySink});
 return {...result,resource_observations_after_return:{bootstrap:bootstrapGuard.resourceSnapshot(),report:reportGuard.resourceSnapshot()},
  guard_observations_after_return:{bootstrap:bootstrapGuard.snapshot(),report:reportGuard.snapshot()},
  output_descriptors_owned_by_outer_launcher:true,output_fsync_or_close_performed_here:false,
  static_loader_already_ran_before_entry:true,loader_environment_verified_before_execution:false};
}

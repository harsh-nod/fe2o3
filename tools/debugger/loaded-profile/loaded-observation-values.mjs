// Pure retained observation semantics; filesystem half deliberately excluded.
import crypto from 'node:crypto';
import {isDeepStrictEqual as eq} from 'node:util';
import {PROFILE} from './loaded-profile-binding.mjs';
const {DEBUGGER,DATA,PYTHON,PACKAGE}=PROFILE.roots;
const requireArtifactReady=()=>PROFILE.artifact;
const validateArtifactContract=a=>{if(!eq(a,PROFILE.artifact))throw Error('loaded review: exact source-bound artifact DATA');return a;};
export const MAX_FILE=256*1024*1024, MAX_TOTAL=1024*1024*1024;
const fail=s=>{throw Error('startup observation: '+s)};
const sha=b=>crypto.createHash('sha256').update(b).digest('hex');
function object(v){if(!v||typeof v!=='object'||Array.isArray(v))fail('object');return v}
function keys(v,names){object(v);if(!eq(Object.keys(v).sort(),names.slice().sort()))fail('closed keys')}
function array(v,n){if(!Array.isArray(v)||v.length>n)fail('array bound');return v}
function string(v,n=4096){if(typeof v!=='string'||v.length>n||v.includes('\0'))fail('string bound');return v}
function integer(v,lo=0,hi=Number.MAX_SAFE_INTEGER){if(!Number.isSafeInteger(v)||v<lo||v>hi)fail('integer');return v}
function decimal(v){if(typeof v!=='string'||!/^(0|[1-9][0-9]{0,24})$/.test(v))fail('decimal');return BigInt(v)}
function nullable(v,n){return v===null?null:string(v,n)}
export function installedPath(path){
 string(path);if(!path.startsWith('/')||path.split('/').some(s=>s==='..'||s==='.')||path.includes('\\')||path.includes(' (deleted)'))fail('path shape');
 if(!['/usr/lib/','/usr/local/lib/','/opt/rocm-7.2.1/'].some(p=>path.startsWith(p))&&path!=='/etc/python3.12/sitecustomize.py'&&path!==DEBUGGER&&!path.startsWith(PYTHON+'/'))fail('installed path roots');
 return path;
}
export function parseMaps(text){
 string(text,1048576);const lines=text.split('\n');if(lines.pop()!==''||lines.length>4096)fail('maps count/termination');
 return lines.map(line=>{
  const m=/^([0-9a-f]+)-([0-9a-f]+) ([r-][w-][x-][ps]) ([0-9a-f]+) ([0-9a-f]+):([0-9a-f]+) ([0-9]+)(?: +(\S.*)| *)$/.exec(line);
  if(!m||BigInt('0x'+m[1])>=BigInt('0x'+m[2]))fail('map syntax/range');
  const path=m[8]??null;if(path!==null&&!path.startsWith('/')&&!/^\[[^\]\n]{1,256}\]$/.test(path))fail('map pseudo path');
  const major=BigInt('0x'+m[5]),minor=BigInt('0x'+m[6]);
  const dev=(minor&255n)|((major&4095n)<<8n)|((minor&~255n)<<12n)|((major&~4095n)<<32n);
  return{range:[m[1],m[2]],permissions:m[3],offset:m[4],dev:String(dev),ino:String(BigInt(m[7])),path};
 });
}
function modules(rows){
 const names=new Set();
 for(const row of array(rows,1024)){
  object(row);string(row.name,256);if(names.has(row.name))fail('duplicate module');names.add(row.name);
  if(row.kind==='none'){keys(row,['name','kind']);continue}
  if(row.kind==='nonmodule'){keys(row,['name','kind','object_type']);string(row.object_type,256);continue}
  if(row.kind!=='module')fail('module kind');
  keys(row,['name','kind','file','cached','origin','loader_type','package_paths']);
  for(const key of ['file','cached','origin'])nullable(row[key],4096);
  nullable(row.loader_type,256);
  if(row.package_paths!==null)for(const p of array(row.package_paths,64))string(p);
 }
 return names;
}
function files(rows){
 const paths=new Map();let total=0;
 for(const row of array(rows,1024)){
  object(row);installedPath(row.path);installedPath(row.realpath);
  if(paths.has(row.path))fail('duplicate observed file');paths.set(row.path,row);
  if(row.exists===false){keys(row,['path','realpath','exists']);continue}
  if(row.exists!==true)fail('file existence');
  keys(row,['path','realpath','exists','bytes','sha256','dev','ino','mode','uid','gid','mtime_ns','ctime_ns','elf']);
  integer(row.bytes,0,MAX_FILE);total+=row.bytes;if(total>MAX_TOTAL)fail('aggregate byte bound');
  if(typeof row.sha256!=='string'||!/^[0-9a-f]{64}$/.test(row.sha256)||typeof row.elf!=='boolean')fail('file hash/ELF');
  for(const key of ['dev','ino','mtime_ns','ctime_ns'])decimal(row[key]);
  for(const key of ['mode','uid','gid'])integer(row[key]);
  if((row.mode&0o170000)!==0o100000)fail('file not regular');
 }
 return{paths,total};
}
const RECORD_KEYS=['schema','identity_before','identity_after','environment_keys','launch_environment_keys','debugger_environment_policy','python_version','python_executable','python_prefix','python_base_prefix','python_exec_prefix','pycache_prefix','sys_path','flags','initial_modules','collection_modules','initial_maps','collection_maps','final_maps','files','file_bytes_hashed','inferior_pids','objfile_count','startup_snapshot_not_import_history','cache_path_not_execution_provenance','system_site_hooks_disabled','debugger_acceptance','runtime_acceptance','physical_capture','gpu_dispatch'];
export function validatePythonOwnership(rows,search,artifacts){
 if(search[0]!==PYTHON)fail('custom Python search prefix');
 const expected=new Set(artifacts.data_files.filter(r=>r.path.startsWith('python/')).map(r=>DATA+'/'+r.path));
 const owners=rows.filter(r=>r.name==='gdb'),owner=owners[0];
 if(owners.length!==1||owner.kind!=='module'||owner.file!==PACKAGE+'/__init__.py'||owner.origin!==owner.file||!eq(owner.package_paths,[PACKAGE]))fail('custom GDB package owner');
 for(const row of rows.filter(r=>r.name==='gdb'||r.name.startsWith('gdb.'))){
  if(row.kind!=='module')fail('custom GDB module kind');
  if(row.file===null){if(![null,'built-in'].includes(row.origin)||row.cached!==null||row.package_paths!==null)fail('custom GDB builtin alias');continue;}
  if(!expected.has(row.file)||row.origin!==row.file)fail('custom GDB module source');
  if(row.cached!==null){
   installedPath(row.cached);const slash=row.file.lastIndexOf('/'),base=row.file.slice(slash+1,-3);
   const prefix=row.file.slice(0,slash)+'/__pycache__/'+base+'.cpython-312';
   if(row.cached!==row.file+'c'&&row.cached!==prefix+'.pyc'&&row.cached!==prefix+'.opt-1.pyc'&&row.cached!==prefix+'.opt-2.pyc')fail('custom GDB module cache lineage');
  }
  if(row.package_paths!==null&&(!row.file.endsWith('/__init__.py')||!eq(row.package_paths,[row.file.slice(0,row.file.lastIndexOf('/'))])))fail('custom GDB package path');
 }
}
export function validateObservation(v,family,ready,artifacts=requireArtifactReady()){
 validateArtifactContract(artifacts);
 keys(v,RECORD_KEYS);
 if(v.schema!=='fe2o3-custom-debugger-startup-observation-v1')fail('schema');
 for(const name of ['debugger_acceptance','runtime_acceptance','physical_capture','gpu_dispatch','system_site_hooks_disabled'])if(v[name]!==false)fail('unsupported claim');
 for(const name of ['startup_snapshot_not_import_history','cache_path_not_execution_provenance'])if(v[name]!==true)fail('missing limitation');
 for(const identity of [v.identity_before,v.identity_after]){
  keys(identity,['pid','ppid','start_ticks','exe','cgroup']);
  integer(identity.pid,2);integer(identity.ppid,2);integer(identity.start_ticks,1);
  if(identity.exe!==DEBUGGER||identity.pid!==family.controller_pid||identity.ppid!==ready.pid
   ||identity.ppid!==family.controller_parent_pid||identity.start_ticks!==family.controller_start_ticks
   ||identity.cgroup!=='0::'+ready.control_group+'/workload\n')fail('launch-owned process identity');
 }
 if(!eq(v.identity_before,v.identity_after))fail('identity drift');
 if(!eq(v.launch_environment_keys,['LANG','LC_ALL','PYTHONDONTWRITEBYTECODE','PYTHONNOUSERSITE','PYTHONSAFEPATH']))fail('launch environment keys');
 if(!eq(v.environment_keys,['COLUMNS','LANG','LC_ALL','LINES','PYTHONDONTWRITEBYTECODE','PYTHONNOUSERSITE','PYTHONSAFEPATH']))fail('environment keys');
 if(v.debugger_environment_policy!=='bundled-readline-dumb-columns80-lines24-v1')fail('debugger environment policy');
 for(const key of ['python_version','python_executable','python_prefix','python_base_prefix','python_exec_prefix'])string(v[key]);
 nullable(v.pycache_prefix,4096);for(const p of array(v.sys_path,256))string(p);
 const flagNames=['debug','inspect','interactive','optimize','dont_write_bytecode','no_user_site','no_site','ignore_environment','verbose','bytes_warning','quiet','hash_randomization','isolated','dev_mode','utf8_mode','warn_default_encoding','safe_path','int_max_str_digits'];
 keys(v.flags,flagNames);for(const [name,value] of Object.entries(v.flags))integer(value,name==="int_max_str_digits"?-1:0,10000000);
 for(const [key,value] of Object.entries({dont_write_bytecode:1,no_user_site:1,safe_path:1,ignore_environment:0,no_site:0}))if(v.flags[key]!==value)fail('effective Python flags');
 const initial=modules(v.initial_modules),collection=modules(v.collection_modules);
 validatePythonOwnership(v.initial_modules,v.sys_path,artifacts);validatePythonOwnership(v.collection_modules,v.sys_path,artifacts);
 for(const [key,field] of Object.entries({executable:'python_executable',prefix:'python_prefix',base_prefix:'python_base_prefix',exec_prefix:'python_exec_prefix'}))if(v[field]!==artifacts.python_runtime[key])fail('actual configured Python runtime');
 if(!initial.has('sys')||!initial.has('gdb'))fail('initial Python owner modules absent');
 for(const row of v.initial_modules){const final=v.collection_modules.find(r=>r.name===row.name);if(!eq(row,final))fail('startup module metadata changed')}
 const parsedMaps=[v.initial_maps,v.collection_maps,v.final_maps].map(parseMaps);
 const fileMap=files(v.files);if(fileMap.total!==v.file_bytes_hashed)fail('file byte receipt');
 const named = rows=>rows.filter(r=>r.path?.startsWith('/'));
 if(!eq(named(parsedMaps[1]),named(parsedMaps[2])))fail('final mapped file roster changed');
 const referenced=new Set();
 for(const rows of parsedMaps)for(const map of named(rows)){
  const file=fileMap.paths.get(map.path);referenced.add(map.path);
  if(!file?.exists||file.ino!==map.ino||file.dev!==map.dev)fail('mapped content identity');
 }
 for(const row of v.initial_modules.concat(v.collection_modules))if(row.kind==='module'){
  for(const k of ['file','cached','origin'])if(row[k]?.startsWith('/')){referenced.add(row[k]);if(!fileMap.paths.has(row[k]))fail('module path missing file observation')}
 }
 if(referenced.size!==fileMap.paths.size)fail('unreferenced file');
 for(const row of v.collection_modules.filter(r=>r.kind==='module'&&(r.name==='gdb'||r.name.startsWith('gdb.'))&&r.file!==null)){
  const expected=artifacts.data_files.find(f=>DATA+'/'+f.path===row.file),got=fileMap.paths.get(row.file);
  if(!expected||!got?.exists||got.realpath!==row.file||got.bytes!==expected.bytes||got.sha256!==expected.sha256)fail('actual generated Python content pin');
 }
 const exe=fileMap.paths.get(DEBUGGER);
 if(!exe?.exists||!exe.elf||exe.sha256!==artifacts.debugger.sha256||exe.bytes!==artifacts.debugger.bytes)fail('actual debugger ELF pin');
 for(const pid of array(v.inferior_pids,4))if(pid!==0)fail('inferior activation');
 if(v.objfile_count!==0)fail('inferior object load');
 if(family.reason!=='complete'||family.controller_wait_status!==0||family.pidfd_exit_observed!==true
  ||family.wait_echild!==true||family.cgroup_populated_zero!==true||family.no_child_cgroups!==true
  ||family.streams_eof!==true||family.stream_error!==false||family.owner_current!==true
  ||family.proof_error!==false||family.observation_refused!==false||family.cleanup_deadline_expired!==false||family.kill_attempts!==0||family.kill_failures!==0
  ||family.cleanup_complete!==true||family.observation_complete!==true||family.adopted_reaped!==0)fail('actual family/EOF/reaping evidence');
 return{initial_module_count:initial.size,collector_added_modules:[...collection].filter(n=>!initial.has(n)),
  mapped_elf_paths:[...new Set(parsedMaps.flat().filter(r=>r.path&&fileMap.paths.get(r.path)?.elf).map(r=>r.path))],
  nonstandard_module_aliases:v.collection_modules.filter(m=>m.kind==='nonmodule').map(m=>m.name),
  unavailable_module_file_paths:[...new Set(v.collection_modules.filter(m=>m.kind==='module').flatMap(m=>[m.file,m.origin]).filter(p=>p?.startsWith('/')&&!fileMap.paths.get(p)?.exists))],
  system_hook_presence:Object.fromEntries(['sitecustomize','apport_python_hook','_distutils_hack','zope','gdb'].map(n=>[n,initial.has(n)])),
  startup_observation_complete:true,closure_review_required:true,complete_import_history:false,
  cache_execution_provenance:false,debugger_acceptance:false,runtime_acceptance:false,physical_capture:false,gpu_dispatch:false,strong_isolation:false};
}
export function parseObservation(bytes,family,ready,artifacts=requireArtifactReady()){
 if(!Buffer.isBuffer(bytes)||bytes.length>8*1024*1024+32)fail('stdout bound');
 const text=bytes.toString('utf8');if(!Buffer.from(text).equals(bytes)||!text.startsWith('FE2O3_CUSTOM_STARTUP_V1 ')||!text.endsWith('\n')||text.indexOf('\n')!==text.length-1)fail('single observation frame');
 const v=JSON.parse(text.slice('FE2O3_CUSTOM_STARTUP_V1 '.length,-1));return{observation:v,summary:validateObservation(v,family,ready,artifacts)};
}

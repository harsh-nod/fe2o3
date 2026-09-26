// SPDX-License-Identifier: MIT OR Apache-2.0
// Read-only selected-file consistency checks; not source or runtime authority.
import fs from 'node:fs';
import path from 'node:path';
import crypto from 'node:crypto';
import {isDeepStrictEqual} from 'node:util';
export const MANIFEST_CAP=32*1024;
export const FILE_CAP=128*1024;
export const TOTAL_CAP=512*1024;
export const PATHS=Object.freeze([
  "Cargo.lock",
  "Cargo.toml",
  "LICENSE-APACHE",
  "LICENSE-MIT",
  "async_stop_tests.rs",
  "controller.rs",
  "lib.rs",
  "native/clock.rs",
  "native/config.rs",
  "native/custody.rs",
  "native/failure_diagnostic.rs",
  "native/main.rs",
  "native/parent.rs",
  "native/profile.rs",
  "native/publication.rs",
  "native/scope.rs",
  "native/setup_diagnostic.rs",
  "native/streams.rs",
  "native/wire.rs",
  "physical_snapshot_v2.rs",
  "physical_snapshot_v2_tests.rs",
  "rocgdb_mi_parser_v3.rs",
  "syntax.rs",
  "target_report.rs",
  "tests.rs",
  "transitions.rs"
]);
const fail=s=>{throw Error('one-stop physical-v2 source package: '+s);};
const sha=b=>crypto.createHash('sha256').update(b).digest('hex');
function keys(v,expected){
 if(v===null||typeof v!=='object'||Array.isArray(v)
  ||!isDeepStrictEqual(Object.keys(v).sort(),expected.slice().sort()))fail('closed keys');
}
export function manifest(bytes){
 if(!Buffer.isBuffer(bytes)||bytes.length===0||bytes.length>MANIFEST_CAP)fail('manifest byte cap');
 const text=new TextDecoder('utf-8',{fatal:true}).decode(bytes);
 const value=JSON.parse(text);
 // A single canonical encoding denies duplicate/escaped-equivalent keys and
 // trailing data without importing a general-purpose unchecked JSON document.
 if(JSON.stringify(value,null,2)+'\n'!==text)fail('canonical manifest encoding');
 return requireManifest(value);
}
function requireManifest(value){
 keys(value,['schema','license','runtime_profile','rust_files','files']);
 if(value.schema!=='fe2o3-gfx950-one-stop-controller-source-v2'
  ||value.license!=='MIT OR Apache-2.0'||value.runtime_profile!=='unbound'||value.rust_files!==22
  ||!Array.isArray(value.files)||value.files.length!==PATHS.length)fail('manifest domain');
 let total=0;
 for(let i=0;i<PATHS.length;i++){
  const row=value.files[i];keys(row,['path','bytes','sha256']);
  if(row.path!==PATHS[i]||!Number.isSafeInteger(row.bytes)||row.bytes<=0||row.bytes>FILE_CAP
   ||typeof row.sha256!=='string'||!/^[0-9a-f]{64}$/.test(row.sha256))fail('source roster/pin');
  total+=row.bytes;if(total>TOTAL_CAP)fail('aggregate source cap');
 }
 return value;
}
export function verifyPayload(value,read){
 requireManifest(value);
 let total=0;
 for(const row of value.files){
  const bytes=read(row.path,row.bytes);
  if(!Buffer.isBuffer(bytes)||bytes.length!==row.bytes||sha(bytes)!==row.sha256)fail('source bytes changed');
  new TextDecoder('utf-8',{fatal:true}).decode(bytes);
  total+=bytes.length;if(total>TOTAL_CAP)fail('aggregate source cap');
 }
 return {schema:'fe2o3-gfx950-one-stop-controller-source-check-v2',files:value.files.length,bytes:total,
  selected_source_bytes_match:true,runtime_profile:'unbound',complete_tree_verified:false,
  source_authenticated:false,built_binary_verified:false,startup_qualified:false,
  native_authority:false,processes_started:0};
}
const identity=m=>[m.dev,m.ino,m.mode,m.size,m.mtimeNs,m.ctimeNs];
const equal=(a,b)=>isDeepStrictEqual(identity(a),identity(b));
function currentDirectory(name,expected){
 const st=fs.lstatSync(name,{bigint:true});
 if(!st.isDirectory()||!equal(st,expected)||fs.realpathSync(name)!==name)fail('directory identity changed');
}
export function readRegular(root,relative,cap){
 if(process.platform!=='linux'||!Number.isSafeInteger(cap)||cap<1||cap>FILE_CAP
  ||!['source-manifest.json',...PATHS].includes(relative))fail('read profile');
 const parts=relative.split('/'),parents=[root];
 for(let i=0;i<parts.length-1;i++)parents.push(path.join(parents.at(-1),parts[i]));
 const dirs=parents.map(name=>{
  const st=fs.lstatSync(name,{bigint:true});
  if(!st.isDirectory()||fs.realpathSync(name)!==name)fail('noncanonical directory');
  return {name,st};
 });
 const name=path.join(root,relative);
 const fd=fs.openSync(name,fs.constants.O_RDONLY|fs.constants.O_NOFOLLOW|fs.constants.O_NONBLOCK);
 try{
  const before=fs.fstatSync(fd,{bigint:true});
  if(!before.isFile()||before.size<1n||before.size>BigInt(cap))fail('regular file size');
  const pathname=fs.lstatSync(name,{bigint:true});
  if(!pathname.isFile()||!equal(before,pathname))fail('file identity');
  // One growth/EOF probe, bounded before allocation. No readFile of arbitrary size.
  const bytes=Buffer.alloc(Number(before.size)+1);let used=0;
  for(;;){
   const n=fs.readSync(fd,bytes,used,bytes.length-used,null);
   if(n===0)break;
   used+=n;if(used>Number(before.size))fail('file grew');
  }
  if(used!==Number(before.size)||!equal(before,fs.fstatSync(fd,{bigint:true}))
   ||!equal(before,fs.lstatSync(name,{bigint:true})))fail('file changed');
  for(const {name,st} of dirs)currentDirectory(name,st);
  return bytes.subarray(0,used);
 }finally{fs.closeSync(fd);}
}
export function readPackage(root){
 if(typeof root!=='string'||root.length>4096||!path.isAbsolute(root)||path.resolve(root)!==root
  ||/[\x00-\x1f\x7f]/.test(root)||fs.realpathSync(root)!==root)fail('canonical absolute package root');
 const rootFd=fs.openSync(root,fs.constants.O_RDONLY|fs.constants.O_DIRECTORY|fs.constants.O_NOFOLLOW);
 try{
  const before=fs.fstatSync(rootFd,{bigint:true});
  if(!before.isDirectory())fail('package directory');
  const value=manifest(readRegular(root,'source-manifest.json',MANIFEST_CAP));
  const result=verifyPayload(value,(name,cap)=>readRegular(root,name,cap));
  currentDirectory(root,before);
  if(!equal(before,fs.fstatSync(rootFd,{bigint:true})))fail('retained root changed');
  return result;
 }finally{fs.closeSync(rootFd);}
}

// SPDX-License-Identifier: MIT
// Read-only verifier for a deliberately narrow private provider source package.
import fs from 'node:fs';
import path from 'node:path';
import {fileURLToPath} from 'node:url';
import {createHash} from 'node:crypto';
import assert from 'node:assert/strict';
import {Account,CAPS,exactEdits} from './tests/source-core.mjs';
import {MANIFEST_PIN} from './tests/contract-pin.mjs';
const ROOT=path.dirname(fileURLToPath(import.meta.url)),sha=b=>createHash('sha256').update(b).digest('hex');
const identity=s=>[s.dev,s.ino,s.mode,s.size,s.mtimeNs,s.ctimeNs];
const anchor=()=> '// SPDX-License-Identifier: MIT\n// Generated source-only manifest anchor; no runtime authority.\nexport const MANIFEST_PIN=Object.freeze('+JSON.stringify(MANIFEST_PIN)+');\n';
function relative(p){assert(typeof p==='string'&&p.length<=192&&/^[A-Za-z0-9._/-]+$/.test(p)&&!p.startsWith('/')&&p.split('/').every(x=>x&&x!=='.'&&x!=='..'));}
export function verifyPackage(options={}){
 const account=new Account(options.lower??{}),end=Date.now()+account.limits().timeout_ms;
 return account.run(()=>{
  assert.deepEqual(Object.keys(options).filter(x=>x!=='lower').sort(),options.mode===undefined?[]:['api','kfd','mode','root']);
  const now=()=>assert(Date.now()<end,'source verification deadline');
  const read=(filename,pin,role)=>account.run(()=>{
   now();assert(path.isAbsolute(filename)&&fs.realpathSync(filename)===filename);
   assert(Number.isSafeInteger(pin.bytes)&&pin.bytes>0&&pin.bytes<=CAPS.perFile&&/^[a-f0-9]{64}$/.test(pin.sha256));
   const before=fs.lstatSync(filename,{bigint:true});assert(before.isFile()&&!before.isSymbolicLink()&&before.size===BigInt(pin.bytes));
   account.admit(role,pin.bytes);const fd=fs.openSync(filename,fs.constants.O_RDONLY|fs.constants.O_NOFOLLOW);
   try{assert.deepEqual(identity(fs.fstatSync(fd,{bigint:true})),identity(before));const b=Buffer.alloc(pin.bytes),eof=Buffer.alloc(1);
    for(let at=0;at<b.length;){now();const n=Math.min(account.limits().chunk,b.length-at);assert.equal(fs.readSync(fd,b,at,n,at),n,'short read refuses without retry');at+=n;}
    now();assert.equal(fs.readSync(fd,eof,0,1,b.length),0,'required EOF');assert.equal(sha(b),pin.sha256);
    assert.deepEqual(identity(fs.fstatSync(fd,{bigint:true})),identity(before));assert.deepEqual(identity(fs.lstatSync(filename,{bigint:true})),identity(before));
    assert.equal(fs.realpathSync(filename),filename);now();return new TextDecoder('utf-8',{fatal:true}).decode(b);
   }finally{fs.closeSync(fd);}
  });
  assert.equal(fs.realpathSync(ROOT),ROOT);assert(MANIFEST_PIN.bytes<=account.limits().manifest);
  const text=read(ROOT+'/source-manifest.json',MANIFEST_PIN,'manifest'),m=JSON.parse(text);
  assert.equal(JSON.stringify(m,null,2)+'\n',text);
  assert.equal(m.schema,'fe2o3-private-owned-queue-health-source-v1');assert.equal(m.license,'MIT');
  assert.deepEqual(m.upstream,{repository:'https://github.com/ROCm/ROCdbgapi',commit:'06465e940698e8423d1b629c834c98bdd7753439'});
  assert.equal(m.native_qualified,false);assert.equal(m.general_queue_error_support,false);assert.equal(m.system_installation,false);
  assert.deepEqual(m.caps,CAPS);assert(Array.isArray(m.files)&&m.files.length<=48);
  const files=new Map(),names=new Set();
  for(const row of m.files){relative(row.path);assert(!names.has(row.path));names.add(row.path);files.set(row.path,read(ROOT+'/'+row.path,row,'package/'+row.path));}
  const boot=anchor();assert.equal(Buffer.byteLength(boot),m.bootstrap.bytes);
  assert.equal(read(ROOT+'/tests/contract-pin.mjs',{bytes:m.bootstrap.bytes,sha256:sha(boot)},'bootstrap'),boot);
  assert.equal(m.preimages.length,6);assert.equal(m.postimages.length,7);
  const edits=JSON.parse(files.get('edits.json'));assert.equal(edits.length,26);
  let admittedEdits=0;
  for(const row of m.preimages){
   const old=files.get('preimages/'+row.path),next=files.get(row.path);assert.equal(typeof old,'string');assert.equal(typeof next,'string');
   const selected=edits.filter(e=>e.file===row.path);admittedEdits+=selected.length;
   assert.equal(exactEdits(account,old,selected),next);assert.equal(exactEdits(account,next,selected,'inverse'),old);
   assert.equal(sha(old),row.sha256);assert.equal(Buffer.byteLength(old),row.bytes);
  }assert.equal(admittedEdits,26);
  for(const row of m.postimages){assert.equal(sha(files.get(row.path)),row.sha256);assert.equal(Buffer.byteLength(files.get(row.path)),row.bytes);}
  const fragments=JSON.parse(files.get('tests/extraction-contracts.json'));
  assert(fragments.sections.length<=24);
  for(const row of fragments.sections){
   const source=files.get(row.source),test=files.get(row.test),fragment=row.text;
   assert(typeof source==='string'&&typeof test==='string'&&typeof fragment==='string'&&fragment.length>0&&Buffer.byteLength(fragment)<=65536);
   account.charge({work:8*(Buffer.byteLength(source)+Buffer.byteLength(test)+Buffer.byteLength(fragment))});
   assert.equal(Buffer.byteLength(fragment),row.bytes);assert.equal(sha(fragment),row.sha256);
   for(const value of [source,test]){const at=value.indexOf(fragment);assert(at>=0&&value.indexOf(fragment,at+fragment.length)<0,'exact unique mock section');}
  }
  if(options.mode!==undefined){
   assert(['preimages','postimages'].includes(options.mode));assert(path.isAbsolute(options.root)&&fs.realpathSync(options.root)===options.root);
   for(const row of m[options.mode]){relative(row.path);read(options.root+'/'+row.path,row,'stage/'+row.path);}
   if(options.mode==='preimages'){
    let absent=false;try{fs.lstatSync(options.root+'/src/queue-health-v1.h');}catch(e){if(e.code==='ENOENT')absent=true;else throw e;}assert(absent,'new health header must be absent');
   }
   read(options.api,m.external_headers.api,'external/api');read(options.kfd,m.external_headers.kfd,'external/kfd');
  }
  now();return {schema:'fe2o3-private-owned-queue-health-source-report-v1',changed_sources:6,postimages:7,edits:26,
   verified_stage:options.mode??null,accounting:account.usage(),module_loader_reads_metered:false,metadata_operations_metered:false,
   whole_checkout_verified:false,built_provider_verified:false,native_qualified:false,milestone_complete:false};
 });
}
if(process.argv[1]&&fileURLToPath(import.meta.url)===path.resolve(process.argv[1])){
 try{assert(process.argv.length===2||process.argv.length===6);
  const options=process.argv.length===2?{}:{mode:process.argv[2],root:process.argv[3],api:process.argv[4],kfd:process.argv[5]};
  process.stdout.write(JSON.stringify(verifyPackage(options))+'\n');
 }catch(e){process.stderr.write('private provider source verification refused: '+String(e.message)+'\n');process.exitCode=1;}
}

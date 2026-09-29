// Explicit data-only fixture IO. Importing performs no read; no historical path/copy fallback exists.
import fs from 'node:fs';
import {createHash} from 'node:crypto';
import {decodeFixtureManifest,requiredFixtureManifestPath,MAX_FIXTURE_MANIFEST_BYTES,MAX_FIXTURE_MEMBER_BYTES,MAX_FIXTURE_RESERVED_BYTES,MAX_FIXTURE_CONTENT_CALLS} from './loaded-fixture-manifest.mjs';
const fields=['dev','ino','size','mode','uid','gid','mtimeNs','ctimeNs'];
const fail=m=>{throw Error('loaded review fixture IO: '+m);};
const sha=b=>createHash('sha256').update(b).digest('hex');
function same(a,b){for(const k of fields)if(a[k]!==b[k])fail('named/descriptor identity drift');}
/** Complete explicit files only; all received source bytes remain data, never modules. */
export function readLoadedReviewFixtures(){
 const manifestPath=requiredFixtureManifestPath(process.env.FE2O3_LOADED_REVIEW_FIXTURES);
 let reserved=0,calls=0;const observations=[];
 function read(path,cap,expected=null){
  const named=fs.lstatSync(path,{bigint:true});if(!named.isFile()||named.isSymbolicLink()||fs.realpathSync(path)!==path)fail('canonical regular fixture');
  const fd=fs.openSync(path,fs.constants.O_RDONLY|fs.constants.O_NOFOLLOW|fs.constants.O_NONBLOCK);
  try{
   const before=fs.fstatSync(fd,{bigint:true});if(!before.isFile()||before.size<0n||before.size>BigInt(cap))fail('fixture member bound');same(named,before);
   const n=Number(before.size);if(expected&&n!==expected.bytes)fail('exact fixture size');
   if(reserved>MAX_FIXTURE_RESERVED_BYTES-(n+1))fail('fixture aggregate');reserved+=n+1;
   const raw=Buffer.alloc(n),eof=Buffer.alloc(1);let at=0;
   while(at<n){const take=Math.min(65536,n-at);if(++calls>MAX_FIXTURE_CONTENT_CALLS)fail('fixture content call bound');if(fs.readSync(fd,raw,at,take,null)!==take)fail('short fixture read without retry');at+=take;}
   if(++calls>MAX_FIXTURE_CONTENT_CALLS)fail('fixture EOF call bound');if(fs.readSync(fd,eof,0,1,null)!==0)fail('fixture EOF growth');
   const after=fs.fstatSync(fd,{bigint:true}),namedAfter=fs.lstatSync(path,{bigint:true});same(before,after);same(before,namedAfter);
   if(!namedAfter.isFile()||fs.realpathSync(path)!==path)fail('fixture resolution drift');
   if(expected&&sha(raw)!==expected.sha256)fail('whole immutable fixture pin');
   observations.push({path,identity:before});return raw;
  }finally{fs.closeSync(fd);}
 }
 const raw=read(manifestPath,MAX_FIXTURE_MANIFEST_BYTES),plan=decodeFixtureManifest(raw);
 if(plan.files.some(p=>p.path===manifestPath))fail('manifest cannot be its own fixture');
 const records=plan.files.map(p=>({role:p.role,bytes:read(p.path,MAX_FIXTURE_MEMBER_BYTES,p.expected)}));
 for(const o of observations){const after=fs.lstatSync(o.path,{bigint:true});if(!after.isFile()||fs.realpathSync(o.path)!==o.path)fail('final fixture resolution');same(o.identity,after);}
 return{profile_records:records.slice(0,72),selector_records:records.slice(72),fixture_accounting:{files:77,reserved_bytes:reserved,content_calls:calls,manifest_bytes:raw.length,fixture_payload_bytes:plan.payload_bytes,limits:{reserved_bytes:MAX_FIXTURE_RESERVED_BYTES,content_calls:MAX_FIXTURE_CONTENT_CALLS},all_files_read_completely:true,all_names_revalidated:true,host_path_fallback:false,data_only:true,metadata_syscalls_metered:false,module_loader_io_metered:false}};
}

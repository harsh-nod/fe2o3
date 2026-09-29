// Pure explicit fixture-manifest admission. Historical .path/.copy values are never IO defaults.
import {PROFILE} from './loaded-profile-binding.mjs';
import {BINDING} from './loaded-selection-binding.mjs';
export const MAX_FIXTURE_MANIFEST_BYTES=65536;
export const MAX_FIXTURE_MEMBER_BYTES=8*1024*1024;
export const MAX_FIXTURE_RESERVED_BYTES=32*1024*1024;
export const MAX_FIXTURE_CONTENT_CALLS=1024;
const freeze=v=>{if(v&&typeof v==='object'){for(const x of Object.values(v))freeze(x);Object.freeze(v);}return v;};
const expected=PROFILE.records.concat(BINDING.selector_records).map(p=>({role:p.role,bytes:p.bytes,sha256:p.sha256}));
if(expected.length!==76||new Set(expected.map(p=>p.role)).size!==76)throw Error('fixture binding: fixed 76 unique roles');
export const FIXTURE_ROLES=freeze(expected.map(p=>p.role));
const fail=m=>{throw Error('loaded review fixtures: '+m);};
function keys(v,n){if(!v||typeof v!=='object'||Array.isArray(v)||JSON.stringify(Object.keys(v).sort())!==JSON.stringify(n.slice().sort()))fail('closed keys');}
export function requiredFixtureManifestPath(value){
 if(typeof value!=='string'||value.length===0)fail('FE2O3_LOADED_REVIEW_FIXTURES is required; no fallback or skip');
 if(Buffer.byteLength(value)>4096||!value.startsWith('/')||value.includes('\\')||value.includes('\0')||value.includes(' (deleted)')||value.split('/').slice(1).some(x=>!x||x==='.'||x==='..'))fail('absolute canonical fixture path required');
 return value;
}
export function decodeFixtureManifest(raw){
 if(!Buffer.isBuffer(raw)||raw.length===0||raw.length>MAX_FIXTURE_MANIFEST_BYTES)fail('manifest byte bound');
 const text=raw.toString('utf8');if(!Buffer.from(text).equals(raw))fail('manifest UTF8');
 const v=JSON.parse(text);keys(v,['schema','files']);
 if(v.schema!=='fe2o3-loaded-review-fixtures-v1'||!Array.isArray(v.files)||v.files.length!==76)fail('exact 76-role manifest');
 const names=new Set(),roles=new Set(),files=[];let bytes=0,calls=0;
 for(let i=0;i<76;i++){
  const row=v.files[i],pin=expected[i];keys(row,['role','path']);
  if(row.role!==pin.role||roles.has(row.role))fail('fixed ordered unique fixture role');roles.add(row.role);
  const path=requiredFixtureManifestPath(row.path);if(names.has(path))fail('distinct fixture file paths');names.add(path);
  if(!Number.isSafeInteger(pin.bytes)||pin.bytes<0||pin.bytes>MAX_FIXTURE_MEMBER_BYTES||!/^[a-f0-9]{64}$/.test(pin.sha256))fail('immutable binding pin');
  bytes+=pin.bytes+1;calls+=Math.ceil(pin.bytes/65536)+1;
  if(!Number.isSafeInteger(bytes)||bytes>MAX_FIXTURE_RESERVED_BYTES||calls>MAX_FIXTURE_CONTENT_CALLS)fail('fixed fixture aggregate');
  files.push({role:row.role,path,expected:{bytes:pin.bytes,sha256:pin.sha256}});
 }
 return freeze({schema:'fe2o3-explicit-loaded-fixture-plan-v1',files,profile_count:72,selector_count:4,payload_bytes:bytes-76,reserved_bytes:bytes,content_calls:calls,historical_paths_used_as_io_defaults:false});
}

// Pure finite named-input graph compiler. No file reads, loader activation, or cap approval.
import {createHash} from 'node:crypto';
import path from 'node:path';
import {isDeepStrictEqual as equal} from 'node:util';
export const GRAPH_LIMITS=Object.freeze({json_bytes:32*1024*1024,claims:65536,names:16384,imports:4096,roles_per_claim:4096,member_bytes:512*1024*1024,path_bytes:4096,outputs:16});
const fail=m=>{throw Error('loaded operational graph: '+m);};
const check=(v,m)=>{if(!v)fail(m);};
const freeze=v=>{if(v&&typeof v==='object'){for(const x of Object.values(v))freeze(x);Object.freeze(v);}return v;};
export function checkedAdd(a,b){check(Number.isSafeInteger(a)&&a>=0&&Number.isSafeInteger(b)&&b>=0&&Number.isSafeInteger(a+b),'checked nonnegative addition');return a+b;}
function uint(v,max=Number.MAX_SAFE_INTEGER){check(Number.isSafeInteger(v)&&v>=0&&v<=max,'integer bound');return v;}
function keys(v,n){check(v&&typeof v==='object'&&!Array.isArray(v)&&equal(Object.keys(v).sort(),n.slice().sort()),'closed keys');}
function text(v,max=256){check(typeof v==='string'&&Buffer.byteLength(v)>0&&Buffer.byteLength(v)<=max&&!/[\x00-\x1f\x7f]/.test(v),'bounded text');return v;}
function name(v){text(v,GRAPH_LIMITS.path_bytes);check(v.startsWith('/')&&v!=='/'&&!v.includes('\\')&&!v.includes(' (deleted)')&&path.posix.normalize(v)===v&&!v.endsWith('/'),'exact absolute named path');check(v.split('/').length<=129,'bounded path components');return v;}
function pin(v,p){keys(v,['path','bytes','sha256']);check(name(v.path)===p,'pin named path');uint(v.bytes,GRAPH_LIMITS.member_bytes);check(typeof v.sha256==='string'&&/^[a-f0-9]{64}$/.test(v.sha256),'whole SHA256');}
function decimal(v){check(typeof v==='string'&&/^(0|[1-9][0-9]{0,24})$/.test(v),'identity decimal');}
function identity(v,size){check(Array.isArray(v)&&v.length===6,'six identity fields');v.forEach(decimal);check(v[2]===String(size)&&(BigInt(v[3])&0o170000n)===0o100000n,'regular-file identity and size');}
export function decodeGraphJSON(raw){
 check(Buffer.isBuffer(raw)&&raw.length>0&&raw.length<=GRAPH_LIMITS.json_bytes,'complete bounded JSON Buffer');
 const s=raw.toString('utf8');check(Buffer.from(s).equals(raw),'exact UTF8');return JSON.parse(s);
}
export function compileOperationalGraph(raw){
 const x=decodeGraphJSON(raw);keys(x,['schema','claims','aliases','imports','unresolved','outputs']);
 check(x.schema==='fe2o3-loaded-operational-graph-input-v1','schema');
 check(Array.isArray(x.claims)&&x.claims.length>0&&x.claims.length<=GRAPH_LIMITS.claims,'finite claims');
 const names=new Map(),ids=new Set(),roleOwner=new Map();
 for(const c of x.claims){
  keys(c,['id','path','kind','pin','resolved','identity','ownership','roles']);text(c.id);check(!ids.has(c.id),'duplicate claim ID');ids.add(c.id);
  name(c.path);name(c.resolved);
  check(Array.isArray(c.roles)&&c.roles.length>0&&c.roles.length<=GRAPH_LIMITS.roles_per_claim&&new Set(c.roles).size===c.roles.length,'unique claim roles');
  for(const role of c.roles){text(role);check(!roleOwner.has(role)||roleOwner.get(role)===c.path,'same role on different named paths');roleOwner.set(role,c.path);}
  if(c.kind==='readable'){
   pin(c.pin,c.path);if(c.identity!==null)identity(c.identity,c.pin.bytes);
   if(c.ownership!==null){keys(c.ownership,['uid','gid']);decimal(c.ownership.uid);decimal(c.ownership.gid);}
  }else{
   check(c.kind==='absence-observation'&&c.pin===null&&c.identity===null&&c.ownership===null&&c.resolved===c.path,'absence is a bounded metadata obligation, not a zero-byte file');
  }
  let row=names.get(c.path);
  if(!row){row={path:c.path,kind:c.kind,pin:c.pin,resolved:c.resolved,identity:c.identity,ownership:c.ownership,claim_ids:[],roles:[]};names.set(c.path,row);}
  else{
   check(row.kind===c.kind&&equal(row.pin,c.pin)&&row.resolved===c.resolved,'same-name content/kind/resolution conflict');
   for(const key of ['identity','ownership']){if(row[key]!==null&&c[key]!==null)check(equal(row[key],c[key]),'same-name '+key+' conflict');else if(row[key]===null)row[key]=c[key];}
  }
  row.claim_ids.push(c.id);for(const role of c.roles)if(!row.roles.includes(role))row.roles.push(role);
 }
 check(names.size<=GRAPH_LIMITS.names,'finite individually named roster');
 check(Array.isArray(x.aliases)&&x.aliases.length<=3,'at most three explicit aliases');
 const aliases=new Map();
 for(const a of x.aliases){
  keys(a,['pin','resolved','link_text','target_pin','target_selected']);pin(a.pin,a.pin.path);name(a.resolved);pin(a.target_pin,a.resolved);
  text(a.link_text,4096);check(typeof a.target_selected==='boolean'&&!aliases.has(a.pin.path),'unique alias policy');
  const row=names.get(a.pin.path);check(row?.kind==='readable'&&equal(row.pin,a.pin)&&row.resolved===a.resolved&&a.resolved!==a.pin.path,'exact named alias');
  check(equal(a.target_pin,{...a.pin,path:a.resolved}),'exact target content');
  check(names.has(a.resolved)===a.target_selected,'independent target selection duty');
  if(a.target_selected)check(equal(names.get(a.resolved).pin,a.target_pin),'independent target pin');
  aliases.set(a.pin.path,a);
 }
 for(const row of names.values())check((row.path!==row.resolved)===aliases.has(row.path),'no implicit alias/canonicalization exemption');
 check(Array.isArray(x.imports)&&x.imports.length<=GRAPH_LIMITS.imports,'finite import edges');
 const edges=new Set();
 for(const e of x.imports){
  keys(e,['from','specifier','to']);name(e.from);name(e.to);text(e.specifier,4096);
  check(names.get(e.from)?.kind==='readable'&&names.get(e.to)?.kind==='readable','every import endpoint individually pinned');
  if(e.specifier.startsWith('node:'))check(/^node:[a-z][a-z0-9_/-]*$/.test(e.specifier),'builtin specifier');
  else check(e.specifier===e.to||(e.specifier.startsWith('./')||e.specifier.startsWith('../'))&&path.posix.resolve(path.posix.dirname(e.from),e.specifier)===e.to,'exact declared source edge');
  const edge=JSON.stringify(e);check(!edges.has(edge),'duplicate import edge');edges.add(edge);
 }
 check(Array.isArray(x.unresolved)&&x.unresolved.length<=1024,'finite unresolved roles');
 const unresolved=new Set();
 for(const r of x.unresolved){keys(r,['role','stage','reason']);text(r.role);text(r.reason,4096);check(['before-read','after-read'].includes(r.stage)&&!unresolved.has(r.role),'distinct staged missing role');unresolved.add(r.role);}
 check(Array.isArray(x.outputs)&&x.outputs.length<=GRAPH_LIMITS.outputs,'finite output roles');
 const outputRoles=new Set(),outputNames=new Set();
 for(const o of x.outputs){
  keys(o,['role','path','cap_bytes']);text(o.role);uint(o.cap_bytes,128*1024*1024);check(o.cap_bytes>0&&!outputRoles.has(o.role),'distinct positive output role');outputRoles.add(o.role);
  if(o.path!==null){name(o.path);check(!names.has(o.path)&&!outputNames.has(o.path),'new separate output name');outputNames.add(o.path);}
 }
 let payload=0,readable=0,absent=0,missingIdentity=0;
 for(const r of names.values()){if(r.kind==='readable'){readable++;payload=checkedAdd(payload,r.pin.bytes);if(r.identity===null)missingIdentity++;}else absent++;}
 return freeze({schema:'fe2o3-inactive-loaded-operational-graph-v1',input_sha256:createHash('sha256').update(raw).digest('hex'),entries:[...names.values()],aliases:x.aliases,imports:x.imports,unresolved:x.unresolved,outputs:x.outputs,
 counts:{named:names.size,readable,absence:absent,claims:x.claims.length,roles:roleOwner.size,imports:x.imports.length,identity_slots_unresolved:missingIdentity,payload_bytes:payload},
 complete_operational_graph:false,qualified:false,execution_authority:false,root_cap_change_approved:false,
 native_authority:false,actual_paths_observed:false,no_packed_or_canonicalized_role_exemption:true});
}

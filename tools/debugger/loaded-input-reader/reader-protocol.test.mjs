import test from 'node:test';
import assert from 'node:assert/strict';
import {createHash} from 'node:crypto';
import {deriveReadBudget,readUnqualifiedProtocol,LoadedReaderRefusal} from './reader-protocol.mjs';
const hash=b=>createHash('sha256').update(b).digest('hex');
const clone=x=>structuredClone(x);
const error=code=>Object.assign(Error(code),{code});
function stat(kind,n=0,ino='7'){return{kind,identity:['1',ino,String(n),String({file:0o100644,directory:0o040755,symlink:0o120777}[kind]),'10','11'],uid:'1000',gid:'1000'};}
function fixture(){
 const data=new Map([['/inert/file',Buffer.alloc(65537,65)],['/inert/target',Buffer.from('data')]]);
 const file=p=>({path:p,kind:'readable',pin:{path:p,bytes:data.get(p).length,sha256:hash(data.get(p))},resolved:p,identity:stat('file',data.get(p).length,p.endsWith('file')?'7':'8').identity,ownership:{uid:'1000',gid:'1000'},labels:{prior:['prior:'+p],loaded:[],duties:['duty:'+p],extras:[]},cap:512*1024*1024});
 const target=file('/inert/target'),alias={...clone(target),path:'/inert/alias',pin:{...target.pin,path:'/inert/alias'},resolved:'/inert/target',labels:{prior:['prior:alias'],loaded:['loaded:alias'],duties:['duty:alias'],extras:[]}};
 const missing={path:'/inert/cache/missing.pyc',kind:'absence-observation',pin:null,resolved:'/inert/cache/missing.pyc',identity:null,ownership:null,labels:{prior:[],loaded:['loaded:missing'],duties:[],extras:[]},cap:0};
 const entries=[file('/inert/file'),target,alias,missing],aliases=[{pin:alias.pin,resolved:target.path,link_text:'target',target_pin:target.pin,target_selected:true}];
 const protocol={schema:'fe2o3-loaded-read-protocol-v1',entries,aliases,passes:2,budget:deriveReadBudget(entries,aliases,2),selection_digest:'a'.repeat(64),custody:{qualified_historical_input_bytes:false,operational_roster_complete:false,root_cap_change_approved:false,execution_authority:false}};
 const descriptors=new Map(),calls=[],overrides={};let next=3;
 const base={
  lstat(p){if(p==='/'||p==='/inert')return stat('directory',0,p==='/'?'1':'2');if(p==='/inert/alias')return stat('symlink',6,'9');const row=entries.find(r=>r.path===p&&r.kind==='readable');if(row&&data.has(p))return{kind:'file',identity:[...row.identity],uid:'1000',gid:'1000'};throw error('ENOENT');},
  realpath:p=>p==='/inert/alias'?'/inert/target':p,
  readlink:p=>{assert.equal(p,'/inert/alias');return'target';},
  open(p){assert.ok(data.has(p));const fd=next++;descriptors.set(fd,{p,at:0});return fd;},
  fstat(fd){const d=descriptors.get(fd);assert.ok(d);return base.lstat(d.p);},
  read(fd,b,offset,n){const d=descriptors.get(fd);assert.ok(d);const value=data.get(d.p),got=Math.min(n,value.length-d.at);value.copy(b,offset,d.at,d.at+got);d.at+=got;return got;},
  close(fd){assert.ok(descriptors.delete(fd));},
 };
 const provider=Object.fromEntries(Object.keys(base).map(name=>[name,(...args)=>{calls.push({name,args:args.map(a=>Buffer.isBuffer(a)?{buffer:a.length}:a)});return(overrides[name]??base[name])(...args);}]));
 const options={guard(){},now:()=>0,milliseconds:240000};
 return{protocol,provider,base,data,calls,overrides,descriptors,options,run(){return readUnqualifiedProtocol(protocol,provider,options);}};
}
function refuses(f,needle){let caught;try{f.run();}catch(e){caught=e;}assert.ok(caught,'must refuse');if(needle)assert.match(caught.message,needle);return caught;}
test('complete two passes hash full contents preserve labels and named target payments',()=>{
 const f=fixture(),r=f.run();assert.equal(r.qualified,false);assert.equal(r.execution_authority,false);assert.equal(r.io_protocol_completed,true);
 assert.equal(r.counts.completed_passes,2);assert.equal(r.counts.completed_readable,6);assert.equal(r.counts.completed_absence,2);
 assert.equal(r.counts.content_calls,14);assert.equal(r.counts.returned_content_bytes,131090);assert.equal(r.counts.attempted_content_bytes,131096);
 assert.equal(r.counts.opened,6);assert.equal(r.counts.closed,6);assert.equal(f.descriptors.size,0);
 assert.equal(r.passes[0][2].pin.path,'/inert/alias');assert.equal(r.passes[0][2].resolved,'/inert/target');assert.equal(r.passes[0][3].content_read,false);
 assert.equal(r.passes[0][3].missing_component,'/inert/cache');assert.equal(r.passes[0][3].existing_ancestors.length,2);
 assert.deepEqual(r.passes[0],r.passes[1]);assert.ok(Object.isFrozen(r.passes[0][2].labels));
 assert.equal(f.calls.filter(x=>x.name==='open'&&x.args[0]==='/inert/target').length,4);
});
test('complete content call bound uses exact chunks plus one EOF per selected name',()=>{
 const f=fixture();f.run();const lens=f.calls.filter(x=>x.name==='read').map(x=>x.args[3]);
 assert.deepEqual(lens,[65536,1,1,4,1,4,1,65536,1,1,4,1,4,1]);
 assert.ok(f.calls.filter(x=>x.name==='read').every(x=>x.args[1].buffer<=65536));
});
test('zero-byte readable files still pay two EOF calls',()=>{
 const f=fixture(),r=f.protocol.entries[0];f.data.set(r.path,Buffer.alloc(0));r.pin.bytes=0;r.pin.sha256=hash(Buffer.alloc(0));r.identity[2]='0';
 f.protocol.budget=deriveReadBudget(f.protocol.entries,f.protocol.aliases,2);
 const out=f.run();assert.equal(out.counts.content_calls,10);assert.equal(out.counts.attempted_content_bytes,22);assert.equal(out.counts.returned_content_bytes,16);
});
test('metadata budget is derived separately and retained on success',()=>{
 const f=fixture(),r=f.run();assert.equal(r.counts.reserved_metadata_calls,114);assert.equal(r.counts.metadata_calls,98);assert.equal(r.syscall_counters_are_provider_calls,true);assert.equal(r.module_loader_io_metered,false);
});
test('short content read refuses immediately without retry and closes descriptor',()=>{
 const f=fixture();f.overrides.read=(fd,b,o,n)=>{f.base.read(fd,b,o,n);return n-1;};
 const e=refuses(f,/short read/);assert.ok(e instanceof LoadedReaderRefusal);assert.equal(e.accounting.content_calls,1);assert.equal(e.accounting.close_attempts,1);assert.equal(e.accounting.closed,1);assert.equal(f.descriptors.size,0);
});
test('EOF growth refuses and preserves charged read counters',()=>{
 const f=fixture();f.overrides.read=(fd,b,o,n)=>{const got=f.base.read(fd,b,o,n);return got===0?1:got;};
 const e=refuses(f,/EOF growth/);assert.equal(e.accounting.content_calls,3);assert.equal(e.accounting.attempted_content_bytes,65538);assert.equal(e.accounting.returned_content_bytes,65538);assert.equal(f.descriptors.size,0);
});
test('wrong whole hash refuses after complete payload and EOF',()=>{
 const f=fixture();f.protocol.entries[0].pin.sha256='0'.repeat(64);const e=refuses(f,/whole content hash/);assert.equal(e.accounting.content_calls,3);assert.equal(e.accounting.closed,1);
});
test('read errors retain debit and close without retry',()=>{
 const f=fixture();f.overrides.read=()=>{throw error('EIO');};const e=refuses(f);assert.equal(e.accounting.content_calls,1);assert.equal(e.accounting.attempted_content_bytes,65536);assert.equal(e.accounting.returned_content_bytes,0);assert.equal(e.accounting.closed,1);
});
test('open errors never invent descriptor custody',()=>{
 const f=fixture();f.overrides.open=()=>{throw error('EACCES');};const e=refuses(f);assert.equal(e.accounting.opened,0);assert.equal(e.accounting.close_attempts,0);
});
test('close error is not accepted or silently retried',()=>{
 const f=fixture();f.overrides.close=()=>{throw error('EIO');};const e=refuses(f);assert.equal(e.accounting.close_attempts,1);assert.equal(e.accounting.closed,0);assert.equal(e.accounting.live_fd_possible,true);
});
test('guard expiry after open still runs descriptor cleanup',()=>{
 const f=fixture();let opened=false;f.overrides.open=p=>{const fd=f.base.open(p);opened=true;return fd;};f.options.guard=()=>{if(opened)throw Error('expired');};
 const e=refuses(f,/expired/);assert.equal(e.accounting.closed,1);assert.equal(e.accounting.content_calls,0);assert.equal(f.descriptors.size,0);
});
test('guard ENOENT must not be promoted into an absence observation',()=>{
 const f=fixture();f.options.guard=()=>{throw error('ENOENT');};const e=refuses(f);assert.equal(e.accounting.completed_absence,0);assert.equal(f.calls.length,0);
});
test('deadline and clock regression refuse before I/O',()=>{
 for(const times of [[0,240000],[10,9],[0,NaN]]){const f=fixture();f.options.now=()=>times.shift();refuses(f);assert.equal(f.calls.length,0);}
});
test('metadata status failures other than ENOENT do not prove absence',()=>{
 for(const code of ['ENOTDIR','EACCES','ELOOP','EIO']){const f=fixture();f.overrides.lstat=p=>p==='/inert/cache'?(()=>{throw error(code);})():f.base.lstat(p);const e=refuses(f);assert.equal(e.accounting.completed_absence,0);}
});
test('existing cache leaf refuses rather than becoming a zero-byte file',()=>{
 const f=fixture();f.overrides.lstat=p=>p.startsWith('/inert/cache')?stat(p.endsWith('.pyc')?'file':'directory'):f.base.lstat(p);refuses(f);assert.equal(f.calls.some(x=>x.name==='open'&&x.args[0].endsWith('.pyc')),false);
});
test('symlink cache ancestor is not accepted as absence',()=>{
 const f=fixture();f.overrides.lstat=p=>p==='/inert/cache'?stat('symlink'):f.base.lstat(p);refuses(f,/file type/);
});
test('absence ancestors are rechecked after the whole pass',()=>{
 const f=fixture();let visits=0;f.overrides.lstat=p=>{if(p==='/inert/cache'&&++visits>1)return stat('directory');return f.base.lstat(p);};refuses(f,/absence\/ancestor changed/);
});
test('named regular symlink is refused without opening',()=>{
 const f=fixture();f.overrides.lstat=p=>p==='/inert/file'?stat('symlink'):f.base.lstat(p);refuses(f,/file type/);assert.equal(f.calls.some(x=>x.name==='open'),false);
});
test('unapproved intermediate resolution refuses',()=>{
 const f=fixture();f.overrides.realpath=p=>p==='/inert/file'?'/other/file':f.base.realpath(p);refuses(f,/named realpath/);
});
test('alias literal link text must match exactly',()=>{
 const f=fixture();f.overrides.readlink=()=>'./target';refuses(f,/exact named link text/);
});
test('alias canonical target cannot itself resolve elsewhere',()=>{
 const f=fixture();f.overrides.realpath=p=>p==='/inert/target'?'/other/target':f.base.realpath(p);refuses(f);
});
for(const [field,index]of [['device',0],['inode',1],['size',2],['mode',3],['mtime',4],['ctime',5]])test('historical '+field+' identity mismatch refuses before read',()=>{
 const f=fixture();f.overrides.fstat=fd=>{const s=f.base.fstat(fd);s.identity[index]=String(BigInt(s.identity[index])+1n);return s;};refuses(f);assert.equal(f.calls.some(x=>x.name==='read'),false);assert.equal(f.descriptors.size,0);
});
for(const owner of ['uid','gid'])test('loaded '+owner+' is preserved and checked',()=>{
 const f=fixture();f.overrides.lstat=p=>{const s=f.base.lstat(p);if(p==='/inert/file')s[owner]='999';return s;};refuses(f,/loaded owner/);
});
test('descriptor mutation after read refuses and closes',()=>{
 const f=fixture();let n=0;f.overrides.fstat=fd=>{const s=f.base.fstat(fd);if(++n===2)s.identity[5]='999';return s;};refuses(f,/descriptor drift/);assert.equal(f.descriptors.size,0);
});
test('final pass revalidation catches early file replacement',()=>{
 const f=fixture();let n=0;f.overrides.lstat=p=>{const s=f.base.lstat(p);if(p==='/inert/file'&&++n===3)s.identity[1]='999';return s;};refuses(f,/named\/target identity/);
});
const admissionMutations=[
 ['duplicate path',p=>p.entries.push(clone(p.entries[0]))],
 ['absent fake pin',p=>p.entries[3].pin={path:p.entries[3].path,bytes:0,sha256:hash(Buffer.alloc(0))}],
 ['absent fake capacity',p=>p.entries[3].cap=1],
 ['alias target duty omitted',p=>p.entries.splice(1,1)],
 ['wrong alias pin',p=>p.aliases[0].pin.sha256='0'.repeat(64)],
 ['empty role labels',p=>p.entries[0].labels={prior:[],loaded:[],duties:[],extras:[]}],
 ['unknown row field',p=>p.entries[0].extra=true],
 ['path traversal',p=>p.entries[0].path='/inert/../file'],
 ['cap below full member',p=>p.entries[0].cap=1],
 ['wrong byte budget',p=>p.budget.total.reserved_bytes--],
 ['wrong call budget',p=>p.budget.total.content_calls--],
 ['wrong metadata budget',p=>p.budget.total.metadata_calls--],
 ['third pass',p=>p.passes=3],
 ['infinite integer',p=>p.entries[0].pin.bytes=Infinity],
 ['operational authority invented',p=>p.custody.execution_authority=true],
 ['approved cap invented',p=>p.custody.root_cap_change_approved=true],
 ['complete roster invented',p=>p.custody.operational_roster_complete=true],
];
for(const [name,mutate]of admissionMutations)test('admission refuses '+name+' before I/O',()=>{const f=fixture();mutate(f.protocol);refuses(f);assert.equal(f.calls.length,0);});
test('input mutation during a read cannot rewrite the admitted owned snapshot',()=>{
 const f=fixture();f.overrides.read=(...a)=>{f.protocol.entries[0].pin.sha256='0'.repeat(64);return f.base.read(...a);};assert.equal(f.run().io_protocol_completed,true);
});
test('caller-supplied historical flag never turns protocol observation into qualification',()=>{
 const f=fixture();f.protocol.custody.qualified_historical_input_bytes=true;const r=f.run();assert.equal(r.qualified,false);assert.equal(r.provider_identity,'caller-supplied-unqualified');assert.equal(r.operational_roster_complete,false);
});

// Explicit synchronous filesystem provider. Importing opens no path and starts no process.
import fs from 'node:fs';
import {buildHistoricalReadProtocol} from './reader-plan.mjs';
import {readUnqualifiedProtocol} from './reader-protocol.mjs';
const fields=['dev','ino','size','mode','mtimeNs','ctimeNs'];
function stat(s){
 const kind=s.isFile()?'file':s.isDirectory()?'directory':s.isSymbolicLink()?'symlink':'other';
 return{kind,identity:fields.map(k=>String(s[k])),uid:String(s.uid),gid:String(s.gid)};
}
/** Source-level counters count provider calls, not kernel-internal realpath syscalls. */
export function filesystemProvider(){
 return Object.freeze({
  lstat:path=>stat(fs.lstatSync(path,{bigint:true})),
  realpath:path=>fs.realpathSync(path),
  readlink:path=>fs.readlinkSync(path,'utf8'),
  open:path=>fs.openSync(path,fs.constants.O_RDONLY|fs.constants.O_NOFOLLOW|fs.constants.O_NONBLOCK),
  fstat:fd=>stat(fs.fstatSync(fd,{bigint:true})),
  read:(fd,buffer,offset,length)=>fs.readSync(fd,buffer,offset,length,null),
  close:fd=>fs.closeSync(fd),
 });
}
/**
 * Root-only explicit read entry: no default call, CLI, live graph edit, or native launch.
 * The external guard must bind reviewed finite scope/currentness. Supplying one does
 * not itself qualify that scope or approve any input-cap change.
 * The outer scheduler must enforce process timeout: synchronous IO cannot be preempted
 * by a JavaScript deadline. This reader does not claim global writer exclusion.
 */
export function inspectHistoricalLoadedFiles(input,{guard,now,milliseconds=240000}){
 const prepared=buildHistoricalReadProtocol(input);
 const observation=readUnqualifiedProtocol(prepared.protocol,filesystemProvider(),{guard,now,milliseconds});
 return Object.freeze({...observation,provider_identity:'node:fs-explicit-root-call',files_freshly_observed:true,qualified:false,historical_plan:prepared.historical_plan,complete_operational_count:null,complete_operational_budget:null,external_scope_qualification_required:true});
}

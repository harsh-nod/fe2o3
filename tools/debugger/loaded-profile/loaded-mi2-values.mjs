// Separate startup-only framing join. Never imported by the runtime controller.
import crypto from 'node:crypto';
import {isDeepStrictEqual as eq} from 'node:util';
const sha=b=>crypto.createHash('sha256').update(b).digest('hex');
const fail=s=>{throw Error('fixed MI2 startup transport: '+s)};
const NAMES=['schema','interpreter','commands','records','stdout_bytes','stdout_sha256','collector_bytes','collector_sha256','mi_exit_observed','no_inferior_command','runtime_selection','debugger_acceptance'];
function uint(n,lo,hi){if(!Number.isSafeInteger(n)||n<lo||n>hi)fail('integer bound')}
export function validateTransport(raw,frame,receipt){
 // Bounds are checked before decode, splitting, hashing or projection.
 if(!Buffer.isBuffer(raw)||raw.length===0||raw.length>20*1024*1024
  ||!Buffer.isBuffer(frame)||frame.length===0||frame.length>8*1024*1024+32)fail('stream bounds');
 if(!receipt||typeof receipt!=='object'||Array.isArray(receipt)||!eq(Object.keys(receipt).sort(),NAMES.slice().sort()))fail('closed receipt');
 if(receipt.schema!=='fe2o3-fixed-mi2-startup-transport-v1'||receipt.interpreter!=='mi2'
  ||receipt.commands!==2||receipt.mi_exit_observed!==true||receipt.no_inferior_command!==true
  ||receipt.runtime_selection!==false||receipt.debugger_acceptance!==false)fail('receipt domain/claim');
 uint(receipt.records,1,8192);uint(receipt.stdout_bytes,1,20*1024*1024);uint(receipt.collector_bytes,1,8*1024*1024+32);
 let records=0,line=0;
 for(const b of raw){if(++line>2*(8*1024*1024+32)+16)fail('line bound');if(b===10){records++;line=0;}}
 if(line!==0||records!==receipt.records||records>8192)fail('record census/EOF');
 if(receipt.stdout_bytes!==raw.length||receipt.collector_bytes!==frame.length
  ||receipt.stdout_sha256!==sha(raw)||receipt.collector_sha256!==sha(frame))fail('raw/decoded exact hashes');
 const prefix=Buffer.from('FE2O3_CUSTOM_STARTUP_V1 ');
 if(!frame.subarray(0,prefix.length).equals(prefix)||frame[frame.length-1]!==10
  ||frame.subarray(0,-1).includes(10)||frame.subarray(prefix.length,-1).some(b=>b<32||b>126))fail('single ASCII collector frame');
 // Full Python/file/maps/currentness semantics stay in the retained closure validator.
 return {schema:'fe2o3-one-stop-mi2-startup-stream-join-v1',interpreter:'mi2',
  raw_stdout:{bytes:raw.length,sha256:sha(raw)},collector_frame:{bytes:frame.length,sha256:sha(frame)},
  commands:2,records,mi_exit_observed:true,no_inferior_command:true,
  startup_only:true,runtime_selection:false,debugger_acceptance:false};
}

// This early diagnostic precedes MI installation in the pinned GDB source.
// The path is source-owned, not provided by a receipt or a caller override.
const STARTUP_WARNING=Buffer.from('/home/harmenon/fe2o3-authoring-280-282-mi350.4VZ42zNr/phase28-one-stop-adapter-loaded-maintenance-observable-build-r37-r1/build/gdb/gdb: warning: Couldn\'t determine a path for the index cache directory.\n');
export function validateStartupStderr(raw){
 if(!Buffer.isBuffer(raw)||raw.length>1024*1024)fail('stderr bounds');
 if(raw.length!==0&&!raw.equals(STARTUP_WARNING))fail('stderr exact optional warning');
}

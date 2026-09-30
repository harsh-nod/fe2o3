// Deliberately executable static entry. Import API modules instead when no activation is intended.
import {runStaticNodeFilesystem} from './launch-fs.mjs';
try{
 const encoded=process.argv[2];
 if(process.argv.length!==3||typeof encoded!=='string'||encoded.length>4*Math.ceil(65536/3))throw Error('explicit bounded bootstrap argument');
 const raw=Buffer.from(encoded,'base64');
 if(raw.length===0||raw.length>65536||raw.toString('base64')!==encoded)throw Error('canonical bootstrap encoding');
 const result=runStaticNodeFilesystem(raw);
 process.exitCode=result.exit_code;
}catch{
 // An unadmitted bootstrap supplies no reporting authority. The outer supervisor retains nonzero exit.
 // No unbounded stack, fallback stream write, child process or recovery read is attempted.
 process.exitCode=1;
}

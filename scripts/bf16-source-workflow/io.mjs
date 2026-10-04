// Linux-only bounded I/O and direct Cargo supervision within the inherited group.
// External group supervision is required; no cgroup, descendant census or write quota.
import fs from 'node:fs';
import path from 'node:path';
import {spawn} from 'node:child_process';
import {createHash} from 'node:crypto';
import {check,LIMITS,MiB} from './protocol.mjs';
const C=fs.constants;
const stamp=s=>[s.dev,s.ino,s.size,s.mtimeNs,s.ctimeNs,s.mode,s.nlink].map(String).join(':');
export class Work {
  constructor(root, deadline) {
    check(process.platform==='linux','Linux is required');
    const ms=Date.parse(deadline);
    check(Number.isFinite(ms) && new Date(ms).toISOString()===deadline &&
      ms>Date.now() && ms-Date.now()<=LIMITS.totalMs,'deadline must be an explicit future ISO UTC time within 30 minutes');
    this.root=root; this.deadline=ms; this.start=process.hrtime.bigint();
    this.readBytes=0; this.steps=0;
  }
  guard() {
    check(Date.now()<this.deadline && process.hrtime.bigint()-this.start<BigInt(LIMITS.totalMs)*1000000n,'workflow deadline');
  }
  debit(n) {
    check(Number.isSafeInteger(n)&&n>=0&&this.readBytes+n<=LIMITS.readBytes,'read budget');
    this.readBytes+=n;
  }
  read(file, cap) {
    this.guard();
    const fd=fs.openSync(file,C.O_RDONLY|C.O_NOFOLLOW|C.O_NONBLOCK|C.O_CLOEXEC);
    try {
      const s=fs.fstatSync(fd,{bigint:true});
      check(s.isFile() && s.nlink===1n && s.size>=0n && s.size<=BigInt(cap),'bounded regular input');
      const n=Number(s.size); this.debit(n+1);
      const b=Buffer.alloc(n); let at=0;
      while(at<n) { this.guard(); const got=fs.readSync(fd,b,at,n-at,null); check(got>0,'early EOF');at+=got; }
      check(fs.readSync(fd,Buffer.alloc(1),0,1,null)===0,'growing input');
      check(stamp(s)===stamp(fs.fstatSync(fd,{bigint:true})) &&
        stamp(s)===stamp(fs.lstatSync(file,{bigint:true})),'input changed');
      return {bytes:b,stat:s};
    } finally {fs.closeSync(fd);}
  }
  tool(file) {
    this.guard();
    check(path.isAbsolute(file),'tool path must be absolute');
    const real=fs.realpathSync(file), fd=fs.openSync(real,C.O_RDONLY|C.O_NOFOLLOW|C.O_NONBLOCK|C.O_CLOEXEC);
    try {
      const s=fs.fstatSync(fd,{bigint:true});
      check(s.isFile() && s.size>0n && s.size<=128n*BigInt(MiB) && (s.mode&0o111n)!==0n,'tool file');
      const b=Buffer.alloc(65536),hash=createHash('sha256');let left=Number(s.size);
      while(left) {
        this.guard();const want=Math.min(left,b.length);this.debit(want);
        const n=fs.readSync(fd,b,0,want,null);check(n>0,'short tool');hash.update(b.subarray(0,n));left-=n;
      }
      this.debit(1);check(fs.readSync(fd,b,0,1,null)===0,'growing tool');
      check(stamp(s)===stamp(fs.fstatSync(fd,{bigint:true})) &&
        stamp(s)===stamp(fs.lstatSync(real,{bigint:true})),'tool changed');
      return {path:real,bytes:Number(s.size),sha256:hash.digest('hex'),identity:stamp(s)};
    }finally{fs.closeSync(fd);}
  }
  census() {
    this.guard();let count=0,target=0,evidence=0;
    const visit=(dir,depth,inTarget)=>{
      check(depth<=LIMITS.depth,'tree depth');
      // Iterate with bounded count; do not first allocate an unbounded readdir array.
      const handle=fs.opendirSync(dir);
      try {
        for(;;) {
          this.guard();const entry=handle.readSync();if(!entry)break;
          check(++count<=LIMITS.entries,'tree entry count');
          const p=path.join(dir,entry.name),s=fs.lstatSync(p,{bigint:true});
          const isTarget=inTarget||(dir===this.root&&entry.name==='target');
          check(!s.isSymbolicLink(),'symlink in owned output tree');
          if(s.isDirectory())visit(p,depth+1,isTarget);
          else {
            check(s.isFile()&&s.size>=0n&&s.size<=BigInt(LIMITS.target),'nonregular/oversized output');
            const n=Number(s.size);
            if(isTarget)target+=n;else evidence+=n;
            check(target<=LIMITS.target&&evidence<=LIMITS.evidence-4*MiB,'retained output limit');
          }
        }
      }finally{handle.closeSync();}
    };
    visit(this.root,0,false);return {entries:count,target_bytes:target,evidence_bytes:evidence};
  }
  save(relative, value) {
    this.guard();const bytes=Buffer.isBuffer(value)?value:Buffer.from(JSON.stringify(value,null,2)+'\n');
    check(bytes.length<=MiB,'parent record cap');
    const file=path.join(this.root,relative);
    check(file.startsWith(this.root+'/'),'owned record path');
    const before=this.census();
    check(before.evidence_bytes+bytes.length<=LIMITS.evidence-4*MiB,'record prepayment');
    saveNew(file,bytes);this.guard();return file;
  }
}
export function saveNew(file,bytes) {
  const fd=fs.openSync(file,C.O_WRONLY|C.O_CREAT|C.O_EXCL|C.O_NOFOLLOW|C.O_CLOEXEC,0o600);
  try{let n=0;while(n<bytes.length){const got=fs.writeSync(fd,bytes,n,bytes.length-n);check(got>0,'short write');n+=got;}fs.fsyncSync(fd);}
  finally{fs.closeSync(fd);}
  const parent=fs.openSync(path.dirname(file),C.O_RDONLY|C.O_DIRECTORY|C.O_CLOEXEC);
  try{fs.fsyncSync(parent);}finally{fs.closeSync(parent);}
}
export function reserveStreamPrefix(admitted, index, bytes) {
  check((index===0||index===1)&&Number.isSafeInteger(bytes)&&bytes>=0,'stream credit input');
  check(Number.isSafeInteger(admitted[index])&&admitted[index]>=0&&admitted[index]<=LIMITS.stream,'stream credit state');
  const keep=Math.min(LIMITS.stream-admitted[index],bytes);
  admitted[index]+=keep;return keep;
}
export function cleanTerminal(t) {
  return t.reason===null&&t.exit_code===0&&t.exit_signal===null&&t.close_code===0&&
    t.close_signal===null&&t.stdout_end&&t.stderr_end&&!t.drain_abandoned;
}
// /proc identity only protects the direct child PID. Descendants stay in the
// inherited group and require the external group owner on parent death/hang.
export function parseStartTime(text,pid) {
  check(Number.isSafeInteger(pid)&&pid>0&&text.startsWith(String(pid)+' ('),'child PID');
  const end=text.lastIndexOf(') ');check(end>0,'child stat framing');
  const fields=text.slice(end+2).trim().split(/\s+/);
  check(fields.length>=20&&/^[1-9][0-9]*$/.test(fields[19]),'child start time');
  return fields[19];
}
function startTime(pid) {
  const fd=fs.openSync('/proc/'+pid+'/stat',C.O_RDONLY|C.O_NOFOLLOW|C.O_NONBLOCK|C.O_CLOEXEC);
  try {
    const b=Buffer.alloc(4096);let n=0;
    while(n<b.length){const got=fs.readSync(fd,b,n,b.length-n,null);if(got===0)break;n+=got;}
    check(n>0&&fs.readSync(fd,Buffer.alloc(1),0,1,null)===0,'child stat cap');
    return parseStartTime(b.subarray(0,n).toString('utf8'),pid);
  }finally{fs.closeSync(fd);}
}
export function paidProcReader(work,read=startTime) {
  work.debit(3*4097); // Capture, TERM, KILL: prepaid even if not all are needed.
  let remaining=3;
  return pid=>{
    check(remaining>0,'direct child identity read slots');
    remaining--;return read(pid); // No expired-clock check may suppress paid cleanup.
  };
}
export function signalOwnedPid(pid,token,signal,read=startTime,kill=process.kill.bind(process)) {
  check(Number.isSafeInteger(pid)&&pid>0&&typeof token==='string'&&/^[1-9][0-9]*$/.test(token),'no captured direct child identity');
  check(signal==='SIGTERM'||signal==='SIGKILL','cleanup signal');
  check(read(pid)===token,'direct child PID identity changed');
  kill(pid,signal); // Positive PID only; never signal an unowned new process group.
}

export async function cargoChild(work, name, executable, args, cwd, env) {
  work.guard();check(++work.steps<=5,'exact five Cargo calls');
  const readChildStart=paidProcReader(work);
  // Streams reserve 16 MiB before spawn; terminal/parent records retain separate headroom.
  const usage=work.census();check(usage.evidence_bytes+18*MiB<=LIMITS.evidence-4*MiB,'child output reservation');
  work.save(name+'.started.json',{name,executable,args,cwd,selected_env:Object.fromEntries(
    Object.entries(env).filter(([k])=>k.startsWith('FE2O3_')||k==='RUSTC'||k==='RUSTC_WRAPPER'||k==='CARGO_ENCODED_RUSTFLAGS')),
    started:new Date().toISOString(),descendants:'Cargo-managed; no fixed total process-count claim'});
  const files=[name+'.stdout',name+'.stderr'].map(n=>path.join(work.root,n));
  const fds=files.map(p=>fs.openSync(p,C.O_WRONLY|C.O_CREAT|C.O_EXCL|C.O_NOFOLLOW|C.O_CLOEXEC,0o600));
  const t={name,reason:null,pid:null,start_time:null,process_group:'inherited',exit_code:null,exit_signal:null,close_code:null,close_signal:null,
    stdout_end:false,stderr_end:false,drain_abandoned:false,stdout_bytes:0,stderr_bytes:0,
    discarded_stdout_bytes:0,discarded_stderr_bytes:0,signals:[],io_errors:[]};
  const admitted=[0,0];
  const ioError=e=>{if(t.io_errors.length<8)t.io_errors.push(String(e?.message??e).slice(0,256));};
  let child,killTimer,drainTimer,interval,stageTimer,closed=false;
  const started=process.hrtime.bigint();
  const signal=s=>{
    if(!child?.pid)return;
    try{signalOwnedPid(child.pid,t.start_time,s,readChildStart);t.signals.push({signal:s,result:'sent_to_original_direct_pid'});}
    catch(e){t.signals.push({signal:s,result:e.code||'identity_or_signal_refusal'});}
  };
  const fail=reason=>{
    if(t.reason!==null)return;t.reason=String(reason).slice(0,1024);signal('SIGTERM');
    // Referenced timers deliberately survive close and outer catch: no process.exit().
    killTimer=setTimeout(()=>signal('SIGKILL'),1000);
    drainTimer=setTimeout(()=>{if(!closed){t.drain_abandoned=true;child?.stdout.destroy();child?.stderr.destroy();}},5000);
  };
  const onSignal=s=>()=>fail('parent '+s);
  const onInt=onSignal('SIGINT'),onTerm=onSignal('SIGTERM');
  process.on('SIGINT',onInt);process.on('SIGTERM',onTerm);
  try {
    await new Promise(resolve=>{
      try {child=spawn(executable,args,{cwd,env,stdio:['ignore','pipe','pipe'],detached:false});t.pid=child.pid??null;}
      catch(e){fail('spawn '+e.message);closed=true;resolve();return;}
      child.once('error',e=>fail('spawn '+e.message));
      if(child.pid){try{t.start_time=readChildStart(child.pid);}catch(e){fail('direct child identity unavailable');}}
      [child.stdout,child.stderr].forEach((stream,index)=>{
        const k=index?'stderr':'stdout';
        stream.on('data',b=>{
          const keep=reserveStreamPrefix(admitted,index,b.length);
          // Prepaid before write; a partial I/O error never refunds the cap.
          try {
            let at=0;while(at<keep){const n=fs.writeSync(fds[index],b,at,keep-at);check(n>0,'stream short write');at+=n;t[k+'_bytes']+=n;}
            if(keep)fs.fsyncSync(fds[index]);work.guard();
          }catch(e){ioError(e);fail('stream write/clock');}
          if(keep!==b.length){t['discarded_'+k+'_bytes']+=b.length-keep;fail(k+' cap');}
        });
        stream.once('end',()=>{t[k+'_end']=true;});
        stream.once('error',e=>fail(k+' '+e.message));
      });
      child.once('exit',(code,sig)=>{
        t.exit_code=code;t.exit_signal=sig;
        if(code!==0||sig!==null)fail('nonzero direct exit');
        if(!drainTimer)drainTimer=setTimeout(()=>{if(!closed){fail('EOF drain timeout');}},5000);
      });
      child.once('close',(code,sig)=>{
        t.close_code=code;t.close_signal=sig;closed=true;
        if(code!==0||sig!==null)fail('nonzero direct close');
        resolve();
      });
      stageTimer=setTimeout(()=>fail('stage timeout'),LIMITS.stageMs);
      interval=setInterval(()=>{try{work.guard();work.census();}catch(e){fail(e.message);}},1000);
    });
  }finally{
    clearTimeout(stageTimer);clearInterval(interval);
    if(closed)clearTimeout(drainTimer); // SIGKILL timer remains live on failure.
    process.removeListener('SIGINT',onInt);process.removeListener('SIGTERM',onTerm);
    for(const fd of fds){try{fs.fsyncSync(fd);}catch(e){ioError(e);t.reason??='stream fsync';}try{fs.closeSync(fd);}catch{}}
  }
  t.elapsed_ns=String(process.hrtime.bigint()-started);
  // Even a deadline/census refusal must retain this original terminal record.
  saveNew(path.join(work.root,name+'.terminal.json'),Buffer.from(JSON.stringify(t,null,2)+'\n'));
  check(cleanTerminal(t),'Cargo stage failed: '+name+'; retain all outputs');
  work.guard();work.census();return t;
}

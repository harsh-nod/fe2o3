import test from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import {Work,saveNew,reserveStreamPrefix,parseStartTime,signalOwnedPid,paidProcReader} from './bf16-source-workflow/io.mjs';
import {LIMITS} from './bf16-source-workflow/protocol.mjs';
function fixture(t) {
  const root=fs.mkdtempSync(path.join(os.tmpdir(),'fe2o3-cargo-source-control-'));
  t.after(()=>fs.rmSync(root,{recursive:true}));
  return {root,work:new Work(root,new Date(Date.now()+60000).toISOString())};
}
test('read exact cap and create-new output retain original bytes',t=>{
  const {root,work}=fixture(t),p=path.join(root,'input');
  saveNew(p,Buffer.from('exact'));
  assert.equal(work.read(p,5).bytes.toString(),'exact');
  assert.throws(()=>saveNew(p,Buffer.from('replace')));
  assert.equal(work.read(p,5).bytes.toString(),'exact');
});
test('bounded read refuses overflow and symlink; no input deletion',t=>{
  const {root,work}=fixture(t),p=path.join(root,'input'),alias=path.join(root,'alias');
  saveNew(p,Buffer.alloc(17));assert.throws(()=>work.read(p,16));
  fs.symlinkSync(p,alias);assert.throws(()=>work.read(alias,17));
  assert.throws(()=>work.census());assert.ok(fs.existsSync(p));
});
test('output census separates target and evidence without following aliases',t=>{
  const {root,work}=fixture(t);
  fs.mkdirSync(path.join(root,'target'));
  saveNew(path.join(root,'target','object'),Buffer.alloc(7));
  saveNew(path.join(root,'report'),Buffer.alloc(3));
  assert.deepEqual(work.census(),{entries:3,target_bytes:7,evidence_bytes:3});
});
test('stream prefix credit is prepaid, sticky and independent for both pipes',()=>{
  const admitted=[0,0];
  assert.equal(reserveStreamPrefix(admitted,0,LIMITS.stream-1),LIMITS.stream-1);
  // A partial writer may fail now, but cannot refund this reservation.
  assert.equal(reserveStreamPrefix(admitted,0,2),1);
  assert.equal(reserveStreamPrefix(admitted,0,100),0);
  assert.equal(reserveStreamPrefix(admitted,1,2),2);
  assert.deepEqual(admitted,[LIMITS.stream,2]);
  assert.throws(()=>reserveStreamPrefix(admitted,0,-1));
});

test('direct PID identity parser handles parenthesized comm and refuses malformed rows',()=>{
  const fields=['S',...Array(18).fill('0'),'123456',...Array(5).fill('0')];
  assert.equal(parseStartTime('123 (cargo (worker)) '+fields.join(' ')+'\n',123),'123456');
  assert.throws(()=>parseStartTime('124 (cargo) '+fields.join(' '),123));
  assert.throws(()=>parseStartTime('123 (cargo) S 0',123));
});
test('cleanup signals only the captured positive PID and refuses reused identities',()=>{
  const calls=[],kill=(pid,s)=>calls.push([pid,s]);
  signalOwnedPid(123,'42','SIGTERM',()=> '42',kill);
  assert.deepEqual(calls,[[123,'SIGTERM']]);
  assert.throws(()=>signalOwnedPid(123,'42','SIGKILL',()=> '43',kill));
  assert.throws(()=>signalOwnedPid(123,null,'SIGKILL',()=> '42',kill));
  assert.equal(calls.length,1);
});

test('three bounded proc reads are prepaid before use and remain usable after deadline',()=>{
  let paid=0,calls=0;
  const reader=paidProcReader({debit:n=>{paid+=n;},guard:()=>{throw Error('expired');}},pid=>{
    assert.equal(pid,123);calls++;return '42';
  });
  assert.equal(paid,3*4097);assert.equal(calls,0);
  assert.equal(reader(123),'42');assert.equal(reader(123),'42');assert.equal(reader(123),'42');
  assert.throws(()=>reader(123));assert.equal(calls,3);assert.equal(paid,3*4097);
});

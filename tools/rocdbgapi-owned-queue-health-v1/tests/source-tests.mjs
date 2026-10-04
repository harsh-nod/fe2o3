// SPDX-License-Identifier: MIT
import test from 'node:test';
import assert from 'node:assert/strict';
import {Account,CAPS,exactEdits} from './source-core.mjs';
import {verifyPackage} from '../verify-package.mjs';
test('closed package pins ancestry transforms and exact mock sections verify without native authority',()=>{
 const result=verifyPackage();assert.equal(result.native_qualified,false);assert.equal(result.milestone_complete,false);
 assert.equal(result.changed_sources,6);assert.equal(result.postimages,7);assert.equal(result.edits,26);
});
test('content account caps are explicit and may only decrease',()=>{
 assert.throws(()=>new Account({bytes:CAPS.bytes+1}));assert.throws(()=>new Account({reset:1}));assert.throws(()=>verifyPackage({lower:{manifest:1}}));
 for(const k of ['bytes','calls','roles','work','expansion'])assert.throws(()=>new Account({[k]:-1}));
});
test('requested content and EOF calls are prepaid before read',()=>{
 const a=new Account();a.admit('member',65537);
 assert.deepEqual(a.usage(),{bytes:65538,calls:3,roles:1,work:524304,expansion:0,failed:false});
});
test('one-short content call and role limits refuse sticky without refund',()=>{
 for(const lower of [{bytes:1},{calls:1},{roles:0},{work:7}]){
  const a=new Account(lower);let first;try{a.admit('x',1);}catch(e){first=e;}
  assert(first);assert.throws(()=>a.admit('later',1),e=>e===first);assert(a.usage().failed);
 }
});
test('duplicate roles and oversized members refuse',()=>{
 const a=new Account();a.admit('x',1);assert.throws(()=>a.admit('x',1));
 assert.throws(()=>new Account().admit('x',CAPS.perFile+1));
});
test('exact edits reconstruct both directions with explicit expansion',()=>{
 const e=[{before:'old',after:'new'}],a=new Account();
 assert.equal(exactEdits(a,'a old b',e),'a new b');
 assert.equal(exactEdits(a,'a new b',e,'inverse'),'a old b');assert.equal(a.usage().expansion,14);
});
test('missing duplicate edit anchors and overbudget reconstruction refuse sticky',()=>{
 for(const s of ['none','old old']){const a=new Account();assert.throws(()=>exactEdits(a,s,[{before:'old',after:'new'}]));assert(a.usage().failed);}
 assert.throws(()=>exactEdits(new Account({expansion:6}),'a old b',[{before:'old',after:'new'}]));
});
test('limits and usage snapshots cannot increase internal authority',()=>{
 const a=new Account({bytes:2}),l=a.limits(),u=a.usage();l.bytes=100;u.bytes=-100;
 a.admit('x',1);assert.throws(()=>a.admit('y',1));
});

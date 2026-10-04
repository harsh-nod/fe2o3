// SPDX-License-Identifier: MIT
// Closed source-only accounting; not a native, heap, RSS or kernel-work budget.
import assert from 'node:assert/strict';
export const CAPS=Object.freeze({bytes:2097152,calls:256,roles:64,work:67108864,expansion:8388608,perFile:1048576,manifest:65536,chunk:65536,timeout_ms:10000});
export class Account {
 #limits;#failure=null;#used={bytes:0,calls:0,roles:0,work:0,expansion:0};#roles=new Set();
 constructor(lower={}){for(const[k,v]of Object.entries(lower)){assert(Object.hasOwn(CAPS,k));assert(Number.isSafeInteger(v)&&v>=0&&v<=CAPS[k]);}this.#limits={...CAPS,...lower};}
 run(fn){if(this.#failure)throw this.#failure;try{return fn();}catch(e){this.#failure=e;throw e;}}
 charge(next){return this.run(()=>{for(const[k,v]of Object.entries(next)){assert(Object.hasOwn(this.#used,k));assert(Number.isSafeInteger(v)&&v>=0);assert(Number.isSafeInteger(this.#used[k]+v)&&this.#used[k]+v<=this.#limits[k]);}for(const[k,v]of Object.entries(next))this.#used[k]+=v;});}
 admit(role,bytes){return this.run(()=>{assert(typeof role==='string'&&role.length<=256&&!this.#roles.has(role));assert(Number.isSafeInteger(bytes)&&bytes>0&&bytes<=this.#limits.perFile);this.charge({bytes:bytes+1,calls:Math.ceil(bytes/this.#limits.chunk)+1,roles:1,work:8*(bytes+1)});this.#roles.add(role);});}
 usage(){return {...this.#used,failed:this.#failure!==null};}
 limits(){return {...this.#limits};}
}
export function exactEdits(account,source,edits,direction='forward'){
 return account.run(()=>{assert(typeof source==='string');assert(Array.isArray(edits)&&edits.length<=26);assert(['forward','inverse'].includes(direction));
 let value=source;const list=direction==='forward'?edits:[...edits].reverse();
 for(const e of list){const from=direction==='forward'?e.before:e.after,to=direction==='forward'?e.after:e.before;
  assert(typeof from==='string'&&from.length>0&&typeof to==='string');
  const nextBytes=Buffer.byteLength(value)-Buffer.byteLength(from)+Buffer.byteLength(to);
  assert(nextBytes>=0&&nextBytes<=account.limits().perFile);
  account.charge({work:8*(Buffer.byteLength(value)+Buffer.byteLength(from)+Buffer.byteLength(to)),expansion:nextBytes});
  const i=value.indexOf(from);assert(i>=0&&value.indexOf(from,i+from.length)<0,'unique exact edit');
  value=value.slice(0,i)+to+value.slice(i+from.length);
 }return value;});
}

// Synthetic parser-only rows; never represented as native/source observations.
import test from 'node:test';
import assert from 'node:assert/strict';
import {checkRoster,mutationRoster} from './check-report.mjs';
function rows(){return mutationRoster.map(mutation=>({mutation,accepted:false,
  reason:'synthetic exact refusal',sha256:'0'.repeat(64),bytes:4096}));}
test('exact independent 83-name mutation roster',()=>{assert.equal(mutationRoster.length,83);checkRoster(rows());});
for(const [name,change] of [
 ['missing',r=>r.pop()],['duplicate',r=>r[3].mutation=r[2].mutation],
 ['corrupt',r=>r[3].mutation='dangling-stringref'],
 ['reordered',r=>[r[1],r[2]]=[r[2],r[1]]],
 ['unexpected',r=>r.push({...r[0],mutation:'extra'})],
 ['accepted',r=>r[0].accepted=true],['reason_empty',r=>r[0].reason=''],
 ['reason_over',r=>r[0].reason='x'.repeat(4097)],['digest_corrupt',r=>r[0].sha256='g'.repeat(64)],
 ['bytes_over',r=>r[0].bytes=1048578],['bytes_negative',r=>r[0].bytes=-1],
 ['bytes_fraction',r=>r[0].bytes=1.5]
]) test('refuses '+name,()=>{const r=rows();change(r);assert.throws(()=>checkRoster(r));});

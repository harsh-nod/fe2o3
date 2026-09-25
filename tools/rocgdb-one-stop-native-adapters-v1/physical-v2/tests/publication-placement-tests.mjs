// Source-only controls. No GDB/source module is imported or executed.
import test from 'node:test';
import assert from 'node:assert/strict';
import {finalSource} from './source-files.mjs';
const files=finalSource();
const read=n=>files['gdb/'+n];
const inherited=read;
const baseline={pub:read('amd-dbgapi-one-stop-publication-v2.h'),snap:read('amd-dbgapi-one-stop-snapshot-v1.h'),header:read('amd-dbgapi-one-stop-native-v1.h'),owner:read('amd-dbgapi-one-stop-native-owner-v1.inc'),resume:read('amd-dbgapi-one-stop-native-resume-v1.inc'),events:inherited('amd-dbgapi-one-stop-native-events-v1.inc'),capture:inherited('amd-dbgapi-one-stop-native-snapshot-v1.inc')};
function section(s,a,b){const x=s.indexOf(a),y=s.indexOf(b,x+a.length);assert(x>=0&&y>x);return s.slice(x,y)}
function order(s,items){let at=-1;for(const i of items){const p=s.indexOf(i,at+1);assert(p>at,i);at=p}}
function valid(s){
 assert(s.pub.includes('snapshot_publication_available () noexcept { return false; }'));
 assert(s.snap.includes('snapshot_capture_available () noexcept { return false; }'));
 const select=section(s.owner,'void native_adapter::select_at_entry ()','void native_adapter::check_owned_breakpoint_owner');
 order(select,['!snapshot_capture_available ()','!snapshot_publication_available ()','m_owner.reserve_selection']);
 assert(s.pub.includes('capacity=1536'));
 assert(s.pub.includes('physical ? 3*capacity : 256'));
 assert(s.header.includes('snapshot_publication m_output;'));
 assert(s.header.includes('sizeof (native_adapter) + 4096 + 1024 + limits::client_output_bytes'));
 const input=section(s.resume,'void native_adapter::mi_input (','void native_adapter::nested_mi');
 order(input,['if (!selected ()) return;','m_snapshot.revoke (snapshot_issue::resumed);','m_output.forget ();','require (text!=nullptr','std::strcmp']);
 const ack=section(s.events,'void native_adapter::wave_ack (','void native_adapter::after_host_or_gpu_stop');
 order(ack,['query_stop (thread,false)','m_owner.stopped_after_ack','m_gpu_thread_ref=','capture_physical_snapshot']);
 const stop=section(s.events,'void native_adapter::after_host_or_gpu_stop','void native_adapter::deleting_breakpoint');
 order(stop,['query_stop (thread,true)','m_owner.gpu_stop_output_completed','confirm_physical_snapshot']);
 const retire=section(s.events,'void native_adapter::after_auto_delete','void native_adapter::disappearing');
 order(retire,['m_stop_retired=true','seal_physical_snapshot','flush ()']);
 const completion=section(s.resume,'} else if (m_owner.state ()==phase::current_gpu_stop)','void native_adapter::after_amd_cpu_resume');
 order(completion,['query_stop (m_gpu_thread,false)','m_owner.consume_completion','m_snapshot.revoke','m_path=path::completion']);
 const flush=s.resume.slice(s.resume.indexOf('void native_adapter::flush ()'));
 order(flush,['m_owner.next_diagnostic','m_owner.consumed','debit (counter::work,snapshot_publication::work_for','m_output.render','m_output.still_current','gdb_puts','gdb_flush','m_output.still_current','m_output.forget']);
 assert(flush.includes('physical ? current_identity () : m_owner.m_identity'));
 assert.equal((flush.match(/gdb_puts/g)||[]).length,1);
 const failed=flush.slice(flush.indexOf('catch (...)'));
 order(failed,['m_output.forget','m_snapshot.revoke','m_owner.poison','throw;']);
 assert(!flush.includes('amd_dbgapi_')); assert(!flush.includes('query_stop'));
 assert.equal((s.pub.match(/=fe2o3-owned-one-stop-v2/g)||[]).length,1);
 assert(!s.pub.includes('=fe2o3-owned-one-stop-v1'));
 assert(!/new |malloc|std::vector|std::string|target_read|amd_dbgapi_read/.test(s.pub));
 order(s.pub,['if (!prepaid','if (!still_current','m_size=0; m_bytes[0]=0;','hex (s.m_register)','hex (s.m_output)']);
}
test('actual inherited stop/ACK/query and post-retirement V2 placement',()=>valid(baseline));
test('source false and original ledger before any format',()=>{
 valid(baseline);
 for(const [k,a,b] of [
 ['pub','snapshot_publication_available () noexcept { return false; }','snapshot_publication_available () noexcept { return true; }'],
 ['owner','|| !snapshot_publication_available ()',''],
 ['header','snapshot_publication m_output;','char m_output[1536];'],
 ['resume','debit (counter::work,snapshot_publication::work_for (physical));',''],
 ['pub','if (!prepaid (o,before,physical)','if (false'],
 ['pub','if (!still_current (o,id,s,stop)) return false;',''],
 ]){assert(baseline[k].includes(a));assert.throws(()=>valid({...baseline,[k]:baseline[k].replace(a,b)}));}
});
test('every next MI input revokes before parsing without read exception',()=>{
 valid(baseline);
 for(const a of ['m_snapshot.revoke (snapshot_issue::resumed); // EVERY next input, before parsing.','m_output.forget (); // Serialized history is never a live value accessor.']){
  assert(baseline.resume.includes(a));assert.throws(()=>valid({...baseline,resume:baseline.resume.replace(a,'')}));
 }
});
test('both same-stop fences and actual post-retirement seal are mandatory',()=>{
 valid(baseline);
 for(const [k,a] of [['events','query_stop (thread,true)'],['events','m_stop_retired=true;'],['events','seal_physical_snapshot ();'],['resume','query_stop (m_gpu_thread,false)']]){
  assert(baseline[k].includes(a));assert.throws(()=>valid({...baseline,[k]:baseline[k].replace(a,'removed')}));
 }
 const first=baseline.resume.indexOf('m_output.still_current');const last=baseline.resume.lastIndexOf('m_output.still_current');
 assert(first!==last);
 for(const at of [first,last])assert.throws(()=>valid({...baseline,resume:baseline.resume.slice(0,at)+'removed'+baseline.resume.slice(at+'m_output.still_current'.length)}));
});
test('no additional capture calls, reads, commands or diagnostic allocations',()=>{
 valid(baseline);
 assert.equal((baseline.capture.match(/api \(\);/g)||[]).length,3);
 assert.equal((baseline.capture.match(/amd_dbgapi_read_register/g)||[]).length,1);
 assert.equal((baseline.capture.match(/amd_dbgapi_read_memory/g)||[]).length,1);
 assert(!/counter::diagnostic_rows|\.note \(/.test(baseline.pub+baseline.resume));
 const input=section(baseline.resume,'void native_adapter::mi_input (','void native_adapter::nested_mi');
 assert.equal((input.match(/std::strcmp/g)||[]).length,2);
 assert(input.includes('"-exec-continue"')&&input.includes('"-gdb-exit"'));
});
test('fixed formatter length and inherited pre-sink work subbudget',()=>{
 const n='18446744073709551615';const keys=['s','p','pid','i','a','t','start','g','w','wg','d','q','ag','ar','pc','c','cp','co','entry','desc','kernarg','out','regid','dwarf','rbytes','mbytes'];
 const base='=fe2o3-owned-one-stop-v2';const fields=k=>k.map(k=>','+k+'="'+n+'"').join('');
 assert.equal((base+fields(keys.slice(0,7))+'\n').length,206);
 const row=base+fields(keys)+',status="register-unavailable",reg="'+'f'.repeat(8)+'",mem="'+'f'.repeat(544)+'"\n';
 assert.equal(row.length,1316);assert(row.length+1<=1536);
 assert.equal(87168+13568+8192+1528+48+256+640+2080+340+192+128+508+4096+15*256+3*1536,127192);
 assert.equal(131072-127192,3880);
});

test('partial/throwing/unknown output is poison with no retry',()=>{
 valid(baseline);
 const marker='m_output.forget (); m_snapshot.revoke (snapshot_issue::owner_changed);';
 assert(baseline.resume.includes(marker));
 assert.throws(()=>valid({...baseline,resume:baseline.resume.replace(marker,'m_output.forget ();')}));
 assert.throws(()=>valid({...baseline,resume:baseline.resume.replace('m_owner.poison (failure::output); throw;','throw;')}));
});

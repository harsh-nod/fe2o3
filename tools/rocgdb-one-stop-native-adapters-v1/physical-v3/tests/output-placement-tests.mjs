// Source-only controls; never import or execute GDB, a hook, or native process.
import test from 'node:test';
import assert from 'node:assert/strict';
import crypto from 'node:crypto';
import {finalSource,readBounded,packageText} from './source-files.mjs';
const files=finalSource();
const here=n=>n.startsWith('gdb/')?files[n]:readBounded(new URL('../'+n,import.meta.url).pathname,131072).toString('utf8');
const inherited=n=>files['gdb/'+n];
const baseline={stdio:here('gdb/ui-file.h'),pager:here('gdb/pager.h'),guard:here('gdb/amd-dbgapi-one-stop-output-v2.h'),header:here('gdb/amd-dbgapi-one-stop-native-v1.h'),resume:here('gdb/amd-dbgapi-one-stop-native-resume-v1.inc'),fixture:here('tests/introspection-fixtures.inc')};
function order(s,items){let pos=-1;for(const item of items){const n=s.indexOf(item,pos+1);assert(n>pos,item);pos=n;}}
function method(s,name){const start=s.indexOf('bool '+name+' (');assert(start>=0);const open=s.indexOf('{',start);let depth=1,end=open+1;for(;end<s.length&&depth;end++){if(s[end]==='{')depth++;else if(s[end]==='}')depth--;}assert.equal(depth,0);return s.slice(start,end);}
function valid(s){
 for(const [key,name] of [['stdio','one_stop_borrowed_stdio_matches'],['pager','one_stop_borrowed_output_matches']])assert.equal(method(s[key],name),method(s.fixture,name));
 const stdio=method(s.stdio,'one_stop_borrowed_stdio_matches');
 for(const text of ['typeid (*this)==typeid (stdio_file)','expected!=nullptr','m_file==expected','!m_close_p'])assert(stdio.includes(text));
 const pager=method(s.pager,'one_stop_borrowed_output_matches');
 for(const text of ['typeid (*this)!=typeid (pager_file)','m_paging','!m_wrap_buffer.empty ()','m_wrap_column!=0','m_wrap_indent!=0','dynamic_cast<const stdio_file *> (m_stream)','stdio!=nullptr','stdio->one_stop_borrowed_stdio_matches (expected)'])assert(pager.includes(text));
 for(const x of [stdio,pager])assert(!/fileno|m_fd|clearerr|fflush|fputs|new |malloc|return m_file/.test(x));
 assert(s.guard.includes('publication_output ()=delete;'));
 assert(s.guard.includes('scratch_bytes=256')&&s.guard.includes('work_per_row=128'));
 assert(s.guard.includes('sizeof (Current)+sizeof (Put)+sizeof (Flush)'));
 assert(s.guard.includes('+12*sizeof (void *)+2*sizeof (FILE *)<=scratch_bytes'));
 order(s.guard,['if (file==nullptr || !current () || std::ferror (file)!=0) return false;','put ();','flush ();','return current () && std::ferror (file)==0;']);
 assert.equal((s.guard.match(/std::ferror/g)||[]).length,2);
 assert.equal((s.guard.match(/put \(\);/g)||[]).length,1);
 assert.equal((s.guard.match(/flush \(\);/g)||[]).length,1);
 assert(!/clearerr|fputs|fflush|fread|fwrite|fileno|malloc|std::vector/.test(s.guard));
 const flush=s.resume.slice(s.resume.indexOf('void native_adapter::flush ()'));
 order(flush,['if (!selected () || m_owner.invalid ()) return;','m_owner.next_diagnostic ()','debit (counter::work,publication_output::work_per_row);','auto *const observed_ui=m_ui;','const auto sink_current=','const auto before=m_owner.consumed ();','debit (counter::work,snapshot_publication::work_for (physical));','m_output.render','m_output.still_current','publication_output::submit_once','gdb_puts','gdb_flush','m_output.still_current','m_output.forget']);
 for(const x of ['current_ui==observed_ui && m_ui==observed_ui','top_level_interpreter ()==observed_interpreter','m_interpreter==observed_interpreter','mi==as_mi_interp (observed_interpreter)','mi->raw_stdout==raw','observed_ui->outstream==output','mi->saved_raw_stdout==nullptr','!mi->logfile_holder && !mi->stdout_holder','mi->mi_uiout!=nullptr','mi->interp_ui_out ()==mi->mi_uiout','mi->mi_uiout->is_mi_like_p ()','pager!=nullptr','pager->one_stop_borrowed_output_matches (output)'])assert(flush.includes(x),x);
 assert.equal((flush.match(/gdb_puts \(/g)||[]).length,1);assert.equal((flush.match(/gdb_flush \(/g)||[]).length,1);
 assert(!/amd_dbgapi_|query_stop|clearerr|gdb_stdout|fileno|fwrite|fread/.test(flush));
 order(flush.slice(flush.indexOf('catch (...)')),['m_output.forget ();','m_snapshot.revoke (snapshot_issue::owner_changed);','m_owner.poison (failure::output);','throw;']);
 assert(s.header.includes('snapshot_scalar_scratch = 1024'));
 assert(s.header.includes('+ limits::client_output_bytes + publication_output::scratch_bytes;'));
 assert(s.header.includes('sizeof (native_adapter) + 4096 + 1024 + 256 + limits::client_output_bytes + 256'));
}
test('exact closed borrowed chain and actual checked submit placement',()=>valid(baseline));
test('unknown, owning, pending, and changed wrapper guards are mandatory',()=>{
 valid(baseline);
 for(const [key,needle] of [['stdio','typeid (*this)==typeid (stdio_file)'],['stdio','m_file==expected'],['stdio','!m_close_p'],['pager','typeid (*this)!=typeid (pager_file)'],['pager','m_paging'],['pager','!m_wrap_buffer.empty ()'],['pager','m_wrap_column!=0'],['pager','m_wrap_indent!=0']]){
  assert(baseline[key].includes(needle));assert.throws(()=>valid({...baseline,[key]:baseline[key].replace(needle,'false')}));
 }
});
test('same FILE sticky errors bracket the original single submit',()=>{
 valid(baseline);
 for(const needle of [' || std::ferror (file)!=0',' && std::ferror (file)==0','!current () || ','current () && ']){
  assert(baseline.guard.includes(needle));assert.throws(()=>valid({...baseline,guard:baseline.guard.replace(needle,'')}));
 }
 assert.throws(()=>valid({...baseline,guard:baseline.guard.replace('flush ();','flush (); flush ();')}));
});
test('selected UI interpreter raw FILE and logging identities stay exact',()=>{
 valid(baseline);
 for(const needle of ['current_ui==observed_ui && m_ui==observed_ui','top_level_interpreter ()==observed_interpreter','m_interpreter==observed_interpreter','mi==as_mi_interp (observed_interpreter)','mi->raw_stdout==raw','observed_ui->outstream==output','mi->saved_raw_stdout==nullptr','!mi->logfile_holder && !mi->stdout_holder','mi->mi_uiout->is_mi_like_p ()'])assert.throws(()=>valid({...baseline,resume:baseline.resume.replace(needle,'true')}));
});
test('work precedes all added row checks and formatter floor; storage separate',()=>{
 valid(baseline);
 for(const [key,needle] of [['resume','debit (counter::work,publication_output::work_per_row);'],['header','+ publication_output::scratch_bytes'],['guard','+12*sizeof (void *)+2*sizeof (FILE *)<=scratch_bytes']])assert.throws(()=>valid({...baseline,[key]:baseline[key].replace(needle,'')}));
 assert.equal(127192+16*128,129240);assert.equal(131072-129240,1832);
});
test('nonthrowing failure takes existing poison/revoke catch with no retry',()=>{
 valid(baseline);
 for(const needle of ['m_snapshot.revoke (snapshot_issue::owner_changed);','m_owner.poison (failure::output);','publication_output::submit_once'])assert.throws(()=>valid({...baseline,resume:baseline.resume.replace(needle,'removed')}));
});
test('after maintenance inverse the inherited sink-only and unselected paths are exact',()=>{
 let restored=baseline.resume;
 const transforms=JSON.parse(packageText('ordinary-transforms.json'));
 for(const op of transforms.operations.filter(x=>x.file==='amd-dbgapi-one-stop-native-resume-v1.inc').reverse()){
  assert.equal(restored.split(op.after).length,2);restored=restored.replace(op.after,op.before);
 }
 const a=restored.indexOf('      // Prepay every added sink check');const b=restored.indexOf('      const auto before=m_owner.consumed ();',a);assert(a>=0&&b>a);
 restored=restored.slice(0,a)+restored.slice(b);
 const x=restored.indexOf('      // The pinned stdio methods');const y=restored.indexOf('      if (physical)',x);assert(x>=0&&y>x);
 restored=restored.slice(0,x)+'      gdb_puts (m_output.data (),mi->raw_stdout); gdb_flush (mi->raw_stdout);\n'+restored.slice(y);
 assert.equal(crypto.createHash('sha256').update(restored).digest('hex'),'322166a7a5b83795be67ec5ab2ab355b754bff0ba0825354fff021a934044c75');
 const owner=inherited('amd-dbgapi-one-stop-native-owner-v1.inc');
 order(owner,['!selection_available ()','!snapshot_capture_available ()','!snapshot_publication_available ()','m_owner.reserve_selection (logical_storage (),limits::logical_bytes)']);
 assert(inherited('amd-dbgapi-one-stop-snapshot-v1.h').includes('snapshot_capture_available () noexcept { return false; }'));
 assert(inherited('amd-dbgapi-one-stop-publication-v2.h').includes('snapshot_publication_available () noexcept { return false; }'));
});
test('pinned actual GDB path explains ignored statuses and MI fast path',()=>{
 const stdio=inherited('ui-file.c'),utils=inherited('utils.c'),ui=inherited('ui.c'),mi=inherited('mi/mi-interp.c');
 assert(stdio.includes('fflush (m_file);'));assert(stdio.includes('fputs (linebuffer, m_file)'));
 assert(ui.includes('m_gdb_stdout (new pager_file (new stdio_file (outstream)))'));
 assert(mi.includes('raw_stdout = gdb_stdout'));assert(mi.includes('gdb_stdout = mi->out'));
 assert(mi.includes('mi->saved_raw_stdout = mi->raw_stdout'));assert(mi.includes('new tee_file (mi->raw_stdout, logfile_p)'));
 assert(utils.includes('top_level_interpreter ()->interp_ui_out ()->is_mi_like_p ()'));
 assert(inherited('posix-hdep.c').includes('gdb_console_fputs'));
});

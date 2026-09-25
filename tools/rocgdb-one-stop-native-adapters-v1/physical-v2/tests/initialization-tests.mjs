// Static regression only. No GDB/source module, native hook or target is executed.
import test from 'node:test';
import assert from 'node:assert/strict';
import crypto from 'node:crypto';
import {finalSource} from './source-files.mjs';
const files=finalSource();
const header=files['gdb/mi/mi-interp.h'];
const mi=files['gdb/mi/mi-interp.c'];
const interps=files['gdb/interps.c'];
function body(text, signature) {
  const at = text.indexOf(signature);
  assert(at >= 0, signature);
  const open = text.indexOf('{', at);
  assert(open >= 0);
  let depth = 1, end = open + 1;
  for (; end < text.length && depth !== 0; ++end) {
    if (text[end] === '{') ++depth;
    else if (text[end] === '}') --depth;
  }
  assert.equal(depth, 0);
  return text.slice(open + 1, end - 1);
}
function initialized(text) {
  assert.equal((text.match(/struct ui_file \*saved_raw_stdout = nullptr;/g) ?? []).length, 1);
  assert.equal((text.match(/\bsaved_raw_stdout\b/g) ?? []).length, 1);
  assert.match(text, /mi_interp \(const char \*name\)\s*:\s*interp \(name\)\s*\{\s*\}/);
  assert(text.includes('ui_file_up logfile_holder;'));
  assert(text.includes('ui_file_up stdout_holder;'));
}
test('actual MI member has defined null state before any logging operation', () => {
  initialized(header);
  const old=header.replace('struct ui_file *saved_raw_stdout = nullptr;', 'struct ui_file *saved_raw_stdout;');
  assert.equal(crypto.createHash('sha256').update(old).digest('hex'),'8737314a08a109cbacae1d1265fd8dd35cd1f01fdfdb1f43bcfb98d6d4a60b9e');
});
test('missing, indeterminate and post-initializer clobber mutants refuse', () => {
  initialized(header);
  for (const replacement of [
    'struct ui_file *saved_raw_stdout;',
    'struct ui_file *saved_raw_stdout = raw_stdout;',
    'struct ui_file *saved_raw_stdout = nullptr; struct ui_file *saved_raw_stdout;',
  ]) {
    assert.throws(() => initialized(header.replace('struct ui_file *saved_raw_stdout = nullptr;', replacement)));
  }
  assert.throws(() => initialized(header.replace(': interp (name)\n  {}', ': interp (name)\n  { saved_raw_stdout = raw_stdout; }')));
});
test('real factory and initialization sequence contain no prior logging reset', () => {
  assert.match(body(mi, 'mi_interp_factory (const char *name)'), /return new mi_interp \(name\);/);
  const startup = body(interps, 'interp_set (struct interp *interp, bool top_level)');
  assert(startup.includes('interp->init (top_level);'));
  assert(startup.includes('current_uiout = interp->interp_ui_out ();'));
  assert(startup.includes('interp->resume ();'));
  assert(!startup.includes('set_logging'));
  assert(!body(mi, 'mi_interp::init (bool top_level)').includes('saved_raw_stdout'));
  assert(!body(mi, 'mi_interp::resume ()').includes('saved_raw_stdout'));
  // Explicit member initialization no longer depends on allocator contents.
});
test('other newly inspected MI members have explicit init or default-owning class semantics', () => {
  initialized(header);
  const init = body(mi, 'mi_interp::init (bool top_level)');
  assert(init.includes('mi->raw_stdout = gdb_stdout;'));
  assert(init.includes('mi->mi_uiout = mi_out_new (name ()).release ();'));
  assert(init.includes('gdb_assert (mi->mi_uiout != nullptr);'));
  const uiFile = files['gdb/ui-file.h'];
  assert(uiFile.includes('typedef std::unique_ptr<ui_file> ui_file_up;'));
});
test('logging transitions and exact R2 refusal remain unchanged', () => {
  const logging = body(mi, 'mi_interp::set_logging (ui_file_up logfile, bool logging_redirect,');
  assert(logging.includes('mi->saved_raw_stdout = mi->raw_stdout;'));
  assert(logging.includes('mi->raw_stdout = mi->saved_raw_stdout;'));
  assert(logging.includes('mi->saved_raw_stdout = nullptr;'));
  const resume = files['gdb/amd-dbgapi-one-stop-native-resume-v1.inc'];
  const flush = body(resume, 'void native_adapter::flush ()');
  assert(flush.includes('mi->saved_raw_stdout==nullptr'));
  assert(flush.includes('!mi->logfile_holder && !mi->stdout_holder'));
  assert.equal((flush.match(/gdb_puts \(/g) ?? []).length, 1);
  assert.equal((flush.match(/gdb_flush \(/g) ?? []).length, 1);
  assert(flush.includes('publication_output::submit_once'));
});

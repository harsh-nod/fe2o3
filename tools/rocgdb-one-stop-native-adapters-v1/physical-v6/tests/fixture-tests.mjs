// SPDX-License-Identifier: GPL-3.0-or-later
import test from 'node:test';
import assert from 'node:assert/strict';
import {SourceSession} from './source-files.mjs';
const s=new SourceSession();
test('diagnostic portable core occurs byte-exact once in packaged production header',()=>{
 const core=s.packageText('tests/diagnostic-core.inc'),header=s.packageText('src/amd-dbgapi-one-stop-native-v1.h');
 assert.equal(header.split(core).length,2);
});
test('diagnostic retains first failure without changing native-result refusal',()=>{
 const io=s.packageText('src/amd-dbgapi-one-stop-native-io-v1.inc');
 const memoryFailure=[
  '  const int status=target_read_memory (address,out,bytes);',
  '  if (status!=0) {',
  '    m_native_result_note.remember (m_owner.selected (),m_owner.invalid (),',
  '      native_result_site::target_memory,native_result_family::none,0,',
  '      static_cast<std::uint32_t> (bytes),status);',
  '    reject (failure::native_result);',
  '  }'
 ].join('\n');
 // The real nonzero-result path still remembers before the same unconditional refusal.
 assert.equal(io.split(memoryFailure).length,2);
 const queryTail=[
  '    m_native_result_note.remember (m_owner.selected (),m_owner.invalid (),',
  '      native_result_site::scalar_query,family,static_cast<std::uint32_t> (query),',
  '      sizeof (value),static_cast<std::int32_t> (status));',
  '  }',
  '  require (status==AMD_DBGAPI_STATUS_SUCCESS,failure::native_result);'
 ].join('\n');
 const queryStart=io.indexOf('void native_adapter::query (');
 assert(queryStart>=0);
 const query=io.slice(queryStart);
 assert.equal(query.split('  if (status!=AMD_DBGAPI_STATUS_SUCCESS) {').length,2);
 assert.equal(query.split(queryTail).length,2);
 const rejectStart=io.indexOf('[[noreturn]] void native_adapter::reject (');
 const rejectEnd=io.indexOf('void native_adapter::require (',rejectStart);
 assert(rejectStart>=0&&rejectEnd>rejectStart);
 const reject=io.slice(rejectStart,rejectEnd);
 const poison=reject.indexOf('  m_owner.poison (why, site);');
 const diagnosticGate=reject.indexOf('  if (m_owner.why ()==failure::native_result\n      && m_native_result_note.site!=native_result_site::none)');
 assert(poison>=0&&diagnosticGate>poison);
 assert(!io.includes('refusal::native_result'));
});
test('debug type hook and two logical scratch reservations remain source-visible',()=>{
 const h=s.packageText('src/amd-dbgapi-one-stop-native-v1.h');
 assert.equal(h.split('native_result_diagnostic_debug_type (const native_result_diagnostic_scratch *p) noexcept').length,2);
 assert.equal(h.split('+ sizeof (native_result_diagnostic_scratch)').length,3);
 assert.equal(h.split('  native_result_note m_native_result_note {};').length,2);
});
test('portable diagnostic fixture includes its exact extracted core',()=>{
 assert(s.packageText('tests/diagnostic-core-test.cc').includes('#include "diagnostic-core.inc"'));
});

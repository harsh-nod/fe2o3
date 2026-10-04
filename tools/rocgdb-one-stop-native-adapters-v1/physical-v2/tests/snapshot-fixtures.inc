/* SPDX-License-Identifier: GPL-3.0-or-later
   Pure inert snapshot/state/resource controls. No GDB or dbgapi invocation.
   Test-only owners below are deliberately not native admission evidence. */
#define FE2O3_ONE_STOP_PURE_TEST
#define FE2O3_ONE_STOP_SNAPSHOT_PURE_TEST
#include "amd-dbgapi-one-stop-snapshot-v1.h"
#include <cassert>
#include <type_traits>
#include <initializer_list>
namespace amd_owned_one_stop_v1 {
struct core_test {
  static void seed (owner &o,const identity &id) {
    assert (o.reserve_selection (sizeof (owner)+sizeof (workspace)
              +sizeof (physical_snapshot)+4096+1024+limits::client_output_bytes,limits::logical_bytes));
    assert (o.bind_selection (id));
    assert (o.debit (counter::work,4096));
    assert (o.debit (counter::api,84)); // First full stop51 + queues30 + object3.
    assert (o.debit (counter::read_bytes,1024)); assert (o.debit (counter::read_calls,8));
    assert (o.debit (counter::file_requested_bytes,32));
    assert (o.debit (counter::file_observed_bytes,32));
    assert (o.debit (counter::file_rounds,64)); assert (o.debit (counter::proc_requested_bytes,1024));
    o.m_phase=phase::provisional_gpu_stop; // Inert setup, not a native transition proof.
  }
  static bool reserve (owner &o,std::uint64_t n) { return o.reserve_selection (n,limits::logical_bytes); }
  static void current (owner &o) { o.m_phase=phase::current_gpu_stop; }
  static bool charge (owner &o,counter c,std::uint64_t n) { return o.debit (c,n); }
  static void invalidate (owner &o) { o.poison (failure::stop_changed); }
  static void resume (owner &o) { o.m_phase=phase::completion_consumed; }
  static usage &use (owner &o) { return o.m_usage; }
};
struct snapshot_test {
  static bool begin (physical_snapshot &s,const owner &o,const identity &i,const gpu_stop &g,
                     std::uint64_t address=0x10000) { return s.begin (o,i,g,address); }
  static void raw (physical_snapshot &s) { s.m_register.fill (0xa7); s.m_output.fill (0x5b); }
  static bool reg (physical_snapshot &s,snapshot_read_result r=snapshot_read_result::success,
                   std::uint64_t id=42,std::uint64_t size=4) { return s.register_complete (id,size,r); }
  static bool mem (physical_snapshot &s,snapshot_read_result r=snapshot_read_result::success,
                   std::uint64_t n=272) { return s.memory_complete (r,n); }
  static bool confirm (physical_snapshot &s,const owner &o,const identity &i,const gpu_stop &g) {
    return s.confirm (o,i,g);
  }
  static bool seal (physical_snapshot &s,const owner &o,const identity &i,const gpu_stop &g,bool b=true) {
    return s.seal (o,i,g,b);
  }
  static bool readable (const physical_snapshot &s,const owner &o,const identity &i,const gpu_stop &g) {
    return s.readable (o,i,g);
  }
  static bool clear (const physical_snapshot &s) {
    for (auto b:s.m_register) if (b) return false;
    for (auto b:s.m_output) if (b) return false;
    return true;
  }
  static bool actual (const physical_snapshot &s) {
    for (auto b:s.m_register) if (b!=0xa7) return false;
    for (auto b:s.m_output) if (b!=0x5b) return false;
    return true;
  }
  static snapshot_issue issue (const physical_snapshot &s) { return s.m_issue; }
  static void revoke (physical_snapshot &s) { s.revoke (snapshot_issue::resumed); }
};
struct inert {
  identity id {1,2,3,4,5,6,7,8};
  gpu_stop stop {9,10,11,12,13,14,15,16,17};
  owner ledger;
  physical_snapshot snapshot;
  inert () { core_test::seed (ledger,id); }
  void stage () {
    assert (snapshot_test::begin (snapshot,ledger,id,stop));
    assert (core_test::charge (ledger,counter::api,4));
    assert (core_test::charge (ledger,counter::read_bytes,276));
    assert (core_test::charge (ledger,counter::read_calls,2));
    snapshot_test::raw (snapshot);
    assert (snapshot_test::reg (snapshot)); assert (snapshot_test::mem (snapshot));
  }
  void confirm () {
    core_test::current (ledger);
    assert (snapshot_test::confirm (snapshot,ledger,id,stop));
  }
  void seal () { confirm (); assert (snapshot_test::seal (snapshot,ledger,id,stop)); }
  bool readable () const { return snapshot_test::readable (snapshot,ledger,id,stop); }
};
void disabled () { assert (!snapshot_capture_available ()); }
void private_lifetime () {
  static_assert (!std::is_copy_constructible<physical_snapshot>::value);
  static_assert (!std::is_move_constructible<physical_snapshot>::value);
  assert (sizeof (physical_snapshot)<=1024);
}
void values_wait_for_retirement () {
  inert x; assert (!x.readable ()); x.stage (); assert (!x.readable ());
  x.confirm (); assert (!x.readable ());
  assert (snapshot_test::seal (x.snapshot,x.ledger,x.id,x.stop)); assert (x.readable ());
}
void actual_bytes_not_fixture_oracle () {
  inert x; x.stage (); x.seal (); assert (x.readable ());
  // Deliberately not fixture s12=0x13579bdf or XOR outputs: retain actual input bytes.
  assert (snapshot_test::actual (x.snapshot));
}
void absent_register () {
  inert x; assert (snapshot_test::begin (x.snapshot,x.ledger,x.id,x.stop));
  snapshot_test::raw (x.snapshot);
  assert (snapshot_test::reg (x.snapshot,snapshot_read_result::unavailable));
  assert (snapshot_test::clear (x.snapshot)); x.seal (); assert (!x.readable ());
  assert (snapshot_test::issue (x.snapshot)==snapshot_issue::register_unavailable);
}
void unavailable_memory () {
  inert x; assert (snapshot_test::begin (x.snapshot,x.ledger,x.id,x.stop));
  snapshot_test::raw (x.snapshot); assert (snapshot_test::reg (x.snapshot));
  assert (snapshot_test::mem (x.snapshot,snapshot_read_result::unavailable,0));
  assert (snapshot_test::clear (x.snapshot)); x.seal (); assert (!x.readable ());
  assert (snapshot_test::issue (x.snapshot)==snapshot_issue::memory_unavailable);
}
void short_success () {
  for (auto n:{0,1,16,271}) {
    inert x; assert (snapshot_test::begin (x.snapshot,x.ledger,x.id,x.stop));
    snapshot_test::raw (x.snapshot); assert (snapshot_test::reg (x.snapshot));
    assert (snapshot_test::mem (x.snapshot,snapshot_read_result::success,n));
    assert (snapshot_test::clear (x.snapshot)); x.seal (); assert (!x.readable ());
    assert (snapshot_test::issue (x.snapshot)==snapshot_issue::short_memory);
  }
}
void oversized_read () {
  inert x; assert (snapshot_test::begin (x.snapshot,x.ledger,x.id,x.stop));
  snapshot_test::raw (x.snapshot); assert (snapshot_test::reg (x.snapshot));
  assert (!snapshot_test::mem (x.snapshot,snapshot_read_result::success,273));
  assert (snapshot_test::clear (x.snapshot)); assert (!x.readable ());
}
void native_register_failure () {
  inert x; assert (snapshot_test::begin (x.snapshot,x.ledger,x.id,x.stop));
  snapshot_test::raw (x.snapshot);
  assert (!snapshot_test::reg (x.snapshot,snapshot_read_result::error));
  assert (snapshot_test::clear (x.snapshot)); assert (!x.readable ());
}
void native_memory_failure () {
  inert x; assert (snapshot_test::begin (x.snapshot,x.ledger,x.id,x.stop));
  snapshot_test::raw (x.snapshot); assert (snapshot_test::reg (x.snapshot));
  assert (!snapshot_test::mem (x.snapshot,snapshot_read_result::error));
  assert (snapshot_test::clear (x.snapshot)); assert (!x.readable ());
}
void false_access_error_bytes () {
  inert x; assert (snapshot_test::begin (x.snapshot,x.ledger,x.id,x.stop));
  snapshot_test::raw (x.snapshot); assert (snapshot_test::reg (x.snapshot));
  assert (!snapshot_test::mem (x.snapshot,snapshot_read_result::unavailable,1));
  assert (snapshot_test::clear (x.snapshot));
}
void register_metadata () {
  for (auto size:{0,1,3,5,256}) {
    inert x; assert (snapshot_test::begin (x.snapshot,x.ledger,x.id,x.stop));
    assert (!snapshot_test::reg (x.snapshot,snapshot_read_result::success,42,size));
  }
  inert x; assert (snapshot_test::begin (x.snapshot,x.ledger,x.id,x.stop));
  assert (!snapshot_test::reg (x.snapshot,snapshot_read_result::success,0,4));
}
void repeat_reads () {
  inert x; x.stage (); assert (!snapshot_test::begin (x.snapshot,x.ledger,x.id,x.stop));
  assert (snapshot_test::clear (x.snapshot));
  inert y; y.stage (); assert (!snapshot_test::mem (y.snapshot)); assert (!y.readable ());
}
void wrong_identity () {
  for (unsigned n=0;n<8;++n) {
    inert x; x.stage (); core_test::current (x.ledger); auto id=x.id;
    switch (n) {
      case 0: ++id.inferior; break; case 1: ++id.program_space; break;
      case 2: ++id.process_owner; break; case 3: ++id.inferior_number; break;
      case 4: ++id.pid; break; case 5: ++id.pid_start; break;
      case 6: ++id.process; break; case 7: ++id.host_thread; break;
    }
    assert (!snapshot_test::confirm (x.snapshot,x.ledger,id,x.stop)); assert (!x.readable ());
  }
}
void wrong_stop () {
  for (unsigned n=0;n<9;++n) {
    inert x; x.stage (); core_test::current (x.ledger); auto s=x.stop;
    switch (n) {
      case 0: ++s.thread; break; case 1: ++s.wave; break; case 2: ++s.workgroup; break;
      case 3: ++s.dispatch; break; case 4: ++s.queue; break; case 5: ++s.agent; break;
      case 6: ++s.architecture; break; case 7: ++s.pc; break; case 8: ++s.completion_base; break;
    }
    assert (!snapshot_test::confirm (x.snapshot,x.ledger,x.id,s)); assert (!x.readable ());
  }
}
void other_ledger () {
  inert x,y; x.stage (); core_test::current (y.ledger);
  assert (!snapshot_test::confirm (x.snapshot,y.ledger,x.id,x.stop)); assert (!x.readable ());
}
void no_refund () {
  for (unsigned n=0;n<10;++n) {
    inert x; x.stage (); core_test::current (x.ledger);
    auto &u=core_test::use (x.ledger);
    switch (n) {
      case 0: --u.logical_bytes; break; case 1: u.api=0; break;
      case 2: u.work=0; break; case 3: u.rows=0; break;
      case 4: u.read_bytes=0; break; case 5: u.read_calls=0; break;
      case 6: u.file_requested_bytes=0; break; case 7: u.file_observed_bytes=0; break;
      case 8: u.file_rounds=0; break; case 9: u.proc_requested_bytes=0; break;
    }
    assert (!snapshot_test::confirm (x.snapshot,x.ledger,x.id,x.stop));
    assert (!x.readable ());
  }
}
void retirement_required () {
  inert x; x.stage (); x.confirm ();
  assert (!snapshot_test::seal (x.snapshot,x.ledger,x.id,x.stop,false)); assert (!x.readable ());
}
void changed_after_confirm () {
  inert x; x.stage (); x.confirm (); auto s=x.stop; ++s.wave;
  assert (!snapshot_test::seal (x.snapshot,x.ledger,x.id,s)); assert (!x.readable ());
}
void repeated_seal () {
  inert x; x.stage (); x.seal ();
  assert (!snapshot_test::seal (x.snapshot,x.ledger,x.id,x.stop)); assert (!x.readable ());
}
void invalidate_owner () {
  inert x; x.stage (); x.seal (); core_test::invalidate (x.ledger); assert (!x.readable ());
}
void revoke_before_effect () {
  inert x; x.stage (); x.seal (); snapshot_test::revoke (x.snapshot);
  assert (!x.readable ()); assert (snapshot_test::clear (x.snapshot));
  snapshot_test::revoke (x.snapshot); assert (snapshot_test::clear (x.snapshot));
}
void resume_state_not_current () {
  inert x; x.stage (); x.seal (); core_test::resume (x.ledger); assert (!x.readable ());
}
void exact_cumulative_budget () {
  inert x; x.stage (); x.seal ();
  assert (core_test::charge (x.ledger,counter::api,103)); // Other full stops102 + terminal1.
  assert (x.ledger.consumed ().api==191 && x.readable ());
  assert (core_test::charge (x.ledger,counter::api,1)); // Exact192 cap, no reset.
  assert (!core_test::charge (x.ledger,counter::api,1)); assert (!x.readable ());
  inert y; core_test::use (y.ledger).api=189;
  assert (core_test::charge (y.ledger,counter::api,3));
  assert (!core_test::charge (y.ledger,counter::api,1)); assert (y.ledger.invalid ());
}
void cumulative_work_exact_and_one_over () {
  inert x; const auto floor=x.ledger.consumed ().work;
  assert (floor>0 && floor<limits::work);
  assert (core_test::charge (x.ledger,counter::work,limits::work-floor));
  assert (x.ledger.consumed ().work==limits::work);
  assert (!core_test::charge (x.ledger,counter::work,1));
  assert (x.ledger.consumed ().work==limits::work && x.ledger.invalid ());
}
void storage_and_read_caps () {
  owner o; assert (!core_test::reserve (o,limits::logical_bytes+1));
  inert x; assert (core_test::charge (x.ledger,counter::read_bytes,limits::read_bytes-x.ledger.consumed ().read_bytes));
  assert (!core_test::charge (x.ledger,counter::read_bytes,1));
  inert y; assert (core_test::charge (y.ledger,counter::read_calls,limits::read_calls-y.ledger.consumed ().read_calls));
  assert (!core_test::charge (y.ledger,counter::read_calls,1));
}
} // namespace amd_owned_one_stop_v1

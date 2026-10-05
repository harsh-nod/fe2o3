/* SPDX-License-Identifier: GPL-3.0-or-later
   Pure bookkeeping/owner controls only; no native qualification. */
#define FE2O3_ONE_STOP_PURE_TEST 1
#include "amd-dbgapi-owned-one-stop-v1.h"
#include <cassert>
using namespace amd_owned_one_stop_v1;
namespace amd_owned_one_stop_v1 {
struct core_test {
  static identity id () { return {1,2,3,4,5,6,7,8}; }
  static gpu_stop stop () { return {9,10,11,12,13,14,15,16,17}; }
  static void seed (owner &o) {
    o.m_selected=true; o.m_identity=id (); o.m_stop=stop ();
    o.m_phase=phase::current_gpu_stop;
  }
  static bool consume (owner &o) {
    return o.consume_completion (id (),stop (),1,1,0,0);
  }
  static void run () {
    // Each skipped, duplicated, reversed, or premature edge is sticky.
    constexpr unsigned count=static_cast<unsigned> (completion_edge::count);
    for (unsigned prefix=0;prefix<=count;++prefix)
      for (unsigned wrong=0;wrong<=count;++wrong) {
        if (prefix<count && wrong==prefix) continue;
        completion_sequence sequence;
        for (unsigned i=0;i<prefix;++i)
          assert (sequence.advance (static_cast<completion_edge> (i)));
        assert (!sequence.advance (static_cast<completion_edge> (wrong)));
        assert (!sequence.completed ());
        for (unsigned i=0;i<count;++i)
          assert (!sequence.advance (static_cast<completion_edge> (i)));
      }
    {
      owner o; seed (o); completion_sequence sequence;
      assert (consume (o));
      assert (sequence.advance (completion_edge::cpu_begin));
      assert (o.cpu_resume_returned ());
      assert (sequence.advance (completion_edge::cpu_return));
      assert (sequence.advance (completion_edge::cpu_body));
      assert (sequence.advance (completion_edge::cpu_generic));
      assert (o.state ()==phase::completion_consumed && !o.m_body_returned);
      assert (sequence.advance (completion_edge::wave_begin));
      assert (o.wave_resume_returned (stop ().wave,0));
      assert (sequence.advance (completion_edge::wave_return));
      assert (sequence.advance (completion_edge::wave_body));
      assert (o.resume_body_returned ());
      assert (sequence.advance (completion_edge::wave_generic));
      assert (sequence.advance (completion_edge::commit_begin));
      assert (o.commit_started (id ()));
      assert (o.state ()==phase::completion_consumed);
      assert (sequence.advance (completion_edge::commit_amd));
      assert (o.state ()==phase::completion_consumed);
      assert (sequence.advance (completion_edge::commit_generic));
      assert (o.commit_returned ());
      assert (o.state ()==phase::awaiting_target_completion);
      assert (sequence.completed ());
      assert (!o.consume_completion (id (),stop (),1,1,0,0));
    }
    // Actual owner identities and requery equality remain mandatory.
    for (unsigned field=0;field<8;++field) {
      owner o; seed (o); auto actual=id ();
      switch (field) {
        case 0: ++actual.inferior; break;
        case 1: ++actual.program_space; break;
        case 2: ++actual.process_owner; break;
        case 3: ++actual.inferior_number; break;
        case 4: ++actual.pid; break;
        case 5: ++actual.pid_start; break;
        case 6: ++actual.process; break;
        case 7: ++actual.host_thread; break;
      }
      assert (!o.consume_completion (actual,stop (),1,1,0,0));
      assert (o.invalid ());
    }
    for (unsigned field=0;field<9;++field) {
      owner o; seed (o); auto stale=stop ();
      switch (field) {
        case 0: ++stale.thread; break; case 1: ++stale.wave; break;
        case 2: ++stale.workgroup; break; case 3: ++stale.dispatch; break;
        case 4: ++stale.queue; break; case 5: ++stale.agent; break;
        case 6: ++stale.architecture; break; case 7: ++stale.pc; break;
        case 8: ++stale.completion_base; break;
      }
      assert (!o.consume_completion (id (),stale,1,1,0,0));
    }
    for (unsigned wrong=0;wrong<6;++wrong) {
      owner o; seed (o);
      assert (!o.consume_completion (id (),stop (),wrong==0 ? 0 : wrong==1 ? 2 : 1,
        wrong==2 ? 0 : wrong==3 ? 2 : 1,wrong==4 ? 1 : 0,wrong==5 ? 5 : 0));
    }
    for (unsigned wrong=0;wrong<7;++wrong) {
      owner o; seed (o); assert (consume (o));
      if (wrong==0) assert (!o.wave_resume_returned (stop ().wave,0));
      if (wrong==1) assert (!o.commit_started (id ()));
      if (wrong==2) { assert (o.cpu_resume_returned ()); assert (!o.cpu_resume_returned ()); }
      if (wrong==3) { assert (o.cpu_resume_returned ()); assert (!o.wave_resume_returned (99,0)); }
      if (wrong==4) { assert (o.cpu_resume_returned ()); assert (!o.wave_resume_returned (stop ().wave,1)); }
      if (wrong==5) { assert (o.cpu_resume_returned ()); assert (!o.resume_body_returned ()); }
      if (wrong==6) { assert (o.cpu_resume_returned ()); assert (o.wave_resume_returned (stop ().wave,0));
        assert (!o.wave_resume_returned (stop ().wave,0)); }
      assert (o.invalid ());
    }
    // Throw/unwind after zero, one, or two actual effects cannot later commit.
    for (unsigned returned=0;returned<3;++returned) {
      owner o; seed (o); assert (consume (o));
      if (returned>0) assert (o.cpu_resume_returned ());
      if (returned>1) assert (o.wave_resume_returned (stop ().wave,0));
      o.poison (failure::native_result);
      assert (!o.resume_body_returned () && !o.commit_started (id ()));
      assert (!o.commit_returned () && o.invalid ());
      assert (o.m_cpu_returned==(returned>0) && o.m_wave_returned==(returned>1));
    }
  }
};
}
int main () { core_test::run (); }

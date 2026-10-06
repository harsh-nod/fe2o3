/* SPDX-License-Identifier: GPL-3.0-or-later
   Synthetic ledger controls only: no native execution, event or observed counter.
   The owner and error fragment are exact production postimages. */
#define FE2O3_ONE_STOP_PURE_TEST 1
#include "../src/amd-dbgapi-owned-one-stop-v1.h"
#include <cassert>
#include <cstdio>
#include <cstdarg>
#include <cstring>
#include <stdexcept>
using namespace amd_owned_one_stop_v1;
namespace amd_owned_one_stop_v1 {
struct rendered { std::array<char,256> text {}; int size=0; };
[[noreturn]] static void error (const char *format,...) {
  rendered r; va_list args; va_start (args,format);
  r.size=std::vsnprintf (r.text.data (),r.text.size (),format,args); va_end (args);
  assert (r.size>=0 && static_cast<std::size_t> (r.size)<r.text.size ());
  throw r;
}
class native_adapter {
public:
  owner m_owner;
  bool emit_budget () {
#include "budget-error.inc"
    return false;
  }
};
struct core_test {
  static identity id () { return {1,2,3,4,5,6,7,8}; }
  static gpu_stop stop () { return {9,10,11,12,13,14,15,16,17}; }
  static bool same_note (const budget_failure_note &a,const budget_failure_note &b) {
    return a.kind==b.kind && a.state==b.state && a.present==b.present
      && a.used==b.used && a.requested==b.requested && a.cap==b.cap;
  }
  static std::uint64_t used (const owner &o,counter k) {
    const auto u=o.consumed ();
    switch(k) {
      case counter::api:return u.api; case counter::read_bytes:return u.read_bytes;
      case counter::read_calls:return u.read_calls; case counter::work:return u.work;
      case counter::diagnostic_rows:return u.rows;
      case counter::file_requested_bytes:return u.file_requested_bytes;
      case counter::file_observed_bytes:return u.file_observed_bytes;
      case counter::file_probe_bytes:return u.file_probe_bytes;
      case counter::file_rounds:return u.file_rounds;
      case counter::proc_requested_bytes:return u.proc_requested_bytes;
    }
    assert(false);return 0;
  }
  static void assert_message (native_adapter &a,const budget_failure_note &n) {
    const auto before=a.m_owner.consumed (); bool caught=false;
    try { assert (!a.emit_budget ()); } catch(const rendered &r) {
      caught=true; std::array<char,256> expected {};
      const int size=std::snprintf(expected.data(),expected.size(),
        "fixed owned one-stop native relation refused (5); commit-site=0; budget-counter=%u; budget-phase=%u; budget-used=%llu; budget-request=%llu; budget-cap=%llu",
        static_cast<unsigned>(n.kind),static_cast<unsigned>(n.state),
        static_cast<unsigned long long>(n.used),static_cast<unsigned long long>(n.requested),
        static_cast<unsigned long long>(n.cap));
      assert(size==r.size && std::strcmp(expected.data(),r.text.data())==0);
    }
    assert(caught && same_note(a.m_owner.m_budget_failure,n));
    const auto after=a.m_owner.consumed ();
    assert(std::memcmp(&before,&after,sizeof(usage))==0);
  }
  static void post_p9 (owner &o) {
    // Synthetic owner state after a successful same-stop consumption, not an
    // imported native owner or assertion of the R72 runtime counter.
    assert(o.select(id(),owner::fixed_storage()));
    o.m_phase=phase::current_gpu_stop; o.m_stop=stop(); o.m_usage.rows=12;
    assert(o.consume_completion(id(),stop(),1,1,0,0));
    assert(o.state()==phase::completion_consumed && o.consumed().rows==13);
    assert(o.cpu_resume_returned());
    assert(o.wave_resume_returned(stop().wave,0));
    assert(o.resume_body_returned());
  }
  static void run () {
    static_assert(static_cast<unsigned>(failure::budget)==5,"observed reason mapping");
    static_assert(limits::logical_bytes==65536 && limits::work==131072
      && limits::api==192 && limits::read_bytes==65536 && limits::read_calls==256
      && limits::proc_requested_bytes==16384 && limits::rows==16,"unchanged caps");
    static_assert(sizeof(budget_failure_note)<=32,"fixed note reservation");
    constexpr counter kinds[]={counter::api,counter::read_bytes,counter::read_calls,
      counter::work,counter::diagnostic_rows,counter::file_requested_bytes,
      counter::file_observed_bytes,counter::file_probe_bytes,counter::file_rounds,
      counter::proc_requested_bytes};
    constexpr std::uint64_t caps[]={limits::api,limits::read_bytes,limits::read_calls,
      limits::work,limits::rows,limits::file_requested_bytes,limits::file_observed_bytes,
      limits::file_probe_bytes,limits::file_rounds,limits::proc_requested_bytes};
    for(unsigned i=0;i<10;++i) {
      native_adapter a; auto &o=a.m_owner; assert(o.select(id(),owner::fixed_storage()));
      const auto k=kinds[i]; const auto p=o.state(); const auto start=used(o,k);
      assert(o.debit(k,caps[i]-start)); assert(used(o,k)==caps[i]);
      assert(!o.m_budget_failure.present && o.debit(k,0));
      assert(!o.debit(k,1)); const auto n=o.m_budget_failure;
      assert(n.present && n.kind==k && n.state==p && n.used==caps[i]
        && n.requested==1 && n.cap==caps[i] && o.why()==failure::budget);
      assert(used(o,k)==caps[i] && o.invalid());
      assert(!o.debit(counter::work,UINT64_MAX));
      o.poison(failure::native_result); assert(same_note(n,o.m_budget_failure));
      assert(o.why()==failure::budget); assert_message(a,n); assert_message(a,n);
    }
    {native_adapter a;auto&o=a.m_owner;assert(o.select(id(),owner::fixed_storage()));
      assert(o.debit(counter::work,1));assert(!o.debit(counter::work,UINT64_MAX));
      assert(o.m_budget_failure.used==1 && o.m_budget_failure.requested==UINT64_MAX);
      assert_message(a,o.m_budget_failure);}
    {owner o;assert(o.select(id(),owner::fixed_storage()));o.m_usage.work=limits::work+1;
      assert(!o.debit(counter::work,0));assert(o.m_budget_failure.used==limits::work+1);}
    {native_adapter a;auto&o=a.m_owner;assert(o.select(id(),owner::fixed_storage()));
      o.poison(failure::owner_changed);assert(!o.debit(counter::work,UINT64_MAX));
      assert(!o.m_budget_failure.present && !a.emit_budget() && o.why()==failure::owner_changed);}
    {native_adapter a;assert(!a.m_owner.debit(counter::work,1));
      assert(!a.m_owner.m_budget_failure.present && !a.emit_budget());}
    {native_adapter a;auto&o=a.m_owner;
      assert(o.reserve_selection(owner::fixed_storage(),owner::fixed_storage()));
      assert(!o.debit(counter::api,1));assert(o.why()==failure::order);
      assert(!o.m_budget_failure.present && !a.emit_budget());}
    {native_adapter a;auto&o=a.m_owner;
      assert(!o.reserve_selection(limits::logical_bytes+1,limits::logical_bytes+1));
      assert(o.why()==failure::budget && !o.m_budget_failure.present && !a.emit_budget());}
    {native_adapter a;auto&o=a.m_owner;assert(o.select(id(),owner::fixed_storage()));
      assert(!o.debit(static_cast<counter>(255),1));
      assert(o.why()==failure::budget && !o.m_budget_failure.present && !a.emit_budget());}
    // Same source-derived fixed one-inferior/two-thread work schedule as
    // completion_current. Guards are not simulated and no target effect occurs.
    constexpr std::uint64_t completion_work[]={160,8,32,32};
    constexpr std::uint64_t row_work[]={128,256};
    {native_adapter a;auto&o=a.m_owner;post_p9(o);
      assert(o.debit(counter::work,limits::work-231));
      for(unsigned i=0;i<3;++i)assert(o.debit(counter::work,completion_work[i]));
      assert(!o.debit(counter::work,completion_work[3]));
      assert(o.m_budget_failure.state==phase::completion_consumed
        && o.m_budget_failure.used==limits::work-31 && o.m_budget_failure.requested==32);
      assert(!o.commit_started(id()) && !o.commit_returned());assert_message(a,o.m_budget_failure);}
    {native_adapter a;auto&o=a.m_owner;post_p9(o);
      assert(o.debit(counter::work,limits::work-232));
      for(auto cost:completion_work)assert(o.debit(counter::work,cost));
      assert(o.commit_started(id()) && o.commit_returned());
      assert(o.state()==phase::awaiting_target_completion && o.consumed().rows==14);
      assert(!o.debit(counter::work,row_work[0]));
      assert(o.m_budget_failure.state==phase::awaiting_target_completion);assert_message(a,o.m_budget_failure);}
    {owner o;post_p9(o);assert(o.debit(counter::work,limits::work-232-384));
      for(auto cost:completion_work)assert(o.debit(counter::work,cost));
      assert(o.commit_started(id()) && o.commit_returned());
      for(auto cost:row_work)assert(o.debit(counter::work,cost));
      assert(!o.invalid() && !o.m_budget_failure.present && o.consumed().work==limits::work);}
  }
};
}
int main(){amd_owned_one_stop_v1::core_test::run();}

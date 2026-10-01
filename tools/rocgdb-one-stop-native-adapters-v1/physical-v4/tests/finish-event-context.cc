// SPDX-License-Identifier: GPL-3.0-or-later
// CPU-only mocks around exact old/new native method bodies. No GDB or GPU runs.
#include <cstdint>
#include <initializer_list>
#include <cstdio>
#include <cstdlib>
#include <optional>
#include <utility>
#define CHECK(x) do { if (!(x)) { std::fprintf(stderr,"check failed line %d: %s\n",__LINE__,#x); std::abort(); } } while (false)
struct inferior { int id=1; };
struct thread_info {
  inferior *inf=nullptr; int ptid=1; bool exec=false,resume=false;
  bool executing() const { return exec; }
  bool resumed() const { return resume; }
};
constexpr int null_ptid=0;
inferior *current_inf=nullptr;
thread_info *current_thread=nullptr;
int inferior_ptid=null_ptid;
unsigned thread_queries=0,switches=0,scope_enters=0,scope_exits=0,frame_saves=0;
struct null_thread_assertion {};
struct injected_identity_error {};
inferior *current_inferior() { return current_inf; }
thread_info *inferior_thread() {
  ++thread_queries; if(current_thread==nullptr) throw null_thread_assertion{};
  return current_thread;
}
void switch_to_thread(thread_info *thread) {
  CHECK(thread!=nullptr); ++switches; current_thread=thread;
  current_inf=thread->inf; inferior_ptid=thread->ptid;
}
// Models precisely the original null-thread constructor/restore path.
// A selected-thread construction would also save a frame, which tests forbid.
struct scoped_restore_current_thread {
  inferior *saved_inf=current_inf;
  thread_info *saved_thread=current_thread;
  int saved_ptid=inferior_ptid;
  scoped_restore_current_thread() {
    ++scope_enters;
    if(inferior_ptid!=null_ptid) { (void)inferior_thread(); ++frame_saves; }
  }
  ~scoped_restore_current_thread() {
    ++scope_exits; current_inf=saved_inf; current_thread=saved_thread;
    inferior_ptid=saved_ptid;
  }
};
enum wait_kind { TARGET_WAITKIND_STOPPED, TARGET_WAITKIND_THREAD_EXITED };
enum gdb_signal { GDB_SIGNAL_TRAP, GDB_SIGNAL_0 };
struct target_waitstatus {
  wait_kind k=TARGET_WAITKIND_STOPPED; gdb_signal s=GDB_SIGNAL_TRAP;
  wait_kind kind() const { return k; }
  gdb_signal sig() const { return s; }
};
namespace amd_owned_one_stop_v1 {
enum class failure { resume_scope,budget };
enum class counter { work };
struct refusal { failure why; };
struct owner_mock {
  bool valid=true; unsigned checks=0;
  bool check_owner(bool identity) { ++checks; return valid && identity; }
};
class native_adapter {
public:
  enum class path { none,entry };
  enum class callback_progress { none,step_ready,step_inflight,continue_ready };
  bool selected_value=true,poisoned=false,identity_ok=true,identity_throws=false;
  failure first=failure::resume_scope;
  unsigned used=0,limit=100;
  path m_path=path::none;
  callback_progress m_callback_progress=callback_progress::step_inflight;
  thread_info *m_host=nullptr; inferior *m_inferior=nullptr;
  std::uint64_t m_callback_pc=0x1234;
  owner_mock m_owner;
  bool selected() const { return selected_value; }
  [[noreturn]] void reject(failure why) {
    if(!poisoned) { first=why; poisoned=true; } throw refusal{first};
  }
  void require(bool yes,failure why) { if(!yes || poisoned) reject(why); }
  void debit(counter c,std::uint64_t amount) {
    CHECK(c==counter::work); CHECK(amount==1);
    if(used>=limit) { reject(failure::budget); }
    ++used;
  }
  bool current_identity() {
    if(identity_throws) throw injected_identity_error{};
    return identity_ok && current_inferior()==m_inferior;
  }
  void before_finish_step(thread_info*,const target_waitstatus&,bool,bool,std::uint64_t);
  void before_finish_step_legacy(thread_info*,const target_waitstatus&,bool,bool,std::uint64_t);
};
#include "actual-finish-step-bodies.inc"
}
using adapter=amd_owned_one_stop_v1::native_adapter;
using failure=amd_owned_one_stop_v1::failure;
using refusal=amd_owned_one_stop_v1::refusal;
struct fixture {
  inferior inf,foreign_inf;
  thread_info host,other;
  adapter a;
  target_waitstatus wait;
  thread_info *event=nullptr;
  bool inl=true,trap=true;
  std::uint64_t address=0x1234;
  fixture() {
    inf.id=1; foreign_inf.id=2;
    host.inf=&inf; host.ptid=7; other.inf=&inf; other.ptid=8;
    a.m_host=&host; a.m_inferior=&inf; event=&host;
    current_inf=&inf; current_thread=nullptr; inferior_ptid=null_ptid;
    thread_queries=switches=scope_enters=scope_exits=frame_saves=0;
  }
  void selected_host() { current_thread=&host; inferior_ptid=host.ptid; }
  void run() { a.before_finish_step(event,wait,inl,trap,address); }
  void expect_refusal(failure why) {
    auto *old_inf=current_inf; auto *old_thread=current_thread;
    const int old_ptid=inferior_ptid;
    bool seen=false;
    try { run(); } catch(const refusal &r) { seen=true; CHECK(r.why==why); }
    CHECK(seen && a.poisoned); CHECK(current_inf==old_inf);
    CHECK(current_thread==old_thread && inferior_ptid==old_ptid);
    CHECK(a.m_callback_progress==adapter::callback_progress::step_inflight);
  }
};
template<class F> void control(unsigned &count,const char *name,F body) {
  body(); ++count; std::printf("ok %u - %s\n",count,name);
}
int main() {
  unsigned count=0;
  control(count,"old exact body exposes no-current-thread assertion",[] {
    fixture f; bool seen=false;
    try { f.a.before_finish_step_legacy(f.event,f.wait,f.inl,f.trap,f.address); }
    catch(const null_thread_assertion&) { seen=true; }
    CHECK(seen && switches==0 && scope_enters==0);
  });
  control(count,"unselected path remains an exact no-op",[] {
    fixture f; f.a.selected_value=false; f.event=nullptr; current_inf=nullptr;
    f.run(); CHECK(f.a.used==0 && thread_queries==0 && switches==0 && scope_enters==0);
    CHECK(f.a.m_callback_progress==adapter::callback_progress::step_inflight);
  });
  control(count,"non-inflight phases do not borrow or debit",[] {
    for(auto p:{adapter::callback_progress::none,adapter::callback_progress::step_ready,
                adapter::callback_progress::continue_ready}) {
      fixture f; f.a.m_callback_progress=p; f.event=nullptr;
      f.run(); CHECK(f.a.used==0 && thread_queries==0 && switches==0 && scope_enters==0);
      CHECK(f.a.m_callback_progress==p);
    }
  });
  control(count,"null-thread event context is borrowed then restored",[] {
    fixture f; f.run();
    CHECK(f.a.m_callback_progress==adapter::callback_progress::continue_ready);
    CHECK(f.a.used==1 && f.a.m_owner.checks==1);
    CHECK(current_thread==nullptr && inferior_ptid==null_ptid && current_inf==&f.inf);
    CHECK(scope_enters==1 && scope_exits==1 && switches==1 && frame_saves==0);
  });
  control(count,"already selected same host requires no context guard",[] {
    fixture f; f.selected_host(); f.run();
    CHECK(current_thread==&f.host && current_inf==&f.inf && inferior_ptid==7);
    CHECK(scope_enters==0 && scope_exits==0 && switches==0 && frame_saves==0);
    CHECK(f.a.used==1 && f.a.m_callback_progress==adapter::callback_progress::continue_ready);
  });
  control(count,"already selected other thread is refused without switch",[] {
    fixture f; current_thread=&f.other; inferior_ptid=f.other.ptid;
    f.expect_refusal(failure::resume_scope);
    CHECK(switches==0 && scope_enters==0 && f.a.used==1);
  });
  control(count,"foreign inferior is not silently repaired",[] {
    fixture f; current_inf=&f.foreign_inf;
    f.expect_refusal(failure::resume_scope);
    CHECK(switches==0 && scope_enters==0 && thread_queries==0);
  });
  control(count,"missing retained inferior is refused before borrowing",[] {
    fixture f; f.a.m_inferior=nullptr;
    f.expect_refusal(failure::resume_scope);
    CHECK(switches==0 && scope_enters==0);
  });
  control(count,"null and non-owner events refuse before dereference",[] {
    { fixture f; f.event=nullptr; f.expect_refusal(failure::resume_scope); CHECK(switches==0); }
    { fixture f; f.event=&f.other; f.expect_refusal(failure::resume_scope); CHECK(switches==0); }
  });
  control(count,"event host from a foreign inferior is refused",[] {
    fixture f; f.host.inf=&f.foreign_inf;
    f.expect_refusal(failure::resume_scope); CHECK(switches==0 && scope_enters==0);
  });
  control(count,"all original stop and owner negatives restore null context",[] {
    // Each scalar mutation exercises the unchanged full predicate, not a
    // replacement pure acceptance function.
    for(unsigned n=0;n!=10;++n) {
      fixture f;
      switch(n) {
        case 0:f.a.m_path=adapter::path::entry;break;
        case 1:f.wait.k=TARGET_WAITKIND_THREAD_EXITED;break;
        case 2:f.wait.s=GDB_SIGNAL_0;break;
        case 3:f.inl=false;break;
        case 4:f.trap=false;break;
        case 5:++f.address;break;
        case 6:f.host.exec=true;break;
        case 7:f.host.resume=true;break;
        case 8:f.a.identity_ok=false;break;
        case 9:f.a.m_owner.valid=false;break;
      }
      f.expect_refusal(failure::resume_scope);
      CHECK(scope_enters==1 && scope_exits==1 && switches==1 && frame_saves==0);
    }
  });
  control(count,"exception during identity validation restores context",[] {
    fixture f; f.a.identity_throws=true; bool seen=false;
    try { f.run(); } catch(const injected_identity_error&) { seen=true; }
    CHECK(seen && current_inf==&f.inf && current_thread==nullptr && inferior_ptid==null_ptid);
    CHECK(scope_enters==1 && scope_exits==1 && frame_saves==0);
    CHECK(f.a.m_callback_progress==adapter::callback_progress::step_inflight);
  });
  control(count,"exhausted original work ledger refuses before context effect",[] {
    fixture f; f.a.limit=0; f.expect_refusal(failure::budget);
    CHECK(f.a.used==0 && switches==0 && scope_enters==0 && thread_queries==0);
  });
  control(count,"exact one-visit boundary and duplicate completion remain bounded",[] {
    fixture f; f.a.limit=1; f.run();
    CHECK(f.a.used==1 && f.a.m_callback_progress==adapter::callback_progress::continue_ready);
    f.run(); CHECK(f.a.used==1 && scope_enters==1 && switches==1);
  });
  control(count,"preexisting first refusal is not replaced",[] {
    fixture f; f.a.poisoned=true; f.a.first=failure::budget;
    f.expect_refusal(failure::budget);
    CHECK(scope_enters==0 && switches==0);
  });
  control(count,"same-host predicate refusal preserves existing selection",[] {
    fixture f; f.selected_host(); f.trap=false;
    f.expect_refusal(failure::resume_scope);
    CHECK(scope_enters==0 && switches==0 && current_thread==&f.host);
  });
  CHECK(count==16);
  std::puts("16 CPU mock controls passed; no native/GDB execution");
}

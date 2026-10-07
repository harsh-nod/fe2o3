// GPL-3.0-or-later. CPU-only object model; exact production functions are included.
#define FE2O3_ONE_STOP_PURE_TEST 1
#include "amd-dbgapi-owned-one-stop-v1.h"
#include <cassert>
#include <cstdarg>
#include <cstdio>
#include <cstring>
#include <string>
#include <vector>
struct program_space {};
struct inferior { int pid=123,num=1; program_space *pspace=nullptr; std::vector<int> thread_list; };
enum thread_state { THREAD_RUNNING, THREAD_EXITED };
struct thread_info { inferior *inf=nullptr; int global_num=7; thread_state state=THREAD_EXITED; };
struct amd_dbgapi_inferior_info { inferior *inf=nullptr; struct { unsigned long long handle=99; } process_id; };
struct ui {}; struct interp {};
static inferior *current_inf=nullptr;
static program_space *current_program_space=nullptr;
static ui *current_ui=nullptr;
static interp *current_interp=nullptr;
static inferior *current_inferior () { return current_inf; }
static interp *top_level_interpreter () { return current_interp; }
static struct registry { amd_dbgapi_inferior_info *p=nullptr; amd_dbgapi_inferior_info *get (inferior *) { return p; } } amd_dbgapi_inferior_data;
template<class T> struct ref { T *p=nullptr; T *get () const { return p; } };
enum bptype { bp_none=0, bp_breakpoint=1, bp_shlib_event=20, bp_thread_event=21 };
enum bpdisp { disp_del,disp_donttouch };
struct breakpoint;
struct bp_location { breakpoint *owner=nullptr; program_space *pspace=nullptr; bool inserted=false; };
struct breakpoint {
  bptype type=bp_shlib_event; int number=-1; program_space *pspace=nullptr;
  bpdisp disposition=disp_donttouch; bool commands=false;
  const char *cond_string=nullptr,*extra_string=nullptr;
  unsigned locations=1; bp_location loc; mutable unsigned location_accesses=0;
  bool has_single_location () const { return locations==1; }
  bp_location &first_loc () { ++location_accesses; assert(locations==1); return loc; }
};
namespace amd_owned_one_stop_v1 {
class native_adapter {
public:
  using terminal_stage=owner::terminal_stage;
  enum class refresh_kind { none,initial,terminal };
  enum class reset_state { idle,armed,consumed };
  enum class path { none,entry,completion };
  enum class callback_progress { none,step_ready };
  enum class breakpoint_site : std::uint8_t {
    none,refresh_begin,refresh_end,reset_begin,reset_bound,reset_candidate,
    reset_consume,reset_mutated,reset_end,unrelated_delete,owned_delete,
    callback_delete,owned_reset,callback_boundary
  };
  owner m_owner;
  inferior inf; program_space ps; thread_info host; amd_dbgapi_inferior_info info;
  ui own_ui; interp own_interp;
  inferior *m_inferior=&inf; ref<inferior> m_inferior_ref{&inf};
  thread_info *m_host=&host; ref<thread_info> m_host_ref{&host};
  amd_dbgapi_inferior_info *m_info=&info;
  ui *m_ui=&own_ui; interp *m_interpreter=&own_interp;
  breakpoint *m_breakpoint=nullptr; std::uintptr_t m_callback_object=0;
  std::uint64_t m_start_ticks=456;
  refresh_kind m_refresh_kind=refresh_kind::none;
  reset_state m_reset_state=reset_state::idle;
  path m_path=path::none;
  callback_progress m_callback_progress=callback_progress::none;
  bool m_command_continue=false,m_exit_requested=false,m_infrun_origin=false;
  identity m_disappeared_identity {};
  std::uint8_t m_terminal_delete_field=0;
  breakpoint_site m_first_breakpoint_site=breakpoint_site::none;
  unsigned m_first_breakpoint_type=0; int m_first_breakpoint_number=0;
  bool flush_fail=false; unsigned flushes=0,reset_consumes=0;
  native_adapter () {
    inf.pspace=&ps;host.inf=&inf;info.inf=&inf;
    current_inf=&inf;current_program_space=&ps;current_ui=&own_ui;current_interp=&own_interp;
    amd_dbgapi_inferior_data.p=&info;
    const identity id={reinterpret_cast<std::uintptr_t>(&inf),reinterpret_cast<std::uintptr_t>(&ps),
      reinterpret_cast<std::uintptr_t>(&info),1,123,456,99,7};
    assert(m_owner.select(id,limits::logical_bytes));
    while(m_owner.next_diagnostic()!=nullptr){}
    m_owner.m_phase=phase::awaiting_target_completion;
    m_owner.m_terminal=terminal_stage::unloaded;
  }
  bool selected () const { return m_owner.selected(); }
  void require (bool yes,failure why) { if(!yes||m_owner.invalid()){m_owner.poison(why);throw m_owner.why();} }
  void debit (counter kind,std::uint64_t amount) { if(!m_owner.debit(kind,amount))throw m_owner.why(); }
  void remember_breakpoint_failure (const breakpoint *bp,breakpoint_site site,bool valid) noexcept {
    if(!valid||m_first_breakpoint_site!=breakpoint_site::none)return;
    m_first_breakpoint_site=site;m_first_breakpoint_type=bp?static_cast<unsigned>(bp->type):0;
    m_first_breakpoint_number=bp?bp->number:0;
  }
  void breakpoint_check (bool yes,const breakpoint *bp,breakpoint_site site) {
    if(!yes&&!m_owner.invalid())remember_breakpoint_failure(bp,site,true);
    require(yes,failure::breakpoint_changed);
  }
  void consume_internal_breakpoint_reset (breakpoint *) { ++reset_consumes; }
  void retire_unloaded_maintenance () {}
  void retire_terminal_maintenance () {}
  void retire_callback_maintenance () {}
  void retire_loaded_maintenance () {}
  void retire_entry_maintenance () {}
  void flush () {
    ++flushes;
    if(flush_fail){m_owner.poison(failure::output);throw failure::output;}
    while(m_owner.next_diagnostic()!=nullptr){}
  }
  void terminal_delete_check (bool,const breakpoint *,std::uint8_t);
  void deleting_breakpoint (breakpoint *);
  void deleting_breakpoint_before (breakpoint *);
  void disappearing (inferior *);
  void published () { disappearing(&inf); inf.pid=0; }
  void live () { m_owner.m_phase=phase::awaiting_target_completion; }
  void stale_owner () { ++m_owner.m_identity.pid_start; }
  void pending_row () { --m_owner.m_sent; }
  void callback_phase (breakpoint *b) { live();m_owner.m_terminal=terminal_stage::withdrawal_complete;m_callback_object=reinterpret_cast<std::uintptr_t>(b); }
  void exhausted () { m_owner.m_usage.work=limits::work-127; }
  void selected_off () { m_owner.m_selected=false; }
  void poisoned () { m_owner.poison(failure::command); }
  bool invalid () const { return m_owner.invalid(); }
  failure why () const { return m_owner.why(); }
  std::uint64_t work () const { return m_owner.consumed().work; }
  std::uint64_t rows () const { return m_owner.consumed().rows; }
  void budget_sticky () const { assert(m_owner.m_budget_failure.present);assert(m_owner.m_budget_failure.requested==128); }
  [[noreturn]] static void error (const char *format,...) {
    char buffer[224];va_list ap;va_start(ap,format);const int n=std::vsnprintf(buffer,sizeof buffer,format,ap);va_end(ap);
    assert(n>=0&&static_cast<unsigned>(n)<sizeof buffer);throw std::string(buffer,static_cast<std::size_t>(n));
  }
  void format_note () {
#include "refusal-format.inc"
  }
};
#include "old-delete.inc"
#include "new-delete.inc"
}
using amd_owned_one_stop_v1::native_adapter;
using amd_owned_one_stop_v1::failure;
static breakpoint internal(native_adapter &n) { breakpoint b;b.pspace=&n.ps;b.loc.pspace=&n.ps;return b; }
static void ready(native_adapter &n,breakpoint &b) { b.loc.owner=&b;n.published(); }
static void refused(native_adapter &n,breakpoint &b,unsigned field) {
  try { n.deleting_breakpoint(&b);assert(false); } catch(failure f){assert(f==failure::breakpoint_changed);}
  assert(n.invalid());assert(n.m_terminal_delete_field==field);
  assert(n.m_first_breakpoint_site==native_adapter::breakpoint_site::unrelated_delete);
  assert(n.m_first_breakpoint_type==static_cast<unsigned>(b.type));assert(n.m_first_breakpoint_number==b.number);
  if(field){try{n.format_note();assert(false);}catch(const std::string &s){assert(s.find("terminal-delete-field="+std::to_string(field))!=std::string::npos);}}
}
int main () {
  // Exact old function reproduces N7 relation9/site9/type20/number-1 on the supported model.
  {native_adapter n;auto b=internal(n);ready(n,b);try{n.deleting_breakpoint_before(&b);assert(false);}catch(failure f){assert(f==failure::breakpoint_changed);}assert(n.m_first_breakpoint_site==native_adapter::breakpoint_site::unrelated_delete);}
  // New candidate allows the one source-produced uninserted internal event, never a normal-exit proof.
  {native_adapter n;auto b=internal(n);ready(n,b);const auto rows=n.rows();n.deleting_breakpoint(&b);assert(!n.invalid());assert(n.work()==128);assert(n.rows()==rows);assert(n.m_disappeared_identity.pid_start==0);assert(b.type==bp_shlib_event);refused(n,b,1);}
  {native_adapter n;auto b=internal(n);b.loc.owner=&b;refused(n,b,0);} // live phase
  {native_adapter n;auto b=internal(n);ready(n,b);b.type=bp_thread_event;refused(n,b,0);}
  {native_adapter n;auto b=internal(n);ready(n,b);n.m_disappeared_identity={};refused(n,b,1);}
  {native_adapter n;auto b=internal(n);ready(n,b);++n.m_disappeared_identity.pid_start;refused(n,b,1);}
  {native_adapter n;auto b=internal(n);ready(n,b);n.stale_owner();refused(n,b,1);}
  {native_adapter n;auto b=internal(n);ready(n,b);++n.m_start_ticks;refused(n,b,1);}
  {native_adapter n;auto b=internal(n);ready(n,b);inferior other;current_inf=&other;refused(n,b,2);}
  {native_adapter n;auto b=internal(n);ready(n,b);n.m_inferior_ref.p=nullptr;refused(n,b,2);}
  {native_adapter n;auto b=internal(n);ready(n,b);n.inf.pid=123;refused(n,b,3);}
  {native_adapter n;auto b=internal(n);ready(n,b);n.inf.thread_list.push_back(1);refused(n,b,3);}
  {native_adapter n;auto b=internal(n);ready(n,b);++n.inf.num;refused(n,b,3);}
  {native_adapter n;auto b=internal(n);ready(n,b);program_space other;current_program_space=&other;refused(n,b,3);}
  {native_adapter n;auto b=internal(n);ready(n,b);n.host.state=THREAD_RUNNING;refused(n,b,4);}
  {native_adapter n;auto b=internal(n);ready(n,b);++n.host.global_num;refused(n,b,4);}
  {native_adapter n;auto b=internal(n);ready(n,b);n.pending_row();refused(n,b,5);}
  {native_adapter n;auto b=internal(n);ready(n,b);current_ui=nullptr;refused(n,b,5);}
  {native_adapter n;auto b=internal(n);ready(n,b);n.m_exit_requested=true;refused(n,b,6);}
  {native_adapter n;auto b=internal(n);ready(n,b);n.m_refresh_kind=native_adapter::refresh_kind::terminal;refused(n,b,6);}
  {native_adapter n;auto b=internal(n);ready(n,b);b.number=1;refused(n,b,7);}
  {native_adapter n;auto b=internal(n);ready(n,b);b.pspace=nullptr;refused(n,b,8);}
  {native_adapter n;auto b=internal(n);ready(n,b);b.locations=2;refused(n,b,9);assert(b.location_accesses==0);}
  {native_adapter n;auto b=internal(n);ready(n,b);b.loc.owner=nullptr;refused(n,b,10);}
  {native_adapter n;auto b=internal(n);ready(n,b);b.loc.pspace=nullptr;refused(n,b,10);}
  {native_adapter n;auto b=internal(n);ready(n,b);b.loc.inserted=true;refused(n,b,11);}
  {native_adapter n;auto b=internal(n);ready(n,b);b.disposition=disp_del;refused(n,b,12);}
  {native_adapter n;auto b=internal(n);ready(n,b);b.commands=true;refused(n,b,12);}
  {native_adapter n;auto b=internal(n);ready(n,b);b.cond_string="secret-not-output";refused(n,b,12);}
  {native_adapter n;auto b=internal(n);ready(n,b);b.extra_string="secret-not-output";refused(n,b,12);}
  {native_adapter n;auto b=internal(n);ready(n,b);n.exhausted();try{n.deleting_breakpoint(&b);assert(false);}catch(failure f){assert(f==failure::budget);}n.budget_sticky();assert(n.m_terminal_delete_field==0);assert(n.m_disappeared_identity.pid_start==456);n.deleting_breakpoint(&b);n.budget_sticky();}
  {native_adapter n;n.flush_fail=true;try{n.disappearing(&n.inf);assert(false);}catch(failure f){assert(f==failure::output);}assert(n.m_disappeared_identity.pid_start==0);}
  {native_adapter n;inferior other;n.disappearing(&other);assert(n.invalid());assert(n.flushes==0);assert(n.m_disappeared_identity.pid_start==0);}
  {native_adapter n;auto b=internal(n);ready(n,b);n.m_reset_state=native_adapter::reset_state::armed;n.deleting_breakpoint(&b);assert(n.reset_consumes==1);assert(n.work()==0);assert(n.m_disappeared_identity.pid_start==456);}
  {native_adapter n;auto b=internal(n);n.callback_phase(&b);n.deleting_breakpoint(&b);assert(!n.invalid());assert(n.work()==0);}
  {native_adapter n;auto b=internal(n);ready(n,b);n.m_callback_object=reinterpret_cast<std::uintptr_t>(&b);try{n.deleting_breakpoint(&b);assert(false);}catch(failure f){assert(f==failure::breakpoint_changed);}assert(n.m_first_breakpoint_site==native_adapter::breakpoint_site::callback_delete);}
  {native_adapter n;auto b=internal(n);ready(n,b);n.m_breakpoint=&b;try{n.deleting_breakpoint(&b);assert(false);}catch(failure f){assert(f==failure::breakpoint_changed);}assert(n.m_first_breakpoint_site==native_adapter::breakpoint_site::owned_delete);}
  {native_adapter n;auto b=internal(n);n.selected_off();n.deleting_breakpoint(&b);assert(n.work()==0);}
  {native_adapter n;auto b=internal(n);n.poisoned();n.deleting_breakpoint(&b);assert(n.why()==failure::command);assert(n.work()==0);}
  std::puts("terminal-deletion CPU scenarios passed; no target/controller/native execution");
}

// SPDX-License-Identifier: GPL-3.0-or-later
// Exact extracted production methods with inert GDB doubles. No native owner,
// target, event, debugger process or hardware is constructed or exercised.
#include <cassert>
#include <cstdarg>
#include <cstdint>
#include <cstdio>
#include <map>
#include <stdexcept>
#include <string>
#include <tuple>
#include <vector>
#include "amd-dbgapi-owned-one-stop-v1.h"
using namespace amd_owned_one_stop_v1;
#define _(x) (x)
struct refused : std::runtime_error { using std::runtime_error::runtime_error; };
[[noreturn]] void error (const char *format,...) {
  char text[512]; va_list args; va_start(args,format);
  const int n=std::vsnprintf(text,sizeof(text),format,args);va_end(args);
  assert(n>=0 && static_cast<std::size_t>(n)<sizeof(text));throw refused(text);
}
struct ptid_t {
  long process=100,lwp=101,tid=0;
  long pid() const{return process;}
  bool operator<(const ptid_t &o)const{return std::tie(process,lwp,tid)<std::tie(o.process,o.lwp,o.tid);}
};
bool ptid_is_gpu(ptid_t p){return p.lwp==1;}
struct amd_dbgapi_event_id_t {std::uint64_t handle=0;};
struct amd_dbgapi_code_object_id_t {std::uint64_t handle=0;};
struct wave_handle {std::uint64_t handle=0;};
wave_handle get_amd_dbgapi_wave_id(ptid_t p){return {static_cast<std::uint64_t>(p.tid)};}
constexpr int THREAD_RUNNING=1,THREAD_EXITED=2,thread_stratum=2;
constexpr int AMD_DBGAPI_RUNTIME_STATE_LOADED_SUCCESS=1,AMD_DBGAPI_RESUME_MODE_NORMAL=0;
enum bptype {bp_none,bp_breakpoint,bp_overlay_event,bp_longjmp_master,
 bp_std_terminate_master,bp_exception_master,bp_shlib_event,bp_thread_event};
enum {bp_disabled,bp_enabled,disp_donttouch,disp_del};
struct program_space {};
struct breakpoint;
struct bp_location {breakpoint *owner=nullptr;program_space *pspace=nullptr;std::uint64_t address=0;};
struct breakpoint {
  int number=-7,enable_state=bp_disabled,disposition=disp_donttouch;
  bptype type=bp_longjmp_master;program_space *pspace=nullptr;
  breakpoint *related_breakpoint=this;bool commands=false;
  const char *cond_string=nullptr,*extra_string=nullptr;
  bp_location location;bool single=true;
  bool has_single_location()const{return single;}
  bp_location &first_loc(){return location;}
  const bp_location &first_loc()const{return location;}
};
namespace mock {class owned_checkpoint_breakpoint : public breakpoint {};}
struct thread_info;
struct target_ops {int level=0;int stratum()const{return level;}};
struct process_stratum_target:target_ops{};
struct inferior {
  process_stratum_target *proc=nullptr;target_ops *base=nullptr,*top=nullptr;
  program_space *pspace=nullptr;int pid=100;bool pushed=true;
  std::map<ptid_t,thread_info*> ptid_thread_map;
  std::vector<thread_info*> raw_threads;
  auto &threads(){return raw_threads;}
  auto *process_target(){return proc;}
  auto *top_target(){return top;}
  bool target_is_pushed(target_ops *t){return pushed&&(t==proc||t==base||t==top);}
  target_ops *find_target_beneath(target_ops *t){if(t==top)return base;if(t==base&&base!=proc)return proc;return nullptr;}
};
struct thread_info {
  inferior *inf=nullptr;int global_num=1,state=THREAD_RUNNING;
  bool running=false,resume=false,pending=false;ptid_t ptid;
  std::uint64_t pc=4096;
  bool executing()const{return running;}
  bool resumed()const{return resume;}
  bool has_pending_waitstatus()const{return pending;}
};
template<class T> struct held {T *p=nullptr;T *get()const{return p;}};
inferior *current_inf=nullptr;thread_info *current_thread=nullptr;
program_space *current_program_space=nullptr;process_stratum_target *native_target=nullptr;
target_ops the_amd_dbgapi_target;
std::vector<inferior*> raw_inferiors;
inferior *current_inferior(){return current_inf;}
thread_info *inferior_thread(){return current_thread;}
auto &all_inferiors(){return raw_inferiors;}
auto *get_native_target(){return native_target;}
thread_info *get_thread_regcache(thread_info *t){return t;}
std::uint64_t regcache_read_pc(thread_info *t){return t->pc;}
struct cached_wave {
  struct {wave_handle wave_id{77},queue_id{55},dispatch_id{66},agent_id{44};} coords;
  int last_resume_mode=AMD_DBGAPI_RESUME_MODE_NORMAL;bool stopping=false;
};
struct amd_dbgapi_inferior_info {
  int runtime_state=AMD_DBGAPI_RUNTIME_STATE_LOADED_SUCCESS;
  bool forward_progress_required=false;
  std::map<std::uint64_t,breakpoint*> breakpoint_map;
  std::map<std::uint64_t,cached_wave> wave_info_map;
};
struct owner_double {
  bool chosen=true,bad=false,identity=true,m_runtime=true,m_callback=true;
  phase state_value=phase::host_running;failure why_value=failure::none;
  commit_site site_value=commit_site::none;
  std::size_t m_callbacks=2,m_completed_callbacks=1;
  std::uintptr_t m_callback_object=0,m_origin_callback_object=0;
  std::uint64_t m_callback_id=1,m_origin_callback_id=1,m_object=0;
  bool m_allocation_active=false,m_empty_objects=false;
  enum class terminal_stage:std::uint8_t {idle,deleting,delete_complete,consistent,objects_acknowledged,withdrawal_complete,unloaded};
  terminal_stage m_terminal=terminal_stage::idle;
  resume_kind m_resume=resume_kind::completion;
  gpu_stop m_stop{2,77,88,66,55,44,33,22,11};
  std::uint64_t work=0,cap=131072;
  bool retirement_ok=false;unsigned retirement_calls=0;
  bool selected()const{return chosen;}
  bool invalid()const{return bad;}
  phase state()const{return state_value;}
  failure why()const{return why_value;}
  commit_site first_commit_site()const{return site_value;}
  void poison(failure f,commit_site s=commit_site::none){if(!chosen||bad)return;bad=true;why_value=f;site_value=s;}
  bool check_owner(int){if(!identity||bad){poison(failure::owner_changed);return false;}return true;}
  bool debit(counter k,std::uint64_t n){assert(k==counter::work);if(bad||n>cap-work){poison(failure::budget);return false;}work+=n;return true;}
  bool deleting_breakpoint(std::uintptr_t,std::uint64_t){++retirement_calls;if(!retirement_ok){poison(failure::breakpoint_changed);return false;}return true;}
};
enum class snapshot_issue {owner_changed};
struct snapshot_double {unsigned revocations=0;void revoke(snapshot_issue){++revocations;}};
namespace mock {
using owner=owner_double;
class native_adapter {
public:
  enum class path:std::uint8_t {none,entry,callback_step,callback_continue,publication,completion};
  enum class callback_progress:std::uint8_t {none,step_ready,step_inflight,continue_ready};
  enum class entry_epoch:std::uint8_t {unused,entry_commit,armed,retired};
  enum class loaded_epoch:std::uint8_t {unused,prospective,armed,in_flight,amd_returned,retired};
  enum class callback_epoch:std::uint8_t {inactive,armed,in_flight,amd_returned};
  using terminal_stage=owner_double::terminal_stage;
  enum class refresh_kind : std::uint8_t { none,initial,terminal };
  enum class reset_state : std::uint8_t { idle,armed,consumed };
  enum class breakpoint_site : std::uint8_t {
    none,refresh_begin,refresh_end,reset_begin,reset_bound,reset_candidate,
    reset_consume,reset_mutated,reset_end,unrelated_delete,owned_delete,
    callback_delete,owned_reset,callback_boundary
  };
  void breakpoint_refresh_current (refresh_kind,bool);
  void clear_breakpoint_refresh () noexcept;
  void consume_internal_breakpoint_reset (breakpoint *);
  void remember_breakpoint_failure (const breakpoint *,breakpoint_site,bool) noexcept;
  void breakpoint_check (bool,const breakpoint *,breakpoint_site);

  owner_double m_owner;snapshot_double m_snapshot;amd_dbgapi_inferior_info actual;
  inferior *m_inferior=nullptr;held<inferior> m_inferior_ref;
  thread_info *m_host=nullptr,*m_gpu_thread=nullptr;held<thread_info> m_host_ref,m_gpu_thread_ref;
  held<target_ops> m_loaded_process_ref,m_loaded_base_ref,m_loaded_top_ref;
  std::uintptr_t m_callback_object=0;
  std::uint64_t m_callback_number=static_cast<std::uint64_t>(-9),m_callback_pc=4096;
  std::uint64_t m_entry_pc=8192,m_checkpoint_pc=12288;
  owned_checkpoint_breakpoint *m_breakpoint=nullptr;
  amd_dbgapi_code_object_id_t m_object;
  bool m_origin_pending=false,m_origin_complete=false;
  bool m_query_output=false,m_in_allocate=false;
  path m_path=path::none;callback_progress m_callback_progress=callback_progress::none;
  bool m_generic_returned=false,m_generic_commit=false,m_amd_body_returned=false,m_amd_commit_returned=false;
  bool m_command_continue=false,m_exit_requested=false,m_infrun_origin=false,m_entry_maintenance_inflight=false;
  entry_epoch m_entry_epoch=entry_epoch::retired;loaded_epoch m_loaded_epoch=loaded_epoch::retired;
  callback_epoch m_callback_epoch=callback_epoch::inactive,m_terminal_epoch=callback_epoch::inactive;
  refresh_kind m_refresh_kind=refresh_kind::none;
  reset_state m_reset_state=reset_state::idle;
  std::uint64_t m_refresh_event=0,m_refresh_generation=0,m_reset_address=0;
  breakpoint *m_reset_object=nullptr;
  bp_location *m_reset_location=nullptr;
  std::size_t m_reset_count=0;
  int m_reset_number=0;
  unsigned m_reset_type=0;
  breakpoint_site m_first_breakpoint_site=breakpoint_site::none;
  unsigned m_first_breakpoint_type=0;
  int m_first_breakpoint_number=0;

  bool selected()const{return m_owner.selected();}
  int current_identity(){return 1;}
  amd_dbgapi_inferior_info &info(){return actual;}
  void thread_roster(std::size_t &host,std::size_t &gpu){host=0;gpu=0;for(auto *t:m_inferior->raw_threads){debit(counter::work,1);if(t==m_host)++host;else ++gpu;}}
  void retire_loaded_maintenance()noexcept{m_loaded_epoch=loaded_epoch::retired;}
  void retire_entry_maintenance()noexcept{m_entry_epoch=entry_epoch::retired;}
  void invalidate(failure why){clear_breakpoint_refresh();m_owner.poison(why);}
  [[noreturn]] void reject(failure,commit_site=commit_site::none);
  void require(bool,failure,commit_site=commit_site::none);
  void debit(counter,std::uint64_t);
  void before_breakpoint_refresh (amd_dbgapi_inferior_info &,amd_dbgapi_event_id_t);
  void after_breakpoint_refresh (amd_dbgapi_inferior_info &,amd_dbgapi_event_id_t);
  void abort_breakpoint_refresh () noexcept;
  bool before_internal_breakpoint_reset (breakpoint *,program_space *);
  void after_internal_breakpoint_reset ();

  void deleting_breakpoint(breakpoint *);
  void check_owned_breakpoint_reset(const owned_checkpoint_breakpoint *);
};
}
using adapter=mock::native_adapter;
adapter *active_adapter=nullptr;
namespace amd_owned_one_stop_native_v1 {
// This guard is constructed only by the stock internal_breakpoint::re_set
// deletion cases. It retains no breakpoint pointer beyond begin/consume.
bool before_internal_breakpoint_reset (breakpoint *,program_space *);
void after_internal_breakpoint_reset ();
void abort_breakpoint_refresh () noexcept;
class internal_breakpoint_reset_guard final {
  bool m_active;
public:
  internal_breakpoint_reset_guard (breakpoint *bp,program_space *pspace)
    : m_active (before_internal_breakpoint_reset (bp,pspace)) {}
  internal_breakpoint_reset_guard (const internal_breakpoint_reset_guard &)=delete;
  internal_breakpoint_reset_guard &operator= (const internal_breakpoint_reset_guard &)=delete;
  ~internal_breakpoint_reset_guard () noexcept {
    if (m_active) abort_breakpoint_refresh ();
  }
  void complete () {
    if (m_active) { after_internal_breakpoint_reset (); m_active=false; }
  }
};

bool before_internal_breakpoint_reset(breakpoint *bp,program_space *p){return active_adapter->before_internal_breakpoint_reset(bp,p);}
void after_internal_breakpoint_reset(){active_adapter->after_internal_breakpoint_reset();}
void abort_breakpoint_refresh()noexcept{active_adapter->abort_breakpoint_refresh();}
}
namespace mock {
#include "actual-breakpoint-methods.inc"
}
struct fixture {
  process_stratum_target proc;target_ops base;program_space space;inferior inf;
  thread_info host,gpu;breakpoint callback,master;mock::owned_checkpoint_breakpoint checkpoint;
  adapter a;amd_dbgapi_event_id_t event{3};
  fixture(){
    base.level=thread_stratum;inf.proc=&proc;inf.base=&base;inf.top=&the_amd_dbgapi_target;inf.pspace=&space;
    current_inf=&inf;current_thread=&host;current_program_space=&space;native_target=&proc;raw_inferiors={&inf};
    host.inf=&inf;gpu.inf=&inf;gpu.global_num=2;gpu.ptid={100,1,77};
    inf.raw_threads={&host};inf.ptid_thread_map={{host.ptid,&host}};
    for(auto *bp:{&callback,&master,static_cast<breakpoint*>(&checkpoint)}){
      bp->pspace=&space;bp->location={bp,&space,16384};
    }
    callback.number=-9;callback.type=bp_breakpoint;callback.enable_state=bp_enabled;callback.location.address=4096;
    checkpoint.number=-10;checkpoint.type=bp_breakpoint;checkpoint.disposition=disp_del;checkpoint.location.address=12288;
    a.m_inferior=&inf;a.m_inferior_ref.p=&inf;a.m_host=&host;a.m_host_ref.p=&host;
    a.m_loaded_process_ref.p=&proc;a.m_loaded_base_ref.p=&base;a.m_loaded_top_ref.p=&the_amd_dbgapi_target;
    a.m_callback_object=reinterpret_cast<std::uintptr_t>(&callback);
    a.m_owner.m_callback_object=a.m_owner.m_origin_callback_object=a.m_callback_object;
    a.actual.breakpoint_map={{1,&callback}};a.m_breakpoint=&checkpoint;active_adapter=&a;
  }
  void terminal(bool exited=false){
    a.m_owner.state_value=phase::awaiting_target_completion;
    a.m_owner.m_terminal=adapter::terminal_stage::consistent;
    a.m_owner.m_object=7;a.m_object.handle=7;a.m_origin_complete=true;
    a.m_gpu_thread=&gpu;a.m_gpu_thread_ref.p=&gpu;
    inf.raw_threads.push_back(&gpu);a.actual.wave_info_map={{77,{}}};
    if(exited){gpu.state=THREAD_EXITED;gpu.running=true;gpu.resume=true;}
    else inf.ptid_thread_map.emplace(gpu.ptid,&gpu);
  }
  void begin(){a.before_breakpoint_refresh(a.actual,event);}
  void observed(){if(a.m_refresh_kind==adapter::refresh_kind::initial){a.m_origin_pending=true;a.m_object.handle=7;}else a.m_owner.m_empty_objects=true;}
  void ready(){begin();observed();}
  void consume(){assert(a.before_internal_breakpoint_reset(&master,&space));a.deleting_breakpoint(&master);a.after_internal_breakpoint_reset();}
  void finish(){a.after_breakpoint_refresh(a.actual,event);}
};
template<class F> std::string reject(fixture &f,F fn,failure why=failure::breakpoint_changed){
  bool seen=false;std::string text;try{fn();}catch(const refused &e){seen=true;text=e.what();}
  assert(seen&&f.a.m_owner.invalid()&&f.a.m_owner.why()==why);return text;
}
template<class F> void bad_candidate(F mutate){
  fixture f;f.ready();mutate(f);reject(f,[&]{f.a.before_internal_breakpoint_reset(&f.master,&f.space);});
}
int main(){
 unsigned groups=0;
 {for(auto kind:{bp_overlay_event,bp_longjmp_master,bp_std_terminate_master,bp_exception_master}){
   fixture f;f.master.type=kind;f.ready();f.consume();f.finish();
   assert(!f.a.m_owner.invalid()&&f.a.m_owner.m_object==0&&f.a.m_origin_pending);
   assert(f.a.m_owner.m_completed_callbacks==1&&f.a.m_refresh_kind==adapter::refresh_kind::none);
 }++groups;}
 {fixture f;f.terminal();f.ready();f.consume();f.finish();
  assert(!f.a.m_owner.invalid()&&f.a.m_owner.m_terminal==adapter::terminal_stage::consistent&&f.a.m_owner.m_empty_objects);++groups;}
 {fixture f;f.terminal(true);f.ready();f.consume();f.finish();assert(!f.a.m_owner.invalid());++groups;}
 {fixture f;f.a.m_owner.chosen=false;f.a.before_breakpoint_refresh(f.a.actual,f.event);
  assert(!f.a.before_internal_breakpoint_reset(&f.master,&f.space));f.a.deleting_breakpoint(&f.master);
  f.a.after_breakpoint_refresh(f.a.actual,f.event);assert(f.a.m_owner.work==0);++groups;}
 {fixture f;reject(f,[&]{f.a.before_internal_breakpoint_reset(&f.master,&f.space);});++groups;}
 {fixture f;f.ready();reject(f,[&]{f.begin();});++groups;}
 {fixture f;f.ready();f.a.before_internal_breakpoint_reset(&f.master,&f.space);
  reject(f,[&]{f.a.before_internal_breakpoint_reset(&f.master,&f.space);});++groups;}
 {fixture f;f.begin();reject(f,[&]{f.a.before_internal_breakpoint_reset(&f.master,&f.space);},failure::order);++groups;}
 {fixture f;f.ready();++f.a.m_owner.m_callbacks;reject(f,[&]{f.a.before_internal_breakpoint_reset(&f.master,&f.space);});++groups;}
 {bad_candidate([](fixture &f){f.master.number=7;});bad_candidate([](fixture &f){f.master.enable_state=bp_enabled;});
  bad_candidate([](fixture &f){f.master.type=bp_breakpoint;});bad_candidate([](fixture &f){f.master.type=bp_shlib_event;});++groups;}
 {bad_candidate([](fixture &f){f.master.pspace=nullptr;});bad_candidate([](fixture &f){f.master.single=false;});
  bad_candidate([](fixture &f){f.master.location.owner=&f.callback;});bad_candidate([](fixture &f){f.master.location.pspace=nullptr;});++groups;}
 {for(auto address:{4096u,8192u,12288u,0u})bad_candidate([&](fixture &f){f.master.location.address=address;});++groups;}
 {bad_candidate([](fixture &f){f.master.related_breakpoint=&f.callback;});bad_candidate([](fixture &f){f.master.commands=true;});
  bad_candidate([](fixture &f){f.master.cond_string="x";});bad_candidate([](fixture &f){f.master.extra_string="x";});++groups;}
 {fixture f;f.ready();f.checkpoint.type=bp_longjmp_master;
  reject(f,[&]{f.a.before_internal_breakpoint_reset(&f.checkpoint,&f.space);});++groups;}
 {fixture f;f.ready();f.callback.type=bp_longjmp_master;f.callback.enable_state=bp_disabled;
  reject(f,[&]{f.a.before_internal_breakpoint_reset(&f.callback,&f.space);});++groups;}
 {fixture f;f.ready();f.a.before_internal_breakpoint_reset(&f.master,&f.space);
  breakpoint other;other.pspace=&f.space;reject(f,[&]{f.a.deleting_breakpoint(&other);});++groups;}
 {fixture f;f.ready();f.a.before_internal_breakpoint_reset(&f.master,&f.space);
  --f.master.number;reject(f,[&]{f.a.deleting_breakpoint(&f.master);});++groups;}
 {fixture f;f.ready();f.a.before_internal_breakpoint_reset(&f.master,&f.space);
  f.master.location.address+=4;reject(f,[&]{f.a.deleting_breakpoint(&f.master);});++groups;}
 {fixture f;f.ready();f.a.before_internal_breakpoint_reset(&f.master,&f.space);
  f.a.deleting_breakpoint(&f.master);reject(f,[&]{f.a.deleting_breakpoint(&f.master);});++groups;}
 {fixture f;f.ready();f.a.before_internal_breakpoint_reset(&f.master,&f.space);
  reject(f,[&]{f.a.after_internal_breakpoint_reset();});++groups;}
 {fixture f;f.ready();f.a.before_internal_breakpoint_reset(&f.master,&f.space);
  reject(f,[&]{f.finish();});++groups;}
 {fixture f;f.ready();f.a.before_internal_breakpoint_reset(&f.master,&f.space);
  f.a.abort_breakpoint_refresh();assert(f.a.m_owner.invalid()&&f.a.m_reset_object==nullptr);
  assert(f.a.m_reset_state==adapter::reset_state::idle);++groups;}
 {fixture f;f.ready();{
   amd_owned_one_stop_native_v1::internal_breakpoint_reset_guard guard(&f.master,&f.space);
   f.a.deleting_breakpoint(&f.master);guard.complete();
  }f.finish();assert(!f.a.m_owner.invalid());++groups;}
 {fixture f;f.ready();try{
   amd_owned_one_stop_native_v1::internal_breakpoint_reset_guard guard(&f.master,&f.space);
   throw std::runtime_error("inert throw before delete");
  }catch(const std::runtime_error&){}
  assert(f.a.m_owner.invalid()&&f.a.m_reset_object==nullptr);++groups;}
 {fixture f;f.ready();try{
   amd_owned_one_stop_native_v1::internal_breakpoint_reset_guard guard(&f.master,&f.space);
   f.a.deleting_breakpoint(&f.master);throw std::runtime_error("inert throw after consume");
  }catch(const std::runtime_error&){}
  assert(f.a.m_owner.invalid()&&f.a.m_reset_object==nullptr);++groups;}
 {fixture f;f.ready();f.a.m_reset_count=63;f.consume();assert(f.a.m_reset_count==64);
  reject(f,[&]{f.a.before_internal_breakpoint_reset(&f.master,&f.space);});++groups;}
 {fixture f;f.ready();f.a.m_owner.cap=f.a.m_owner.work;
  reject(f,[&]{f.a.before_internal_breakpoint_reset(&f.master,&f.space);},failure::budget);
  assert(f.a.m_reset_object==nullptr);++groups;}
 {fixture f;f.host.running=true;reject(f,[&]{f.begin();},failure::callback_identity);++groups;}
 {fixture f;f.host.resume=true;reject(f,[&]{f.begin();},failure::callback_identity);++groups;}
 {fixture f;f.host.pc+=1;reject(f,[&]{f.begin();},failure::callback_identity);++groups;}
 {fixture f;f.inf.pushed=false;reject(f,[&]{f.begin();},failure::owner_changed);++groups;}
 {fixture f;f.a.m_owner.identity=false;reject(f,[&]{f.begin();},failure::owner_changed);++groups;}
 {fixture f;f.a.actual.forward_progress_required=true;reject(f,[&]{f.begin();},failure::callback_identity);++groups;}
 {fixture f;f.terminal();f.a.m_owner.m_terminal=adapter::terminal_stage::deleting;
  reject(f,[&]{f.begin();},failure::order);++groups;}
 {fixture f;f.terminal();f.a.actual.wave_info_map.begin()->second.coords.queue_id.handle+=1;
  reject(f,[&]{f.begin();},failure::stop_changed);++groups;}
 {fixture f;f.terminal(true);f.inf.ptid_thread_map.emplace(f.gpu.ptid,&f.gpu);
  reject(f,[&]{f.begin();},failure::stop_changed);++groups;}
 {fixture f;f.terminal();f.gpu.running=true;reject(f,[&]{f.begin();},failure::stop_changed);++groups;}
 {fixture f;f.ready();const auto first=reject(f,[&]{f.a.deleting_breakpoint(&f.master);});
  const auto site=f.a.m_first_breakpoint_site;const auto number=f.a.m_first_breakpoint_number;
  const auto again=reject(f,[&]{f.a.check_owned_breakpoint_reset(&f.checkpoint);});
  assert(first==again&&number==-7&&site==adapter::breakpoint_site::unrelated_delete);
  assert(first.size()<224);++groups;}
 {fixture f;f.checkpoint.location.address+=1;
  reject(f,[&]{f.a.check_owned_breakpoint_reset(&f.checkpoint);});
  assert(f.a.m_first_breakpoint_site==adapter::breakpoint_site::owned_reset);++groups;}
 {fixture f;reject(f,[&]{f.a.deleting_breakpoint(&f.checkpoint);});
  assert(f.a.m_first_breakpoint_site==adapter::breakpoint_site::owned_delete&&f.a.m_owner.retirement_calls==1);++groups;}
 {fixture f;reject(f,[&]{f.a.deleting_breakpoint(&f.callback);});
  assert(f.a.m_first_breakpoint_site==adapter::breakpoint_site::callback_delete);++groups;}
 {fixture f;f.a.m_owner.retirement_ok=true;f.a.deleting_breakpoint(&f.checkpoint);
  assert(f.a.m_breakpoint==nullptr&&f.a.m_owner.retirement_calls==1&&!f.a.m_owner.invalid());++groups;}
 {fixture f;f.a.m_owner.state_value=phase::awaiting_target_completion;f.a.m_owner.m_callback=false;
  f.a.m_owner.m_terminal=adapter::terminal_stage::withdrawal_complete;
  f.a.deleting_breakpoint(&f.callback);assert(!f.a.m_owner.invalid());++groups;}
 {fixture f;f.ready();f.a.before_internal_breakpoint_reset(&f.master,&f.space);
  f.a.deleting_breakpoint(&f.master);f.master.location.owner=nullptr;f.master.pspace=nullptr;
  f.a.after_internal_breakpoint_reset();f.finish();assert(!f.a.m_owner.invalid());++groups;}
 {fixture f;breakpoint other;f.a.actual.breakpoint_map.emplace(2,&other);
  f.ready();f.consume();f.finish();assert(!f.a.m_owner.invalid());++groups;}
 {fixture f;for(unsigned n=2;n<=65;++n)f.a.actual.breakpoint_map.emplace(n,&f.master);
  reject(f,[&]{f.begin();},failure::callback_identity);++groups;}
 assert(groups==46);std::printf("breakpoint-refresh mock groups %u passed\n",groups);
}

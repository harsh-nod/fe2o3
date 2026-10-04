// SPDX-License-Identifier: GPL-3.0-or-later
// Source-extracted production methods with inert GDB owner/target doubles.
// This test constructs no authenticated native owner and performs no I/O/API call.
#include <cassert>
#include <cstddef>
#include <cstdint>
#include <initializer_list>
#include <cstdio>
#include <stdexcept>
#include <map>
#include <vector>
#include <tuple>
#include "../src/amd-dbgapi-owned-one-stop-v1.h"
using amd_owned_one_stop_v1::counter;
using amd_owned_one_stop_v1::failure;
using amd_owned_one_stop_v1::commit_site;
using amd_owned_one_stop_v1::phase;
struct refused { failure why; commit_site site; };
struct ptid_t {
  long process,lwp_value,tid_value;
  explicit ptid_t (long p=0,long l=0,long t=0):process(p),lwp_value(l),tid_value(t) {}
  long pid () const {return process;}
  bool operator== (ptid_t other) const {return process==other.process&&lwp_value==other.lwp_value&&tid_value==other.tid_value;}
  bool operator< (ptid_t other) const {return std::tie(process,lwp_value,tid_value)<std::tie(other.process,other.lwp_value,other.tid_value);}
};
const ptid_t minus_one_ptid (-1);
constexpr int thread_stratum=2,THREAD_RUNNING=1,THREAD_EXITED=2,GDB_SIGNAL_0=0;
constexpr int AMD_DBGAPI_RUNTIME_STATE_LOADED_SUCCESS=1,AMD_DBGAPI_RUNTIME_STATE_UNLOADED=0,AMD_DBGAPI_RESUME_MODE_NORMAL=0;
bool ptid_is_gpu(ptid_t p){return p.process!=1&&p.lwp_value==1;}
struct wave_handle {std::uint64_t handle;};
wave_handle get_amd_dbgapi_wave_id(ptid_t p){assert(ptid_is_gpu(p));return {static_cast<std::uint64_t>(p.tid_value)};}
struct thread_info;
struct target_ops { int level=0; int stratum () const { return level; } };
struct process_stratum_target: target_ops {
  bool threads_executing=true,commit_resumed_state=true,pending=false;
  bool has_resumed_with_pending_wait_status () const { return pending; }
};
struct inferior {
  process_stratum_target *proc=nullptr;
  target_ops *base=nullptr,*top=nullptr;
  int pid=100;
  bool pushed=true;
  std::map<ptid_t,thread_info *> ptid_thread_map;
  std::vector<thread_info *> raw_threads;
  const auto &threads () {return raw_threads;}
  process_stratum_target *process_target () { return proc; }
  target_ops *top_target () { return top; }
  bool target_is_pushed (target_ops *p) { return pushed && (p==proc||p==base||p==top); }
  target_ops *find_target_beneath (target_ops *p) {
    if(p==top&&top!=base)return base;if(p==base&&base!=proc)return proc;return nullptr;
  }
};
struct thread_info {
  inferior *inf=nullptr;int global_num=1;
  int state=THREAD_RUNNING;
  bool running=true,resume=true,pending=false;
  struct { bool trap_expected=true; } control;
  ptid_t ptid {100,101};
  bool executing () const { return running; }
  bool resumed () const { return resume; }
  bool has_pending_waitstatus () const { return pending; }
};
template<class T> struct held { T *p=nullptr; T *get () const {return p;} };
std::vector<inferior *> raw_inferiors;
const auto &all_inferiors () {return raw_inferiors;}
inferior *current_inf=nullptr;
process_stratum_target *native_target=nullptr;
target_ops the_amd_dbgapi_target;
inferior *current_inferior () {return current_inf;}
process_stratum_target *get_native_target () {return native_target;}
struct owner_double {
  bool selected_value=true,invalid_value=false,identity_matches=true;
  phase phase_value=phase::host_running;
  bool m_runtime=true,m_callback=false;
  enum class terminal_stage:std::uint8_t {idle,deleting,delete_complete,consistent,objects_acknowledged,withdrawal_complete,unloaded};
  terminal_stage m_terminal=terminal_stage::idle;
  amd_owned_one_stop_v1::resume_kind m_resume=amd_owned_one_stop_v1::resume_kind::completion;
  bool m_cpu_returned=false,m_wave_returned=false,m_body_returned=false,m_commit_started=false,m_empty_objects=false,m_allocation_active=false;
  amd_owned_one_stop_v1::gpu_stop m_stop {2,77,88,66,55,44,33,22,11};
  std::uint64_t m_callbacks=1,m_completed_callbacks=1;
  std::uintptr_t m_origin_callback_object=99;
  std::uint64_t m_origin_callback_id=11;
  std::uint64_t work=0,work_limit=131072;
  unsigned resume_credit=0,commit_credit=0;
  bool selected () const {return selected_value;}
  bool invalid () const {return invalid_value;}
  bool check_owner (int) const {return !invalid_value&&identity_matches;}
  phase state () const {return phase_value;}
  bool debit (counter kind,std::uint64_t amount) {
    assert(kind==counter::work);
    if(invalid_value||amount>work_limit-work){invalid_value=true;return false;}
    work+=amount;return true;
  }
  void poison (failure,commit_site=commit_site::none) {invalid_value=true;}
  bool commit_started (int) {++commit_credit;return true;}
  bool commit_returned () {++commit_credit;phase_value=phase::awaiting_target_completion;
    m_cpu_returned=m_wave_returned=m_body_returned=m_commit_started=false;return true;}
};
struct cached_wave {struct {wave_handle wave_id{77},dispatch_id{66},queue_id{55},agent_id{44};} coords;
  int last_resume_mode=AMD_DBGAPI_RESUME_MODE_NORMAL;bool stopping=false;};
struct info_double {std::map<std::uint64_t,cached_wave> wave_info_map {{77,{}}};int runtime_state=AMD_DBGAPI_RUNTIME_STATE_LOADED_SUCCESS;
  bool forward_progress_required=true;};
namespace amd_owned_one_stop_v1 {
class native_adapter final {
public:
  enum class path:std::uint8_t {none,entry,callback_step,callback_continue,publication,completion};
  enum class callback_progress:std::uint8_t {none,step_ready,step_inflight,continue_ready};
  enum class entry_epoch:std::uint8_t {unused,entry_commit,armed,retired};
  enum class loaded_epoch:std::uint8_t {unused,prospective,armed,in_flight,amd_returned,retired};
  enum class callback_epoch:std::uint8_t {inactive,armed,in_flight,amd_returned};
  using terminal_stage=owner_double::terminal_stage;
  enum class terminal_origin:std::uint8_t {none,completion,callback_step,callback_continue};
  owner_double m_owner;
  inferior *m_inferior=nullptr;held<inferior> m_inferior_ref;
  thread_info *m_host=nullptr;held<thread_info> m_host_ref;
  thread_info *m_gpu_thread=nullptr;held<thread_info> m_gpu_thread_ref;
  held<target_ops> m_loaded_process_ref,m_loaded_base_ref,m_loaded_top_ref;
  entry_epoch m_entry_epoch=entry_epoch::retired;
  loaded_epoch m_loaded_epoch=loaded_epoch::retired;
  callback_epoch m_callback_epoch=callback_epoch::inactive;
  bool m_entry_maintenance_inflight=false;
  path m_path=path::none;
  callback_progress m_callback_progress=callback_progress::step_inflight;
  std::uint64_t m_callback_generation=0,m_callback_pc=4096,m_callback_number=7;
  std::uintptr_t m_callback_object=99;
  bool m_callback_stepping=false;
  callback_epoch m_terminal_epoch=callback_epoch::inactive;
  terminal_origin m_terminal_origin=terminal_origin::none;
  terminal_stage m_terminal_stage=terminal_stage::idle;
  std::uint64_t m_terminal_generation=0;
  enum class unloaded_epoch:std::uint8_t {unused,armed,in_flight,retired};
  unloaded_epoch m_unloaded_epoch=unloaded_epoch::unused;
  bool m_generic_returned=false,m_generic_commit=false;
  bool m_amd_body_returned=false,m_amd_commit_returned=false;
  bool m_command_continue=false,m_exit_requested=false,m_infrun_origin=false;
  bool m_infrun_step=true,m_infrun_trap=true,m_infrun_inline=true;
  bool m_infrun_software=false,m_infrun_user_step=false;
  ptid_t m_infrun_scope {100,101};int m_infrun_signal=GDB_SIGNAL_0;
  info_double info_value;
  std::size_t host_count=1,gpu_count=0;unsigned roster_calls=0,flush_calls=0;
  bool selected () const {return m_owner.selected ();}
  void require (bool yes,failure why,commit_site site=commit_site::none) {
    if(!yes){m_owner.poison(why,site);throw refused{why,site};}
  }
  void debit (counter kind,std::uint64_t amount) {require(m_owner.debit(kind,amount),failure::budget);}
  int current_identity () {return 42;}
  info_double &info () {return info_value;}
  void thread_roster (std::size_t &host,std::size_t &gpu) {
    debit(counter::work,2);++roster_calls;host=host_count;gpu=gpu_count;
  }
  void flush () {++flush_calls;}
  void loaded_maintenance_current (bool,bool) {throw std::logic_error("unexpected loaded route");}
  void entry_maintenance_current () {throw std::logic_error("unexpected entry route");}
  void retain_entry_commit_targets () {throw std::logic_error("unexpected entry retain");}
  void callback_maintenance_current (bool);
  void arm_callback_maintenance (bool);
  void retire_callback_maintenance () noexcept;
  void terminal_gpu_current ();void terminal_maintenance_current (bool);
  void arm_terminal_maintenance (terminal_origin);void retire_terminal_maintenance () noexcept;
  void unloaded_maintenance_current (bool);void arm_unloaded_maintenance ();
  void retire_unloaded_maintenance () noexcept;
  void before_generic_commit ();void after_amd_commit ();void after_generic_commit ();
};
}
using adapter=amd_owned_one_stop_v1::native_adapter;
namespace amd_owned_one_stop_v1 {
#include "actual-unloaded-methods.inc"
}
struct fixture {
  process_stratum_target proc;target_ops base;inferior inf;thread_info host,gpu;adapter a;
  fixture () {
    base.level=thread_stratum;inf.proc=&proc;inf.base=&base;inf.top=&the_amd_dbgapi_target;
    current_inf=&inf;native_target=&proc;raw_inferiors={&inf};
    host.inf=&inf;gpu.inf=&inf;gpu.global_num=2;gpu.ptid=ptid_t(100,1,77);
    inf.raw_threads={&host,&gpu};inf.ptid_thread_map={{host.ptid,&host},{gpu.ptid,&gpu}};
    a.m_inferior=&inf;a.m_inferior_ref.p=&inf;a.m_host=&host;a.m_host_ref.p=&host;
    a.m_loaded_process_ref.p=&proc;a.m_loaded_base_ref.p=&base;
    a.m_loaded_top_ref.p=&the_amd_dbgapi_target;
    a.m_gpu_thread=&gpu;a.m_gpu_thread_ref.p=&gpu;
    a.m_owner.phase_value=phase::awaiting_target_completion;
  }
  void arm_step () {a.arm_callback_maintenance(true);}
  void completed () {
    a.m_path=adapter::path::completion;a.m_generic_returned=a.m_generic_commit=true;
    a.m_amd_body_returned=a.m_amd_commit_returned=true;
    a.m_callback_progress=adapter::callback_progress::none;
    host.control.trap_expected=false;
    a.m_infrun_step=a.m_infrun_trap=a.m_infrun_inline=false;a.m_infrun_scope=minus_one_ptid;
    a.after_generic_commit();
  }
  void callback_step (adapter::terminal_stage stage) {
    a.retire_terminal_maintenance();a.m_owner.m_terminal=stage;
    a.m_owner.m_empty_objects=stage==adapter::terminal_stage::withdrawal_complete;
    ++a.m_owner.m_callbacks;++a.m_owner.m_completed_callbacks;
    a.m_path=adapter::path::callback_step;a.m_generic_returned=a.m_generic_commit=true;
    a.m_amd_body_returned=a.m_amd_commit_returned=true;
    a.m_callback_progress=adapter::callback_progress::step_inflight;
    host.control.trap_expected=true;a.m_infrun_step=a.m_infrun_trap=a.m_infrun_inline=true;
    a.m_infrun_scope=host.ptid;a.after_generic_commit();
  }
  void callback_continue () {
    a.retire_terminal_maintenance();a.m_callback_progress=adapter::callback_progress::continue_ready;
    a.m_path=adapter::path::callback_continue;a.m_generic_returned=a.m_generic_commit=true;
    a.m_amd_body_returned=a.m_amd_commit_returned=true;
    host.control.trap_expected=false;a.m_infrun_step=a.m_infrun_trap=a.m_infrun_inline=false;
    a.after_generic_commit();
  }
  void finish_terminal () {
    completed();callback_step(adapter::terminal_stage::delete_complete);callback_continue();
    callback_step(adapter::terminal_stage::withdrawal_complete);callback_continue();
  }
  void unload_ack () {
    a.retire_terminal_maintenance();a.m_owner.m_terminal=adapter::terminal_stage::unloaded;
    a.m_owner.m_runtime=false;a.info_value.runtime_state=AMD_DBGAPI_RUNTIME_STATE_UNLOADED;
    inf.top=&base;a.info_value.forward_progress_required=false;a.flush();a.arm_unloaded_maintenance();
  }
  void host_cycle () {a.before_generic_commit();a.after_generic_commit();}
  void exited () {gpu.state=THREAD_EXITED;inf.ptid_thread_map.erase(gpu.ptid);}
  void cycle () {a.before_generic_commit();a.after_amd_commit();a.after_generic_commit();}
  void enter_continue () {
    a.retire_callback_maintenance();
    a.m_callback_progress=adapter::callback_progress::none;
    host.control.trap_expected=false;
    a.m_infrun_step=a.m_infrun_trap=a.m_infrun_inline=false;
    a.arm_callback_maintenance(false);
  }
  void finish_original (bool stepping) {
    a.m_path=stepping?adapter::path::callback_step:adapter::path::callback_continue;
    a.m_generic_returned=a.m_generic_commit=true;
    a.m_amd_body_returned=a.m_amd_commit_returned=true;
    if(!stepping){a.m_callback_progress=adapter::callback_progress::continue_ready;
      host.control.trap_expected=false;
      a.m_infrun_step=a.m_infrun_trap=a.m_infrun_inline=false;}
    a.after_generic_commit();
  }
};
template<class F> void reject (fixture &f,F fn,failure why,commit_site site=commit_site::none) {
  bool seen=false;try{fn();}catch(const refused &e){seen=true;assert(e.why==why);
    if(site!=commit_site::none)assert(e.site==site);}
  assert(seen&&f.a.m_owner.invalid());
}
template<class F> void check_bad (F mutate,failure why=failure::commit) {
  fixture f;f.completed();mutate(f);
  reject(f,[&]{f.a.before_generic_commit();},why);
}
int main () {
  unsigned groups=0;
  {fixture f;f.completed();f.cycle();f.cycle();
   assert(f.a.m_terminal_origin==adapter::terminal_origin::completion);
   assert(f.a.m_terminal_stage==adapter::terminal_stage::idle);
   assert(f.a.m_owner.commit_credit==1&&f.a.m_owner.resume_credit==0);
   assert(f.a.flush_calls==1&&f.a.m_owner.m_callbacks==1);++groups;}
  {fixture f;f.completed();f.callback_step(adapter::terminal_stage::delete_complete);
   f.cycle();f.cycle();f.callback_continue();f.cycle();
   f.callback_step(adapter::terminal_stage::withdrawal_complete);f.cycle();
   f.callback_continue();f.cycle();assert(f.a.m_terminal_generation==3);
   assert(f.a.m_owner.commit_credit==1&&f.a.m_owner.resume_credit==0);
   assert(f.a.m_owner.m_completed_callbacks==3&&f.a.flush_calls==5);++groups;}
  {fixture f;f.exited();f.completed();f.cycle();f.callback_step(adapter::terminal_stage::delete_complete);
   f.cycle();f.callback_continue();f.cycle();f.callback_step(adapter::terminal_stage::withdrawal_complete);
   f.callback_continue();f.cycle();assert(!f.a.m_owner.invalid());++groups;}
  {fixture f;f.completed();f.exited();f.gpu.pending=true;
   f.cycle();assert(f.gpu.executing()&&f.gpu.resumed()&&f.gpu.pending);
   assert(f.a.m_owner.m_terminal==adapter::terminal_stage::idle);++groups;}
  {check_bad([](fixture&f){f.inf.ptid_thread_map.erase(f.gpu.ptid);});
   check_bad([](fixture&f){f.gpu.state=THREAD_EXITED;});
   check_bad([](fixture&f){f.inf.ptid_thread_map[f.gpu.ptid]=&f.host;});++groups;}
  {check_bad([](fixture&f){f.inf.raw_threads={&f.host};},failure::resume_scope);
   check_bad([](fixture&f){f.exited();f.inf.raw_threads={&f.host};},failure::resume_scope);
   check_bad([](fixture&f){f.inf.raw_threads.push_back(&f.host);},failure::resume_scope);++groups;}
  {check_bad([](fixture&f){f.a.m_gpu_thread_ref.p=&f.host;});
   check_bad([](fixture&f){f.gpu.global_num=3;});
   check_bad([](fixture&f){f.gpu.ptid=ptid_t(100,1,78);});
   check_bad([](fixture&f){f.gpu.inf=nullptr;});++groups;}
  {check_bad([](fixture&f){f.a.info_value.wave_info_map.clear();});
   check_bad([](fixture&f){f.a.info_value.wave_info_map.emplace(78,cached_wave{});});
   check_bad([](fixture&f){f.a.info_value.wave_info_map[77].coords.wave_id.handle=78;});
   check_bad([](fixture&f){f.a.info_value.wave_info_map[77].coords.queue_id.handle=99;});
   check_bad([](fixture&f){f.a.info_value.wave_info_map[77].coords.dispatch_id.handle=99;});
   check_bad([](fixture&f){f.a.info_value.wave_info_map[77].coords.agent_id.handle=99;});++groups;}
  {check_bad([](fixture&f){f.a.info_value.wave_info_map[77].last_resume_mode=1;});
   check_bad([](fixture&f){f.a.info_value.wave_info_map[77].stopping=true;});
   check_bad([](fixture&f){f.gpu.running=false;});
   check_bad([](fixture&f){f.gpu.resume=false;});
   check_bad([](fixture&f){f.gpu.pending=true;});++groups;}
  {check_bad([](fixture&f){f.host.running=false;});
   check_bad([](fixture&f){f.host.resume=false;});
   check_bad([](fixture&f){f.host.pending=true;});
   check_bad([](fixture&f){f.proc.pending=true;});
   check_bad([](fixture&f){f.proc.commit_resumed_state=false;});++groups;}
  {check_bad([](fixture&f){f.inf.top=&f.base;},failure::owner_changed);
   check_bad([](fixture&f){f.inf.pushed=false;},failure::owner_changed);
   check_bad([](fixture&f){f.a.m_owner.identity_matches=false;},failure::owner_changed);
   check_bad([](fixture&f){f.a.m_loaded_top_ref.p=&f.base;},failure::owner_changed);++groups;}
  {check_bad([](fixture&f){++f.a.m_owner.m_completed_callbacks;});
   check_bad([](fixture&f){f.a.m_owner.m_callback=true;});
   check_bad([](fixture&f){f.a.m_owner.m_runtime=false;});
   check_bad([](fixture&f){f.a.m_owner.phase_value=phase::host_running;});
   check_bad([](fixture&f){f.a.m_owner.m_resume=amd_owned_one_stop_v1::resume_kind::publication;});++groups;}
  {check_bad([](fixture&f){f.a.m_owner.m_terminal=adapter::terminal_stage::delete_complete;});
   check_bad([](fixture&f){f.a.m_owner.m_empty_objects=true;});
   check_bad([](fixture&f){f.a.m_callback_progress=adapter::callback_progress::continue_ready;});
   check_bad([](fixture&f){f.a.m_infrun_scope=f.host.ptid;});++groups;}
  {fixture f;f.completed();f.callback_step(adapter::terminal_stage::delete_complete);
   f.a.m_infrun_scope=minus_one_ptid;
   reject(f,[&]{f.cycle();},failure::commit,commit_site::terminal_thread_state);++groups;}
  {fixture f;f.completed();f.callback_step(adapter::terminal_stage::delete_complete);
   f.a.m_infrun_inline=false;reject(f,[&]{f.cycle();},failure::commit,commit_site::terminal_thread_state);++groups;}
  {fixture f;f.completed();f.callback_step(adapter::terminal_stage::delete_complete);
   f.callback_continue();f.host.control.trap_expected=true;
   reject(f,[&]{f.cycle();},failure::commit,commit_site::terminal_thread_state);++groups;}
  {fixture f;f.completed();reject(f,[&]{f.callback_step(adapter::terminal_stage::withdrawal_complete);},
     failure::commit,commit_site::terminal_arm);++groups;}
  {fixture f;f.completed();f.callback_step(adapter::terminal_stage::delete_complete);
   reject(f,[&]{f.callback_step(adapter::terminal_stage::withdrawal_complete);},
     failure::commit,commit_site::terminal_arm);++groups;}
  {fixture f;f.completed();f.callback_step(adapter::terminal_stage::delete_complete);
   f.callback_continue();reject(f,[&]{f.callback_step(adapter::terminal_stage::delete_complete);},
     failure::commit,commit_site::terminal_arm);++groups;}
  {fixture f;f.completed();f.callback_step(adapter::terminal_stage::delete_complete);
   f.callback_continue();f.callback_step(adapter::terminal_stage::withdrawal_complete);
   f.a.m_owner.m_empty_objects=false;
   reject(f,[&]{f.cycle();},failure::commit,commit_site::terminal_environment);++groups;}
  {fixture f;f.completed();f.a.before_generic_commit();
   reject(f,[&]{f.a.before_generic_commit();},failure::commit,commit_site::terminal_before);++groups;}
  {fixture f;f.completed();f.a.before_generic_commit();
   reject(f,[&]{f.a.after_generic_commit();},failure::commit,commit_site::terminal_after);++groups;}
  {fixture f;f.completed();f.a.before_generic_commit();f.a.after_amd_commit();
   reject(f,[&]{f.a.after_amd_commit();},failure::commit,commit_site::terminal_after_amd);++groups;}
  {fixture f;f.completed();f.a.before_generic_commit();f.a.info_value.forward_progress_required=false;
   reject(f,[&]{f.a.after_amd_commit();},failure::commit,commit_site::terminal_environment);++groups;}
  {fixture f;f.completed();f.a.retire_terminal_maintenance();assert(!f.a.m_owner.invalid());
   assert(f.a.m_terminal_origin==adapter::terminal_origin::completion);
   reject(f,[&]{f.a.before_generic_commit();},failure::commit,commit_site::before_maintenance);++groups;}
  {fixture f;f.completed();f.a.before_generic_commit();f.a.retire_terminal_maintenance();
   assert(f.a.m_owner.invalid()&&f.a.m_terminal_epoch==adapter::callback_epoch::inactive);++groups;}
  {fixture f;f.completed();f.a.before_generic_commit();f.a.after_amd_commit();f.a.retire_terminal_maintenance();
   assert(f.a.m_owner.invalid());++groups;}
  {fixture f;f.a.m_owner.work_limit=0;
   reject(f,[&]{f.completed();},failure::budget);
   assert(f.a.m_terminal_epoch==adapter::callback_epoch::inactive);++groups;}
  {fixture f;f.completed();const auto before=f.a.m_owner.work;f.a.m_owner.work_limit=before;
   reject(f,[&]{f.a.before_generic_commit();},failure::budget);
   assert(f.a.m_owner.work==before&&f.a.m_terminal_epoch==adapter::callback_epoch::armed);++groups;}
  {fixture f;f.a.m_owner.selected_value=false;f.a.before_generic_commit();f.a.after_amd_commit();f.a.after_generic_commit();
   assert(f.a.m_owner.work==0&&f.a.flush_calls==0);++groups;}
  {fixture f;f.a.m_owner.phase_value=phase::host_running;f.a.m_gpu_thread=nullptr;f.a.m_gpu_thread_ref.p=nullptr;
   f.finish_original(true);f.cycle();f.a.retire_callback_maintenance();f.finish_original(false);f.cycle();
   assert(f.a.m_terminal_origin==adapter::terminal_origin::none&&f.a.m_owner.commit_credit==0);++groups;}
  {fixture f;f.completed();f.a.retire_terminal_maintenance();
   reject(f,[&]{f.a.arm_terminal_maintenance(adapter::terminal_origin::completion);},
     failure::commit,commit_site::terminal_arm);++groups;}
  {fixture f;f.completed();f.a.retire_terminal_maintenance();f.a.m_terminal_generation=UINT64_MAX;
   reject(f,[&]{f.a.arm_terminal_maintenance(adapter::terminal_origin::callback_step);},
     failure::commit,commit_site::terminal_arm);++groups;}
  {fixture f;f.finish_terminal();f.unload_ack();f.host_cycle();f.host_cycle();
   assert(f.a.m_unloaded_epoch==adapter::unloaded_epoch::armed);
   assert(f.a.m_owner.commit_credit==1&&f.a.flush_calls==6);
   assert(f.a.m_owner.m_terminal==adapter::terminal_stage::unloaded);++groups;}
  {fixture f;f.exited();f.gpu.pending=true;f.finish_terminal();f.unload_ack();f.host_cycle();
   assert(f.gpu.executing()&&f.gpu.resumed()&&f.gpu.pending);++groups;}
  {fixture f;f.finish_terminal();f.proc.commit_resumed_state=false;f.unload_ack();
   f.proc.commit_resumed_state=true;f.host_cycle();++groups;}
  {fixture f;f.finish_terminal();f.unload_ack();
   reject(f,[&]{f.a.after_amd_commit();},failure::commit,commit_site::unloaded_unexpected_amd);++groups;}
  {fixture f;f.finish_terminal();f.unload_ack();f.a.before_generic_commit();
   reject(f,[&]{f.a.after_amd_commit();},failure::commit,commit_site::unloaded_unexpected_amd);++groups;}
  {fixture f;f.finish_terminal();f.unload_ack();f.a.before_generic_commit();
   reject(f,[&]{f.a.before_generic_commit();},failure::commit,commit_site::unloaded_before);++groups;}
  {fixture f;f.finish_terminal();f.unload_ack();
   reject(f,[&]{f.a.after_generic_commit();},failure::commit,commit_site::unloaded_after);++groups;}
  {fixture f;f.finish_terminal();f.unload_ack();f.inf.top=&the_amd_dbgapi_target;
   reject(f,[&]{f.host_cycle();},failure::owner_changed);++groups;}
  {fixture f;f.finish_terminal();f.unload_ack();f.a.m_owner.m_runtime=true;
   reject(f,[&]{f.host_cycle();},failure::commit,commit_site::unloaded_environment);++groups;}
  {fixture f;f.finish_terminal();f.unload_ack();f.a.info_value.runtime_state=AMD_DBGAPI_RUNTIME_STATE_LOADED_SUCCESS;
   reject(f,[&]{f.host_cycle();},failure::commit,commit_site::unloaded_environment);++groups;}
  {fixture f;f.completed();reject(f,[&]{f.unload_ack();},failure::commit,commit_site::unloaded_environment);++groups;}
  {fixture f;f.finish_terminal();f.unload_ack();f.a.m_owner.m_terminal=adapter::terminal_stage::withdrawal_complete;
   reject(f,[&]{f.host_cycle();},failure::commit,commit_site::unloaded_environment);++groups;}
  {fixture f;f.finish_terminal();f.unload_ack();++f.a.m_owner.m_completed_callbacks;
   reject(f,[&]{f.host_cycle();},failure::commit,commit_site::unloaded_environment);++groups;}
  {fixture f;f.finish_terminal();f.unload_ack();f.a.m_owner.m_empty_objects=false;
   reject(f,[&]{f.host_cycle();},failure::commit,commit_site::unloaded_environment);++groups;}
  {fixture f;f.finish_terminal();f.unload_ack();f.a.m_infrun_scope=minus_one_ptid;
   reject(f,[&]{f.host_cycle();},failure::commit,commit_site::unloaded_thread_state);++groups;}
  {fixture f;f.finish_terminal();f.unload_ack();f.a.m_owner.m_allocation_active=true;
   reject(f,[&]{f.host_cycle();},failure::commit,commit_site::unloaded_environment);++groups;}
  {fixture f;f.finish_terminal();f.unload_ack();f.a.retire_unloaded_maintenance();
   assert(f.a.m_unloaded_epoch==adapter::unloaded_epoch::retired&&!f.a.m_owner.invalid());
   reject(f,[&]{f.a.arm_unloaded_maintenance();},failure::commit,commit_site::unloaded_arm);++groups;}
  {fixture f;f.finish_terminal();f.unload_ack();f.a.before_generic_commit();f.a.retire_unloaded_maintenance();
   assert(f.a.m_owner.invalid()&&f.a.m_unloaded_epoch==adapter::unloaded_epoch::retired);++groups;}
  {fixture f;f.finish_terminal();const auto before=f.a.m_owner.work;f.a.m_owner.work_limit=before;
   reject(f,[&]{f.unload_ack();},failure::budget);
   assert(f.a.m_unloaded_epoch==adapter::unloaded_epoch::unused);++groups;}
  {fixture f;f.finish_terminal();f.unload_ack();f.a.m_owner.work_limit=f.a.m_owner.work;
   reject(f,[&]{f.a.before_generic_commit();},failure::budget);++groups;}
  {fixture f;f.finish_terminal();f.unload_ack();f.a.info_value.forward_progress_required=true;
   reject(f,[&]{f.host_cycle();},failure::commit,commit_site::unloaded_environment);++groups;}
  assert(groups==54);std::printf("unload-maintenance mock groups %u passed\n",groups);
}

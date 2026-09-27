// SPDX-License-Identifier: GPL-3.0-or-later
// CPU-only GDB surface mocks around the exact proposed adapter method bodies.
#include "amd-dbgapi-owned-one-stop-v1.h"
#define ATTRIBUTE_UNUSED_RESULT
#include "gdb_ref_ptr.h"
#include <cstdio>
#include <cstdlib>
#include <vector>
#include <cstdint>
enum strata { dummy_stratum,file_stratum,process_stratum,thread_stratum,record_stratum,arch_stratum,debug_stratum };
enum thread_state { THREAD_STOPPED,THREAD_RUNNING,THREAD_EXITED };
enum { AMD_DBGAPI_RUNTIME_STATE_UNLOADED=0,AMD_DBGAPI_RUNTIME_STATE_LOADED_SUCCESS=1 };
struct inferior;
using amd_dbgapi_status_t=int;
using amd_dbgapi_runtime_state_t=int;
struct amd_dbgapi_event_id_t {std::uint64_t handle;};
struct ptid_t { int value=7; bool wave=false; int pid() const { return value; } };
struct thread_info {
  inferior *inf=nullptr;
  int global_num=8;
  ptid_t ptid;
  thread_state state=THREAD_RUNNING;
  bool exec=true,resume=true,pending=false,gpu=false;
  bool executing()const{return exec;}
  bool resumed()const{return resume;}
  bool has_pending_waitstatus()const{return pending;}
};
bool ptid_is_gpu(ptid_t p){return p.wave;}
struct target_ops {
  strata kind=process_stratum;
  unsigned refs=1,closes=0;
  explicit target_ops(strata s=process_stratum):kind(s){}
  strata stratum()const{return kind;}
  void incref(){++refs;}
  void decref(){if(refs==0)std::abort();--refs;}
};
struct process_stratum_target:target_ops {
  bool threads_executing=true,commit_resumed_state=true,pending=false;
  thread_info *host=nullptr;
  bool has_resumed_with_pending_wait_status()const{return pending;}
  thread_info *find_thread(ptid_t p){return host && host->ptid.pid()==p.pid()?host:nullptr;}
};
unsigned closed_targets=0;
struct target_ops_ref_policy {
  static void incref(target_ops*t){t->incref();}
  static void decref(target_ops*t){t->decref();if(t->refs==0){++t->closes;++closed_targets;}}
};
using target_ops_ref=gdb::ref_ptr<target_ops,target_ops_ref_policy>;
extern target_ops the_amd_dbgapi_target;
struct program_space {};
struct inferior {
  program_space *pspace=nullptr;int pid=7,num=4;
  process_stratum_target *proc=nullptr;
  target_ops *top=nullptr,*host_top=nullptr;
  bool process_pushed=true,top_pushed=true,amd_pushed=false;
  process_stratum_target *process_target(){return proc;}
  target_ops *top_target(){return top;}
  bool target_is_pushed(const target_ops*t)const;
  target_ops *find_target_beneath(const target_ops*t){
    if(t==&the_amd_dbgapi_target && top==t)return host_top;
    return (t==host_top || t==top)?proc:nullptr;
  }
};
target_ops the_amd_dbgapi_target(arch_stratum);
bool inferior::target_is_pushed(const target_ops*t)const {
  if(t==&the_amd_dbgapi_target)return amd_pushed;
  if(t==proc)return process_pushed;
  return (t==host_top || t==top) && top_pushed;
}
inferior *current_inf=nullptr;
target_ops *native_target=nullptr;
inferior *current_inferior(){return current_inf;}
target_ops *get_native_target(){return native_target;}
struct mock_process_id { std::uint64_t handle=6; };
struct amd_dbgapi_inferior_info { inferior*inf=nullptr;mock_process_id process_id;int runtime_state=AMD_DBGAPI_RUNTIME_STATE_UNLOADED;bool forward_progress_required=true; };
struct registry {
  amd_dbgapi_inferior_info*value=nullptr;
  amd_dbgapi_inferior_info*get(inferior*){return value;}
} amd_dbgapi_inferior_data;
template<class T>struct retained_ref { T*value=nullptr;T*get()const{return value;} };
std::vector<inferior*>inferior_roster;
std::vector<thread_info*>thread_roster_fixture;
const std::vector<inferior*>&all_non_exited_inferiors(){return inferior_roster;}
const std::vector<thread_info*>&all_non_exited_threads(process_stratum_target*,int){return thread_roster_fixture;}
constexpr int minus_one_ptid=-1;
namespace amd_owned_one_stop_v1 {
enum class snapshot_issue {owner_changed,native_error};
struct mock_snapshot {void revoke(snapshot_issue){}};
struct observed_refusal { failure why; commit_site site; };
#include "loaded-scratch.inc"
class native_adapter {
public:
  enum class path:std::uint8_t {none,entry,callback_step,callback_continue,publication,completion};
  enum class callback_progress:std::uint8_t {none,step_ready,step_inflight,continue_ready};
  enum class entry_epoch:std::uint8_t {unused,entry_commit,armed,retired};
  owner m_owner;
  mock_snapshot m_snapshot;
  void native_unwind()noexcept;
  void invalidate(failure);
  inferior*m_inferior=nullptr;
  retained_ref<inferior>m_inferior_ref;
  thread_info*m_host=nullptr,*m_gpu_thread=nullptr;
  retained_ref<thread_info>m_host_ref,m_gpu_thread_ref;
  amd_dbgapi_inferior_info*m_info=nullptr;
  std::uint64_t m_start_ticks=5;
  enum class loaded_epoch:std::uint8_t {unused,prospective,armed,in_flight,amd_returned,retired};
  target_ops_ref m_loaded_process_ref,m_loaded_base_ref,m_loaded_top_ref;
  loaded_epoch m_loaded_epoch=loaded_epoch::unused;
  std::uintptr_t m_callback_object=0;
  target_ops_ref m_entry_process_ref,m_entry_top_ref;
  entry_epoch m_entry_epoch=entry_epoch::unused;
  bool m_entry_maintenance_inflight=false;
  path m_path=path::none;
  callback_progress m_callback_progress=callback_progress::none;
  bool m_generic_returned=false,m_generic_commit=false,m_amd_body_returned=false,m_amd_commit_returned=false;
  bool m_command_continue=false,m_exit_requested=false,m_infrun_origin=false;
  unsigned resumes=0,commits=0,flushes=0;
  bool selected()const noexcept{return m_owner.selected();}
  [[noreturn]]void reject(failure why,commit_site site=commit_site::none){m_owner.poison(why,site);retire_loaded_maintenance();retire_entry_maintenance();throw observed_refusal{m_owner.why(),m_owner.first_commit_site()};}
  void require(bool yes,failure why,commit_site site=commit_site::none);
  void debit(counter c,std::uint64_t n){if(!m_owner.debit(c,n))reject(failure::budget);}
  amd_dbgapi_inferior_info&info();
  identity current_identity();
  void thread_roster(std::size_t&,std::size_t&);
  void entry_maintenance_current();
  void retain_entry_commit_targets();
  void retire_entry_maintenance()noexcept;
  void prepare_loaded_maintenance(amd_dbgapi_inferior_info&);
  void loaded_maintenance_current(bool,bool);
  void arm_loaded_maintenance();
  void retire_loaded_maintenance()noexcept;
  void runtime_ack(amd_dbgapi_inferior_info&,amd_dbgapi_event_id_t,amd_dbgapi_runtime_state_t,amd_dbgapi_status_t);
  void runtime_ack_legacy(amd_dbgapi_inferior_info&,amd_dbgapi_event_id_t,amd_dbgapi_runtime_state_t,amd_dbgapi_status_t);
  void loaded_owner_fixture(unsigned n){if(n==0)m_owner.m_runtime=false;else if(n==1)m_owner.m_callbacks=1;else m_owner.m_object=9;}
  void before_generic_commit();
  void after_amd_commit();
  void after_generic_commit();
  void flush(){++flushes;}
  void setup(inferior*i,thread_info*t,amd_dbgapi_inferior_info*a){
    m_inferior=i;m_inferior_ref.value=i;m_host=t;m_host_ref.value=t;m_info=a;
    require(m_owner.reserve_selection(owner::fixed_storage(),limits::logical_bytes),failure::budget);
    require(m_owner.bind_selection(current_identity()),failure::owner_changed);
  }
  void returned_entry_fixture(){
    require(m_owner.entry_continuation_consumed(current_identity()),failure::resume_scope);
    m_path=path::entry;m_generic_returned=true;++resumes;
  }
  void phase_fixture(phase p){m_owner.m_phase=p;}
  void work_fixture(std::uint64_t n){m_owner.m_usage.work=n;}
  void owner_fixture(unsigned n){if(n==0)++m_owner.m_identity.inferior;else ++m_owner.m_identity.process;}
};
#include "actual-maintenance-bodies.inc"
} // namespace amd_owned_one_stop_v1
struct fixture {
  program_space space;
  process_stratum_target proc,other_proc;
  target_ops top{thread_stratum},other_top{thread_stratum};
  inferior inf,other_inf;
  thread_info host,extra;
  amd_dbgapi_inferior_info info;
  amd_owned_one_stop_v1::native_adapter a;
  fixture(){
    inf.pspace=&space;inf.proc=&proc;inf.top=&top;inf.host_top=&top;host.inf=&inf;proc.host=&host;info.inf=&inf;
    current_inf=&inf;native_target=&proc;amd_dbgapi_inferior_data.value=&info;
    inferior_roster={&inf};thread_roster_fixture={&host};
    a.setup(&inf,&host,&info);
  }
  void initial_commit(){
    a.returned_entry_fixture();a.before_generic_commit();++a.commits;a.after_generic_commit();
  }
  void maintenance(){a.before_generic_commit();++a.commits;a.after_generic_commit();}
  unsigned progress_calls=0,ack_calls=0;
  void activate(){inf.top=&the_amd_dbgapi_target;inf.amd_pushed=true;info.runtime_state=AMD_DBGAPI_RUNTIME_STATE_LOADED_SUCCESS;++ack_calls;a.runtime_ack(info,amd_dbgapi_event_id_t{1},AMD_DBGAPI_RUNTIME_STATE_LOADED_SUCCESS,0);}
  void loaded_maintenance(){a.before_generic_commit();++a.commits;if(!info.forward_progress_required){++progress_calls;info.forward_progress_required=true;}a.after_amd_commit();a.after_generic_commit();}
};
namespace {
unsigned checks=0,groups=0;
void check(bool b){++checks;if(!b)std::abort();}
template<class Call>void refuses(amd_owned_one_stop_v1::failure f,Call call){
 bool caught=false;try{call();}catch(const amd_owned_one_stop_v1::observed_refusal&r){caught=true;check(r.why==f);}check(caught);
}
}

using amd_owned_one_stop_v1::failure;
failure mutate(fixture&f,unsigned which){
 using A=amd_owned_one_stop_v1::native_adapter;
 switch(which){
  case 0:current_inf=&f.other_inf;return failure::owner_changed;
  case 1:f.inf.proc=&f.other_proc;return failure::owner_changed;
  case 2:f.inf.top=&f.other_top;return failure::owner_changed;
  case 3:native_target=&f.other_proc;return failure::owner_changed;
  case 4:f.inf.process_pushed=false;return failure::owner_changed;
  case 5:f.inf.top_pushed=false;return failure::owner_changed;
  case 6:f.top.kind=record_stratum;return failure::owner_changed;
  case 7:f.host.inf=&f.other_inf;return failure::owner_changed;
  case 8:amd_dbgapi_inferior_data.value=nullptr;return failure::owner_changed;
  case 9:f.proc.host=nullptr;return failure::owner_changed;
  case 10:f.a.m_inferior_ref.value=nullptr;return failure::owner_changed;
  case 11:f.a.m_host_ref.value=nullptr;return failure::owner_changed;
  case 12:f.a.owner_fixture(0);return failure::owner_changed;
  case 13:++f.info.process_id.handle;return failure::owner_changed;
  case 14:++f.inf.pid;return failure::owner_changed;
  case 15:++f.host.global_num;return failure::owner_changed;
  case 16:f.host.exec=false;return failure::commit;
  case 17:f.host.resume=false;return failure::commit;
  case 18:f.host.state=THREAD_STOPPED;return failure::commit;
  case 19:f.host.pending=true;return failure::commit;
  case 20:f.proc.threads_executing=false;return failure::commit;
  case 21:f.proc.commit_resumed_state=false;return failure::commit;
  case 22:f.proc.pending=true;return failure::commit;
  case 23:f.info.runtime_state=AMD_DBGAPI_RUNTIME_STATE_LOADED_SUCCESS;return failure::commit;
  case 24:f.inf.amd_pushed=true;return failure::commit;
  case 25:f.a.m_command_continue=true;return failure::commit;
  case 26:f.a.m_exit_requested=true;return failure::commit;
  case 27:f.a.m_infrun_origin=true;return failure::commit;
  case 28:f.a.m_callback_progress=A::callback_progress::step_ready;return failure::commit;
  case 29:f.a.m_gpu_thread=&f.extra;return failure::commit;
  case 30:f.a.phase_fixture(amd_owned_one_stop_v1::phase::provisional_checkpoint);return failure::commit;
  case 31:f.extra.inf=&f.inf;thread_roster_fixture.push_back(&f.extra);return failure::resume_scope;
  case 32:inferior_roster.push_back(&f.other_inf);return failure::resume_scope;
  case 33:f.a.m_path=A::path::publication;return failure::commit;
  case 34:f.a.m_generic_returned=true;return failure::commit;
  case 35:f.a.m_generic_commit=true;return failure::commit;
  case 36:f.a.m_amd_body_returned=true;return failure::commit;
  case 37:f.a.m_amd_commit_returned=true;return failure::commit;
  case 38:f.a.m_entry_epoch=A::entry_epoch::retired;return failure::commit;
  case 39:f.a.m_entry_maintenance_inflight=!f.a.m_entry_maintenance_inflight;return failure::commit;
  case 40:f.extra.inf=&f.inf;f.extra.ptid.wave=true;thread_roster_fixture.push_back(&f.extra);return failure::resume_scope;
  case 41:inferior_roster.clear();return failure::resume_scope;
  case 42:thread_roster_fixture.clear();return failure::resume_scope;
 }
 std::abort();
}
amd_owned_one_stop_v1::failure mutate_loaded(fixture&f,unsigned n){
 using A=amd_owned_one_stop_v1::native_adapter;
 switch(n){
 case 23:f.info.runtime_state=AMD_DBGAPI_RUNTIME_STATE_UNLOADED;return failure::commit;
 case 24:f.inf.amd_pushed=false;return failure::owner_changed;
 case 38:f.a.m_loaded_epoch=A::loaded_epoch::retired;return failure::commit;
 case 43:f.a.loaded_owner_fixture(0);return failure::commit;
 case 44:f.a.loaded_owner_fixture(1);return failure::commit;
 case 45:f.a.loaded_owner_fixture(2);return failure::commit;
 case 46:f.a.m_loaded_base_ref.reset(nullptr);return failure::owner_changed;
 case 47:f.a.m_loaded_process_ref.reset(nullptr);return failure::owner_changed;
 case 48:f.a.m_loaded_top_ref.reset(nullptr);return failure::owner_changed;
 case 49:f.a.m_callback_object=1;return failure::commit;
 default:return mutate(f,n);
 }
}
int main(int argc,char**){
 if(argc!=1)return 2;
 using namespace amd_owned_one_stop_v1;
 using A=native_adapter;
 {
  ++groups;fixture f;f.initial_commit();
  const auto rows=f.a.m_owner.consumed().rows;
  const auto work=f.a.m_owner.consumed().work;
  check(f.a.m_entry_epoch==A::entry_epoch::armed);
  check(f.proc.refs==2 && f.top.refs==2);
  for(unsigned n=0;n<3;++n)f.maintenance();
  check(f.a.resumes==1 && f.a.commits==4 && f.a.flushes==1);
  check(f.a.m_owner.state()==phase::host_running && !f.a.m_owner.invalid());
  check(f.a.m_owner.consumed().rows==rows && f.a.m_owner.consumed().work==work+3*260);
  check(!f.a.m_entry_maintenance_inflight && f.a.m_path==A::path::none);
  f.a.retire_entry_maintenance();
  check(f.a.m_entry_epoch==A::entry_epoch::retired);
  check(f.proc.refs==1 && f.top.refs==1 && f.proc.closes==0 && f.top.closes==0);
  refuses(failure::commit,[&]{f.maintenance();});
  check(f.a.resumes==1 && f.a.commits==4);
 }
 {
  ++groups;fixture f;f.inf.top=&f.proc;f.initial_commit();f.maintenance();
  check(f.proc.refs==3 && f.a.commits==2 && f.a.resumes==1);
  f.a.retire_entry_maintenance();check(f.proc.refs==1 && f.proc.closes==0);
 }
 {
  ++groups;fixture f;refuses(failure::commit,[&]{f.maintenance();});
  check(f.a.resumes==0 && f.a.commits==0 && f.proc.refs==1);
 }
 {
  ++groups;native_adapter a;a.before_generic_commit();a.after_amd_commit();a.after_generic_commit();
  check(!a.selected() && a.commits==0 && a.resumes==0 && a.flushes==0);
 }
 for(unsigned which=0;which<43;++which){
  ++groups;fixture f;f.initial_commit();
  const failure expected=mutate(f,which);
  refuses(expected,[&]{f.maintenance();});
  check(f.a.resumes==1 && f.a.commits==1 && f.a.m_owner.invalid());
  refuses(expected,[&]{f.a.before_generic_commit();});
 }
 for(unsigned which=0;which<43;++which){
  ++groups;fixture f;f.initial_commit();f.a.before_generic_commit();++f.a.commits;
  const failure expected=mutate(f,which);
  refuses(expected,[&]{f.a.after_generic_commit();});
  check(f.a.resumes==1 && f.a.commits==2 && f.a.m_owner.invalid());
  refuses(expected,[&]{f.a.before_generic_commit();});
 }
 {
  ++groups;fixture f;f.a.returned_entry_fixture();f.a.before_generic_commit();
  check(f.a.m_entry_epoch==A::entry_epoch::entry_commit && f.proc.refs==2);
  f.inf.proc=&f.other_proc;++f.a.commits;
  refuses(failure::owner_changed,[&]{f.a.after_generic_commit();});
  check(f.a.m_entry_epoch==A::entry_epoch::retired && f.a.resumes==1);
 }
 {
  ++groups;fixture f;f.initial_commit();f.a.before_generic_commit();
  refuses(failure::commit,[&]{f.a.before_generic_commit();});
  check(f.a.commits==1 && f.a.resumes==1);
 }
 {
  ++groups;fixture f;f.initial_commit();f.a.before_generic_commit();
  refuses(failure::commit,[&]{f.a.after_amd_commit();});
  check(f.a.commits==1 && f.a.resumes==1);
 }
 {
  ++groups;fixture f;f.initial_commit();
  refuses(failure::commit,[&]{f.a.after_generic_commit();});
  check(f.a.commits==1 && f.a.resumes==1);
 }
 {
  ++groups;fixture f;f.initial_commit();
  refuses(failure::order,[&]{f.a.returned_entry_fixture();});
  check(f.a.resumes==1);
 }
 {
  ++groups;fixture f;f.initial_commit();f.a.work_fixture(limits::work-260);
  f.maintenance();check(f.a.m_owner.consumed().work==limits::work);
  refuses(failure::budget,[&]{f.maintenance();});
  check(f.a.commits==2 && f.a.resumes==1);
 }
 {
  ++groups;fixture f;f.initial_commit();f.a.work_fixture(limits::work-129);
  refuses(failure::budget,[&]{f.maintenance();});
  check(f.a.commits==1 && f.a.resumes==1);
 }
 {
  ++groups;fixture f;f.initial_commit();f.a.before_generic_commit();
  f.a.retire_entry_maintenance();
  check(f.a.m_owner.invalid() && f.a.m_owner.why()==failure::commit);
  check(f.proc.refs==2 && f.top.refs==2); // Effect refs survive retirement.
  refuses(failure::commit,[&]{f.a.after_generic_commit();});
 }
 {
  ++groups;const auto before=closed_targets;
  {
   fixture f;f.initial_commit();f.inf.process_pushed=false;f.inf.top_pushed=false;
   f.proc.decref();f.top.decref(); // Model actual stack references being removed.
   refuses(failure::owner_changed,[&]{f.maintenance();});
   check(f.proc.refs==1 && f.top.refs==1 && closed_targets==before);
   check(f.a.m_entry_epoch==A::entry_epoch::retired);
  } // Actual gdb::ref_ptr destructor releases the two retained mock refs.
  check(closed_targets==before+2);
 }
 {
  ++groups;fixture f;f.initial_commit();f.a.retire_entry_maintenance();
  const auto work=f.a.m_owner.consumed().work;
  f.a.retire_entry_maintenance();check(f.a.m_owner.consumed().work==work);
  check(f.proc.refs==1 && f.top.refs==1);
 }

 // Added controls only. Every pre-existing fixture/control above is retained.
 {
  ++groups;fixture f;f.initial_commit();f.maintenance();
  check(f.a.m_owner.first_commit_site()==commit_site::none);
 }
 for(unsigned which=16;which<=30;++which){
  ++groups;fixture f;f.initial_commit();const auto why=mutate(f,which);
  refuses(why,[&]{f.maintenance();});
  const auto site=which<=22?commit_site::entry_thread_state:commit_site::entry_environment;
  check(f.a.m_owner.first_commit_site()==site);
  refuses(why,[&]{f.a.after_amd_commit();});
  check(f.a.m_owner.first_commit_site()==site);
 }
 {
  ++groups;fixture f;refuses(failure::commit,[&]{f.maintenance();});
  check(f.a.m_owner.first_commit_site()==commit_site::before_maintenance);
 }
 {
  ++groups;fixture f;f.a.returned_entry_fixture();
  f.a.m_entry_epoch=A::entry_epoch::retired;
  refuses(failure::commit,[&]{f.a.before_generic_commit();});
  check(f.a.m_owner.first_commit_site()==commit_site::entry_retention);
 }
 {
  ++groups;fixture f;f.initial_commit();f.a.before_generic_commit();
  f.a.retire_entry_maintenance();
  check(f.a.m_owner.first_commit_site()==commit_site::entry_retirement);
  refuses(failure::commit,[&]{f.a.after_generic_commit();});
  check(f.a.m_owner.first_commit_site()==commit_site::entry_retirement);
 }
 {
  ++groups;fixture f;f.initial_commit();f.a.m_path=A::path::publication;
  refuses(failure::commit,[&]{f.a.before_generic_commit();});
  check(f.a.m_owner.first_commit_site()==commit_site::before_pending);
 }
 {
  ++groups;fixture f;f.initial_commit();
  refuses(failure::commit,[&]{f.a.after_amd_commit();});
  check(f.a.m_owner.first_commit_site()==commit_site::after_amd);
 }
 {
  ++groups;fixture f;f.initial_commit();f.a.before_generic_commit();
  f.a.m_generic_returned=true;
  refuses(failure::commit,[&]{f.a.after_generic_commit();});
  check(f.a.m_owner.first_commit_site()==commit_site::after_maintenance);
 }
 {
  ++groups;fixture f;f.initial_commit();
  refuses(failure::commit,[&]{f.a.after_generic_commit();});
  check(f.a.m_owner.first_commit_site()==commit_site::after_pending);
 }
 {
  ++groups;fixture f;f.a.returned_entry_fixture();f.a.before_generic_commit();
  f.a.m_entry_epoch=A::entry_epoch::retired;
  refuses(failure::commit,[&]{f.a.after_generic_commit();});
  check(f.a.m_owner.first_commit_site()==commit_site::after_entry_epoch);
 }


// Source-extracted loaded ACK/commit controls. Mock effects follow the original
// target.c/amd-dbgapi-target.c order; no debugger, process or GPU is executed.
{
 ++groups; fixture f; f.initial_commit(); f.inf.top=&the_amd_dbgapi_target;
 f.inf.amd_pushed=true; f.info.runtime_state=AMD_DBGAPI_RUNTIME_STATE_LOADED_SUCCESS;
 f.a.runtime_ack_legacy(f.info,amd_dbgapi_event_id_t{1},AMD_DBGAPI_RUNTIME_STATE_LOADED_SUCCESS,0);
 check(f.a.m_entry_epoch==A::entry_epoch::retired);
 refuses(failure::commit,[&]{f.maintenance();});
 check(f.a.m_owner.first_commit_site()==commit_site::before_maintenance);
 check(f.a.resumes==1 && f.a.commits==1);
}
{
 ++groups; fixture f; f.initial_commit(); f.activate();
 check(f.a.m_entry_epoch==A::entry_epoch::retired && f.a.m_loaded_epoch==A::loaded_epoch::armed);
 check(f.proc.refs==2 && f.top.refs==2 && the_amd_dbgapi_target.refs==2);
 const auto rows=f.a.m_owner.consumed().rows,work=f.a.m_owner.consumed().work;
 const auto flush=f.a.flushes;
 for(unsigned n=0;n<3;++n){f.info.forward_progress_required=false;f.loaded_maintenance();}
 check(f.a.resumes==1 && f.a.commits==4 && f.a.flushes==flush);
 check(f.progress_calls==3 && f.ack_calls==1);
 check(f.a.m_owner.consumed().rows==rows && f.a.m_owner.consumed().work==work+3*486);
 check(f.a.m_loaded_epoch==A::loaded_epoch::armed && !f.a.m_owner.invalid());
 f.a.retire_loaded_maintenance();
 check(f.a.m_loaded_epoch==A::loaded_epoch::retired);
 check(f.proc.refs==2 && f.top.refs==2 && the_amd_dbgapi_target.refs==2);
 refuses(failure::commit,[&]{f.loaded_maintenance();});
 check(f.a.commits==4);
}
{
 ++groups; fixture f; f.inf.top=&f.proc; f.inf.host_top=&f.proc;
 f.initial_commit();f.activate();f.loaded_maintenance();
 check(f.a.resumes==1 && f.a.commits==2 && f.proc.refs==3);
}
{
 ++groups; fixture f; refuses(failure::commit,[&]{f.activate();});
 check(f.a.resumes==0 && f.a.commits==0 && f.a.m_owner.invalid());
}
{
 ++groups; fixture f; f.initial_commit(); f.inf.top=&the_amd_dbgapi_target;
 f.inf.amd_pushed=true;f.info.runtime_state=AMD_DBGAPI_RUNTIME_STATE_LOADED_SUCCESS;
 refuses(failure::owner_changed,[&]{f.loaded_maintenance();});
 check(f.a.commits==1 && f.a.m_loaded_epoch==A::loaded_epoch::unused);
}
{
 ++groups; fixture f; f.initial_commit();f.activate();
 const auto rows=f.a.m_owner.consumed().rows;
 refuses(failure::commit,[&]{f.activate();});
 check(f.a.m_owner.consumed().rows==rows && f.a.commits==1);
}
for(unsigned which=0;which<50;++which) {
 ++groups;fixture f;f.initial_commit();f.activate();
 const auto why=mutate_loaded(f,which);
 refuses(why,[&]{f.loaded_maintenance();});
 check(f.a.resumes==1 && f.a.commits==1 && f.a.m_owner.invalid());
}
for(unsigned which=0;which<50;++which) {
 ++groups;fixture f;f.initial_commit();f.activate();
 f.a.before_generic_commit();++f.a.commits;
 // Simulated original beneath + successful progress effect returns once.
 f.info.forward_progress_required=true;f.a.after_amd_commit();
 const auto why=mutate_loaded(f,which);
 refuses(why,[&]{f.a.after_generic_commit();});
 check(f.a.resumes==1 && f.a.commits==2 && f.a.m_owner.invalid());
}
{
 ++groups;fixture f;f.initial_commit();f.activate();
 f.a.before_generic_commit();
 refuses(failure::commit,[&]{f.a.before_generic_commit();});
 check(f.a.commits==1 && f.a.m_loaded_epoch==A::loaded_epoch::retired);
}
{
 ++groups;fixture f;f.initial_commit();f.activate();f.a.before_generic_commit();++f.a.commits;
 refuses(failure::commit,[&]{f.a.after_generic_commit();});
 check(f.a.m_owner.first_commit_site()==commit_site::loaded_after);
}
{
 ++groups;fixture f;f.initial_commit();f.activate();f.a.before_generic_commit();++f.a.commits;
 f.info.forward_progress_required=false;
 refuses(failure::commit,[&]{f.a.after_amd_commit();});
 check(f.a.m_owner.first_commit_site()==commit_site::loaded_environment);
}
{
 ++groups;fixture f;f.initial_commit();f.activate();f.a.before_generic_commit();++f.a.commits;
 f.info.forward_progress_required=true;f.a.after_amd_commit();
 refuses(failure::commit,[&]{f.a.after_amd_commit();});
 check(f.a.m_owner.invalid() && f.a.commits==2);
}
{
 ++groups;fixture f;f.initial_commit();f.activate();
 refuses(failure::commit,[&]{f.a.after_amd_commit();});check(f.a.commits==1);
}
{
 ++groups;fixture f;f.initial_commit();f.activate();
 refuses(failure::commit,[&]{f.a.after_generic_commit();});check(f.a.commits==1);
}
{
 ++groups;fixture f;f.initial_commit();f.activate();f.a.before_generic_commit();
 f.a.retire_loaded_maintenance();
 check(f.a.m_owner.invalid() && f.a.m_owner.first_commit_site()==commit_site::loaded_retirement);
 check(f.proc.refs==2 && f.top.refs==2 && the_amd_dbgapi_target.refs==2);
 const auto work=f.a.m_owner.consumed().work;f.a.retire_loaded_maintenance();
 check(f.a.m_owner.consumed().work==work && f.a.commits==1);
}
{
 ++groups;fixture f;f.initial_commit();f.activate();f.a.work_fixture(limits::work-486);
 f.loaded_maintenance();check(f.a.m_owner.consumed().work==limits::work);
 refuses(failure::budget,[&]{f.loaded_maintenance();});check(f.a.commits==2);
}
{
 ++groups;fixture f;f.initial_commit();f.activate();f.a.work_fixture(limits::work-161);
 refuses(failure::budget,[&]{f.loaded_maintenance();});check(f.a.commits==1);
}
{
 ++groups;fixture f;f.initial_commit();f.activate();f.a.work_fixture(limits::work-323);
 refuses(failure::budget,[&]{f.loaded_maintenance();});check(f.a.commits==2);
}
{
 ++groups;fixture f;f.initial_commit();f.a.work_fixture(limits::work-127);
 refuses(failure::budget,[&]{f.activate();});
 check(f.a.m_loaded_epoch==A::loaded_epoch::unused && f.a.commits==1);
}
{
 ++groups;fixture f;f.initial_commit();
 f.inf.top=&the_amd_dbgapi_target;f.inf.amd_pushed=true;
 f.info.runtime_state=AMD_DBGAPI_RUNTIME_STATE_LOADED_SUCCESS;
 refuses(failure::acknowledgement,[&]{f.a.runtime_ack(f.info,amd_dbgapi_event_id_t{1},AMD_DBGAPI_RUNTIME_STATE_LOADED_SUCCESS,1);});
 check(f.a.m_loaded_epoch==A::loaded_epoch::retired && f.proc.refs==2 && f.top.refs==2);
 check(f.a.commits==1);
}
{
 ++groups;const auto closes=closed_targets;{
  fixture f;f.initial_commit();f.activate();f.a.before_generic_commit();
  f.inf.process_pushed=false;f.inf.top_pushed=false;f.inf.amd_pushed=false;
  f.proc.decref();f.top.decref();
  refuses(failure::owner_changed,[&]{f.a.after_amd_commit();});
  check(f.proc.refs==1 && f.top.refs==1 && closed_targets==closes);
 }check(closed_targets==closes+2);
}
{
 ++groups;fixture f;f.initial_commit();f.activate();f.a.retire_loaded_maintenance();
 f.a.loaded_owner_fixture(1); // First callback already occurred: no later rearm.
 refuses(failure::commit,[&]{f.loaded_maintenance();});check(f.a.commits==1);
}


// Additional actual transition/refusal and typed-closure controls.
for(unsigned which=0;which<16;++which) {
 ++groups;fixture f;f.initial_commit();const auto why=mutate(f,which);
 if(which==2)f.inf.host_top=&f.other_top; // Activation changes top; preserve hostile base identity.
 refuses(why,[&]{f.activate();});check(f.a.commits==1 && f.a.m_owner.invalid());
}
{
 ++groups;fixture f;f.initial_commit();f.a.work_fixture(limits::work-354);
 refuses(failure::budget,[&]{f.activate();});
 check(f.a.m_loaded_epoch==A::loaded_epoch::retired && f.a.commits==1);
 check(f.proc.refs==2 && f.top.refs==2 && the_amd_dbgapi_target.refs==2);
}
{
 ++groups;fixture f;f.initial_commit();f.activate();f.a.before_generic_commit();
 f.a.native_unwind();
 check(f.a.m_owner.why()==failure::native_result && f.a.m_loaded_epoch==A::loaded_epoch::retired);
 check(f.proc.refs==2 && f.top.refs==2 && f.a.commits==1);
 refuses(failure::native_result,[&]{f.a.after_amd_commit();});
}
{
 ++groups;fixture f;f.initial_commit();f.activate();
 f.a.invalidate(failure::owner_changed);
 check(f.a.m_owner.why()==failure::owner_changed && f.a.m_loaded_epoch==A::loaded_epoch::retired);
 refuses(failure::owner_changed,[&]{f.loaded_maintenance();});check(f.a.commits==1);
}
{
 ++groups;using S=loaded_maintenance_scratch;
 const std::size_t selected=sizeof(S::ack_frame)+sizeof(S::prepare_frame)
  +sizeof(S::current_frame)+sizeof(S::arm_frame)+sizeof(S::retirement_frame)
  +sizeof(S::hook_frames)+sizeof(S::identity_frame)+sizeof(S::info_frame)
  +sizeof(S::roster_frame)+sizeof(S::require_frame)+sizeof(S::debit_frame)
  +sizeof(S::core_frames)+3*sizeof(S::ref_frame);
 check(sizeof(S)>=selected);
 check(sizeof(S::prepare_frame)>=sizeof(native_adapter*)+sizeof(amd_dbgapi_inferior_info*)
       +sizeof(process_stratum_target*)+2*sizeof(target_ops*));
 check(sizeof(S::current_frame)>=sizeof(native_adapter*)+2*sizeof(bool)
       +sizeof(process_stratum_target*)+2*sizeof(target_ops*)+2*sizeof(std::size_t)+sizeof(identity));
 check(sizeof(S::roster_frame)>=sizeof(native_adapter*)+2*sizeof(std::size_t*)
       +2*sizeof(std::size_t)+sizeof(inferior*)+sizeof(thread_info*));
 check(sizeof(S::ref_frame)>=3*sizeof(target_ops*)+sizeof(target_ops_ref)+2*sizeof(target_ops_ref*));
}

 std::printf("CPU-only real maintenance-hook controls: %u groups, %u checks; no native authority\n",groups,checks);
 return 0;
}

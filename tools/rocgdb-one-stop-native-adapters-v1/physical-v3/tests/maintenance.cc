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
struct program_space {};
struct inferior {
  program_space *pspace=nullptr;int pid=7,num=4;
  process_stratum_target *proc=nullptr;
  target_ops *top=nullptr;
  bool process_pushed=true,top_pushed=true,amd_pushed=false;
  process_stratum_target *process_target(){return proc;}
  target_ops *top_target(){return top;}
  bool target_is_pushed(const target_ops*t)const;
  target_ops *find_target_beneath(const target_ops*t){return t==top?proc:nullptr;}
};
target_ops the_amd_dbgapi_target(arch_stratum);
bool inferior::target_is_pushed(const target_ops*t)const {
  if(t==&the_amd_dbgapi_target)return amd_pushed;
  if(t==proc)return process_pushed;
  return t==top && top_pushed;
}
inferior *current_inf=nullptr;
target_ops *native_target=nullptr;
inferior *current_inferior(){return current_inf;}
target_ops *get_native_target(){return native_target;}
struct mock_process_id { std::uint64_t handle=6; };
struct amd_dbgapi_inferior_info { inferior*inf=nullptr;mock_process_id process_id;int runtime_state=AMD_DBGAPI_RUNTIME_STATE_UNLOADED; };
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
struct observed_refusal { failure why; };
class native_adapter {
public:
  enum class path:std::uint8_t {none,entry,callback_step,callback_continue,publication,completion};
  enum class callback_progress:std::uint8_t {none,step_ready,step_inflight,continue_ready};
  enum class entry_epoch:std::uint8_t {unused,entry_commit,armed,retired};
  owner m_owner;
  inferior*m_inferior=nullptr;
  retained_ref<inferior>m_inferior_ref;
  thread_info*m_host=nullptr,*m_gpu_thread=nullptr;
  retained_ref<thread_info>m_host_ref,m_gpu_thread_ref;
  amd_dbgapi_inferior_info*m_info=nullptr;
  std::uint64_t m_start_ticks=5;
  target_ops_ref m_entry_process_ref,m_entry_top_ref;
  entry_epoch m_entry_epoch=entry_epoch::unused;
  bool m_entry_maintenance_inflight=false;
  path m_path=path::none;
  callback_progress m_callback_progress=callback_progress::none;
  bool m_generic_returned=false,m_generic_commit=false,m_amd_body_returned=false,m_amd_commit_returned=false;
  bool m_command_continue=false,m_exit_requested=false,m_infrun_origin=false;
  unsigned resumes=0,commits=0,flushes=0;
  bool selected()const noexcept{return m_owner.selected();}
  [[noreturn]]void reject(failure why){m_owner.poison(why);retire_entry_maintenance();throw observed_refusal{m_owner.why()};}
  void require(bool yes,failure why);
  void debit(counter c,std::uint64_t n){if(!m_owner.debit(c,n))reject(failure::budget);}
  amd_dbgapi_inferior_info&info();
  identity current_identity();
  void thread_roster(std::size_t&,std::size_t&);
  void entry_maintenance_current();
  void retain_entry_commit_targets();
  void retire_entry_maintenance()noexcept;
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
    inf.pspace=&space;inf.proc=&proc;inf.top=&top;host.inf=&inf;proc.host=&host;info.inf=&inf;
    current_inf=&inf;native_target=&proc;amd_dbgapi_inferior_data.value=&info;
    inferior_roster={&inf};thread_roster_fixture={&host};
    a.setup(&inf,&host,&info);
  }
  void initial_commit(){
    a.returned_entry_fixture();a.before_generic_commit();++a.commits;a.after_generic_commit();
  }
  void maintenance(){a.before_generic_commit();++a.commits;a.after_generic_commit();}
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
 std::printf("CPU-only real maintenance-hook controls: %u groups, %u checks; no native authority\n",groups,checks);
 return 0;
}

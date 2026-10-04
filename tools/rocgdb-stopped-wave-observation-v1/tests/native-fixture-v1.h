/* SPDX-License-Identifier: GPL-3.0-or-later
   Synthetic native API/GDB stand-ins ONLY. Includes actual production hook
   leaves; no fake object in this file is runtime/source authority. */
#include <amd-dbgapi/amd-dbgapi.h>
#include <array>
#include <cstring>
#include <cinttypes>
#include <cstdio>
#include <cstdlib>
#include <stdexcept>
#include <string>
#include <type_traits>
#include <vector>
#include "../src/amd-dbgapi-stopped-wave-observation-v1.h"
#include "../../rocgdb-runtime-observation-v1/src/amd-dbgapi-runtime-observation-v1.h"
#define _(s) (s)
#define EQ_ID(T) inline bool operator==(T a,T b){return a.handle==b.handle;} \
  inline bool operator!=(T a,T b){return !(a==b);}
EQ_ID(amd_dbgapi_process_id_t)
EQ_ID(amd_dbgapi_wave_id_t)
EQ_ID(amd_dbgapi_workgroup_id_t)
EQ_ID(amd_dbgapi_dispatch_id_t)
EQ_ID(amd_dbgapi_queue_id_t)
EQ_ID(amd_dbgapi_agent_id_t)
EQ_ID(amd_dbgapi_architecture_id_t)
EQ_ID(amd_dbgapi_event_id_t)
#undef EQ_ID
struct ptid_t { int process=123; std::uint64_t wave=21;
  int pid()const{return process;} };
inline bool operator==(ptid_t a,ptid_t b){return a.process==b.process&&a.wave==b.wave;}
inline bool ptid_is_gpu(ptid_t p){return p.wave!=0;}
inline amd_dbgapi_wave_id_t get_amd_dbgapi_wave_id(ptid_t p){return {p.wave};}
inline ptid_t make_gpu_ptid(int pid,amd_dbgapi_wave_id_t w){return {pid,w.handle};}
struct program_space { void *cbfd=nullptr; };
struct thread_info;
struct fake_target { thread_info *thread=nullptr; thread_info *find_thread(ptid_t); };
static int the_amd_dbgapi_target;
enum { arch_stratum=1, THREAD_STOPPED=1 };
struct inferior { int num=1,pid=123; program_space *pspace=nullptr; fake_target target;
  int *target_at(int){return &the_amd_dbgapi_target;}
  fake_target *process_target(){return &target;} };
struct thread_info { inferior *inf=nullptr; ptid_t ptid; int global_num=9,state=THREAD_STOPPED;
  bool execution=false,resumption=false;
  bool executing()const{return execution;} bool resumed()const{return resumption;} };
thread_info *fake_target::find_thread(ptid_t p)
{ return thread && thread->ptid==p ? thread : nullptr; }
struct amd_dbgapi_inferior_info { inferior *inf=nullptr; amd_dbgapi_process_id_t process_id{11}; };
static program_space space;
static inferior inf;
static thread_info thread;
static amd_dbgapi_inferior_info info;
static ptid_t inferior_ptid;
static thread_info *selected=&thread;
static thread_info *inferior_thread(){return selected;}
static amd_dbgapi_inferior_info *get_amd_dbgapi_inferior_info(inferior*){return &info;}
static const char *get_status_string(amd_dbgapi_status_t){return "synthetic";}
[[noreturn]] static void error(const char *,...){throw std::runtime_error("synthetic refusal");}
static void warning(const char *,...){}
static unsigned queries, fail_at, acknowledgements, state_queries;
static bool not_available, foreign_process, changed_dispatch, changed_final_state;
static bool output_fails;
static amd_dbgapi_status_t ack_status=AMD_DBGAPI_STATUS_SUCCESS;
static std::vector<amd_stopped_wave_observation_v1::record> emitted;
static void interps_notify_amd_stopped_wave_observation_v1
  (const amd_stopped_wave_observation_v1::record &r)
{ if(output_fails)throw std::runtime_error("synthetic output failure");emitted.push_back(r); }
static void interps_notify_amd_runtime_observation_v1
  (const amd_runtime_observation_v1::record &){}
template<class T>
static amd_dbgapi_status_t give(std::size_t size,void *output,const T &value)
{
  if(size!=sizeof(T))throw std::runtime_error("native API size mismatch");
  std::memcpy(output,&value,size);return AMD_DBGAPI_STATUS_SUCCESS;
}
template<class Id,class Query>
static amd_dbgapi_status_t fake_info(Id id,Query query,std::size_t size,void *output)
{
  ++queries;
  if(fail_at==queries)return not_available?AMD_DBGAPI_STATUS_ERROR_NOT_AVAILABLE:AMD_DBGAPI_STATUS_ERROR;
  const amd_dbgapi_process_id_t process{foreign_process?12u:11u};
  const amd_dbgapi_agent_id_t agent{25};
  const amd_dbgapi_architecture_id_t architecture{26};
  const amd_dbgapi_queue_id_t queue{24};
  const amd_dbgapi_dispatch_id_t dispatch{changed_dispatch?33u:23u};
  const std::array<std::uint32_t,3> coord{0,0,0}, grid{64,1,1};
  const std::array<std::uint16_t,3> workgroup{64,1,1};
  const auto u32=[&](std::uint32_t v){return give(size,output,v);};
  const auto u64=[&](std::uint64_t v){return give(size,output,v);};
  static_cast<void>(u32); static_cast<void>(u64);
  if constexpr(std::is_same_v<Id,amd_dbgapi_event_id_t>) {
    if(id.handle!=100)throw std::runtime_error("wrong event");
    switch(query) {
      case AMD_DBGAPI_EVENT_INFO_PROCESS:return give(size,output,process);
      case AMD_DBGAPI_EVENT_INFO_KIND:return give(size,output,AMD_DBGAPI_EVENT_KIND_WAVE_STOP);
      case AMD_DBGAPI_EVENT_INFO_WAVE:return give(size,output,amd_dbgapi_wave_id_t{21});
      default:break;
    }
  } else if constexpr(std::is_same_v<Id,amd_dbgapi_wave_id_t>) {
    if(id.handle!=21)throw std::runtime_error("wrong wave");
    switch(query) {
      case AMD_DBGAPI_WAVE_INFO_STATE:
        ++state_queries;return u32(changed_final_state&&state_queries>1?1:3);
      case AMD_DBGAPI_WAVE_INFO_STOP_REASON:return u32(1u<<10);
      case AMD_DBGAPI_WAVE_INFO_PROCESS:return give(size,output,process);
      case AMD_DBGAPI_WAVE_INFO_WORKGROUP:return give(size,output,amd_dbgapi_workgroup_id_t{22});
      case AMD_DBGAPI_WAVE_INFO_DISPATCH:return give(size,output,amd_dbgapi_dispatch_id_t{23});
      case AMD_DBGAPI_WAVE_INFO_QUEUE:return give(size,output,queue);
      case AMD_DBGAPI_WAVE_INFO_AGENT:return give(size,output,agent);
      case AMD_DBGAPI_WAVE_INFO_ARCHITECTURE:return give(size,output,architecture);
      case AMD_DBGAPI_WAVE_INFO_LANE_COUNT:return give(size,output,std::size_t{64});
      case AMD_DBGAPI_WAVE_INFO_WORKGROUP_COORD:return give(size,output,coord);
      case AMD_DBGAPI_WAVE_INFO_WAVE_NUMBER_IN_WORKGROUP:return u32(0);
      default:break;
    }
  } else if constexpr(std::is_same_v<Id,amd_dbgapi_workgroup_id_t>) {
    if(id.handle!=22)throw std::runtime_error("wrong workgroup");
    switch(query) {
      case AMD_DBGAPI_WORKGROUP_INFO_PROCESS:return give(size,output,process);
      case AMD_DBGAPI_WORKGROUP_INFO_QUEUE:return give(size,output,queue);
      case AMD_DBGAPI_WORKGROUP_INFO_DISPATCH:return give(size,output,dispatch);
      case AMD_DBGAPI_WORKGROUP_INFO_AGENT:return give(size,output,agent);
      case AMD_DBGAPI_WORKGROUP_INFO_ARCHITECTURE:return give(size,output,architecture);
      case AMD_DBGAPI_WORKGROUP_INFO_WORKGROUP_COORD:return give(size,output,coord);
      default:break;
    }
  } else if constexpr(std::is_same_v<Id,amd_dbgapi_dispatch_id_t>) {
    if(id.handle!=23)throw std::runtime_error("wrong dispatch");
    switch(query) {
      case AMD_DBGAPI_DISPATCH_INFO_PROCESS:return give(size,output,process);
      case AMD_DBGAPI_DISPATCH_INFO_QUEUE:return give(size,output,queue);
      case AMD_DBGAPI_DISPATCH_INFO_AGENT:return give(size,output,agent);
      case AMD_DBGAPI_DISPATCH_INFO_ARCHITECTURE:return give(size,output,architecture);
      case AMD_DBGAPI_DISPATCH_INFO_OS_QUEUE_PACKET_ID:return u64(0);
      case AMD_DBGAPI_DISPATCH_INFO_GRID_DIMENSIONS:return u32(1);
      case AMD_DBGAPI_DISPATCH_INFO_GRID_SIZES:return give(size,output,grid);
      case AMD_DBGAPI_DISPATCH_INFO_WORKGROUP_SIZES:return give(size,output,workgroup);
      case AMD_DBGAPI_DISPATCH_INFO_PRIVATE_SEGMENT_SIZE:return u64(0);
      case AMD_DBGAPI_DISPATCH_INFO_GROUP_SEGMENT_SIZE:return u64(0);
      default:break;
    }
  } else if constexpr(std::is_same_v<Id,amd_dbgapi_queue_id_t>) {
    if(id.handle!=24)throw std::runtime_error("wrong queue");
    switch(query) {
      case AMD_DBGAPI_QUEUE_INFO_PROCESS:return give(size,output,process);
      case AMD_DBGAPI_QUEUE_INFO_AGENT:return give(size,output,agent);
      case AMD_DBGAPI_QUEUE_INFO_ARCHITECTURE:return give(size,output,architecture);
      case AMD_DBGAPI_QUEUE_INFO_OS_ID:return u64(0);
      case AMD_DBGAPI_QUEUE_INFO_TYPE:return u32(1);
      case AMD_DBGAPI_QUEUE_INFO_STATE:return u32(1);
      case AMD_DBGAPI_QUEUE_INFO_ERROR_REASON:return u32(0);
      default:break;
    }
  } else if constexpr(std::is_same_v<Id,amd_dbgapi_agent_id_t>) {
    if(id.handle!=25)throw std::runtime_error("wrong agent");
    switch(query) {
      case AMD_DBGAPI_AGENT_INFO_PROCESS:return give(size,output,process);
      case AMD_DBGAPI_AGENT_INFO_ARCHITECTURE:return give(size,output,architecture);
      case AMD_DBGAPI_AGENT_INFO_OS_ID:return u64(39903);
      case AMD_DBGAPI_AGENT_INFO_STATE:return u32(1);
      default:break;
    }
  } else if constexpr(std::is_same_v<Id,amd_dbgapi_architecture_id_t>) {
    if(id.handle!=26)throw std::runtime_error("wrong architecture");
    if(query==AMD_DBGAPI_ARCHITECTURE_INFO_ELF_AMDGPU_MACHINE)return u32(0x4f);
  }
  throw std::runtime_error("unsupported synthetic query");
}
static amd_dbgapi_status_t fake_processed(amd_dbgapi_event_id_t event)
{ if(event.handle!=100)std::abort();++acknowledgements;return ack_status; }
static inline amd_dbgapi_status_t fake_event_info(amd_dbgapi_event_id_t id, amd_dbgapi_event_info_t query, std::size_t size, void *output)
{ return fake_info(id,query,size,output); }
static inline amd_dbgapi_status_t fake_wave_info(amd_dbgapi_wave_id_t id, amd_dbgapi_wave_info_t query, std::size_t size, void *output)
{ return fake_info(id,query,size,output); }
static inline amd_dbgapi_status_t fake_workgroup_info(amd_dbgapi_workgroup_id_t id, amd_dbgapi_workgroup_info_t query, std::size_t size, void *output)
{ return fake_info(id,query,size,output); }
static inline amd_dbgapi_status_t fake_dispatch_info(amd_dbgapi_dispatch_id_t id, amd_dbgapi_dispatch_info_t query, std::size_t size, void *output)
{ return fake_info(id,query,size,output); }
static inline amd_dbgapi_status_t fake_queue_info(amd_dbgapi_queue_id_t id, amd_dbgapi_queue_info_t query, std::size_t size, void *output)
{ return fake_info(id,query,size,output); }
static inline amd_dbgapi_status_t fake_agent_info(amd_dbgapi_agent_id_t id, amd_dbgapi_agent_info_t query, std::size_t size, void *output)
{ return fake_info(id,query,size,output); }
static inline amd_dbgapi_status_t fake_architecture_info(amd_dbgapi_architecture_id_t id, amd_dbgapi_architecture_info_t query, std::size_t size, void *output)
{ return fake_info(id,query,size,output); }
#define amd_dbgapi_event_get_info fake_event_info
#define amd_dbgapi_wave_get_info fake_wave_info
#define amd_dbgapi_workgroup_get_info fake_workgroup_info
#define amd_dbgapi_dispatch_get_info fake_dispatch_info
#define amd_dbgapi_queue_get_info fake_queue_info
#define amd_dbgapi_agent_get_info fake_agent_info
#define amd_dbgapi_architecture_get_info fake_architecture_info
#define amd_dbgapi_event_processed fake_processed
#include "../src/amd-dbgapi-stopped-wave-observation-hooks-v1.inc"
#include "../src/amd-dbgapi-runtime-observation-hooks-v1.inc"
#include "../src/amd-dbgapi-stopped-wave-query-v1.inc"
static void reset_fixture()
{
  stopped_wave_observation=stopped_obs::ledger{};
  runtime_observation=runtime_obs::ledger{};
  queries=fail_at=acknowledgements=state_queries=0;
  not_available=foreign_process=changed_dispatch=changed_final_state=output_fails=false;
  ack_status=AMD_DBGAPI_STATUS_SUCCESS;emitted.clear();
  runtime_observation_pspace=&space;
  space={};inf={};inf.pspace=&space;thread={};thread.inf=&inf;inf.target.thread=&thread;
  info={};info.inf=&inf;inferior_ptid=thread.ptid;selected=&thread;
  stopped_obs::bind(info);stopped_obs::attached();
  stopped_obs::completed(stopped_obs::kind::runtime_complete,90,5,1,0);
  runtime_observation.bind(reinterpret_cast<std::uintptr_t>(&info),1,123,11);
  runtime_observation.attached();
}
static void stage_actual_hooks()
{
  observed_native_event_v1 handled(info,{100},AMD_DBGAPI_EVENT_KIND_WAVE_STOP);
  auto r=query_stopped_wave_event_v1(info,{100});
  const auto status=handled.finish();
  stopped_wave_observation.stage(100,1,r.thread,r.identity,status);
}

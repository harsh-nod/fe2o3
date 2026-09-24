/* SPDX-License-Identifier: GPL-3.0-or-later
   Actual same-client scalar query roster. This is separate private currentness,
   not an override of the legacy stopped-observation ledger's invalidation. */
namespace amd_owned_one_stop_v1 {
gpu_stop native_adapter::query_stop (thread_info *thread, bool require_gdb_current) {
  auto &info=this->info ();
  require (m_owner.check_owner (current_identity ()) && thread!=nullptr
           && thread->inf==m_inferior && thread->global_num>0
           && ptid_is_gpu (thread->ptid)
           && thread->ptid.pid ()==m_inferior->pid
           && m_inferior->pspace->cbfd==nullptr
           && m_inferior->target_at (arch_stratum)==&the_amd_dbgapi_target,
           failure::stop_changed);
  if (require_gdb_current)
    require (thread==inferior_thread () && thread->ptid==inferior_ptid
             && thread->state==THREAD_STOPPED && !thread->executing ()
             && !thread->resumed (),failure::stop_changed);
  require (read_start_ticks ()==m_start_ticks,failure::owner_changed);
  const auto join=[this] (bool equal) { require (equal,failure::stop_changed); };
  const auto wave = get_amd_dbgapi_wave_id (thread->ptid);
  stopped_obs::stop_identity s {};
  s.wave = wave.handle;
  amd_dbgapi_process_id_t process {};
  amd_dbgapi_workgroup_id_t workgroup {};
  amd_dbgapi_dispatch_id_t dispatch {};
  amd_dbgapi_queue_id_t queue {};
  amd_dbgapi_agent_id_t agent {};
  amd_dbgapi_architecture_id_t architecture {};
  std::size_t lanes = 0;
  query (amd_dbgapi_wave_get_info, wave, AMD_DBGAPI_WAVE_INFO_STATE, s.state);
  join (s.state == AMD_DBGAPI_WAVE_STATE_STOP);
  query (amd_dbgapi_wave_get_info, wave, AMD_DBGAPI_WAVE_INFO_STOP_REASON, s.stop_reason);
  query (amd_dbgapi_wave_get_info, wave, AMD_DBGAPI_WAVE_INFO_PROCESS, process);
  query (amd_dbgapi_wave_get_info, wave, AMD_DBGAPI_WAVE_INFO_WORKGROUP, workgroup);
  query (amd_dbgapi_wave_get_info, wave, AMD_DBGAPI_WAVE_INFO_DISPATCH, dispatch);
  query (amd_dbgapi_wave_get_info, wave, AMD_DBGAPI_WAVE_INFO_QUEUE, queue);
  query (amd_dbgapi_wave_get_info, wave, AMD_DBGAPI_WAVE_INFO_AGENT, agent);
  query (amd_dbgapi_wave_get_info, wave, AMD_DBGAPI_WAVE_INFO_ARCHITECTURE, architecture);
  query (amd_dbgapi_wave_get_info, wave, AMD_DBGAPI_WAVE_INFO_LANE_COUNT, lanes);
  query (amd_dbgapi_wave_get_info, wave, AMD_DBGAPI_WAVE_INFO_WORKGROUP_COORD, s.coordinate);
  query (amd_dbgapi_wave_get_info, wave, AMD_DBGAPI_WAVE_INFO_WAVE_NUMBER_IN_WORKGROUP, s.wave_index);
  join (process == info.process_id && lanes == 64);
  s.lanes = static_cast<std::uint32_t> (lanes);
  s.workgroup = workgroup.handle; s.dispatch = dispatch.handle;
  s.queue = queue.handle; s.agent = agent.handle; s.architecture = architecture.handle;

  amd_dbgapi_process_id_t group_process {};
  amd_dbgapi_queue_id_t group_queue {};
  amd_dbgapi_dispatch_id_t group_dispatch {};
  amd_dbgapi_agent_id_t group_agent {};
  amd_dbgapi_architecture_id_t group_architecture {};
  std::array<std::uint32_t, 3> group_coordinate {};
  query (amd_dbgapi_workgroup_get_info, workgroup, AMD_DBGAPI_WORKGROUP_INFO_PROCESS, group_process);
  query (amd_dbgapi_workgroup_get_info, workgroup, AMD_DBGAPI_WORKGROUP_INFO_QUEUE, group_queue);
  query (amd_dbgapi_workgroup_get_info, workgroup, AMD_DBGAPI_WORKGROUP_INFO_DISPATCH, group_dispatch);
  query (amd_dbgapi_workgroup_get_info, workgroup, AMD_DBGAPI_WORKGROUP_INFO_AGENT, group_agent);
  query (amd_dbgapi_workgroup_get_info, workgroup, AMD_DBGAPI_WORKGROUP_INFO_ARCHITECTURE, group_architecture);
  query (amd_dbgapi_workgroup_get_info, workgroup, AMD_DBGAPI_WORKGROUP_INFO_WORKGROUP_COORD, group_coordinate);
  join (group_process == process && group_queue == queue && group_dispatch == dispatch
                  && group_agent == agent && group_architecture == architecture
                  && group_coordinate == s.coordinate);

  amd_dbgapi_process_id_t dispatch_process {};
  amd_dbgapi_queue_id_t dispatch_queue {};
  amd_dbgapi_agent_id_t dispatch_agent {};
  amd_dbgapi_architecture_id_t dispatch_architecture {};
  std::array<std::uint16_t, 3> native_workgroup {};
  std::uint32_t dimensions = 0;
  query (amd_dbgapi_dispatch_get_info, dispatch, AMD_DBGAPI_DISPATCH_INFO_PROCESS, dispatch_process);
  query (amd_dbgapi_dispatch_get_info, dispatch, AMD_DBGAPI_DISPATCH_INFO_QUEUE, dispatch_queue);
  query (amd_dbgapi_dispatch_get_info, dispatch, AMD_DBGAPI_DISPATCH_INFO_AGENT, dispatch_agent);
  query (amd_dbgapi_dispatch_get_info, dispatch, AMD_DBGAPI_DISPATCH_INFO_ARCHITECTURE, dispatch_architecture);
  query (amd_dbgapi_dispatch_get_info, dispatch, AMD_DBGAPI_DISPATCH_INFO_OS_QUEUE_PACKET_ID, s.packet);
  query (amd_dbgapi_dispatch_get_info, dispatch, AMD_DBGAPI_DISPATCH_INFO_GRID_DIMENSIONS, dimensions);
  query (amd_dbgapi_dispatch_get_info, dispatch, AMD_DBGAPI_DISPATCH_INFO_GRID_SIZES, s.grid);
  query (amd_dbgapi_dispatch_get_info, dispatch, AMD_DBGAPI_DISPATCH_INFO_WORKGROUP_SIZES, native_workgroup);
  query (amd_dbgapi_dispatch_get_info, dispatch, AMD_DBGAPI_DISPATCH_INFO_PRIVATE_SEGMENT_SIZE, s.private_bytes);
  query (amd_dbgapi_dispatch_get_info, dispatch, AMD_DBGAPI_DISPATCH_INFO_GROUP_SEGMENT_SIZE, s.group_bytes);
  join (dispatch_process == process && dispatch_queue == queue
                  && dispatch_agent == agent && dispatch_architecture == architecture
                  && dimensions == 1);
  for (std::size_t i = 0; i < 3; ++i) s.workgroup_size[i] = native_workgroup[i];

  amd_dbgapi_process_id_t queue_process {};
  amd_dbgapi_agent_id_t queue_agent {};
  amd_dbgapi_architecture_id_t queue_architecture {};
  std::uint32_t queue_error = 0;
  query (amd_dbgapi_queue_get_info, queue, AMD_DBGAPI_QUEUE_INFO_PROCESS, queue_process);
  query (amd_dbgapi_queue_get_info, queue, AMD_DBGAPI_QUEUE_INFO_AGENT, queue_agent);
  query (amd_dbgapi_queue_get_info, queue, AMD_DBGAPI_QUEUE_INFO_ARCHITECTURE, queue_architecture);
  query (amd_dbgapi_queue_get_info, queue, AMD_DBGAPI_QUEUE_INFO_OS_ID, s.os_queue);
  query (amd_dbgapi_queue_get_info, queue, AMD_DBGAPI_QUEUE_INFO_TYPE, s.queue_type);
  query (amd_dbgapi_queue_get_info, queue, AMD_DBGAPI_QUEUE_INFO_STATE, s.queue_state);
  query (amd_dbgapi_queue_get_info, queue, AMD_DBGAPI_QUEUE_INFO_ERROR_REASON, queue_error);
  join (queue_process == process && queue_agent == agent
                  && queue_architecture == architecture && queue_error == 0);

  amd_dbgapi_process_id_t agent_process {};
  amd_dbgapi_architecture_id_t agent_architecture {};
  std::uint32_t agent_state = 0;
  query (amd_dbgapi_agent_get_info, agent, AMD_DBGAPI_AGENT_INFO_PROCESS, agent_process);
  query (amd_dbgapi_agent_get_info, agent, AMD_DBGAPI_AGENT_INFO_ARCHITECTURE, agent_architecture);
  query (amd_dbgapi_agent_get_info, agent, AMD_DBGAPI_AGENT_INFO_OS_ID, s.os_agent);
  query (amd_dbgapi_agent_get_info, agent, AMD_DBGAPI_AGENT_INFO_STATE, agent_state);
  query (amd_dbgapi_architecture_get_info, architecture,
                    AMD_DBGAPI_ARCHITECTURE_INFO_ELF_AMDGPU_MACHINE, s.elf_machine);
  join (agent_process == process && agent_architecture == architecture
                  && agent_state == AMD_DBGAPI_AGENT_STATE_SUPPORTED);
  join (stopped_obs::closed_profile (s));
  const checkpoint_view cp (m_owner.m_checkpoint.bytes);
  amd_dbgapi_global_address_t queue_address=0, kernarg=0, descriptor=0, entry=0;
  amd_dbgapi_global_address_t completion=0, pc=0;
  amd_dbgapi_size_t queue_bytes=0;
  std::uint32_t barrier=0, acquire=0, release=0;
  query (amd_dbgapi_queue_get_info,queue,AMD_DBGAPI_QUEUE_INFO_ADDRESS,queue_address);
  query (amd_dbgapi_queue_get_info,queue,AMD_DBGAPI_QUEUE_INFO_SIZE,queue_bytes);
  query (amd_dbgapi_dispatch_get_info,dispatch,AMD_DBGAPI_DISPATCH_INFO_BARRIER,barrier);
  query (amd_dbgapi_dispatch_get_info,dispatch,AMD_DBGAPI_DISPATCH_INFO_ACQUIRE_FENCE,acquire);
  query (amd_dbgapi_dispatch_get_info,dispatch,AMD_DBGAPI_DISPATCH_INFO_RELEASE_FENCE,release);
  query (amd_dbgapi_dispatch_get_info,dispatch,AMD_DBGAPI_DISPATCH_INFO_KERNEL_ARGUMENT_SEGMENT_ADDRESS,kernarg);
  query (amd_dbgapi_dispatch_get_info,dispatch,AMD_DBGAPI_DISPATCH_INFO_KERNEL_DESCRIPTOR_ADDRESS,descriptor);
  query (amd_dbgapi_dispatch_get_info,dispatch,AMD_DBGAPI_DISPATCH_INFO_KERNEL_CODE_ENTRY_ADDRESS,entry);
  query (amd_dbgapi_dispatch_get_info,dispatch,AMD_DBGAPI_DISPATCH_INFO_KERNEL_COMPLETION_ADDRESS,completion);
  query (amd_dbgapi_wave_get_info,wave,AMD_DBGAPI_WAVE_INFO_PC,pc);
  join (queue.handle==m_owner.m_checkpoint.queue && s.os_queue==cp.u32 (20)
        && s.os_agent==cp.u32 (24) && s.packet==0
        && queue_address==cp.u64 (88) && queue_bytes==cp.u64 (96)
        && barrier==AMD_DBGAPI_DISPATCH_BARRIER_NONE
        && acquire==AMD_DBGAPI_DISPATCH_FENCE_SCOPE_SYSTEM
        && release==AMD_DBGAPI_DISPATCH_FENCE_SCOPE_SYSTEM
        && kernarg==cp.u64 (144) && descriptor==cp.u64 (72)
        && entry==cp.u64 (80) && entry<=UINT64_MAX-80 && pc==entry+80
        && completion==cp.u64 (128)); // API17 is the actual signal BASE, never VALUE+8.
  // End fence, independent of the initial state/stop-reason query.
  std::uint32_t final_state = 0, final_reason = 0;
  query (amd_dbgapi_wave_get_info, wave, AMD_DBGAPI_WAVE_INFO_STATE, final_state);
  query (amd_dbgapi_wave_get_info, wave, AMD_DBGAPI_WAVE_INFO_STOP_REASON, final_reason);
  join (final_state == s.state && final_reason == s.stop_reason
                  && info.inf->process_target ()->find_thread (thread->ptid) == thread);
  return {static_cast<std::uint64_t> (thread->global_num),wave.handle,workgroup.handle,
          dispatch.handle,queue.handle,agent.handle,architecture.handle,pc,completion};
}

void native_adapter::sole_queue (const checkpoint_view &cp,bool recheck) {
  auto &i=info ();
  const auto queue=list_one<amd_dbgapi_queue_id_t>
    ([&i] (std::size_t *count,amd_dbgapi_queue_id_t **rows) {
      return amd_dbgapi_process_queue_list (i.process_id,count,rows,nullptr);
    });
  amd_dbgapi_process_id_t process {},agent_process {};
  amd_dbgapi_agent_id_t agent {};
  amd_dbgapi_architecture_id_t architecture {},agent_architecture {};
  amd_dbgapi_global_address_t address=0;
  amd_dbgapi_size_t bytes=0;
  amd_dbgapi_os_queue_id_t os_queue=0;
  amd_dbgapi_os_agent_id_t os_agent=0;
  std::uint32_t type=0,state=0,error=0,agent_state=0,machine=0;
  query (amd_dbgapi_queue_get_info,queue,AMD_DBGAPI_QUEUE_INFO_PROCESS,process);
  query (amd_dbgapi_queue_get_info,queue,AMD_DBGAPI_QUEUE_INFO_AGENT,agent);
  query (amd_dbgapi_queue_get_info,queue,AMD_DBGAPI_QUEUE_INFO_ARCHITECTURE,architecture);
  query (amd_dbgapi_queue_get_info,queue,AMD_DBGAPI_QUEUE_INFO_OS_ID,os_queue);
  query (amd_dbgapi_queue_get_info,queue,AMD_DBGAPI_QUEUE_INFO_TYPE,type);
  query (amd_dbgapi_queue_get_info,queue,AMD_DBGAPI_QUEUE_INFO_STATE,state);
  query (amd_dbgapi_queue_get_info,queue,AMD_DBGAPI_QUEUE_INFO_ERROR_REASON,error);
  query (amd_dbgapi_queue_get_info,queue,AMD_DBGAPI_QUEUE_INFO_ADDRESS,address);
  query (amd_dbgapi_queue_get_info,queue,AMD_DBGAPI_QUEUE_INFO_SIZE,bytes);
  query (amd_dbgapi_agent_get_info,agent,AMD_DBGAPI_AGENT_INFO_PROCESS,agent_process);
  query (amd_dbgapi_agent_get_info,agent,AMD_DBGAPI_AGENT_INFO_ARCHITECTURE,agent_architecture);
  query (amd_dbgapi_agent_get_info,agent,AMD_DBGAPI_AGENT_INFO_OS_ID,os_agent);
  query (amd_dbgapi_agent_get_info,agent,AMD_DBGAPI_AGENT_INFO_STATE,agent_state);
  query (amd_dbgapi_architecture_get_info,architecture,AMD_DBGAPI_ARCHITECTURE_INFO_ELF_AMDGPU_MACHINE,machine);
  require (process==i.process_id && agent_process==process && agent_architecture==architecture
           && agent.handle!=0 && architecture.handle!=0
           && type==AMD_DBGAPI_OS_QUEUE_TYPE_HSA_AQL
           && state==AMD_DBGAPI_QUEUE_STATE_VALID && error==0
           && agent_state==AMD_DBGAPI_AGENT_STATE_SUPPORTED && machine==0x4f
           && os_queue==cp.u32 (20) && os_agent==cp.u32 (24)
           && address==cp.u64 (88) && bytes==cp.u64 (96),failure::checkpoint_changed);
  if (recheck) require (queue.handle==m_owner.m_checkpoint.queue,failure::checkpoint_changed);
  m_candidate.queue=queue.handle; // Actual returned handle, never cast from OS ID.
}
}

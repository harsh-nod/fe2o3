/* SPDX-License-Identifier: GPL-3.0-or-later
   This file is part of GDB; GPL version 3 or (at your option) any later version.
   Included after the actual GPU-thread helpers. No polling or new AMD client. */

struct stopped_wave_candidate_v1 {
  std::uint64_t thread = 0;
  stopped_obs::stop_identity identity {};
};

template<typename Get, typename Id, typename Query, typename Value>
static void
stopped_query_v1 (Get get, Id id, Query query, Value &value)
{
  const auto status = get (id, query, sizeof (value), &value);
  if (status != AMD_DBGAPI_STATUS_SUCCESS) {
    stopped_obs::invalidate
      (status == AMD_DBGAPI_STATUS_ERROR_NOT_AVAILABLE
         ? stopped_obs::reason::required_identity_unavailable
         : stopped_obs::reason::query_failed, status);
    error (_("stopped-wave observation query failed (%s)"),
           get_status_string (status));
  }
}
static void
stopped_join_v1 (bool equal)
{
  if (!equal) {
    stopped_obs::invalidate (stopped_obs::reason::wave_identity);
    error (_("stopped-wave observation native identity differs"));
  }
}

/* Every returned field is queried on this same real library process. The
   boolean equality helper above is a local assertion, NOT runtime evidence
   accepted from any caller/DTO or an execution permission. */
static stopped_wave_candidate_v1
query_stopped_wave_v1 (amd_dbgapi_inferior_info &info, thread_info *thread)
{
  stopped_obs::check_owner (info);
  stopped_join_v1 (!stopped_wave_observation.invalid () && info.inf != nullptr
                  && thread != nullptr && thread->inf == info.inf
                  && info.inf->pspace->cbfd == nullptr
                  && info.inf->target_at (arch_stratum) == &the_amd_dbgapi_target
                  && ptid_is_gpu (thread->ptid)
                  && thread->ptid.pid () == info.inf->pid
                  && static_cast<std::uint64_t> (info.inf->pid)
                       == stopped_wave_observation.pid ()
                  && info.process_id.handle == stopped_wave_observation.process ()
                  && thread->global_num > 0);

  const auto wave = get_amd_dbgapi_wave_id (thread->ptid);
  stopped_wave_candidate_v1 result;
  result.thread = thread->global_num;
  auto &s = result.identity;
  s.wave = wave.handle;
  amd_dbgapi_process_id_t process {};
  amd_dbgapi_workgroup_id_t workgroup {};
  amd_dbgapi_dispatch_id_t dispatch {};
  amd_dbgapi_queue_id_t queue {};
  amd_dbgapi_agent_id_t agent {};
  amd_dbgapi_architecture_id_t architecture {};
  std::size_t lanes = 0;
  stopped_query_v1 (amd_dbgapi_wave_get_info, wave, AMD_DBGAPI_WAVE_INFO_STATE, s.state);
  stopped_join_v1 (s.state == AMD_DBGAPI_WAVE_STATE_STOP);
  stopped_query_v1 (amd_dbgapi_wave_get_info, wave, AMD_DBGAPI_WAVE_INFO_STOP_REASON, s.stop_reason);
  stopped_query_v1 (amd_dbgapi_wave_get_info, wave, AMD_DBGAPI_WAVE_INFO_PROCESS, process);
  stopped_query_v1 (amd_dbgapi_wave_get_info, wave, AMD_DBGAPI_WAVE_INFO_WORKGROUP, workgroup);
  stopped_query_v1 (amd_dbgapi_wave_get_info, wave, AMD_DBGAPI_WAVE_INFO_DISPATCH, dispatch);
  stopped_query_v1 (amd_dbgapi_wave_get_info, wave, AMD_DBGAPI_WAVE_INFO_QUEUE, queue);
  stopped_query_v1 (amd_dbgapi_wave_get_info, wave, AMD_DBGAPI_WAVE_INFO_AGENT, agent);
  stopped_query_v1 (amd_dbgapi_wave_get_info, wave, AMD_DBGAPI_WAVE_INFO_ARCHITECTURE, architecture);
  stopped_query_v1 (amd_dbgapi_wave_get_info, wave, AMD_DBGAPI_WAVE_INFO_LANE_COUNT, lanes);
  stopped_query_v1 (amd_dbgapi_wave_get_info, wave, AMD_DBGAPI_WAVE_INFO_WORKGROUP_COORD, s.coordinate);
  stopped_query_v1 (amd_dbgapi_wave_get_info, wave, AMD_DBGAPI_WAVE_INFO_WAVE_NUMBER_IN_WORKGROUP, s.wave_index);
  stopped_join_v1 (process == info.process_id && lanes == 64);
  s.lanes = static_cast<std::uint32_t> (lanes);
  s.workgroup = workgroup.handle; s.dispatch = dispatch.handle;
  s.queue = queue.handle; s.agent = agent.handle; s.architecture = architecture.handle;

  amd_dbgapi_process_id_t group_process {};
  amd_dbgapi_queue_id_t group_queue {};
  amd_dbgapi_dispatch_id_t group_dispatch {};
  amd_dbgapi_agent_id_t group_agent {};
  amd_dbgapi_architecture_id_t group_architecture {};
  std::array<std::uint32_t, 3> group_coordinate {};
  stopped_query_v1 (amd_dbgapi_workgroup_get_info, workgroup, AMD_DBGAPI_WORKGROUP_INFO_PROCESS, group_process);
  stopped_query_v1 (amd_dbgapi_workgroup_get_info, workgroup, AMD_DBGAPI_WORKGROUP_INFO_QUEUE, group_queue);
  stopped_query_v1 (amd_dbgapi_workgroup_get_info, workgroup, AMD_DBGAPI_WORKGROUP_INFO_DISPATCH, group_dispatch);
  stopped_query_v1 (amd_dbgapi_workgroup_get_info, workgroup, AMD_DBGAPI_WORKGROUP_INFO_AGENT, group_agent);
  stopped_query_v1 (amd_dbgapi_workgroup_get_info, workgroup, AMD_DBGAPI_WORKGROUP_INFO_ARCHITECTURE, group_architecture);
  stopped_query_v1 (amd_dbgapi_workgroup_get_info, workgroup, AMD_DBGAPI_WORKGROUP_INFO_WORKGROUP_COORD, group_coordinate);
  stopped_join_v1 (group_process == process && group_queue == queue && group_dispatch == dispatch
                  && group_agent == agent && group_architecture == architecture
                  && group_coordinate == s.coordinate);

  amd_dbgapi_process_id_t dispatch_process {};
  amd_dbgapi_queue_id_t dispatch_queue {};
  amd_dbgapi_agent_id_t dispatch_agent {};
  amd_dbgapi_architecture_id_t dispatch_architecture {};
  std::array<std::uint16_t, 3> native_workgroup {};
  std::uint32_t dimensions = 0;
  stopped_query_v1 (amd_dbgapi_dispatch_get_info, dispatch, AMD_DBGAPI_DISPATCH_INFO_PROCESS, dispatch_process);
  stopped_query_v1 (amd_dbgapi_dispatch_get_info, dispatch, AMD_DBGAPI_DISPATCH_INFO_QUEUE, dispatch_queue);
  stopped_query_v1 (amd_dbgapi_dispatch_get_info, dispatch, AMD_DBGAPI_DISPATCH_INFO_AGENT, dispatch_agent);
  stopped_query_v1 (amd_dbgapi_dispatch_get_info, dispatch, AMD_DBGAPI_DISPATCH_INFO_ARCHITECTURE, dispatch_architecture);
  stopped_query_v1 (amd_dbgapi_dispatch_get_info, dispatch, AMD_DBGAPI_DISPATCH_INFO_OS_QUEUE_PACKET_ID, s.packet);
  stopped_query_v1 (amd_dbgapi_dispatch_get_info, dispatch, AMD_DBGAPI_DISPATCH_INFO_GRID_DIMENSIONS, dimensions);
  stopped_query_v1 (amd_dbgapi_dispatch_get_info, dispatch, AMD_DBGAPI_DISPATCH_INFO_GRID_SIZES, s.grid);
  stopped_query_v1 (amd_dbgapi_dispatch_get_info, dispatch, AMD_DBGAPI_DISPATCH_INFO_WORKGROUP_SIZES, native_workgroup);
  stopped_query_v1 (amd_dbgapi_dispatch_get_info, dispatch, AMD_DBGAPI_DISPATCH_INFO_PRIVATE_SEGMENT_SIZE, s.private_bytes);
  stopped_query_v1 (amd_dbgapi_dispatch_get_info, dispatch, AMD_DBGAPI_DISPATCH_INFO_GROUP_SEGMENT_SIZE, s.group_bytes);
  stopped_join_v1 (dispatch_process == process && dispatch_queue == queue
                  && dispatch_agent == agent && dispatch_architecture == architecture
                  && dimensions == 1);
  for (std::size_t i = 0; i < 3; ++i) s.workgroup_size[i] = native_workgroup[i];

  amd_dbgapi_process_id_t queue_process {};
  amd_dbgapi_agent_id_t queue_agent {};
  amd_dbgapi_architecture_id_t queue_architecture {};
  std::uint32_t queue_error = 0;
  stopped_query_v1 (amd_dbgapi_queue_get_info, queue, AMD_DBGAPI_QUEUE_INFO_PROCESS, queue_process);
  stopped_query_v1 (amd_dbgapi_queue_get_info, queue, AMD_DBGAPI_QUEUE_INFO_AGENT, queue_agent);
  stopped_query_v1 (amd_dbgapi_queue_get_info, queue, AMD_DBGAPI_QUEUE_INFO_ARCHITECTURE, queue_architecture);
  stopped_query_v1 (amd_dbgapi_queue_get_info, queue, AMD_DBGAPI_QUEUE_INFO_OS_ID, s.os_queue);
  stopped_query_v1 (amd_dbgapi_queue_get_info, queue, AMD_DBGAPI_QUEUE_INFO_TYPE, s.queue_type);
  stopped_query_v1 (amd_dbgapi_queue_get_info, queue, AMD_DBGAPI_QUEUE_INFO_STATE, s.queue_state);
  stopped_query_v1 (amd_dbgapi_queue_get_info, queue, AMD_DBGAPI_QUEUE_INFO_ERROR_REASON, queue_error);
  stopped_join_v1 (queue_process == process && queue_agent == agent
                  && queue_architecture == architecture && queue_error == 0);

  amd_dbgapi_process_id_t agent_process {};
  amd_dbgapi_architecture_id_t agent_architecture {};
  std::uint32_t agent_state = 0;
  stopped_query_v1 (amd_dbgapi_agent_get_info, agent, AMD_DBGAPI_AGENT_INFO_PROCESS, agent_process);
  stopped_query_v1 (amd_dbgapi_agent_get_info, agent, AMD_DBGAPI_AGENT_INFO_ARCHITECTURE, agent_architecture);
  stopped_query_v1 (amd_dbgapi_agent_get_info, agent, AMD_DBGAPI_AGENT_INFO_OS_ID, s.os_agent);
  stopped_query_v1 (amd_dbgapi_agent_get_info, agent, AMD_DBGAPI_AGENT_INFO_STATE, agent_state);
  stopped_query_v1 (amd_dbgapi_architecture_get_info, architecture,
                    AMD_DBGAPI_ARCHITECTURE_INFO_ELF_AMDGPU_MACHINE, s.elf_machine);
  stopped_join_v1 (agent_process == process && agent_architecture == architecture
                  && agent_state == AMD_DBGAPI_AGENT_STATE_SUPPORTED);
  if (!stopped_obs::closed_profile (s)) {
    stopped_obs::invalidate (stopped_obs::reason::unsupported_profile);
    error (_("stopped-wave observation does not match the closed gfx950 profile"));
  }
  // End fence, independent of the initial state/stop-reason query.
  std::uint32_t final_state = 0, final_reason = 0;
  stopped_query_v1 (amd_dbgapi_wave_get_info, wave, AMD_DBGAPI_WAVE_INFO_STATE, final_state);
  stopped_query_v1 (amd_dbgapi_wave_get_info, wave, AMD_DBGAPI_WAVE_INFO_STOP_REASON, final_reason);
  stopped_join_v1 (final_state == s.state && final_reason == s.stop_reason
                  && info.inf->process_target ()->find_thread (thread->ptid) == thread);
  return result;
}

static stopped_wave_candidate_v1
query_stopped_wave_event_v1 (amd_dbgapi_inferior_info &info, amd_dbgapi_event_id_t event)
{
  amd_dbgapi_wave_id_t wave {};
  stopped_query_v1 (amd_dbgapi_event_get_info, event, AMD_DBGAPI_EVENT_INFO_WAVE, wave);
  const auto ptid = make_gpu_ptid (info.inf->pid, wave);
  auto *thread = info.inf->process_target ()->find_thread (ptid);
  auto result = query_stopped_wave_v1 (info, thread);
  stopped_join_v1 (result.identity.wave == wave.handle);
  return result;
}

void
stopped_obs::after_normal_stop (thread_info *thread)
{
  if (stopped_wave_observation.invalid ()) { flush_safe_point (); return; }
  if (!stopped_wave_observation.pending ()) {
    stopped_wave_observation.change (reason::stop_changed);
    flush_safe_point ();
    return;
  }
  try {
    stopped_join_v1 (thread != nullptr && thread->state == THREAD_STOPPED
                    && !thread->executing () && !thread->resumed ()
                    && thread == inferior_thread ()
                    && thread->ptid == inferior_ptid
                    && static_cast<std::uint64_t> (thread->global_num)
                         == stopped_wave_observation.candidate ().thread);
    auto *info = get_amd_dbgapi_inferior_info (thread->inf);
    auto observed = query_stopped_wave_v1 (*info, thread);
    stopped_wave_observation.publish_normal_stop (observed.thread, observed.identity);
  } catch (...) {
    stopped_obs::invalidate (reason::handler_unwind);
    flush_safe_point ();
    throw;
  }
  // Output exceptions are not caught to attempt another immediate drain.
  flush_safe_point ();
}

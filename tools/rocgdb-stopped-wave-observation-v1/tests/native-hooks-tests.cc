/* SPDX-License-Identifier: GPL-3.0-or-later
   Root-run CPU fixture only; no AMD library or inferior is started. */
#include "native-fixture-v1.h"
static unsigned controls;
static void require(bool value){if(!value)std::abort();}
template<class F>static void test(const char*name,F run)
{reset_fixture();run();++controls;std::printf("PASS %s\n",name);}
template<class F>static void refuses(F run)
{bool threw=false;try{run();}catch(const std::runtime_error&){threw=true;}require(threw);}
static stopped_obs::reason terminal()
{require(stopped_wave_observation.invalid());return stopped_wave_observation.at(stopped_wave_observation.count()-1).invalidation;}
int main()
{
  test("actual-hook-stage-needs-normal-stop",[]{
    stage_actual_hooks();require(acknowledgements==1&&stopped_wave_observation.pending());
    require(stopped_wave_observation.count()==2&&emitted.empty());
  });
  test("actual-hook-publish-after-requery",[]{
    stage_actual_hooks();const auto before=queries;stopped_obs::after_normal_stop(&thread);
    require(queries>before&&acknowledgements==1&&emitted.size()==3
      &&emitted.back().type==stopped_obs::kind::wave_stop_complete);
  });
  test("repeated-flush-no-replay",[]{
    stage_actual_hooks();stopped_obs::after_normal_stop(&thread);auto count=emitted.size();
    stopped_obs::flush_safe_point();stopped_obs::flush_safe_point();require(emitted.size()==count);
  });
  test("changed-workgroup-dispatch-refused",[]{
    changed_dispatch=true;refuses([]{stage_actual_hooks();});
    require(terminal()==stopped_obs::reason::wave_identity&&acknowledgements==1);
  });
  test("foreign-process-refused",[]{
    foreign_process=true;refuses([]{stage_actual_hooks();});
    require(terminal()==stopped_obs::reason::event_identity&&acknowledgements==1);
  });
  test("end-query-state-fence-refused",[]{
    changed_final_state=true;refuses([]{stage_actual_hooks();});
    require(terminal()==stopped_obs::reason::wave_identity&&acknowledgements==1);
  });
  test("required-initial-identity-unavailable",[]{
    fail_at=4;not_available=true;refuses([]{stage_actual_hooks();});
    require(terminal()==stopped_obs::reason::required_identity_unavailable&&acknowledgements==1);
  });
  test("ack-failure-never-stages",[]{
    ack_status=AMD_DBGAPI_STATUS_ERROR;stage_actual_hooks();
    require(terminal()==stopped_obs::reason::acknowledgement&&acknowledgements==1
      &&!stopped_wave_observation.pending());
  });
  test("null-normal-stop-refused",[]{
    stage_actual_hooks();refuses([]{stopped_obs::after_normal_stop(nullptr);});
    require(terminal()==stopped_obs::reason::wave_identity&&acknowledgements==1);
  });
  test("running-thread-refused",[]{
    stage_actual_hooks();thread.execution=true;refuses([]{stopped_obs::after_normal_stop(&thread);});
    require(terminal()==stopped_obs::reason::wave_identity);
  });
  test("resumed-thread-refused",[]{
    stage_actual_hooks();thread.resumption=true;refuses([]{stopped_obs::after_normal_stop(&thread);});
    require(terminal()==stopped_obs::reason::wave_identity);
  });
  test("host-selection-refused",[]{
    stage_actual_hooks();inferior_ptid.wave=0;refuses([]{stopped_obs::after_normal_stop(&thread);});
    require(terminal()==stopped_obs::reason::wave_identity);
  });
  test("different-current-thread-refused",[]{
    stage_actual_hooks();selected=nullptr;refuses([]{stopped_obs::after_normal_stop(&thread);});
    require(terminal()==stopped_obs::reason::wave_identity);
  });
  test("missing-native-thread-refused",[]{
    inf.target.thread=nullptr;refuses([]{stage_actual_hooks();});
    require(terminal()==stopped_obs::reason::wave_identity&&acknowledgements==1);
  });
  test("core-owner-refused",[]{
    stopped_wave_observation=stopped_obs::ledger{};space.cbfd=&space;stopped_obs::bind(info);
    require(terminal()==stopped_obs::reason::unsupported_profile);
  });
  test("negative-pid-refused-before-cast",[]{
    stopped_wave_observation=stopped_obs::ledger{};inf.pid=-1;stopped_obs::bind(info);
    require(terminal()==stopped_obs::reason::unsupported_profile);
  });
  test("after-stop-next-command-terminal",[]{
    stage_actual_hooks();stopped_obs::after_normal_stop(&thread);
    stopped_obs::change(stopped_obs::reason::stop_changed);stopped_obs::flush_safe_point();
    require(terminal()==stopped_obs::reason::stop_changed
      &&emitted.back().type==stopped_obs::kind::invalidated);
  });
  test("output-failure-sticky-and-consumed",[]{
    stage_actual_hooks();output_fails=true;refuses([]{stopped_obs::after_normal_stop(&thread);});
    require(terminal()==stopped_obs::reason::output_failure);
    output_fails=false;stopped_obs::flush_safe_point();
    require(!emitted.empty()&&emitted.front().sequence>1
      &&emitted.back().type==stopped_obs::kind::invalidated);
  });
  test("old-noqueue-wave-refusal-independent",[]{
    runtime_observation.invalidate(runtime_obs::reason::unsupported_event);
    stage_actual_hooks();stopped_obs::after_normal_stop(&thread);
    require(runtime_observation.invalid()&&!stopped_wave_observation.invalid()
      &&emitted.back().type==stopped_obs::kind::wave_stop_complete);
  });
  test("callback-hooks-remain-joined",[]{
    stopped_obs::begin_callback(40,9);stopped_obs::end_callback(0,0,1,0);
    require(!stopped_wave_observation.invalid()&&stopped_wave_observation.at(2).callback==1);
  });
  test("shared-invalidations-broadcast",[]{
    invalidate_runtime_observation(runtime_obs::reason::inferior_exit);
    require(runtime_observation.invalid()&&terminal()==stopped_obs::reason::inferior_exit);
  });
  reset_fixture();stage_actual_hooks();const auto initial_queries=queries;
  stopped_obs::after_normal_stop(&thread);const auto total_queries=queries;
  require(initial_queries==44&&total_queries==85);
  for(unsigned i=1;i<=initial_queries;++i)test("each-query-failure-ack-once",[i]{
    fail_at=i;refuses([]{stage_actual_hooks();});
    require(stopped_wave_observation.invalid()&&acknowledgements==1
      &&!stopped_wave_observation.pending());
  });
  for(unsigned i=initial_queries+1;i<=total_queries;++i)test("each-requery-failure-no-row",[i]{
    stage_actual_hooks();fail_at=i;not_available=true;
    refuses([]{stopped_obs::after_normal_stop(&thread);});
    require(terminal()==stopped_obs::reason::required_identity_unavailable&&acknowledgements==1);
    for(const auto&r:emitted)require(r.type!=stopped_obs::kind::wave_stop_complete);
  });
  std::printf("native hook controls=%u initial-queries=%u requery=%u\n",
    controls,initial_queries,total_queries-initial_queries);
}

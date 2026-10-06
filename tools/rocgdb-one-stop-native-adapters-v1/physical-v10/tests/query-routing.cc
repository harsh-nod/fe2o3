#include <cassert>
#include <cstddef>
#include <cstdint>
#include <optional>
#include "amd-dbgapi-stopped-wave-observation-v1.h"
#include "queue-health-v1.h"
namespace stopped_obs = amd_stopped_wave_observation_v1;
static stopped_obs::ledger stopped_wave_observation;
static amd::dbgapi::detail::queue_health_v1 health;
static bool owned_selected=false;
static unsigned calls=0;
namespace amd_owned_one_stop_native_v1 { bool selected(){return owned_selected;} }
void stopped_obs::invalidate(reason r,std::int64_t status) noexcept {
  if (stopped_wave_observation.bound())
    stopped_wave_observation.invalidate(r,status);
}
constexpr unsigned AMD_DBGAPI_EVENT_KIND_WAVE_STOP=1;
struct amd_dbgapi_inferior_info {};
struct stopped_wave_candidate_v1 {};
static stopped_wave_candidate_v1 query_stopped_wave_event_v1(amd_dbgapi_inferior_info&,unsigned){
  ++calls; assert(health.take_query()); assert(health.take_query()); return {};
}
static bool route(unsigned event_kind){
  amd_dbgapi_inferior_info info; unsigned event_id=1;
#include "routing-under-test.inc"
  return observed.has_value();
}
static void reset(bool selected){
  stopped_wave_observation={}; health={}; calls=0;owned_selected=selected;
  assert(stopped_wave_observation.bind(1,1,1234,1));
  assert(health.begin_baseline());health.finish_baseline(true);
}
static void pair(){assert(health.take_query());assert(health.take_query());}
int main(){
  reset(false);assert(route(1));assert(calls==1&&health.queries()==2);
  assert(!stopped_wave_observation.invalid());
  reset(false);assert(!route(2));assert(calls==0&&health.queries()==0);
  reset(true);assert(!route(1));assert(calls==0&&health.queries()==0);
  assert(stopped_wave_observation.invalid()&&stopped_wave_observation.count()==1);
  assert(stopped_wave_observation.at(0).invalidation==stopped_obs::reason::unsupported_profile);
  reset(true);assert(!route(2));assert(!stopped_wave_observation.invalid()&&calls==0);
  reset(true);stopped_wave_observation.invalidate(stopped_obs::reason::query_failed,-3);
  assert(!route(1));assert(stopped_wave_observation.count()==1);
  assert(stopped_wave_observation.at(0).invalidation==stopped_obs::reason::query_failed);
  assert(stopped_wave_observation.at(0).status==-3);
  reset(true);assert(!route(1));assert(!route(1));
  assert(calls==0&&stopped_wave_observation.count()==1);
  reset(false);stopped_wave_observation.invalidate(stopped_obs::reason::query_failed,-3);
  assert(!route(1)&&calls==0);
  // Prior schedule: two checkpoint pairs, legacy pair, two owned stop pairs.
  reset(false);pair();pair();assert(route(1));pair();pair();
  assert(health.queries()==10);assert(!health.take_query());assert(!health.candidate());
  // Successor preserves all five owned pairs and the exact original cap.
  reset(true);pair();pair();assert(!route(1));pair();pair();pair();
  assert(health.queries()==10&&health.candidate());
  assert(!health.take_query());assert(!health.candidate());
  reset(true);pair();health.taint();assert(!route(1));assert(!health.take_query());
  static_assert(decltype(health)::query_limit==10,"original lifetime cap");
  static_assert(sizeof(health)==5,"no provider storage change");
}

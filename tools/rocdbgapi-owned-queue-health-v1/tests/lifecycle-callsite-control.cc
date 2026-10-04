// CPU-only exact changed lifecycle call-site controls, not full process_t.
// No process, native API, ioctl or GPU. Helpers and driver boundary are mocked.
#include <cstdint>
#include <cstddef>
#include <cstdio>
#include <type_traits>
#include <utility>
#include <initializer_list>
#include "amd-dbgapi.h"
#include "queue-health-v1.h"
namespace amd::dbgapi {
[[noreturn]] inline void fatal_error(const char *,const char *) { throw 99; }
inline const char *to_cstring(amd_dbgapi_status_t) { return "mock-status"; }
enum class os_exception_code_t : uint32_t
{
  none = 0,

  /* per queue exceptions  */
  queue_wave_abort = 1,
  queue_wave_trap = 2,
  queue_wave_math_error = 3,
  queue_wave_illegal_instruction = 4,
  queue_wave_memory_violation = 5,
  queue_wave_address_error = 6,
  queue_packet_dispatch_dim_invalid = 16,
  queue_packet_dispatch_group_segment_size_invalid = 17,
  queue_packet_dispatch_code_invalid = 18,
  queue_packet_unsupported = 20,
  queue_packet_dispatch_work_group_size_invalid = 21,
  queue_packet_dispatch_register_invalid = 22,
  queue_packet_vendor_unsupported = 23,
  queue_preemption_error = 30,
  queue_new = 31,

  /* per device exceptions  */
  device_queue_delete = 32,
  device_memory_violation = 33,
  device_ras_error = 34,
  device_fatal_halt = 35,
  device_new = 36,

  /* per process exceptions  */
  process_runtime = 48,
  process_device_remove = 49,
};
enum class os_runtime_state_t : uint32_t
{
  disabled = 0,
  enabled = 1,
  enabled_busy = 2,
  enabled_error = 3,
};
struct os_runtime_info_t
{
  amd_dbgapi_global_address_t r_debug;
  os_runtime_state_t runtime_state;
  bool ttmp_setup;
};
constexpr auto
ec_mask (os_exception_code_t c)
{
  using underlying_t = typename std::underlying_type_t<os_exception_code_t>;
  return 1ull << (static_cast<underlying_t> (c) - 1);
}
enum class os_exception_mask_t : uint64_t
{
  none = 0,

  /* per queue exceptions  */
  queue_wave_abort = ec_mask (os_exception_code_t::queue_wave_abort),
  queue_wave_trap = ec_mask (os_exception_code_t::queue_wave_trap),
  queue_wave_math_error = ec_mask (os_exception_code_t::queue_wave_math_error),
  queue_wave_illegal_instruction
  = ec_mask (os_exception_code_t::queue_wave_illegal_instruction),
  queue_wave_memory_violation
  = ec_mask (os_exception_code_t::queue_wave_memory_violation),
  queue_wave_address_error
  = ec_mask (os_exception_code_t::queue_wave_address_error),
  queue_packet_dispatch_dim_invalid
  = ec_mask (os_exception_code_t::queue_packet_dispatch_dim_invalid),
  queue_packet_dispatch_group_segment_size_invalid = ec_mask (
    os_exception_code_t::queue_packet_dispatch_group_segment_size_invalid),
  queue_packet_dispatch_code_invalid
  = ec_mask (os_exception_code_t::queue_packet_dispatch_code_invalid),
  queue_packet_unsupported
  = ec_mask (os_exception_code_t::queue_packet_unsupported),
  queue_packet_dispatch_work_group_size_invalid = ec_mask (
    os_exception_code_t::queue_packet_dispatch_work_group_size_invalid),
  queue_packet_dispatch_register_invalid
  = ec_mask (os_exception_code_t::queue_packet_dispatch_register_invalid),
  queue_packet_vendor_unsupported
  = ec_mask (os_exception_code_t::queue_packet_vendor_unsupported),
  queue_preemption_error
  = ec_mask (os_exception_code_t::queue_preemption_error),
  queue_new = ec_mask (os_exception_code_t::queue_new),

  /* per device exceptions  */
  device_queue_delete = ec_mask (os_exception_code_t::device_queue_delete),
  device_memory_violation
  = ec_mask (os_exception_code_t::device_memory_violation),
  device_ras_error = ec_mask (os_exception_code_t::device_ras_error),
  device_fatal_halt = ec_mask (os_exception_code_t::device_fatal_halt),
  device_new = ec_mask (os_exception_code_t::device_new),

  /* per process exceptions  */
  process_runtime = ec_mask (os_exception_code_t::process_runtime),
  process_device_remove = ec_mask (os_exception_code_t::process_device_remove),
};
constexpr os_exception_mask_t operator|(os_exception_mask_t a,os_exception_mask_t b) {
  return static_cast<os_exception_mask_t>(uint64_t(a)|uint64_t(b));
}
constexpr os_exception_mask_t operator&(os_exception_mask_t a,os_exception_mask_t b) {
  return static_cast<os_exception_mask_t>(uint64_t(a)&uint64_t(b));
}
constexpr os_exception_mask_t operator~(os_exception_mask_t a) {
  return static_cast<os_exception_mask_t>(~uint64_t(a));
}
struct os_queue_snapshot_entry_t { uint64_t unused=0; };
namespace utils {
template<class F> struct mock_scope_exit {
  F callback;bool active=true;
  explicit mock_scope_exit(F f):callback(std::move(f)) {}
  ~mock_scope_exit() { if(active)callback(); }
  void dismiss() { active=false; }
};
template<class F> auto make_scope_exit(F f) { return mock_scope_exit<F>(std::move(f)); }
}
struct mock_process {
  detail::queue_health_v1 m_queue_health;
  enum class flag_t { runtime_enable_during_attach };
  bool attached=false,debug=true,throw_snapshot=false,throw_forward=false;
  bool healthy_at_disable=true;
  size_t returned_count=0,calls=0,forwards=0,subscriptions=0;
  size_t event_calls=0,event_writes=0;
  os_exception_mask_t event_output=os_exception_mask_t::none;
  amd_dbgapi_status_t result=AMD_DBGAPI_STATUS_SUCCESS;
  amd_dbgapi_runtime_state_t m_runtime_state=AMD_DBGAPI_RUNTIME_STATE_LOADED_SUCCESS;
  bool is_flag_set(flag_t)const { return attached; }
  mock_process &os_driver() { return *this; }
  bool is_debug_enabled()const { return debug; }
  void set_exceptions_reported(os_exception_mask_t) { ++subscriptions; }
  amd_dbgapi_status_t disable_debug() {
    healthy_at_disable=m_queue_health.candidate();debug=false;
    return AMD_DBGAPI_STATUS_SUCCESS;
  }
  amd_dbgapi_status_t queue_health_snapshot_v1(os_queue_snapshot_entry_t *,size_t *count) {
    ++calls;
    if(throw_snapshot)throw 37;
    if(result!=AMD_DBGAPI_STATUS_SUCCESS)return result;
    *count=returned_count;return result;
  }
  void transition(os_runtime_info_t runtime_info) {
  // Lose the positive proof before any callback/subscription/lifecycle effect.
  if (runtime_info.runtime_state != os_runtime_state_t::enabled
      || m_queue_health.candidate ())
    m_queue_health.taint ();

  }
  void baseline() {
  auto restriction_error = utils::make_scope_exit (
    [this] ()
    {
      m_queue_health.taint ();
      os_driver ().set_exceptions_reported (
        os_exception_mask_t::process_runtime);
      m_runtime_state = AMD_DBGAPI_RUNTIME_STATE_LOADED_ERROR_RESTRICTION;
    });
  // Qualify once, after continuous exception subscriptions were installed and
  // before the runtime enable event is ACKed. Attached-existing and core paths
  // cannot acquire this positive proof. A second lifetime cannot reset taint.
  if (!is_flag_set (flag_t::runtime_enable_during_attach)
      && m_queue_health.begin_baseline ())
    {
      os_queue_snapshot_entry_t observed{};
      size_t observed_count = 0;
      const auto observed_status = os_driver ().queue_health_snapshot_v1 (
        &observed, &observed_count);
      m_queue_health.finish_baseline (
        observed_status == AMD_DBGAPI_STATUS_SUCCESS && observed_count == 0
        && os_driver ().is_debug_enabled ());
    }
  else
    m_queue_health.taint ();

  restriction_error.dismiss();
  }
  os_exception_mask_t suspension_clear() {
    return
    m_queue_health.candidate () ? os_exception_mask_t::none
      : os_exception_mask_t::queue_wave_abort
          | os_exception_mask_t::queue_wave_trap
          | os_exception_mask_t::queue_wave_math_error
          | os_exception_mask_t::queue_wave_illegal_instruction
          | os_exception_mask_t::queue_wave_memory_violation
          | os_exception_mask_t::queue_wave_address_error
    ;
  }
  amd_dbgapi_status_t mock_query_debug_event(amd_dbgapi_status_t status,
      os_exception_mask_t supplied,os_exception_mask_t *out) {
    ++event_calls;
    if(status==AMD_DBGAPI_STATUS_SUCCESS) {
      *out=supplied;++event_writes;
    }
    event_output=*out; // Caller-owned valid sentinel is unchanged on failure.
    return status;
  }
  bool observe(amd_dbgapi_status_t status,os_exception_mask_t supplied) {
    // Model the driver's no-output-on-failure contract with a valid nonzero
    // caller-owned sentinel. Exact copied guards below remain status-first.
    // Do not require optimizer path analysis of an indeterminate mock local.
    os_exception_mask_t exceptions=os_exception_mask_t::device_fatal_halt;
    status=mock_query_debug_event(status,supplied,&exceptions);
      if (status != AMD_DBGAPI_STATUS_SUCCESS
          && status != AMD_DBGAPI_STATUS_ERROR_PROCESS_EXITED)
        fatal_error ("os_driver_t::query_debug_event failed (%s)",
                     to_cstring (status));

      // The driver returned the exact mask that was observed before clearing.
      // A trap is not called benign: subscribed wave exceptions cannot reach
      // the runtime without send_exceptions(), which taints before forwarding.
      if (status != AMD_DBGAPI_STATUS_SUCCESS
          || (exceptions & ~(os_exception_mask_t::queue_wave_trap
                             | os_exception_mask_t::queue_new
                             | os_exception_mask_t::process_runtime))
               != os_exception_mask_t::none)
        m_queue_health.taint ();

      if (status == AMD_DBGAPI_STATUS_ERROR_PROCESS_EXITED
          || exceptions == os_exception_mask_t::none)
      return true;
    return false;
  }
  void forward(os_exception_mask_t exceptions) {
  // Sticky BEFORE any effect: failure/throwing handoffs cannot retry health.
  if ((exceptions & ~os_exception_mask_t::process_runtime)
      != os_exception_mask_t::none)
    m_queue_health.taint ();

  ++forwards;
  if(throw_forward)throw 73;
  }
  void disable() {
      m_queue_health.taint ();
      amd_dbgapi_status_t status = os_driver ().disable_debug ();
    (void)status;
  }
};
}
using namespace amd::dbgapi;
static unsigned assertions=0;
#define CHECK(x) do { if(!(x))return __LINE__; ++assertions; } while(false)
static mock_process qualified() { mock_process p;p.baseline();return p; }
int main() {
  auto q=qualified();
  CHECK(q.m_queue_health.candidate() && q.calls==1);
  q.baseline();
  CHECK(!q.m_queue_health.candidate() && q.calls==1);
  mock_process attached;attached.attached=true;attached.baseline();
  CHECK(!attached.m_queue_health.candidate() && attached.calls==0);
  for(size_t count:{1u,2u}) {
    mock_process p;p.returned_count=count;p.baseline();
    CHECK(!p.m_queue_health.candidate() && p.calls==1);
    p.returned_count=0;p.baseline();
    CHECK(!p.m_queue_health.candidate() && p.calls==1);
  }
  for(auto status:{AMD_DBGAPI_STATUS_ERROR,AMD_DBGAPI_STATUS_ERROR_PROCESS_EXITED,
                   AMD_DBGAPI_STATUS_ERROR_NOT_IMPLEMENTED}) {
    mock_process p;p.result=status;p.baseline();
    CHECK(!p.m_queue_health.candidate() && p.calls==1);
  }
  mock_process disabled;disabled.debug=false;disabled.baseline();
  CHECK(!disabled.m_queue_health.candidate() && disabled.calls==1);
  mock_process thrown;thrown.throw_snapshot=true;
  bool threw=false;try{thrown.baseline();}catch(int x){threw=x==37;}
  CHECK(threw && !thrown.m_queue_health.candidate() && thrown.calls==1
        && thrown.subscriptions==1
        && thrown.m_runtime_state==AMD_DBGAPI_RUNTIME_STATE_LOADED_ERROR_RESTRICTION);
  thrown.throw_snapshot=false;thrown.baseline();
  CHECK(thrown.calls==1 && !thrown.m_queue_health.candidate());
  for(auto state:{os_runtime_state_t::disabled,os_runtime_state_t::enabled_busy,
                  os_runtime_state_t::enabled_error}) {
    auto p=qualified();p.transition({0,state,false});
    CHECK(!p.m_queue_health.candidate());
    p.transition({0,os_runtime_state_t::enabled,false});p.baseline();
    CHECK(!p.m_queue_health.candidate() && p.calls==1);
  }
  auto duplicate=qualified();duplicate.transition({0,os_runtime_state_t::enabled,false});
  CHECK(!duplicate.m_queue_health.candidate());
  auto detached=qualified();detached.disable();
  CHECK(!detached.m_queue_health.candidate() && !detached.debug && !detached.healthy_at_disable);
  auto clean=qualified();
  CHECK(clean.observe(AMD_DBGAPI_STATUS_SUCCESS,os_exception_mask_t::none)
        && clean.m_queue_health.candidate() && clean.event_calls==1
        && clean.event_writes==1 && clean.event_output==os_exception_mask_t::none);
  bool failed_outputs_unchanged=true;
  for(auto status:{AMD_DBGAPI_STATUS_ERROR,AMD_DBGAPI_STATUS_ERROR_NOT_IMPLEMENTED}) {
    auto rejected=qualified();bool query_threw=false;
    try { rejected.observe(status,os_exception_mask_t::none); }
    catch(int value) { query_threw=value==99; }
    failed_outputs_unchanged=failed_outputs_unchanged && query_threw
      && rejected.event_calls==1 && rejected.event_writes==0
      && rejected.event_output==os_exception_mask_t::device_fatal_halt;
  }
  auto exited=qualified();
  CHECK(exited.observe(AMD_DBGAPI_STATUS_ERROR_PROCESS_EXITED,os_exception_mask_t::none)
        && !exited.m_queue_health.candidate() && exited.event_calls==1
        && exited.event_writes==0
        && exited.event_output==os_exception_mask_t::device_fatal_halt
        && failed_outputs_unchanged);
  for(unsigned bit=0;bit!=64;++bit) {
    auto p=qualified();auto mask=static_cast<os_exception_mask_t>(1ull<<bit);
    CHECK(!p.observe(AMD_DBGAPI_STATUS_SUCCESS,mask)
          && p.m_queue_health.candidate()==(bit==1||bit==30||bit==47));
  }
  for(unsigned bit=0;bit!=64;++bit) {
    auto p=qualified();auto mask=static_cast<os_exception_mask_t>(1ull<<bit);
    p.forward(mask);
    CHECK(p.forwards==1 && p.m_queue_health.candidate()==(bit==47));
  }
  auto handoff=qualified();handoff.throw_forward=true;threw=false;
  try{handoff.forward(os_exception_mask_t::queue_wave_trap);}catch(int x){threw=x==73;}
  CHECK(threw && !handoff.m_queue_health.candidate() && handoff.forwards==1);
  for(auto fault:{os_exception_mask_t::queue_wave_abort,
                  os_exception_mask_t::queue_wave_math_error,
                  os_exception_mask_t::queue_wave_illegal_instruction,
                  os_exception_mask_t::queue_wave_memory_violation,
                  os_exception_mask_t::queue_wave_address_error}) {
    auto p=qualified();
    CHECK(p.observe(AMD_DBGAPI_STATUS_SUCCESS,os_exception_mask_t::none));
    auto pending=fault; // Arrives AFTER query but BEFORE suspension.
    auto clear=p.suspension_clear();
    pending=pending & ~clear;
    CHECK(clear==os_exception_mask_t::none && pending==fault);
    p.observe(AMD_DBGAPI_STATUS_SUCCESS,pending);
    CHECK(!p.m_queue_health.candidate());
    auto already=qualified();pending=fault; // Already suspended: no clearing syscall.
    already.observe(AMD_DBGAPI_STATUS_SUCCESS,pending);
    CHECK(!already.m_queue_health.candidate());
  }
  auto held=qualified();held.observe(AMD_DBGAPI_STATUS_SUCCESS,os_exception_mask_t::queue_wave_trap);
  CHECK(held.m_queue_health.candidate() && held.suspension_clear()==os_exception_mask_t::none);
  held.forward(os_exception_mask_t::queue_wave_trap);
  CHECK(!held.m_queue_health.candidate());
  std::printf("assertions=%u; native=false; authority=false; scope=changed-call-sites-not-full-process\n",assertions);
}

// CPU-only production-body mock. No process attach, native API or GPU.
#include <cstddef>
#include <cstdint>
#include <cstdio>
#include <cstring>
#include <initializer_list>
#include "amd-dbgapi.h"
#include "queue-health-v1.h"
namespace amd::dbgapi {
using os_queue_id_t=uint32_t;
using os_agent_id_t=uint32_t;
enum class os_queue_state_t : uint8_t { error=1, invalid=2 };
enum class os_queue_type_t : uint32_t { unknown,compute,sdma,compute_aql,sdma_xgmi };
enum class os_exception_mask_t : uint64_t { none=0,queue_new=1ull<<30 };
struct api_error_t {
  amd_dbgapi_status_t status;
  explicit api_error_t(amd_dbgapi_status_t value):status(value) {}
};
namespace utils {
template<class T> void get_info(size_t bytes,void *out,const T &v) {
  if (!out || bytes!=sizeof(T)) throw api_error_t{AMD_DBGAPI_STATUS_ERROR_INVALID_ARGUMENT};
  std::memcpy(out,&v,sizeof(v));
}
}
struct os_queue_snapshot_entry_t
{
  os_queue_id_t queue_id;
  os_queue_state_t state{};
  bool exception_status_complete{ false }; // Never assume unknown raw bits are clean.
  os_agent_id_t gpu_id;
  os_queue_type_t queue_type{ os_queue_type_t::unknown };
  os_exception_mask_t exception_status;
  amd_dbgapi_global_address_t ring_base_address;
  amd_dbgapi_size_t ring_size;
  amd_dbgapi_global_address_t write_pointer_address;
  amd_dbgapi_global_address_t read_pointer_address;
  amd_dbgapi_global_address_t ctx_save_restore_address;
  amd_dbgapi_size_t ctx_save_restore_area_size;
};

struct driver_exception { int marker; };
struct fake_driver {
  bool enabled=true,throw_failure=false;
  amd_dbgapi_status_t result=AMD_DBGAPI_STATUS_SUCCESS;
  size_t reported=1,calls=0,last_capacity=0;
  os_exception_mask_t last_clear=static_cast<os_exception_mask_t>(~0ull);
  os_queue_snapshot_entry_t returned{};
  bool is_debug_enabled() const { return enabled; }
  amd_dbgapi_status_t queue_health_snapshot_v1(os_queue_snapshot_entry_t *out,size_t *count) {
    ++calls;last_capacity=1;last_clear=os_exception_mask_t::none;
    if (throw_failure) throw driver_exception{73};
    if (result!=AMD_DBGAPI_STATUS_SUCCESS) return result; // Do not initialize outputs.
    *out=returned;
    *count=reported;return result;
  }
};
struct process_t {
  mutable detail::queue_health_v1 m_queue_health;
  bool core=false;
  fake_driver driver;
  bool from_core() const { return core; }
  fake_driver &os_driver() { return driver; }
  bool take_queue_health_query() const { return m_queue_health.take_query(); }
  void taint_queue_health() const { m_queue_health.taint(); }
  void qualify() { m_queue_health.begin_baseline();m_queue_health.finish_baseline(true); }
};
struct queue_t {
  process_t &m_process;
  const os_queue_snapshot_entry_t m_os_queue_info;
  bool valid=true;
  process_t &process() const { return m_process; }
  bool is_valid() const { return valid; }
  bool healthy_live_v1() const;
  void get_health_info(amd_dbgapi_queue_info_t query,size_t value_size,void *value) const;
};

bool
queue_t::healthy_live_v1 () const
{
  process_t &owner = process ();
  if (!owner.take_queue_health_query ())
    return false;

  try
    {
      if (owner.from_core () || !owner.os_driver ().is_debug_enabled ()
          || !is_valid () || m_os_queue_info.queue_type != os_queue_type_t::compute_aql)
        {
          owner.taint_queue_health ();
          return false;
        }
    
      os_queue_snapshot_entry_t current{};
      size_t count = 0;
      amd_dbgapi_status_t status = owner.os_driver ().queue_health_snapshot_v1 (
        &current, &count);
      if (status != AMD_DBGAPI_STATUS_SUCCESS || count != 1
          || !detail::same_healthy_queue_v1 (
            m_os_queue_info, current, os_exception_mask_t::queue_new,
            os_exception_mask_t::none))
        {
          owner.taint_queue_health ();
          return false;
        }
      return true;
    }
  catch (...)
    {
      owner.taint_queue_health ();
      throw; // Preserve the original exception; never regain a read permit.
    }
}

void queue_t::get_health_info(amd_dbgapi_queue_info_t query,size_t value_size,void *value) const {
  switch(query) {
    case AMD_DBGAPI_QUEUE_INFO_STATE:
    case AMD_DBGAPI_QUEUE_INFO_ERROR_REASON:
      {
        // Bad output arguments do not consume a driver observation.
        if (value == nullptr)
          throw api_error_t (AMD_DBGAPI_STATUS_ERROR_INVALID_ARGUMENT);
        if (value_size != sizeof (uint32_t))
          throw api_error_t (AMD_DBGAPI_STATUS_ERROR_INVALID_ARGUMENT_COMPATIBILITY);
        if (!healthy_live_v1 ())
          throw api_error_t (AMD_DBGAPI_STATUS_ERROR_NOT_IMPLEMENTED);
        // These are conclusions from the live positive proof, never fallback
        // constants for an unsupported/unknown/error observation.
        const uint32_t result
          = query == AMD_DBGAPI_QUEUE_INFO_STATE
              ? static_cast<uint32_t> (AMD_DBGAPI_QUEUE_STATE_VALID)
              : static_cast<uint32_t> (AMD_DBGAPI_EXCEPTION_NONE);
        utils::get_info (value_size, value, result);
        return;
      }
  default: throw api_error_t{AMD_DBGAPI_STATUS_ERROR_INVALID_ARGUMENT};
  }
}

}
using namespace amd::dbgapi;
static unsigned assertions=0;
#define CHECK(x) do { if (!(x)) return __LINE__; ++assertions; } while(false)
static os_queue_snapshot_entry_t birth() {
  os_queue_snapshot_entry_t b{};
  b.queue_id=0;b.gpu_id=7;b.queue_type=os_queue_type_t::compute_aql;
  b.exception_status_complete=true;b.exception_status=os_exception_mask_t::queue_new;
  b.ring_base_address=4096;b.ring_size=4096;
  b.write_pointer_address=8192;b.read_pointer_address=8200;
  b.ctx_save_restore_address=12288;b.ctx_save_restore_area_size=4096;
  return b;
}
static void current(process_t &p) {
  p.driver.returned=birth();p.driver.returned.exception_status=os_exception_mask_t::none;
}
template<class Call> static bool fails(Call call,amd_dbgapi_status_t status) {
  try {call();}catch(const api_error_t &e){return e.status==status;}
  return false;
}
int main() {
  process_t p;current(p);queue_t q{p,birth()};
  CHECK(!q.healthy_live_v1() && p.driver.calls==0);
  CHECK(!p.m_queue_health.candidate());
  process_t good;good.qualify();current(good);queue_t g{good,birth()};
  uint32_t value=0xdecafbad;
  g.get_health_info(AMD_DBGAPI_QUEUE_INFO_STATE,sizeof value,&value);
  CHECK(value==AMD_DBGAPI_QUEUE_STATE_VALID && good.driver.calls==1);
  CHECK(good.driver.last_capacity==1 && good.driver.last_clear==os_exception_mask_t::none);
  value=0xdecafbad;
  g.get_health_info(AMD_DBGAPI_QUEUE_INFO_ERROR_REASON,sizeof value,&value);
  CHECK(value==AMD_DBGAPI_EXCEPTION_NONE && good.driver.calls==2);
  CHECK(fails([&]{g.get_health_info(AMD_DBGAPI_QUEUE_INFO_STATE,4,nullptr);},
              AMD_DBGAPI_STATUS_ERROR_INVALID_ARGUMENT));
  value=0xdecafbad;
  CHECK(fails([&]{g.get_health_info(AMD_DBGAPI_QUEUE_INFO_ERROR_REASON,8,&value);},
              AMD_DBGAPI_STATUS_ERROR_INVALID_ARGUMENT_COMPATIBILITY));
  CHECK(value==0xdecafbad && good.driver.calls==2 && good.m_queue_health.queries()==2);
  for(unsigned i=2;i!=10;++i) CHECK(g.healthy_live_v1());
  value=0xdecafbad;
  CHECK(fails([&]{g.get_health_info(AMD_DBGAPI_QUEUE_INFO_STATE,4,&value);},
              AMD_DBGAPI_STATUS_ERROR_NOT_IMPLEMENTED));
  CHECK(good.driver.calls==10 && value==0xdecafbad);
  for(unsigned which=0;which!=4;++which) {
    process_t x;x.qualify();current(x);auto b=birth();
    if(which==0)x.core=true;
    if(which==1)x.driver.enabled=false;
    if(which==2)b.queue_type=os_queue_type_t::sdma;
    queue_t target{x,b};if(which==3)target.valid=false;
    CHECK(!target.healthy_live_v1() && x.driver.calls==0 && !x.m_queue_health.candidate());
  }
  for(auto status:{AMD_DBGAPI_STATUS_ERROR_PROCESS_EXITED,AMD_DBGAPI_STATUS_ERROR,
                   AMD_DBGAPI_STATUS_FATAL,AMD_DBGAPI_STATUS_ERROR_NOT_IMPLEMENTED}) {
    process_t x;x.qualify();current(x);x.driver.result=status;queue_t target{x,birth()};
    value=0xdecafbad;
    CHECK(fails([&]{target.get_health_info(AMD_DBGAPI_QUEUE_INFO_STATE,4,&value);},
                AMD_DBGAPI_STATUS_ERROR_NOT_IMPLEMENTED));
    CHECK(x.driver.calls==1 && value==0xdecafbad && !x.m_queue_health.candidate());
    x.driver.result=AMD_DBGAPI_STATUS_SUCCESS;
    CHECK(!target.healthy_live_v1() && x.driver.calls==1);
  }
  for(size_t count:{0u,2u,999u}) {
    process_t x;x.qualify();current(x);x.driver.reported=count;queue_t target{x,birth()};
    CHECK(!target.healthy_live_v1() && x.driver.calls==1 && !x.m_queue_health.candidate());
  }
  for(unsigned which=0;which!=4;++which) {
    process_t x;x.qualify();current(x);auto b=birth();
    if(which==0)b.exception_status_complete=false;
    if(which==1)x.driver.returned.exception_status_complete=false;
    if(which==2)x.driver.returned.exception_status=os_exception_mask_t::queue_new;
    if(which==3)x.driver.returned.ring_base_address+=4096;
    queue_t target{x,b};
    CHECK(!target.healthy_live_v1() && x.driver.calls==1 && !x.m_queue_health.candidate());
  }
  {
    process_t x;x.qualify();current(x);x.driver.throw_failure=true;queue_t target{x,birth()};
    bool preserved=false;
    try { target.healthy_live_v1(); } catch (const driver_exception &e) { preserved=e.marker==73; }
    CHECK(preserved && x.driver.calls==1 && !x.m_queue_health.candidate());
    x.driver.throw_failure=false;
    CHECK(!target.healthy_live_v1() && x.driver.calls==1);
  }
  CHECK(sizeof(os_queue_snapshot_entry_t)==72 && alignof(os_queue_snapshot_entry_t)==8);
  CHECK(sizeof(detail::queue_health_v1)==5);
  std::printf("production_body_assertions=%u; os_snapshot=%zu; core=%zu; native=false; authority=false\n",
              assertions,sizeof(os_queue_snapshot_entry_t),sizeof(detail::queue_health_v1));
}

// CPU-only exact driver-body controls. The ioctl token is intercepted by a
// local mock at preprocessing; no device, process, provider API or GPU is used.
#include <cstddef>
#include <cstdint>
#include <cstdio>
#include <cstring>
#include <optional>
#include <type_traits>
#include <limits>
#include <cerrno>
#include <initializer_list>
#include "amd-dbgapi.h"
#include "linux/kfd_ioctl.h"
namespace amd::dbgapi {
using os_queue_id_t=uint32_t;
using os_agent_id_t=uint32_t;
enum class os_queue_state_t : uint8_t { error=1,invalid=2 };
enum class os_queue_type_t : uint32_t { unknown,compute,sdma,compute_aql,sdma_xgmi };
enum class os_exception_mask_t : uint64_t { none=0,queue_new=1ull<<30 };
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

struct default_driver {
  virtual amd_dbgapi_status_t
  queue_health_snapshot_v1 (os_queue_snapshot_entry_t *, size_t *) const
  {
    return AMD_DBGAPI_STATUS_ERROR_NOT_IMPLEMENTED;
  }

};
struct core_driver : default_driver {};
struct kfd_driver_t : default_driver {
  static inline std::optional<int> s_kfd_fd{17};
  std::optional<amd_dbgapi_os_process_id_t> m_os_pid{123};
  bool valid=true,enabled=true;
  bool is_valid() const { return valid; }
  bool is_debug_enabled() const { return enabled; }
  amd_dbgapi_status_t queue_health_snapshot_v1(
      os_queue_snapshot_entry_t *snapshot,size_t *queue_count) const override;
};
}
struct mock_control {
  unsigned calls=0;
  int result=0,error=0,mutation=0;
  uint32_t count=1;
  bool input_exact=true;
  kfd_queue_snapshot_entry raw{};
  mock_control() {
    raw.queue_id=7;raw.gpu_id=9;raw.queue_type=KFD_IOC_QUEUE_TYPE_COMPUTE_AQL;
    raw.ring_base_address=0x10000;raw.ring_size=4096;
    raw.write_pointer_address=0x20000;raw.read_pointer_address=0x20008;
    raw.ctx_save_restore_address=0x30000;raw.ctx_save_restore_area_size=8192;
  }
};
static mock_control control;
static int fe2o3_mock_ioctl(int fd,unsigned long request,void *opaque) {
  ++control.calls;
  auto &a=*static_cast<kfd_ioctl_dbg_trap_args*>(opaque);
  auto *raw=reinterpret_cast<kfd_queue_snapshot_entry*>(
      static_cast<uintptr_t>(a.queue_snapshot.snapshot_buf_ptr));
  kfd_queue_snapshot_entry zero{};
  control.input_exact &= fd==17 && request==AMDKFD_IOC_DBG_TRAP
      && a.pid==123 && a.op==KFD_IOC_DBG_TRAP_GET_QUEUE_SNAPSHOT
      && a.queue_snapshot.exception_mask==0
      && a.queue_snapshot.num_queues==1
      && a.queue_snapshot.entry_size==sizeof(*raw)
      && raw!=nullptr && std::memcmp(raw,&zero,sizeof(zero))==0;
  if (control.result!=0) { errno=control.error;return control.result; }
  *raw=control.raw;
  a.queue_snapshot.num_queues=control.count;
  switch(control.mutation) {
    case 1: ++a.pid;break;
    case 2: ++a.op;break;
    case 3: a.queue_snapshot.exception_mask=1;break;
    case 4: ++a.queue_snapshot.snapshot_buf_ptr;break;
    case 5: --a.queue_snapshot.entry_size;break;
    case 6: ++a.queue_snapshot.entry_size;break;
  }
  return 0;
}
#define ioctl fe2o3_mock_ioctl
namespace amd::dbgapi {
amd_dbgapi_status_t
kfd_driver_t::queue_health_snapshot_v1 (
  os_queue_snapshot_entry_t *snapshot, size_t *queue_count) const
{
  if (snapshot == nullptr || queue_count == nullptr)
    return AMD_DBGAPI_STATUS_ERROR_INVALID_ARGUMENT;
  if (!is_valid () || !is_debug_enabled () || !m_os_pid || *m_os_pid <= 0
      || !s_kfd_fd)
    return AMD_DBGAPI_STATUS_ERROR_NOT_IMPLEMENTED;

  // Capacity one, no clearing, no heap or TRACE formatting. The caller has
  // already consumed its one baseline or one of ten lifetime query permits.
  const uint32_t pid = static_cast<uint32_t> (*m_os_pid);
  kfd_queue_snapshot_entry raw{};
  kfd_ioctl_dbg_trap_args args{};
  args.pid = pid;
  args.op = KFD_IOC_DBG_TRAP_GET_QUEUE_SNAPSHOT;
  args.queue_snapshot.exception_mask = 0;
  args.queue_snapshot.snapshot_buf_ptr = reinterpret_cast<uintptr_t> (&raw);
  args.queue_snapshot.num_queues = 1;
  args.queue_snapshot.entry_size = sizeof (raw);

  // Do not use kfd_ioctl(): it retries EINTR. This private observation gets
  // exactly one syscall, and every nonzero return is a refusal.
  const int result = ::ioctl (*s_kfd_fd, AMDKFD_IOC_DBG_TRAP, &args);
  if (result != 0)
    return result < 0 && errno == ESRCH
             ? AMD_DBGAPI_STATUS_ERROR_PROCESS_EXITED
             : AMD_DBGAPI_STATUS_ERROR;

  if (args.pid != pid
      || args.op != KFD_IOC_DBG_TRAP_GET_QUEUE_SNAPSHOT
      || args.queue_snapshot.exception_mask != 0
      || args.queue_snapshot.snapshot_buf_ptr
           != reinterpret_cast<uintptr_t> (&raw)
      || args.queue_snapshot.entry_size != sizeof (raw)
      || args.queue_snapshot.num_queues > 1)
    return AMD_DBGAPI_STATUS_ERROR;

  if (args.queue_snapshot.num_queues == 0)
    {
      *queue_count = 0; // A real empty snapshot, never a synthesized healthy row.
      return AMD_DBGAPI_STATUS_SUCCESS;
    }

  // Refuse all unknown as well as known exception bits before any mapping can
  // discard them. No queue flags, unsupported type or reserved ABI data qualify.
  if (raw.exception_status != 0 || raw.reserved != 0
      || (raw.queue_id
          & (uint32_t (KFD_DBG_QUEUE_ERROR_MASK)
             | uint32_t (KFD_DBG_QUEUE_INVALID_MASK))) != 0
      || raw.queue_type != KFD_IOC_QUEUE_TYPE_COMPUTE_AQL)
    return AMD_DBGAPI_STATUS_ERROR_NOT_IMPLEMENTED;

  os_queue_snapshot_entry_t observed{};
  observed.queue_id = raw.queue_id;
  observed.state = {};
  observed.exception_status_complete = true;
  observed.gpu_id = raw.gpu_id;
  observed.queue_type = os_queue_type_t::compute_aql;
  observed.exception_status = os_exception_mask_t::none;
  observed.ring_base_address = raw.ring_base_address;
  observed.ring_size = raw.ring_size;
  observed.write_pointer_address = raw.write_pointer_address;
  observed.read_pointer_address = raw.read_pointer_address;
  observed.ctx_save_restore_address = raw.ctx_save_restore_address;
  observed.ctx_save_restore_area_size = raw.ctx_save_restore_area_size;
  *snapshot = observed;
  *queue_count = 1;
  return AMD_DBGAPI_STATUS_SUCCESS;
}


}
#undef ioctl
using namespace amd::dbgapi;
static unsigned assertions=0;
#define CHECK(x) do { if (!(x)) return __LINE__; ++assertions; } while(false)
static os_queue_snapshot_entry_t sentinel() {
  os_queue_snapshot_entry_t row{};
  row.queue_id=0xa5a5;row.gpu_id=0x5a5a;row.ring_base_address=0x9999;
  return row;
}
static bool same_row(const os_queue_snapshot_entry_t &row,
                     const os_queue_snapshot_entry_t &before) {
  return row.queue_id==before.queue_id && row.state==before.state
    && row.exception_status_complete==before.exception_status_complete
    && row.gpu_id==before.gpu_id && row.queue_type==before.queue_type
    && row.exception_status==before.exception_status
    && row.ring_base_address==before.ring_base_address
    && row.ring_size==before.ring_size
    && row.write_pointer_address==before.write_pointer_address
    && row.read_pointer_address==before.read_pointer_address
    && row.ctx_save_restore_address==before.ctx_save_restore_address
    && row.ctx_save_restore_area_size==before.ctx_save_restore_area_size;
}
static bool unchanged(const os_queue_snapshot_entry_t &row,
                      const os_queue_snapshot_entry_t &before,size_t count) {
  return count==999 && same_row(row,before);
}
int main() {
  static_assert(sizeof(kfd_queue_snapshot_entry)==64,"fixed ABI raw row");
  static_assert(std::is_signed_v<amd_dbgapi_os_process_id_t>,"actual public signed PID");
  default_driver other;core_driver core;kfd_driver_t live;
  auto before=sentinel(),out=before;size_t count=999;
  CHECK(other.queue_health_snapshot_v1(&out,&count)
        ==AMD_DBGAPI_STATUS_ERROR_NOT_IMPLEMENTED && unchanged(out,before,count));
  CHECK(core.queue_health_snapshot_v1(&out,&count)
        ==AMD_DBGAPI_STATUS_ERROR_NOT_IMPLEMENTED && unchanged(out,before,count));
  CHECK(control.calls==0);
  CHECK(live.queue_health_snapshot_v1(nullptr,&count)
        ==AMD_DBGAPI_STATUS_ERROR_INVALID_ARGUMENT && control.calls==0 && count==999);
  CHECK(live.queue_health_snapshot_v1(&out,nullptr)
        ==AMD_DBGAPI_STATUS_ERROR_INVALID_ARGUMENT && control.calls==0
        && same_row(out,before));
  for(unsigned which=0;which!=4;++which) {
    kfd_driver_t d;
    if(which==0)d.valid=false;
    if(which==1)d.enabled=false;
    if(which==2)d.m_os_pid.reset();
    if(which==3)kfd_driver_t::s_kfd_fd.reset();
    CHECK(d.queue_health_snapshot_v1(&out,&count)
          ==AMD_DBGAPI_STATUS_ERROR_NOT_IMPLEMENTED && control.calls==0
          && unchanged(out,before,count));
    kfd_driver_t::s_kfd_fd=17;
  }
  for(auto pid:{amd_dbgapi_os_process_id_t{0},amd_dbgapi_os_process_id_t{-1},
                std::numeric_limits<amd_dbgapi_os_process_id_t>::min()}) {
    kfd_driver_t d;d.m_os_pid=pid;
    CHECK(d.queue_health_snapshot_v1(&out,&count)
          ==AMD_DBGAPI_STATUS_ERROR_NOT_IMPLEMENTED && control.calls==0
          && unchanged(out,before,count));
  }
  control=mock_control{};control.count=0;
  CHECK(live.queue_health_snapshot_v1(&out,&count)==AMD_DBGAPI_STATUS_SUCCESS
        && count==0 && same_row(out,before)
        && control.calls==1 && control.input_exact);
  control=mock_control{};count=999;
  CHECK(live.queue_health_snapshot_v1(&out,&count)==AMD_DBGAPI_STATUS_SUCCESS
        && count==1 && control.calls==1 && control.input_exact);
  CHECK(out.queue_id==control.raw.queue_id);
  CHECK(out.gpu_id==control.raw.gpu_id);
  CHECK(out.queue_type==os_queue_type_t::compute_aql);
  CHECK(out.state==os_queue_state_t{} && out.exception_status_complete
        && out.exception_status==os_exception_mask_t::none);
  CHECK(out.ring_base_address==control.raw.ring_base_address);
  CHECK(out.ring_size==control.raw.ring_size);
  CHECK(out.write_pointer_address==control.raw.write_pointer_address);
  CHECK(out.read_pointer_address==control.raw.read_pointer_address);
  CHECK(out.ctx_save_restore_address==control.raw.ctx_save_restore_address);
  CHECK(out.ctx_save_restore_area_size==control.raw.ctx_save_restore_area_size);
  for(uint32_t n:{2u,0xffffffffu}) {
    control=mock_control{};control.count=n;out=before;count=999;
    CHECK(live.queue_health_snapshot_v1(&out,&count)==AMD_DBGAPI_STATUS_ERROR
          && control.calls==1 && control.input_exact && unchanged(out,before,count));
  }
  for(int mutation=1;mutation<=6;++mutation) {
    control=mock_control{};control.mutation=mutation;out=before;count=999;
    CHECK(live.queue_health_snapshot_v1(&out,&count)==AMD_DBGAPI_STATUS_ERROR
          && control.calls==1 && control.input_exact && unchanged(out,before,count));
  }
  for(unsigned bit=0;bit!=64;++bit) {
    control=mock_control{};control.raw.exception_status=1ull<<bit;out=before;count=999;
    CHECK(live.queue_health_snapshot_v1(&out,&count)
          ==AMD_DBGAPI_STATUS_ERROR_NOT_IMPLEMENTED
          && control.calls==1 && control.input_exact && unchanged(out,before,count));
  }
  for(unsigned bit:{30u,31u}) {
    control=mock_control{};control.raw.queue_id|=1u<<bit;out=before;count=999;
    CHECK(live.queue_health_snapshot_v1(&out,&count)
          ==AMD_DBGAPI_STATUS_ERROR_NOT_IMPLEMENTED
          && control.calls==1 && control.input_exact && unchanged(out,before,count));
  }
  for(uint32_t type:{0u,1u,3u,0xffffffffu}) {
    control=mock_control{};control.raw.queue_type=type;out=before;count=999;
    CHECK(live.queue_health_snapshot_v1(&out,&count)
          ==AMD_DBGAPI_STATUS_ERROR_NOT_IMPLEMENTED
          && control.calls==1 && control.input_exact && unchanged(out,before,count));
  }
  control=mock_control{};control.raw.reserved=1;out=before;count=999;
  CHECK(live.queue_health_snapshot_v1(&out,&count)
        ==AMD_DBGAPI_STATUS_ERROR_NOT_IMPLEMENTED
        && control.calls==1 && control.input_exact && unchanged(out,before,count));
  for(int result:{-1,1,7}) for(int error:{EINTR,ESRCH,EIO,0}) {
    control=mock_control{};control.result=result;control.error=error;
    out=before;count=999;
    auto expected=result<0 && error==ESRCH
        ? AMD_DBGAPI_STATUS_ERROR_PROCESS_EXITED : AMD_DBGAPI_STATUS_ERROR;
    CHECK(live.queue_health_snapshot_v1(&out,&count)==expected
          && control.calls==1 && control.input_exact && unchanged(out,before,count));
  }
  // This proves only exact body behavior with a mock boundary, not live KFD.
  std::printf("assertions=%u; raw=%zu; args=%zu; os_snapshot=%zu; native=false; authority=false\n",
      assertions,sizeof(kfd_queue_snapshot_entry),sizeof(kfd_ioctl_dbg_trap_args),
      sizeof(os_queue_snapshot_entry_t));
}

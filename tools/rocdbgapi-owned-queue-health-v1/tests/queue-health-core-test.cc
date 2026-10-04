// CPU-only policy controls; no ROCdbgapi invocation, KFD, process or GPU.
#include <cstdint>
#include <cstdio>
#include <initializer_list>
#include "queue-health-v1.h"
using amd::dbgapi::detail::queue_health_v1;
using amd::dbgapi::detail::same_healthy_queue_v1;
struct snapshot {
  std::uint32_t queue_id=0, gpu_id=7, queue_type=3;
  std::uint8_t state=0;
  bool exception_status_complete=true;
  std::uint64_t exception_status=0;
  std::uint64_t ring_base_address=4096, ring_size=4096;
  std::uint64_t write_pointer_address=8192, read_pointer_address=8200;
  std::uint64_t ctx_save_restore_address=12288, ctx_save_restore_area_size=4096;
};
static unsigned passed=0;
#define CHECK(x) do { if (!(x)) return __LINE__; ++passed; } while (false)
static queue_health_v1 qualified () {
  queue_health_v1 q; q.begin_baseline (); q.finish_baseline (true); return q;
}
int main () {
  constexpr std::uint64_t birth_mask=1ull<<30, none=0;
  queue_health_v1 empty;
  CHECK (!empty.candidate ());
  CHECK (!empty.take_query () && empty.queries ()==0);
  CHECK (!empty.begin_baseline ());
  queue_health_v1 failed;
  CHECK (failed.begin_baseline ());
  failed.finish_baseline (false);
  CHECK (!failed.candidate () && !failed.take_query ());
  queue_health_v1 order;
  order.finish_baseline (true);
  CHECK (!order.candidate () && !order.begin_baseline ());
  auto duplicate=qualified ();
  CHECK (duplicate.candidate ());
  CHECK (!duplicate.begin_baseline () && !duplicate.candidate ());
  auto refinish=qualified (); refinish.finish_baseline (true);
  CHECK (!refinish.candidate ());
  auto tainted=qualified (); tainted.taint ();
  CHECK (!tainted.candidate () && !tainted.take_query ());
  auto bounded=qualified ();
  for (unsigned i=0;i!=10;++i) CHECK (bounded.take_query () && bounded.queries ()==i+1);
  CHECK (!bounded.take_query () && bounded.queries ()==10 && !bounded.candidate ());
  CHECK (!bounded.take_query () && bounded.queries ()==10);
  snapshot birth, now; birth.exception_status=birth_mask;
  CHECK (same_healthy_queue_v1 (birth,now,birth_mask,none));
  for (unsigned bit=0;bit!=64;++bit) {
    snapshot bad=now; bad.exception_status=1ull<<bit;
    CHECK (!same_healthy_queue_v1 (birth,bad,birth_mask,none));
  }
  snapshot badbirth=birth; badbirth.exception_status=0;
  CHECK (!same_healthy_queue_v1 (badbirth,now,birth_mask,none));
  for (unsigned bit=0;bit!=64;++bit) {
    if (bit==30) continue;
    snapshot bad=birth; bad.exception_status|=1ull<<bit;
    CHECK (!same_healthy_queue_v1 (bad,now,birth_mask,none));
  }
  for (unsigned which=0;which!=4;++which) {
    auto b=birth,n=now;
    if (which==0) b.exception_status_complete=false;
    if (which==1) n.exception_status_complete=false;
    if (which==2) b.state=1;
    if (which==3) n.state=2;
    CHECK (!same_healthy_queue_v1 (b,n,birth_mask,none));
  }
  for (auto member : {&snapshot::queue_id,&snapshot::gpu_id,&snapshot::queue_type}) {
    auto n=now; n.*member+=1;
    CHECK (!same_healthy_queue_v1 (birth,n,birth_mask,none));
  }
  for (auto member : {&snapshot::ring_base_address,&snapshot::ring_size,
       &snapshot::write_pointer_address,&snapshot::read_pointer_address,
       &snapshot::ctx_save_restore_address,&snapshot::ctx_save_restore_area_size}) {
    auto n=now; n.*member+=8;
    CHECK (!same_healthy_queue_v1 (birth,n,birth_mask,none));
  }
  auto calls=qualified (); unsigned reads=0;
  const auto read_once=[&](bool status_success,std::size_t count,const snapshot &value) {
    if (!calls.take_query ()) return false;
    ++reads;
    if (!status_success || count!=1
        || !same_healthy_queue_v1 (birth,value,birth_mask,none)) {
      calls.taint (); return false;
    }
    return true;
  };
  CHECK (read_once (true,1,now) && reads==1);
  CHECK (!read_once (false,1,now) && reads==2);
  CHECK (!read_once (true,1,now) && reads==2);
  for (std::size_t count : {0u,2u}) {
    auto q=qualified (); CHECK (q.take_query ());
    if (count!=1) q.taint ();
    CHECK (!q.candidate () && !q.take_query ());
  }
  CHECK (sizeof (queue_health_v1)==5);
  std::printf ("assertions=%u; native=false; authority=false\n",passed);
}

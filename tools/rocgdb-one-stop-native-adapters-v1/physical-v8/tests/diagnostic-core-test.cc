// CPU-only data controls; no GDB, process attach, API, file I/O or GPU.
#include <cstdint>
#include <cstdio>
#include <climits>
#include "diagnostic-core.inc"
static unsigned passed = 0;
#define CHECK(x) do { if (!(x)) return __LINE__; ++passed; } while (false)
int main () {
  native_result_note n;
  CHECK (n.site == native_result_site::none && n.status == 0);
  n.remember (false,false,native_result_site::scalar_query,native_result_family::queue,5,4,-3);
  CHECK (n.site == native_result_site::none);
  n.remember (true,true,native_result_site::scalar_query,native_result_family::queue,5,4,-3);
  CHECK (n.site == native_result_site::none);
  n.remember (true,false,native_result_site::scalar_query,native_result_family::queue,5,4,0);
  CHECK (n.site == native_result_site::none);
  n.remember (true,false,native_result_site::none,native_result_family::queue,5,4,-3);
  CHECK (n.site == native_result_site::none);
  n.remember (true,false,native_result_site::scalar_query,native_result_family::queue,5,4,-3);
  CHECK (n.site == native_result_site::scalar_query && n.family == native_result_family::queue
         && n.selector == 5 && n.bytes == 4 && n.status == -3);
  n.remember (true,false,native_result_site::scalar_query,native_result_family::queue,6,4,-8);
  CHECK (n.selector == 5 && n.status == -3);
  n.remember (true,true,native_result_site::target_memory,native_result_family::none,0,776,5);
  CHECK (n.site == native_result_site::scalar_query && n.status == -3);
  native_result_note read;
  read.remember (true,false,native_result_site::target_memory,native_result_family::none,0,776,5);
  CHECK (read.site == native_result_site::target_memory && read.family == native_result_family::none
         && read.selector == 0 && read.bytes == 776 && read.status == 5);
  native_result_note limits;
  limits.remember (true,false,native_result_site::scalar_query,native_result_family::other,
                   UINT32_MAX,UINT32_MAX,INT32_MIN);
  CHECK (limits.selector == UINT32_MAX && limits.bytes == UINT32_MAX && limits.status == INT32_MIN);
  char bounded[224];
  const int size = std::snprintf (bounded,sizeof bounded,
    "fixed owned one-stop native relation refused (%u); commit-site=%u; native-site=%u; native-family=%u; native-selector=%u; native-bytes=%u; native-status=%d",
    UINT32_MAX,UINT32_MAX,255u,255u,UINT32_MAX,UINT32_MAX,INT32_MIN);
  CHECK (size > 0 && static_cast<unsigned> (size) < sizeof bounded);
  CHECK (sizeof (native_result_note) == 16 && alignof (native_result_note) == 4);
  std::printf ("passed=%u; native=false; authority=false\n", passed);
}

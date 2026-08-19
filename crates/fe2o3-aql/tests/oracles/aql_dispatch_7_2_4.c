#include <stddef.h>
#include <stdint.h>
#include <stdio.h>

#include "hsa.h"
#include "amd_hsa_signal.h"

int main(void) {
  _Static_assert(sizeof(hsa_kernel_dispatch_packet_t) == 64, "packet size");
  _Static_assert(_Alignof(hsa_kernel_dispatch_packet_t) == 8, "packet align");
  _Static_assert(offsetof(hsa_kernel_dispatch_packet_t, header) == 0, "header");
  _Static_assert(offsetof(hsa_kernel_dispatch_packet_t, setup) == 2, "setup");
  _Static_assert(offsetof(hsa_kernel_dispatch_packet_t, workgroup_size_x) == 4, "workgroup");
  _Static_assert(offsetof(hsa_kernel_dispatch_packet_t, grid_size_x) == 12, "grid");
  _Static_assert(offsetof(hsa_kernel_dispatch_packet_t, private_segment_size) == 24, "private");
  _Static_assert(offsetof(hsa_kernel_dispatch_packet_t, group_segment_size) == 28, "group");
  _Static_assert(offsetof(hsa_kernel_dispatch_packet_t, kernel_object) == 32, "kernel object");
  _Static_assert(offsetof(hsa_kernel_dispatch_packet_t, kernarg_address) == 40, "kernarg");
  _Static_assert(offsetof(hsa_kernel_dispatch_packet_t, reserved2) == 48, "reserved2");
  _Static_assert(offsetof(hsa_kernel_dispatch_packet_t, completion_signal) == 56, "signal");

  _Static_assert(sizeof(amd_signal_t) == 64, "signal size");
  _Static_assert(_Alignof(amd_signal_t) == 64, "signal align");
  _Static_assert(offsetof(amd_signal_t, kind) == 0, "signal kind");
  _Static_assert(offsetof(amd_signal_t, value) == 8, "signal value");

  uint16_t header =
      (uint16_t)(HSA_PACKET_TYPE_KERNEL_DISPATCH << HSA_PACKET_HEADER_TYPE) |
      (uint16_t)(HSA_FENCE_SCOPE_SYSTEM << HSA_PACKET_HEADER_SCACQUIRE_FENCE_SCOPE) |
      (uint16_t)(HSA_FENCE_SCOPE_SYSTEM << HSA_PACKET_HEADER_SCRELEASE_FENCE_SCOPE);
  printf("packet=64/8 signal=64/64 header=0x%04x kind=%d\n", header,
         AMD_SIGNAL_KIND_USER);
  return header == 0x1402 && AMD_SIGNAL_KIND_USER == 1 ? 0 : 1;
}

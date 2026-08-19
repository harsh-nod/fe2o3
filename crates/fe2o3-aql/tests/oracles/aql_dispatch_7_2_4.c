#include <stddef.h>
#include <stdint.h>
#include <stdio.h>
#include <string.h>

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
  _Static_assert(offsetof(amd_signal_t, event_mailbox_ptr) == 16, "signal mailbox");
  _Static_assert(offsetof(amd_signal_t, event_id) == 24, "signal event id");
  _Static_assert(offsetof(amd_signal_t, reserved1) == 28, "signal reserved1");
  _Static_assert(offsetof(amd_signal_t, start_ts) == 32, "signal start timestamp");
  _Static_assert(offsetof(amd_signal_t, end_ts) == 40, "signal end timestamp");
  _Static_assert(offsetof(amd_signal_t, reserved2) == 48, "signal reserved2");
  _Static_assert(offsetof(amd_signal_t, reserved3) == 56, "signal reserved3");

  hsa_kernel_dispatch_packet_t packet;
  memset(&packet, 0, sizeof(packet));
  packet.header = HSA_PACKET_TYPE_INVALID << HSA_PACKET_HEADER_TYPE;
  packet.setup = 1u << HSA_KERNEL_DISPATCH_PACKET_SETUP_DIMENSIONS;

  uint32_t initial_word = 0;
  memcpy(&initial_word, &packet, sizeof(initial_word));

  uint16_t header =
      (uint16_t)(HSA_PACKET_TYPE_KERNEL_DISPATCH << HSA_PACKET_HEADER_TYPE) |
      (uint16_t)(HSA_FENCE_SCOPE_SYSTEM << HSA_PACKET_HEADER_SCACQUIRE_FENCE_SCOPE) |
      (uint16_t)(HSA_FENCE_SCOPE_SYSTEM << HSA_PACKET_HEADER_SCRELEASE_FENCE_SCOPE);
  uint32_t final_word = (1u << 16) | header;
  uint32_t published = initial_word;
  __atomic_store_n(&published, final_word, __ATOMIC_RELEASE);

  amd_signal_t signal;
  memset(&signal, 0, sizeof(signal));
  signal.kind = AMD_SIGNAL_KIND_USER;
  signal.value = 1;
  const unsigned char *signal_bytes = (const unsigned char *)&signal;
  int reserved_zero = 1;
  for (size_t i = 16; i < sizeof(signal); ++i) {
    reserved_zero &= signal_bytes[i] == 0;
  }

  printf("packet=64/8 signal=64/64 invalid=%d initial=0x%08x published=0x%08x kind=%d reserved-zero=%d\n",
         HSA_PACKET_TYPE_INVALID, initial_word, published, AMD_SIGNAL_KIND_USER,
         reserved_zero);
  return HSA_PACKET_TYPE_INVALID == 1 && initial_word == 0x00010001u &&
                 header == 0x1402 && published == 0x00011402u &&
                 AMD_SIGNAL_KIND_USER == 1 && signal.value == 1 && reserved_zero
             ? 0
             : 1;
}

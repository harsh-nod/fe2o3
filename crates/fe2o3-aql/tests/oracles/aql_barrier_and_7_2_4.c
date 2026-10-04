#include <stddef.h>
#include <stdint.h>
#include <stdio.h>
#include <string.h>

#include "hsa.h"

int main(void) {
  _Static_assert(sizeof(hsa_barrier_and_packet_t) == 64, "packet size");
  _Static_assert(_Alignof(hsa_barrier_and_packet_t) == 8, "packet align");
  _Static_assert(offsetof(hsa_barrier_and_packet_t, header) == 0, "header");
  _Static_assert(offsetof(hsa_barrier_and_packet_t, reserved0) == 2, "reserved0");
  _Static_assert(offsetof(hsa_barrier_and_packet_t, reserved1) == 4, "reserved1");
  _Static_assert(offsetof(hsa_barrier_and_packet_t, dep_signal) == 8, "dependencies");
  _Static_assert(offsetof(hsa_barrier_and_packet_t, reserved2) == 48, "reserved2");
  _Static_assert(offsetof(hsa_barrier_and_packet_t, completion_signal) == 56,
                 "completion signal");

  hsa_barrier_and_packet_t packet;
  memset(&packet, 0, sizeof(packet));
  packet.header = HSA_PACKET_TYPE_INVALID << HSA_PACKET_HEADER_TYPE;
  packet.completion_signal.handle = UINT64_C(0x3040);

  unsigned char unpublished[sizeof(packet)];
  memcpy(unpublished, &packet, sizeof(packet));
  uint32_t initial_word = 0;
  memcpy(&initial_word, unpublished, sizeof(initial_word));

  uint16_t header =
      (uint16_t)(HSA_PACKET_TYPE_BARRIER_AND << HSA_PACKET_HEADER_TYPE) |
      (uint16_t)(HSA_FENCE_SCOPE_SYSTEM
                 << HSA_PACKET_HEADER_SCACQUIRE_FENCE_SCOPE) |
      (uint16_t)(HSA_FENCE_SCOPE_SYSTEM
                 << HSA_PACKET_HEADER_SCRELEASE_FENCE_SCOPE);
  uint32_t published = initial_word;
  __atomic_store_n(&published, (uint32_t)header, __ATOMIC_RELEASE);

  int zero_dependencies = 1;
  for (size_t i = 0; i < 5; ++i) {
    zero_dependencies &= packet.dep_signal[i].handle == 0;
  }

  printf("barrier-and=64/8 invalid=%d initial=0x%08x published=0x%08x "
         "zero-dependencies=%d completion=0x%llx\n",
         HSA_PACKET_TYPE_INVALID, initial_word, published, zero_dependencies,
         (unsigned long long)packet.completion_signal.handle);
  return HSA_PACKET_TYPE_INVALID == 1 && initial_word == 0x00000001u &&
                 header == 0x1403 && published == 0x00001403u &&
                 zero_dependencies &&
                 packet.completion_signal.handle == UINT64_C(0x3040)
             ? 0
             : 1;
}

#include <stddef.h>
#include <stdint.h>
#include <stdio.h>
#include <string.h>

#include "hsa.h"

int main(void) {
  _Static_assert(sizeof(hsa_barrier_and_packet_t) == 64, "packet size");
  _Static_assert(_Alignof(hsa_barrier_and_packet_t) == 8, "packet alignment");
  _Static_assert(offsetof(hsa_barrier_and_packet_t, dep_signal) == 8, "dependencies");
  _Static_assert(offsetof(hsa_barrier_and_packet_t, completion_signal) == 56, "completion");
  hsa_barrier_and_packet_t packet;
  memset(&packet, 0, sizeof(packet));
  packet.header = HSA_PACKET_TYPE_INVALID << HSA_PACKET_HEADER_TYPE;
  packet.dep_signal[0].handle = UINT64_C(0x2040);
  packet.dep_signal[1].handle = UINT64_C(0x2080);
  packet.completion_signal.handle = UINT64_C(0x3040);
  const unsigned char *bytes = (const unsigned char *)&packet;
  const unsigned char expected[64] = {
      [0] = 1, [8] = 0x40, [9] = 0x20, [16] = 0x80,
      [17] = 0x20, [56] = 0x40, [57] = 0x30};
  for (size_t i = 0; i < sizeof(packet); ++i) {
    printf("%02x", bytes[i]);
  }
  puts("");
  uint16_t header = (HSA_PACKET_TYPE_BARRIER_AND << HSA_PACKET_HEADER_TYPE) |
                    (1 << HSA_PACKET_HEADER_BARRIER) |
                    (HSA_FENCE_SCOPE_SYSTEM << HSA_PACKET_HEADER_SCACQUIRE_FENCE_SCOPE) |
                    (HSA_FENCE_SCOPE_SYSTEM << HSA_PACKET_HEADER_SCRELEASE_FENCE_SCOPE);
  printf("ordered-peer-barrier-header=0x%04x\n", header);
  return header == 0x1503 && memcmp(bytes, expected, sizeof(expected)) == 0 ? 0 : 1;
}

#include "device_properties.c"

#include <stdint.h>
#include <string.h>

static void set_architecture_features(hipDeviceProp_t *properties,
                                      uint64_t features) {
  properties->arch.hasGlobalInt32Atomics =
      (features & FE2O3_HIP_DEVICE_ARCH_HAS_GLOBAL_INT32_ATOMICS) != 0;
  properties->arch.hasSharedInt32Atomics =
      (features & FE2O3_HIP_DEVICE_ARCH_HAS_SHARED_INT32_ATOMICS) != 0;
  properties->arch.hasGlobalInt64Atomics =
      (features & FE2O3_HIP_DEVICE_ARCH_HAS_GLOBAL_INT64_ATOMICS) != 0;
  properties->arch.hasSharedInt64Atomics =
      (features & FE2O3_HIP_DEVICE_ARCH_HAS_SHARED_INT64_ATOMICS) != 0;
  properties->arch.hasWarpVote =
      (features & FE2O3_HIP_DEVICE_ARCH_HAS_WARP_VOTE) != 0;
  properties->arch.hasWarpBallot =
      (features & FE2O3_HIP_DEVICE_ARCH_HAS_WARP_BALLOT) != 0;
  properties->arch.hasWarpShuffle =
      (features & FE2O3_HIP_DEVICE_ARCH_HAS_WARP_SHUFFLE) != 0;
}

int main(void) {
  for (uint64_t expected = 0; expected <= UINT64_C(0x7f); ++expected) {
    hipDeviceProp_t properties;
    memset(&properties, 0, sizeof(properties));
    set_architecture_features(&properties, expected);
    if (architecture_features(&properties) != expected) {
      return 1;
    }
  }
  return 0;
}

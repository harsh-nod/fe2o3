#ifndef FE2O3_HIP_DEVICE_PROPERTIES_H
#define FE2O3_HIP_DEVICE_PROPERTIES_H

#include <stdint.h>

#define FE2O3_HIP_DEVICE_ARCH_HAS_GLOBAL_INT32_ATOMICS (UINT64_C(1) << 0)
#define FE2O3_HIP_DEVICE_ARCH_HAS_SHARED_INT32_ATOMICS (UINT64_C(1) << 1)
#define FE2O3_HIP_DEVICE_ARCH_HAS_GLOBAL_INT64_ATOMICS (UINT64_C(1) << 2)
#define FE2O3_HIP_DEVICE_ARCH_HAS_SHARED_INT64_ATOMICS (UINT64_C(1) << 3)
#define FE2O3_HIP_DEVICE_ARCH_HAS_WARP_VOTE (UINT64_C(1) << 4)
#define FE2O3_HIP_DEVICE_ARCH_HAS_WARP_BALLOT (UINT64_C(1) << 5)
#define FE2O3_HIP_DEVICE_ARCH_HAS_WARP_SHUFFLE (UINT64_C(1) << 6)

typedef struct Fe2o3HipDeviceProperties {
  char gcn_arch_name[256];
  int32_t warp_size;
  int32_t max_threads_per_block;
  int32_t max_block_dim[3];
  int32_t max_grid_dim[3];
  uint64_t shared_mem_per_block;
  uint64_t shared_mem_per_block_optin;
  uint64_t architecture_features;
} Fe2o3HipDeviceProperties;

int32_t fe2o3HipGetDeviceProperties(int32_t device_id,
                                    Fe2o3HipDeviceProperties *properties);

uint64_t fe2o3HipTestArchitectureFeatures(uint64_t requested_features);

#endif

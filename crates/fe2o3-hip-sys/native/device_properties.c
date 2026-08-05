#include "device_properties.h"

#include <stddef.h>
#include <string.h>

#include <hip/hip_runtime_api.h>
#include <hip/hip_version.h>

_Static_assert(sizeof(int) == sizeof(int32_t), "HIP int fields must be 32-bit");
_Static_assert(sizeof(hipError_t) == sizeof(int32_t),
               "hipError_t must have a 32-bit representation");
_Static_assert(sizeof(size_t) <= sizeof(uint64_t),
               "HIP size_t fields must fit in the stable result");
_Static_assert(sizeof(Fe2o3HipDeviceProperties) == 312,
               "unexpected device-property result size");
_Static_assert(_Alignof(Fe2o3HipDeviceProperties) == 8,
               "unexpected device-property result alignment");
_Static_assert(offsetof(Fe2o3HipDeviceProperties, gcn_arch_name) == 0,
               "unexpected gcn_arch_name offset");
_Static_assert(offsetof(Fe2o3HipDeviceProperties, warp_size) == 256,
               "unexpected warp_size offset");
_Static_assert(offsetof(Fe2o3HipDeviceProperties, max_threads_per_block) == 260,
               "unexpected max_threads_per_block offset");
_Static_assert(offsetof(Fe2o3HipDeviceProperties, max_block_dim) == 264,
               "unexpected max_block_dim offset");
_Static_assert(offsetof(Fe2o3HipDeviceProperties, max_grid_dim) == 276,
               "unexpected max_grid_dim offset");
_Static_assert(offsetof(Fe2o3HipDeviceProperties, shared_mem_per_block) == 288,
               "unexpected shared_mem_per_block offset");
_Static_assert(offsetof(Fe2o3HipDeviceProperties, shared_mem_per_block_optin) ==
                   296,
               "unexpected shared_mem_per_block_optin offset");
_Static_assert(offsetof(Fe2o3HipDeviceProperties, architecture_features) == 304,
               "unexpected architecture_features offset");

static uint64_t architecture_features(const hipDeviceProp_t *properties) {
  uint64_t features = 0;

  if (properties->arch.hasGlobalInt32Atomics) {
    features |= FE2O3_HIP_DEVICE_ARCH_HAS_GLOBAL_INT32_ATOMICS;
  }
  if (properties->arch.hasSharedInt32Atomics) {
    features |= FE2O3_HIP_DEVICE_ARCH_HAS_SHARED_INT32_ATOMICS;
  }
  if (properties->arch.hasGlobalInt64Atomics) {
    features |= FE2O3_HIP_DEVICE_ARCH_HAS_GLOBAL_INT64_ATOMICS;
  }
  if (properties->arch.hasSharedInt64Atomics) {
    features |= FE2O3_HIP_DEVICE_ARCH_HAS_SHARED_INT64_ATOMICS;
  }
  if (properties->arch.hasWarpVote) {
    features |= FE2O3_HIP_DEVICE_ARCH_HAS_WARP_VOTE;
  }
  if (properties->arch.hasWarpBallot) {
    features |= FE2O3_HIP_DEVICE_ARCH_HAS_WARP_BALLOT;
  }
  if (properties->arch.hasWarpShuffle) {
    features |= FE2O3_HIP_DEVICE_ARCH_HAS_WARP_SHUFFLE;
  }
  return features;
}

int32_t fe2o3HipGetDeviceProperties(int32_t device_id,
                                    Fe2o3HipDeviceProperties *properties) {
  hipDeviceProp_t hip_properties;
  hipError_t status;

  if (properties == NULL) {
    return (int32_t)hipErrorInvalidValue;
  }

  memset(properties, 0, sizeof(*properties));
  memset(&hip_properties, 0, sizeof(hip_properties));
  status = hipGetDeviceProperties(&hip_properties, (int)device_id);
  if (status != hipSuccess) {
    return (int32_t)status;
  }

  memcpy(properties->gcn_arch_name, hip_properties.gcnArchName,
         sizeof(properties->gcn_arch_name));
  properties->gcn_arch_name[sizeof(properties->gcn_arch_name) - 1] = '\0';
  properties->warp_size = (int32_t)hip_properties.warpSize;
  properties->max_threads_per_block =
      (int32_t)hip_properties.maxThreadsPerBlock;
  for (size_t index = 0; index < 3; ++index) {
    properties->max_block_dim[index] =
        (int32_t)hip_properties.maxThreadsDim[index];
    properties->max_grid_dim[index] =
        (int32_t)hip_properties.maxGridSize[index];
  }
  properties->shared_mem_per_block = (uint64_t)hip_properties.sharedMemPerBlock;
#if HIP_VERSION_MAJOR >= 6
  properties->shared_mem_per_block_optin =
      (uint64_t)hip_properties.sharedMemPerBlockOptin;
#else
  properties->shared_mem_per_block_optin = 0;
#endif
  properties->architecture_features = architecture_features(&hip_properties);

  return (int32_t)hipSuccess;
}

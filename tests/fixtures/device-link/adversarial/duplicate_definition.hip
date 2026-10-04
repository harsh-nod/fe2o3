#include "../device_ffi.h"

// Linking this module with hip/bidirectional.hip must reject the second strong
// definition of the exact same C ABI symbol.
extern "C" __device__ fe2o3_u32 external_scale_bias_v1(fe2o3_u32 value,
                                                        fe2o3_u32 lane) {
  return value + lane;
}

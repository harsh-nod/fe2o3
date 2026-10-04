#include "../device_ffi.h"

// The fixture runner intentionally compiles this definition for gfx90a. The
// source ABI is valid, making target identity the sole rejection reason.
extern "C" __device__ fe2o3_u32 external_scale_bias_v1(fe2o3_u32 value,
                                                        fe2o3_u32 lane) {
  return rust_accumulate_v1(value * 3u + 5u, lane);
}

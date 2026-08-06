#include <hip/hip_runtime.h>

#include "../device_ffi.h"

extern "C" __device__ fe2o3_u32 external_scale_bias_v1(fe2o3_u32 value,
                                                        fe2o3_u32 lane) {
  // Rust kernel result: 3 * value + 5 + lane, modulo 2^32.
  return rust_accumulate_v1(value * 3u + 5u, lane);
}

extern "C" __global__ void
hip_calls_rust_kernel_v1(const fe2o3_u32 *input, fe2o3_u32 *output,
                         fe2o3_u64 count) {
  const fe2o3_u64 lane =
      static_cast<fe2o3_u64>(blockIdx.x) * blockDim.x + threadIdx.x;
  if (lane < count) {
    // Observable result: 7 * input[lane] + 11 + lane, modulo 2^32.
    output[lane] = rust_accumulate_v1(input[lane] * 7u + 11u,
                                      static_cast<fe2o3_u32>(lane));
  }
}

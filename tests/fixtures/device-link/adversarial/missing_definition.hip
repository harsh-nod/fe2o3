#include <hip/hip_runtime.h>

#include "../device_ffi.h"

// This module deliberately imports both device functions and defines neither
// external_scale_bias_v1 nor a replacement for the Rust export.
extern "C" __global__ void
missing_external_definition_kernel_v1(const fe2o3_u32 *input,
                                      fe2o3_u32 *output) {
  if (threadIdx.x == 0) {
    output[0] = rust_accumulate_v1(external_scale_bias_v1(input[0], 0u), 0u);
  }
}

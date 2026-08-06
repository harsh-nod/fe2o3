#ifndef FE2O3_DEVICE_LINK_FIXTURE_DEVICE_FFI_H
#define FE2O3_DEVICE_LINK_FIXTURE_DEVICE_FFI_H

typedef unsigned int fe2o3_u32;
typedef unsigned long long fe2o3_u64;

static_assert(sizeof(fe2o3_u32) == 4, "fixture requires a 32-bit unsigned int");
static_assert(sizeof(fe2o3_u64) == 8, "fixture requires a 64-bit unsigned long long");

// Rust exports this exact C ABI device function.
extern "C" __device__ fe2o3_u32 rust_accumulate_v1(fe2o3_u32 value,
                                                    fe2o3_u32 lane);

// HIP exports this exact C ABI device function for the Rust kernel to import.
extern "C" __device__ fe2o3_u32 external_scale_bias_v1(fe2o3_u32 value,
                                                        fe2o3_u32 lane);

#endif

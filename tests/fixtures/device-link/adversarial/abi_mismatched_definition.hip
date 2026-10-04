typedef unsigned int fe2o3_u32;
typedef unsigned long long fe2o3_u64;

// The C symbol matches, but the required ABI is (u32, u32) -> u32.
extern "C" __device__ fe2o3_u64 external_scale_bias_v1(fe2o3_u64 value,
                                                        fe2o3_u32 lane) {
  return value + lane;
}

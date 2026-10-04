typedef unsigned int fe2o3_u32;

// The name matches the Rust import, but this is a kernel entry rather than the
// required scalar device function.
extern "C" __global__ void external_scale_bias_v1(fe2o3_u32 value,
                                                   fe2o3_u32 lane) {
  (void)value;
  (void)lane;
}

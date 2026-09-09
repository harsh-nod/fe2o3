#include <hip/hip_runtime.h>

#include "r60_pipeline_common.hpp"

#define HIP_CHECK(call)                                                        \
  do {                                                                        \
    const hipError_t checked_status = (call);                                  \
    if (checked_status != hipSuccess) {                                        \
      std::fprintf(stderr, "%s: %s\n", #call,                                 \
                    hipGetErrorString(checked_status));                        \
      fe2o3::r60::fail("HIP API error");                                       \
    }                                                                         \
  } while (false)

int main(int argc, char **argv) {
  using namespace fe2o3::r60;
  const auto config = parse_config(argc, argv);
  const auto code = load_hsaco(argv[1]);
  HIP_CHECK(hipSetDevice(config.device_index));
  hipUUID uuid{};
  hipDeviceProp_t properties{};
  HIP_CHECK(hipDeviceGetUuid(&uuid, config.device_index));
  HIP_CHECK(hipGetDeviceProperties(&properties, config.device_index));
  char expected_uuid[17]{};
  std::snprintf(expected_uuid, sizeof(expected_uuid), "%016llx",
                static_cast<unsigned long long>(config.unique_id));
  const char *xnack = std::strstr(properties.gcnArchName, ":xnack-");
  if (std::memcmp(uuid.bytes, expected_uuid, 16) != 0 ||
      std::strncmp(properties.gcnArchName, "gfx942", 6) != 0 ||
      (properties.gcnArchName[6] != '\0' && properties.gcnArchName[6] != ':') ||
      xnack == nullptr || (xnack[7] != '\0' && xnack[7] != ':'))
    fail("HIP UUID or gfx942:xnack- target mismatch");

  hipStream_t stream = nullptr;
  hipModule_t module = nullptr;
  hipFunction_t function = nullptr;
  HIP_CHECK(hipStreamCreateWithFlags(&stream, hipStreamNonBlocking));
  // Load exactly the bytes already hashed, with no second path read.
  HIP_CHECK(hipModuleLoadData(&module, code.data()));
  HIP_CHECK(hipModuleGetFunction(&function, module, "vecadd"));
  std::array<float *, 3> host{};
  std::array<float *, 3> device{};
  for (std::size_t i = 0; i < host.size(); ++i) {
    HIP_CHECK(hipHostMalloc(reinterpret_cast<void **>(&host[i]), kBytes,
                            hipHostMallocMapped | hipHostMallocCoherent));
    HIP_CHECK(hipHostGetDevicePointer(reinterpret_cast<void **>(&device[i]),
                                      host[i], 0));
    unsigned flags = 0;
    HIP_CHECK(hipHostGetFlags(&flags, host[i]));
    if ((flags & (hipHostMallocMapped | hipHostMallocCoherent)) !=
        (hipHostMallocMapped | hipHostMallocCoherent))
      fail("HIP host allocation did not retain mapped coherent flags");
  }
  const Workload workload;
  workload.initialize(host[0], host[1]);
  std::uint64_t length = kElements;
  void *arguments[] = {&device[0], &length, &device[1], &length,
                       &device[2], &length};
  std::array<Timings, kWarmups + kSamples> samples{};
  for (auto &sample : samples) {
    workload.reset(host[2]);
    const auto start = Clock::now();
    const auto deadline = start + std::chrono::nanoseconds(kTimeoutNs);
    for (std::size_t launch = 0; launch < kDepth; ++launch) {
      HIP_CHECK(hipModuleLaunchKernel(function, kElements / kWorkgroup, 1, 1,
                                      kWorkgroup, 1, 1, 0, stream, arguments,
                                      nullptr));
    }
    const auto issued = Clock::now();
    std::size_t polls = 0;
    for (;;) {
      const auto status = hipStreamQuery(stream);
      if (status == hipSuccess)
        break;
      if (status != hipErrorNotReady)
        HIP_CHECK(status);
      poll_deadline(deadline, polls++);
    }
    const auto done = Clock::now();
    sample = timings(start, issued, done);
    workload.validate(host[0], host[1], host[2]);
  }
  for (auto *allocation : host)
    HIP_CHECK(hipHostFree(allocation));
  HIP_CHECK(hipModuleUnload(module));
  HIP_CHECK(hipStreamDestroy(stream));
  report("hip", config, samples);
  return 0;
}

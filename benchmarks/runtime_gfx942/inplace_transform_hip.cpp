#include <hip/hip_runtime.h>

#include <cstdint>
#include <cstdio>
#include <cstring>
#include <vector>

#include "inplace_benchmark_common.hpp"

#define HIP_CHECK(call)                                                        \
  do {                                                                         \
    const hipError_t status_ = (call);                                         \
    if (status_ != hipSuccess) {                                               \
      std::fprintf(stderr, "%s failed: %s\n", #call,                           \
                   hipGetErrorString(status_));                                \
      std::exit(2);                                                            \
    }                                                                          \
  } while (false)

namespace {

bool uuid_matches(const hipUUID &uuid, std::uint64_t expected) {
  char ascii[17] = {};
  std::snprintf(ascii, sizeof(ascii), "%016llx",
                static_cast<unsigned long long>(expected));
  return std::memcmp(uuid.bytes, ascii, 16) == 0;
}

bool target_matches(const char *target) {
  if (std::strncmp(target, "gfx942", 6) != 0 ||
      (target[6] != '\0' && target[6] != ':'))
    return false;
  const char *xnack = std::strstr(target, ":xnack-");
  return xnack != nullptr && (xnack[7] == '\0' || xnack[7] == ':');
}

} // namespace

int main(int argc, char **argv) {
  static_assert(std::chrono::steady_clock::is_steady);
  if (argc != 4) {
    std::fprintf(stderr,
                 "usage: %s EXACT_HSACO VISIBLE_DEVICE_INDEX "
                 "EXPECTED_UNIQUE_ID\n",
                 argv[0]);
    return 2;
  }

  std::size_t parsed_device_index = 0;
  std::uint64_t expected_unique_id = 0;
  if (!fe2o3::r26::parse_index(argv[2], &parsed_device_index) ||
      parsed_device_index > static_cast<std::size_t>(INT32_MAX) ||
      !fe2o3::r26::parse_unique_id(argv[3], &expected_unique_id)) {
    std::fputs("invalid device index or expected unique ID\n", stderr);
    return 2;
  }
  const int device_index = static_cast<int>(parsed_device_index);

  HIP_CHECK(hipSetDevice(device_index));
  hipUUID uuid{};
  hipDeviceProp_t properties{};
  HIP_CHECK(hipDeviceGetUuid(&uuid, device_index));
  HIP_CHECK(hipGetDeviceProperties(&properties, device_index));
  if (!uuid_matches(uuid, expected_unique_id) ||
      !target_matches(properties.gcnArchName)) {
    std::fputs("HIP device UUID, unique ID, or target mismatch\n", stderr);
    return 2;
  }

  hipStream_t stream = nullptr;
  hipModule_t module = nullptr;
  hipFunction_t function = nullptr;
  std::uint32_t *upload = nullptr;
  std::uint32_t *download = nullptr;
  std::uint32_t *device = nullptr;
  HIP_CHECK(hipStreamCreateWithFlags(&stream, hipStreamNonBlocking));
  HIP_CHECK(hipModuleLoad(&module, argv[1]));
  HIP_CHECK(hipModuleGetFunction(&function, module, fe2o3::r26::kKernel));
  HIP_CHECK(
      hipHostMalloc(reinterpret_cast<void **>(&upload), fe2o3::r26::kBytes));
  HIP_CHECK(
      hipHostMalloc(reinterpret_cast<void **>(&download), fe2o3::r26::kBytes));
  HIP_CHECK(hipMalloc(reinterpret_cast<void **>(&device), fe2o3::r26::kBytes));

  std::vector<std::uint32_t> pattern_a;
  std::vector<std::uint32_t> pattern_b;
  fe2o3::r26::initialize_inputs(&pattern_a, &pattern_b);

  const auto launch = [&]() {
    std::uint64_t length = fe2o3::r26::kElements;
    void *arguments[] = {&device, &length};
    constexpr unsigned int blocks = static_cast<unsigned int>(
        fe2o3::r26::kElements / fe2o3::r26::kWorkgroup);
    HIP_CHECK(hipModuleLaunchKernel(function, blocks, 1, 1,
                                    fe2o3::r26::kWorkgroup, 1, 1, 0, stream,
                                    arguments, nullptr));
  };

  const auto iteration = [&](std::size_t ordinal) {
    const auto &input = (ordinal & 1U) == 0 ? pattern_a : pattern_b;
    std::memcpy(upload, input.data(), fe2o3::r26::kBytes);

    fe2o3::r26::Timings timings;
    const auto e2e_start = std::chrono::steady_clock::now();

    const auto h2d_start = std::chrono::steady_clock::now();
    HIP_CHECK(hipMemcpyAsync(device, upload, fe2o3::r26::kBytes,
                             hipMemcpyHostToDevice, stream));
    HIP_CHECK(hipStreamSynchronize(stream));
    const auto h2d_end = std::chrono::steady_clock::now();

    const auto compute_start = std::chrono::steady_clock::now();
    launch();
    HIP_CHECK(hipStreamSynchronize(stream));
    const auto compute_end = std::chrono::steady_clock::now();

    const auto d2h_start = std::chrono::steady_clock::now();
    HIP_CHECK(hipMemcpyAsync(download, device, fe2o3::r26::kBytes,
                             hipMemcpyDeviceToHost, stream));
    HIP_CHECK(hipStreamSynchronize(stream));
    const auto d2h_end = std::chrono::steady_clock::now();

    const auto e2e_end = std::chrono::steady_clock::now();
    timings.h2d_ns = fe2o3::r26::elapsed_ns(h2d_start, h2d_end);
    timings.compute_ns = fe2o3::r26::elapsed_ns(compute_start, compute_end);
    timings.d2h_ns = fe2o3::r26::elapsed_ns(d2h_start, d2h_end);
    timings.e2e_ns = fe2o3::r26::elapsed_ns(e2e_start, e2e_end);
    if (!fe2o3::r26::validate(download, input, ordinal, "HIP"))
      std::exit(3);
    return timings;
  };

  for (std::size_t warmup = 0; warmup < fe2o3::r26::kWarmups; ++warmup) {
    (void)iteration(warmup);
  }

  fe2o3::r26::Samples samples;
  std::size_t ordinal = fe2o3::r26::kWarmups;
  for (std::size_t sample = 0; sample < fe2o3::r26::kSamples; ++sample) {
    fe2o3::r26::Timings total;
    for (std::size_t inner = 0; inner < fe2o3::r26::kIterationsPerSample;
         ++inner, ++ordinal) {
      const auto observed = iteration(ordinal);
      total.h2d_ns += observed.h2d_ns;
      total.compute_ns += observed.compute_ns;
      total.d2h_ns += observed.d2h_ns;
      total.e2e_ns += observed.e2e_ns;
    }
    fe2o3::r26::append_average(&samples, total);
  }

  HIP_CHECK(hipFree(device));
  HIP_CHECK(hipHostFree(download));
  HIP_CHECK(hipHostFree(upload));
  HIP_CHECK(hipModuleUnload(module));
  HIP_CHECK(hipStreamDestroy(stream));
  fe2o3::r26::report("hip", "n/a", "host-staged-one-buffer", "n/a",
                     parsed_device_index, expected_unique_id, samples);
  return 0;
}

#include <hip/hip_runtime.h>

#include <algorithm>
#include <cerrno>
#include <chrono>
#include <cstddef>
#include <cstdint>
#include <cstdio>
#include <cstdlib>
#include <cstring>
#include <limits>
#include <numeric>
#include <vector>

#define HIP_CHECK(call)                                                        \
  do {                                                                         \
    const hipError_t status = (call);                                          \
    if (status != hipSuccess) {                                                \
      std::fprintf(stderr, "%s failed: %s\n", #call,                         \
                   hipGetErrorString(status));                                 \
      return 1;                                                                \
    }                                                                          \
  } while (false)

namespace {

constexpr unsigned int kWorkgroupSize = 256;
constexpr std::size_t kQualifiedElementCount = 1048576;
constexpr unsigned int kQualifiedBlockCount = static_cast<unsigned int>(
    (kQualifiedElementCount + kWorkgroupSize - 1) / kWorkgroupSize);
constexpr std::size_t kQualifiedGlobalExtent =
    static_cast<std::size_t>(kQualifiedBlockCount) * kWorkgroupSize;
static_assert(kQualifiedGlobalExtent == kQualifiedElementCount);

bool parse_size(const char *name, const char *text, std::size_t *value) {
  errno = 0;
  char *end = nullptr;
  const unsigned long long parsed = std::strtoull(text, &end, 10);
  if (errno != 0 || end == text || *end != '\0' || parsed == 0 ||
      parsed > std::numeric_limits<std::size_t>::max()) {
    std::fprintf(stderr, "invalid %s=%s\n", name, text);
    return false;
  }
  *value = static_cast<std::size_t>(parsed);
  return true;
}

double percentile(const std::vector<double> &sorted, std::size_t percentile) {
  return sorted[(sorted.size() - 1) * percentile / 100];
}

void report(const char *metric, std::vector<double> microseconds,
            std::size_t length, std::size_t launches_per_sample) {
  std::sort(microseconds.begin(), microseconds.end());
  const double mean =
      std::accumulate(microseconds.begin(), microseconds.end(), 0.0) /
      static_cast<double>(microseconds.size());
  std::printf(
      "backend=hip metric=%s n=%zu samples=%zu launches_per_sample=%zu "
      "min_us=%.3f p50_us=%.3f mean_us=%.3f p90_us=%.3f max_us=%.3f\n",
      metric, length, microseconds.size(), launches_per_sample,
      microseconds.front(), percentile(microseconds, 50), mean,
      percentile(microseconds, 90), microseconds.back());
}

} // namespace

int main(int argc, char **argv) {
  if (argc != 6) {
    std::fprintf(stderr,
                 "usage: %s EXACT_HSACO N WARMUPS SAMPLES "
                 "LAUNCHES_PER_SAMPLE\n",
                 argv[0]);
    return 2;
  }

  std::size_t length = 0;
  std::size_t warmups = 0;
  std::size_t samples = 0;
  std::size_t launches_per_sample = 0;
  if (!parse_size("N", argv[2], &length) ||
      !parse_size("WARMUPS", argv[3], &warmups) ||
      !parse_size("SAMPLES", argv[4], &samples) ||
      !parse_size("LAUNCHES_PER_SAMPLE", argv[5], &launches_per_sample) ||
      length != kQualifiedElementCount ||
      length > std::numeric_limits<unsigned int>::max() - kWorkgroupSize) {
    if (length != 0 && length != kQualifiedElementCount) {
      std::fprintf(stderr, "N must equal the qualified fixture length %zu\n",
                   kQualifiedElementCount);
    }
    return 2;
  }

  const std::size_t bytes = length * sizeof(float);
  if (bytes / sizeof(float) != length) {
    std::fputs("vector byte length overflow\n", stderr);
    return 2;
  }

  constexpr std::uint32_t kInitialOutputBits = 0x7fc00000;
  float initial_output = 0.0F;
  static_assert(sizeof(initial_output) == sizeof(kInitialOutputBits));
  std::memcpy(&initial_output, &kInitialOutputBits, sizeof(initial_output));
  std::vector<float> a(length);
  std::vector<float> b(length);
  const std::vector<float> initial_c(length, initial_output);
  std::vector<float> c(length, initial_output);
  for (std::size_t index = 0; index < length; ++index) {
    a[index] = static_cast<float>(index % 1024) / 2.0F;
    b[index] = static_cast<float>(index % 256) / 4.0F;
  }

  HIP_CHECK(hipSetDevice(0));
  hipStream_t stream = nullptr;
  hipModule_t module = nullptr;
  hipFunction_t function = nullptr;
  float *device_a = nullptr;
  float *device_b = nullptr;
  float *device_c = nullptr;
  HIP_CHECK(hipStreamCreateWithFlags(&stream, hipStreamNonBlocking));
  HIP_CHECK(hipModuleLoad(&module, argv[1]));
  HIP_CHECK(hipModuleGetFunction(&function, module, "vecadd"));
  HIP_CHECK(hipMalloc(&device_a, bytes));
  HIP_CHECK(hipMalloc(&device_b, bytes));
  HIP_CHECK(hipMalloc(&device_c, bytes));

  const auto launch = [&]() -> hipError_t {
    std::uint64_t argument_length = length;
    void *arguments[] = {&device_a, &argument_length, &device_b,
                         &argument_length, &device_c, &argument_length};
    return hipModuleLaunchKernel(function, kQualifiedBlockCount, 1, 1,
                                 kWorkgroupSize, 1, 1, 0, stream, arguments,
                                 nullptr);
  };

  const auto staged_iteration = [&]() -> hipError_t {
    hipError_t status =
        hipMemcpyAsync(device_a, a.data(), bytes, hipMemcpyHostToDevice, stream);
    if (status != hipSuccess) {
      return status;
    }
    status =
        hipMemcpyAsync(device_b, b.data(), bytes, hipMemcpyHostToDevice, stream);
    if (status != hipSuccess) {
      return status;
    }
    status =
        hipMemcpyAsync(device_c, initial_c.data(), bytes, hipMemcpyHostToDevice, stream);
    if (status != hipSuccess) {
      return status;
    }
    status = launch();
    if (status != hipSuccess) {
      return status;
    }
    status =
        hipMemcpyAsync(c.data(), device_c, bytes, hipMemcpyDeviceToHost, stream);
    if (status != hipSuccess) {
      return status;
    }
    return hipStreamSynchronize(stream);
  };
  const auto validate_output = [&]() -> bool {
    for (std::size_t index = 0; index < length; ++index) {
      const float expected = a[index] + b[index];
      if (c[index] != expected) {
        std::fprintf(
            stderr,
            "HIP result mismatch at %zu: expected %.9g, observed %.9g\n",
            index, expected, c[index]);
        return false;
      }
    }
    return true;
  };

  for (std::size_t warmup = 0; warmup < warmups; ++warmup) {
    HIP_CHECK(staged_iteration());
  }

  std::vector<double> staged_microseconds;
  staged_microseconds.reserve(samples);
  for (std::size_t sample = 0; sample < samples; ++sample) {
    const auto start = std::chrono::steady_clock::now();
    for (std::size_t iteration = 0; iteration < launches_per_sample;
         ++iteration) {
      HIP_CHECK(staged_iteration());
    }
    const auto stop = std::chrono::steady_clock::now();
    const double elapsed =
        std::chrono::duration<double, std::micro>(stop - start).count();
    staged_microseconds.push_back(elapsed /
                                  static_cast<double>(launches_per_sample));
    if (!validate_output()) {
      return 1;
    }
  }
  report("staged_submit_wait_readback", std::move(staged_microseconds), length,
         launches_per_sample);

  std::vector<double> synchronized_microseconds;
  synchronized_microseconds.reserve(samples);
  for (std::size_t sample = 0; sample < samples; ++sample) {
    double elapsed_microseconds = 0.0;
    for (std::size_t iteration = 0; iteration < launches_per_sample;
         ++iteration) {
      HIP_CHECK(hipMemcpyAsync(device_c, initial_c.data(), bytes,
                               hipMemcpyHostToDevice, stream));
      HIP_CHECK(hipStreamSynchronize(stream));
      const auto start = std::chrono::steady_clock::now();
      HIP_CHECK(launch());
      HIP_CHECK(hipStreamSynchronize(stream));
      const auto stop = std::chrono::steady_clock::now();
      elapsed_microseconds +=
          std::chrono::duration<double, std::micro>(stop - start).count();
    }
    synchronized_microseconds.push_back(
        elapsed_microseconds / static_cast<double>(launches_per_sample));
    HIP_CHECK(
        hipMemcpyAsync(c.data(), device_c, bytes, hipMemcpyDeviceToHost, stream));
    HIP_CHECK(hipStreamSynchronize(stream));
    if (!validate_output()) {
      return 1;
    }
  }
  report("synchronized_launch_wait", std::move(synchronized_microseconds),
         length, launches_per_sample);

  hipEvent_t start_event = nullptr;
  hipEvent_t stop_event = nullptr;
  HIP_CHECK(hipEventCreate(&start_event));
  HIP_CHECK(hipEventCreate(&stop_event));
  std::vector<double> event_microseconds;
  event_microseconds.reserve(samples);
  for (std::size_t sample = 0; sample < samples; ++sample) {
    HIP_CHECK(hipMemcpyAsync(device_c, initial_c.data(), bytes,
                             hipMemcpyHostToDevice, stream));
    HIP_CHECK(hipStreamSynchronize(stream));
    HIP_CHECK(hipEventRecord(start_event, stream));
    HIP_CHECK(launch());
    HIP_CHECK(hipEventRecord(stop_event, stream));
    HIP_CHECK(hipEventSynchronize(stop_event));
    float milliseconds = 0.0F;
    HIP_CHECK(hipEventElapsedTime(&milliseconds, start_event, stop_event));
    event_microseconds.push_back(static_cast<double>(milliseconds) * 1000.0);
    HIP_CHECK(
        hipMemcpyAsync(c.data(), device_c, bytes, hipMemcpyDeviceToHost, stream));
    HIP_CHECK(hipStreamSynchronize(stream));
    if (!validate_output()) {
      return 1;
    }
  }
  report("device_event_interval", std::move(event_microseconds), length, 1);

  HIP_CHECK(hipEventDestroy(stop_event));
  HIP_CHECK(hipEventDestroy(start_event));
  HIP_CHECK(hipFree(device_c));
  HIP_CHECK(hipFree(device_b));
  HIP_CHECK(hipFree(device_a));
  HIP_CHECK(hipModuleUnload(module));
  HIP_CHECK(hipStreamDestroy(stream));
  std::printf("backend=hip validation=exact status=passed n=%zu\n", length);
  return 0;
}

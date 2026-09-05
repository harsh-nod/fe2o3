#ifndef FE2O3_RUNTIME_GFX942_INPLACE_BENCHMARK_COMMON_HPP
#define FE2O3_RUNTIME_GFX942_INPLACE_BENCHMARK_COMMON_HPP

#include <algorithm>
#include <cerrno>
#include <chrono>
#include <cstddef>
#include <cstdint>
#include <cstdio>
#include <cstdlib>
#include <limits>
#include <string>
#include <vector>

namespace fe2o3::r26 {

constexpr std::size_t kElements = 262144;
constexpr std::size_t kBytes = kElements * sizeof(std::uint32_t);
constexpr std::uint32_t kWorkgroup = 256;
constexpr std::size_t kWarmups = 10;
constexpr std::size_t kSamples = 30;
constexpr std::size_t kIterationsPerSample = 10;
constexpr std::size_t kValidatedIterations =
    kWarmups + kSamples * kIterationsPerSample;
constexpr std::size_t kPatternAIterations = kValidatedIterations / 2;
constexpr std::size_t kPatternBIterations = kValidatedIterations / 2;

constexpr char kSchema[] = "fe2o3.r26-inplace-benchmark.v1";
constexpr char kKernel[] = "inplace_transform";
constexpr char kKernelDescriptor[] = "inplace_transform.kd";
constexpr std::uint32_t kHsaKernargAlignment = 16;
constexpr char kInputASha256[] =
    "ce96f8d88572648c07a6c03d7ce49af52c637af65267645eafdd2193ee6e49b7";
constexpr char kOutputASha256[] =
    "4a42778046c60e35849ad35fe4dc4bf39a0a4d616b75c9e62d146dbdb41ec960";
constexpr char kInputBSha256[] =
    "061cc02d1e9f513366e292544724ef6592b6ca4f59cfb2464a29bd94ff71236e";
constexpr char kOutputBSha256[] =
    "49f9da5c37cd051649cf257f528b1b573b44a1937b865b05643823267579cf62";

struct Timings {
  std::uint64_t h2d_ns = 0;
  std::uint64_t compute_ns = 0;
  std::uint64_t d2h_ns = 0;
  std::uint64_t e2e_ns = 0;
};

struct Samples {
  std::vector<std::uint64_t> h2d;
  std::vector<std::uint64_t> compute;
  std::vector<std::uint64_t> d2h;
  std::vector<std::uint64_t> e2e;

  Samples() {
    h2d.reserve(kSamples);
    compute.reserve(kSamples);
    d2h.reserve(kSamples);
    e2e.reserve(kSamples);
  }
};

inline bool parse_index(const char *text, std::size_t *value) {
  if (text == nullptr || *text == '\0' || *text == '-')
    return false;
  char *end = nullptr;
  errno = 0;
  const unsigned long long parsed = std::strtoull(text, &end, 10);
  if (errno != 0 || end == text || *end != '\0' ||
      parsed > std::numeric_limits<std::size_t>::max()) {
    return false;
  }
  *value = static_cast<std::size_t>(parsed);
  return true;
}

inline bool parse_unique_id(const char *text, std::uint64_t *value) {
  if (text == nullptr || *text == '\0' || *text == '-')
    return false;
  char *end = nullptr;
  errno = 0;
  const unsigned long long parsed = std::strtoull(text, &end, 0);
  if (errno != 0 || end == text || *end != '\0' || parsed == 0)
    return false;
  *value = static_cast<std::uint64_t>(parsed);
  return true;
}

constexpr std::uint32_t input_value(std::size_t index, bool pattern_b) {
  const auto narrowed = static_cast<std::uint32_t>(index);
  return pattern_b ? (narrowed * UINT32_C(0x27d4eb2d)) ^ UINT32_C(0x5a5aa5a5)
                   : (narrowed * UINT32_C(0x045d9f3b)) ^ UINT32_C(0xa5a55a5a);
}

constexpr std::uint32_t expected_value(std::uint32_t input, std::size_t index) {
  const std::uint32_t rotated = (input << 13) | (input >> 19);
  return (rotated ^ UINT32_C(0x9e3779b9)) + static_cast<std::uint32_t>(index);
}

inline void initialize_inputs(std::vector<std::uint32_t> *pattern_a,
                              std::vector<std::uint32_t> *pattern_b) {
  pattern_a->resize(kElements);
  pattern_b->resize(kElements);
  for (std::size_t index = 0; index < kElements; ++index) {
    (*pattern_a)[index] = input_value(index, false);
    (*pattern_b)[index] = input_value(index, true);
  }
}

inline bool validate(const std::uint32_t *observed,
                     const std::vector<std::uint32_t> &input,
                     std::size_t iteration, const char *backend) {
  for (std::size_t index = 0; index < kElements; ++index) {
    const std::uint32_t expected = expected_value(input[index], index);
    if (observed[index] != expected) {
      std::fprintf(stderr,
                   "%s iteration %zu mismatch at element %zu: expected "
                   "0x%08x, observed 0x%08x\n",
                   backend, iteration, index, expected, observed[index]);
      return false;
    }
  }
  return true;
}

inline std::uint64_t elapsed_ns(std::chrono::steady_clock::time_point start,
                                std::chrono::steady_clock::time_point end) {
  const auto elapsed =
      std::chrono::duration_cast<std::chrono::nanoseconds>(end - start).count();
  if (elapsed <= 0) {
    std::fputs("steady-clock phase did not have positive duration\n", stderr);
    std::exit(3);
  }
  return static_cast<std::uint64_t>(elapsed);
}

inline void append_average(Samples *samples, const Timings &total) {
  samples->h2d.push_back(total.h2d_ns / kIterationsPerSample);
  samples->compute.push_back(total.compute_ns / kIterationsPerSample);
  samples->d2h.push_back(total.d2h_ns / kIterationsPerSample);
  samples->e2e.push_back(total.e2e_ns / kIterationsPerSample);
}

inline void print_csv(const char *name,
                      const std::vector<std::uint64_t> &values) {
  std::printf(" %s=", name);
  for (std::size_t index = 0; index < values.size(); ++index) {
    std::printf("%s%llu", index == 0 ? "" : ",",
                static_cast<unsigned long long>(values[index]));
  }
}

inline void print_summary(const char *phase,
                          const std::vector<std::uint64_t> &values) {
  std::vector<std::uint64_t> sorted = values;
  std::sort(sorted.begin(), sorted.end());
  std::uint64_t total = 0;
  for (const std::uint64_t value : values)
    total += value;
  const std::size_t p50_rank = (sorted.size() + 1) / 2;
  const std::size_t p95_rank = (sorted.size() * 95 + 99) / 100;
  std::printf(" %s_min_ns=%llu %s_mean_ns=%llu %s_max_ns=%llu %s_p50_ns=%llu "
              "%s_p95_ns=%llu",
              phase, static_cast<unsigned long long>(sorted.front()), phase,
              static_cast<unsigned long long>(total / values.size()), phase,
              static_cast<unsigned long long>(sorted.back()), phase,
              static_cast<unsigned long long>(sorted[p50_rank - 1]), phase,
              static_cast<unsigned long long>(sorted[p95_rank - 1]));
}

inline void report(const char *backend, const char *promotion,
                   const char *data_path, const char *materializations,
                   std::size_t device_index, std::uint64_t unique_id,
                   const Samples &samples) {
  std::printf(
      "backend=%s schema=%s device_index=%zu unique_id=%016llx "
      "uuid=GPU-%016llx target=gfx942:xnack- xnack=disabled kernel=%s "
      "bytes=%zu elements=%zu workgroup=%u warmups=%zu samples=%zu "
      "iterations_per_sample=%zu "
      "sample_value=integer-average-ns-over-10-iterations "
      "trimming=none input_pattern=alternating-full-a-b pattern_start=a "
      "validation=every-element-every-iteration validated_iterations=%zu "
      "pattern_a_iterations=%zu pattern_b_iterations=%zu timing=host-monotonic "
      "interphase_control=e2e-h2d-compute-d2h promotion=%s data_path=%s "
      "user_data_materializations=%s "
      "input_a_sha256=%s output_a_sha256=%s input_b_sha256=%s "
      "output_b_sha256=%s",
      backend, kSchema, device_index,
      static_cast<unsigned long long>(unique_id),
      static_cast<unsigned long long>(unique_id), kKernel, kBytes, kElements,
      kWorkgroup, kWarmups, kSamples, kIterationsPerSample,
      kValidatedIterations, kPatternAIterations, kPatternBIterations, promotion,
      data_path, materializations, kInputASha256, kOutputASha256, kInputBSha256,
      kOutputBSha256);
  print_csv("h2d_samples_ns", samples.h2d);
  print_summary("h2d", samples.h2d);
  print_csv("compute_samples_ns", samples.compute);
  print_summary("compute", samples.compute);
  print_csv("d2h_samples_ns", samples.d2h);
  print_summary("d2h", samples.d2h);
  print_csv("e2e_samples_ns", samples.e2e);
  print_summary("e2e", samples.e2e);
  std::printf(
      " promotion_samples_ns=n/a promotion_min_ns=n/a promotion_mean_ns=n/a "
      "promotion_max_ns=n/a promotion_p50_ns=n/a promotion_p95_ns=n/a");
  std::putchar('\n');
}

} // namespace fe2o3::r26

#endif

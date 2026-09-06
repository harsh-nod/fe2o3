#ifndef FE2O3_RUNTIME_GFX942_STRIPED_COPY_BENCHMARK_COMMON_HPP
#define FE2O3_RUNTIME_GFX942_STRIPED_COPY_BENCHMARK_COMMON_HPP

#include "native_benchmark_args.hpp"

#include <algorithm>
#include <chrono>
#include <cstddef>
#include <cstdint>
#include <cstdio>
#include <cstring>
#include <limits>
#include <string>
#include <vector>

namespace fe2o3::r40 {

constexpr char kSchema[] = "fe2o3.async-copy-striped-benchmark.v2";
constexpr char kAssignment[] = "rotating-round-robin-v1";
constexpr char kSubmitOrder[] = "rotating-queue-major-v1";
constexpr std::size_t kMaximumBytes = 4U * 1024U * 1024U - 1024U;
constexpr std::size_t kMaximumDepth = 1008;
constexpr std::size_t kMaximumLogicalQueues = 16;

struct Config {
  int device_index = 0;
  std::uint64_t unique_id = 0;
  std::size_t bytes = 0;
  std::size_t depth = 0;
  std::size_t warmups = 0;
  std::size_t samples = 0;
  std::size_t rounds = 0;
  std::size_t transfer_bytes = 0;
  std::size_t logical_queue_count = 0;
  std::string profile;
  std::string workload_id;
};

inline bool admitted_profile(const std::string &profile,
                             std::size_t queue_count) {
  if (profile == "striped16")
    return queue_count == 16;
  constexpr std::size_t kCombinedCounts[] = {2, 4, 8, 14};
  for (const std::size_t count : kCombinedCounts) {
    if (queue_count == count &&
        profile == "combined-striped" + std::to_string(count))
      return true;
  }
  return false;
}

inline bool parse_config(int argc, char **argv, Config *config) {
  if (argc != 9 || config == nullptr)
    return false;
  fe2o3::runtime_gfx942::WorkloadShape shape;
  if (!fe2o3::runtime_gfx942::parse_device_index(argv[1],
                                                 &config->device_index) ||
      !fe2o3::runtime_gfx942::parse_unique_id(argv[2], &config->unique_id) ||
      config->unique_id == 0 ||
      !fe2o3::runtime_gfx942::parse_workload_shape(argv[3], argv[4], argv[5],
                                                   argv[6], 1, &shape) ||
      !fe2o3::runtime_gfx942::parse_size(argv[7], &config->logical_queue_count))
    return false;
  config->bytes = shape.bytes;
  config->depth = shape.depth;
  config->warmups = shape.warmups;
  config->samples = shape.samples;
  config->rounds = shape.total_iterations;
  config->transfer_bytes = shape.transfer_bytes;
  config->profile = argv[8];
  if (config->bytes > kMaximumBytes || config->depth > kMaximumDepth ||
      config->logical_queue_count == 0 ||
      config->depth % config->logical_queue_count != 0 ||
      !admitted_profile(config->profile, config->logical_queue_count))
    return false;
  config->workload_id =
      "bytes" + std::to_string(config->bytes) + "-q" +
      std::to_string(config->logical_queue_count) + "-" +
      (config->profile == "striped16" ? "standalone" : "combined");
  return true;
}

inline std::uint8_t round_pattern(std::size_t round, std::size_t request) {
  return static_cast<std::uint8_t>(
      (((round % 251) * 67 + (request % 251) * 29 + 1) % 251) + 1);
}

inline std::vector<std::size_t>
publication_order(std::size_t submission_ordinal, std::size_t depth,
                  std::size_t logical_queue_count) {
  std::vector<std::size_t> order;
  if (logical_queue_count == 0 || logical_queue_count > kMaximumLogicalQueues ||
      depth == 0 || depth > kMaximumDepth || depth % logical_queue_count != 0)
    return order;
  order.reserve(depth);
  const std::size_t origin = submission_ordinal % logical_queue_count;
  for (std::size_t lane_offset = 0; lane_offset < logical_queue_count;
       ++lane_offset) {
    const std::size_t lane = (origin + lane_offset) % logical_queue_count;
    for (std::size_t request = 0; request < depth; ++request) {
      if ((origin + request) % logical_queue_count == lane)
        order.push_back(request);
    }
  }
  return order;
}

inline std::uint64_t elapsed_ns(std::chrono::steady_clock::time_point start,
                                std::chrono::steady_clock::time_point end) {
  const auto elapsed =
      std::chrono::duration_cast<std::chrono::nanoseconds>(end - start).count();
  if (elapsed <= 0)
    return 0;
  return static_cast<std::uint64_t>(elapsed);
}

struct PhaseSamples {
  std::vector<std::uint64_t> submit;
  std::vector<std::uint64_t> wait;
  std::vector<std::uint64_t> e2e;

  explicit PhaseSamples(std::size_t count) {
    submit.reserve(count);
    wait.reserve(count);
    e2e.reserve(count);
  }

  bool append(std::uint64_t submit_ns, std::uint64_t wait_ns) {
    if (submit_ns == 0 || wait_ns == 0 ||
        submit_ns > std::numeric_limits<std::uint64_t>::max() - wait_ns)
      return false;
    submit.push_back(submit_ns);
    wait.push_back(wait_ns);
    e2e.push_back(submit_ns + wait_ns);
    return true;
  }
};

inline std::uint64_t percentile(const std::vector<std::uint64_t> &values,
                                std::size_t numerator,
                                std::size_t denominator) {
  std::vector<std::uint64_t> sorted = values;
  std::sort(sorted.begin(), sorted.end());
  const std::size_t rank =
      (sorted.size() * numerator + denominator - 1) / denominator;
  return sorted[rank - 1];
}

inline void print_vector(const char *name,
                         const std::vector<std::uint64_t> &values) {
  std::printf(" %s=", name);
  for (std::size_t index = 0; index < values.size(); ++index)
    std::printf("%s%llu", index == 0 ? "" : ",",
                static_cast<unsigned long long>(values[index]));
}

inline void print_phase(const char *direction, const PhaseSamples &phase,
                        std::size_t transfer_bytes) {
  const struct {
    const char *name;
    const std::vector<std::uint64_t> *values;
  } components[] = {
      {"submit", &phase.submit}, {"wait", &phase.wait}, {"e2e", &phase.e2e}};
  for (const auto &component : components) {
    const std::string stem = std::string(direction) + "_" + component.name;
    print_vector((stem + "_samples_ns").c_str(), *component.values);
    std::printf(
        " %s_p50_ns=%llu %s_p95_ns=%llu", stem.c_str(),
        static_cast<unsigned long long>(percentile(*component.values, 1, 2)),
        stem.c_str(),
        static_cast<unsigned long long>(percentile(*component.values, 19, 20)));
  }
  const std::uint64_t e2e_p50 = percentile(phase.e2e, 1, 2);
  const double gbps =
      static_cast<double>(transfer_bytes) / static_cast<double>(e2e_p50);
  std::printf(" %s_e2e_p50_GBps=%.9f", direction, gbps);
}

inline bool validate_buffers(const std::vector<std::uint8_t *> &buffers,
                             std::size_t bytes, std::size_t round,
                             const char *backend) {
  for (std::size_t request = 0; request < buffers.size(); ++request) {
    const std::uint8_t expected = round_pattern(round, request);
    const std::uint8_t *const observed = buffers[request];
    for (std::size_t offset = 0; offset < bytes; ++offset) {
      if (observed[offset] != expected) {
        std::fprintf(stderr,
                     "%s round %zu request %zu offset %zu: expected 0x%02x, "
                     "observed 0x%02x\n",
                     backend, round, request, offset,
                     static_cast<unsigned>(expected),
                     static_cast<unsigned>(observed[offset]));
        return false;
      }
    }
  }
  return true;
}

inline void report_native(const char *backend, const char *api,
                          const char *resource_profile,
                          const char *physical_engine_count,
                          const Config &config, const PhaseSamples &h2d,
                          const PhaseSamples &d2h) {
  std::printf(
      "backend=%s schema=%s workload_id=%s unique_id=%016llx bytes=%zu "
      "depth=%zu logical_queue_count=%zu per_queue_depth=%zu assignment=%s "
      "submit_order=%s direction=h2d-then-d2h warmups=%zu samples=%zu "
      "validation=full-buffer-every-round queue_creation_timed=no "
      "allocation_timed=no api=%s resource_profile=%s "
      "physical_engine_count=%s",
      backend, kSchema, config.workload_id.c_str(),
      static_cast<unsigned long long>(config.unique_id), config.bytes,
      config.depth, config.logical_queue_count,
      config.depth / config.logical_queue_count, kAssignment, kSubmitOrder,
      config.warmups, config.samples, api, resource_profile,
      physical_engine_count);
  print_phase("h2d", h2d, config.transfer_bytes);
  print_phase("d2h", d2h, config.transfer_bytes);
  std::putchar('\n');
}

} // namespace fe2o3::r40

#endif

#ifndef FE2O3_RUNTIME_GFX942_HSA_COPY_DIAGNOSTIC_HPP
#define FE2O3_RUNTIME_GFX942_HSA_COPY_DIAGNOSTIC_HPP

#include "native_benchmark_args.hpp"

#include <algorithm>
#include <cstddef>
#include <cstdint>
#include <cstring>
#include <vector>

namespace fe2o3::copy_diagnostic {

constexpr std::uint32_t kFine = 2;
constexpr std::uint32_t kCoarse = 4;
constexpr std::size_t kMaxBytes = 256 * 1024 * 1024;
constexpr std::size_t kMaxRounds = 10000;
constexpr const char *kSchema = "fe2o3.hsa-pool-engine-diagnostic.v1";
constexpr const char *kUsage =
    "usage: hsa-copy-pool-engine <gpu-index> <cpu-index> <bytes> <warmups> "
    "<samples> <expected-unique-id> <fine|coarse> <engine0|engine1>\n";

struct Config {
  std::size_t gpu_index = 0;
  std::size_t cpu_index = 0;
  runtime_gfx942::WorkloadShape workload;
  std::uint64_t unique_id = 0;
  std::uint32_t host_flags = 0;
  std::uint32_t engine_mask = 0;
};

inline bool parse_config(int argc, const char *const *argv, Config *result) {
  if (argc != 9 || argv == nullptr || result == nullptr)
    return false;
  for (int index = 1; index < argc; ++index)
    if (argv[index] == nullptr)
      return false;
  Config parsed;
  if (!runtime_gfx942::parse_size(argv[1], &parsed.gpu_index) ||
      !runtime_gfx942::parse_size(argv[2], &parsed.cpu_index) ||
      !runtime_gfx942::parse_workload_shape(argv[3], "1", argv[4], argv[5], 1,
                                            &parsed.workload) ||
      parsed.workload.bytes > kMaxBytes ||
      parsed.workload.total_iterations > kMaxRounds ||
      !runtime_gfx942::parse_unique_id(argv[6], &parsed.unique_id) ||
      parsed.unique_id == 0)
    return false;
  if (std::strcmp(argv[7], "fine") == 0)
    parsed.host_flags = kFine;
  else if (std::strcmp(argv[7], "coarse") == 0)
    parsed.host_flags = kCoarse;
  else
    return false;
  if (std::strcmp(argv[8], "engine0") == 0)
    parsed.engine_mask = 1;
  else if (std::strcmp(argv[8], "engine1") == 0)
    parsed.engine_mask = 2;
  else
    return false;
  *result = parsed;
  return true;
}

enum class Location { Cpu, Gpu, Other };

struct PoolFacts {
  std::uint64_t handle = 0;
  Location location = Location::Other;
  bool global = false;
  bool allocatable = false;
  std::uint32_t flags = 0;
  std::size_t maximum_bytes = 0;
  std::size_t granule = 0;
  std::size_t alignment = 0;
  std::uint32_t cpu_access = 0;
  std::uint32_t gpu_access = 0;
};

struct PoolSelection {
  std::size_t index = 0;
  std::size_t rounded_bytes = 0;
  std::size_t aggregate_bytes = 0;
};

inline bool accessible(std::uint32_t access) {
  return access == 1 || access == 2;
}

inline bool rounded_extent(std::size_t bytes, std::size_t granule,
                           std::size_t count, std::size_t *rounded,
                           std::size_t *aggregate) {
  if (bytes == 0 || granule == 0 || count == 0 || rounded == nullptr ||
      aggregate == nullptr)
    return false;
  std::size_t extent = bytes;
  const std::size_t remainder = bytes % granule;
  if (remainder != 0 &&
      !runtime_gfx942::checked_add(bytes, granule - remainder, &extent))
    return false;
  std::size_t total = 0;
  if (!runtime_gfx942::checked_multiply(extent, count, &total))
    return false;
  *rounded = extent;
  *aggregate = total;
  return true;
}

inline bool select_pool(const std::vector<PoolFacts> &pools, Location location,
                        std::uint32_t exact_flags, std::size_t bytes,
                        std::size_t allocation_count, PoolSelection *result) {
  if (result == nullptr || location == Location::Other ||
      (exact_flags != kFine && exact_flags != kCoarse) || bytes == 0 ||
      allocation_count == 0)
    return false;
  PoolSelection selected;
  std::size_t matches = 0;
  for (std::size_t index = 0; index < pools.size(); ++index) {
    const auto &pool = pools[index];
    std::size_t rounded = 0, aggregate = 0;
    if (pool.handle == 0 || pool.location != location || !pool.global ||
        !pool.allocatable || pool.flags != exact_flags ||
        !accessible(pool.cpu_access) || !accessible(pool.gpu_access) ||
        pool.alignment < alignof(std::uint32_t) ||
        (pool.alignment & (pool.alignment - 1)) != 0 ||
        !rounded_extent(bytes, pool.granule, allocation_count, &rounded,
                        &aggregate) ||
        aggregate > pool.maximum_bytes)
      continue;
    selected = {index, rounded, aggregate};
    ++matches;
  }
  if (matches != 1)
    return false;
  *result = selected;
  return true;
}

inline bool engine_available(std::uint32_t requested,
                             std::uint32_t h2d_available,
                             std::uint32_t d2h_available) {
  return (requested == 1 || requested == 2) &&
         (h2d_available & requested) == requested &&
         (d2h_available & requested) == requested;
}

inline bool exact_completion(std::int64_t signal_value) {
  return signal_value == 0;
}

inline std::uint8_t pattern(std::size_t round) {
  return static_cast<std::uint8_t>(((round % 251) * 67 + 1) % 251 + 1);
}

inline bool valid_buffer(const std::uint8_t *buffer, std::size_t bytes,
                         std::uint8_t expected) {
  return buffer != nullptr && bytes != 0 &&
         std::all_of(buffer, buffer + bytes, [expected](std::uint8_t byte) {
           return byte == expected;
         });
}

struct Timing {
  std::uint64_t submit_ns = 0;
  std::uint64_t wait_reset_ns = 0;
  std::uint64_t total_ns = 0;
};

inline bool timing(std::uint64_t start, std::uint64_t submitted,
                   std::uint64_t finished, Timing *result) {
  if (result == nullptr || submitted < start || finished < submitted ||
      finished == start)
    return false;
  *result = {submitted - start, finished - submitted, finished - start};
  return true;
}

} // namespace fe2o3::copy_diagnostic

#endif

#include <hip/hip_runtime.h>

#include "native_benchmark_args.hpp"

#include <algorithm>
#include <chrono>
#include <cstdint>
#include <cstdio>
#include <cstdlib>
#include <cstring>
#include <vector>

#define HIP_CHECK(call)                                                        \
  do {                                                                         \
    hipError_t status_ = (call);                                                \
    if (status_ != hipSuccess) {                                                \
      std::fprintf(stderr, "%s failed: %s\n", #call, hipGetErrorString(status_)); \
      std::exit(2);                                                             \
    }                                                                          \
  } while (0)

static uint64_t percentile(std::vector<uint64_t> values, size_t numerator,
                           size_t denominator) {
  std::sort(values.begin(), values.end());
  size_t rank = (values.size() * numerator + denominator - 1) / denominator;
  return values[rank - 1];
}

static double gbps(size_t bytes, uint64_t nanoseconds) {
  return static_cast<double>(bytes) / static_cast<double>(nanoseconds);
}

static uint8_t round_pattern(size_t round, size_t slot) {
  return static_cast<uint8_t>((round * 67 + slot * 29 + 1) % 251 + 1);
}

static bool uuid_matches(hipUUID uuid, uint64_t expected) {
  char ascii[17] = {};
  std::snprintf(ascii, sizeof(ascii), "%016llx",
                static_cast<unsigned long long>(expected));
  return std::memcmp(uuid.bytes, ascii, 16) == 0;
}

static bool target_matches(const char* target) {
  const char* xnack = std::strstr(target, ":xnack-");
  return std::strncmp(target, "gfx942:", 7) == 0 && xnack != nullptr &&
         (xnack[7] == '\0' || xnack[7] == ':') &&
         std::strstr(target, ":xnack+") == nullptr;
}

int main(int argc, char** argv) {
  const bool diagnostic =
      argc == 8 && std::strcmp(argv[7], "diagnostic-copy-only") == 0;
  if (argc != 7 && !diagnostic) {
    std::fprintf(stderr,
                 "usage: async-copy-hip <device-index> <bytes> <depth> <warmups> <samples> <expected-unique-id> [diagnostic-copy-only]\n");
    return 2;
  }
  int device_index = 0;
  uint64_t expected_unique_id = 0;
  fe2o3::runtime_gfx942::WorkloadShape workload;
  if (!fe2o3::runtime_gfx942::parse_device_index(argv[1], &device_index) ||
      !fe2o3::runtime_gfx942::parse_workload_shape(
          argv[2], argv[3], argv[4], argv[5], 1, &workload) ||
      !fe2o3::runtime_gfx942::parse_unique_id(argv[6],
                                              &expected_unique_id) ||
      expected_unique_id == 0)
    return 2;
  const size_t bytes = workload.bytes;
  const size_t depth = workload.depth;
  const size_t warmups = workload.warmups;
  const size_t samples = workload.samples;
  if (diagnostic && (depth != 1 || bytes > 256 * 1024 * 1024 ||
                     workload.total_iterations > 10000))
    return 2;
  struct Round {
    uint64_t h2d_ns;
    uint64_t d2h_ns;
  };
  // Bounded diagnostic storage is admitted before native setup. No output row
  // becomes a result until every round, validation and explicit release succeeds.
  std::vector<Round> rounds;
  if (diagnostic) rounds.reserve(workload.total_iterations);
  HIP_CHECK(hipSetDevice(device_index));
  hipUUID uuid{};
  hipDeviceProp_t properties{};
  HIP_CHECK(hipDeviceGetUuid(&uuid, device_index));
  HIP_CHECK(hipGetDeviceProperties(&properties, device_index));
  if (!uuid_matches(uuid, expected_unique_id) ||
      !target_matches(properties.gcnArchName)) {
    std::fprintf(stderr, "HIP device identity or target mismatch\n");
    return 2;
  }

  std::vector<hipStream_t> streams(depth);
  std::vector<uint8_t*> upload(depth), download(depth);
  std::vector<void*> device(depth);
  for (size_t i = 0; i < depth; ++i) {
    HIP_CHECK(hipStreamCreateWithFlags(&streams[i], hipStreamNonBlocking));
    HIP_CHECK(hipHostMalloc(reinterpret_cast<void**>(&upload[i]), bytes));
    HIP_CHECK(hipHostMalloc(reinterpret_cast<void**>(&download[i]), bytes));
    HIP_CHECK(hipMalloc(&device[i], bytes));
  }

  std::vector<uint64_t> h2d, d2h;
  if (!diagnostic) {
    h2d.reserve(samples);
    d2h.reserve(samples);
  }
  for (size_t iteration = 0; iteration < workload.total_iterations;
       ++iteration) {
    for (size_t i = 0; i < depth; ++i) {
      uint8_t value = round_pattern(iteration, i);
      std::memset(upload[i], value, bytes);
      std::memset(download[i], value ^ 0xff, bytes);
    }
    auto start = std::chrono::steady_clock::now();
    for (size_t i = 0; i < depth; ++i)
      HIP_CHECK(hipMemcpyAsync(device[i], upload[i], bytes, hipMemcpyHostToDevice,
                               streams[i]));
    for (auto stream : streams) HIP_CHECK(hipStreamSynchronize(stream));
    auto middle = std::chrono::steady_clock::now();
    for (size_t i = 0; i < depth; ++i)
      HIP_CHECK(hipMemcpyAsync(download[i], device[i], bytes, hipMemcpyDeviceToHost,
                               streams[i]));
    for (auto stream : streams) HIP_CHECK(hipStreamSynchronize(stream));
    auto end = std::chrono::steady_clock::now();
    for (size_t i = 0; i < depth; ++i) {
      uint8_t value = round_pattern(iteration, i);
      const auto* observed = download[i];
      if (!std::all_of(observed, observed + bytes,
                       [value](uint8_t byte) { return byte == value; }))
        return 3;
    }
    if (diagnostic) {
      const auto upload_ns =
          std::chrono::duration_cast<std::chrono::nanoseconds>(middle - start).count();
      const auto download_ns =
          std::chrono::duration_cast<std::chrono::nanoseconds>(end - middle).count();
      if (upload_ns <= 0 || download_ns <= 0) return 2;
      rounds.push_back({static_cast<uint64_t>(upload_ns),
                        static_cast<uint64_t>(download_ns)});
    } else if (iteration >= warmups) {
      h2d.push_back(std::chrono::duration_cast<std::chrono::nanoseconds>(middle - start).count());
      d2h.push_back(std::chrono::duration_cast<std::chrono::nanoseconds>(end - middle).count());
    }
  }
  uint64_t pool_ns = 0;
  if (!diagnostic) {
    constexpr size_t pool_iterations = 10000;
    auto pool_start = std::chrono::steady_clock::now();
    for (size_t i = 0; i < pool_iterations; ++i) {
      void* ptr = nullptr;
      HIP_CHECK(hipMallocAsync(&ptr, bytes, streams[0]));
      HIP_CHECK(hipFreeAsync(ptr, streams[0]));
    }
    HIP_CHECK(hipStreamSynchronize(streams[0]));
    auto pool_end = std::chrono::steady_clock::now();
    pool_ns = std::chrono::duration_cast<std::chrono::nanoseconds>(pool_end - pool_start).count() /
              pool_iterations;
  }

  for (size_t i = 0; i < depth; ++i) {
    HIP_CHECK(hipFree(device[i]));
    HIP_CHECK(hipHostFree(upload[i]));
    HIP_CHECK(hipHostFree(download[i]));
    HIP_CHECK(hipStreamDestroy(streams[i]));
  }
  if (diagnostic) {
    constexpr const char* schema = "fe2o3.hip-directional-copy-diagnostic.v1";
    std::printf(
        "schema=%s record=config device_index=%d unique_id=%016llx target=%s "
        "xnack=disabled bytes=%zu depth=1 warmups=%zu samples=%zu "
        "host_allocation=hipHostMallocDefault stream=nonblocking "
        "engine=runtime_selected allocator_benchmark=disabled\n",
        schema, device_index, static_cast<unsigned long long>(expected_unique_id),
        properties.gcnArchName, bytes, warmups, samples);
    for (size_t index = 0; index < rounds.size(); ++index) {
      std::printf(
          "schema=%s record=round index=%zu phase=%s pattern=%u checked_bytes=%zu "
          "h2d_total_ns=%llu d2h_total_ns=%llu\n",
          schema, index, index < warmups ? "warmup" : "sample",
          static_cast<unsigned>(round_pattern(index, 0)), bytes,
          static_cast<unsigned long long>(rounds[index].h2d_ns),
          static_cast<unsigned long long>(rounds[index].d2h_ns));
    }
    std::printf(
        "schema=%s record=complete validated_rounds=%zu measured_rounds=%zu "
        "allocations_released=3 streams_destroyed=1\n",
        schema, rounds.size(), samples);
    if (std::fflush(stdout) != 0 || std::ferror(stdout)) return 2;
    return 0;
  }
  uint64_t h2d_p50 = percentile(h2d, 1, 2), h2d_p95 = percentile(h2d, 19, 20);
  uint64_t d2h_p50 = percentile(d2h, 1, 2), d2h_p95 = percentile(d2h, 19, 20);
  std::printf(
      "backend=hip schema=fe2o3.async-copy-benchmark.v1 device_index=%d unique_id=%016llx target=%s xnack=disabled bytes=%zu depth=%zu warmups=%zu samples=%zu h2d_p50_ns=%llu h2d_p95_ns=%llu h2d_p50_GBps=%.3f d2h_p50_ns=%llu d2h_p95_ns=%llu d2h_p50_GBps=%.3f device_pool_alloc_free_pair_ns=%llu\n",
      device_index, static_cast<unsigned long long>(expected_unique_id),
      properties.gcnArchName, bytes, depth, warmups, samples,
      static_cast<unsigned long long>(h2d_p50),
      static_cast<unsigned long long>(h2d_p95),
      gbps(workload.transfer_bytes, h2d_p50),
      static_cast<unsigned long long>(d2h_p50),
      static_cast<unsigned long long>(d2h_p95),
      gbps(workload.transfer_bytes, d2h_p50),
      static_cast<unsigned long long>(pool_ns));
  return 0;
}

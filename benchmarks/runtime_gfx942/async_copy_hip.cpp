#include <hip/hip_runtime.h>

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
  return std::strncmp(target, "gfx942", 6) == 0 &&
         std::strstr(target, ":xnack-") != nullptr;
}

int main(int argc, char** argv) {
  if (argc != 7) {
    std::fprintf(stderr,
                 "usage: async-copy-hip <device-index> <bytes> <depth> <warmups> <samples> <expected-unique-id>\n");
    return 2;
  }
  int device_index = std::atoi(argv[1]);
  size_t bytes = std::strtoull(argv[2], nullptr, 10);
  size_t depth = std::strtoull(argv[3], nullptr, 10);
  size_t warmups = std::strtoull(argv[4], nullptr, 10);
  size_t samples = std::strtoull(argv[5], nullptr, 10);
  uint64_t expected_unique_id = std::strtoull(argv[6], nullptr, 0);
  if (bytes == 0 || depth == 0 || samples == 0 || expected_unique_id == 0) return 2;
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
  h2d.reserve(samples);
  d2h.reserve(samples);
  for (size_t iteration = 0; iteration < warmups + samples; ++iteration) {
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
    if (iteration >= warmups) {
      h2d.push_back(std::chrono::duration_cast<std::chrono::nanoseconds>(middle - start).count());
      d2h.push_back(std::chrono::duration_cast<std::chrono::nanoseconds>(end - middle).count());
    }
  }
  constexpr size_t pool_iterations = 10000;
  auto pool_start = std::chrono::steady_clock::now();
  for (size_t i = 0; i < pool_iterations; ++i) {
    void* ptr = nullptr;
    HIP_CHECK(hipMallocAsync(&ptr, bytes, streams[0]));
    HIP_CHECK(hipFreeAsync(ptr, streams[0]));
  }
  HIP_CHECK(hipStreamSynchronize(streams[0]));
  auto pool_end = std::chrono::steady_clock::now();
  uint64_t pool_ns =
      std::chrono::duration_cast<std::chrono::nanoseconds>(pool_end - pool_start).count() /
      pool_iterations;

  uint64_t h2d_p50 = percentile(h2d, 1, 2), h2d_p95 = percentile(h2d, 19, 20);
  uint64_t d2h_p50 = percentile(d2h, 1, 2), d2h_p95 = percentile(d2h, 19, 20);
  for (size_t i = 0; i < depth; ++i) {
    HIP_CHECK(hipFree(device[i]));
    HIP_CHECK(hipHostFree(upload[i]));
    HIP_CHECK(hipHostFree(download[i]));
    HIP_CHECK(hipStreamDestroy(streams[i]));
  }
  std::printf(
      "backend=hip schema=fe2o3.async-copy-benchmark.v1 device_index=%d unique_id=%016llx target=%s xnack=disabled bytes=%zu depth=%zu warmups=%zu samples=%zu h2d_p50_ns=%llu h2d_p95_ns=%llu h2d_p50_GBps=%.3f d2h_p50_ns=%llu d2h_p95_ns=%llu d2h_p50_GBps=%.3f device_pool_alloc_free_pair_ns=%llu\n",
      device_index, static_cast<unsigned long long>(expected_unique_id),
      properties.gcnArchName, bytes, depth, warmups, samples,
      static_cast<unsigned long long>(h2d_p50),
      static_cast<unsigned long long>(h2d_p95), gbps(bytes * depth, h2d_p50),
      static_cast<unsigned long long>(d2h_p50),
      static_cast<unsigned long long>(d2h_p95), gbps(bytes * depth, d2h_p50),
      static_cast<unsigned long long>(pool_ns));
  return 0;
}

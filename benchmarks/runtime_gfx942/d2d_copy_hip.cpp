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
      std::fprintf(stderr, "%s failed: %s\n", #call,                           \
                   hipGetErrorString(status_));                                 \
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
    std::fprintf(
        stderr,
        "usage: d2d-copy-hip <device-index> <bytes> <depth> <warmups> <samples> <expected-unique-id>\n");
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
  std::vector<uint8_t*> upload(depth), source_download(depth), download(depth);
  std::vector<void*> source(depth), destination(depth);
  for (size_t i = 0; i < depth; ++i) {
    HIP_CHECK(hipStreamCreateWithFlags(&streams[i], hipStreamNonBlocking));
    HIP_CHECK(hipHostMalloc(reinterpret_cast<void**>(&upload[i]), bytes));
    HIP_CHECK(
        hipHostMalloc(reinterpret_cast<void**>(&source_download[i]), bytes));
    HIP_CHECK(hipHostMalloc(reinterpret_cast<void**>(&download[i]), bytes));
    HIP_CHECK(hipMalloc(&source[i], bytes));
    HIP_CHECK(hipMalloc(&destination[i], bytes));
  }

  std::vector<uint64_t> d2d;
  d2d.reserve(samples);
  for (size_t iteration = 0; iteration < workload.total_iterations;
       ++iteration) {
    for (size_t i = 0; i < depth; ++i) {
      uint8_t value = round_pattern(iteration, i);
      std::memset(upload[i], value, bytes);
      std::memset(source_download[i], value ^ 0xff, bytes);
      std::memset(download[i], value ^ 0xff, bytes);
      HIP_CHECK(hipMemcpyAsync(source[i], upload[i], bytes,
                               hipMemcpyHostToDevice, streams[i]));
      HIP_CHECK(hipMemsetAsync(destination[i], value ^ 0xff, bytes,
                               streams[i]));
    }
    for (auto stream : streams) HIP_CHECK(hipStreamSynchronize(stream));

    auto start = std::chrono::steady_clock::now();
    for (size_t i = 0; i < depth; ++i)
      HIP_CHECK(hipMemcpyAsync(destination[i], source[i], bytes,
                               hipMemcpyDeviceToDevice, streams[i]));
    for (auto stream : streams) HIP_CHECK(hipStreamSynchronize(stream));
    auto end = std::chrono::steady_clock::now();

    for (size_t i = 0; i < depth; ++i)
      HIP_CHECK(hipMemcpyAsync(download[i], destination[i], bytes,
                               hipMemcpyDeviceToHost, streams[i]));
    for (size_t i = 0; i < depth; ++i)
      HIP_CHECK(hipMemcpyAsync(source_download[i], source[i], bytes,
                               hipMemcpyDeviceToHost, streams[i]));
    for (auto stream : streams) HIP_CHECK(hipStreamSynchronize(stream));
    for (size_t i = 0; i < depth; ++i) {
      uint8_t value = round_pattern(iteration, i);
      const auto* observed = download[i];
      if (!std::all_of(observed, observed + bytes,
                       [value](uint8_t byte) { return byte == value; }))
        return 3;
      const auto* source_observed = source_download[i];
      if (!std::all_of(source_observed, source_observed + bytes,
                       [value](uint8_t byte) { return byte == value; }))
        return 3;
    }
    if (iteration >= warmups) {
      d2d.push_back(
          std::chrono::duration_cast<std::chrono::nanoseconds>(end - start)
              .count());
    }
  }

  uint64_t d2d_p50 = percentile(d2d, 1, 2);
  uint64_t d2d_p95 = percentile(d2d, 19, 20);
  for (size_t i = 0; i < depth; ++i) {
    HIP_CHECK(hipFree(destination[i]));
    HIP_CHECK(hipFree(source[i]));
    HIP_CHECK(hipHostFree(download[i]));
    HIP_CHECK(hipHostFree(source_download[i]));
    HIP_CHECK(hipHostFree(upload[i]));
    HIP_CHECK(hipStreamDestroy(streams[i]));
  }
  std::printf(
      "backend=hip schema=fe2o3.d2d-copy-benchmark.v1 device_index=%d unique_id=%016llx target=%s xnack=disabled bytes=%zu depth=%zu warmups=%zu samples=%zu d2d_p50_ns=%llu d2d_p95_ns=%llu d2d_p50_GBps=%.3f\n",
      device_index, static_cast<unsigned long long>(expected_unique_id),
      properties.gcnArchName, bytes, depth, warmups, samples,
      static_cast<unsigned long long>(d2d_p50),
      static_cast<unsigned long long>(d2d_p95),
      gbps(workload.transfer_bytes, d2d_p50));
  return 0;
}

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

static uint8_t round_pattern(size_t round, size_t slot, size_t device) {
  return static_cast<uint8_t>((round * 67 + slot * 29 + device * 101 + 1) % 251 + 1);
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

static double gbps(size_t bytes, uint64_t nanoseconds) {
  return static_cast<double>(bytes) / static_cast<double>(nanoseconds);
}

int main(int argc, char** argv) {
  if (argc != 9) {
    std::fprintf(stderr,
                 "usage: async-copy-multi-hip <device-0> <device-1> <bytes> <depth-per-device> <warmups> <samples> <expected-unique-id-0> <expected-unique-id-1>\n");
    return 2;
  }
  int devices[2] = {};
  uint64_t expected_unique_ids[2] = {};
  fe2o3::runtime_gfx942::WorkloadShape workload;
  if (!fe2o3::runtime_gfx942::parse_device_index(argv[1], &devices[0]) ||
      !fe2o3::runtime_gfx942::parse_device_index(argv[2], &devices[1]) ||
      !fe2o3::runtime_gfx942::parse_workload_shape(
          argv[3], argv[4], argv[5], argv[6], 2, &workload) ||
      !fe2o3::runtime_gfx942::parse_unique_id(argv[7],
                                              &expected_unique_ids[0]) ||
      !fe2o3::runtime_gfx942::parse_unique_id(argv[8],
                                              &expected_unique_ids[1]) ||
      devices[0] == devices[1] ||
      expected_unique_ids[0] == 0 || expected_unique_ids[1] == 0 ||
      expected_unique_ids[0] == expected_unique_ids[1])
    return 2;
  const size_t bytes = workload.bytes;
  const size_t depth = workload.depth;
  const size_t warmups = workload.warmups;
  const size_t samples = workload.samples;

  const size_t total = workload.total_depth;
  std::vector<hipStream_t> streams(total);
  std::vector<uint8_t*> upload(total), download(total);
  std::vector<void*> device_memory(total);
  hipDeviceProp_t properties[2] = {};
  for (size_t device = 0; device < 2; ++device) {
    HIP_CHECK(hipSetDevice(devices[device]));
    hipUUID uuid{};
    HIP_CHECK(hipDeviceGetUuid(&uuid, devices[device]));
    HIP_CHECK(hipGetDeviceProperties(&properties[device], devices[device]));
    if (!uuid_matches(uuid, expected_unique_ids[device]) ||
        !target_matches(properties[device].gcnArchName))
      return 2;
    for (size_t i = 0; i < depth; ++i) {
      const size_t index = device * depth + i;
      HIP_CHECK(hipStreamCreateWithFlags(&streams[index], hipStreamNonBlocking));
      HIP_CHECK(hipHostMalloc(reinterpret_cast<void**>(&upload[index]), bytes));
      HIP_CHECK(hipHostMalloc(reinterpret_cast<void**>(&download[index]), bytes));
      HIP_CHECK(hipMalloc(&device_memory[index], bytes));
    }
  }

  std::vector<uint64_t> h2d, d2h;
  h2d.reserve(samples);
  d2h.reserve(samples);
  for (size_t iteration = 0; iteration < workload.total_iterations;
       ++iteration) {
    for (size_t device = 0; device < 2; ++device)
      for (size_t i = 0; i < depth; ++i) {
        const size_t index = device * depth + i;
        uint8_t value = round_pattern(iteration, i, device);
        std::memset(upload[index], value, bytes);
        std::memset(download[index], value ^ 0xff, bytes);
      }
    auto start = std::chrono::steady_clock::now();
    for (size_t device = 0; device < 2; ++device) {
      HIP_CHECK(hipSetDevice(devices[device]));
      for (size_t i = 0; i < depth; ++i) {
        const size_t index = device * depth + i;
        HIP_CHECK(hipMemcpyAsync(device_memory[index], upload[index], bytes,
                                 hipMemcpyHostToDevice, streams[index]));
      }
    }
    for (size_t device = 0; device < 2; ++device) {
      HIP_CHECK(hipSetDevice(devices[device]));
      for (size_t i = 0; i < depth; ++i)
        HIP_CHECK(hipStreamSynchronize(streams[device * depth + i]));
    }
    auto middle = std::chrono::steady_clock::now();
    for (size_t device = 0; device < 2; ++device) {
      HIP_CHECK(hipSetDevice(devices[device]));
      for (size_t i = 0; i < depth; ++i) {
        const size_t index = device * depth + i;
        HIP_CHECK(hipMemcpyAsync(download[index], device_memory[index], bytes,
                                 hipMemcpyDeviceToHost, streams[index]));
      }
    }
    for (size_t device = 0; device < 2; ++device) {
      HIP_CHECK(hipSetDevice(devices[device]));
      for (size_t i = 0; i < depth; ++i)
        HIP_CHECK(hipStreamSynchronize(streams[device * depth + i]));
    }
    auto end = std::chrono::steady_clock::now();
    for (size_t device = 0; device < 2; ++device)
      for (size_t i = 0; i < depth; ++i) {
        const size_t index = device * depth + i;
        uint8_t value = round_pattern(iteration, i, device);
        const auto* observed = download[index];
        if (!std::all_of(observed, observed + bytes,
                         [value](uint8_t byte) { return byte == value; }))
          return 3;
      }
    if (iteration >= warmups) {
      h2d.push_back(std::chrono::duration_cast<std::chrono::nanoseconds>(middle - start).count());
      d2h.push_back(std::chrono::duration_cast<std::chrono::nanoseconds>(end - middle).count());
    }
  }
  uint64_t h2d_p50 = percentile(h2d, 1, 2), h2d_p95 = percentile(h2d, 19, 20);
  uint64_t d2h_p50 = percentile(d2h, 1, 2), d2h_p95 = percentile(d2h, 19, 20);
  for (size_t device = 0; device < 2; ++device) {
    HIP_CHECK(hipSetDevice(devices[device]));
    for (size_t i = 0; i < depth; ++i) {
      const size_t index = device * depth + i;
      HIP_CHECK(hipFree(device_memory[index]));
      HIP_CHECK(hipHostFree(upload[index]));
      HIP_CHECK(hipHostFree(download[index]));
      HIP_CHECK(hipStreamDestroy(streams[index]));
    }
  }
  std::printf(
      "backend=hip schema=fe2o3.async-copy-multi-device-benchmark.v1 devices=2 device_indices=%d,%d unique_ids=%016llx,%016llx targets=%s,%s xnack=disabled host_context=single-thread-device-switching bytes=%zu depth_per_device=%zu warmups=%zu samples=%zu h2d_p50_ns=%llu h2d_p95_ns=%llu h2d_aggregate_p50_GBps=%.3f d2h_p50_ns=%llu d2h_p95_ns=%llu d2h_aggregate_p50_GBps=%.3f\n",
      devices[0], devices[1],
      static_cast<unsigned long long>(expected_unique_ids[0]),
      static_cast<unsigned long long>(expected_unique_ids[1]),
      properties[0].gcnArchName, properties[1].gcnArchName, bytes, depth,
      warmups, samples, static_cast<unsigned long long>(h2d_p50),
      static_cast<unsigned long long>(h2d_p95),
      gbps(workload.transfer_bytes, h2d_p50),
      static_cast<unsigned long long>(d2h_p50),
      static_cast<unsigned long long>(d2h_p95),
      gbps(workload.transfer_bytes, d2h_p50));
  return 0;
}

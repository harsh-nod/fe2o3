#include <hip/hip_runtime.h>

#include "native_benchmark_args.hpp"
#include "xgmi_peer_benchmark_common.hpp"

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
    }                                                                           \
  } while (0)

static uint64_t percentile(std::vector<uint64_t> values, size_t numerator,
                           size_t denominator) {
  std::sort(values.begin(), values.end());
  const size_t rank =
      (values.size() * numerator + denominator - 1) / denominator;
  return values[rank - 1];
}

static uint8_t pattern(size_t round, size_t slot, size_t direction) {
  return static_cast<uint8_t>(
      (round * 67 + slot * 29 + direction * 101 + 1) % 251 + 1);
}

static bool uuid_matches(hipUUID uuid, uint64_t expected) {
  char ascii[17] = {};
  std::snprintf(ascii, sizeof(ascii), "%016llx",
                static_cast<unsigned long long>(expected));
  return std::memcmp(uuid.bytes, ascii, 16) == 0;
}

static bool target_matches(const char *target) {
  return std::strncmp(target, "gfx942", 6) == 0 &&
         std::strstr(target, ":xnack-") != nullptr;
}

static double gbps(size_t bytes, uint64_t nanoseconds) {
  return static_cast<double>(bytes) / static_cast<double>(nanoseconds);
}

struct DirectionBuffers {
  std::vector<void *> source;
  std::vector<void *> destination;
  std::vector<hipStream_t> streams;
  std::vector<uint8_t> host;
};

static DirectionBuffers allocate_direction(int source_device,
                                           int destination_device,
                                           size_t bytes, size_t depth) {
  DirectionBuffers buffers;
  buffers.source.resize(depth);
  buffers.destination.resize(depth);
  buffers.streams.resize(depth);
  buffers.host.resize(bytes);
  for (size_t slot = 0; slot < depth; ++slot) {
    HIP_CHECK(hipSetDevice(source_device));
    HIP_CHECK(hipMalloc(&buffers.source[slot], bytes));
    HIP_CHECK(hipSetDevice(destination_device));
    HIP_CHECK(hipMalloc(&buffers.destination[slot], bytes));
    HIP_CHECK(hipStreamCreateWithFlags(&buffers.streams[slot],
                                       hipStreamNonBlocking));
  }
  return buffers;
}

static void release_direction(DirectionBuffers &buffers, int source_device,
                              int destination_device) {
  for (size_t slot = 0; slot < buffers.source.size(); ++slot) {
    HIP_CHECK(hipSetDevice(destination_device));
    HIP_CHECK(hipStreamDestroy(buffers.streams[slot]));
    HIP_CHECK(hipFree(buffers.destination[slot]));
    HIP_CHECK(hipSetDevice(source_device));
    HIP_CHECK(hipFree(buffers.source[slot]));
  }
}

static uint64_t run_direction(DirectionBuffers &buffers, int source_device,
                              int destination_device, size_t bytes,
                              size_t round, size_t direction) {
  for (size_t slot = 0; slot < buffers.source.size(); ++slot) {
    const uint8_t value = pattern(round, slot, direction);
    std::fill(buffers.host.begin(), buffers.host.end(), value);
    HIP_CHECK(hipSetDevice(source_device));
    HIP_CHECK(hipMemcpy(buffers.source[slot], buffers.host.data(), bytes,
                        hipMemcpyHostToDevice));
    HIP_CHECK(hipSetDevice(destination_device));
    HIP_CHECK(hipMemset(buffers.destination[slot], value ^ 0xff, bytes));
  }

  HIP_CHECK(hipSetDevice(destination_device));
  const auto start = std::chrono::steady_clock::now();
  for (size_t slot = 0; slot < buffers.source.size(); ++slot) {
    HIP_CHECK(hipMemcpyPeerAsync(
        buffers.destination[slot], destination_device, buffers.source[slot],
        source_device, bytes, buffers.streams[slot]));
  }
  for (hipStream_t stream : buffers.streams)
    HIP_CHECK(hipStreamSynchronize(stream));
  const auto end = std::chrono::steady_clock::now();

  for (size_t slot = 0; slot < buffers.source.size(); ++slot) {
    HIP_CHECK(hipMemcpy(buffers.host.data(), buffers.destination[slot], bytes,
                        hipMemcpyDeviceToHost));
    const uint8_t expected = pattern(round, slot, direction);
    if (!std::all_of(buffers.host.begin(), buffers.host.end(),
                     [expected](uint8_t byte) { return byte == expected; })) {
      std::fprintf(stderr,
                   "HIP XGMI peer mismatch at direction %zu round %zu slot %zu\n",
                   direction, round, slot);
      std::exit(3);
    }
  }
  return std::chrono::duration_cast<std::chrono::nanoseconds>(end - start)
      .count();
}

static bool prepare_persistent_hot(
    DirectionBuffers &buffers, int source_device, int destination_device,
    const fe2o3::runtime_gfx942::PeerBenchmarkControls &controls,
    size_t bytes, size_t direction) {
  const uint8_t value = fe2o3::runtime_gfx942::peer_pattern(
      controls.hot_pattern_round, 0, direction);
  if (!fe2o3::runtime_gfx942::fill_peer_guarded(
          buffers.host.data(), controls.allocation_bytes, bytes,
          fe2o3::runtime_gfx942::peer_source_canary(direction), value))
    return false;
  HIP_CHECK(hipSetDevice(source_device));
  HIP_CHECK(hipMemcpy(buffers.source[0], buffers.host.data(),
                      controls.allocation_bytes, hipMemcpyHostToDevice));

  if (!fe2o3::runtime_gfx942::fill_peer_guarded(
          buffers.host.data(), controls.allocation_bytes, bytes,
          fe2o3::runtime_gfx942::peer_destination_canary(direction),
          value ^ 0xff))
    return false;
  HIP_CHECK(hipSetDevice(destination_device));
  HIP_CHECK(hipMemcpy(buffers.destination[0], buffers.host.data(),
                      controls.allocation_bytes, hipMemcpyHostToDevice));
  return true;
}

static uint64_t copy_persistent_hot(
    DirectionBuffers &buffers, int source_device, int destination_device,
    const fe2o3::runtime_gfx942::PeerBenchmarkControls &controls,
    size_t bytes) {
  auto *source = static_cast<uint8_t *>(buffers.source[0]) + controls.copy_offset;
  auto *destination =
      static_cast<uint8_t *>(buffers.destination[0]) + controls.copy_offset;
  HIP_CHECK(hipSetDevice(destination_device));
  const auto start = std::chrono::steady_clock::now();
  HIP_CHECK(hipMemcpyPeerAsync(destination, destination_device, source,
                              source_device, bytes, buffers.streams[0]));
  HIP_CHECK(hipStreamSynchronize(buffers.streams[0]));
  const auto end = std::chrono::steady_clock::now();
  return std::chrono::duration_cast<std::chrono::nanoseconds>(end - start)
      .count();
}

static bool validate_persistent_hot(
    DirectionBuffers &buffers, int source_device, int destination_device,
    const fe2o3::runtime_gfx942::PeerBenchmarkControls &controls,
    size_t bytes, size_t direction) {
  const uint8_t expected = fe2o3::runtime_gfx942::peer_pattern(
      controls.hot_pattern_round, 0, direction);
  HIP_CHECK(hipSetDevice(source_device));
  HIP_CHECK(hipMemcpy(buffers.host.data(), buffers.source[0],
                      controls.allocation_bytes, hipMemcpyDeviceToHost));
  if (!fe2o3::runtime_gfx942::validate_peer_guarded(
          buffers.host.data(), controls.allocation_bytes, bytes,
          fe2o3::runtime_gfx942::peer_source_canary(direction), expected))
    return false;

  HIP_CHECK(hipSetDevice(destination_device));
  HIP_CHECK(hipMemcpy(buffers.host.data(), buffers.destination[0],
                      controls.allocation_bytes, hipMemcpyDeviceToHost));
  return fe2o3::runtime_gfx942::validate_peer_guarded(
      buffers.host.data(), controls.allocation_bytes, bytes,
      fe2o3::runtime_gfx942::peer_destination_canary(direction), expected);
}

int main(int argc, char **argv) {
  if (argc != 9 && argc != 10) {
    std::fprintf(stderr,
                 "usage: xgmi-peer-hip <device-0> <device-1> <bytes> <depth> <warmups> <samples> <expected-unique-id-0> <expected-unique-id-1> [--persistent-hot]\n");
    return 2;
  }
  int devices[2] = {};
  uint64_t unique_ids[2] = {};
  fe2o3::runtime_gfx942::WorkloadShape workload;
  fe2o3::runtime_gfx942::PeerBenchmarkControls controls;
  if (!fe2o3::runtime_gfx942::parse_device_index(argv[1], &devices[0]) ||
      !fe2o3::runtime_gfx942::parse_device_index(argv[2], &devices[1]) ||
      !fe2o3::runtime_gfx942::parse_workload_shape(
          argv[3], argv[4], argv[5], argv[6], 1, &workload) ||
      !fe2o3::runtime_gfx942::parse_unique_id(argv[7], &unique_ids[0]) ||
      !fe2o3::runtime_gfx942::parse_unique_id(argv[8], &unique_ids[1]) ||
      !fe2o3::runtime_gfx942::parse_peer_controls(
          argc == 10 ? argv[9] : nullptr, workload, &controls) ||
      devices[0] == devices[1] ||
      unique_ids[0] == 0 || unique_ids[1] == 0 ||
      unique_ids[0] == unique_ids[1])
    return 2;
  const size_t bytes = workload.bytes;
  const size_t depth = workload.depth;
  const size_t warmups = workload.warmups;
  const size_t samples = workload.samples;

  hipDeviceProp_t properties[2] = {};
  for (size_t index = 0; index < 2; ++index) {
    hipUUID uuid{};
    HIP_CHECK(hipDeviceGetUuid(&uuid, devices[index]));
    HIP_CHECK(hipGetDeviceProperties(&properties[index], devices[index]));
    if (!uuid_matches(uuid, unique_ids[index]) ||
        !target_matches(properties[index].gcnArchName))
      return 2;
  }
  int access_01 = 0, access_10 = 0;
  HIP_CHECK(hipDeviceCanAccessPeer(&access_01, devices[0], devices[1]));
  HIP_CHECK(hipDeviceCanAccessPeer(&access_10, devices[1], devices[0]));
  if (access_01 == 0 || access_10 == 0)
    return 2;
  for (size_t source = 0; source < 2; ++source) {
    HIP_CHECK(hipSetDevice(devices[source]));
    hipError_t status = hipDeviceEnablePeerAccess(devices[1 - source], 0);
    if (status != hipSuccess && status != hipErrorPeerAccessAlreadyEnabled)
      HIP_CHECK(status);
  }

  const size_t allocation_bytes =
      controls.persistent_hot ? controls.allocation_bytes : bytes;
  DirectionBuffers forward = allocate_direction(devices[0], devices[1],
                                                 allocation_bytes, depth);
  DirectionBuffers reverse = allocate_direction(devices[1], devices[0],
                                                 allocation_bytes, depth);
  std::vector<uint64_t> forward_samples, reverse_samples;
  forward_samples.reserve(samples);
  reverse_samples.reserve(samples);
  if (controls.persistent_hot) {
    const auto prepare = [&](size_t direction) {
      return direction == 0
                 ? prepare_persistent_hot(forward, devices[0], devices[1],
                                          controls, bytes, direction)
                 : prepare_persistent_hot(reverse, devices[1], devices[0],
                                          controls, bytes, direction);
    };
    const auto copy = [&](size_t direction) {
      return direction == 0
                 ? copy_persistent_hot(forward, devices[0], devices[1], controls,
                                       bytes)
                 : copy_persistent_hot(reverse, devices[1], devices[0], controls,
                                       bytes);
    };
    const auto validate = [&](size_t direction) {
      return direction == 0
                 ? validate_persistent_hot(forward, devices[0], devices[1],
                                           controls, bytes, direction)
                 : validate_persistent_hot(reverse, devices[1], devices[0],
                                           controls, bytes, direction);
    };
    const auto record_sample = [&](uint64_t forward_ns, uint64_t reverse_ns) {
      forward_samples.push_back(forward_ns);
      reverse_samples.push_back(reverse_ns);
    };
    const bool valid = fe2o3::runtime_gfx942::run_peer_persistent_hot(
        workload, prepare, copy, validate, record_sample);
    release_direction(reverse, devices[1], devices[0]);
    release_direction(forward, devices[0], devices[1]);
    if (!valid) {
      std::fputs("HIP XGMI persistent-hot payload or canary mismatch\n", stderr);
      return 3;
    }

    const uint64_t forward_p50 = percentile(forward_samples, 1, 2);
    const uint64_t forward_p95 = percentile(forward_samples, 19, 20);
    const uint64_t reverse_p50 = percentile(reverse_samples, 1, 2);
    const uint64_t reverse_p95 = percentile(reverse_samples, 19, 20);
    if (forward_p50 == 0 || reverse_p50 == 0)
      return 2;
    std::printf(
        "backend=hip schema=fe2o3.xgmi-peer-persistent-hot-benchmark.v1 surface=native-api devices=%d,%d unique_ids=%016llx,%016llx targets=%s,%s bytes=%zu depth=%zu warmups=%zu samples=%zu peer_access=enabled measurement=persistent-hot mapping_lifetime=process-persistent-hot prime_batches=1 direction=forward-then-reverse outstanding_depth=1 engine_parallelism=runtime-selected-unknown progress=peer-async-then-stream-synchronize timing=native-enqueue-through-observed-completion canaries=pass teardown=explicit forward_p50_ns=%llu forward_p95_ns=%llu forward_p50_GBps=%.3f reverse_p50_ns=%llu reverse_p95_ns=%llu reverse_p50_GBps=%.3f\n",
        devices[0], devices[1], static_cast<unsigned long long>(unique_ids[0]),
        static_cast<unsigned long long>(unique_ids[1]), properties[0].gcnArchName,
        properties[1].gcnArchName, bytes, depth, warmups, samples,
        static_cast<unsigned long long>(forward_p50),
        static_cast<unsigned long long>(forward_p95),
        gbps(workload.transfer_bytes, forward_p50),
        static_cast<unsigned long long>(reverse_p50),
        static_cast<unsigned long long>(reverse_p95),
        gbps(workload.transfer_bytes, reverse_p50));
    return 0;
  }

  for (size_t round = 0; round < workload.total_iterations; ++round) {
    const uint64_t forward_ns =
        run_direction(forward, devices[0], devices[1], bytes, round, 0);
    const uint64_t reverse_ns =
        run_direction(reverse, devices[1], devices[0], bytes, round, 1);
    if (round >= warmups) {
      forward_samples.push_back(forward_ns);
      reverse_samples.push_back(reverse_ns);
    }
  }
  release_direction(reverse, devices[1], devices[0]);
  release_direction(forward, devices[0], devices[1]);

  const uint64_t forward_p50 = percentile(forward_samples, 1, 2);
  const uint64_t forward_p95 = percentile(forward_samples, 19, 20);
  const uint64_t reverse_p50 = percentile(reverse_samples, 1, 2);
  const uint64_t reverse_p95 = percentile(reverse_samples, 19, 20);
  if (forward_p50 == 0 || reverse_p50 == 0)
    return 2;
  std::printf(
      "backend=hip schema=fe2o3.xgmi-peer-benchmark.v1 devices=%d,%d unique_ids=%016llx,%016llx targets=%s,%s bytes=%zu depth=%zu warmups=%zu samples=%zu peer_access=enabled forward_p50_ns=%llu forward_p95_ns=%llu forward_p50_GBps=%.3f reverse_p50_ns=%llu reverse_p95_ns=%llu reverse_p50_GBps=%.3f\n",
      devices[0], devices[1], static_cast<unsigned long long>(unique_ids[0]),
      static_cast<unsigned long long>(unique_ids[1]), properties[0].gcnArchName,
      properties[1].gcnArchName, bytes, depth, warmups, samples,
      static_cast<unsigned long long>(forward_p50),
      static_cast<unsigned long long>(forward_p95),
      gbps(workload.transfer_bytes, forward_p50),
      static_cast<unsigned long long>(reverse_p50),
      static_cast<unsigned long long>(reverse_p95),
      gbps(workload.transfer_bytes, reverse_p50));
  return 0;
}

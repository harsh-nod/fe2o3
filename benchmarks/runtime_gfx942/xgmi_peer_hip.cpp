#include <hip/hip_runtime.h>

#include "native_benchmark_args.hpp"
#include "xgmi_peer_benchmark_common.hpp"
#include "xgmi_peer_segments_common.hpp"

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
    const fe2o3::runtime_gfx942::PeerBenchmarkControls &controls, size_t bytes,
    size_t direction) {
  return fe2o3::runtime_gfx942::visit_peer_buffers(
      buffers.source.size(), [&](size_t slot, bool source) {
        const uint8_t value = fe2o3::runtime_gfx942::peer_pattern(
            controls.hot_pattern_round, slot, direction);
        const auto canary =
            source ? fe2o3::runtime_gfx942::peer_source_canary(direction)
                   : fe2o3::runtime_gfx942::peer_destination_canary(direction);
        if (!fe2o3::runtime_gfx942::fill_peer_guarded(
                buffers.host.data(), controls.allocation_bytes, bytes, canary,
                source ? value : static_cast<uint8_t>(value ^ 0xff)))
          return false;
        HIP_CHECK(hipSetDevice(source ? source_device : destination_device));
        HIP_CHECK(
            hipMemcpy(source ? buffers.source[slot] : buffers.destination[slot],
                      buffers.host.data(), controls.allocation_bytes,
                      hipMemcpyHostToDevice));
        return true;
      });
}

static uint64_t copy_persistent_hot(
    DirectionBuffers &buffers, int source_device, int destination_device,
    const fe2o3::runtime_gfx942::PeerBenchmarkControls &controls,
    size_t bytes) {
  HIP_CHECK(hipSetDevice(destination_device));
  const auto start = std::chrono::steady_clock::now();
  fe2o3::runtime_gfx942::run_peer_batch(
      buffers.source.size(),
      [&](size_t slot) {
        auto *source =
            static_cast<uint8_t *>(buffers.source[slot]) + controls.copy_offset;
        auto *destination = static_cast<uint8_t *>(buffers.destination[slot]) +
                            controls.copy_offset;
        HIP_CHECK(hipMemcpyPeerAsync(destination, destination_device, source,
                                     source_device, bytes,
                                     buffers.streams[slot]));
      },
      [&](size_t slot) {
        HIP_CHECK(hipStreamSynchronize(buffers.streams[slot]));
      });
  const auto end = std::chrono::steady_clock::now();
  return std::chrono::duration_cast<std::chrono::nanoseconds>(end - start)
      .count();
}

static bool validate_persistent_hot(
    DirectionBuffers &buffers, int source_device, int destination_device,
    const fe2o3::runtime_gfx942::PeerBenchmarkControls &controls, size_t bytes,
    size_t direction) {
  return fe2o3::runtime_gfx942::visit_peer_buffers(
      buffers.source.size(), [&](size_t slot, bool source) {
        const uint8_t expected = fe2o3::runtime_gfx942::peer_pattern(
            controls.hot_pattern_round, slot, direction);
        HIP_CHECK(hipSetDevice(source ? source_device : destination_device));
        HIP_CHECK(
            hipMemcpy(buffers.host.data(),
                      source ? buffers.source[slot] : buffers.destination[slot],
                      controls.allocation_bytes, hipMemcpyDeviceToHost));
        const auto canary =
            source ? fe2o3::runtime_gfx942::peer_source_canary(direction)
                   : fe2o3::runtime_gfx942::peer_destination_canary(direction);
        const bool valid = fe2o3::runtime_gfx942::validate_peer_guarded(
            buffers.host.data(), controls.allocation_bytes, bytes, canary,
            expected);
        if (!valid)
          std::fprintf(
              stderr,
              "HIP persistent-hot %s mismatch at direction %zu slot %zu\n",
              source ? "source" : "destination", direction, slot);
        return valid;
      });
}

namespace peer = fe2o3::runtime_gfx942;

struct OrderedHipDirection {
  void *source = nullptr;
  void *destination = nullptr;
  hipStream_t stream{};
};

static void ordered_hip(hipError_t status) {
  if (status != hipSuccess)
    peer::segment_fail(hipGetErrorString(status));
}

static void run_ordered_hip(const int devices[2], const uint64_t ids[2],
                            const peer::PeerSegmentPlan &plan) {
  OrderedHipDirection directions[2];
  for (size_t direction = 0; direction < 2; ++direction) {
    auto &buffers = directions[direction];
    const auto source = peer::segment_source(plan, direction);
    const auto poison = peer::segment_destination(plan, direction, false);
    ordered_hip(hipSetDevice(devices[direction]));
    ordered_hip(hipMalloc(&buffers.source, plan.band_bytes));
    ordered_hip(hipMemcpy(buffers.source, source.data(), source.size(), hipMemcpyHostToDevice));
    ordered_hip(hipSetDevice(devices[1 - direction]));
    ordered_hip(hipMalloc(&buffers.destination, plan.destination_bytes));
    ordered_hip(hipMemcpy(buffers.destination, poison.data(), poison.size(), hipMemcpyHostToDevice));
    ordered_hip(hipStreamCreateWithFlags(&buffers.stream, hipStreamNonBlocking));
  }
  peer::run_peer_segments("hip", "enqueue-list-stream-query", plan, ids,
      [&](size_t direction, size_t band) {
        auto &buffers = directions[direction];
        ordered_hip(hipSetDevice(devices[1 - direction]));
        const auto start = peer::PeerClock::now();
        const auto deadline = start + peer::peer_list_timeout;
        for (const auto &segment : plan.segments) {
          peer::require_segment_deadline(deadline);
          ordered_hip(hipMemcpyPeerAsync(
              static_cast<uint8_t *>(buffers.destination) + band * plan.band_bytes + segment.destination_offset,
              devices[1 - direction], static_cast<uint8_t *>(buffers.source) + segment.source_offset,
              devices[direction], segment.bytes, buffers.stream));
        }
        for (;;) {
          peer::require_segment_deadline(deadline);
          const auto status = hipStreamQuery(buffers.stream);
          if (status == hipSuccess)
            break;
          if (status != hipErrorNotReady)
            ordered_hip(status);
        }
        const auto end = peer::PeerClock::now();
        if (end >= deadline)
          peer::segment_fail("late HIP completion");
        return static_cast<uint64_t>(std::chrono::duration_cast<std::chrono::nanoseconds>(end - start).count());
      },
      [&](size_t direction) {
        auto &buffers = directions[direction];
        auto observed = peer::segment_source(plan, direction);
        const auto expected_source = observed;
        ordered_hip(hipSetDevice(devices[direction]));
        ordered_hip(hipMemcpy(observed.data(), buffers.source, observed.size(), hipMemcpyDeviceToHost));
        if (observed != expected_source)
          return false;
        const auto expected = peer::segment_destination(plan, direction, true);
        observed.resize(plan.destination_bytes);
        ordered_hip(hipSetDevice(devices[1 - direction]));
        ordered_hip(hipMemcpy(observed.data(), buffers.destination, observed.size(), hipMemcpyDeviceToHost));
        return observed == expected;
      },
      [&] {
        for (size_t i = 2; i-- > 0;) {
          ordered_hip(hipSetDevice(devices[1 - i]));
          ordered_hip(hipStreamDestroy(directions[i].stream));
          ordered_hip(hipFree(directions[i].destination));
          ordered_hip(hipSetDevice(devices[i]));
          ordered_hip(hipFree(directions[i].source));
        }
      });
}

int main(int argc, char **argv) {
  if (argc != 9 && argc != 10 && argc != 11) {
    std::fprintf(stderr,
                 "usage: xgmi-peer-hip <device-0> <device-1> <bytes> <depth> <warmups> <samples> <expected-unique-id-0> <expected-unique-id-1> [--persistent-hot | --ordered-segments <count>]\n");
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
  peer::PeerSegmentPlan segment_plan;
  if (argc == 11) {
    size_t count = 0;
    if (std::strcmp(argv[9], "--ordered-segments") != 0 || depth != 1 ||
        !peer::parse_size(argv[10], &count) ||
        !peer::make_peer_segment_plan(bytes, count, warmups, samples, &segment_plan))
      return 2;
  }

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

  if (argc == 11) {
    run_ordered_hip(devices, unique_ids, segment_plan);
    return 0;
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
        "backend=hip schema=fe2o3.xgmi-peer-persistent-hot-benchmark.v1 surface=native-api devices=%d,%d unique_ids=%016llx,%016llx targets=%s,%s bytes=%zu depth=%zu warmups=%zu samples=%zu peer_access=enabled measurement=persistent-hot mapping_lifetime=process-persistent-hot prime_batches=1 direction=forward-then-reverse outstanding_depth=%zu engine_parallelism=runtime-selected-unknown progress=peer-async-then-stream-synchronize timing=native-enqueue-through-observed-completion canaries=pass teardown=explicit forward_p50_ns=%llu forward_p95_ns=%llu forward_p50_GBps=%.3f reverse_p50_ns=%llu reverse_p95_ns=%llu reverse_p50_GBps=%.3f\n",
        devices[0], devices[1], static_cast<unsigned long long>(unique_ids[0]),
        static_cast<unsigned long long>(unique_ids[1]), properties[0].gcnArchName,
        properties[1].gcnArchName, bytes, depth, warmups, samples, depth,
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

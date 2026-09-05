#include <hsa/hsa.h>
#include <hsa/hsa_ext_amd.h>

#include "native_benchmark_args.hpp"

#include <algorithm>
#include <chrono>
#include <cstdint>
#include <cstdio>
#include <cstdlib>
#include <cstring>
#include <limits>
#include <vector>

#define HSA_CHECK(call)                                                        \
  do {                                                                         \
    hsa_status_t status_ = (call);                                              \
    if (status_ != HSA_STATUS_SUCCESS) {                                        \
      const char *text_ = nullptr;                                              \
      hsa_status_string(status_, &text_);                                       \
      std::fprintf(stderr, "%s failed: %s\n", #call,                           \
                   text_ ? text_ : "unknown");                                 \
      std::exit(2);                                                             \
    }                                                                           \
  } while (0)

struct Agents {
  std::vector<hsa_agent_t> cpus;
  std::vector<hsa_agent_t> gpus;
};

static hsa_status_t collect_agent(hsa_agent_t agent, void *data) {
  hsa_device_type_t type;
  hsa_status_t status =
      hsa_agent_get_info(agent, HSA_AGENT_INFO_DEVICE, &type);
  if (status != HSA_STATUS_SUCCESS)
    return status;
  auto *agents = static_cast<Agents *>(data);
  if (type == HSA_DEVICE_TYPE_CPU)
    agents->cpus.push_back(agent);
  if (type == HSA_DEVICE_TYPE_GPU)
    agents->gpus.push_back(agent);
  return HSA_STATUS_SUCCESS;
}

struct PoolSelection {
  bool want_gpu;
  bool found = false;
  hsa_amd_memory_pool_t pool{};
};

static hsa_status_t choose_pool(hsa_amd_memory_pool_t pool, void *data) {
  auto *selection = static_cast<PoolSelection *>(data);
  hsa_amd_segment_t segment;
  hsa_amd_memory_pool_location_t location;
  bool allowed = false;
  HSA_CHECK(hsa_amd_memory_pool_get_info(
      pool, HSA_AMD_MEMORY_POOL_INFO_SEGMENT, &segment));
  HSA_CHECK(hsa_amd_memory_pool_get_info(
      pool, HSA_AMD_MEMORY_POOL_INFO_LOCATION, &location));
  HSA_CHECK(hsa_amd_memory_pool_get_info(
      pool, HSA_AMD_MEMORY_POOL_INFO_RUNTIME_ALLOC_ALLOWED, &allowed));
  if (segment == HSA_AMD_SEGMENT_GLOBAL && allowed &&
      ((selection->want_gpu &&
        location == HSA_AMD_MEMORY_POOL_LOCATION_GPU) ||
       (!selection->want_gpu &&
        location == HSA_AMD_MEMORY_POOL_LOCATION_CPU))) {
    selection->pool = pool;
    selection->found = true;
    return HSA_STATUS_INFO_BREAK;
  }
  return HSA_STATUS_SUCCESS;
}

static uint64_t wait_timeout_hint = 0;

static void wait_and_reset(hsa_signal_t signal) {
  const hsa_signal_value_t value = hsa_signal_wait_scacquire(
      signal, HSA_SIGNAL_CONDITION_LT, 1, wait_timeout_hint,
      HSA_WAIT_STATE_BLOCKED);
  if (value != 0)
    std::exit(3);
  hsa_signal_store_screlease(signal, 1);
}

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

static double gbps(size_t bytes, uint64_t nanoseconds) {
  return static_cast<double>(bytes) / static_cast<double>(nanoseconds);
}

struct DirectionBuffers {
  std::vector<void *> source;
  std::vector<void *> destination;
  std::vector<uint8_t *> upload;
  std::vector<uint8_t *> download;
  std::vector<hsa_signal_t> signals;
};

static DirectionBuffers allocate_direction(
    hsa_amd_memory_pool_t source_pool, hsa_amd_memory_pool_t destination_pool,
    hsa_amd_memory_pool_t host_pool, const hsa_agent_t gpus[2], size_t bytes,
    size_t depth) {
  DirectionBuffers buffers;
  buffers.source.resize(depth);
  buffers.destination.resize(depth);
  buffers.upload.resize(depth);
  buffers.download.resize(depth);
  buffers.signals.resize(depth);
  for (size_t slot = 0; slot < depth; ++slot) {
    HSA_CHECK(hsa_amd_memory_pool_allocate(source_pool, bytes, 0,
                                           &buffers.source[slot]));
    HSA_CHECK(hsa_amd_memory_pool_allocate(destination_pool, bytes, 0,
                                           &buffers.destination[slot]));
    HSA_CHECK(hsa_amd_memory_pool_allocate(
        host_pool, bytes, 0,
        reinterpret_cast<void **>(&buffers.upload[slot])));
    HSA_CHECK(hsa_amd_memory_pool_allocate(
        host_pool, bytes, 0,
        reinterpret_cast<void **>(&buffers.download[slot])));
    HSA_CHECK(hsa_amd_agents_allow_access(2, gpus, nullptr,
                                          buffers.source[slot]));
    HSA_CHECK(hsa_amd_agents_allow_access(2, gpus, nullptr,
                                          buffers.destination[slot]));
    HSA_CHECK(hsa_amd_agents_allow_access(2, gpus, nullptr,
                                          buffers.upload[slot]));
    HSA_CHECK(hsa_amd_agents_allow_access(2, gpus, nullptr,
                                          buffers.download[slot]));
    HSA_CHECK(hsa_signal_create(1, 0, nullptr, &buffers.signals[slot]));
  }
  return buffers;
}

static void release_direction(DirectionBuffers &buffers) {
  for (size_t slot = 0; slot < buffers.source.size(); ++slot) {
    HSA_CHECK(hsa_signal_destroy(buffers.signals[slot]));
    HSA_CHECK(hsa_amd_memory_pool_free(buffers.source[slot]));
    HSA_CHECK(hsa_amd_memory_pool_free(buffers.destination[slot]));
    HSA_CHECK(hsa_amd_memory_pool_free(buffers.upload[slot]));
    HSA_CHECK(hsa_amd_memory_pool_free(buffers.download[slot]));
  }
}

static uint64_t run_direction(DirectionBuffers &buffers,
                              hsa_agent_t source_agent,
                              hsa_agent_t destination_agent,
                              hsa_agent_t cpu_agent, size_t bytes,
                              size_t round, size_t direction) {
  for (size_t slot = 0; slot < buffers.source.size(); ++slot) {
    const uint8_t value = pattern(round, slot, direction);
    std::memset(buffers.upload[slot], value, bytes);
    std::memset(buffers.download[slot], value ^ 0xff, bytes);
    HSA_CHECK(hsa_amd_memory_async_copy(
        buffers.source[slot], source_agent, buffers.upload[slot], cpu_agent,
        bytes, 0, nullptr, buffers.signals[slot]));
    wait_and_reset(buffers.signals[slot]);
    HSA_CHECK(hsa_amd_memory_async_copy(
        buffers.destination[slot], destination_agent, buffers.download[slot],
        cpu_agent, bytes, 0, nullptr, buffers.signals[slot]));
    wait_and_reset(buffers.signals[slot]);
  }

  const auto start = std::chrono::steady_clock::now();
  for (size_t slot = 0; slot < buffers.source.size(); ++slot) {
    HSA_CHECK(hsa_amd_memory_async_copy(
        buffers.destination[slot], destination_agent, buffers.source[slot],
        source_agent, bytes, 0, nullptr, buffers.signals[slot]));
  }
  for (hsa_signal_t signal : buffers.signals)
    wait_and_reset(signal);
  const auto end = std::chrono::steady_clock::now();

  for (size_t slot = 0; slot < buffers.source.size(); ++slot) {
    HSA_CHECK(hsa_amd_memory_async_copy(
        buffers.download[slot], cpu_agent, buffers.destination[slot],
        destination_agent, bytes, 0, nullptr, buffers.signals[slot]));
    wait_and_reset(buffers.signals[slot]);
    const uint8_t expected = pattern(round, slot, direction);
    if (!std::all_of(buffers.download[slot], buffers.download[slot] + bytes,
                     [expected](uint8_t byte) { return byte == expected; })) {
      std::fprintf(stderr,
                   "HSA XGMI peer mismatch at direction %zu round %zu slot %zu\n",
                   direction, round, slot);
      std::exit(3);
    }
  }
  return std::chrono::duration_cast<std::chrono::nanoseconds>(end - start)
      .count();
}

int main(int argc, char **argv) {
  if (argc != 9) {
    std::fprintf(stderr,
                 "usage: xgmi-peer-hsa <gpu-0> <gpu-1> <bytes> <depth> <warmups> <samples> <expected-unique-id-0> <expected-unique-id-1>\n");
    return 2;
  }
  size_t indices[2] = {};
  uint64_t unique_ids[2] = {};
  fe2o3::runtime_gfx942::WorkloadShape workload;
  if (!fe2o3::runtime_gfx942::parse_size(argv[1], &indices[0]) ||
      !fe2o3::runtime_gfx942::parse_size(argv[2], &indices[1]) ||
      !fe2o3::runtime_gfx942::parse_workload_shape(
          argv[3], argv[4], argv[5], argv[6], 1, &workload) ||
      !fe2o3::runtime_gfx942::parse_unique_id(argv[7], &unique_ids[0]) ||
      !fe2o3::runtime_gfx942::parse_unique_id(argv[8], &unique_ids[1]) ||
      indices[0] == indices[1] ||
      unique_ids[0] == 0 || unique_ids[1] == 0 ||
      unique_ids[0] == unique_ids[1])
    return 2;
  const size_t bytes = workload.bytes;
  const size_t depth = workload.depth;
  const size_t warmups = workload.warmups;
  const size_t samples = workload.samples;

  HSA_CHECK(hsa_init());
  bool xnack_enabled = true;
  uint64_t timestamp_frequency = 0;
  HSA_CHECK(hsa_system_get_info(HSA_AMD_SYSTEM_INFO_XNACK_ENABLED,
                                &xnack_enabled));
  HSA_CHECK(hsa_system_get_info(HSA_SYSTEM_INFO_TIMESTAMP_FREQUENCY,
                                &timestamp_frequency));
  if (xnack_enabled || timestamp_frequency == 0 ||
      timestamp_frequency > std::numeric_limits<uint64_t>::max() / 60)
    return 2;
  wait_timeout_hint = timestamp_frequency * 60;

  Agents agents;
  HSA_CHECK(hsa_iterate_agents(collect_agent, &agents));
  if (agents.cpus.empty() || indices[0] >= agents.gpus.size() ||
      indices[1] >= agents.gpus.size())
    return 2;
  const hsa_agent_t cpu = agents.cpus[0];
  const hsa_agent_t gpus[2] = {agents.gpus[indices[0]],
                               agents.gpus[indices[1]]};
  char uuids[2][21] = {};
  char targets[2][64] = {};
  for (size_t index = 0; index < 2; ++index) {
    char expected_uuid[21] = {};
    HSA_CHECK(hsa_agent_get_info(
        gpus[index], static_cast<hsa_agent_info_t>(HSA_AMD_AGENT_INFO_UUID),
        uuids[index]));
    HSA_CHECK(
        hsa_agent_get_info(gpus[index], HSA_AGENT_INFO_NAME, targets[index]));
    std::snprintf(expected_uuid, sizeof(expected_uuid), "GPU-%016llx",
                  static_cast<unsigned long long>(unique_ids[index]));
    if (std::strcmp(uuids[index], expected_uuid) != 0 ||
        std::strncmp(targets[index], "gfx942", 6) != 0)
      return 2;
  }

  PoolSelection host_pool{false};
  hsa_status_t status =
      hsa_amd_agent_iterate_memory_pools(cpu, choose_pool, &host_pool);
  if (status != HSA_STATUS_INFO_BREAK)
    HSA_CHECK(status);
  PoolSelection device_pools[2] = {{true}, {true}};
  for (size_t index = 0; index < 2; ++index) {
    status = hsa_amd_agent_iterate_memory_pools(gpus[index], choose_pool,
                                                &device_pools[index]);
    if (status != HSA_STATUS_INFO_BREAK)
      HSA_CHECK(status);
  }
  if (!host_pool.found || !device_pools[0].found || !device_pools[1].found)
    return 2;

  DirectionBuffers forward = allocate_direction(
      device_pools[0].pool, device_pools[1].pool, host_pool.pool, gpus, bytes,
      depth);
  DirectionBuffers reverse = allocate_direction(
      device_pools[1].pool, device_pools[0].pool, host_pool.pool, gpus, bytes,
      depth);
  std::vector<uint64_t> forward_samples, reverse_samples;
  forward_samples.reserve(samples);
  reverse_samples.reserve(samples);
  for (size_t round = 0; round < workload.total_iterations; ++round) {
    const uint64_t forward_ns = run_direction(
        forward, gpus[0], gpus[1], cpu, bytes, round, 0);
    const uint64_t reverse_ns = run_direction(
        reverse, gpus[1], gpus[0], cpu, bytes, round, 1);
    if (round >= warmups) {
      forward_samples.push_back(forward_ns);
      reverse_samples.push_back(reverse_ns);
    }
  }
  release_direction(reverse);
  release_direction(forward);
  HSA_CHECK(hsa_shut_down());

  const uint64_t forward_p50 = percentile(forward_samples, 1, 2);
  const uint64_t forward_p95 = percentile(forward_samples, 19, 20);
  const uint64_t reverse_p50 = percentile(reverse_samples, 1, 2);
  const uint64_t reverse_p95 = percentile(reverse_samples, 19, 20);
  if (forward_p50 == 0 || reverse_p50 == 0)
    return 2;
  std::printf(
      "backend=hsa schema=fe2o3.xgmi-peer-benchmark.v1 gpu_indices=%zu,%zu unique_ids=%016llx,%016llx targets=%s,%s xnack=disabled bytes=%zu depth=%zu warmups=%zu samples=%zu forward_p50_ns=%llu forward_p95_ns=%llu forward_p50_GBps=%.3f reverse_p50_ns=%llu reverse_p95_ns=%llu reverse_p50_GBps=%.3f\n",
      indices[0], indices[1], static_cast<unsigned long long>(unique_ids[0]),
      static_cast<unsigned long long>(unique_ids[1]), targets[0], targets[1],
      bytes, depth, warmups, samples,
      static_cast<unsigned long long>(forward_p50),
      static_cast<unsigned long long>(forward_p95),
      gbps(workload.transfer_bytes, forward_p50),
      static_cast<unsigned long long>(reverse_p50),
      static_cast<unsigned long long>(reverse_p95),
      gbps(workload.transfer_bytes, reverse_p50));
  return 0;
}

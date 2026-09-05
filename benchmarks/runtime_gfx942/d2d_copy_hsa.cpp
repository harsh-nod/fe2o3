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
      const char* text_ = nullptr;                                              \
      hsa_status_string(status_, &text_);                                       \
      std::fprintf(stderr, "%s failed: %s\n", #call,                           \
                   text_ ? text_ : "unknown");                                 \
      std::exit(2);                                                             \
    }                                                                          \
  } while (0)

struct Agents {
  std::vector<hsa_agent_t> cpus;
  std::vector<hsa_agent_t> gpus;
};

static hsa_status_t collect_agent(hsa_agent_t agent, void* data) {
  hsa_device_type_t type;
  hsa_status_t status = hsa_agent_get_info(agent, HSA_AGENT_INFO_DEVICE, &type);
  if (status != HSA_STATUS_SUCCESS) return status;
  auto* agents = static_cast<Agents*>(data);
  if (type == HSA_DEVICE_TYPE_CPU) agents->cpus.push_back(agent);
  if (type == HSA_DEVICE_TYPE_GPU) agents->gpus.push_back(agent);
  return HSA_STATUS_SUCCESS;
}

struct PoolSelection {
  bool want_gpu;
  bool found = false;
  hsa_amd_memory_pool_t pool{};
};

static hsa_status_t choose_pool(hsa_amd_memory_pool_t pool, void* data) {
  auto* selection = static_cast<PoolSelection*>(data);
  hsa_amd_segment_t segment;
  bool allowed = false;
  hsa_amd_memory_pool_location_t location;
  HSA_CHECK(hsa_amd_memory_pool_get_info(
      pool, HSA_AMD_MEMORY_POOL_INFO_SEGMENT, &segment));
  HSA_CHECK(hsa_amd_memory_pool_get_info(
      pool, HSA_AMD_MEMORY_POOL_INFO_RUNTIME_ALLOC_ALLOWED, &allowed));
  HSA_CHECK(hsa_amd_memory_pool_get_info(
      pool, HSA_AMD_MEMORY_POOL_INFO_LOCATION, &location));
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

static uint64_t wait_timeout_hint = 0;

static void wait_and_reset(const std::vector<hsa_signal_t>& signals) {
  for (auto signal : signals) {
    hsa_signal_value_t value = hsa_signal_wait_scacquire(
        signal, HSA_SIGNAL_CONDITION_LT, 1, wait_timeout_hint,
        HSA_WAIT_STATE_BLOCKED);
    if (value != 0) {
      std::fprintf(stderr, "unexpected HSA copy signal value %lld\n",
                   static_cast<long long>(value));
      std::exit(3);
    }
    hsa_signal_store_screlease(signal, 1);
  }
}

int main(int argc, char** argv) {
  if (argc != 7) {
    std::fprintf(
        stderr,
        "usage: d2d-copy-hsa <gpu-index> <bytes> <depth> <warmups> <samples> <expected-unique-id>\n");
    return 2;
  }
  size_t gpu_index = 0;
  uint64_t expected_unique_id = 0;
  fe2o3::runtime_gfx942::WorkloadShape workload;
  if (!fe2o3::runtime_gfx942::parse_size(argv[1], &gpu_index) ||
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
  if (agents.cpus.empty() || gpu_index >= agents.gpus.size()) return 2;
  hsa_agent_t cpu = agents.cpus[0];
  hsa_agent_t gpu = agents.gpus[gpu_index];
  char uuid[21] = {};
  char expected_uuid[21] = {};
  char target[64] = {};
  HSA_CHECK(hsa_agent_get_info(
      gpu, static_cast<hsa_agent_info_t>(HSA_AMD_AGENT_INFO_UUID), uuid));
  HSA_CHECK(hsa_agent_get_info(gpu, HSA_AGENT_INFO_NAME, target));
  std::snprintf(expected_uuid, sizeof(expected_uuid), "GPU-%016llx",
                static_cast<unsigned long long>(expected_unique_id));
  if (std::strcmp(uuid, expected_uuid) != 0 ||
      std::strncmp(target, "gfx942", 6) != 0) {
    std::fprintf(stderr, "HSA device identity or target mismatch\n");
    return 2;
  }

  PoolSelection host_pool{false};
  PoolSelection device_pool{true};
  hsa_status_t host_status =
      hsa_amd_agent_iterate_memory_pools(cpu, choose_pool, &host_pool);
  if (host_status != HSA_STATUS_INFO_BREAK) HSA_CHECK(host_status);
  hsa_status_t device_status =
      hsa_amd_agent_iterate_memory_pools(gpu, choose_pool, &device_pool);
  if (device_status != HSA_STATUS_INFO_BREAK) HSA_CHECK(device_status);
  if (!host_pool.found || !device_pool.found) return 2;

  std::vector<void*> upload(depth), download(depth), source(depth),
      destination(depth);
  std::vector<hsa_signal_t> signals(depth);
  for (size_t i = 0; i < depth; ++i) {
    HSA_CHECK(
        hsa_amd_memory_pool_allocate(host_pool.pool, bytes, 0, &upload[i]));
    HSA_CHECK(
        hsa_amd_memory_pool_allocate(host_pool.pool, bytes, 0, &download[i]));
    HSA_CHECK(
        hsa_amd_memory_pool_allocate(device_pool.pool, bytes, 0, &source[i]));
    HSA_CHECK(hsa_amd_memory_pool_allocate(device_pool.pool, bytes, 0,
                                           &destination[i]));
    HSA_CHECK(hsa_amd_agents_allow_access(1, &gpu, nullptr, upload[i]));
    HSA_CHECK(hsa_amd_agents_allow_access(1, &gpu, nullptr, download[i]));
    HSA_CHECK(hsa_amd_agents_allow_access(1, &gpu, nullptr, source[i]));
    HSA_CHECK(hsa_amd_agents_allow_access(1, &gpu, nullptr, destination[i]));
    HSA_CHECK(hsa_signal_create(1, 0, nullptr, &signals[i]));
  }

  std::vector<uint64_t> d2d;
  d2d.reserve(samples);
  for (size_t iteration = 0; iteration < workload.total_iterations;
       ++iteration) {
    for (size_t i = 0; i < depth; ++i) {
      uint8_t value = round_pattern(iteration, i);
      std::memset(upload[i], value, bytes);
      std::memset(download[i], value ^ 0xff, bytes);
      HSA_CHECK(hsa_amd_memory_async_copy(source[i], gpu, upload[i], cpu, bytes,
                                          0, nullptr, signals[i]));
    }
    wait_and_reset(signals);
    for (size_t i = 0; i < depth; ++i)
      HSA_CHECK(hsa_amd_memory_async_copy(destination[i], gpu, download[i], cpu,
                                          bytes, 0, nullptr, signals[i]));
    wait_and_reset(signals);

    auto start = std::chrono::steady_clock::now();
    for (size_t i = 0; i < depth; ++i)
      HSA_CHECK(hsa_amd_memory_async_copy(destination[i], gpu, source[i], gpu,
                                          bytes, 0, nullptr, signals[i]));
    wait_and_reset(signals);
    auto end = std::chrono::steady_clock::now();

    for (size_t i = 0; i < depth; ++i)
      HSA_CHECK(hsa_amd_memory_async_copy(download[i], cpu, destination[i], gpu,
                                          bytes, 0, nullptr, signals[i]));
    wait_and_reset(signals);
    for (size_t i = 0; i < depth; ++i) {
      uint8_t value = round_pattern(iteration, i);
      const auto* observed = static_cast<const uint8_t*>(download[i]);
      if (!std::all_of(observed, observed + bytes,
                       [value](uint8_t byte) { return byte == value; }))
        return 3;
    }
    for (size_t i = 0; i < depth; ++i) {
      uint8_t value = round_pattern(iteration, i);
      std::memset(download[i], value ^ 0x5a, bytes);
      HSA_CHECK(hsa_amd_memory_async_copy(download[i], cpu, source[i], gpu,
                                          bytes, 0, nullptr, signals[i]));
    }
    wait_and_reset(signals);
    for (size_t i = 0; i < depth; ++i) {
      uint8_t value = round_pattern(iteration, i);
      const auto* observed = static_cast<const uint8_t*>(download[i]);
      if (!std::all_of(observed, observed + bytes,
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
    HSA_CHECK(hsa_signal_destroy(signals[i]));
    HSA_CHECK(hsa_amd_memory_pool_free(destination[i]));
    HSA_CHECK(hsa_amd_memory_pool_free(source[i]));
    HSA_CHECK(hsa_amd_memory_pool_free(download[i]));
    HSA_CHECK(hsa_amd_memory_pool_free(upload[i]));
  }
  HSA_CHECK(hsa_shut_down());
  std::printf(
      "backend=hsa schema=fe2o3.d2d-copy-benchmark.v1 device_index=%zu unique_id=%016llx target=%s xnack=disabled bytes=%zu depth=%zu warmups=%zu samples=%zu d2d_p50_ns=%llu d2d_p95_ns=%llu d2d_p50_GBps=%.3f\n",
      gpu_index, static_cast<unsigned long long>(expected_unique_id), target,
      bytes, depth, warmups, samples,
      static_cast<unsigned long long>(d2d_p50),
      static_cast<unsigned long long>(d2d_p95),
      gbps(workload.transfer_bytes, d2d_p50));
  return 0;
}

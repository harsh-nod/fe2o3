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
      std::fprintf(stderr, "%s failed: %s\n", #call, text_ ? text_ : "unknown"); \
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
      ((selection->want_gpu && location == HSA_AMD_MEMORY_POOL_LOCATION_GPU) ||
       (!selection->want_gpu && location == HSA_AMD_MEMORY_POOL_LOCATION_CPU))) {
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

static uint8_t round_pattern(size_t round, size_t slot, size_t device) {
  return static_cast<uint8_t>((round * 67 + slot * 29 + device * 101 + 1) % 251 + 1);
}

static double gbps(size_t bytes, uint64_t nanoseconds) {
  return static_cast<double>(bytes) / static_cast<double>(nanoseconds);
}

static uint64_t wait_timeout_hint = 0;

static void wait_and_reset(const std::vector<hsa_signal_t>& signals) {
  for (auto signal : signals) {
    hsa_signal_value_t value = hsa_signal_wait_scacquire(
        signal, HSA_SIGNAL_CONDITION_LT, 1, wait_timeout_hint,
        HSA_WAIT_STATE_BLOCKED);
    if (value != 0) std::exit(3);
    hsa_signal_store_screlease(signal, 1);
  }
}

int main(int argc, char** argv) {
  if (argc != 9) {
    std::fprintf(stderr,
                 "usage: async-copy-multi-hsa <gpu-0> <gpu-1> <bytes> <depth-per-device> <warmups> <samples> <expected-unique-id-0> <expected-unique-id-1>\n");
    return 2;
  }
  size_t gpu_indices[2] = {};
  uint64_t expected_unique_ids[2] = {};
  fe2o3::runtime_gfx942::WorkloadShape workload;
  if (!fe2o3::runtime_gfx942::parse_size(argv[1], &gpu_indices[0]) ||
      !fe2o3::runtime_gfx942::parse_size(argv[2], &gpu_indices[1]) ||
      !fe2o3::runtime_gfx942::parse_workload_shape(
          argv[3], argv[4], argv[5], argv[6], 2, &workload) ||
      !fe2o3::runtime_gfx942::parse_unique_id(argv[7],
                                              &expected_unique_ids[0]) ||
      !fe2o3::runtime_gfx942::parse_unique_id(argv[8],
                                              &expected_unique_ids[1]) ||
      gpu_indices[0] == gpu_indices[1] ||
      expected_unique_ids[0] == 0 || expected_unique_ids[1] == 0 ||
      expected_unique_ids[0] == expected_unique_ids[1])
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
  if (agents.cpus.empty() || gpu_indices[0] >= agents.gpus.size() ||
      gpu_indices[1] >= agents.gpus.size())
    return 2;
  hsa_agent_t cpu = agents.cpus[0];
  const hsa_agent_t gpus[2] = {agents.gpus[gpu_indices[0]],
                               agents.gpus[gpu_indices[1]]};
  char uuids[2][21] = {};
  char targets[2][64] = {};
  for (size_t device = 0; device < 2; ++device) {
    char expected_uuid[21] = {};
    HSA_CHECK(hsa_agent_get_info(
        gpus[device], static_cast<hsa_agent_info_t>(HSA_AMD_AGENT_INFO_UUID),
        uuids[device]));
    HSA_CHECK(hsa_agent_get_info(gpus[device], HSA_AGENT_INFO_NAME,
                                 targets[device]));
    std::snprintf(expected_uuid, sizeof(expected_uuid), "GPU-%016llx",
                  static_cast<unsigned long long>(expected_unique_ids[device]));
    if (std::strcmp(uuids[device], expected_uuid) != 0 ||
        std::strncmp(targets[device], "gfx942", 6) != 0)
      return 2;
  }
  PoolSelection host_pool{false};
  hsa_status_t host_status =
      hsa_amd_agent_iterate_memory_pools(cpu, choose_pool, &host_pool);
  if (host_status != HSA_STATUS_INFO_BREAK) HSA_CHECK(host_status);
  PoolSelection device_pools[2] = {{true}, {true}};
  for (size_t device = 0; device < 2; ++device) {
    hsa_status_t status = hsa_amd_agent_iterate_memory_pools(
        gpus[device], choose_pool, &device_pools[device]);
    if (status != HSA_STATUS_INFO_BREAK) HSA_CHECK(status);
  }
  if (!host_pool.found || !device_pools[0].found || !device_pools[1].found)
    return 2;

  const size_t total = workload.total_depth;
  std::vector<void*> upload(total), device_memory(total), download(total);
  std::vector<hsa_signal_t> signals(total);
  for (size_t device = 0; device < 2; ++device) {
    for (size_t i = 0; i < depth; ++i) {
      const size_t index = device * depth + i;
      HSA_CHECK(hsa_amd_memory_pool_allocate(host_pool.pool, bytes, 0, &upload[index]));
      HSA_CHECK(hsa_amd_memory_pool_allocate(host_pool.pool, bytes, 0, &download[index]));
      HSA_CHECK(hsa_amd_memory_pool_allocate(device_pools[device].pool, bytes, 0,
                                             &device_memory[index]));
      HSA_CHECK(hsa_amd_agents_allow_access(1, &gpus[device], nullptr, upload[index]));
      HSA_CHECK(hsa_amd_agents_allow_access(1, &gpus[device], nullptr, download[index]));
      HSA_CHECK(hsa_amd_agents_allow_access(1, &gpus[device], nullptr,
                                            device_memory[index]));
      HSA_CHECK(hsa_signal_create(1, 0, nullptr, &signals[index]));
    }
  }

  std::vector<uint64_t> h2d, d2h;
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
    for (size_t device = 0; device < 2; ++device)
      for (size_t i = 0; i < depth; ++i) {
        const size_t index = device * depth + i;
        HSA_CHECK(hsa_amd_memory_async_copy(device_memory[index], gpus[device],
                                            upload[index], cpu, bytes, 0, nullptr,
                                            signals[index]));
      }
    wait_and_reset(signals);
    auto middle = std::chrono::steady_clock::now();
    for (size_t device = 0; device < 2; ++device)
      for (size_t i = 0; i < depth; ++i) {
        const size_t index = device * depth + i;
        HSA_CHECK(hsa_amd_memory_async_copy(download[index], cpu,
                                            device_memory[index], gpus[device],
                                            bytes, 0, nullptr, signals[index]));
      }
    wait_and_reset(signals);
    auto end = std::chrono::steady_clock::now();
    for (size_t device = 0; device < 2; ++device)
      for (size_t i = 0; i < depth; ++i) {
        const size_t index = device * depth + i;
        uint8_t value = round_pattern(iteration, i, device);
        const auto* observed = static_cast<const uint8_t*>(download[index]);
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
  for (size_t i = 0; i < total; ++i) {
    HSA_CHECK(hsa_signal_destroy(signals[i]));
    HSA_CHECK(hsa_amd_memory_pool_free(device_memory[i]));
    HSA_CHECK(hsa_amd_memory_pool_free(upload[i]));
    HSA_CHECK(hsa_amd_memory_pool_free(download[i]));
  }
  HSA_CHECK(hsa_shut_down());
  std::printf(
      "backend=hsa schema=fe2o3.async-copy-multi-device-benchmark.v1 devices=2 gpu_indices=%zu,%zu unique_ids=%016llx,%016llx targets=%s,%s xnack=disabled bytes=%zu depth_per_device=%zu warmups=%zu samples=%zu h2d_p50_ns=%llu h2d_p95_ns=%llu h2d_aggregate_p50_GBps=%.3f d2h_p50_ns=%llu d2h_p95_ns=%llu d2h_aggregate_p50_GBps=%.3f\n",
      gpu_indices[0], gpu_indices[1],
      static_cast<unsigned long long>(expected_unique_ids[0]),
      static_cast<unsigned long long>(expected_unique_ids[1]), targets[0],
      targets[1], bytes, depth, warmups, samples,
      static_cast<unsigned long long>(h2d_p50),
      static_cast<unsigned long long>(h2d_p95),
      gbps(workload.transfer_bytes, h2d_p50),
      static_cast<unsigned long long>(d2h_p50),
      static_cast<unsigned long long>(d2h_p95),
      gbps(workload.transfer_bytes, d2h_p50));
  return 0;
}

#include <hsa/hsa.h>
#include <hsa/hsa_ext_amd.h>

#include "r26_hsa_pool_policy.hpp"
#include "striped_copy_benchmark_common.hpp"

#include <chrono>
#include <cstdint>
#include <cstdio>
#include <cstdlib>
#include <cstring>
#include <limits>
#include <string>
#include <vector>

#define HSA_CHECK(call)                                                        \
  do {                                                                         \
    const hsa_status_t status_ = (call);                                       \
    if (status_ != HSA_STATUS_SUCCESS) {                                       \
      const char *text_ = nullptr;                                             \
      hsa_status_string(status_, &text_);                                      \
      std::fprintf(stderr, "%s failed: %s\n", #call,                           \
                   text_ == nullptr ? "unknown" : text_);                      \
      std::exit(2);                                                            \
    }                                                                          \
  } while (0)

namespace {

struct Agents {
  std::vector<hsa_agent_t> cpus;
  std::vector<hsa_agent_t> gpus;
};

hsa_status_t collect_agent(hsa_agent_t agent, void *data) {
  hsa_device_type_t type;
  const hsa_status_t status =
      hsa_agent_get_info(agent, HSA_AGENT_INFO_DEVICE, &type);
  if (status != HSA_STATUS_SUCCESS)
    return status;
  auto *const agents = static_cast<Agents *>(data);
  if (type == HSA_DEVICE_TYPE_CPU)
    agents->cpus.push_back(agent);
  if (type == HSA_DEVICE_TYPE_GPU)
    agents->gpus.push_back(agent);
  return HSA_STATUS_SUCCESS;
}

struct CollectedPool {
  hsa_amd_memory_pool_t pool{};
  fe2o3::r26::HsaPoolCandidate facts;
};

struct PoolCollector {
  fe2o3::r26::HsaPoolOwner owner;
  hsa_agent_t nearest_cpu{};
  hsa_agent_t selected_gpu{};
  std::vector<CollectedPool> *pools = nullptr;
};

fe2o3::r26::HsaPoolLocation
pool_location(hsa_amd_memory_pool_location_t location) {
  if (location == HSA_AMD_MEMORY_POOL_LOCATION_CPU)
    return fe2o3::r26::HsaPoolLocation::Cpu;
  if (location == HSA_AMD_MEMORY_POOL_LOCATION_GPU)
    return fe2o3::r26::HsaPoolLocation::Gpu;
  return fe2o3::r26::HsaPoolLocation::Other;
}

hsa_status_t collect_pool(hsa_amd_memory_pool_t pool, void *data) {
  auto *const collector = static_cast<PoolCollector *>(data);
  hsa_amd_segment_t segment;
  bool allowed = false;
  hsa_amd_memory_pool_location_t location;
  hsa_status_t status = hsa_amd_memory_pool_get_info(
      pool, HSA_AMD_MEMORY_POOL_INFO_SEGMENT, &segment);
  if (status != HSA_STATUS_SUCCESS)
    return status;
  if (segment != HSA_AMD_SEGMENT_GLOBAL)
    return HSA_STATUS_SUCCESS;
  std::uint32_t flags = 0;
  status = hsa_amd_memory_pool_get_info(
      pool, HSA_AMD_MEMORY_POOL_INFO_RUNTIME_ALLOC_ALLOWED, &allowed);
  if (status != HSA_STATUS_SUCCESS)
    return status;
  status = hsa_amd_memory_pool_get_info(pool, HSA_AMD_MEMORY_POOL_INFO_LOCATION,
                                        &location);
  if (status != HSA_STATUS_SUCCESS)
    return status;
  status = hsa_amd_memory_pool_get_info(
      pool, HSA_AMD_MEMORY_POOL_INFO_GLOBAL_FLAGS, &flags);
  if (status != HSA_STATUS_SUCCESS)
    return status;
  std::size_t maximum_allocation = 0;
  std::size_t granule = 0;
  std::size_t alignment = 0;
  if (allowed) {
    status = hsa_amd_memory_pool_get_info(
        pool, HSA_AMD_MEMORY_POOL_INFO_ALLOC_MAX_SIZE, &maximum_allocation);
    if (status != HSA_STATUS_SUCCESS)
      return status;
    status = hsa_amd_memory_pool_get_info(
        pool, HSA_AMD_MEMORY_POOL_INFO_RUNTIME_ALLOC_GRANULE, &granule);
    if (status != HSA_STATUS_SUCCESS)
      return status;
    status = hsa_amd_memory_pool_get_info(
        pool, HSA_AMD_MEMORY_POOL_INFO_RUNTIME_ALLOC_ALIGNMENT, &alignment);
    if (status != HSA_STATUS_SUCCESS)
      return status;
  }
  hsa_amd_memory_pool_access_t cpu_access{};
  hsa_amd_memory_pool_access_t gpu_access{};
  status = hsa_amd_agent_memory_pool_get_info(
      collector->nearest_cpu, pool, HSA_AMD_AGENT_MEMORY_POOL_INFO_ACCESS,
      &cpu_access);
  if (status != HSA_STATUS_SUCCESS)
    return status;
  status = hsa_amd_agent_memory_pool_get_info(
      collector->selected_gpu, pool, HSA_AMD_AGENT_MEMORY_POOL_INFO_ACCESS,
      &gpu_access);
  if (status != HSA_STATUS_SUCCESS)
    return status;
  collector->pools->push_back(CollectedPool{
      pool,
      fe2o3::r26::HsaPoolCandidate{
          pool.handle,
          collector->owner,
          pool_location(location),
          true,
          allowed,
          flags,
          maximum_allocation,
          granule,
          alignment,
          cpu_access != HSA_AMD_MEMORY_POOL_ACCESS_NEVER_ALLOWED,
          gpu_access != HSA_AMD_MEMORY_POOL_ACCESS_NEVER_ALLOWED,
      },
  });
  return HSA_STATUS_SUCCESS;
}

void collect_pools(hsa_agent_t owner_agent, fe2o3::r26::HsaPoolOwner owner,
                   hsa_agent_t nearest_cpu, hsa_agent_t selected_gpu,
                   std::vector<CollectedPool> *pools) {
  PoolCollector collector{owner, nearest_cpu, selected_gpu, pools};
  HSA_CHECK(hsa_amd_agent_iterate_memory_pools(owner_agent, collect_pool,
                                               &collector));
}

} // namespace

int main(int argc, char **argv) {
  fe2o3::r40::Config config;
  if (!fe2o3::r40::parse_config(argc, argv, &config)) {
    std::fputs(
        "usage: striped-copy-hsa <gpu-index> <unique-id> <bytes> <depth> "
        "<warmups> <samples> <logical-queue-count> <profile>\n",
        stderr);
    return 2;
  }
  HSA_CHECK(hsa_init());
  bool xnack_enabled = true;
  std::uint64_t timestamp_frequency = 0;
  HSA_CHECK(
      hsa_system_get_info(HSA_AMD_SYSTEM_INFO_XNACK_ENABLED, &xnack_enabled));
  HSA_CHECK(hsa_system_get_info(HSA_SYSTEM_INFO_TIMESTAMP_FREQUENCY,
                                &timestamp_frequency));
  if (xnack_enabled || timestamp_frequency == 0 ||
      timestamp_frequency > std::numeric_limits<std::uint64_t>::max() / 60)
    return 2;
  const std::uint64_t timeout_hint = timestamp_frequency * 60;
  Agents agents;
  HSA_CHECK(hsa_iterate_agents(collect_agent, &agents));
  if (agents.cpus.empty() ||
      static_cast<std::size_t>(config.device_index) >= agents.gpus.size())
    return 2;
  const hsa_agent_t gpu = agents.gpus[config.device_index];
  char uuid[21] = {};
  char expected_uuid[21] = {};
  char target[64] = {};
  HSA_CHECK(hsa_agent_get_info(
      gpu, static_cast<hsa_agent_info_t>(HSA_AMD_AGENT_INFO_UUID), uuid));
  HSA_CHECK(hsa_agent_get_info(gpu, HSA_AGENT_INFO_NAME, target));
  std::snprintf(expected_uuid, sizeof(expected_uuid), "GPU-%016llx",
                static_cast<unsigned long long>(config.unique_id));
  if (std::strcmp(uuid, expected_uuid) != 0 ||
      std::strncmp(target, "gfx942", 6) != 0) {
    std::fputs("HSA device identity or target mismatch\n", stderr);
    return 2;
  }
  hsa_agent_t cpu{};
  HSA_CHECK(hsa_agent_get_info(
      gpu, static_cast<hsa_agent_info_t>(HSA_AMD_AGENT_INFO_NEAREST_CPU),
      &cpu));
  hsa_device_type_t nearest_cpu_type{};
  HSA_CHECK(hsa_agent_get_info(cpu, HSA_AGENT_INFO_DEVICE, &nearest_cpu_type));
  std::vector<std::uint64_t> enumerated_cpu_handles;
  enumerated_cpu_handles.reserve(agents.cpus.size());
  for (const hsa_agent_t agent : agents.cpus)
    enumerated_cpu_handles.push_back(agent.handle);
  const fe2o3::r26::HsaAgentType normalized_type =
      nearest_cpu_type == HSA_DEVICE_TYPE_CPU
          ? fe2o3::r26::HsaAgentType::Cpu
          : (nearest_cpu_type == HSA_DEVICE_TYPE_GPU
                 ? fe2o3::r26::HsaAgentType::Gpu
                 : fe2o3::r26::HsaAgentType::Other);
  if (!fe2o3::r26::unique_enumerated_nearest_cpu(cpu.handle, normalized_type,
                                                 enumerated_cpu_handles)) {
    std::fputs("HSA GPU does not have one enumerated nearest CPU\n", stderr);
    return 2;
  }
  std::vector<CollectedPool> collected_pools;
  collect_pools(cpu, fe2o3::r26::HsaPoolOwner::NearestCpu, cpu, gpu,
                &collected_pools);
  collect_pools(gpu, fe2o3::r26::HsaPoolOwner::SelectedGpu, cpu, gpu,
                &collected_pools);
  std::vector<fe2o3::r26::HsaPoolCandidate> pool_facts;
  pool_facts.reserve(collected_pools.size());
  for (const CollectedPool &pool : collected_pools)
    pool_facts.push_back(pool.facts);
  fe2o3::r26::HsaPoolRoles pool_roles;
  if (fe2o3::r26::select_hsa_pool_roles(pool_facts, config.bytes, 16, 16,
                                        &pool_roles) !=
      fe2o3::r26::HsaPoolPolicyStatus::Accepted) {
    std::fputs("HSA memory pools do not satisfy the exact R26 policy\n",
               stderr);
    return 2;
  }
  const hsa_amd_memory_pool_t host_pool = collected_pools[pool_roles.host].pool;
  const hsa_amd_memory_pool_t device_pool =
      collected_pools[pool_roles.device].pool;

  std::vector<std::uint8_t *> upload(config.depth), download(config.depth);
  std::vector<void *> device(config.depth);
  std::vector<hsa_signal_t> signals(config.depth);
  for (std::size_t request = 0; request < config.depth; ++request) {
    void *upload_pointer = nullptr;
    void *download_pointer = nullptr;
    HSA_CHECK(hsa_amd_memory_pool_allocate(host_pool, config.bytes, 0,
                                           &upload_pointer));
    HSA_CHECK(hsa_amd_memory_pool_allocate(host_pool, config.bytes, 0,
                                           &download_pointer));
    upload[request] = static_cast<std::uint8_t *>(upload_pointer);
    download[request] = static_cast<std::uint8_t *>(download_pointer);
    HSA_CHECK(hsa_amd_memory_pool_allocate(device_pool, config.bytes, 0,
                                           &device[request]));
    HSA_CHECK(hsa_amd_agents_allow_access(1, &gpu, nullptr, upload[request]));
    HSA_CHECK(hsa_amd_agents_allow_access(1, &gpu, nullptr, download[request]));
    HSA_CHECK(hsa_amd_agents_allow_access(1, &gpu, nullptr, device[request]));
    HSA_CHECK(hsa_signal_create(1, 0, nullptr, &signals[request]));
  }
  std::vector<std::vector<std::size_t>> orders;
  orders.reserve(config.logical_queue_count);
  for (std::size_t ordinal = 0; ordinal < config.logical_queue_count; ++ordinal)
    orders.push_back(fe2o3::r40::publication_order(ordinal, config.depth,
                                                   config.logical_queue_count));

  fe2o3::r40::PhaseSamples h2d(config.samples), d2h(config.samples);
  std::size_t submission_ordinal = 0;
  for (std::size_t round = 0; round < config.rounds; ++round) {
    for (std::size_t request = 0; request < config.depth; ++request) {
      const std::uint8_t value = fe2o3::r40::round_pattern(round, request);
      std::memset(upload[request], value, config.bytes);
      std::memset(download[request], value ^ 0xffU, config.bytes);
    }
    const auto run_phase = [&](bool upload_direction,
                               std::size_t submission_ordinal,
                               fe2o3::r40::PhaseSamples *samples) {
      const auto &order =
          orders[submission_ordinal % config.logical_queue_count];
      std::vector<hsa_signal_t> lane_tail(config.logical_queue_count);
      std::vector<bool> lane_has_tail(config.logical_queue_count, false);
      const auto t0 = std::chrono::steady_clock::now();
      for (const std::size_t request : order) {
        const std::size_t lane =
            (submission_ordinal + request) % config.logical_queue_count;
        const std::uint32_t dependency_count = lane_has_tail[lane] ? 1U : 0U;
        const hsa_signal_t *dependency =
            lane_has_tail[lane] ? &lane_tail[lane] : nullptr;
        HSA_CHECK(hsa_amd_memory_async_copy(
            upload_direction ? device[request] : download[request],
            upload_direction ? gpu : cpu,
            upload_direction ? static_cast<void *>(upload[request])
                             : device[request],
            upload_direction ? cpu : gpu, config.bytes, dependency_count,
            dependency, signals[request]));
        lane_tail[lane] = signals[request];
        lane_has_tail[lane] = true;
      }
      const auto t1 = std::chrono::steady_clock::now();
      for (std::size_t lane_offset = 0;
           lane_offset < config.logical_queue_count; ++lane_offset) {
        const std::size_t lane =
            (submission_ordinal + lane_offset) % config.logical_queue_count;
        if (!lane_has_tail[lane])
          std::exit(3);
        const hsa_signal_value_t observed =
            hsa_signal_wait_scacquire(lane_tail[lane], HSA_SIGNAL_CONDITION_LT,
                                      1, timeout_hint, HSA_WAIT_STATE_BLOCKED);
        if (observed != 0)
          std::exit(3);
      }
      const auto t2 = std::chrono::steady_clock::now();
      for (const hsa_signal_t signal : signals)
        hsa_signal_store_screlease(signal, 1);
      if (round >= config.warmups &&
          !samples->append(fe2o3::r40::elapsed_ns(t0, t1),
                           fe2o3::r40::elapsed_ns(t1, t2)))
        std::exit(3);
    };
    run_phase(true, submission_ordinal, &h2d);
    submission_ordinal = (submission_ordinal + 1) % config.logical_queue_count;
    run_phase(false, submission_ordinal, &d2h);
    submission_ordinal = (submission_ordinal + 1) % config.logical_queue_count;
    if (!fe2o3::r40::validate_buffers(download, config.bytes, round, "hsa"))
      return 3;
  }

  for (std::size_t request = 0; request < config.depth; ++request) {
    HSA_CHECK(hsa_signal_destroy(signals[request]));
    HSA_CHECK(hsa_amd_memory_pool_free(device[request]));
    HSA_CHECK(hsa_amd_memory_pool_free(upload[request]));
    HSA_CHECK(hsa_amd_memory_pool_free(download[request]));
  }
  HSA_CHECK(hsa_shut_down());
  const std::string resource_profile =
      "logical-dependency-width-q" + std::to_string(config.logical_queue_count);
  fe2o3::r40::report_native("hsa", "hsa-amd-memory-async-copy",
                            resource_profile.c_str(), "not-observed", config,
                            h2d, d2h);
  return 0;
}

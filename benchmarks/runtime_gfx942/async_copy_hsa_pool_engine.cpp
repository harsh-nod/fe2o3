#include <hsa/hsa.h>
#include <hsa/hsa_ext_amd.h>

#include "hsa_copy_diagnostic.hpp"

#include <chrono>
#include <cinttypes>
#include <cstdio>
#include <cstdlib>
#include <cstring>
#include <limits>
#include <vector>

namespace policy = fe2o3::copy_diagnostic;

static_assert(policy::kFine == HSA_AMD_MEMORY_POOL_GLOBAL_FLAG_FINE_GRAINED);
static_assert(policy::kCoarse ==
              HSA_AMD_MEMORY_POOL_GLOBAL_FLAG_COARSE_GRAINED);
static_assert(HSA_AMD_SDMA_ENGINE_0 == 1 && HSA_AMD_SDMA_ENGINE_1 == 2);
static_assert(HSA_AMD_MEMORY_POOL_ACCESS_ALLOWED_BY_DEFAULT == 1 &&
              HSA_AMD_MEMORY_POOL_ACCESS_DISALLOWED_BY_DEFAULT == 2);

namespace {

[[noreturn]] void fail(const char *message) {
  std::fprintf(stderr, "HSA copy diagnostic rejected: %s\n", message);
  // Never free or recycle storage after an unproved completion. Fatal native
  // errors terminate this process; only the successful path performs cleanup.
  std::exit(2);
}

void check(hsa_status_t status, const char *operation) {
  if (status == HSA_STATUS_SUCCESS)
    return;
  const char *text = nullptr;
  hsa_status_string(status, &text);
  std::fprintf(stderr, "%s failed: status=%u %s\n", operation,
               static_cast<unsigned>(status), text ? text : "unknown");
  fail("HSA operation failed");
}

void flush_output() {
  if (std::fflush(stdout) != 0 || std::ferror(stdout))
    fail("diagnostic output failed");
}

#define HSA_CHECK(call) check((call), #call)

struct Agents {
  std::vector<hsa_agent_t> cpus;
  std::vector<hsa_agent_t> gpus;
};

hsa_status_t collect_agent(hsa_agent_t agent, void *data) {
  hsa_device_type_t type;
  const auto status = hsa_agent_get_info(agent, HSA_AGENT_INFO_DEVICE, &type);
  if (status != HSA_STATUS_SUCCESS)
    return status;
  auto &agents = *static_cast<Agents *>(data);
  if (type == HSA_DEVICE_TYPE_CPU)
    agents.cpus.push_back(agent);
  if (type == HSA_DEVICE_TYPE_GPU)
    agents.gpus.push_back(agent);
  return HSA_STATUS_SUCCESS;
}

struct PoolCollector {
  hsa_agent_t cpu;
  hsa_agent_t gpu;
  std::vector<policy::PoolFacts> pools;
};

hsa_status_t collect_pool(hsa_amd_memory_pool_t pool, void *data) {
  auto &collector = *static_cast<PoolCollector *>(data);
  hsa_amd_segment_t segment;
  HSA_CHECK(hsa_amd_memory_pool_get_info(pool, HSA_AMD_MEMORY_POOL_INFO_SEGMENT,
                                         &segment));
  if (segment != HSA_AMD_SEGMENT_GLOBAL)
    return HSA_STATUS_SUCCESS;
  policy::PoolFacts facts;
  facts.handle = pool.handle;
  facts.global = true;
  hsa_amd_memory_pool_location_t location;
  HSA_CHECK(hsa_amd_memory_pool_get_info(
      pool, HSA_AMD_MEMORY_POOL_INFO_LOCATION, &location));
  facts.location =
      location == HSA_AMD_MEMORY_POOL_LOCATION_CPU   ? policy::Location::Cpu
      : location == HSA_AMD_MEMORY_POOL_LOCATION_GPU ? policy::Location::Gpu
                                                     : policy::Location::Other;
  HSA_CHECK(hsa_amd_memory_pool_get_info(
      pool, HSA_AMD_MEMORY_POOL_INFO_RUNTIME_ALLOC_ALLOWED,
      &facts.allocatable));
  HSA_CHECK(hsa_amd_memory_pool_get_info(
      pool, HSA_AMD_MEMORY_POOL_INFO_GLOBAL_FLAGS, &facts.flags));
  if (facts.allocatable) {
    HSA_CHECK(hsa_amd_memory_pool_get_info(
        pool, HSA_AMD_MEMORY_POOL_INFO_ALLOC_MAX_SIZE, &facts.maximum_bytes));
    HSA_CHECK(hsa_amd_memory_pool_get_info(
        pool, HSA_AMD_MEMORY_POOL_INFO_RUNTIME_ALLOC_GRANULE, &facts.granule));
    HSA_CHECK(hsa_amd_memory_pool_get_info(
        pool, HSA_AMD_MEMORY_POOL_INFO_RUNTIME_ALLOC_ALIGNMENT,
        &facts.alignment));
  }
  hsa_amd_memory_pool_access_t cpu_access, gpu_access;
  HSA_CHECK(hsa_amd_agent_memory_pool_get_info(
      collector.cpu, pool, HSA_AMD_AGENT_MEMORY_POOL_INFO_ACCESS, &cpu_access));
  HSA_CHECK(hsa_amd_agent_memory_pool_get_info(
      collector.gpu, pool, HSA_AMD_AGENT_MEMORY_POOL_INFO_ACCESS, &gpu_access));
  facts.cpu_access = static_cast<std::uint32_t>(cpu_access);
  facts.gpu_access = static_cast<std::uint32_t>(gpu_access);
  collector.pools.push_back(facts);
  return HSA_STATUS_SUCCESS;
}

void print_inventory(const char *owner,
                     const std::vector<policy::PoolFacts> &pools) {
  for (std::size_t index = 0; index < pools.size(); ++index) {
    const auto &pool = pools[index];
    const char *location = pool.location == policy::Location::Cpu   ? "cpu"
                           : pool.location == policy::Location::Gpu ? "gpu"
                                                                    : "other";
    std::printf(
        "schema=%s record=pool owner=%s index=%zu handle=%" PRIu64
        " location=%s global=1 allocatable=%u flags=%u max_aggregate_bytes=%zu"
        " granule=%zu alignment=%zu cpu_access=%u gpu_access=%u\n",
        policy::kSchema, owner, index, pool.handle, location,
        static_cast<unsigned>(pool.allocatable), pool.flags, pool.maximum_bytes,
        pool.granule, pool.alignment, pool.cpu_access, pool.gpu_access);
  }
}

std::uint64_t now_ns() {
  const auto value = std::chrono::duration_cast<std::chrono::nanoseconds>(
                         std::chrono::steady_clock::now().time_since_epoch())
                         .count();
  if (value < 0)
    fail("negative steady-clock timestamp");
  return static_cast<std::uint64_t>(value);
}

policy::Timing copy(void *destination, hsa_agent_t destination_agent,
                    const void *source, hsa_agent_t source_agent,
                    std::size_t bytes, hsa_signal_t signal,
                    std::uint32_t engine_mask, std::uint64_t timeout_hint) {
  const auto start = now_ns();
  HSA_CHECK(hsa_amd_memory_async_copy_on_engine(
      destination, destination_agent, source, source_agent, bytes, 0, nullptr,
      signal, static_cast<hsa_amd_sdma_engine_id_t>(engine_mask), false));
  const auto submitted = now_ns();
  const auto value = hsa_signal_wait_scacquire(
      signal, HSA_SIGNAL_CONDITION_LT, 1, timeout_hint, HSA_WAIT_STATE_BLOCKED);
  if (!policy::exact_completion(value))
    fail("copy signal is not exactly zero; no signal reset or storage reuse");
  hsa_signal_store_screlease(signal, 1);
  const auto finished = now_ns();
  policy::Timing result;
  if (!policy::timing(start, submitted, finished, &result))
    fail("nonmonotonic or zero-duration timing");
  return result;
}

struct Round {
  policy::Timing h2d;
  policy::Timing d2h;
};

} // namespace

int main(int argc, char **argv) {
  policy::Config config;
  if (!policy::parse_config(argc, argv, &config)) {
    std::fputs(policy::kUsage, stderr);
    return 2;
  }
  HSA_CHECK(hsa_init());
  bool xnack = true;
  std::uint64_t frequency = 0;
  HSA_CHECK(hsa_system_get_info(HSA_AMD_SYSTEM_INFO_XNACK_ENABLED, &xnack));
  HSA_CHECK(
      hsa_system_get_info(HSA_SYSTEM_INFO_TIMESTAMP_FREQUENCY, &frequency));
  if (xnack || frequency == 0 ||
      frequency > std::numeric_limits<std::uint64_t>::max() / 60)
    fail("requires XNACK disabled and a bounded HSA timeout hint");
  Agents agents;
  HSA_CHECK(hsa_iterate_agents(collect_agent, &agents));
  if (config.gpu_index >= agents.gpus.size() ||
      config.cpu_index >= agents.cpus.size())
    fail("selected CPU or GPU index was not enumerated");
  const auto cpu = agents.cpus[config.cpu_index];
  const auto gpu = agents.gpus[config.gpu_index];
  char uuid[21] = {}, expected_uuid[21] = {}, target[64] = {};
  HSA_CHECK(hsa_agent_get_info(
      gpu, static_cast<hsa_agent_info_t>(HSA_AMD_AGENT_INFO_UUID), uuid));
  HSA_CHECK(hsa_agent_get_info(gpu, HSA_AGENT_INFO_NAME, target));
  std::snprintf(expected_uuid, sizeof(expected_uuid), "GPU-%016" PRIx64,
                config.unique_id);
  if (std::strcmp(uuid, expected_uuid) != 0 ||
      std::strcmp(target, "gfx942") != 0)
    fail("GPU UUID or gfx942 target mismatch");

  std::uint32_t cpu_node = 0, gpu_node = 0, bdf = 0, domain = 0;
  hsa_agent_t nearest_cpu{};
  HSA_CHECK(hsa_agent_get_info(
      cpu, static_cast<hsa_agent_info_t>(HSA_AMD_AGENT_INFO_DRIVER_NODE_ID),
      &cpu_node));
  HSA_CHECK(hsa_agent_get_info(
      gpu, static_cast<hsa_agent_info_t>(HSA_AMD_AGENT_INFO_DRIVER_NODE_ID),
      &gpu_node));
  HSA_CHECK(hsa_agent_get_info(
      gpu, static_cast<hsa_agent_info_t>(HSA_AMD_AGENT_INFO_BDFID), &bdf));
  HSA_CHECK(hsa_agent_get_info(
      gpu, static_cast<hsa_agent_info_t>(HSA_AMD_AGENT_INFO_DOMAIN), &domain));
  HSA_CHECK(hsa_agent_get_info(
      gpu, static_cast<hsa_agent_info_t>(HSA_AMD_AGENT_INFO_NEAREST_CPU),
      &nearest_cpu));
  PoolCollector host{cpu, gpu, {}}, device{cpu, gpu, {}};
  HSA_CHECK(hsa_amd_agent_iterate_memory_pools(cpu, collect_pool, &host));
  HSA_CHECK(hsa_amd_agent_iterate_memory_pools(gpu, collect_pool, &device));
  print_inventory("cpu", host.pools);
  print_inventory("gpu", device.pools);
  flush_output();
  policy::PoolSelection host_selection, device_selection;
  const auto bytes = config.workload.bytes;
  if (!policy::select_pool(host.pools, policy::Location::Cpu, config.host_flags,
                           bytes, 2, &host_selection) ||
      !policy::select_pool(device.pools, policy::Location::Gpu, policy::kCoarse,
                           bytes, 1, &device_selection))
    fail("missing, ambiguous, inaccessible or insufficient exact pool role");
  std::uint32_t h2d_engines = 0, d2h_engines = 0;
  HSA_CHECK(hsa_amd_memory_copy_engine_status(gpu, cpu, &h2d_engines));
  HSA_CHECK(hsa_amd_memory_copy_engine_status(cpu, gpu, &d2h_engines));
  if (!policy::engine_available(config.engine_mask, h2d_engines, d2h_engines))
    fail("requested engine is not available in both directions");
  const hsa_amd_memory_pool_t host_pool{
      host.pools[host_selection.index].handle};
  const hsa_amd_memory_pool_t device_pool{
      device.pools[device_selection.index].handle};
  std::printf(
      "schema=%s record=config gpu_index=%zu cpu_index=%zu "
      "unique_id=%016" PRIx64 " target=gfx942 xnack=disabled gpu_agent=%" PRIu64
      " cpu_agent=%" PRIu64 " nearest_cpu_agent=%" PRIu64
      " cpu_driver_node=%u gpu_driver_node=%u gpu_domain=%u gpu_bdf_id=%u"
      " bytes=%zu depth=1 warmups=%zu samples=%zu host_grain=%s"
      " host_pool=%" PRIu64 " device_pool=%" PRIu64
      " host_rounded_bytes=%zu host_aggregate_bytes=%zu "
      "device_rounded_bytes=%zu"
      " requested_engine_mask=%u h2d_available_mask=%u d2h_available_mask=%u"
      " allocation_flags=0 force_copy_on_sdma=0 wait=blocked_scacquire"
      " host_release=signal_screlease_before_h2d_timing"
      " timing=host_submit_wait_reset timeout_hint=%" PRIu64
      " timestamp_frequency=%" PRIu64 "\n",
      policy::kSchema, config.gpu_index, config.cpu_index, config.unique_id,
      gpu.handle, cpu.handle, nearest_cpu.handle, cpu_node, gpu_node, domain,
      bdf, bytes, config.workload.warmups, config.workload.samples,
      config.host_flags == policy::kFine ? "fine" : "coarse", host_pool.handle,
      device_pool.handle, host_selection.rounded_bytes,
      host_selection.aggregate_bytes, device_selection.rounded_bytes,
      config.engine_mask, h2d_engines, d2h_engines, frequency * 60, frequency);
  flush_output();

  std::vector<Round> rounds(config.workload.total_iterations);
  void *upload = nullptr, *download = nullptr, *buffer = nullptr;
  HSA_CHECK(hsa_amd_memory_pool_allocate(host_pool, bytes, 0, &upload));
  HSA_CHECK(hsa_amd_memory_pool_allocate(host_pool, bytes, 0, &download));
  HSA_CHECK(hsa_amd_memory_pool_allocate(device_pool, bytes, 0, &buffer));
  const hsa_agent_t access_agents[] = {cpu, gpu};
  HSA_CHECK(hsa_amd_agents_allow_access(2, access_agents, nullptr, upload));
  HSA_CHECK(hsa_amd_agents_allow_access(2, access_agents, nullptr, download));
  HSA_CHECK(hsa_amd_agents_allow_access(2, access_agents, nullptr, buffer));
  hsa_signal_t signal{};
  HSA_CHECK(hsa_signal_create(1, 0, nullptr, &signal));
  for (std::size_t index = 0; index < rounds.size(); ++index) {
    const auto expected = policy::pattern(index);
    std::memset(upload, expected, bytes);
    std::memset(download, expected ^ 0xff, bytes);
    // The signal is idle at one. Release the new host contents to system scope
    // before the timed submission; receive ordering is the scacquire wait.
    hsa_signal_store_screlease(signal, 1);
    rounds[index].h2d = copy(buffer, gpu, upload, cpu, bytes, signal,
                             config.engine_mask, frequency * 60);
    rounds[index].d2h = copy(download, cpu, buffer, gpu, bytes, signal,
                             config.engine_mask, frequency * 60);
    if (!policy::valid_buffer(static_cast<const std::uint8_t *>(download),
                              bytes, expected))
      fail("full returned buffer mismatch");
  }
  HSA_CHECK(hsa_signal_destroy(signal));
  HSA_CHECK(hsa_amd_memory_pool_free(buffer));
  HSA_CHECK(hsa_amd_memory_pool_free(download));
  HSA_CHECK(hsa_amd_memory_pool_free(upload));
  HSA_CHECK(hsa_shut_down());

  for (std::size_t index = 0; index < rounds.size(); ++index) {
    const auto &round = rounds[index];
    std::printf("schema=%s record=round index=%zu phase=%s pattern=%u"
                " h2d_submit_ns=%" PRIu64 " h2d_wait_reset_ns=%" PRIu64
                " h2d_total_ns=%" PRIu64 " d2h_submit_ns=%" PRIu64
                " d2h_wait_reset_ns=%" PRIu64 " d2h_total_ns=%" PRIu64
                " h2d_signal=0 d2h_signal=0 checked_bytes=%zu\n",
                policy::kSchema, index,
                index < config.workload.warmups ? "warmup" : "sample",
                static_cast<unsigned>(policy::pattern(index)),
                round.h2d.submit_ns, round.h2d.wait_reset_ns,
                round.h2d.total_ns, round.d2h.submit_ns,
                round.d2h.wait_reset_ns, round.d2h.total_ns, bytes);
  }
  std::printf(
      "schema=%s record=complete validated_rounds=%zu measured_rounds=%zu"
      " checked_bytes_per_round=%zu signals_destroyed=1 allocations_freed=3"
      " shutdown=1\n",
      policy::kSchema, rounds.size(), config.workload.samples, bytes);
  flush_output();
  return 0;
}

// CPU-only adapter fixture. This file must never be linked into hardware runs.
#include <hsa/hsa.h>
#include <hsa/hsa_ext_amd.h>

#include <cstdint>
#include <cstdio>
#include <cstdlib>
#include <cstring>
#include <limits>
#include <map>
#include <vector>

namespace {

bool mode(const char *name) {
  const char *value = std::getenv("FE2O3_HSA_COPY_TEST_CASE");
  return value != nullptr && std::strcmp(value, name) == 0;
}

void require(bool condition) {
  if (!condition) {
    std::fputs("MOCK_PROTOCOL_ERROR\n", stderr);
    std::abort();
  }
}

template <typename T> hsa_status_t result(void *output, T value) {
  *static_cast<T *>(output) = value;
  return HSA_STATUS_SUCCESS;
}

struct Allocation {
  std::uint64_t pool;
  std::size_t bytes;
  bool access_granted = false;
};

std::map<const void *, Allocation> allocations;
std::vector<void *> allocation_order;
std::uint64_t selected_cpu = 0, selected_gpu = 0;
std::size_t copies = 0, waits = 0, resets = 0, frees = 0, statuses = 0;
bool signal_live = false;
bool prepared = false;
hsa_signal_value_t signal_value = 1;

bool gpu_pool(std::uint64_t handle) { return handle / 100 >= 20; }

} // namespace

extern "C" {

hsa_status_t hsa_init() {
  std::fputs("MOCK init\n", stderr);
  return HSA_STATUS_SUCCESS;
}

hsa_status_t hsa_shut_down() {
  std::fputs("MOCK shutdown\n", stderr);
  require(!signal_live && allocations.empty());
  if (mode("late_output_error"))
    require(std::freopen("/dev/full", "w", stdout) != nullptr);
  return mode("shutdown_error") ? HSA_STATUS_ERROR : HSA_STATUS_SUCCESS;
}

hsa_status_t hsa_status_string(hsa_status_t, const char **text) {
  *text = "mock status";
  return HSA_STATUS_SUCCESS;
}

hsa_status_t hsa_system_get_info(hsa_system_info_t attribute, void *output) {
  if (attribute == HSA_AMD_SYSTEM_INFO_XNACK_ENABLED)
    return result(output, mode("xnack"));
  require(attribute == HSA_SYSTEM_INFO_TIMESTAMP_FREQUENCY);
  const auto frequency = mode("frequency_zero") ? std::uint64_t{0}
                         : mode("frequency_overflow")
                             ? std::numeric_limits<std::uint64_t>::max()
                             : std::uint64_t{1000000000};
  return result(output, frequency);
}

hsa_status_t hsa_iterate_agents(hsa_status_t (*callback)(hsa_agent_t, void *),
                                void *data) {
  for (std::uint64_t handle : {10, 11, 20, 21}) {
    if (mode("missing_gpu") && handle == 21)
      continue;
    const auto status = callback(hsa_agent_t{handle}, data);
    if (status != HSA_STATUS_SUCCESS)
      return status;
  }
  return HSA_STATUS_SUCCESS;
}

hsa_status_t hsa_agent_get_info(hsa_agent_t agent, hsa_agent_info_t attribute,
                                void *output) {
  require(agent.handle == 10 || agent.handle == 11 || agent.handle == 20 ||
          agent.handle == 21);
  if (attribute == HSA_AGENT_INFO_DEVICE)
    return result(output, agent.handle < 20 ? HSA_DEVICE_TYPE_CPU
                                            : HSA_DEVICE_TYPE_GPU);
  if (attribute == static_cast<hsa_agent_info_t>(HSA_AMD_AGENT_INFO_UUID)) {
    std::strcpy(static_cast<char *>(output),
                mode("uuid") ? "GPU-0000000000000001" : "GPU-ab83d2ffef0d3cdf");
    return HSA_STATUS_SUCCESS;
  }
  if (attribute == HSA_AGENT_INFO_NAME) {
    std::strcpy(static_cast<char *>(output),
                mode("target") ? "gfx942x" : "gfx942");
    return HSA_STATUS_SUCCESS;
  }
  if (attribute ==
      static_cast<hsa_agent_info_t>(HSA_AMD_AGENT_INFO_NEAREST_CPU))
    return result(output, hsa_agent_t{11});
  if (attribute ==
      static_cast<hsa_agent_info_t>(HSA_AMD_AGENT_INFO_DRIVER_NODE_ID))
    return result(output, static_cast<std::uint32_t>(agent.handle));
  if (attribute == static_cast<hsa_agent_info_t>(HSA_AMD_AGENT_INFO_BDFID))
    return result(output, std::uint32_t{0x2600});
  require(attribute ==
          static_cast<hsa_agent_info_t>(HSA_AMD_AGENT_INFO_DOMAIN));
  return result(output, std::uint32_t{0});
}

hsa_status_t hsa_amd_agent_iterate_memory_pools(
    hsa_agent_t agent, hsa_status_t (*callback)(hsa_amd_memory_pool_t, void *),
    void *data) {
  if (agent.handle < 20)
    selected_cpu = agent.handle;
  else
    selected_gpu = agent.handle;
  // 0 is non-GLOBAL; 8 is GLOBAL but not allocatable. Invalid attribute reads
  // for either pool abort the fixture rather than silently returning zeros.
  for (std::uint64_t suffix : {0, 2, 4, 8}) {
    if (agent.handle >= 20 && suffix == 2)
      continue;
    const auto status =
        callback(hsa_amd_memory_pool_t{agent.handle * 100 + suffix}, data);
    if (status != HSA_STATUS_SUCCESS)
      return status;
  }
  if (agent.handle < 20 && mode("duplicate_pool"))
    return callback(hsa_amd_memory_pool_t{agent.handle * 100 + 12}, data);
  return HSA_STATUS_SUCCESS;
}

hsa_status_t hsa_amd_memory_pool_get_info(hsa_amd_memory_pool_t pool,
                                          hsa_amd_memory_pool_info_t attribute,
                                          void *output) {
  const auto suffix = pool.handle % 100;
  if (attribute == HSA_AMD_MEMORY_POOL_INFO_SEGMENT)
    return result(output,
                  suffix == 0 ? HSA_AMD_SEGMENT_GROUP : HSA_AMD_SEGMENT_GLOBAL);
  require(suffix != 0);
  if (attribute == HSA_AMD_MEMORY_POOL_INFO_LOCATION)
    return result(output, gpu_pool(pool.handle)
                              ? HSA_AMD_MEMORY_POOL_LOCATION_GPU
                              : HSA_AMD_MEMORY_POOL_LOCATION_CPU);
  if (attribute == HSA_AMD_MEMORY_POOL_INFO_RUNTIME_ALLOC_ALLOWED)
    return result(output, suffix != 8);
  if (attribute == HSA_AMD_MEMORY_POOL_INFO_GLOBAL_FLAGS) {
    std::uint32_t flags = suffix == 4 ? 4 : 2;
    if (mode("kernarg_pool") && suffix == 2)
      flags = 3;
    return result(output, flags);
  }
  require(suffix != 8);
  if (attribute == HSA_AMD_MEMORY_POOL_INFO_ALLOC_MAX_SIZE)
    return result(output, mode("aggregate_capacity") && !gpu_pool(pool.handle)
                              ? std::size_t{4096}
                              : std::size_t{1 << 20});
  if (attribute == HSA_AMD_MEMORY_POOL_INFO_RUNTIME_ALLOC_GRANULE)
    return result(output, std::size_t{4096});
  require(attribute == HSA_AMD_MEMORY_POOL_INFO_RUNTIME_ALLOC_ALIGNMENT);
  return result(output, std::size_t{4096});
}

hsa_status_t hsa_amd_agent_memory_pool_get_info(
    hsa_agent_t agent, hsa_amd_memory_pool_t pool,
    hsa_amd_agent_memory_pool_info_t attribute, void *output) {
  require(attribute == HSA_AMD_AGENT_MEMORY_POOL_INFO_ACCESS);
  return result(output, mode("inaccessible")
                            ? HSA_AMD_MEMORY_POOL_ACCESS_NEVER_ALLOWED
                        : mode("cpu_access_nondefault") && agent.handle < 20 &&
                                gpu_pool(pool.handle)
                            ? HSA_AMD_MEMORY_POOL_ACCESS_DISALLOWED_BY_DEFAULT
                            : HSA_AMD_MEMORY_POOL_ACCESS_ALLOWED_BY_DEFAULT);
}

hsa_status_t hsa_amd_memory_copy_engine_status(hsa_agent_t destination,
                                               hsa_agent_t source,
                                               std::uint32_t *mask) {
  const bool h2d =
      destination.handle == selected_gpu && source.handle == selected_cpu;
  const bool d2h =
      destination.handle == selected_cpu && source.handle == selected_gpu;
  require(h2d || d2h);
  std::fprintf(stderr, "MOCK engines %s\n", h2d ? "h2d" : "d2h");
  ++statuses;
  *mask = (h2d && mode("h2d_unavailable")) || (d2h && mode("d2h_unavailable"))
              ? 0
              : 3;
  return HSA_STATUS_SUCCESS;
}

hsa_status_t hsa_amd_memory_pool_allocate(hsa_amd_memory_pool_t pool,
                                          std::size_t bytes,
                                          std::uint32_t flags, void **output) {
  require(statuses == 2 && bytes == 4096 && flags == 0);
  std::fprintf(stderr, "MOCK allocate %llu\n",
               static_cast<unsigned long long>(pool.handle));
  *output = std::malloc(bytes);
  require(*output != nullptr);
  std::memset(*output, 0, bytes);
  allocations.emplace(*output, Allocation{pool.handle, bytes});
  allocation_order.push_back(*output);
  return HSA_STATUS_SUCCESS;
}

hsa_status_t hsa_amd_agents_allow_access(std::uint32_t count,
                                         const hsa_agent_t *agents,
                                         const std::uint32_t *flags,
                                         const void *pointer) {
  require(count == 2 && agents[0].handle == selected_cpu &&
          agents[1].handle == selected_gpu && flags == nullptr &&
          allocations.count(pointer) == 1);
  std::fprintf(stderr, "MOCK access %llu %llu\n",
               static_cast<unsigned long long>(agents[0].handle),
               static_cast<unsigned long long>(agents[1].handle));
  if (mode("access_error"))
    return HSA_STATUS_ERROR;
  allocations.at(pointer).access_granted = true;
  return HSA_STATUS_SUCCESS;
}

hsa_status_t hsa_signal_create(hsa_signal_value_t value, std::uint32_t count,
                               const hsa_agent_t *consumers,
                               hsa_signal_t *signal) {
  require(value == 1 && count == 0 && consumers == nullptr && !signal_live);
  signal_live = true;
  signal->handle = 42;
  return HSA_STATUS_SUCCESS;
}

hsa_status_t hsa_amd_memory_async_copy_on_engine(
    void *destination, hsa_agent_t destination_agent, const void *source,
    hsa_agent_t source_agent, std::size_t bytes, std::uint32_t dependencies,
    const hsa_signal_t *dependency_signals, hsa_signal_t signal,
    hsa_amd_sdma_engine_id_t engine, bool force) {
  require(signal_live && signal.handle == 42 && signal_value == 1 &&
          copies == waits && copies == resets && dependencies == 0 &&
          dependency_signals == nullptr && !force &&
          (engine == HSA_AMD_SDMA_ENGINE_0 || engine == HSA_AMD_SDMA_ENGINE_1));
  const auto dst = allocations.at(destination), src = allocations.at(source);
  const bool h2d = copies % 2 == 0;
  require(dst.access_granted && src.access_granted && (!h2d || prepared) &&
          bytes == dst.bytes && bytes == src.bytes &&
          gpu_pool(dst.pool) == h2d && gpu_pool(src.pool) != h2d &&
          destination_agent.handle == (h2d ? selected_gpu : selected_cpu) &&
          source_agent.handle == (h2d ? selected_cpu : selected_gpu));
  std::fprintf(stderr, "MOCK copy %s engine=%u\n", h2d ? "h2d" : "d2h",
               static_cast<unsigned>(engine));
  if (mode("submit_error"))
    return HSA_STATUS_ERROR;
  prepared = false;
  std::memcpy(destination, source, bytes);
  if (!h2d && (mode("bad_first") || mode("bad_middle") || mode("bad_last"))) {
    const auto index = mode("bad_first")    ? 0
                       : mode("bad_middle") ? bytes / 2
                                            : bytes - 1;
    static_cast<std::uint8_t *>(destination)[index] ^= 0xff;
  }
  ++copies;
  return HSA_STATUS_SUCCESS;
}

hsa_signal_value_t hsa_signal_wait_scacquire(hsa_signal_t signal,
                                             hsa_signal_condition_t condition,
                                             hsa_signal_value_t compare,
                                             std::uint64_t timeout,
                                             hsa_wait_state_t state) {
  require(signal_live && signal.handle == 42 &&
          condition == HSA_SIGNAL_CONDITION_LT && compare == 1 &&
          timeout == 60000000000ULL && state == HSA_WAIT_STATE_BLOCKED &&
          copies == waits + 1);
  std::fputs("MOCK wait\n", stderr);
  ++waits;
  signal_value = mode("wait_pending") ? 1 : mode("wait_negative") ? -1 : 0;
  return signal_value;
}

void hsa_signal_store_screlease(hsa_signal_t signal, hsa_signal_value_t value) {
  if (signal_value == 1) {
    require(signal_live && signal.handle == 42 && value == 1 && !prepared &&
            copies == waits && copies == resets && copies % 2 == 0);
    require(allocation_order.size() == 3);
    const auto expected =
        static_cast<std::uint8_t>(((copies / 2) * 67 + 1) % 251 + 1);
    const auto *upload = static_cast<const std::uint8_t *>(allocation_order[0]);
    const auto *download =
        static_cast<const std::uint8_t *>(allocation_order[1]);
    for (std::size_t index = 0; index < 4096; ++index)
      require(upload[index] == expected &&
              download[index] == (expected ^ 0xff));
    std::fputs("MOCK prepare_release\n", stderr);
    prepared = true;
    return;
  }
  require(signal_live && signal.handle == 42 && value == 1 &&
          signal_value == 0 && resets + 1 == waits);
  std::fputs("MOCK reset\n", stderr);
  signal_value = value;
  ++resets;
}

hsa_status_t hsa_signal_destroy(hsa_signal_t signal) {
  require(signal_live && signal.handle == 42 && signal_value == 1 &&
          copies == waits && copies == resets);
  std::fputs("MOCK destroy\n", stderr);
  if (mode("destroy_error"))
    return HSA_STATUS_ERROR;
  signal_live = false;
  return HSA_STATUS_SUCCESS;
}

hsa_status_t hsa_amd_memory_pool_free(void *pointer) {
  require(!signal_live && allocations.count(pointer) == 1);
  std::fputs("MOCK free\n", stderr);
  if (mode("free_error") && frees == 1)
    return HSA_STATUS_ERROR;
  allocations.erase(pointer);
  std::free(pointer);
  ++frees;
  return HSA_STATUS_SUCCESS;
}

} // extern "C"

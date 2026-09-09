#include <hsa/hsa.h>
#include <hsa/hsa_ext_amd.h>

#include <algorithm>
#include <limits>

#include "r60_pipeline_common.hpp"

#define HSA_CHECK(call)                                                        \
  do {                                                                        \
    const hsa_status_t status = (call);                                        \
    if (status != HSA_STATUS_SUCCESS) {                                        \
      const char *message = nullptr;                                          \
      hsa_status_string(status, &message);                                     \
      std::fprintf(stderr, "%s: %s\n", #call,                                 \
                    message == nullptr ? "unknown HSA error" : message);      \
      fe2o3::r60::fail("HSA API error");                                       \
    }                                                                         \
  } while (false)

namespace {

using namespace fe2o3::r60;

struct Agents {
  std::vector<hsa_agent_t> cpus;
  std::vector<hsa_agent_t> gpus;
};

hsa_status_t collect_agent(hsa_agent_t agent, void *data) {
  hsa_device_type_t type{};
  const auto status = hsa_agent_get_info(agent, HSA_AGENT_INFO_DEVICE, &type);
  if (status != HSA_STATUS_SUCCESS)
    return status;
  auto &agents = *static_cast<Agents *>(data);
  if (type == HSA_DEVICE_TYPE_GPU)
    agents.gpus.push_back(agent);
  else if (type == HSA_DEVICE_TYPE_CPU)
    agents.cpus.push_back(agent);
  return HSA_STATUS_SUCCESS;
}

struct Pools {
  hsa_agent_t gpu{};
  hsa_amd_memory_pool_t data{};
  hsa_amd_memory_pool_t kernarg{};
};

hsa_status_t collect_pool(hsa_amd_memory_pool_t pool, void *data) {
  auto &pools = *static_cast<Pools *>(data);
  hsa_amd_segment_t segment{};
  HSA_CHECK(hsa_amd_memory_pool_get_info(
      pool, HSA_AMD_MEMORY_POOL_INFO_SEGMENT, &segment));
  if (segment != HSA_AMD_SEGMENT_GLOBAL)
    return HSA_STATUS_SUCCESS;
  bool allocatable = false;
  std::uint32_t flags = 0;
  hsa_amd_memory_pool_location_t location{};
  HSA_CHECK(hsa_amd_memory_pool_get_info(
      pool, HSA_AMD_MEMORY_POOL_INFO_RUNTIME_ALLOC_ALLOWED, &allocatable));
  HSA_CHECK(hsa_amd_memory_pool_get_info(
      pool, HSA_AMD_MEMORY_POOL_INFO_GLOBAL_FLAGS, &flags));
  HSA_CHECK(hsa_amd_memory_pool_get_info(
      pool, HSA_AMD_MEMORY_POOL_INFO_LOCATION, &location));
  if (!allocatable || location != HSA_AMD_MEMORY_POOL_LOCATION_CPU ||
      (flags & HSA_AMD_MEMORY_POOL_GLOBAL_FLAG_FINE_GRAINED) == 0 ||
      (flags & HSA_AMD_MEMORY_POOL_GLOBAL_FLAG_COARSE_GRAINED) != 0)
    return HSA_STATUS_SUCCESS;
  hsa_amd_memory_pool_access_t access{};
  HSA_CHECK(hsa_amd_agent_memory_pool_get_info(
      pools.gpu, pool, HSA_AMD_AGENT_MEMORY_POOL_INFO_ACCESS, &access));
  if (access == HSA_AMD_MEMORY_POOL_ACCESS_NEVER_ALLOWED)
    return HSA_STATUS_SUCCESS;
  std::size_t maximum = 0;
  std::size_t alignment = 0;
  HSA_CHECK(hsa_amd_memory_pool_get_info(
      pool, HSA_AMD_MEMORY_POOL_INFO_ALLOC_MAX_SIZE, &maximum));
  HSA_CHECK(hsa_amd_memory_pool_get_info(
      pool, HSA_AMD_MEMORY_POOL_INFO_RUNTIME_ALLOC_ALIGNMENT, &alignment));
  if (maximum >= kBytes && alignment >= alignof(float) &&
      (pools.data.handle == 0 || pool.handle < pools.data.handle))
    pools.data = pool;
  if ((flags & HSA_AMD_MEMORY_POOL_GLOBAL_FLAG_KERNARG_INIT) != 0 &&
      maximum >= 48 && alignment >= 8 &&
      (pools.kernarg.handle == 0 || pool.handle < pools.kernarg.handle))
    pools.kernarg = pool;
  return HSA_STATUS_SUCCESS;
}

void queue_error(hsa_status_t, hsa_queue_t *, void *) {
  fail("asynchronous HSA queue error");
}

struct Kernel {
  hsa_executable_t executable{};
  std::uint64_t object = 0;
};

Kernel load_kernel(const std::vector<char> &code, hsa_agent_t gpu) {
  hsa_code_object_reader_t reader{};
  HSA_CHECK(hsa_code_object_reader_create_from_memory(code.data(), code.size(),
                                                      &reader));
  Kernel kernel;
  HSA_CHECK(hsa_executable_create_alt(HSA_PROFILE_FULL,
                                      HSA_DEFAULT_FLOAT_ROUNDING_MODE_DEFAULT,
                                      nullptr, &kernel.executable));
  hsa_loaded_code_object_t loaded{};
  HSA_CHECK(hsa_executable_load_agent_code_object(kernel.executable, gpu,
                                                  reader, nullptr, &loaded));
  HSA_CHECK(hsa_executable_freeze(kernel.executable, nullptr));
  HSA_CHECK(hsa_code_object_reader_destroy(reader));
  hsa_executable_symbol_t symbol{};
  HSA_CHECK(hsa_executable_get_symbol_by_name(kernel.executable, "vecadd.kd",
                                             &gpu, &symbol));
  HSA_CHECK(hsa_executable_symbol_get_info(
      symbol, HSA_EXECUTABLE_SYMBOL_INFO_KERNEL_OBJECT, &kernel.object));
  std::uint32_t size = 0;
  std::uint32_t alignment = 0;
  std::uint32_t group_size = 0;
  std::uint32_t private_size = 0;
  HSA_CHECK(hsa_executable_symbol_get_info(
      symbol, HSA_EXECUTABLE_SYMBOL_INFO_KERNEL_KERNARG_SEGMENT_SIZE, &size));
  HSA_CHECK(hsa_executable_symbol_get_info(
      symbol, HSA_EXECUTABLE_SYMBOL_INFO_KERNEL_KERNARG_SEGMENT_ALIGNMENT,
      &alignment));
  HSA_CHECK(hsa_executable_symbol_get_info(
      symbol, HSA_EXECUTABLE_SYMBOL_INFO_KERNEL_GROUP_SEGMENT_SIZE, &group_size));
  HSA_CHECK(hsa_executable_symbol_get_info(
      symbol, HSA_EXECUTABLE_SYMBOL_INFO_KERNEL_PRIVATE_SEGMENT_SIZE,
      &private_size));
  if (kernel.object == 0 || size != 48 || alignment != 8 || group_size != 0 ||
      private_size != 0)
    fail("HSA kernel metadata differs from the frozen vecadd ABI");
  return kernel;
}

struct Kernarg {
  float *left;
  std::uint64_t left_length;
  float *right;
  std::uint64_t right_length;
  float *output;
  std::uint64_t output_length;
};
static_assert(sizeof(Kernarg) == 48 && alignof(Kernarg) == 8);
static_assert(offsetof(Kernarg, left_length) == 8 &&
              offsetof(Kernarg, right) == 16 &&
              offsetof(Kernarg, right_length) == 24 &&
              offsetof(Kernarg, output) == 32 &&
              offsetof(Kernarg, output_length) == 40);

} // namespace

int main(int argc, char **argv) {
  const auto config = parse_config(argc, argv);
  const auto code = load_hsaco(argv[1]);
  HSA_CHECK(hsa_init());
  bool xnack = true;
  HSA_CHECK(hsa_system_get_info(HSA_AMD_SYSTEM_INFO_XNACK_ENABLED, &xnack));
  if (xnack)
    fail("HSA process requires XNACK disabled");
  Agents agents;
  HSA_CHECK(hsa_iterate_agents(collect_agent, &agents));
  if (static_cast<std::size_t>(config.device_index) >= agents.gpus.size())
    fail("HSA visible GPU index is unavailable");
  const hsa_agent_t gpu = agents.gpus[config.device_index];
  char uuid[21]{};
  char expected_uuid[21]{};
  char target[64]{};
  std::snprintf(expected_uuid, sizeof(expected_uuid), "GPU-%016llx",
                static_cast<unsigned long long>(config.unique_id));
  HSA_CHECK(hsa_agent_get_info(
      gpu, static_cast<hsa_agent_info_t>(HSA_AMD_AGENT_INFO_UUID), uuid));
  HSA_CHECK(hsa_agent_get_info(gpu, HSA_AGENT_INFO_NAME, target));
  if (std::strcmp(uuid, expected_uuid) != 0 ||
      std::strncmp(target, "gfx942", 6) != 0 ||
      (target[6] != '\0' && target[6] != ':'))
    fail("HSA UUID or gfx942 target mismatch");
  hsa_agent_t cpu{};
  HSA_CHECK(hsa_agent_get_info(
      gpu, static_cast<hsa_agent_info_t>(HSA_AMD_AGENT_INFO_NEAREST_CPU), &cpu));
  if (cpu.handle == 0 ||
      std::count_if(agents.cpus.begin(), agents.cpus.end(),
                    [&](hsa_agent_t agent) { return agent.handle == cpu.handle; }) != 1)
    fail("HSA nearest CPU is not uniquely enumerated");
  Pools pools{gpu, {}, {}};
  HSA_CHECK(hsa_amd_agent_iterate_memory_pools(cpu, collect_pool, &pools));
  if (pools.data.handle == 0 || pools.kernarg.handle == 0)
    fail("HSA nearest CPU lacks accessible fine-grained data or kernarg pools");
  const auto kernel = load_kernel(code, gpu);
  std::uint32_t queue_max = 0;
  std::uint32_t queue_min = 0;
  HSA_CHECK(hsa_agent_get_info(gpu, HSA_AGENT_INFO_QUEUE_MAX_SIZE, &queue_max));
  HSA_CHECK(hsa_agent_get_info(gpu, HSA_AGENT_INFO_QUEUE_MIN_SIZE, &queue_min));
  const auto queue_size = std::max(std::uint32_t(kDepth), queue_min);
  if (queue_size > queue_max || (queue_size & (queue_size - 1)) != 0)
    fail("HSA queue cannot hold all 64 dispatches without host waits");
  hsa_queue_t *queue = nullptr;
  HSA_CHECK(hsa_queue_create(gpu, queue_size, HSA_QUEUE_TYPE_SINGLE, queue_error,
                             nullptr, UINT32_MAX, UINT32_MAX, &queue));
  std::array<float *, 3> buffers{};
  for (auto &buffer : buffers) {
    HSA_CHECK(hsa_amd_memory_pool_allocate(pools.data, kBytes, 0,
                                           reinterpret_cast<void **>(&buffer)));
    HSA_CHECK(hsa_amd_agents_allow_access(1, &gpu, nullptr, buffer));
  }
  void *kernarg = nullptr;
  HSA_CHECK(hsa_amd_memory_pool_allocate(pools.kernarg, sizeof(Kernarg), 0,
                                         &kernarg));
  HSA_CHECK(hsa_amd_agents_allow_access(1, &gpu, nullptr, kernarg));
  if (reinterpret_cast<std::uintptr_t>(kernarg) % alignof(Kernarg) != 0)
    fail("HSA kernarg allocation is misaligned");
  const Kernarg arguments{buffers[0], kElements, buffers[1], kElements,
                          buffers[2], kElements};
  std::memcpy(kernarg, &arguments, sizeof(arguments));
  std::array<hsa_signal_t, kDepth> signals{};
  std::array<hsa_kernel_dispatch_packet_t, kDepth> packets{};
  for (std::size_t i = 0; i < kDepth; ++i) {
    HSA_CHECK(hsa_signal_create(0, 0, nullptr, &signals[i]));
    auto &packet = packets[i];
    packet.workgroup_size_x = kWorkgroup;
    packet.workgroup_size_y = 1;
    packet.workgroup_size_z = 1;
    packet.grid_size_x = kElements;
    packet.grid_size_y = 1;
    packet.grid_size_z = 1;
    packet.kernel_object = kernel.object;
    packet.kernarg_address = kernarg;
    packet.completion_signal = signals[i];
  }
  constexpr std::uint16_t header =
      (HSA_PACKET_TYPE_KERNEL_DISPATCH << HSA_PACKET_HEADER_TYPE) |
      (1U << HSA_PACKET_HEADER_BARRIER) |
      (HSA_FENCE_SCOPE_SYSTEM << HSA_PACKET_HEADER_SCACQUIRE_FENCE_SCOPE) |
      (HSA_FENCE_SCOPE_SYSTEM << HSA_PACKET_HEADER_SCRELEASE_FENCE_SCOPE);
  constexpr std::uint16_t setup =
      1U << HSA_KERNEL_DISPATCH_PACKET_SETUP_DIMENSIONS;
  constexpr std::uint32_t published_header =
      std::uint32_t(header) | (std::uint32_t(setup) << 16);
  const Workload workload;
  workload.initialize(buffers[0], buffers[1]);
  auto *ring = static_cast<hsa_kernel_dispatch_packet_t *>(queue->base_address);
  std::array<Timings, kWarmups + kSamples> samples{};
  for (auto &sample : samples) {
    workload.reset(buffers[2]);
    for (auto signal : signals) {
      if (hsa_signal_load_scacquire(signal) != 0)
        fail("HSA signal was reused before completion");
    }
    const auto write = hsa_queue_load_write_index_relaxed(queue);
    const auto read = hsa_queue_load_read_index_scacquire(queue);
    if (write < read || write - read > queue->size - kDepth ||
        write > std::numeric_limits<std::uint64_t>::max() - kDepth)
      fail("HSA queue lacks space for the entire nonblocking issue batch");
    const auto start = Clock::now();
    const auto deadline = start + std::chrono::nanoseconds(kTimeoutNs);
    for (std::size_t i = 0; i < kDepth; ++i) {
      hsa_signal_store_screlease(signals[i], 1);
      const auto id = hsa_queue_add_write_index_relaxed(queue, 1);
      auto *packet = &ring[id & (queue->size - 1)];
      // Keep the queue slot's INVALID header until the body is fully visible.
      constexpr std::size_t header_bytes = sizeof(std::uint32_t);
      std::memcpy(reinterpret_cast<std::byte *>(packet) + header_bytes,
                  reinterpret_cast<const std::byte *>(&packets[i]) + header_bytes,
                  sizeof(*packet) - header_bytes);
      __atomic_store_n(reinterpret_cast<std::uint32_t *>(&packet->header),
                        published_header, __ATOMIC_RELEASE);
      hsa_signal_store_screlease(queue->doorbell_signal,
                                 static_cast<hsa_signal_value_t>(id));
    }
    const auto issued = Clock::now();
    std::size_t polls = 0;
    for (;;) {
      const auto value = hsa_signal_load_scacquire(signals.back());
      if (value == 0)
        break;
      if (value != 1)
        fail("HSA tail completion signal has an unexpected value");
      poll_deadline(deadline, polls++);
    }
    const auto done = Clock::now();
    for (auto signal : signals)
      if (hsa_signal_load_scacquire(signal) != 0)
        fail("HSA ordered tail did not establish all 64 completions");
    sample = timings(start, issued, done);
    workload.validate(buffers[0], buffers[1], buffers[2]);
  }
  HSA_CHECK(hsa_queue_destroy(queue));
  for (auto signal : signals)
    HSA_CHECK(hsa_signal_destroy(signal));
  HSA_CHECK(hsa_amd_memory_pool_free(kernarg));
  for (auto *buffer : buffers)
    HSA_CHECK(hsa_amd_memory_pool_free(buffer));
  HSA_CHECK(hsa_executable_destroy(kernel.executable));
  HSA_CHECK(hsa_shut_down());
  report("hsa", config, samples);
  return 0;
}

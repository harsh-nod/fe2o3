#include <hsa/hsa.h>
#include <hsa/hsa_ext_amd.h>

#include <algorithm>
#include <cstddef>
#include <cstdint>
#include <cstdio>
#include <cstring>
#include <limits>
#include <vector>

#include "bounded_binary_file_reader.hpp"
#include "inplace_benchmark_common.hpp"
#include "r26_hsa_pool_policy.hpp"

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
  } while (false)

namespace {

constexpr std::streamoff kMaximumHsacoBytes = 64 * 1024 * 1024;
static_assert(fe2o3::r26::kHsaPoolFlagKernargInit ==
              HSA_AMD_MEMORY_POOL_GLOBAL_FLAG_KERNARG_INIT);
static_assert(fe2o3::r26::kHsaPoolFlagFineGrained ==
              HSA_AMD_MEMORY_POOL_GLOBAL_FLAG_FINE_GRAINED);
static_assert(fe2o3::r26::kHsaPoolFlagCoarseGrained ==
              HSA_AMD_MEMORY_POOL_GLOBAL_FLAG_COARSE_GRAINED);

struct Agents {
  std::vector<hsa_agent_t> cpus;
  std::vector<hsa_agent_t> gpus;
};

hsa_status_t collect_agent(hsa_agent_t agent, void *data) {
  hsa_device_type_t type{};
  const hsa_status_t status =
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
  auto *collector = static_cast<PoolCollector *>(data);
  hsa_amd_segment_t segment{};
  bool allowed = false;
  hsa_amd_memory_pool_location_t location{};
  std::uint32_t flags = 0;
  hsa_status_t status = hsa_amd_memory_pool_get_info(
      pool, HSA_AMD_MEMORY_POOL_INFO_SEGMENT, &segment);
  if (status != HSA_STATUS_SUCCESS)
    return status;
  status = hsa_amd_memory_pool_get_info(
      pool, HSA_AMD_MEMORY_POOL_INFO_RUNTIME_ALLOC_ALLOWED, &allowed);
  if (status != HSA_STATUS_SUCCESS)
    return status;
  status = hsa_amd_memory_pool_get_info(pool, HSA_AMD_MEMORY_POOL_INFO_LOCATION,
                                        &location);
  if (status != HSA_STATUS_SUCCESS)
    return status;
  if (segment == HSA_AMD_SEGMENT_GLOBAL) {
    status = hsa_amd_memory_pool_get_info(
        pool, HSA_AMD_MEMORY_POOL_INFO_GLOBAL_FLAGS, &flags);
    if (status != HSA_STATUS_SUCCESS)
      return status;
  }

  std::size_t maximum_single_allocation = 0;
  std::size_t allocation_granule = 0;
  std::size_t allocation_alignment = 0;
  if (allowed) {
    status = hsa_amd_memory_pool_get_info(
        pool, HSA_AMD_MEMORY_POOL_INFO_ALLOC_MAX_SIZE,
        &maximum_single_allocation);
    if (status != HSA_STATUS_SUCCESS)
      return status;
    status = hsa_amd_memory_pool_get_info(
        pool, HSA_AMD_MEMORY_POOL_INFO_RUNTIME_ALLOC_GRANULE,
        &allocation_granule);
    if (status != HSA_STATUS_SUCCESS)
      return status;
    status = hsa_amd_memory_pool_get_info(
        pool, HSA_AMD_MEMORY_POOL_INFO_RUNTIME_ALLOC_ALIGNMENT,
        &allocation_alignment);
    if (status != HSA_STATUS_SUCCESS)
      return status;
  }

  hsa_amd_memory_pool_access_t nearest_cpu_access{};
  hsa_amd_memory_pool_access_t selected_gpu_access{};
  status = hsa_amd_agent_memory_pool_get_info(
      collector->nearest_cpu, pool, HSA_AMD_AGENT_MEMORY_POOL_INFO_ACCESS,
      &nearest_cpu_access);
  if (status != HSA_STATUS_SUCCESS)
    return status;
  status = hsa_amd_agent_memory_pool_get_info(
      collector->selected_gpu, pool, HSA_AMD_AGENT_MEMORY_POOL_INFO_ACCESS,
      &selected_gpu_access);
  if (status != HSA_STATUS_SUCCESS)
    return status;

  collector->pools->push_back(CollectedPool{
      pool,
      fe2o3::r26::HsaPoolCandidate{
          pool.handle,
          collector->owner,
          pool_location(location),
          segment == HSA_AMD_SEGMENT_GLOBAL,
          allowed,
          flags,
          maximum_single_allocation,
          allocation_granule,
          allocation_alignment,
          nearest_cpu_access != HSA_AMD_MEMORY_POOL_ACCESS_NEVER_ALLOWED,
          selected_gpu_access != HSA_AMD_MEMORY_POOL_ACCESS_NEVER_ALLOWED,
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

void queue_error(hsa_status_t status, hsa_queue_t *, void *) {
  const char *text = nullptr;
  hsa_status_string(status, &text);
  std::fprintf(stderr, "asynchronous HSA queue error: %s\n",
               text == nullptr ? "unknown" : text);
  std::abort();
}

std::vector<char> read_binary(const char *path) {
  std::vector<char> bytes;
  const auto status =
      fe2o3::r26::read_bounded_binary_file(path, kMaximumHsacoBytes, &bytes);
  if (status == fe2o3::r26::BoundedBinaryFileReadStatus::OpenFailed) {
    std::fprintf(stderr, "could not open HSACO: %s\n", path);
    std::exit(2);
  }
  if (status == fe2o3::r26::BoundedBinaryFileReadStatus::InvalidSize) {
    std::fprintf(stderr, "HSACO size is empty or exceeds the limit: %s\n",
                 path);
    std::exit(2);
  }
  if (status == fe2o3::r26::BoundedBinaryFileReadStatus::SeekFailed) {
    std::fprintf(stderr, "could not seek HSACO: %s\n", path);
    std::exit(2);
  }
  if (status == fe2o3::r26::BoundedBinaryFileReadStatus::ReadFailed) {
    std::fprintf(stderr, "could not read nonempty HSACO: %s\n", path);
    std::exit(2);
  }
  if (status == fe2o3::r26::BoundedBinaryFileReadStatus::ChangedOrReadFailed) {
    std::fprintf(stderr, "HSACO changed or failed while reading: %s\n", path);
    std::exit(2);
  }
  return bytes;
}

std::uint64_t timeout_hint = 0;

void wait_for_zero(hsa_signal_t signal, const char *operation) {
  const hsa_signal_value_t value = hsa_signal_wait_scacquire(
      signal, HSA_SIGNAL_CONDITION_LT, 1, timeout_hint, HSA_WAIT_STATE_BLOCKED);
  if (value != 0) {
    std::fprintf(stderr, "%s did not complete: signal=%lld\n", operation,
                 static_cast<long long>(value));
    std::exit(3);
  }
}

struct Kernel {
  hsa_executable_t executable{};
  std::uint64_t object = 0;
  std::uint32_t kernarg_size = 0;
  std::uint32_t kernarg_alignment = 0;
  std::uint32_t group_segment_size = 0;
  std::uint32_t private_segment_size = 0;
};

Kernel load_kernel(const std::vector<char> &code_object, hsa_agent_t gpu) {
  hsa_code_object_reader_t reader{};
  HSA_CHECK(hsa_code_object_reader_create_from_memory(
      code_object.data(), code_object.size(), &reader));
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
  HSA_CHECK(hsa_executable_get_symbol_by_name(
      kernel.executable, fe2o3::r26::kKernelDescriptor, &gpu, &symbol));
  HSA_CHECK(hsa_executable_symbol_get_info(
      symbol, HSA_EXECUTABLE_SYMBOL_INFO_KERNEL_OBJECT, &kernel.object));
  HSA_CHECK(hsa_executable_symbol_get_info(
      symbol, HSA_EXECUTABLE_SYMBOL_INFO_KERNEL_KERNARG_SEGMENT_SIZE,
      &kernel.kernarg_size));
  HSA_CHECK(hsa_executable_symbol_get_info(
      symbol, HSA_EXECUTABLE_SYMBOL_INFO_KERNEL_KERNARG_SEGMENT_ALIGNMENT,
      &kernel.kernarg_alignment));
  HSA_CHECK(hsa_executable_symbol_get_info(
      symbol, HSA_EXECUTABLE_SYMBOL_INFO_KERNEL_GROUP_SEGMENT_SIZE,
      &kernel.group_segment_size));
  HSA_CHECK(hsa_executable_symbol_get_info(
      symbol, HSA_EXECUTABLE_SYMBOL_INFO_KERNEL_PRIVATE_SEGMENT_SIZE,
      &kernel.private_segment_size));
  if (kernel.object == 0 || kernel.kernarg_size != 16 ||
      kernel.kernarg_alignment != fe2o3::r26::kHsaKernargAlignment ||
      kernel.group_segment_size != 0 ||
      kernel.private_segment_size != 0) {
    std::fputs("HSA kernel metadata does not match the frozen R26 ABI\n",
               stderr);
    std::exit(2);
  }
  return kernel;
}

void publish_dispatch(hsa_queue_t *queue, const Kernel &kernel, void *kernarg,
                      hsa_signal_t completion) {
  hsa_signal_store_screlease(completion, 1);
  const std::uint64_t packet_id = hsa_queue_add_write_index_relaxed(queue, 1);
  while (packet_id - hsa_queue_load_read_index_scacquire(queue) >=
         queue->size) {
  }
  auto *ring = static_cast<hsa_kernel_dispatch_packet_t *>(queue->base_address);
  auto *packet = &ring[packet_id & (queue->size - 1)];
  std::memset(packet, 0, sizeof(*packet));
  packet->workgroup_size_x = fe2o3::r26::kWorkgroup;
  packet->workgroup_size_y = 1;
  packet->workgroup_size_z = 1;
  packet->grid_size_x = fe2o3::r26::kElements;
  packet->grid_size_y = 1;
  packet->grid_size_z = 1;
  packet->private_segment_size = kernel.private_segment_size;
  packet->group_segment_size = kernel.group_segment_size;
  packet->kernel_object = kernel.object;
  packet->kernarg_address = kernarg;
  packet->completion_signal = completion;

  const std::uint16_t header =
      (HSA_PACKET_TYPE_KERNEL_DISPATCH << HSA_PACKET_HEADER_TYPE) |
      (1U << HSA_PACKET_HEADER_BARRIER) |
      (HSA_FENCE_SCOPE_SYSTEM << HSA_PACKET_HEADER_SCACQUIRE_FENCE_SCOPE) |
      (HSA_FENCE_SCOPE_SYSTEM << HSA_PACKET_HEADER_SCRELEASE_FENCE_SCOPE);
  const std::uint16_t setup = 1U << HSA_KERNEL_DISPATCH_PACKET_SETUP_DIMENSIONS;
  const std::uint32_t header_and_setup =
      static_cast<std::uint32_t>(header) |
      (static_cast<std::uint32_t>(setup) << 16);
  __atomic_store_n(reinterpret_cast<std::uint32_t *>(&packet->header),
                   header_and_setup, __ATOMIC_RELEASE);
  hsa_signal_store_screlease(queue->doorbell_signal,
                             static_cast<hsa_signal_value_t>(packet_id));
}

} // namespace

int main(int argc, char **argv) {
  static_assert(std::chrono::steady_clock::is_steady);
  static_assert(sizeof(void *) == sizeof(std::uint64_t));
  if (argc != 4) {
    std::fprintf(stderr,
                 "usage: %s EXACT_HSACO VISIBLE_GPU_INDEX "
                 "EXPECTED_UNIQUE_ID\n",
                 argv[0]);
    return 2;
  }

  std::size_t gpu_index = 0;
  std::uint64_t expected_unique_id = 0;
  if (!fe2o3::r26::parse_index(argv[2], &gpu_index) ||
      !fe2o3::r26::parse_unique_id(argv[3], &expected_unique_id)) {
    std::fputs("invalid GPU index or expected unique ID\n", stderr);
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
      timestamp_frequency > std::numeric_limits<std::uint64_t>::max() / 60) {
    std::fputs("HSA XNACK state or timestamp frequency is unsupported\n",
               stderr);
    return 2;
  }
  timeout_hint = timestamp_frequency * 60;

  Agents agents;
  HSA_CHECK(hsa_iterate_agents(collect_agent, &agents));
  if (agents.cpus.empty() || gpu_index >= agents.gpus.size()) {
    std::fputs("requested HSA CPU/GPU agent is unavailable\n", stderr);
    return 2;
  }
  const hsa_agent_t gpu = agents.gpus[gpu_index];
  hsa_agent_t cpu{};
  HSA_CHECK(hsa_agent_get_info(
      gpu, static_cast<hsa_agent_info_t>(HSA_AMD_AGENT_INFO_NEAREST_CPU),
      &cpu));
  if (cpu.handle == 0) {
    std::fputs("HSA GPU reported a zero nearest CPU agent\n", stderr);
    return 2;
  }
  hsa_device_type_t nearest_cpu_type{};
  HSA_CHECK(hsa_agent_get_info(cpu, HSA_AGENT_INFO_DEVICE, &nearest_cpu_type));
  std::vector<std::uint64_t> enumerated_cpu_handles;
  enumerated_cpu_handles.reserve(agents.cpus.size());
  for (hsa_agent_t agent : agents.cpus)
    enumerated_cpu_handles.push_back(agent.handle);
  const auto normalized_cpu_type =
      nearest_cpu_type == HSA_DEVICE_TYPE_CPU
          ? fe2o3::r26::HsaAgentType::Cpu
          : (nearest_cpu_type == HSA_DEVICE_TYPE_GPU
                 ? fe2o3::r26::HsaAgentType::Gpu
                 : fe2o3::r26::HsaAgentType::Other);
  if (!fe2o3::r26::unique_enumerated_nearest_cpu(
          cpu.handle, normalized_cpu_type, enumerated_cpu_handles)) {
    std::fputs(
        "HSA GPU did not report one uniquely enumerated nearest CPU agent\n",
        stderr);
    return 2;
  }
  char uuid[21] = {};
  char expected_uuid[21] = {};
  char target[64] = {};
  HSA_CHECK(hsa_agent_get_info(
      gpu, static_cast<hsa_agent_info_t>(HSA_AMD_AGENT_INFO_UUID), uuid));
  HSA_CHECK(hsa_agent_get_info(gpu, HSA_AGENT_INFO_NAME, target));
  std::snprintf(expected_uuid, sizeof(expected_uuid), "GPU-%016llx",
                static_cast<unsigned long long>(expected_unique_id));
  if (std::strcmp(uuid, expected_uuid) != 0 ||
      std::strncmp(target, "gfx942", 6) != 0 ||
      (target[6] != '\0' && target[6] != ':')) {
    std::fputs("HSA GPU UUID, unique ID, or target mismatch\n", stderr);
    return 2;
  }

  const std::vector<char> code_object = read_binary(argv[1]);
  const Kernel kernel = load_kernel(code_object, gpu);

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
  if (fe2o3::r26::select_hsa_pool_roles(
          pool_facts, fe2o3::r26::kBytes, kernel.kernarg_size,
          kernel.kernarg_alignment,
          &pool_roles) != fe2o3::r26::HsaPoolPolicyStatus::Accepted) {
    std::fputs("HSA memory pools do not satisfy the exact R26 policy\n",
               stderr);
    return 2;
  }
  const hsa_amd_memory_pool_t host_pool = collected_pools[pool_roles.host].pool;
  const hsa_amd_memory_pool_t device_pool =
      collected_pools[pool_roles.device].pool;
  const hsa_amd_memory_pool_t kernarg_pool =
      collected_pools[pool_roles.kernarg].pool;
  std::size_t kernarg_pool_alignment = 0;
  HSA_CHECK(hsa_amd_memory_pool_get_info(
      kernarg_pool, HSA_AMD_MEMORY_POOL_INFO_RUNTIME_ALLOC_ALIGNMENT,
      &kernarg_pool_alignment));
  if (kernarg_pool_alignment < kernel.kernarg_alignment) {
    std::fputs("HSA kernarg pool cannot satisfy the kernel alignment\n",
               stderr);
    return 2;
  }
  std::uint32_t queue_max_size = 0;
  HSA_CHECK(
      hsa_agent_get_info(gpu, HSA_AGENT_INFO_QUEUE_MAX_SIZE, &queue_max_size));
  const std::uint32_t queue_size = std::min(UINT32_C(1024), queue_max_size);
  if (queue_size == 0 || (queue_size & (queue_size - 1)) != 0) {
    std::fputs("HSA GPU did not report a usable power-of-two queue size\n",
               stderr);
    return 2;
  }
  hsa_queue_t *queue = nullptr;
  HSA_CHECK(hsa_queue_create(gpu, queue_size, HSA_QUEUE_TYPE_SINGLE,
                             queue_error, nullptr, UINT32_MAX, UINT32_MAX,
                             &queue));

  std::uint32_t *upload = nullptr;
  std::uint32_t *download = nullptr;
  std::uint32_t *device = nullptr;
  void *kernarg = nullptr;
  HSA_CHECK(hsa_amd_memory_pool_allocate(host_pool, fe2o3::r26::kBytes, 0,
                                         reinterpret_cast<void **>(&upload)));
  HSA_CHECK(hsa_amd_memory_pool_allocate(host_pool, fe2o3::r26::kBytes, 0,
                                         reinterpret_cast<void **>(&download)));
  HSA_CHECK(hsa_amd_memory_pool_allocate(device_pool, fe2o3::r26::kBytes, 0,
                                         reinterpret_cast<void **>(&device)));
  HSA_CHECK(hsa_amd_memory_pool_allocate(kernarg_pool, kernel.kernarg_size, 0,
                                         &kernarg));
  if (reinterpret_cast<std::uintptr_t>(kernarg) % kernel.kernarg_alignment !=
      0) {
    std::fputs("HSA kernarg allocation is misaligned\n", stderr);
    return 2;
  }
  HSA_CHECK(hsa_amd_agents_allow_access(1, &gpu, nullptr, upload));
  HSA_CHECK(hsa_amd_agents_allow_access(1, &gpu, nullptr, download));
  HSA_CHECK(hsa_amd_agents_allow_access(1, &cpu, nullptr, device));
  HSA_CHECK(hsa_amd_agents_allow_access(1, &gpu, nullptr, kernarg));
  std::memcpy(static_cast<std::byte *>(kernarg), &device, sizeof(device));
  const std::uint64_t length = fe2o3::r26::kElements;
  std::memcpy(static_cast<std::byte *>(kernarg) + 8, &length, sizeof(length));

  hsa_signal_t copy_signal{};
  hsa_signal_t dispatch_signal{};
  HSA_CHECK(hsa_signal_create(1, 0, nullptr, &copy_signal));
  HSA_CHECK(hsa_signal_create(1, 0, nullptr, &dispatch_signal));

  std::vector<std::uint32_t> pattern_a;
  std::vector<std::uint32_t> pattern_b;
  fe2o3::r26::initialize_inputs(&pattern_a, &pattern_b);
  const auto iteration = [&](std::size_t ordinal) {
    const auto &input = (ordinal & 1U) == 0 ? pattern_a : pattern_b;
    std::memcpy(upload, input.data(), fe2o3::r26::kBytes);
    fe2o3::r26::Timings timings;
    const auto e2e_start = std::chrono::steady_clock::now();

    const auto h2d_start = std::chrono::steady_clock::now();
    hsa_signal_store_screlease(copy_signal, 1);
    HSA_CHECK(hsa_amd_memory_async_copy(
        device, gpu, upload, cpu, fe2o3::r26::kBytes, 0, nullptr, copy_signal));
    wait_for_zero(copy_signal, "H2D");
    const auto h2d_end = std::chrono::steady_clock::now();

    const auto compute_start = std::chrono::steady_clock::now();
    publish_dispatch(queue, kernel, kernarg, dispatch_signal);
    wait_for_zero(dispatch_signal, "compute");
    const auto compute_end = std::chrono::steady_clock::now();

    const auto d2h_start = std::chrono::steady_clock::now();
    hsa_signal_store_screlease(copy_signal, 1);
    HSA_CHECK(hsa_amd_memory_async_copy(download, cpu, device, gpu,
                                        fe2o3::r26::kBytes, 0, nullptr,
                                        copy_signal));
    wait_for_zero(copy_signal, "D2H");
    const auto d2h_end = std::chrono::steady_clock::now();

    const auto e2e_end = std::chrono::steady_clock::now();
    timings.h2d_ns = fe2o3::r26::elapsed_ns(h2d_start, h2d_end);
    timings.compute_ns = fe2o3::r26::elapsed_ns(compute_start, compute_end);
    timings.d2h_ns = fe2o3::r26::elapsed_ns(d2h_start, d2h_end);
    timings.e2e_ns = fe2o3::r26::elapsed_ns(e2e_start, e2e_end);
    if (!fe2o3::r26::validate(download, input, ordinal, "HSA"))
      std::exit(3);
    return timings;
  };

  for (std::size_t warmup = 0; warmup < fe2o3::r26::kWarmups; ++warmup) {
    (void)iteration(warmup);
  }
  fe2o3::r26::Samples samples;
  std::size_t ordinal = fe2o3::r26::kWarmups;
  for (std::size_t sample = 0; sample < fe2o3::r26::kSamples; ++sample) {
    fe2o3::r26::Timings total;
    for (std::size_t inner = 0; inner < fe2o3::r26::kIterationsPerSample;
         ++inner, ++ordinal) {
      const auto observed = iteration(ordinal);
      total.h2d_ns += observed.h2d_ns;
      total.compute_ns += observed.compute_ns;
      total.d2h_ns += observed.d2h_ns;
      total.e2e_ns += observed.e2e_ns;
    }
    fe2o3::r26::append_average(&samples, total);
  }

  HSA_CHECK(hsa_signal_destroy(dispatch_signal));
  HSA_CHECK(hsa_signal_destroy(copy_signal));
  HSA_CHECK(hsa_amd_memory_pool_free(kernarg));
  HSA_CHECK(hsa_amd_memory_pool_free(device));
  HSA_CHECK(hsa_amd_memory_pool_free(download));
  HSA_CHECK(hsa_amd_memory_pool_free(upload));
  HSA_CHECK(hsa_queue_destroy(queue));
  HSA_CHECK(hsa_executable_destroy(kernel.executable));
  HSA_CHECK(hsa_shut_down());
  fe2o3::r26::report("hsa", "n/a", "host-staged-one-buffer", "n/a", gpu_index,
                     expected_unique_id, samples);
  return 0;
}

#include <hsa/hsa.h>
#include <hsa/hsa_ext_amd.h>

#include <algorithm>
#include <cstddef>
#include <cstdint>
#include <cstdio>
#include <cstring>
#include <fstream>
#include <iterator>
#include <limits>
#include <vector>

#include "inplace_benchmark_common.hpp"

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

enum class PoolKind { Host, Device, Kernarg };

struct PoolSelection {
  PoolKind kind;
  bool found = false;
  hsa_amd_memory_pool_t pool{};
};

hsa_status_t choose_pool(hsa_amd_memory_pool_t pool, void *data) {
  auto *selection = static_cast<PoolSelection *>(data);
  hsa_amd_segment_t segment{};
  bool allowed = false;
  hsa_amd_memory_pool_location_t location{};
  uint32_t flags = 0;
  hsa_status_t status = hsa_amd_memory_pool_get_info(
      pool, HSA_AMD_MEMORY_POOL_INFO_SEGMENT, &segment);
  if (status != HSA_STATUS_SUCCESS)
    return status;
  status = hsa_amd_memory_pool_get_info(
      pool, HSA_AMD_MEMORY_POOL_INFO_RUNTIME_ALLOC_ALLOWED, &allowed);
  if (status != HSA_STATUS_SUCCESS)
    return status;
  if (segment != HSA_AMD_SEGMENT_GLOBAL || !allowed)
    return HSA_STATUS_SUCCESS;
  status = hsa_amd_memory_pool_get_info(pool, HSA_AMD_MEMORY_POOL_INFO_LOCATION,
                                        &location);
  if (status != HSA_STATUS_SUCCESS)
    return status;
  status = hsa_amd_memory_pool_get_info(
      pool, HSA_AMD_MEMORY_POOL_INFO_GLOBAL_FLAGS, &flags);
  if (status != HSA_STATUS_SUCCESS)
    return status;

  const bool matches =
      (selection->kind == PoolKind::Device &&
       location == HSA_AMD_MEMORY_POOL_LOCATION_GPU &&
       (flags & HSA_AMD_MEMORY_POOL_GLOBAL_FLAG_COARSE_GRAINED) != 0) ||
      (selection->kind == PoolKind::Host &&
       location == HSA_AMD_MEMORY_POOL_LOCATION_CPU &&
       (flags & HSA_AMD_MEMORY_POOL_GLOBAL_FLAG_FINE_GRAINED) != 0) ||
      (selection->kind == PoolKind::Kernarg &&
       location == HSA_AMD_MEMORY_POOL_LOCATION_CPU &&
       (flags & HSA_AMD_MEMORY_POOL_GLOBAL_FLAG_KERNARG_INIT) != 0);
  if (!matches)
    return HSA_STATUS_SUCCESS;
  selection->pool = pool;
  selection->found = true;
  return HSA_STATUS_INFO_BREAK;
}

void select_pool(hsa_agent_t agent, PoolSelection *selection) {
  const hsa_status_t status =
      hsa_amd_agent_iterate_memory_pools(agent, choose_pool, selection);
  if (status != HSA_STATUS_INFO_BREAK)
    HSA_CHECK(status);
  if (!selection->found) {
    std::fputs("required HSA memory pool was not found\n", stderr);
    std::exit(2);
  }
}

void queue_error(hsa_status_t status, hsa_queue_t *, void *) {
  const char *text = nullptr;
  hsa_status_string(status, &text);
  std::fprintf(stderr, "asynchronous HSA queue error: %s\n",
               text == nullptr ? "unknown" : text);
  std::abort();
}

std::vector<char> read_binary(const char *path) {
  std::ifstream input(path, std::ios::binary);
  if (!input) {
    std::fprintf(stderr, "could not open HSACO: %s\n", path);
    std::exit(2);
  }
  std::vector<char> bytes((std::istreambuf_iterator<char>(input)),
                          std::istreambuf_iterator<char>());
  if (!input.eof() || bytes.empty()) {
    std::fprintf(stderr, "could not read nonempty HSACO: %s\n", path);
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
      kernel.executable, fe2o3::r26::kKernel, &gpu, &symbol));
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
      kernel.kernarg_alignment != 8 || kernel.group_segment_size != 0 ||
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
  const bool cpu_is_enumerated =
      std::any_of(agents.cpus.begin(), agents.cpus.end(), [&](hsa_agent_t agent) {
        return agent.handle == cpu.handle;
      });
  if (cpu.handle == 0 || !cpu_is_enumerated) {
    std::fputs("HSA GPU did not report an enumerated nearest CPU agent\n",
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

  PoolSelection host_pool{PoolKind::Host};
  PoolSelection device_pool{PoolKind::Device};
  PoolSelection kernarg_pool{PoolKind::Kernarg};
  select_pool(cpu, &host_pool);
  select_pool(gpu, &device_pool);
  select_pool(cpu, &kernarg_pool);

  const std::vector<char> code_object = read_binary(argv[1]);
  const Kernel kernel = load_kernel(code_object, gpu);
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
  HSA_CHECK(hsa_amd_memory_pool_allocate(host_pool.pool, fe2o3::r26::kBytes, 0,
                                         reinterpret_cast<void **>(&upload)));
  HSA_CHECK(hsa_amd_memory_pool_allocate(host_pool.pool, fe2o3::r26::kBytes, 0,
                                         reinterpret_cast<void **>(&download)));
  HSA_CHECK(hsa_amd_memory_pool_allocate(device_pool.pool, fe2o3::r26::kBytes,
                                         0,
                                         reinterpret_cast<void **>(&device)));
  HSA_CHECK(hsa_amd_memory_pool_allocate(kernarg_pool.pool, kernel.kernarg_size,
                                         0, &kernarg));
  HSA_CHECK(hsa_amd_agents_allow_access(1, &gpu, nullptr, upload));
  HSA_CHECK(hsa_amd_agents_allow_access(1, &gpu, nullptr, download));
  HSA_CHECK(hsa_amd_agents_allow_access(1, &gpu, nullptr, device));
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

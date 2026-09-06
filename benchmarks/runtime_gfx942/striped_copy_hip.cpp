#include <hip/hip_runtime.h>

#include "striped_copy_benchmark_common.hpp"

#include <chrono>
#include <cstdint>
#include <cstdio>
#include <cstdlib>
#include <cstring>
#include <string>
#include <vector>

#define HIP_CHECK(call)                                                        \
  do {                                                                         \
    const hipError_t status_ = (call);                                         \
    if (status_ != hipSuccess) {                                               \
      std::fprintf(stderr, "%s failed: %s\n", #call,                           \
                   hipGetErrorString(status_));                                \
      std::exit(2);                                                            \
    }                                                                          \
  } while (0)

namespace {

bool uuid_matches(const hipUUID &uuid, std::uint64_t expected) {
  char ascii[17] = {};
  std::snprintf(ascii, sizeof(ascii), "%016llx",
                static_cast<unsigned long long>(expected));
  return std::memcmp(uuid.bytes, ascii, 16) == 0;
}

bool target_matches(const char *target) {
  return std::strncmp(target, "gfx942", 6) == 0 &&
         std::strstr(target, ":xnack-") != nullptr;
}

} // namespace

int main(int argc, char **argv) {
  fe2o3::r40::Config config;
  if (!fe2o3::r40::parse_config(argc, argv, &config)) {
    std::fputs(
        "usage: striped-copy-hip <device-index> <unique-id> <bytes> <depth> "
        "<warmups> <samples> <logical-queue-count> <profile>\n",
        stderr);
    return 2;
  }
  HIP_CHECK(hipSetDevice(config.device_index));
  hipUUID uuid{};
  hipDeviceProp_t properties{};
  HIP_CHECK(hipDeviceGetUuid(&uuid, config.device_index));
  HIP_CHECK(hipGetDeviceProperties(&properties, config.device_index));
  if (!uuid_matches(uuid, config.unique_id) ||
      !target_matches(properties.gcnArchName)) {
    std::fputs("HIP device identity or target mismatch\n", stderr);
    return 2;
  }

  std::vector<hipStream_t> streams(config.logical_queue_count);
  std::vector<std::uint8_t *> upload(config.depth), download(config.depth);
  std::vector<void *> device(config.depth);
  for (hipStream_t &stream : streams)
    HIP_CHECK(hipStreamCreateWithFlags(&stream, hipStreamNonBlocking));
  for (std::size_t request = 0; request < config.depth; ++request) {
    HIP_CHECK(hipHostMalloc(reinterpret_cast<void **>(&upload[request]),
                            config.bytes));
    HIP_CHECK(hipHostMalloc(reinterpret_cast<void **>(&download[request]),
                            config.bytes));
    HIP_CHECK(hipMalloc(&device[request], config.bytes));
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
      const auto t0 = std::chrono::steady_clock::now();
      for (const std::size_t request : order) {
        const std::size_t lane =
            (submission_ordinal + request) % config.logical_queue_count;
        HIP_CHECK(hipMemcpyAsync(
            upload_direction ? device[request] : download[request],
            upload_direction ? static_cast<void *>(upload[request])
                             : device[request],
            config.bytes,
            upload_direction ? hipMemcpyHostToDevice : hipMemcpyDeviceToHost,
            streams[lane]));
      }
      const auto t1 = std::chrono::steady_clock::now();
      for (std::size_t lane_offset = 0;
           lane_offset < config.logical_queue_count; ++lane_offset) {
        const std::size_t lane =
            (submission_ordinal + lane_offset) % config.logical_queue_count;
        HIP_CHECK(hipStreamSynchronize(streams[lane]));
      }
      const auto t2 = std::chrono::steady_clock::now();
      if (round >= config.warmups &&
          !samples->append(fe2o3::r40::elapsed_ns(t0, t1),
                           fe2o3::r40::elapsed_ns(t1, t2)))
        std::exit(3);
    };
    run_phase(true, submission_ordinal, &h2d);
    submission_ordinal = (submission_ordinal + 1) % config.logical_queue_count;
    run_phase(false, submission_ordinal, &d2h);
    submission_ordinal = (submission_ordinal + 1) % config.logical_queue_count;
    if (!fe2o3::r40::validate_buffers(download, config.bytes, round, "hip"))
      return 3;
  }

  for (std::size_t request = 0; request < config.depth; ++request) {
    HIP_CHECK(hipFree(device[request]));
    HIP_CHECK(hipHostFree(upload[request]));
    HIP_CHECK(hipHostFree(download[request]));
  }
  for (const hipStream_t stream : streams)
    HIP_CHECK(hipStreamDestroy(stream));
  const std::string resource_profile =
      "nonblocking-streams-q" + std::to_string(config.logical_queue_count);
  fe2o3::r40::report_native("hip", "hip", resource_profile.c_str(), "n/a",
                            config, h2d, d2h);
  return 0;
}

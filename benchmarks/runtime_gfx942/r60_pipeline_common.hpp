#ifndef FE2O3_RUNTIME_GFX942_R60_PIPELINE_COMMON_HPP
#define FE2O3_RUNTIME_GFX942_R60_PIPELINE_COMMON_HPP

#include <openssl/evp.h>

#include <array>
#include <atomic>
#include <chrono>
#include <cstddef>
#include <cstdint>
#include <cstdio>
#include <cstdlib>
#include <cstring>
#include <string>
#include <thread>
#include <vector>

#include "bounded_binary_file_reader.hpp"
#include "native_benchmark_args.hpp"

namespace fe2o3::r60 {

constexpr std::size_t kElements = 1048576;
constexpr std::size_t kBytes = kElements * sizeof(float);
constexpr unsigned kWorkgroup = 256;
constexpr std::size_t kDepth = 64;
constexpr std::size_t kWarmups = 10;
constexpr std::size_t kSamples = 30;
constexpr std::uint64_t kTimeoutNs = 10000000000ULL;
constexpr char kHsacoSha256[] =
    "3a25e364dd1e1931d1a16c24b37aa998df2c6ef1cbcf0ec2afb6372cbc878bab";
constexpr char kOutputSha256[] =
    "79fd0768604fe9de0ced87297f7d653343e998926b59e2cce7df5e38194c52b3";
using Clock = std::chrono::steady_clock;
static_assert(Clock::is_steady);
static_assert(sizeof(float) == 4 && sizeof(void *) == 8);

[[noreturn]] inline void fail(const char *message) {
  std::fprintf(stderr, "R60 benchmark failed: %s\n", message);
  // A failed batch may still own live GPU work. Let process teardown reclaim it.
  std::exit(2);
}

inline bool hex_string(const char *text, std::size_t length) {
  if (std::strlen(text) != length)
    return false;
  for (std::size_t i = 0; i < length; ++i)
    if (!((text[i] >= '0' && text[i] <= '9') ||
          (text[i] >= 'a' && text[i] <= 'f')))
      return false;
  return true;
}

struct Config {
  int device_index = 0;
  std::uint64_t unique_id = 0;
  const char *source_commit = nullptr;
  const char *run_id = nullptr;
};

inline Config parse_config(int argc, char **argv) {
  if (argc != 6) {
    std::fprintf(stderr,
                 "usage: %s EXACT_HSACO VISIBLE_DEVICE_INDEX "
                 "EXPECTED_UNIQUE_ID SOURCE_COMMIT RUN_ID\n", argv[0]);
    std::exit(2);
  }
  Config config;
  if (!runtime_gfx942::parse_device_index(argv[2], &config.device_index) ||
      !runtime_gfx942::parse_unique_id(argv[3], &config.unique_id) ||
      config.unique_id == 0 || !hex_string(argv[4], 40) ||
      !hex_string(argv[5], 64))
    fail("invalid device index, unique ID, source commit, or run ID");
  config.source_commit = argv[4];
  config.run_id = argv[5];
  return config;
}

inline std::string sha256(const void *bytes, std::size_t size) {
  std::array<unsigned char, EVP_MAX_MD_SIZE> digest{};
  unsigned length = 0;
  if (EVP_Digest(bytes, size, digest.data(), &length, EVP_sha256(), nullptr) !=
          1 ||
      length != 32)
    fail("SHA-256 failed");
  std::string result(64, '0');
  constexpr char digits[] = "0123456789abcdef";
  for (std::size_t i = 0; i < 32; ++i) {
    result[2 * i] = digits[digest[i] >> 4];
    result[2 * i + 1] = digits[digest[i] & 15];
  }
  return result;
}

inline std::vector<char> load_hsaco(const char *path) {
  std::vector<char> bytes;
  if (r26::read_bounded_binary_file(path, 64 * 1024 * 1024, &bytes) !=
          r26::BoundedBinaryFileReadStatus::Success ||
      sha256(bytes.data(), bytes.size()) != kHsacoSha256)
    fail("HSACO is unreadable or differs from trusted-gfx942-vecadd-v1");
  return bytes;
}

struct Workload {
  std::vector<float> a = std::vector<float>(kElements);
  std::vector<float> b = std::vector<float>(kElements);
  std::vector<float> expected = std::vector<float>(kElements);
  std::vector<std::uint32_t> sentinel =
      std::vector<std::uint32_t>(kElements, 0x7fc00000);

  Workload() {
    for (std::size_t i = 0; i < kElements; ++i) {
      a[i] = static_cast<float>(i % 1024) / 2.0F;
      b[i] = static_cast<float>(i % 256) / 4.0F;
      expected[i] = a[i] + b[i];
    }
    if (sha256(expected.data(), kBytes) != kOutputSha256)
      fail("host arithmetic or byte order differs from the frozen workload");
  }

  void initialize(float *left, float *right) const {
    std::memcpy(left, a.data(), kBytes);
    std::memcpy(right, b.data(), kBytes);
  }

  void reset(float *output) const {
    std::memcpy(output, sentinel.data(), kBytes);
    std::atomic_thread_fence(std::memory_order_release);
  }

  void validate(const float *left, const float *right,
                const float *output) const {
    std::atomic_thread_fence(std::memory_order_acquire);
    if (std::memcmp(left, a.data(), kBytes) != 0 ||
        std::memcmp(right, b.data(), kBytes) != 0 ||
        std::memcmp(output, expected.data(), kBytes) != 0 ||
        sha256(output, kBytes) != kOutputSha256)
      fail("byte-exact output or read-only input validation failed");
  }
};

struct Timings {
  std::uint64_t issue = 0;
  std::uint64_t tail = 0;
  std::uint64_t total = 0;
};

inline std::uint64_t elapsed(Clock::time_point begin, Clock::time_point end) {
  const auto ns =
      std::chrono::duration_cast<std::chrono::nanoseconds>(end - begin).count();
  if (ns <= 0 || static_cast<std::uint64_t>(ns) > kTimeoutNs)
    fail("nonpositive or over-deadline duration");
  return static_cast<std::uint64_t>(ns);
}

inline Timings timings(Clock::time_point start, Clock::time_point issued,
                       Clock::time_point done) {
  return {elapsed(start, issued), elapsed(issued, done), elapsed(start, done)};
}

inline void poll_deadline(Clock::time_point deadline, std::size_t iteration) {
  if (Clock::now() >= deadline)
    fail("GPU batch exceeded the 10-second deadline");
  if ((iteration & 4095U) == 4095U)
    std::this_thread::yield();
}

inline void report(const char *backend, const Config &config,
                   const std::array<Timings, kWarmups + kSamples> &samples) {
  std::printf(
      "{\"record\":\"config\",\"schema\":\"fe2o3.r60-pipeline-benchmark.v1\","
      "\"backend\":\"%s\",\"run_id\":\"%s\",\"source_commit\":\"%s\","
      "\"unique_id\":\"%016llx\",\"target\":\"gfx942:xnack-\","
      "\"hsaco_sha256\":\"%s\",\"workload\":\"trusted-gfx942-vecadd-v1\","
      "\"elements\":1048576,\"bytes_per_buffer\":4194304,"
      "\"grid\":[1048576,1,1],\"workgroup\":[256,1,1],"
      "\"access\":[\"read\",\"read\",\"write\"],"
      "\"memory\":\"host-visible-coherent\",\"launches_per_batch\":64,"
      "\"warmups\":10,\"samples\":30,\"ordering\":\"same-stream-ordered\","
      "\"issue_api_calls_per_batch\":%u,\"host_waits_during_issue\":0,"
      "\"reset_timed\":false,\"explicit_allocation_api_timed\":false,"
      "\"validation\":\"byte-exact-every-batch\","
      "\"clock\":\"steady-monotonic-ns\",\"wait_timeout_ns\":10000000000}\n",
      backend, config.run_id, config.source_commit,
      static_cast<unsigned long long>(config.unique_id), kHsacoSha256,
      std::strcmp(backend, "hip") == 0 ? 64U : 192U);
  for (std::size_t i = 0; i < samples.size(); ++i) {
    const auto &sample = samples[i];
    std::printf(
        "{\"record\":\"batch\",\"phase\":\"%s\",\"index\":%zu,"
        "\"issued_launches\":64,\"completed_launches\":64,"
        "\"issue_batch_ns\":%llu,\"tail_wait_batch_ns\":%llu,"
        "\"total_batch_ns\":%llu,\"output_sha256\":\"%s\"}\n",
        i < kWarmups ? "warmup" : "sample",
        i < kWarmups ? i : i - kWarmups,
        static_cast<unsigned long long>(sample.issue),
        static_cast<unsigned long long>(sample.tail),
        static_cast<unsigned long long>(sample.total), kOutputSha256);
  }
  std::puts("{\"record\":\"complete\",\"validated_batches\":40}");
  if (std::fflush(stdout) != 0 || std::ferror(stdout))
    fail("could not write benchmark evidence");
}

} // namespace fe2o3::r60

#endif

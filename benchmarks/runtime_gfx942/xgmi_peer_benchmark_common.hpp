#ifndef FE2O3_RUNTIME_GFX942_XGMI_PEER_BENCHMARK_COMMON_HPP
#define FE2O3_RUNTIME_GFX942_XGMI_PEER_BENCHMARK_COMMON_HPP

#include "native_benchmark_args.hpp"

#include <algorithm>
#include <cstring>

namespace fe2o3::runtime_gfx942 {

inline constexpr std::size_t peer_canary_bytes = 32;

struct PeerBenchmarkControls {
  bool persistent_hot = false;
  std::size_t allocation_bytes = 0;
  std::size_t copy_offset = 0;
  std::size_t hot_pattern_round = 0;
};

inline bool parse_peer_controls(const char *optional_flag,
                                const WorkloadShape &workload,
                                PeerBenchmarkControls *result) {
  std::size_t iterations = 0;
  if (result == nullptr || workload.bytes == 0 || workload.depth == 0 ||
      workload.samples == 0 ||
      !checked_add(workload.warmups, workload.samples, &iterations) ||
      iterations != workload.total_iterations)
    return false;

  PeerBenchmarkControls parsed;
  parsed.allocation_bytes = workload.bytes;
  if (optional_flag != nullptr) {
    if (std::strcmp(optional_flag, "--persistent-hot") != 0 ||
        workload.depth != 1 ||
        !checked_add(workload.bytes, 2 * peer_canary_bytes,
                     &parsed.allocation_bytes) ||
        !checked_add(iterations, 1, &parsed.hot_pattern_round))
      return false;
    parsed.persistent_hot = true;
    parsed.copy_offset = peer_canary_bytes;
  }
  *result = parsed;
  return true;
}

inline std::uint8_t peer_pattern(std::size_t round, std::size_t slot,
                                  std::size_t direction) {
  // Reduce first to match the Rust u128 expression without size_t overflow.
  return static_cast<std::uint8_t>(
      ((round % 251) * 67 + (slot % 251) * 29 +
       (direction % 251) * 101 + 1) %
          251 +
      1);
}

inline std::uint8_t peer_source_canary(std::size_t direction) {
  return direction == 0 ? 0x17 : 0x71;
}

inline std::uint8_t peer_destination_canary(std::size_t direction) {
  return direction == 0 ? 0xa5 : 0x5a;
}

inline bool fill_peer_guarded(std::uint8_t *data, std::size_t total,
                              std::size_t bytes, std::uint8_t outer,
                              std::uint8_t inner) {
  std::size_t expected_total = 0;
  if (data == nullptr || bytes == 0 ||
      !checked_add(bytes, 2 * peer_canary_bytes, &expected_total) ||
      total != expected_total)
    return false;
  std::fill_n(data, total, outer);
  std::fill_n(data + peer_canary_bytes, bytes, inner);
  return true;
}

inline bool validate_peer_guarded(const std::uint8_t *data, std::size_t total,
                                  std::size_t bytes, std::uint8_t outer,
                                  std::uint8_t inner) {
  std::size_t expected_total = 0;
  if (data == nullptr || bytes == 0 ||
      !checked_add(bytes, 2 * peer_canary_bytes, &expected_total) ||
      total != expected_total)
    return false;
  const auto *payload = data + peer_canary_bytes;
  const auto *suffix = payload + bytes;
  return std::all_of(data, payload,
                     [outer](std::uint8_t byte) { return byte == outer; }) &&
         std::all_of(payload, suffix,
                     [inner](std::uint8_t byte) { return byte == inner; }) &&
         std::all_of(suffix, data + total,
                     [outer](std::uint8_t byte) { return byte == outer; });
}

template <typename Prepare, typename Copy, typename Validate,
          typename RecordSample>
bool run_peer_persistent_hot(const WorkloadShape &workload, Prepare prepare,
                             Copy copy, Validate validate,
                             RecordSample record_sample) {
  PeerBenchmarkControls controls;
  if (!parse_peer_controls("--persistent-hot", workload, &controls) ||
      !prepare(0) || !prepare(1))
    return false;

  // Neither preparation nor readback may interrupt the primed copy sequence.
  copy(0);
  copy(1);
  for (std::size_t round = 0; round < workload.total_iterations; ++round) {
    const std::uint64_t forward = copy(0);
    const std::uint64_t reverse = copy(1);
    if (round >= workload.warmups)
      record_sample(forward, reverse);
  }
  const bool forward_valid = validate(0);
  const bool reverse_valid = validate(1);
  return forward_valid && reverse_valid;
}

} // namespace fe2o3::runtime_gfx942

#endif

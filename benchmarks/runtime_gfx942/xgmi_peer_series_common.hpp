#ifndef FE2O3_RUNTIME_GFX942_XGMI_PEER_SERIES_COMMON_HPP
#define FE2O3_RUNTIME_GFX942_XGMI_PEER_SERIES_COMMON_HPP

#include "xgmi_peer_benchmark_common.hpp"

namespace fe2o3::runtime_gfx942 {

// A separate order contract: one complete forward series, then one reverse
// series. The historical alternating-round helper is deliberately unchanged.
template <typename Prepare, typename Copy, typename Validate,
          typename RecordSample>
bool run_peer_persistent_series(const WorkloadShape &workload, Prepare prepare,
                                Copy copy, Validate validate,
                                RecordSample record_sample) {
  PeerBenchmarkControls controls;
  if (!parse_peer_controls("--persistent-hot", workload, &controls) ||
      !prepare(0) || !prepare(1))
    return false;

  for (std::size_t direction = 0; direction < 2; ++direction) {
    copy(direction);
    for (std::size_t round = 0; round < workload.total_iterations; ++round) {
      const std::uint64_t elapsed = copy(direction);
      if (round >= workload.warmups)
        record_sample(direction, elapsed);
    }
  }
  const bool forward_valid = validate(0);
  const bool reverse_valid = validate(1);
  return forward_valid && reverse_valid;
}

} // namespace fe2o3::runtime_gfx942

#endif

#ifndef FE2O3_RUNTIME_GFX942_XGMI_PEER_SEGMENTS_COMMON_HPP
#define FE2O3_RUNTIME_GFX942_XGMI_PEER_SEGMENTS_COMMON_HPP

#include "native_benchmark_args.hpp"

#include <algorithm>
#include <chrono>
#include <cstdio>
#include <cstdlib>
#include <utility>
#include <vector>

namespace fe2o3::runtime_gfx942 {

struct PeerSegment {
  std::size_t source_offset;
  std::size_t destination_offset;
  std::size_t bytes;
};

struct PeerSegmentPlan {
  std::size_t useful_bytes = 0;
  std::size_t warmups = 0;
  std::size_t samples = 0;
  std::size_t band_bytes = 0;
  std::size_t destination_bytes = 0;
  std::size_t bands = 0;
  std::vector<PeerSegment> segments;
};

inline bool make_peer_segment_plan(std::size_t bytes, std::size_t count,
                                   std::size_t warmups, std::size_t samples,
                                   PeerSegmentPlan *result) {
  std::size_t rounds = 0;
  if (result == nullptr || bytes == 0 || bytes > 2 * 1024 * 1024 ||
      count == 0 || count > 4096 || bytes < count || samples == 0 ||
      !checked_add(warmups, samples, &rounds) || rounds > 64)
    return false;
  PeerSegmentPlan plan;
  plan.useful_bytes = bytes;
  plan.warmups = warmups;
  plan.samples = samples;
  plan.bands = rounds + 1;
  // Fixed-width slots permit a reversed scatter with unequal lengths and gaps.
  const auto quotient = bytes / count;
  const auto remainder = bytes % count;
  const auto slot_bytes = ((quotient + 2 + 16 + 63) / 64) * 64;
  plan.band_bytes = ((64 + count * slot_bytes + 4095) / 4096) * 4096;
  if (!checked_multiply(plan.bands, plan.band_bytes, &plan.destination_bytes) ||
      plan.destination_bytes > 256 * 1024 * 1024)
    return false;
  plan.segments.reserve(count);
  for (std::size_t i = 0; i < count; ++i) {
    auto length = quotient + static_cast<std::size_t>(i < remainder);
    if (quotient >= 2 && count > 1) {
      if (i % 2 == 0 && i + 1 < count)
        ++length;
      else if (i % 2 == 1)
        --length;
    }
    plan.segments.push_back({32 + i * slot_bytes,
                             32 + (count - 1 - i) * slot_bytes, length});
  }
  *result = std::move(plan);
  return true;
}

inline std::uint8_t segment_source_byte(std::size_t position,
                                        std::size_t direction) {
  return static_cast<std::uint8_t>(
      ((position % 251) * 131 + ((position / 256) % 251) * 17 +
       direction * 73 + 1) % 251 + 1);
}

inline std::vector<std::uint8_t> segment_source(const PeerSegmentPlan &plan,
                                               std::size_t direction) {
  std::vector<std::uint8_t> result(plan.band_bytes);
  for (std::size_t i = 0; i < result.size(); ++i)
    result[i] = segment_source_byte(i, direction);
  return result;
}

inline std::vector<std::uint8_t> segment_destination(
    const PeerSegmentPlan &plan, std::size_t direction, bool completed) {
  std::vector<std::uint8_t> result(plan.destination_bytes,
                                  direction == 0 ? 0xa5 : 0x5a);
  for (std::size_t band = 0; band < plan.bands; ++band)
    for (const auto &segment : plan.segments)
      for (std::size_t byte = 0; byte < segment.bytes; ++byte) {
        const auto expected = segment_source_byte(segment.source_offset + byte,
                                                   direction);
        result[band * plan.band_bytes + segment.destination_offset + byte] =
            completed ? expected : static_cast<std::uint8_t>(expected ^ (band + 1));
      }
  return result;
}

using PeerClock = std::chrono::steady_clock;
inline constexpr auto peer_list_timeout = std::chrono::seconds(60);

[[noreturn]] inline void segment_fail(const char *reason) {
  std::fprintf(stderr, "ordered peer benchmark failed: %s\n", reason);
  std::fflush(stderr);
  // A partial enqueue may still own DMA resources. Do not run normal teardown.
  std::_Exit(3);
}

inline void require_segment_deadline(PeerClock::time_point deadline) {
  if (PeerClock::now() >= deadline)
    segment_fail("whole-list deadline");
}

template <typename Issue, typename Load, typename Reset, typename InTime,
          typename Completed>
bool execute_peer_segment_chain(std::size_t count, Issue issue, Load load,
                                Reset reset, InTime in_time, Completed completed) {
  if (count == 0 || count > 4096)
    return false;
  for (std::size_t i = 0; i < count; ++i)
    if (!in_time() || !issue(i))
      return false;
  for (;;) {
    if (!in_time())
      return false;
    const auto value = load(count - 1);
    if (value < 0)
      return false;
    if (value == 0)
      break;
  }
  if (!in_time())
    return false;
  completed();
  for (std::size_t i = 0; i < count; ++i)
    if (load(i) != 0)
      return false;
  for (std::size_t i = 0; i < count; ++i)
    reset(i);
  return true;
}

template <typename Copy, typename Validate, typename Release>
void run_peer_segments(const char *backend, const char *progress,
                       const PeerSegmentPlan &plan, const std::uint64_t ids[2],
                       Copy copy, Validate validate, Release release) {
  std::vector<std::uint64_t> times;
  times.reserve(2 * plan.bands);
  for (std::size_t band = 0; band < plan.bands; ++band)
    for (std::size_t direction = 0; direction < 2; ++direction) {
      const auto elapsed = copy(direction, band);
      if (elapsed == 0 || elapsed > 60'000'000'000ULL)
        segment_fail("invalid or late duration");
      times.push_back(elapsed);
    }
  if (!validate(0) || !validate(1))
    segment_fail("source, destination, or canary mismatch");
  release();
  // Printing and allocation readback never interrupt the primed list sequence.
  for (std::size_t band = 0; band < plan.bands; ++band)
    for (std::size_t direction = 0; direction < 2; ++direction)
      std::printf("schema=fe2o3.xgmi-ordered-segments.v1 record=list backend=%s band=%zu direction=%zu population=%s elapsed_ns=%llu\n",
                  backend, band, direction,
                  band == 0 ? "prime" : (band <= plan.warmups ? "warmup" : "sample"),
                  static_cast<unsigned long long>(times[2 * band + direction]));
  std::printf("schema=fe2o3.xgmi-ordered-segments.v1 record=complete backend=%s unique_ids=%016llx,%016llx useful_bytes=%zu descriptor_count=%zu logical_depth=1 warmups=%zu samples=%zu prime_lists=1 band_bytes=%zu source_bytes=%zu destination_bytes=%zu layout=reversed-ragged-slots-v1 progress=%s deadline_ns=60000000000 timing=list-admission-through-observed-completion mapping_lifetime=retained-pair-no-allocation-host-readwrite-between-lists completion_cleanup=outside-timing correctness=passed teardown=explicit\n",
              backend, static_cast<unsigned long long>(ids[0]),
              static_cast<unsigned long long>(ids[1]), plan.useful_bytes,
              plan.segments.size(), plan.warmups, plan.samples, plan.band_bytes,
              plan.band_bytes, plan.destination_bytes, progress);
}

} // namespace fe2o3::runtime_gfx942

#endif

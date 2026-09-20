#include "xgmi_peer_segments_common.hpp"

#include <cstring>
#include <limits>
#include <stdexcept>
#include <string>

namespace peer = fe2o3::runtime_gfx942;

static void need(bool value) {
  if (!value)
    throw std::runtime_error("ordered segment test failed");
}

static uint64_t checksum(const std::vector<uint8_t> &bytes) {
  uint64_t hash = 14695981039346656037ULL;
  for (auto byte : bytes)
    hash = (hash ^ byte) * 1099511628211ULL;
  return hash;
}

static void describe(const peer::PeerSegmentPlan &plan) {
  std::printf("{\"useful_bytes\":%zu,\"warmups\":%zu,\"samples\":%zu,\"band_bytes\":%zu,\"destination_bytes\":%zu,\"bands\":%zu,\"segments\":[",
              plan.useful_bytes, plan.warmups, plan.samples, plan.band_bytes,
              plan.destination_bytes, plan.bands);
  for (size_t i = 0; i < plan.segments.size(); ++i) {
    const auto &s = plan.segments[i];
    std::printf("%s[%zu,%zu,%zu]", i ? "," : "", s.source_offset, s.destination_offset, s.bytes);
  }
  std::printf("],\"compatibility_checksums_fnv1a64\":[");
  for (size_t direction = 0; direction < 2; ++direction)
    std::printf("%s[\"%016llx\",\"%016llx\",\"%016llx\"]", direction ? "," : "",
                static_cast<unsigned long long>(checksum(peer::segment_source(plan, direction))),
                static_cast<unsigned long long>(checksum(peer::segment_destination(plan, direction, false))),
                static_cast<unsigned long long>(checksum(peer::segment_destination(plan, direction, true))));
  std::puts("]}");
}

static void plans() {
  for (size_t bytes : {size_t{65536}, size_t{2097152}})
    for (size_t count : {size_t{1}, size_t{65}, size_t{256}, size_t{4096}}) {
      peer::PeerSegmentPlan plan;
      need(peer::make_peer_segment_plan(bytes, count, 10, 30, &plan));
      need(plan.segments.size() == count && plan.bands == 41 && plan.band_bytes % 4096 == 0);
      size_t sum = 0;
      std::vector<std::pair<size_t, size_t>> destination;
      for (const auto &s : plan.segments) {
        need(s.bytes > 0 && s.source_offset >= 32 && s.destination_offset >= 32);
        need(s.source_offset + s.bytes < plan.band_bytes && s.destination_offset + s.bytes < plan.band_bytes);
        sum += s.bytes;
        destination.emplace_back(s.destination_offset, s.destination_offset + s.bytes);
      }
      need(sum == bytes);
      std::sort(destination.begin(), destination.end());
      for (size_t i = 1; i < count; ++i)
        need(destination[i - 1].second < destination[i].first);
    }
  peer::PeerSegmentPlan plan;
  need(peer::make_peer_segment_plan(257, 65, 1, 3, &plan));
  for (size_t direction = 0; direction < 2; ++direction) {
    const auto source = peer::segment_source(plan, direction);
    const auto expected = peer::segment_destination(plan, direction, true);
    for (size_t omitted = 0; omitted <= plan.bands; ++omitted) {
      auto observed = peer::segment_destination(plan, direction, false);
      for (size_t band = 0; band < plan.bands; ++band) {
        if (band == omitted)
          continue;
        for (const auto &s : plan.segments)
          std::memcpy(observed.data() + band * plan.band_bytes + s.destination_offset,
                      source.data() + s.source_offset, s.bytes);
      }
      need((observed == expected) == (omitted == plan.bands));
    }
    for (size_t band = 0; band < plan.bands; ++band) {
      auto changed = expected;
      changed[band * plan.band_bytes] ^= 0xff;
      need(changed != expected);
    }
  }
  const auto before = plan.destination_bytes;
  const size_t maximum = std::numeric_limits<size_t>::max();
  for (const auto &bad : std::vector<std::vector<size_t>>{
           {0,1,0,1}, {1,0,0,1}, {1,2,0,1}, {4097,4097,0,1},
           {2097153,1,0,1}, {1,1,0,0}, {1,1,64,1}, {1,1,maximum,1}}) {
    need(!peer::make_peer_segment_plan(bad[0], bad[1], bad[2], bad[3], &plan));
    need(plan.destination_bytes == before);
  }
  need(!peer::make_peer_segment_plan(1, 1, 0, 1, nullptr));
}

static void chains() {
  for (size_t count : {size_t{1}, size_t{65}, size_t{4096}}) {
    std::vector<size_t> issued, resets;
    size_t polls = 0;
    bool completed = false;
    need(peer::execute_peer_segment_chain(count,
        [&](size_t i) { issued.push_back(i); return true; },
        [&](size_t i) {
          need(issued.size() == count && resets.empty());
          if (!completed) { need(i == count - 1); return ++polls < 4 ? 1 : 0; }
          return 0;
        },
        [&](size_t i) { need(completed); resets.push_back(i); },
        [] { return true; }, [&] { completed = true; }));
    need(issued.size() == count && issued == resets && polls == 4);
  }
  // Every failure position retains the entire signal roster without a reset.
  for (size_t fault = 0; fault < 4; ++fault)
    for (size_t mode = 0; mode < 4; ++mode) {
      size_t ticks = 0, reset_count = 0;
      bool observed = false;
      const bool result = peer::execute_peer_segment_chain(4,
          [&](size_t i) { return mode != 0 || i != fault; },
          [&](size_t i) {
            if (mode == 1) return i == fault ? -1 : 1;
            if (mode == 2 && observed && i == fault) return 1;
            return 0;
          },
          [&](size_t) { ++reset_count; },
          [&] { return ++ticks < (mode == 3 ? fault + 1 : 20); },
          [&] { observed = true; });
      need(!result && reset_count == 0);
    }
  // Expiry after enqueue, during tail polling, and after observing tail zero.
  for (size_t expiry : {size_t{5}, size_t{7}, size_t{9}}) {
    size_t ticks = 0, polls = 0, issued = 0, resets = 0;
    bool completed = false;
    need(!peer::execute_peer_segment_chain(4,
        [&](size_t) { ++issued; return true; },
        [&](size_t) { return ++polls < 4 ? 1 : 0; },
        [&](size_t) { ++resets; },
        [&] { return ++ticks < expiry; },
        [&] { completed = true; }));
    need(issued == 4 && resets == 0 && !completed);
    need(polls == (expiry == 5 ? 0 : expiry == 7 ? 2 : 4));
  }
  for (size_t count : {size_t{0}, size_t{4097}}) {
    need(!peer::execute_peer_segment_chain(count,
        [](size_t) { need(false); return true; },
        [](size_t) { need(false); return 0; },
        [](size_t) { need(false); },
        [] { need(false); return true; },
        [] { need(false); }));
  }
}

int main(int argc, char **argv) {
  if (argc == 5) {
    size_t values[4] = {};
    for (size_t i = 0; i < 4; ++i)
      if (!peer::parse_size(argv[i + 1], &values[i])) return 2;
    peer::PeerSegmentPlan plan;
    if (!peer::make_peer_segment_plan(values[0], values[1], values[2], values[3], &plan)) return 2;
    describe(plan);
    return 0;
  }
  if (argc != 1) return 2;
  plans();
  chains();
  std::puts("ordered segment plans, independent bands, and chain custody: pass");
}

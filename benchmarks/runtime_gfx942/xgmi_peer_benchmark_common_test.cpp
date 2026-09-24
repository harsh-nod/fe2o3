#include "xgmi_peer_benchmark_common.hpp"

#include <array>
#include <cstdio>
#include <limits>
#include <string>
#include <utility>
#include <vector>

namespace peer = fe2o3::runtime_gfx942;

#define EXPECT(condition)                                                      \
  do {                                                                         \
    if (!(condition)) {                                                        \
      std::fprintf(stderr, "expectation failed at line %d: %s\n", __LINE__,     \
                   #condition);                                                \
      return false;                                                            \
    }                                                                          \
  } while (false)

static bool controls() {
  peer::WorkloadShape shape;
  EXPECT(peer::parse_workload_shape("1048576", "1", "10", "30", 1, &shape));
  peer::PeerBenchmarkControls value;
  EXPECT(peer::parse_peer_controls("--persistent-hot", shape, &value));
  EXPECT(value.persistent_hot && value.allocation_bytes == 1048640 &&
         value.copy_offset == 32 && value.hot_pattern_round == 41);
  for (const char *flag : {"", "--persistent-hot=1", "--unknown"}) {
    EXPECT(!peer::parse_peer_controls(flag, shape, &value));
    EXPECT(value.persistent_hot && value.allocation_bytes == 1048640 &&
           value.copy_offset == 32 && value.hot_pattern_round == 41);
  }
  EXPECT(!peer::parse_peer_controls(nullptr, shape, nullptr));
  for (const char *depth : {"1", "16", "32"}) {
    EXPECT(peer::parse_workload_shape("1048576", depth, "10", "30", 1, &shape));
    EXPECT(peer::parse_peer_controls("--persistent-hot", shape, &value));
    EXPECT(value.persistent_hot && value.allocation_bytes == 1048640 &&
           value.copy_offset == 32 && value.hot_pattern_round == 41);
    EXPECT(shape.transfer_bytes == shape.bytes * shape.depth);
  }
  shape.depth = 33;
  EXPECT(!peer::parse_peer_controls("--persistent-hot", shape, &value));
  EXPECT(value.persistent_hot && value.allocation_bytes == 1048640 &&
         value.copy_offset == 32 && value.hot_pattern_round == 41);
  EXPECT(peer::parse_peer_controls(nullptr, shape, &value));
  EXPECT(!value.persistent_hot && value.allocation_bytes == shape.bytes &&
         value.copy_offset == 0 && value.hot_pattern_round == 0);

  shape.depth = 1;
  shape.bytes = std::numeric_limits<std::size_t>::max();
  EXPECT(!peer::parse_peer_controls("--persistent-hot", shape, &value));
  EXPECT(peer::parse_peer_controls(nullptr, shape, &value));
  shape.bytes = 1;
  shape.warmups = std::numeric_limits<std::size_t>::max() - 1;
  shape.samples = 1;
  shape.total_iterations = std::numeric_limits<std::size_t>::max();
  EXPECT(!peer::parse_peer_controls("--persistent-hot", shape, &value));
  EXPECT(peer::parse_peer_controls(nullptr, shape, &value));
  shape.warmups += 1;
  EXPECT(!peer::parse_peer_controls(nullptr, shape, &value));
  shape.warmups = 0;
  shape.total_iterations = 2;
  EXPECT(!peer::parse_peer_controls(nullptr, shape, &value));
  shape.total_iterations = 1;
  for (auto member : {&peer::WorkloadShape::bytes, &peer::WorkloadShape::depth,
                      &peer::WorkloadShape::samples}) {
    auto invalid = shape;
    invalid.*member = 0;
    EXPECT(!peer::parse_peer_controls(nullptr, invalid, &value));
    EXPECT(!peer::parse_peer_controls("--persistent-hot", invalid, &value));
  }
  const auto maximum = std::to_string(std::numeric_limits<std::size_t>::max());
  EXPECT(!peer::parse_workload_shape(maximum.c_str(), "32", "10", "30", 1, &shape));
  return true;
}

static bool guards_and_patterns() {
  constexpr std::size_t bytes = 7;
  std::array<std::uint8_t, bytes + 64> data;
  for (std::size_t direction = 0; direction != 2; ++direction) {
    const auto inner = peer::peer_pattern(41, 0, direction);
    for (auto outer : {peer::peer_source_canary(direction),
                       peer::peer_destination_canary(direction)}) {
      EXPECT(peer::fill_peer_guarded(data.data(), data.size(), bytes, outer,
                                     inner));
      for (std::size_t i = 0; i < data.size(); ++i)
        EXPECT(data[i] == (i >= 32 && i < 32 + bytes ? inner : outer));
      EXPECT(peer::validate_peer_guarded(data.data(), data.size(), bytes,
                                         outer, inner));
      for (std::size_t i = 0; i < data.size(); ++i) {
        data[i] ^= 0xff;
        EXPECT(!peer::validate_peer_guarded(data.data(), data.size(), bytes,
                                            outer, inner));
        data[i] ^= 0xff;
      }
    }
  }
  const auto unchanged = data;
  for (auto total : {std::size_t{0}, data.size() - 1, data.size() + 1}) {
    EXPECT(!peer::fill_peer_guarded(data.data(), total, bytes, 0, 0));
    EXPECT(!peer::validate_peer_guarded(data.data(), total, bytes, 0, 0));
    EXPECT(data == unchanged);
  }
  for (auto invalid_bytes : {std::size_t{0},
                             std::numeric_limits<std::size_t>::max()}) {
    EXPECT(!peer::fill_peer_guarded(data.data(), data.size(), invalid_bytes,
                                    0, 0));
    EXPECT(!peer::validate_peer_guarded(data.data(), data.size(), invalid_bytes,
                                        0, 0));
    EXPECT(data == unchanged);
  }
  EXPECT(!peer::fill_peer_guarded(nullptr, data.size(), bytes, 0, 0));
  EXPECT(!peer::validate_peer_guarded(nullptr, data.size(), bytes, 0, 0));
  EXPECT(peer::peer_source_canary(0) == 0x17);
  EXPECT(peer::peer_source_canary(1) == 0x71);
  EXPECT(peer::peer_destination_canary(0) == 0xa5);
  EXPECT(peer::peer_destination_canary(1) == 0x5a);
  EXPECT(peer::peer_pattern(0, 0, 0) == 2);
  EXPECT(peer::peer_pattern(41, 0, 0) == 239);
  EXPECT(peer::peer_pattern(41, 0, 1) == 89);
  const auto maximum = std::numeric_limits<std::size_t>::max();
  for (auto round : {std::size_t{0}, std::size_t{250}, std::size_t{251},
                     maximum})
    for (auto slot : {std::size_t{0}, maximum})
      for (auto direction : {std::size_t{0}, std::size_t{1}, maximum}) {
        const auto expected = ((round % 251) * 67 + (slot % 251) * 29 +
                               (direction % 251) * 101 + 1) % 251 + 1;
        EXPECT(peer::peer_pattern(round, slot, direction) == expected);
        EXPECT(peer::peer_pattern(round, slot, direction) >= 1);
        EXPECT(peer::peer_pattern(round, slot, direction) <= 251);
      }
  return true;
}

static bool lifecycle() {
  for (auto depth : {"1", "16", "32"}) {
    for (auto warmups : {"0", "2"}) {
      for (int failure = -1; failure < 4; ++failure) {
        peer::WorkloadShape shape;
        EXPECT(peer::parse_workload_shape("7", depth, warmups, "3", 1, &shape));
        std::vector<std::string> events;
        std::vector<std::pair<std::uint64_t, std::uint64_t>> samples;
        std::uint64_t elapsed = 0;
        const bool result = peer::run_peer_persistent_hot(
            shape,
            [&](std::size_t direction) {
              events.push_back("prepare" + std::to_string(direction));
              return failure != static_cast<int>(direction);
            },
            [&](std::size_t direction) {
              events.push_back("copy" + std::to_string(direction));
              return ++elapsed;
            },
            [&](std::size_t direction) {
              events.push_back("validate" + std::to_string(direction));
              return failure != static_cast<int>(direction + 2);
            },
            [&](std::uint64_t forward, std::uint64_t reverse) {
              events.push_back("sample");
              samples.emplace_back(forward, reverse);
            });
        EXPECT(result == (failure == -1));
        std::vector<std::string> expected{"prepare0"};
        if (failure != 0)
          expected.push_back("prepare1");
        if (failure != 0 && failure != 1) {
          expected.insert(expected.end(), {"copy0", "copy1"});
          for (std::size_t round = 0; round < shape.total_iterations; ++round) {
            expected.insert(expected.end(), {"copy0", "copy1"});
            if (round >= shape.warmups)
              expected.push_back("sample");
          }
          expected.push_back("validate0");
          expected.push_back("validate1");
          EXPECT(samples.size() == shape.samples);
          for (std::size_t i = 0; i < samples.size(); ++i) {
            const auto first = 2 * (1 + shape.warmups + i) + 1;
            EXPECT(samples[i].first == first && samples[i].second == first + 1);
          }
        } else {
          EXPECT(samples.empty());
        }
        EXPECT(events == expected);
      }
    }
  }
  peer::WorkloadShape invalid;
  std::size_t calls = 0;
  EXPECT(!peer::run_peer_persistent_hot(
      invalid,
      [&](std::size_t) {
        ++calls;
        return true;
      },
      [&](std::size_t) {
        ++calls;
        return std::uint64_t{0};
      },
      [&](std::size_t) {
        ++calls;
        return true;
      },
      [&](std::uint64_t, std::uint64_t) { ++calls; }));
  EXPECT(calls == 0);
  return true;
}

static bool batch_lifecycle() {
  constexpr std::size_t bytes = 7;
  using Buffer = std::array<std::uint8_t, bytes + 64>;
  using Slot = std::array<Buffer, 2>;
  for (const char *depth : {"1", "16", "32"}) {
    peer::WorkloadShape shape;
    EXPECT(peer::parse_workload_shape("7", depth, "2", "3", 1, &shape));
    for (int corrupt : {-1, 0, static_cast<int>(shape.depth / 2),
                        static_cast<int>(shape.depth - 1)}) {
      for (std::size_t corrupt_direction = 0; corrupt_direction < 2;
           ++corrupt_direction) {
        for (std::size_t corrupt_buffer = 0; corrupt_buffer < 2;
             ++corrupt_buffer) {
          std::array<std::vector<Slot>, 2> storage;
          for (auto &direction : storage)
            direction.resize(shape.depth);
          std::vector<std::string> events;
          const auto event = [&](const char *kind, std::size_t direction,
                                 std::size_t slot, std::size_t buffer) {
            return std::string(kind) + ":" + std::to_string(direction) + ":" +
                   std::to_string(slot) + ":" + std::to_string(buffer);
          };
          const auto canary = [](std::size_t direction, bool source) {
            return source ? peer::peer_source_canary(direction)
                          : peer::peer_destination_canary(direction);
          };
          const std::size_t hot_round = shape.total_iterations + 1;
          const bool valid = peer::run_peer_persistent_hot(
              shape,
              [&](std::size_t direction) {
                return peer::visit_peer_buffers(
                    shape.depth, [&](std::size_t slot, bool source) {
                      const std::size_t buffer = source ? 0 : 1;
                      events.push_back(
                          event("prepare", direction, slot, buffer));
                      const auto pattern =
                          peer::peer_pattern(hot_round, slot, direction);
                      auto &data = storage[direction][slot][buffer];
                      return peer::fill_peer_guarded(
                          data.data(), data.size(), bytes,
                          canary(direction, source),
                          source ? pattern : pattern ^ 0xff);
                    });
              },
              [&](std::size_t direction) {
                peer::run_peer_batch(
                    shape.depth,
                    [&](std::size_t slot) {
                      events.push_back(event("enqueue", direction, slot, 0));
                    },
                    [&](std::size_t slot) {
                      events.push_back(event("wait", direction, slot, 0));
                      std::copy_n(storage[direction][slot][0].data() + 32,
                                  bytes,
                                  storage[direction][slot][1].data() + 32);
                    });
                return std::uint64_t{1};
              },
              [&](std::size_t direction) {
                return peer::visit_peer_buffers(
                    shape.depth, [&](std::size_t slot, bool source) {
                      const std::size_t buffer = source ? 0 : 1;
                      events.push_back(
                          event("validate", direction, slot, buffer));
                      auto &data = storage[direction][slot][buffer];
                      if (corrupt == static_cast<int>(slot) &&
                          direction == corrupt_direction &&
                          buffer == corrupt_buffer)
                        data[32] ^= 0xff;
                      return peer::validate_peer_guarded(
                          data.data(), data.size(), bytes,
                          canary(direction, source),
                          peer::peer_pattern(hot_round, slot, direction));
                    });
              },
              [&](std::uint64_t, std::uint64_t) {
                events.push_back("sample");
              });
          EXPECT(valid == (corrupt == -1));
          std::vector<std::string> expected;
          for (std::size_t direction = 0; direction < 2; ++direction)
            for (std::size_t slot = 0; slot < shape.depth; ++slot)
              for (std::size_t buffer = 0; buffer < 2; ++buffer)
                expected.push_back(event("prepare", direction, slot, buffer));
          for (std::size_t round = 0; round <= shape.total_iterations;
               ++round) {
            for (std::size_t direction = 0; direction < 2; ++direction)
              for (const char *kind : {"enqueue", "wait"})
                for (std::size_t slot = 0; slot < shape.depth; ++slot)
                  expected.push_back(event(kind, direction, slot, 0));
            if (round > shape.warmups)
              expected.push_back("sample");
          }
          for (std::size_t direction = 0; direction < 2; ++direction)
            for (std::size_t slot = 0; slot < shape.depth; ++slot)
              for (std::size_t buffer = 0; buffer < 2; ++buffer)
                expected.push_back(event("validate", direction, slot, buffer));
          EXPECT(events == expected);
        }
      }
    }
  }
  // No two admitted slots, even across directions, share a payload pattern.
  std::array<bool, 256> seen{};
  for (std::size_t direction = 0; direction < 2; ++direction)
    for (std::size_t slot = 0; slot < peer::peer_hot_max_depth; ++slot) {
      const auto pattern = peer::peer_pattern(41, slot, direction);
      EXPECT(!seen[pattern]);
      seen[pattern] = true;
    }
  return true;
}

int main() {
  if (!controls() || !guards_and_patterns() || !lifecycle() ||
      !batch_lifecycle())
    return 1;
  std::puts("peer benchmark controls, guards, patterns, and lifecycle: pass");
  return 0;
}

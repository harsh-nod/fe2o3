#include "xgmi_peer_series_common.hpp"

#include <cstdlib>
#include <iostream>
#include <utility>
#include <vector>

namespace peer = fe2o3::runtime_gfx942;

static void require(bool condition) {
  if (!condition)
    std::abort();
}

int main() {
  for (std::size_t depth : {1, 16, 32}) {
    for (std::size_t warmups : {0, 2}) {
      for (int mismatch : {-1, 0, 1}) {
        peer::WorkloadShape shape;
        const auto depth_text = std::to_string(depth);
        const auto warmups_text = std::to_string(warmups);
        require(peer::parse_workload_shape("7", depth_text.c_str(), warmups_text.c_str(), "3", 1, &shape));
        std::vector<std::pair<char, std::size_t>> events;
        std::vector<std::vector<std::uint64_t>> samples(2);
        std::size_t calls[2] = {};
        const bool valid = peer::run_peer_persistent_series(
            shape,
            [&](std::size_t direction) { events.emplace_back('p', direction); return true; },
            [&](std::size_t direction) { events.emplace_back('c', direction); return ++calls[direction]; },
            [&](std::size_t direction) { events.emplace_back('v', direction); return int(direction) != mismatch; },
            [&](std::size_t direction, std::uint64_t elapsed) {
              events.emplace_back('r', direction);
              samples[direction].push_back(elapsed);
            });
        require(valid == (mismatch == -1));
        std::vector<std::pair<char, std::size_t>> expected = {{'p', 0}, {'p', 1}};
        for (std::size_t direction = 0; direction < 2; ++direction) {
          expected.emplace_back('c', direction);
          for (std::size_t round = 0; round < shape.total_iterations; ++round) {
            expected.emplace_back('c', direction);
            if (round >= warmups)
              expected.emplace_back('r', direction);
          }
          require(calls[direction] == 1 + warmups + 3);
          require(samples[direction] == std::vector<std::uint64_t>({warmups + 2, warmups + 3, warmups + 4}));
        }
        expected.emplace_back('v', 0);
        expected.emplace_back('v', 1);
        require(events == expected);
      }
    }
  }
  for (int fail_prepare : {0, 1}) {
    peer::WorkloadShape shape;
    require(peer::parse_workload_shape("7", "1", "0", "1", 1, &shape));
    std::size_t prepared = 0, copied = 0, validated = 0, recorded = 0;
    require(!peer::run_peer_persistent_series(shape,
        [&](std::size_t direction) { ++prepared; return int(direction) != fail_prepare; },
        [&](std::size_t) { ++copied; return 1; },
        [&](std::size_t) { ++validated; return true; },
        [&](std::size_t, std::uint64_t) { ++recorded; }));
    require(prepared == std::size_t(fail_prepare + 1));
    require(copied == 0 && validated == 0 && recorded == 0);
  }
  std::cout << "series control flow: pass (CPU callbacks only)\n";
}

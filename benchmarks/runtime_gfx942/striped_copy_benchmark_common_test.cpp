#include "striped_copy_benchmark_common.hpp"

#include <cassert>
#include <cstddef>
#include <cstdint>
#include <limits>
#include <set>
#include <string>
#include <vector>

namespace {

fe2o3::r40::Config parse(std::vector<std::string> arguments, bool *accepted) {
  std::vector<char *> argv;
  argv.reserve(arguments.size());
  for (std::string &argument : arguments)
    argv.push_back(argument.data());
  fe2o3::r40::Config config;
  *accepted = fe2o3::r40::parse_config(static_cast<int>(argv.size()),
                                       argv.data(), &config);
  return config;
}

} // namespace

int main() {
  bool accepted = false;
  auto config = parse({"benchmark", "0", "0xd2e26fef80cf5c33", "4096", "112",
                       "10", "30", "14", "combined-striped14"},
                      &accepted);
  assert(accepted);
  assert(config.workload_id == "bytes4096-q14-combined");
  assert(config.depth / config.logical_queue_count == 8);
  assert(config.rounds == 40);
  assert(config.transfer_bytes == 4096 * 112);

  for (const std::size_t queue_count : {2U, 4U, 8U, 14U, 16U}) {
    const std::string profile =
        queue_count == 16 ? "striped16"
                          : "combined-striped" + std::to_string(queue_count);
    config = parse({"benchmark", "0", "0xd2e26fef80cf5c33", "1048576", "112",
                    "10", "30", std::to_string(queue_count), profile},
                   &accepted);
    assert(accepted);
    const auto order = fe2o3::r40::publication_order(
        31, config.depth, config.logical_queue_count);
    assert(order.size() == config.depth);
    assert(std::set<std::size_t>(order.begin(), order.end()).size() ==
           config.depth);
    std::size_t prior_lane = (31 + order.front()) % config.logical_queue_count;
    for (const std::size_t request : order) {
      const std::size_t lane = (31 + request) % config.logical_queue_count;
      assert(lane >= prior_lane ||
             (prior_lane == config.logical_queue_count - 1 && lane == 0));
      prior_lane = lane;
    }
  }

  parse({"benchmark", "0", "0xd2e26fef80cf5c33", "4096", "112", "10", "30", "0",
         "combined-striped2"},
        &accepted);
  assert(!accepted);
  assert(fe2o3::r40::publication_order(0, 112, 0).empty());
  assert(fe2o3::r40::publication_order(0, 111, 14).empty());
  assert(fe2o3::r40::publication_order(0, 112, 17).empty());
  parse({"benchmark", "0", "0xd2e26fef80cf5c33", "4096", "112", "10", "30",
         "16", "combined-striped16"},
        &accepted);
  assert(!accepted);
  parse({"benchmark", "0", "0xd2e26fef80cf5c33", "4096", "1009", "10", "30",
         "2", "combined-striped2"},
        &accepted);
  assert(!accepted);
  parse({"benchmark",
         std::to_string(
             static_cast<std::size_t>(std::numeric_limits<int>::max()) + 1),
         "0xd2e26fef80cf5c33", "4096", "112", "10", "30", "2",
         "combined-striped2"},
        &accepted);
  assert(!accepted);
  parse({"benchmark", "0", "0xd2e26fef80cf5c33", "4096", "112",
         std::to_string(std::numeric_limits<std::size_t>::max()), "1", "2",
         "combined-striped2"},
        &accepted);
  assert(!accepted);
  parse({"benchmark", "0", "0xd2e26fef80cf5c33", "4096", "111", "10", "30",
         "14", "combined-striped14"},
        &accepted);
  assert(!accepted);
  parse({"benchmark", "0", "0xd2e26fef80cf5c33",
         std::to_string(std::numeric_limits<std::size_t>::max()), "112", "10",
         "30", "2", "combined-striped2"},
        &accepted);
  assert(!accepted);
  fe2o3::r40::PhaseSamples phase(1);
  assert(!phase.append(std::numeric_limits<std::uint64_t>::max(), 1));
  assert(phase.append(1, 1));
  assert(fe2o3::r40::round_pattern(std::numeric_limits<std::size_t>::max(),
                                   std::numeric_limits<std::size_t>::max()) >=
         1);
  assert(fe2o3::r40::round_pattern(std::numeric_limits<std::size_t>::max(),
                                   std::numeric_limits<std::size_t>::max()) <=
         251);
  return 0;
}

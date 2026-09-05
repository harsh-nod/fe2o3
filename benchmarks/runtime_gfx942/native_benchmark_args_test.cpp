#include "native_benchmark_args.hpp"

#include <cstddef>
#include <cstdint>
#include <cstdio>
#include <limits>
#include <string>

namespace args = fe2o3::runtime_gfx942;

#define EXPECT(condition)                                                      \
  do {                                                                         \
    if (!(condition)) {                                                        \
      std::fprintf(stderr, "expectation failed at line %d: %s\n", __LINE__,    \
                   #condition);                                                \
      return 1;                                                                \
    }                                                                          \
  } while (false)

int main() {
  std::size_t size = 0;
  int device = 0;
  std::uint64_t unique_id = 0;
  args::WorkloadShape shape;

  EXPECT(!args::parse_size("", &size));
  EXPECT(!args::parse_size("-1", &size));
  EXPECT(!args::parse_size("1tail", &size));
  EXPECT(!args::parse_device_index("+1", &device));
  EXPECT(!args::parse_unique_id("0x", &unique_id));

  const std::string size_max =
      std::to_string(std::numeric_limits<std::size_t>::max());
  EXPECT(!args::parse_size((size_max + "0").c_str(), &size));
  EXPECT(!args::parse_unique_id("0x10000000000000000", &unique_id));

  EXPECT(
      !args::parse_workload_shape("1", "1", size_max.c_str(), "1", 1, &shape));

  const std::string overflowing_double_depth =
      std::to_string(std::numeric_limits<std::size_t>::max() / 2 + 1);
  EXPECT(!args::parse_workload_shape("1", overflowing_double_depth.c_str(), "0",
                                     "1", 2, &shape));

  EXPECT(
      !args::parse_workload_shape("2", size_max.c_str(), "0", "1", 1, &shape));

  EXPECT(args::parse_device_index("0", &device) && device == 0);
  EXPECT(args::parse_unique_id("0x10", &unique_id) && unique_id == 16);
  EXPECT(args::parse_unique_id("010", &unique_id) && unique_id == 10);
  EXPECT(args::parse_workload_shape("4096", "4", "0", "3", 2, &shape));
  EXPECT(shape.bytes == 4096);
  EXPECT(shape.depth == 4);
  EXPECT(shape.total_iterations == 3);
  EXPECT(shape.total_depth == 8);
  EXPECT(shape.transfer_bytes == 32768);
  return 0;
}

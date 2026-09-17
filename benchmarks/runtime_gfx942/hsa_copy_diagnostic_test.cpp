#include "hsa_copy_diagnostic.hpp"

#include <array>
#include <cstdio>
#include <limits>

namespace policy = fe2o3::copy_diagnostic;

#define EXPECT(condition)                                                      \
  do {                                                                         \
    if (!(condition)) {                                                        \
      std::fprintf(stderr, "expectation failed at line %d: %s\n", __LINE__,    \
                   #condition);                                                \
      return 1;                                                                \
    }                                                                          \
  } while (false)

int main() {
  const std::array<const char *, 9> valid{
      "diagnostic",         "1",    "0",      "268435456", "3", "10",
      "0xab83d2ffef0d3cdf", "fine", "engine0"};
  policy::Config config;
  EXPECT(policy::parse_config(valid.size(), valid.data(), &config));
  EXPECT(config.gpu_index == 1 && config.cpu_index == 0);
  EXPECT(config.workload.bytes == policy::kMaxBytes);
  EXPECT(config.workload.depth == 1 && config.workload.total_iterations == 13);
  EXPECT(config.host_flags == policy::kFine && config.engine_mask == 1);
  EXPECT(config.unique_id == 0xab83d2ffef0d3cdfULL);
  auto args = valid;
  args[7] = "coarse";
  args[8] = "engine1";
  EXPECT(policy::parse_config(args.size(), args.data(), &config));
  EXPECT(config.host_flags == policy::kCoarse && config.engine_mask == 2);
  const auto last_valid = config;
  for (std::size_t index = 1; index < args.size(); ++index) {
    for (const char *bad : std::array<const char *, 7>{
             nullptr, "", "-1", "+1", "1 ", "1x", "18446744073709551616"}) {
      args = valid;
      args[index] = bad;
      EXPECT(!policy::parse_config(args.size(), args.data(), &config));
      EXPECT(config.engine_mask == last_valid.engine_mask &&
             config.host_flags == last_valid.host_flags);
    }
  }
  for (const auto &entry : std::vector<std::pair<std::size_t, const char *>>{
           {3, "0"},
           {3, "268435457"},
           {4, "10000"},
           {5, "0"},
           {5, "10001"},
           {6, "0"},
           {7, "Fine"},
           {7, "first"},
           {7, "fine+kernarg"},
           {8, "auto"},
           {8, "0"},
           {8, "1"},
           {8, "engine2"},
           {8, "engine0,engine1"}}) {
    args = valid;
    args[entry.first] = entry.second;
    EXPECT(!policy::parse_config(args.size(), args.data(), &config));
  }
  EXPECT(!policy::parse_config(8, valid.data(), &config));
  EXPECT(!policy::parse_config(10, valid.data(), &config));
  EXPECT(!policy::parse_config(9, nullptr, &config));
  EXPECT(!policy::parse_config(9, valid.data(), nullptr));

  policy::PoolFacts host{
      10, policy::Location::Cpu, true, true, policy::kFine, 8192, 4096, 4096, 1,
      2};
  policy::PoolSelection selected;
  EXPECT(policy::select_pool({host}, policy::Location::Cpu, policy::kFine, 4095,
                             2, &selected));
  EXPECT(selected.index == 0 && selected.rounded_bytes == 4096 &&
         selected.aggregate_bytes == 8192);
  auto coarse = host;
  coarse.handle = 11;
  coarse.flags = policy::kCoarse;
  EXPECT(policy::select_pool({host, coarse}, policy::Location::Cpu,
                             policy::kCoarse, 4096, 2, &selected));
  EXPECT(selected.index == 1);
  auto gpu = coarse;
  gpu.handle = 12;
  gpu.location = policy::Location::Gpu;
  EXPECT(policy::select_pool({gpu}, policy::Location::Gpu, policy::kCoarse,
                             8192, 1, &selected));
  for (std::size_t field = 0; field < 14; ++field) {
    auto bad = host;
    switch (field) {
    case 0:
      bad.handle = 0;
      break;
    case 1:
      bad.location = policy::Location::Gpu;
      break;
    case 2:
      bad.global = false;
      break;
    case 3:
      bad.allocatable = false;
      break;
    case 4:
      bad.flags |= 1;
      break;
    case 5:
      bad.flags |= policy::kCoarse;
      break;
    case 6:
      bad.flags = 8;
      break;
    case 7:
      bad.maximum_bytes = 8191;
      break;
    case 8:
      bad.granule = 0;
      break;
    case 9:
      bad.alignment = 0;
      break;
    case 10:
      bad.alignment = 3;
      break;
    case 11:
      bad.cpu_access = 0;
      break;
    case 12:
      bad.gpu_access = 0;
      break;
    case 13:
      bad.gpu_access = 3;
      break;
    }
    selected = {17, 18, 19};
    EXPECT(!policy::select_pool({bad}, policy::Location::Cpu, policy::kFine,
                                4096, 2, &selected));
    EXPECT(selected.index == 17 && selected.rounded_bytes == 18 &&
           selected.aggregate_bytes == 19);
  }
  EXPECT(!policy::select_pool({}, policy::Location::Cpu, policy::kFine, 4096, 2,
                              &selected));
  EXPECT(!policy::select_pool({host, host}, policy::Location::Cpu,
                              policy::kFine, 4096, 2, &selected));
  auto duplicate = host;
  duplicate.handle = 13;
  EXPECT(!policy::select_pool({host, duplicate}, policy::Location::Cpu,
                              policy::kFine, 4096, 2, &selected));
  EXPECT(!policy::select_pool({host}, policy::Location::Other, policy::kFine,
                              4096, 2, &selected));
  EXPECT(!policy::select_pool({host}, policy::Location::Cpu, 3, 4096, 2,
                              &selected));
  EXPECT(!policy::select_pool({host}, policy::Location::Cpu, policy::kFine, 0,
                              2, &selected));
  EXPECT(!policy::select_pool({host}, policy::Location::Cpu, policy::kFine,
                              4096, 0, &selected));
  EXPECT(!policy::select_pool({host}, policy::Location::Cpu, policy::kFine,
                              4096, 2, nullptr));
  std::size_t rounded = 7, aggregate = 8;
  const auto maximum = std::numeric_limits<std::size_t>::max();
  EXPECT(!policy::rounded_extent(maximum, 4096, 1, &rounded, &aggregate));
  EXPECT(!policy::rounded_extent(maximum / 2 + 1, 1, 2, &rounded, &aggregate));
  EXPECT(rounded == 7 && aggregate == 8);
  EXPECT(policy::rounded_extent(8, 3, 2, &rounded, &aggregate));
  EXPECT(rounded == 9 && aggregate == 18);
  EXPECT(!policy::rounded_extent(1, 1, 1, nullptr, &aggregate));
  EXPECT(!policy::rounded_extent(1, 1, 1, &rounded, nullptr));

  for (std::uint32_t requested = 0; requested < 8; ++requested)
    for (std::uint32_t h2d = 0; h2d < 8; ++h2d)
      for (std::uint32_t d2h = 0; d2h < 8; ++d2h)
        EXPECT(policy::engine_available(requested, h2d, d2h) ==
               ((requested == 1 || requested == 2) && (h2d & requested) != 0 &&
                (d2h & requested) != 0));
  EXPECT(policy::exact_completion(0));
  for (auto value : {std::int64_t{-1}, std::int64_t{1},
                     std::numeric_limits<std::int64_t>::min(),
                     std::numeric_limits<std::int64_t>::max()})
    EXPECT(!policy::exact_completion(value));

  std::array<std::uint8_t, 7> buffer{};
  for (std::size_t round = 0; round < policy::kMaxRounds; ++round) {
    const auto expected = policy::pattern(round);
    EXPECT(expected != 0 && expected <= 251);
    buffer.fill(expected);
    EXPECT(policy::valid_buffer(buffer.data(), buffer.size(), expected));
    for (std::size_t index = 0; index < buffer.size(); ++index) {
      buffer[index] ^= 0xff;
      EXPECT(!policy::valid_buffer(buffer.data(), buffer.size(), expected));
      buffer[index] ^= 0xff;
    }
  }
  EXPECT(!policy::valid_buffer(nullptr, 7, 1));
  EXPECT(!policy::valid_buffer(buffer.data(), 0, 1));
  policy::Timing timing;
  EXPECT(policy::timing(3, 8, 17, &timing));
  EXPECT(timing.submit_ns == 5 && timing.wait_reset_ns == 9 &&
         timing.total_ns == 14);
  EXPECT(policy::timing(0, 0, 1, &timing));
  EXPECT(policy::timing(0, 1, 1, &timing));
  EXPECT(!policy::timing(2, 1, 3, &timing));
  EXPECT(!policy::timing(1, 3, 2, &timing));
  EXPECT(!policy::timing(1, 1, 1, &timing));
  EXPECT(!policy::timing(1, 2, 3, nullptr));
  EXPECT(std::strcmp(policy::kSchema, "fe2o3.async-copy-benchmark.v1") != 0);
  std::puts("HSA_COPY_DIAGNOSTIC_POLICY_OK");
  return 0;
}

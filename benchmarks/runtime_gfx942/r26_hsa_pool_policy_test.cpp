#include "r26_hsa_pool_policy.hpp"

#include <cstddef>
#include <cstdint>
#include <cstdio>
#include <limits>
#include <vector>

namespace policy = fe2o3::r26;

#define EXPECT(condition)                                                      \
  do {                                                                         \
    if (!(condition)) {                                                        \
      std::fprintf(stderr, "expectation failed at line %d: %s\n", __LINE__,    \
                   #condition);                                                \
      return 1;                                                                \
    }                                                                          \
  } while (false)

constexpr std::size_t kDataBytes = 1 << 20;
constexpr std::size_t kKernargBytes = 16;
constexpr std::size_t kKernargAlignment = 16;

policy::HsaPoolCandidate candidate(std::uint64_t handle,
                                   policy::HsaPoolOwner owner,
                                   policy::HsaPoolLocation location,
                                   std::uint32_t flags,
                                   std::size_t maximum_allocation_size) {
  return policy::HsaPoolCandidate{
      handle, owner, location, true, true, flags, maximum_allocation_size,
      4096,   4096,  true,     true,
  };
}

std::vector<policy::HsaPoolCandidate> valid_candidates() {
  return {
      candidate(10, policy::HsaPoolOwner::NearestCpu,
                policy::HsaPoolLocation::Cpu, policy::kHsaPoolFlagFineGrained,
                kDataBytes),
      candidate(
          11, policy::HsaPoolOwner::NearestCpu, policy::HsaPoolLocation::Cpu,
          policy::kHsaPoolFlagFineGrained | policy::kHsaPoolFlagKernargInit,
          4096),
      candidate(12, policy::HsaPoolOwner::SelectedGpu,
                policy::HsaPoolLocation::Gpu, policy::kHsaPoolFlagCoarseGrained,
                kDataBytes),
  };
}

policy::HsaPoolPolicyStatus
select(const std::vector<policy::HsaPoolCandidate> &candidates) {
  policy::HsaPoolRoles roles;
  return policy::select_hsa_pool_roles(candidates, kDataBytes, kKernargBytes,
                                       kKernargAlignment, &roles);
}

int main() {
  policy::HsaPoolRoles roles;
  auto candidates = valid_candidates();
  EXPECT(policy::select_hsa_pool_roles(candidates, kDataBytes, kKernargBytes,
                                       kKernargAlignment, &roles) ==
         policy::HsaPoolPolicyStatus::Accepted);
  EXPECT(roles.host == 0 && roles.kernarg == 1 && roles.device == 2);
  EXPECT(candidates[0].maximum_allocation_size == kDataBytes);

  EXPECT(policy::select_hsa_pool_roles(candidates, kDataBytes, kKernargBytes,
                                       kKernargAlignment, nullptr) ==
         policy::HsaPoolPolicyStatus::InvalidRequest);
  EXPECT(policy::select_hsa_pool_roles(candidates, 0, kKernargBytes,
                                       kKernargAlignment, &roles) ==
         policy::HsaPoolPolicyStatus::InvalidRequest);
  EXPECT(policy::select_hsa_pool_roles(candidates, kDataBytes, 0,
                                       kKernargAlignment, &roles) ==
         policy::HsaPoolPolicyStatus::InvalidRequest);
  EXPECT(policy::select_hsa_pool_roles(candidates, kDataBytes, kKernargBytes, 0,
                                       &roles) ==
         policy::HsaPoolPolicyStatus::InvalidRequest);
  EXPECT(policy::select_hsa_pool_roles(candidates, kDataBytes, kKernargBytes, 3,
                                       &roles) ==
         policy::HsaPoolPolicyStatus::InvalidRequest);

  EXPECT(policy::unique_enumerated_nearest_cpu(
      7, policy::HsaAgentType::Cpu, std::vector<std::uint64_t>{3, 7, 9}));
  EXPECT(!policy::unique_enumerated_nearest_cpu(7, policy::HsaAgentType::Gpu,
                                                std::vector<std::uint64_t>{7}));
  EXPECT(!policy::unique_enumerated_nearest_cpu(
      7, policy::HsaAgentType::Cpu, std::vector<std::uint64_t>{7, 7}));
  EXPECT(!policy::unique_enumerated_nearest_cpu(7, policy::HsaAgentType::Cpu,
                                                std::vector<std::uint64_t>{3}));

  candidates = valid_candidates();
  candidates.push_back(candidates[0]);
  candidates.back().handle = 13;
  EXPECT(select(candidates) ==
         policy::HsaPoolPolicyStatus::HostRoleCardinality);

  candidates = valid_candidates();
  candidates.push_back(candidates[1]);
  candidates.back().handle = 13;
  EXPECT(select(candidates) ==
         policy::HsaPoolPolicyStatus::KernargRoleCardinality);

  candidates = valid_candidates();
  candidates.push_back(candidates[2]);
  candidates.back().handle = 13;
  EXPECT(select(candidates) ==
         policy::HsaPoolPolicyStatus::DeviceRoleCardinality);

  for (std::size_t field = 0; field < 10; ++field) {
    candidates = valid_candidates();
    auto &host = candidates[0];
    switch (field) {
    case 0:
      host.owner = policy::HsaPoolOwner::SelectedGpu;
      break;
    case 1:
      host.location = policy::HsaPoolLocation::Gpu;
      break;
    case 2:
      host.global_segment = false;
      break;
    case 3:
      host.runtime_allocation_allowed = false;
      break;
    case 4:
      host.global_flags |= policy::kHsaPoolFlagKernargInit;
      break;
    case 5:
      host.maximum_allocation_size = kDataBytes - 1;
      break;
    case 6:
      host.allocation_granule = 0;
      break;
    case 7:
      host.allocation_alignment = 2;
      break;
    case 8:
      host.nearest_cpu_accessible = false;
      break;
    case 9:
      host.selected_gpu_accessible = false;
      break;
    }
    EXPECT(select(candidates) ==
           policy::HsaPoolPolicyStatus::HostRoleCardinality);
  }

  for (std::size_t field = 0; field < 10; ++field) {
    candidates = valid_candidates();
    auto &kernarg = candidates[1];
    switch (field) {
    case 0:
      kernarg.owner = policy::HsaPoolOwner::SelectedGpu;
      break;
    case 1:
      kernarg.location = policy::HsaPoolLocation::Gpu;
      break;
    case 2:
      kernarg.global_segment = false;
      break;
    case 3:
      kernarg.runtime_allocation_allowed = false;
      break;
    case 4:
      kernarg.global_flags = policy::kHsaPoolFlagFineGrained;
      break;
    case 5:
      kernarg.maximum_allocation_size = 4095;
      break;
    case 6:
      kernarg.allocation_granule = 0;
      break;
    case 7:
      kernarg.allocation_alignment = 8;
      break;
    case 8:
      kernarg.nearest_cpu_accessible = false;
      break;
    case 9:
      kernarg.selected_gpu_accessible = false;
      break;
    }
    EXPECT(select(candidates) ==
           policy::HsaPoolPolicyStatus::KernargRoleCardinality);
  }

  for (std::size_t field = 0; field < 10; ++field) {
    candidates = valid_candidates();
    auto &device = candidates[2];
    switch (field) {
    case 0:
      device.owner = policy::HsaPoolOwner::NearestCpu;
      break;
    case 1:
      device.location = policy::HsaPoolLocation::Cpu;
      break;
    case 2:
      device.global_segment = false;
      break;
    case 3:
      device.runtime_allocation_allowed = false;
      break;
    case 4:
      device.global_flags = policy::kHsaPoolFlagFineGrained;
      break;
    case 5:
      device.maximum_allocation_size = kDataBytes - 1;
      break;
    case 6:
      device.allocation_granule = 0;
      break;
    case 7:
      device.allocation_alignment = 2;
      break;
    case 8:
      device.nearest_cpu_accessible = false;
      break;
    case 9:
      device.selected_gpu_accessible = false;
      break;
    }
    EXPECT(select(candidates) ==
           policy::HsaPoolPolicyStatus::DeviceRoleCardinality);
  }

  candidates = valid_candidates();
  candidates[0].allocation_granule = 2 * kDataBytes;
  candidates[0].maximum_allocation_size = kDataBytes;
  EXPECT(select(candidates) ==
         policy::HsaPoolPolicyStatus::HostRoleCardinality);

  candidates = valid_candidates();
  candidates[0].maximum_allocation_size =
      std::numeric_limits<std::size_t>::max();
  candidates[0].allocation_granule = 4096;
  EXPECT(policy::select_hsa_pool_roles(
             candidates, std::numeric_limits<std::size_t>::max(), kKernargBytes,
             kKernargAlignment,
             &roles) == policy::HsaPoolPolicyStatus::HostRoleCardinality);

  candidates = valid_candidates();
  candidates[2].allocation_alignment = 3;
  EXPECT(select(candidates) ==
         policy::HsaPoolPolicyStatus::DeviceRoleCardinality);
  return 0;
}

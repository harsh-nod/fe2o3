#ifndef FE2O3_RUNTIME_GFX942_R26_HSA_POOL_POLICY_HPP
#define FE2O3_RUNTIME_GFX942_R26_HSA_POOL_POLICY_HPP

#include <algorithm>
#include <cstddef>
#include <cstdint>
#include <limits>
#include <vector>

namespace fe2o3::r26 {

constexpr std::uint32_t kHsaPoolFlagKernargInit = 0x1;
constexpr std::uint32_t kHsaPoolFlagFineGrained = 0x2;
constexpr std::uint32_t kHsaPoolFlagCoarseGrained = 0x4;

enum class HsaAgentType { Cpu, Gpu, Other };
enum class HsaPoolOwner { NearestCpu, SelectedGpu };
enum class HsaPoolLocation { Cpu, Gpu, Other };

struct HsaPoolCandidate {
  std::uint64_t handle = 0;
  HsaPoolOwner owner = HsaPoolOwner::NearestCpu;
  HsaPoolLocation location = HsaPoolLocation::Other;
  bool global_segment = false;
  bool runtime_allocation_allowed = false;
  std::uint32_t global_flags = 0;
  std::size_t maximum_allocation_size = 0;
  std::size_t allocation_granule = 0;
  std::size_t allocation_alignment = 0;
  bool nearest_cpu_accessible = false;
  bool selected_gpu_accessible = false;
};

struct HsaPoolRoles {
  std::size_t host = 0;
  std::size_t kernarg = 0;
  std::size_t device = 0;
};

enum class HsaPoolPolicyStatus {
  Accepted,
  InvalidRequest,
  HostRoleCardinality,
  KernargRoleCardinality,
  DeviceRoleCardinality,
};

inline bool unique_enumerated_nearest_cpu(
    std::uint64_t nearest_cpu_handle, HsaAgentType reported_type,
    const std::vector<std::uint64_t> &enumerated_cpu_handles) {
  return nearest_cpu_handle != 0 && reported_type == HsaAgentType::Cpu &&
         std::count(enumerated_cpu_handles.begin(),
                    enumerated_cpu_handles.end(), nearest_cpu_handle) == 1;
}

namespace detail {

inline bool is_power_of_two(std::size_t value) {
  return value != 0 && (value & (value - 1)) == 0;
}

inline bool checked_add(std::size_t left, std::size_t right,
                        std::size_t *result) {
  if (result == nullptr ||
      left > std::numeric_limits<std::size_t>::max() - right)
    return false;
  *result = left + right;
  return true;
}

inline bool supports_allocations(const HsaPoolCandidate &candidate,
                                 std::size_t allocation_size,
                                 std::size_t required_alignment) {
  if (candidate.handle == 0 || !candidate.global_segment ||
      !candidate.runtime_allocation_allowed || allocation_size == 0 ||
      !is_power_of_two(required_alignment) ||
      candidate.allocation_granule == 0 ||
      !is_power_of_two(candidate.allocation_alignment) ||
      candidate.allocation_alignment < required_alignment ||
      !candidate.nearest_cpu_accessible || !candidate.selected_gpu_accessible)
    return false;

  const std::size_t remainder = allocation_size % candidate.allocation_granule;
  std::size_t rounded_size = allocation_size;
  if (remainder != 0 &&
      !checked_add(allocation_size, candidate.allocation_granule - remainder,
                   &rounded_size))
    return false;
  return rounded_size <= candidate.maximum_allocation_size;
}

inline bool matches_role(const HsaPoolCandidate &candidate, HsaPoolOwner owner,
                         HsaPoolLocation location, std::uint32_t exact_flags,
                         std::size_t allocation_size,
                         std::size_t required_alignment) {
  return candidate.owner == owner && candidate.location == location &&
         candidate.global_flags == exact_flags &&
         supports_allocations(candidate, allocation_size, required_alignment);
}

template <typename Predicate>
inline std::size_t unique_match(const std::vector<HsaPoolCandidate> &candidates,
                                Predicate predicate,
                                std::size_t *selected_index) {
  std::size_t count = 0;
  for (std::size_t index = 0; index < candidates.size(); ++index) {
    if (!predicate(candidates[index]))
      continue;
    ++count;
    if (selected_index != nullptr)
      *selected_index = index;
  }
  return count;
}

} // namespace detail

inline HsaPoolPolicyStatus
select_hsa_pool_roles(const std::vector<HsaPoolCandidate> &candidates,
                      std::size_t data_bytes, std::size_t kernarg_bytes,
                      std::size_t kernarg_alignment, HsaPoolRoles *roles) {
  if (roles == nullptr || data_bytes == 0 || kernarg_bytes == 0 ||
      !detail::is_power_of_two(kernarg_alignment))
    return HsaPoolPolicyStatus::InvalidRequest;

  HsaPoolRoles selected;
  const auto host_matches = detail::unique_match(
      candidates,
      [&](const HsaPoolCandidate &candidate) {
        return detail::matches_role(
            candidate, HsaPoolOwner::NearestCpu, HsaPoolLocation::Cpu,
            kHsaPoolFlagFineGrained, data_bytes, alignof(std::uint32_t));
      },
      &selected.host);
  if (host_matches != 1)
    return HsaPoolPolicyStatus::HostRoleCardinality;

  const auto kernarg_matches = detail::unique_match(
      candidates,
      [&](const HsaPoolCandidate &candidate) {
        return detail::matches_role(
            candidate, HsaPoolOwner::NearestCpu, HsaPoolLocation::Cpu,
            kHsaPoolFlagFineGrained | kHsaPoolFlagKernargInit, kernarg_bytes,
            kernarg_alignment);
      },
      &selected.kernarg);
  if (kernarg_matches != 1)
    return HsaPoolPolicyStatus::KernargRoleCardinality;

  const auto device_matches = detail::unique_match(
      candidates,
      [&](const HsaPoolCandidate &candidate) {
        return detail::matches_role(
            candidate, HsaPoolOwner::SelectedGpu, HsaPoolLocation::Gpu,
            kHsaPoolFlagCoarseGrained, data_bytes, alignof(std::uint32_t));
      },
      &selected.device);
  if (device_matches != 1)
    return HsaPoolPolicyStatus::DeviceRoleCardinality;

  *roles = selected;
  return HsaPoolPolicyStatus::Accepted;
}

} // namespace fe2o3::r26

#endif

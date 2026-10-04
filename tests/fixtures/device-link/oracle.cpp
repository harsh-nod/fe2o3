#include <array>
#include <cstdint>

static std::uint32_t rust_accumulate_v1(std::uint32_t value,
                                        std::uint32_t lane) {
  return value + lane;
}

static std::uint32_t rust_calls_hip(std::uint32_t value, std::uint32_t lane) {
  return rust_accumulate_v1(value * 3u + 5u, lane);
}

static std::uint32_t hip_calls_rust(std::uint32_t value, std::uint32_t lane) {
  return rust_accumulate_v1(value * 7u + 11u, lane);
}

int main() {
  constexpr std::array<std::uint32_t, 5> input{
      0u, 1u, 7u, UINT32_MAX, 0x80000000u};
  constexpr std::array<std::uint32_t, 5> expected_rust_calls_hip{
      5u, 9u, 28u, 5u, 0x80000009u};
  constexpr std::array<std::uint32_t, 5> expected_hip_calls_rust{
      11u, 19u, 62u, 7u, 0x8000000fu};

  for (std::uint32_t lane = 0; lane < input.size(); ++lane) {
    if (rust_calls_hip(input[lane], lane) != expected_rust_calls_hip[lane])
      return 1;
    if (hip_calls_rust(input[lane], lane) != expected_hip_calls_rust[lane])
      return 2;
  }
  return 0;
}

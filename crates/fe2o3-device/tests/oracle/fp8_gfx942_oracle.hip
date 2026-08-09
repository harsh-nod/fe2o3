#include <hip/hip_fp8.h>
#include <hip/hip_runtime.h>

#include <array>
#include <bit>
#include <cstdint>
#include <cstdio>
#include <cstdlib>
#include <cstring>
#include <string>
#include <utility>
#include <vector>

namespace {
constexpr char GFX942_TARGET[] = "gfx942";

void check_hip(hipError_t status, const char *expression) {
  if (status != hipSuccess) {
    std::fprintf(stderr, "%s failed: %s\n", expression,
                 hipGetErrorString(status));
    std::exit(2);
  }
}

#define CHECK_HIP(expression) check_hip((expression), #expression)

__global__ void widen_all(std::uint32_t *e4m3, std::uint32_t *e5m2) {
  const unsigned raw = blockIdx.x * blockDim.x + threadIdx.x;
  if (raw >= 256)
    return;

  __hip_fp8_e4m3_fnuz e4;
  __hip_fp8_e5m2_fnuz e5;
  e4.__x = static_cast<unsigned char>(raw);
  e5.__x = static_cast<unsigned char>(raw);
  e4m3[raw] = std::bit_cast<std::uint32_t>(static_cast<float>(e4));
  e5m2[raw] = std::bit_cast<std::uint32_t>(static_cast<float>(e5));
}

__global__ void narrow_all(const std::uint32_t *inputs, unsigned char *e4m3,
                           unsigned char *e5m2, unsigned count) {
  const unsigned index = blockIdx.x * blockDim.x + threadIdx.x;
  if (index >= count)
    return;

  const float input = std::bit_cast<float>(inputs[index]);
  e4m3[index] = __hip_fp8_e4m3_fnuz(input).__x;
  e5m2[index] = __hip_fp8_e5m2_fnuz(input).__x;
}

__global__ void pack_all(const std::uint32_t *inputs, std::uint32_t *e4m3,
                         std::uint32_t *e5m2, unsigned count) {
  const unsigned index = blockIdx.x * blockDim.x + threadIdx.x;
  if (index >= count)
    return;

  const unsigned base = index * 4;
  const float4 lanes = make_float4(std::bit_cast<float>(inputs[base]),
                                   std::bit_cast<float>(inputs[base + 1]),
                                   std::bit_cast<float>(inputs[base + 2]),
                                   std::bit_cast<float>(inputs[base + 3]));
  e4m3[index] = __hip_fp8x4_e4m3_fnuz(lanes).__x;
  e5m2[index] = __hip_fp8x4_e5m2_fnuz(lanes).__x;
}

struct NarrowCase {
  char format;
  std::string label;
  std::uint32_t input;
};

struct PackCase {
  char format;
  std::string label;
  std::array<unsigned char, 4> lanes;
};

float as_float(std::uint32_t bits) { return std::bit_cast<float>(bits); }

std::uint32_t as_bits(float value) {
  return std::bit_cast<std::uint32_t>(value);
}

void add_narrowing_cases(char format,
                         const std::array<std::uint32_t, 256> &widened,
                         std::vector<NarrowCase> &cases) {
  char label[64];
  for (unsigned raw = 0; raw < 256; ++raw) {
    std::snprintf(label, sizeof(label), "exact-%02x", raw);
    cases.push_back({format, label, widened[raw]});
  }

  for (unsigned lower = 0; lower < 127; ++lower) {
    const float midpoint =
        (as_float(widened[lower]) + as_float(widened[lower + 1])) * 0.5f;
    const std::uint32_t midpoint_bits = as_bits(midpoint);
    const std::array<std::pair<const char *, std::uint32_t>, 6> boundaries = {{
        {"below-positive", midpoint_bits - 1},
        {"tie-positive", midpoint_bits},
        {"above-positive", midpoint_bits + 1},
        {"below-negative", (midpoint_bits - 1) | 0x80000000u},
        {"tie-negative", midpoint_bits | 0x80000000u},
        {"above-negative", (midpoint_bits + 1) | 0x80000000u},
    }};
    for (const auto &boundary : boundaries) {
      std::snprintf(label, sizeof(label), "boundary-%02x-%s", lower,
                    boundary.first);
      cases.push_back({format, label, boundary.second});
    }
  }

  const std::array<std::pair<const char *, std::uint32_t>, 28> exceptions = {{
      {"positive-zero", 0x00000000},
      {"negative-zero", 0x80000000},
      {"positive-infinity", 0x7f800000},
      {"negative-infinity", 0xff800000},
      {"positive-quiet-nan", 0x7fc00000},
      {"negative-quiet-nan", 0xffc00000},
      {"positive-signaling-nan", 0x7f800001},
      {"negative-signaling-nan", 0xff800001},
      {"positive-payload-nan", 0x7fe12345},
      {"negative-payload-nan", 0xffe12345},
      {"positive-min-subnormal-f32", 0x00000001},
      {"negative-min-subnormal-f32", 0x80000001},
      {"positive-max-subnormal-f32", 0x007fffff},
      {"negative-max-subnormal-f32", 0x807fffff},
      {"positive-min-normal-f32", 0x00800000},
      {"negative-min-normal-f32", 0x80800000},
      {"positive-max-f32", 0x7f7fffff},
      {"negative-max-f32", 0xff7fffff},
      {"positive-e4m3-max", 0x43700000},
      {"positive-e4m3-above-max", 0x43700001},
      {"negative-e4m3-above-max", 0xc3700001},
      {"positive-e5m2-max", 0x47600000},
      {"positive-e5m2-above-max", 0x47600001},
      {"negative-e5m2-above-max", 0xc7600001},
      {"positive-tiny-e4m3", 0x1a738b2f},
      {"negative-tiny-e4m3", 0x9c22a078},
      {"positive-tiny-e5m2", 0x1715609d},
      {"negative-tiny-e5m2", 0x96af9ca5},
  }};
  for (const auto &exception : exceptions) {
    cases.push_back({format, std::string("exception-") + exception.first,
                     exception.second});
  }
}

void add_pack_cases(char format, std::vector<PackCase> &cases) {
  cases.push_back({format, "lane-order", {0x01, 0x23, 0x45, 0x67}});
  cases.push_back({format, "limits", {0x00, 0x40, 0x7f, 0xff}});
  cases.push_back({format, "lane-zero", {0x01, 0x00, 0x00, 0x00}});
  cases.push_back({format, "lane-one", {0x00, 0x02, 0x00, 0x00}});
  cases.push_back({format, "lane-two", {0x00, 0x00, 0x04, 0x00}});
  cases.push_back({format, "lane-three", {0x00, 0x00, 0x00, 0x08}});
  cases.push_back({format, "nan-order", {0x80, 0x01, 0x81, 0x7f}});
}

} // namespace

int main() {
  hipDeviceProp_t device_properties{};
  CHECK_HIP(hipGetDeviceProperties(&device_properties, 0));
  const std::string gcn_arch_name = device_properties.gcnArchName;
  if (gcn_arch_name != GFX942_TARGET &&
      !gcn_arch_name.starts_with(std::string(GFX942_TARGET) + ":")) {
    std::fprintf(stderr, "HIP device 0 has gcnArchName '%s'; expected gfx942\n",
                 gcn_arch_name.c_str());
    return 3;
  }
  CHECK_HIP(hipSetDevice(0));
  std::printf("meta device-index 0\n");
  std::printf("meta device-gcn-arch-name %s\n", gcn_arch_name.c_str());

  std::uint32_t *device_e4_widen = nullptr, *device_e5_widen = nullptr;
  CHECK_HIP(hipMalloc(&device_e4_widen, 256 * sizeof(std::uint32_t)));
  CHECK_HIP(hipMalloc(&device_e5_widen, 256 * sizeof(std::uint32_t)));
  hipLaunchKernelGGL(widen_all, dim3(4), dim3(64), 0, 0, device_e4_widen,
                     device_e5_widen);
  CHECK_HIP(hipGetLastError());
  CHECK_HIP(hipDeviceSynchronize());

  std::array<std::uint32_t, 256> e4_widen{};
  std::array<std::uint32_t, 256> e5_widen{};
  CHECK_HIP(hipMemcpy(e4_widen.data(), device_e4_widen, sizeof(e4_widen),
                      hipMemcpyDeviceToHost));
  CHECK_HIP(hipMemcpy(e5_widen.data(), device_e5_widen, sizeof(e5_widen),
                      hipMemcpyDeviceToHost));
  CHECK_HIP(hipFree(device_e5_widen));
  CHECK_HIP(hipFree(device_e4_widen));

  for (unsigned raw = 0; raw < 256; ++raw) {
    std::printf("widen e4 %02x %08x\n", raw, e4_widen[raw]);
  }
  for (unsigned raw = 0; raw < 256; ++raw) {
    std::printf("widen e5 %02x %08x\n", raw, e5_widen[raw]);
  }

  std::vector<NarrowCase> narrow_cases;
  add_narrowing_cases('4', e4_widen, narrow_cases);
  add_narrowing_cases('5', e5_widen, narrow_cases);
  std::vector<std::uint32_t> narrow_inputs;
  narrow_inputs.reserve(narrow_cases.size());
  for (const auto &test : narrow_cases)
    narrow_inputs.push_back(test.input);

  std::uint32_t *device_narrow_inputs = nullptr;
  unsigned char *device_e4_narrow = nullptr, *device_e5_narrow = nullptr;
  CHECK_HIP(hipMalloc(&device_narrow_inputs,
                      narrow_inputs.size() * sizeof(std::uint32_t)));
  CHECK_HIP(hipMalloc(&device_e4_narrow, narrow_inputs.size()));
  CHECK_HIP(hipMalloc(&device_e5_narrow, narrow_inputs.size()));
  CHECK_HIP(hipMemcpy(device_narrow_inputs, narrow_inputs.data(),
                      narrow_inputs.size() * sizeof(std::uint32_t),
                      hipMemcpyHostToDevice));
  const unsigned narrow_blocks = (narrow_inputs.size() + 255) / 256;
  hipLaunchKernelGGL(narrow_all, dim3(narrow_blocks), dim3(256), 0, 0,
                     device_narrow_inputs, device_e4_narrow, device_e5_narrow,
                     static_cast<unsigned>(narrow_inputs.size()));
  CHECK_HIP(hipGetLastError());
  CHECK_HIP(hipDeviceSynchronize());
  std::vector<unsigned char> e4_narrow(narrow_inputs.size());
  std::vector<unsigned char> e5_narrow(narrow_inputs.size());
  CHECK_HIP(hipMemcpy(e4_narrow.data(), device_e4_narrow, e4_narrow.size(),
                      hipMemcpyDeviceToHost));
  CHECK_HIP(hipMemcpy(e5_narrow.data(), device_e5_narrow, e5_narrow.size(),
                      hipMemcpyDeviceToHost));
  CHECK_HIP(hipFree(device_e5_narrow));
  CHECK_HIP(hipFree(device_e4_narrow));
  CHECK_HIP(hipFree(device_narrow_inputs));

  for (std::size_t index = 0; index < narrow_cases.size(); ++index) {
    const auto &test = narrow_cases[index];
    const unsigned char expected =
        test.format == '4' ? e4_narrow[index] : e5_narrow[index];
    std::printf("narrow e%c %s %08x %02x\n", test.format, test.label.c_str(),
                test.input, expected);
  }

  std::vector<PackCase> pack_cases;
  add_pack_cases('4', pack_cases);
  add_pack_cases('5', pack_cases);
  std::vector<std::uint32_t> pack_inputs;
  pack_inputs.reserve(pack_cases.size() * 4);
  for (const auto &test : pack_cases) {
    const auto &widened = test.format == '4' ? e4_widen : e5_widen;
    for (unsigned char lane : test.lanes)
      pack_inputs.push_back(widened[lane]);
  }

  std::uint32_t *device_pack_inputs = nullptr, *device_e4_pack = nullptr,
                *device_e5_pack = nullptr;
  CHECK_HIP(hipMalloc(&device_pack_inputs,
                      pack_inputs.size() * sizeof(std::uint32_t)));
  CHECK_HIP(
      hipMalloc(&device_e4_pack, pack_cases.size() * sizeof(std::uint32_t)));
  CHECK_HIP(
      hipMalloc(&device_e5_pack, pack_cases.size() * sizeof(std::uint32_t)));
  CHECK_HIP(hipMemcpy(device_pack_inputs, pack_inputs.data(),
                      pack_inputs.size() * sizeof(std::uint32_t),
                      hipMemcpyHostToDevice));
  hipLaunchKernelGGL(pack_all, dim3(1), dim3(64), 0, 0, device_pack_inputs,
                     device_e4_pack, device_e5_pack,
                     static_cast<unsigned>(pack_cases.size()));
  CHECK_HIP(hipGetLastError());
  CHECK_HIP(hipDeviceSynchronize());
  std::vector<std::uint32_t> e4_pack(pack_cases.size());
  std::vector<std::uint32_t> e5_pack(pack_cases.size());
  CHECK_HIP(hipMemcpy(e4_pack.data(), device_e4_pack,
                      e4_pack.size() * sizeof(std::uint32_t),
                      hipMemcpyDeviceToHost));
  CHECK_HIP(hipMemcpy(e5_pack.data(), device_e5_pack,
                      e5_pack.size() * sizeof(std::uint32_t),
                      hipMemcpyDeviceToHost));
  CHECK_HIP(hipFree(device_e5_pack));
  CHECK_HIP(hipFree(device_e4_pack));
  CHECK_HIP(hipFree(device_pack_inputs));

  for (std::size_t index = 0; index < pack_cases.size(); ++index) {
    const auto &test = pack_cases[index];
    const std::uint32_t expected =
        test.format == '4' ? e4_pack[index] : e5_pack[index];
    std::printf("pack e%c %s %02x %02x %02x %02x %08x\n", test.format,
                test.label.c_str(), test.lanes[0], test.lanes[1], test.lanes[2],
                test.lanes[3], expected);
  }

  std::printf("meta widening-cases 512\n");
  std::printf("meta narrowing-cases %zu\n", narrow_cases.size());
  std::printf("meta packed-cases %zu\n", pack_cases.size());
}

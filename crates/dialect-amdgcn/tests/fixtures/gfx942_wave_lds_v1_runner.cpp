#include <hip/hip_runtime_api.h>

#include <cstdint>
#include <cstdio>
#include <cstdlib>
#include <vector>

static void check(hipError_t status, const char *operation) {
  if (status != hipSuccess) {
    std::fprintf(stderr, "%s: %s\n", operation, hipGetErrorString(status));
    std::exit(1);
  }
}

int main(int argc, char **argv) {
  if (argc != 2) {
    std::fprintf(stderr, "usage: %s wave_lds.hsaco\n", argv[0]);
    return 2;
  }

  constexpr std::size_t lanes = 256;
  std::vector<std::uint32_t> values(lanes);
  std::vector<std::uint32_t> active(lanes);
  std::vector<std::uint32_t> wave(lanes, 0xdeadbeefU);
  std::vector<std::uint32_t> workgroup(lanes, 0xdeadbeefU);
  for (std::size_t lane = 0; lane < lanes; ++lane) {
    values[lane] = static_cast<std::uint32_t>(lane + 1);
    active[lane] = lane % 3 == 1 ? 0U : 7U;
  }

  check(hipInit(0), "hipInit");
  check(hipSetDevice(0), "hipSetDevice");
  hipModule_t module = nullptr;
  hipFunction_t function = nullptr;
  check(hipModuleLoad(&module, argv[1]), "hipModuleLoad");
  check(hipModuleGetFunction(&function, module, "gfx942_wave_lds_v1_hw"),
        "hipModuleGetFunction");

  std::uint32_t *device_values = nullptr;
  std::uint32_t *device_active = nullptr;
  std::uint32_t *device_wave = nullptr;
  std::uint32_t *device_workgroup = nullptr;
  const std::size_t bytes = lanes * sizeof(std::uint32_t);
  check(hipMalloc(reinterpret_cast<void **>(&device_values), bytes),
        "hipMalloc values");
  check(hipMalloc(reinterpret_cast<void **>(&device_active), bytes),
        "hipMalloc active");
  check(hipMalloc(reinterpret_cast<void **>(&device_wave), bytes),
        "hipMalloc wave");
  check(hipMalloc(reinterpret_cast<void **>(&device_workgroup), bytes),
        "hipMalloc workgroup");
  check(hipMemcpy(device_values, values.data(), bytes, hipMemcpyHostToDevice),
        "copy values");
  check(hipMemcpy(device_active, active.data(), bytes, hipMemcpyHostToDevice),
        "copy active");
  check(hipMemcpy(device_wave, wave.data(), bytes, hipMemcpyHostToDevice),
        "initialize wave");
  check(hipMemcpy(device_workgroup, workgroup.data(), bytes,
                  hipMemcpyHostToDevice),
        "initialize workgroup");

  std::uint64_t length = lanes;
  void *parameters[] = {&device_values, &length,      &device_active, &length,
                        &device_wave,   &length,      &device_workgroup,
                        &length};
  check(hipModuleLaunchKernel(function, lanes, 1, 1, lanes, 1, 1, 0, nullptr,
                              parameters, nullptr),
        "hipModuleLaunchKernel");
  check(hipDeviceSynchronize(), "hipDeviceSynchronize");
  check(hipMemcpy(wave.data(), device_wave, bytes, hipMemcpyDeviceToHost),
        "read wave");
  check(hipMemcpy(workgroup.data(), device_workgroup, bytes,
                  hipMemcpyDeviceToHost),
        "read workgroup");

  std::uint32_t expected_workgroup = 0;
  for (std::size_t lane = 0; lane < lanes; ++lane) {
    if (active[lane] != 0)
      expected_workgroup += values[lane];
  }
  for (std::size_t lane = 0; lane < lanes; ++lane) {
    const std::size_t wave_start = lane / 64 * 64;
    std::uint32_t expected_wave = 0;
    for (std::size_t peer = wave_start; peer < wave_start + 64; ++peer) {
      if (active[peer] != 0)
        expected_wave += values[peer];
    }
    if (wave[lane] != expected_wave || workgroup[lane] != expected_workgroup) {
      std::fprintf(stderr,
                   "lane %zu: wave=%u expected=%u workgroup=%u expected=%u\n",
                   lane, wave[lane], expected_wave, workgroup[lane],
                   expected_workgroup);
      return 1;
    }
  }

  check(hipFree(device_workgroup), "hipFree workgroup");
  check(hipFree(device_wave), "hipFree wave");
  check(hipFree(device_active), "hipFree active");
  check(hipFree(device_values), "hipFree values");
  check(hipModuleUnload(module), "hipModuleUnload");
  std::puts("PASS gfx942 wave/LDS V1");
  return 0;
}

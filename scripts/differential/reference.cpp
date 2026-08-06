#include <cstdint>
#include <cstdio>
#include <cstring>
#include <limits>
#include <stdexcept>
#include <string>
#include <vector>

#if defined(FE2O3_REFERENCE_HIP)
#include <hip/hip_runtime.h>
#endif

namespace {

constexpr uint64_t kBaseSeed = UINT64_C(0x6a09e667f3bcc909);
constexpr uint64_t kLengthSalt = UINT64_C(0x9e3779b97f4a7c15);
constexpr uint32_t kLeftF32 = UINT32_C(0x4f123456);
constexpr uint32_t kRightF32 = UINT32_C(0xcf234567);
constexpr uint32_t kPoisonF32 = UINT32_C(0x7fc0d1ff);
constexpr size_t kLengths[] = {0, 1, 31, 255, 256, 257};

uint64_t mix64(uint64_t value) {
  value += UINT64_C(0x9e3779b97f4a7c15);
  value = (value ^ (value >> 30)) * UINT64_C(0xbf58476d1ce4e5b9);
  value = (value ^ (value >> 27)) * UINT64_C(0x94d049bb133111eb);
  return value ^ (value >> 31);
}

uint64_t case_seed(uint64_t kernel, size_t length) {
  return kBaseSeed ^ (kernel << 56) ^
         (static_cast<uint64_t>(length) * kLengthSalt);
}

int32_t sample_i32(uint64_t seed, size_t index, uint64_t channel) {
  const uint64_t value =
      mix64(seed ^ (static_cast<uint64_t>(index) * kLengthSalt) ^
            (channel * UINT64_C(0xd1b54a32d192ed03)));
  return static_cast<int32_t>(value % 2001) - 1000;
}

float bits_to_float(uint32_t bits) {
  float value;
  std::memcpy(&value, &bits, sizeof(value));
  return value;
}

uint32_t float_to_bits(float value) {
  uint32_t bits;
  std::memcpy(&bits, &value, sizeof(bits));
  return bits;
}

float sample_f32(uint64_t seed, size_t index) {
  if (index == 0) {
    return bits_to_float(UINT32_C(0x7fc12345));
  }
  if (index == 1) {
    return std::numeric_limits<float>::infinity();
  }
  return static_cast<float>(sample_i32(seed, index, 7)) / 32.0f;
}

float sample_vec_f32(uint64_t seed, size_t index, uint64_t channel) {
  if (index == 0 && channel == 2) {
    return bits_to_float(UINT32_C(0x7fc12345));
  }
  if (index == 1 && channel == 2) {
    return std::numeric_limits<float>::infinity();
  }
  return static_cast<float>(sample_i32(seed, index, channel)) / 32.0f;
}

void emit(const char *kernel, const char *kind, uint64_t seed,
          const std::vector<uint32_t> &words, uint32_t left, uint32_t right) {
  std::printf("FE2O3_DIFF_RESULT_V1\t%s\t%s\t%016llx\t%zu\t%08x\t%08x\t",
              kernel, kind, static_cast<unsigned long long>(seed), words.size(),
              left, right);
  for (uint32_t word : words) {
    std::printf("%08x", word);
  }
  std::printf("\n");
}

#if defined(FE2O3_REFERENCE_HIP)

void hip_check(hipError_t status, const char *operation) {
  if (status != hipSuccess) {
    throw std::runtime_error(std::string(operation) + ": " +
                             hipGetErrorString(status));
  }
}

__global__ void fill_kernel(const float *bounds, size_t bounds_len,
                            float *output, size_t output_len) {
  (void)bounds;
  const size_t index = blockIdx.x * blockDim.x + threadIdx.x;
  if (index < bounds_len && index < output_len) {
    output[index] = 42.5f;
  }
}

__global__ void vecadd_kernel(const float *a, size_t a_len, const float *b,
                              size_t b_len, float *output, size_t output_len) {
  const size_t index = blockIdx.x * blockDim.x + threadIdx.x;
  if (index < a_len && index < b_len && index < output_len) {
    output[index] = a[index] + b[index];
  }
}

__global__ void affine_kernel(float alpha, float bias, const float *input,
                              size_t input_len, float *output,
                              size_t output_len) {
  const size_t index = blockIdx.x * blockDim.x + threadIdx.x;
  if (index < input_len && index < output_len) {
    output[index] = alpha * input[index] + bias;
  }
}

template <typename T> T *device_copy(const std::vector<T> &host) {
  if (host.empty()) {
    return nullptr;
  }
  T *device = nullptr;
  hip_check(hipMalloc(&device, host.size() * sizeof(T)), "hipMalloc");
  hip_check(hipMemcpy(device, host.data(), host.size() * sizeof(T),
                      hipMemcpyHostToDevice),
            "hipMemcpy host-to-device");
  return device;
}

template <typename T> void copy_back(std::vector<T> &host, T *device) {
  hip_check(hipMemcpy(host.data(), device, host.size() * sizeof(T),
                      hipMemcpyDeviceToHost),
            "hipMemcpy device-to-host");
}

template <typename T> void release(T *device) {
  if (device != nullptr) {
    hip_check(hipFree(device), "hipFree");
  }
}

#endif

std::vector<float> run_fill(uint64_t seed, size_t length) {
  (void)seed;
  std::vector<float> bounds(length, 0.0f);
  std::vector<float> output(length + 2, bits_to_float(kPoisonF32));
  output.front() = bits_to_float(kLeftF32);
  output.back() = bits_to_float(kRightF32);
#if defined(FE2O3_REFERENCE_HIP)
  float *bounds_device = device_copy(bounds);
  float *output_device = device_copy(output);
  if (length != 0) {
    hipLaunchKernelGGL(fill_kernel, dim3((length + 255) / 256), dim3(256), 0, 0,
                       bounds_device, bounds.size(), output_device + 1, length);
    hip_check(hipGetLastError(), "fill launch");
    hip_check(hipDeviceSynchronize(), "fill synchronize");
  }
  copy_back(output, output_device);
  release(bounds_device);
  release(output_device);
#else
  for (size_t index = 0; index < bounds.size(); ++index) {
    output[index + 1] = 42.5f;
  }
#endif
  return output;
}

std::vector<float> run_vecadd(uint64_t seed, size_t length) {
  std::vector<float> a(length);
  std::vector<float> b(length);
  for (size_t index = 0; index < length; ++index) {
    a[index] = sample_vec_f32(seed, index, 2);
    b[index] = sample_vec_f32(seed, index, 3);
  }
  std::vector<float> output(length + 2, bits_to_float(kPoisonF32));
  output.front() = bits_to_float(kLeftF32);
  output.back() = bits_to_float(kRightF32);
#if defined(FE2O3_REFERENCE_HIP)
  float *a_device = device_copy(a);
  float *b_device = device_copy(b);
  float *output_device = device_copy(output);
  if (length != 0) {
    hipLaunchKernelGGL(vecadd_kernel, dim3((length + 255) / 256), dim3(256), 0,
                       0, a_device, a.size(), b_device, b.size(),
                       output_device + 1, length);
    hip_check(hipGetLastError(), "vecadd launch");
    hip_check(hipDeviceSynchronize(), "vecadd synchronize");
  }
  copy_back(output, output_device);
  release(a_device);
  release(b_device);
  release(output_device);
#else
  for (size_t index = 0; index < length; ++index) {
    output[index + 1] = a[index] + b[index];
  }
#endif
  return output;
}

std::vector<float> run_affine(uint64_t seed, size_t length) {
  constexpr float alpha = 1.25f;
  constexpr float bias = -0.75f;
  std::vector<float> input(length);
  for (size_t index = 0; index < length; ++index) {
    input[index] = sample_f32(seed, index);
  }
  std::vector<float> output(length + 2, bits_to_float(kPoisonF32));
  output.front() = bits_to_float(kLeftF32);
  output.back() = bits_to_float(kRightF32);
#if defined(FE2O3_REFERENCE_HIP)
  float *input_device = device_copy(input);
  float *output_device = device_copy(output);
  if (length != 0) {
    hipLaunchKernelGGL(affine_kernel, dim3((length + 255) / 256), dim3(256), 0,
                       0, alpha, bias, input_device, input.size(),
                       output_device + 1, length);
    hip_check(hipGetLastError(), "affine launch");
    hip_check(hipDeviceSynchronize(), "affine synchronize");
  }
  copy_back(output, output_device);
  release(input_device);
  release(output_device);
#else
  for (size_t index = 0; index < length; ++index) {
    output[index + 1] = alpha * input[index] + bias;
  }
#endif
  return output;
}

std::vector<uint32_t> interior_f32(const std::vector<float> &values) {
  std::vector<uint32_t> words;
  words.reserve(values.size() - 2);
  for (size_t index = 1; index + 1 < values.size(); ++index) {
    words.push_back(float_to_bits(values[index]));
  }
  return words;
}

} // namespace

int main() {
  try {
    for (size_t length : kLengths) {
      const uint64_t seed = case_seed(1, length);
      const auto output = run_fill(seed, length);
      emit("fill", "bits32", seed, interior_f32(output),
           float_to_bits(output.front()), float_to_bits(output.back()));
    }
    for (size_t length : kLengths) {
      const uint64_t seed = case_seed(2, length);
      const auto output = run_vecadd(seed, length);
      emit("vecadd", "f32", seed, interior_f32(output),
           float_to_bits(output.front()), float_to_bits(output.back()));
    }
    for (size_t length : kLengths) {
      const uint64_t seed = case_seed(3, length);
      const auto output = run_affine(seed, length);
      emit("affine", "f32", seed, interior_f32(output),
           float_to_bits(output.front()), float_to_bits(output.back()));
    }
  } catch (const std::exception &error) {
    std::fprintf(stderr, "reference failed: %s\n", error.what());
    return 1;
  }
  return 0;
}

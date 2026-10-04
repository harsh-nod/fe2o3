#include <hip/hip_runtime.h>

#include <algorithm>
#include <cmath>
#include <cstdint>
#include <cstdio>
#include <cstdlib>
#include <string>
#include <vector>

using i32x2 = int __attribute__((ext_vector_type(2)));
using i32x8 = int __attribute__((ext_vector_type(8)));
using f32x4 = float __attribute__((ext_vector_type(4)));
using lds_i32x2 = i32x2 __attribute__((address_space(3)));

namespace {

constexpr int kM = 16;
constexpr int kN = 16;
constexpr int kK = 128;
constexpr int kWaveSize = 64;

#define HIP_CHECK(call)                                                                  \
    do {                                                                                 \
        const hipError_t status = (call);                                                \
        if (status != hipSuccess) {                                                      \
            std::fprintf(stderr, "%s failed: %s\n", #call, hipGetErrorString(status)); \
            std::exit(1);                                                                \
        }                                                                                \
    } while (false)

__host__ __device__ float decode_fp4(std::uint8_t value)
{
    constexpr float positive[8] = {0.0f, 0.5f, 1.0f, 1.5f, 2.0f, 3.0f, 4.0f, 6.0f};
    const float magnitude = positive[value & 7];
    return (value & 8) != 0 ? -magnitude : magnitude;
}

__host__ __device__ float decode_fp8(std::uint8_t value)
{
    const int sign = value >> 7;
    const int exponent = (value >> 3) & 15;
    const int mantissa = value & 7;
    float magnitude;
    if (exponent == 0) {
        magnitude = std::ldexp(static_cast<float>(mantissa), -9);
    } else {
        magnitude = std::ldexp(1.0f + static_cast<float>(mantissa) / 8.0f, exponent - 7);
    }
    return sign != 0 ? -magnitude : magnitude;
}

std::uint8_t encode_fp4(float value)
{
    constexpr float table[8] = {0.0f, 0.5f, 1.0f, 1.5f, 2.0f, 3.0f, 4.0f, 6.0f};
    const bool negative = std::signbit(value);
    const float magnitude = std::fabs(value);
    int best = 0;
    for (int i = 1; i < 8; ++i) {
        if (std::fabs(table[i] - magnitude) < std::fabs(table[best] - magnitude)) {
            best = i;
        }
    }
    return static_cast<std::uint8_t>(best | (negative && best != 0 ? 8 : 0));
}

std::uint8_t encode_fp8(float value)
{
    if (value == 0.0f) {
        return 0;
    }
    const bool negative = std::signbit(value);
    float magnitude = std::fabs(value);
    int exponent = 0;
    const float fraction = std::frexp(magnitude, &exponent);
    int encoded_exponent = exponent - 1 + 7;
    int mantissa = static_cast<int>(std::nearbyint((fraction * 2.0f - 1.0f) * 8.0f));
    if (mantissa == 8) {
        mantissa = 0;
        ++encoded_exponent;
    }
    encoded_exponent = std::max(0, std::min(15, encoded_exponent));
    return static_cast<std::uint8_t>((negative ? 0x80 : 0) | (encoded_exponent << 3) | mantissa);
}

__device__ i32x8 pack_fp4_fragment(const std::uint8_t* matrix, int row, int column_stride)
{
    const int lane = static_cast<int>(threadIdx.x) & 63;
    const int depth_base = (lane / 16) * 32;
    i32x8 packed = {};
#pragma unroll
    for (int i = 0; i < 32; ++i) {
        const std::uint32_t nibble = matrix[(depth_base + i) * column_stride + row] & 15;
        packed[i / 8] |= static_cast<int>(nibble << ((i % 8) * 4));
    }
    return packed;
}

__device__ i32x8 pack_fp8_fragment(const std::uint8_t* matrix, int row, int column_stride)
{
    const int lane = static_cast<int>(threadIdx.x) & 63;
    const int group = lane / 16;
    i32x8 packed = {};
#pragma unroll
    for (int i = 0; i < 32; ++i) {
        const int depth = i < 16 ? group * 16 + i : 64 + group * 16 + (i - 16);
        const std::uint32_t byte = matrix[depth * column_stride + row];
        packed[i / 4] |= static_cast<int>(byte << ((i % 4) * 8));
    }
    return packed;
}

// A is row-major. Treating its row as the fragment's "column" lets the same
// packer gather A[row, depth] without a second hand-written packing path.
__device__ i32x8 pack_fp4_a(const std::uint8_t* matrix, int row)
{
    const int lane = static_cast<int>(threadIdx.x) & 63;
    const int depth_base = (lane / 16) * 32;
    i32x8 packed = {};
#pragma unroll
    for (int i = 0; i < 32; ++i) {
        const std::uint32_t nibble = matrix[row * kK + depth_base + i] & 15;
        packed[i / 8] |= static_cast<int>(nibble << ((i % 8) * 4));
    }
    return packed;
}

__device__ i32x8 pack_fp8_a(const std::uint8_t* matrix, int row)
{
    const int lane = static_cast<int>(threadIdx.x) & 63;
    const int group = lane / 16;
    i32x8 packed = {};
#pragma unroll
    for (int i = 0; i < 32; ++i) {
        const int depth = i < 16 ? group * 16 + i : 64 + group * 16 + (i - 16);
        const std::uint32_t byte = matrix[row * kK + depth];
        packed[i / 4] |= static_cast<int>(byte << ((i % 4) * 8));
    }
    return packed;
}

__device__ i32x2 stage_fp4_transpose_source(
    const std::uint8_t* matrix,
    int lane,
    int part)
{
    const int lane_in_group = lane & 15;
    const int depth = (lane / 16) * 32 + part * 16 + lane_in_group;
    i32x2 packed = {};
#pragma unroll
    for (int token = 0; token < 16; ++token) {
        const std::uint32_t nibble = matrix[token * kK + depth] & 15;
        packed[token / 8] |= static_cast<int>(nibble << ((token % 8) * 4));
    }
    return packed;
}

__device__ i32x2 stage_fp8_transpose_source(
    const std::uint8_t* matrix,
    int lane,
    int part)
{
    const int lane_in_group = lane & 15;
    const int token_base = (lane_in_group & 1) * 8;
    const int group = lane / 16;
    const int depth = part < 2
        ? group * 16 + part * 8 + lane_in_group / 2
        : 64 + group * 16 + (part - 2) * 8 + lane_in_group / 2;
    i32x2 packed = {};
#pragma unroll
    for (int token_offset = 0; token_offset < 8; ++token_offset) {
        const std::uint32_t byte = matrix[(token_base + token_offset) * kK + depth];
        packed[token_offset / 4] |= static_cast<int>(byte << ((token_offset % 4) * 8));
    }
    return packed;
}

__device__ f32x4 mfma_fp4(i32x8 a, i32x8 b)
{
    const f32x4 zero = {};
    return __builtin_amdgcn_mfma_scale_f32_16x16x128_f8f6f4(
        a, b, zero, 4, 4, 0, 0, 0, 0);
}

__device__ f32x4 mfma_fp8(i32x8 a, i32x8 b)
{
    const f32x4 zero = {};
    return __builtin_amdgcn_mfma_scale_f32_16x16x128_f8f6f4(
        a, b, zero, 0, 0, 0, 0, 0, 0);
}

template <typename Encode>
std::vector<std::uint8_t> make_matrix(int rows, int columns, Encode encode, int salt)
{
    std::vector<std::uint8_t> result(static_cast<std::size_t>(rows) * columns);
    constexpr float values[] = {-1.0f, -0.5f, 0.0f, 0.5f, 1.0f};
    for (int row = 0; row < rows; ++row) {
        for (int column = 0; column < columns; ++column) {
            result[row * columns + column] = encode(values[(row * 3 + column * 2 + salt) % 5]);
        }
    }
    return result;
}

template <typename Decode>
std::vector<float> gemm_reference(
    const std::vector<std::uint8_t>& a,
    const std::vector<std::uint8_t>& b,
    Decode decode)
{
    std::vector<float> result(kM * kN, 0.0f);
    for (int row = 0; row < kM; ++row) {
        for (int column = 0; column < kN; ++column) {
            for (int depth = 0; depth < kK; ++depth) {
                result[row * kN + column] +=
                    decode(a[row * kK + depth]) * decode(b[depth * kN + column]);
            }
        }
    }
    return result;
}

template <typename Decode>
std::vector<float> attention_reference(
    const std::vector<std::uint8_t>& q,
    const std::vector<std::uint8_t>& k,
    const std::vector<std::uint8_t>& v,
    Decode decode)
{
    constexpr int tokens = 16;
    constexpr int value_width = 16;
    const float scale = 1.0f / std::sqrt(static_cast<float>(kK));
    std::vector<float> result(tokens * value_width, 0.0f);
    std::vector<float> scores(tokens);
    for (int row = 0; row < tokens; ++row) {
        float maximum = -INFINITY;
        for (int column = 0; column < tokens; ++column) {
            float score = 0.0f;
            for (int depth = 0; depth < kK; ++depth) {
                score += decode(q[row * kK + depth]) * decode(k[column * kK + depth]);
            }
            scores[column] = score * scale;
            maximum = std::max(maximum, scores[column]);
        }
        float denominator = 0.0f;
        for (float score : scores) {
            denominator += std::exp(score - maximum);
        }
        for (int dimension = 0; dimension < value_width; ++dimension) {
            for (int column = 0; column < tokens; ++column) {
                const float probability = std::exp(scores[column] - maximum) / denominator;
                result[row * value_width + dimension] +=
                    probability * decode(v[column * value_width + dimension]);
            }
        }
    }
    return result;
}

void check_result(const char* name, const std::vector<float>& actual, const std::vector<float>& expected)
{
    float maximum_error = 0.0f;
    int maximum_index = 0;
    for (std::size_t i = 0; i < actual.size(); ++i) {
        if (!std::isfinite(actual[i]) || !std::isfinite(expected[i])) {
            std::fprintf(
                stderr,
                "FAIL %-14s non-finite index=%zu gpu=%g cpu=%g\n",
                name,
                i,
                actual[i],
                expected[i]);
            std::exit(1);
        }
        const float error = std::fabs(actual[i] - expected[i]);
        if (!std::isfinite(error)) {
            std::fprintf(stderr, "FAIL %-14s non-finite error index=%zu\n", name, i);
            std::exit(1);
        }
        if (error > maximum_error) {
            maximum_error = error;
            maximum_index = static_cast<int>(i);
        }
    }
    if (maximum_error > 2.0e-3f) {
        std::fprintf(
            stderr,
            "FAIL %-14s max_error=%g index=%d gpu=%g cpu=%g\n",
            name,
            maximum_error,
            maximum_index,
            actual[maximum_index],
            expected[maximum_index]);
        std::exit(1);
    }
    std::printf("PASS %-14s max_error=%g\n", name, maximum_error);
}

template <typename Kernel>
std::vector<float> run_gemm(
    Kernel kernel,
    const std::vector<std::uint8_t>& a,
    const std::vector<std::uint8_t>& b)
{
    std::uint8_t* device_a = nullptr;
    std::uint8_t* device_b = nullptr;
    float* device_c = nullptr;
    HIP_CHECK(hipMalloc(&device_a, a.size()));
    HIP_CHECK(hipMalloc(&device_b, b.size()));
    HIP_CHECK(hipMalloc(&device_c, kM * kN * sizeof(float)));
    HIP_CHECK(hipMemcpy(device_a, a.data(), a.size(), hipMemcpyHostToDevice));
    HIP_CHECK(hipMemcpy(device_b, b.data(), b.size(), hipMemcpyHostToDevice));
    kernel<<<1, kWaveSize>>>(device_a, device_b, device_c);
    HIP_CHECK(hipGetLastError());
    std::vector<float> c(kM * kN);
    HIP_CHECK(hipMemcpy(c.data(), device_c, c.size() * sizeof(float), hipMemcpyDeviceToHost));
    HIP_CHECK(hipFree(device_a));
    HIP_CHECK(hipFree(device_b));
    HIP_CHECK(hipFree(device_c));
    return c;
}

template <typename Kernel>
std::vector<float> run_attention(
    Kernel kernel,
    const std::vector<std::uint8_t>& q,
    const std::vector<std::uint8_t>& k,
    const std::vector<std::uint8_t>& v)
{
    std::uint8_t* device_q = nullptr;
    std::uint8_t* device_k = nullptr;
    std::uint8_t* device_v = nullptr;
    float* device_o = nullptr;
    HIP_CHECK(hipMalloc(&device_q, q.size()));
    HIP_CHECK(hipMalloc(&device_k, k.size()));
    HIP_CHECK(hipMalloc(&device_v, v.size()));
    HIP_CHECK(hipMalloc(&device_o, kM * kN * sizeof(float)));
    HIP_CHECK(hipMemcpy(device_q, q.data(), q.size(), hipMemcpyHostToDevice));
    HIP_CHECK(hipMemcpy(device_k, k.data(), k.size(), hipMemcpyHostToDevice));
    HIP_CHECK(hipMemcpy(device_v, v.data(), v.size(), hipMemcpyHostToDevice));
    kernel<<<1, kWaveSize>>>(device_q, device_k, device_v, device_o);
    HIP_CHECK(hipGetLastError());
    std::vector<float> o(kM * kN);
    HIP_CHECK(hipMemcpy(o.data(), device_o, o.size() * sizeof(float), hipMemcpyDeviceToHost));
    HIP_CHECK(hipFree(device_q));
    HIP_CHECK(hipFree(device_k));
    HIP_CHECK(hipFree(device_v));
    HIP_CHECK(hipFree(device_o));
    return o;
}

} // namespace

extern "C" __global__ __launch_bounds__(64) void gfx950_fp4_gemm(
    const std::uint8_t* a,
    const std::uint8_t* b,
    float* c)
{
    const int lane = static_cast<int>(threadIdx.x) & 63;
    const int fragment_row = lane & 15;
    const f32x4 accumulator = mfma_fp4(pack_fp4_a(a, fragment_row), pack_fp4_fragment(b, fragment_row, kN));
#pragma unroll
    for (int component = 0; component < 4; ++component) {
        const int row = (lane / 16) * 4 + component;
        c[row * kN + fragment_row] = accumulator[component];
    }
}

extern "C" __global__ __launch_bounds__(64) void gfx950_fp8_gemm(
    const std::uint8_t* a,
    const std::uint8_t* b,
    float* c)
{
    const int lane = static_cast<int>(threadIdx.x) & 63;
    const int fragment_row = lane & 15;
    const f32x4 accumulator = mfma_fp8(pack_fp8_a(a, fragment_row), pack_fp8_fragment(b, fragment_row, kN));
#pragma unroll
    for (int component = 0; component < 4; ++component) {
        const int row = (lane / 16) * 4 + component;
        c[row * kN + fragment_row] = accumulator[component];
    }
}

extern "C" __global__ __launch_bounds__(64) void gfx950_fp4_flash_attention(
    const std::uint8_t* q,
    const std::uint8_t* k,
    const std::uint8_t* v,
    float* output)
{
    __shared__ i32x2 transposed_k[64 * 2];
    const int lane = static_cast<int>(threadIdx.x) & 63;
    const int column = lane & 15;
#pragma unroll
    for (int part = 0; part < 2; ++part) {
        transposed_k[lane * 2 + part] = stage_fp4_transpose_source(k, lane, part);
    }
    __syncthreads();

    i32x8 b = {};
    const i32x2 b0 = __builtin_amdgcn_ds_read_tr4_b64_v2i32(
        (lds_i32x2*)&transposed_k[lane * 2]);
    const i32x2 b1 = __builtin_amdgcn_ds_read_tr4_b64_v2i32(
        (lds_i32x2*)&transposed_k[lane * 2 + 1]);
    b[0] = b0[0];
    b[1] = b0[1];
    b[2] = b1[0];
    b[3] = b1[1];
    f32x4 score = mfma_fp4(pack_fp4_a(q, column), b);

#pragma unroll
    for (int component = 0; component < 4; ++component) {
        float scaled = score[component] * 0.08838834764831845f;
        float maximum = scaled;
#pragma unroll
        for (int offset = 1; offset < 16; offset *= 2) {
            maximum = fmaxf(maximum, __shfl_xor(maximum, offset, 16));
        }
        float probability = expf(scaled - maximum);
        float denominator = probability;
#pragma unroll
        for (int offset = 1; offset < 16; offset *= 2) {
            denominator += __shfl_xor(denominator, offset, 16);
        }
        probability /= denominator;

        float value = 0.0f;
#pragma unroll
        for (int token = 0; token < 16; ++token) {
            const float token_probability = __shfl(probability, token, 16);
            value += token_probability * decode_fp4(v[token * 16 + column]);
        }
        const int row = (lane / 16) * 4 + component;
        output[row * 16 + column] = value;
    }
}

extern "C" __global__ __launch_bounds__(64) void gfx950_fp8_flash_attention(
    const std::uint8_t* q,
    const std::uint8_t* k,
    const std::uint8_t* v,
    float* output)
{
    __shared__ i32x2 transposed_k[64 * 4];
    const int lane = static_cast<int>(threadIdx.x) & 63;
    const int column = lane & 15;
#pragma unroll
    for (int part = 0; part < 4; ++part) {
        transposed_k[lane * 4 + part] = stage_fp8_transpose_source(k, lane, part);
    }
    __syncthreads();

    i32x8 b = {};
#pragma unroll
    for (int part = 0; part < 4; ++part) {
        const i32x2 pair =
            __builtin_amdgcn_ds_read_tr8_b64_v2i32(
                (lds_i32x2*)&transposed_k[lane * 4 + part]);
        b[part * 2] = pair[0];
        b[part * 2 + 1] = pair[1];
    }
    f32x4 score = mfma_fp8(pack_fp8_a(q, column), b);

#pragma unroll
    for (int component = 0; component < 4; ++component) {
        float scaled = score[component] * 0.08838834764831845f;
        float maximum = scaled;
#pragma unroll
        for (int offset = 1; offset < 16; offset *= 2) {
            maximum = fmaxf(maximum, __shfl_xor(maximum, offset, 16));
        }
        float probability = expf(scaled - maximum);
        float denominator = probability;
#pragma unroll
        for (int offset = 1; offset < 16; offset *= 2) {
            denominator += __shfl_xor(denominator, offset, 16);
        }
        probability /= denominator;

        float value = 0.0f;
#pragma unroll
        for (int token = 0; token < 16; ++token) {
            const float token_probability = __shfl(probability, token, 16);
            value += token_probability * decode_fp8(v[token * 16 + column]);
        }
        const int row = (lane / 16) * 4 + component;
        output[row * 16 + column] = value;
    }
}

int main()
{
    int device = 0;
    HIP_CHECK(hipGetDevice(&device));
    hipDeviceProp_t properties{};
    HIP_CHECK(hipGetDeviceProperties(&properties, device));
    if (std::string(properties.gcnArchName).find("gfx950") == std::string::npos) {
        std::fprintf(
            stderr,
            "This example requires a visible gfx950 device; device %d is %s (%s).\n",
            device,
            properties.name,
            properties.gcnArchName);
        return 2;
    }

    const auto fp4_a = make_matrix(kM, kK, encode_fp4, 1);
    const auto fp4_b = make_matrix(kK, kN, encode_fp4, 3);
    check_result(
        "FP4 GEMM",
        run_gemm(gfx950_fp4_gemm, fp4_a, fp4_b),
        gemm_reference(fp4_a, fp4_b, decode_fp4));

    const auto fp8_a = make_matrix(kM, kK, encode_fp8, 2);
    const auto fp8_b = make_matrix(kK, kN, encode_fp8, 4);
    check_result(
        "FP8 GEMM",
        run_gemm(gfx950_fp8_gemm, fp8_a, fp8_b),
        gemm_reference(fp8_a, fp8_b, decode_fp8));

    auto fp4_q = make_matrix(kM, kK, encode_fp4, 0);
    auto fp4_k = make_matrix(kN, kK, encode_fp4, 4);
    auto fp4_v = make_matrix(kN, kN, encode_fp4, 2);
    check_result(
        "FP4 attention",
        run_attention(gfx950_fp4_flash_attention, fp4_q, fp4_k, fp4_v),
        attention_reference(fp4_q, fp4_k, fp4_v, decode_fp4));

    auto fp8_q = make_matrix(kM, kK, encode_fp8, 1);
    auto fp8_k = make_matrix(kN, kK, encode_fp8, 0);
    auto fp8_v = make_matrix(kN, kN, encode_fp8, 3);
    check_result(
        "FP8 attention",
        run_attention(gfx950_fp8_flash_attention, fp8_q, fp8_k, fp8_v),
        attention_reference(fp8_q, fp8_k, fp8_v, decode_fp8));

    return 0;
}

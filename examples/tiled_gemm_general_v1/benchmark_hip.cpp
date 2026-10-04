#include <hip/hip_runtime.h>

#include <algorithm>
#include <cstdint>
#include <cstdio>
#include <cstdlib>
#include <vector>

using u16x4 = unsigned short __attribute__((ext_vector_type(4)));
using f32x4 = float __attribute__((ext_vector_type(4)));

#define HIP_CHECK(call)                                                                     \
    do {                                                                                    \
        hipError_t status = (call);                                                         \
        if (status != hipSuccess) {                                                         \
            std::fprintf(stderr, "%s failed: %s\n", #call, hipGetErrorString(status));      \
            std::exit(1);                                                                   \
        }                                                                                   \
    } while (false)

__global__ __launch_bounds__(64) void tiled_gemm_general_hip(
    const std::uint16_t* a,
    std::size_t a_len,
    const std::uint16_t* b,
    std::size_t b_len,
    float* c,
    std::size_t c_len,
    std::uint32_t m,
    std::uint32_t n,
    std::uint32_t k,
    std::uint32_t lda,
    std::uint32_t ldb,
    std::uint32_t ldc,
    float alpha,
    float beta)
{
    const bool invalid_stride = (m != 0 && k != 0 && lda < k)
        || (k != 0 && n != 0 && ldb < n)
        || (m != 0 && n != 0 && ldc < n);
    const std::size_t a_extent = m == 0 || k == 0
        ? 0
        : static_cast<std::size_t>(m - 1) * lda + k;
    const std::size_t b_extent = k == 0 || n == 0
        ? 0
        : static_cast<std::size_t>(k - 1) * ldb + n;
    const std::size_t c_extent = m == 0 || n == 0
        ? 0
        : static_cast<std::size_t>(m - 1) * ldc + n;
    if (invalid_stride || a_len < a_extent || b_len < b_extent || c_len < c_extent) {
        return;
    }
    const std::size_t raw_index = blockIdx.x * blockDim.x + threadIdx.x;
    const std::size_t lane = raw_index % 64;
    const std::size_t tiles_per_row = (static_cast<std::size_t>(n) + 15) / 16;
    if (tiles_per_row == 0) {
        return;
    }
    const std::size_t tile = raw_index / 64;
    const std::size_t tile_row = tile / tiles_per_row;
    const std::size_t tile_column = tile % tiles_per_row;
    const std::size_t lane_column = lane % 16;
    const std::size_t depth_offset = (lane / 16) * 4;
    const std::size_t a_row = tile_row * 16 + lane_column;
    const std::size_t b_column = tile_column * 16 + lane_column;

    f32x4 accumulator = {0.0f, 0.0f, 0.0f, 0.0f};
    for (std::size_t phase = 0; phase < k; phase += 16) {
        u16x4 lhs = {};
        u16x4 rhs = {};
#pragma unroll
        for (std::size_t component = 0; component < 4; ++component) {
            const std::size_t depth = phase + depth_offset + component;
            lhs[component] = a_row < m && depth < k ? a[a_row * lda + depth] : 0;
            rhs[component] = depth < k && b_column < n ? b[depth * ldb + b_column] : 0;
        }
        accumulator = __builtin_amdgcn_mfma_f32_16x16x16bf16_1k(
            lhs, rhs, accumulator, 0, 0, 0);
    }

#pragma unroll
    for (std::size_t component = 0; component < 4; ++component) {
        const std::size_t row = tile_row * 16 + (lane / 16) * 4 + component;
        const std::size_t column = tile_column * 16 + lane_column;
        if (row < m && column < n) {
            const std::size_t index = row * ldc + column;
            c[index] = alpha * accumulator[component] + beta * c[index];
        }
    }
}

static float percentile(const std::vector<float>& sorted, std::size_t percentile)
{
    return sorted[(sorted.size() - 1) * percentile / 100];
}

static void benchmark(std::uint32_t size, std::size_t launches_per_sample)
{
    constexpr std::size_t warmups = 5;
    constexpr std::size_t samples = 15;
    const std::size_t elements = static_cast<std::size_t>(size) * size;
    std::uint16_t* a = nullptr;
    std::uint16_t* b = nullptr;
    float* c = nullptr;
    HIP_CHECK(hipMalloc(&a, elements * sizeof(*a)));
    HIP_CHECK(hipMalloc(&b, elements * sizeof(*b)));
    HIP_CHECK(hipMalloc(&c, elements * sizeof(*c)));
    HIP_CHECK(hipMemset(a, 0, elements * sizeof(*a)));
    HIP_CHECK(hipMemset(b, 0, elements * sizeof(*b)));
    HIP_CHECK(hipMemset(c, 0, elements * sizeof(*c)));

    const dim3 block(64);
    const dim3 grid(((size + 15) / 16) * ((size + 15) / 16));
    for (std::size_t i = 0; i < warmups; ++i) {
        tiled_gemm_general_hip<<<grid, block>>>(
            a, elements, b, elements, c, elements, size, size, size, size, size, size, 1.0f, 0.0f);
    }
    HIP_CHECK(hipDeviceSynchronize());

    hipEvent_t start = nullptr;
    hipEvent_t stop = nullptr;
    HIP_CHECK(hipEventCreate(&start));
    HIP_CHECK(hipEventCreate(&stop));
    std::vector<float> microseconds;
    microseconds.reserve(samples);
    for (std::size_t sample = 0; sample < samples; ++sample) {
        HIP_CHECK(hipEventRecord(start));
        for (std::size_t launch = 0; launch < launches_per_sample; ++launch) {
            tiled_gemm_general_hip<<<grid, block>>>(
                a, elements, b, elements, c, elements, size, size, size, size, size, size, 1.0f, 0.0f);
        }
        HIP_CHECK(hipEventRecord(stop));
        HIP_CHECK(hipEventSynchronize(stop));
        float milliseconds = 0.0f;
        HIP_CHECK(hipEventElapsedTime(&milliseconds, start, stop));
        microseconds.push_back(milliseconds * 1000.0f / launches_per_sample);
    }
    std::sort(microseconds.begin(), microseconds.end());
    const float median_us = percentile(microseconds, 50);
    const double operations = 2.0 * size * size * size;
    const double gflops = operations / median_us / 1000.0;
    std::printf(
        "BENCH hip M=%u N=%u K=%u median_us=%.3f p10_us=%.3f p90_us=%.3f gflops=%.2f\n",
        size,
        size,
        size,
        median_us,
        percentile(microseconds, 10),
        percentile(microseconds, 90),
        gflops);

    HIP_CHECK(hipEventDestroy(start));
    HIP_CHECK(hipEventDestroy(stop));
    HIP_CHECK(hipFree(a));
    HIP_CHECK(hipFree(b));
    HIP_CHECK(hipFree(c));
}

int main()
{
    HIP_CHECK(hipSetDevice(0));
    benchmark(256, 20);
    benchmark(512, 10);
    benchmark(1024, 3);
}

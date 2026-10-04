#include <hip/hip_runtime.h>

#include <algorithm>
#include <cerrno>
#include <cstdio>
#include <cstdlib>
#include <cstring>
#include <limits>
#include <vector>

#define HIP_CHECK(call)                                                        \
    do {                                                                       \
        const hipError_t status = (call);                                      \
        if (status != hipSuccess) {                                            \
            std::fprintf(stderr, "%s failed: %s\n", #call,                  \
                         hipGetErrorString(status));                           \
            return 1;                                                          \
        }                                                                      \
    } while (false)

__global__ void vecadd(const float* a, const float* b, float* c,
                       std::size_t length) {
    const std::size_t index =
        static_cast<std::size_t>(blockIdx.x) * blockDim.x + threadIdx.x;
    if (index < length) {
        c[index] = a[index] + b[index];
    }
}

static bool parse_size(const char* name, const char* text, std::size_t* value) {
    errno = 0;
    char* end = nullptr;
    const unsigned long long parsed = std::strtoull(text, &end, 10);
    if (errno != 0 || end == text || *end != '\0' ||
        parsed > std::numeric_limits<std::size_t>::max()) {
        std::fprintf(stderr, "invalid %s=%s\n", name, text);
        return false;
    }
    *value = static_cast<std::size_t>(parsed);
    return true;
}

static float percentile(const std::vector<float>& sorted, std::size_t percent) {
    return sorted[(sorted.size() - 1) * percent / 100];
}

int main(int argc, char** argv) {
    std::size_t length = 1024;
    std::size_t warmups = 0;
    std::size_t samples = 0;
    std::size_t launches_per_sample = 1;
    if (argc != 1 &&
        (argc != 6 || std::strcmp(argv[1], "--benchmark") != 0 ||
         !parse_size("N", argv[2], &length) ||
         !parse_size("WARMUPS", argv[3], &warmups) ||
         !parse_size("SAMPLES", argv[4], &samples) ||
         !parse_size("LAUNCHES_PER_SAMPLE", argv[5], &launches_per_sample))) {
        std::fputs(
            "usage: vecadd-hip [--benchmark N WARMUPS SAMPLES LAUNCHES_PER_SAMPLE]\n",
            stderr);
        return 1;
    }
    if (length == 0 || length > std::numeric_limits<unsigned int>::max() ||
        (samples != 0 && launches_per_sample == 0)) {
        std::fputs("HIP VecAdd requires 1 <= N <= UINT_MAX and a nonzero timed batch\n",
                   stderr);
        return 1;
    }

    std::vector<float> a(length);
    std::vector<float> b(length);
    std::vector<float> c(length);
    for (std::size_t i = 0; i < length; ++i) {
        a[i] = static_cast<float>(i);
        b[i] = static_cast<float>(i) * 2.0F;
    }

    float* device_a = nullptr;
    float* device_b = nullptr;
    float* device_c = nullptr;
    const std::size_t bytes = length * sizeof(float);
    HIP_CHECK(hipMalloc(&device_a, bytes));
    HIP_CHECK(hipMalloc(&device_b, bytes));
    HIP_CHECK(hipMalloc(&device_c, bytes));
    HIP_CHECK(hipMemcpy(device_a, a.data(), bytes, hipMemcpyHostToDevice));
    HIP_CHECK(hipMemcpy(device_b, b.data(), bytes, hipMemcpyHostToDevice));

    constexpr unsigned int threads = 256;
    const unsigned int blocks = static_cast<unsigned int>((length + threads - 1) / threads);
    for (std::size_t i = 0; i < warmups; ++i) {
        hipLaunchKernelGGL(vecadd, dim3(blocks), dim3(threads), 0, nullptr,
                           device_a, device_b, device_c, length);
    }

    if (samples == 0) {
        hipLaunchKernelGGL(vecadd, dim3(blocks), dim3(threads), 0, nullptr,
                           device_a, device_b, device_c, length);
        HIP_CHECK(hipGetLastError());
    } else {
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
                hipLaunchKernelGGL(vecadd, dim3(blocks), dim3(threads), 0, nullptr,
                                   device_a, device_b, device_c, length);
            }
            HIP_CHECK(hipGetLastError());
            HIP_CHECK(hipEventRecord(stop));
            HIP_CHECK(hipEventSynchronize(stop));
            float milliseconds = 0.0F;
            HIP_CHECK(hipEventElapsedTime(&milliseconds, start, stop));
            microseconds.push_back(milliseconds * 1000.0F /
                                   static_cast<float>(launches_per_sample));
        }
        std::sort(microseconds.begin(), microseconds.end());
        const float median_us = percentile(microseconds, 50);
        const double bandwidth_gb_s =
            static_cast<double>(3 * bytes) / static_cast<double>(median_us) / 1000.0;
        std::printf(
            "hip vecadd queued path: n=%zu samples=%zu batch=%zu "
            "event_interval_median_us=%.3f p10_us=%.3f p90_us=%.3f "
            "effective_bandwidth_gb_s=%.2f\n",
            length, samples, launches_per_sample, median_us,
            percentile(microseconds, 10), percentile(microseconds, 90),
            bandwidth_gb_s);
        HIP_CHECK(hipEventDestroy(stop));
        HIP_CHECK(hipEventDestroy(start));
    }

    HIP_CHECK(hipMemcpy(c.data(), device_c, bytes, hipMemcpyDeviceToHost));
    for (std::size_t i = 0; i < length; ++i) {
        const float expected = a[i] + b[i];
        if (c[i] != expected) {
            std::fprintf(stderr, "mismatch at %zu: expected %.9g, got %.9g\n", i,
                         expected, c[i]);
            return 1;
        }
    }

    HIP_CHECK(hipFree(device_c));
    HIP_CHECK(hipFree(device_b));
    HIP_CHECK(hipFree(device_a));
    std::printf("hip vecadd passed for %zu elements\n", length);
    return 0;
}

#include <hip/hip_runtime.h>
#include <hipblaslt/hipblaslt-ext.hpp>
#include <hipblaslt/hipblaslt.h>

#include <algorithm>
#include <cmath>
#include <cstdint>
#include <cstdio>
#include <cstdlib>
#include <fstream>
#include <functional>
#include <iostream>
#include <limits>
#include <string>
#include <vector>

namespace {

constexpr std::size_t M = 16;
constexpr std::size_t N = 16;
constexpr std::size_t K = 128;
constexpr std::size_t Batches = 16;
constexpr std::size_t AElements = M * K;
constexpr std::size_t BElements = K * N;
constexpr std::size_t CElements = M * N;
constexpr unsigned GridX = 4;
constexpr unsigned WorkgroupX = 256;
constexpr int Warmups = 1000;
constexpr int Samples = 3000;
constexpr int ThroughputSamples = 300;
constexpr int ThroughputLaunchesPerSample = 100;
constexpr int HipblasltSolution = 458429;

[[noreturn]] void fail(const std::string& message) {
  std::cerr << "ERROR: " << message << '\n';
  std::exit(1);
}

void check_hip(hipError_t status, const char* operation) {
  if (status != hipSuccess) {
    fail(std::string(operation) + ": " + hipGetErrorString(status));
  }
}

void check_blas(hipblasStatus_t status, const char* operation) {
  if (status != HIPBLAS_STATUS_SUCCESS) {
    fail(std::string(operation) + ": hipBLASLt status " +
         std::to_string(static_cast<int>(status)));
  }
}

float decode_e4m3(std::uint8_t bits) {
  const std::uint8_t exponent = (bits >> 3) & 0xf;
  const std::uint8_t mantissa = bits & 0x7;
  if (exponent == 0xf && mantissa == 0x7) {
    return std::numeric_limits<float>::quiet_NaN();
  }
  const float magnitude = exponent == 0
                              ? static_cast<float>(mantissa) / 512.0f
                              : (1.0f + static_cast<float>(mantissa) / 8.0f) *
                                    std::ldexp(1.0f, static_cast<int>(exponent) - 7);
  return (bits & 0x80) == 0 ? magnitude : -magnitude;
}

void deterministic_inputs(std::vector<std::uint8_t>& lhs,
                          std::vector<std::uint8_t>& rhs) {
  constexpr std::uint8_t codes[] = {0x00, 0x28, 0xa8, 0x30, 0xb0, 0x38,
                                    0xb8, 0x3c, 0xbc, 0x40, 0xc0};
  constexpr std::size_t code_count = sizeof(codes) / sizeof(codes[0]);
  lhs.resize(Batches * AElements);
  rhs.resize(Batches * BElements);
  for (std::size_t batch = 0; batch < Batches; ++batch) {
    for (std::size_t index = 0; index < AElements; ++index) {
      const std::size_t row = index / K;
      const std::size_t depth = index % K;
      lhs[batch * AElements + index] =
          codes[(row * 3 + depth * 5 + row * depth + batch +
                 (batch / code_count) * (depth % 5)) %
                code_count];
    }
    for (std::size_t index = 0; index < BElements; ++index) {
      const std::size_t depth = index / N;
      const std::size_t column = index % N;
      rhs[batch * BElements + index] =
          codes[(depth * 7 + column * 2 + depth * column + 1 + batch * 3 +
                 (batch / code_count) * (column % 5)) %
                code_count];
    }
  }
}

std::vector<float> cpu_reference(const std::vector<std::uint8_t>& lhs,
                                 const std::vector<std::uint8_t>& rhs) {
  std::vector<float> result(Batches * CElements, 0.0f);
  for (std::size_t batch = 0; batch < Batches; ++batch) {
    for (std::size_t row = 0; row < M; ++row) {
      for (std::size_t column = 0; column < N; ++column) {
        float value = 0.0f;
        for (std::size_t depth = 0; depth < K; ++depth) {
          value += decode_e4m3(lhs[batch * AElements + row * K + depth]) *
                   decode_e4m3(rhs[batch * BElements + depth * N + column]);
        }
        result[batch * CElements + row * N + column] = value;
      }
    }
  }
  return result;
}

float verify(const char* implementation, const std::vector<float>& actual,
             const std::vector<float>& expected) {
  float maximum_error = 0.0f;
  for (std::size_t index = 0; index < actual.size(); ++index) {
    const float error = std::abs(actual[index] - expected[index]);
    maximum_error = std::max(maximum_error, error);
    if (!std::isfinite(actual[index]) || error > 1.0e-5f) {
      fail(std::string(implementation) + " output mismatch at index " +
           std::to_string(index) + ": actual=" + std::to_string(actual[index]) +
           " expected=" + std::to_string(expected[index]));
    }
  }
  return maximum_error;
}

struct Summary {
  float p05;
  float p50;
  float p95;
};

Summary summarize(std::vector<float> values) {
  std::sort(values.begin(), values.end());
  return {values[values.size() / 20], values[values.size() / 2],
          values[values.size() * 19 / 20]};
}

float time_one(hipStream_t stream, hipEvent_t start, hipEvent_t stop,
               const std::function<void()>& launch) {
  check_hip(hipEventRecord(start, stream), "hipEventRecord(start)");
  launch();
  check_hip(hipEventRecord(stop, stream), "hipEventRecord(stop)");
  check_hip(hipEventSynchronize(stop), "hipEventSynchronize(stop)");
  float milliseconds = 0.0f;
  check_hip(hipEventElapsedTime(&milliseconds, start, stop),
            "hipEventElapsedTime");
  return milliseconds * 1000.0f;
}

float time_many(hipStream_t stream, hipEvent_t start, hipEvent_t stop,
                const std::function<void()>& launch) {
  check_hip(hipEventRecord(start, stream), "hipEventRecord(start)");
  for (int index = 0; index < ThroughputLaunchesPerSample; ++index) {
    launch();
  }
  check_hip(hipEventRecord(stop, stream), "hipEventRecord(stop)");
  check_hip(hipEventSynchronize(stop), "hipEventSynchronize(stop)");
  float milliseconds = 0.0f;
  check_hip(hipEventElapsedTime(&milliseconds, start, stop),
            "hipEventElapsedTime");
  return milliseconds * 1000.0f /
         static_cast<float>(ThroughputLaunchesPerSample);
}

void set_batched_layout(hipblasLtMatrixLayout_t layout, std::int64_t stride) {
  const std::int32_t batch_count = Batches;
  check_blas(hipblasLtMatrixLayoutSetAttribute(
                 layout, HIPBLASLT_MATRIX_LAYOUT_BATCH_COUNT, &batch_count,
                 sizeof(batch_count)),
             "hipblasLtMatrixLayoutSetAttribute(batch_count)");
  check_blas(hipblasLtMatrixLayoutSetAttribute(
                 layout, HIPBLASLT_MATRIX_LAYOUT_STRIDED_BATCH_OFFSET, &stride,
                 sizeof(stride)),
             "hipblasLtMatrixLayoutSetAttribute(stride)");
}

}  // namespace

int main(int argc, char** argv) {
  if (argc != 3) {
    std::cerr << "usage: " << argv[0] << " <fe2o3-fp8.hsaco> <raw.csv>\n";
    return 2;
  }

  check_hip(hipSetDevice(0), "hipSetDevice");
  hipStream_t stream = nullptr;
  hipEvent_t start = nullptr;
  hipEvent_t stop = nullptr;
  check_hip(hipStreamCreate(&stream), "hipStreamCreate");
  check_hip(hipEventCreate(&start), "hipEventCreate(start)");
  check_hip(hipEventCreate(&stop), "hipEventCreate(stop)");

  std::vector<std::uint8_t> lhs_host;
  std::vector<std::uint8_t> rhs_host;
  deterministic_inputs(lhs_host, rhs_host);
  const std::vector<float> expected = cpu_reference(lhs_host, rhs_host);

  std::uint8_t* lhs = nullptr;
  std::uint8_t* rhs = nullptr;
  float* fe2o3_output = nullptr;
  float* hipblaslt_output = nullptr;
  check_hip(hipMalloc(reinterpret_cast<void**>(&lhs), lhs_host.size()),
            "hipMalloc(lhs)");
  check_hip(hipMalloc(reinterpret_cast<void**>(&rhs), rhs_host.size()),
            "hipMalloc(rhs)");
  check_hip(hipMalloc(reinterpret_cast<void**>(&fe2o3_output),
                      expected.size() * sizeof(float)),
            "hipMalloc(fe2o3_output)");
  check_hip(hipMalloc(reinterpret_cast<void**>(&hipblaslt_output),
                      expected.size() * sizeof(float)),
            "hipMalloc(hipblaslt_output)");
  check_hip(hipMemcpyAsync(lhs, lhs_host.data(), lhs_host.size(),
                          hipMemcpyHostToDevice, stream),
            "hipMemcpyAsync(lhs)");
  check_hip(hipMemcpyAsync(rhs, rhs_host.data(), rhs_host.size(),
                          hipMemcpyHostToDevice, stream),
            "hipMemcpyAsync(rhs)");
  check_hip(hipMemsetAsync(fe2o3_output, 0,
                          expected.size() * sizeof(float), stream),
            "hipMemsetAsync(fe2o3_output)");
  check_hip(hipMemsetAsync(hipblaslt_output, 0,
                          expected.size() * sizeof(float), stream),
            "hipMemsetAsync(hipblaslt_output)");

  hipModule_t module = nullptr;
  hipFunction_t function = nullptr;
  check_hip(hipModuleLoad(&module, argv[1]), "hipModuleLoad");
  check_hip(hipModuleGetFunction(&function, module, "gfx950_fp8_gemm_rust"),
            "hipModuleGetFunction");
  std::uint64_t lhs_elements = lhs_host.size();
  std::uint64_t rhs_elements = rhs_host.size();
  std::uint64_t output_elements = expected.size();
  void* fe2o3_arguments[] = {&lhs,          &lhs_elements, &rhs,
                             &rhs_elements, &fe2o3_output, &output_elements};
  const auto launch_fe2o3 = [&] {
    check_hip(hipModuleLaunchKernel(function, GridX, 1, 1, WorkgroupX, 1, 1, 0,
                                    stream, fe2o3_arguments, nullptr),
              "hipModuleLaunchKernel(fe2o3)");
  };

  hipblasLtHandle_t handle = nullptr;
  hipblasLtMatmulDesc_t operation = nullptr;
  hipblasLtMatrixLayout_t a_layout = nullptr;
  hipblasLtMatrixLayout_t b_layout = nullptr;
  hipblasLtMatrixLayout_t c_layout = nullptr;
  hipblasLtMatrixLayout_t d_layout = nullptr;
  check_blas(hipblasLtCreate(&handle), "hipblasLtCreate");
  check_blas(hipblasLtMatmulDescCreate(&operation, HIPBLAS_COMPUTE_32F,
                                       HIP_R_32F),
             "hipblasLtMatmulDescCreate");
  const hipblasOperation_t op_n = HIPBLAS_OP_N;
  check_blas(hipblasLtMatmulDescSetAttribute(
                 operation, HIPBLASLT_MATMUL_DESC_TRANSA, &op_n, sizeof(op_n)),
             "hipblasLtMatmulDescSetAttribute(transA)");
  check_blas(hipblasLtMatmulDescSetAttribute(
                 operation, HIPBLASLT_MATMUL_DESC_TRANSB, &op_n, sizeof(op_n)),
             "hipblasLtMatmulDescSetAttribute(transB)");

  // hipBLASLt uses column-major descriptors here. rhs(row KxN) is A(col NxK),
  // lhs(row MxK) is B(col KxM), and D(col NxM) has the same bytes as the
  // desired row-major MxN result: D = rhs^T * lhs^T = (lhs * rhs)^T.
  check_blas(hipblasLtMatrixLayoutCreate(&a_layout, HIP_R_8F_E4M3, N, K, N),
             "hipblasLtMatrixLayoutCreate(A)");
  check_blas(hipblasLtMatrixLayoutCreate(&b_layout, HIP_R_8F_E4M3, K, M, K),
             "hipblasLtMatrixLayoutCreate(B)");
  check_blas(hipblasLtMatrixLayoutCreate(&c_layout, HIP_R_32F, N, M, N),
             "hipblasLtMatrixLayoutCreate(C)");
  check_blas(hipblasLtMatrixLayoutCreate(&d_layout, HIP_R_32F, N, M, N),
             "hipblasLtMatrixLayoutCreate(D)");
  set_batched_layout(a_layout, BElements);
  set_batched_layout(b_layout, AElements);
  set_batched_layout(c_layout, CElements);
  set_batched_layout(d_layout, CElements);

  std::vector<int> requested_indices{HipblasltSolution};
  std::vector<hipblasLtMatmulHeuristicResult_t> algorithms;
  check_blas(hipblaslt_ext::getAlgosFromIndex(handle, requested_indices, algorithms),
             "hipblaslt_ext::getAlgosFromIndex");
  if (algorithms.size() != 1) {
    fail("hipBLASLt did not return pinned solution 458429");
  }
  hipblasLtMatmulAlgo_t algorithm = algorithms.front().algo;
  const float alpha = 1.0f;
  const float beta = 0.0f;
  hipblaslt_ext::Gemm hipblaslt_gemm(
      handle, operation, &alpha, rhs, a_layout, lhs, b_layout, &beta,
      hipblaslt_output, c_layout, hipblaslt_output, d_layout);
  std::size_t workspace_bytes = 0;
  check_blas(hipblaslt_gemm.isAlgoSupported(algorithm, workspace_bytes),
             "hipblaslt_ext::Gemm::isAlgoSupported");
  hipblaslt_gemm.setMaxWorkspaceBytes(workspace_bytes);
  void* workspace = nullptr;
  if (workspace_bytes != 0) {
    check_hip(hipMalloc(&workspace, workspace_bytes), "hipMalloc(workspace)");
  }
  check_blas(hipblaslt_gemm.initialize(algorithm, workspace, true, stream),
             "hipblaslt_ext::Gemm::initialize");
  check_hip(hipStreamSynchronize(stream),
            "hipStreamSynchronize(hipBLASLt initialization)");
  const std::string solution_name = hipblaslt_gemm.getSolutionName();
  const std::string kernel_name = hipblaslt_gemm.getKernelName();
  const auto launch_hipblaslt = [&] {
    check_blas(hipblaslt_gemm.run(stream), "hipblaslt_ext::Gemm::run");
  };

  launch_fe2o3();
  launch_hipblaslt();
  check_hip(hipStreamSynchronize(stream), "hipStreamSynchronize(correctness)");
  std::vector<float> fe2o3_host(expected.size());
  std::vector<float> hipblaslt_host(expected.size());
  check_hip(hipMemcpy(fe2o3_host.data(), fe2o3_output,
                      expected.size() * sizeof(float), hipMemcpyDeviceToHost),
            "hipMemcpy(fe2o3_output)");
  check_hip(hipMemcpy(hipblaslt_host.data(), hipblaslt_output,
                      expected.size() * sizeof(float), hipMemcpyDeviceToHost),
            "hipMemcpy(hipblaslt_output)");
  const float fe2o3_error = verify("fe2o3", fe2o3_host, expected);
  const float hipblaslt_error = verify("hipBLASLt", hipblaslt_host, expected);

  for (int index = 0; index < Warmups; ++index) {
    if ((index & 1) == 0) {
      launch_fe2o3();
      launch_hipblaslt();
    } else {
      launch_hipblaslt();
      launch_fe2o3();
    }
  }
  check_hip(hipStreamSynchronize(stream), "hipStreamSynchronize(warmups)");

  std::vector<float> fe2o3_times;
  std::vector<float> hipblaslt_times;
  fe2o3_times.reserve(Samples);
  hipblaslt_times.reserve(Samples);
  std::ofstream raw(argv[2]);
  if (!raw) {
    fail(std::string("could not open raw timing file: ") + argv[2]);
  }
  raw << "mode,sample,launches_per_event_interval,first,fe2o3_us,hipblaslt_us\n";
  for (int index = 0; index < Samples; ++index) {
    float fe2o3_us = 0.0f;
    float hipblaslt_us = 0.0f;
    if ((index & 1) == 0) {
      fe2o3_us = time_one(stream, start, stop, launch_fe2o3);
      hipblaslt_us = time_one(stream, start, stop, launch_hipblaslt);
    } else {
      hipblaslt_us = time_one(stream, start, stop, launch_hipblaslt);
      fe2o3_us = time_one(stream, start, stop, launch_fe2o3);
    }
    fe2o3_times.push_back(fe2o3_us);
    hipblaslt_times.push_back(hipblaslt_us);
    raw << "synchronized_latency," << index << ",1,"
        << (((index & 1) == 0) ? "fe2o3" : "hipblaslt") << ',' << fe2o3_us
        << ',' << hipblaslt_us << '\n';
  }
  const Summary fe2o3_summary = summarize(fe2o3_times);
  const Summary hipblaslt_summary = summarize(hipblaslt_times);

  std::vector<float> fe2o3_throughput_times;
  std::vector<float> hipblaslt_throughput_times;
  fe2o3_throughput_times.reserve(ThroughputSamples);
  hipblaslt_throughput_times.reserve(ThroughputSamples);
  for (int index = 0; index < ThroughputSamples; ++index) {
    float fe2o3_us = 0.0f;
    float hipblaslt_us = 0.0f;
    if ((index & 1) == 0) {
      fe2o3_us = time_many(stream, start, stop, launch_fe2o3);
      hipblaslt_us = time_many(stream, start, stop, launch_hipblaslt);
    } else {
      hipblaslt_us = time_many(stream, start, stop, launch_hipblaslt);
      fe2o3_us = time_many(stream, start, stop, launch_fe2o3);
    }
    fe2o3_throughput_times.push_back(fe2o3_us);
    hipblaslt_throughput_times.push_back(hipblaslt_us);
    raw << "queue_hot_amortized," << index << ','
        << ThroughputLaunchesPerSample << ','
        << (((index & 1) == 0) ? "fe2o3" : "hipblaslt") << ',' << fe2o3_us
        << ',' << hipblaslt_us << '\n';
  }
  raw.close();
  const Summary fe2o3_throughput_summary =
      summarize(fe2o3_throughput_times);
  const Summary hipblaslt_throughput_summary =
      summarize(hipblaslt_throughput_times);

  int hipblaslt_version = 0;
  check_blas(hipblasLtGetVersion(handle, &hipblaslt_version),
             "hipblasLtGetVersion");
  std::cout << "{\"shape\":{\"batch\":16,\"m\":16,\"n\":16,\"k\":128},"
            << "\"precision\":\"E4M3xE4M3->FP32\","
            << "\"timer\":\"HIP events on one non-default stream\","
            << "\"cache\":\"persistent cache-hot allocations; alternating pair order\","
            << "\"hipblaslt_api\":\"hipblaslt_ext::Gemm initialized once with UserArgs\","
            << "\"warmups_per_implementation\":" << Warmups << ','
            << "\"synchronized_latency\":{\"samples_per_implementation\":"
            << Samples << ",\"launches_per_event_interval\":1,"
            << "\"fe2o3\":{\"p05_us\":" << fe2o3_summary.p05
            << ",\"p50_us\":" << fe2o3_summary.p50
            << ",\"p95_us\":" << fe2o3_summary.p95
            << ",\"max_abs_error\":" << fe2o3_error << "},"
            << "\"hipblaslt\":{\"p05_us\":" << hipblaslt_summary.p05
            << ",\"p50_us\":" << hipblaslt_summary.p50
            << ",\"p95_us\":" << hipblaslt_summary.p95
            << ",\"max_abs_error\":" << hipblaslt_error << "}},"
            << "\"queue_hot_amortized\":{\"samples_per_implementation\":"
            << ThroughputSamples << ",\"launches_per_event_interval\":"
            << ThroughputLaunchesPerSample
            << ",\"fe2o3\":{\"p05_us\":" << fe2o3_throughput_summary.p05
            << ",\"p50_us\":" << fe2o3_throughput_summary.p50
            << ",\"p95_us\":" << fe2o3_throughput_summary.p95 << "},"
            << "\"hipblaslt\":{\"p05_us\":"
            << hipblaslt_throughput_summary.p05
            << ",\"p50_us\":" << hipblaslt_throughput_summary.p50
            << ",\"p95_us\":" << hipblaslt_throughput_summary.p95 << "}},"
            << "\"hipblaslt\":{\"version\":" << hipblaslt_version
            << ",\"solution_index\":" << HipblasltSolution << "},"
            << "\"solution_name\":\"" << solution_name << "\","
            << "\"kernel_name\":\"" << kernel_name << "\"}\n";

  if (workspace != nullptr) {
    check_hip(hipFree(workspace), "hipFree(workspace)");
  }
  check_blas(hipblasLtMatrixLayoutDestroy(d_layout),
             "hipblasLtMatrixLayoutDestroy(D)");
  check_blas(hipblasLtMatrixLayoutDestroy(c_layout),
             "hipblasLtMatrixLayoutDestroy(C)");
  check_blas(hipblasLtMatrixLayoutDestroy(b_layout),
             "hipblasLtMatrixLayoutDestroy(B)");
  check_blas(hipblasLtMatrixLayoutDestroy(a_layout),
             "hipblasLtMatrixLayoutDestroy(A)");
  check_blas(hipblasLtMatmulDescDestroy(operation),
             "hipblasLtMatmulDescDestroy");
  check_blas(hipblasLtDestroy(handle), "hipblasLtDestroy");
  check_hip(hipModuleUnload(module), "hipModuleUnload");
  check_hip(hipFree(hipblaslt_output), "hipFree(hipblaslt_output)");
  check_hip(hipFree(fe2o3_output), "hipFree(fe2o3_output)");
  check_hip(hipFree(rhs), "hipFree(rhs)");
  check_hip(hipFree(lhs), "hipFree(lhs)");
  check_hip(hipEventDestroy(stop), "hipEventDestroy(stop)");
  check_hip(hipEventDestroy(start), "hipEventDestroy(start)");
  check_hip(hipStreamDestroy(stream), "hipStreamDestroy");
  return 0;
}

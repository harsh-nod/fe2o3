#include <hip/hip_runtime.h>

#include <algorithm>
#include <array>
#include <cmath>
#include <cstdint>
#include <cstdio>
#include <cstdlib>
#include <cstring>
#include <limits>
#include <numeric>
#include <string>
#include <vector>

using i32x8 = int __attribute__((ext_vector_type(8)));
using f32x4 = float __attribute__((ext_vector_type(4)));

namespace {

constexpr int kTokens = 16;
constexpr int kHidden = 128;
constexpr int kOutput = 16;
constexpr int kExperts = 4;
constexpr int kSharedExpert = 4;
constexpr int kAllExperts = 5;
constexpr int kTopK = 2;
constexpr int kMoeThreads = 320;
constexpr int kDispatchCapacity = kTokens * kTopK;

constexpr int kCandidates = 8;
constexpr int kDraftSteps = 4;
constexpr int kStateWidth = 8;

constexpr int kQueries = 8;
constexpr int kNgram = 3;
constexpr int kTableSize = 16;

constexpr int kMuonDim = 4;
constexpr int kMuonElements = kMuonDim * kMuonDim;
constexpr int kGradientShards = 2;
constexpr int kMuonIterations = 5;
constexpr float kMuonLearningRate = 0.05f;

#define HIP_CHECK(call)                                                                    \
    do {                                                                                   \
        const hipError_t status = (call);                                                  \
        if (status != hipSuccess) {                                                        \
            std::fprintf(stderr, "%s failed: %s\n", #call, hipGetErrorString(status)); \
            std::exit(1);                                                                  \
        }                                                                                  \
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
    const float magnitude = std::fabs(value);
    int exponent = 0;
    const float fraction = std::frexp(magnitude, &exponent);
    int encoded_exponent = exponent - 1 + 7;
    int mantissa = static_cast<int>(std::nearbyint((fraction * 2.0f - 1.0f) * 8.0f));
    if (mantissa == 8) {
        mantissa = 0;
        ++encoded_exponent;
    }
    encoded_exponent = std::max(0, std::min(15, encoded_exponent));
    return static_cast<std::uint8_t>(
        (negative ? 0x80 : 0) | (encoded_exponent << 3) | mantissa);
}

__host__ __device__ float silu(float value)
{
    return value / (1.0f + expf(-value));
}

__device__ i32x8 pack_fp4_rows(const std::uint8_t* matrix, int row)
{
    const int lane = static_cast<int>(threadIdx.x) & 63;
    const int depth_base = (lane / 16) * 32;
    i32x8 packed = {};
#pragma unroll
    for (int i = 0; i < 32; ++i) {
        const std::uint32_t nibble = matrix[row * kHidden + depth_base + i] & 15;
        packed[i / 8] |= static_cast<int>(nibble << ((i % 8) * 4));
    }
    return packed;
}

__device__ i32x8 pack_fp8_columns(
    const std::uint8_t* weights,
    int expert,
    int column)
{
    const int lane = static_cast<int>(threadIdx.x) & 63;
    const int depth_group = lane / 16;
    const std::uint8_t* expert_weights =
        weights + static_cast<std::size_t>(expert) * kHidden * kOutput;
    i32x8 packed = {};
    // CDNA4 FP8 lanes take 16 K values from each 64-element half; FP4 lanes
    // take one contiguous 32-element group. This split aligns mixed operands.
#pragma unroll
    for (int i = 0; i < 32; ++i) {
        const int depth = i < 16
            ? depth_group * 16 + i
            : 64 + depth_group * 16 + (i - 16);
        const std::uint32_t byte = expert_weights[depth * kOutput + column];
        packed[i / 4] |= static_cast<int>(byte << ((i % 4) * 8));
    }
    return packed;
}

__device__ f32x4 mixed_fp4_fp8_mfma(i32x8 activations, i32x8 weights)
{
    const f32x4 zero = {};
    return __builtin_amdgcn_mfma_scale_f32_16x16x128_f8f6f4(
        activations, weights, zero, 4, 0, 0, 0, 0, 0);
}

bool better_logit(float candidate, int candidate_id, float current, int current_id)
{
    return candidate > current || (candidate == current && candidate_id < current_id);
}

std::uint64_t ngram_hash(const std::int32_t* gram)
{
    std::uint64_t hash = 1469598103934665603ULL;
    for (int i = 0; i < kNgram; ++i) {
        hash ^= static_cast<std::uint32_t>(gram[i]);
        hash *= 1099511628211ULL;
    }
    return hash;
}

template <typename T>
T* device_alloc(std::size_t count)
{
    T* pointer = nullptr;
    HIP_CHECK(hipMalloc(&pointer, count * sizeof(T)));
    return pointer;
}

template <typename T>
void copy_to_device(T* destination, const std::vector<T>& source)
{
    HIP_CHECK(hipMemcpy(
        destination,
        source.data(),
        source.size() * sizeof(T),
        hipMemcpyHostToDevice));
}

template <typename T>
std::vector<T> copy_from_device(const T* source, std::size_t count)
{
    std::vector<T> result(count);
    HIP_CHECK(hipMemcpy(
        result.data(), source, count * sizeof(T), hipMemcpyDeviceToHost));
    return result;
}

float max_error(const std::vector<float>& actual, const std::vector<float>& expected)
{
    float result = 0.0f;
    for (std::size_t i = 0; i < actual.size(); ++i) {
        const float difference = std::fabs(actual[i] - expected[i]);
        if (!std::isfinite(actual[i]) || !std::isfinite(expected[i])
            || !std::isfinite(difference)) {
            return std::numeric_limits<float>::infinity();
        }
        result = std::max(result, difference);
    }
    return result;
}

void require_error(const char* name, float error, float tolerance)
{
    if (!std::isfinite(error) || !(error <= tolerance)) {
        std::fprintf(stderr, "FAIL %-28s max_error=%g tolerance=%g\n", name, error, tolerance);
        std::exit(1);
    }
    std::printf("PASS %-28s max_error=%g\n", name, error);
}

struct MoeReference {
    std::vector<float> output;
    std::vector<int> top_experts;
    std::vector<float> top_weights;
    std::vector<int> expert_counts;
    std::vector<int> dispatch;
};

MoeReference moe_reference(
    const std::vector<std::uint8_t>& activations,
    const std::vector<std::uint8_t>& expert_weights,
    const std::vector<float>& router_weights)
{
    MoeReference result{
        std::vector<float>(kTokens * kOutput),
        std::vector<int>(kTokens * kTopK),
        std::vector<float>(kTokens * kTopK),
        std::vector<int>(kExperts),
        std::vector<int>(kExperts * kDispatchCapacity, -1),
    };

    for (int token = 0; token < kTokens; ++token) {
        std::array<float, kExperts> logits{};
        for (int expert = 0; expert < kExperts; ++expert) {
            for (int depth = 0; depth < kHidden; ++depth) {
                logits[expert] += decode_fp4(activations[token * kHidden + depth])
                    * router_weights[expert * kHidden + depth];
            }
        }
        int first = 0;
        for (int expert = 1; expert < kExperts; ++expert) {
            if (better_logit(logits[expert], expert, logits[first], first)) {
                first = expert;
            }
        }
        int second = first == 0 ? 1 : 0;
        for (int expert = 0; expert < kExperts; ++expert) {
            if (expert != first
                && better_logit(logits[expert], expert, logits[second], second)) {
                second = expert;
            }
        }
        const float maximum = std::max(logits[first], logits[second]);
        const float first_exp = std::exp(logits[first] - maximum);
        const float second_exp = std::exp(logits[second] - maximum);
        result.top_experts[token * 2] = first;
        result.top_experts[token * 2 + 1] = second;
        result.top_weights[token * 2] = first_exp / (first_exp + second_exp);
        result.top_weights[token * 2 + 1] = second_exp / (first_exp + second_exp);
        for (int choice = 0; choice < kTopK; ++choice) {
            const int expert = result.top_experts[token * 2 + choice];
            const int slot = result.expert_counts[expert]++;
            result.dispatch[expert * kDispatchCapacity + slot] = token * kTopK + choice;
        }
    }

    std::vector<float> dense(kAllExperts * kTokens * kOutput);
    for (int expert = 0; expert < kAllExperts; ++expert) {
        for (int token = 0; token < kTokens; ++token) {
            for (int column = 0; column < kOutput; ++column) {
                float value = 0.0f;
                for (int depth = 0; depth < kHidden; ++depth) {
                    value += decode_fp4(activations[token * kHidden + depth])
                        * decode_fp8(
                            expert_weights[(expert * kHidden + depth) * kOutput + column]);
                }
                dense[(expert * kTokens + token) * kOutput + column] = value;
            }
        }
    }
    for (int token = 0; token < kTokens; ++token) {
        for (int column = 0; column < kOutput; ++column) {
            float value = 0.0f;
            for (int choice = 0; choice < kTopK; ++choice) {
                const int expert = result.top_experts[token * 2 + choice];
                value += result.top_weights[token * 2 + choice]
                    * silu(dense[(expert * kTokens + token) * kOutput + column]);
            }
            value += 0.25f
                * silu(dense[(kSharedExpert * kTokens + token) * kOutput + column]);
            result.output[token * kOutput + column] = value;
        }
    }
    return result;
}

std::vector<float> expert_rank_reference(
    const std::vector<std::uint8_t>& activations,
    const std::vector<std::uint8_t>& expert_weights,
    const std::vector<int>& top_experts,
    const std::vector<float>& top_weights,
    int first_expert)
{
    std::vector<float> result(kTokens * kOutput);
    for (int token = 0; token < kTokens; ++token) {
        for (int column = 0; column < kOutput; ++column) {
            for (int choice = 0; choice < kTopK; ++choice) {
                const int expert = top_experts[token * kTopK + choice];
                if (expert < first_expert || expert >= first_expert + 2) {
                    continue;
                }
                float value = 0.0f;
                for (int depth = 0; depth < kHidden; ++depth) {
                    value += decode_fp4(activations[token * kHidden + depth])
                        * decode_fp8(
                            expert_weights[(expert * kHidden + depth) * kOutput + column]);
                }
                result[token * kOutput + column] +=
                    top_weights[token * kTopK + choice] * silu(value);
            }
        }
    }
    return result;
}

struct SpeculativeReference {
    std::vector<int> accepted;
    std::vector<int> committed;
    std::vector<float> state;
};

SpeculativeReference speculative_reference(
    const std::vector<int>& draft,
    const std::vector<int>& target,
    const std::vector<float>& scores,
    const std::vector<float>& thresholds,
    const std::vector<float>& base,
    const std::vector<float>& deltas)
{
    SpeculativeReference result{
        std::vector<int>(kCandidates),
        std::vector<int>(kCandidates),
        std::vector<float>(kCandidates * kStateWidth),
    };
    for (int candidate = 0; candidate < kCandidates; ++candidate) {
        int accepted = 0;
        while (accepted < kDraftSteps
            && draft[candidate * kDraftSteps + accepted] == target[accepted]
            && scores[candidate * kDraftSteps + accepted] >= thresholds[accepted]) {
            ++accepted;
        }
        result.accepted[candidate] = accepted;
        result.committed[candidate] = accepted == kDraftSteps ? 1 : 0;
        for (int element = 0; element < kStateWidth; ++element) {
            float value = base[element];
            if (result.committed[candidate]) {
                for (int step = 0; step < kDraftSteps; ++step) {
                    value += deltas[(candidate * kDraftSteps + step) * kStateWidth + element];
                }
            }
            result.state[candidate * kStateWidth + element] = value;
        }
    }
    return result;
}

std::vector<int> ngram_reference(
    const std::vector<int>& queries,
    const std::vector<std::uint64_t>& table_hashes,
    const std::vector<int>& table_grams,
    const std::vector<int>& table_values,
    const std::vector<int>& priorities)
{
    std::vector<int> result(kQueries, -1);
    for (int query = 0; query < kQueries; ++query) {
        const std::int32_t* gram = queries.data() + query * kNgram;
        const std::uint64_t hash = ngram_hash(gram);
        int best_slot = -1;
        for (int probe = 0; probe < kTableSize; ++probe) {
            const int slot = static_cast<int>((hash + probe) & (kTableSize - 1));
            bool equal = table_hashes[slot] == hash;
            for (int i = 0; i < kNgram; ++i) {
                equal = equal && table_grams[slot * kNgram + i] == gram[i];
            }
            if (equal
                && (best_slot < 0 || priorities[slot] > priorities[best_slot]
                    || (priorities[slot] == priorities[best_slot] && slot < best_slot))) {
                best_slot = slot;
            }
        }
        if (best_slot >= 0) {
            result[query] = table_values[best_slot];
        }
    }
    return result;
}

struct MuonReference {
    std::vector<float> update;
    float norm;
};

MuonReference muon_reference(const std::vector<float>& shards)
{
    std::array<float, kMuonElements> matrix{};
    for (int element = 0; element < kMuonElements; ++element) {
        for (int shard = 0; shard < kGradientShards; ++shard) {
            matrix[element] += shards[shard * kMuonElements + element];
        }
    }
    float squared_norm = 0.0f;
    for (float value : matrix) {
        squared_norm += value * value;
    }
    const float norm = std::sqrt(squared_norm);
    for (float& value : matrix) {
        value /= norm + 1.0e-6f;
    }
    for (int iteration = 0; iteration < kMuonIterations; ++iteration) {
        std::array<float, kMuonElements> gram{};
        std::array<float, kMuonElements> cubic{};
        for (int row = 0; row < kMuonDim; ++row) {
            for (int column = 0; column < kMuonDim; ++column) {
                for (int inner = 0; inner < kMuonDim; ++inner) {
                    gram[row * kMuonDim + column] +=
                        matrix[row * kMuonDim + inner] * matrix[column * kMuonDim + inner];
                }
            }
        }
        for (int row = 0; row < kMuonDim; ++row) {
            for (int column = 0; column < kMuonDim; ++column) {
                for (int inner = 0; inner < kMuonDim; ++inner) {
                    cubic[row * kMuonDim + column] +=
                        gram[row * kMuonDim + inner] * matrix[inner * kMuonDim + column];
                }
            }
        }
        for (int element = 0; element < kMuonElements; ++element) {
            matrix[element] = 1.5f * matrix[element] - 0.5f * cubic[element];
        }
    }
    MuonReference result{std::vector<float>(kMuonElements), norm};
    for (int element = 0; element < kMuonElements; ++element) {
        result.update[element] = -kMuonLearningRate * matrix[element];
    }
    return result;
}

bool is_gfx950_device(int device)
{
    hipDeviceProp_t properties{};
    HIP_CHECK(hipGetDeviceProperties(&properties, device));
    std::string architecture(properties.gcnArchName);
    const std::size_t feature_suffix = architecture.find(':');
    if (feature_suffix != std::string::npos) {
        architecture.resize(feature_suffix);
    }
    return architecture == "gfx950";
}

} // namespace

extern "C" __global__ __launch_bounds__(kMoeThreads) void gfx950_fused_fp4_fp8_moe(
    const std::uint8_t* activations,
    const std::uint8_t* expert_weights,
    const float* router_weights,
    float* output,
    int* top_experts,
    float* top_weights,
    int* expert_counts,
    int* dispatch)
{
    __shared__ float expert_output[kAllExperts * kTokens * kOutput];
    const int thread = static_cast<int>(threadIdx.x);
    if (thread == 0) {
        for (int expert = 0; expert < kExperts; ++expert) {
            expert_counts[expert] = 0;
            for (int slot = 0; slot < kDispatchCapacity; ++slot) {
                dispatch[expert * kDispatchCapacity + slot] = -1;
            }
        }
        for (int token = 0; token < kTokens; ++token) {
            float logits[kExperts] = {};
            for (int expert = 0; expert < kExperts; ++expert) {
                for (int depth = 0; depth < kHidden; ++depth) {
                    logits[expert] += decode_fp4(activations[token * kHidden + depth])
                        * router_weights[expert * kHidden + depth];
                }
            }
            int first = 0;
            for (int expert = 1; expert < kExperts; ++expert) {
                if (logits[expert] > logits[first]
                    || (logits[expert] == logits[first] && expert < first)) {
                    first = expert;
                }
            }
            int second = first == 0 ? 1 : 0;
            for (int expert = 0; expert < kExperts; ++expert) {
                if (expert != first
                    && (logits[expert] > logits[second]
                        || (logits[expert] == logits[second] && expert < second))) {
                    second = expert;
                }
            }
            const float maximum = fmaxf(logits[first], logits[second]);
            const float first_exp = expf(logits[first] - maximum);
            const float second_exp = expf(logits[second] - maximum);
            top_experts[token * 2] = first;
            top_experts[token * 2 + 1] = second;
            top_weights[token * 2] = first_exp / (first_exp + second_exp);
            top_weights[token * 2 + 1] = second_exp / (first_exp + second_exp);
            for (int choice = 0; choice < kTopK; ++choice) {
                const int expert = top_experts[token * 2 + choice];
                const int slot = expert_counts[expert]++;
                dispatch[expert * kDispatchCapacity + slot] = token * kTopK + choice;
            }
        }
    }
    __syncthreads();

    const int wave = thread / 64;
    const int lane = thread & 63;
    const int column = lane & 15;
    const f32x4 accumulator = mixed_fp4_fp8_mfma(
        pack_fp4_rows(activations, column),
        pack_fp8_columns(expert_weights, wave, column));
#pragma unroll
    for (int component = 0; component < 4; ++component) {
        const int token = (lane / 16) * 4 + component;
        expert_output[(wave * kTokens + token) * kOutput + column] = accumulator[component];
    }
    __syncthreads();

    if (thread < kTokens * kOutput) {
        const int token = thread / kOutput;
        const int output_column = thread % kOutput;
        float value = 0.0f;
        for (int choice = 0; choice < kTopK; ++choice) {
            const int expert = top_experts[token * kTopK + choice];
            value += top_weights[token * kTopK + choice]
                * silu(expert_output[(expert * kTokens + token) * kOutput + output_column]);
        }
        value += 0.25f
            * silu(expert_output[(kSharedExpert * kTokens + token) * kOutput + output_column]);
        output[token * kOutput + output_column] = value;
    }
}

extern "C" __global__ void gfx950_expert_parallel_rank(
    const std::uint8_t* activations,
    const std::uint8_t* expert_weights,
    const int* top_experts,
    const float* top_weights,
    int first_expert,
    float* partial_output)
{
    const int element = static_cast<int>(blockIdx.x * blockDim.x + threadIdx.x);
    if (element >= kTokens * kOutput) {
        return;
    }
    const int token = element / kOutput;
    const int column = element % kOutput;
    float result = 0.0f;
    for (int choice = 0; choice < kTopK; ++choice) {
        const int expert = top_experts[token * kTopK + choice];
        if (expert < first_expert || expert >= first_expert + 2) {
            continue;
        }
        float value = 0.0f;
        for (int depth = 0; depth < kHidden; ++depth) {
            value += decode_fp4(activations[token * kHidden + depth])
                * decode_fp8(expert_weights[(expert * kHidden + depth) * kOutput + column]);
        }
        result += top_weights[token * kTopK + choice] * silu(value);
    }
    partial_output[element] = result;
}

extern "C" __global__ void gfx950_combine_expert_ranks(
    const float* rank0,
    const float* rank1,
    float* output)
{
    const int element = static_cast<int>(blockIdx.x * blockDim.x + threadIdx.x);
    if (element < kTokens * kOutput) {
        output[element] = rank0[element] + rank1[element];
    }
}

extern "C" __global__ void gfx950_speculative_transaction(
    const int* draft_tokens,
    const int* target_tokens,
    const float* draft_scores,
    const float* thresholds,
    const float* base_state,
    const float* proposed_deltas,
    int* accepted_steps,
    int* committed,
    float* output_state)
{
    const int candidate = static_cast<int>(blockIdx.x * blockDim.x + threadIdx.x);
    if (candidate >= kCandidates) {
        return;
    }
    int accepted = 0;
    while (accepted < kDraftSteps
        && draft_tokens[candidate * kDraftSteps + accepted] == target_tokens[accepted]
        && draft_scores[candidate * kDraftSteps + accepted] >= thresholds[accepted]) {
        ++accepted;
    }
    accepted_steps[candidate] = accepted;
    committed[candidate] = accepted == kDraftSteps ? 1 : 0;
    for (int element = 0; element < kStateWidth; ++element) {
        float value = base_state[element];
        if (committed[candidate]) {
            for (int step = 0; step < kDraftSteps; ++step) {
                value += proposed_deltas[
                    (candidate * kDraftSteps + step) * kStateWidth + element];
            }
        }
        output_state[candidate * kStateWidth + element] = value;
    }
}

extern "C" __global__ void gfx950_qwen_ngram_gather(
    const int* queries,
    const std::uint64_t* table_hashes,
    const int* table_grams,
    const int* table_values,
    const int* priorities,
    int* output)
{
    const int query = static_cast<int>(blockIdx.x * blockDim.x + threadIdx.x);
    if (query >= kQueries) {
        return;
    }
    const int* gram = queries + query * kNgram;
    std::uint64_t hash = 1469598103934665603ULL;
    for (int i = 0; i < kNgram; ++i) {
        hash ^= static_cast<std::uint32_t>(gram[i]);
        hash *= 1099511628211ULL;
    }
    int best_slot = -1;
    for (int probe = 0; probe < kTableSize; ++probe) {
        const int slot = static_cast<int>((hash + probe) & (kTableSize - 1));
        bool equal = table_hashes[slot] == hash;
        for (int i = 0; i < kNgram; ++i) {
            equal = equal && table_grams[slot * kNgram + i] == gram[i];
        }
        if (equal
            && (best_slot < 0 || priorities[slot] > priorities[best_slot]
                || (priorities[slot] == priorities[best_slot] && slot < best_slot))) {
            best_slot = slot;
        }
    }
    output[query] = best_slot < 0 ? -1 : table_values[best_slot];
}

extern "C" __global__ void gfx950_stage_gradient_shard(
    const float* input,
    float* output)
{
    const int element = static_cast<int>(threadIdx.x);
    if (element < kMuonElements) {
        output[element] = input[element];
    }
}

extern "C" __global__ void gfx950_muon_update(
    const float* shards,
    float* output,
    float* output_norm)
{
    if (threadIdx.x != 0 || blockIdx.x != 0) {
        return;
    }
    float matrix[kMuonElements] = {};
    for (int element = 0; element < kMuonElements; ++element) {
        for (int shard = 0; shard < kGradientShards; ++shard) {
            matrix[element] += shards[shard * kMuonElements + element];
        }
    }
    float squared_norm = 0.0f;
    for (int element = 0; element < kMuonElements; ++element) {
        squared_norm += matrix[element] * matrix[element];
    }
    const float norm = sqrtf(squared_norm);
    output_norm[0] = norm;
    for (int element = 0; element < kMuonElements; ++element) {
        matrix[element] /= norm + 1.0e-6f;
    }
    for (int iteration = 0; iteration < kMuonIterations; ++iteration) {
        float gram[kMuonElements] = {};
        float cubic[kMuonElements] = {};
        for (int row = 0; row < kMuonDim; ++row) {
            for (int column = 0; column < kMuonDim; ++column) {
                for (int inner = 0; inner < kMuonDim; ++inner) {
                    gram[row * kMuonDim + column] +=
                        matrix[row * kMuonDim + inner] * matrix[column * kMuonDim + inner];
                }
            }
        }
        for (int row = 0; row < kMuonDim; ++row) {
            for (int column = 0; column < kMuonDim; ++column) {
                for (int inner = 0; inner < kMuonDim; ++inner) {
                    cubic[row * kMuonDim + column] +=
                        gram[row * kMuonDim + inner] * matrix[inner * kMuonDim + column];
                }
            }
        }
        for (int element = 0; element < kMuonElements; ++element) {
            matrix[element] = 1.5f * matrix[element] - 0.5f * cubic[element];
        }
    }
    for (int element = 0; element < kMuonElements; ++element) {
        output[element] = -kMuonLearningRate * matrix[element];
    }
}

namespace {

std::vector<std::uint8_t> make_activations()
{
    constexpr float values[] = {-1.0f, -0.5f, 0.0f, 0.5f, 1.0f};
    std::vector<std::uint8_t> result(kTokens * kHidden);
    for (int token = 0; token < kTokens; ++token) {
        for (int depth = 0; depth < kHidden; ++depth) {
            result[token * kHidden + depth] =
                encode_fp4(values[(token * 3 + depth * 2 + 1) % 5]);
        }
    }
    return result;
}

std::vector<std::uint8_t> make_expert_weights()
{
    constexpr float values[] = {-0.5f, -0.25f, 0.0f, 0.25f, 0.5f};
    std::vector<std::uint8_t> result(kAllExperts * kHidden * kOutput);
    for (int expert = 0; expert < kAllExperts; ++expert) {
        for (int depth = 0; depth < kHidden; ++depth) {
            for (int column = 0; column < kOutput; ++column) {
                result[(expert * kHidden + depth) * kOutput + column] =
                    encode_fp8(values[(expert * 2 + depth * 3 + column * 4) % 5]);
            }
        }
    }
    return result;
}

std::vector<float> make_router_weights()
{
    std::vector<float> result(kExperts * kHidden);
    for (int expert = 0; expert < kExperts; ++expert) {
        for (int depth = 0; depth < kHidden; ++depth) {
            result[expert * kHidden + depth] =
                static_cast<float>(((expert + 1) * (depth % 7) + depth / 13) % 9 - 4) / 64.0f;
        }
    }
    return result;
}

MoeReference run_moe(
    const std::vector<std::uint8_t>& activations,
    const std::vector<std::uint8_t>& expert_weights,
    const std::vector<float>& router_weights)
{
    auto* d_activations = device_alloc<std::uint8_t>(activations.size());
    auto* d_expert_weights = device_alloc<std::uint8_t>(expert_weights.size());
    auto* d_router_weights = device_alloc<float>(router_weights.size());
    auto* d_output = device_alloc<float>(kTokens * kOutput);
    auto* d_top_experts = device_alloc<int>(kTokens * kTopK);
    auto* d_top_weights = device_alloc<float>(kTokens * kTopK);
    auto* d_expert_counts = device_alloc<int>(kExperts);
    auto* d_dispatch = device_alloc<int>(kExperts * kDispatchCapacity);
    copy_to_device(d_activations, activations);
    copy_to_device(d_expert_weights, expert_weights);
    copy_to_device(d_router_weights, router_weights);
    gfx950_fused_fp4_fp8_moe<<<1, kMoeThreads>>>(
        d_activations,
        d_expert_weights,
        d_router_weights,
        d_output,
        d_top_experts,
        d_top_weights,
        d_expert_counts,
        d_dispatch);
    HIP_CHECK(hipGetLastError());
    HIP_CHECK(hipDeviceSynchronize());
    MoeReference result{
        copy_from_device(d_output, kTokens * kOutput),
        copy_from_device(d_top_experts, kTokens * kTopK),
        copy_from_device(d_top_weights, kTokens * kTopK),
        copy_from_device(d_expert_counts, kExperts),
        copy_from_device(d_dispatch, kExperts * kDispatchCapacity),
    };
    HIP_CHECK(hipFree(d_activations));
    HIP_CHECK(hipFree(d_expert_weights));
    HIP_CHECK(hipFree(d_router_weights));
    HIP_CHECK(hipFree(d_output));
    HIP_CHECK(hipFree(d_top_experts));
    HIP_CHECK(hipFree(d_top_weights));
    HIP_CHECK(hipFree(d_expert_counts));
    HIP_CHECK(hipFree(d_dispatch));
    return result;
}

void verify_moe_metadata(const MoeReference& actual, const MoeReference& expected)
{
    if (actual.top_experts != expected.top_experts
        || actual.expert_counts != expected.expert_counts
        || actual.dispatch != expected.dispatch) {
        std::fprintf(stderr, "FAIL MoE stable routing/dispatch metadata mismatch\n");
        std::exit(1);
    }
    require_error("MoE top-k weights", max_error(actual.top_weights, expected.top_weights), 2.0e-6f);
}

void run_expert_parallel_test(
    const std::vector<std::uint8_t>& activations,
    const std::vector<std::uint8_t>& expert_weights,
    const MoeReference& routing,
    const std::vector<int>& gfx950_devices)
{
    const bool two_devices = gfx950_devices.size() >= 2;
    const int ranks = 2;
    std::vector<std::vector<float>> partials(ranks);
    bool used_peer_transport = false;
    std::uint8_t* rank0_activations = nullptr;
    int* rank0_routes = nullptr;
    float* rank0_route_weights = nullptr;
    float* rank0_partial = nullptr;
    std::vector<float> peer_combined;

    for (int rank = 0; rank < ranks; ++rank) {
        const int device = two_devices ? gfx950_devices[rank] : gfx950_devices[0];
        HIP_CHECK(hipSetDevice(device));
        auto* d_activations = device_alloc<std::uint8_t>(activations.size());
        auto* d_weights = device_alloc<std::uint8_t>(expert_weights.size());
        auto* d_routes = device_alloc<int>(routing.top_experts.size());
        auto* d_route_weights = device_alloc<float>(routing.top_weights.size());
        auto* d_output = device_alloc<float>(kTokens * kOutput);
        copy_to_device(d_weights, expert_weights);

        if (rank == 0) {
            copy_to_device(d_activations, activations);
            copy_to_device(d_routes, routing.top_experts);
            copy_to_device(d_route_weights, routing.top_weights);
            rank0_activations = d_activations;
            rank0_routes = d_routes;
            rank0_route_weights = d_route_weights;
        } else if (two_devices) {
            int can_send = 0;
            int can_return = 0;
            HIP_CHECK(hipDeviceCanAccessPeer(&can_send, device, gfx950_devices[0]));
            HIP_CHECK(hipDeviceCanAccessPeer(&can_return, gfx950_devices[0], device));
            if (can_send != 0 && can_return != 0) {
                const hipError_t enable = hipDeviceEnablePeerAccess(gfx950_devices[0], 0);
                if (enable != hipSuccess && enable != hipErrorPeerAccessAlreadyEnabled) {
                    HIP_CHECK(enable);
                }
                HIP_CHECK(hipMemcpyPeer(
                    d_activations,
                    device,
                    rank0_activations,
                    gfx950_devices[0],
                    activations.size()));
                HIP_CHECK(hipMemcpyPeer(
                    d_routes,
                    device,
                    rank0_routes,
                    gfx950_devices[0],
                    routing.top_experts.size() * sizeof(int)));
                HIP_CHECK(hipMemcpyPeer(
                    d_route_weights,
                    device,
                    rank0_route_weights,
                    gfx950_devices[0],
                    routing.top_weights.size() * sizeof(float)));
                used_peer_transport = true;
            } else {
                copy_to_device(d_activations, activations);
                copy_to_device(d_routes, routing.top_experts);
                copy_to_device(d_route_weights, routing.top_weights);
            }
        } else {
            copy_to_device(d_activations, activations);
            copy_to_device(d_routes, routing.top_experts);
            copy_to_device(d_route_weights, routing.top_weights);
        }

        gfx950_expert_parallel_rank<<<1, 256>>>(
            d_activations,
            d_weights,
            d_routes,
            d_route_weights,
            rank * 2,
            d_output);
        HIP_CHECK(hipGetLastError());
        HIP_CHECK(hipDeviceSynchronize());
        partials[rank] = copy_from_device(d_output, kTokens * kOutput);

        if (rank == 0) {
            rank0_partial = d_output;
        } else if (used_peer_transport) {
            HIP_CHECK(hipSetDevice(gfx950_devices[0]));
            const hipError_t enable = hipDeviceEnablePeerAccess(device, 0);
            if (enable != hipSuccess && enable != hipErrorPeerAccessAlreadyEnabled) {
                HIP_CHECK(enable);
            }
            auto* d_returned = device_alloc<float>(kTokens * kOutput);
            auto* d_combined = device_alloc<float>(kTokens * kOutput);
            HIP_CHECK(hipMemcpyPeer(
                d_returned,
                gfx950_devices[0],
                d_output,
                device,
                kTokens * kOutput * sizeof(float)));
            gfx950_combine_expert_ranks<<<1, 256>>>(
                rank0_partial, d_returned, d_combined);
            HIP_CHECK(hipGetLastError());
            peer_combined = copy_from_device(d_combined, kTokens * kOutput);
            HIP_CHECK(hipFree(d_returned));
            HIP_CHECK(hipFree(d_combined));
            HIP_CHECK(hipSetDevice(device));
        }

        if (rank != 0) {
            HIP_CHECK(hipFree(d_activations));
            HIP_CHECK(hipFree(d_routes));
            HIP_CHECK(hipFree(d_route_weights));
        }
        HIP_CHECK(hipFree(d_weights));
        if (rank != 0) {
            HIP_CHECK(hipFree(d_output));
        }
    }
    HIP_CHECK(hipSetDevice(gfx950_devices[0]));
    HIP_CHECK(hipFree(rank0_activations));
    HIP_CHECK(hipFree(rank0_routes));
    HIP_CHECK(hipFree(rank0_route_weights));
    HIP_CHECK(hipFree(rank0_partial));

    std::vector<float> combined = used_peer_transport
        ? peer_combined
        : std::vector<float>(kTokens * kOutput);
    std::vector<float> expected(kTokens * kOutput);
    for (int rank = 0; rank < ranks; ++rank) {
        const auto rank_expected = expert_rank_reference(
            activations,
            expert_weights,
            routing.top_experts,
            routing.top_weights,
            rank * 2);
        require_error(
            rank == 0 ? "expert logical shard 0" : "expert logical shard 1",
            max_error(partials[rank], rank_expected),
            3.0e-3f);
        for (std::size_t i = 0; i < combined.size(); ++i) {
            if (!used_peer_transport) {
                combined[i] += partials[rank][i];
            }
            expected[i] += rank_expected[i];
        }
    }
    require_error("expert-parallel routed combine", max_error(combined, expected), 3.0e-3f);
    if (two_devices && used_peer_transport) {
        std::printf("PASS expert transport             mode=two-device-peer ranks=2\n");
    } else if (two_devices) {
        std::printf("PASS expert transport             mode=two-device-host-staged ranks=2\n");
    } else {
        std::printf(
            "PASS expert transport             mode=one-device-host-staged-simulation ranks=2\n");
    }
}

void run_speculative_test()
{
    std::vector<int> target = {11, 12, 13, 14};
    std::vector<int> draft(kCandidates * kDraftSteps);
    std::vector<float> scores(kCandidates * kDraftSteps);
    std::vector<float> thresholds = {0.25f, 0.35f, 0.45f, 0.55f};
    std::vector<float> base(kStateWidth);
    std::vector<float> deltas(kCandidates * kDraftSteps * kStateWidth);
    for (int candidate = 0; candidate < kCandidates; ++candidate) {
        for (int step = 0; step < kDraftSteps; ++step) {
            draft[candidate * kDraftSteps + step] =
                (step == candidate % (kDraftSteps + 1)) ? 90 + candidate : target[step];
            scores[candidate * kDraftSteps + step] =
                0.2f + 0.11f * static_cast<float>((candidate + step) % 6);
            for (int element = 0; element < kStateWidth; ++element) {
                deltas[(candidate * kDraftSteps + step) * kStateWidth + element] =
                    0.001f * static_cast<float>(1 + candidate + step * 2 + element);
            }
        }
    }
    // Two candidates deterministically commit; the rest exercise token or score rollback.
    for (int candidate : {4, 7}) {
        for (int step = 0; step < kDraftSteps; ++step) {
            draft[candidate * kDraftSteps + step] = target[step];
            scores[candidate * kDraftSteps + step] = 0.9f;
        }
    }
    for (int element = 0; element < kStateWidth; ++element) {
        base[element] = 0.125f * static_cast<float>(element - 3);
    }
    const auto expected = speculative_reference(draft, target, scores, thresholds, base, deltas);

    auto* d_draft = device_alloc<int>(draft.size());
    auto* d_target = device_alloc<int>(target.size());
    auto* d_scores = device_alloc<float>(scores.size());
    auto* d_thresholds = device_alloc<float>(thresholds.size());
    auto* d_base = device_alloc<float>(base.size());
    auto* d_deltas = device_alloc<float>(deltas.size());
    auto* d_accepted = device_alloc<int>(kCandidates);
    auto* d_committed = device_alloc<int>(kCandidates);
    auto* d_state = device_alloc<float>(kCandidates * kStateWidth);
    copy_to_device(d_draft, draft);
    copy_to_device(d_target, target);
    copy_to_device(d_scores, scores);
    copy_to_device(d_thresholds, thresholds);
    copy_to_device(d_base, base);
    copy_to_device(d_deltas, deltas);
    gfx950_speculative_transaction<<<1, 32>>>(
        d_draft,
        d_target,
        d_scores,
        d_thresholds,
        d_base,
        d_deltas,
        d_accepted,
        d_committed,
        d_state);
    HIP_CHECK(hipGetLastError());
    const auto accepted = copy_from_device(d_accepted, kCandidates);
    const auto committed = copy_from_device(d_committed, kCandidates);
    const auto state = copy_from_device(d_state, kCandidates * kStateWidth);
    if (accepted != expected.accepted || committed != expected.committed) {
        std::fprintf(stderr, "FAIL speculative accept/commit decisions mismatch\n");
        std::exit(1);
    }
    require_error("speculative transactional state", max_error(state, expected.state), 1.0e-7f);
    for (int candidate = 0; candidate < kCandidates; ++candidate) {
        if (committed[candidate] != 0) {
            continue;
        }
        for (int element = 0; element < kStateWidth; ++element) {
            if (std::memcmp(
                    &state[candidate * kStateWidth + element],
                    &base[element],
                    sizeof(float)) != 0) {
                std::fprintf(stderr, "FAIL speculative rollback is not bitwise exact\n");
                std::exit(1);
            }
        }
    }
    std::printf(
        "PASS speculative decisions         committed=%d rolled_back=%d\n",
        std::accumulate(committed.begin(), committed.end(), 0),
        kCandidates - std::accumulate(committed.begin(), committed.end(), 0));
    HIP_CHECK(hipFree(d_draft));
    HIP_CHECK(hipFree(d_target));
    HIP_CHECK(hipFree(d_scores));
    HIP_CHECK(hipFree(d_thresholds));
    HIP_CHECK(hipFree(d_base));
    HIP_CHECK(hipFree(d_deltas));
    HIP_CHECK(hipFree(d_accepted));
    HIP_CHECK(hipFree(d_committed));
    HIP_CHECK(hipFree(d_state));
}

void run_ngram_test()
{
    std::vector<int> queries(kQueries * kNgram);
    for (int query = 0; query < kQueries; ++query) {
        queries[query * kNgram] = 100 + query;
        queries[query * kNgram + 1] = 200 + (query % 3);
        queries[query * kNgram + 2] = 300 + query * 2;
    }
    std::vector<std::uint64_t> table_hashes(kTableSize, 0);
    std::vector<int> table_grams(kTableSize * kNgram, -1);
    std::vector<int> table_values(kTableSize, -1);
    std::vector<int> priorities(kTableSize, -1);
    for (int query = 0; query < 6; ++query) {
        const auto hash = ngram_hash(queries.data() + query * kNgram);
        const int slot = static_cast<int>((hash + 3) & (kTableSize - 1));
        table_hashes[slot] = hash;
        for (int i = 0; i < kNgram; ++i) {
            table_grams[slot * kNgram + i] = queries[query * kNgram + i];
        }
        table_values[slot] = 1000 + query;
        priorities[slot] = query % 2;
    }
    // Duplicate query zero with equal priority: the lower physical slot wins.
    const auto duplicate_hash = ngram_hash(queries.data());
    const int duplicate_slot = 1;
    table_hashes[duplicate_slot] = duplicate_hash;
    for (int i = 0; i < kNgram; ++i) {
        table_grams[duplicate_slot * kNgram + i] = queries[i];
    }
    table_values[duplicate_slot] = 4242;
    priorities[duplicate_slot] = 0;
    // Deliberate hash collision with different gram: exact key comparison rejects it.
    const int collision_slot = 2;
    table_hashes[collision_slot] = duplicate_hash;
    table_grams[collision_slot * kNgram] = 999;
    table_grams[collision_slot * kNgram + 1] = 998;
    table_grams[collision_slot * kNgram + 2] = 997;
    table_values[collision_slot] = 7777;
    priorities[collision_slot] = 99;
    const auto expected = ngram_reference(
        queries, table_hashes, table_grams, table_values, priorities);

    auto* d_queries = device_alloc<int>(queries.size());
    auto* d_hashes = device_alloc<std::uint64_t>(table_hashes.size());
    auto* d_grams = device_alloc<int>(table_grams.size());
    auto* d_values = device_alloc<int>(table_values.size());
    auto* d_priorities = device_alloc<int>(priorities.size());
    auto* d_output = device_alloc<int>(kQueries);
    copy_to_device(d_queries, queries);
    copy_to_device(d_hashes, table_hashes);
    copy_to_device(d_grams, table_grams);
    copy_to_device(d_values, table_values);
    copy_to_device(d_priorities, priorities);
    gfx950_qwen_ngram_gather<<<1, 32>>>(
        d_queries, d_hashes, d_grams, d_values, d_priorities, d_output);
    HIP_CHECK(hipGetLastError());
    const auto actual = copy_from_device(d_output, kQueries);
    if (actual != expected) {
        std::fprintf(stderr, "FAIL Qwen-style N-gram gather mismatch\n");
        std::exit(1);
    }
    std::printf(
        "PASS Qwen-style N-gram gather    hits=%d misses=%d tie_value=%d\n",
        static_cast<int>(std::count_if(actual.begin(), actual.end(), [](int value) {
            return value >= 0;
        })),
        static_cast<int>(std::count(actual.begin(), actual.end(), -1)),
        actual[0]);
    HIP_CHECK(hipFree(d_queries));
    HIP_CHECK(hipFree(d_hashes));
    HIP_CHECK(hipFree(d_grams));
    HIP_CHECK(hipFree(d_values));
    HIP_CHECK(hipFree(d_priorities));
    HIP_CHECK(hipFree(d_output));
}

void run_muon_test(const std::vector<int>& gfx950_devices)
{
    std::vector<float> source_shards(kGradientShards * kMuonElements);
    for (int shard = 0; shard < kGradientShards; ++shard) {
        for (int element = 0; element < kMuonElements; ++element) {
            source_shards[shard * kMuonElements + element] =
                0.025f * static_cast<float>((shard + 1) * ((element * 5 + shard * 3) % 11 - 5));
        }
    }
    std::vector<float> staged(source_shards.size());
    const bool two_devices = gfx950_devices.size() >= 2;
    for (int shard = 0; shard < kGradientShards; ++shard) {
        const int device = two_devices ? gfx950_devices[shard] : gfx950_devices[0];
        HIP_CHECK(hipSetDevice(device));
        std::vector<float> source(
            source_shards.begin() + shard * kMuonElements,
            source_shards.begin() + (shard + 1) * kMuonElements);
        auto* d_source = device_alloc<float>(kMuonElements);
        auto* d_staged = device_alloc<float>(kMuonElements);
        copy_to_device(d_source, source);
        gfx950_stage_gradient_shard<<<1, 32>>>(d_source, d_staged);
        HIP_CHECK(hipGetLastError());
        const auto returned = copy_from_device(d_staged, kMuonElements);
        require_error(
            shard == 0 ? "Muon staged shard 0" : "Muon staged shard 1",
            max_error(returned, source),
            0.0f);
        std::copy(returned.begin(), returned.end(), staged.begin() + shard * kMuonElements);
        HIP_CHECK(hipFree(d_source));
        HIP_CHECK(hipFree(d_staged));
    }
    HIP_CHECK(hipSetDevice(gfx950_devices[0]));
    require_error("Muon staged rank order", max_error(staged, source_shards), 0.0f);
    const auto expected = muon_reference(source_shards);
    auto* d_shards = device_alloc<float>(staged.size());
    auto* d_update = device_alloc<float>(kMuonElements);
    auto* d_norm = device_alloc<float>(1);
    copy_to_device(d_shards, staged);
    gfx950_muon_update<<<1, 64>>>(d_shards, d_update, d_norm);
    HIP_CHECK(hipGetLastError());
    const auto update = copy_from_device(d_update, kMuonElements);
    const auto norm = copy_from_device(d_norm, 1);
    require_error("Muon polar update", max_error(update, expected.update), 2.0e-6f);
    require_error("Muon reduced norm", std::fabs(norm[0] - expected.norm), 2.0e-6f);
    std::printf(
        "PASS Muon shard reduction        mode=%s fixed_rank_order=0,1 iterations=%d norm=%g\n",
        two_devices ? "two-device-host-staged" : "one-device-host-staged-simulation",
        kMuonIterations,
        norm[0]);
    HIP_CHECK(hipFree(d_shards));
    HIP_CHECK(hipFree(d_update));
    HIP_CHECK(hipFree(d_norm));
}

} // namespace

int main()
{
    int device_count = 0;
    HIP_CHECK(hipGetDeviceCount(&device_count));
    std::vector<int> gfx950_devices;
    for (int device = 0; device < device_count; ++device) {
        if (is_gfx950_device(device)) {
            gfx950_devices.push_back(device);
        }
    }
    if (gfx950_devices.empty()) {
        std::fprintf(stderr, "This executable requires a visible gfx950 device.\n");
        return 2;
    }
    HIP_CHECK(hipSetDevice(gfx950_devices[0]));
    hipDeviceProp_t properties{};
    HIP_CHECK(hipGetDeviceProperties(&properties, gfx950_devices[0]));
    std::printf(
        "DEVICE primary=%d name=%s arch=%s visible_gfx950=%zu\n",
        gfx950_devices[0],
        properties.name,
        properties.gcnArchName,
        gfx950_devices.size());

    const auto activations = make_activations();
    const auto expert_weights = make_expert_weights();
    const auto router_weights = make_router_weights();
    const auto expected_moe = moe_reference(activations, expert_weights, router_weights);
    const auto actual_moe = run_moe(activations, expert_weights, router_weights);
    verify_moe_metadata(actual_moe, expected_moe);
    require_error(
        "fused FP4/FP8 MoE",
        max_error(actual_moe.output, expected_moe.output),
        4.0e-3f);
    std::printf(
        "PASS MoE stable dispatch         counts=%d,%d,%d,%d shared_expert=yes\n",
        actual_moe.expert_counts[0],
        actual_moe.expert_counts[1],
        actual_moe.expert_counts[2],
        actual_moe.expert_counts[3]);
    run_expert_parallel_test(activations, expert_weights, expected_moe, gfx950_devices);
    HIP_CHECK(hipSetDevice(gfx950_devices[0]));
    run_speculative_test();
    run_ngram_test();
    run_muon_test(gfx950_devices);
    return 0;
}

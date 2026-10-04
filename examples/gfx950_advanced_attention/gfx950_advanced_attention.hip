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

constexpr int kChannels = 16;
constexpr int kPrefillTokens = 8;
constexpr int kAttentionTokens = 16;
constexpr int kHeadDimension = 128;
constexpr int kWaveSize = 64;

#define HIP_CHECK(call)                                                                  \
    do {                                                                                 \
        const hipError_t status = (call);                                                \
        if (status != hipSuccess) {                                                      \
            std::fprintf(stderr, "%s failed: %s\n", #call, hipGetErrorString(status)); \
            std::exit(1);                                                                \
        }                                                                                \
    } while (false)

__host__ __device__ float sigmoid(float value)
{
    return 1.0f / (1.0f + expf(-value));
}

__host__ __device__ float decode_fp8(std::uint8_t value)
{
    const int exponent = (value >> 3) & 15;
    const int mantissa = value & 7;
    float magnitude = exponent == 0
        ? ldexpf(static_cast<float>(mantissa), -9)
        : ldexpf(1.0f + static_cast<float>(mantissa) / 8.0f, exponent - 7);
    return (value & 0x80) != 0 ? -magnitude : magnitude;
}

std::uint8_t encode_fp8(float value)
{
    if (value == 0.0f) {
        return 0;
    }
    const bool negative = std::signbit(value);
    int exponent = 0;
    const float fraction = std::frexp(std::fabs(value), &exponent);
    int encoded_exponent = exponent - 1 + 7;
    int mantissa = static_cast<int>(std::nearbyint((fraction * 2.0f - 1.0f) * 8.0f));
    if (mantissa == 8) {
        mantissa = 0;
        ++encoded_exponent;
    }
    encoded_exponent = std::max(0, std::min(15, encoded_exponent));
    return static_cast<std::uint8_t>((negative ? 0x80 : 0) | (encoded_exponent << 3) | mantissa);
}

__device__ float subgroup_sum_16(float value)
{
#pragma unroll
    for (int offset = 1; offset < 16; offset *= 2) {
        value += __shfl_xor(value, offset, 16);
    }
    return value;
}

__device__ float subgroup_max_16(float value)
{
#pragma unroll
    for (int offset = 1; offset < 16; offset *= 2) {
        value = fmaxf(value, __shfl_xor(value, offset, 16));
    }
    return value;
}

__device__ i32x8 pack_fp8_row(const std::uint8_t* matrix, int row)
{
    const int lane = static_cast<int>(threadIdx.x) & 63;
    const int group = lane / 16;
    i32x8 packed = {};
#pragma unroll
    for (int i = 0; i < 32; ++i) {
        const int depth = i < 16 ? group * 16 + i : 64 + group * 16 + (i - 16);
        const std::uint32_t byte = matrix[row * kHeadDimension + depth];
        packed[i / 4] |= static_cast<int>(byte << ((i % 4) * 8));
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
        const std::uint32_t byte = matrix[(token_base + token_offset) * kHeadDimension + depth];
        packed[token_offset / 4] |= static_cast<int>(byte << ((token_offset % 4) * 8));
    }
    return packed;
}

__device__ f32x4 fp8_mfma(i32x8 a, i32x8 b)
{
    const f32x4 zero = {};
    return __builtin_amdgcn_mfma_scale_f32_16x16x128_f8f6f4(
        a, b, zero, 0, 0, 0, 0, 0, 0);
}

template <typename T>
class DeviceBuffer {
public:
    explicit DeviceBuffer(std::size_t count) : count_(count)
    {
        HIP_CHECK(hipMalloc(&data_, count * sizeof(T)));
    }

    ~DeviceBuffer()
    {
        if (data_ != nullptr) {
            (void)hipFree(data_);
        }
    }

    DeviceBuffer(const DeviceBuffer&) = delete;
    DeviceBuffer& operator=(const DeviceBuffer&) = delete;

    T* get() { return data_; }

    void copy_from(const std::vector<T>& source)
    {
        if (source.size() != count_) {
            std::fprintf(stderr, "device buffer size mismatch\n");
            std::exit(1);
        }
        HIP_CHECK(hipMemcpy(data_, source.data(), count_ * sizeof(T), hipMemcpyHostToDevice));
    }

    std::vector<T> copy_to_host() const
    {
        std::vector<T> result(count_);
        HIP_CHECK(hipMemcpy(result.data(), data_, count_ * sizeof(T), hipMemcpyDeviceToHost));
        return result;
    }

private:
    T* data_ = nullptr;
    std::size_t count_;
};

void check_result(
    const char* name,
    const std::vector<float>& actual,
    const std::vector<float>& expected,
    float tolerance = 3.0e-3f)
{
    float maximum_error = 0.0f;
    std::size_t maximum_index = 0;
    for (std::size_t i = 0; i < actual.size(); ++i) {
        if (!std::isfinite(actual[i]) || !std::isfinite(expected[i])) {
            std::fprintf(
                stderr,
                "FAIL %-24s non-finite index=%zu gpu=%g cpu=%g\n",
                name,
                i,
                actual[i],
                expected[i]);
            std::exit(1);
        }
        const float error = std::fabs(actual[i] - expected[i]);
        if (!std::isfinite(error)) {
            std::fprintf(stderr, "FAIL %-24s non-finite error index=%zu\n", name, i);
            std::exit(1);
        }
        if (error > maximum_error) {
            maximum_error = error;
            maximum_index = i;
        }
    }
    if (maximum_error > tolerance) {
        std::fprintf(
            stderr,
            "FAIL %-24s max_error=%g index=%zu gpu=%g cpu=%g\n",
            name,
            maximum_error,
            maximum_index,
            actual[maximum_index],
            expected[maximum_index]);
        std::exit(1);
    }
    std::printf("PASS %-24s max_error=%g\n", name, maximum_error);
}

std::vector<float> deterministic_floats(std::size_t count, int salt, float scale = 1.0f)
{
    std::vector<float> result(count);
    for (std::size_t i = 0; i < count; ++i) {
        const int centered = static_cast<int>((i * 17 + salt * 11) % 19) - 9;
        result[i] = scale * static_cast<float>(centered) / 9.0f;
    }
    return result;
}

std::vector<std::uint8_t> deterministic_fp8(std::size_t count, int salt)
{
    constexpr float values[] = {-1.0f, -0.5f, 0.0f, 0.5f, 1.0f};
    std::vector<std::uint8_t> result(count);
    for (std::size_t i = 0; i < count; ++i) {
        result[i] = encode_fp8(values[(i * 3 + salt) % 5]);
    }
    return result;
}

void kda_step_reference(
    const float* history,
    const float* gate_input,
    const float* state,
    const float* convolution_weights,
    float* state_output,
    float* normalized_output)
{
    float square_sum = 0.0f;
    for (int channel = 0; channel < kChannels; ++channel) {
        float convolution = 0.0f;
        for (int tap = 0; tap < 3; ++tap) {
            convolution += history[tap * kChannels + channel] * convolution_weights[tap];
        }
        const float proposal = std::tanh(convolution + 0.25f * state[channel]);
        const float gate = sigmoid(gate_input[channel]);
        state_output[channel] = gate * state[channel] + (1.0f - gate) * proposal;
        square_sum += state_output[channel] * state_output[channel];
    }
    const float inverse_rms = 1.0f / std::sqrt(square_sum / kChannels + 1.0e-5f);
    for (int channel = 0; channel < kChannels; ++channel) {
        normalized_output[channel] = state_output[channel] * inverse_rms;
    }
}

std::vector<float> kda_prefill_reference(
    const std::vector<float>& input,
    const std::vector<float>& gates,
    const std::vector<float>& initial_state,
    const std::vector<float>& weights,
    std::vector<float>* final_state)
{
    std::vector<float> state = initial_state;
    std::vector<float> output(kPrefillTokens * kChannels);
    for (int chunk = 0; chunk < 2; ++chunk) {
        for (int offset = 0; offset < 4; ++offset) {
            const int token = chunk * 4 + offset;
            std::vector<float> history(3 * kChannels, 0.0f);
            for (int tap = 0; tap < 3; ++tap) {
                const int source_token = token - tap;
                if (source_token >= 0) {
                    std::copy_n(
                        input.data() + source_token * kChannels,
                        kChannels,
                        history.data() + tap * kChannels);
                }
            }
            std::vector<float> next_state(kChannels);
            kda_step_reference(
                history.data(),
                gates.data() + token * kChannels,
                state.data(),
                weights.data(),
                next_state.data(),
                output.data() + token * kChannels);
            state = next_state;
        }
    }
    *final_state = state;
    return output;
}

std::vector<int> sparse_selection_reference(const std::vector<float>& content_scores)
{
    std::vector<int> blocks = {0, 1, 2, 3};
    std::stable_sort(blocks.begin(), blocks.end(), [&](int lhs, int rhs) {
        float lhs_score = -INFINITY;
        float rhs_score = -INFINITY;
        for (int i = 0; i < 4; ++i) {
            lhs_score = std::max(lhs_score, content_scores[lhs * 4 + i]);
            rhs_score = std::max(rhs_score, content_scores[rhs * 4 + i]);
        }
        return lhs_score > rhs_score;
    });
    std::vector<int> candidates;
    for (int rank = 0; rank < 2; ++rank) {
        for (int i = 0; i < 4; ++i) {
            candidates.push_back(blocks[rank] * 4 + i);
        }
    }
    std::stable_sort(candidates.begin(), candidates.end(), [&](int lhs, int rhs) {
        return content_scores[lhs] > content_scores[rhs];
    });
    candidates.resize(3);
    return candidates;
}

std::vector<float> sparse_attention_reference(
    const std::vector<std::uint8_t>& q,
    const std::vector<std::uint8_t>& k,
    const std::vector<std::uint8_t>& v,
    const std::vector<float>& content_scores,
    const std::vector<int>& selected)
{
    std::vector<float> scores(kAttentionTokens, -INFINITY);
    float maximum = -INFINITY;
    for (int token : selected) {
        float score = content_scores[token] * 0.75f;
        for (int depth = 0; depth < kHeadDimension; ++depth) {
            score += decode_fp8(q[depth]) * decode_fp8(k[token * kHeadDimension + depth])
                / std::sqrt(static_cast<float>(kHeadDimension));
        }
        scores[token] = score;
        maximum = std::max(maximum, score);
    }
    float denominator = 0.0f;
    for (int token : selected) {
        denominator += std::exp(scores[token] - maximum);
    }
    std::vector<float> output(kChannels, 0.0f);
    for (int dimension = 0; dimension < kChannels; ++dimension) {
        for (int token : selected) {
            output[dimension] += std::exp(scores[token] - maximum) / denominator
                * decode_fp8(v[token * kChannels + dimension]);
        }
        output[dimension] *= sigmoid(maximum * 0.01f);
    }
    return output;
}

std::vector<float> hybrid_attention_reference(
    const std::vector<std::uint8_t>& q,
    const std::vector<std::uint8_t>& k,
    const std::vector<std::uint8_t>& v,
    const std::vector<float>& token_bias)
{
    std::vector<float> scores(kAttentionTokens);
    for (int token = 0; token < kAttentionTokens; ++token) {
        scores[token] = token_bias[token];
        for (int depth = 0; depth < kHeadDimension; ++depth) {
            scores[token] += decode_fp8(q[depth]) * decode_fp8(k[token * kHeadDimension + depth])
                / std::sqrt(static_cast<float>(kHeadDimension));
        }
    }
    float local_max = -INFINITY;
    float global_max = -INFINITY;
    for (int token = 12; token < 16; ++token) {
        local_max = std::max(local_max, scores[token]);
    }
    for (int block = 0; block < 3; ++block) {
        global_max = std::max(global_max, scores[block * 4]);
    }
    float local_sum = 0.0f;
    float global_sum = 0.0f;
    for (int token = 12; token < 16; ++token) {
        local_sum += std::exp(scores[token] - local_max);
    }
    for (int block = 0; block < 3; ++block) {
        global_sum += std::exp(scores[block * 4] - global_max);
    }
    const float mix = sigmoid(scores[0] * 0.01f);
    std::vector<float> output(kChannels, 0.0f);
    for (int dimension = 0; dimension < kChannels; ++dimension) {
        float local = 0.0f;
        float global = 0.0f;
        for (int token = 12; token < 16; ++token) {
            local += std::exp(scores[token] - local_max) / local_sum
                * decode_fp8(v[token * kChannels + dimension]);
        }
        for (int block = 0; block < 3; ++block) {
            float compressed_value = 0.0f;
            for (int within = 0; within < 4; ++within) {
                compressed_value += decode_fp8(v[(block * 4 + within) * kChannels + dimension]) * 0.25f;
            }
            global += std::exp(scores[block * 4] - global_max) / global_sum * compressed_value;
        }
        output[dimension] = mix * global + (1.0f - mix) * local;
    }
    return output;
}

} // namespace

extern "C" __global__ __launch_bounds__(64) void gfx950_kda_gdn_decode(
    const float* history,
    const float* gate_input,
    const float* state,
    const float* convolution_weights,
    float* state_output,
    float* normalized_output)
{
    const int channel = static_cast<int>(threadIdx.x);
    float updated = 0.0f;
    if (channel < kChannels) {
        float convolution = 0.0f;
#pragma unroll
        for (int tap = 0; tap < 3; ++tap) {
            convolution += history[tap * kChannels + channel] * convolution_weights[tap];
        }
        const float proposal = tanhf(convolution + 0.25f * state[channel]);
        const float gate = sigmoid(gate_input[channel]);
        updated = gate * state[channel] + (1.0f - gate) * proposal;
        state_output[channel] = updated;
    }
    const float square_sum = subgroup_sum_16(updated * updated);
    if (channel < kChannels) {
        normalized_output[channel] = updated * rsqrtf(square_sum / kChannels + 1.0e-5f);
    }
}

extern "C" __global__ __launch_bounds__(64) void gfx950_kda_gdn_prefill(
    const float* input,
    const float* gate_input,
    const float* initial_state,
    const float* convolution_weights,
    float* final_state,
    float* normalized_output)
{
    const int channel = static_cast<int>(threadIdx.x);
    float state = channel < kChannels ? initial_state[channel] : 0.0f;
#pragma unroll
    for (int chunk = 0; chunk < 2; ++chunk) {
#pragma unroll
        for (int offset = 0; offset < 4; ++offset) {
            const int token = chunk * 4 + offset;
            float convolution = 0.0f;
#pragma unroll
            for (int tap = 0; tap < 3; ++tap) {
                if (channel < kChannels && token >= tap) {
                    convolution += input[(token - tap) * kChannels + channel]
                        * convolution_weights[tap];
                }
            }
            const float proposal = tanhf(convolution + 0.25f * state);
            const float gate = channel < kChannels ? sigmoid(gate_input[token * kChannels + channel]) : 0.0f;
            state = channel < kChannels ? gate * state + (1.0f - gate) * proposal : 0.0f;
            const float square_sum = subgroup_sum_16(state * state);
            if (channel < kChannels) {
                normalized_output[token * kChannels + channel] =
                    state * rsqrtf(square_sum / kChannels + 1.0e-5f);
            }
        }
    }
    if (channel < kChannels) {
        final_state[channel] = state;
    }
}

extern "C" __global__ __launch_bounds__(64) void gfx950_content_sparse_attention(
    const std::uint8_t* q,
    const std::uint8_t* k,
    const std::uint8_t* v,
    const float* content_scores,
    float* output,
    int* selected_output)
{
    __shared__ i32x2 transposed_k[64 * 4];
    __shared__ int selected[3];
    const int lane = static_cast<int>(threadIdx.x) & 63;
    const int column = lane & 15;

    if (lane == 0) {
        int best_block = -1;
        int second_block = -1;
        float best_score = -INFINITY;
        float second_score = -INFINITY;
        for (int block = 0; block < 4; ++block) {
            float block_score = -INFINITY;
            for (int within = 0; within < 4; ++within) {
                block_score = fmaxf(block_score, content_scores[block * 4 + within]);
            }
            if (block_score > best_score) {
                second_score = best_score;
                second_block = best_block;
                best_score = block_score;
                best_block = block;
            } else if (block_score > second_score) {
                second_score = block_score;
                second_block = block;
            }
        }
        for (int rank = 0; rank < 3; ++rank) {
            int best_token = -1;
            float token_score = -INFINITY;
            for (int candidate = 0; candidate < 8; ++candidate) {
                const int block = candidate < 4 ? best_block : second_block;
                const int token = block * 4 + (candidate & 3);
                bool already_selected = false;
                for (int previous = 0; previous < rank; ++previous) {
                    already_selected |= selected[previous] == token;
                }
                if (!already_selected && content_scores[token] > token_score) {
                    token_score = content_scores[token];
                    best_token = token;
                }
            }
            selected[rank] = best_token;
            selected_output[rank] = best_token;
        }
    }

#pragma unroll
    for (int part = 0; part < 4; ++part) {
        transposed_k[lane * 4 + part] = stage_fp8_transpose_source(k, lane, part);
    }
    __syncthreads();

    i32x8 b = {};
#pragma unroll
    for (int part = 0; part < 4; ++part) {
        const i32x2 pair = __builtin_amdgcn_ds_read_tr8_b64_v2i32(
            (lds_i32x2*)&transposed_k[lane * 4 + part]);
        b[part * 2] = pair[0];
        b[part * 2 + 1] = pair[1];
    }
    const f32x4 accumulator = fp8_mfma(pack_fp8_row(q, column), b);

    if (lane < 16) {
        bool active = false;
#pragma unroll
        for (int rank = 0; rank < 3; ++rank) {
            active |= selected[rank] == column;
        }
        float score = accumulator[0] * 0.08838834764831845f + content_scores[column] * 0.75f;
        score = active ? score : -INFINITY;
        const float maximum = subgroup_max_16(score);
        float probability = active ? expf(score - maximum) : 0.0f;
        probability /= subgroup_sum_16(probability);
        float value = 0.0f;
#pragma unroll
        for (int token = 0; token < 16; ++token) {
            value += __shfl(probability, token, 16) * decode_fp8(v[token * kChannels + column]);
        }
        output[column] = value * sigmoid(maximum * 0.01f);
    }
}

extern "C" __global__ __launch_bounds__(64) void gfx950_compressed_hybrid_attention(
    const std::uint8_t* q,
    const std::uint8_t* k,
    const std::uint8_t* v,
    const float* token_bias,
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
        const i32x2 pair = __builtin_amdgcn_ds_read_tr8_b64_v2i32(
            (lds_i32x2*)&transposed_k[lane * 4 + part]);
        b[part * 2] = pair[0];
        b[part * 2 + 1] = pair[1];
    }
    const f32x4 accumulator = fp8_mfma(pack_fp8_row(q, column), b);

    if (lane < 16) {
        const float score = accumulator[0] * 0.08838834764831845f + token_bias[column];
        const bool local_active = column >= 12;
        const bool global_active = column == 0 || column == 4 || column == 8;
        const float local_max = subgroup_max_16(local_active ? score : -INFINITY);
        const float global_max = subgroup_max_16(global_active ? score : -INFINITY);
        float local_probability = local_active ? expf(score - local_max) : 0.0f;
        float global_probability = global_active ? expf(score - global_max) : 0.0f;
        local_probability /= subgroup_sum_16(local_probability);
        global_probability /= subgroup_sum_16(global_probability);

        float local_value = 0.0f;
        float global_value = 0.0f;
#pragma unroll
        for (int token = 12; token < 16; ++token) {
            local_value += __shfl(local_probability, token, 16)
                * decode_fp8(v[token * kChannels + column]);
        }
#pragma unroll
        for (int block = 0; block < 3; ++block) {
            float compressed_value = 0.0f;
#pragma unroll
            for (int within = 0; within < 4; ++within) {
                compressed_value += decode_fp8(v[(block * 4 + within) * kChannels + column]) * 0.25f;
            }
            global_value += __shfl(global_probability, block * 4, 16) * compressed_value;
        }
        const float mix = sigmoid(__shfl(score, 0, 16) * 0.01f);
        output[column] = mix * global_value + (1.0f - mix) * local_value;
    }
}

extern "C" __global__ __launch_bounds__(64) void gfx950_attnres_aggregate(
    const float* depth_values,
    const float* depth_logits,
    float* output)
{
    const int channel = static_cast<int>(threadIdx.x);
    if (channel >= kChannels) {
        return;
    }
    float maximum = -INFINITY;
#pragma unroll
    for (int depth = 0; depth < 4; ++depth) {
        maximum = fmaxf(maximum, depth_logits[depth * kChannels + channel]);
    }
    float denominator = 0.0f;
    float value = 0.0f;
#pragma unroll
    for (int depth = 0; depth < 4; ++depth) {
        const float weight = expf(depth_logits[depth * kChannels + channel] - maximum);
        denominator += weight;
        value += weight * depth_values[depth * kChannels + channel];
    }
    output[channel] = value / denominator;
}

extern "C" __global__ __launch_bounds__(64) void gfx950_four_branch_residual(
    const float* residual,
    const float* branches,
    const float* gate_logits,
    float* output)
{
    const int channel = static_cast<int>(threadIdx.x);
    if (channel >= kChannels) {
        return;
    }
    float value = residual[channel];
#pragma unroll
    for (int branch = 0; branch < 4; ++branch) {
        value += 0.25f * sigmoid(gate_logits[branch * kChannels + channel])
            * branches[branch * kChannels + channel];
    }
    output[channel] = value;
}

extern "C" __global__ __launch_bounds__(64) void gfx950_mhc_sinkhorn_mix(
    const float* streams,
    const float* mixing_logits,
    float* output)
{
    __shared__ float matrix[16];
    const int lane = static_cast<int>(threadIdx.x);
    if (lane < 16) {
        matrix[lane] = expf(mixing_logits[lane]);
    }
    __syncthreads();
    if (lane == 0) {
#pragma unroll
        for (int iteration = 0; iteration < 3; ++iteration) {
            for (int row = 0; row < 4; ++row) {
                float sum = 0.0f;
                for (int column = 0; column < 4; ++column) {
                    sum += matrix[row * 4 + column];
                }
                for (int column = 0; column < 4; ++column) {
                    matrix[row * 4 + column] /= sum;
                }
            }
            for (int column = 0; column < 4; ++column) {
                float sum = 0.0f;
                for (int row = 0; row < 4; ++row) {
                    sum += matrix[row * 4 + column];
                }
                for (int row = 0; row < 4; ++row) {
                    matrix[row * 4 + column] /= sum;
                }
            }
        }
    }
    __syncthreads();
    if (lane < kChannels) {
        for (int row = 0; row < 4; ++row) {
            float value = 0.0f;
            for (int column = 0; column < 4; ++column) {
                value += matrix[row * 4 + column] * streams[column * kChannels + lane];
            }
            output[row * kChannels + lane] = value;
        }
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
            "This suite requires gfx950; device %d is %s (%s).\n",
            device,
            properties.name,
            properties.gcnArchName);
        return 2;
    }

    const auto history = deterministic_floats(3 * kChannels, 1, 0.6f);
    const auto decode_gates = deterministic_floats(kChannels, 2, 0.8f);
    const auto initial_state = deterministic_floats(kChannels, 3, 0.5f);
    const std::vector<float> convolution_weights = {0.5f, -0.25f, 0.125f};
    std::vector<float> decode_state_reference(kChannels);
    std::vector<float> decode_output_reference(kChannels);
    kda_step_reference(
        history.data(),
        decode_gates.data(),
        initial_state.data(),
        convolution_weights.data(),
        decode_state_reference.data(),
        decode_output_reference.data());
    DeviceBuffer<float> d_history(history.size());
    DeviceBuffer<float> d_decode_gates(decode_gates.size());
    DeviceBuffer<float> d_initial_state(initial_state.size());
    DeviceBuffer<float> d_weights(convolution_weights.size());
    DeviceBuffer<float> d_decode_state(kChannels);
    DeviceBuffer<float> d_decode_output(kChannels);
    d_history.copy_from(history);
    d_decode_gates.copy_from(decode_gates);
    d_initial_state.copy_from(initial_state);
    d_weights.copy_from(convolution_weights);
    gfx950_kda_gdn_decode<<<1, kWaveSize>>>(
        d_history.get(),
        d_decode_gates.get(),
        d_initial_state.get(),
        d_weights.get(),
        d_decode_state.get(),
        d_decode_output.get());
    HIP_CHECK(hipGetLastError());
    check_result("KDA/GDN decode state", d_decode_state.copy_to_host(), decode_state_reference);
    check_result("KDA/GDN decode norm", d_decode_output.copy_to_host(), decode_output_reference);

    const auto prefill_input = deterministic_floats(kPrefillTokens * kChannels, 4, 0.5f);
    const auto prefill_gates = deterministic_floats(kPrefillTokens * kChannels, 5, 0.7f);
    std::vector<float> prefill_final_reference;
    const auto prefill_output_reference = kda_prefill_reference(
        prefill_input, prefill_gates, initial_state, convolution_weights, &prefill_final_reference);
    DeviceBuffer<float> d_prefill_input(prefill_input.size());
    DeviceBuffer<float> d_prefill_gates(prefill_gates.size());
    DeviceBuffer<float> d_prefill_final(kChannels);
    DeviceBuffer<float> d_prefill_output(prefill_output_reference.size());
    d_prefill_input.copy_from(prefill_input);
    d_prefill_gates.copy_from(prefill_gates);
    gfx950_kda_gdn_prefill<<<1, kWaveSize>>>(
        d_prefill_input.get(),
        d_prefill_gates.get(),
        d_initial_state.get(),
        d_weights.get(),
        d_prefill_final.get(),
        d_prefill_output.get());
    HIP_CHECK(hipGetLastError());
    check_result("KDA/GDN prefill state", d_prefill_final.copy_to_host(), prefill_final_reference);
    check_result("KDA/GDN prefill norm", d_prefill_output.copy_to_host(), prefill_output_reference);

    auto attention_q = deterministic_fp8(kAttentionTokens * kHeadDimension, 1);
    auto attention_k = deterministic_fp8(kAttentionTokens * kHeadDimension, 2);
    auto attention_v = deterministic_fp8(kAttentionTokens * kChannels, 3);
    const std::vector<float> content_scores = {
        0.10f, 0.82f, -0.20f, 0.35f,
        0.61f, 0.55f, 0.14f, 0.92f,
        0.73f, -0.10f, 0.48f, 0.31f,
        0.41f, 0.67f, 0.22f, 0.05f};
    const auto selected_reference = sparse_selection_reference(content_scores);
    const auto sparse_reference = sparse_attention_reference(
        attention_q, attention_k, attention_v, content_scores, selected_reference);
    DeviceBuffer<std::uint8_t> d_q(attention_q.size());
    DeviceBuffer<std::uint8_t> d_k(attention_k.size());
    DeviceBuffer<std::uint8_t> d_v(attention_v.size());
    DeviceBuffer<float> d_content(content_scores.size());
    DeviceBuffer<float> d_sparse_output(kChannels);
    DeviceBuffer<int> d_selected(3);
    d_q.copy_from(attention_q);
    d_k.copy_from(attention_k);
    d_v.copy_from(attention_v);
    d_content.copy_from(content_scores);
    gfx950_content_sparse_attention<<<1, kWaveSize>>>(
        d_q.get(), d_k.get(), d_v.get(), d_content.get(), d_sparse_output.get(), d_selected.get());
    HIP_CHECK(hipGetLastError());
    const auto selected_actual = d_selected.copy_to_host();
    if (selected_actual != selected_reference) {
        std::fprintf(
            stderr,
            "FAIL sparse selection gpu=[%d,%d,%d] cpu=[%d,%d,%d]\n",
            selected_actual[0], selected_actual[1], selected_actual[2],
            selected_reference[0], selected_reference[1], selected_reference[2]);
        return 1;
    }
    std::printf(
        "PASS %-24s ids=[%d,%d,%d]\n",
        "sparse top-k selection",
        selected_actual[0], selected_actual[1], selected_actual[2]);
    check_result("sparse attention", d_sparse_output.copy_to_host(), sparse_reference, 5.0e-3f);

    const auto token_bias = deterministic_floats(kAttentionTokens, 7, 0.4f);
    const auto hybrid_reference = hybrid_attention_reference(
        attention_q, attention_k, attention_v, token_bias);
    DeviceBuffer<float> d_token_bias(token_bias.size());
    DeviceBuffer<float> d_hybrid_output(kChannels);
    d_token_bias.copy_from(token_bias);
    gfx950_compressed_hybrid_attention<<<1, kWaveSize>>>(
        d_q.get(), d_k.get(), d_v.get(), d_token_bias.get(), d_hybrid_output.get());
    HIP_CHECK(hipGetLastError());
    check_result("compressed hybrid attn", d_hybrid_output.copy_to_host(), hybrid_reference, 5.0e-3f);

    const auto depth_values = deterministic_floats(4 * kChannels, 8, 0.7f);
    const auto depth_logits = deterministic_floats(4 * kChannels, 9, 0.9f);
    std::vector<float> attnres_reference(kChannels);
    for (int channel = 0; channel < kChannels; ++channel) {
        float maximum = -INFINITY;
        for (int depth = 0; depth < 4; ++depth) {
            maximum = std::max(maximum, depth_logits[depth * kChannels + channel]);
        }
        float denominator = 0.0f;
        for (int depth = 0; depth < 4; ++depth) {
            const float weight = std::exp(depth_logits[depth * kChannels + channel] - maximum);
            denominator += weight;
            attnres_reference[channel] += weight * depth_values[depth * kChannels + channel];
        }
        attnres_reference[channel] /= denominator;
    }
    DeviceBuffer<float> d_depth_values(depth_values.size());
    DeviceBuffer<float> d_depth_logits(depth_logits.size());
    DeviceBuffer<float> d_attnres(kChannels);
    d_depth_values.copy_from(depth_values);
    d_depth_logits.copy_from(depth_logits);
    gfx950_attnres_aggregate<<<1, kWaveSize>>>(
        d_depth_values.get(), d_depth_logits.get(), d_attnres.get());
    HIP_CHECK(hipGetLastError());
    check_result("AttnRes aggregation", d_attnres.copy_to_host(), attnres_reference);

    const auto residual = deterministic_floats(kChannels, 10, 0.4f);
    const auto branches = deterministic_floats(4 * kChannels, 11, 0.6f);
    const auto branch_gates = deterministic_floats(4 * kChannels, 12, 0.8f);
    std::vector<float> branch_reference = residual;
    for (int channel = 0; channel < kChannels; ++channel) {
        for (int branch = 0; branch < 4; ++branch) {
            branch_reference[channel] += 0.25f * sigmoid(branch_gates[branch * kChannels + channel])
                * branches[branch * kChannels + channel];
        }
    }
    DeviceBuffer<float> d_residual(residual.size());
    DeviceBuffer<float> d_branches(branches.size());
    DeviceBuffer<float> d_branch_gates(branch_gates.size());
    DeviceBuffer<float> d_branch_output(kChannels);
    d_residual.copy_from(residual);
    d_branches.copy_from(branches);
    d_branch_gates.copy_from(branch_gates);
    gfx950_four_branch_residual<<<1, kWaveSize>>>(
        d_residual.get(), d_branches.get(), d_branch_gates.get(), d_branch_output.get());
    HIP_CHECK(hipGetLastError());
    check_result("four-branch residual", d_branch_output.copy_to_host(), branch_reference);

    const auto streams = deterministic_floats(4 * kChannels, 13, 0.7f);
    const auto mixing_logits = deterministic_floats(16, 14, 0.5f);
    std::vector<float> sinkhorn_matrix(16);
    for (int i = 0; i < 16; ++i) {
        sinkhorn_matrix[i] = std::exp(mixing_logits[i]);
    }
    for (int iteration = 0; iteration < 3; ++iteration) {
        for (int row = 0; row < 4; ++row) {
            float sum = 0.0f;
            for (int column = 0; column < 4; ++column) {
                sum += sinkhorn_matrix[row * 4 + column];
            }
            for (int column = 0; column < 4; ++column) {
                sinkhorn_matrix[row * 4 + column] /= sum;
            }
        }
        for (int column = 0; column < 4; ++column) {
            float sum = 0.0f;
            for (int row = 0; row < 4; ++row) {
                sum += sinkhorn_matrix[row * 4 + column];
            }
            for (int row = 0; row < 4; ++row) {
                sinkhorn_matrix[row * 4 + column] /= sum;
            }
        }
    }
    std::vector<float> sinkhorn_reference(4 * kChannels, 0.0f);
    for (int row = 0; row < 4; ++row) {
        for (int channel = 0; channel < kChannels; ++channel) {
            for (int column = 0; column < 4; ++column) {
                sinkhorn_reference[row * kChannels + channel] +=
                    sinkhorn_matrix[row * 4 + column] * streams[column * kChannels + channel];
            }
        }
    }
    DeviceBuffer<float> d_streams(streams.size());
    DeviceBuffer<float> d_mixing_logits(mixing_logits.size());
    DeviceBuffer<float> d_sinkhorn_output(sinkhorn_reference.size());
    d_streams.copy_from(streams);
    d_mixing_logits.copy_from(mixing_logits);
    gfx950_mhc_sinkhorn_mix<<<1, kWaveSize>>>(
        d_streams.get(), d_mixing_logits.get(), d_sinkhorn_output.get());
    HIP_CHECK(hipGetLastError());
    check_result("mHC Sinkhorn mixing", d_sinkhorn_output.copy_to_host(), sinkhorn_reference);

    HIP_CHECK(hipDeviceSynchronize());
    return 0;
}

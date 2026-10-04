use crate::protocol::{Output, Request, buffer, floats, unsigned};
use crate::references::{READ, WRITE, output};
use fe2o3_gfx950_gpt_oss_decode::reference as cpu;

pub const KERNELS: [&str; 4] = [
    "gfx950_gpt_oss_120b_decode_megakernel_v1",
    "gfx950_gpt_oss_120b_router_v1",
    "gfx950_gpt_oss_120b_attention_v1",
    "gfx950_gpt_oss_120b_expert_v1",
];
pub fn contains(k: &str) -> bool {
    KERNELS.contains(&k)
}
fn words(values: &[u16]) -> crate::protocol::Argument {
    buffer(
        "u16",
        READ,
        &values
            .iter()
            .flat_map(|v| v.to_le_bytes())
            .collect::<Vec<_>>(),
    )
}
pub fn generate(k: &str) -> Result<Request, String> {
    let v = cpu::deterministic_batch_inputs();
    let args = match k {
        "gfx950_gpt_oss_120b_decode_megakernel_v1" => vec![
            floats(&v.hidden_f32, READ),
            floats(&v.router_f32, READ),
            words(&v.query_bf16),
            words(&v.key_transposed_bf16),
            floats(&v.value_f32, READ),
            floats(&v.sinks_f32, READ),
            buffer("u8", READ, &v.expert_activation_blocks_fp4),
            buffer("u8", READ, &v.expert_weight_blocks_fp4),
            floats(&v.activation_scales, READ),
            floats(&v.expert_weight_scales, READ),
            output(4096),
            output(4096),
            unsigned(&[u32::MAX; 1024], WRITE),
        ],
        "gfx950_gpt_oss_120b_router_v1" => vec![
            floats(&v.hidden_f32, READ),
            floats(&v.router_f32, READ),
            unsigned(&[u32::MAX; 1024], WRITE),
        ],
        "gfx950_gpt_oss_120b_attention_v1" => vec![
            words(&v.query_bf16),
            words(&v.key_transposed_bf16),
            floats(&v.value_f32, READ),
            floats(&v.sinks_f32, READ),
            output(4096),
        ],
        "gfx950_gpt_oss_120b_expert_v1" => vec![
            buffer("u8", READ, &v.expert_activation_blocks_fp4),
            buffer("u8", READ, &v.expert_weight_blocks_fp4),
            floats(&v.activation_scales, READ),
            floats(&v.expert_weight_scales, READ),
            unsigned(&cpu::reference_batch(&v).packed_top4, READ),
            output(4096),
        ],
        _ => return Err("unknown GPT kernel".to_owned()),
    };
    Ok(Request::new(k, 256, 4, args))
}

// Component projections follow the independent host equations, never device
// operations or an original/optimized KIR interpreter. Inputs are the request.
fn router(hidden: &[f32], weights: &[f32]) -> Vec<u32> {
    let mut out = Vec::with_capacity(1024);
    for item in 0..16 {
        let mut scores: Vec<(usize, f32)> = (0..128)
            .map(|e| {
                (
                    e,
                    (0..2880)
                        .map(|d| hidden[item * 2880 + d] * weights[e * 2880 + d])
                        .sum(),
                )
            })
            .collect();
        scores.sort_by(|a, b| b.1.total_cmp(&a.1).then(a.0.cmp(&b.0)));
        let packed = (0..4).fold(0u32, |p, i| p | ((scores[i].0 as u32) << (i * 7)));
        out.extend([packed; 64]);
    }
    out
}
fn attention(q: &[u16], k: &[u16], v: &[f32], sinks: &[f32]) -> Vec<f32> {
    let mut out = vec![0.0; 4096];
    for item in 0..16 {
        for row in 0..16 {
            let sink = if row < 8 { sinks[item * 16 + row] } else { 0.0 };
            let mut scores = [0.0f32; 16];
            for token in 0..16 {
                scores[token] = (0..64)
                    .map(|d| {
                        cpu::decode_bf16(q[item * 1024 + row * 64 + d])
                            * cpu::decode_bf16(k[item * 1024 + d * 16 + token])
                    })
                    .sum::<f32>()
                    * 0.125;
            }
            let maximum = scores.iter().copied().fold(sink, f32::max);
            let mut denominator = (sink - maximum).exp();
            for s in &mut scores {
                *s = (*s - maximum).exp();
                denominator += *s;
            }
            for col in 0..16 {
                out[item * 256 + row * 16 + col] = (0..16)
                    .map(|token| scores[token] / denominator * v[item * 256 + token * 16 + col])
                    .sum();
            }
        }
    }
    out
}
fn expert(a: &[u8], w: &[u8], scales: &[f32], wscales: &[f32], routes: &[u32]) -> Vec<f32> {
    let mut out = vec![0.0; 4096];
    for item in 0..16 {
        let selected = (routes[item * 64] & 127) as usize;
        for row in 0..16 {
            for col in 0..16 {
                for block in 0..4 {
                    let dot = (0..128)
                        .map(|d| {
                            cpu::decode_fp4(a[item * 8192 + block * 2048 + row * 128 + d])
                                * cpu::decode_fp4(w[selected * 8192 + block * 2048 + d * 16 + col])
                        })
                        .sum::<f32>();
                    out[item * 256 + row * 16 + col] += dot
                        * (scales[item * 4 + block] * wscales[(selected * 4 + block) * 16 + col]);
                }
            }
        }
    }
    out
}
pub fn evaluate(r: &Request) -> Result<Vec<Output>, String> {
    r.corpus_shape(&generate(&r.kernel)?)?;
    match r.kernel.as_str() {
        "gfx950_gpt_oss_120b_decode_megakernel_v1" => {
            let input = cpu::GptOssBatchInputs {
                hidden_f32: r.f32s(0)?,
                router_f32: r.f32s(1)?,
                query_bf16: r.u16s(2)?,
                key_transposed_bf16: r.u16s(3)?,
                value_f32: r.f32s(4)?,
                sinks_f32: r.f32s(5)?,
                expert_activation_blocks_fp4: r.data(6, "u8")?,
                expert_weight_blocks_fp4: r.data(7, "u8")?,
                activation_scales: r.f32s(8)?,
                expert_weight_scales: r.f32s(9)?,
            };
            let routes = router(&input.hidden_f32, &input.router_f32);
            Ok(vec![
                Output::f32s(
                    10,
                    &attention(
                        &input.query_bf16,
                        &input.key_transposed_bf16,
                        &input.value_f32,
                        &input.sinks_f32,
                    ),
                    0.0001,
                ),
                Output::f32s(
                    11,
                    &expert(
                        &input.expert_activation_blocks_fp4,
                        &input.expert_weight_blocks_fp4,
                        &input.activation_scales,
                        &input.expert_weight_scales,
                        &routes,
                    ),
                    0.0001,
                ),
                Output::u32s(12, &routes),
            ])
        }
        "gfx950_gpt_oss_120b_router_v1" => {
            Ok(vec![Output::u32s(2, &router(&r.f32s(0)?, &r.f32s(1)?))])
        }
        "gfx950_gpt_oss_120b_attention_v1" => Ok(vec![Output::f32s(
            4,
            &attention(&r.u16s(0)?, &r.u16s(1)?, &r.f32s(2)?, &r.f32s(3)?),
            0.0001,
        )]),
        "gfx950_gpt_oss_120b_expert_v1" => Ok(vec![Output::f32s(
            5,
            &expert(
                &r.data(0, "u8")?,
                &r.data(1, "u8")?,
                &r.f32s(2)?,
                &r.f32s(3)?,
                &r.u32s(4)?,
            ),
            0.0001,
        )]),
        _ => Err("unknown GPT reference".to_owned()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn component_equations_match_existing_independent_full_reference_on_same_inputs() {
        let v = cpu::deterministic_batch_inputs();
        let full = cpu::reference_batch(&v);
        assert_eq!(router(&v.hidden_f32, &v.router_f32), full.packed_top4);
        assert_eq!(
            attention(
                &v.query_bf16,
                &v.key_transposed_bf16,
                &v.value_f32,
                &v.sinks_f32
            ),
            full.attention
        );
        assert_eq!(
            expert(
                &v.expert_activation_blocks_fp4,
                &v.expert_weight_blocks_fp4,
                &v.activation_scales,
                &v.expert_weight_scales,
                &full.packed_top4
            ),
            full.expert
        );
    }
}

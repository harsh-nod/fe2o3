use crate::protocol::{Output, Request, buffer, floats, integers, scalar, unsigned};
use crate::references::{READ, WRITE, input, output};
use fe2o3_gfx950_advanced_systems::reference as cpu;

pub const KERNELS: [&str; 7] = [
    "gfx950_moe_route_fp4_t16_e4_k2_v1",
    "gfx950_moe_expert_rank_fp4_fp8_v1",
    "gfx950_combine_expert_ranks_v1",
    "gfx950_speculative_transaction_v1",
    "gfx950_qwen_ngram_gather_v1",
    "gfx950_stage_gradient_shard_v1",
    "gfx950_muon_update_4x4_v1",
];
pub fn contains(k: &str) -> bool {
    KERNELS.contains(&k)
}
fn codes(n: usize, fp8: bool) -> Vec<u8> {
    (0..n)
        .map(|i| {
            if fp8 {
                [0xb8, 0xb0, 0, 0x30, 0x38][i % 5]
            } else {
                [0xa, 9, 0, 1, 2][i % 5]
            }
        })
        .collect()
}
fn hash_gram(g: &[i32]) -> u64 {
    g.iter().fold(1_469_598_103_934_665_603u64, |h, v| {
        (h ^ u64::from(*v as u32)).wrapping_mul(1_099_511_628_211)
    })
}
pub fn generate(k: &str) -> Result<Request, String> {
    let args = match k {
        "gfx950_moe_route_fp4_t16_e4_k2_v1" => vec![
            buffer("u8", READ, &codes(16 * 2048, false)),
            input(16 * 512),
            unsigned(&[u32::MAX; 512], WRITE),
            output(512),
            unsigned(&[u32::MAX; 64], WRITE),
            integers(&[-999; 2048], WRITE),
        ],
        "gfx950_moe_expert_rank_fp4_fp8_v1" => vec![
            buffer("u8", READ, &codes(16 * 2048, false)),
            buffer("u8", READ, &codes(16 * 5 * 2048, true)),
            unsigned(&(0..512).map(|i| (i % 4) as u32).collect::<Vec<_>>(), READ),
            floats(&[0.5; 512], READ),
            scalar(0),
            scalar(1),
            output(4096),
        ],
        "gfx950_combine_expert_ranks_v1" => vec![input(1024), input(1024), output(1024)],
        "gfx950_speculative_transaction_v1" => {
            let draft: Vec<i32> = (0..512)
                .map(|i| if (i / 4) % 2 == 0 { (i % 4) as i32 } else { 99 })
                .collect();
            vec![
                integers(&draft, READ),
                integers(&(0..64).map(|i| (i % 4) as i32).collect::<Vec<_>>(), READ),
                floats(&[0.75; 512], READ),
                floats(&[0.5; 64], READ),
                input(128),
                input(4096),
                unsigned(&[u32::MAX; 128], WRITE),
                unsigned(&[u32::MAX; 128], WRITE),
                output(1024),
            ]
        }
        "gfx950_qwen_ngram_gather_v1" => {
            let mut grams = Vec::new();
            let mut hashes = Vec::new();
            let mut queries = Vec::new();
            for batch in 0..16 {
                for slot in 0..16 {
                    let gram = [batch, slot % 8, slot % 8 + 1];
                    grams.extend(gram);
                    hashes.push(hash_gram(&gram));
                    if slot < 8 {
                        queries.extend(gram);
                    }
                }
            }
            vec![
                integers(&queries, READ),
                buffer(
                    "u64",
                    READ,
                    &hashes
                        .iter()
                        .flat_map(|n| n.to_le_bytes())
                        .collect::<Vec<_>>(),
                ),
                integers(&grams, READ),
                integers(&(0..256).collect::<Vec<_>>(), READ),
                integers(&(0..256).map(|i| i % 16).collect::<Vec<_>>(), READ),
                integers(&[-999; 128], WRITE),
            ]
        }
        "gfx950_stage_gradient_shard_v1" => vec![input(256), output(256)],
        "gfx950_muon_update_4x4_v1" => vec![input(512), output(256), output(16)],
        _ => return Err("unknown systems kernel".to_owned()),
    };
    Ok(Request::new(k, 256, 4, args))
}
pub fn evaluate(r: &Request) -> Result<Vec<Output>, String> {
    r.corpus_shape(&generate(&r.kernel)?)?;
    let out = |i, v: &[f32]| Output::f32s(i, v, 0.0001);
    match r.kernel.as_str() {
        "gfx950_moe_route_fp4_t16_e4_k2_v1" => {
            let v = cpu::batched_moe_routing_reference(&r.data(0, "u8")?, &r.f32s(1)?);
            Ok(vec![
                Output::u32s(2, &v.top_experts),
                out(3, &v.top_weights),
                Output::u32s(4, &v.expert_counts),
                Output::i32s(5, &v.dispatch),
            ])
        }
        "gfx950_moe_expert_rank_fp4_fp8_v1" => {
            let routing = cpu::MoeRoutingReference {
                top_experts: r.u32s(2)?,
                top_weights: r.f32s(3)?,
                expert_counts: Vec::new(),
                dispatch: Vec::new(),
            };
            let first = r.scalar(4, "u32")?;
            let shared = r.scalar(5, "u32")?;
            if first >= 3 || shared > 1 || routing.top_experts.iter().any(|x| *x >= 4) {
                return Err("expert selector domain".to_owned());
            }
            Ok(vec![out(
                6,
                &cpu::batched_moe_rank_reference(
                    &r.data(0, "u8")?,
                    &r.data(1, "u8")?,
                    &routing,
                    first as usize,
                    shared != 0,
                ),
            )])
        }
        "gfx950_combine_expert_ranks_v1" => Ok(vec![Output::f32s(
            2,
            &r.f32s(0)?
                .iter()
                .zip(r.f32s(1)?)
                .map(|(a, b)| a + b)
                .collect::<Vec<_>>(),
            0.0,
        )]),
        "gfx950_speculative_transaction_v1" => {
            let v = cpu::batched_speculative_reference(
                &r.i32s(0)?,
                &r.i32s(1)?,
                &r.f32s(2)?,
                &r.f32s(3)?,
                &r.f32s(4)?,
                &r.f32s(5)?,
            );
            Ok(vec![
                Output::u32s(6, &v.accepted),
                Output::u32s(7, &v.committed),
                out(8, &v.state),
            ])
        }
        "gfx950_qwen_ngram_gather_v1" => Ok(vec![Output::i32s(
            5,
            &cpu::batched_ngram_reference(
                &r.i32s(0)?,
                &r.u64s(1)?,
                &r.i32s(2)?,
                &r.i32s(3)?,
                &r.i32s(4)?,
            ),
        )]),
        "gfx950_stage_gradient_shard_v1" => Ok(vec![Output::f32s(1, &r.f32s(0)?, 0.0)]),
        "gfx950_muon_update_4x4_v1" => {
            let v = cpu::batched_muon_reference(&r.f32s(0)?);
            Ok(vec![out(1, &v.update), out(2, &v.norms)])
        }
        _ => Err("unknown systems reference".to_owned()),
    }
}

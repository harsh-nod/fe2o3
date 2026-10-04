use crate::protocol::{Output, Request, buffer, floats, scalar};
use crate::references::{READ, WRITE, input, output};
use fe2o3_gfx950_advanced_attention::reference as cpu;

pub const KERNELS: [&str; 8] = [
    "gfx950_kda_decode",
    "gfx950_kda_chunkwise_prefill",
    "gfx950_content_sparse_attention",
    "gfx950_deepseek_sparse_attention",
    "gfx950_compressed_hybrid_attention",
    "gfx950_attnres_aggregate",
    "gfx950_four_branch_residual",
    "gfx950_mhc_sinkhorn_mix",
];
pub fn contains(kernel: &str) -> bool {
    KERNELS.contains(&kernel)
}
fn fp8(count: usize) -> crate::protocol::Argument {
    buffer(
        "u8",
        READ,
        &(0..count)
            .map(|i| [0xb8, 0xb0, 0, 0x30, 0x38][i % 5])
            .collect::<Vec<_>>(),
    )
}
pub fn generate(kernel: &str) -> Result<Request, String> {
    let args = match kernel {
        "gfx950_kda_decode" => vec![
            input(64),
            input(64),
            input(64),
            floats(&[0.875; 64], READ),
            floats(&[0.25; 4], READ),
            input(1024),
            output(1024),
            output(1024),
        ],
        "gfx950_kda_chunkwise_prefill" => vec![
            input(512),
            input(512),
            input(512),
            floats(&[0.875; 512], READ),
            floats(&[0.25; 32], READ),
            input(1024),
            output(1024),
            output(1024),
            output(1024),
        ],
        "gfx950_content_sparse_attention" => vec![
            fp8(16 * 2048),
            fp8(16 * 2048),
            fp8(16 * 256),
            input(16 * 16),
            output(256),
            buffer("u32", WRITE, &[0xff; 16 * 3 * 4]),
        ],
        "gfx950_compressed_hybrid_attention" => vec![
            fp8(16 * 2048),
            fp8(16 * 2048),
            fp8(16 * 256),
            input(16 * 16),
            output(256),
        ],
        "gfx950_deepseek_sparse_attention" => vec![
            input(64 * 128),
            input(64 * 2048),
            input(64 * 256),
            scalar(1),
            scalar(7),
            scalar(12),
            scalar(u32::MAX),
            output(1024),
            output(1024),
            output(1024),
        ],
        "gfx950_attnres_aggregate" => vec![input(64 * 64), input(64 * 64), output(1024)],
        "gfx950_four_branch_residual" => {
            vec![input(1024), input(64 * 64), input(64 * 64), output(1024)]
        }
        "gfx950_mhc_sinkhorn_mix" => vec![input(16 * 64), input(16 * 16), output(1024)],
        _ => return Err("unknown attention kernel".to_owned()),
    };
    Ok(Request::new(kernel, 256, 4, args))
}
fn shape<T>(data: &[T], count: usize) -> Result<(), String> {
    if data.len() == count {
        Ok(())
    } else {
        Err("fixed device tensor shape differs".to_owned())
    }
}
fn failure(error: cpu::ReferenceErrorV1) -> String {
    format!("CPU attention reference: {error:?}")
}
pub fn evaluate(r: &Request) -> Result<Vec<Output>, String> {
    let expected = generate(&r.kernel)?;
    r.launch(256, 4, expected.arguments.len())?;
    let float_out = |index, value: &[f32]| Output::f32s(index, value, 0.0001);
    match r.kernel.as_str() {
        "gfx950_attnres_aggregate" | "gfx950_four_branch_residual" | "gfx950_mhc_sinkhorn_mix" => {
            let args = (0..r.arguments.len() - 1)
                .map(|i| r.f32s(i))
                .collect::<Result<Vec<_>, _>>()?;
            let mut result = Vec::new();
            match r.kernel.as_str() {
                "gfx950_attnres_aggregate" => {
                    shape(&args[0], 4096)?;
                    shape(&args[1], 4096)?;
                    for b in 0..64 {
                        result.extend(
                            cpu::attnres_aggregate_reference_v1(
                                &args[0][b * 64..(b + 1) * 64],
                                &args[1][b * 64..(b + 1) * 64],
                            )
                            .map_err(failure)?,
                        );
                    }
                }
                "gfx950_four_branch_residual" => {
                    shape(&args[0], 1024)?;
                    shape(&args[1], 4096)?;
                    shape(&args[2], 4096)?;
                    for b in 0..64 {
                        result.extend(
                            cpu::four_branch_residual_reference_v1(
                                &args[0][b * 16..(b + 1) * 16],
                                &args[1][b * 64..(b + 1) * 64],
                                &args[2][b * 64..(b + 1) * 64],
                            )
                            .map_err(failure)?,
                        );
                    }
                }
                _ => {
                    shape(&args[0], 1024)?;
                    shape(&args[1], 256)?;
                    for b in 0..16 {
                        result.extend(
                            cpu::mhc_sinkhorn_mix_reference_v1(
                                &args[0][b * 64..(b + 1) * 64],
                                &args[1][b * 16..(b + 1) * 16],
                            )
                            .map_err(failure)?,
                        );
                    }
                }
            }
            Ok(vec![float_out(r.arguments.len() - 1, &result)])
        }
        "gfx950_kda_decode" | "gfx950_kda_chunkwise_prefill" => {
            let prefill = r.kernel == "gfx950_kda_chunkwise_prefill";
            let tokens = if prefill { 8 } else { 1 };
            let args = (0..6).map(|i| r.f32s(i)).collect::<Result<Vec<_>, _>>()?;
            for values in &args[..4] {
                shape(values, 4 * tokens * 16)?;
            }
            shape(&args[4], 4 * tokens)?;
            shape(&args[5], 1024)?;
            let (mut states, mut output0, mut output1) = (Vec::new(), Vec::new(), Vec::new());
            for b in 0..4 {
                // Device state is [value,key]; the independent recurrence is [key,value].
                let state: Vec<f32> = (0..256)
                    .map(|i| args[5][b * 256 + (i % 16) * 16 + i / 16])
                    .collect();
                let arg = |i: usize| &args[i][b * tokens * 16..(b + 1) * tokens * 16];
                let beta = &args[4][b * tokens..(b + 1) * tokens];
                let (next, result) = if prefill {
                    let v =
                        cpu::kda_prefill_reference_v2(arg(0), arg(1), arg(2), arg(3), beta, &state)
                            .map_err(failure)?;
                    (v.final_state, v.output)
                } else {
                    let v =
                        cpu::kda_decode_reference_v2(arg(0), arg(1), arg(2), arg(3), beta, &state)
                            .map_err(failure)?;
                    (v.state, v.output)
                };
                for value in 0..16 {
                    for key in 0..16 {
                        states.push(next[key * 16 + value]);
                        output0.push(
                            result[if prefill {
                                (key / 4) * 16 + value
                            } else {
                                value
                            }],
                        );
                        if prefill {
                            output1.push(result[(4 + key / 4) * 16 + value]);
                        }
                    }
                }
            }
            let mut out = vec![float_out(6, &states), float_out(7, &output0)];
            if prefill {
                out.push(float_out(8, &output1));
            }
            Ok(out)
        }
        "gfx950_deepseek_sparse_attention" => {
            let q = r.f32s(0)?;
            let k = r.f32s(1)?;
            let v = r.f32s(2)?;
            shape(&q, 64 * 128)?;
            shape(&k, 64 * 2048)?;
            shape(&v, 64 * 256)?;
            let indices = (3..7)
                .map(|i| r.scalar(i, "u32"))
                .collect::<Result<Vec<_>, _>>()?;
            let (mut result, mut maximum, mut normalizer) = (Vec::new(), Vec::new(), Vec::new());
            for b in 0..64 {
                let value = cpu::deepseek_sparse_attention_reference_v1(
                    &q[b * 128..(b + 1) * 128],
                    &k[b * 2048..(b + 1) * 2048],
                    &v[b * 256..(b + 1) * 256],
                    &indices,
                )
                .map_err(failure)?;
                result.extend(value.output);
                maximum.extend([value.softmax_maximum; 16]);
                normalizer.extend([value.softmax_normalizer; 16]);
            }
            Ok(vec![
                float_out(7, &result),
                float_out(8, &maximum),
                float_out(9, &normalizer),
            ])
        }
        "gfx950_content_sparse_attention" | "gfx950_compressed_hybrid_attention" => {
            let q = r.data(0, "u8")?;
            let k = r.data(1, "u8")?;
            let v = r.data(2, "u8")?;
            let bias = r.f32s(3)?;
            shape(&q, 16 * 2048)?;
            shape(&k, 16 * 2048)?;
            shape(&v, 16 * 256)?;
            shape(&bias, 256)?;
            let (mut result, mut indices) = (Vec::new(), Vec::new());
            for b in 0..16 {
                // All sixteen query rows are provided to the device MFMA. Its selected row is zero.
                let query = &q[b * 2048..b * 2048 + 128];
                let keys = &k[b * 2048..(b + 1) * 2048];
                let values = &v[b * 256..(b + 1) * 256];
                let scores = &bias[b * 16..(b + 1) * 16];
                if r.kernel == "gfx950_content_sparse_attention" {
                    let value =
                        cpu::content_sparse_attention_reference_v1(query, keys, values, scores)
                            .map_err(failure)?;
                    result.extend(value.output);
                    indices.extend(value.selected);
                } else {
                    result.extend(
                        cpu::compressed_hybrid_attention_reference_v1(query, keys, values, scores)
                            .map_err(failure)?,
                    );
                }
            }
            let mut out = vec![float_out(4, &result)];
            if !indices.is_empty() {
                out.push(Output::u32s(5, &indices));
            }
            Ok(out)
        }
        _ => Err("unknown attention reference".to_owned()),
    }
}

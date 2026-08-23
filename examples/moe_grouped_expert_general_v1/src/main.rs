use fe2o3_core::{DeviceBuffer, GpuContext, GpuModule, LaunchConfig, Stream};
use fe2o3_device::Bf16;
use fe2o3_host::launch;
use std::path::PathBuf;

const TOKENS: u32 = 17;
const EXPERTS: u32 = 3;
const REDUCTION: u32 = 18;
const OUTPUT_COLUMNS: u32 = 19;
const TOKEN_STRIDE: u32 = 23;
const WEIGHT_STRIDE: u32 = 29;
const EXPERT_WEIGHT_STRIDE: u32 = REDUCTION * WEIGHT_STRIDE + 7;
const BIAS_STRIDE: u32 = 23;
const OUTPUT_STRIDE: u32 = 27;

fn sample(seed: u32, modulus: u32, center: i32, scale: f32) -> f32 {
    ((seed % modulus) as i32 - center) as f32 * scale
}

fn launch_expert(
    stream: &Stream,
    module: &std::sync::Arc<GpuModule>,
    tokens: &DeviceBuffer<u16>,
    weights: &DeviceBuffer<u16>,
    gates: &DeviceBuffer<f32>,
    bias: &DeviceBuffer<f32>,
    output: &DeviceBuffer<f32>,
    rows_padded: u32,
    expert: u32,
) -> fe2o3_core::Result<()> {
    let workgroups = (rows_padded / 16) * OUTPUT_COLUMNS.div_ceil(16);
    // SAFETY: the five buffers and ten scalar arguments exactly match the
    // generated kernel ABI and remain alive through readback.
    unsafe {
        launch! {
            kernel: moe_grouped_expert_general_v1,
            stream: stream,
            module: module,
            config: LaunchConfig {
                grid_dim: (workgroups, 1, 1),
                block_dim: (64, 1, 1),
                shared_mem_bytes: 0,
            },
            args: [
                slice(tokens),
                slice(weights),
                slice(gates),
                slice(bias),
                slice_mut(output),
                scalar(rows_padded),
                scalar(OUTPUT_COLUMNS),
                scalar(REDUCTION),
                scalar(TOKEN_STRIDE),
                scalar(WEIGHT_STRIDE),
                scalar(EXPERT_WEIGHT_STRIDE),
                scalar(BIAS_STRIDE),
                scalar(OUTPUT_STRIDE),
                scalar(expert),
                scalar(EXPERTS),
            ]
        }
    }
}

fn main() -> fe2o3_core::Result<()> {
    let hsaco = std::env::var_os("FE2O3_MOE_EXPERT_HSACO")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            PathBuf::from("target/fe2o3-gfx942/moe_grouped_expert_general_v1.hsaco")
        });
    let context = GpuContext::new(0)?;
    // SAFETY: this executable is the explicit qualification boundary.
    let module = unsafe { context.load_module_from_file_unchecked(hsaco) }?;
    let stream = context.default_stream();

    let mut source = vec![Bf16::ZERO.to_bits(); TOKENS as usize * TOKEN_STRIDE as usize];
    for token in 0..TOKENS {
        for d in 0..REDUCTION {
            source[token as usize * TOKEN_STRIDE as usize + d as usize] =
                Bf16::from_f32(sample(token * 17 + d * 5, 31, 15, 0.03125)).to_bits();
        }
    }
    let mut weights = vec![Bf16::ZERO.to_bits(); EXPERTS as usize * EXPERT_WEIGHT_STRIDE as usize];
    let mut bias = vec![0.0_f32; EXPERTS as usize * BIAS_STRIDE as usize];
    for expert in 0..EXPERTS {
        for d in 0..REDUCTION {
            for column in 0..OUTPUT_COLUMNS {
                weights[expert as usize * EXPERT_WEIGHT_STRIDE as usize
                    + d as usize * WEIGHT_STRIDE as usize
                    + column as usize] =
                    Bf16::from_f32(sample(expert * 97 + d * 11 + column * 7, 29, 14, 0.03125))
                        .to_bits();
            }
        }
        for column in 0..OUTPUT_COLUMNS {
            bias[expert as usize * BIAS_STRIDE as usize + column as usize] =
                sample(expert * 19 + column * 3, 17, 8, 0.015625);
        }
    }

    let mut routes = vec![Vec::<(u32, f32)>::new(); EXPERTS as usize];
    for token in 0..TOKENS {
        routes[(token % EXPERTS) as usize].push((token, 0.65));
        routes[((token + 1) % EXPERTS) as usize].push((token, 0.35));
    }
    let weights_device = DeviceBuffer::from_host(&stream, &weights)?;
    let bias_device = DeviceBuffer::from_host(&stream, &bias)?;
    let mut combined = vec![0.0_f32; TOKENS as usize * OUTPUT_COLUMNS as usize];
    let sentinel = -83.0_f32;
    for expert in 0..EXPERTS {
        let entries = &routes[expert as usize];
        let rows_padded = (entries.len() as u32).div_ceil(16) * 16;
        let mut routed_tokens =
            vec![Bf16::ZERO.to_bits(); rows_padded as usize * TOKEN_STRIDE as usize];
        let mut gates = vec![0.0_f32; rows_padded as usize];
        for (row, (token, gate)) in entries.iter().copied().enumerate() {
            routed_tokens
                [row * TOKEN_STRIDE as usize..row * TOKEN_STRIDE as usize + REDUCTION as usize]
                .copy_from_slice(
                    &source[token as usize * TOKEN_STRIDE as usize
                        ..token as usize * TOKEN_STRIDE as usize + REDUCTION as usize],
                );
            gates[row] = gate;
        }
        let output = vec![sentinel; rows_padded as usize * OUTPUT_STRIDE as usize];
        let tokens_device = DeviceBuffer::from_host(&stream, &routed_tokens)?;
        let gates_device = DeviceBuffer::from_host(&stream, &gates)?;
        let output_device = DeviceBuffer::from_host(&stream, &output)?;
        launch_expert(
            &stream,
            &module,
            &tokens_device,
            &weights_device,
            &gates_device,
            &bias_device,
            &output_device,
            rows_padded,
            expert,
        )?;
        let actual = output_device.to_host_vec(&stream)?;
        for (row, (token, _)) in entries.iter().copied().enumerate() {
            for column in 0..OUTPUT_COLUMNS as usize {
                combined[token as usize * OUTPUT_COLUMNS as usize + column] +=
                    actual[row * OUTPUT_STRIDE as usize + column];
            }
        }
        for row in 0..rows_padded as usize {
            assert!(
                actual[row * OUTPUT_STRIDE as usize + OUTPUT_COLUMNS as usize
                    ..(row + 1) * OUTPUT_STRIDE as usize]
                    .iter()
                    .all(|value| *value == sentinel),
                "expert {expert} wrote output padding"
            );
        }
    }

    let mut maximum_error = 0.0_f32;
    for token in 0..TOKENS {
        for column in 0..OUTPUT_COLUMNS {
            let mut expected = 0.0_f32;
            for (expert, gate) in [(token % EXPERTS, 0.65_f32), ((token + 1) % EXPERTS, 0.35)] {
                let mut projection = 0.0_f32;
                for d in 0..REDUCTION {
                    projection += Bf16::from_bits(
                        source[token as usize * TOKEN_STRIDE as usize + d as usize],
                    )
                    .to_f32()
                        * Bf16::from_bits(
                            weights[expert as usize * EXPERT_WEIGHT_STRIDE as usize
                                + d as usize * WEIGHT_STRIDE as usize
                                + column as usize],
                        )
                        .to_f32();
                }
                projection += bias[expert as usize * BIAS_STRIDE as usize + column as usize];
                expected += gate * projection;
            }
            let actual = combined[token as usize * OUTPUT_COLUMNS as usize + column as usize];
            let error = (actual - expected).abs();
            maximum_error = maximum_error.max(error);
            assert!(
                error <= 2.0e-3,
                "token {token} column {column}: actual={actual} expected={expected}"
            );
        }
    }
    println!(
        "PASS top2-routed-moe tokens={TOKENS} experts={EXPERTS} K={REDUCTION} N={OUTPUT_COLUMNS} routes={} max_error={maximum_error}",
        TOKENS * 2
    );
    Ok(())
}

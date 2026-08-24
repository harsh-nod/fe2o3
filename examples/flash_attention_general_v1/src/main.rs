use fe2o3_core::{DeviceBuffer, GpuContext, GpuModule, LaunchConfig, Stream};
use fe2o3_device::Bf16;
use fe2o3_flash_attention_general_v1::reference::{ReferenceLayoutV1, evaluate_reference_v1};
use fe2o3_host::launch;
use std::path::PathBuf;

#[derive(Clone, Copy)]
struct Case {
    name: &'static str,
    batch_heads: u32,
    queries: u32,
    query_rows_padded: u32,
    keys: u32,
    keys_padded: u32,
    depth: u32,
    value_dimension: u32,
    q_stride: u32,
    k_depth_stride: u32,
    v_stride: u32,
    mask_stride: u32,
    output_stride: u32,
}

const CASES: [Case; 2] = [
    Case {
        name: "tails-and-strides",
        batch_heads: 1,
        queries: 16,
        query_rows_padded: 16,
        keys: 13,
        keys_padded: 16,
        depth: 18,
        value_dimension: 7,
        q_stride: 23,
        k_depth_stride: 19,
        v_stride: 11,
        mask_stride: 20,
        output_stride: 13,
    },
    Case {
        name: "multi-head-multi-tile",
        batch_heads: 2,
        queries: 17,
        query_rows_padded: 32,
        keys: 19,
        keys_padded: 32,
        depth: 33,
        value_dimension: 16,
        q_stride: 37,
        k_depth_stride: 35,
        v_stride: 19,
        mask_stride: 37,
        output_stride: 21,
    },
];

fn sample(seed: u32, modulus: u32, center: i32, scale: f32) -> f32 {
    ((seed % modulus) as i32 - center) as f32 * scale
}

struct DeviceBuffers<'a> {
    q: &'a DeviceBuffer<u16>,
    k: &'a DeviceBuffer<u16>,
    v: &'a DeviceBuffer<f32>,
    mask: &'a DeviceBuffer<f32>,
    output: &'a DeviceBuffer<f32>,
}

#[derive(Clone, Copy)]
struct LaunchLayout {
    k_head_stride: u32,
    v_head_stride: u32,
    output_rows: u32,
    scale: f32,
}

fn launch_case(
    stream: &Stream,
    module: &std::sync::Arc<GpuModule>,
    buffers: DeviceBuffers<'_>,
    case: Case,
    layout: LaunchLayout,
) -> fe2o3_core::Result<()> {
    let workgroups = case.batch_heads * (case.query_rows_padded / 16);
    // SAFETY: buffers match five slice ABI pairs and the scalar list below
    // exactly; all allocations remain live through the synchronous readback.
    unsafe {
        launch! {
            kernel: flash_attention_general_v1,
            stream: stream,
            module: module,
            config: LaunchConfig {
                grid_dim: (workgroups, 1, 1),
                block_dim: (64, 1, 1),
                shared_mem_bytes: 0,
            },
            args: [
                slice(buffers.q),
                slice(buffers.k),
                slice(buffers.v),
                slice(buffers.mask),
                slice_mut(buffers.output),
                scalar(case.batch_heads),
                scalar(case.queries),
                scalar(case.query_rows_padded),
                scalar(case.keys),
                scalar(case.keys_padded),
                scalar(case.depth),
                scalar(case.value_dimension),
                scalar(case.q_stride),
                scalar(case.k_depth_stride),
                scalar(layout.k_head_stride),
                scalar(case.v_stride),
                scalar(layout.v_head_stride),
                scalar(case.mask_stride),
                scalar(case.output_stride),
                scalar(layout.output_rows),
                scalar(layout.scale),
            ]
        }
    }
}

fn run_case(
    context: &std::sync::Arc<GpuContext>,
    module: &std::sync::Arc<GpuModule>,
    case: Case,
) -> fe2o3_core::Result<()> {
    let stream = context.default_stream();
    let output_rows = case.batch_heads * case.query_rows_padded;
    let k_head_stride = case.depth * case.k_depth_stride + 7;
    let v_head_stride = case.keys_padded * case.v_stride + 5;
    let q_len = output_rows as usize * case.q_stride as usize;
    let k_len = case.batch_heads as usize * k_head_stride as usize;
    let v_len = case.batch_heads as usize * v_head_stride as usize;
    let mask_len = output_rows as usize * case.mask_stride as usize;
    let output_len = output_rows as usize * case.output_stride as usize;
    let mut q = vec![Bf16::ZERO.to_bits(); q_len];
    let mut k = vec![Bf16::ZERO.to_bits(); k_len];
    let mut v = vec![0.0_f32; v_len];
    let mut mask = vec![f32::NEG_INFINITY; mask_len];
    let sentinel = -77.0_f32;
    let output = vec![sentinel; output_len];

    for head in 0..case.batch_heads {
        for row in 0..case.query_rows_padded {
            let global_row = head * case.query_rows_padded + row;
            for d in 0..case.depth {
                q[global_row as usize * case.q_stride as usize + d as usize] =
                    Bf16::from_f32(sample(head * 101 + row * 17 + d * 7, 31, 15, 0.03125))
                        .to_bits();
            }
            for key in 0..case.keys_padded {
                let visible = row >= case.queries
                    || key >= case.keys
                    || (row != case.queries / 2 && key <= row);
                if visible {
                    mask[global_row as usize * case.mask_stride as usize + key as usize] = 0.0;
                }
            }
        }
        for d in 0..case.depth {
            for key in 0..case.keys_padded {
                k[head as usize * k_head_stride as usize
                    + d as usize * case.k_depth_stride as usize
                    + key as usize] =
                    Bf16::from_f32(sample(head * 79 + d * 11 + key * 5, 29, 14, 0.03125)).to_bits();
            }
        }
        for key in 0..case.keys_padded {
            for d in 0..case.value_dimension {
                v[head as usize * v_head_stride as usize
                    + key as usize * case.v_stride as usize
                    + d as usize] = sample(head * 67 + key * 13 + d * 3, 23, 11, 0.0625);
            }
        }
    }

    let scale = 1.0 / (case.depth as f32).sqrt();
    let q_device = DeviceBuffer::from_host(&stream, &q)?;
    let k_device = DeviceBuffer::from_host(&stream, &k)?;
    let v_device = DeviceBuffer::from_host(&stream, &v)?;
    let mask_device = DeviceBuffer::from_host(&stream, &mask)?;
    let output_device = DeviceBuffer::from_host(&stream, &output)?;
    launch_case(
        &stream,
        module,
        DeviceBuffers {
            q: &q_device,
            k: &k_device,
            v: &v_device,
            mask: &mask_device,
            output: &output_device,
        },
        case,
        LaunchLayout {
            k_head_stride,
            v_head_stride,
            output_rows,
            scale,
        },
    )?;
    let actual = output_device.to_host_vec(&stream)?;
    let expected = evaluate_reference_v1(
        &q,
        &k,
        &v,
        &mask,
        &output,
        ReferenceLayoutV1 {
            batch_heads: case.batch_heads,
            queries: case.queries,
            query_rows_padded: case.query_rows_padded,
            keys: case.keys,
            keys_padded: case.keys_padded,
            depth: case.depth,
            value_dimension: case.value_dimension,
            query_stride: case.q_stride,
            key_depth_stride: case.k_depth_stride,
            key_head_stride: k_head_stride,
            value_stride: case.v_stride,
            value_head_stride: v_head_stride,
            mask_stride: case.mask_stride,
            output_stride: case.output_stride,
            scale,
        },
    )
    .expect("qualification case satisfies the safe CPU reference contract");

    let mut max_error = 0.0_f32;
    for head in 0..case.batch_heads {
        for row in 0..case.query_rows_padded {
            let global_row = head * case.query_rows_padded + row;
            for d in 0..case.value_dimension {
                let expected =
                    expected[global_row as usize * case.output_stride as usize + d as usize];
                let observed =
                    actual[global_row as usize * case.output_stride as usize + d as usize];
                assert!(
                    observed.is_finite(),
                    "{} head {head} row {row} dim {d}: non-finite output {observed}",
                    case.name
                );
                if row >= case.queries || row == case.queries / 2 {
                    assert_eq!(
                        observed, 0.0,
                        "{} head {head} row {row} dim {d}: zero-output policy violated",
                        case.name
                    );
                }
                let error = (observed - expected).abs();
                max_error = max_error.max(error);
                assert!(
                    error <= 2.0e-4,
                    "{} head {head} row {row} dim {d}: actual={observed} expected={expected}",
                    case.name
                );
            }
        }
    }
    for row in 0..output_rows as usize {
        assert!(
            actual[row * case.output_stride as usize + case.value_dimension as usize
                ..(row + 1) * case.output_stride as usize]
                .iter()
                .all(|value| *value == sentinel),
            "{} wrote output padding",
            case.name
        );
    }
    println!(
        "PASS {:<24} heads={} queries={}/{} keys={}/{} depth={} value_dim={} all_masked_row={} max_error={max_error}",
        case.name,
        case.batch_heads,
        case.queries,
        case.query_rows_padded,
        case.keys,
        case.keys_padded,
        case.depth,
        case.value_dimension,
        case.queries / 2,
    );
    Ok(())
}

fn main() -> fe2o3_core::Result<()> {
    let hsaco = std::env::var_os("FE2O3_FLASH_ATTENTION_HSACO")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("target/fe2o3-gfx942/flash_attention_general_v1.hsaco"));
    let context = GpuContext::new(0)?;
    // SAFETY: this executable is the explicit qualification boundary.
    let module = unsafe { context.load_module_from_file_unchecked(hsaco) }?;
    for case in CASES {
        run_case(&context, &module, case)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn qualification_cases_cover_independent_logical_tails() {
        assert!(
            CASES
                .iter()
                .all(|case| case.queries <= case.query_rows_padded)
        );
        assert!(CASES.iter().all(|case| case.keys <= case.keys_padded));
        assert!(
            CASES
                .iter()
                .any(|case| case.queries < case.query_rows_padded)
        );
        assert!(CASES.iter().any(|case| case.keys < case.keys_padded));
        assert!(CASES.iter().any(|case| case.batch_heads > 1));
    }

    #[test]
    fn fully_masked_reference_policy_is_finite_zero() {
        let scores = [f32::NEG_INFINITY; 7];
        let maximum = scores.iter().copied().fold(f32::NEG_INFINITY, f32::max);
        let denominator = if maximum == f32::NEG_INFINITY {
            0.0
        } else {
            scores
                .iter()
                .map(|score| (*score - maximum).exp())
                .sum::<f32>()
        };
        let output = if denominator > 0.0 {
            1.0 / denominator
        } else {
            0.0
        };
        assert_eq!(output, 0.0);
        assert!(output.is_finite());
    }
}

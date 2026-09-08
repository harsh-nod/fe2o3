use std::io;

use fe2o3_core::{DeviceBuffer, GpuContext, KernelParams, LaunchConfig, launch_kernel_on_stream};
use fe2o3_device::Bf16;
use fe2o3_flash_attention_general_v1::reference::{ReferenceLayoutV1, evaluate_reference_v1};
use fe2o3_host::{
    TutorialRuntimeLaunchIdentityV1, TutorialRuntimeSemanticRegionsV1,
    publish_tutorial_runtime_semantic_observation_v1, tutorial_runtime_semantic_bytes_v1,
};

const HSACO_ENV: &str = "FE2O3_FLASH_ATTENTION_HSACO";
const KERNEL: &str = "flash_attention_general_v1";
const OUTPUT_CANARY_ELEMENTS: usize = 8;
const OUTPUT_PREFIX: f32 = f32::from_bits(0x4f12_3456);
const OUTPUT_SUFFIX: f32 = f32::from_bits(0xcf65_4321);

fn invalid_input(message: &'static str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidInput, message)
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let layout = ReferenceLayoutV1 {
        batch_heads: 2,
        queries: 19,
        query_rows_padded: 32,
        keys: 21,
        keys_padded: 32,
        depth: 23,
        value_dimension: 13,
        query_stride: 27,
        key_depth_stride: 37,
        key_head_stride: 23 * 37 + 5,
        value_stride: 17,
        value_head_stride: 32 * 17 + 7,
        mask_stride: 37,
        output_stride: 19,
        scale: 0.2,
    };
    let output_rows = layout
        .batch_heads
        .checked_mul(layout.query_rows_padded)
        .ok_or_else(|| invalid_input("output row extent overflow"))?;
    let query_len = output_rows as usize * layout.query_stride as usize;
    let key_len = layout.batch_heads as usize * layout.key_head_stride as usize;
    let value_len = layout.batch_heads as usize * layout.value_head_stride as usize;
    let mask_len = output_rows as usize * layout.mask_stride as usize;
    let output_len = output_rows as usize * layout.output_stride as usize;

    let query = (0..query_len)
        .map(|index| Bf16::from_f32((index % 17) as f32 * 0.0625 - 0.5).to_bits())
        .collect::<Vec<_>>();
    let key = (0..key_len)
        .map(|index| Bf16::from_f32((index % 19) as f32 * 0.05 - 0.45).to_bits())
        .collect::<Vec<_>>();
    let value = (0..value_len)
        .map(|index| (index % 23) as f32 * 0.03125 - 0.25)
        .collect::<Vec<_>>();
    let mut mask = vec![f32::NAN; mask_len];
    for head in 0..layout.batch_heads as usize {
        for row in 0..layout.query_rows_padded as usize {
            let global_row = head * layout.query_rows_padded as usize + row;
            for key_index in 0..layout.keys as usize {
                let is_fully_masked = head == 1 && row == 3;
                let is_visible = row < layout.queries as usize && key_index <= row + 4;
                mask[global_row * layout.mask_stride as usize + key_index] =
                    if is_visible && !is_fully_masked {
                        (key_index % 5) as f32 * 0.03125 - 0.0625
                    } else {
                        f32::NEG_INFINITY
                    };
            }
        }
    }
    let initial_output = (0..output_len)
        .map(|index| 2000.0 + index as f32)
        .collect::<Vec<_>>();
    let expected = evaluate_reference_v1(&query, &key, &value, &mask, &initial_output, layout)
        .map_err(invalid_input)?;

    let context = GpuContext::new(0)?;
    let observed = context.observe_target()?;
    if observed.target_id().processor() != "gfx942" || observed.hip_default_warp_size() != 64 {
        return Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "qualification requires a gfx942 wave64 device",
        )
        .into());
    }
    let stream = context.create_stream()?;
    let query_device = DeviceBuffer::from_host(&stream, &query)?;
    let key_device = DeviceBuffer::from_host(&stream, &key)?;
    let value_device = DeviceBuffer::from_host(&stream, &value)?;
    let mask_device = DeviceBuffer::from_host(&stream, &mask)?;
    let mut guarded_output = Vec::with_capacity(output_len + 2 * OUTPUT_CANARY_ELEMENTS);
    guarded_output.extend(std::iter::repeat_n(OUTPUT_PREFIX, OUTPUT_CANARY_ELEMENTS));
    guarded_output.extend_from_slice(&initial_output);
    guarded_output.extend(std::iter::repeat_n(OUTPUT_SUFFIX, OUTPUT_CANARY_ELEMENTS));
    let output_device = DeviceBuffer::from_host(&stream, &guarded_output)?;
    let output_view =
        output_device.view(OUTPUT_CANARY_ELEMENTS..OUTPUT_CANARY_ELEMENTS + output_len)?;
    let hsaco = std::env::var_os(HSACO_ENV).ok_or_else(|| {
        io::Error::new(io::ErrorKind::NotFound, format!("{HSACO_ENV} is not set"))
    })?;

    // SAFETY: the qualification runner compiled this exact symbol for gfx942. The argument
    // order and widths mirror the compiler-emitted 144-byte explicit kernarg ABI, and every
    // allocation remains live through synchronous stream completion.
    unsafe {
        let module = context.load_module_from_file_unchecked(hsaco)?;
        let function = module.load_function(KERNEL)?;
        let mut arguments = KernelParams::new();
        arguments.push(query_device.as_device_ptr());
        arguments.push(query_device.len());
        arguments.push(key_device.as_device_ptr());
        arguments.push(key_device.len());
        arguments.push(value_device.as_device_ptr());
        arguments.push(value_device.len());
        arguments.push(mask_device.as_device_ptr());
        arguments.push(mask_device.len());
        arguments.push(output_view.as_device_ptr());
        arguments.push(output_view.len());
        arguments.push(layout.batch_heads);
        arguments.push(layout.queries);
        arguments.push(layout.query_rows_padded);
        arguments.push(layout.keys);
        arguments.push(layout.keys_padded);
        arguments.push(layout.depth);
        arguments.push(layout.value_dimension);
        arguments.push(layout.query_stride);
        arguments.push(layout.key_depth_stride);
        arguments.push(layout.key_head_stride);
        arguments.push(layout.value_stride);
        arguments.push(layout.value_head_stride);
        arguments.push(layout.mask_stride);
        arguments.push(layout.output_stride);
        arguments.push(output_rows);
        arguments.push(layout.scale);
        let workgroups = layout.batch_heads * (layout.query_rows_padded / 16);
        launch_kernel_on_stream(
            &function,
            LaunchConfig {
                grid_dim: (workgroups, 1, 1),
                block_dim: (64, 1, 1),
                shared_mem_bytes: 0,
            },
            &stream,
            &mut arguments,
        )?;
        stream.synchronize()?;
    }

    let actual_allocation = output_device.to_host_vec(&stream)?;
    let actual = &actual_allocation[OUTPUT_CANARY_ELEMENTS..OUTPUT_CANARY_ELEMENTS + output_len];
    let mut maximum_error = 0.0_f32;
    for row in 0..output_rows as usize {
        for column in 0..layout.output_stride as usize {
            let index = row * layout.output_stride as usize + column;
            if column < layout.value_dimension as usize {
                let error = (actual[index] - expected[index]).abs();
                maximum_error = maximum_error.max(error);
                let tolerance = 5.0e-3_f32.max(expected[index].abs() * 5.0e-3);
                if error > tolerance {
                    return Err(io::Error::other(format!(
                        "attention mismatch at ({row}, {column}): got {}, expected {}, tolerance {tolerance}",
                        actual[index], expected[index]
                    ))
                    .into());
                }
            } else if actual[index].to_bits() != initial_output[index].to_bits() {
                return Err(io::Error::other(format!(
                    "attention modified output padding at ({row}, {column})"
                ))
                .into());
            }
        }
    }
    let query_after = query_device.to_host_vec(&stream)?;
    let key_after = key_device.to_host_vec(&stream)?;
    let value_after = value_device.to_host_vec(&stream)?;
    let mask_after = mask_device.to_host_vec(&stream)?;
    let mut expected_allocation = guarded_output.clone();
    expected_allocation[OUTPUT_CANARY_ELEMENTS..OUTPUT_CANARY_ELEMENTS + output_len]
        .copy_from_slice(&expected);
    let padding_before = (0..output_rows as usize)
        .flat_map(|row| {
            initial_output[row * layout.output_stride as usize + layout.value_dimension as usize
                ..(row + 1) * layout.output_stride as usize]
                .iter()
                .copied()
        })
        .collect::<Vec<_>>();
    let padding_after = (0..output_rows as usize)
        .flat_map(|row| {
            actual[row * layout.output_stride as usize + layout.value_dimension as usize
                ..(row + 1) * layout.output_stride as usize]
                .iter()
                .copied()
        })
        .collect::<Vec<_>>();
    drop(output_view);
    drop(output_device);
    drop(mask_device);
    drop(value_device);
    drop(key_device);
    drop(query_device);
    drop(stream);
    drop(context);
    publish_tutorial_runtime_semantic_observation_v1(
        TutorialRuntimeLaunchIdentityV1 {
            target: "gfx942",
            kernel_symbols: &[KERNEL],
            grid: [layout.batch_heads * (layout.query_rows_padded / 16), 1, 1],
            workgroup: [64, 1, 1],
            dynamic_lds_bytes: 0,
        },
        TutorialRuntimeSemanticRegionsV1 {
            inputs_before: &[
                tutorial_runtime_semantic_bytes_v1(&query),
                tutorial_runtime_semantic_bytes_v1(&key),
                tutorial_runtime_semantic_bytes_v1(&value),
                tutorial_runtime_semantic_bytes_v1(&mask),
            ],
            inputs_after: &[
                tutorial_runtime_semantic_bytes_v1(&query_after),
                tutorial_runtime_semantic_bytes_v1(&key_after),
                tutorial_runtime_semantic_bytes_v1(&value_after),
                tutorial_runtime_semantic_bytes_v1(&mask_after),
            ],
            canaries_before: &[
                tutorial_runtime_semantic_bytes_v1(&guarded_output[..OUTPUT_CANARY_ELEMENTS]),
                tutorial_runtime_semantic_bytes_v1(
                    &guarded_output[OUTPUT_CANARY_ELEMENTS + output_len..],
                ),
            ],
            canaries_after: &[
                tutorial_runtime_semantic_bytes_v1(&actual_allocation[..OUTPUT_CANARY_ELEMENTS]),
                tutorial_runtime_semantic_bytes_v1(
                    &actual_allocation[OUTPUT_CANARY_ELEMENTS + output_len..],
                ),
            ],
            padding_before: &[tutorial_runtime_semantic_bytes_v1(&padding_before)],
            padding_after: &[tutorial_runtime_semantic_bytes_v1(&padding_after)],
            expected_output: &[tutorial_runtime_semantic_bytes_v1(&expected_allocation)],
            observed_output: &[tutorial_runtime_semantic_bytes_v1(&actual_allocation)],
        },
    )?;
    println!(
        "PASS {KERNEL}: {} heads, {}x{} logical attention, depth {}, value dimension {}, {} workgroups, max_abs_error={maximum_error:.6}",
        layout.batch_heads,
        layout.queries,
        layout.keys,
        layout.depth,
        layout.value_dimension,
        layout.batch_heads * (layout.query_rows_padded / 16),
    );
    Ok(())
}

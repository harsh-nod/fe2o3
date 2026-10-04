use std::io;

use fe2o3_core::{
    DeviceBuffer, GpuContext, KernelParams, LaunchConfig, launch_kernel_on_stream,
};
use fe2o3_device::Bf16;
use fe2o3_moe_grouped_expert_general_v1::reference::{
    ReferenceLayoutV1, evaluate_reference_v1,
};

const HSACO_ENV: &str = "FE2O3_MOE_EXPERT_HSACO";
const KERNEL: &str = "moe_grouped_expert_general_v1";

fn invalid_input(message: &'static str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidInput, message)
}

fn checked_len(rows: u32, stride: u32, name: &'static str) -> Result<usize, io::Error> {
    usize::try_from(rows)
        .ok()
        .and_then(|rows| rows.checked_mul(stride as usize))
        .ok_or_else(|| invalid_input(name))
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let layout = ReferenceLayoutV1 {
        rows_padded: 32,
        output_columns: 21,
        reduction: 32,
        token_stride: 37,
        weight_stride: 25,
        expert_weight_stride: 32 * 25 + 7,
        bias_stride: 29,
        output_stride: 31,
        expert: 2,
        experts: 4,
    };
    let token_len = checked_len(
        layout.rows_padded,
        layout.token_stride,
        "routed-token extent overflow",
    )?;
    let weight_len = checked_len(
        layout.experts,
        layout.expert_weight_stride,
        "expert-weight extent overflow",
    )?;
    let bias_len = checked_len(layout.experts, layout.bias_stride, "bias extent overflow")?;
    let output_len = checked_len(
        layout.rows_padded,
        layout.output_stride,
        "routed-output extent overflow",
    )?;

    let routed_tokens = (0..token_len)
        .map(|index| Bf16::from_f32((index % 17) as f32 * 0.03125 - 0.25).to_bits())
        .collect::<Vec<_>>();
    let expert_weights = (0..weight_len)
        .map(|index| Bf16::from_f32((index % 19) as f32 * 0.025 - 0.225).to_bits())
        .collect::<Vec<_>>();
    let route_gates = (0..layout.rows_padded as usize)
        .map(|row| {
            if row >= 19 {
                0.0
            } else {
                0.25 + (row % 5) as f32 * 0.125
            }
        })
        .collect::<Vec<_>>();
    let expert_bias = (0..bias_len)
        .map(|index| (index % 11) as f32 * 0.02 - 0.1)
        .collect::<Vec<_>>();
    let initial_output = (0..output_len)
        .map(|index| 3000.0 + index as f32)
        .collect::<Vec<_>>();
    let expected = evaluate_reference_v1(
        &routed_tokens,
        &expert_weights,
        &route_gates,
        &expert_bias,
        &initial_output,
        layout,
    )
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
    let routed_tokens_device = DeviceBuffer::from_host(&stream, &routed_tokens)?;
    let expert_weights_device = DeviceBuffer::from_host(&stream, &expert_weights)?;
    let route_gates_device = DeviceBuffer::from_host(&stream, &route_gates)?;
    let expert_bias_device = DeviceBuffer::from_host(&stream, &expert_bias)?;
    let routed_output_device = DeviceBuffer::from_host(&stream, &initial_output)?;
    let hsaco = std::env::var_os(HSACO_ENV).ok_or_else(|| {
        io::Error::new(io::ErrorKind::NotFound, format!("{HSACO_ENV} is not set"))
    })?;

    // SAFETY: the qualification runner compiles this exact symbol for gfx942. The argument
    // order and scalar widths mirror the compiler-emitted 120-byte explicit kernarg ABI, and
    // all allocations remain live through the synchronous stream completion.
    unsafe {
        let module = context.load_module_from_file_unchecked(hsaco)?;
        let function = module.load_function(KERNEL)?;
        let mut arguments = KernelParams::new();
        arguments.push(routed_tokens_device.as_device_ptr());
        arguments.push(routed_tokens_device.len());
        arguments.push(expert_weights_device.as_device_ptr());
        arguments.push(expert_weights_device.len());
        arguments.push(route_gates_device.as_device_ptr());
        arguments.push(route_gates_device.len());
        arguments.push(expert_bias_device.as_device_ptr());
        arguments.push(expert_bias_device.len());
        arguments.push(routed_output_device.as_device_ptr());
        arguments.push(routed_output_device.len());
        arguments.push(layout.rows_padded);
        arguments.push(layout.output_columns);
        arguments.push(layout.reduction);
        arguments.push(layout.token_stride);
        arguments.push(layout.weight_stride);
        arguments.push(layout.expert_weight_stride);
        arguments.push(layout.bias_stride);
        arguments.push(layout.output_stride);
        arguments.push(layout.expert);
        arguments.push(layout.experts);
        let tile_rows = layout.rows_padded / 16;
        let tile_columns = layout.output_columns.div_ceil(16);
        launch_kernel_on_stream(
            &function,
            LaunchConfig {
                grid_dim: (tile_rows * tile_columns, 1, 1),
                block_dim: (64, 1, 1),
                shared_mem_bytes: 0,
            },
            &stream,
            &mut arguments,
        )?;
        stream.synchronize()?;
    }

    let actual = routed_output_device.to_host_vec(&stream)?;
    let mut maximum_error = 0.0_f32;
    for row in 0..layout.rows_padded as usize {
        for column in 0..layout.output_stride as usize {
            let index = row * layout.output_stride as usize + column;
            if column < layout.output_columns as usize {
                let error = (actual[index] - expected[index]).abs();
                maximum_error = maximum_error.max(error);
                let tolerance = 5.0e-3_f32.max(expected[index].abs() * 5.0e-3);
                if error > tolerance {
                    return Err(io::Error::other(format!(
                        "MoE expert mismatch at ({row}, {column}): got {}, expected {}, tolerance {tolerance}",
                        actual[index], expected[index]
                    ))
                    .into());
                }
            } else if actual[index].to_bits() != initial_output[index].to_bits() {
                return Err(io::Error::other(format!(
                    "MoE expert modified output padding at ({row}, {column})"
                ))
                .into());
            }
        }
    }
    println!(
        "PASS {KERNEL}: expert {}, {}x{}x{}, {} workgroups, max_abs_error={maximum_error:.6}",
        layout.expert,
        layout.rows_padded,
        layout.output_columns,
        layout.reduction,
        (layout.rows_padded / 16) * layout.output_columns.div_ceil(16),
    );
    Ok(())
}

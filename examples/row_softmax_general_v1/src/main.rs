use std::io;

use fe2o3_core::{DeviceBuffer, GpuContext, KernelParams, LaunchConfig, launch_kernel_on_stream};
use fe2o3_row_softmax_general_v1::reference::{ReferenceLayoutV1, evaluate_reference_v1};

const HSACO_ENV: &str = "FE2O3_ROW_SOFTMAX_HSACO";
const KERNEL: &str = "row_softmax_general_v1";

fn invalid_input(message: &'static str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidInput, message)
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let layout = ReferenceLayoutV1 {
        rows: 5,
        columns: 129,
        input_stride: 137,
        output_stride: 129,
    };
    let input_len = layout.rows as usize * layout.input_stride as usize;
    let output_len = layout.rows as usize * layout.output_stride as usize;
    let input = (0..input_len)
        .map(|index| {
            let row = index / layout.input_stride as usize;
            let column = index % layout.input_stride as usize;
            if column < layout.columns as usize {
                ((row * 17 + column * 5) % 31) as f32 * 0.0625 - 0.75
            } else {
                f32::NAN
            }
        })
        .collect::<Vec<_>>();
    let initial_output = (0..output_len)
        .map(|index| 3000.0 + index as f32)
        .collect::<Vec<_>>();
    let expected = evaluate_reference_v1(&input, &initial_output, layout).map_err(invalid_input)?;

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
    let input_device = DeviceBuffer::from_host(&stream, &input)?;
    let output_device = DeviceBuffer::from_host(&stream, &initial_output)?;
    let hsaco = std::env::var_os(HSACO_ENV).ok_or_else(|| {
        io::Error::new(io::ErrorKind::NotFound, format!("{HSACO_ENV} is not set"))
    })?;

    // SAFETY: the qualification runner compiled this exact symbol for gfx942. The argument
    // order and widths mirror the compiler-emitted row-softmax explicit kernarg ABI, and both
    // allocations remain live through synchronous stream completion.
    unsafe {
        let module = context.load_module_from_file_unchecked(hsaco)?;
        let function = module.load_function(KERNEL)?;
        let mut arguments = KernelParams::new();
        arguments.push(input_device.as_device_ptr());
        arguments.push(input_device.len());
        arguments.push(output_device.as_device_ptr());
        arguments.push(output_device.len());
        arguments.push(layout.rows);
        arguments.push(layout.columns);
        arguments.push(layout.input_stride);
        arguments.push(layout.output_stride);
        launch_kernel_on_stream(
            &function,
            LaunchConfig {
                grid_dim: (1, 1, 1),
                block_dim: (64, 1, 1),
                shared_mem_bytes: 0,
            },
            &stream,
            &mut arguments,
        )?;
        stream.synchronize()?;
    }

    let actual = output_device.to_host_vec(&stream)?;
    let mut maximum_error = 0.0_f32;
    for row in 0..layout.rows as usize {
        for column in 0..layout.output_stride as usize {
            let index = row * layout.output_stride as usize + column;
            if column < layout.columns as usize {
                let error = (actual[index] - expected[index]).abs();
                maximum_error = maximum_error.max(error);
                let tolerance = 5.0e-4_f32.max(expected[index].abs() * 5.0e-4);
                if error > tolerance {
                    return Err(io::Error::other(format!(
                        "row-softmax mismatch at ({row}, {column}): got {}, expected {}, tolerance {tolerance}",
                        actual[index], expected[index]
                    ))
                    .into());
                }
            } else if actual[index].to_bits() != initial_output[index].to_bits() {
                return Err(io::Error::other(format!(
                    "row-softmax modified output padding at ({row}, {column})"
                ))
                .into());
            }
        }
    }
    println!(
        "PASS {KERNEL}: {} rows, {} columns, {} workgroups, max_abs_error={maximum_error:.6}",
        layout.rows, layout.columns, 1,
    );
    Ok(())
}

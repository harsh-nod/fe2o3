use std::io;

use fe2o3_core::{DeviceBuffer, GpuContext, KernelParams, LaunchConfig, launch_kernel_on_stream};
use fe2o3_device::Bf16;
use fe2o3_tiled_gemm_general_v1::reference::{ReferenceProblemV1, evaluate_reference_v1};

const HSACO_ENV: &str = "FE2O3_GENERAL_GEMM_HSACO";
#[cfg(not(feature = "kernel-simt-gemm-general"))]
const KERNEL: &str = "tiled_gemm_general_v1";
#[cfg(feature = "kernel-simt-gemm-general")]
const KERNEL: &str = "simt_gemm_general_v1";

fn invalid_input(message: &'static str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidInput, message)
}

fn qualification_workgroups(problem: ReferenceProblemV1) -> Result<u32, io::Error> {
    if problem.rows == 0 || problem.columns == 0 || problem.output_stride < problem.columns {
        return Err(invalid_input(
            "qualification requires a nonempty valid output",
        ));
    }
    #[cfg(feature = "kernel-simt-gemm-general")]
    let groups = {
        // SIMT indices address physical C slots, including inter-row padding.
        let extent = u64::from(problem.rows - 1) * u64::from(problem.output_stride)
            + u64::from(problem.columns);
        extent.div_ceil(64)
    };
    #[cfg(not(feature = "kernel-simt-gemm-general"))]
    let groups = u64::from(problem.rows).div_ceil(16) * u64::from(problem.columns).div_ceil(16);
    u32::try_from(groups).map_err(|_| invalid_input("qualification grid exceeds u32"))
}

fn finite_reference_error(actual: f32, expected: f32) -> Option<f32> {
    if !actual.is_finite() || !expected.is_finite() {
        return None;
    }
    let error = (actual - expected).abs();
    error.is_finite().then_some(error)
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let problem = ReferenceProblemV1 {
        rows: 19,
        columns: 21,
        reduction: 23,
        lhs_stride: 27,
        rhs_stride: 25,
        output_stride: 29,
        product_scale: 0.75,
        output_scale: -0.25,
    };
    let lhs_len = usize::try_from(problem.rows)? * usize::try_from(problem.lhs_stride)?;
    let rhs_len = usize::try_from(problem.reduction)? * usize::try_from(problem.rhs_stride)?;
    let output_len = usize::try_from(problem.rows)? * usize::try_from(problem.output_stride)?;
    let lhs = (0..lhs_len)
        .map(|index| Bf16::from_f32((index % 13) as f32 * 0.125 - 0.75).to_bits())
        .collect::<Vec<_>>();
    let rhs = (0..rhs_len)
        .map(|index| Bf16::from_f32((index % 11) as f32 * 0.1 - 0.5).to_bits())
        .collect::<Vec<_>>();
    let initial_output = (0..output_len)
        .map(|index| 1000.0 + index as f32)
        .collect::<Vec<_>>();
    let expected =
        evaluate_reference_v1(&lhs, &rhs, &initial_output, problem).map_err(invalid_input)?;
    let workgroups = qualification_workgroups(problem)?;

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
    let lhs_device = DeviceBuffer::from_host(&stream, &lhs)?;
    let rhs_device = DeviceBuffer::from_host(&stream, &rhs)?;
    let output_device = DeviceBuffer::from_host(&stream, &initial_output)?;
    let hsaco = std::env::var_os(HSACO_ENV).ok_or_else(|| {
        io::Error::new(io::ErrorKind::NotFound, format!("{HSACO_ENV} is not set"))
    })?;

    // SAFETY: the qualification runner just compiled this exact symbol for gfx942. The argument
    // order and scalar widths below mirror the compiler-emitted 80-byte explicit kernarg ABI;
    // all three allocations remain live through the synchronous stream completion.
    unsafe {
        let module = context.load_module_from_file_unchecked(hsaco)?;
        let function = module.load_function(KERNEL)?;
        let mut arguments = KernelParams::new();
        arguments.push(lhs_device.as_device_ptr());
        arguments.push(lhs_device.len());
        arguments.push(rhs_device.as_device_ptr());
        arguments.push(rhs_device.len());
        arguments.push(output_device.as_device_ptr());
        arguments.push(output_device.len());
        arguments.push(problem.rows);
        arguments.push(problem.columns);
        arguments.push(problem.reduction);
        arguments.push(problem.lhs_stride);
        arguments.push(problem.rhs_stride);
        arguments.push(problem.output_stride);
        arguments.push(problem.product_scale);
        arguments.push(problem.output_scale);
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

    let actual = output_device.to_host_vec(&stream)?;
    let mut maximum_error = 0.0_f32;
    for row in 0..problem.rows as usize {
        for column in 0..problem.output_stride as usize {
            let index = row * problem.output_stride as usize + column;
            if column < problem.columns as usize {
                let error =
                    finite_reference_error(actual[index], expected[index]).ok_or_else(|| {
                        io::Error::other(format!(
                            "non-finite GEMM result/reference at ({row}, {column})"
                        ))
                    })?;
                maximum_error = maximum_error.max(error);
                let tolerance = 2.0e-3_f32.max(expected[index].abs() * 2.0e-3);
                if error > tolerance {
                    return Err(io::Error::other(format!(
                        "GEMM mismatch at ({row}, {column}): got {}, expected {}, tolerance {tolerance}",
                        actual[index], expected[index]
                    ))
                    .into());
                }
            } else if actual[index].to_bits() != initial_output[index].to_bits() {
                return Err(io::Error::other(format!(
                    "GEMM modified output padding at ({row}, {column})"
                ))
                .into());
            }
        }
    }
    println!(
        "PASS {KERNEL}: {}x{}x{}, {} workgroups, max_abs_error={maximum_error:.6}",
        problem.rows, problem.columns, problem.reduction, workgroups,
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn problem() -> ReferenceProblemV1 {
        ReferenceProblemV1 {
            rows: 19,
            columns: 21,
            reduction: 23,
            lhs_stride: 27,
            rhs_stride: 25,
            output_stride: 29,
            product_scale: 0.75,
            output_scale: -0.25,
        }
    }

    #[test]
    fn selected_symbol_and_grid_match_the_source_mapping() {
        #[cfg(feature = "kernel-simt-gemm-general")]
        {
            assert_eq!(KERNEL, "simt_gemm_general_v1");
            // Last logical output is physical slot 542: ceil(543 / 64) = 9.
            assert_eq!(qualification_workgroups(problem()).unwrap(), 9);
        }
        #[cfg(not(feature = "kernel-simt-gemm-general"))]
        {
            assert_eq!(KERNEL, "tiled_gemm_general_v1");
            assert_eq!(qualification_workgroups(problem()).unwrap(), 4);
        }
    }

    #[test]
    fn grid_rejects_empty_invalid_and_unrepresentable_outputs() {
        for candidate in [
            ReferenceProblemV1 {
                rows: 0,
                ..problem()
            },
            ReferenceProblemV1 {
                columns: 0,
                ..problem()
            },
            ReferenceProblemV1 {
                output_stride: 20,
                ..problem()
            },
            ReferenceProblemV1 {
                rows: u32::MAX,
                columns: u32::MAX,
                output_stride: u32::MAX,
                ..problem()
            },
        ] {
            assert!(qualification_workgroups(candidate).is_err());
        }
    }

    #[test]
    fn nonfinite_results_and_reference_never_pass_the_comparison() {
        for value in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY] {
            assert_eq!(finite_reference_error(value, 1.0), None);
            assert_eq!(finite_reference_error(1.0, value), None);
        }
        assert_eq!(finite_reference_error(f32::MAX, -f32::MAX), None);
        assert_eq!(finite_reference_error(-3.0, 2.0), Some(5.0));
        assert_eq!(finite_reference_error(-0.0, 0.0), Some(0.0));
    }
}

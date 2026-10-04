use std::io;

use fe2o3_core::{
    DeviceBuffer, Event, GpuContext, GpuFunction, KernelParams, LaunchConfig, Stream,
    launch_kernel_on_stream,
};
use fe2o3_device::Bf16;
use fe2o3_gemm_autoresearch_v1::reference::{ReferenceProblemV1, evaluate_reference_v1};

const HSACO_ENV: &str = "FE2O3_AUTORESEARCH_GEMM_HSACO";
const BENCHMARK_ENV: &str = "FE2O3_BENCHMARK";
const KERNEL: &str = "gemm_autoresearch_v1";
const BENCHMARK_SIZES: [(u32, usize); 3] = [(256, 20), (512, 10), (1024, 3)];

fn invalid_input(message: &'static str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidInput, message)
}

fn kernel_params(
    lhs: &DeviceBuffer<u16>,
    rhs: &DeviceBuffer<u16>,
    output: &DeviceBuffer<f32>,
    problem: ReferenceProblemV1,
) -> KernelParams {
    let mut arguments = KernelParams::new();
    arguments.push(lhs.as_device_ptr());
    arguments.push(lhs.len());
    arguments.push(rhs.as_device_ptr());
    arguments.push(rhs.len());
    arguments.push(output.as_device_ptr());
    arguments.push(output.len());
    arguments.push(problem.rows);
    arguments.push(problem.columns);
    arguments.push(problem.reduction);
    arguments.push(problem.lhs_stride);
    arguments.push(problem.rhs_stride);
    arguments.push(problem.output_stride);
    arguments.push(problem.product_scale);
    arguments.push(problem.output_scale);
    arguments
}

fn launch_config(problem: ReferenceProblemV1) -> LaunchConfig {
    let tile_rows = problem.rows.div_ceil(16);
    let tile_columns = problem.columns.div_ceil(16);
    LaunchConfig {
        grid_dim: (tile_rows * tile_columns, 1, 1),
        block_dim: (64, 1, 1),
        shared_mem_bytes: 0,
    }
}

unsafe fn launch(
    function: &GpuFunction,
    stream: &Stream,
    problem: ReferenceProblemV1,
    arguments: &mut KernelParams,
) -> fe2o3_core::Result<()> {
    // SAFETY: the caller retains the matching module and allocations and passes
    // the compiler-emitted ABI assembled by `kernel_params`.
    unsafe { launch_kernel_on_stream(function, launch_config(problem), stream, arguments) }
}

fn percentile(sorted: &[f32], percentile: usize) -> f32 {
    sorted[(sorted.len() - 1) * percentile / 100]
}

fn benchmark(
    context: &std::sync::Arc<GpuContext>,
    function: &GpuFunction,
    stream: &Stream,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut throughputs = Vec::with_capacity(BENCHMARK_SIZES.len());
    for (size, launches_per_sample) in BENCHMARK_SIZES {
        let elements = usize::try_from(size)? * usize::try_from(size)?;
        let lhs = DeviceBuffer::from_host(stream, &vec![0_u16; elements])?;
        let rhs = DeviceBuffer::from_host(stream, &vec![0_u16; elements])?;
        let output = DeviceBuffer::from_host(stream, &vec![0.0_f32; elements])?;
        let problem = ReferenceProblemV1 {
            rows: size,
            columns: size,
            reduction: size,
            lhs_stride: size,
            rhs_stride: size,
            output_stride: size,
            product_scale: 1.0,
            output_scale: 0.0,
        };
        let mut arguments = kernel_params(&lhs, &rhs, &output, problem);
        for _ in 0..10 {
            // SAFETY: buffers, module, function, and ABI remain live through synchronization.
            unsafe { launch(function, stream, problem, &mut arguments)? };
        }
        stream.synchronize()?;

        let mut start = Event::new(context)?;
        let mut stop = Event::new(context)?;
        let mut samples = Vec::with_capacity(11);
        for _ in 0..11 {
            start.record(stream)?;
            for _ in 0..launches_per_sample {
                // SAFETY: buffers, module, function, and ABI remain live through the stop event.
                unsafe { launch(function, stream, problem, &mut arguments)? };
            }
            stop.record(stream)?;
            stop.synchronize()?;
            samples.push(stop.elapsed_time_ms_since(&start)? * 1000.0 / launches_per_sample as f32);
        }
        samples.sort_by(f32::total_cmp);
        let median_us = percentile(&samples, 50);
        let operations = 2.0_f64 * f64::from(size).powi(3);
        let gflops = operations / f64::from(median_us) / 1000.0;
        throughputs.push(gflops);
        println!(
            "AUTORESEARCH_BENCH size={size} median_us={median_us:.3} p10_us={:.3} p90_us={:.3} gflops={gflops:.2}",
            percentile(&samples, 10),
            percentile(&samples, 90),
        );
    }
    let score = throughputs
        .iter()
        .product::<f64>()
        .powf(1.0 / throughputs.len() as f64);
    println!("AUTORESEARCH_SCORE geometric_mean_gflops={score:.2}");
    Ok(())
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

    // SAFETY: the runner just compiled this symbol for gfx942. The module, function,
    // buffers, and exact compiler-emitted ABI remain live through synchronization.
    let module = unsafe { context.load_module_from_file_unchecked(&hsaco)? };
    let function = module.load_function(KERNEL)?;
    let mut arguments = kernel_params(&lhs_device, &rhs_device, &output_device, problem);
    // SAFETY: the exact generated ABI and all referenced resources remain live.
    unsafe { launch(&function, &stream, problem, &mut arguments)? };
    stream.synchronize()?;

    let actual = output_device.to_host_vec(&stream)?;
    let mut maximum_error = 0.0_f32;
    for row in 0..problem.rows as usize {
        for column in 0..problem.output_stride as usize {
            let index = row * problem.output_stride as usize + column;
            if column < problem.columns as usize {
                let error = (actual[index] - expected[index]).abs();
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
        problem.rows,
        problem.columns,
        problem.reduction,
        problem.rows.div_ceil(16) * problem.columns.div_ceil(16),
    );
    if std::env::var_os(BENCHMARK_ENV).is_some() {
        benchmark(&context, &function, &stream)?;
    }
    Ok(())
}

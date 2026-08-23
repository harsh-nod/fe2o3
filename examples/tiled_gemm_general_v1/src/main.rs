use fe2o3_core::{DeviceBuffer, Event, GpuContext, GpuModule, LaunchConfig, Stream};
use fe2o3_device::Bf16;
use fe2o3_host::launch;
use std::path::PathBuf;

#[derive(Clone, Copy)]
struct Case {
    name: &'static str,
    m: u32,
    n: u32,
    k: u32,
    lda: u32,
    ldb: u32,
    ldc: u32,
    alpha: f32,
    beta: f32,
}

const CASES: [Case; 4] = [
    Case {
        name: "packed",
        m: 16,
        n: 16,
        k: 16,
        lda: 16,
        ldb: 16,
        ldc: 16,
        alpha: 1.0,
        beta: 0.0,
    },
    Case {
        name: "strided-all-tails",
        m: 17,
        n: 19,
        k: 18,
        lda: 23,
        ldb: 29,
        ldc: 31,
        alpha: 0.5,
        beta: -1.0,
    },
    Case {
        name: "multi-workgroup-dynamic-k",
        m: 33,
        n: 35,
        k: 33,
        lda: 37,
        ldb: 41,
        ldc: 43,
        alpha: 2.0,
        beta: 0.25,
    },
    Case {
        name: "zero-k-epilogue",
        m: 17,
        n: 19,
        k: 0,
        lda: 0,
        ldb: 0,
        ldc: 23,
        alpha: 7.0,
        beta: 0.5,
    },
];

fn extent(rows: u32, columns: u32, stride: u32) -> usize {
    if rows == 0 || columns == 0 {
        0
    } else {
        (rows - 1) as usize * stride as usize + columns as usize
    }
}

fn input_value(seed: u32, modulus: u32, center: i32, scale: f32) -> f32 {
    ((seed % modulus) as i32 - center) as f32 * scale
}

fn launch_case(
    stream: &Stream,
    module: &std::sync::Arc<GpuModule>,
    a: &DeviceBuffer<u16>,
    b: &DeviceBuffer<u16>,
    c: &DeviceBuffer<f32>,
    case: Case,
) -> fe2o3_core::Result<()> {
    let workgroups = case
        .m
        .div_ceil(16)
        .checked_mul(case.n.div_ceil(16))
        .expect("tile grid fits u32");
    // SAFETY: the symbol has exactly two u16 slice pairs, one f32 slice pair,
    // six u32 scalars, and two f32 scalars. Rust borrows keep the allocations
    // alive and distinct for the asynchronous launch, and each slice length is
    // passed from its owning DeviceBuffer.
    unsafe {
        launch! {
            kernel: tiled_gemm_general_v1,
            stream: stream,
            module: module,
            config: LaunchConfig {
                grid_dim: (workgroups, 1, 1),
                block_dim: (64, 1, 1),
                shared_mem_bytes: 0,
            },
            args: [
                slice(a),
                slice(b),
                slice_mut(c),
                scalar(case.m),
                scalar(case.n),
                scalar(case.k),
                scalar(case.lda),
                scalar(case.ldb),
                scalar(case.ldc),
                scalar(case.alpha),
                scalar(case.beta),
            ]
        }
    }
}

fn run_case(
    context: &std::sync::Arc<GpuContext>,
    module: &std::sync::Arc<fe2o3_core::GpuModule>,
    case: Case,
) -> fe2o3_core::Result<()> {
    let stream = context.default_stream();
    let a_len = extent(case.m, case.k, case.lda);
    let b_len = extent(case.k, case.n, case.ldb);
    let c_len = case.m as usize * case.ldc as usize;
    let mut a = vec![Bf16::from_f32(-91.0).to_bits(); a_len];
    let mut b = vec![Bf16::from_f32(-92.0).to_bits(); b_len];
    let mut c = vec![-93.0_f32; c_len];

    for row in 0..case.m {
        for depth in 0..case.k {
            a[row as usize * case.lda as usize + depth as usize] = Bf16::from_f32(input_value(
                row * 7 + depth * 3,
                17,
                8,
                0.125,
            ))
            .to_bits();
        }
    }
    for depth in 0..case.k {
        for column in 0..case.n {
            b[depth as usize * case.ldb as usize + column as usize] = Bf16::from_f32(input_value(
                depth * 5 + column * 11,
                19,
                9,
                0.0625,
            ))
            .to_bits();
        }
    }
    for row in 0..case.m {
        for column in 0..case.n {
            c[row as usize * case.ldc as usize + column as usize] =
                input_value(row + column, 13, 6, 0.25);
        }
    }

    let initial = c.clone();
    let mut expected = c.clone();
    for row in 0..case.m {
        for column in 0..case.n {
            let mut sum = 0.0_f32;
            for depth in 0..case.k {
                sum += Bf16::from_bits(
                    a[row as usize * case.lda as usize + depth as usize],
                )
                .to_f32()
                    * Bf16::from_bits(
                        b[depth as usize * case.ldb as usize + column as usize],
                    )
                    .to_f32();
            }
            let index = row as usize * case.ldc as usize + column as usize;
            expected[index] = case.alpha * sum + case.beta * expected[index];
        }
    }

    let a_device = DeviceBuffer::from_host(&stream, &a)?;
    let b_device = DeviceBuffer::from_host(&stream, &b)?;
    let c_device = DeviceBuffer::from_host(&stream, &c)?;
    launch_case(&stream, module, &a_device, &b_device, &c_device, case)?;

    let actual = c_device.to_host_vec(&stream)?;
    let mut max_error = 0.0_f32;
    for (index, value) in actual.into_iter().enumerate() {
        let column = index % case.ldc as usize;
        let target = if column < case.n as usize {
            expected[index]
        } else {
            initial[index]
        };
        let error = (value - target).abs();
        max_error = max_error.max(error);
        assert!(
            error <= 2.0e-3,
            "{} mismatch at {index}: actual {value}, expected {target}, error {error}",
            case.name
        );
    }

    println!(
        "PASS {:<28} M={} N={} K={} groups={} max_error={max_error}",
        case.name,
        case.m,
        case.n,
        case.k,
        case.m.div_ceil(16) * case.n.div_ceil(16)
    );
    Ok(())
}

fn percentile(sorted: &[f32], percentile: usize) -> f32 {
    sorted[(sorted.len() - 1) * percentile / 100]
}

fn benchmark_case(
    context: &std::sync::Arc<GpuContext>,
    module: &std::sync::Arc<GpuModule>,
    case: Case,
    launches_per_sample: usize,
) -> fe2o3_core::Result<()> {
    const WARMUPS: usize = 5;
    const SAMPLES: usize = 15;

    let stream = context.default_stream();
    let a = DeviceBuffer::<u16>::zeroed(&stream, extent(case.m, case.k, case.lda))?;
    let b = DeviceBuffer::<u16>::zeroed(&stream, extent(case.k, case.n, case.ldb))?;
    let c = DeviceBuffer::<f32>::zeroed(&stream, case.m as usize * case.ldc as usize)?;
    for _ in 0..WARMUPS {
        launch_case(&stream, module, &a, &b, &c, case)?;
    }
    stream.synchronize()?;

    let mut start = Event::new(context)?;
    let mut stop = Event::new(context)?;
    let mut microseconds = Vec::with_capacity(SAMPLES);
    for _ in 0..SAMPLES {
        start.record(&stream)?;
        for _ in 0..launches_per_sample {
            launch_case(&stream, module, &a, &b, &c, case)?;
        }
        stop.record(&stream)?;
        stop.synchronize()?;
        microseconds.push(
            stop.elapsed_time_ms_since(&start)? * 1_000.0 / launches_per_sample as f32,
        );
    }
    microseconds.sort_by(f32::total_cmp);
    let median_us = percentile(&microseconds, 50);
    let operations = 2.0 * f64::from(case.m) * f64::from(case.n) * f64::from(case.k);
    let gflops = operations / f64::from(median_us) / 1_000.0;
    println!(
        "BENCH fe2o3 M={} N={} K={} median_us={median_us:.3} p10_us={:.3} p90_us={:.3} gflops={gflops:.2}",
        case.m,
        case.n,
        case.k,
        percentile(&microseconds, 10),
        percentile(&microseconds, 90),
    );
    Ok(())
}

fn main() -> fe2o3_core::Result<()> {
    let hsaco = std::env::var_os("FE2O3_GENERAL_GEMM_HSACO")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("target/fe2o3-gfx942/tiled_gemm_general_v1.hsaco"));
    let context = GpuContext::new(0)?;
    // SAFETY: the path is explicit qualification input. Each launch below
    // independently binds and documents the exact generated kernel ABI.
    let module = unsafe { context.load_module_from_file_unchecked(hsaco) }?;
    if std::env::var_os("FE2O3_BENCHMARK").is_some() {
        for (size, launches) in [(256, 20), (512, 10), (1024, 3)] {
            benchmark_case(
                &context,
                &module,
                Case {
                    name: "benchmark",
                    m: size,
                    n: size,
                    k: size,
                    lda: size,
                    ldb: size,
                    ldc: size,
                    alpha: 1.0,
                    beta: 0.0,
                },
                launches,
            )?;
        }
    } else {
        for case in CASES {
            run_case(&context, &module, case)?;
        }
    }
    Ok(())
}

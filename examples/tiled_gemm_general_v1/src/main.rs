use fe2o3_core::{DeviceBuffer, GpuContext, LaunchConfig};
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

fn run_case(
    context: &std::sync::Arc<GpuContext>,
    module: &std::sync::Arc<fe2o3_core::GpuModule>,
    case: Case,
) -> fe2o3_core::Result<()> {
    let stream = context.default_stream();
    let a_len = extent(case.m, case.k, case.lda);
    let b_len = extent(case.k, case.n, case.ldb);
    let c_len = case.m as usize * case.ldc as usize;
    let mut a = vec![-91.0_f32; a_len];
    let mut b = vec![-92.0_f32; b_len];
    let mut c = vec![-93.0_f32; c_len];

    for row in 0..case.m {
        for depth in 0..case.k {
            a[row as usize * case.lda as usize + depth as usize] =
                input_value(row * 7 + depth * 3, 17, 8, 0.125);
        }
    }
    for depth in 0..case.k {
        for column in 0..case.n {
            b[depth as usize * case.ldb as usize + column as usize] =
                input_value(depth * 5 + column * 11, 19, 9, 0.0625);
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
                sum += a[row as usize * case.lda as usize + depth as usize]
                    * b[depth as usize * case.ldb as usize + column as usize];
            }
            let index = row as usize * case.ldc as usize + column as usize;
            expected[index] = case.alpha * sum + case.beta * expected[index];
        }
    }

    let a_device = DeviceBuffer::from_host(&stream, &a)?;
    let b_device = DeviceBuffer::from_host(&stream, &b)?;
    let c_device = DeviceBuffer::from_host(&stream, &c)?;
    let work_items = u32::try_from(c_len).expect("qualification case fits u32");

    // SAFETY: the generated symbol has exactly three f32 slice pairs, six u32
    // scalars, and two f32 scalars. The allocations are disjoint, carry the
    // lengths passed in their slice ABIs, and outlive stream synchronization.
    unsafe {
        launch! {
            kernel: tiled_gemm_general_v1,
            stream: stream,
            module: module,
            config: LaunchConfig::for_num_elems(work_items),
            args: [
                slice(a_device),
                slice(b_device),
                slice_mut(c_device),
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
    }?;

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
            error <= 2.0e-4,
            "{} mismatch at {index}: actual {value}, expected {target}, error {error}",
            case.name
        );
    }

    let groups = work_items.div_ceil(256);
    println!(
        "PASS {:<28} M={} N={} K={} groups={} max_error={max_error}",
        case.name, case.m, case.n, case.k, groups
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
    for case in CASES {
        run_case(&context, &module, case)?;
    }
    Ok(())
}

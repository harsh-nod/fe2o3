use fe2o3_core::{DeviceBuffer, GpuContext, GpuModule, LaunchConfig, Stream};
use fe2o3_host::launch;
use std::path::PathBuf;

const OUTPUT_STRIDE: usize = 4096;

#[derive(Clone, Copy)]
struct Case {
    name: &'static str,
    rows: u32,
    columns: u32,
    input_stride: u32,
}

const CASES: [Case; 4] = [
    Case {
        name: "single-column",
        rows: 3,
        columns: 1,
        input_stride: 4099,
    },
    Case {
        name: "wave-tail",
        rows: 5,
        columns: 63,
        input_stride: 4101,
    },
    Case {
        name: "multi-iteration",
        rows: 7,
        columns: 257,
        input_stride: 4103,
    },
    Case {
        name: "maximum-width",
        rows: 2,
        columns: 4096,
        input_stride: 4103,
    },
];

fn launch_case(
    stream: &Stream,
    module: &std::sync::Arc<GpuModule>,
    input: &DeviceBuffer<f32>,
    output: &DeviceBuffer<f32>,
    case: Case,
) -> fe2o3_core::Result<()> {
    // SAFETY: the generated symbol has two slice pairs followed by four u32
    // scalars. The buffers remain alive and disjoint through synchronization.
    unsafe {
        launch! {
            kernel: row_softmax_general_v1,
            stream: stream,
            module: module,
            config: LaunchConfig {
                grid_dim: (case.rows, 1, 1),
                block_dim: (64, 1, 1),
                shared_mem_bytes: 0,
            },
            args: [
                slice(input),
                slice_mut(output),
                scalar(case.rows),
                scalar(case.columns),
                scalar(case.input_stride),
                scalar(case.rows * 64),
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
    let mut input = vec![f32::NEG_INFINITY; case.rows as usize * case.input_stride as usize];
    for row in 0..case.rows as usize {
        for column in 0..case.columns as usize {
            input[row * case.input_stride as usize + column] =
                ((row * 17 + column * 13) % 41) as f32 * 0.125 - 2.5;
        }
    }
    let sentinel = -91.0_f32;
    let output = vec![sentinel; case.rows as usize * OUTPUT_STRIDE];
    let input_device = DeviceBuffer::from_host(&stream, &input)?;
    let output_device = DeviceBuffer::from_host(&stream, &output)?;
    launch_case(&stream, module, &input_device, &output_device, case)?;
    let actual = output_device.to_host_vec(&stream)?;

    let mut maximum_error = 0.0_f32;
    for row in 0..case.rows as usize {
        let values = &input[row * case.input_stride as usize
            ..row * case.input_stride as usize + case.columns as usize];
        let maximum = values.iter().copied().fold(f32::NEG_INFINITY, f32::max);
        let denominator = values
            .iter()
            .map(|value| (*value - maximum).exp())
            .sum::<f32>();
        for column in 0..case.columns as usize {
            let expected = (values[column] - maximum).exp() / denominator;
            let observed = actual[row * OUTPUT_STRIDE + column];
            maximum_error = maximum_error.max((observed - expected).abs());
            assert!(
                (observed - expected).abs() <= 3.0e-5,
                "{} row {row} column {column}: actual={observed} expected={expected}",
                case.name
            );
        }
        assert!(
            actual[row * OUTPUT_STRIDE + case.columns as usize..(row + 1) * OUTPUT_STRIDE]
                .iter()
                .all(|value| *value == sentinel),
            "{} wrote output padding",
            case.name
        );
    }
    println!(
        "PASS {:<18} rows={} columns={} stride={} max_error={maximum_error}",
        case.name, case.rows, case.columns, case.input_stride
    );
    Ok(())
}

fn main() -> fe2o3_core::Result<()> {
    let hsaco = std::env::var_os("FE2O3_ROW_SOFTMAX_HSACO")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("target/fe2o3-gfx942/row_softmax_general_v1.hsaco"));
    let context = GpuContext::new(0)?;
    // SAFETY: this executable is the explicit qualification boundary.
    let module = unsafe { context.load_module_from_file_unchecked(hsaco) }?;
    for case in CASES {
        run_case(&context, &module, case)?;
    }
    Ok(())
}

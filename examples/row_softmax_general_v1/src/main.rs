use fe2o3_host::{
    AqlDispatchGeometryV1, GeneratedHostReadSliceV1, GeneratedHostReadWriteSliceV1,
    TutorialRuntimeLaunchIdentityV1, TutorialRuntimeSemanticRegionsV1,
};
use fe2o3_row_softmax_general_v1::kernel::row_softmax_general_v1_gpu;
use fe2o3_row_softmax_general_v1::reference::{ReferenceLayoutV1, evaluate_reference_v1};

fe2o3_host::compiler_generated_kernel_expectation_roster_v1! {
    struct RowSoftmaxRoster = [row_softmax_general_v1_gpu::Marker];
}

fn output_padding(values: &[f32], rows: usize, columns: usize, stride: usize) -> Vec<f32> {
    (0..rows)
        .flat_map(|row| {
            values[row * stride + columns..(row + 1) * stride]
                .iter()
                .copied()
        })
        .collect()
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut arguments = std::env::args_os().skip(1);
    if arguments.next().as_deref() != Some(std::ffi::OsStr::new("--gfx942-qualification"))
        || arguments.next().is_some()
    {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "usage: fe2o3-row-softmax-general-v1 --gfx942-qualification",
        )
        .into());
    }

    fe2o3_host::run_generated_application_v1::<RowSoftmaxRoster, _>(|application| {
        const ROWS: u32 = 3;
        const COLUMNS: u32 = 5;
        const INPUT_STRIDE: u32 = 8;
        const OUTPUT_STRIDE: u32 = 7;
        const GRID: u32 = ROWS * 64;
        let mut input = vec![-1_000.0_f32; (ROWS * INPUT_STRIDE) as usize];
        for row in 0..ROWS as usize {
            for column in 0..COLUMNS as usize {
                input[row * INPUT_STRIDE as usize + column] =
                    row as f32 * 0.75 + column as f32 * 0.25 - 1.5;
            }
        }
        let input_before = input.clone();
        let canary = f32::from_bits(0x7f01_2345);
        let canaries_before = [canary, canary];
        let initial_output = vec![-17.0_f32; (ROWS * OUTPUT_STRIDE) as usize];
        let padding_before = output_padding(
            &initial_output,
            ROWS as usize,
            COLUMNS as usize,
            OUTPUT_STRIDE as usize,
        );
        let mut storage = vec![canary; initial_output.len() + 2];
        storage[1..=initial_output.len()].copy_from_slice(&initial_output);

        let input_argument = GeneratedHostReadSliceV1::new(&input);
        let output_argument =
            GeneratedHostReadWriteSliceV1::new(&mut storage[1..=initial_output.len()]);
        let generated = row_softmax_general_v1_gpu::Arguments::new(
            input_argument,
            output_argument,
            ROWS,
            COLUMNS,
            INPUT_STRIDE,
            OUTPUT_STRIDE,
        );
        let geometry = AqlDispatchGeometryV1::new([GRID, 1, 1], [64, 1, 1]).map_err(|error| {
            std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                format!("invalid generated row-softmax geometry: {error:?}"),
            )
        })?;
        let _dispatch = application
            .prepare_generated_application_invocation_v1(generated, geometry, 0, 30_000)?
            .execute()?;

        let expected = evaluate_reference_v1(
            &input_before,
            &initial_output,
            ReferenceLayoutV1 {
                rows: ROWS,
                columns: COLUMNS,
                input_stride: INPUT_STRIDE,
                output_stride: OUTPUT_STRIDE,
            },
        )?;
        let observed = &storage[1..=initial_output.len()];
        let canaries_after = [storage[0], storage[initial_output.len() + 1]];
        let padding_after = output_padding(
            observed,
            ROWS as usize,
            COLUMNS as usize,
            OUTPUT_STRIDE as usize,
        );
        if input != input_before
            || observed != expected
            || canaries_after != canaries_before
            || padding_after != padding_before
        {
            return Err("row softmax gfx942 semantic observation mismatch".into());
        }
        fe2o3_host::publish_tutorial_runtime_semantic_observation_v1(
            TutorialRuntimeLaunchIdentityV1 {
                target: "gfx942",
                kernel_symbols: &["row_softmax_general_v1"],
                grid: [GRID, 1, 1],
                workgroup: [64, 1, 1],
                dynamic_lds_bytes: 0,
            },
            TutorialRuntimeSemanticRegionsV1 {
                inputs_before: &[fe2o3_host::tutorial_runtime_semantic_bytes_v1(
                    &input_before,
                )],
                inputs_after: &[fe2o3_host::tutorial_runtime_semantic_bytes_v1(&input)],
                canaries_before: &[fe2o3_host::tutorial_runtime_semantic_bytes_v1(
                    &canaries_before,
                )],
                canaries_after: &[fe2o3_host::tutorial_runtime_semantic_bytes_v1(
                    &canaries_after,
                )],
                padding_before: &[fe2o3_host::tutorial_runtime_semantic_bytes_v1(
                    &padding_before,
                )],
                padding_after: &[fe2o3_host::tutorial_runtime_semantic_bytes_v1(
                    &padding_after,
                )],
                expected_output: &[fe2o3_host::tutorial_runtime_semantic_bytes_v1(&expected)],
                observed_output: &[fe2o3_host::tutorial_runtime_semantic_bytes_v1(observed)],
            },
        )?;
        Ok(())
    })
}

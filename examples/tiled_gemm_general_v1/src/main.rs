use std::io;

use fe2o3_device::Bf16;
use fe2o3_host::{
    AqlDispatchGeometryV1, GeneratedHostReadSliceV1, GeneratedHostReadWriteSliceV1,
    TutorialRuntimeLaunchIdentityV1, TutorialRuntimeSemanticRegionsV1,
};
use fe2o3_tiled_gemm_general_v1::contract::{SUBGROUP_WIDTH_V1, TILE_M_V1, TILE_N_V1};
use fe2o3_tiled_gemm_general_v1::kernel::tiled_gemm_general_v1_gpu;
use fe2o3_tiled_gemm_general_v1::reference::{ReferenceProblemV1, evaluate_reference_v1};

const KERNEL: &str = "tiled_gemm_general_v1";
const OUTPUT_CANARY_ELEMENTS: usize = 8;
const OUTPUT_PREFIX: f32 = f32::from_bits(0x4f23_4567);
const OUTPUT_SUFFIX: f32 = f32::from_bits(0xcf76_5432);

fe2o3_host::compiler_generated_kernel_expectation_roster_v1! {
    struct GeneralGemmRoster = [tiled_gemm_general_v1_gpu::Marker];
}

fn invalid_input(message: &'static str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidInput, message)
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut arguments = std::env::args_os().skip(1);
    let mode = arguments.next();
    if mode.as_deref() == Some(std::ffi::OsStr::new("--simulate-v8")) {
        let bundle = arguments
            .next()
            .ok_or_else(|| invalid_input("missing Bundle V8 path"))?;
        let request = arguments
            .next()
            .ok_or_else(|| invalid_input("missing simulation request path"))?;
        if arguments.next().is_some() {
            return Err(invalid_input(
                "usage: fe2o3-tiled-gemm-general-v1 --simulate-v8 BUNDLE REQUEST",
            )
            .into());
        }
        #[cfg(feature = "bundle-v8-simulator")]
        {
            for value in
                fe2o3_tiled_gemm_general_v1::simulator::simulate_bundle_v8(bundle, request)?
            {
                println!("{value:?}");
            }
            return Ok(());
        }
        #[cfg(not(feature = "bundle-v8-simulator"))]
        {
            let _ = (bundle, request);
            return Err(invalid_input(
                "Bundle V8 simulation requires --features bundle-v8-simulator",
            )
            .into());
        }
    }
    if mode.as_deref() != Some(std::ffi::OsStr::new("--qualification"))
        || arguments.next().is_some()
    {
        return Err(invalid_input("usage: fe2o3-tiled-gemm-general-v1 --qualification").into());
    }

    fe2o3_host::run_generated_application_v1::<GeneralGemmRoster, _>(|application| {
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
        let lhs_before = lhs.clone();
        let rhs_before = rhs.clone();
        let initial_output = (0..output_len)
            .map(|index| 1000.0 + index as f32)
            .collect::<Vec<_>>();
        let expected =
            evaluate_reference_v1(&lhs, &rhs, &initial_output, problem).map_err(invalid_input)?;
        let mut guarded_output = Vec::with_capacity(output_len + 2 * OUTPUT_CANARY_ELEMENTS);
        guarded_output.extend(std::iter::repeat_n(OUTPUT_PREFIX, OUTPUT_CANARY_ELEMENTS));
        guarded_output.extend_from_slice(&initial_output);
        guarded_output.extend(std::iter::repeat_n(OUTPUT_SUFFIX, OUTPUT_CANARY_ELEMENTS));
        let canaries_before = [
            guarded_output[..OUTPUT_CANARY_ELEMENTS].to_vec(),
            guarded_output[OUTPUT_CANARY_ELEMENTS + output_len..].to_vec(),
        ];
        let tile_rows = problem.rows.div_ceil(TILE_M_V1 as u32);
        let tile_columns = problem.columns.div_ceil(TILE_N_V1 as u32);
        let workgroups = tile_rows
            .checked_mul(tile_columns)
            .ok_or_else(|| invalid_input("GEMM workgroup count overflow"))?;
        let grid = workgroups
            .checked_mul(SUBGROUP_WIDTH_V1 as u32)
            .ok_or_else(|| invalid_input("GEMM grid size overflow"))?;
        let geometry = AqlDispatchGeometryV1::new([grid, 1, 1], [SUBGROUP_WIDTH_V1 as u32, 1, 1])
            .map_err(|_| invalid_input("invalid generated GEMM geometry"))?;
        let generated = tiled_gemm_general_v1_gpu::Arguments::new(
            GeneratedHostReadSliceV1::new(&lhs),
            GeneratedHostReadSliceV1::new(&rhs),
            GeneratedHostReadWriteSliceV1::new(
                &mut guarded_output[OUTPUT_CANARY_ELEMENTS..OUTPUT_CANARY_ELEMENTS + output_len],
            ),
            problem.rows,
            problem.columns,
            problem.reduction,
            problem.lhs_stride,
            problem.rhs_stride,
            problem.output_stride,
            problem.product_scale,
            problem.output_scale,
        );
        let dispatch = application
            .prepare_generated_application_invocation_v1(generated, geometry, 0, 30_000)?
            .execute()?;

        let actual = &guarded_output[OUTPUT_CANARY_ELEMENTS..OUTPUT_CANARY_ELEMENTS + output_len];
        for row in 0..problem.rows as usize {
            for column in 0..problem.output_stride as usize {
                let index = row * problem.output_stride as usize + column;
                if column < problem.columns as usize {
                    if actual[index].to_bits() != expected[index].to_bits() {
                        return Err(io::Error::other(format!(
                            "GEMM numerical-policy mismatch at ({row}, {column}): got {} ({:#010x}), expected {} ({:#010x})",
                            actual[index],
                            actual[index].to_bits(),
                            expected[index],
                            expected[index].to_bits(),
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
        if lhs != lhs_before || rhs != rhs_before {
            return Err(io::Error::other("GEMM modified a read-only input").into());
        }
        let canaries_after = [
            guarded_output[..OUTPUT_CANARY_ELEMENTS].to_vec(),
            guarded_output[OUTPUT_CANARY_ELEMENTS + output_len..].to_vec(),
        ];
        if canaries_after != canaries_before {
            return Err(io::Error::other("GEMM modified an output canary").into());
        }
        let padding_before = (0..problem.rows as usize)
            .flat_map(|row| {
                initial_output[row * problem.output_stride as usize + problem.columns as usize
                    ..(row + 1) * problem.output_stride as usize]
                    .iter()
                    .copied()
            })
            .collect::<Vec<_>>();
        let padding_after = (0..problem.rows as usize)
            .flat_map(|row| {
                actual[row * problem.output_stride as usize + problem.columns as usize
                    ..(row + 1) * problem.output_stride as usize]
                    .iter()
                    .copied()
            })
            .collect::<Vec<_>>();
        let mut expected_allocation = Vec::with_capacity(guarded_output.len());
        expected_allocation.extend_from_slice(&canaries_before[0]);
        expected_allocation.extend_from_slice(&expected);
        expected_allocation.extend_from_slice(&canaries_before[1]);
        let target = dispatch.target().processor().to_owned();
        fe2o3_host::publish_tutorial_runtime_semantic_observation_v1(
            TutorialRuntimeLaunchIdentityV1 {
                target: &target,
                kernel_symbols: &[KERNEL],
                grid: [grid, 1, 1],
                workgroup: [SUBGROUP_WIDTH_V1 as u32, 1, 1],
                dynamic_lds_bytes: 0,
            },
            TutorialRuntimeSemanticRegionsV1 {
                inputs_before: &[
                    fe2o3_host::tutorial_runtime_semantic_bytes_v1(&lhs_before),
                    fe2o3_host::tutorial_runtime_semantic_bytes_v1(&rhs_before),
                ],
                inputs_after: &[
                    fe2o3_host::tutorial_runtime_semantic_bytes_v1(&lhs),
                    fe2o3_host::tutorial_runtime_semantic_bytes_v1(&rhs),
                ],
                canaries_before: &[
                    fe2o3_host::tutorial_runtime_semantic_bytes_v1(&canaries_before[0]),
                    fe2o3_host::tutorial_runtime_semantic_bytes_v1(&canaries_before[1]),
                ],
                canaries_after: &[
                    fe2o3_host::tutorial_runtime_semantic_bytes_v1(&canaries_after[0]),
                    fe2o3_host::tutorial_runtime_semantic_bytes_v1(&canaries_after[1]),
                ],
                padding_before: &[fe2o3_host::tutorial_runtime_semantic_bytes_v1(
                    &padding_before,
                )],
                padding_after: &[fe2o3_host::tutorial_runtime_semantic_bytes_v1(
                    &padding_after,
                )],
                expected_output: &[fe2o3_host::tutorial_runtime_semantic_bytes_v1(
                    &expected_allocation,
                )],
                observed_output: &[fe2o3_host::tutorial_runtime_semantic_bytes_v1(
                    &guarded_output,
                )],
            },
        )?;
        println!(
            "BITWISE PASS {KERNEL}: {}x{}x{}, {workgroups} workgroups on {target}",
            problem.rows, problem.columns, problem.reduction,
        );
        Ok(())
    })
}

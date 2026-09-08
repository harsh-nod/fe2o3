use fe2o3_host::{
    AqlDispatchGeometryV1, GeneratedHostReadSliceV1, GeneratedHostWriteSliceV1,
    TutorialRuntimeLaunchIdentityV1, TutorialRuntimeSemanticRegionsV1,
};
use fe2o3_wave64_collectives_v1::kernel::wave64_collectives_v1_gpu;

fe2o3_host::compiler_generated_kernel_expectation_roster_v1! {
    struct Wave64Roster = [wave64_collectives_v1_gpu::Marker];
}

#[allow(dead_code)]
fn generated_arguments<'allocation>(
    input: &'allocation [f32],
    active_mask: u64,
    reduction: &'allocation mut [f32],
    inclusive: &'allocation mut [f32],
    exclusive: &'allocation mut [f32],
) -> wave64_collectives_v1_gpu::Arguments<'allocation> {
    wave64_collectives_v1_gpu::Arguments::new(
        GeneratedHostReadSliceV1::new(input),
        active_mask,
        GeneratedHostWriteSliceV1::new(reduction),
        GeneratedHostWriteSliceV1::new(inclusive),
        GeneratedHostWriteSliceV1::new(exclusive),
    )
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut arguments = std::env::args_os().skip(1);
    if arguments.next().as_deref() != Some(std::ffi::OsStr::new("--gfx942-qualification"))
        || arguments.next().is_some()
    {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "usage: fe2o3-wave64-collectives-v1 --gfx942-qualification",
        )
        .into());
    }
    fe2o3_host::run_generated_application_v1::<Wave64Roster, _>(|application| {
        let input: [f32; 64] = core::array::from_fn(|lane| (lane as i32 - 31) as f32);
        let input_before = input;
        let active_mask = 0xd3ad_beef_93c5_712b_u64;
        let canary = f32::from_bits(0x7f01_2345);
        let mut reduction_storage = [canary; 66];
        let mut inclusive_storage = [canary; 66];
        let mut exclusive_storage = [canary; 66];
        reduction_storage[1..65].fill(-91.0);
        inclusive_storage[1..65].fill(-92.0);
        exclusive_storage[1..65].fill(-93.0);
        let canaries_before = [canary; 6];
        let geometry = AqlDispatchGeometryV1::new([64, 1, 1], [64, 1, 1]).map_err(|error| {
            std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                format!("invalid generated wave64 geometry: {error:?}"),
            )
        })?;
        let arguments = generated_arguments(
            &input,
            active_mask,
            &mut reduction_storage[1..65],
            &mut inclusive_storage[1..65],
            &mut exclusive_storage[1..65],
        );
        let _dispatch = application
            .prepare_generated_application_invocation_v1(arguments, geometry, 0, 30_000)?
            .execute()?;

        let mut expected_reduction = [0.0_f32; 64];
        let mut expected_inclusive = [0.0_f32; 64];
        let mut expected_exclusive = [0.0_f32; 64];
        fe2o3_wave64_collectives_v1::wave64_collectives_oracle_v1(
            &input_before,
            active_mask,
            &mut expected_reduction,
            &mut expected_inclusive,
            &mut expected_exclusive,
        )?;
        let canaries_after = [
            reduction_storage[0],
            reduction_storage[65],
            inclusive_storage[0],
            inclusive_storage[65],
            exclusive_storage[0],
            exclusive_storage[65],
        ];
        if input != input_before
            || reduction_storage[1..65] != expected_reduction
            || inclusive_storage[1..65] != expected_inclusive
            || exclusive_storage[1..65] != expected_exclusive
            || canaries_after != canaries_before
        {
            return Err("wave64 gfx942 semantic observation mismatch".into());
        }
        fe2o3_host::publish_tutorial_runtime_semantic_observation_v1(
            TutorialRuntimeLaunchIdentityV1 {
                target: "gfx942",
                kernel_symbols: &["wave64_collectives_v1"],
                grid: [64, 1, 1],
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
                padding_before: &[],
                padding_after: &[],
                expected_output: &[
                    fe2o3_host::tutorial_runtime_semantic_bytes_v1(&expected_reduction),
                    fe2o3_host::tutorial_runtime_semantic_bytes_v1(&expected_inclusive),
                    fe2o3_host::tutorial_runtime_semantic_bytes_v1(&expected_exclusive),
                ],
                observed_output: &[
                    fe2o3_host::tutorial_runtime_semantic_bytes_v1(&reduction_storage[1..65]),
                    fe2o3_host::tutorial_runtime_semantic_bytes_v1(&inclusive_storage[1..65]),
                    fe2o3_host::tutorial_runtime_semantic_bytes_v1(&exclusive_storage[1..65]),
                ],
            },
        )?;
        Ok(())
    })
}

#[cfg(test)]
mod tests {
    #[test]
    fn generated_argument_adapter_retains_every_output_borrow() {
        let input = [1.0_f32; 64];
        let mut reduction = [0.0_f32; 64];
        let mut inclusive = [0.0_f32; 64];
        let mut exclusive = [0.0_f32; 64];
        let arguments = super::generated_arguments(
            &input,
            u64::MAX,
            &mut reduction,
            &mut inclusive,
            &mut exclusive,
        );
        drop(arguments);
        reduction[0] = 1.0;
        assert_eq!(reduction[0], 1.0);
    }
}

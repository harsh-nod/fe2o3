use fe2o3_host::{
    AqlDispatchGeometryV1, GeneratedHostReadSliceV1, GeneratedHostReadWriteSliceV1,
    GeneratedHostWriteSliceV1, TutorialRuntimeLaunchIdentityV1, TutorialRuntimeSemanticRegionsV1,
};
use fe2o3_workgroup_sync_v1::kernel::lds_publish_read_reduce_i32_v1_gpu;
use fe2o3_workgroup_sync_v1::scoped_atomic::scoped_atomic_add_u32_v1_gpu;

fe2o3_host::compiler_generated_kernel_expectation_roster_v1! {
    struct WorkgroupRoster = [
        lds_publish_read_reduce_i32_v1_gpu::Marker,
        scoped_atomic_add_u32_v1_gpu::Marker,
    ];
}

#[allow(dead_code)]
fn reduction_arguments<'allocation>(
    values: &'allocation [i32],
    output: &'allocation mut [i32],
) -> lds_publish_read_reduce_i32_v1_gpu::Arguments<'allocation> {
    lds_publish_read_reduce_i32_v1_gpu::Arguments::new(
        GeneratedHostReadSliceV1::new(values),
        GeneratedHostWriteSliceV1::new(output),
    )
}

#[allow(dead_code)]
fn atomic_arguments<'allocation>(
    values: &'allocation [u32],
    eligible: &'allocation [u32],
    target: &'allocation mut [u32],
) -> scoped_atomic_add_u32_v1_gpu::Arguments<'allocation> {
    scoped_atomic_add_u32_v1_gpu::Arguments::new(
        GeneratedHostReadSliceV1::new(values),
        GeneratedHostReadSliceV1::new(eligible),
        GeneratedHostReadWriteSliceV1::new(target),
    )
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut arguments = std::env::args_os().skip(1);
    if arguments.next().as_deref() != Some(std::ffi::OsStr::new("--gfx942-qualification"))
        || arguments.next().is_some()
    {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "usage: fe2o3-workgroup-sync-v1 --gfx942-qualification",
        )
        .into());
    }
    fe2o3_host::run_generated_application_v1::<WorkgroupRoster, _>(|application| {
        let reduction_case = fe2o3_workgroup_sync_v1::reduction_vectors_v1()
            .into_iter()
            .find(|case| case.name == "alternating-cancellation")
            .ok_or("missing reduction qualification vector")?;
        let atomic_case = fe2o3_workgroup_sync_v1::atomic_vectors_v1()
            .into_iter()
            .find(|case| case.name == "alternating-eligible")
            .ok_or("missing atomic qualification vector")?;
        let reduction_input_before = reduction_case.values;
        let atomic_input_before = atomic_case.values;
        let atomic_eligible = atomic_case.eligible.map(u32::from);
        let atomic_eligible_before = atomic_eligible;
        let i32_canary = i32::MIN + 17;
        let u32_canary = 0xdead_beef;
        let mut reduction_storage = [i32_canary, -1, i32_canary];
        let mut atomic_storage = [u32_canary, atomic_case.initial, u32_canary];
        let canaries_before = [i32_canary.to_ne_bytes(), i32_canary.to_ne_bytes()]
            .concat()
            .into_iter()
            .chain(u32_canary.to_ne_bytes())
            .chain(u32_canary.to_ne_bytes())
            .collect::<Vec<_>>();

        let geometry = AqlDispatchGeometryV1::new([64, 1, 1], [64, 1, 1]).map_err(|error| {
            std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                format!("invalid generated workgroup geometry: {error:?}"),
            )
        })?;
        let _reduction_dispatch = application
            .prepare_generated_application_invocation_v1(
                reduction_arguments(&reduction_case.values, &mut reduction_storage[1..2]),
                geometry,
                0,
                30_000,
            )?
            .execute()?;
        let _atomic_dispatch = application
            .prepare_generated_application_invocation_v1(
                atomic_arguments(
                    &atomic_case.values,
                    &atomic_eligible,
                    &mut atomic_storage[1..2],
                ),
                geometry,
                0,
                30_000,
            )?
            .execute()?;

        let mut expected_reduction = [-1_i32];
        let reduction_trace =
            fe2o3_workgroup_sync_v1::canonical_reduction_trace_v1(reduction_case.epoch);
        fe2o3_workgroup_sync_v1::lds_reduction_oracle_v1(
            &reduction_input_before,
            reduction_case.epoch,
            &reduction_trace,
            &mut expected_reduction,
        )?;
        let mut expected_atomic = [atomic_case.initial];
        let atomic_lanes = fe2o3_workgroup_sync_v1::canonical_atomic_lanes_v1(
            &atomic_input_before,
            &atomic_case.eligible,
        );
        fe2o3_workgroup_sync_v1::atomic_add_oracle_v1(
            atomic_case.initial,
            fe2o3_workgroup_sync_v1::canonical_atomic_profile_v1(),
            &atomic_lanes,
            &mut expected_atomic,
        )?;
        let canaries_after = [
            reduction_storage[0].to_ne_bytes(),
            reduction_storage[2].to_ne_bytes(),
        ]
        .concat()
        .into_iter()
        .chain(atomic_storage[0].to_ne_bytes())
        .chain(atomic_storage[2].to_ne_bytes())
        .collect::<Vec<_>>();
        if reduction_case.values != reduction_input_before
            || atomic_case.values != atomic_input_before
            || atomic_eligible != atomic_eligible_before
            || reduction_storage[1..2] != expected_reduction
            || atomic_storage[1..2] != expected_atomic
            || canaries_after != canaries_before
        {
            return Err("workgroup gfx942 semantic observation mismatch".into());
        }
        fe2o3_host::publish_tutorial_runtime_semantic_observation_v1(
            TutorialRuntimeLaunchIdentityV1 {
                target: "gfx942",
                kernel_symbols: &["lds_publish_read_reduce_i32_v1", "scoped_atomic_add_u32_v1"],
                grid: [64, 1, 1],
                workgroup: [64, 1, 1],
                dynamic_lds_bytes: 0,
            },
            TutorialRuntimeSemanticRegionsV1 {
                inputs_before: &[
                    fe2o3_host::tutorial_runtime_semantic_bytes_v1(&reduction_input_before),
                    fe2o3_host::tutorial_runtime_semantic_bytes_v1(&atomic_input_before),
                    fe2o3_host::tutorial_runtime_semantic_bytes_v1(&atomic_eligible_before),
                ],
                inputs_after: &[
                    fe2o3_host::tutorial_runtime_semantic_bytes_v1(&reduction_case.values),
                    fe2o3_host::tutorial_runtime_semantic_bytes_v1(&atomic_case.values),
                    fe2o3_host::tutorial_runtime_semantic_bytes_v1(&atomic_eligible),
                ],
                canaries_before: &[&canaries_before],
                canaries_after: &[&canaries_after],
                padding_before: &[],
                padding_after: &[],
                expected_output: &[
                    fe2o3_host::tutorial_runtime_semantic_bytes_v1(&expected_reduction),
                    fe2o3_host::tutorial_runtime_semantic_bytes_v1(&expected_atomic),
                ],
                observed_output: &[
                    fe2o3_host::tutorial_runtime_semantic_bytes_v1(&reduction_storage[1..2]),
                    fe2o3_host::tutorial_runtime_semantic_bytes_v1(&atomic_storage[1..2]),
                ],
            },
        )?;
        Ok(())
    })
}

#[cfg(test)]
mod tests {
    #[test]
    fn generated_argument_adapters_retain_mutable_borrows() {
        let reduction_values = [1_i32; 64];
        let mut reduction_output = [0_i32; 1];
        let reduction = super::reduction_arguments(&reduction_values, &mut reduction_output);
        drop(reduction);

        let atomic_values = [1_u32; 64];
        let eligible = [1_u32; 64];
        let mut target = [0_u32; 1];
        let atomic = super::atomic_arguments(&atomic_values, &eligible, &mut target);
        drop(atomic);

        reduction_output[0] = 64;
        target[0] = 64;
        assert_eq!((reduction_output[0], target[0]), (64, 64));
    }
}

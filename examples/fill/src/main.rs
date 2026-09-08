use fe2o3_host::{
    AqlDispatchGeometryV1, TutorialRuntimeLaunchIdentityV1, TutorialRuntimeSemanticRegionsV1,
};

fe2o3_host::compiler_generated_kernel_expectation_roster_v1! {
    struct FillRoster = [fe2o3_fill::fill_gpu::Marker];
}

fn generated_arguments(output: &mut [f32]) -> fe2o3_fill::fill_gpu::Arguments<'_> {
    fe2o3_fill::fill_gpu::Arguments::new(fe2o3_host::GeneratedHostWriteSliceV1::new(output))
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut arguments = std::env::args_os().skip(1);
    if arguments.next().as_deref() != Some(std::ffi::OsStr::new("--gfx942-qualification"))
        || arguments.next().is_some()
    {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "usage: fe2o3-fill --gfx942-qualification",
        )
        .into());
    }
    fe2o3_host::run_generated_application_v1::<FillRoster, _>(|application| {
        let canary = -12_345.0_f32;
        let canaries_before = [canary, canary];
        let mut storage = [canary; 66];
        let geometry = AqlDispatchGeometryV1::new([64, 1, 1], [64, 1, 1]).map_err(|error| {
            std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                format!("invalid generated fill geometry: {error:?}"),
            )
        })?;
        let _dispatch = application
            .prepare_generated_application_invocation_v1(
                generated_arguments(&mut storage[1..65]),
                geometry,
                0,
                30_000,
            )?
            .execute()?;

        let mut expected = [canary; 64];
        fe2o3_fill::fill_cpu_reference(&mut expected, 64);
        if storage[1..65] != expected || [storage[0], storage[65]] != canaries_before {
            return Err("fill gfx942 output or canary mismatch".into());
        }
        let canaries_after = [storage[0], storage[65]];
        fe2o3_host::publish_tutorial_runtime_semantic_observation_v1(
            TutorialRuntimeLaunchIdentityV1 {
                target: "gfx942",
                kernel_symbols: &["fill"],
                grid: [64, 1, 1],
                workgroup: [64, 1, 1],
                dynamic_lds_bytes: 0,
            },
            TutorialRuntimeSemanticRegionsV1 {
                inputs_before: &[],
                inputs_after: &[],
                canaries_before: &[fe2o3_host::tutorial_runtime_semantic_bytes_v1(
                    &canaries_before,
                )],
                canaries_after: &[fe2o3_host::tutorial_runtime_semantic_bytes_v1(
                    &canaries_after,
                )],
                padding_before: &[],
                padding_after: &[],
                expected_output: &[fe2o3_host::tutorial_runtime_semantic_bytes_v1(&expected)],
                observed_output: &[fe2o3_host::tutorial_runtime_semantic_bytes_v1(
                    &storage[1..65],
                )],
            },
        )?;
        Ok(())
    })
}

#[cfg(test)]
mod tests {
    #[test]
    fn generated_argument_adapter_accepts_only_a_borrowed_output() {
        let mut output = [0.0_f32; 64];
        let arguments = super::generated_arguments(&mut output);
        drop(arguments);
        output[0] = 1.0;
        assert_eq!(output[0], 1.0);
    }
}

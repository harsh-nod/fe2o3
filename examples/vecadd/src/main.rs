use fe2o3_host::{TutorialRuntimeLaunchIdentityV1, TutorialRuntimeSemanticRegionsV1};

fe2o3_host::compiler_generated_kernel_expectation_roster_v1! {
    struct VecaddRoster = [fe2o3_vecadd::vecadd_gpu::Marker];
}

fn run_gfx942() -> Result<(), Box<dyn std::error::Error>> {
    fe2o3_host::run_generated_application_v1::<VecaddRoster, _>(|application| {
        const ELEMENTS: usize = 257;
        let a = (0..ELEMENTS)
            .map(|index| index as f32 * 0.25)
            .collect::<Vec<_>>();
        let b = (0..ELEMENTS)
            .map(|index| 100.0_f32 - index as f32 * 0.5)
            .collect::<Vec<_>>();
        let a_before = a.clone();
        let b_before = b.clone();
        let canary = -12_345.0_f32;
        let canaries_before = [canary, canary];
        let mut storage = vec![canary; ELEMENTS + 2];
        storage[1..=ELEMENTS].fill(-17.0);

        let geometry = fe2o3_vecadd::vecadd_launch_geometry(ELEMENTS, ELEMENTS, ELEMENTS)?;
        let launch_grid = geometry.grid();
        let launch_workgroup = geometry.workgroup().map(u32::from);
        let arguments = fe2o3_vecadd::vecadd_gpu::Arguments::new(
            fe2o3_host::GeneratedHostReadSliceV1::new(&a),
            fe2o3_host::GeneratedHostReadSliceV1::new(&b),
            fe2o3_host::GeneratedHostWriteSliceV1::new(&mut storage[1..=ELEMENTS]),
        );
        let dispatch = application
            .prepare_generated_application_invocation_v1(arguments, geometry, 0, 30_000)?
            .execute()?;

        let mut expected = vec![-17.0_f32; ELEMENTS];
        fe2o3_vecadd::vecadd_cpu_oracle(
            &a_before,
            &b_before,
            &mut expected,
            launch_grid[0] as usize,
        );
        let canaries_after = [storage[0], storage[ELEMENTS + 1]];
        if a != a_before
            || b != b_before
            || storage[1..=ELEMENTS] != expected
            || canaries_after != canaries_before
        {
            return Err("vecadd gfx942 semantic observation mismatch".into());
        }
        let target = dispatch.target().processor().to_owned();
        fe2o3_host::publish_tutorial_runtime_semantic_observation_v1(
            TutorialRuntimeLaunchIdentityV1 {
                target: &target,
                kernel_symbols: &["vecadd"],
                grid: launch_grid,
                workgroup: launch_workgroup,
                dynamic_lds_bytes: 0,
            },
            TutorialRuntimeSemanticRegionsV1 {
                inputs_before: &[
                    fe2o3_host::tutorial_runtime_semantic_bytes_v1(&a_before),
                    fe2o3_host::tutorial_runtime_semantic_bytes_v1(&b_before),
                ],
                inputs_after: &[
                    fe2o3_host::tutorial_runtime_semantic_bytes_v1(&a),
                    fe2o3_host::tutorial_runtime_semantic_bytes_v1(&b),
                ],
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
                    &storage[1..=ELEMENTS],
                )],
            },
        )?;
        Ok(())
    })
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut arguments = std::env::args_os().skip(1);
    let command = arguments.next();
    if command.as_deref() == Some(std::ffi::OsStr::new("--simulate-v8")) {
        let bundle = arguments.next().ok_or_else(|| {
            std::io::Error::new(std::io::ErrorKind::InvalidInput, "missing Bundle V8 path")
        })?;
        let request = arguments.next().ok_or_else(|| {
            std::io::Error::new(std::io::ErrorKind::InvalidInput, "missing request path")
        })?;
        if arguments.next().is_some() {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "usage: fe2o3-vecadd --simulate-v8 BUNDLE REQUEST",
            )
            .into());
        }
        for value in fe2o3_vecadd::simulate_bundle_v8(&bundle, &request)? {
            println!("{value:?}");
        }
        return Ok(());
    }
    if command.as_deref() == Some(std::ffi::OsStr::new("--gfx942-qualification"))
        && arguments.next().is_none()
    {
        return run_gfx942();
    }
    Err(std::io::Error::new(
        std::io::ErrorKind::InvalidInput,
        "usage: fe2o3-vecadd --simulate-v8 BUNDLE REQUEST | --gfx942-qualification",
    )
    .into())
}

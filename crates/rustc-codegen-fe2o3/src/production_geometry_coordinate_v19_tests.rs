use super::*;

#[test]
fn coordinate_prefix_preserves_full_geometry_coordinates_for_both_targets_and_all_ranks() {
    for profile in [
        fe2o3_amd_target::ProductionAmdTargetProfileV1::Gfx942,
        fe2o3_amd_target::ProductionAmdTargetProfileV1::Gfx950,
    ] {
        for rank in [1, 2, 3] {
            for group in [1, 64, 1024] {
                let workgroup = [group, 1, 1];
                let mut source = module(rank, workgroup, None);
                let dimensions = SemanticWorkgroupDimensionsV1::new(workgroup).unwrap();
                for grid in [1, 17, u32::MAX] {
                    let launch = launch_with_grid(rank, workgroup, [grid, 1, 1], 0, 0);
                    for static_x in [false, true] {
                        source.kernels[0].domain = match (rank, static_x) {
                            (1, false) => LaunchDomain::D1 {
                                x: LaunchExtent::Dynamic,
                            },
                            (1, true) => LaunchDomain::D1 {
                                x: LaunchExtent::Static(1),
                            },
                            (2, value) => LaunchDomain::D2 {
                                x: if value {
                                    LaunchExtent::Static(1)
                                } else {
                                    LaunchExtent::Dynamic
                                },
                                y: LaunchExtent::Static(1),
                            },
                            (3, value) => LaunchDomain::D3 {
                                x: if value {
                                    LaunchExtent::Static(1)
                                } else {
                                    LaunchExtent::Dynamic
                                },
                                y: LaunchExtent::Static(1),
                                z: LaunchExtent::Static(1),
                            },
                            _ => unreachable!(),
                        };
                        let (prefix, _) = coordinate_geometry_v19(
                            || Ok(&source.kernels[0]),
                            Some(dimensions),
                            Some(dimensions),
                            &launch,
                            profile.device_target(),
                        )
                        .unwrap();
                        let full = derive_production_geometry_from_launch_for_target_v1(
                            &source,
                            "kernel",
                            Some(dimensions),
                            Some(dimensions),
                            &launch,
                            profile.device_target(),
                        )
                        .unwrap();
                        assert_eq!(
                            (
                                prefix.rank,
                                prefix.workgroup,
                                prefix.max_grid,
                                prefix.max_flat_workgroup_size
                            ),
                            (
                                full.rank(),
                                full.workgroup(),
                                full.max_grid(),
                                full.max_flat_workgroup_size()
                            )
                        );
                        let expected = [u64::from(group) * u64::from(grid), 1, 1];
                        assert_eq!(prefix.formal_coordinate_envelope_v19().unwrap(), expected);
                        assert_eq!(full.formal_coordinate_envelope_v2().unwrap(), expected);
                    }
                }
            }
        }
    }
}

#[test]
fn coordinate_prefix_does_not_grant_full_resource_admission() {
    let workgroup = [64, 1, 1];
    let mut source = module(1, workgroup, None);
    let dimensions = SemanticWorkgroupDimensionsV1::new(workgroup).unwrap();
    // An unresolved call is irrelevant to coordinates but still refuses the
    // old full closure/resource path. No successful coordinate value bypasses it.
    call(&mut source, "missing_device_function");
    let descriptor = launch(1, workgroup, 0, 0);
    let (prefix, _) = coordinate_geometry_v19(
        || Ok(&source.kernels[0]),
        Some(dimensions),
        Some(dimensions),
        &descriptor,
        fe2o3_amd_target::PRODUCTION_GFX942_DEVICE_TARGET_V1,
    )
    .unwrap();
    assert_eq!(prefix.workgroup, workgroup);
    assert_eq!(
        derive_production_geometry_from_launch_v1(
            &source,
            Some(dimensions),
            Some(dimensions),
            &descriptor
        ),
        Err(ProductionGeometryErrorV1::IncompleteKirCallClosure)
    );
}

#[test]
fn coordinate_prefix_retains_descriptor_before_lookup_and_target_error_order() {
    let group = SemanticWorkgroupDimensionsV1::new([64, 1, 1]).unwrap();
    let calls = std::cell::Cell::new(0);
    let lookup = || {
        calls.set(calls.get() + 1);
        Err(ProductionGeometryErrorV1::KernelClosure)
    };
    let mismatch = launch(1, [32, 1, 1], 0, 0);
    assert_eq!(
        coordinate_geometry_v19(lookup, Some(group), Some(group), &mismatch, "bad_target"),
        Err(ProductionGeometryErrorV1::SourceGeometryMismatch {
            semantic: [64, 1, 1],
            descriptor: [32, 1, 1]
        })
    );
    assert_eq!(calls.get(), 0);
    let descriptor = launch(1, [64, 1, 1], 0, 0);
    assert_eq!(
        coordinate_geometry_v19(lookup, Some(group), Some(group), &descriptor, "bad_target"),
        Err(ProductionGeometryErrorV1::KernelClosure)
    );
    assert_eq!(calls.get(), 1);
    let source = module(1, [64, 1, 1], None);
    assert_eq!(
        coordinate_geometry_v19(
            || Ok(&source.kernels[0]),
            Some(group),
            Some(group),
            &descriptor,
            "bad_target"
        ),
        Err(ProductionGeometryErrorV1::MissingTargetCapabilities)
    );
}

#[test]
fn coordinate_prefix_preserves_multidimensional_padding_refusal() {
    for rank in [2, 3] {
        let group = SemanticWorkgroupDimensionsV1::new([64, 1, 1]).unwrap();
        let source = module(rank, [64, 1, 1], None);
        let grid = if rank == 2 { [2, 2, 1] } else { [2, 1, 2] };
        let descriptor = launch_with_grid(rank, [64, 1, 1], grid, 0, 0);
        let (prefix, _) = coordinate_geometry_v19(
            || Ok(&source.kernels[0]),
            Some(group),
            Some(group),
            &descriptor,
            fe2o3_amd_target::PRODUCTION_GFX942_DEVICE_TARGET_V1,
        )
        .unwrap();
        assert_eq!(
            prefix.formal_coordinate_envelope_v19(),
            Err(
                ProductionGeometryErrorV1::UnsupportedFormalCoordinateEnvelope {
                    rank,
                    extents: [128, u64::from(grid[1]), u64::from(grid[2])]
                }
            )
        );
    }
}

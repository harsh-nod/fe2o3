use fe2o3_contracts::{
    AssemblyEffectSetV1, AssemblyOperandSetV1, AssemblyOptionSetV1, KernelFrontendContractErrorV1,
    KernelFrontendContractV1, LaunchBoundsV1, UnsafeAssemblyDeclarationV1, UnsafeAssemblyTargetV1,
    WorkgroupDimensionsV1,
};

fn dims(x: u32, y: u32, z: u32) -> WorkgroupDimensionsV1 {
    WorkgroupDimensionsV1::new(x, y, z).unwrap()
}

#[test]
fn launch_bounds_are_bounded_and_componentwise_consistent() {
    let bounds = LaunchBoundsV1::new(Some(dims(64, 2, 1)), Some(dims(256, 4, 1)), Some(2)).unwrap();
    assert_eq!(bounds.required().unwrap().as_array(), [64, 2, 1]);
    assert_eq!(bounds.maximum().unwrap().volume(), 1_024);
    assert_eq!(bounds.min_workgroups_per_compute_unit(), Some(2));

    assert_eq!(
        WorkgroupDimensionsV1::new(0, 1, 1),
        Err(KernelFrontendContractErrorV1::ZeroWorkgroupDimension)
    );
    assert!(matches!(
        WorkgroupDimensionsV1::new(1_025, 1, 1),
        Err(KernelFrontendContractErrorV1::WorkgroupVolumeTooLarge { .. })
    ));
    assert_eq!(
        LaunchBoundsV1::new(Some(dims(64, 2, 1)), Some(dims(32, 4, 1)), None),
        Err(KernelFrontendContractErrorV1::RequiredExceedsMaximum)
    );
    assert_eq!(
        LaunchBoundsV1::new(Some(dims(64, 1, 1)), None, Some(2)),
        Err(KernelFrontendContractErrorV1::OccupancyRequiresMaximum)
    );
    for invalid in [0, 65] {
        assert!(matches!(
            LaunchBoundsV1::new(None, Some(dims(256, 1, 1)), Some(invalid)),
            Err(KernelFrontendContractErrorV1::InvalidOccupancy { .. })
        ));
    }
}

#[test]
fn unsafe_assembly_is_explicit_bounded_and_non_authoritative() {
    let declaration = UnsafeAssemblyDeclarationV1::new(
        UnsafeAssemblyTargetV1::AmdGpuGfx942,
        AssemblyOperandSetV1::SGPR.union(AssemblyOperandSetV1::IMMEDIATE),
        AssemblyOptionSetV1::NOMEM
            .union(AssemblyOptionSetV1::PURE)
            .union(AssemblyOptionSetV1::NOSTACK),
        AssemblyEffectSetV1::from_bits(0).unwrap(),
    )
    .unwrap();
    assert_eq!(declaration.target().canonical_name(), "gfx942");
    assert!(
        declaration
            .operands()
            .contains(AssemblyOperandSetV1::IMMEDIATE)
    );
    assert_eq!(declaration.effects().bits(), 0);
    assert!(
        KernelFrontendContractV1::new(None, Some(declaration))
            .unwrap()
            .launch()
            .is_none()
    );

    assert!(matches!(
        AssemblyOperandSetV1::from_bits(0x8000),
        Err(KernelFrontendContractErrorV1::UnsupportedAssemblyOperandBits(_))
    ));
    assert_eq!(
        UnsafeAssemblyDeclarationV1::new(
            UnsafeAssemblyTargetV1::AmdGpuGfx942,
            AssemblyOperandSetV1::SGPR,
            AssemblyOptionSetV1::NOMEM.union(AssemblyOptionSetV1::READONLY),
            AssemblyEffectSetV1::from_bits(0).unwrap(),
        ),
        Err(KernelFrontendContractErrorV1::ConflictingAssemblyOptions)
    );
    assert_eq!(
        UnsafeAssemblyDeclarationV1::new(
            UnsafeAssemblyTargetV1::AmdGpuGfx942,
            AssemblyOperandSetV1::ADDRESS,
            AssemblyOptionSetV1::READONLY,
            AssemblyEffectSetV1::WRITE_GLOBAL,
        ),
        Err(KernelFrontendContractErrorV1::AssemblyEffectsConflictWithOptions)
    );
}

#[test]
fn empty_or_ambiguous_contracts_fail_closed() {
    assert_eq!(
        LaunchBoundsV1::new(None, None, None),
        Err(KernelFrontendContractErrorV1::EmptyContract)
    );
    assert_eq!(
        KernelFrontendContractV1::new(None, None),
        Err(KernelFrontendContractErrorV1::EmptyContract)
    );
    assert_eq!(
        UnsafeAssemblyDeclarationV1::new(
            UnsafeAssemblyTargetV1::AmdGpuGfx942,
            AssemblyOperandSetV1::from_bits(0).unwrap(),
            AssemblyOptionSetV1::NOMEM,
            AssemblyEffectSetV1::from_bits(0).unwrap(),
        ),
        Err(KernelFrontendContractErrorV1::EmptyAssemblyOperands)
    );
}

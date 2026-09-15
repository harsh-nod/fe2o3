use super::*;

mod fixture {
    use fe2o3_kernel_ir::*;
    include!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../fe2o3-kernel-ir/src/execution_capability_v1/subgroup_partition/fixture.rs"
    ));

    pub(super) fn graph() -> Module {
        module()
    }
}

fn collective(participants: u32) -> TargetCapabilityRequirementV1 {
    TargetCapabilityRequirementV1::Collective(TargetCollectiveRequirementV1::new(
        TargetExecutionScopeV1::Subgroup,
        TargetCollectiveOperationV1::ReduceAdd,
        TargetScalarTypeV1::new(TargetScalarKindV1::Float, 32),
        participants,
        TargetCollectiveParticipationV1::Full,
        Some(TargetNumericalModeV1::IeeeStrict),
    ))
}

#[test]
fn partition16_keeps_logical_participants_and_explicit_hardware_wave64() {
    let tile = collective(16);
    let wave = TargetCapabilityRequirementV1::SubgroupSize(64);
    let graph = exact_requirement_graph(&BTreeSet::from([tile, wave])).unwrap();
    assert_eq!(graph[&tile], BTreeSet::from([wave]));
    assert!(!graph.contains_key(&TargetCapabilityRequirementV1::SubgroupSize(16)));
    let full_wave = collective(64);
    assert_eq!(
        exact_requirement_graph(&BTreeSet::from([full_wave, wave])).unwrap()[&full_wave],
        BTreeSet::from([wave])
    );
}

#[test]
fn partition_does_not_infer_hardware_from_logical_participants() {
    assert!(matches!(
        exact_requirement_graph(&BTreeSet::from([collective(16)])),
        Err(ProductionTargetCapabilityErrorV1::OmittedAxis {
            axis: OmittedCapabilityAxisV1::SubgroupWidth,
            ..
        })
    ));
    let widths = BTreeSet::from([
        collective(16),
        TargetCapabilityRequirementV1::SubgroupSize(16),
        TargetCapabilityRequirementV1::SubgroupSize(64),
    ]);
    let graph = exact_requirement_graph(&widths).unwrap();
    assert_eq!(
        graph[&collective(16)],
        BTreeSet::from([
            TargetCapabilityRequirementV1::SubgroupSize(16),
            TargetCapabilityRequirementV1::SubgroupSize(64),
        ])
    );
}

#[test]
fn partition_canonical_target_closure_roundtrips_without_wave16_authority() {
    let canonical = VerifiedCanonicalKernelIrV13::from_module(fixture::graph()).unwrap();
    let launch = ProductionTargetLaunchEvidenceV13::for_static_launches(&canonical, 0).unwrap();
    let closure = legalize_production_target_capabilities_v13(
        &canonical,
        0,
        &launch,
        ProductionAmdTargetProfileV1::Gfx950,
    )
    .unwrap();
    let mut collectives = 0;
    for record in closure.legalization_records() {
        assert_ne!(
            record.decision().requirement(),
            TargetCapabilityRequirementV1::SubgroupSize(16)
        );
        if let TargetCapabilityRequirementV1::Collective(requirement) =
            record.decision().requirement()
        {
            collectives += 1;
            assert_eq!(requirement.participants(), 16);
            assert_eq!(
                requirement.participation(),
                TargetCollectiveParticipationV1::Full
            );
            assert_eq!(
                requirement.numerical_mode(),
                Some(TargetNumericalModeV1::IeeeStrict)
            );
            assert_eq!(
                record.dependencies(),
                &[TargetCapabilityRequirementV1::SubgroupSize(64)]
            );
        }
    }
    assert_eq!(collectives, 2);
    let decoded =
        ProductionTargetCapabilityClosureV13::decode_canonical(closure.canonical_bytes(), &launch)
            .unwrap();
    assert_eq!(decoded.canonical_bytes(), closure.canonical_bytes());
    assert!(!closure.grants_launch_authority());
}

#[test]
fn explicit_hardware_wave16_requirement_remains_rejected() {
    let mut module = fixture::graph();
    module
        .required_capabilities
        .insert(TargetCapability::SubgroupSize(16));
    let error = VerifiedCanonicalKernelIrV13::from_module(module)
        .expect_err("wave64 cannot be canonically admitted with a declared subgroup size of 16");
    let fe2o3_kernel_ir::VerifiedCanonicalKernelIrErrorV13::Verification(errors) = error else {
        panic!("expected the existing canonical capability rejection: {error:?}");
    };
    assert!(errors.diagnostics().iter().any(|diagnostic| {
        diagnostic.code == fe2o3_kernel_ir::DiagnosticCode::InvalidCapability
            && diagnostic.message == "wave width 64 conflicts with the declared subgroup size"
    }));
}

#[test]
fn hardware_wave16_target_query_remains_unsupported() {
    let model = ProductionAmdTargetProfileV1::Gfx950.capability_model().unwrap();
    let requirement = TargetCapabilityRequirementV1::SubgroupSize(16);
    let decision = query_target_capability_v1(&model, requirement).unwrap();
    assert_eq!(decision.requirement(), requirement);
    assert_eq!(decision.outcome(), TargetCapabilityDecisionOutcomeV1::Unsupported);
}

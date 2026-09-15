use fe2o3_kernel_ir::{VerifiedCanonicalKernelIrV13, encode_module_v13};
use fe2o3_kir_sim::{
    AdmittedSimulationModuleV1, IncompleteExecutionCapabilityOperationV13,
    SimulationAdmissionErrorV1, SimulationCapabilityCoordinateKindV13,
    SimulationExecutionCapabilityFamilyV13, SimulationLimitsV1, SimulationLogicalCapabilityKindV13,
    SimulationRequestV1, SimulationTargetV1,
};

#[path = "../../fe2o3-kernel-ir/tests/support/numerical_policy_v13.rs"]
mod fixture;

#[test]
fn unused_numerical_policy_projects_with_exact_source_identity_and_no_authority() {
    let canonical = VerifiedCanonicalKernelIrV13::from_module(fixture::module()).unwrap();
    let identity = *canonical.identity();
    let admitted =
        AdmittedSimulationModuleV1::admit_v13(canonical, SimulationLimitsV1::default()).unwrap();
    assert_eq!(admitted.identity().digest(), identity.digest());
    assert_eq!(admitted.identity().wire_version(), 13);
    assert!(!admitted.grants_execution_authority());
    let receipt = admitted.capability_projection_receipt_v13().unwrap();
    let definition = receipt
        .coordinates()
        .iter()
        .find(|coordinate| {
            coordinate.execution_family()
                == Some(SimulationExecutionCapabilityFamilyV13::NumericalPolicyIssue)
                && matches!(
                    coordinate.coordinate_kind(),
                    SimulationCapabilityCoordinateKindV13::OperationResultDefinition { result: 0 }
                )
        })
        .unwrap();
    assert_eq!(definition.source(), Some(fixture::contract().source));
    assert_eq!(
        definition.logical_kind(),
        SimulationLogicalCapabilityKindV13::Execution
    );
    assert_eq!(definition.value(), fe2o3_kernel_ir::ValueId(1));
    assert_eq!(definition.operation(), Some(1));
    assert!(
        admitted.module().functions[0].body.as_ref().unwrap().blocks[0]
            .operations
            .is_empty()
    );
    let execution = admitted
        .simulate(
            &SimulationRequestV1::new("entry_kernel", [1, 1, 1], [1, 1, 1], vec![]),
            SimulationTargetV1::amdgpu_64(),
            SimulationLimitsV1::default(),
        )
        .unwrap();
    assert_eq!(execution.invocations_executed(), 1);
}

#[test]
fn numerical_policy_use_is_not_silently_erased_by_simulator_admission() {
    let canonical =
        VerifiedCanonicalKernelIrV13::from_module(fixture::used_policy_module()).unwrap();
    assert!(matches!(
        AdmittedSimulationModuleV1::admit_v13(canonical, SimulationLimitsV1::default()),
        Err(
            SimulationAdmissionErrorV1::IncompleteExecutionCapabilityV13(
                IncompleteExecutionCapabilityOperationV13::NumericalPolicyIssue
            )
        )
    ));
}

#[test]
fn numerical_policy_simulation_rejects_substituted_mode_type_and_provenance() {
    for (name, bad) in fixture::rejected_issuance_mutations() {
        assert!(
            VerifiedCanonicalKernelIrV13::from_module(bad.clone()).is_err(),
            "{name}"
        );
        if let Ok(bytes) = encode_module_v13(&bad) {
            assert!(
                VerifiedCanonicalKernelIrV13::from_canonical_bytes(bytes).is_err(),
                "{name}"
            );
        }
    }
}

#[test]
fn issuing_strict_policy_does_not_waive_a_retained_numerical_requirement() {
    use fe2o3_kernel_ir::{
        ExecutionCapabilityRequirementV1, NumericalModeV1, ScalarType, TargetCapability,
    };
    for mode in [
        NumericalModeV1::AllowContraction,
        NumericalModeV1::AllowApproximation,
    ] {
        let requirement = ExecutionCapabilityRequirementV1::Numerical {
            value_type: ScalarType::F32,
            mode,
        };
        for with_policy in [false, true] {
            let module = fixture::floating_point_module(with_policy, mode);
            assert!(
                module
                    .required_capabilities
                    .contains(&TargetCapability::Execution(requirement.clone()))
            );
            let canonical = VerifiedCanonicalKernelIrV13::from_module(module).unwrap();
            assert!(matches!(
                AdmittedSimulationModuleV1::admit_v13(canonical, SimulationLimitsV1::default()),
                Err(SimulationAdmissionErrorV1::UnsupportedExecutionCapability(found)) if found == requirement
            ));
        }
    }
}

#[test]
fn unused_policy_preserves_simulator_fp_graph_and_numerical_requirements() {
    use fe2o3_kernel_ir::NumericalModeV1;

    let admit = |with_policy| {
        AdmittedSimulationModuleV1::admit_v13(
            VerifiedCanonicalKernelIrV13::from_module(fixture::floating_point_module(
                with_policy,
                NumericalModeV1::StrictIeee,
            ))
            .unwrap(),
            SimulationLimitsV1::default(),
        )
        .unwrap()
    };
    let baseline = admit(false);
    let policy = admit(true);
    assert_eq!(baseline.module(), policy.module());
    assert_eq!(
        baseline.module().required_capabilities,
        policy.module().required_capabilities
    );
    assert_eq!(
        policy.module().functions[0].body.as_ref().unwrap().blocks[0]
            .operations
            .len(),
        5
    );
    assert_ne!(baseline.identity(), policy.identity());
    for admitted in [&baseline, &policy] {
        assert!(!admitted.grants_execution_authority());
        let execution = admitted
            .simulate(
                &SimulationRequestV1::new("entry_kernel", [1, 1, 1], [1, 1, 1], vec![]),
                SimulationTargetV1::amdgpu_64(),
                SimulationLimitsV1::default(),
            )
            .unwrap();
        assert_eq!(execution.invocations_executed(), 1);
    }
}

#[test]
fn physically_identical_tokens_keep_distinct_policy_and_provenance_identities() {
    let admit = |module| {
        AdmittedSimulationModuleV1::admit_v13(
            VerifiedCanonicalKernelIrV13::from_module(module).unwrap(),
            SimulationLimitsV1::default(),
        )
        .unwrap()
    };
    let original = admit(fixture::module());
    for change_policy in [false, true] {
        let mut module = fixture::module();
        if change_policy {
            let fe2o3_kernel_ir::ExecutionCapabilityOperationV1::NumericalPolicyIssue {
                policy,
                ..
            } = &mut fixture::contract_mut(&mut module).operation
            else {
                unreachable!()
            };
            *policy = fixture::identity(99);
            fixture::authority_mut(&mut module).role =
                fe2o3_kernel_ir::ExecutionCapabilityRoleV1::NumericalPolicy {
                    policy: fixture::identity(99),
                    mode: fe2o3_kernel_ir::NumericalModeV1::StrictIeee,
                };
        } else {
            fixture::contract_mut(&mut module).provenance.frontend_unit[0] ^= 1;
            fixture::authority_mut(&mut module).provenance.frontend_unit[0] ^= 1;
        }
        let changed = admit(module);
        assert_eq!(changed.module(), original.module());
        assert_ne!(changed.identity(), original.identity());
        assert!(!changed.grants_execution_authority());
    }
}

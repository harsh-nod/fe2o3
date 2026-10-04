use fe2o3_rustc_front::{
    ConstantSwitchCaseV1, ConstantSwitchV1, DeadBranchContextV1, FixedWidthIntegerV1,
    MonomorphizationDeadEvidenceV1, prove_constant_switch_v1,
};
use fe2o3_verifier::{
    MonomorphizationDeadBindingErrorV1, MonomorphizationDeadClaimV1,
    reconcile_monomorphization_dead_evidence_v1,
};

fn context(function: u8, cfg: u8, source: u8, target: u8) -> DeadBranchContextV1 {
    DeadBranchContextV1::new([function; 32], [cfg; 32], [source; 32], [target; 32]).unwrap()
}

fn evidence(context: DeadBranchContextV1, discriminant: u128) -> MonomorphizationDeadEvidenceV1 {
    let ty = |bits| FixedWidthIntegerV1::new(32, false, bits).unwrap();
    let switch = ConstantSwitchV1::new(
        0,
        ty(discriminant).into(),
        vec![
            ConstantSwitchCaseV1::new(ty(0), 1),
            ConstantSwitchCaseV1::new(ty(1), 2),
        ],
        3,
    )
    .unwrap();
    let decision = prove_constant_switch_v1(1, &switch).unwrap();
    MonomorphizationDeadEvidenceV1::new(1, context, vec![decision]).unwrap()
}

#[test]
fn exact_observation_and_claim_have_one_canonical_inert_binding() {
    let observation = evidence(context(1, 2, 3, 4), 1);
    let claim = MonomorphizationDeadClaimV1::from_evidence(&observation);
    let binding = reconcile_monomorphization_dead_evidence_v1(&observation, claim).unwrap();
    let repeated = reconcile_monomorphization_dead_evidence_v1(&observation, claim).unwrap();

    assert_eq!(binding, repeated);
    assert_eq!(binding.policy_version(), 1);
    assert_eq!(binding.context(), observation.context());
    assert_eq!(binding.evidence_identity(), observation.identity());
    assert_eq!(
        binding.evidence_byte_len() as usize,
        observation.canonical_bytes().len()
    );
    assert_ne!(binding.binding_identity().as_bytes(), &[0; 32]);
    assert!(!claim.grants_compiler_authority());
    assert!(!binding.grants_compiler_authority());
    assert!(!binding.grants_panic_exclusion_authority());
    assert!(!binding.grants_address_space_exclusion_authority());
    assert!(!binding.grants_load_authority());
    assert!(!binding.grants_launch_authority());
}

#[test]
fn substituted_function_cfg_source_and_target_identities_fail_distinctly() {
    let observation = evidence(context(1, 2, 3, 4), 1);
    for (substituted, expected) in [
        (
            context(9, 2, 3, 4),
            MonomorphizationDeadBindingErrorV1::FunctionIdentityMismatch,
        ),
        (
            context(1, 9, 3, 4),
            MonomorphizationDeadBindingErrorV1::CfgIdentityMismatch,
        ),
        (
            context(1, 2, 9, 4),
            MonomorphizationDeadBindingErrorV1::SourceIdentityMismatch,
        ),
        (
            context(1, 2, 3, 9),
            MonomorphizationDeadBindingErrorV1::TargetIdentityMismatch,
        ),
    ] {
        let claim = MonomorphizationDeadClaimV1::new(1, substituted, observation.identity());
        assert_eq!(
            reconcile_monomorphization_dead_evidence_v1(&observation, claim),
            Err(expected)
        );
    }
}

#[test]
fn substituted_decisions_and_policy_version_drift_fail_closed() {
    let observation = evidence(context(1, 2, 3, 4), 1);
    let changed_decision = evidence(context(1, 2, 3, 4), 0);
    assert_eq!(
        reconcile_monomorphization_dead_evidence_v1(
            &observation,
            MonomorphizationDeadClaimV1::from_evidence(&changed_decision),
        ),
        Err(MonomorphizationDeadBindingErrorV1::EvidenceIdentityMismatch)
    );

    let drifted =
        MonomorphizationDeadClaimV1::new(2, observation.context(), observation.identity());
    assert_eq!(
        reconcile_monomorphization_dead_evidence_v1(&observation, drifted),
        Err(MonomorphizationDeadBindingErrorV1::PolicyVersionMismatch {
            observed: 1,
            claimed: 2,
        })
    );
}

#[test]
fn binding_validation_rejects_cross_observation_substitution() {
    let first = evidence(context(1, 2, 3, 4), 1);
    let second = evidence(context(1, 2, 4, 4), 1);
    let first = reconcile_monomorphization_dead_evidence_v1(
        &first,
        MonomorphizationDeadClaimV1::from_evidence(&first),
    )
    .unwrap();
    let second = reconcile_monomorphization_dead_evidence_v1(
        &second,
        MonomorphizationDeadClaimV1::from_evidence(&second),
    )
    .unwrap();

    assert_eq!(
        first.validate_against(&second),
        Err(MonomorphizationDeadBindingErrorV1::BindingMismatch)
    );
}

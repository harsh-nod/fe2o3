use fe2o3_rustc_front::{
    CONSTANT_FOLD_POLICY_VERSION_V1, ConstantFoldBinaryOpV1, ConstantFoldFailureV1,
    ConstantFoldInputV1, ConstantSwitchCaseV1, ConstantSwitchV1, DeadBranchContextV1,
    DeadBranchDecisionV1, FixedWidthIntegerV1, MonomorphizationDeadEvidenceErrorV1,
    MonomorphizationDeadEvidenceV1, fold_binary_v1, prove_constant_switch_v1,
};

fn unsigned(width: u16, bits: u128) -> FixedWidthIntegerV1 {
    FixedWidthIntegerV1::new(width, false, bits).unwrap()
}

fn signed(width: u16, bits: u128) -> FixedWidthIntegerV1 {
    FixedWidthIntegerV1::new(width, true, bits).unwrap()
}

fn same_type(value: FixedWidthIntegerV1, bits: u128) -> FixedWidthIntegerV1 {
    FixedWidthIntegerV1::new(value.width(), value.is_signed(), bits).unwrap()
}

fn context(seed: u8) -> DeadBranchContextV1 {
    DeadBranchContextV1::new([seed; 32], [seed + 1; 32], [seed + 2; 32]).unwrap()
}

fn decision(block: u32, value: FixedWidthIntegerV1) -> DeadBranchDecisionV1 {
    prove_constant_switch_v1(
        CONSTANT_FOLD_POLICY_VERSION_V1,
        &ConstantSwitchV1::new(
            block,
            value.into(),
            vec![
                ConstantSwitchCaseV1::new(same_type(value, 0), block + 1),
                ConstantSwitchCaseV1::new(same_type(value, 1), block + 2),
            ],
            block + 3,
        )
        .unwrap(),
    )
    .unwrap()
}

#[test]
fn checked_fixed_width_policy_folds_only_exact_values() {
    let add = fold_binary_v1(
        CONSTANT_FOLD_POLICY_VERSION_V1,
        ConstantFoldBinaryOpV1::Add,
        unsigned(8, 40).into(),
        unsigned(8, 2).into(),
    )
    .unwrap();
    assert_eq!(add, unsigned(8, 42));

    let signed_compare = fold_binary_v1(
        CONSTANT_FOLD_POLICY_VERSION_V1,
        ConstantFoldBinaryOpV1::LessThan,
        signed(8, 0xff).into(),
        signed(8, 1).into(),
    )
    .unwrap();
    assert_eq!(signed_compare, FixedWidthIntegerV1::boolean(true));

    let arithmetic_shift = fold_binary_v1(
        CONSTANT_FOLD_POLICY_VERSION_V1,
        ConstantFoldBinaryOpV1::ShiftRight,
        signed(8, 0xfe).into(),
        signed(8, 1).into(),
    )
    .unwrap();
    assert_eq!(arithmetic_shift, signed(8, 0xff));
}

#[test]
fn unknown_poison_and_target_dependent_values_fail_closed() {
    for (input, expected) in [
        (ConstantFoldInputV1::Unknown, ConstantFoldFailureV1::Unknown),
        (ConstantFoldInputV1::Poison, ConstantFoldFailureV1::Poison),
        (
            ConstantFoldInputV1::TargetDependent,
            ConstantFoldFailureV1::TargetDependent,
        ),
    ] {
        let switch = ConstantSwitchV1::new(
            0,
            input,
            vec![ConstantSwitchCaseV1::new(unsigned(32, 0), 1)],
            2,
        )
        .unwrap();
        assert_eq!(
            prove_constant_switch_v1(CONSTANT_FOLD_POLICY_VERSION_V1, &switch),
            Err(MonomorphizationDeadEvidenceErrorV1::Fold(expected))
        );
    }
}

#[test]
fn widths_overflow_division_and_shifts_fail_closed() {
    assert_eq!(
        FixedWidthIntegerV1::new(24, false, 0),
        Err(ConstantFoldFailureV1::UnsupportedIntegerWidth(24))
    );
    assert_eq!(
        FixedWidthIntegerV1::new(8, false, 256),
        Err(ConstantFoldFailureV1::IntegerOutOfRange {
            width: 8,
            bits: 256,
        })
    );
    assert_eq!(
        fold_binary_v1(
            1,
            ConstantFoldBinaryOpV1::Add,
            unsigned(8, 255).into(),
            unsigned(8, 1).into(),
        ),
        Err(ConstantFoldFailureV1::Overflow)
    );
    assert_eq!(
        fold_binary_v1(
            1,
            ConstantFoldBinaryOpV1::Divide,
            signed(8, 0x80).into(),
            signed(8, 0xff).into(),
        ),
        Err(ConstantFoldFailureV1::Overflow)
    );
    assert_eq!(
        fold_binary_v1(
            1,
            ConstantFoldBinaryOpV1::Divide,
            unsigned(32, 1).into(),
            unsigned(32, 0).into(),
        ),
        Err(ConstantFoldFailureV1::DivisionByZero)
    );
    assert_eq!(
        fold_binary_v1(
            1,
            ConstantFoldBinaryOpV1::ShiftLeft,
            unsigned(32, 1).into(),
            unsigned(32, 32).into(),
        ),
        Err(ConstantFoldFailureV1::InvalidShift {
            width: 32,
            amount: 32,
        })
    );
    assert_eq!(
        fold_binary_v1(
            1,
            ConstantFoldBinaryOpV1::ShiftLeft,
            unsigned(128, 1 << 127).into(),
            unsigned(128, 1).into(),
        ),
        Err(ConstantFoldFailureV1::Overflow)
    );
}

#[test]
fn switch_proof_selects_one_edge_and_canonicalizes_dead_successors() {
    let switch = ConstantSwitchV1::new(
        7,
        unsigned(32, 1).into(),
        vec![
            ConstantSwitchCaseV1::new(unsigned(32, 2), 11),
            ConstantSwitchCaseV1::new(unsigned(32, 0), 9),
            ConstantSwitchCaseV1::new(unsigned(32, 1), 10),
        ],
        11,
    )
    .unwrap();
    let proof = prove_constant_switch_v1(1, &switch).unwrap();
    assert_eq!(proof.branch_block(), 7);
    assert_eq!(proof.selected_successor(), 10);
    assert_eq!(proof.dead_successors(), &[9, 11]);
}

#[test]
fn evidence_is_canonical_and_every_identity_axis_matters() {
    let first = MonomorphizationDeadEvidenceV1::new(
        1,
        context(1),
        vec![decision(10, unsigned(32, 1)), decision(0, unsigned(32, 0))],
    )
    .unwrap();
    let reordered = MonomorphizationDeadEvidenceV1::new(
        1,
        context(1),
        vec![decision(0, unsigned(32, 0)), decision(10, unsigned(32, 1))],
    )
    .unwrap();
    assert_eq!(first, reordered);
    assert_eq!(first.canonical_bytes(), reordered.canonical_bytes());
    assert_eq!(first.identity(), reordered.identity());

    for changed in [
        DeadBranchContextV1::new([9; 32], [2; 32], [3; 32]).unwrap(),
        DeadBranchContextV1::new([1; 32], [9; 32], [3; 32]).unwrap(),
        DeadBranchContextV1::new([1; 32], [2; 32], [9; 32]).unwrap(),
    ] {
        let substituted =
            MonomorphizationDeadEvidenceV1::new(1, changed, first.decisions().to_vec()).unwrap();
        assert_ne!(substituted.identity(), first.identity());
        assert_ne!(substituted.canonical_bytes(), first.canonical_bytes());
    }
}

#[test]
fn policy_version_drift_is_rejected_and_claims_remain_inert() {
    assert_eq!(
        MonomorphizationDeadEvidenceV1::new(
            CONSTANT_FOLD_POLICY_VERSION_V1 + 1,
            context(1),
            vec![],
        ),
        Err(MonomorphizationDeadEvidenceErrorV1::Fold(
            ConstantFoldFailureV1::UnsupportedPolicyVersion(2),
        ))
    );
    assert_eq!(
        fold_binary_v1(
            2,
            ConstantFoldBinaryOpV1::Equal,
            unsigned(8, 1).into(),
            unsigned(8, 1).into(),
        ),
        Err(ConstantFoldFailureV1::UnsupportedPolicyVersion(2))
    );

    let evidence =
        MonomorphizationDeadEvidenceV1::new(1, context(1), vec![decision(0, unsigned(32, 0))])
            .unwrap();
    assert!(!evidence.grants_compiler_authority());
    assert!(!evidence.grants_panic_exclusion_authority());
    assert!(!evidence.grants_address_space_exclusion_authority());
}

#[test]
fn cross_width_switch_cases_and_zero_identities_are_rejected() {
    assert_eq!(
        ConstantSwitchV1::new(
            0,
            unsigned(32, 0).into(),
            vec![ConstantSwitchCaseV1::new(unsigned(64, 0), 1)],
            2,
        ),
        Err(MonomorphizationDeadEvidenceErrorV1::Fold(
            ConstantFoldFailureV1::TypeMismatch,
        ))
    );
    assert_eq!(
        DeadBranchContextV1::new([1; 32], [0; 32], [3; 32]),
        Err(MonomorphizationDeadEvidenceErrorV1::ZeroIdentity {
            field: "CFG identity",
        })
    );
}

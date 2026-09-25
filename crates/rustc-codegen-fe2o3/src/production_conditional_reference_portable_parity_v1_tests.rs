//! Adapter parity only. These records do not authenticate source or proof.
use super::*;
use fe2o3_kernel_ir::{
    CanonicalKernelIrVerificationResourceBudgetV1 as Budget, CanonicalKernelIrWorkBudgetV1 as Work,
};
use fe2o3_verifier::conditional_reference_v1::{
    self as shared, ConditionalReferenceErrorV1 as SharedError,
};
use fe2o3_verifier::portable_reference_v1::{
    ReferenceAssignmentV1, ReferenceBlockV1, ReferencePlaceV1, ReferenceTerminatorV1,
    ReferenceValueV1, ReplayedCpuEffectsV1, ReplayedCpuValueV1,
};

fn ir() -> ReferenceEffectIrV1 {
    ReferenceEffectIrV1 {
        argument_count: 1,
        local_count: 3,
        relations: vec![ReferenceArgumentRelationV1::ScalarInput {
            argument: 0,
            scalar: ReferenceScalarTypeV1::U32,
        }]
        .into_boxed_slice(),
        blocks: vec![ReferenceBlockV1 {
            block: 0,
            assignments: vec![ReferenceAssignmentV1 {
                statement: 0,
                destination: ReferencePlaceV1 {
                    local: 2,
                    projection: Box::default(),
                },
                value: ReferenceValueV1::Use(
                    crate::reference_effect_v1::ReferenceOperandV1::Constant(
                        ReferenceConstantV1::Scalar {
                            scalar: ReferenceScalarTypeV1::U32,
                            bits: 7,
                        },
                    ),
                ),
            }]
            .into_boxed_slice(),
            terminator: ReferenceTerminatorV1::Return,
        }]
        .into_boxed_slice(),
        loop_summaries: Box::default(),
        observable_output_effects: Box::default(),
    }
}

#[test]
fn conditional_portable_conversion_adapter_preserves_values_and_all_error_variants() {
    let ir = ir();
    for expression in [
        ReferenceEffectExpressionV1::KernelScalarArgument { argument: 0 },
        ReferenceEffectExpressionV1::Constant(ReferenceConstantV1::Scalar {
            scalar: ReferenceScalarTypeV1::U32,
            bits: 7,
        }),
        ReferenceEffectExpressionV1::Constant(ReferenceConstantV1::ZeroSized),
        ReferenceEffectExpressionV1::InputLength {
            reference_argument: 0,
        },
        ReferenceEffectExpressionV1::InputLoad {
            reference_argument: 0,
            index: Box::new(ReferenceEffectExpressionV1::PointCoordinate { axis: 0 }),
        },
    ] {
        for expected in [ReferenceScalarTypeV1::U32, ReferenceScalarTypeV1::F32] {
            let backend = reference_expression_inner_checked_v2(&ir, &expression, expected, None)
                .map_err(|error| error.to_string());
            let portable = shared::conversion::reference_expression_inner_checked_v2(
                &ir,
                &expression,
                expected,
                None,
            )
            .map_err(|error| error.to_string());
            assert_eq!(backend, portable);
        }
    }
    for error in [
        SharedError::UnsupportedReference("exact reference detail"),
        SharedError::UnsupportedGpuIndex("exact index detail"),
        SharedError::SemanticExpression(
            fe2o3_pliron::ProductionSemanticExpressionErrorV2::TypeMismatch,
        ),
        SharedError::Subjects("exact subjects detail".into()),
        SharedError::ProofExecution("exact resource detail".into()),
    ] {
        let text = error.to_string();
        let backend = ProductionReferenceEffectJoinErrorV2::from(error);
        assert_eq!(text, backend.to_string());
        assert!(matches!(
            backend,
            ProductionReferenceEffectJoinErrorV2::UnsupportedReference(_)
                | ProductionReferenceEffectJoinErrorV2::UnsupportedGpuIndex(_)
                | ProductionReferenceEffectJoinErrorV2::SemanticExpression(_)
                | ProductionReferenceEffectJoinErrorV2::Subjects(_)
                | ProductionReferenceEffectJoinErrorV2::ProofExecution(_)
        ));
    }
}

#[test]
fn conditional_portable_read_adapter_preserves_results_debits_floors_and_first_denials() {
    let ir = ir();
    for mutation in [false, true] {
        let replay = ReplayedCpuEffectsV1 {
            writes: vec![],
            values: vec![ReplayedCpuValueV1 {
                block: 0,
                statement: u32::from(mutation),
                expression: ReferenceEffectExpressionV1::Constant(ReferenceConstantV1::Scalar {
                    scalar: ReferenceScalarTypeV1::U32,
                    bits: 7,
                }),
            }],
            bounds: vec![],
        };
        let run = |portable_entry: bool, limit: usize, denied: bool| {
            let mut work = Work::new(limit);
            let mut budget = Budget::new(&mut work, 31);
            budget.charge_work(17).unwrap();
            budget.reserve_storage(31).unwrap();
            let account = budget.work_ledger_identity_v1();
            if denied {
                assert!(budget.charge_work(usize::MAX).is_err());
                assert!(budget.reserve_storage(1).is_err());
            }
            let result = if portable_entry {
                shared::read_premises::check_replay_v1::<SharedError>(
                    &ir,
                    &replay,
                    &mut budget,
                    |_, _| panic!("constant cannot read"),
                )
                .map_err(|error| error.to_string())
            } else {
                conditional_source_v1::read_premises_v1::check_replay_v1(
                    &ir,
                    &replay,
                    &mut budget,
                    |_, _| panic!("constant cannot read"),
                )
                .map_err(|error| error.to_string())
            };
            assert!(budget.work_ledger_identity_v1() == account);
            (
                result,
                budget.work(),
                budget.storage(),
                budget.peak_storage(),
                budget.failed_work(),
                budget.failed_storage(),
            )
        };
        let measured = run(false, usize::MAX, false);
        assert_eq!(measured.0.is_ok(), !mutation);
        for limit in [usize::MAX, measured.1, measured.1 - 1] {
            for denied in [false, true] {
                let observed = run(false, limit, denied);
                assert_eq!(observed, run(true, limit, denied));
                assert_eq!((observed.2, observed.3), (31, 31));
                if denied {
                    assert_eq!(observed.4, Some(usize::MAX));
                    assert_eq!(observed.5, Some(32));
                }
            }
        }
    }
}

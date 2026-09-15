//! Unsupported-symbolic-domain tests, with an actual observable store baseline.
//! These raw phase records are not canonical lifecycle or source proof owners.
use super::super::{
    FinalKirOutputEquivalenceErrorV1, extract_kernel_effects, parameter_value, render_kernel_lemma,
};
use super::{execution_provenance, kernel_module};
use fe2o3_kernel_ir::{
    ExecutionSafetyObligationsV1, ExecutionTypeIdentityV1, Operation, OperationKind, PhaseKeyV1,
    PhaseLoanStateV1, PhaseOperationSourceV1, ReusablePhaseOpV1, ReusablePhaseOperationV1,
    ReusablePhaseTokenRoleV1, ReusablePhaseTokenTypeV1, ScalarType, Type, ValueDef, ValueId,
};
use std::collections::BTreeMap;

fn token(state: PhaseLoanStateV1) -> Type {
    let token = ReusablePhaseTokenTypeV1 {
        provenance: execution_provenance(),
        phase: PhaseKeyV1::from_untrusted_bytes([7; 32]),
        owner_source: ExecutionTypeIdentityV1::new([8; 32]),
        owner_anchor_epoch: [9; 32],
        outer_brand: [10; 32],
        phase_brand: [11; 32],
        initial_epoch: [12; 32],
        role: ReusablePhaseTokenRoleV1::OwnerLoan(state),
    };
    assert!(token.is_complete());
    Type::ReusablePhaseToken(token)
}

fn phase(results: Vec<ValueDef>) -> Operation {
    Operation::new(
        results,
        OperationKind::ReusablePhase(ReusablePhaseOpV1 {
            // Known physical parameters prevent an unrelated missing-operand failure.
            operands: vec![ValueId(0), ValueId(1)],
            operation: ReusablePhaseOperationV1::End { storage_count: 0 },
            provenance: execution_provenance(),
            source: PhaseOperationSourceV1::WrapperEnd {
                phase: PhaseKeyV1::from_untrusted_bytes([7; 32]),
                wrapper_normal_target: 0,
                source_protocol: [8; 32],
            },
            obligations: ExecutionSafetyObligationsV1::from_bits(
                ReusablePhaseOperationV1::End { storage_count: 0 }.required_obligations(),
            ),
        }),
    )
}

#[test]
fn phase_operation_cannot_be_skipped_beside_real_observable_store() {
    let module = kernel_module(None);
    let effects = extract_kernel_effects(&module.functions[0], &BTreeMap::new(), None).unwrap();
    assert!(effects.output_roots.contains(&1));
    assert_eq!(effects.outcomes.len(), 1);
    assert_eq!(effects.outcomes[0].memories[&1].writes.len(), 1);
    for position in [0, 1] {
        for results in [
            vec![],
            vec![ValueDef::new(ValueId(99), Type::Scalar(ScalarType::U32))],
        ] {
            let mut module = kernel_module(None);
            let operation = phase(results);
            assert!(!operation.has_complete_effect_summary());
            module.functions[0].body.as_mut().unwrap().blocks[0]
                .operations
                .insert(position, operation);
            assert!(matches!(
                extract_kernel_effects(&module.functions[0], &BTreeMap::new(), None),
                Err(FinalKirOutputEquivalenceErrorV1::UnsupportedOperation)
            ));
        }
    }
}

#[test]
fn phase_parameter_is_not_an_opaque_or_unit_symbolic_value() {
    assert!(parameter_value(0, &Type::Scalar(ScalarType::U32)).is_ok());
    for state in [
        PhaseLoanStateV1::Active,
        PhaseLoanStateV1::Sealed,
        PhaseLoanStateV1::Returned,
    ] {
        assert!(matches!(
            parameter_value(0, &token(state)),
            Err(FinalKirOutputEquivalenceErrorV1::UnsupportedScalarType)
        ));
    }
}

#[test]
fn phase_parameter_cannot_be_erased_from_rendered_lemma() {
    let module = kernel_module(None);
    let mut effects = extract_kernel_effects(&module.functions[0], &BTreeMap::new(), None).unwrap();
    let mut positive = String::new();
    render_kernel_lemma(&mut positive, 0, &effects, &effects).unwrap();
    assert!(positive.contains("proof fn fe2o3_final_kir_output_equivalence_0"));
    assert!(positive.contains("output_query_0"));
    effects
        .parameter_types
        .push(token(PhaseLoanStateV1::Returned));
    let mut rejected = String::from("unchanged-prefix");
    assert!(matches!(
        render_kernel_lemma(&mut rejected, 0, &effects, &effects),
        Err(FinalKirOutputEquivalenceErrorV1::UnsupportedScalarType)
    ));
    assert_eq!(rejected, "unchanged-prefix");
}

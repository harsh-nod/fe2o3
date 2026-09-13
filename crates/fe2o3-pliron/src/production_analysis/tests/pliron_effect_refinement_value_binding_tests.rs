use super::*;
use crate::{
    PlironAtomicTargetCapabilityV1, PlironAtomicTargetContextV1,
    require_production_pliron_checks_with_atomic_target_before_lowering_v2,
};
use dialect_kernel::{
    AtomicOrderingAttr, AtomicScopeAttr, SemanticScalarKindAttr, SemanticTypedConstantOp,
    SemanticTypedScalarV1,
};
use pliron::{
    linked_list::ContainsLinkedList,
    operation::{Operation, verify_operation},
};

#[derive(Clone, Copy)]
enum Rhs {
    Exact,
    Missing,
    EquivalentOther,
}

#[test]
fn effect_claims_bind_the_actual_write_operand_not_an_equivalent_expression() {
    use KernelCheckStatusV1::{Incomplete, Rejected};
    const MISSING: &str = "the matched write has no retained scalar RHS";
    const MISMATCH: &str = "the claimed GPU value is not the matched write's actual SSA operand";
    const RMW: &str =
        "an atomic read-modify-write update operand does not establish the final stored value";
    for typed in [false, true] {
        for (kind, rhs, expected) in [
            (AccessKindAttr::Write, Rhs::Exact, None),
            (AccessKindAttr::AtomicWrite, Rhs::Exact, None),
            (
                AccessKindAttr::Write,
                Rhs::Missing,
                Some((Incomplete, MISSING)),
            ),
            (
                AccessKindAttr::Write,
                Rhs::EquivalentOther,
                Some((Rejected, MISMATCH)),
            ),
            (
                AccessKindAttr::AtomicReadModifyWrite,
                Rhs::Exact,
                Some((Incomplete, RMW)),
            ),
            (
                AccessKindAttr::AtomicReadModifyWrite,
                Rhs::EquivalentOther,
                Some((Rejected, MISMATCH)),
            ),
        ] {
            let context = &mut setup();
            let function = effect_function(
                context,
                FormulaCase::Equivalent,
                true,
                true,
                ExtentCase::Static,
                false,
                false,
            );
            let operations = function
                .get_entry_block(context)
                .deref(context)
                .iter(context)
                .collect::<Vec<_>>();
            let store = RankedAccessOp::from_operation(
                *operations
                    .iter()
                    .find(|op| Operation::is_op::<RankedAccessOp>(**op, context))
                    .unwrap(),
            );
            let mut contract_position = operations
                .iter()
                .position(|op| Operation::is_op::<RequireEffectRefinementOp>(*op, context))
                .unwrap();
            let contract = RequireEffectRefinementOp::from_operation(operations[contract_position]);
            let mut actual = contract.gpu_value(context);
            let mut reference = contract.reference_value(context);
            if typed {
                let scalar =
                    SemanticTypedScalarV1::new(SemanticScalarKindAttr::UnsignedInteger, 32)
                        .unwrap();
                let constants = [
                    SemanticTypedConstantOp::new(context, 7, scalar),
                    SemanticTypedConstantOp::new(context, 7, scalar),
                ];
                for constant in &constants {
                    constant
                        .get_operation()
                        .insert_before(context, store.get_operation());
                }
                actual = constants[0].result(context);
                reference = constants[1].result(context);
                Operation::replace_operand(contract.get_operation(), context, 8, actual);
                Operation::replace_operand(contract.get_operation(), context, 9, reference);
                Operation::replace_operand(store.get_operation(), context, 2, actual);
                contract_position += 2;
            }
            assert_ne!(actual, reference);
            assert_eq!(store.stored_value(context), Some(actual));
            match rhs {
                Rhs::Exact => {}
                Rhs::Missing => {
                    Operation::remove_operand(store.get_operation(), context, 2);
                }
                Rhs::EquivalentOther => {
                    Operation::replace_operand(store.get_operation(), context, 2, reference)
                }
            }
            store.set_attr_kernel_access_kind(context, kind);
            if kind.is_atomic() {
                store.set_attr_kernel_atomic_ordering(
                    context,
                    if kind == AccessKindAttr::AtomicWrite {
                        AtomicOrderingAttr::Release
                    } else {
                        AtomicOrderingAttr::AcquireRelease
                    },
                );
                store.set_attr_kernel_atomic_scope(context, AtomicScopeAttr::Device);
            }
            verify_operation(function.get_operation(), context).unwrap();
            let report = run_pliron_effect_refinement_check_v1(context, &function);
            let target = PlironAtomicTargetContextV1::new([PlironAtomicTargetCapabilityV1::new(
                32,
                MemorySpaceAttr::Global,
                AtomicScopeAttr::Device,
            )
            .unwrap()])
            .unwrap();
            let pipeline = require_production_pliron_checks_with_atomic_target_before_lowering_v2(
                context, &function, &target,
            );
            let pipeline_effect = match (expected, pipeline) {
                (None, Ok(pipeline)) => {
                    assert!(pipeline.atomics().is_clean());
                    assert!(!pipeline.grants_compiler_refinement_authority());
                    assert!(!pipeline.grants_artifact_or_launch_authority());
                    pipeline.semantics().effect_refinement().clone()
                }
                (Some(_), Err(ProductionPlironPreloweringErrorV2::Semantic(error))) => {
                    error.report().effect_refinement().clone()
                }
                other => panic!("expected the mandatory effect gate: {other:?}"),
            };
            assert_eq!(pipeline_effect, report);
            assert_eq!(report.contract_count(), 1);
            assert!(!report.grants_compiler_refinement_authority());
            assert!(!report.grants_artifact_or_launch_authority());
            if let Some((status, reason)) = expected {
                assert_eq!(report.status(), status, "{report:?}");
                assert_eq!(report.proved_contract_count(), 0);
                assert!(!report.all_declared_effects_are_proved());
                let [finding] = report.findings() else {
                    panic!("{report:?}")
                };
                let (obligation, location, actual_reason) = match finding {
                    PlironEffectRefinementFindingV1::ReferenceProofIncomplete {
                        obligation,
                        location,
                        reason,
                    } if status == Incomplete => (obligation, location, reason),
                    PlironEffectRefinementFindingV1::ReferenceProofRejected {
                        obligation,
                        location,
                        reason,
                    } if status == Rejected => (obligation, location, reason),
                    _ => panic!("{report:?}"),
                };
                assert_eq!(*obligation, OBLIGATION);
                assert_eq!(location.block(), 0);
                assert_eq!(location.operation(), contract_position);
                assert_eq!(*actual_reason, reason);
                assert!(finding.to_string().contains("FE2O3-EFFECT-007"));
            } else {
                assert!(report.is_clean(), "{report:?}");
                assert_eq!(report.proved_contract_count(), 1);
                assert!(report.all_declared_effects_are_proved());
            }
        }
    }
}

use super::*;

#[derive(Clone, Copy, Debug)]
enum DivisorCase {
    GuardedNe,
    GuardedEq,
    Unguarded,
    WrongGuard,
    WrongPolarity,
    WrongDenominator,
    BeforeGuard,
    Zero,
    Nonzero,
}

fn arithmetic_source(
    case: DivisorCase,
    operation: SemanticBinaryOpV1,
) -> ProductionPreRankedKirOwnerV1 {
    let (ssa, launch) = fixture_with_blocks_and_symbol(
        Fixture::ElidedBounds,
        false,
        |_, _| {
            let denominator = match case {
                DivisorCase::Zero | DivisorCase::WrongDenominator => constant(U64, 0, 8),
                DivisorCase::Nonzero => constant(U64, 7, 8),
                _ => value(2, U64),
            };
            let arithmetic = assignment(
                5,
                U64,
                SemanticRvalueKindV1::Binary {
                    operation,
                    left: value(3, U64),
                    right: denominator.clone(),
                },
            );
            let mut statements = vec![
                assignment(
                    2,
                    U64,
                    SemanticRvalueKindV1::Unary {
                        operation: SemanticUnaryOpV1::PointerMetadata,
                        operand: value(1, SLICE_REF),
                    },
                ),
                assignment(3, U64, SemanticRvalueKindV1::Use(constant(U64, 84, 8))),
            ];
            let guarded = !matches!(
                case,
                DivisorCase::Unguarded | DivisorCase::Zero | DivisorCase::Nonzero
            );
            if guarded {
                statements.push(assignment(
                    4,
                    BOOL,
                    SemanticRvalueKindV1::Binary {
                        operation: if matches!(case, DivisorCase::GuardedEq) {
                            SemanticBinaryOpV1::Equal
                        } else {
                            SemanticBinaryOpV1::NotEqual
                        },
                        left: value(
                            if matches!(case, DivisorCase::WrongGuard) {
                                3
                            } else {
                                2
                            },
                            U64,
                        ),
                        right: constant(U64, 0, 8),
                    },
                ));
            }
            let mut body = Vec::new();
            if matches!(case, DivisorCase::BeforeGuard) {
                statements.push(arithmetic);
            } else {
                body.push(arithmetic);
            }
            let terminator = if guarded {
                SemanticTerminatorKindV1::Assert {
                    condition: value(4, BOOL),
                    expected: !matches!(case, DivisorCase::GuardedEq | DivisorCase::WrongPolarity),
                    message: if operation == SemanticBinaryOpV1::Divide {
                        SemanticAssertMessageV1::DivisionByZero(denominator)
                    } else {
                        SemanticAssertMessageV1::RemainderByZero(denominator)
                    },
                    target: edge(SemanticEdgeRoleV1::AssertSuccess, 1),
                    unwind: SemanticUnwindActionV1::Unreachable,
                }
            } else {
                SemanticTerminatorKindV1::Goto(edge(SemanticEdgeRoleV1::Goto, 1))
            };
            vec![
                block(31, statements, terminator),
                block(32, body, SemanticTerminatorKindV1::Return),
            ]
        },
        |_| "private_array_relation".to_owned(),
        &[U64],
    );
    let mut work = CanonicalKernelIrWorkBudgetV1::new(WORK);
    let mut budget = AssertOriginBudgetV1::new(&mut work, STORAGE);
    ProductionPreRankedKirOwnerV1::try_materialize_with_budget(
        ssa,
        launch,
        ProductionSemanticKirLimitsV1::default(),
        &mut budget,
    )
    .unwrap()
}

fn arithmetic_receipt(
    case: DivisorCase,
    operation: SemanticBinaryOpV1,
) -> ProductionMaterializedRankedModuleReceiptV1 {
    // As in the existing scalar/general component fixtures, the ranked side
    // carries genuine launch/effect custody. This unused scalar RHS stays in
    // the authenticated source/N executable. This is not a backend projector
    // or a scoped ranked semantic-expression qualification test.
    array_output_ranked_receipt_v1(arithmetic_source(case, operation))
}

fn native_operation(operation: SemanticBinaryOpV1) -> BinaryOp {
    match operation {
        SemanticBinaryOpV1::Divide => BinaryOp::Divide,
        SemanticBinaryOpV1::Remainder => BinaryOp::Remainder,
        _ => panic!("unsigned arithmetic fixture opcode"),
    }
}

fn count_arithmetic(module: &Module, operation: SemanticBinaryOpV1) -> usize {
    module.functions.iter().filter_map(|function| function.body.as_ref())
        .flat_map(|body| &body.blocks).flat_map(|block| &block.operations)
        .filter(|row| matches!(row.kind, OperationKind::Binary { op, .. } if op == native_operation(operation)))
        .count()
}

fn assert_native_refusal(error: AdmissionError) {
    assert!(
        matches!(
            error,
            AdmissionError::Unsupported {
                phase: "B",
                detail: "unsigned division requires a nonzero divisor on every incoming path",
            }
        ),
        "the genuine source reaches native divisor admission: {error:?}"
    );
}

#[test]
fn general_policy3_unsigned_division_consumes_source_ranked_n_b_and_actual_c() {
    for operation in [SemanticBinaryOpV1::Divide, SemanticBinaryOpV1::Remainder] {
        for case in [
            DivisorCase::GuardedNe,
            DivisorCase::GuardedEq,
            DivisorCase::Nonzero,
        ] {
            let receipt = arithmetic_receipt(case, operation);
            assert_eq!(
                count_arithmetic(receipt.materialized.executable().module(), operation),
                1
            );
            with_prepared(prepare(receipt, Profile::Gfx942, None), |input, budget| {
                assert_eq!(count_arithmetic(input.bound.module(), operation), 1);
                assert_eq!(
                    count_arithmetic(input.output.owner().module(), operation),
                    1
                );
                let identity = *input.output.owner().canonical().identity();
                let floor = budget.storage();
                let owner = AdmittedOutput::try_admit_general_v1(
                    input.receipt,
                    input.bound,
                    input.output,
                    budget,
                )
                .unwrap();
                assert_eq!(budget.storage(), floor);
                assert_eq!(*owner.output().canonical().identity(), identity);
                assert_eq!(count_arithmetic(owner.output().module(), operation), 1);
                assert!(owner.kernels()[0].accesses().is_empty());
                assert!(!owner.grants_artifact_or_launch_authority());
                owner.verify_equivalence(budget).unwrap();
                assert_eq!(budget.storage(), floor);
            });
        }
    }
}

#[test]
fn general_policy3_unsigned_division_does_not_inherit_an_unrelated_or_late_guard() {
    for operation in [SemanticBinaryOpV1::Divide, SemanticBinaryOpV1::Remainder] {
        for case in [
            DivisorCase::Unguarded,
            DivisorCase::WrongGuard,
            DivisorCase::WrongPolarity,
            DivisorCase::WrongDenominator,
            DivisorCase::BeforeGuard,
            DivisorCase::Zero,
        ] {
            with_prepared(
                prepare(arithmetic_receipt(case, operation), Profile::Gfx942, None),
                |input, budget| {
                    let floor = budget.storage();
                    let error = AdmittedOutput::try_admit_general_v1(
                        input.receipt,
                        input.bound,
                        input.output,
                        budget,
                    )
                    .unwrap_err();
                    assert_native_refusal(error);
                    assert_eq!(budget.storage(), floor);
                },
            );
        }
    }
}

fn replace_divisor_with_zero(module: &mut Module) {
    let body = module.functions[0].body.as_mut().unwrap();
    let divisor_type = body
        .blocks
        .iter()
        .flat_map(|block| &block.operations)
        .find(|operation| {
            matches!(
                operation.kind,
                OperationKind::Binary {
                    op: BinaryOp::Divide | BinaryOp::Remainder,
                    ..
                }
            )
        })
        .unwrap()
        .results[0]
        .ty
        .clone();
    let zero = body
        .blocks
        .iter()
        .flat_map(|block| &block.operations)
        .find_map(|operation| match operation.kind {
            OperationKind::Constant(Constant::U64(0) | Constant::Index(0))
                if operation.results[0].ty == divisor_type =>
            {
                Some(operation.results[0].id)
            }
            _ => None,
        })
        .expect("actual source zero used by the dominating comparison");
    let operation = body
        .blocks
        .iter_mut()
        .flat_map(|block| &mut block.operations)
        .find(|operation| {
            matches!(
                operation.kind,
                OperationKind::Binary {
                    op: BinaryOp::Divide | BinaryOp::Remainder,
                    ..
                }
            )
        })
        .unwrap();
    let OperationKind::Binary { rhs, .. } = &mut operation.kind else {
        unreachable!()
    };
    *rhs = zero;
}

fn reverse_success_edge(module: &mut Module) {
    let (then_target, else_target) = module.functions[0]
        .body
        .as_mut()
        .unwrap()
        .blocks
        .iter_mut()
        .find_map(|block| match block.terminator.as_mut() {
            Some(Terminator::ConditionalBranch {
                then_target,
                else_target,
                ..
            }) => Some((then_target, else_target)),
            _ => None,
        })
        .expect("actual source nonzero guard");
    std::mem::swap(then_target, else_target);
}

fn replace_guard_with_unconditional_success(module: &mut Module) {
    let block = module.functions[0]
        .body
        .as_mut()
        .unwrap()
        .blocks
        .iter_mut()
        .find(|block| matches!(block.terminator, Some(Terminator::ConditionalBranch { .. })))
        .unwrap();
    let Some(Terminator::ConditionalBranch {
        then_target,
        then_arguments,
        ..
    }) = &block.terminator
    else {
        unreachable!()
    };
    block.terminator = Some(Terminator::Branch {
        target: *then_target,
        arguments: then_arguments.clone(),
    });
}

#[test]
fn general_policy3_unsigned_divisor_and_guard_substitution_fail_the_exact_n_b_join() {
    for mutate in [
        replace_divisor_with_zero as fn(&mut Module),
        reverse_success_edge,
        replace_guard_with_unconditional_success,
    ] {
        let input = prepare(
            arithmetic_receipt(DivisorCase::GuardedNe, SemanticBinaryOpV1::Divide),
            Profile::Gfx942,
            Some(mutate),
        );
        with_prepared(input, |input, budget| {
            let floor = budget.storage();
            assert!(matches!(
                AdmittedOutput::try_admit_general_v1(
                    input.receipt,
                    input.bound,
                    input.output,
                    budget
                ),
                Err(AdmissionError::Coordinates(_))
            ));
            assert_eq!(budget.storage(), floor);
        });
    }
}

#[test]
fn general_policy3_unsigned_arithmetic_c_cannot_be_transplanted_from_another_b() {
    let mut original = prepare(
        arithmetic_receipt(DivisorCase::GuardedNe, SemanticBinaryOpV1::Divide),
        Profile::Gfx942,
        None,
    );
    let replacement = prepare(
        arithmetic_receipt(DivisorCase::GuardedNe, SemanticBinaryOpV1::Divide),
        Profile::Gfx942,
        Some(replace_divisor_with_zero),
    );
    assert_ne!(
        original.bound.canonical().identity(),
        replacement.bound.canonical().identity()
    );
    original.output = replacement.output;
    original.output_storage = replacement.output_storage;
    drop((replacement.receipt, replacement.bound));
    with_prepared(original, |input, budget| {
        assert!(matches!(
            AdmittedOutput::try_admit_general_v1(input.receipt, input.bound, input.output, budget),
            Err(AdmissionError::SourceOutput(
                ProductionSourceOutputErrorV1::InputCustody
            ))
        ));
    });
}

struct PreparedArithmetic4 {
    receipt: ProductionMaterializedRankedModuleReceiptV1,
    bound: VerifiedCanonicalKernelIrModuleV12,
    checked: fe2o3_kernel_opt::CheckedCanonicalKernelIrOwnerPolicy4V1,
    source_storage: usize,
    bound_storage: usize,
}

impl PreparedArithmetic4 {
    fn floor(&self) -> usize {
        FLOOR + self.source_storage + self.bound_storage + self.checked.retained_storage()
    }
}

fn prepare_arithmetic4(
    case: DivisorCase,
    operation: SemanticBinaryOpV1,
    mutation: Option<fn(&mut Module)>,
) -> PreparedArithmetic4 {
    let Prepared {
        receipt,
        bound,
        output,
        source_storage,
        bound_storage,
        ..
    } = prepare(
        arithmetic_receipt(case, operation),
        Profile::Gfx942,
        mutation,
    );
    drop(output);
    let mut work = CanonicalKernelIrWorkBudgetV1::new(WORK);
    let mut budget = AssertOriginBudgetV1::new(&mut work, STORAGE);
    let floor = FLOOR + source_storage + bound_storage;
    budget.reserve_storage(floor).unwrap();
    let checked =
        fe2o3_kernel_opt::optimize_checked_canonical_kernel_ir_policy4_v1(&bound, &mut budget)
            .unwrap();
    assert_eq!(budget.storage(), floor);
    PreparedArithmetic4 {
        receipt,
        bound,
        checked,
        source_storage,
        bound_storage,
    }
}

#[test]
fn general_policy4_unsigned_division_replays_c_o_and_qualifies_actual_o() {
    for operation in [SemanticBinaryOpV1::Divide, SemanticBinaryOpV1::Remainder] {
        for case in [DivisorCase::GuardedNe, DivisorCase::GuardedEq] {
            let input = prepare_arithmetic4(case, operation, None);
            assert_eq!(
                count_arithmetic(
                    input.checked.intermediate_policy3().owner().module(),
                    operation
                ),
                1
            );
            assert_eq!(
                count_arithmetic(input.checked.owner().module(), operation),
                1
            );
            let identity = *input.checked.owner().canonical().identity();
            let floor = input.floor();
            let mut work = CanonicalKernelIrWorkBudgetV1::new(WORK);
            let mut budget = AssertOriginBudgetV1::new(&mut work, STORAGE);
            budget.reserve_storage(floor).unwrap();
            let owner = crate::ProductionCheckedOutputOwnerPolicy4V1::try_admit_v1(
                input.receipt,
                input.bound,
                input.checked,
                &mut budget,
            )
            .unwrap();
            assert_eq!(budget.storage(), floor);
            assert_eq!(*owner.output().canonical().identity(), identity);
            assert_eq!(count_arithmetic(owner.output().module(), operation), 1);
            owner.verify_equivalence(&mut budget).unwrap();
            assert_eq!(budget.storage(), floor);
            assert!(!owner.grants_artifact_or_launch_authority());
            drop(owner);
            budget.release_storage(floor - FLOOR).unwrap();
            assert_eq!(budget.storage(), FLOOR);
        }
    }
}

#[test]
fn general_policy4_unsigned_division_keeps_source_and_composition_refusals() {
    for case in [
        DivisorCase::Unguarded,
        DivisorCase::WrongDenominator,
        DivisorCase::BeforeGuard,
    ] {
        let input = prepare_arithmetic4(case, SemanticBinaryOpV1::Remainder, None);
        let floor = input.floor();
        let mut work = CanonicalKernelIrWorkBudgetV1::new(WORK);
        let mut budget = AssertOriginBudgetV1::new(&mut work, STORAGE);
        budget.reserve_storage(floor).unwrap();
        let error = crate::ProductionCheckedOutputOwnerPolicy4V1::try_admit_v1(
            input.receipt,
            input.bound,
            input.checked,
            &mut budget,
        )
        .unwrap_err();
        let crate::ProductionCheckedOutputAdmissionErrorPolicy4V1::Admission(error) = error else {
            panic!("source/native arithmetic refusal")
        };
        assert_native_refusal(error);
        assert_eq!(budget.storage(), floor);
        budget.release_storage(floor - FLOOR).unwrap();
        assert_eq!(budget.storage(), FLOOR);
    }
    let original = prepare_arithmetic4(DivisorCase::GuardedNe, SemanticBinaryOpV1::Divide, None);
    let replacement = prepare_arithmetic4(
        DivisorCase::GuardedNe,
        SemanticBinaryOpV1::Divide,
        Some(replace_divisor_with_zero),
    );
    let floor = FLOOR
        + original.source_storage
        + original.bound_storage
        + replacement.checked.retained_storage();
    drop((original.checked, replacement.receipt, replacement.bound));
    let mut work = CanonicalKernelIrWorkBudgetV1::new(WORK);
    let mut budget = AssertOriginBudgetV1::new(&mut work, STORAGE);
    budget.reserve_storage(floor).unwrap();
    assert!(matches!(
        crate::ProductionCheckedOutputOwnerPolicy4V1::try_admit_v1(
            original.receipt,
            original.bound,
            replacement.checked,
            &mut budget
        ),
        Err(
            crate::ProductionCheckedOutputAdmissionErrorPolicy4V1::Admission(
                AdmissionError::SourceOutput(ProductionSourceOutputErrorV1::InputCustody)
            )
        )
    ));
    assert_eq!(budget.storage(), floor);
    budget.release_storage(floor - FLOOR).unwrap();
    assert_eq!(budget.storage(), FLOOR);
}

#[test]
fn general_policy3_unsigned_division_full_transaction_restores_floor_on_late_budget_failure() {
    let mut used = 0;
    let mut scratch = 0;
    with_prepared(
        prepare(
            arithmetic_receipt(DivisorCase::GuardedNe, SemanticBinaryOpV1::Divide),
            Profile::Gfx942,
            None,
        ),
        |input, budget| {
            let floor = budget.storage();
            drop(
                AdmittedOutput::try_admit_general_v1(
                    input.receipt,
                    input.bound,
                    input.output,
                    budget,
                )
                .unwrap(),
            );
            used = budget.work();
            scratch = budget.peak_storage() - floor;
            assert_eq!(budget.storage(), floor);
        },
    );
    assert!(used > 8 && scratch > 0);
    for short_work in [true, false] {
        let input = prepare(
            arithmetic_receipt(DivisorCase::GuardedNe, SemanticBinaryOpV1::Divide),
            Profile::Gfx942,
            None,
        );
        let floor = FLOOR + input.source_storage + input.bound_storage + input.output_storage;
        let mut work = CanonicalKernelIrWorkBudgetV1::new(if short_work { used - 1 } else { WORK });
        let mut budget = AssertOriginBudgetV1::new(
            &mut work,
            if short_work {
                STORAGE
            } else {
                floor + scratch - 1
            },
        );
        budget.reserve_storage(floor).unwrap();
        assert!(
            AdmittedOutput::try_admit_general_v1(
                input.receipt,
                input.bound,
                input.output,
                &mut budget
            )
            .is_err()
        );
        assert_eq!(budget.storage(), floor);
        assert!(budget.work() > 8);
        if !short_work {
            assert!(budget.failed_storage().is_some())
        }
        budget.release_storage(floor - FLOOR).unwrap();
        assert_eq!(budget.storage(), FLOOR);
        if short_work {
            assert!(work.failed_work().is_some())
        }
    }
}

#[test]
fn general_policy4_unsigned_division_full_transaction_restores_floor_on_late_budget_failure() {
    let input = prepare_arithmetic4(DivisorCase::GuardedNe, SemanticBinaryOpV1::Remainder, None);
    let floor = input.floor();
    let mut work = CanonicalKernelIrWorkBudgetV1::new(WORK);
    let mut budget = AssertOriginBudgetV1::new(&mut work, STORAGE);
    budget.reserve_storage(floor).unwrap();
    drop(
        crate::ProductionCheckedOutputOwnerPolicy4V1::try_admit_v1(
            input.receipt,
            input.bound,
            input.checked,
            &mut budget,
        )
        .unwrap(),
    );
    let used = budget.work();
    let scratch = budget.peak_storage() - floor;
    assert!(used > 8 && scratch > 0);
    assert_eq!(budget.storage(), floor);
    for short_work in [true, false] {
        let input =
            prepare_arithmetic4(DivisorCase::GuardedNe, SemanticBinaryOpV1::Remainder, None);
        let floor = input.floor();
        let mut work = CanonicalKernelIrWorkBudgetV1::new(if short_work { used - 1 } else { WORK });
        let mut budget = AssertOriginBudgetV1::new(
            &mut work,
            if short_work {
                STORAGE
            } else {
                floor + scratch - 1
            },
        );
        budget.reserve_storage(floor).unwrap();
        assert!(
            crate::ProductionCheckedOutputOwnerPolicy4V1::try_admit_v1(
                input.receipt,
                input.bound,
                input.checked,
                &mut budget
            )
            .is_err()
        );
        assert_eq!(budget.storage(), floor);
        assert!(budget.work() > 8);
        if !short_work {
            assert!(budget.failed_storage().is_some())
        }
        budget.release_storage(floor - FLOOR).unwrap();
        assert_eq!(budget.storage(), FLOOR);
        if short_work {
            assert!(work.failed_work().is_some())
        }
    }
    budget.release_storage(floor - FLOOR).unwrap();
    assert_eq!(budget.storage(), FLOOR);
}

#[test]
fn native_unsigned_admission_does_not_bypass_ranked_semantic_expression_definedness() {
    use fe2o3_pliron::{
        ProductionOverflowContractV2, ProductionSemanticBinaryOpV2,
        ProductionSemanticExpressionErrorV2, ProductionSemanticExpressionV2 as Expression,
        ProductionSemanticScalarTypeV2,
    };
    let scalar = ProductionSemanticScalarTypeV2::Integer {
        signed: false,
        bits: 64,
    };
    for operation in [
        ProductionSemanticBinaryOpV2::Divide,
        ProductionSemanticBinaryOpV2::Remainder,
    ] {
        let expression = Expression::Binary {
            operation,
            scalar,
            overflow: ProductionOverflowContractV2::Wrapping,
            lhs: Box::new(Expression::Constant { scalar, bits: 84 }),
            rhs: Box::new(Expression::Symbol { symbol: 0, scalar }),
        };
        assert_eq!(
            expression.validate_static_domains(),
            Err(ProductionSemanticExpressionErrorV2::IncompleteDomain)
        );
    }
}

use super::super::constant_shifts::tests::{FIXED, candidate, inspect};
use super::*;
use crate::production_semantic_kir_v1::helper_source_fixture_v1 as fixture;
use fe2o3_kernel_ir::{
    CanonicalKernelIrWorkBudgetV1, Constant, Operation, ScalarType, ValueDef, ValueId,
};
use fe2o3_mir_model::semantic_mir_v1::*;

fn input(
    mask: u32,
    gap: Option<SemanticStatementKindV1>,
    moved: bool,
) -> AdmittedInertSemanticMirV1 {
    let mut statements = vec![fixture::assign(
        3,
        SemanticRvalueKindV1::Binary {
            operation: SemanticBinaryOpV1::BitAnd,
            left: fixture::copy(2),
            right: SemanticOperandV1::Constant(SemanticConstantV1::new(
                fixture::WORD,
                SemanticConstantValueV1::Scalar(
                    SemanticScalarValueV1::new(u128::from(mask), 4).unwrap(),
                ),
            )),
        },
    )];
    if let Some(gap) = gap {
        statements.push(SemanticStatementV1::new(
            SemanticSourceProvenanceV1::unavailable(),
            gap,
        ));
    }
    statements.push(fixture::assign(
        0,
        SemanticRvalueKindV1::Binary {
            operation: SemanticBinaryOpV1::ShiftRight,
            left: fixture::copy(1),
            right: if moved {
                SemanticOperandV1::Move(fixture::place(3))
            } else {
                fixture::copy(3)
            },
        },
    ));
    fixture::source(vec![fixture::function(
        60,
        false,
        1,
        vec![fixture::block(
            61,
            statements,
            SemanticTerminatorKindV1::Return,
        )],
    )])
}

#[test]
fn native_mask_totality_is_not_a_source_authority() {
    for scalar in FIXED {
        let width = u64::from(scalar.bit_width().unwrap());
        for op in [BinaryOp::ShiftLeft, BinaryOp::ShiftRight] {
            for helper in [false, true] {
                for limit in [width - 2, width - 1, width + 1] {
                    inspect(
                        &candidate(scalar, op, None, Some(limit), helper),
                        |inventory, ordinal, budget| {
                            assert_eq!(
                                native(inventory, ordinal, budget).unwrap(),
                                limit == width - 1
                            );
                        },
                    );
                }
                inspect(
                    &candidate(scalar, op, None, None, helper),
                    |inventory, ordinal, budget| {
                        assert!(!native(inventory, ordinal, budget).unwrap());
                    },
                );
            }
        }
    }
    // Raw dynamic source negatives remain in the genuine V410 whole-source
    // census test, even though its lowerer always introduces a native mask.
}

#[test]
fn native_cast_of_an_actual_mask_is_total_without_a_redundant_outer_mask() {
    for target in [
        ScalarType::U8,
        ScalarType::I8,
        ScalarType::U64,
        ScalarType::I64,
    ] {
        let limit = u64::from(target.bit_width().unwrap()) - 1;
        let mut module = candidate(target, BinaryOp::ShiftRight, None, Some(limit), false);
        let function = &mut module.functions[0];
        function.signature = fe2o3_kernel_ir::Signature::new(
            vec![Type::Scalar(target), Type::Scalar(ScalarType::U32)],
            vec![],
        );
        let ops = &mut function.body.as_mut().unwrap().blocks[0].operations;
        ops[0].results[0].ty = Type::Scalar(ScalarType::U32);
        ops[0].kind = OperationKind::Constant(Constant::U32(limit as u32));
        ops[1].results[0].ty = Type::Scalar(ScalarType::U32);
        let kind = fe2o3_kernel_ir::plan_integer_cast_v1(ScalarType::U32, target).unwrap()[0]
            .unwrap()
            .0;
        ops.insert(
            2,
            Operation::new(
                vec![ValueDef::new(ValueId(6), Type::Scalar(target))],
                OperationKind::Cast {
                    kind,
                    value: ValueId(4),
                    to: Type::Scalar(target),
                },
            ),
        );
        let OperationKind::Binary { rhs, .. } = &mut ops[3].kind else {
            unreachable!()
        };
        *rhs = ValueId(6);
        inspect(&module, |inventory, ordinal, budget| {
            assert!(native(inventory, ordinal, budget).unwrap())
        });
        module.functions[0].body.as_mut().unwrap().blocks[0].operations[0].kind =
            OperationKind::Constant(Constant::U32(limit as u32 + 1));
        inspect(&module, |inventory, ordinal, budget| {
            assert!(!native(inventory, ordinal, budget).unwrap())
        });
    }
}

#[test]
fn source_query_requires_actual_owner_and_immediate_predecessor_not_a_matching_local() {
    for moved in [false, true] {
        for (limit, gap, expected) in [
            (31, None, true),
            (30, None, false),
            (63, None, false),
            (31, Some(SemanticStatementKindV1::Nop), false),
            (
                31,
                Some(SemanticStatementKindV1::StorageDead(
                    SemanticLocalIdV1::from_index(3),
                )),
                false,
            ),
            (
                31,
                Some(SemanticStatementKindV1::Assign(SemanticAssignmentV1::new(
                    fixture::place(3),
                    SemanticRvalueV1::new(
                        fixture::WORD,
                        SemanticRvalueKindV1::Use(fixture::copy(2)),
                    ),
                ))),
                false,
            ),
        ] {
            let ordinal = if gap.is_some() { 2 } else { 1 };
            let source_owner = input(limit, gap, moved);
            let mut work = CanonicalKernelIrWorkBudgetV1::new(10_000);
            let mut budget = AssertOriginBudgetV1::new(&mut work, 19);
            budget.reserve_storage(19).unwrap();
            assert_eq!(
                source(&source_owner, 1, 0, ordinal, &mut budget).unwrap(),
                expected
            );
            assert!(!source(&source_owner, 0, 0, ordinal, &mut budget).unwrap());
            assert!(!source(&source_owner, 1, 1, ordinal, &mut budget).unwrap());
            assert!(!source(&source_owner, 1, 0, 0, &mut budget).unwrap());
            assert_eq!(budget.storage(), 19);
        }
    }
}

#[test]
fn actual_semantic_ssa_materialization_and_whole_source_census_require_exact_mask() {
    for limit in [30, 31, 63] {
        let input = input(limit, None, false);
        let launch = crate::ProductionSourceLaunchRosterV1::try_new(
            &input,
            &[crate::ProductionSourceLaunchRootInputV1::new(
                "helper_value_source",
                [31; 32],
                crate::ProductionSourceLaunchInputV1::new(1, Some([1, 1, 1]), [1, 1, 1]),
            )],
        )
        .unwrap();
        let semantic = fe2o3_pliron::ProductionSemanticMirOwnerV1::try_new(
            input,
            fe2o3_pliron::ProductionSemanticMirLimitsV1::default(),
        )
        .unwrap();
        let ssa = fe2o3_pliron::ProductionSemanticSsaOwnerV1::try_new(
            semantic,
            fe2o3_pliron::ProductionSemanticSsaLimitsV1::default(),
        )
        .unwrap();
        let mut work = CanonicalKernelIrWorkBudgetV1::new(100_000_000);
        let mut budget = AssertOriginBudgetV1::new(&mut work, 128 << 20);
        budget.reserve_storage(19).unwrap();
        let materialized = ProductionPreRankedKirOwnerV1::try_materialize_with_budget(
            ssa,
            launch,
            ProductionSemanticKirLimitsV1::default(),
            &mut budget,
        )
        .unwrap();
        let storage = materialized.unit_local_source_storage_floor_v1().unwrap();
        budget.reserve_storage(storage).unwrap();
        let floor = budget.storage();
        let result = census::source_parts(
            materialized.semantic_ssa().source_semantic(),
            &materialized.correspondence,
            &mut budget,
        );
        if limit == 31 {
            result.unwrap();
        } else {
            assert!(matches!(
                result,
                Err(E::Unsupported {
                    phase: "source",
                    detail: "total scalar/global recipe; no unchecked arithmetic"
                })
            ));
        }
        budget.release_storage(budget.storage() - floor).unwrap();
        drop(materialized);
        budget.release_storage(storage).unwrap();
        assert_eq!(budget.storage(), 19);
    }
}

#[test]
fn exact_prepaid_source_and_native_work_restore_the_inherited_floor() {
    let input = input(31, None, false);
    for cap in [63, 64] {
        let mut work = CanonicalKernelIrWorkBudgetV1::new(cap);
        let mut budget = AssertOriginBudgetV1::new(&mut work, 19);
        budget.reserve_storage(19).unwrap();
        let result = source(&input, 1, 0, 1, &mut budget);
        if cap == 64 {
            assert!(result.unwrap());
            assert_eq!(budget.work(), 64);
        } else {
            assert!(matches!(
                result,
                Err(E::Resource(AssertOriginResourceV1::Work(_)))
            ));
            assert_eq!(budget.work(), 0);
        }
        assert_eq!(budget.storage(), 19);
    }
    inspect(
        &candidate(ScalarType::U64, BinaryOp::ShiftRight, None, Some(63), false),
        |inventory, ordinal, outer| {
            for cap in [95, 96] {
                let floor = outer.storage();
                let mut work = CanonicalKernelIrWorkBudgetV1::new(cap);
                let mut budget = AssertOriginBudgetV1::new(&mut work, floor);
                budget.reserve_storage(floor).unwrap();
                let result = native(inventory, ordinal, &mut budget);
                if cap == 96 {
                    assert!(result.unwrap());
                    assert_eq!(budget.work(), 96);
                } else {
                    assert!(matches!(
                        result,
                        Err(E::Resource(AssertOriginResourceV1::Work(_)))
                    ));
                    assert_eq!(budget.work(), 0);
                }
                assert_eq!(budget.storage(), floor);
            }
        },
    );
}

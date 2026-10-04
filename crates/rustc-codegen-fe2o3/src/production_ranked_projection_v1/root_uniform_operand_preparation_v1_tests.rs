// Component-only controls; no complete producer/source or bounds authority.
mod uniform_operand_preparation_controls {
    use super::super::root_uniform_operand_preparation_v1 as uniform;
    use super::*;
    use fe2o3_kernel_ir::{
        CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
        CanonicalKernelIrWorkBudgetV1 as Work,
    };
    use fe2o3_mir_model::semantic_mir_v1::{SemanticProjectionKindV1, SemanticProjectionV1};
    const LIMIT: usize = 16 * 1024 * 1024;

    // BEGIN FROZEN ORIGINAL UNIFORM OPERAND ORACLE
    #[allow(clippy::too_many_arguments)]
    fn original(
        operand: &SemanticOperandV1,
        constants: &[Option<u64>],
        stable_argument_origins: &[Option<u32>],
        arguments: &mut [Option<u32>],
        next_argument: &mut usize,
        operations: &mut Vec<ProductionRankedOperationV1>,
        next_value: &mut u32,
    ) -> Result<Option<ProductionRankedValueV1>, ProductionRankedProjectionErrorV1> {
        if let Some(value) = constant_operand_value(operand, constants) {
            reserve_operation(operations)?;
            let result = next_value_id(next_value)?;
            operations.push(ProductionRankedOperationV1::IndexConstant { result, value });
            return Ok(Some(ProductionRankedValueV1::Local(result)));
        }
        let Some(local) = simple_operand_local(operand) else {
            return Ok(None);
        };
        let local_index = local.index() as usize;
        let origin = stable_argument_origins.get(local_index).copied().flatten();
        let Some(origin) = origin.map(|origin| origin as usize) else {
            return Ok(None);
        };
        let slot =
            arguments
                .get_mut(origin)
                .ok_or(ProductionRankedProjectionErrorV1::Unsupported(
                    "a uniform switch argument origin outside the semantic local table",
                ))?;
        let argument = match *slot {
            Some(argument) => argument,
            None => {
                let argument = u32::try_from(*next_argument).map_err(|_| {
                    ProductionRankedProjectionErrorV1::Unsupported(
                        "too many uniform switch ranked arguments",
                    )
                })?;
                *next_argument = next_argument.checked_add(1).ok_or(
                    ProductionRankedProjectionErrorV1::Unsupported(
                        "uniform switch ranked argument count overflow",
                    ),
                )?;
                *slot = Some(argument);
                argument
            }
        };
        Ok(Some(ProductionRankedValueV1::Argument(argument)))
    }
    // END FROZEN ORIGINAL UNIFORM OPERAND ORACLE

    const SCALAR_TYPE: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(0);
    fn constant(value: u64) -> SemanticOperandV1 {
        use fe2o3_mir_model::semantic_mir_v1::{SemanticConstantV1, SemanticScalarValueV1};
        SemanticOperandV1::Constant(SemanticConstantV1::new(
            SCALAR_TYPE,
            SemanticConstantValueV1::Scalar(
                SemanticScalarValueV1::new(u128::from(value), 8).unwrap(),
            ),
        ))
    }

    #[derive(Default)]
    struct State {
        arguments: Vec<Option<u32>>,
        next_argument: usize,
        operations: Vec<ProductionRankedOperationV1>,
        next_value: u32,
    }
    fn state(arguments: &[Option<u32>], next_argument: usize, next_value: u32) -> State {
        State {
            arguments: arguments.to_vec(),
            next_argument,
            next_value,
            ..Default::default()
        }
    }
    fn snapshot(s: &State) -> (Vec<Option<u32>>, usize, String, u32) {
        (
            s.arguments.clone(),
            s.next_argument,
            format!("{:?}", s.operations),
            s.next_value,
        )
    }
    fn projected(local: u32, projections: usize) -> SemanticOperandV1 {
        SemanticOperandV1::Copy(
            SemanticPlaceV1::new(
                SemanticLocalIdV1::from_index(local),
                (0..projections)
                    .map(|_| {
                        SemanticProjectionV1::new(SemanticProjectionKindV1::OpaqueCast, SCALAR_TYPE)
                            .unwrap()
                    })
                    .collect(),
                SCALAR_TYPE,
            )
            .unwrap(),
        )
    }
    fn old(
        s: &mut State,
        operand: &SemanticOperandV1,
        constants: &[Option<u64>],
        origins: &[Option<u32>],
    ) -> Result<Option<ProductionRankedValueV1>, ProductionRankedProjectionErrorV1> {
        original(
            operand,
            constants,
            origins,
            &mut s.arguments,
            &mut s.next_argument,
            &mut s.operations,
            &mut s.next_value,
        )
    }
    fn legacy(
        s: &mut State,
        operand: &SemanticOperandV1,
        constants: &[Option<u64>],
        origins: &[Option<u32>],
    ) -> Result<Option<ProductionRankedValueV1>, ProductionRankedProjectionErrorV1> {
        uniform::uniform_operand_legacy_v1(
            operand,
            constants,
            origins,
            &mut s.arguments,
            &mut s.next_argument,
            &mut s.operations,
            &mut s.next_value,
        )
    }
    fn paid(
        s: &mut State,
        operand: &SemanticOperandV1,
        constants: &[Option<u64>],
        origins: &[Option<u32>],
        resources: &mut PreparationResourcesV1<'_, '_>,
    ) -> Result<Option<ProductionRankedValueV1>, ProductionRankedProjectionErrorV1> {
        uniform::uniform_operand_paid_v1(
            operand,
            constants,
            origins,
            &mut s.arguments,
            &mut s.next_argument,
            &mut s.operations,
            &mut s.next_value,
            resources,
        )
    }
    fn parity(
        operands: &[SemanticOperandV1],
        constants: &[Option<u64>],
        origins: &[Option<u32>],
        arguments: &[Option<u32>],
        next_argument: usize,
        next_value: u32,
    ) {
        let mut expected = state(arguments, next_argument, next_value);
        let mut ordinary = state(arguments, next_argument, next_value);
        let mut metered = state(arguments, next_argument, next_value);
        let mut work = Work::new(LIMIT);
        let mut budget = Budget::new(&mut work, LIMIT);
        let mut owned = 0;
        for operand in operands {
            let before = old(&mut expected, operand, constants, origins);
            let legacy = legacy(&mut ordinary, operand, constants, origins);
            let paid = paid(
                &mut metered,
                operand,
                constants,
                origins,
                &mut PreparationResourcesV1::new(&mut budget, &mut owned),
            );
            assert_eq!(format!("{legacy:?}"), format!("{before:?}"));
            assert_eq!(format!("{paid:?}"), format!("{before:?}"));
            assert_eq!(snapshot(&ordinary), snapshot(&expected));
            assert_eq!(snapshot(&metered), snapshot(&expected));
        }
        assert_eq!(budget.storage(), owned);
        drop(metered);
        budget.release_storage(owned).unwrap();
        assert_eq!(budget.storage(), 0);
    }

    #[test]
    fn uniform_operand_frozen_oracle_constants_new_arguments_and_reuse() {
        parity(
            &[
                constant(7),
                projected(1, 0),
                projected(1, 0),
                projected(2, 0),
            ],
            &[],
            &[None, Some(0), Some(1)],
            &[None, None, Some(19)],
            3,
            5,
        );
        parity(
            &[projected(1, 0), constant(9)],
            &[None, Some(12)],
            &[],
            &[Some(11)],
            21,
            17,
        );
    }
    #[test]
    fn uniform_operand_frozen_oracle_missing_origin_and_projection_refusals() {
        for n in [1, 64, 2048] {
            parity(&[projected(1, n)], &[], &[None, Some(0)], &[None], 3, 0);
        }
        parity(
            &[projected(0, 0), projected(4, 0)],
            &[],
            &[None],
            &[None],
            3,
            0,
        );
    }
    #[test]
    fn uniform_operand_frozen_oracle_slot_and_counter_error_order() {
        parity(
            &[projected(1, 0)],
            &[],
            &[None, Some(99)],
            &[None],
            usize::MAX,
            0,
        );
        for next in [0, 7, u32::MAX as usize, usize::MAX] {
            for cached in [None, Some(11)] {
                parity(
                    &[projected(1, 0)],
                    &[],
                    &[None, Some(0)],
                    &[cached],
                    next,
                    0,
                );
            }
        }
    }
    #[test]
    fn uniform_operand_paid_preserves_left_argument_when_right_is_unsupported() {
        let lhs = projected(1, 0);
        let rhs = projected(2, 1);
        parity(
            &[lhs.clone(), rhs.clone()],
            &[],
            &[None, Some(0), Some(1)],
            &[None, None],
            3,
            0,
        );
        let mut state = state(&[None, None], 3, 0);
        let mut work = Work::new(LIMIT);
        let mut budget = Budget::new(&mut work, LIMIT);
        let mut owned = 0;
        assert!(
            paid(
                &mut state,
                &lhs,
                &[],
                &[None, Some(0), Some(1)],
                &mut PreparationResourcesV1::new(&mut budget, &mut owned)
            )
            .unwrap()
            .is_some()
        );
        assert!(
            paid(
                &mut state,
                &rhs,
                &[],
                &[None, Some(0), Some(1)],
                &mut PreparationResourcesV1::new(&mut budget, &mut owned)
            )
            .unwrap()
            .is_none()
        );
        assert_eq!(state.arguments, [Some(3), None]);
        assert_eq!(state.next_argument, 4);
        assert!(state.operations.is_empty());
        drop(state);
        budget.release_storage(owned).unwrap();
    }
    #[test]
    fn uniform_operand_paid_preserves_left_constant_operation_when_right_refuses() {
        parity(
            &[constant(7), projected(2, 1)],
            &[],
            &[None, None, Some(0)],
            &[None],
            3,
            41,
        );
        let mut state = state(&[None], 3, 41);
        let mut work = Work::new(LIMIT);
        let mut budget = Budget::new(&mut work, LIMIT);
        let mut owned = 0;
        paid(
            &mut state,
            &constant(7),
            &[],
            &[],
            &mut PreparationResourcesV1::new(&mut budget, &mut owned),
        )
        .unwrap();
        assert!(
            paid(
                &mut state,
                &projected(2, 1),
                &[],
                &[],
                &mut PreparationResourcesV1::new(&mut budget, &mut owned)
            )
            .unwrap()
            .is_none()
        );
        assert_eq!(state.operations.len(), 1);
        assert_eq!(state.next_value, 42);
        assert_eq!(state.next_argument, 3);
        drop(state);
        budget.release_storage(owned).unwrap();
    }
    #[test]
    fn uniform_operand_paid_exact_new_and_reused_argument_work() {
        let mut state = state(&[None], 3, 0);
        let mut work = Work::new(LIMIT);
        let mut budget = Budget::new(&mut work, LIMIT);
        let mut owned = 0;
        let h = uniform::operand_frame_v1().unwrap();
        for expected in [h + 6, h + 3] {
            let before = budget.work();
            let stored = budget.storage();
            paid(
                &mut state,
                &projected(1, 0),
                &[],
                &[None, Some(0)],
                &mut PreparationResourcesV1::new(&mut budget, &mut owned),
            )
            .unwrap();
            assert_eq!(budget.work() - before, expected);
            assert_eq!(budget.storage() - stored, h);
        }
        assert_eq!(state.arguments, [Some(3)]);
        assert_eq!(state.next_argument, 4);
        drop(state);
        budget.release_storage(owned).unwrap();
        assert_eq!(budget.storage(), 0);
    }
    #[test]
    fn uniform_operand_paid_projection_work_is_upfront_and_denial_keeps_state() {
        let h = uniform::operand_frame_v1().unwrap();
        let n = 2048;
        let mut state = state(&[None], 3, 0);
        let before = snapshot(&state);
        let mut work = Work::new(h + 1 + n - 1);
        let mut budget = Budget::new(&mut work, LIMIT);
        let mut owned = 0;
        assert!(
            paid(
                &mut state,
                &projected(1, n),
                &[],
                &[None, Some(0)],
                &mut PreparationResourcesV1::new(&mut budget, &mut owned)
            )
            .is_err()
        );
        assert!(budget.failed_work().is_some());
        assert_eq!(budget.work(), h + 1);
        assert_eq!(owned, h);
        assert_eq!(snapshot(&state), before);
        drop(state);
        budget.release_storage(owned).unwrap();
        assert_eq!(budget.storage(), 0);
    }
    #[test]
    fn uniform_operand_paid_header_work_and_storage_boundaries() {
        let h = uniform::operand_frame_v1().unwrap();
        for storage_denial in [false, true] {
            let mut state = state(&[None], 3, 0);
            let before = snapshot(&state);
            let mut work = Work::new(if storage_denial { LIMIT } else { h - 1 });
            let mut budget = Budget::new(&mut work, if storage_denial { h - 1 } else { LIMIT });
            let mut owned = 0;
            assert!(
                paid(
                    &mut state,
                    &constant(1),
                    &[],
                    &[],
                    &mut PreparationResourcesV1::new(&mut budget, &mut owned)
                )
                .is_err()
            );
            assert_eq!(owned, 0);
            assert_eq!(snapshot(&state), before);
            assert!(if storage_denial {
                budget.failed_storage().is_some()
            } else {
                budget.failed_work().is_some()
            });
        }
    }
    #[test]
    fn uniform_operand_paid_refuses_unmetered_without_touching_state() {
        let mut state = state(&[None], 3, 0);
        let before = snapshot(&state);
        assert!(
            paid(
                &mut state,
                &constant(1),
                &[],
                &[],
                &mut PreparationResourcesV1::unmetered()
            )
            .is_err()
        );
        assert_eq!(snapshot(&state), before);
    }
    #[test]
    fn uniform_operand_paid_ssa_failure_retains_reserved_operation_owner() {
        let mut state = state(&[None], 3, u32::MAX);
        let mut work = Work::new(LIMIT);
        let mut budget = Budget::new(&mut work, LIMIT);
        let mut owned = 0;
        assert!(matches!(
            paid(
                &mut state,
                &constant(1),
                &[],
                &[],
                &mut PreparationResourcesV1::new(&mut budget, &mut owned)
            ),
            Err(ProductionRankedProjectionErrorV1::Unsupported(
                "too many ranked SSA values"
            ))
        ));
        assert!(state.operations.is_empty());
        assert_eq!(state.operations.capacity(), 1);
        assert_eq!(state.next_value, u32::MAX);
        assert_eq!(
            owned,
            uniform::operand_frame_v1().unwrap()
                + std::mem::size_of::<ProductionRankedOperationV1>()
        );
        assert_eq!(budget.storage(), owned);
        drop(state);
        budget.release_storage(owned).unwrap();
        assert_eq!(budget.storage(), 0);
    }
    #[test]
    fn uniform_operand_paid_expected_unwind_preserves_mutated_outer_owner() {
        let mut state = state(&[None], 3, 0);
        let mut work = Work::new(LIMIT);
        let mut budget = Budget::new(&mut work, LIMIT);
        let mut owned = 0;
        let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            paid(
                &mut state,
                &projected(1, 0),
                &[],
                &[None, Some(0)],
                &mut PreparationResourcesV1::new(&mut budget, &mut owned),
            )
            .unwrap();
            panic!("fixed operand component unwind");
        }));
        assert!(outcome.is_err());
        assert_eq!(state.arguments, [Some(3)]);
        assert_eq!(state.next_argument, 4);
        assert_eq!(budget.storage(), owned);
        drop(state);
        budget.release_storage(owned).unwrap();
        assert_eq!(budget.storage(), 0);
    }
    #[test]
    fn uniform_operand_constant_nested_callers_have_separate_typed_frames() {
        uniform::test_access::constant_callers_control();
    }
    #[test]
    fn uniform_operand_frame_rows_checked_sum_and_overflow() {
        uniform::test_access::frame_control();
    }
}

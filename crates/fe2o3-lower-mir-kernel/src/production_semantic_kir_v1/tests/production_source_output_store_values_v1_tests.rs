mod source_output_store_value_components_v1 {
    use super::*;
    use fe2o3_kernel_ir::{
        CanonicalKernelIrWorkBudgetV1 as Work, CanonicalKirBlockCoordinateV1 as Block,
        CanonicalKirDefinitionCoordinateV1 as Definition,
        CanonicalKirFunctionCoordinateV1 as Function,
        CanonicalKirOperationCoordinateV1 as Operation, CanonicalKirUseCoordinateV1 as Use,
    };

    fn row(component: u32, operation: u32) -> SourceOutputStoreValueRowV1 {
        let original = Operation {
            block: Block {
                function: Function(0),
                block: 0,
            },
            operation,
        };
        let definition = Definition::FunctionArgument {
            function: Function(0),
            argument: 0,
        };
        SourceOutputStoreValueRowV1 {
            key: [0, 0, 0, 0, component],
            source: component as usize,
            original,
            definition,
            output: Some(fe2o3_kernel_analysis::CanonicalKirOutputUseV1 {
                coordinate: Use::OperationOperand {
                    operation: original,
                    operand: 1,
                },
                definition,
            }),
            executable: true,
        }
    }

    fn filled(
        budget: &mut AssertOriginBudgetV1<'_>,
    ) -> Result<(SourceOutputStoreValueIndexV1, usize), ProductionSourceOutputErrorV1> {
        use ProductionSourceOutputErrorV1 as Error;
        budget.charge_work(1).map_err(Error::Resource)?;
        budget
            .reserve_storage(std::mem::size_of::<SourceOutputStoreValueIndexV1>())
            .map_err(Error::Resource)?;
        let mut index = SourceOutputStoreValueIndexV1::default();
        assert_origin_push_v1(&mut index.rows, row(0, 0), budget).map_err(Error::SourceOrigin)?;
        let payload = source_output_store_value_finish_v1(&mut index, budget)?;
        Ok((index, payload))
    }

    #[test]
    fn store_index_components_pay_both_capacities_and_exact_work_before_publication() {
        const HISTORY: usize = 7;
        const FLOOR: usize = 19;
        // Header1 + first reserve/push2 + row3 + output reserve/push2 + payload5.
        const WORK: usize = 13;
        let header = std::mem::size_of::<SourceOutputStoreValueIndexV1>();
        let payload = 4
            * (std::mem::size_of::<SourceOutputStoreValueRowV1>()
                + std::mem::size_of::<SourceOutputStoreValueOutputRowV1>());
        for exact in [true, false] {
            let total = HISTORY + WORK;
            let mut work = Work::new(total - usize::from(!exact));
            let mut budget = AssertOriginBudgetV1::new(&mut work, FLOOR + header + payload);
            budget.reserve_storage(FLOOR).unwrap();
            budget.charge_work(HISTORY).unwrap();
            let result = source_output_global_scratch_scope_v1(&mut budget, |budget| {
                let (index, actual_payload) = filled(budget)?;
                assert_eq!(actual_payload, payload);
                assert_eq!(budget.storage(), FLOOR + header + actual_payload);
                drop(index);
                Ok(())
            });
            assert_eq!(result.is_ok(), exact);
            assert_eq!(budget.storage(), FLOOR);
            assert_eq!(budget.peak_storage(), FLOOR + header + payload);
            if exact {
                assert_eq!(budget.work(), total);
            } else {
                assert_eq!(budget.work(), total - 5);
            }
            assert_eq!(work.failed_work(), (!exact).then_some(total));
        }
    }

    #[test]
    fn store_index_component_allocation_denial_preserves_floor_and_prefix() {
        const FLOOR: usize = 19;
        let header = std::mem::size_of::<SourceOutputStoreValueIndexV1>();
        let rows = 4 * std::mem::size_of::<SourceOutputStoreValueRowV1>();
        let output = 4 * std::mem::size_of::<SourceOutputStoreValueOutputRowV1>();
        // First reserve fails after header1+reserve1. Second after those two,
        // row push1, row inspection3, and output reserve1.
        for (prior, next, work_prefix) in [(0, rows, 2), (rows, output, 7)] {
            let mut work = Work::new(1000);
            let mut budget =
                AssertOriginBudgetV1::new(&mut work, FLOOR + header + prior + next - 1);
            budget.reserve_storage(FLOOR).unwrap();
            budget.charge_work(7).unwrap();
            assert!(
                source_output_global_scratch_scope_v1(&mut budget, |budget| {
                    filled(budget).map(|(index, _)| {
                        drop(index);
                    })
                })
                .is_err()
            );
            assert_eq!(budget.storage(), FLOOR);
            assert_eq!(budget.peak_storage(), FLOOR + header + prior);
            assert_eq!(budget.failed_storage(), Some(FLOOR + header + prior + next));
            assert_eq!(budget.work(), 7 + work_prefix);
        }
    }

    #[test]
    fn store_index_rejects_duplicate_components_and_same_output_operand() {
        for duplicate_component in [true, false] {
            let mut work = Work::new(10_000);
            let mut budget = AssertOriginBudgetV1::new(&mut work, 100_000);
            let error = source_output_global_scratch_scope_v1(&mut budget, |budget| {
                budget
                    .reserve_storage(std::mem::size_of::<SourceOutputStoreValueIndexV1>())
                    .map_err(ProductionSourceOutputErrorV1::Resource)?;
                let mut index = SourceOutputStoreValueIndexV1::default();
                assert_origin_push_v1(&mut index.rows, row(0, 0), budget)
                    .map_err(ProductionSourceOutputErrorV1::SourceOrigin)?;
                assert_origin_push_v1(
                    &mut index.rows,
                    if duplicate_component {
                        row(0, 1)
                    } else {
                        row(1, 0)
                    },
                    budget,
                )
                .map_err(ProductionSourceOutputErrorV1::SourceOrigin)?;
                source_output_store_value_finish_v1(&mut index, budget)
            })
            .unwrap_err();
            assert!(
                matches!(error, ProductionSourceOutputErrorV1::Invalid(message)
                if message == if duplicate_component {
                    "source Store component appears more than once"
                } else {
                    "source Store output operand appears more than once"
                })
            );
            assert_eq!(budget.storage(), 0);
        }
    }

    #[test]
    fn store_index_scope_drops_paid_rows_on_error_or_unwind_and_can_continue() {
        for unwind in [true, false] {
            let mut work = Work::new(1000);
            let mut budget = AssertOriginBudgetV1::new(&mut work, 100_000);
            budget.reserve_storage(19).unwrap();
            budget.charge_work(7).unwrap();
            let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                source_output_global_scratch_scope_v1(&mut budget, |budget| {
                    let (index, _) = filled(budget)?;
                    if unwind {
                        panic!("paid Store index component unwind");
                    }
                    drop(index);
                    Err::<(), _>(ProductionSourceOutputErrorV1::Invalid("component refusal"))
                })
            }));
            if unwind {
                assert!(result.is_err());
            } else {
                assert!(result.unwrap().is_err());
            }
            assert_eq!(budget.storage(), 19);
            assert_eq!(budget.work(), 20);
            source_output_global_scratch_scope_v1(&mut budget, |budget| {
                filled(budget).map(|(index, _)| {
                    drop(index);
                })
            })
            .unwrap();
            assert_eq!(budget.storage(), 19);
            assert_eq!(budget.work(), 33);
            assert_eq!(work.failed_work(), None);
        }
    }
}

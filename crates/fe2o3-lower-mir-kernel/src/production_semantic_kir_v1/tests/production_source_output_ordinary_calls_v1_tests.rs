#[cfg(test)]
mod retained_ordinary_call_components_v1 {
    use super::*;
    use fe2o3_kernel_ir::{
        CanonicalKernelIrWorkBudgetV1 as Work, CanonicalKirBlockCoordinateV1 as Block,
        CanonicalKirFunctionCoordinateV1 as Function,
        CanonicalKirOperationCoordinateV1 as Coordinate,
    };

    fn function(index: u32, calls: std::ops::Range<usize>) -> SourceOutputOrdinaryFunctionV1 {
        SourceOutputOrdinaryFunctionV1 {
            input: Function(index),
            source: Some(SemanticFunctionIdV1::from_index(index)),
            calls,
            returns: 0..0,
            return_occurrences: 0,
            return_values: 0,
            refusal: None,
        }
    }

    fn call(caller: u32, target: usize) -> SourceOutputOrdinaryCallV1 {
        let coordinate = Coordinate {
            block: Block {
                function: Function(caller),
                block: 0,
            },
            operation: 0,
        };
        SourceOutputOrdinaryCallV1 {
            output: coordinate,
            original: coordinate,
            target: Some(target),
            defined: true,
            arguments: 0..0,
            results: 0..0,
            source_bound: true,
            typed: true,
        }
    }

    fn filled_index(
        budget: &mut AssertOriginBudgetV1<'_>,
    ) -> Result<SourceOutputOrdinaryCallIndexV1, ProductionSourceOutputErrorV1> {
        use fe2o3_kernel_ir::{
            CanonicalKirDefinitionCoordinateV1 as Definition, CanonicalKirUseCoordinateV1 as Use,
            ScalarType,
        };
        budget
            .charge_work(1)
            .map_err(ProductionSourceOutputErrorV1::Resource)?;
        budget
            .reserve_storage(std::mem::size_of::<SourceOutputOrdinaryCallIndexV1>())
            .map_err(ProductionSourceOutputErrorV1::Resource)?;
        let mut index = SourceOutputOrdinaryCallIndexV1::default();
        let coordinate = call(0, 0).output;
        let use_ = Use::OperationOperand {
            operation: coordinate,
            operand: 0,
        };
        let definition = Definition::FunctionArgument {
            function: Function(0),
            argument: 0,
        };
        source_output_ordinary_push_v1(&mut index.functions, function(0, 0..1), budget)?;
        source_output_ordinary_push_v1(&mut index.calls, call(0, 0), budget)?;
        source_output_ordinary_push_v1(
            &mut index.arguments,
            ProductionSourceOutputOrdinaryArgumentV1 {
                original_use: use_,
                output_use: use_,
                actual: definition,
                formal: definition,
                scalar: ScalarType::U32,
            },
            budget,
        )?;
        source_output_ordinary_push_v1(
            &mut index.results,
            ProductionSourceOutputOrdinaryResultV1 {
                original: definition,
                output: definition,
                scalar: ScalarType::U32,
            },
            budget,
        )?;
        source_output_ordinary_push_v1(
            &mut index.returns,
            ProductionSourceOutputOrdinaryReturnV1 {
                block: coordinate.block,
                value: Some((0, use_, use_, definition, ScalarType::U32)),
            },
            budget,
        )?;
        source_output_ordinary_push_v1(
            &mut index.aliases,
            SourceOutputOrdinaryAliasV1 {
                original: coordinate,
                owner: SemanticFunctionIdV1::from_index(0),
                function: SemanticFunctionIdV1::from_index(0),
                block: SemanticBlockIdV1::from_index(0),
                callee: SemanticFunctionIdV1::from_index(0),
                target: Function(0),
            },
            budget,
        )?;
        Ok(index)
    }

    #[test]
    fn every_numeric_roster_is_prepaid_before_its_allocation() {
        let header = std::mem::size_of::<SourceOutputOrdinaryCallIndexV1>();
        let payloads = [
            std::mem::size_of::<SourceOutputOrdinaryFunctionV1>(),
            std::mem::size_of::<SourceOutputOrdinaryCallV1>(),
            std::mem::size_of::<ProductionSourceOutputOrdinaryArgumentV1>(),
            std::mem::size_of::<ProductionSourceOutputOrdinaryResultV1>(),
            std::mem::size_of::<ProductionSourceOutputOrdinaryReturnV1>(),
            std::mem::size_of::<SourceOutputOrdinaryAliasV1>(),
        ]
        .map(|size| size * 4);
        for denied in 0..payloads.len() {
            let prior = payloads[..denied].iter().sum::<usize>();
            let mut work = Work::new(10_000);
            let mut budget =
                AssertOriginBudgetV1::new(&mut work, 19 + header + prior + payloads[denied] - 1);
            budget.charge_work(7).unwrap();
            budget.reserve_storage(19).unwrap();
            assert!(
                source_output_global_scratch_scope_v1(&mut budget, |budget| {
                    filled_index(budget).map(|index| {
                        drop(index);
                    })
                })
                .is_err()
            );
            assert_eq!(budget.storage(), 19);
            assert_eq!(budget.peak_storage(), 19 + header + prior);
            assert_eq!(
                budget.failed_storage(),
                Some(19 + header + prior + payloads[denied])
            );
            assert_eq!(budget.work(), 7 + 1 + denied * 2 + 1);
        }
    }

    #[test]
    fn closed_module_walk_propagates_deep_refusals_and_reuses_complete_callees() {
        for mutation in 0..5 {
            let mut functions = vec![function(0, 0..2), function(1, 2..3), function(2, 3..3)];
            let mut calls = vec![call(0, 1), call(0, 2), call(1, 2)];
            match mutation {
                1 => functions[2].refusal = Some("hidden memory"),
                2 => calls[2].source_bound = false,
                3 => calls[2].typed = false,
                4 => calls[2].target = Some(0),
                _ => {}
            }
            let mut work = Work::new(10_000);
            let mut budget = AssertOriginBudgetV1::new(&mut work, 10_000);
            budget.reserve_storage(19).unwrap();
            source_output_global_scratch_scope_v1(&mut budget, |budget| {
                source_output_ordinary_closure_v1(&mut functions, &calls, budget)
            })
            .unwrap();
            assert_eq!(functions[0].refusal.is_none(), mutation == 0);
            assert_eq!(functions[1].refusal.is_none(), mutation == 0);
            assert_eq!(budget.storage(), 19);
            assert!(budget.peak_storage() > 19);
        }
    }

    #[test]
    fn closure_component_has_exact_12_work_and_prepaid_storage_boundaries() {
        // One call-free helper: header1, first state push2, root2, first pending
        // push2, completion4, final empty-stack lookup1. Two Vec headers plus
        // minimum capacities four u8 states and four (usize,usize) stack rows.
        const WORK: usize = 1 + 2 + 2 + 2 + 4 + 1;
        let scratch = std::mem::size_of::<Vec<u8>>()
            + std::mem::size_of::<Vec<(usize, usize)>>()
            + 4 * std::mem::size_of::<u8>()
            + 4 * std::mem::size_of::<(usize, usize)>();
        for under in [false, true] {
            let mut work = Work::new(7 + WORK - usize::from(under));
            {
                let mut budget = AssertOriginBudgetV1::new(&mut work, 19 + scratch);
                budget.charge_work(7).unwrap();
                budget.reserve_storage(19).unwrap();
                let mut functions = [function(0, 0..0)];
                let result = source_output_global_scratch_scope_v1(&mut budget, |budget| {
                    source_output_ordinary_closure_v1(&mut functions, &[], budget)
                });
                assert_eq!(result.is_err(), under);
                assert_eq!(budget.work(), 7 + WORK - usize::from(under));
                assert_eq!(budget.storage(), 19);
                assert_eq!(budget.peak_storage(), 19 + scratch);
            }
            assert_eq!(work.failed_work(), under.then_some(7 + WORK));
        }
        let mut work = Work::new(10_000);
        let headers = std::mem::size_of::<Vec<u8>>() + std::mem::size_of::<Vec<(usize, usize)>>();
        let mut budget = AssertOriginBudgetV1::new(&mut work, 19 + headers + 3);
        budget.reserve_storage(19).unwrap();
        let mut functions = [function(0, 0..0)];
        assert!(
            source_output_global_scratch_scope_v1(&mut budget, |budget| {
                source_output_ordinary_closure_v1(&mut functions, &[], budget)
            })
            .is_err()
        );
        assert_eq!(budget.work(), 2);
        assert_eq!(budget.storage(), 19);
        assert_eq!(budget.peak_storage(), 19 + headers);
        assert_eq!(budget.failed_storage(), Some(19 + headers + 4));
    }

    #[test]
    fn index_rows_drop_before_error_and_panic_scope_restores_caller_floor() {
        for panic in [false, true] {
            let mut work = Work::new(10_000);
            let mut budget = AssertOriginBudgetV1::new(&mut work, 10_000);
            budget.charge_work(7).unwrap();
            budget.reserve_storage(19).unwrap();
            let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                source_output_global_scratch_scope_v1(&mut budget, |budget| {
                    let index = filled_index(budget)?;
                    assert!(index.payload()? > 0);
                    if panic {
                        panic!("paid ordinary-index component unwind");
                    }
                    Err::<(), _>(ProductionSourceOutputErrorV1::Invalid("paid index refusal"))
                })
            }));
            assert_eq!(result.is_err(), panic);
            if let Ok(result) = result {
                assert!(result.is_err());
            }
            assert_eq!(budget.storage(), 19);
            assert!(budget.peak_storage() > 19);
            assert_eq!(budget.work(), 20);
            budget.charge_work(3).unwrap();
            assert_eq!(budget.work(), 23);
        }
    }
}

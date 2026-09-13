#[cfg(test)]
mod block_state_resource_tests {
    use super::*;
    use crate::production_analysis::pliron_ir_identity::LivePlironStructuralIdentityProviderV1;
    use crate::production_analysis::pliron_pass_contract::PlironStructuralIdentityProviderV1;
    use crate::production_analysis::pliron_sparse_index::analyze_pliron_sparse_indices_v1;
    use dialect_kernel::{DIALECT_NAME, IndexConstantOp, IndexType, register_dialect};
    use pliron::{
        builtin::{op_interfaces::OneRegionInterface, ops::FuncOp, types::FunctionType},
        dialect::DialectName,
        r#type::TypeHandle,
    };

    fn unlimited() -> ProductionAnalysisResourceLimitsV1 {
        ProductionAnalysisResourceLimitsV1::new(usize::MAX, usize::MAX)
    }

    fn setup() -> Context {
        let mut context = Context::new();
        register_dialect(&mut context, &DialectName::try_new(DIALECT_NAME).unwrap()).unwrap();
        context
    }

    fn function(context: &mut Context, name: &str, widths: &[usize]) -> FuncOp {
        let index: TypeHandle = IndexType::get(context).into();
        let function = FuncOp::new(
            context,
            name.try_into().unwrap(),
            FunctionType::get(context, vec![index; widths[0]], vec![]),
        );
        let entry = function.get_entry_block(context);
        ReturnOp::new(context)
            .get_operation()
            .insert_at_back(entry, context);
        for width in &widths[1..] {
            let block = BasicBlock::new(context, None, vec![index; *width]);
            block.insert_at_back(function.get_region(context), context);
            ReturnOp::new(context)
                .get_operation()
                .insert_at_back(block, context);
        }
        function
    }

    fn authenticated_inputs(
        context: &Context,
        function: &FuncOp,
    ) -> (
        ProductionAnalysisInputCensusV1,
        BoundedPlironFunctionInventoryV1,
    ) {
        let census = LivePlironStructuralIdentityProviderV1::new(context, function)
            .capture_with_resource_limits_v1(unlimited())
            .ok()
            .expect("the actual function passes structural identity verification")
            .input_census;
        let inventory = BoundedPlironFunctionInventoryV1::collect(context, function).unwrap();
        (census, inventory)
    }

    #[test]
    fn trace_block_state_census_includes_entry_and_unreachable_widths() {
        for (widths, expected) in [([3, 0, 2], 3), ([2, 0, 3], 3), ([0, 0, 0], 0)] {
            let context = &mut setup();
            let function = function(context, "trace_width", &widths);
            let (census, inventory) = authenticated_inputs(context, &function);
            assert_eq!(
                (census.blocks, census.block_arguments),
                (3, widths.iter().sum())
            );
            let result = collect_trace_block_state_census_v1(
                context,
                &inventory,
                census,
                ProductionAnalysisResourceLimitsV1::new(4, 3),
            )
            .unwrap();
            assert_eq!((result.max_block_arguments, result.work), (expected, 4));
        }
    }

    #[test]
    fn trace_block_state_census_prepays_and_rejects_count_mismatches() {
        let context = &mut setup();
        let function = function(context, "trace_width_prefix", &[2, 0, 3]);
        let (census, inventory) = authenticated_inputs(context, &function);
        for (limits, resource) in [
            (
                ProductionAnalysisResourceLimitsV1::new(3, 3),
                "work upper bound",
            ),
            (
                ProductionAnalysisResourceLimitsV1::new(4, 2),
                "peak storage upper bound",
            ),
        ] {
            let error = collect_trace_block_state_census_v1(context, &inventory, census, limits)
                .err()
                .expect("the complete census is prepaid");
            assert_eq!(
                error,
                ProductionAnalysisResourceLimitV1 {
                    phase: ProductionAnalysisResourcePhaseV1::InvocationTrace,
                    resource,
                }
            );
        }
        for bad in [
            ProductionAnalysisInputCensusV1 {
                blocks: 2,
                ..census
            },
            ProductionAnalysisInputCensusV1 {
                blocks: 4,
                ..census
            },
            ProductionAnalysisInputCensusV1 {
                block_arguments: 4,
                ..census
            },
            ProductionAnalysisInputCensusV1 {
                block_arguments: 6,
                ..census
            },
        ] {
            assert_eq!(
                collect_trace_block_state_census_v1(context, &inventory, bad, unlimited()).err(),
                Some(trace_block_state_census_error_v1()),
            );
        }
        let overflow = ProductionAnalysisInputCensusV1 {
            blocks: usize::MAX,
            ..census
        };
        assert_eq!(
            collect_trace_block_state_census_v1(context, &inventory, overflow, unlimited()).err(),
            Some(trace_resource_overflow_v1()),
        );
    }

    #[test]
    fn trace_block_state_bound_has_independent_literal_limits() {
        let context = &mut setup();
        let function = function(context, "trace_width_bound", &[2, 0, 3]);
        let (census, inventory) = authenticated_inputs(context, &function);
        let sparse = analyze_pliron_sparse_indices_v1(context, &function).unwrap();
        assert_eq!(sparse.launch_extents(), &[1]);
        assert_eq!(
            static_invocation_shape_for_resource_v1(&sparse, None),
            Some((1, 1))
        );
        assert_eq!(
            (
                census.blocks,
                census.operations,
                census.results,
                census.block_arguments
            ),
            (3, 3, 0, 5)
        );
        let preflight = preflight_invocation_trace_resource_upper_bound_v1(
            context,
            Some(&inventory),
            census,
            Some(&sparse),
            None,
            unlimited(),
        )
        .unwrap();
        let bound = preflight.attempt_upper_bound();
        // 4*O + B + (B+1) census + one coordinate decode
        // + (MAX+1)*(step1 + value32 + setup3).
        const WORK: usize = 37_748_792;
        // One invocation header and coordinate plus MAX rank-8 event rows.
        const RETAINED: usize = 17_825_794;
        // Block map6 + global environment15 + MAX*(max-width3+key2)
        // + launch extent1 + evaluator scratch1836. The census ends first.
        const PEAK: usize = 23_070_532;
        assert_eq!(
            WORK,
            12 + 3 + 4 + 1 + (MAX_PLIRON_TRACE_TOTAL_STEPS_V1 + 1) * 36
        );
        assert_eq!(RETAINED, 2 + MAX_PLIRON_TRACE_TOTAL_STEPS_V1 * 17);
        assert_eq!(
            PEAK,
            RETAINED + 6 + 15 + MAX_PLIRON_TRACE_TOTAL_STEPS_V1 * 5 + 1 + 1836
        );
        assert_eq!(bound.work_upper_bound(), WORK);
        assert_eq!(bound.retained_storage_upper_bound(), RETAINED);
        assert_eq!(bound.peak_storage_upper_bound(), PEAK);
        assert_eq!(
            preflight_invocation_trace_resource_upper_bound_v1(
                context,
                Some(&inventory),
                census,
                Some(&sparse),
                None,
                ProductionAnalysisResourceLimitsV1::new(WORK, PEAK),
            )
            .unwrap(),
            preflight,
        );
        for (work, storage, resource) in [
            (WORK - 1, PEAK, "work upper bound"),
            (WORK, PEAK - 1, "peak storage upper bound"),
        ] {
            assert_eq!(
                preflight_invocation_trace_resource_upper_bound_v1(
                    context,
                    Some(&inventory),
                    census,
                    Some(&sparse),
                    None,
                    ProductionAnalysisResourceLimitsV1::new(work, storage),
                ),
                Err(ProductionAnalysisResourceLimitV1 {
                    phase: ProductionAnalysisResourcePhaseV1::InvocationTrace,
                    resource,
                }),
            );
        }
        let coarse =
            invocation_trace_resource_upper_bound_for_shape_v1(census, 1, 1, unlimited()).unwrap();
        assert_eq!(
            bound.work_upper_bound(),
            coarse.upper_bound().work_upper_bound() + 4
        );
        assert_eq!(
            coarse.upper_bound().peak_storage_upper_bound() - PEAK,
            2 * MAX_PLIRON_TRACE_TOTAL_STEPS_V1
        );
    }

    #[test]
    fn trace_block_state_equal_aggregates_do_not_supply_the_width() {
        let context = &mut setup();
        let first = function(context, "trace_width_first", &[0, 1, 4]);
        let second = function(context, "trace_width_second", &[0, 2, 3]);
        let mut peaks = Vec::new();
        for (function, width) in [(&first, 4), (&second, 3)] {
            let (census, inventory) = authenticated_inputs(context, function);
            assert_eq!(
                (
                    census.blocks,
                    census.operations,
                    census.results,
                    census.block_arguments
                ),
                (3, 3, 0, 5)
            );
            let actual =
                collect_trace_block_state_census_v1(context, &inventory, census, unlimited())
                    .unwrap();
            assert_eq!(actual.max_block_arguments, width);
            let sparse = analyze_pliron_sparse_indices_v1(context, function).unwrap();
            let result = preflight_invocation_trace_resource_upper_bound_v1(
                context,
                Some(&inventory),
                census,
                Some(&sparse),
                None,
                unlimited(),
            )
            .unwrap();
            peaks.push(result.attempt_upper_bound().peak_storage_upper_bound());
        }
        assert_eq!(peaks[0] - peaks[1], MAX_PLIRON_TRACE_TOTAL_STEPS_V1);
    }

    #[test]
    fn trace_block_state_static_and_rejected_launch_inventory_order_is_preserved() {
        let context = &mut setup();
        let function = function(context, "trace_width_launch", &[0]);
        let (census, inventory) = authenticated_inputs(context, &function);
        let sparse = analyze_pliron_sparse_indices_v1(context, &function).unwrap();
        assert_eq!(
            preflight_invocation_trace_resource_upper_bound_v1(
                context,
                None,
                census,
                Some(&sparse),
                None,
                unlimited(),
            ),
            Err(ProductionAnalysisResourceLimitV1 {
                phase: ProductionAnalysisResourcePhaseV1::InvocationTrace,
                resource: "invocation trace inventory unavailable",
            }),
        );
        let layout = PlironExecutionLayoutV1 {
            grid: 0,
            global_extents: [1, 1, 1],
            workgroup_extents: [1, 1, 1],
            subgroup_size: 1,
            execution_domain: ExecutionDomainAttr::FullPhysicalWorkgroups,
        };
        for supplied_layout in [None, Some(layout)] {
            let admitted = preflight_invocation_trace_resource_upper_bound_v1(
                context,
                Some(&inventory),
                census,
                Some(&sparse),
                supplied_layout,
                unlimited(),
            )
            .unwrap();
            assert!(admitted.exact_admission.is_some());
            let traces = trace_pliron_invocations_with_inputs_v1(
                context,
                &inventory,
                &sparse,
                supplied_layout,
            )
            .unwrap();
            assert_eq!(traces.len(), 1);
            assert!(admitted.exact_admission(Ok(&traces)).unwrap().is_some());
        }
        // A rejected/dynamic attempt never inspects an unavailable inventory
        // or the irrelevant block totals. Layout presence alone is not sparse authority.
        let bad_census = ProductionAnalysisInputCensusV1 {
            blocks: usize::MAX,
            block_arguments: usize::MAX,
            ..census
        };
        for (sparse, supplied_layout, work, peak) in [
            (None, None, 33, 16),
            (None, Some(layout), 23, 11),
            (
                Some(&sparse),
                Some(PlironExecutionLayoutV1 {
                    global_extents: [0, 1, 1],
                    ..layout
                }),
                23,
                11,
            ),
        ] {
            let result = preflight_invocation_trace_resource_upper_bound_v1(
                context,
                None,
                bad_census,
                sparse,
                supplied_layout,
                ProductionAnalysisResourceLimitsV1::new(work, peak),
            )
            .unwrap();
            assert!(result.exact_admission.is_none());
            assert_eq!(result.attempt_upper_bound().work_upper_bound(), work);
            assert_eq!(
                result.attempt_upper_bound().peak_storage_upper_bound(),
                peak
            );
        }
    }

    #[test]
    fn trace_block_state_repeated_keys_and_rejecting_step_keep_the_same_boundary() {
        for width in [0, 1] {
            let context = &mut setup();
            let function = FuncOp::new(
                context,
                "trace_width_cycle".try_into().unwrap(),
                FunctionType::get(context, vec![], vec![]),
            );
            let entry = function.get_entry_block(context);
            let index: TypeHandle = IndexType::get(context).into();
            let cycle = BasicBlock::new(context, None, vec![index; width]);
            cycle.insert_at_back(function.get_region(context), context);
            let mut inputs = Vec::new();
            if width != 0 {
                let zero = IndexConstantOp::new(context, 0);
                zero.get_operation().insert_at_back(entry, context);
                inputs.push(zero.result(context));
            }
            BranchArgsOp::new(context, inputs, cycle)
                .get_operation()
                .insert_at_back(entry, context);
            let arguments = (0..width)
                .map(|ordinal| cycle.deref(context).get_argument(ordinal))
                .collect();
            BranchArgsOp::new(context, arguments, cycle)
                .get_operation()
                .insert_at_back(cycle, context);
            let (census, inventory) = authenticated_inputs(context, &function);
            let sparse = analyze_pliron_sparse_indices_v1(context, &function).unwrap();
            let admitted = preflight_invocation_trace_resource_upper_bound_v1(
                context,
                Some(&inventory),
                census,
                Some(&sparse),
                None,
                unlimited(),
            )
            .unwrap();
            let failure =
                trace_pliron_invocations_with_inputs_v1(context, &inventory, &sparse, None)
                    .unwrap_err();
            assert_eq!(
                failure,
                PlironTraceFailureV1::CyclicControlFlow { block: 1 }
            );
            assert_eq!(admitted.exact_admission(Err(failure)), Ok(None));

            // A duplicate candidate coexists with old keys but consumes its
            // own admitted step. The next rejected step allocates no key.
            let mut visited = HashSet::new();
            assert!(visited.insert((1_usize, vec![Some(0_u64); width])));
            let candidate = (1_usize, vec![Some(0_u64); width]);
            assert_eq!(
                visited.len() * (width + 2) + candidate.1.len() + 2,
                2 * (width + 2)
            );
            assert!(!visited.insert(candidate));
            let mut steps = MAX_PLIRON_TRACE_TOTAL_STEPS_V1 - 1;
            assert_eq!(charge_trace_work_v1(&mut steps, 1), Ok(()));
            let mut allocated = false;
            let denied = charge_trace_work_v1(&mut steps, 1).map(|()| {
                allocated = true;
                vec![None::<u64>; width]
            });
            assert_eq!(denied, Err(PlironTraceFailureV1::ResourceLimit));
            assert!(!allocated);
            assert_eq!(steps, MAX_PLIRON_TRACE_TOTAL_STEPS_V1 + 1);
        }
    }
}

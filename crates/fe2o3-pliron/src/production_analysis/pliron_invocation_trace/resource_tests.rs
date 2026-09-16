#[cfg(test)]
mod resource_upper_bound_tests {
    use super::*;

    fn census() -> ProductionAnalysisInputCensusV1 {
        ProductionAnalysisInputCensusV1 {
            blocks: 3,
            operations: 11,
            operands: 17,
            results: 9,
            successors: 4,
            block_arguments: 5,
            attributes: 7,
            type_nodes: 13,
            identifier_bytes: 19,
            canonical_bytes: 101,
            max_operation_arity: 4,
            max_successor_arity: 2,
            pipeline_creates: 0,
            pipeline_events: 0,
            ranked_accesses: 0,
            workgroup_ranked_accesses: 0,
            allocation_effects: 0,
            collective_transpose_candidates: 0,
            ownership_contracts: 0,
            effect_refinement_contracts: 0,
            index_lt_branch_candidates: 0,
            memory_bounds_guard_candidates: 0,
            semantic_definitions: 0,
            semantic_refinement_contracts: 0,
            native_switch_verification_work: 0,
            native_switch_verification_scratch: 0,
        }
    }

    #[test]
    fn trace_shape_bound_is_admitted_at_exact_and_rejected_one_under() {
        let unlimited = ProductionAnalysisResourceLimitsV1::new(usize::MAX, usize::MAX);
        let exact =
            invocation_trace_resource_upper_bound_for_shape_v1(census(), 8, 3, unlimited).unwrap();
        let bound = exact.upper_bound();
        assert_eq!(exact.invocation_count(), 8);
        assert_eq!(exact.launch_rank(), 3);
        assert_eq!(exact.event_upper_bound(), MAX_PLIRON_TRACE_TOTAL_STEPS_V1);
        let charged_steps = MAX_PLIRON_TRACE_TOTAL_STEPS_V1 + 1;
        let value_visits = MAX_PLIRON_TRACE_TOTAL_STEPS_V1 + 1;
        let expected_evaluation_work = value_visits * (8 + 16 + 8) + value_visits * 3;
        let expected_work = census().operations * 4
            + census().blocks
            + 8 * 3
            + charged_steps
            + expected_evaluation_work;
        let expected_retained =
            8 + 8 * 3 + MAX_PLIRON_TRACE_TOTAL_STEPS_V1 * (MAX_RANKED_MEMORY_RANK * 2 + 1);
        let expected_evaluation_temporary = (65 * 2 + 1) * 8 + 65 * 8 + 65 * 4 + 8;
        let expected_temporary = census().blocks * 2
            + census().results
            + census().block_arguments * 3
            + MAX_PLIRON_TRACE_TOTAL_STEPS_V1 * (census().block_arguments + 2)
            + 3
            + expected_evaluation_temporary;
        assert_eq!(bound.work_upper_bound(), expected_work);
        assert_eq!(bound.retained_storage_upper_bound(), expected_retained);
        assert_eq!(
            bound.peak_storage_upper_bound(),
            expected_retained + expected_temporary
        );

        assert_eq!(
            invocation_trace_resource_upper_bound_for_shape_v1(
                census(),
                8,
                3,
                ProductionAnalysisResourceLimitsV1::new(
                    bound.work_upper_bound() - 1,
                    bound.peak_storage_upper_bound(),
                ),
            ),
            Err(ProductionAnalysisResourceLimitV1 {
                phase: ProductionAnalysisResourcePhaseV1::InvocationTrace,
                resource: "work upper bound",
            })
        );
        assert_eq!(
            invocation_trace_resource_upper_bound_for_shape_v1(
                census(),
                8,
                3,
                ProductionAnalysisResourceLimitsV1::new(
                    bound.work_upper_bound(),
                    bound.peak_storage_upper_bound() - 1,
                ),
            ),
            Err(ProductionAnalysisResourceLimitV1 {
                phase: ProductionAnalysisResourcePhaseV1::InvocationTrace,
                resource: "peak storage upper bound",
            })
        );
    }

    #[test]
    fn successful_trace_refines_downstream_cardinality_from_the_cache() {
        let unlimited = ProductionAnalysisResourceLimitsV1::new(usize::MAX, usize::MAX);
        let attempt =
            invocation_trace_resource_upper_bound_for_shape_v1(census(), 8, 3, unlimited).unwrap();
        let preflight = ProductionInvocationTraceResourcePreflightV1 {
            attempt_upper_bound: attempt.upper_bound(),
            exact_admission: Some(attempt),
        };
        let traces = vec![
            PlironInvocationTraceV1 {
                invocation: vec![0, 0, 0],
                grid: 0,
                workgroup: 0,
                subgroup: 0,
                lane: 0,
                events: vec![
                    PlironTraceEventV1::Trap {
                        location: PlironTraceLocationV1 {
                            block: 0,
                            operation: 0,
                        },
                    },
                    PlironTraceEventV1::Trap {
                        location: PlironTraceLocationV1 {
                            block: 0,
                            operation: 1,
                        },
                    },
                ],
            },
            PlironInvocationTraceV1 {
                invocation: vec![1, 0, 0],
                grid: 0,
                workgroup: 0,
                subgroup: 0,
                lane: 1,
                events: vec![PlironTraceEventV1::Trap {
                    location: PlironTraceLocationV1 {
                        block: 0,
                        operation: 0,
                    },
                }],
            },
        ];

        let exact = preflight.exact_admission(Ok(&traces)).unwrap().unwrap();
        assert_eq!(exact.upper_bound(), attempt.upper_bound());
        assert_eq!(exact.invocation_count(), 2);
        assert_eq!(exact.launch_rank(), 3);
        assert_eq!(exact.event_upper_bound(), 3);
    }

    #[test]
    fn successful_trace_cardinality_cannot_exceed_the_attempt_admission() {
        let attempt = ProductionInvocationTraceResourceAdmissionV1 {
            upper_bound: ProductionAnalysisResourceUpperBoundV1::zero(),
            invocation_count: 1,
            launch_rank: 3,
            event_upper_bound: 1,
        };
        let preflight = ProductionInvocationTraceResourcePreflightV1 {
            attempt_upper_bound: attempt.upper_bound(),
            exact_admission: Some(attempt),
        };
        let traces = [PlironInvocationTraceV1 {
            invocation: vec![0, 0, 0],
            grid: 0,
            workgroup: 0,
            subgroup: 0,
            lane: 0,
            events: vec![
                PlironTraceEventV1::Trap {
                    location: PlironTraceLocationV1 {
                        block: 0,
                        operation: 0,
                    },
                },
                PlironTraceEventV1::Trap {
                    location: PlironTraceLocationV1 {
                        block: 0,
                        operation: 1,
                    },
                },
            ],
        }];

        assert_eq!(
            preflight.exact_admission(Ok(&traces)),
            Err(trace_authenticated_cardinality_error_v1())
        );
    }

    #[test]
    fn failed_trace_materialization_has_no_downstream_exact_admission() {
        let unlimited = ProductionAnalysisResourceLimitsV1::new(usize::MAX, usize::MAX);
        let attempt =
            invocation_trace_resource_upper_bound_for_shape_v1(census(), 8, 3, unlimited).unwrap();
        assert_eq!(attempt.event_upper_bound(), MAX_PLIRON_TRACE_TOTAL_STEPS_V1);
        let preflight = ProductionInvocationTraceResourcePreflightV1 {
            attempt_upper_bound: attempt.upper_bound(),
            exact_admission: Some(attempt),
        };

        assert_eq!(
            preflight.exact_admission(Err(PlironTraceFailureV1::UnresolvedBranch { block: 1 })),
            Ok(None)
        );
    }

    #[test]
    fn execution_layout_scan_is_admitted_before_extraction() {
        let exact = preflight_execution_layout_resource_upper_bound_v1(
            census(),
            ProductionAnalysisResourceLimitsV1::new(23, 10),
        )
        .unwrap();
        assert_eq!(exact.work_upper_bound(), 23);
        assert_eq!(exact.retained_storage_upper_bound(), 10);
        assert_eq!(
            preflight_execution_layout_resource_upper_bound_v1(
                census(),
                ProductionAnalysisResourceLimitsV1::new(22, 10),
            ),
            Err(ProductionAnalysisResourceLimitV1 {
                phase: ProductionAnalysisResourcePhaseV1::LaunchContract,
                resource: "work upper bound",
            })
        );
        assert_eq!(
            preflight_execution_layout_resource_upper_bound_v1(
                census(),
                ProductionAnalysisResourceLimitsV1::new(23, 9),
            ),
            Err(ProductionAnalysisResourceLimitV1 {
                phase: ProductionAnalysisResourcePhaseV1::LaunchContract,
                resource: "peak storage upper bound",
            })
        );
    }

    #[test]
    fn rejected_trace_attempt_is_admitted_at_exact_and_rejected_one_under() {
        let context = Context::new();
        let unlimited = ProductionAnalysisResourceLimitsV1::new(usize::MAX, usize::MAX);
        let preflight = preflight_invocation_trace_resource_upper_bound_v1(
            &context,
            None,
            census(),
            None,
            None,
            unlimited,
        )
        .unwrap();
        assert_eq!(
            preflight.exact_admission(Err(PlironTraceFailureV1::DynamicLaunch { dimension: 0 })),
            Ok(None)
        );
        let bound = preflight.attempt_upper_bound();
        assert!(bound.work_upper_bound() > 0);
        assert!(bound.retained_storage_upper_bound() > 0);

        assert_eq!(
            preflight_invocation_trace_resource_upper_bound_v1(
                &context,
                None,
                census(),
                None,
                None,
                ProductionAnalysisResourceLimitsV1::new(
                    bound.work_upper_bound() - 1,
                    bound.peak_storage_upper_bound(),
                ),
            ),
            Err(ProductionAnalysisResourceLimitV1 {
                phase: ProductionAnalysisResourcePhaseV1::InvocationTrace,
                resource: "work upper bound",
            })
        );
        assert_eq!(
            preflight_invocation_trace_resource_upper_bound_v1(
                &context,
                None,
                census(),
                None,
                None,
                ProductionAnalysisResourceLimitsV1::new(
                    bound.work_upper_bound(),
                    bound.peak_storage_upper_bound() - 1,
                ),
            ),
            Err(ProductionAnalysisResourceLimitV1 {
                phase: ProductionAnalysisResourcePhaseV1::InvocationTrace,
                resource: "peak storage upper bound",
            })
        );
    }

    #[test]
    fn trace_shape_bound_rejects_overflow_before_admission() {
        let mut overflowing = census();
        overflowing.blocks = usize::MAX;
        assert_eq!(
            invocation_trace_resource_upper_bound_for_shape_v1(
                overflowing,
                2,
                3,
                ProductionAnalysisResourceLimitsV1::new(usize::MAX, usize::MAX),
            ),
            Err(trace_resource_overflow_v1())
        );
    }
}

#[cfg(test)]
mod value_evaluator_tests {
    use super::*;
    use crate::production_analysis::pliron_sparse_index::analyze_pliron_sparse_indices_v1;
    use dialect_kernel::{DIALECT_NAME, IndexConstantOp, register_dialect};
    use pliron::{
        builtin::{ops::FuncOp, types::FunctionType},
        dialect::DialectName,
    };

    fn setup_sparse_analysis(context: &mut Context) -> crate::SparseIndexAnalysisV1 {
        register_dialect(
            context,
            &DialectName::try_new(DIALECT_NAME).expect("valid kernel dialect name"),
        )
        .expect("register kernel dialect");
        dialect_gpu::register_dialect(context).expect("register gpu dialect");
        let function = FuncOp::new(
            context,
            "trace_value_evaluator"
                .try_into()
                .expect("valid function name"),
            FunctionType::get(context, vec![], vec![]),
        );
        let ret = ReturnOp::new(context);
        ret.get_operation()
            .insert_at_back(function.get_entry_block(context), context);
        analyze_pliron_sparse_indices_v1(context, &function).expect("sparse analysis")
    }

    #[test]
    fn duplicated_subexpression_depth_64_is_memoized() {
        let context = &mut Context::new();
        let sparse = setup_sparse_analysis(context);
        let base = IndexConstantOp::new(context, 0);
        let base_value = base.result(context);
        let mut value = base_value;
        for _ in 0..64 {
            value =
                IndexBinaryOp::new(context, IndexBinaryKindAttr::Add, value, value).result(context);
        }
        let environment = HashMap::from([(base_value, 0)]);
        let mut total_visits = 0;

        assert_eq!(
            evaluate_trace_value_v1(
                context,
                &sparse,
                &[],
                &environment,
                &mut total_visits,
                value,
            ),
            Ok(Some(0))
        );
        assert_eq!(total_visits, MAX_PLIRON_TRACE_VALUE_VISITS_PER_QUERY_V1);
    }

    #[test]
    fn value_evaluator_charges_global_max_plus_one_rejection() {
        let context = &mut Context::new();
        let sparse = setup_sparse_analysis(context);
        let base = IndexConstantOp::new(context, 0);
        let base_value = base.result(context);
        let environment = HashMap::from([(base_value, 7)]);
        let mut total_visits = MAX_PLIRON_TRACE_TOTAL_STEPS_V1;

        assert_eq!(
            evaluate_trace_value_v1(
                context,
                &sparse,
                &[],
                &environment,
                &mut total_visits,
                base_value,
            ),
            Err(PlironTraceFailureV1::ResourceLimit)
        );
        assert_eq!(total_visits, MAX_PLIRON_TRACE_TOTAL_STEPS_V1 + 1);
    }

    #[test]
    fn value_evaluator_charges_per_query_max_plus_one_rejection() {
        let context = &mut Context::new();
        let sparse = setup_sparse_analysis(context);
        let base = IndexConstantOp::new(context, 0);
        let base_value = base.result(context);
        let mut value = base_value;
        for _ in 0..65 {
            value =
                IndexBinaryOp::new(context, IndexBinaryKindAttr::Add, value, value).result(context);
        }
        let environment = HashMap::from([(base_value, 0)]);
        let mut total_visits = 0;

        assert_eq!(
            evaluate_trace_value_v1(
                context,
                &sparse,
                &[],
                &environment,
                &mut total_visits,
                value,
            ),
            Err(PlironTraceFailureV1::ResourceLimit)
        );
        assert_eq!(total_visits, MAX_PLIRON_TRACE_VALUE_VISITS_PER_QUERY_V1 + 1);
    }
}

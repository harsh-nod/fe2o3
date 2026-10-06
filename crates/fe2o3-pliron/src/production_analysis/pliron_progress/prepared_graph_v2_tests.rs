#[cfg(test)]
mod prepared_graph_v2_tests {
    use super::*;
    use crate::production_analysis::pliron_ir_identity::LivePlironStructuralIdentityProviderV1;
    use crate::production_analysis::pliron_pass_contract::{
        PRODUCTION_PLIRON_PASS_CONTRACTS_V1, PlironPassPreservationErrorV1,
        PlironStructuralIdentityProviderV1, begin_production_pliron_pass_contract_session_v1,
    };
    use dialect_kernel::ReturnOp;
    use pliron::{
        builtin::{op_interfaces::OneRegionInterface, types::FunctionType},
        dialect::DialectName,
    };

    fn context() -> Context {
        let mut context = Context::new();
        dialect_kernel::register_dialect(
            &mut context,
            &DialectName::try_new(dialect_kernel::DIALECT_NAME).unwrap(),
        )
        .unwrap();
        context
    }

    fn chain(context: &mut Context, count: usize, cycle: bool) -> FuncOp {
        let signature = FunctionType::get(context, vec![], vec![]);
        let function = FuncOp::new(context, "prepared_graph".try_into().unwrap(), signature);
        let mut blocks = vec![function.get_entry_block(context)];
        for i in 1..count {
            let block = BasicBlock::new(context, Some(format!("b{i}").try_into().unwrap()), vec![]);
            block.insert_at_back(function.get_region(context), context);
            blocks.push(block);
        }
        for (i, block) in blocks.iter().copied().enumerate() {
            let operation = match blocks.get(i + 1) {
                Some(next) => BranchOp::new(context, *next).get_operation(),
                None if cycle => BranchOp::new(context, block).get_operation(),
                None => ReturnOp::new(context).get_operation(),
            };
            operation.insert_at_back(block, context);
        }
        function
    }

    fn census(context: &Context, function: &FuncOp) -> ProductionAnalysisInputCensusV1 {
        LivePlironStructuralIdentityProviderV1::new(context, function)
            .capture_with_resource_limits_v1(
                ProductionAnalysisResourceLimitsV1::production_hard_ceiling(),
            )
            .ok()
            .unwrap()
            .input_census
    }

    fn scoped(
        context: &Context,
        function: &FuncOp,
        graph: &PreparedProgressGraphV2<'_>,
    ) -> Result<PlironProgressReportV1, PlironPassPreservationErrorV1> {
        scoped_observed(context, function, graph, None)
    }

    fn scoped_observed(
        context: &Context,
        function: &FuncOp,
        graph: &PreparedProgressGraphV2<'_>,
        observer: ProgressObserverV1<'_, '_, '_>,
    ) -> Result<PlironProgressReportV1, PlironPassPreservationErrorV1> {
        let mut session = begin_production_pliron_pass_contract_session_v1(
            LivePlironStructuralIdentityProviderV1::new(context, function),
        )?;
        for contract in &PRODUCTION_PLIRON_PASS_CONTRACTS_V1[..8] {
            session
                .run_contiguous_pass(contract.pass(), || Ok::<_, ()>(()))?
                .unwrap();
        }
        session
            .run_scoped_semantic_refinement_with_resource_limits_v1(
                ProductionAnalysisResourceLimitsV1::production_hard_ceiling(),
                |input| {
                    Ok(Ok::<_, ()>(
                        run_pliron_progress_with_prepared_graph_v2(input, Some(graph), observer)?
                            .report,
                    ))
                },
            )
            .map(Result::unwrap)
    }

    #[test]
    fn graph_first_literal_work_and_retained_peak_cutoffs_are_independent() {
        assert!(
            std::mem::size_of::<PreparedProgressGraphV2<'_>>() <= 64 * std::mem::size_of::<usize>()
        );
        let census = ProductionAnalysisInputCensusV1 {
            blocks: 3,
            operations: 4,
            operands: 2,
            results: 1,
            attributes: 2,
            block_arguments: 1,
            successors: 3,
            ..Default::default()
        };
        // No region census exists: pay all 4096 permitted regions, including
        // possible multiple/empty regions per operation, then other records.
        let structural = 4096 + 3 + 4 + 2 + 1 + 2 + 1 + 3;
        let operation_lookup = 64 + (4 * 4 + 20) * 32;
        let block_lookup = 64 + (4 * 3 + 20) * 32;
        let work = 64
            + 16 * structural
            + 64 * (3 + 3)
            + 3 * 3
            + 3 * 4 * operation_lookup
            + (3 * 3 + 3) * block_lookup;
        let retained = 64 + 1024 + 48 * 3 + 12 * 3 + 8 * 4;
        let temporary = 64 + 1024 + 16 * 3 + 4 * 3 + 8 * 4 + 2 * structural;
        let exact = preflight_progress_graph_resource_upper_bound_v2(
            census,
            ProductionAnalysisResourceLimitsV1::new(work, retained + temporary),
        )
        .unwrap();
        assert_eq!(exact.work_upper_bound(), work);
        assert_eq!(exact.retained_storage_upper_bound(), retained);
        assert_eq!(exact.peak_storage_upper_bound(), retained + temporary);
        for (work, peak, resource) in [
            (work - 1, retained + temporary, "work upper bound"),
            (work, retained + temporary - 1, "peak storage upper bound"),
        ] {
            assert_eq!(
                preflight_progress_graph_resource_upper_bound_v2(
                    census,
                    ProductionAnalysisResourceLimitsV1::new(work, peak)
                ),
                Err(progress_resource_error_v1(resource))
            );
        }
        for census in [
            ProductionAnalysisInputCensusV1 {
                blocks: MAX_PLIRON_PROGRESS_BLOCKS_V1 + 1,
                ..Default::default()
            },
            ProductionAnalysisInputCensusV1 {
                operands: usize::MAX,
                ..Default::default()
            },
        ] {
            assert_eq!(
                preflight_progress_graph_resource_upper_bound_v2(
                    census,
                    ProductionAnalysisResourceLimitsV1::new(usize::MAX, usize::MAX)
                ),
                Err(progress_resource_error_v1("progress structural hard limit"))
            );
        }
    }

    #[test]
    fn graph_first_retained_bound_covers_actual_typed_capacities_and_table_control_groups() {
        use std::mem::{align_of, size_of, size_of_val};
        fn words(bytes: usize) -> usize {
            bytes.div_ceil(size_of::<usize>())
        }
        fn payload<T>(values: &Vec<T>) -> usize {
            words(values.capacity() * size_of::<T>())
        }
        fn rows<T>(values: &Vec<Vec<T>>) -> usize {
            payload(values) + values.iter().map(payload).sum::<usize>()
        }
        fn table<K, V>(values: &HashMap<K, V>) -> usize {
            let capacity = values.capacity();
            if capacity == 0 {
                return 0;
            }
            let buckets = if capacity < 8 {
                capacity + 1
            } else {
                capacity * 8 / 7
            };
            assert!(buckets <= 4 * values.len() + 4);
            let alignment = align_of::<(K, V)>().max(16);
            let entries = (buckets * size_of::<(K, V)>()).div_ceil(alignment) * alignment;
            words(entries + buckets + 16)
        }
        for count in [1, 3, 4, 8, 31, 257] {
            let mut context = context();
            let function = chain(&mut context, count, false);
            let bound = preflight_progress_graph_resource_upper_bound_v2(
                census(&context, &function),
                ProductionAnalysisResourceLimitsV1::production_hard_ceiling(),
            )
            .unwrap();
            let owner = PreparedProgressGraphV2::new(&context, &function, None);
            let graph = owner.state.as_ref().ok().unwrap();
            let actual = words(size_of_val(&owner))
                + payload(&graph.inventory.root_blocks)
                + table(&graph.inventory.root_operation_blocks)
                + table(&graph.block_indices)
                + rows(&graph.graph.edges)
                + rows(&graph.graph.unconditional_edges)
                + rows(&graph.graph.predecessors)
                + rows(&graph.graph.incoming)
                + payload(&graph.reachable)
                + payload(&graph.definitely_reachable)
                + rows(&graph.components);
            assert!(
                actual <= bound.retained_storage_upper_bound(),
                "{count}: {actual}"
            );
        }
    }

    #[test]
    fn graph_first_large_dag_uses_only_graph_and_constant_continuation_work() {
        let mut context = context();
        let function = chain(&mut context, 434, false);
        let graph = PreparedProgressGraphV2::new(&context, &function, None);
        let census = census(&context, &function);
        assert_eq!(
            (census.blocks, census.operations, census.successors),
            (434, 434, 433)
        );
        let limits = ProductionAnalysisResourceLimitsV1::new(usize::MAX, usize::MAX);
        let preparation = preflight_progress_graph_resource_upper_bound_v2(census, limits).unwrap();
        let continuation = graph.continuation_bound(census, limits).unwrap();
        let combined = preparation
            .checked_then_retain(continuation, ProductionAnalysisResourcePhaseV1::Progress)
            .unwrap();
        let bounded = ProductionAnalysisResourceLimitsV1::new(
            combined.work_upper_bound(),
            combined.peak_storage_upper_bound(),
        );
        assert!(preflight_scoped_progress_resource_upper_bound_v1(census, bounded).is_err());
        assert_eq!(continuation.work_upper_bound(), 2 * (1024 + 32));
        drop(graph);
        use crate::production_analysis::pliron_pipeline::invocation_receipt_v1::InvocationReceiptV1 as Receipt;
        let phase_kind = ProductionAnalysisResourcePhaseV1::Progress;
        let mut receipt = Receipt::new(Default::default(), bounded).unwrap();
        let phase = receipt.phase(phase_kind, 0).unwrap();
        let observer = phase.observer(&Ok);
        observer
            .require(
                bounded,
                phase_kind,
                preflight_progress_graph_resource_upper_bound_v2(census, bounded),
            )
            .unwrap();
        let graph = PreparedProgressGraphV2::new(&context, &function, Some(&observer));
        phase.commit(preparation).unwrap();
        let phase = receipt.phase(phase_kind, 0).unwrap();
        let observer = phase.observer(&Ok);
        let continuation_limits = ProductionAnalysisResourceLimitsV1::new(
            continuation.work_upper_bound(),
            continuation.peak_storage_upper_bound(),
        );
        observer
            .require(
                continuation_limits,
                phase_kind,
                graph.continuation_bound(census, continuation_limits),
            )
            .unwrap();
        // This receipt covers exactly progress preparation and consumption;
        // the genuine pass session retains its separate identity/verification
        // contract rather than pretending that work belongs to progress.
        assert!(
            scoped_observed(&context, &function, &graph, Some(&observer))
                .unwrap()
                .is_clean()
        );
        phase.commit(continuation).unwrap();
        assert_eq!(receipt.complete(), Ok(combined));
        assert_eq!(
            graph.run(None),
            run_pliron_progress_check_v1(&context, &function)
        );
    }

    #[test]
    fn graph_first_reachable_cycle_keeps_original_bound_and_report() {
        let mut context = context();
        let function = chain(&mut context, 3, true);
        let graph = PreparedProgressGraphV2::new(&context, &function, None);
        let census = census(&context, &function);
        let limits = ProductionAnalysisResourceLimitsV1::new(usize::MAX, usize::MAX);
        assert_eq!(
            graph.continuation_bound(census, limits),
            preflight_scoped_progress_resource_upper_bound_v1(census, limits)
        );
        let old = run_pliron_progress_check_v1(&context, &function);
        assert!(!old.is_clean());
        assert_eq!(scoped(&context, &function, &graph).unwrap(), old);
    }

    #[test]
    fn graph_first_unreachable_cycle_is_checked_but_not_a_reachable_obligation() {
        let mut context = context();
        let function = chain(&mut context, 1, false);
        let dead = BasicBlock::new(&mut context, Some("dead".try_into().unwrap()), vec![]);
        dead.insert_at_back(function.get_region(&context), &context);
        BranchOp::new(&mut context, dead)
            .get_operation()
            .insert_at_back(dead, &context);
        let graph = PreparedProgressGraphV2::new(&context, &function, None);
        let state = graph.state.as_ref().ok().unwrap();
        assert_eq!(state.components.len(), 2);
        assert!(!state.reachable_cycle);
        assert!(scoped(&context, &function, &graph).unwrap().is_clean());
        assert_eq!(
            graph.run(None),
            run_pliron_progress_check_v1(&context, &function)
        );
    }

    #[test]
    fn graph_first_unreachable_malformed_block_and_nested_region_never_become_clean() {
        for nested in [false, true] {
            let mut context = context();
            let function = chain(&mut context, 1, false);
            if nested {
                let signature = FunctionType::get(&context, vec![], vec![]);
                let child = FuncOp::new(
                    &mut context,
                    "malformed_nested".try_into().unwrap(),
                    signature,
                );
                child
                    .get_operation()
                    .insert_at_front(function.get_entry_block(&context), &context);
            } else {
                let dead = BasicBlock::new(
                    &mut context,
                    Some("malformed_dead".try_into().unwrap()),
                    vec![],
                );
                dead.insert_at_back(function.get_region(&context), &context);
            }
            let graph = PreparedProgressGraphV2::new(&context, &function, None);
            assert!(scoped(&context, &function, &graph).is_err());
            assert!(!run_pliron_progress_check_v1(&context, &function).is_clean());
            assert!(crate::production_analysis::pliron_pipeline::require_production_pliron_checks_before_lowering_v2(&context, &function).is_err());
        }
    }

    #[test]
    fn graph_first_rejects_equal_shape_foreign_context_and_function() {
        let mut context = context();
        let function = chain(&mut context, 2, false);
        let other = chain(&mut context, 2, false);
        let mut foreign_context = self::context();
        let foreign = chain(&mut foreign_context, 2, false);
        let graph = PreparedProgressGraphV2::new(&context, &function, None);
        for (context, function) in [(&context, &other), (&foreign_context, &foreign)] {
            assert!(matches!(
                scoped(context, function, &graph),
                Err(PlironPassPreservationErrorV1::InvalidSessionState {
                    detail: "prepared progress graph differs from exact scoped endpoints or epoch"
                })
            ));
        }
        assert!(scoped(&context, &function, &graph).unwrap().is_clean());
    }

    #[test]
    fn graph_first_rejects_stale_graph_even_after_fresh_unchanged_identity_verification() {
        let mut context = context();
        let function = chain(&mut context, 2, false);
        let graph = PreparedProgressGraphV2::new(&context, &function, None);
        let operation = function.get_operation();
        let held = operation.deref(&context);
        assert!(operation.try_deref_mut(&context).is_err());
        drop(held);
        // The new session verifies current bytes. The older graph must still
        // reject the intervening mutable-borrow attempt, not trust equality.
        assert!(matches!(
            scoped(&context, &function, &graph),
            Err(PlironPassPreservationErrorV1::InvalidSessionState { .. })
        ));
        let current = PreparedProgressGraphV2::new(&context, &function, None);
        assert!(scoped(&context, &function, &current).unwrap().is_clean());
    }

    #[test]
    fn graph_first_fixed_pipeline_consumes_actual_dag_without_new_authority() {
        let mut context = context();
        let function = chain(&mut context, 64, false);
        let report = crate::production_analysis::pliron_pipeline::require_production_pliron_checks_before_lowering_v2(&context, &function).unwrap();
        assert!(report.is_clean());
        assert_eq!(report.preservation().certificates().len(), 9);
        assert!(report.preservation().is_exact_identity());
        assert!(!report.grants_compiler_refinement_authority());
        assert!(!report.grants_artifact_or_launch_authority());
    }

    #[test]
    fn graph_first_swallowed_preflight_denial_remains_first_and_prevents_construction() {
        use crate::production_analysis::pliron_pipeline::invocation_receipt_v1::{
            InvocationReceiptFailureV1 as Failure, InvocationReceiptV1 as Receipt,
        };
        let mut context = context();
        let function = chain(&mut context, 3, false);
        let census = census(&context, &function);
        let hard = ProductionAnalysisResourceLimitsV1::production_hard_ceiling();
        let bound = preflight_progress_graph_resource_upper_bound_v2(census, hard).unwrap();
        for (limits, resource) in [
            (
                ProductionAnalysisResourceLimitsV1::new(
                    bound.work_upper_bound() - 1,
                    bound.peak_storage_upper_bound(),
                ),
                "work upper bound",
            ),
            (
                ProductionAnalysisResourceLimitsV1::new(
                    bound.work_upper_bound(),
                    bound.peak_storage_upper_bound() - 1,
                ),
                "peak storage upper bound",
            ),
        ] {
            let mut receipt = Receipt::new(Default::default(), hard).unwrap();
            let phase = receipt
                .phase(ProductionAnalysisResourcePhaseV1::Progress, 0)
                .unwrap();
            let observer = phase.observer(&Ok);
            let constructed = std::cell::Cell::new(false);
            let refused = observer
                .require(
                    limits,
                    ProductionAnalysisResourcePhaseV1::Progress,
                    preflight_progress_graph_resource_upper_bound_v2(census, limits),
                )
                .map(|_| {
                    constructed.set(true);
                    PreparedProgressGraphV2::new(&context, &function, Some(&observer))
                });
            assert!(refused.is_err());
            assert!(!constructed.get());
            // A later successful query cannot remove the first observation.
            assert!(preflight_progress_graph_resource_upper_bound_v2(census, hard).is_ok());
            drop(refused);
            drop(phase);
            assert_eq!(
                receipt.complete(),
                Err(Failure::Denied(progress_resource_error_v1(resource)))
            );
        }
    }

    #[test]
    fn graph_first_real_borrow_panic_is_sticky_and_never_replaces_prior_denial() {
        use crate::production_analysis::pliron_pipeline::invocation_receipt_v1::{
            InvocationReceiptFailureV1 as Failure, InvocationReceiptV1 as Receipt,
        };
        for prior_denial in [false, true] {
            let mut context = context();
            let function = chain(&mut context, 2, false);
            let inventory = bounded_structural_inventory(&context, &function).unwrap();
            let mut work = ProgressWorkBudgetV1::default();
            work.charge(inventory.structural_work().unwrap()).unwrap();
            let mut receipt = Receipt::new(
                Default::default(),
                ProductionAnalysisResourceLimitsV1::production_hard_ceiling(),
            )
            .unwrap();
            let phase = receipt
                .phase(ProductionAnalysisResourcePhaseV1::Progress, 0)
                .unwrap();
            let observer = phase.observer(&Ok);
            let denial = progress_resource_error_v1("test earlier graph denial");
            if prior_denial {
                observer.deny(denial);
            }
            let state = capture_progress_graph_v2(Some(&observer), || {
                let block = function.get_entry_block(&context);
                let _held = block.deref_mut(&context);
                build_progress_graph_v2(&context, inventory, work)
            });
            let finding = match state {
                Err(finding @ PlironProgressFindingV1::StructuralPrerequisiteRejected { .. }) => {
                    finding
                }
                _ => panic!("real graph borrow must become a structural finding"),
            };
            assert!(!observed_progress_report_v1(finding, Some(&observer)).is_clean());
            // Both the mutable graph borrow and unwind guard have dropped.
            // Ignoring that report or subsequently deriving a clean graph
            // cannot recover authority from this invocation receipt.
            let clean = PreparedProgressGraphV2::new(&context, &function, Some(&observer));
            assert!(clean.run(Some(&observer)).is_clean());
            drop(clean);
            drop(phase);
            assert_eq!(
                receipt.complete(),
                Err(if prior_denial {
                    Failure::Denied(denial)
                } else {
                    Failure::CaughtPanic
                })
            );
        }
    }
}

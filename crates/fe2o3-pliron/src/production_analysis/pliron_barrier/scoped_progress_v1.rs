use crate::production_analysis::pliron_pass_contract::{
    PlironPassPreservationErrorV1, ScopedVerifiedProgressInputV1,
};

pub(crate) fn require_pliron_barrier_with_scoped_observation_v1(
    input: ScopedVerifiedProgressInputV1<'_, true>,
    analyses: &mut PlironAnalysisManagerV1,
    progress_admitted: bool,
    observer: BarrierObserverV1<'_, '_, '_>,
) -> Result<Result<PlironBarrierReportV1, PlironBarrierCheckErrorV1>, PlironPassPreservationErrorV1>
{
    let run = || {
        let (context, function) = input.endpoints()?;
        let report = run_barrier_with_progress_observation_v1(
            context,
            function,
            analyses,
            || {
                if !progress_admitted {
                    return Err(PlironPassPreservationErrorV1::InvalidSessionState {
                        detail: "barrier progress was not resource-admitted",
                    });
                }
                crate::production_analysis::pliron_progress::run_pliron_progress_with_scoped_observation_v1(input, observer)
            .map(|progress| progress.report)
            },
            observer,
        )?;
        Ok(if report.is_clean() {
            Ok(report)
        } else {
            Err(PlironBarrierCheckErrorV1 { report })
        })
    };
    match observer {
        None => run(),
        Some(observer) => observer.with_projection(&Ok, |_| run()),
    }
}

#[cfg(test)]
pub(crate) fn run_pliron_barrier_convergence_check_with_analyses_v1(
    context: &Context,
    function: &FuncOp,
    analyses: &mut PlironAnalysisManagerV1,
) -> PlironBarrierReportV1 {
    run_barrier_with_progress_v1(context, function, analyses, || {
        Ok(
            crate::production_analysis::pliron_progress::run_pliron_progress_check_v1(
                context, function,
            ),
        )
    })
    .expect("the standalone test path does not create a scoped preservation error")
}

#[cfg(test)]
mod observed_barrier_tests {
    use super::*;
    use crate::production_analysis::pliron_pipeline::invocation_receipt_v1::{
        InvocationReceiptFailureV1 as ReceiptFailure, InvocationReceiptV1 as Receipt,
    };
    use crate::production_analysis::pliron_progress::run_pliron_progress_check_v1 as check_progress;
    use dialect_gpu::ExecutionLayoutOp;
    use dialect_kernel::{
        BranchOp, IndexConstantOp, IndexLessThanBranchOp, InvocationIndexOp, ReturnOp,
    };
    use pliron::{
        basic_block::BasicBlock,
        builtin::{op_interfaces::OneRegionInterface, types::FunctionType},
        op::Op,
    };

    fn setup() -> Context {
        let mut context = Context::new();
        dialect_kernel::register_dialect(
            &mut context,
            &pliron::dialect::DialectName::try_new(dialect_kernel::DIALECT_NAME).unwrap(),
        )
        .unwrap();
        dialect_gpu::register_dialect(&mut context).unwrap();
        context
    }

    fn barrier(context: &mut Context) -> BarrierOp {
        BarrierOp::new(
            context,
            HierarchyAttr::Workgroup,
            MemoryScopeAttr::Workgroup,
            AddressSpaceAttr::Workgroup,
            MemoryOrderAttr::AcquireRelease,
        )
    }

    fn fixture(context: &mut Context, n: u64, divergent: bool) -> FuncOp {
        let signature = FunctionType::get(context, vec![], vec![]);
        let function = FuncOp::new(context, "observed_barrier".try_into().unwrap(), signature);
        let entry = function.get_entry_block(context);
        let workgroup = if n == 4 { 4 } else { 1 };
        ExecutionLayoutOp::new(context, 7, [n, 1, 1], [workgroup, 1, 1], workgroup)
            .get_operation()
            .insert_at_back(entry, context);
        let [body, exit] = if divergent {
            let blocks = ["sync", "exit"].map(|name| {
                let block = BasicBlock::new(context, Some(name.try_into().unwrap()), vec![]);
                block.insert_at_back(function.get_region(context), context);
                block
            });
            let index = InvocationIndexOp::new(context, 0, n);
            let two = IndexConstantOp::new(context, 2);
            let lhs = index.result(context);
            let rhs = two.result(context);
            let branch = IndexLessThanBranchOp::new(context, lhs, rhs, blocks[0], blocks[1]);
            for op in [
                index.get_operation(),
                two.get_operation(),
                branch.get_operation(),
            ] {
                op.insert_at_back(entry, context);
            }
            blocks
        } else {
            [entry, entry]
        };
        barrier(context)
            .get_operation()
            .insert_at_back(body, context);
        if divergent {
            BranchOp::new(context, exit)
                .get_operation()
                .insert_at_back(body, context);
        }
        ReturnOp::new(context)
            .get_operation()
            .insert_at_back(exit, context);
        function
    }

    fn receipt() -> Receipt<'static> {
        Receipt::new(
            Default::default(),
            ProductionAnalysisResourceLimitsV1::production_hard_ceiling(),
        )
        .unwrap()
    }

    #[test]
    fn actual_trace_quota_survives_clean_barrier_fallback() {
        assert_eq!(crate::MAX_PLIRON_RACE_INVOCATIONS_V1 + 1, 65_537);
        for (n, divergent) in [(4, false), (4, true), (65_537, false)] {
            let mut context = setup();
            let function = fixture(&mut context, n, divergent);
            let prove = || Ok(check_progress(&context, &function));
            let ordinary = run_barrier_with_progress_v1(
                &context,
                &function,
                &mut PlironAnalysisManagerV1::new(&function),
                prove,
            )
            .unwrap();
            let mut manager = PlironAnalysisManagerV1::new(&function);
            let mut receipt = receipt();
            let phase = receipt
                .phase(ProductionAnalysisResourcePhaseV1::BarrierConvergence, 0)
                .unwrap();
            let observed = run_barrier_with_progress_observation_v1(
                &context,
                &function,
                &mut manager,
                prove,
                Some(&phase.observer(&Ok)),
            )
            .unwrap();
            drop(phase);
            assert_eq!(observed, ordinary);
            assert_eq!(observed.is_clean(), !divergent);
            if divergent {
                assert!(matches!(
                    observed.findings(),
                    [PlironBarrierFindingV1::DivergentBarrierTrace { .. }]
                ));
            }
            assert!(!receipt.snapshot().caught_panic);
            // These leaf tests do not admit or transfer a report owner.
            assert_eq!(receipt.snapshot().committed, Default::default());
            if n == 4 {
                assert_eq!(manager.exact_trace().unwrap().len(), 4);
                assert!(manager.simt_protocol().unwrap().issues().is_empty());
                assert_eq!(receipt.complete(), Ok(Default::default()));
            } else {
                let failure = PlironTraceFailureV1::LaunchTooLarge { invocations: n };
                assert_eq!(manager.exact_trace().unwrap_err(), failure);
                assert_eq!(
                    manager.simt_protocol().unwrap_err(),
                    PlironSimtProtocolAnalysisFailureV1::Trace(failure)
                );
                let error = ProductionAnalysisResourceLimitV1 {
                    phase: ProductionAnalysisResourcePhaseV1::InvocationTrace,
                    resource: "barrier trace launch limit",
                };
                assert_eq!(receipt.snapshot().first_denial, Some(error));
                assert_eq!(receipt.complete(), Err(ReceiptFailure::Denied(error)));
            }
        }
    }

    #[test]
    fn actual_borrow_panic_preserves_prior_barrier_trace_quota() {
        let mut context = setup();
        let function = fixture(&mut context, 65_537, false);
        let mut manager = PlironAnalysisManagerV1::new(&function);
        let mut receipt = receipt();
        let phase = receipt
            .phase(ProductionAnalysisResourcePhaseV1::BarrierConvergence, 0)
            .unwrap();
        assert!(
            run_barrier_with_progress_observation_v1(
                &context,
                &function,
                &mut manager,
                || Ok(check_progress(&context, &function)),
                Some(&phase.observer(&Ok)),
            )
            .unwrap()
            .is_clean()
        );
        drop(phase);
        let denial = receipt
            .snapshot()
            .first_denial
            .expect("actual launch quota");
        let pointer = manager.function_inventory().unwrap().operations()[0].pointer();
        let phase = receipt
            .phase(ProductionAnalysisResourcePhaseV1::BarrierConvergence, 0)
            .unwrap();
        let held = pointer.deref_mut(&context);
        let panic = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            run_barrier_with_progress_observation_v1(
                &context,
                &function,
                &mut manager,
                || Ok(check_progress(&context, &function)),
                Some(&phase.observer(&Ok)),
            )
        }));
        drop(held);
        drop(phase);
        assert!(panic.is_err());
        assert!(receipt.snapshot().caught_panic);
        assert_eq!(receipt.snapshot().first_denial, Some(denial));
        assert_eq!(receipt.complete(), Err(ReceiptFailure::Denied(denial)));
    }

    #[test]
    fn tensor_only_early_return_keeps_actual_trace_quota() {
        let mut context = setup();
        let signature = FunctionType::get(&context, vec![], vec![]);
        let function = FuncOp::new(
            &mut context,
            "observed_tensor_only_barrier_stage".try_into().unwrap(),
            signature,
        );
        let entry = function.get_entry_block(&context);
        let launch = crate::MAX_PLIRON_RACE_INVOCATIONS_V1 + 1;
        ExecutionLayoutOp::new(&mut context, 7, [launch, 1, 1], [64, 1, 1], 64)
            .get_operation()
            .insert_at_back(entry, &context);
        TensorLayoutOp::new(
            &mut context,
            &fe2o3_kernel_ir::TensorLayoutContractV1::gfx942_mfma_bf16_f32_m16n16k16_wave64(),
            dialect_kernel::TensorConvergenceAttr::UniformSubgroup,
            64,
        )
        .get_operation()
        .insert_at_back(entry, &context);
        ReturnOp::new(&mut context)
            .get_operation()
            .insert_at_back(entry, &context);
        let ordinary = run_barrier_with_progress_v1(
            &context,
            &function,
            &mut PlironAnalysisManagerV1::new(&function),
            || panic!("tensor-only stage must not invoke progress"),
        )
        .unwrap();
        let mut manager = PlironAnalysisManagerV1::new(&function);
        let mut receipt = receipt();
        let phase = receipt
            .phase(ProductionAnalysisResourcePhaseV1::BarrierConvergence, 0)
            .unwrap();
        let observed = run_barrier_with_progress_observation_v1(
            &context,
            &function,
            &mut manager,
            || panic!("tensor-only stage must not invoke progress"),
            Some(&phase.observer(&Ok)),
        )
        .unwrap();
        drop(phase);
        assert!(observed.is_clean());
        assert_eq!(observed, ordinary);
        assert_eq!(
            manager.exact_trace().unwrap_err(),
            PlironTraceFailureV1::LaunchTooLarge {
                invocations: launch
            }
        );
        let error = ProductionAnalysisResourceLimitV1 {
            phase: ProductionAnalysisResourcePhaseV1::InvocationTrace,
            resource: "barrier trace launch limit",
        };
        assert_eq!(receipt.snapshot().first_denial, Some(error));
        assert!(!receipt.snapshot().caught_panic);
        assert_eq!(receipt.complete(), Err(ReceiptFailure::Denied(error)));
    }

    fn capped_cfg(context: &mut Context, events: &[usize]) -> FuncOp {
        let signature = FunctionType::get(context, vec![], vec![]);
        let function = FuncOp::new(
            context,
            "observed_barrier_caps".try_into().unwrap(),
            signature,
        );
        let mut blocks = vec![function.get_entry_block(context)];
        for _ in 1..events.len() {
            let block = BasicBlock::new(context, None, vec![]);
            block.insert_at_back(function.get_region(context), context);
            blocks.push(block);
        }
        for (i, (&block, &count)) in blocks.iter().zip(events).enumerate() {
            for _ in 0..count {
                barrier(context)
                    .get_operation()
                    .insert_at_back(block, context);
            }
            let terminator = match blocks.get(i + 1) {
                Some(next) => BranchOp::new(context, *next).get_operation(),
                None => ReturnOp::new(context).get_operation(),
            };
            terminator.insert_at_back(block, context);
        }
        function
    }

    #[test]
    fn actual_fallback_cfg_and_event_caps_keep_exact_boundaries() {
        for (events, resource) in [
            (vec![0; 512], None),
            (vec![0; 513], Some("fallback barrier CFG block limit")),
            (vec![256], None),
            (vec![257], Some("fallback barrier block event limit")),
            (vec![128, 128], None),
            (vec![128, 129], Some("fallback barrier path event limit")),
        ] {
            let mut context = setup();
            let function = capped_cfg(&mut context, &events);
            let mut manager = PlironAnalysisManagerV1::new(&function);
            manager.prepare_function_inventory(&context, &function);
            let inventory = manager.function_inventory().unwrap();
            let ordinary = barrier_paths::summarize_all_barrier_paths(&context, inventory, || {
                panic!("acyclic CFG must not invoke progress")
            })
            .unwrap();
            let mut receipt = receipt();
            let phase = receipt
                .phase(ProductionAnalysisResourcePhaseV1::BarrierConvergence, 0)
                .unwrap();
            let observed = barrier_paths::summarize_all_barrier_paths_with_observation_v1(
                &context,
                inventory,
                || panic!("acyclic CFG must not invoke progress"),
                Some(&phase.observer(&Ok)),
            )
            .unwrap();
            drop(phase);
            match (ordinary, observed, resource) {
                (BarrierPathSummaryV1::Unique, BarrierPathSummaryV1::Unique, None) => {
                    assert_eq!(receipt.complete(), Ok(Default::default()));
                }
                (
                    BarrierPathSummaryV1::Incomplete(first),
                    BarrierPathSummaryV1::Incomplete(second),
                    Some(resource),
                ) => {
                    assert_eq!(first, second);
                    let error = ProductionAnalysisResourceLimitV1 {
                        phase: ProductionAnalysisResourcePhaseV1::BarrierConvergence,
                        resource,
                    };
                    assert_eq!(receipt.snapshot().first_denial, Some(error));
                    assert_eq!(receipt.complete(), Err(ReceiptFailure::Denied(error)));
                }
                _ => panic!("unexpected ordinary/observed fallback boundary result"),
            }
            assert!(!receipt.snapshot().caught_panic);
        }
    }
}

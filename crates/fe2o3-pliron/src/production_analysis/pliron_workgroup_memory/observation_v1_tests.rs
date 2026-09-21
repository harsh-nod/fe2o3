#[cfg(test)]
mod observed_workgroup_tests {
    use super::*;
    use crate::production_analysis::pliron_invocation_trace::PlironTraceFailureV1 as TraceFailure;
    use crate::production_analysis::pliron_pipeline::invocation_receipt_v1::{
        InvocationReceiptFailureV1 as ReceiptFailure, InvocationReceiptV1 as Receipt,
    };
    use crate::production_analysis::pliron_resource_envelope::{
        ProductionAnalysisResourceLimitV1 as Limit, ProductionAnalysisResourcePhaseV1 as Phase,
    };
    use dialect_gpu::ExecutionLayoutOp;
    use dialect_kernel::{InvocationIndexOp, RankedViewType};
    use pliron::{
        builtin::{op_interfaces::OneRegionInterface, types::FunctionType},
        op::Op,
    };
    use std::panic::{AssertUnwindSafe, catch_unwind};

    #[derive(Debug)]
    enum Expected {
        Clean,
        PartialTrap,
        Uninitialized(usize),
        Denied(Limit),
    }

    #[derive(Clone, Copy, Debug)]
    enum CollectiveCase {
        Exact,
        FourLocal,
        FourSplit,
        EightTraps,
        NineTraps,
    }

    fn fixture_base(n: u64, workgroup: u64, subgroup: u64) -> (Context, FuncOp) {
        let mut context = Context::new();
        let dialect = pliron::dialect::DialectName::try_new(dialect_kernel::DIALECT_NAME).unwrap();
        dialect_kernel::register_dialect(&mut context, &dialect).unwrap();
        dialect_gpu::register_dialect(&mut context).unwrap();
        let signature = FunctionType::get(&context, vec![], vec![]);
        let function = FuncOp::new(
            &mut context,
            "observed_workgroup".try_into().unwrap(),
            signature,
        );
        let entry = function.get_entry_block(&context);
        ExecutionLayoutOp::new_with_domain(
            &mut context,
            7,
            [n, 1, 1],
            [workgroup, 1, 1],
            subgroup,
            ExecutionDomainAttr::FullPhysicalWorkgroups,
        )
        .get_operation()
        .insert_at_back(entry, &context);
        (context, function)
    }

    fn collective_fixture(case: CollectiveCase) -> (Context, FuncOp, Expected) {
        let (mut context, function) = fixture_base(64, 64, 64);
        let entry = function.get_entry_block(&context);
        let block = |context: &mut Context| {
            let block = BasicBlock::new(context, None, vec![]);
            block.insert_at_back(function.get_region(context), context);
            block
        };
        let barrier = |context: &mut Context| {
            BarrierOp::new(
                context,
                HierarchyAttr::Workgroup,
                MemoryScopeAttr::Workgroup,
                AddressSpaceAttr::Workgroup,
                MemoryOrderAttr::AcquireRelease,
            )
            .get_operation()
        };
        let effect = |context: &mut Context, kind| {
            AllocationEffectOp::new(
                context,
                kind,
                MemorySpaceAttr::Workgroup,
                GFX950_TRANSPOSE_FP4_WORKGROUP_ALLOCATION_ORIGIN_V1,
                GFX950_TRANSPOSE_FP4_WORKGROUP_NOALIAS_CLASS_V1,
            )
            .unwrap()
            .get_operation()
        };
        let traps = match case {
            CollectiveCase::EightTraps => 8,
            CollectiveCase::NineTraps => 9,
            _ => 0,
        };
        let normal = if traps == 0 {
            entry
        } else {
            block(&mut context)
        };
        let mut cursor = entry;
        for index in 0..traps {
            let trapped = block(&mut context);
            let next = if index + 1 == traps {
                normal
            } else {
                block(&mut context)
            };
            AnalysisSplitOp::new(&mut context, trapped, next)
                .get_operation()
                .insert_at_back(cursor, &context);
            barrier(&mut context).insert_at_back(trapped, &context);
            TrapOp::new(&mut context)
                .get_operation()
                .insert_at_back(trapped, &context);
            cursor = next;
        }
        effect(&mut context, AccessKindAttr::Write).insert_at_back(normal, &context);
        barrier(&mut context).insert_at_back(normal, &context);
        let tail = if matches!(case, CollectiveCase::FourSplit) {
            let tail = block(&mut context);
            BranchOp::new(&mut context, tail)
                .get_operation()
                .insert_at_back(normal, &context);
            tail
        } else {
            normal
        };
        effect(&mut context, AccessKindAttr::Read).insert_at_back(tail, &context);
        if matches!(case, CollectiveCase::FourLocal | CollectiveCase::FourSplit) {
            barrier(&mut context).insert_at_back(tail, &context);
        }
        ReturnOp::new(&mut context)
            .get_operation()
            .insert_at_back(tail, &context);
        let expected = match case {
            CollectiveCase::Exact => Expected::Clean,
            CollectiveCase::EightTraps => Expected::PartialTrap,
            other => Expected::Denied(Limit {
                phase: Phase::WorkgroupMemory,
                resource: match other {
                    CollectiveCase::FourLocal => "collective transpose block event limit",
                    CollectiveCase::FourSplit => "collective transpose path event limit",
                    CollectiveCase::NineTraps => "collective transpose terminal trap trace limit",
                    _ => unreachable!(),
                },
            }),
        };
        (context, function, expected)
    }

    fn lds_fixture(n: u64) -> (Context, FuncOp, Expected) {
        assert!(matches!(n, 4 | 65_537));
        let (mut context, function) = fixture_base(n, 1, 1);
        let entry = function.get_entry_block(&context);
        let ty = RankedViewType::new(&context, 32, true, vec![n]).unwrap();
        let view = RankedViewOp::new_in_space_with_allocation_contract(
            &mut context,
            ty,
            vec![],
            MemorySpaceAttr::Workgroup,
            7,
            7,
        )
        .unwrap();
        let invocation = InvocationIndexOp::new(&mut context, 0, n);
        let value = view.result(&context);
        let index = invocation.result(&context);
        let read =
            RankedAccessOp::new(&mut context, AccessKindAttr::Read, value, vec![index]).unwrap();
        let ret = ReturnOp::new(&mut context);
        for op in [
            view.get_operation(),
            invocation.get_operation(),
            read.get_operation(),
            ret.get_operation(),
        ] {
            op.insert_at_back(entry, &context);
        }
        let expected = if n == 4 {
            Expected::Uninitialized(4)
        } else {
            Expected::Denied(Limit {
                phase: Phase::InvocationTrace,
                resource: "workgroup trace launch limit",
            })
        };
        (context, function, expected)
    }

    fn assert_fixture((context, function, expected): (Context, FuncOp, Expected)) {
        let mut ordinary_manager = PlironAnalysisManagerV1::new(&function);
        let ordinary = run_pliron_workgroup_memory_check_with_analyses_v1(
            &context,
            &function,
            &mut ordinary_manager,
        );
        let mut manager = PlironAnalysisManagerV1::new(&function);
        let mut receipt = Receipt::new(
            Default::default(),
            ProductionAnalysisResourceLimitsV1::production_hard_ceiling(),
        )
        .unwrap();
        let phase = receipt.phase(Phase::WorkgroupMemory, 0).unwrap();
        let observed = run_pliron_workgroup_memory_with_observation_v1(
            &context,
            &function,
            &mut manager,
            Some(&phase.observer(&Ok)),
        );
        drop(phase);
        assert_eq!(observed, ordinary);
        assert_eq!(
            manager.resource_upper_bound(),
            ordinary_manager.resource_upper_bound()
        );
        let denial = match expected {
            Expected::Clean => {
                assert!(observed.is_clean());
                None
            }
            Expected::PartialTrap => {
                assert!(matches!(observed.findings(),
                    [PlironWorkgroupMemoryFindingV1::AnalysisIncomplete { detail }]
                    if detail.contains("a terminal trap path partially executes")));
                None
            }
            Expected::Uninitialized(count) => {
                assert_eq!(observed.findings().len(), count);
                assert!(observed.findings().iter().all(|finding| matches!(
                    finding,
                    PlironWorkgroupMemoryFindingV1::ReadBeforeInitialization { .. }
                )));
                assert_eq!(manager.exact_trace().unwrap().len(), count);
                None
            }
            Expected::Denied(error) => {
                assert!(matches!(
                    observed.findings(),
                    [PlironWorkgroupMemoryFindingV1::AnalysisIncomplete { .. }]
                ));
                if error.phase == Phase::InvocationTrace {
                    assert_eq!(
                        manager.exact_trace().unwrap_err(),
                        TraceFailure::LaunchTooLarge {
                            invocations: 65_537
                        }
                    );
                }
                Some(error)
            }
        };
        let state = receipt.snapshot();
        assert_eq!(state.first_denial, denial);
        assert!(!state.caught_panic);
        assert_eq!(state.committed, Default::default());
        match denial {
            None => assert_eq!(receipt.complete(), Ok(Default::default())),
            Some(error) => {
                assert_eq!(receipt.complete(), Err(ReceiptFailure::Denied(error)));
                let operation = manager.function_inventory().unwrap().operations()[0].pointer();
                let phase = receipt.phase(Phase::WorkgroupMemory, 0).unwrap();
                let held = operation.deref_mut(&context);
                let outcome = catch_unwind(AssertUnwindSafe(|| {
                    run_pliron_workgroup_memory_with_observation_v1(
                        &context,
                        &function,
                        &mut manager,
                        Some(&phase.observer(&Ok)),
                    )
                }));
                drop(held);
                drop(phase);
                assert!(outcome.is_err());
                assert!(receipt.snapshot().caught_panic);
                assert_eq!(receipt.snapshot().first_denial, Some(error));
                assert_eq!(receipt.snapshot().committed, state.committed);
                assert_eq!(receipt.complete(), Err(ReceiptFailure::Denied(error)));
            }
        }
    }

    #[test]
    fn actual_collective_caps_keep_semantic_control_and_caught_panic() {
        for case in [
            CollectiveCase::Exact,
            CollectiveCase::FourLocal,
            CollectiveCase::FourSplit,
            CollectiveCase::EightTraps,
            CollectiveCase::NineTraps,
        ] {
            assert_fixture(collective_fixture(case));
        }
    }

    #[test]
    fn actual_lds_trace_quota_and_uninitialized_control() {
        for invocations in [4, 65_537] {
            assert_fixture(lds_fixture(invocations));
        }
    }

    fn nested_protocol_fixture() -> (Context, FuncOp) {
        let (mut context, function) = fixture_base(4, 1, 1);
        let entry = function.get_entry_block(&context);
        let ty = RankedViewType::new(&context, 32, true, vec![2, 4]).unwrap();
        let view = RankedViewOp::new_in_space_with_allocation_contract(
            &mut context,
            ty,
            vec![],
            MemorySpaceAttr::Workgroup,
            7,
            7,
        )
        .unwrap();
        let zero = dialect_kernel::IndexConstantOp::new(&mut context, 0);
        let lane = InvocationIndexOp::new(&mut context, 0, 4);
        let value = view.result(&context);
        let indices = vec![zero.result(&context), lane.result(&context)];
        let create = PipelineCreateOp::new(&mut context, value, 2, 1).unwrap();
        let read = RankedAccessOp::new(&mut context, AccessKindAttr::Read, value, indices).unwrap();
        let ret = ReturnOp::new(&mut context);
        for op in [
            view.get_operation(),
            zero.get_operation(),
            lane.get_operation(),
            create.get_operation(),
            read.get_operation(),
            ret.get_operation(),
        ] {
            op.insert_at_back(entry, &context);
        }
        (context, function)
    }

    #[test]
    fn nested_protocol_refusal_survives_workgroup_semantic_fallback() {
        type Bound = ProductionAnalysisResourceUpperBoundV1;
        type Limits = ProductionAnalysisResourceLimitsV1;
        let hard = Limits::production_hard_ceiling();
        let (context, function) = nested_protocol_fixture();
        let mut baseline = PlironAnalysisManagerV1::new(&function);
        assert!(baseline.input_census().is_none());
        let ordinary =
            run_pliron_workgroup_memory_check_with_analyses_v1(&context, &function, &mut baseline);
        assert_eq!(ordinary.findings().len(), 4);
        assert!(ordinary.findings().iter().all(|finding| matches!(
            finding,
            PlironWorkgroupMemoryFindingV1::ReadBeforeInitialization { .. }
        )));
        let bound = baseline.resource_upper_bound();
        assert!(bound.work_upper_bound() > 0 && bound.retained_storage_upper_bound() > 0);
        drop(baseline);
        for refuse in [false, true] {
            let limits = if refuse {
                Limits::new(0, hard.max_peak_storage())
            } else {
                hard
            };
            let mut receipt = Receipt::new(Bound::default(), limits).unwrap();
            let phase = receipt.phase(Phase::WorkgroupMemory, 0).unwrap();
            let mut manager = PlironAnalysisManagerV1::new(&function);
            assert!(manager.input_census().is_none());
            let actual = run_pliron_workgroup_memory_with_observation_v1(
                &context,
                &function,
                &mut manager,
                Some(&phase.observer(&Ok)),
            );
            // The test retains the accepted reservation, not a committed owner.
            drop(phase);
            assert_eq!(actual, ordinary);
            assert_eq!(manager.exact_trace().unwrap().len(), 4);
            let state = receipt.snapshot();
            assert!(!state.caught_panic);
            let denial = refuse.then_some(Limit {
                phase: Phase::PipelineProtocol,
                resource: "work upper bound",
            });
            let held = if refuse {
                Bound::default()
            } else {
                Bound::checked_phase(
                    Phase::PipelineProtocol,
                    bound.work_upper_bound(),
                    bound.peak_storage_upper_bound(),
                    0,
                )
                .unwrap()
            };
            assert_eq!(state.first_denial, denial);
            assert_eq!(state.committed, held);
            assert_eq!(
                manager.resource_upper_bound(),
                if refuse { Bound::default() } else { bound }
            );
            assert_eq!(
                receipt.complete(),
                denial.map_or(Ok(held), |error| Err(ReceiptFailure::Denied(error)))
            );
        }
    }

    #[test]
    fn prepared_trace_quota_precedes_later_local_preflight_refusal() {
        use crate::production_analysis::pliron_ir_identity::LivePlironStructuralIdentityProviderV1;
        use crate::production_analysis::pliron_memory_order::preflight_memory_order_attempt_resource_upper_bound_v1;
        use crate::production_analysis::pliron_pass_contract::begin_production_pliron_pass_contract_session_with_resource_limits_v1;

        let hard = ProductionAnalysisResourceLimitsV1::production_hard_ceiling();
        let (context, function, _) = lds_fixture(65_537);
        let preservation = begin_production_pliron_pass_contract_session_with_resource_limits_v1(
            LivePlironStructuralIdentityProviderV1::new(&context, &function),
            hard,
        )
        .unwrap();
        let census = preservation.input_census_v1();
        let admission =
            preflight_memory_order_attempt_resource_upper_bound_v1(census, None, hard).unwrap();
        let mut analyses = PlironAnalysisManagerV1::new(&function);
        analyses.prepare_memory_order(&context, &function);
        assert_eq!(
            analyses.exact_trace().unwrap_err(),
            TraceFailure::LaunchTooLarge {
                invocations: 65_537
            }
        );
        for short in [false, true] {
            let local = if short {
                ProductionAnalysisResourceLimitsV1::new(0, hard.max_peak_storage())
            } else {
                hard
            };
            let expected = preflight_prepared_workgroup_memory_resource_upper_bound_v1(
                census,
                admission,
                analyses.memory_order().map(|cache| cache.issues()),
                local,
            );
            let mut receipt = Receipt::new(Default::default(), hard).unwrap();
            let phase = receipt.phase(Phase::WorkgroupMemory, 0).unwrap();
            let actual = preflight_prepared_workgroup_memory_with_observation_v1(
                census,
                admission,
                analyses.memory_order().map(|cache| cache.issues()),
                local,
                Some(&phase.observer(&Ok)),
            );
            drop(phase);
            assert_eq!(actual, expected);
            if short {
                assert_eq!(
                    actual,
                    Err(Limit {
                        phase: Phase::WorkgroupMemory,
                        resource: "work upper bound"
                    })
                );
            } else {
                assert!(actual.unwrap().work_upper_bound() > 0);
            }
            let denial = Limit {
                phase: Phase::InvocationTrace,
                resource: "workgroup trace launch limit",
            };
            assert_eq!(receipt.snapshot().first_denial, Some(denial));
            assert_eq!(receipt.snapshot().committed, Default::default());
            assert!(!receipt.snapshot().caught_panic);
            assert_eq!(receipt.complete(), Err(ReceiptFailure::Denied(denial)));
        }
    }
}

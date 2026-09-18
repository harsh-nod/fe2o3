use super::*;
use production_call_instances_v1::{
    ProductionCallInstanceErrorV1, with_production_call_instances_v1,
};

fn with_plan(test: impl FnOnce(&ProductionCallInstancePlanV1<'_>, &mut ArgumentBudgetV1<'_>)) {
    let mut ssa = ProductionSemanticSsaOwnerV1::try_new(
        resource_tests::helper_closure_semantic_owner(),
        ProductionSemanticSsaLimitsV1::default(),
    )
    .unwrap();
    let mut work = fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1::new(1_000_000);
    let mut budget = ArgumentBudgetV1::new(&mut work, 1_000_000);
    let capture = ssa
        .try_capture_occurrences_with_budget_v1(&mut budget)
        .unwrap();
    budget.reserve_storage(capture.retained_storage()).unwrap();
    let root = ssa.source_semantic().roots()[0];
    with_production_call_instances_v1(&ssa, root, &mut budget, |plan, budget| {
        test(plan, budget);
        Ok::<_, ProductionCallInstanceErrorV1>(())
    })
    .unwrap();
    assert_eq!(budget.storage(), capture.retained_storage());
}

fn span(
    instance: ProductionCallInstanceIdV1,
    function: SemanticFunctionIdV1,
    first: u32,
    count: u32,
) -> InstanceMappedSpanV1 {
    let source = SemanticKirTerminatorOperationSpanV1 {
        correspondence_owner: function,
        semantic_function: function,
        semantic_block: SemanticBlockIdV1::from_index(0),
        kernel_ir_block: BlockId(7),
        first_operation_ordinal: first,
        operation_count: count,
    };
    InstanceMappedSpanV1 {
        instance,
        source: InstanceSpanSourceV1::Terminator(source),
        segments: [
            Some(InstancePhysicalSpanV1 {
                block: BlockId(7),
                first,
                count,
            }),
            None,
        ],
        removed_call: None,
    }
}

fn split(ordinal: usize) -> CallInstanceSplitV1 {
    CallInstanceSplitV1 {
        call: FunctionOperationLocation::new(BlockId(7), ordinal),
        entry: BlockId(31),
        callee_entry: BlockId(17),
        continuation: BlockId(32),
        callee_blocks: 3,
        returns: 2,
        result_components: 1,
    }
}

#[test]
fn instance_span_splits_around_removed_call_and_keeps_original_source() {
    with_plan(|plan, _| {
        let instance = plan.root();
        let function = plan.instance(instance).unwrap().function();
        let call = plan.calls(instance).unwrap()[0].occurrence();
        let original = span(instance, function, 0, 5);
        let mapped = original.after_splice(call, split(2)).unwrap();
        assert_eq!(mapped.source, original.source);
        assert_eq!(mapped.instance, instance);
        assert_eq!(mapped.removed_call, Some(call));
        assert_eq!(
            mapped.segments,
            [
                Some(InstancePhysicalSpanV1 {
                    block: BlockId(7),
                    first: 0,
                    count: 2
                }),
                Some(InstancePhysicalSpanV1 {
                    block: BlockId(32),
                    first: 0,
                    count: 2
                }),
            ]
        );
        let suffix = span(instance, function, 4, 1)
            .after_splice(call, split(2))
            .unwrap();
        assert_eq!(
            suffix.segments[0],
            Some(InstancePhysicalSpanV1 {
                block: BlockId(32),
                first: 1,
                count: 1
            })
        );
        assert_eq!(suffix.removed_call, None);
    });
}

#[test]
fn instance_empty_spans_keep_exact_boundaries_and_call_only_span_has_tombstone() {
    with_plan(|plan, _| {
        let instance = plan.root();
        let function = plan.instance(instance).unwrap().function();
        let call = plan.calls(instance).unwrap()[0].occurrence();
        for (first, block, expected) in [(0, 7, 0), (2, 7, 2), (3, 32, 0), (5, 32, 2)] {
            let mapped = span(instance, function, first, 0)
                .after_splice(call, split(2))
                .unwrap();
            assert_eq!(
                mapped.segments,
                [
                    Some(InstancePhysicalSpanV1 {
                        block: BlockId(block),
                        first: expected,
                        count: 0
                    }),
                    None
                ]
            );
            assert_eq!(mapped.removed_call, None);
        }
        let mapped = span(instance, function, 2, 1)
            .after_splice(call, split(2))
            .unwrap();
        assert_eq!(mapped.segments, [None, None]);
        assert_eq!(mapped.removed_call, Some(call));
    });
}

#[test]
fn instance_span_rejects_foreign_instance_second_removal_and_overflow() {
    with_plan(|plan, _| {
        let call = &plan.calls(plan.root()).unwrap()[0];
        let child = call.child().unwrap();
        let function = plan.instance(child).unwrap().function();
        assert_eq!(
            span(child, function, 0, 5).after_splice(call.occurrence(), split(2)),
            Err(InstanceCorrespondenceErrorV1::SpanCoverage)
        );
        let mut original = span(plan.root(), function, 0, 5);
        original.removed_call = Some(call.occurrence());
        assert_eq!(
            original.after_splice(call.occurrence(), split(2)),
            Err(InstanceCorrespondenceErrorV1::SpanCoverage)
        );
        let overflowing = span(plan.root(), function, u32::MAX, 2);
        assert_eq!(
            overflowing.after_splice(call.occurrence(), split(2)),
            Err(InstanceCorrespondenceErrorV1::Resource(
                ArgumentResourceV1::Arithmetic
            ))
        );
    });
}

#[test]
fn instance_control_checks_ordered_values_targets_and_parameter_ids() {
    let mut work = fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1::new(1_000);
    let mut budget = ArgumentBudgetV1::new(&mut work, 1_000);
    let mut block = BasicBlock::new(BlockId(8));
    block.terminator = Some(Terminator::Branch {
        target: BlockId(9),
        arguments: vec![ValueId(2), ValueId(1)],
    });
    assert_eq!(
        instance_check_branch_v1(&block, BlockId(9), &[ValueId(1), ValueId(2)], &mut budget),
        Err(InstanceCorrespondenceErrorV1::Control)
    );
    assert_eq!(
        instance_check_branch_v1(&block, BlockId(7), &[ValueId(2), ValueId(1)], &mut budget),
        Err(InstanceCorrespondenceErrorV1::Control)
    );
    instance_check_branch_v1(&block, BlockId(9), &[ValueId(2), ValueId(1)], &mut budget).unwrap();
    block.parameters = vec![
        ValueDef::new(ValueId(4), Type::INDEX),
        ValueDef::new(ValueId(3), Type::INDEX),
    ];
    assert_eq!(
        instance_check_parameters_v1(&block, &[ValueId(3), ValueId(4)], &mut budget),
        Err(InstanceCorrespondenceErrorV1::Control)
    );
    instance_check_parameters_v1(&block, &[ValueId(4), ValueId(3)], &mut budget).unwrap();
    let mut work = fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1::new(2);
    let mut budget = ArgumentBudgetV1::new(&mut work, 7);
    budget.reserve_storage(7).unwrap();
    assert!(matches!(
        instance_check_branch_v1(&block, BlockId(9), &[ValueId(2), ValueId(1)], &mut budget),
        Err(InstanceCorrespondenceErrorV1::Resource(
            ArgumentResourceV1::Work(_)
        ))
    ));
    assert_eq!(budget.storage(), 7);
}

#[test]
fn instance_row_growth_prepays_relocation_coexistence_and_exact_storage_boundary() {
    let run = |limit| {
        let mut work = fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1::new(100);
        let mut budget = ArgumentBudgetV1::new(&mut work, limit);
        budget.reserve_storage(7).unwrap();
        let mut storage = 0;
        let mut rows = InstanceRowsV1::<u32>::new();
        rows.reserve(1, &mut budget, &mut storage).unwrap();
        rows.rows.push(1);
        let result = rows.reserve(4, &mut budget, &mut storage);
        let peak = budget.peak_storage();
        drop(rows);
        budget.release_storage(storage).unwrap();
        assert_eq!(budget.storage(), 7);
        (result, peak, budget.work())
    };
    let (result, peak, work) = run(1_000);
    result.unwrap();
    assert_eq!(peak, 7 + 12 * std::mem::size_of::<u32>());
    assert_eq!(work, 1);
    run(peak).0.unwrap();
    assert!(matches!(
        run(peak - 1).0,
        Err(InstanceCorrespondenceErrorV1::Resource(
            ArgumentResourceV1::Storage(_)
        ))
    ));
}

#[test]
fn instance_scope_releases_only_its_rows_on_success_and_failure() {
    with_plan(|plan, budget| {
        let floor = budget.storage();
        for fail in [false, true] {
            let result = with_production_instance_correspondence_v1(plan, budget, |map, budget| {
                map.values.reserve(7, budget, &mut map.storage)?;
                map.values.rows.push(ValueId(1));
                budget.reserve_storage(13)?;
                if fail {
                    Err(InstanceCorrespondenceErrorV1::Control)
                } else {
                    Ok(())
                }
            });
            if fail {
                assert_eq!(result, Err(InstanceCorrespondenceErrorV1::Control));
            } else {
                result.unwrap();
            }
            assert_eq!(budget.storage(), floor + 13);
            budget.release_storage(13).unwrap();
        }
    });
}

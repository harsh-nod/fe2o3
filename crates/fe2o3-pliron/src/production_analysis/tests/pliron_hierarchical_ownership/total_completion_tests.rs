#[test]
fn total_view_rejects_unknown_launch_out_of_range_and_duplicate_writers() {
    let context = &mut setup();
    let (dynamic_launch, _) = function(context, "total_dynamic_launch", 0);
    let entry = dynamic_launch.get_entry_block(context);
    let execution = layout(context, [0, 1, 1], [4, 1, 1], 2);
    let output = view(context, vec![1], vec![], MemorySpaceAttr::Global);
    let ownership = coverage_contract(
        context,
        output.result(context),
        OwnershipCoverageAttr::TotalView,
    );
    let ret = ReturnOp::new(context);
    for operation in [
        execution.get_operation(),
        output.get_operation(),
        ownership.get_operation(),
        ret.get_operation(),
    ] {
        operation.insert_at_back(entry, context);
    }
    let report = run_pliron_hierarchical_ownership_check_v1(context, &dynamic_launch);
    assert_eq!(report.status(), KernelCheckStatusV1::Incomplete);
    assert!(matches!(
        report.findings(),
        [HierarchicalOwnershipFindingV1::TraceIncomplete { detail }]
            if detail.contains("launch dimension 0 is dynamic")
    ));
    assert_eq!(report.coverage_summary().total_view_declared(), 1);
    assert_eq!(report.coverage_summary().total_view_proved(), 0);

    let context = &mut setup();
    let out_of_range = static_1d_with_coverage(
        context,
        "total_out_of_range",
        2,
        2,
        2,
        1,
        None,
        OwnershipCoverageAttr::TotalView,
        OwnershipPartitionAttr::ExactSets,
    );
    assert!(matches!(
        run_pliron_hierarchical_ownership_check_v1(context, &out_of_range).findings(),
        [HierarchicalOwnershipFindingV1::OutOfRange {
            coordinate,
            owner,
            ..
        }] if coordinate == &[1] && owner.invocation() == [1, 0, 0]
    ));

    let context = &mut setup();
    let duplicate = static_1d_with_coverage(
        context,
        "total_duplicate_writers",
        2,
        2,
        2,
        1,
        Some(1),
        OwnershipCoverageAttr::TotalView,
        OwnershipPartitionAttr::ExactSets,
    );
    assert!(matches!(
        run_pliron_hierarchical_ownership_check_v1(context, &duplicate).findings(),
        [HierarchicalOwnershipFindingV1::OverlappingOwners {
            coordinate,
            first,
            second,
            ..
        }] if coordinate == &[0]
            && first.invocation() == [0, 0, 0]
            && second.invocation() == [1, 0, 0]
    ));
}

#[test]
fn total_view_requires_normal_completion_and_disjoint_output_allocations() {
    let context = &mut setup();
    let (trapping, _) = function(context, "total_trap_after_write", 0);
    let entry = trapping.get_entry_block(context);
    let execution = layout(context, [1, 1, 1], [1, 1, 1], 1);
    let zero = IndexConstantOp::new(context, 0);
    let output = view(context, vec![1], vec![], MemorySpaceAttr::Global);
    let ownership = coverage_contract(
        context,
        output.result(context),
        OwnershipCoverageAttr::TotalView,
    );
    let store = write(context, output.result(context), vec![zero.result(context)]);
    let trap = TrapOp::new(context);
    for operation in [
        execution.get_operation(),
        zero.get_operation(),
        output.get_operation(),
        ownership.get_operation(),
        store.get_operation(),
        trap.get_operation(),
    ] {
        operation.insert_at_back(entry, context);
    }
    assert!(matches!(
        run_pliron_hierarchical_ownership_check_v1(context, &trapping).findings(),
        [HierarchicalOwnershipFindingV1::AbnormalCompletion {
            invocation,
            location,
            ..
        }] if invocation.invocation() == [0, 0, 0] && location.operation() == 5
    ));

    let context = &mut setup();
    let (aliasing, _) = function(context, "total_may_alias_outputs", 0);
    let entry = aliasing.get_entry_block(context);
    let execution = layout(context, [1, 1, 1], [1, 1, 1], 1);
    let zero = IndexConstantOp::new(context, 0);
    let first = view(context, vec![1], vec![], MemorySpaceAttr::Global);
    let second = view(context, vec![1], vec![], MemorySpaceAttr::Global);
    let first_contract = coverage_contract(
        context,
        first.result(context),
        OwnershipCoverageAttr::TotalView,
    );
    let second_contract = coverage_contract(
        context,
        second.result(context),
        OwnershipCoverageAttr::TotalView,
    );
    let first_write = write(context, first.result(context), vec![zero.result(context)]);
    let second_write = write(context, second.result(context), vec![zero.result(context)]);
    let ret = ReturnOp::new(context);
    for operation in [
        execution.get_operation(),
        zero.get_operation(),
        first.get_operation(),
        second.get_operation(),
        first_contract.get_operation(),
        second_contract.get_operation(),
        first_write.get_operation(),
        second_write.get_operation(),
        ret.get_operation(),
    ] {
        operation.insert_at_back(entry, context);
    }
    assert!(matches!(
        run_pliron_hierarchical_ownership_check_v1(context, &aliasing).findings(),
        [HierarchicalOwnershipFindingV1::MayAliasObservableWrite {
            contracted_noalias_class: 17,
            alias_noalias_class: 17,
            ..
        }]
    ));

    let context = &mut setup();
    let (disjoint, _) = function(context, "total_disjoint_outputs", 0);
    let entry = disjoint.get_entry_block(context);
    let execution = layout(context, [1, 1, 1], [1, 1, 1], 1);
    let zero = IndexConstantOp::new(context, 0);
    let first = view_with_allocation(context, vec![1], vec![], MemorySpaceAttr::Global, 17, 17);
    let second = view_with_allocation(context, vec![1], vec![], MemorySpaceAttr::Global, 18, 18);
    let first_contract = coverage_contract(
        context,
        first.result(context),
        OwnershipCoverageAttr::TotalView,
    );
    let second_contract = coverage_contract(
        context,
        second.result(context),
        OwnershipCoverageAttr::TotalView,
    );
    let first_write = write(context, first.result(context), vec![zero.result(context)]);
    let second_write = write(context, second.result(context), vec![zero.result(context)]);
    let ret = ReturnOp::new(context);
    for operation in [
        execution.get_operation(),
        zero.get_operation(),
        first.get_operation(),
        second.get_operation(),
        first_contract.get_operation(),
        second_contract.get_operation(),
        first_write.get_operation(),
        second_write.get_operation(),
        ret.get_operation(),
    ] {
        operation.insert_at_back(entry, context);
    }
    let report = run_pliron_hierarchical_ownership_check_v1(context, &disjoint);
    assert!(report.is_clean(), "{:#?}", report.findings());
    assert_eq!(report.coverage_summary().total_view_declared(), 2);
    assert_eq!(report.coverage_summary().total_view_proved(), 2);
}

#[test]
fn total_view_inventories_whole_allocation_global_writes() {
    let context = &mut setup();
    let (function, _) = function(context, "total_allocation_write", 0);
    let entry = function.get_entry_block(context);
    let execution = layout(context, [1, 1, 1], [1, 1, 1], 1);
    let zero = IndexConstantOp::new(context, 0);
    let output = view(context, vec![1], vec![], MemorySpaceAttr::Global);
    let ownership = coverage_contract(
        context,
        output.result(context),
        OwnershipCoverageAttr::TotalView,
    );
    let store = write(context, output.result(context), vec![zero.result(context)]);
    let allocation_effect = AllocationEffectOp::new(
        context,
        AccessKindAttr::Read,
        MemorySpaceAttr::Global,
        17,
        17,
    )
    .unwrap();
    allocation_effect.set_attr_kernel_allocation_effect_access_kind(context, AccessKindAttr::Write);
    let ret = ReturnOp::new(context);
    for operation in [
        execution.get_operation(),
        zero.get_operation(),
        output.get_operation(),
        ownership.get_operation(),
        store.get_operation(),
        allocation_effect.get_operation(),
        ret.get_operation(),
    ] {
        operation.insert_at_back(entry, context);
    }
    assert!(matches!(
        run_pliron_hierarchical_ownership_check_v1(context, &function).findings(),
        [HierarchicalOwnershipFindingV1::UnmodeledObservableAllocationWrite {
            allocation_origin: 17,
            noalias_class: 17,
            location,
        }] if location.operation() == 5
    ));
}

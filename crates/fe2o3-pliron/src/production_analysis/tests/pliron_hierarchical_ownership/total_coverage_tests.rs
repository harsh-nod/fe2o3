#[test]
fn total_view_proves_multidimensional_surjectivity_and_guarded_tail_finality() {
    let context = &mut setup();
    let (total_2d, _) = function(context, "total_2d", 0);
    let entry = total_2d.get_entry_block(context);
    let execution = layout(context, [3, 2, 1], [3, 1, 1], 1);
    let x = InvocationIndexOp::new(context, 0, 3);
    let y = InvocationIndexOp::new(context, 1, 2);
    let output = view(context, vec![3, 2], vec![], MemorySpaceAttr::Global);
    let ownership = coverage_contract(
        context,
        output.result(context),
        OwnershipCoverageAttr::TotalView,
    );
    let store = write(
        context,
        output.result(context),
        vec![x.result(context), y.result(context)],
    );
    for operation in [
        execution.get_operation(),
        x.get_operation(),
        y.get_operation(),
        output.get_operation(),
        ownership.get_operation(),
        store.get_operation(),
        ReturnOp::new(context).get_operation(),
    ] {
        operation.insert_at_back(entry, context);
    }
    let report = run_pliron_hierarchical_ownership_check_v1(context, &total_2d);
    assert!(report.is_clean(), "{:#?}", report.findings());
    assert!(report.all_total_view_contracts_are_proved());
    assert!(!report.all_collective_contribution_contracts_are_proved());
    assert_eq!(report.coverage_summary().total_view_declared(), 1);
    assert_eq!(report.coverage_summary().total_view_proved(), 1);
    assert!(
        report
            .regions()
            .iter()
            .all(|region| region.coverage() == OwnershipCoverageAttr::TotalView)
    );
    assert_eq!(
        report
            .regions()
            .iter()
            .find(|region| matches!(region.identity(), HierarchicalRegionIdentityV1::Grid(41)))
            .unwrap()
            .element_count(),
        6,
    );

    let context = &mut setup();
    let (tail, _) = function(context, "total_guarded_tail", 0);
    let entry = tail.get_entry_block(context);
    let body = block(context, &tail, "write");
    let exit = block(context, &tail, "exit");
    let execution = layout(context, [8, 1, 1], [4, 1, 1], 2);
    let invocation = InvocationIndexOp::new(context, 0, 8);
    let seven = IndexConstantOp::new(context, 7);
    let output = view(
        context,
        vec![0],
        vec![seven.result(context)],
        MemorySpaceAttr::Global,
    );
    let dimension = DimensionOp::new(context, output.result(context), 0).unwrap();
    let ownership = coverage_contract(
        context,
        output.result(context),
        OwnershipCoverageAttr::TotalView,
    );
    let guard = IndexLessThanBranchOp::new(
        context,
        invocation.result(context),
        dimension.result(context),
        body,
        exit,
    );
    for operation in [
        execution.get_operation(),
        invocation.get_operation(),
        seven.get_operation(),
        output.get_operation(),
        dimension.get_operation(),
        ownership.get_operation(),
        guard.get_operation(),
    ] {
        operation.insert_at_back(entry, context);
    }
    let store = write(
        context,
        output.result(context),
        vec![invocation.result(context)],
    );
    let branch = BranchOp::new(context, exit);
    let ret = ReturnOp::new(context);
    append(context, body, &store);
    append(context, body, &branch);
    append(context, exit, &ret);
    let report = run_pliron_hierarchical_ownership_check_v1(context, &tail);
    assert!(report.is_clean(), "{:#?}", report.findings());
    assert_eq!(
        report
            .regions()
            .iter()
            .find(|region| matches!(region.identity(), HierarchicalRegionIdentityV1::Grid(41)))
            .unwrap()
            .element_count(),
        7,
    );
}

#[test]
fn total_view_rejects_holes_overwrites_and_unmodeled_global_writes_with_witnesses() {
    let context = &mut setup();
    let hole = static_1d_with_coverage(
        context,
        "total_hole",
        8,
        4,
        2,
        9,
        None,
        OwnershipCoverageAttr::TotalView,
        OwnershipPartitionAttr::ExactSets,
    );
    assert!(matches!(
        run_pliron_hierarchical_ownership_check_v1(context, &hole).findings(),
        [HierarchicalOwnershipFindingV1::CoverageHole { coordinate, .. }]
            if coordinate == &[8]
    ));

    let context = &mut setup();
    let (overwrite, _) = function(context, "total_overwrite", 0);
    let entry = overwrite.get_entry_block(context);
    let execution = layout(context, [1, 1, 1], [1, 1, 1], 1);
    let invocation = InvocationIndexOp::new(context, 0, 1);
    let output = view(context, vec![1], vec![], MemorySpaceAttr::Global);
    let ownership = coverage_contract(
        context,
        output.result(context),
        OwnershipCoverageAttr::TotalView,
    );
    for operation in [
        execution.get_operation(),
        invocation.get_operation(),
        output.get_operation(),
        ownership.get_operation(),
        write(
            context,
            output.result(context),
            vec![invocation.result(context)],
        )
        .get_operation(),
        write(
            context,
            output.result(context),
            vec![invocation.result(context)],
        )
        .get_operation(),
        ReturnOp::new(context).get_operation(),
    ] {
        operation.insert_at_back(entry, context);
    }
    let report = run_pliron_hierarchical_ownership_check_v1(context, &overwrite);
    assert!(matches!(
        report.findings(),
        [HierarchicalOwnershipFindingV1::OutputOverwritten {
            coordinate,
            first,
            overwrite,
            ..
        }] if coordinate == &[0]
            && first.invocation() == [0, 0, 0]
            && overwrite.invocation() == [0, 0, 0]
    ));
    assert!(
        report.findings()[0]
            .to_string()
            .contains("one final observable write")
    );

    let context = &mut setup();
    let (extra, _) = function(context, "total_extra_write", 0);
    let entry = extra.get_entry_block(context);
    let execution = layout(context, [1, 1, 1], [1, 1, 1], 1);
    let invocation = InvocationIndexOp::new(context, 0, 1);
    let output = view(context, vec![1], vec![], MemorySpaceAttr::Global);
    let output_contract = coverage_contract(
        context,
        output.result(context),
        OwnershipCoverageAttr::TotalView,
    );
    let extra_type = RankedViewType::new(context, 32, true, vec![1]).unwrap();
    let extra_view = RankedViewOp::new_in_space_with_allocation_contract(
        context,
        extra_type,
        vec![],
        MemorySpaceAttr::Global,
        18,
        18,
    )
    .unwrap();
    for operation in [
        execution.get_operation(),
        invocation.get_operation(),
        output.get_operation(),
        extra_view.get_operation(),
        output_contract.get_operation(),
        write(
            context,
            output.result(context),
            vec![invocation.result(context)],
        )
        .get_operation(),
        write(
            context,
            extra_view.result(context),
            vec![invocation.result(context)],
        )
        .get_operation(),
        ReturnOp::new(context).get_operation(),
    ] {
        operation.insert_at_back(entry, context);
    }
    let report = run_pliron_hierarchical_ownership_check_v1(context, &extra);
    assert!(matches!(
        report.findings(),
        [HierarchicalOwnershipFindingV1::UnmodeledObservableWrite { location, .. }]
            if location.operation() == 6
    ));
}

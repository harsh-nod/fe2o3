fn cross_view_alias_report(
    first_origin: u64,
    first_class: u64,
    second_origin: u64,
    second_class: u64,
) -> crate::RankedRaceReportV1 {
    let context = &mut setup();
    let function = function(context, "cross_view_alias");
    let entry = function.get_entry_block(context);
    let layout = ExecutionLayoutOp::new(context, 52, [2, 1, 1], [2, 1, 1], 2);
    let first = view_with_contract(
        context,
        vec![3],
        MemorySpaceAttr::Global,
        first_origin,
        first_class,
    );
    let second = view_with_contract(
        context,
        vec![3],
        MemorySpaceAttr::Global,
        second_origin,
        second_class,
    );
    let invocation = InvocationIndexOp::new(context, 0, 2);
    let one = IndexConstantOp::new(context, 1);
    let shifted = IndexBinaryOp::new(
        context,
        IndexBinaryKindAttr::Add,
        invocation.result(context),
        one.result(context),
    );
    let first_write = access(
        context,
        AccessKindAttr::Write,
        first.result(context),
        invocation.result(context),
    );
    let second_write = access(
        context,
        AccessKindAttr::Write,
        second.result(context),
        shifted.result(context),
    );
    let ret = ReturnOp::new(context);
    append(context, entry, &layout);
    append(context, entry, &first);
    append(context, entry, &second);
    append(context, entry, &invocation);
    append(context, entry, &one);
    append(context, entry, &shifted);
    append(context, entry, &first_write);
    append(context, entry, &second_write);
    append(context, entry, &ret);
    run_pliron_ranked_race_check_v1(context, &function)
}

#[test]
fn same_noalias_class_without_relative_offsets_fails_closed() {
    let report = cross_view_alias_report(521, 53, 522, 53);
    assert_eq!(report.status(), KernelCheckStatusV1::Incomplete);
    assert!(report.findings().iter().any(|finding| matches!(
        finding,
        RankedRaceFindingV1::AllocationContractUnavailable { detail }
            if detail.contains("relative base offset")
    )));
}

#[test]
fn unknown_alias_views_without_relative_offsets_fail_closed() {
    let report = cross_view_alias_report(0, 0, 0, 0);
    assert_eq!(report.status(), KernelCheckStatusV1::Incomplete);
    assert!(report.findings().iter().any(|finding| matches!(
        finding,
        RankedRaceFindingV1::AllocationContractUnavailable { detail }
            if detail.contains("relative base offset")
    )));
}

#[test]
fn distinct_authenticated_noalias_classes_are_disjoint() {
    assert_eq!(
        cross_view_alias_report(521, 54, 522, 55).status(),
        KernelCheckStatusV1::Clean
    );
}

fn allocation_read_and_write_report(
    invocation_count: u64,
    read_origin: u64,
    read_class: u64,
    write_origin: u64,
    write_class: u64,
) -> crate::RankedRaceReportV1 {
    let context = &mut setup();
    let function = function(context, "allocation_read_and_write");
    let entry = function.get_entry_block(context);
    let layout = ExecutionLayoutOp::new(
        context,
        58,
        [invocation_count, 1, 1],
        [invocation_count, 1, 1],
        invocation_count,
    );
    let read = AllocationEffectOp::new(
        context,
        AccessKindAttr::Read,
        MemorySpaceAttr::Global,
        read_origin,
        read_class,
    )
    .unwrap();
    let output = view_with_contract(
        context,
        vec![invocation_count],
        MemorySpaceAttr::Global,
        write_origin,
        write_class,
    );
    let invocation = InvocationIndexOp::new(context, 0, invocation_count);
    let write = access(
        context,
        AccessKindAttr::Write,
        output.result(context),
        invocation.result(context),
    );
    let ret = ReturnOp::new(context);
    append(context, entry, &layout);
    append(context, entry, &read);
    append(context, entry, &output);
    append(context, entry, &invocation);
    append(context, entry, &write);
    append(context, entry, &ret);
    run_pliron_ranked_race_check_v1(context, &function)
}

#[test]
fn whole_allocation_read_is_safe_with_a_distinct_exclusive_output() {
    assert_eq!(
        allocation_read_and_write_report(64, 581, 58, 582, 59).status(),
        KernelCheckStatusV1::Clean
    );
}

#[test]
fn whole_allocation_read_and_same_class_write_are_safe_for_one_invocation() {
    assert_eq!(
        allocation_read_and_write_report(1, 581, 58, 581, 58).status(),
        KernelCheckStatusV1::Clean
    );
}

#[test]
fn whole_allocation_read_and_same_class_write_fail_closed_when_concurrent() {
    let report = allocation_read_and_write_report(64, 581, 58, 581, 58);
    assert_eq!(report.status(), KernelCheckStatusV1::Incomplete);
    assert!(matches!(
        report.findings(),
        [RankedRaceFindingV1::AllocationContractUnavailable { detail }]
            if detail.contains("whole-allocation read")
                && detail.contains("concurrent invocations")
                && !detail.contains("[0]")
                && !detail.contains("coordinate")
    ));
}

#[test]
fn whole_allocation_unknown_alias_read_fails_closed_against_an_output() {
    let report = allocation_read_and_write_report(64, 0, 0, 582, 59);
    assert_eq!(report.status(), KernelCheckStatusV1::Incomplete);
    assert!(matches!(
        report.findings(),
        [RankedRaceFindingV1::AllocationContractUnavailable { detail }]
            if detail.contains("unknown-alias")
    ));
}

#[test]
fn reserved_gfx950_transpose_effect_is_not_a_global_race() {
    let context = &mut setup();
    let function = function(context, "reserved_gfx950_transpose_effect");
    let entry = function.get_entry_block(context);
    let layout = ExecutionLayoutOp::new(context, 59, [64, 1, 1], [64, 1, 1], 64);
    let write = AllocationEffectOp::new(
        context,
        AccessKindAttr::Write,
        MemorySpaceAttr::Workgroup,
        GFX950_TRANSPOSE_FP8_WORKGROUP_ALLOCATION_ORIGIN_V1,
        GFX950_TRANSPOSE_FP8_WORKGROUP_NOALIAS_CLASS_V1,
    )
    .expect("reserved transpose write");
    let read = AllocationEffectOp::new(
        context,
        AccessKindAttr::Read,
        MemorySpaceAttr::Workgroup,
        GFX950_TRANSPOSE_FP8_WORKGROUP_ALLOCATION_ORIGIN_V1,
        GFX950_TRANSPOSE_FP8_WORKGROUP_NOALIAS_CLASS_V1,
    )
    .expect("reserved transpose read");
    let ret = ReturnOp::new(context);
    append(context, entry, &layout);
    append(context, entry, &write);
    append(context, entry, &read);
    append(context, entry, &ret);

    let preservation = crate::production_analysis::pliron_pass_contract::begin_production_pliron_pass_contract_session_v1(
        crate::production_analysis::pliron_ir_identity::LivePlironStructuralIdentityProviderV1::new(context, &function),
    ).unwrap();
    let census = preservation.input_census_v1();
    assert_eq!(census.operations, 4);
    assert_eq!(census.ranked_accesses, 0);
    assert_eq!(census.allocation_effects, 2);

    assert_eq!(
        run_pliron_ranked_race_check_v1(context, &function).status(),
        KernelCheckStatusV1::Clean
    );
}

#[test]
fn malformed_non_global_allocation_effect_cannot_fail_open() {
    let context = &mut setup();
    let function = function(context, "malformed_non_global_allocation_effect");
    let entry = function.get_entry_block(context);
    let effect = AllocationEffectOp::new(
        context,
        AccessKindAttr::Read,
        MemorySpaceAttr::Global,
        581,
        58,
    )
    .expect("valid allocation effect before hostile mutation");
    effect.set_attr_kernel_allocation_effect_memory_space(context, MemorySpaceAttr::Workgroup);
    let ret = ReturnOp::new(context);
    append(context, entry, &effect);
    append(context, entry, &ret);

    let report = run_pliron_ranked_race_check_v1(context, &function);
    assert_eq!(report.status(), KernelCheckStatusV1::Incomplete);
    assert!(matches!(
        report.findings(),
        [RankedRaceFindingV1::BoundsPrerequisiteRejected]
    ));
}

#[test]
fn incompatible_potentially_aliasing_view_signatures_fail_closed() {
    let context = &mut setup();
    let function = function(context, "incompatible_alias_views");
    let entry = function.get_entry_block(context);
    let first = view(context, vec![2], MemorySpaceAttr::Global);
    let second_type = RankedViewType::new(context, 64, true, vec![2]).expect("ranked view type");
    let second = RankedViewOp::new_in_space(context, second_type, vec![], MemorySpaceAttr::Global)
        .expect("ranked view");
    let zero = IndexConstantOp::new(context, 0);
    let first_write = access(
        context,
        AccessKindAttr::Write,
        first.result(context),
        zero.result(context),
    );
    let second_write = access(
        context,
        AccessKindAttr::Write,
        second.result(context),
        zero.result(context),
    );
    let ret = ReturnOp::new(context);
    append(context, entry, &first);
    append(context, entry, &second);
    append(context, entry, &zero);
    append(context, entry, &first_write);
    append(context, entry, &second_write);
    append(context, entry, &ret);

    let report = run_pliron_ranked_race_check_v1(context, &function);
    assert_eq!(report.status(), KernelCheckStatusV1::Incomplete);
    assert!(matches!(
        report.findings(),
        [RankedRaceFindingV1::AllocationContractUnavailable { .. }]
    ));
}

#[test]
fn heterogeneous_read_only_views_in_one_alias_class_are_clean() {
    let context = &mut setup();
    let function = function(context, "heterogeneous_shared_inputs");
    let entry = function.get_entry_block(context);
    let layout = ExecutionLayoutOp::new(context, 57, [64, 1, 1], [64, 1, 1], 64);
    let first_type = RankedViewType::new(context, 16, false, vec![4]).expect("ranked view type");
    let first = RankedViewOp::new_in_space_with_allocation_contract(
        context,
        first_type,
        vec![],
        MemorySpaceAttr::Global,
        571,
        57,
    )
    .expect("ranked view");
    let second_type = RankedViewType::new(context, 32, false, vec![8]).expect("ranked view type");
    let second = RankedViewOp::new_in_space_with_allocation_contract(
        context,
        second_type,
        vec![],
        MemorySpaceAttr::Global,
        572,
        57,
    )
    .expect("ranked view");
    let zero = IndexConstantOp::new(context, 0);
    let first_read = access(
        context,
        AccessKindAttr::Read,
        first.result(context),
        zero.result(context),
    );
    let second_read = access(
        context,
        AccessKindAttr::Read,
        second.result(context),
        zero.result(context),
    );
    let ret = ReturnOp::new(context);
    append(context, entry, &layout);
    append(context, entry, &first);
    append(context, entry, &second);
    append(context, entry, &zero);
    append(context, entry, &first_read);
    append(context, entry, &second_read);
    append(context, entry, &ret);

    assert!(run_pliron_ranked_race_check_v1(context, &function).is_clean());
}

fn guarded_unknown_read_with_two_disjoint_writes(
    context: &mut Context,
    read_noalias_class: u64,
) -> FuncOp {
    let function = function(context, "guarded_unknown_read_with_two_disjoint_writes");
    let entry = function.get_entry_block(context);
    let read_block = block(context, &function, "read");
    let exit = block(context, &function, "exit");
    let input_type = RankedViewType::new(context, 32, true, vec![128]).expect("input type");
    let input = RankedViewOp::new_in_space_with_allocation_contract(
        context,
        input_type,
        vec![],
        MemorySpaceAttr::Global,
        read_noalias_class,
        read_noalias_class,
    )
    .expect("input view");
    let output = view_with_contract(context, vec![128], MemorySpaceAttr::Global, 702, 702);
    let invocation = InvocationIndexOp::new(context, 0, 64);
    let offset = IndexConstantOp::new(context, 64);
    let input_extent = IndexConstantOp::new(context, 128);
    let second_index = IndexBinaryOp::new(
        context,
        IndexBinaryKindAttr::Add,
        invocation.result(context),
        offset.result(context),
    );
    let unknown = IndexUnknownOp::new(context);
    let guard = IndexLessThanBranchOp::new(
        context,
        unknown.result(context),
        input_extent.result(context),
        read_block,
        exit,
    );
    let first_write = access(
        context,
        AccessKindAttr::Write,
        output.result(context),
        invocation.result(context),
    );
    let second_write = access(
        context,
        AccessKindAttr::Write,
        output.result(context),
        second_index.result(context),
    );
    let read = access(
        context,
        AccessKindAttr::Read,
        input.result(context),
        unknown.result(context),
    );
    let to_exit = BranchOp::new(context, exit);
    let ret = ReturnOp::new(context);
    for operation in [
        input.get_operation(),
        output.get_operation(),
        invocation.get_operation(),
        offset.get_operation(),
        input_extent.get_operation(),
        second_index.get_operation(),
        unknown.get_operation(),
        first_write.get_operation(),
        second_write.get_operation(),
        guard.get_operation(),
    ] {
        operation.insert_at_back(entry, context);
    }
    append(context, read_block, &read);
    append(context, read_block, &to_exit);
    append(context, exit, &ret);
    function
}

#[test]
fn unresolved_read_only_class_does_not_block_disjoint_write_proof() {
    let context = &mut setup();
    let function = guarded_unknown_read_with_two_disjoint_writes(context, 701);

    assert!(run_pliron_ranked_race_check_v1(context, &function).is_clean());
}

#[test]
fn unresolved_read_in_writable_alias_class_still_fails_closed() {
    let context = &mut setup();
    let function = guarded_unknown_read_with_two_disjoint_writes(context, 702);

    let report = run_pliron_ranked_race_check_v1(context, &function);
    assert_eq!(report.status(), KernelCheckStatusV1::Incomplete);
    assert!(
        matches!(
            report.findings(),
            [RankedRaceFindingV1::UnresolvedIndex { .. }]
        ),
        "{:#?}",
        report.findings()
    );
}

#[test]
fn multidimensional_workgroup_identity_is_componentwise() {
    let context = &mut setup();
    let function = function(context, "multidimensional_scope");
    let entry = function.get_entry_block(context);
    let layout = ExecutionLayoutOp::new(context, 56, [16, 16, 1], [8, 8, 1], 64);
    let memory = view_with_contract(context, vec![1], MemorySpaceAttr::Global, 56, 56);
    let x = InvocationIndexOp::new(context, 0, 16);
    let y = InvocationIndexOp::new(context, 1, 16);
    let zero = IndexConstantOp::new(context, 0);
    let atomic = RankedAccessOp::new_atomic(
        context,
        AccessKindAttr::AtomicReadModifyWrite,
        AtomicOrderingAttr::AcquireRelease,
        AtomicScopeAttr::Workgroup,
        memory.result(context),
        vec![zero.result(context)],
    )
    .unwrap();
    let ret = ReturnOp::new(context);
    append(context, entry, &layout);
    append(context, entry, &memory);
    append(context, entry, &x);
    append(context, entry, &y);
    append(context, entry, &zero);
    append(context, entry, &atomic);
    append(context, entry, &ret);

    let report = run_pliron_ranked_race_check_v1(context, &function);
    assert_eq!(report.status(), KernelCheckStatusV1::Rejected);
    assert!(matches!(
        report.findings(),
        [RankedRaceFindingV1::InsufficientAtomicScope { first, second, .. }]
            if first.invocation() == [0, 0, 0]
                && second.invocation() == [8, 0, 0]
                && first.workgroup() == Some(0)
                && second.workgroup() == Some(1)
    ));
}

#[test]
fn invocation_axis_outside_retained_layout_fails_closed() {
    let context = &mut setup();
    let function = function(context, "unsupported_fourth_axis");
    let entry = function.get_entry_block(context);
    let layout = ExecutionLayoutOp::new(context, 58, [1, 1, 1], [1, 1, 1], 1);
    let memory = view_with_contract(context, vec![1], MemorySpaceAttr::Global, 58, 58);
    let fourth_axis = InvocationIndexOp::new(context, 3, 0);
    let zero = IndexConstantOp::new(context, 0);
    let read = access(
        context,
        AccessKindAttr::Read,
        memory.result(context),
        zero.result(context),
    );
    let ret = ReturnOp::new(context);
    append(context, entry, &layout);
    append(context, entry, &memory);
    append(context, entry, &fourth_axis);
    append(context, entry, &zero);
    append(context, entry, &read);
    append(context, entry, &ret);

    let report = run_pliron_ranked_race_check_v1(context, &function);
    assert_eq!(report.status(), KernelCheckStatusV1::Incomplete);
    assert!(matches!(
        report.findings(),
        [RankedRaceFindingV1::ExecutionLayoutUnavailable { detail }]
            if detail.contains("axis 3")
                && detail.contains("outside the three-dimensional gpu.execution_layout")
    ));
}

#[test]
fn dialect_index_type_is_still_the_only_function_index_type() {
    let context = &mut setup();
    let index: TypeHandle = dialect_kernel::IndexType::get(context).into();
    assert!(index.deref(context).is::<dialect_kernel::IndexType>());
}

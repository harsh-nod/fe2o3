#[test]
fn dynamic_launch_identity_is_symbolically_disjoint_after_a_bounds_guard() {
    let context = &mut setup();
    let function = function(context, "dynamic_identity");
    let entry = function.get_entry_block(context);
    let access_block = block(context, &function, "access");
    let exit = block(context, &function, "exit");
    let output = view(context, vec![1024], MemorySpaceAttr::Global);
    let invocation = InvocationIndexOp::new(context, 0, 0);
    let extent = IndexConstantOp::new(context, 1024);
    let branch = IndexLessThanBranchOp::new(
        context,
        invocation.result(context),
        extent.result(context),
        access_block,
        exit,
    );
    let write = RankedAccessOp::new(
        context,
        AccessKindAttr::Write,
        output.result(context),
        vec![invocation.result(context)],
    )
    .unwrap();
    let to_exit = BranchOp::new(context, exit);
    let ret = ReturnOp::new(context);
    append(context, entry, &output);
    append(context, entry, &invocation);
    append(context, entry, &extent);
    append(context, entry, &branch);
    append(context, access_block, &write);
    append(context, access_block, &to_exit);
    append(context, exit, &ret);

    assert!(run_pliron_ranked_race_check_v1(context, &function).is_clean());
}

#[test]
fn dynamic_launch_shift_is_symbolically_disjoint_after_a_bounds_guard() {
    let context = &mut setup();
    let function = function(context, "dynamic_shift");
    let entry = function.get_entry_block(context);
    let bounds_block = block(context, &function, "bounds");
    let access_block = block(context, &function, "access");
    let exit = block(context, &function, "exit");
    let output = view(context, vec![1028], MemorySpaceAttr::Global);
    let invocation = InvocationIndexOp::new(context, 0, 0);
    let offset = IndexConstantOp::new(context, 4);
    let shifted = IndexBinaryOp::new(
        context,
        IndexBinaryKindAttr::Add,
        invocation.result(context),
        offset.result(context),
    );
    let extent = IndexConstantOp::new(context, 1024);
    let branch = IndexLessThanBranchOp::new(
        context,
        invocation.result(context),
        extent.result(context),
        bounds_block,
        exit,
    );
    let output_extent = IndexConstantOp::new(context, 1028);
    let bounds_branch = IndexLessThanBranchOp::new(
        context,
        shifted.result(context),
        output_extent.result(context),
        access_block,
        exit,
    );
    let write = access(
        context,
        AccessKindAttr::Write,
        output.result(context),
        shifted.result(context),
    );
    let to_exit = BranchOp::new(context, exit);
    let ret = ReturnOp::new(context);
    for operation in [
        output.get_operation(),
        invocation.get_operation(),
        offset.get_operation(),
        shifted.get_operation(),
        extent.get_operation(),
        branch.get_operation(),
    ] {
        operation.insert_at_back(entry, context);
    }
    append(context, bounds_block, &output_extent);
    append(context, bounds_block, &bounds_branch);
    append(context, access_block, &write);
    append(context, access_block, &to_exit);
    append(context, exit, &ret);

    assert!(run_pliron_ranked_race_check_v1(context, &function).is_clean());
}

#[test]
fn dynamic_launch_bound_is_discarded_at_an_unguarded_merge() {
    let context = &mut setup();
    let function = function(context, "unguarded_merge");
    let entry = function.get_entry_block(context);
    let guarded = block(context, &function, "guarded");
    let unguarded = block(context, &function, "unguarded");
    let merge = block(context, &function, "merge");
    let access_block = block(context, &function, "access");
    let exit = block(context, &function, "exit");
    let output = view(context, vec![1028], MemorySpaceAttr::Global);
    let invocation = InvocationIndexOp::new(context, 0, 0);
    let offset = IndexConstantOp::new(context, 4);
    let shifted = IndexBinaryOp::new(
        context,
        IndexBinaryKindAttr::Add,
        invocation.result(context),
        offset.result(context),
    );
    let extent = IndexConstantOp::new(context, 1024);
    let branch = IndexLessThanBranchOp::new(
        context,
        invocation.result(context),
        extent.result(context),
        guarded,
        unguarded,
    );
    let guarded_to_merge = BranchOp::new(context, merge);
    let unguarded_to_merge = BranchOp::new(context, merge);
    let output_extent = IndexConstantOp::new(context, 1028);
    let bounds_branch = IndexLessThanBranchOp::new(
        context,
        shifted.result(context),
        output_extent.result(context),
        access_block,
        exit,
    );
    let write = access(
        context,
        AccessKindAttr::Write,
        output.result(context),
        shifted.result(context),
    );
    let to_exit = BranchOp::new(context, exit);
    let ret = ReturnOp::new(context);
    for operation in [
        output.get_operation(),
        invocation.get_operation(),
        offset.get_operation(),
        shifted.get_operation(),
        extent.get_operation(),
        branch.get_operation(),
    ] {
        operation.insert_at_back(entry, context);
    }
    append(context, guarded, &guarded_to_merge);
    append(context, unguarded, &unguarded_to_merge);
    append(context, merge, &output_extent);
    append(context, merge, &bounds_branch);
    append(context, access_block, &write);
    append(context, access_block, &to_exit);
    append(context, exit, &ret);

    let report = run_pliron_ranked_race_check_v1(context, &function);
    assert_eq!(report.status(), KernelCheckStatusV1::Incomplete);
    assert!(report.findings().iter().any(|finding| matches!(
        finding,
        RankedRaceFindingV1::DynamicLaunchExtent { dimension: 0 }
    )));
}

#[test]
fn dynamic_launch_equality_zero_proves_a_single_writer() {
    let context = &mut setup();
    let function = function(context, "single_writer");
    let entry = function.get_entry_block(context);
    let access_block = block(context, &function, "access");
    let exit = block(context, &function, "exit");
    let output = view(context, vec![1], MemorySpaceAttr::Global);
    let invocation = InvocationIndexOp::new(context, 0, 0);
    let zero = IndexConstantOp::new(context, 0);
    let branch = IndexEqualBranchOp::new(
        context,
        invocation.result(context),
        zero.result(context),
        access_block,
        exit,
    );
    let write = access(
        context,
        AccessKindAttr::Write,
        output.result(context),
        zero.result(context),
    );
    let to_exit = BranchOp::new(context, exit);
    let ret = ReturnOp::new(context);
    for operation in [
        output.get_operation(),
        invocation.get_operation(),
        zero.get_operation(),
        branch.get_operation(),
    ] {
        operation.insert_at_back(entry, context);
    }
    append(context, access_block, &write);
    append(context, access_block, &to_exit);
    append(context, exit, &ret);

    assert!(run_pliron_ranked_race_check_v1(context, &function).is_clean());
}

fn guarded_unknown_write(
    context: &mut Context,
    name: &str,
    launch_extents: [u64; 3],
    singleton_guards: [bool; 3],
) -> FuncOp {
    let function = function(context, name);
    let entry = function.get_entry_block(context);
    let access_block = block(context, &function, "access");
    let exit = block(context, &function, "exit");
    let layout = ExecutionLayoutOp::new(context, 73, launch_extents, [1, 1, 1], 1);
    let output = view(context, vec![8], MemorySpaceAttr::Global);
    let invocations: [InvocationIndexOp; 3] = std::array::from_fn(|dimension| {
        InvocationIndexOp::new(context, dimension as u32, launch_extents[dimension])
    });
    let one = IndexConstantOp::new(context, 1);
    let eight = IndexConstantOp::new(context, 8);
    let unknown = IndexUnknownOp::new(context);
    append(context, entry, &layout);
    append(context, entry, &output);
    for invocation in &invocations {
        append(context, entry, invocation);
    }
    append(context, entry, &one);
    append(context, entry, &eight);
    append(context, entry, &unknown);

    let mut current = entry;
    for (dimension, guarded) in singleton_guards.into_iter().enumerate() {
        if guarded {
            let next = block(context, &function, &format!("axis_{dimension}"));
            let guard = IndexLessThanBranchOp::new(
                context,
                invocations[dimension].result(context),
                one.result(context),
                next,
                exit,
            );
            append(context, current, &guard);
            current = next;
        }
    }
    let bounds = IndexLessThanBranchOp::new(
        context,
        unknown.result(context),
        eight.result(context),
        access_block,
        exit,
    );
    let write = access(
        context,
        AccessKindAttr::Write,
        output.result(context),
        unknown.result(context),
    );
    let to_exit = BranchOp::new(context, exit);
    let ret = ReturnOp::new(context);
    append(context, current, &bounds);
    append(context, access_block, &write);
    append(context, access_block, &to_exit);
    append(context, exit, &ret);
    function
}

#[test]
fn unknown_address_is_race_free_under_an_exact_singleton_invocation_domain() {
    let context = &mut setup();
    let function = guarded_unknown_write(
        context,
        "guarded_singleton_unknown_write",
        [0, 1, 1],
        [true, false, false],
    );

    assert!(run_pliron_ranked_race_check_v1(context, &function).is_clean());
}

#[test]
fn unknown_address_singleton_proof_rejects_every_static_extent_above_one() {
    for dimension in 0..3 {
        let context = &mut setup();
        let mut launch_extents = [1, 1, 1];
        launch_extents[dimension] = 2;
        let function = guarded_unknown_write(
            context,
            "nonsingleton_unknown_write",
            launch_extents,
            [false; 3],
        );

        assert_eq!(
            run_pliron_ranked_race_check_v1(context, &function).status(),
            KernelCheckStatusV1::Incomplete,
            "axis {dimension} must remain concurrent",
        );
    }
}

#[test]
fn unknown_address_singleton_proof_rejects_every_unbounded_dynamic_extent() {
    for dimension in 0..3 {
        let context = &mut setup();
        let mut launch_extents = [1, 1, 1];
        launch_extents[dimension] = 0;
        let function =
            guarded_unknown_write(context, "dynamic_unknown_write", launch_extents, [false; 3]);

        let report = run_pliron_ranked_race_check_v1(context, &function);
        assert_eq!(report.status(), KernelCheckStatusV1::Incomplete);
        assert!(matches!(
            report.findings(),
            [RankedRaceFindingV1::DynamicLaunchExtent { dimension: actual }]
                if *actual == dimension
        ));
    }
}

#[test]
fn unknown_address_singleton_proof_requires_extent_evidence_for_every_axis() {
    let context = &mut setup();
    let function = guarded_unknown_write(
        context,
        "partially_guarded_unknown_write",
        [0, 0, 0],
        [true, true, false],
    );

    let report = run_pliron_ranked_race_check_v1(context, &function);
    assert_eq!(report.status(), KernelCheckStatusV1::Incomplete);
    assert!(matches!(
        report.findings(),
        [RankedRaceFindingV1::DynamicLaunchExtent { .. }]
    ));
}

#[test]
fn unknown_address_singleton_proof_does_not_merge_distinct_coordinate_predicates() {
    let context = &mut setup();
    let function = function(context, "distinct_predicate_coordinates");
    let entry = function.get_entry_block(context);
    let second_predicate = block(context, &function, "second_predicate");
    let first_bounds = block(context, &function, "first_bounds");
    let second_bounds = block(context, &function, "second_bounds");
    let first_access = block(context, &function, "first_access");
    let second_access = block(context, &function, "second_access");
    let exit = block(context, &function, "exit");
    let layout = ExecutionLayoutOp::new(context, 74, [0, 1, 1], [1, 1, 1], 1);
    let output = view(context, vec![8], MemorySpaceAttr::Global);
    let invocation = InvocationIndexOp::new(context, 0, 0);
    let zero = IndexConstantOp::new(context, 0);
    let one = IndexConstantOp::new(context, 1);
    let eight = IndexConstantOp::new(context, 8);
    let first_unknown = IndexUnknownOp::new(context);
    let second_unknown = IndexUnknownOp::new(context);
    let select_zero = IndexEqualBranchOp::new(
        context,
        invocation.result(context),
        zero.result(context),
        first_bounds,
        second_predicate,
    );
    let select_one = IndexEqualBranchOp::new(
        context,
        invocation.result(context),
        one.result(context),
        second_bounds,
        exit,
    );
    let first_in_bounds = IndexLessThanBranchOp::new(
        context,
        first_unknown.result(context),
        eight.result(context),
        first_access,
        exit,
    );
    let second_in_bounds = IndexLessThanBranchOp::new(
        context,
        second_unknown.result(context),
        eight.result(context),
        second_access,
        exit,
    );
    let first_write = access(
        context,
        AccessKindAttr::Write,
        output.result(context),
        first_unknown.result(context),
    );
    let second_write = access(
        context,
        AccessKindAttr::Write,
        output.result(context),
        second_unknown.result(context),
    );
    let first_to_exit = BranchOp::new(context, exit);
    let second_to_exit = BranchOp::new(context, exit);
    let ret = ReturnOp::new(context);
    for operation in [
        layout.get_operation(),
        output.get_operation(),
        invocation.get_operation(),
        zero.get_operation(),
        one.get_operation(),
        eight.get_operation(),
        first_unknown.get_operation(),
        second_unknown.get_operation(),
        select_zero.get_operation(),
    ] {
        operation.insert_at_back(entry, context);
    }
    append(context, second_predicate, &select_one);
    append(context, first_bounds, &first_in_bounds);
    append(context, second_bounds, &second_in_bounds);
    append(context, first_access, &first_write);
    append(context, first_access, &first_to_exit);
    append(context, second_access, &second_write);
    append(context, second_access, &second_to_exit);
    append(context, exit, &ret);

    let report = run_pliron_ranked_race_check_v1(context, &function);
    assert_eq!(report.status(), KernelCheckStatusV1::Incomplete);
    assert!(matches!(
        report.findings(),
        [RankedRaceFindingV1::DynamicLaunchExtent { dimension: 0 }]
    ));
}

#[test]
fn opaque_index_is_race_free_only_in_an_exact_singleton_invocation_domain() {
    #[derive(Clone, Copy)]
    enum Case {
        StaticXGuarded,
        DynamicXUnguarded,
        DynamicYUnguarded,
        MissingLaunchLayout,
    }

    for case in [
        Case::StaticXGuarded,
        Case::DynamicXUnguarded,
        Case::DynamicYUnguarded,
        Case::MissingLaunchLayout,
    ] {
        let context = &mut setup();
        let (function, arguments) =
            function_with_index_arguments(context, "opaque_singleton_writer", 2);
        let entry = function.get_entry_block(context);
        let bounds_block = block(context, &function, "bounds");
        let access_block = block(context, &function, "access");
        let exit = block(context, &function, "exit");
        let layout = (!matches!(case, Case::MissingLaunchLayout)).then(|| {
            let launch_extents = match case {
                Case::DynamicXUnguarded => [0, 1, 1],
                Case::DynamicYUnguarded => [64, 0, 1],
                Case::StaticXGuarded | Case::MissingLaunchLayout => [64, 1, 1],
            };
            ExecutionLayoutOp::new(context, 75, launch_extents, [1, 1, 1], 1)
        });
        let view_type =
            RankedViewType::new(context, 32, true, vec![dialect_kernel::DYNAMIC_EXTENT]).unwrap();
        let output = RankedViewOp::new(context, view_type, vec![arguments[0]]).unwrap();
        let x = InvocationIndexOp::new(
            context,
            0,
            if matches!(case, Case::DynamicXUnguarded) {
                0
            } else {
                64
            },
        );
        let y =
            matches!(case, Case::DynamicYUnguarded).then(|| InvocationIndexOp::new(context, 1, 0));
        let one = IndexConstantOp::new(context, 1);
        let leader_guard = (!matches!(case, Case::DynamicXUnguarded)).then(|| {
            IndexLessThanBranchOp::new(
                context,
                x.result(context),
                one.result(context),
                bounds_block,
                exit,
            )
        });
        let unguarded =
            matches!(case, Case::DynamicXUnguarded).then(|| BranchOp::new(context, bounds_block));
        let bounds_guard =
            IndexLessThanBranchOp::new(context, arguments[1], arguments[0], access_block, exit);
        let write = access(
            context,
            AccessKindAttr::Write,
            output.result(context),
            arguments[1],
        );
        let to_exit = BranchOp::new(context, exit);
        let ret = ReturnOp::new(context);
        if let Some(layout) = &layout {
            append(context, entry, layout);
        }
        append(context, entry, &output);
        append(context, entry, &x);
        if let Some(y) = &y {
            append(context, entry, y);
        }
        append(context, entry, &one);
        match case {
            Case::DynamicXUnguarded => append(
                context,
                entry,
                unguarded.as_ref().expect("unguarded branch"),
            ),
            Case::StaticXGuarded | Case::DynamicYUnguarded | Case::MissingLaunchLayout => {
                append(context, entry, leader_guard.as_ref().expect("leader guard"))
            }
        }
        append(context, bounds_block, &bounds_guard);
        append(context, access_block, &write);
        append(context, access_block, &to_exit);
        append(context, exit, &ret);

        let report = run_pliron_ranked_race_check_v1(context, &function);
        match case {
            Case::StaticXGuarded => assert!(report.is_clean(), "{report:?}"),
            Case::DynamicXUnguarded => assert!(matches!(
                report.findings(),
                [RankedRaceFindingV1::DynamicLaunchExtent { dimension: 0 }]
            )),
            Case::DynamicYUnguarded => assert!(matches!(
                report.findings(),
                [RankedRaceFindingV1::DynamicLaunchExtent { dimension: 1 }]
            )),
            Case::MissingLaunchLayout => assert!(matches!(
                report.findings(),
                [RankedRaceFindingV1::UnresolvedIndex { dimension: 0, .. }]
            )),
        }
    }
}

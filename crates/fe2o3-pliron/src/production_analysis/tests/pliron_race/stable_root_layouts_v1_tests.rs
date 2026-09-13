fn stable_root_layout_report(tiled: bool, hostile: u8) -> RankedRaceReportV1 {
    use dialect_kernel::BranchArgsOp;
    let context = &mut setup();
    let (function, args) = function_with_index_arguments(context, "forwarded_checked_layout", 5);
    let entry = function.get_entry_block(context);
    let index: TypeHandle = IndexType::get(context).into();
    let mut blocks = Vec::new();
    for name in ["first", "second"] {
        let block = BasicBlock::new(context, Some(name.try_into().unwrap()), vec![index; 3]);
        block.insert_at_back(function.get_region(context), context);
        blocks.push(block);
    }
    let writes = [
        block(context, &function, "first_write"),
        block(context, &function, "second_write"),
    ];
    let exit = block(context, &function, "exit");
    let layout = ExecutionLayoutOp::new(context, 102, [64, 1, 1], [64, 1, 1], 64);
    let view_type =
        RankedViewType::new(context, 32, true, vec![dialect_kernel::DYNAMIC_EXTENT]).unwrap();
    let view = RankedViewOp::new(context, view_type, vec![args[0]]).unwrap();
    let invocation = InvocationIndexOp::new(context, 0, 64);
    let zero = IndexConstantOp::new(context, 0);
    let one = IndexConstantOp::new(context, 1);
    let enter = BranchArgsOp::new(
        context,
        vec![
            if hostile == 2 {
                invocation.result(context)
            } else {
                args[1]
            },
            args[2],
            args[3],
        ],
        blocks[0],
    );
    for op in [
        layout.get_operation(),
        view.get_operation(),
        invocation.get_operation(),
        zero.get_operation(),
        one.get_operation(),
        enter.get_operation(),
    ] {
        op.insert_at_back(entry, context);
    }
    for (position, block) in blocks.iter().copied().enumerate() {
        let rows = block.deref(context).get_argument(0);
        let columns = block.deref(context).get_argument(1);
        let stride = block.deref(context).get_argument(2);
        let invocation_value = if hostile == 3 {
            zero.result(context)
        } else {
            invocation.result(context)
        };
        let component = if position == 0 {
            zero.result(context)
        } else {
            one.result(context)
        };
        let (operation, value, success) = if tiled {
            let checked = CheckedTiledIndex2DOp::new_predicated(
                context,
                invocation_value,
                component,
                rows,
                columns,
                stride,
                args[0],
                [64, 16, 16, 4],
            );
            (
                checked.get_operation(),
                checked.result(context),
                checked.success(context).unwrap(),
            )
        } else {
            let checked = CheckedRowStripedIndex2DOp::new_predicated(
                context,
                invocation_value,
                component,
                rows,
                columns,
                stride,
                args[0],
                [8, 2],
            );
            (
                checked.get_operation(),
                checked.result(context),
                checked.success(context).unwrap(),
            )
        };
        operation.insert_at_back(block, context);
        // Checked success is not independent bounds authority. Guard the
        // actual address against this view's exact physical extent first.
        let write_block = writes[position];
        let guard = IndexLessThanBranchOp::new(context, value, args[0], write_block, exit);
        append(context, block, &guard);
        let write = RankedAccessOp::new_predicated(
            context,
            AccessKindAttr::Write,
            view.result(context),
            value,
            success,
        )
        .unwrap();
        append(context, write_block, &write);
        if position == 0 {
            let next = BranchArgsOp::new(
                context,
                vec![
                    args[1],
                    args[2],
                    if hostile == 1 { args[4] } else { args[3] },
                ],
                blocks[1],
            );
            append(context, write_block, &next);
        } else {
            let done = BranchOp::new(context, exit);
            append(context, write_block, &done);
        }
    }
    let ret = ReturnOp::new(context);
    append(context, exit, &ret);
    let bounds = crate::run_pliron_ranked_bounds_check_v1(context, &function);
    assert!(
        bounds.is_clean(),
        "race fixture prerequisite: tiled={tiled}, hostile={hostile}: {bounds:?}"
    );
    run_pliron_ranked_race_check_v1(context, &function)
}

#[test]
fn stable_roots_authenticate_forwarded_global_layouts_across_checked_effects() {
    for tiled in [false, true] {
        let report = stable_root_layout_report(tiled, 0);
        assert!(report.is_clean(), "tiled={tiled}: {report:?}");
        assert!(!report.grants_compiler_refinement_authority());
        assert!(!report.grants_artifact_or_launch_authority());
    }
}

#[test]
fn stable_roots_preserve_layout_equality_uniformity_and_injectivity_requirements() {
    for tiled in [false, true] {
        for hostile in 1..=3 {
            let report = stable_root_layout_report(tiled, hostile);
            assert!(
                !report.is_clean(),
                "tiled={tiled}, hostile={hostile}: {report:?}"
            );
            assert!(
                report
                    .findings()
                    .iter()
                    .any(|finding| matches!(finding, RankedRaceFindingV1::UnresolvedIndex { .. })),
                "tiled={tiled}, hostile={hostile}: {report:?}"
            );
            assert!(!report.grants_artifact_or_launch_authority());
        }
    }
}

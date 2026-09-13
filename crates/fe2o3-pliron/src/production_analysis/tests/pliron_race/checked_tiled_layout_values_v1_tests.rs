#[test]
fn predicated_checked_tiled_pairs_require_equivalent_uniform_layouts() {
    for (second_stride, varying_rows, equivalent) in
        [(16, false, true), (32, false, false), (16, true, false)]
    {
        let context = &mut setup();
        let (function, arguments) =
            function_with_index_arguments(context, "predicated_tiled_distinct_constants", 2);
        let entry = function.get_entry_block(context);
        let first_access = block(context, &function, "first_access");
        let second_access = block(context, &function, "second_access");
        let exit = block(context, &function, "exit");
        let view_type =
            RankedViewType::new(context, 32, true, vec![dialect_kernel::DYNAMIC_EXTENT]).unwrap();
        let output = RankedViewOp::new(context, view_type, vec![arguments[0]]).unwrap();
        let invocation = InvocationIndexOp::new(context, 0, 0);
        let first_component = IndexConstantOp::new(context, 0);
        let first_columns = IndexConstantOp::new(context, 16);
        let first_stride = IndexConstantOp::new(context, 16);
        let first = CheckedTiledIndex2DOp::new_predicated(
            context,
            invocation.result(context),
            first_component.result(context),
            arguments[1],
            first_columns.result(context),
            first_stride.result(context),
            arguments[0],
            [64, 16, 16, 4],
        );
        let second_component = IndexConstantOp::new(context, 1);
        let second_columns = IndexConstantOp::new(context, 16);
        let second_stride = IndexConstantOp::new(context, second_stride);
        let second = CheckedTiledIndex2DOp::new_predicated(
            context,
            invocation.result(context),
            second_component.result(context),
            if varying_rows {
                invocation.result(context)
            } else {
                arguments[1]
            },
            second_columns.result(context),
            second_stride.result(context),
            arguments[0],
            [64, 16, 16, 4],
        );
        let first_guard = IndexLessThanBranchOp::new(
            context,
            first.result(context),
            arguments[0],
            first_access,
            exit,
        );
        let second_guard = IndexLessThanBranchOp::new(
            context,
            second.result(context),
            arguments[0],
            second_access,
            exit,
        );
        for operation in [
            output.get_operation(),
            invocation.get_operation(),
            first_component.get_operation(),
            first_columns.get_operation(),
            first_stride.get_operation(),
            first.get_operation(),
            second_component.get_operation(),
            second_columns.get_operation(),
            second_stride.get_operation(),
            second.get_operation(),
            first_guard.get_operation(),
        ] {
            operation.insert_at_back(entry, context);
        }
        let first_write = RankedAccessOp::new_predicated(
            context,
            AccessKindAttr::Write,
            output.result(context),
            first.result(context),
            first.success(context).unwrap(),
        )
        .unwrap();
        let second_write = RankedAccessOp::new_predicated(
            context,
            AccessKindAttr::Write,
            output.result(context),
            second.result(context),
            second.success(context).unwrap(),
        )
        .unwrap();
        let to_exit = BranchOp::new(context, exit);
        let ret = ReturnOp::new(context);
        append(context, first_access, &first_write);
        append(context, first_access, &second_guard);
        append(context, second_access, &second_write);
        append(context, second_access, &to_exit);
        append(context, exit, &ret);

        let report = run_pliron_ranked_race_check_v1(context, &function);
        assert_eq!(report.is_clean(), equivalent, "{report:?}");
        assert!(!report.grants_compiler_refinement_authority());
        assert!(!report.grants_artifact_or_launch_authority());
    }
}

#[test]
fn checked_tiled_overflowing_invocation_is_not_proved_injective() {
    let context = &mut setup();
    let function = function(context, "checked_tiled_overflowing_invocation");
    let entry = function.get_entry_block(context);
    let access_block = block(context, &function, "access");
    let exit = block(context, &function, "exit");
    let output = view(context, vec![1], MemorySpaceAttr::Global);
    let invocation = InvocationIndexOp::new(context, 0, 3);
    let factor = IndexConstantOp::new(context, 1_u64 << 63);
    let tile_invocation = IndexBinaryOp::new(
        context,
        IndexBinaryKindAttr::Multiply,
        invocation.result(context),
        factor.result(context),
    );
    let zero = IndexConstantOp::new(context, 0);
    let sixteen = IndexConstantOp::new(context, 16);
    let tiled = CheckedTiledIndex2DOp::new(
        context,
        tile_invocation.result(context),
        zero.result(context),
        sixteen.result(context),
        sixteen.result(context),
        sixteen.result(context),
        [64, 16, 16, 4],
    );
    let extent = IndexConstantOp::new(context, 1);
    let guard = IndexLessThanBranchOp::new(
        context,
        tiled.result(context),
        extent.result(context),
        access_block,
        exit,
    );
    let write = access(
        context,
        AccessKindAttr::Write,
        output.result(context),
        tiled.result(context),
    );
    let to_exit = BranchOp::new(context, exit);
    let ret = ReturnOp::new(context);
    for operation in [
        output.get_operation(),
        invocation.get_operation(),
        factor.get_operation(),
        tile_invocation.get_operation(),
        zero.get_operation(),
        sixteen.get_operation(),
        tiled.get_operation(),
        extent.get_operation(),
        guard.get_operation(),
    ] {
        operation.insert_at_back(entry, context);
    }
    append(context, access_block, &write);
    append(context, access_block, &to_exit);
    append(context, exit, &ret);

    let report = run_pliron_ranked_race_check_v1(context, &function);
    assert_eq!(report.status(), KernelCheckStatusV1::Incomplete);
    assert!(
        report
            .findings()
            .iter()
            .any(|finding| matches!(finding, RankedRaceFindingV1::UnresolvedIndex { .. }))
    );
}

#[test]
fn checked_tiled_raw_marker_cannot_authorize_a_dynamic_launch() {
    let context = &mut setup();
    let function = function(context, "checked_tiled_dynamic_raw_invocation");
    let entry = function.get_entry_block(context);
    let access_block = block(context, &function, "access");
    let exit = block(context, &function, "exit");
    let output = view(context, vec![1], MemorySpaceAttr::Global);
    let invocation = InvocationIndexOp::new(context, 0, 0);
    let zero = IndexConstantOp::new(context, 0);
    let sixteen = IndexConstantOp::new(context, 16);
    let tiled = CheckedTiledIndex2DOp::new(
        context,
        invocation.result(context),
        zero.result(context),
        sixteen.result(context),
        sixteen.result(context),
        sixteen.result(context),
        [64, 16, 16, 4],
    );
    let extent = IndexConstantOp::new(context, 1);
    let guard = IndexLessThanBranchOp::new(
        context,
        tiled.result(context),
        extent.result(context),
        access_block,
        exit,
    );
    let write = access(
        context,
        AccessKindAttr::Write,
        output.result(context),
        tiled.result(context),
    );
    let to_exit = BranchOp::new(context, exit);
    let ret = ReturnOp::new(context);
    for operation in [
        output.get_operation(),
        invocation.get_operation(),
        zero.get_operation(),
        sixteen.get_operation(),
        tiled.get_operation(),
        extent.get_operation(),
        guard.get_operation(),
    ] {
        operation.insert_at_back(entry, context);
    }
    append(context, access_block, &write);
    append(context, access_block, &to_exit);
    append(context, exit, &ret);

    let report = run_pliron_ranked_race_check_v1(context, &function);
    assert_eq!(report.status(), KernelCheckStatusV1::Incomplete);
    assert!(matches!(
        report.findings(),
        [RankedRaceFindingV1::DynamicLaunchExtent { dimension: 0 }]
    ));
}

#[test]
fn checked_tiled_marker_shape_cannot_replace_validity_and_success_proofs() {
    #[derive(Clone, Copy, Debug)]
    enum HostileMarker {
        MissingSuccessEdge,
        ComponentEqualsElements,
        ComponentOutOfRange,
        DynamicComponent,
        TooSmallStride,
    }

    for hostile in [
        HostileMarker::MissingSuccessEdge,
        HostileMarker::ComponentEqualsElements,
        HostileMarker::ComponentOutOfRange,
        HostileMarker::DynamicComponent,
        HostileMarker::TooSmallStride,
    ] {
        let context = &mut setup();
        let function = function(context, "hostile_checked_tiled_marker");
        let entry = function.get_entry_block(context);
        let access_block = block(context, &function, "access");
        let exit = block(context, &function, "exit");
        let output = view(context, vec![4096], MemorySpaceAttr::Global);
        let invocation = InvocationIndexOp::new(context, 0, 2);
        let component_constant = IndexConstantOp::new(
            context,
            match hostile {
                HostileMarker::MissingSuccessEdge
                | HostileMarker::DynamicComponent
                | HostileMarker::TooSmallStride => 0,
                HostileMarker::ComponentEqualsElements => 4,
                HostileMarker::ComponentOutOfRange => 5,
            },
        );
        let rows = IndexConstantOp::new(context, 16);
        let columns = IndexConstantOp::new(context, 16);
        let stride = IndexConstantOp::new(
            context,
            if matches!(hostile, HostileMarker::TooSmallStride) {
                1
            } else {
                16
            },
        );
        let component = if matches!(hostile, HostileMarker::DynamicComponent) {
            invocation.result(context)
        } else {
            component_constant.result(context)
        };
        let tiled = CheckedTiledIndex2DOp::new(
            context,
            invocation.result(context),
            component,
            rows.result(context),
            columns.result(context),
            stride.result(context),
            [64, 16, 16, 4],
        );
        let extent = IndexConstantOp::new(context, 4096);
        let guard = IndexLessThanBranchOp::new(
            context,
            tiled.result(context),
            extent.result(context),
            access_block,
            exit,
        );
        let write = access(
            context,
            AccessKindAttr::Write,
            output.result(context),
            tiled.result(context),
        );
        let to_exit = BranchOp::new(context, exit);
        let ret = ReturnOp::new(context);
        for operation in [
            output.get_operation(),
            invocation.get_operation(),
            component_constant.get_operation(),
            rows.get_operation(),
            columns.get_operation(),
            stride.get_operation(),
            tiled.get_operation(),
            extent.get_operation(),
            guard.get_operation(),
        ] {
            operation.insert_at_back(entry, context);
        }
        append(context, access_block, &write);
        append(context, access_block, &to_exit);
        append(context, exit, &ret);

        let report = run_pliron_ranked_race_check_v1(context, &function);
        assert_eq!(
            report.status(),
            KernelCheckStatusV1::Incomplete,
            "raw marker {hostile:?} must not grant a proof: {report:?}"
        );
        assert!(matches!(
            report.findings(),
            [RankedRaceFindingV1::UnresolvedIndex { .. }]
        ));
        assert!(
            report.findings()[0]
                .to_string()
                .contains("no supported checked structured contract")
        );
    }
}

#[test]
fn checked_tiled_equivalent_layout_markers_do_not_supply_success_semantics() {
    let context = &mut setup();
    let function = function(context, "checked_tiled_distinct_layout_constants");
    let entry = function.get_entry_block(context);
    let second_access = block(context, &function, "second_access");
    let third_access = block(context, &function, "third_access");
    let exit = block(context, &function, "exit");
    let output = view(context, vec![256], MemorySpaceAttr::Global);
    let invocation = InvocationIndexOp::new(context, 0, 0);
    let component_zero = IndexConstantOp::new(context, 0);
    let component_one = IndexConstantOp::new(context, 1);
    let first_rows = IndexConstantOp::new(context, 16);
    let first_columns = IndexConstantOp::new(context, 16);
    let first_stride = IndexConstantOp::new(context, 16);
    let second_rows = IndexConstantOp::new(context, 16);
    let second_columns = IndexConstantOp::new(context, 16);
    let second_stride = IndexConstantOp::new(context, 16);
    let first = CheckedTiledIndex2DOp::new(
        context,
        invocation.result(context),
        component_zero.result(context),
        first_rows.result(context),
        first_columns.result(context),
        first_stride.result(context),
        [64, 16, 16, 4],
    );
    let second = CheckedTiledIndex2DOp::new(
        context,
        invocation.result(context),
        component_one.result(context),
        second_rows.result(context),
        second_columns.result(context),
        second_stride.result(context),
        [64, 16, 16, 4],
    );
    let extent = IndexConstantOp::new(context, 256);
    let first_guard = IndexLessThanBranchOp::new(
        context,
        first.result(context),
        extent.result(context),
        second_access,
        exit,
    );
    let first_write = access(
        context,
        AccessKindAttr::Write,
        output.result(context),
        first.result(context),
    );
    let second_guard = IndexLessThanBranchOp::new(
        context,
        second.result(context),
        extent.result(context),
        third_access,
        exit,
    );
    let second_write = access(
        context,
        AccessKindAttr::Write,
        output.result(context),
        second.result(context),
    );
    let to_exit = BranchOp::new(context, exit);
    let ret = ReturnOp::new(context);
    for operation in [
        output.get_operation(),
        invocation.get_operation(),
        component_zero.get_operation(),
        component_one.get_operation(),
        first_rows.get_operation(),
        first_columns.get_operation(),
        first_stride.get_operation(),
        second_rows.get_operation(),
        second_columns.get_operation(),
        second_stride.get_operation(),
        first.get_operation(),
        second.get_operation(),
        extent.get_operation(),
        first_guard.get_operation(),
    ] {
        operation.insert_at_back(entry, context);
    }
    append(context, second_access, &first_write);
    append(context, second_access, &second_guard);
    append(context, third_access, &second_write);
    append(context, third_access, &to_exit);
    append(context, exit, &ret);

    let report = run_pliron_ranked_race_check_v1(context, &function);
    assert_eq!(report.status(), KernelCheckStatusV1::Incomplete);
    assert!(matches!(
        report.findings(),
        [RankedRaceFindingV1::DynamicLaunchExtent { dimension: 0 }]
    ));
}

#[test]
fn checked_tiled_dynamic_layout_never_authorizes_a_raw_marker() {
    #[derive(Clone, Copy, Debug)]
    enum LayoutCase {
        SharedEntryArguments,
        DifferentEntryArguments,
        InvocationVarying,
    }

    for case in [
        LayoutCase::SharedEntryArguments,
        LayoutCase::DifferentEntryArguments,
        LayoutCase::InvocationVarying,
    ] {
        let context = &mut setup();
        let argument_count = match case {
            LayoutCase::SharedEntryArguments => 3,
            LayoutCase::DifferentEntryArguments => 4,
            LayoutCase::InvocationVarying => 0,
        };
        let (function, arguments) =
            function_with_index_arguments(context, "checked_tiled_dynamic_layout", argument_count);
        let entry = function.get_entry_block(context);
        let second_access = block(context, &function, "second_access");
        let third_access = block(context, &function, "third_access");
        let exit = block(context, &function, "exit");
        let output = view(context, vec![4096], MemorySpaceAttr::Global);
        let invocation = InvocationIndexOp::new(context, 0, 0);
        let component_zero = IndexConstantOp::new(context, 0);
        let component_one = IndexConstantOp::new(context, 1);
        let fallback_layout = IndexConstantOp::new(context, 16);
        let extent = IndexConstantOp::new(context, 4096);
        let (first_layout, second_layout) = match case {
            LayoutCase::SharedEntryArguments => (
                [arguments[0], arguments[1], arguments[2]],
                [arguments[0], arguments[1], arguments[2]],
            ),
            LayoutCase::DifferentEntryArguments => (
                [arguments[0], arguments[1], arguments[2]],
                [arguments[0], arguments[1], arguments[3]],
            ),
            LayoutCase::InvocationVarying => (
                [
                    invocation.result(context),
                    fallback_layout.result(context),
                    fallback_layout.result(context),
                ],
                [
                    invocation.result(context),
                    fallback_layout.result(context),
                    fallback_layout.result(context),
                ],
            ),
        };
        let first = CheckedTiledIndex2DOp::new(
            context,
            invocation.result(context),
            component_zero.result(context),
            first_layout[0],
            first_layout[1],
            first_layout[2],
            [64, 16, 16, 4],
        );
        let second = CheckedTiledIndex2DOp::new(
            context,
            invocation.result(context),
            component_one.result(context),
            second_layout[0],
            second_layout[1],
            second_layout[2],
            [64, 16, 16, 4],
        );
        let first_guard = IndexLessThanBranchOp::new(
            context,
            first.result(context),
            extent.result(context),
            second_access,
            exit,
        );
        let first_write = access(
            context,
            AccessKindAttr::Write,
            output.result(context),
            first.result(context),
        );
        let second_guard = IndexLessThanBranchOp::new(
            context,
            second.result(context),
            extent.result(context),
            third_access,
            exit,
        );
        let second_write = access(
            context,
            AccessKindAttr::Write,
            output.result(context),
            second.result(context),
        );
        let to_exit = BranchOp::new(context, exit);
        let ret = ReturnOp::new(context);
        for operation in [
            output.get_operation(),
            invocation.get_operation(),
            component_zero.get_operation(),
            component_one.get_operation(),
            fallback_layout.get_operation(),
            extent.get_operation(),
            first.get_operation(),
            second.get_operation(),
            first_guard.get_operation(),
        ] {
            operation.insert_at_back(entry, context);
        }
        append(context, second_access, &first_write);
        append(context, second_access, &second_guard);
        append(context, third_access, &second_write);
        append(context, third_access, &to_exit);
        append(context, exit, &ret);

        let report = run_pliron_ranked_race_check_v1(context, &function);
        assert_eq!(report.status(), KernelCheckStatusV1::Incomplete);
        assert!(matches!(
            report.findings(),
            [RankedRaceFindingV1::DynamicLaunchExtent { dimension: 0 }]
        ));
    }
}

#[test]
fn predicated_checked_access_proves_only_race_freedom() {
    for launch_extent in [0, 64] {
        for access_uses in [1, 2] {
            let context = &mut setup();
            let (function, arguments) =
                function_with_index_arguments(context, "raw_predicated_checked_access", 4);
            let entry = function.get_entry_block(context);
            let access_block = block(context, &function, "access");
            let exit = block(context, &function, "exit");
            let view_type =
                RankedViewType::new(context, 32, true, vec![dialect_kernel::DYNAMIC_EXTENT])
                    .unwrap();
            let output = RankedViewOp::new(context, view_type, vec![arguments[0]]).unwrap();
            let invocation = InvocationIndexOp::new(context, 0, launch_extent);
            let component = IndexConstantOp::new(context, 0);
            let checked = CheckedTiledIndex2DOp::new_predicated(
                context,
                invocation.result(context),
                component.result(context),
                arguments[1],
                arguments[2],
                arguments[3],
                arguments[0],
                [64, 16, 16, 4],
            );
            let guard = IndexLessThanBranchOp::new(
                context,
                checked.result(context),
                arguments[0],
                access_block,
                exit,
            );
            for operation in [
                output.get_operation(),
                invocation.get_operation(),
                component.get_operation(),
                checked.get_operation(),
                guard.get_operation(),
            ] {
                operation.insert_at_back(entry, context);
            }
            for _ in 0..access_uses {
                let write = RankedAccessOp::new_predicated(
                    context,
                    AccessKindAttr::Write,
                    output.result(context),
                    checked.result(context),
                    checked.success(context).unwrap(),
                )
                .unwrap();
                append(context, access_block, &write);
            }
            let to_exit = BranchOp::new(context, exit);
            let ret = ReturnOp::new(context);
            append(context, access_block, &to_exit);
            append(context, exit, &ret);

            let report = run_pliron_ranked_race_check_v1(context, &function);
            assert_eq!(report.status(), KernelCheckStatusV1::Clean);
            assert!(!report.grants_compiler_refinement_authority());
            assert!(!report.grants_artifact_or_launch_authority());
            let admitted =
                require_pliron_ranked_race_freedom_before_lowering_v1(context, &function)
                    .expect("the exact successful checked mapping is injective");
            assert!(!admitted.grants_compiler_refinement_authority());
            assert!(!admitted.grants_artifact_or_launch_authority());
        }
    }
}

#[test]
fn predicated_checked_row_striped_access_proves_four_workgroups_disjoint() {
    let context = &mut setup();
    let (function, arguments) =
        function_with_index_arguments(context, "predicated_row_striped_grid_four", 1);
    let entry = function.get_entry_block(context);
    let access_block = block(context, &function, "access");
    let second_guard_block = block(context, &function, "second_guard");
    let second_access_block = block(context, &function, "second_access");
    let exit = block(context, &function, "exit");
    let view_type =
        RankedViewType::new(context, 32, true, vec![dialect_kernel::DYNAMIC_EXTENT]).unwrap();
    let output = RankedViewOp::new(context, view_type, vec![arguments[0]]).unwrap();
    let invocation = InvocationIndexOp::new(context, 0, 1024);
    let first_component = IndexConstantOp::new(context, 0);
    let first_rows = IndexConstantOp::new(context, 16);
    let first_columns = IndexConstantOp::new(context, 128);
    let first_stride = IndexConstantOp::new(context, 128);
    let first_checked = CheckedRowStripedIndex2DOp::new_predicated(
        context,
        invocation.result(context),
        first_component.result(context),
        first_rows.result(context),
        first_columns.result(context),
        first_stride.result(context),
        arguments[0],
        [64, 2],
    );
    let second_component = IndexConstantOp::new(context, 1);
    let second_rows = IndexConstantOp::new(context, 16);
    let second_columns = IndexConstantOp::new(context, 128);
    let second_stride = IndexConstantOp::new(context, 128);
    let second_checked = CheckedRowStripedIndex2DOp::new_predicated(
        context,
        invocation.result(context),
        second_component.result(context),
        second_rows.result(context),
        second_columns.result(context),
        second_stride.result(context),
        arguments[0],
        [64, 2],
    );
    let guard = IndexLessThanBranchOp::new(
        context,
        first_checked.result(context),
        arguments[0],
        access_block,
        exit,
    );
    let second_guard = IndexLessThanBranchOp::new(
        context,
        second_checked.result(context),
        arguments[0],
        second_access_block,
        exit,
    );
    for operation in [
        output.get_operation(),
        invocation.get_operation(),
        first_component.get_operation(),
        first_rows.get_operation(),
        first_columns.get_operation(),
        first_stride.get_operation(),
        first_checked.get_operation(),
        second_component.get_operation(),
        second_rows.get_operation(),
        second_columns.get_operation(),
        second_stride.get_operation(),
        second_checked.get_operation(),
        guard.get_operation(),
    ] {
        operation.insert_at_back(entry, context);
    }
    let first_write = RankedAccessOp::new_predicated(
        context,
        AccessKindAttr::Write,
        output.result(context),
        first_checked.result(context),
        first_checked.success(context).unwrap(),
    )
    .unwrap();
    let second_write = RankedAccessOp::new_predicated(
        context,
        AccessKindAttr::Write,
        output.result(context),
        second_checked.result(context),
        second_checked.success(context).unwrap(),
    )
    .unwrap();
    let to_second_guard = BranchOp::new(context, second_guard_block);
    let to_exit = BranchOp::new(context, exit);
    let ret = ReturnOp::new(context);
    append(context, access_block, &first_write);
    append(context, access_block, &to_second_guard);
    append(context, second_guard_block, &second_guard);
    append(context, second_access_block, &second_write);
    append(context, second_access_block, &to_exit);
    append(context, exit, &ret);

    let report = run_pliron_ranked_race_check_v1(context, &function);
    assert!(report.is_clean(), "{report:?}");
}

#[test]
fn predicated_checked_row_striped_dynamic_component_is_disjoint() {
    for invocation_varying_layout in [false, true] {
        let context = &mut setup();
        let (function, arguments) =
            function_with_index_arguments(context, "predicated_row_striped_dynamic_component", 5);
        let entry = function.get_entry_block(context);
        let access_block = block(context, &function, "access");
        let exit = block(context, &function, "exit");
        let view_type =
            RankedViewType::new(context, 32, true, vec![dialect_kernel::DYNAMIC_EXTENT]).unwrap();
        let output = RankedViewOp::new(context, view_type, vec![arguments[0]]).unwrap();
        let invocation = InvocationIndexOp::new(context, 0, 0);
        let rows = if invocation_varying_layout {
            invocation.result(context)
        } else {
            arguments[2]
        };
        let checked = CheckedRowStripedIndex2DOp::new_predicated(
            context,
            invocation.result(context),
            arguments[1],
            rows,
            arguments[3],
            arguments[4],
            arguments[0],
            [64, 64],
        );
        let guard = IndexLessThanBranchOp::new(
            context,
            checked.result(context),
            arguments[0],
            access_block,
            exit,
        );
        for operation in [
            output.get_operation(),
            invocation.get_operation(),
            checked.get_operation(),
            guard.get_operation(),
        ] {
            operation.insert_at_back(entry, context);
        }
        let write = RankedAccessOp::new_predicated(
            context,
            AccessKindAttr::Write,
            output.result(context),
            checked.result(context),
            checked.success(context).unwrap(),
        )
        .unwrap();
        let to_exit = BranchOp::new(context, exit);
        let ret = ReturnOp::new(context);
        append(context, access_block, &write);
        append(context, access_block, &to_exit);
        append(context, exit, &ret);

        let report = run_pliron_ranked_race_check_v1(context, &function);
        if invocation_varying_layout {
            assert_eq!(report.status(), KernelCheckStatusV1::Incomplete);
            assert!(matches!(
                report.findings(),
                [RankedRaceFindingV1::DynamicLaunchExtent { dimension: 0 }]
            ));
        } else {
            assert!(report.is_clean(), "{report:?}");
        }
    }
}

#[test]
fn predicated_checked_access_rejects_invocation_varying_runtime_layout() {
    let context = &mut setup();
    let (function, arguments) =
        function_with_index_arguments(context, "varying_predicated_layout", 4);
    let entry = function.get_entry_block(context);
    let access_block = block(context, &function, "access");
    let exit = block(context, &function, "exit");
    let view_type =
        RankedViewType::new(context, 32, true, vec![dialect_kernel::DYNAMIC_EXTENT]).unwrap();
    let output = RankedViewOp::new(context, view_type, vec![arguments[0]]).unwrap();
    let invocation = InvocationIndexOp::new(context, 0, 0);
    let component = IndexConstantOp::new(context, 0);
    let checked = CheckedTiledIndex2DOp::new_predicated(
        context,
        invocation.result(context),
        component.result(context),
        invocation.result(context),
        arguments[2],
        arguments[3],
        arguments[0],
        [64, 16, 16, 4],
    );
    let guard = IndexLessThanBranchOp::new(
        context,
        checked.result(context),
        arguments[0],
        access_block,
        exit,
    );
    for operation in [
        output.get_operation(),
        invocation.get_operation(),
        component.get_operation(),
        checked.get_operation(),
        guard.get_operation(),
    ] {
        operation.insert_at_back(entry, context);
    }
    let write = RankedAccessOp::new_predicated(
        context,
        AccessKindAttr::Write,
        output.result(context),
        checked.result(context),
        checked.success(context).unwrap(),
    )
    .unwrap();
    let to_exit = BranchOp::new(context, exit);
    let ret = ReturnOp::new(context);
    append(context, access_block, &write);
    append(context, access_block, &to_exit);
    append(context, exit, &ret);

    let report = run_pliron_ranked_race_check_v1(context, &function);
    assert_eq!(report.status(), KernelCheckStatusV1::Incomplete);
    assert!(matches!(
        report.findings(),
        [RankedRaceFindingV1::DynamicLaunchExtent { dimension: 0 }]
    ));
}

#[test]
fn dynamic_multiaxis_mapping_requires_every_active_axis() {
    for drop_y in [false, true] {
        let context = &mut setup();
        let function = function(context, "dynamic_multiaxis");
        let entry = function.get_entry_block(context);
        let y_guard = block(context, &function, "y_guard");
        let access_block = block(context, &function, "access");
        let exit = block(context, &function, "exit");
        let shape = if drop_y { vec![1024] } else { vec![1024, 1024] };
        let output = view(context, shape, MemorySpaceAttr::Global);
        let x = InvocationIndexOp::new(context, 0, 0);
        let y = InvocationIndexOp::new(context, 1, 0);
        let extent = IndexConstantOp::new(context, 1024);
        let x_branch = IndexLessThanBranchOp::new(
            context,
            x.result(context),
            extent.result(context),
            y_guard,
            exit,
        );
        let y_branch = IndexLessThanBranchOp::new(
            context,
            y.result(context),
            extent.result(context),
            access_block,
            exit,
        );
        let indices = if drop_y {
            vec![x.result(context)]
        } else {
            vec![x.result(context), y.result(context)]
        };
        let write = RankedAccessOp::new(
            context,
            AccessKindAttr::Write,
            output.result(context),
            indices,
        )
        .unwrap();
        let to_exit = BranchOp::new(context, exit);
        let ret = ReturnOp::new(context);
        for operation in [
            output.get_operation(),
            x.get_operation(),
            y.get_operation(),
            extent.get_operation(),
            x_branch.get_operation(),
        ] {
            operation.insert_at_back(entry, context);
        }
        append(context, y_guard, &y_branch);
        append(context, access_block, &write);
        append(context, access_block, &to_exit);
        append(context, exit, &ret);

        let report = run_pliron_ranked_race_check_v1(context, &function);
        assert_eq!(
            report.status(),
            if drop_y {
                KernelCheckStatusV1::Incomplete
            } else {
                KernelCheckStatusV1::Clean
            }
        );
    }
}

#[test]
fn nonlinear_dynamic_mapping_remains_incomplete_even_with_finite_guards() {
    let context = &mut setup();
    let function = function(context, "nonlinear_dynamic_mapping");
    let entry = function.get_entry_block(context);
    let index_guard = block(context, &function, "index_guard");
    let access_block = block(context, &function, "access");
    let exit = block(context, &function, "exit");
    let output = view(context, vec![16], MemorySpaceAttr::Global);
    let invocation = InvocationIndexOp::new(context, 0, 0);
    let square = IndexBinaryOp::new(
        context,
        IndexBinaryKindAttr::Multiply,
        invocation.result(context),
        invocation.result(context),
    );
    let invocation_extent = IndexConstantOp::new(context, 4);
    let output_extent = IndexConstantOp::new(context, 16);
    let invocation_guard = IndexLessThanBranchOp::new(
        context,
        invocation.result(context),
        invocation_extent.result(context),
        index_guard,
        exit,
    );
    let bounds_guard = IndexLessThanBranchOp::new(
        context,
        square.result(context),
        output_extent.result(context),
        access_block,
        exit,
    );
    let write = access(
        context,
        AccessKindAttr::Write,
        output.result(context),
        square.result(context),
    );
    let to_exit = BranchOp::new(context, exit);
    let ret = ReturnOp::new(context);
    for operation in [
        output.get_operation(),
        invocation.get_operation(),
        square.get_operation(),
        invocation_extent.get_operation(),
        output_extent.get_operation(),
        invocation_guard.get_operation(),
    ] {
        operation.insert_at_back(entry, context);
    }
    append(context, index_guard, &bounds_guard);
    append(context, access_block, &write);
    append(context, access_block, &to_exit);
    append(context, exit, &ret);

    let report = run_pliron_ranked_race_check_v1(context, &function);
    assert_eq!(report.status(), KernelCheckStatusV1::Incomplete);
    assert!(matches!(
        report.findings(),
        [RankedRaceFindingV1::DynamicLaunchExtent { dimension: 0 }]
    ));
}

#[test]
fn checked_row_striped_raw_marker_cannot_authorize_a_dynamic_launch() {
    let context = &mut setup();
    let function = function(context, "checked_row_striped_dynamic_raw_invocation");
    let entry = function.get_entry_block(context);
    let access_block = block(context, &function, "access");
    let exit = block(context, &function, "exit");
    let output = view(context, vec![1], MemorySpaceAttr::Global);
    let invocation = InvocationIndexOp::new(context, 0, 0);
    let zero = IndexConstantOp::new(context, 0);
    let rows = IndexConstantOp::new(context, 7);
    let columns = IndexConstantOp::new(context, 257);
    let stride = IndexConstantOp::new(context, 269);
    let striped = CheckedRowStripedIndex2DOp::new(
        context,
        invocation.result(context),
        zero.result(context),
        rows.result(context),
        columns.result(context),
        stride.result(context),
        [64, 64],
    );
    let extent = IndexConstantOp::new(context, 1);
    let guard = IndexLessThanBranchOp::new(
        context,
        striped.result(context),
        extent.result(context),
        access_block,
        exit,
    );
    let write = access(
        context,
        AccessKindAttr::Write,
        output.result(context),
        striped.result(context),
    );
    let to_exit = BranchOp::new(context, exit);
    let ret = ReturnOp::new(context);
    for operation in [
        output.get_operation(),
        invocation.get_operation(),
        zero.get_operation(),
        rows.get_operation(),
        columns.get_operation(),
        stride.get_operation(),
        striped.get_operation(),
        extent.get_operation(),
        guard.get_operation(),
    ] {
        operation.insert_at_back(entry, context);
    }
    append(context, access_block, &write);
    append(context, access_block, &to_exit);
    append(context, exit, &ret);
    let report = run_pliron_ranked_race_check_v1(context, &function);
    assert_eq!(report.status(), KernelCheckStatusV1::Incomplete);
    assert!(matches!(
        report.findings(),
        [RankedRaceFindingV1::DynamicLaunchExtent { dimension: 0 }]
    ));
}

#[test]
fn checked_row_striped_shared_dynamic_layout_marker_is_not_authority() {
    let context = &mut setup();
    let (function, layout) =
        function_with_index_arguments(context, "checked_row_striped_dynamic_layout", 3);
    let entry = function.get_entry_block(context);
    let second_access = block(context, &function, "second_access");
    let third_access = block(context, &function, "third_access");
    let exit = block(context, &function, "exit");
    let output = view(context, vec![4096], MemorySpaceAttr::Global);
    let invocation = InvocationIndexOp::new(context, 0, 0);
    let component_zero = IndexConstantOp::new(context, 0);
    let component_one = IndexConstantOp::new(context, 1);
    let extent = IndexConstantOp::new(context, 4096);
    let first = CheckedRowStripedIndex2DOp::new(
        context,
        invocation.result(context),
        component_zero.result(context),
        layout[0],
        layout[1],
        layout[2],
        [64, 4],
    );
    let second = CheckedRowStripedIndex2DOp::new(
        context,
        invocation.result(context),
        component_one.result(context),
        layout[0],
        layout[1],
        layout[2],
        [64, 4],
    );
    let first_guard = IndexLessThanBranchOp::new(
        context,
        first.result(context),
        extent.result(context),
        second_access,
        exit,
    );
    let first_write = access(
        context,
        AccessKindAttr::Write,
        output.result(context),
        first.result(context),
    );
    let second_guard = IndexLessThanBranchOp::new(
        context,
        second.result(context),
        extent.result(context),
        third_access,
        exit,
    );
    let second_write = access(
        context,
        AccessKindAttr::Write,
        output.result(context),
        second.result(context),
    );
    let to_exit = BranchOp::new(context, exit);
    let ret = ReturnOp::new(context);
    for operation in [
        output.get_operation(),
        invocation.get_operation(),
        component_zero.get_operation(),
        component_one.get_operation(),
        extent.get_operation(),
        first.get_operation(),
        second.get_operation(),
        first_guard.get_operation(),
    ] {
        operation.insert_at_back(entry, context);
    }
    append(context, second_access, &first_write);
    append(context, second_access, &second_guard);
    append(context, third_access, &second_write);
    append(context, third_access, &to_exit);
    append(context, exit, &ret);

    let report = run_pliron_ranked_race_check_v1(context, &function);
    assert_eq!(report.status(), KernelCheckStatusV1::Incomplete);
    assert!(matches!(
        report.findings(),
        [RankedRaceFindingV1::DynamicLaunchExtent { dimension: 0 }]
    ));
}

#[test]
fn checked_row_striped_overflowing_invocation_is_not_proved_injective() {
    let context = &mut setup();
    let function = function(context, "checked_row_striped_overflowing_invocation");
    let entry = function.get_entry_block(context);
    let access_block = block(context, &function, "access");
    let exit = block(context, &function, "exit");
    let output = view(context, vec![1], MemorySpaceAttr::Global);
    let invocation = InvocationIndexOp::new(context, 0, 3);
    let factor = IndexConstantOp::new(context, 1_u64 << 63);
    let mapped_invocation = IndexBinaryOp::new(
        context,
        IndexBinaryKindAttr::Multiply,
        invocation.result(context),
        factor.result(context),
    );
    let zero = IndexConstantOp::new(context, 0);
    let rows = IndexConstantOp::new(context, 7);
    let columns = IndexConstantOp::new(context, 257);
    let stride = IndexConstantOp::new(context, 269);
    let striped = CheckedRowStripedIndex2DOp::new(
        context,
        mapped_invocation.result(context),
        zero.result(context),
        rows.result(context),
        columns.result(context),
        stride.result(context),
        [64, 64],
    );
    let extent = IndexConstantOp::new(context, 1);
    let guard = IndexLessThanBranchOp::new(
        context,
        striped.result(context),
        extent.result(context),
        access_block,
        exit,
    );
    let write = access(
        context,
        AccessKindAttr::Write,
        output.result(context),
        striped.result(context),
    );
    let to_exit = BranchOp::new(context, exit);
    let ret = ReturnOp::new(context);
    for operation in [
        output.get_operation(),
        invocation.get_operation(),
        factor.get_operation(),
        mapped_invocation.get_operation(),
        zero.get_operation(),
        rows.get_operation(),
        columns.get_operation(),
        stride.get_operation(),
        striped.get_operation(),
        extent.get_operation(),
        guard.get_operation(),
    ] {
        operation.insert_at_back(entry, context);
    }
    append(context, access_block, &write);
    append(context, access_block, &to_exit);
    append(context, exit, &ret);
    assert_eq!(
        run_pliron_ranked_race_check_v1(context, &function).status(),
        KernelCheckStatusV1::Incomplete
    );
}

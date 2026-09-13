#[derive(Clone, Copy)]
enum PhiSelectorFixtureV1 {
    Lane,
    Parameter,
    SubgroupCutoff,
    Unknown,
}

fn phi_selected_matrix_report_v1(
    selector: PhiSelectorFixtureV1,
    direct_duplicate_target: bool,
    identical_incoming: bool,
    equality: bool,
) -> crate::PlironTensorLayoutReportV1 {
    let context = &mut setup();
    let (function, arguments) = function(context, "phi_selected_matrix", 1);
    let entry = function.get_entry_block(context);
    let (join, phi) = index_block(context, &function, "join");
    let matrix_block = block(context, &function, "matrix");
    let exit = block(context, &function, "exit");
    let execution = layout(context, 0, 64, 64);
    let zero = IndexConstantOp::new(context, 0);
    let one = IndexConstantOp::new(context, 1);
    let cutoff = IndexConstantOp::new(
        context,
        if matches!(selector, PhiSelectorFixtureV1::SubgroupCutoff) {
            64
        } else {
            32
        },
    );
    append(context, entry, &execution);
    append(context, entry, &zero);
    append(context, entry, &one);
    append(context, entry, &cutoff);
    let control = match selector {
        PhiSelectorFixtureV1::Parameter => arguments[0],
        PhiSelectorFixtureV1::Unknown => {
            let unknown = IndexUnknownOp::new(context);
            append(context, entry, &unknown);
            unknown.result(context)
        }
        PhiSelectorFixtureV1::Lane | PhiSelectorFixtureV1::SubgroupCutoff => {
            let lane = InvocationIndexOp::new(context, 0, 0);
            append(context, entry, &lane);
            lane.result(context)
        }
    };
    let left_value = zero.result(context);
    let right_value = if identical_incoming {
        left_value
    } else {
        one.result(context)
    };
    if direct_duplicate_target {
        if equality {
            let branch = IndexEqualBranchArgsOp::new(
                context,
                control,
                cutoff.result(context),
                vec![left_value],
                vec![right_value],
                join,
                join,
            );
            append(context, entry, &branch);
        } else if matches!(selector, PhiSelectorFixtureV1::Unknown) {
            let branch = AnalysisSplitOp::new_with_control_and_arguments(
                context,
                vec![control],
                vec![left_value],
                vec![right_value],
                join,
                join,
            );
            append(context, entry, &branch);
        } else {
            let branch = IndexLessThanBranchArgsOp::new(
                context,
                control,
                cutoff.result(context),
                vec![left_value],
                vec![right_value],
                join,
                join,
            );
            append(context, entry, &branch);
        }
    } else {
        let left = block(context, &function, "left");
        let right = block(context, &function, "right");
        let branch =
            IndexLessThanBranchOp::new(context, control, cutoff.result(context), left, right);
        let left_join = BranchArgsOp::new(context, vec![left_value], join);
        let right_join = BranchArgsOp::new(context, vec![right_value], join);
        append(context, entry, &branch);
        append(context, left, &left_join);
        append(context, right, &right_join);
    }
    let select_matrix =
        IndexEqualBranchOp::new(context, phi, zero.result(context), matrix_block, exit);
    let matrix = tensor(context, 64);
    let matrix_exit = BranchOp::new(context, exit);
    let ret = ReturnOp::new(context);
    append(context, join, &select_matrix);
    append(context, matrix_block, &matrix);
    append(context, matrix_block, &matrix_exit);
    append(context, exit, &ret);
    verify_operation(function.get_operation(), context).unwrap();
    run_pliron_tensor_layout_check_v1(context, &function)
}

#[test]
fn divergent_diamond_phi_cannot_select_a_later_matrix_after_reconvergence() {
    let report = phi_selected_matrix_report_v1(PhiSelectorFixtureV1::Lane, false, false, false);
    assert!(
        report.findings().iter().any(|finding| matches!(
            finding,
            PlironTensorLayoutFindingV1::DivergentSubgroupControl { controller: 1, .. }
        )),
        "{:?}",
        report.findings()
    );
}

#[test]
fn duplicate_target_edges_keep_distinct_selector_dependent_phi_values() {
    for equality in [false, true] {
        let report =
            phi_selected_matrix_report_v1(PhiSelectorFixtureV1::Lane, true, false, equality);
        assert_eq!(
            report.status(),
            KernelCheckStatusV1::Rejected,
            "{:?}",
            report.findings()
        );
    }
}

#[test]
fn identical_phi_inputs_do_not_inherit_irrelevant_divergent_selection() {
    for direct in [false, true] {
        assert!(
            phi_selected_matrix_report_v1(PhiSelectorFixtureV1::Lane, direct, true, false)
                .is_clean()
        );
    }
}

#[test]
fn uniform_control_can_select_distinct_uniform_phi_values() {
    for direct in [false, true] {
        assert!(
            phi_selected_matrix_report_v1(PhiSelectorFixtureV1::Parameter, direct, false, false)
                .is_clean()
        );
        assert!(
            phi_selected_matrix_report_v1(
                PhiSelectorFixtureV1::SubgroupCutoff,
                direct,
                false,
                false
            )
            .is_clean()
        );
    }
}

#[test]
fn unknown_split_selection_remains_incomplete_after_phi_reconvergence() {
    let report = phi_selected_matrix_report_v1(PhiSelectorFixtureV1::Unknown, true, false, false);
    assert_eq!(
        report.status(),
        KernelCheckStatusV1::Incomplete,
        "{:?}",
        report.findings()
    );
    assert!(
        phi_selected_matrix_report_v1(PhiSelectorFixtureV1::Unknown, true, true, false).is_clean()
    );
}

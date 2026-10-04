#[derive(Clone, Copy)]
enum ForwardedLoopSelectorV1 {
    Opaque,
    Lane,
}

fn forwarded_loop_phi_report_v1(
    selector: ForwardedLoopSelectorV1,
    identical_payload: bool,
) -> crate::PlironTensorLayoutReportV1 {
    let context = &mut setup();
    let (function, arguments) = function(context, "forwarded_loop_phi", 1);
    let entry = function.get_entry_block(context);
    let (header, induction) = index_block(context, &function, "header");
    let (body, body_induction) = index_block(context, &function, "body");
    let (left, left_induction) = index_block(context, &function, "left");
    let (right, right_induction) = index_block(context, &function, "right");
    let (join, joined_induction) = index_block(context, &function, "join");
    let exit = block(context, &function, "exit");
    let execution = layout(context, 0, 64, 64);
    let zero = IndexConstantOp::new(context, 0);
    let one = IndexConstantOp::new(context, 1);
    let cutoff = IndexConstantOp::new(context, 32);
    let lane = InvocationIndexOp::new(context, 0, 0);
    let enter = BranchArgsOp::new(context, vec![zero.result(context)], header);
    let condition = IndexLessThanBranchArgsOp::new(
        context,
        induction,
        arguments[0],
        vec![induction],
        vec![],
        body,
        exit,
    );
    let matrix = tensor(context, 64);
    let right_payload = if identical_payload {
        body_induction
    } else {
        one.result(context)
    };
    assert_ne!(left_induction, right_induction);
    let left_join = BranchArgsOp::new(context, vec![left_induction], join);
    let right_join = BranchArgsOp::new(context, vec![right_induction], join);
    let next = IndexBinaryOp::new(
        context,
        IndexBinaryKindAttr::Add,
        joined_induction,
        one.result(context),
    );
    let repeat = BranchArgsOp::new(context, vec![next.result(context)], header);
    let ret = ReturnOp::new(context);
    append(context, entry, &execution);
    append(context, entry, &zero);
    append(context, entry, &one);
    append(context, entry, &cutoff);
    append(context, entry, &lane);
    append(context, entry, &enter);
    append(context, header, &condition);
    append(context, body, &matrix);
    match selector {
        ForwardedLoopSelectorV1::Opaque => {
            let split = AnalysisSplitOp::new_with_control_and_arguments(
                context,
                vec![],
                vec![body_induction],
                vec![right_payload],
                left,
                right,
            );
            append(context, body, &split);
        }
        ForwardedLoopSelectorV1::Lane => {
            let split = IndexLessThanBranchArgsOp::new(
                context,
                lane.result(context),
                cutoff.result(context),
                vec![body_induction],
                vec![right_payload],
                left,
                right,
            );
            append(context, body, &split);
        }
    }
    append(context, left, &left_join);
    append(context, right, &right_join);
    append(context, join, &next);
    append(context, join, &repeat);
    append(context, exit, &ret);
    verify_operation(function.get_operation(), context).unwrap();
    run_pliron_tensor_layout_check_v1(context, &function)
}

#[test]
fn forwarded_equal_loop_phi_does_not_inherit_opaque_selection() {
    let report = forwarded_loop_phi_report_v1(ForwardedLoopSelectorV1::Opaque, true);
    assert!(report.is_clean(), "{:?}", report.findings());
}

#[test]
fn forwarded_equal_loop_phi_does_not_inherit_lane_selection() {
    let report = forwarded_loop_phi_report_v1(ForwardedLoopSelectorV1::Lane, true);
    assert!(report.is_clean(), "{:?}", report.findings());
}

#[test]
fn forwarded_unequal_loop_phi_keeps_selector_dependence() {
    let opaque = forwarded_loop_phi_report_v1(ForwardedLoopSelectorV1::Opaque, false);
    assert_eq!(opaque.status(), KernelCheckStatusV1::Incomplete);
    assert!(opaque.findings().iter().any(|finding| matches!(
        finding,
        PlironTensorLayoutFindingV1::ConvergenceAnalysisIncomplete { detail }
            if detail.contains("unresolved branch block 1")
    )));
    let varying = forwarded_loop_phi_report_v1(ForwardedLoopSelectorV1::Lane, false);
    assert_eq!(varying.status(), KernelCheckStatusV1::Rejected);
    assert!(varying.findings().iter().any(|finding| matches!(
        finding,
        PlironTensorLayoutFindingV1::DivergentSubgroupControl { controller: 1, .. }
    )));
}

#[test]
fn forwarded_phi_cycles_terminate_without_changing_control() {
    for forwarding_blocks in [1, 2, 9] {
        let context = &mut setup();
        let (function, arguments) = function(context, "forwarded_phi_cycle", 1);
        let entry = function.get_entry_block(context);
        let (header, induction) = index_block(context, &function, "header");
        let forwarding = (0..forwarding_blocks)
            .map(|ordinal| index_block(context, &function, &format!("forward_{ordinal}")))
            .collect::<Vec<_>>();
        let exit = block(context, &function, "exit");
        let execution = layout(context, 0, 64, 64);
        let zero = IndexConstantOp::new(context, 0);
        let one = IndexConstantOp::new(context, 1);
        let enter = BranchArgsOp::new(context, vec![zero.result(context)], header);
        let condition = IndexLessThanBranchArgsOp::new(
            context,
            arguments[0],
            one.result(context),
            vec![induction],
            vec![],
            forwarding[0].0,
            exit,
        );
        append(context, entry, &execution);
        append(context, entry, &zero);
        append(context, entry, &one);
        append(context, entry, &enter);
        append(context, header, &condition);
        for (ordinal, (source, value)) in forwarding.iter().copied().enumerate() {
            let target = forwarding.get(ordinal + 1).map_or(header, |next| next.0);
            let edge = BranchArgsOp::new(context, vec![value], target);
            append(context, source, &edge);
        }
        let matrix = tensor(context, 64);
        let ret = ReturnOp::new(context);
        append(context, exit, &matrix);
        append(context, exit, &ret);
        verify_operation(function.get_operation(), context).unwrap();
        let report = run_pliron_tensor_layout_check_v1(context, &function);
        assert!(
            report.is_clean(),
            "forwarding blocks={forwarding_blocks}: {:?}",
            report.findings()
        );
    }
}

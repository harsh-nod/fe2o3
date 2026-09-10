use super::*;
use dialect_kernel::{AnalysisSplitOp, IndexEqualBranchArgsOp};

#[derive(Clone, Copy, Debug)]
enum EdgeKind {
    Split,
    LessThan,
    Equal,
}

fn duplicate_target_loop(context: &mut Context, kind: EdgeKind, reset: Option<usize>) -> FuncOp {
    let (function, arguments) = make_function(context, "duplicate_target_loop", 1);
    let entry = function.get_entry_block(context);
    let (header, induction) = index_block_n(context, &function, "header", 2);
    let (body, body_values) = index_block_n(context, &function, "body", 2);
    let (latch, latch_values) = index_block_n(context, &function, "latch", 2);
    let exit = block(context, &function, "exit");
    let zero = IndexConstantOp::new(context, 0);
    let one = IndexConstantOp::new(context, 1);
    let bound = IndexConstantOp::new(context, 2);
    for operation in [&zero, &one, &bound] {
        append(context, entry, operation);
    }
    let enter = BranchArgsOp::new(
        context,
        vec![zero.result(context), one.result(context)],
        header,
    );
    append(context, entry, &enter);
    let condition = IndexLessThanBranchArgsOp::new(
        context,
        induction[0],
        bound.result(context),
        induction.clone(),
        vec![],
        body,
        exit,
    );
    append(context, header, &condition);

    let mut first = body_values.clone();
    let mut second = body_values.clone();
    match reset {
        Some(0) => first[0] = zero.result(context),
        Some(1) => second[0] = zero.result(context),
        None => {}
        _ => panic!("invalid reset edge"),
    }
    match kind {
        EdgeKind::Split => {
            let split = AnalysisSplitOp::new_with_arguments(context, first, second, latch, latch);
            append(context, body, &split);
        }
        EdgeKind::LessThan => {
            let split = IndexLessThanBranchArgsOp::new(
                context,
                arguments[0],
                one.result(context),
                first,
                second,
                latch,
                latch,
            );
            append(context, body, &split);
        }
        EdgeKind::Equal => {
            let split = IndexEqualBranchArgsOp::new(
                context,
                arguments[0],
                one.result(context),
                first,
                second,
                latch,
                latch,
            );
            append(context, body, &split);
        }
    }
    let next = IndexBinaryOp::new(
        context,
        IndexBinaryKindAttr::Add,
        latch_values[0],
        one.result(context),
    );
    append(context, latch, &next);
    let repeat = BranchArgsOp::new(context, vec![next.result(context), latch_values[1]], header);
    append(context, latch, &repeat);
    let ret = ReturnOp::new(context);
    append(context, exit, &ret);
    verify_operation(function.get_operation(), context).expect("well-formed parallel CFG edges");
    function
}

#[test]
fn identical_duplicate_target_arguments_preserve_nested_progress() {
    for kind in [EdgeKind::Split, EdgeKind::LessThan, EdgeKind::Equal] {
        let mut context = setup();
        let function = duplicate_target_loop(&mut context, kind, None);
        let report = run_pliron_progress_check_v1(&context, &function);
        assert!(report.is_clean(), "{kind:?}: {:?}", report.findings());
        assert_eq!(report.certificates().len(), 1);
    }
}

#[test]
fn first_duplicate_target_edge_cannot_reset_induction() {
    reject_reset_edge(0);
}

#[test]
fn second_duplicate_target_edge_cannot_reset_induction() {
    reject_reset_edge(1);
}

fn reject_reset_edge(edge: usize) {
    for kind in [EdgeKind::Split, EdgeKind::LessThan, EdgeKind::Equal] {
        let mut context = setup();
        let function = duplicate_target_loop(&mut context, kind, Some(edge));
        let report = run_pliron_progress_check_v1(&context, &function);
        assert_eq!(
            report.status(),
            KernelCheckStatusV1::Incomplete,
            "{kind:?}: {report:?}"
        );
        assert!(report.certificates().is_empty());
        assert!(matches!(
            report.findings(),
            [PlironProgressFindingV1::ProgressIncomplete { .. }]
        ));
    }
}

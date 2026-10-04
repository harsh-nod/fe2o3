#[derive(Clone, Copy)]
enum NestedLoopCase {
    Canonical,
    ZeroInnerStep,
    NonzeroInnerStart,
    LoopLocalInnerBound,
}

fn nested_loop(context: &mut Context, case: NestedLoopCase) -> FuncOp {
    let (function, _) = make_function(context, "nested_loop", 0);
    let entry = function.get_entry_block(context);
    let (outer_header, outer) = index_block_n(context, &function, "outer_header", 2);
    let (inner_header, inner) = index_block_n(context, &function, "inner_header", 3);
    let (inner_body, body) = index_block_n(context, &function, "inner_body", 3);
    let (outer_latch, latch) = index_block_n(context, &function, "outer_latch", 2);
    let exit = block(context, &function, "exit");
    let zero = IndexConstantOp::new(context, 0);
    let one = IndexConstantOp::new(context, 1);
    let carried = IndexConstantOp::new(context, 9);
    let outer_bound = IndexConstantOp::new(context, 3);
    let inner_bound = IndexConstantOp::new(context, 4);
    for operation in [
        zero.get_operation(),
        one.get_operation(),
        carried.get_operation(),
        outer_bound.get_operation(),
    ] {
        operation.insert_at_back(entry, context);
    }
    if matches!(case, NestedLoopCase::LoopLocalInnerBound) {
        append(context, inner_header, &inner_bound);
    } else {
        append(context, entry, &inner_bound);
    }
    let enter = BranchArgsOp::new(
        context,
        vec![zero.result(context), carried.result(context)],
        outer_header,
    );
    append(context, entry, &enter);
    let inner_start = if matches!(case, NestedLoopCase::NonzeroInnerStart) {
        one.result(context)
    } else {
        zero.result(context)
    };
    let outer_condition = IndexLessThanBranchArgsOp::new(
        context,
        outer[0],
        outer_bound.result(context),
        vec![inner_start, outer[0], outer[1]],
        vec![],
        inner_header,
        exit,
    );
    append(context, outer_header, &outer_condition);
    let inner_condition = IndexLessThanBranchArgsOp::new(
        context,
        inner[0],
        inner_bound.result(context),
        vec![inner[0], inner[1], inner[2]],
        vec![inner[1], inner[2]],
        inner_body,
        outer_latch,
    );
    append(context, inner_header, &inner_condition);
    let inner_step = if matches!(case, NestedLoopCase::ZeroInnerStep) {
        zero.result(context)
    } else {
        one.result(context)
    };
    let inner_next = IndexBinaryOp::new(context, IndexBinaryKindAttr::Add, body[0], inner_step);
    let inner_repeat = BranchArgsOp::new(
        context,
        vec![inner_next.result(context), body[1], body[2]],
        inner_header,
    );
    append(context, inner_body, &inner_next);
    append(context, inner_body, &inner_repeat);
    let outer_next = IndexBinaryOp::new(
        context,
        IndexBinaryKindAttr::Add,
        latch[0],
        one.result(context),
    );
    let outer_repeat = BranchArgsOp::new(
        context,
        vec![outer_next.result(context), latch[1]],
        outer_header,
    );
    append(context, outer_latch, &outer_next);
    append(context, outer_latch, &outer_repeat);
    let ret = ReturnOp::new(context);
    append(context, exit, &ret);
    verify_operation(function.get_operation(), context).unwrap();
    function
}

fn constant_loop(context: &mut Context, start: u64, bound: u64, step: u64) -> FuncOp {
    let (function, _) = make_function(context, "constant_loop", 0);
    let entry = function.get_entry_block(context);
    let (header, induction) = index_block(context, &function, "header");
    let (body, body_induction) = index_block(context, &function, "body");
    let exit = block(context, &function, "exit");
    let start = IndexConstantOp::new(context, start);
    let bound = IndexConstantOp::new(context, bound);
    let step = IndexConstantOp::new(context, step);
    let enter = BranchArgsOp::new(context, vec![start.result(context)], header);
    let condition = IndexLessThanBranchArgsOp::new(
        context,
        induction,
        bound.result(context),
        vec![induction],
        vec![],
        body,
        exit,
    );
    let next = IndexBinaryOp::new(
        context,
        IndexBinaryKindAttr::Add,
        body_induction,
        step.result(context),
    );
    let repeat = BranchArgsOp::new(context, vec![next.result(context)], header);
    let ret = ReturnOp::new(context);
    for operation in [
        start.get_operation(),
        bound.get_operation(),
        step.get_operation(),
        enter.get_operation(),
    ] {
        operation.insert_at_back(entry, context);
    }
    append(context, header, &condition);
    append(context, body, &next);
    append(context, body, &repeat);
    append(context, exit, &ret);
    verify_operation(function.get_operation(), context).unwrap();
    function
}

#[derive(Clone, Copy)]
enum MultiBlockCase {
    Canonical,
    MutatedForwarding,
    GuardedForwarding,
    GuardedMutatedForwarding,
    GuardedInternalFork,
    GuardedResetFork,
    ExternalIntermediateEntry,
    MultipleHeaderEntries,
    DistinctHeaderSeeds,
    InvocationLatchUpdate,
}

#[derive(Clone, Copy)]
enum RangeCase {
    None,
    Bound(&'static [u64]),
    Mismatched(u64),
    NonEntry(u64),
}

fn multi_block_loop(
    context: &mut Context,
    static_bound: Option<u64>,
    step_value: u64,
    case: MultiBlockCase,
    range_case: RangeCase,
) -> FuncOp {
    let (function, arguments) = make_function(
        context,
        "multi_block_loop",
        usize::from(static_bound.is_none()),
    );
    let entry = function.get_entry_block(context);
    let (header, induction) = index_block(context, &function, "header");
    let (first, first_induction) = index_block(context, &function, "first");
    let (second, second_induction) = index_block(context, &function, "second");
    let (latch, latch_induction) = index_block(context, &function, "latch");
    let exit = block(context, &function, "exit");
    let start = IndexConstantOp::new(context, 0);
    let zero = IndexConstantOp::new(context, 0);
    let one = IndexConstantOp::new(context, 1);
    let step = IndexConstantOp::new(context, step_value);
    let bound_constant = static_bound.map(|bound| IndexConstantOp::new(context, bound));
    let mut bound = bound_constant.as_ref().map_or_else(
        || arguments.first().copied().expect("symbolic bound"),
        |bound| bound.result(context),
    );
    let invocation = matches!(case, MultiBlockCase::InvocationLatchUpdate)
        .then(|| InvocationIndexOp::new(context, 0, 8));

    for operation in [
        start.get_operation(),
        zero.get_operation(),
        one.get_operation(),
        step.get_operation(),
    ] {
        operation.insert_at_back(entry, context);
    }
    if let Some(bound) = &bound_constant {
        append(context, entry, bound);
    }
    if let Some(invocation) = &invocation {
        append(context, entry, invocation);
    }
    match range_case {
        RangeCase::None | RangeCase::NonEntry(_) => {}
        RangeCase::Bound(widths) => {
            for width in widths {
                let cast = IndexUnsignedCastOp::new(context, bound, *width);
                append(context, entry, &cast);
                bound = cast.result(context);
            }
        }
        RangeCase::Mismatched(width) => {
            let cast = IndexUnsignedCastOp::new(context, start.result(context), width);
            append(context, entry, &cast);
        }
    }
    if matches!(
        case,
        MultiBlockCase::MultipleHeaderEntries | MultiBlockCase::DistinctHeaderSeeds
    ) {
        let first_entry = block(context, &function, "first_entry");
        let second_entry = block(context, &function, "second_entry");
        let split = IndexLessThanBranchArgsOp::new(
            context,
            zero.result(context),
            one.result(context),
            vec![],
            vec![],
            first_entry,
            second_entry,
        );
        let first_enter = BranchArgsOp::new(context, vec![start.result(context)], header);
        let second_seed = if matches!(case, MultiBlockCase::DistinctHeaderSeeds) {
            // Equal literals do not substitute for the same dominating SSA value.
            zero.result(context)
        } else {
            start.result(context)
        };
        let second_enter = BranchArgsOp::new(context, vec![second_seed], header);
        append(context, entry, &split);
        append(context, first_entry, &first_enter);
        append(context, second_entry, &second_enter);
    } else if matches!(case, MultiBlockCase::ExternalIntermediateEntry) {
        let enter = IndexLessThanBranchArgsOp::new(
            context,
            zero.result(context),
            one.result(context),
            vec![start.result(context)],
            vec![start.result(context)],
            header,
            second,
        );
        append(context, entry, &enter);
    } else {
        let enter = BranchArgsOp::new(context, vec![start.result(context)], header);
        append(context, entry, &enter);
    }

    let condition = IndexLessThanBranchArgsOp::new(
        context,
        induction,
        bound,
        vec![induction],
        vec![],
        first,
        exit,
    );
    append(context, header, &condition);

    if matches!(case, MultiBlockCase::MutatedForwarding) {
        let changed = IndexBinaryOp::new(
            context,
            IndexBinaryKindAttr::Add,
            first_induction,
            one.result(context),
        );
        let forward = BranchArgsOp::new(context, vec![changed.result(context)], second);
        append(context, first, &changed);
        append(context, first, &forward);
    } else if matches!(
        case,
        MultiBlockCase::GuardedForwarding
            | MultiBlockCase::GuardedMutatedForwarding
            | MultiBlockCase::GuardedInternalFork
            | MultiBlockCase::GuardedResetFork
    ) {
        let payload = if matches!(case, MultiBlockCase::GuardedMutatedForwarding) {
            let changed = IndexBinaryOp::new(
                context,
                IndexBinaryKindAttr::Add,
                first_induction,
                one.result(context),
            );
            append(context, first, &changed);
            changed.result(context)
        } else {
            first_induction
        };
        let internal_fork = matches!(
            case,
            MultiBlockCase::GuardedInternalFork | MultiBlockCase::GuardedResetFork
        );
        let reset_fork = matches!(case, MultiBlockCase::GuardedResetFork);
        let guard = IndexLessThanBranchArgsOp::new(
            context,
            if reset_fork {
                first_induction
            } else {
                zero.result(context)
            },
            one.result(context),
            vec![payload],
            internal_fork
                .then_some(if reset_fork {
                    zero.result(context)
                } else {
                    first_induction
                })
                .into_iter()
                .collect(),
            second,
            if internal_fork { latch } else { exit },
        );
        append(context, first, &guard);
    } else {
        if let RangeCase::NonEntry(width) = range_case {
            let cast = IndexUnsignedCastOp::new(context, bound, width);
            append(context, first, &cast);
        }
        let forward = BranchArgsOp::new(context, vec![first_induction], second);
        append(context, first, &forward);
    }
    let forward = BranchArgsOp::new(context, vec![second_induction], latch);
    append(context, second, &forward);

    let latch_base = invocation
        .as_ref()
        .map_or(latch_induction, |invocation| invocation.result(context));
    let next = IndexBinaryOp::new(
        context,
        IndexBinaryKindAttr::Add,
        latch_base,
        step.result(context),
    );
    let repeat = BranchArgsOp::new(context, vec![next.result(context)], header);
    let ret = ReturnOp::new(context);
    append(context, latch, &next);
    append(context, latch, &repeat);
    append(context, exit, &ret);
    verify_operation(function.get_operation(), context).unwrap();
    function
}

#[derive(Clone, Copy)]
enum BranchyLoopCase {
    Canonical,
    MutatedArm,
    BypassUpdate,
}

fn branchy_loop(context: &mut Context, case: BranchyLoopCase) -> FuncOp {
    let (function, _) = make_function(context, "branchy_loop", 0);
    let entry = function.get_entry_block(context);
    let (header, induction) = index_block(context, &function, "header");
    let (split, split_induction) = index_block(context, &function, "split");
    let (left, left_induction) = index_block(context, &function, "left");
    let (right, right_induction) = index_block(context, &function, "right");
    let (merge, merge_induction) = index_block(context, &function, "merge");
    let (latch, latch_induction) = index_block(context, &function, "latch");
    let exit = block(context, &function, "exit");
    let zero = IndexConstantOp::new(context, 0);
    let one = IndexConstantOp::new(context, 1);
    let bound = IndexConstantOp::new(context, 8);
    let enter = BranchArgsOp::new(context, vec![zero.result(context)], header);
    let condition = IndexLessThanBranchArgsOp::new(
        context,
        induction,
        bound.result(context),
        vec![induction],
        vec![],
        split,
        exit,
    );
    let fork = IndexLessThanBranchArgsOp::new(
        context,
        zero.result(context),
        one.result(context),
        vec![split_induction],
        vec![split_induction],
        left,
        right,
    );
    let left_target = if matches!(case, BranchyLoopCase::BypassUpdate) {
        split
    } else {
        merge
    };
    let left_forward = BranchArgsOp::new(context, vec![left_induction], left_target);
    let changed = matches!(case, BranchyLoopCase::MutatedArm).then(|| {
        IndexBinaryOp::new(
            context,
            IndexBinaryKindAttr::Add,
            right_induction,
            one.result(context),
        )
    });
    let right_value = changed
        .as_ref()
        .map_or(right_induction, |operation| operation.result(context));
    let right_forward = BranchArgsOp::new(context, vec![right_value], merge);
    let to_latch = BranchArgsOp::new(context, vec![merge_induction], latch);
    let next = IndexBinaryOp::new(
        context,
        IndexBinaryKindAttr::Add,
        latch_induction,
        one.result(context),
    );
    let repeat = BranchArgsOp::new(context, vec![next.result(context)], header);
    let ret = ReturnOp::new(context);

    for operation in [
        zero.get_operation(),
        one.get_operation(),
        bound.get_operation(),
        enter.get_operation(),
    ] {
        operation.insert_at_back(entry, context);
    }
    append(context, header, &condition);
    append(context, split, &fork);
    append(context, left, &left_forward);
    if let Some(changed) = &changed {
        append(context, right, changed);
    }
    append(context, right, &right_forward);
    append(context, merge, &to_latch);
    append(context, latch, &next);
    append(context, latch, &repeat);
    append(context, exit, &ret);
    verify_operation(function.get_operation(), context).unwrap();
    function
}

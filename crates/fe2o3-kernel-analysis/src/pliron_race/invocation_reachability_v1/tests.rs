use super::*;
use crate::{
    KernelCheckStatusV1, RankedRaceFindingV1, analyze_pliron_sparse_indices_v1,
    run_pliron_ranked_race_check_v1,
};
use dialect_kernel::{
    AccessKindAttr, BranchArgsOp, BranchOp, DIALECT_NAME, IndexBinaryKindAttr, IndexBinaryOp,
    IndexConstantOp, IndexType, IndexUnknownOp, InvocationIndexOp, MemorySpaceAttr, RankedAccessOp,
    RankedViewOp, RankedViewType, register_dialect,
};
use pliron::{
    basic_block::BasicBlock,
    builtin::{op_interfaces::OneRegionInterface, types::FunctionType},
    context::Ptr,
    dialect::DialectName,
    op::Op,
    r#type::TypeHandle,
};

fn setup() -> Context {
    let mut context = Context::new();
    register_dialect(&mut context, &DialectName::try_new(DIALECT_NAME).unwrap()).unwrap();
    dialect_gpu::register_dialect(&mut context).unwrap();
    context
}

macro_rules! append {
    ($context:expr, $block:expr, $operation:expr $(,)?) => {{
        let operation = $operation;
        operation.get_operation().insert_at_back($block, $context);
    }};
}

fn block(context: &mut Context, function: &FuncOp, name: &str, argument: bool) -> Ptr<BasicBlock> {
    let index: TypeHandle = IndexType::get(context).into();
    let block = BasicBlock::new(
        context,
        Some(name.try_into().unwrap()),
        if argument { vec![index] } else { vec![] },
    );
    block.insert_at_back(function.get_region(context), context);
    block
}

#[derive(Clone, Copy)]
enum Guard {
    Less(u64),
    Reversed,
    Removed,
    WrongEdge,
    SameSuccessor,
    Unknown,
    Bypass,
    Rejoin,
    Equal,
    EqualArgs,
    AgreeingPhi,
    DisagreeingPhi,
    UnknownPhi,
    StableLoop,
    ChangingLoop,
}

fn compact(context: &mut Context, name: &str, guard: Guard, launch: u64, effects: usize) -> FuncOp {
    let function = FuncOp::new(
        context,
        name.try_into().unwrap(),
        FunctionType::get(context, vec![], vec![]),
    );
    let entry = function.get_entry_block(context);
    let branch = block(context, &function, "guard", false);
    let active = block(context, &function, "active", false);
    let inactive = block(context, &function, "inactive", false);
    let exit = block(context, &function, "exit", false);
    let memory = block(context, &function, "memory", false);
    let ty = RankedViewType::new(context, 32, true, vec![launch + 128]).unwrap();
    let view = RankedViewOp::new_in_space(context, ty, vec![], MemorySpaceAttr::Global).unwrap();
    let invocation = InvocationIndexOp::new(context, 0, launch);
    let width = IndexConstantOp::new(context, 64);
    let stride = IndexConstantOp::new(context, 16);
    let limit = IndexConstantOp::new(
        context,
        match guard {
            Guard::Less(n) => n,
            _ => 16,
        },
    );
    let zero = IndexConstantOp::new(context, 0);
    let extent = IndexConstantOp::new(context, launch + 128);
    let unknown = IndexUnknownOp::new(context);
    let batch = IndexBinaryOp::new(
        context,
        IndexBinaryKindAttr::Divide,
        invocation.result(context),
        width.result(context),
    );
    let element = IndexBinaryOp::new(
        context,
        IndexBinaryKindAttr::Remainder,
        invocation.result(context),
        width.result(context),
    );
    let base = IndexBinaryOp::new(
        context,
        IndexBinaryKindAttr::Multiply,
        batch.result(context),
        stride.result(context),
    );
    let index = IndexBinaryOp::new(
        context,
        IndexBinaryKindAttr::Add,
        base.result(context),
        element.result(context),
    );
    for operation in [
        view.get_operation(),
        invocation.get_operation(),
        width.get_operation(),
        stride.get_operation(),
        limit.get_operation(),
        zero.get_operation(),
        extent.get_operation(),
        unknown.get_operation(),
        batch.get_operation(),
        element.get_operation(),
        base.get_operation(),
        index.get_operation(),
    ] {
        operation.insert_at_back(entry, context);
    }
    let start = if matches!(guard, Guard::Bypass) {
        IndexLessThanBranchOp::new(
            context,
            unknown.result(context),
            limit.result(context),
            branch,
            active,
        )
        .get_operation()
    } else {
        BranchOp::new(context, branch).get_operation()
    };
    start.insert_at_back(entry, context);
    let mut active_return = exit;
    match guard {
        Guard::Removed => append!(context, branch, &BranchOp::new(context, inactive)),
        Guard::Reversed => {
            let fifteen = IndexConstantOp::new(context, 15);
            append!(context, branch, &fifteen);
            append!(
                context,
                branch,
                &IndexLessThanBranchOp::new(
                    context,
                    fifteen.result(context),
                    element.result(context),
                    inactive,
                    active,
                ),
            );
        }
        Guard::WrongEdge => append!(
            context,
            branch,
            &IndexLessThanBranchOp::new(
                context,
                element.result(context),
                limit.result(context),
                inactive,
                active,
            ),
        ),
        Guard::SameSuccessor => append!(
            context,
            branch,
            &IndexLessThanBranchOp::new(
                context,
                element.result(context),
                limit.result(context),
                inactive,
                inactive
            )
        ),
        Guard::Unknown => append!(
            context,
            branch,
            &IndexLessThanBranchOp::new(
                context,
                unknown.result(context),
                limit.result(context),
                active,
                inactive,
            ),
        ),
        Guard::Equal => append!(
            context,
            branch,
            &IndexEqualBranchOp::new(
                context,
                element.result(context),
                zero.result(context),
                active,
                inactive,
            ),
        ),
        Guard::EqualArgs => append!(
            context,
            branch,
            &IndexEqualBranchArgsOp::new(
                context,
                element.result(context),
                zero.result(context),
                vec![],
                vec![],
                active,
                inactive,
            ),
        ),
        Guard::AgreeingPhi | Guard::DisagreeingPhi | Guard::UnknownPhi => {
            let left = block(context, &function, "left", false);
            let right = block(context, &function, "right", false);
            let join = block(context, &function, "join", true);
            append!(
                context,
                branch,
                &IndexLessThanBranchOp::new(
                    context,
                    unknown.result(context),
                    limit.result(context),
                    left,
                    right,
                ),
            );
            append!(
                context,
                left,
                &BranchArgsOp::new(context, vec![element.result(context)], join),
            );
            let second = match guard {
                Guard::AgreeingPhi => element.result(context),
                Guard::DisagreeingPhi => zero.result(context),
                _ => unknown.result(context),
            };
            append!(
                context,
                right,
                &BranchArgsOp::new(context, vec![second], join),
            );
            let value = join.deref(context).get_argument(0);
            append!(
                context,
                join,
                &IndexLessThanBranchArgsOp::new(
                    context,
                    value,
                    limit.result(context),
                    vec![],
                    vec![],
                    active,
                    inactive,
                ),
            );
        }
        Guard::StableLoop | Guard::ChangingLoop => {
            let header = block(context, &function, "header", true);
            let latch = block(context, &function, "latch", false);
            append!(
                context,
                branch,
                &BranchArgsOp::new(context, vec![element.result(context)], header),
            );
            let value = header.deref(context).get_argument(0);
            append!(
                context,
                header,
                &IndexLessThanBranchArgsOp::new(
                    context,
                    value,
                    limit.result(context),
                    vec![],
                    vec![],
                    active,
                    inactive,
                ),
            );
            let incoming = if matches!(guard, Guard::StableLoop) {
                value
            } else {
                zero.result(context)
            };
            append!(
                context,
                latch,
                &BranchArgsOp::new(context, vec![incoming], header),
            );
            active_return = latch;
        }
        _ => append!(
            context,
            branch,
            &IndexLessThanBranchOp::new(
                context,
                element.result(context),
                limit.result(context),
                active,
                inactive,
            ),
        ),
    }
    for _ in 0..effects {
        let access_index = if matches!(guard, Guard::Less(0)) {
            zero.result(context)
        } else {
            index.result(context)
        };
        let write = RankedAccessOp::new(
            context,
            AccessKindAttr::Write,
            view.result(context),
            vec![access_index],
        )
        .unwrap();
        append!(context, memory, &write);
    }
    append!(
        context,
        active,
        &IndexLessThanBranchOp::new(
            context,
            index.result(context),
            extent.result(context),
            memory,
            exit
        )
    );
    append!(context, memory, &BranchOp::new(context, active_return));
    append!(
        context,
        inactive,
        &BranchOp::new(
            context,
            if matches!(guard, Guard::Rejoin | Guard::Removed | Guard::SameSuccessor) {
                active
            } else {
                exit
            },
        ),
    );
    append!(context, exit, &ReturnOp::new(context));
    function
}

fn check(context: &Context, function: &FuncOp, clean: bool) {
    let bounds = crate::run_pliron_ranked_bounds_check_v1(context, function);
    assert!(
        bounds.is_clean(),
        "fixture must reach the race checker: {bounds:?}"
    );
    let report = run_pliron_ranked_race_check_v1(context, function);
    if clean {
        assert!(report.is_clean(), "{report:?}");
    } else {
        assert_eq!(report.status(), KernelCheckStatusV1::Rejected, "{report:?}");
        assert!(
            report
                .findings()
                .iter()
                .any(|f| matches!(f, RankedRaceFindingV1::ConflictingEffects { .. })),
            "{report:?}"
        );
    }
}

#[test]
fn race_reachability_modulo_compaction_and_reversed_predicate_are_clean() {
    for (index, guard) in [
        Guard::Less(16),
        Guard::Reversed,
        Guard::Equal,
        Guard::EqualArgs,
    ]
    .into_iter()
    .enumerate()
    {
        let context = &mut setup();
        let function = compact(context, &format!("clean{index}"), guard, 128, 1);
        check(context, &function, true);
    }
}

#[test]
fn race_reachability_removed_or_sixty_three_guard_keeps_actual_collision() {
    for (index, guard) in [Guard::Removed, Guard::Less(63)].into_iter().enumerate() {
        let context = &mut setup();
        let function = compact(context, &format!("collision{index}"), guard, 128, 1);
        let report = run_pliron_ranked_race_check_v1(context, &function);
        assert!(
            report.findings().iter().any(|finding| matches!(
                finding,
                RankedRaceFindingV1::ConflictingEffects { indices, first, second, .. }
                    if indices == &[16] && first.invocation() == [16] && second.invocation() == [64]
            )),
            "{report:?}"
        );
    }
}

#[test]
fn race_reachability_unknown_bypass_rejoin_and_wrong_edge_do_not_hide_writes() {
    for (index, guard) in [
        Guard::Unknown,
        Guard::Bypass,
        Guard::Rejoin,
        Guard::WrongEdge,
        Guard::SameSuccessor,
    ]
    .into_iter()
    .enumerate()
    {
        let context = &mut setup();
        let function = compact(context, &format!("uncertain{index}"), guard, 128, 1);
        check(context, &function, false);
    }
}

#[test]
fn race_reachability_only_agreeing_phi_facts_prune() {
    for (index, guard) in [Guard::AgreeingPhi, Guard::DisagreeingPhi, Guard::UnknownPhi]
        .into_iter()
        .enumerate()
    {
        let context = &mut setup();
        let function = compact(context, &format!("phi{index}"), guard, 128, 1);
        check(context, &function, index == 0);
    }
}

#[test]
fn race_reachability_loop_uses_stable_facts_not_first_iteration() {
    for (index, guard) in [Guard::StableLoop, Guard::ChangingLoop]
        .into_iter()
        .enumerate()
    {
        let context = &mut setup();
        let function = compact(context, &format!("loop{index}"), guard, 128, 1);
        check(context, &function, index == 0);
    }
}

#[test]
fn race_reachability_partial_and_exhausted_walks_return_all_effect_blocks() {
    let context = &mut setup();
    let function = compact(context, "budget", Guard::Less(16), 128, 1);
    let sparse = analyze_pliron_sparse_indices_v1(context, &function).unwrap();
    let inventory = BoundedPlironFunctionInventoryV1::collect(context, &function).unwrap();
    let mut work = 0;
    let mut reach = Reachability::new(context, &function, &inventory, &sparse, &mut work).unwrap();
    reach.prepare(&[16], &mut work);
    assert!(reach.complete);
    assert!(!reach.may_reach(2));
    let before = work;
    reach.prepare(&[64], &mut work);
    assert!(reach.complete);
    assert!(reach.may_reach(2));
    assert!(work > before);
    for remaining in [0, 1, 3, 5] {
        work = MAX_BOUNDS_WITNESS_EVALUATION_STEPS_V1 - remaining;
        reach.prepare(&[16], &mut work);
        assert!(!reach.complete);
        assert!((0..inventory.blocks().len()).all(|block| reach.may_reach(block)));
        assert!(work > MAX_BOUNDS_WITNESS_EVALUATION_STEPS_V1);
        reach.prepare(&[16], &mut work);
        assert!(!reach.complete);
        assert!(reach.may_reach(2));
    }
    work = MAX_BOUNDS_WITNESS_EVALUATION_STEPS_V1;
    assert!(Reachability::new(context, &function, &inventory, &sparse, &mut work).is_none());
}

#[test]
fn race_reachability_subjects_and_invocations_do_not_share_pruning() {
    let context = &mut setup();
    let safe = compact(context, "first_root", Guard::Less(16), 128, 1);
    let racing = compact(context, "second_root", Guard::Removed, 128, 1);
    let inventory = BoundedPlironFunctionInventoryV1::collect(context, &safe).unwrap();
    let sparse = analyze_pliron_sparse_indices_v1(context, &safe).unwrap();
    assert!(Reachability::new(context, &racing, &inventory, &sparse, &mut 0).is_none());
    check(context, &safe, true);
    check(context, &racing, false);
    check(context, &safe, true);
}

#[test]
fn race_reachability_inactive_effects_still_consume_original_instance_budget() {
    let context = &mut setup();
    let function = compact(context, "instance_cap", Guard::Less(0), 65_536, 17);
    let report = run_pliron_ranked_race_check_v1(context, &function);
    assert!(
        matches!(report.findings(), [RankedRaceFindingV1::EffectInstanceLimitExceeded { actual, limit }]
        if *actual == 1_048_577 && *limit == 1_048_576),
        "{report:?}"
    );
}

#[test]
fn race_reachability_overflowing_guard_is_unknown_not_wrapped() {
    let context = &mut setup();
    let function = FuncOp::new(
        context,
        "overflowing_guard".try_into().unwrap(),
        FunctionType::get(context, vec![], vec![]),
    );
    let entry = function.get_entry_block(context);
    let yes = block(context, &function, "yes", false);
    let no = block(context, &function, "no", false);
    let maximum = IndexConstantOp::new(context, u64::MAX);
    let one = IndexConstantOp::new(context, 1);
    let overflow = IndexBinaryOp::new(
        context,
        IndexBinaryKindAttr::Add,
        maximum.result(context),
        one.result(context),
    );
    append!(context, entry, &maximum);
    append!(context, entry, &one);
    append!(context, entry, &overflow);
    append!(
        context,
        entry,
        &IndexEqualBranchOp::new(
            context,
            overflow.result(context),
            one.result(context),
            yes,
            no
        )
    );
    append!(context, yes, &ReturnOp::new(context));
    append!(context, no, &ReturnOp::new(context));
    let inventory = BoundedPlironFunctionInventoryV1::collect(context, &function).unwrap();
    let sparse = analyze_pliron_sparse_indices_v1(context, &function).unwrap();
    let mut work = 0;
    let mut reach = Reachability::new(context, &function, &inventory, &sparse, &mut work).unwrap();
    reach.prepare(&[0], &mut work);
    assert!(reach.complete);
    assert!(reach.may_reach(1));
    assert!(reach.may_reach(2));
}

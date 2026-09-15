use super::*;
use fe2o3_kernel_analysis::{RankedBoundsFindingV1, run_pliron_ranked_bounds_check_v1};

#[derive(Clone, Copy)]
enum Mutation {
    None,
    LaneModulus,
    BlockStride,
    ForeignExtent,
    MissingGuard,
}

fn view_for_extent(
    context: &mut Context,
    entry: Ptr<BasicBlock>,
    extent: u64,
    allocation: u64,
) -> RankedViewOp {
    if extent != 0 {
        return view_with_contract(
            context,
            vec![extent],
            MemorySpaceAttr::Global,
            allocation,
            allocation,
        );
    }
    // Zero in the type's shape is a dynamic marker, not a static empty extent.
    let zero = IndexConstantOp::new(context, 0);
    append(context, entry, &zero);
    let view_type = RankedViewType::new(context, 32, true, vec![dialect_kernel::DYNAMIC_EXTENT])
        .expect("ranked dynamic view type");
    let view = RankedViewOp::new_in_space_with_allocation_contract(
        context,
        view_type,
        vec![zero.result(context)],
        MemorySpaceAttr::Global,
        allocation,
        allocation,
    )
    .expect("empty allocation binds its actual zero extent");
    assert_eq!(view.dynamic_extent(context, 0), Some(zero.result(context)));
    view
}

fn relation(
    context: &mut Context,
    launch: u64,
    extents: [u64; 2],
    components: &[u64],
    mutation: Mutation,
) -> FuncOp {
    let function = function(context, "exact_blocked_relation");
    let entry = function.get_entry_block(context);
    let invocation = InvocationIndexOp::new(context, 0, launch);
    let first = view_for_extent(context, entry, extents[0], 11);
    let second = view_for_extent(context, entry, extents[1], 12);
    append(context, entry, &invocation);
    append(context, entry, &first);
    append(context, entry, &second);
    let mut current = entry;
    for (ordinal, component) in components.iter().copied().enumerate() {
        for (output, memory) in [first.result(context), second.result(context)]
            .into_iter()
            .enumerate()
        {
            let lanes = IndexConstantOp::new(context, 16);
            let modulus = IndexConstantOp::new(
                context,
                if matches!(mutation, Mutation::LaneModulus) {
                    8
                } else {
                    16
                },
            );
            let stride = IndexConstantOp::new(
                context,
                if matches!(mutation, Mutation::BlockStride) {
                    63
                } else {
                    64
                },
            );
            let offset = IndexConstantOp::new(context, component * 16);
            let quotient = IndexBinaryOp::new(
                context,
                IndexBinaryKindAttr::Divide,
                invocation.result(context),
                lanes.result(context),
            );
            let lane = IndexBinaryOp::new(
                context,
                IndexBinaryKindAttr::Remainder,
                invocation.result(context),
                modulus.result(context),
            );
            let base = IndexBinaryOp::new(
                context,
                IndexBinaryKindAttr::Multiply,
                quotient.result(context),
                stride.result(context),
            );
            let lane_base = IndexBinaryOp::new(
                context,
                IndexBinaryKindAttr::Add,
                base.result(context),
                lane.result(context),
            );
            let index = IndexBinaryOp::new(
                context,
                IndexBinaryKindAttr::Add,
                lane_base.result(context),
                offset.result(context),
            );
            let extent = IndexConstantOp::new(
                context,
                if matches!(mutation, Mutation::ForeignExtent) {
                    extents[0]
                } else {
                    extents[output]
                },
            );
            for op in [
                lanes.get_operation(),
                modulus.get_operation(),
                stride.get_operation(),
                offset.get_operation(),
                quotient.get_operation(),
                lane.get_operation(),
                base.get_operation(),
                lane_base.get_operation(),
                index.get_operation(),
                extent.get_operation(),
            ] {
                op.insert_at_back(current, context);
            }
            let continuation = block(context, &function, &format!("next_{ordinal}_{output}"));
            if matches!(mutation, Mutation::MissingGuard) {
                let write = access(
                    context,
                    AccessKindAttr::Write,
                    memory,
                    index.result(context),
                );
                append(context, current, &write);
                let next = BranchOp::new(context, continuation);
                append(context, current, &next);
            } else {
                let store = block(context, &function, &format!("store_{ordinal}_{output}"));
                let guard = IndexLessThanBranchOp::new(
                    context,
                    index.result(context),
                    extent.result(context),
                    store,
                    continuation,
                );
                append(context, current, &guard);
                let write = access(
                    context,
                    AccessKindAttr::Write,
                    memory,
                    index.result(context),
                );
                append(context, store, &write);
                let next = BranchOp::new(context, continuation);
                append(context, store, &next);
            }
            current = continuation;
        }
    }
    let end = ReturnOp::new(context);
    append(context, current, &end);
    function
}

#[test]
fn exact_quotient_remainder_blocked_stores_pass_bounds_and_race_checks() {
    for extents in [[4096, 4096], [4096, 37], [0, 1]] {
        let context = &mut setup();
        let function = relation(context, 1024, extents, &[0, 1, 2, 3], Mutation::None);
        let bounds = run_pliron_ranked_bounds_check_v1(context, &function);
        assert!(bounds.is_clean(), "{:#?}", bounds.findings());
        let race = run_pliron_ranked_race_check_v1(context, &function);
        assert!(race.is_clean(), "{:#?}", race.findings());
    }
}

#[test]
fn exact_quotient_remainder_requires_the_access_allocation_extent() {
    let context = &mut setup();
    let function = relation(
        context,
        1024,
        [4096, 37],
        &[0, 1, 2, 3],
        Mutation::ForeignExtent,
    );
    let bounds = run_pliron_ranked_bounds_check_v1(context, &function);
    assert!(
        bounds
            .findings()
            .iter()
            .any(|finding| matches!(finding, RankedBoundsFindingV1::UnprovedBound { .. }))
    );
}

#[test]
fn exact_quotient_remainder_requires_dominating_bounds_guard() {
    let context = &mut setup();
    let function = relation(
        context,
        1024,
        [4096, 37],
        &[0, 1, 2, 3],
        Mutation::MissingGuard,
    );
    let bounds = run_pliron_ranked_bounds_check_v1(context, &function);
    assert!(
        bounds
            .findings()
            .iter()
            .any(|finding| matches!(finding, RankedBoundsFindingV1::UnprovedBound { .. }))
    );
}

#[test]
fn exact_quotient_remainder_detects_changed_lane_and_block_coefficients() {
    for mutation in [Mutation::LaneModulus, Mutation::BlockStride] {
        let context = &mut setup();
        let function = relation(context, 1024, [4096, 4096], &[0, 1, 2, 3], mutation);
        let race = run_pliron_ranked_race_check_v1(context, &function);
        assert!(
            race.findings()
                .iter()
                .any(|finding| matches!(finding, RankedRaceFindingV1::ConflictingEffects { .. })),
            "{:#?}",
            race.findings()
        );
    }
}

#[test]
fn exact_quotient_remainder_exposes_out_of_range_component_collision() {
    let context = &mut setup();
    let function = relation(context, 1024, [4096, 4096], &[0, 4], Mutation::None);
    let race = run_pliron_ranked_race_check_v1(context, &function);
    assert!(
        race.findings()
            .iter()
            .any(|finding| matches!(finding, RankedRaceFindingV1::ConflictingEffects { .. })),
        "{:#?}",
        race.findings()
    );
}

#[test]
fn exact_quotient_remainder_preserves_existing_trace_domain_limit() {
    let context = &mut setup();
    let function = relation(context, 65_537, [262_148, 262_148], &[0], Mutation::None);
    let race = run_pliron_ranked_race_check_v1(context, &function);
    assert!(matches!(
        race.findings(),
        [RankedRaceFindingV1::LaunchDomainTooLarge {
            invocations: 65_537,
            limit: 65_536
        }]
    ));
}

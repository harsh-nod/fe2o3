use super::*;
use dialect_kernel::{
    AccessKindAttr, BranchOp, DIALECT_NAME, DeterministicJoinOp, IndexBinaryOp, IndexConstantOp,
    IndexEqualBranchOp, IndexLessThanBranchOp, InvocationIndexOp, MemorySpaceAttr, RankedAccessOp,
    RankedViewOp, RankedViewType, ReturnOp,
};
use fe2o3_kernel_analysis::{RankedRaceFindingV1, RankedRaceReportV1};
use pliron::{
    basic_block::BasicBlock,
    builtin::{op_interfaces::OneRegionInterface, ops::FuncOp, types::FunctionType},
    context::Context,
    dialect::DialectName,
    op::Op,
};

fn race_consumer(bound: u64, remove: bool, wrong_edge: bool, opaque: bool) -> RankedRaceReportV1 {
    let fixture = Fixture::new(SemanticBinaryOpV1::GreaterOrEqual, bound, false);
    let (predicates, operations) = fixture.project(0).unwrap();
    let [(lhs, rhs)] = predicates[5].as_ref().unwrap().comparisons.as_slice() else {
        panic!()
    };
    let context = &mut Context::new();
    dialect_kernel::register_dialect(context, &DialectName::try_new(DIALECT_NAME).unwrap())
        .unwrap();
    dialect_gpu::register_dialect(context).unwrap();
    let function = FuncOp::new(
        context,
        "comparison_consumer".try_into().unwrap(),
        FunctionType::get(context, vec![], vec![]),
    );
    let entry = function.get_entry_block(context);
    let write = BasicBlock::new(context, Some("write".try_into().unwrap()), vec![]);
    let bounds = BasicBlock::new(context, Some("bounds".try_into().unwrap()), vec![]);
    let exit = BasicBlock::new(context, Some("exit".try_into().unwrap()), vec![]);
    write.insert_at_back(function.get_region(context), context);
    bounds.insert_at_back(function.get_region(context), context);
    exit.insert_at_back(function.get_region(context), context);
    let mut values = HashMap::new();
    for operation in operations {
        let (result, value, operation) = match operation {
            ProductionRankedOperationV1::InvocationIndex {
                result,
                dimension,
                launch_extent,
            } => {
                let op = InvocationIndexOp::new(context, dimension, launch_extent);
                (result, op.result(context), op.get_operation())
            }
            ProductionRankedOperationV1::IndexConstant { result, value } => {
                let op = IndexConstantOp::new(context, value);
                (result, op.result(context), op.get_operation())
            }
            ProductionRankedOperationV1::IndexBinary {
                result,
                kind,
                lhs,
                rhs,
            } => {
                let op = IndexBinaryOp::new(context, kind, values[&lhs], values[&rhs]);
                (result, op.result(context), op.get_operation())
            }
            _ => panic!("consumer requires exact projected index operations"),
        };
        operation.insert_at_back(entry, context);
        values.insert(ProductionRankedValueV1::Local(result), value);
    }
    let width = IndexConstantOp::new(context, 64);
    let stride = IndexConstantOp::new(context, 16);
    let extent = IndexConstantOp::new(context, 2048);
    let invocation = values[&ProductionRankedValueV1::Local(ProductionRankedValueIdV1::new(0))];
    let batch = IndexBinaryOp::new(
        context,
        IndexBinaryKindAttr::Divide,
        invocation,
        width.result(context),
    );
    let element = IndexBinaryOp::new(
        context,
        IndexBinaryKindAttr::Remainder,
        invocation,
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
    let view_type = RankedViewType::new(context, 32, true, vec![2048]).unwrap();
    let view =
        RankedViewOp::new_in_space(context, view_type, vec![], MemorySpaceAttr::Global).unwrap();
    for operation in [
        width.get_operation(),
        stride.get_operation(),
        extent.get_operation(),
        batch.get_operation(),
        element.get_operation(),
        base.get_operation(),
        index.get_operation(),
        view.get_operation(),
    ] {
        operation.insert_at_back(entry, context);
    }
    let terminal = if opaque {
        let zero = IndexConstantOp::new(context, 0);
        let limit = IndexConstantOp::new(context, bound);
        let mask = IndexConstantOp::new(context, 63);
        let join = DeterministicJoinOp::new(
            context,
            vec![invocation, limit.result(context), mask.result(context)],
        );
        for operation in [
            zero.get_operation(),
            limit.get_operation(),
            mask.get_operation(),
            join.get_operation(),
        ] {
            operation.insert_at_back(entry, context);
        }
        IndexEqualBranchOp::new(
            context,
            join.result(context),
            zero.result(context),
            bounds,
            exit,
        )
        .get_operation()
    } else if remove {
        BranchOp::new(context, bounds).get_operation()
    } else {
        let (yes, no) = if wrong_edge {
            (bounds, exit)
        } else {
            (exit, bounds)
        };
        IndexLessThanBranchOp::new(context, values[lhs], values[rhs], yes, no).get_operation()
    };
    terminal.insert_at_back(entry, context);
    IndexLessThanBranchOp::new(
        context,
        index.result(context),
        extent.result(context),
        write,
        exit,
    )
    .get_operation()
    .insert_at_back(bounds, context);
    RankedAccessOp::new(
        context,
        AccessKindAttr::Write,
        view.result(context),
        vec![index.result(context)],
    )
    .unwrap()
    .get_operation()
    .insert_at_back(write, context);
    BranchOp::new(context, exit)
        .get_operation()
        .insert_at_back(write, context);
    ReturnOp::new(context)
        .get_operation()
        .insert_at_back(exit, context);
    let bounds = fe2o3_kernel_analysis::run_pliron_ranked_bounds_check_v1(context, &function);
    assert!(bounds.is_clean(), "{bounds:?}");
    fe2o3_kernel_analysis::run_pliron_ranked_race_check_v1(context, &function)
}

#[test]
fn unsigned_comparison_actual_race_consumer_accepts_exact_guard() {
    let report = race_consumer(16, false, false, false);
    assert!(report.is_clean(), "{report:?}");
}

#[test]
fn unsigned_comparison_actual_race_consumer_retains_wrong_threshold_and_removed_guard_witness() {
    for (bound, remove) in [(63, false), (16, true)] {
        let report = race_consumer(bound, remove, false, false);
        assert!(
            report.findings().iter().any(|finding| matches!(finding,
                RankedRaceFindingV1::ConflictingEffects { indices, first, second, .. }
                    if indices == &[16] && first.invocation() == [16] && second.invocation() == [64]
            )),
            "{report:?}"
        );
    }
}

#[test]
fn unsigned_comparison_actual_race_consumer_does_not_repair_wrong_edge() {
    let report = race_consumer(16, false, true, false);
    assert!(
        report
            .findings()
            .iter()
            .any(|finding| matches!(finding, RankedRaceFindingV1::ConflictingEffects { .. })),
        "{report:?}"
    );
}

#[test]
fn unsigned_comparison_source37_dependency_join_still_keeps_actual_collision() {
    let report = race_consumer(16, false, false, true);
    assert!(
        report.findings().iter().any(|finding| matches!(finding,
            RankedRaceFindingV1::ConflictingEffects { indices, first, second, .. }
                if indices == &[16] && first.invocation() == [16] && second.invocation() == [64]
        )),
        "{report:?}"
    );
}

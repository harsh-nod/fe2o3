use super::*;
use crate::production_analysis::pliron_function_inventory::BoundedPlironFunctionInventoryV1;
use dialect_kernel::{BranchOp, ReturnOp};
use pliron::{
    basic_block::BasicBlock,
    builtin::{op_interfaces::OneRegionInterface, types::FunctionType},
    dialect::DialectName,
    op::Op,
};

fn fixture(
    barrier: bool,
    targets: &[Option<usize>],
) -> (
    Context,
    FuncOp,
    BoundedPlironFunctionInventoryV1,
    ProductionAnalysisInputCensusV1,
) {
    let mut context = Context::new();
    dialect_kernel::register_dialect(
        &mut context,
        &DialectName::try_new(dialect_kernel::DIALECT_NAME).unwrap(),
    )
    .unwrap();
    dialect_gpu::register_dialect(&mut context).unwrap();
    let ty = FunctionType::get(&context, vec![], vec![]);
    let function = FuncOp::new(&mut context, "dependency_probe".try_into().unwrap(), ty);
    let mut blocks = vec![function.get_entry_block(&context)];
    for ordinal in 1..targets.len() {
        // Deliberately reverse numeric-looking labels; only roster order matters.
        let name = format!("row_{}", 1000 - ordinal * 17);
        let block = BasicBlock::new(&mut context, Some(name.try_into().unwrap()), vec![]);
        block.insert_at_back(function.get_region(&context), &context);
        blocks.push(block);
    }
    if barrier {
        BarrierOp::new(
            &mut context,
            HierarchyAttr::Workgroup,
            MemoryScopeAttr::Workgroup,
            AddressSpaceAttr::Workgroup,
            MemoryOrderAttr::AcquireRelease,
        )
        .get_operation()
        .insert_at_back(blocks[0], &context);
    }
    for (row, target) in targets.iter().enumerate() {
        let terminator = match target {
            Some(target) => BranchOp::new(&mut context, blocks[*target]).get_operation(),
            None => ReturnOp::new(&mut context).get_operation(),
        };
        terminator.insert_at_back(blocks[row], &context);
    }
    let inventory = BoundedPlironFunctionInventoryV1::collect(&context, &function).unwrap();
    let census = ProductionAnalysisInputCensusV1 {
        blocks: targets.len(),
        operations: targets.len() + usize::from(barrier),
        successors: targets.iter().filter(|target| target.is_some()).count(),
        ..ProductionAnalysisInputCensusV1::default()
    };
    (context, function, inventory, census)
}

#[test]
fn barrier_dependency_probe_skips_exact_trace_no_barrier_and_forward_cfg() {
    for (has_barrier, trace, targets, expected) in [
        (false, false, vec![Some(0)], false),
        (true, true, vec![Some(0)], false),
        (true, false, vec![Some(1), None], false),
        (true, false, vec![Some(2), Some(3), Some(3), None], false),
        (true, false, vec![Some(1), Some(2), Some(1)], true),
        (true, false, vec![Some(0)], true),
    ] {
        let (context, _, inventory, census) = fixture(has_barrier, &targets);
        assert_eq!(
            barrier_progress_may_be_needed_v1(&context, &inventory, trace, &census),
            Ok(expected)
        );
    }
}

#[test]
fn barrier_dependency_reservation_is_not_a_progress_proof_and_can_go_unused() {
    let (context, _, inventory, census) = fixture(true, &[Some(2), Some(3), Some(1), None]);
    assert_eq!(
        barrier_progress_may_be_needed_v1(&context, &inventory, false, &census),
        Ok(true)
    );
    // This backward-row edge is acyclic. Even an admitted reservation must not
    // force an unnecessary progress execution or invent a progress certificate.
    let summary = barrier_paths::summarize_all_barrier_paths(&context, &inventory, || {
        panic!("acyclic SCCs must not consume the progress callback")
    })
    .unwrap();
    assert!(matches!(summary, BarrierPathSummaryV1::Unique));

    let (context, _, inventory, census) = fixture(true, &[Some(0)]);
    assert_eq!(
        barrier_progress_may_be_needed_v1(&context, &inventory, false, &census),
        Ok(true)
    );
    let summary = barrier_paths::summarize_all_barrier_paths(&context, &inventory, || {
        panic!("a cyclic collective must reject before progress")
    })
    .unwrap();
    assert!(matches!(summary, BarrierPathSummaryV1::Incomplete(_)));
}

#[test]
fn barrier_dependency_probe_has_exact_cumulative_and_denied_before_access_boundaries() {
    let (context, function, _, census) = fixture(false, &[None]);
    let bound = barrier_progress_probe_bound_v1(&census).unwrap();
    assert_eq!(bound.work_upper_bound(), 114);
    assert_eq!(bound.retained_storage_upper_bound(), 1);
    assert_eq!(bound.peak_storage_upper_bound(), 65);
    let zero = ProductionAnalysisResourceUpperBoundV1::checked_phase(
        ProductionAnalysisResourcePhaseV1::StructuralIdentity,
        0,
        0,
        0,
    )
    .unwrap();
    for (work, peak, succeeds) in [(117, 68, true), (116, 68, false), (117, 67, false)] {
        let mut analyses = PlironAnalysisManagerV1::new_with_resource_contract(
            &function,
            census,
            zero,
            0,
            ProductionAnalysisResourceLimitsV1::new(work, peak),
        )
        .unwrap();
        // The constructor charges the 3/3 inventory envelope. An unprepared
        // inventory would panic if either denied probe reached its body.
        if succeeds {
            analyses.prepare_function_inventory(&context, &function);
        }
        let result = admit_barrier_progress_probe_v1(&context, &mut analyses, &census);
        if succeeds {
            assert_eq!(result, Ok(false));
            assert_eq!(analyses.resource_upper_bound().work_upper_bound(), 117);
            assert_eq!(
                analyses
                    .resource_upper_bound()
                    .retained_storage_upper_bound(),
                4
            );
            assert_eq!(
                analyses.resource_upper_bound().peak_storage_upper_bound(),
                68
            );
        } else {
            assert!(result.is_err());
            assert_eq!(analyses.resource_upper_bound().work_upper_bound(), 3);
            assert_eq!(
                analyses
                    .resource_upper_bound()
                    .retained_storage_upper_bound(),
                3
            );
        }
    }
}

#[test]
fn barrier_dependency_probe_bounds_raw_edges_and_never_authorizes_foreign_targets() {
    let (mut context, function, inventory, mut census) = fixture(true, &[Some(1), None]);
    census.successors = 0;
    assert!(barrier_progress_may_be_needed_v1(&context, &inventory, false, &census).is_err());
    census.successors = 1;
    census.operations -= 1;
    assert!(barrier_progress_may_be_needed_v1(&context, &inventory, false, &census).is_err());
    census.operations += 1;
    let ty = FunctionType::get(&context, vec![], vec![]);
    let foreign = FuncOp::new(&mut context, "foreign".try_into().unwrap(), ty);
    let source = function
        .get_entry_block(&context)
        .deref(&context)
        .get_terminator(&context)
        .unwrap();
    Operation::replace_successor(source, &context, 0, foreign.get_entry_block(&context));
    assert_eq!(
        barrier_progress_may_be_needed_v1(&context, &inventory, false, &census),
        Ok(true)
    );
    let summary = barrier_paths::summarize_all_barrier_paths(&context, &inventory, || {
        panic!("foreign successor must fail before progress")
    })
    .unwrap();
    assert!(matches!(summary, BarrierPathSummaryV1::Incomplete(_)));
}

#[test]
fn barrier_dependency_composition_sums_work_but_not_disjoint_child_peaks() {
    let phase = ProductionAnalysisResourcePhaseV1::BarrierConvergence;
    let make = |work, retained, temporary| {
        ProductionAnalysisResourceUpperBoundV1::checked_phase(phase, work, retained, temporary)
            .unwrap()
    };
    let local = make(31, 3, 7);
    let pipeline = make(11, 5, 8);
    let progress = make(17, 7, 12);
    let combined = compose_barrier_dependencies_v1(local, pipeline, Some(progress)).unwrap();
    assert_eq!(combined.work_upper_bound(), 59);
    assert_eq!(combined.retained_storage_upper_bound(), 3);
    assert_eq!(combined.peak_storage_upper_bound(), 29);
    assert_eq!(
        ProductionAnalysisResourceLimitsV1::new(59, 29).require(phase, combined),
        Ok(combined)
    );
    assert!(
        ProductionAnalysisResourceLimitsV1::new(58, 29)
            .require(phase, combined)
            .is_err()
    );
    assert!(
        ProductionAnalysisResourceLimitsV1::new(59, 28)
            .require(phase, combined)
            .is_err()
    );
    let without = compose_barrier_dependencies_v1(local, pipeline, None).unwrap();
    assert_eq!(without.work_upper_bound(), 42);
    assert_eq!(without.peak_storage_upper_bound(), 23);
    assert!(
        barrier_progress_probe_bound_v1(&ProductionAnalysisInputCensusV1 {
            successors: usize::MAX,
            ..ProductionAnalysisInputCensusV1::default()
        })
        .is_err()
    );
}

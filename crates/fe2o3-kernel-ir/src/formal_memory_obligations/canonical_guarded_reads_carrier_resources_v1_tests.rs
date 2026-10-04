use super::*;
use std::ops::Range;

fn minimal_carrier_graph() -> VerifiedCanonicalKernelIrModuleV12 {
    let mut entry = BasicBlock::new(BlockId(10));
    entry.operations.push(op(
        0,
        Type::BOOL,
        OperationKind::Constant(Constant::Bool(true)),
    ));
    entry.terminator = Some(Terminator::Branch {
        target: BlockId(15),
        arguments: vec![ValueId(0)],
    });
    let mut guard = BasicBlock::new(BlockId(15));
    guard.parameters.push(ValueDef::new(ValueId(1), Type::BOOL));
    guard.terminator = Some(Terminator::ConditionalBranch {
        condition: ValueId(1),
        then_target: BlockId(20),
        then_arguments: vec![],
        else_target: BlockId(30),
        else_arguments: vec![],
    });
    let mut selected = BasicBlock::new(BlockId(20));
    selected.operations.push(op(
        2,
        Type::BOOL,
        OperationKind::Select {
            condition: ValueId(1),
            true_value: ValueId(0),
            false_value: ValueId(0),
        },
    ));
    selected.terminator = Some(Terminator::Return { values: vec![] });
    let mut exit = BasicBlock::new(BlockId(30));
    exit.terminator = Some(Terminator::Return { values: vec![] });
    let mut module = Module::new("nonempty-boolean-carrier-resource-cut");
    module.functions.push(Function::kernel_entry(
        "entry",
        Signature::new(vec![], vec![]),
        vec![],
        vec![entry, guard, selected, exit],
    ));
    module.kernels.push(Kernel::new(
        "kernel",
        "entry",
        LaunchDomain::D1 {
            x: LaunchExtent::Dynamic,
        },
    ));
    owner(module)
}

// Independently counted from the four-block/two-operation/one-carrier graph:
// selection costs 32 + 4 * 2 + 2 * 8, including both operation profiles.
// local width-one lookups cost 5, 4 and 3 for 4, 2 and 1 indexed rows.
// Each of the three reachable incoming edges also queries its source's
// dominator interval: ceil_log2(4) + 4 lookup work, then four comparisons.
const CONTROL_WORK: usize = (32 + 4 * 2 + 2 * 8) + 2 + 4 * 20 + 3 * 14 + 3 * (2 + 4 + 4) + 32;
const DEFINITION_WORK: usize = 4 * 7 + 2 * 2 + 4 + 8;
const ORIGIN_ADAPTER_BEFORE_SCC: usize = 4 * 7 + 4 + 6 + 8 + 5 + 3 + 3 + 3;
// Twenty reserves, twelve one-row fills, one input and its external seed,
// one DFS/pop, one reverse visit, one condensation node and one output row.
const SCC_WORK: usize = 20 * 2 + 12 + 1 + 4 + (2 + 3) + (2 + 5) + (2 + 3) + 6 + 3 + 4 + 2;
const ORIGIN_ADAPTER_AFTER_SCC: usize = 2 + 3;
const ROOT_WORK: usize = 2 + 2 + 4 * 37 + 4 + (3 + 4 + 4 + 1) + 5;
const EXPANSION_WORK: usize = 4 + 4 + 3 + 4 + 4 + 12 + 4 + 1 + 3 + 8 + 2 + 2;
const RUNTIME_MATCHER_WORK: usize = (2 + 4 + 4 * 6 + 2 * 3 + 2 * 4 + 2) + (2 + 25 + 4);
const CENSUS_WORK: usize = 4 + 2 * 2;
const SCC_END: usize = CONTROL_WORK + DEFINITION_WORK + ORIGIN_ADAPTER_BEFORE_SCC + SCC_WORK;
const ROOT_HELPER_END: usize = SCC_END + ORIGIN_ADAPTER_AFTER_SCC + 2 + 2 + 2 * 37 + 4 + 12;
const FUNCTION_WORK: usize = SCC_END
    + ORIGIN_ADAPTER_AFTER_SCC
    + ROOT_WORK
    + EXPANSION_WORK
    + RUNTIME_MATCHER_WORK
    + CENSUS_WORK;

#[test]
fn actual_nonempty_scc_and_carrier_lookup_have_source_derived_local_work_cuts() {
    assert_eq!(
        (SCC_WORK, SCC_END, ROOT_HELPER_END, FUNCTION_WORK),
        (89, 435, 534, 749)
    );
    let graph = minimal_carrier_graph();
    // Exact interior cuts admit their last event, then reject the next phase.
    // The complete per-function exact cut admits the public callback.
    for (limit, attempted) in [
        (SCC_END - 1, Some(SCC_END)),
        (SCC_END, Some(SCC_END + 2)),
        (ROOT_HELPER_END - 1, Some(ROOT_HELPER_END)),
        (ROOT_HELPER_END, Some(ROOT_HELPER_END + 5)),
        (FUNCTION_WORK - 1, Some(FUNCTION_WORK)),
        (FUNCTION_WORK, None),
    ] {
        let mut work = CanonicalKernelIrWorkBudgetV1::new(1_000_000);
        let mut budget = Budget::new(&mut work, 1 << 20);
        budget.reserve_storage(17).unwrap();
        let mut callbacks = 0;
        let result = with_canonical_guarded_global_reads_v1(
            &graph,
            CanonicalGuardedGlobalReadLimitsV1 {
                per_function_work: limit,
                ..Default::default()
            },
            &mut budget,
            |view, budget| {
                callbacks += 1;
                let fact = view.true_at(coordinate(2, 0), ValueId(0), budget)?.unwrap();
                assert!(std::ptr::eq(fact.owner(), &graph));
                assert_eq!(fact.value(), ValueId(0));
                assert_eq!(fact.edge(), (BlockId(15), 0, BlockId(20)));
                assert_eq!(
                    view.function_effects(FunctionCoordinate(0), budget)?,
                    (0, 0, 0)
                );
                Ok(())
            },
        );
        if let Some(attempted) = attempted {
            let Err(Failure::Resource(ResourceError::Work(error))) = result else {
                panic!("source-derived interior work cut must refuse");
            };
            assert_eq!((error.actual(), error.limit()), (attempted, limit));
            assert_eq!(callbacks, 0);
        } else {
            result.unwrap();
            assert_eq!(callbacks, 1);
        }
        assert_eq!(budget.storage(), 17);
        assert_eq!(work.failed_work(), None);
    }
}

fn bytes_before_first_carrier_result_slot() -> usize {
    // Fixed logical frames and cumulative capacity requests, not global peak/RSS.
    let frames = size_of::<GuardedControlCollectionV1<LiveGuardMeter<'_, '_>>>()
        + size_of::<GuardedAnalysisV1<'_, LiveGuardMeter<'_, '_>>>()
        + (5 + 3 + 20) * size_of::<Vec<()>>();
    let control_and_definitions = 4 * size_of::<ControlRow>() + 2 * size_of::<DefinitionRow<'_>>();
    let origin_adapter = 2 * size_of::<ValueId>()
        + 2 * size_of::<origins::Input>()
        + 2 * size_of::<(ValueId, &Type)>();
    // The SCC has no dependency edges: one unused edge slot, a two-tuple DFS
    // stack, two origin summaries, three Boolean rows, four ranges, four usize
    // rows and two Option<ValueId> rows. Zero-capacity vectors reserve no bytes.
    let scc = 3 * size_of::<(usize, usize)>()
        + 2 * size_of::<OriginSummary>()
        + 3 * size_of::<bool>()
        + 4 * size_of::<Range<usize>>()
        + 4 * size_of::<usize>()
        + 2 * size_of::<Option<ValueId>>();
    let retained_origins_and_roots =
        size_of::<runtime_slice_read_v1::Origin<'_>>() + 4 * size_of::<TrueRow>();
    frames + control_and_definitions + origin_adapter + scc + retained_origins_and_roots
}

#[test]
fn actual_nonempty_graph_prepays_carrier_result_slot_before_lookup() {
    let graph = minimal_carrier_graph();
    let exact = bytes_before_first_carrier_result_slot() + size_of::<Option<usize>>();
    for (limit, attempted) in [
        (exact - 1, exact),
        (exact, exact + 4 * size_of::<Vec<()>>()),
    ] {
        let mut work = CanonicalKernelIrWorkBudgetV1::new(1_000_000);
        let mut budget = Budget::new(&mut work, 1 << 20);
        budget.reserve_storage(17).unwrap();
        let mut callbacks = 0;
        let result = with_canonical_guarded_global_reads_v1(
            &graph,
            CanonicalGuardedGlobalReadLimitsV1 {
                per_function_work: FUNCTION_WORK,
                per_function_new_bytes: limit,
                ..Default::default()
            },
            &mut budget,
            |_, _| {
                callbacks += 1;
                Ok(())
            },
        );
        assert_eq!(
            result,
            Err(Failure::Resource(ResourceError::Storage {
                actual: attempted,
                limit,
            }))
        );
        assert_eq!(callbacks, 0);
        assert_eq!(budget.storage(), 17);
        assert_eq!(budget.failed_storage(), None);
    }
}

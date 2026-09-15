// Included inside connected_v12_tests: use the real owned bridge, its retained
// capture, and the existing owner/budget helpers. No production fault hook.

#[test]
fn mapped_finish_result_failure_after_map_construction_restores_floor_and_retries_cleanly() {
    use crate::kir_optimization_map_v12::{LiveKeyV12, LiveRosterV12};
    use crate::{KirOptimizationEndpointV12 as Endpoint, KirOptimizationMapErrorV12 as MapError};
    use fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceErrorV1 as ResourceError;
    use std::mem::size_of;

    let ty = Type::Scalar(ScalarType::U32);
    let mut entry = KirBlock::new(BlockId(42));
    entry.operations.push(KirOperation::effect_free(
        ValueDef::new(ValueId(2), ty.clone()),
        OperationKind::Select {
            condition: ValueId(0),
            true_value: ValueId(1),
            false_value: ValueId(1),
        },
    ));
    entry.terminator = Some(Terminator::Return {
        values: vec![ValueId(2)],
    });
    let mut source = Module::new("map-result-error");
    source.functions.push(Function::internal_helper(
        "f",
        Signature::new(vec![Type::BOOL, ty.clone()], vec![ty]),
        vec![ValueId(0), ValueId(1)],
        vec![entry],
    ));
    let input = owner(&source);
    let mut setup_work = CanonicalKernelIrWorkBudgetV1::new(AMPLE_WORK);
    let mut setup =
        CanonicalKernelIrVerificationResourceBudgetV1::new(&mut setup_work, AMPLE_STORAGE);
    setup.reserve_storage(STORAGE_FLOOR).unwrap();
    let (mut graph, imported) = KirPlironGraphV12::import(&input, &mut setup).unwrap();
    setup.reserve_storage(imported.retained_storage()).unwrap();
    let (report, executed) = graph
        .execute_production_optimization_v12(&mut setup)
        .unwrap();
    setup.reserve_storage(executed.retained_storage()).unwrap();
    assert!(
        report.passes()[2].changed(),
        "the real select canonicalizer must run"
    );
    let (output, bridge, baseline_map, extracted) = graph
        .extract_optimized_canonical_kir_module_with_map_v12(&mut setup)
        .unwrap();
    setup.reserve_storage(extracted.retained_storage()).unwrap();

    // Capacity/storage stay frozen. Actual trace: two arguments, Select/result,
    // Return => N=5; replacing the Select result then erasing result/producer
    // yields E=3, S=2; endpoint counts I=5,O=3, F=2,B=2, log=3,Tcap=10.
    // Check envelope=128*(1+2+2+5+3+3*9+(4*5+5+3+10)*4)=24576.
    // Census=6+6+5=17. Finish runs two checks plus128*L*(N+1).
    let bytes = input.canonical().canonical_bytes().len();
    let nodes = (2 * bytes + 64).min(131_072);
    assert_eq!(baseline_map.neutral_node_count_v1(), 5);
    assert_eq!(baseline_map.neutral_event_count_v1(), 3);
    let census_work = 17;
    let map_work = 50_705;
    let retry_work = 51_473;
    let map_storage = 1536 * nodes + 4096;
    let capture = graph.optimization_capture.as_ref().unwrap().clone();
    let roster = graph.optimization_roster_v12(nodes).unwrap();
    let mut incomplete = roster.clone();
    let missing = incomplete
        .iter()
        .position(|(_, endpoint)| {
            *endpoint
                == Endpoint::Operation(KirBridgeCoordinateV1::Terminator {
                    function: 0,
                    block: 0,
                })
        })
        .unwrap();
    incomplete.remove(missing);
    assert_eq!(incomplete.len() + 1, roster.len());
    assert!(!roster.is_empty());

    // These two caller-owned fault-injection rosters already exist before the
    // seeded phase. Their headers/capacities are explicitly transferred with
    // the graph, report, output and baseline map into its storage floor.
    let roster_storage = 2 * size_of::<LiveRosterV12>()
        + (roster.capacity() + incomplete.capacity()) * size_of::<(LiveKeyV12, Endpoint)>();
    setup.reserve_storage(roster_storage).unwrap();
    let graph_storage = graph.retained_storage();
    let floor = STORAGE_FLOOR
        + graph_storage
        + executed.retained_storage()
        + extracted.retained_storage()
        + roster_storage;
    assert_eq!(setup.storage(), floor);

    for boundary in 0..3 {
        let work_limit = if boundary == 0 {
            WORK_FLOOR + map_work - 1
        } else {
            WORK_FLOOR + map_work + retry_work
        };
        let storage_limit = floor + map_storage - usize::from(boundary == 1);
        let mut work = CanonicalKernelIrWorkBudgetV1::new(work_limit);
        work.charge_work(WORK_FLOOR).unwrap();
        let mut budget =
            CanonicalKernelIrVerificationResourceBudgetV1::new(&mut work, storage_limit);
        budget.reserve_storage(floor).unwrap();
        assert!(budget.reserve_storage(usize::MAX).is_err());
        assert!(budget.charge_work(usize::MAX).is_err());
        let result = capture.finish(&input, &output, &incomplete, &mut budget);
        match boundary {
            0 => assert!(matches!(result,
                Err(MapError::Resources(ResourceError::Work(error)))
                if error.actual() == WORK_FLOOR + map_work
                    && error.limit() == WORK_FLOOR + map_work - 1)),
            1 => assert!(matches!(result,
                Err(MapError::Resources(ResourceError::Storage(error)))
                if error.actual() == floor + map_storage
                    && error.limit() == floor + map_storage - 1)),
            _ => {
                // finish constructs terminal/relations/owned map vectors and its
                // digest before check_inner rejects this incomplete output
                // coverage. This is a Result cleanup path after map allocation,
                // unlike either precharge rejection above.
                assert!(matches!(result, Err(MapError::Coverage)));
            }
        }
        assert_eq!(budget.storage(), floor);
        assert_eq!(
            budget.work(),
            if boundary == 0 {
                WORK_FLOOR + census_work
            } else {
                WORK_FLOOR + map_work
            }
        );
        assert_eq!(
            budget.peak_storage(),
            if boundary == 2 {
                floor + map_storage
            } else {
                floor
            }
        );
        assert_eq!(budget.failed_storage(), Some(usize::MAX));
        assert_eq!(input.module(), &source);

        if boundary == 2 {
            // Retry the same untouched capture/session, without rerunning a
            // pass or clearing either seeded failure history.
            let (fresh, retained) = capture
                .finish(&input, &output, &roster, &mut budget)
                .unwrap();
            assert_eq!(budget.storage(), floor);
            budget.reserve_storage(retained).unwrap();
            assert_eq!(fresh, baseline_map);
            assert!(fresh.matches_execution(&report));
            assert_eq!(budget.work(), WORK_FLOOR + map_work + retry_work);
            assert_eq!(budget.peak_storage(), floor + map_storage);
            assert_eq!(budget.failed_storage(), Some(usize::MAX));
            drop(fresh);
            budget.release_storage(retained).unwrap();
            assert_eq!(budget.storage(), floor);
        }
        assert_eq!(work.failed_work(), Some(usize::MAX));
    }

    // End the explicit transfer between phase ledgers before releasing any
    // still-live payload. The captured Arc must drop before graph custody.
    let mut cleanup_work = CanonicalKernelIrWorkBudgetV1::new(0);
    let mut cleanup = CanonicalKernelIrVerificationResourceBudgetV1::new(&mut cleanup_work, floor);
    cleanup.reserve_storage(floor).unwrap();
    drop((roster, incomplete));
    cleanup.release_storage(roster_storage).unwrap();
    drop(capture);
    drop(graph);
    cleanup.release_storage(graph_storage).unwrap();
    drop((output, bridge, baseline_map, report));
    cleanup
        .release_storage(extracted.retained_storage() + executed.retained_storage())
        .unwrap();
    assert_eq!(cleanup.storage(), STORAGE_FLOOR);
}

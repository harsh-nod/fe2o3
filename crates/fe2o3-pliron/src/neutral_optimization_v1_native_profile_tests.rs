#[test]
fn native_empty_execution_has_independent_exact_and_one_under_resource_limits() {
    // Native empty-module census: four visits, structural N=1. The occurrence
    // observer prepays W=512, S=11520. B=37 gives native map observer W=128,
    // S=5632 and execution P=272176, T=528992, W=27367936.
    const CENSUS: usize = 4;
    const OCCURRENCE_WORK: usize = 512;
    const EXECUTION_WORK: usize = 27_367_936;
    const PERSISTENT: usize = 272_176;
    const TEMPORARY: usize = 528_992;
    const OCCURRENCE_STORAGE: usize = 11_520;
    const WORK_PREFIX: usize = 7;
    const STORAGE_PREFIX: usize = 47;
    let report = std::mem::size_of::<PlironOptimizationReportV1>()
        + 7 * std::mem::size_of::<crate::PlironOptimizationPassReportV1>();
    let expected_work = WORK_PREFIX + CENSUS + OCCURRENCE_WORK + EXECUTION_WORK;
    let expected_peak_delta = OCCURRENCE_STORAGE + PERSISTENT + TEMPORARY + report;
    for case in 0..3 {
        let input = owner(&Module::new("m"));
        assert_eq!(input.canonical().canonical_bytes().len(), 37);
        let mut setup_work = CanonicalKernelIrWorkBudgetV1::new(WORK);
        let mut setup = Budget::new(&mut setup_work, STORAGE);
        let (mut graph, imported) = KirPlironGraphV12::import(&input, &mut setup).unwrap();
        let floor = STORAGE_PREFIX + input.storage + imported.retained_storage();
        let work_limit = expected_work - usize::from(case == 1);
        let storage_limit = floor + expected_peak_delta - usize::from(case == 2);
        let mut work = CanonicalKernelIrWorkBudgetV1::new(work_limit);
        {
            let mut budget = Budget::new(&mut work, storage_limit);
            budget.charge_work(WORK_PREFIX).unwrap();
            budget.reserve_storage(floor).unwrap();
            {
                let result = graph.execute_production_neutral_optimization_v1(&mut budget);
                match result {
                    Ok(lease) => {
                        assert_eq!(case, 0);
                        // Abandoning the actual seven-pass lease drops its observers
                        // and report, but must keep the graph's persistent growth.
                        drop(lease);
                    }
                    Err(error) => {
                        assert_ne!(case, 0);
                        assert!(matches!(
                            error,
                            KirNeutralOptimizationErrorV1::Execution(
                                PlironOptimizationErrorV12::Resources(_)
                            )
                        ));
                    }
                }
            }
            if case == 0 {
                assert_eq!(budget.work(), expected_work);
                assert_eq!(budget.peak_storage(), floor + expected_peak_delta);
                assert_eq!(budget.storage(), floor + PERSISTENT);
                assert_eq!(
                    graph.retained_storage(),
                    imported.retained_storage() + PERSISTENT
                );
                assert!(graph.optimization_started);
            } else {
                assert_eq!(budget.storage(), floor);
                assert_eq!(graph.retained_storage(), imported.retained_storage());
                assert!(!graph.optimization_started);
                if case == 1 {
                    assert_eq!(budget.work(), WORK_PREFIX + CENSUS + OCCURRENCE_WORK);
                } else {
                    assert_eq!(budget.work(), expected_work);
                    assert_eq!(budget.failed_storage(), Some(floor + expected_peak_delta));
                }
            }
            assert!(graph.validate_custody_v12().is_err());
            let graph_storage = graph.retained_storage();
            drop(graph);
            budget.release_storage(graph_storage).unwrap();
            let input_storage = input.storage;
            drop(input);
            budget.release_storage(input_storage).unwrap();
            assert_eq!(budget.storage(), STORAGE_PREFIX);
        }
        if case == 1 {
            assert_eq!(work.failed_work(), Some(expected_work));
        }
    }
}

#[test]
fn actual_native_pipeline_handles_padded_metadata_without_legacy_observer_precharge() {
    let mut source = duplicate_edges(None);
    source.id = format!("module_{}", "x".repeat(3_000)).into();
    source.functions[0].id = format!("function_{}", "y".repeat(1_000)).into();
    let input = owner(&source);
    assert!(input.canonical().canonical_bytes().len() > 4_096);
    let mut work = CanonicalKernelIrWorkBudgetV1::new(1_000_000_000);
    let mut budget = Budget::new(&mut work, STORAGE);
    budget.reserve_storage(53 + input.storage).unwrap();
    let (mut graph, imported) = KirPlironGraphV12::import(&input, &mut budget).unwrap();
    budget.reserve_storage(imported.retained_storage()).unwrap();
    let observed = graph
        .execute_production_neutral_optimization_v1(&mut budget)
        .unwrap()
        .extract()
        .unwrap();
    budget
        .reserve_storage(observed.storage().retained_storage())
        .unwrap();
    let graph_storage = graph.retained_storage();
    drop(graph);
    budget.release_storage(graph_storage).unwrap();
    let checked = observed.try_check_and_finish_v1(&mut budget).unwrap();
    budget
        .reserve_storage(checked.storage().retained_storage())
        .unwrap();
    assert_eq!(checked.report().passes().len(), 7);
    assert_eq!(
        checked.native_input_audit_bytes(),
        input.canonical().canonical_bytes()
    );
    assert!(!checked.grants_authority());
    let checked_storage = checked.storage().retained_storage();
    drop(checked);
    budget.release_storage(checked_storage).unwrap();
    let input_storage = input.storage;
    drop(input);
    budget.release_storage(input_storage).unwrap();
    assert_eq!(budget.storage(), 53);
}

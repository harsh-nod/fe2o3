fn checked_roster_phase(
    graph: &KirPlironGraphV12<'_>,
    output: &VerifiedCanonicalKernelIrModuleV12,
    scratch: usize,
    budget: &mut CanonicalKernelIrVerificationResourceBudgetV1<'_>,
) -> Result<usize, crate::KirOptimizationMapErrorV12> {
    let floor = budget.storage();
    let result = (|| {
        let limit = optimization_roster_endpoint_count_v12(output.module(), budget)?;
        budget.reserve_storage(scratch)?;
        let roster = graph.optimization_roster_metered_v1::<true, _>(limit, &mut |units| {
            budget.charge_work(units).map_err(Into::into)
        })?;
        let count = roster.len();
        assert_eq!(count, limit);
        drop(roster);
        Ok(count)
    })();
    budget.release_storage(budget.storage() - floor)?;
    result
}

#[test]
fn v12_actual_roster_census_and_visit_have_independent_exact_limits() {
    use crate::kir_optimization_map_v12::CaptureLimitsV12;
    for (count, padding) in [(0usize, 0usize), (1, 0), (2, 0), (8, 0), (2, 256)] {
        let mut source = interleaved_functions(count as u32);
        source.id = format!("roster-{}", "x".repeat(padding)).into();
        for (index, function) in source.functions.iter_mut().enumerate() {
            function.id = format!("function-{index}-{}", "x".repeat(padding)).into();
        }
        let mut build_work = CanonicalKernelIrWorkBudgetV1::new(AMPLE_WORK);
        let mut build_budget =
            CanonicalKernelIrVerificationResourceBudgetV1::new(&mut build_work, AMPLE_STORAGE);
        let (input, input_storage) =
            VerifiedCanonicalKernelIrModuleV12::from_module_ref_with_verification_budget_v12(
                &source,
                &mut build_budget,
            )
            .unwrap();
        build_budget
            .reserve_storage(input_storage.retained_storage())
            .unwrap();
        let (graph, graph_storage) = KirPlironGraphV12::import(&input, &mut build_budget).unwrap();
        build_budget
            .reserve_storage(graph_storage.retained_storage())
            .unwrap();
        // Each pair has one declaration and one body with a constant and return:
        // F=2c, B=c, O=c, A=0, P=2c, R=c, N=3c.
        let census_work = 1 + 4 * count + 4 * count * count;
        let visitor_work = 4 + 14 * count;
        let expected_work = census_work + visitor_work;
        assert_eq!(source.functions.len(), 2 * count);
        // The storage policy is intentionally unchanged: Ncap=min(2*bytes+64,
        // 131072), Ecap=Tcap=8*Ncap, S=512*Ncap+64*Ecap+64*Tcap+4096.
        let byte_cap = (2 * input.canonical().canonical_bytes().len() + 64).min(131_072);
        let scratch = 1536 * byte_cap + 4096;
        assert_eq!(
            CaptureLimitsV12::for_bytes(input.canonical().canonical_bytes().len())
                .unwrap()
                .storage()
                .unwrap(),
            scratch,
        );
        let floor =
            STORAGE_FLOOR + input_storage.retained_storage() + graph_storage.retained_storage();
        for (work_limit, storage_limit, succeeds) in [
            (WORK_FLOOR + expected_work, floor + scratch, true),
            (WORK_FLOOR + expected_work - 1, floor + scratch, false),
            (WORK_FLOOR + expected_work, floor + scratch - 1, false),
        ] {
            let mut work = CanonicalKernelIrWorkBudgetV1::new(work_limit);
            work.charge_work(WORK_FLOOR).unwrap();
            let mut budget =
                CanonicalKernelIrVerificationResourceBudgetV1::new(&mut work, storage_limit);
            budget.reserve_storage(floor).unwrap();
            let result = checked_roster_phase(&graph, &input, scratch, &mut budget);
            assert_eq!(result.is_ok(), succeeds);
            if succeeds {
                assert_eq!(result.unwrap(), 3 * count);
                assert_eq!(budget.work(), WORK_FLOOR + expected_work);
                assert_eq!(budget.peak_storage(), floor + scratch);
            } else if storage_limit < floor + scratch {
                assert!(matches!(
                    result,
                    Err(crate::KirOptimizationMapErrorV12::Resources(_))
                ));
                assert_eq!(budget.failed_storage(), Some(floor + scratch));
                assert_eq!(budget.work(), WORK_FLOOR + census_work);
            } else {
                assert!(matches!(
                    result,
                    Err(crate::KirOptimizationMapErrorV12::Resources(_))
                ));
                assert_eq!(budget.peak_storage(), floor + scratch);
            }
            assert_eq!(budget.storage(), floor);
        }
        drop(graph);
        build_budget
            .release_storage(graph_storage.retained_storage())
            .unwrap();
        drop(input);
        build_budget
            .release_storage(input_storage.retained_storage())
            .unwrap();
        assert_eq!(build_budget.storage(), 0);
    }
}

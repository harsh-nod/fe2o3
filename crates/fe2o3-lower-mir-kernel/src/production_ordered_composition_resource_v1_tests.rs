#[test]
fn ordered_composition_exact_and_one_short_resources_preserve_prior_floor() {
    fn trial(floor: usize, work_limit: usize, storage_limit: usize) -> (bool, usize, usize) {
        let (mut ssa, launch) = composition_owners(composition_request(CompositionCase::Twice));
        let mut work = CanonicalKernelIrWorkBudgetV1::new(work_limit);
        let mut budget = ArgumentBudgetV1::new(&mut work, storage_limit);
        budget.reserve_storage(floor).unwrap();
        let capture = ssa
            .try_capture_occurrences_with_budget_v1(&mut budget)
            .unwrap();
        budget.reserve_storage(capture.retained_storage()).unwrap();
        let live = budget.storage();
        let result = ProductionOrderedCompositionPreRankedKirOwnerV1::try_materialize_with_budget(
            ssa,
            launch,
            ProductionSemanticKirLimitsV1::default(),
            &mut budget,
        );
        assert_eq!(budget.storage(), live);
        let ok = result.is_ok();
        let accepted = budget.work();
        let peak = budget.peak_storage();
        drop(result);
        budget.release_storage(capture.retained_storage()).unwrap();
        assert_eq!(budget.storage(), floor);
        (ok, accepted, peak)
    }
    for floor in [0, 83] {
        let (ok, work, peak) = trial(floor, 1_000_000_000, 64 * 1024 * 1024);
        assert!(ok);
        assert_eq!(trial(floor, work, peak), (true, work, peak));
        assert!(!trial(floor, work - 1, peak).0);
        assert!(!trial(floor, work, peak - 1).0);
    }
}
#[test]
fn ordered_composition_query_errors_and_unwind_restore_storage_floor() {
    with_composition(CompositionCase::Helper, |owner, budget| {
        let floor = budget.storage();
        let before = budget.work();
        assert!(
            owner
                .with_checked_canonical_calls_v17(budget, |_, _| Err::<(), _>(
                    ProductionSemanticKirErrorV1::CorrespondenceMismatch
                ))
                .is_err()
        );
        assert_eq!(budget.storage(), floor);
        assert!(budget.work() > before);
        let panic = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let _ = owner.with_checked_canonical_calls_v17(
                budget,
                |_, _| -> Result<(), ProductionSemanticKirErrorV1> {
                    panic!("injected scratch consumer panic")
                },
            );
        }));
        assert!(panic.is_err());
        assert_eq!(budget.storage(), floor);
    });
}
#[test]
fn ordered_composition_ninth_static_definition_refuses() {
    let admitted = Fixture::default()
        .request()
        .admit_current_production(SemanticMirLimitsV1::default())
        .unwrap();
    let mut blocks = (0..9)
        .map(|i| {
            block(
                i,
                vec![],
                SemanticTerminatorKindV1::Call(marker(
                    1,
                    function_identity(),
                    32 + i,
                    4,
                    u32::from(i) + 1,
                    1,
                )),
            )
        })
        .collect::<Vec<_>>();
    blocks.push(block(9, vec![], SemanticTerminatorKindV1::Return));
    let request = InertSemanticMirRequestV1::new_with_callables(
        admitted.target(),
        admitted.types().to_vec(),
        vec![],
        vec![],
        vec![],
        vec![rebuild(&admitted.functions()[0], blocks)],
        admitted.callables().to_vec(),
        vec![ROOT],
    )
    .unwrap();
    let (mut ssa, launch) = composition_owners(request);
    let mut work = CanonicalKernelIrWorkBudgetV1::new(1_000_000);
    let mut budget = ArgumentBudgetV1::new(&mut work, 1_000_000);
    let capture = ssa
        .try_capture_occurrences_with_budget_v1(&mut budget)
        .unwrap();
    budget.reserve_storage(capture.retained_storage()).unwrap();
    let floor = budget.storage();
    assert!(
        ProductionOrderedCompositionPreRankedKirOwnerV1::try_materialize_with_budget(
            ssa,
            launch,
            ProductionSemanticKirLimitsV1::default(),
            &mut budget
        )
        .is_err()
    );
    assert_eq!(budget.storage(), floor);
}
#[test]
fn ordered_composition_prefix_does_not_admit_division_or_projection() {
    let t = types();
    assert!(!ordered_composition_assignment_v1(
        &t,
        &assignment(SemanticRvalueKindV1::Binary {
            operation: SemanticBinaryOpV1::Divide,
            left: input(1),
            right: input(2),
        })
    ));
    assert!(!ordered_composition_assignment_v1(
        &t,
        &assignment(SemanticRvalueKindV1::Use(SemanticOperandV1::Copy(
            projected_place()
        )))
    ));
}

#[test]
fn original_raw_classifier_cannot_choose_zero_raw_by_omitting_the_reference_plan() {
    for flow in [
        AddressFlow::Read,
        AddressFlow::ReferenceCast,
        AddressFlow::BeforeInitialization,
    ] {
        let mut completed = false;
        let result = with_address_plan(flow, |plan, budget| {
            let floor = budget.storage();
            for original_plan in [Some(plan), None] {
                assert!(matches!(
                    scoped_raw_admission_v29::require_original_zero_raw_v29(
                        plan.instances,
                        original_plan,
                        budget
                    ),
                    Err(ProductionSemanticKirErrorV1::Unsupported {
                        detail: "original raw source requires consuming expanded physical admission",
                        ..
                    })
                ));
                assert_eq!(
                    budget.storage(),
                    floor,
                    "closed census must drop its own scratch"
                );
            }
            completed = true;
            Ok(())
        });
        assert!(result.is_ok(), "{flow:?}: {result:?}");
        assert!(completed, "all original-source assertions must finish");
    }
}

#[test]
fn original_raw_classifier_does_not_count_execution_carrier_implementation_pointers() {
    let mut owner = lifecycle_owner(false);
    let mut work = CanonicalKernelIrWorkBudgetV1::new(20_000_000);
    let mut budget = ArgumentBudgetV1::new(&mut work, 20_000_000);
    budget.reserve_storage(29).unwrap();
    let capture = owner
        .try_capture_occurrences_with_budget_v1(&mut budget)
        .unwrap();
    budget.reserve_storage(capture.retained_storage()).unwrap();
    let mut completed = false;
    production_call_instances_v1::with_production_call_instances_v1(
        &owner,
        ROOT,
        &mut budget,
        |instances, budget| {
            let floor = budget.storage();
            let result =
                scoped_raw_admission_v29::require_original_zero_raw_v29(instances, None, budget);
            assert_eq!(budget.storage(), floor);
            completed = true;
            Ok::<_, production_call_instances_v1::ProductionCallInstanceErrorV1>(result)
        },
    )
    .unwrap()
    .unwrap();
    assert!(completed);
    drop(owner);
    budget.release_storage(capture.retained_storage()).unwrap();
    assert_eq!(budget.storage(), 29);
}

fn run_original_raw_classifier_budget(
    work_limit: usize,
    storage_limit: usize,
) -> (Result<(), ProductionSemanticKirErrorV1>, usize, usize, bool) {
    let mut owner = lifecycle_owner(false);
    let mut work = CanonicalKernelIrWorkBudgetV1::new(work_limit);
    let mut budget = ArgumentBudgetV1::new(&mut work, storage_limit);
    budget.reserve_storage(29).unwrap();
    let mut completed = false;
    let mut retained_capture = 0;
    let result = (|| {
        let capture = owner
            .try_capture_occurrences_with_budget_v1(&mut budget)
            .map_err(|error| match error {
                fe2o3_pliron::ProductionSemanticSsaOccurrenceErrorV1::Resource(error) => {
                    ProductionSemanticKirErrorV1::from(error)
                }
                other => panic!("unexpected original capture failure: {other:?}"),
            })?;
        budget.reserve_storage(capture.retained_storage())?;
        retained_capture = capture.retained_storage();
        production_call_instances_v1::with_production_call_instances_v1(
            &owner,
            ROOT,
            &mut budget,
            |instances, budget| {
                let result = scoped_raw_admission_v29::require_original_zero_raw_v29(
                    instances, None, budget,
                );
                completed = result.is_ok();
                Ok::<_, production_call_instances_v1::ProductionCallInstanceErrorV1>(result)
            },
        )
        .map_err(|error| match error {
            production_call_instances_v1::ProductionCallInstanceErrorV1::Resource(error) => {
                error.into()
            }
            _ => source_raw_physical_error_v29(),
        })
        .and_then(|result| result)
    })();
    drop(owner);
    budget.release_storage(retained_capture).unwrap();
    assert_eq!(budget.storage(), 29);
    (result, budget.work(), budget.peak_storage(), completed)
}

#[test]
fn original_raw_classifier_exact_and_one_short_limits_are_resource_errors_not_panics() {
    let (result, work, storage, completed) =
        run_original_raw_classifier_budget(20_000_000, 20_000_000);
    result.unwrap();
    assert!(completed);
    let (result, _, _, completed) = run_original_raw_classifier_budget(work, storage);
    result.unwrap();
    assert!(completed);
    let (result, _, _, _) = run_original_raw_classifier_budget(work - 1, storage);
    assert!(matches!(
        result,
        Err(
            ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(ArgumentResourceV1::Work(
                _
            ))
        )
    ));
    let (result, _, _, _) = run_original_raw_classifier_budget(work, storage - 1);
    assert!(matches!(
        result,
        Err(
            ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(
                ArgumentResourceV1::Storage(_)
            )
        )
    ));
}

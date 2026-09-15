use super::*;
use fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceErrorV1 as Resource;

fn void_cfg() -> Module {
    let mut module = Module::new("m");
    module.functions.push(Function::internal_helper(
        "f",
        Signature::new(vec![], vec![]),
        vec![],
        vec![returning(u32::MAX, &[])],
    ));
    module
}

fn assert_exact_census(input: &Inventory<'_>, output: &Inventory<'_>, defined: bool) {
    let count = usize::from(defined);
    for inventory in [input, output] {
        assert_eq!(
            inventory.owner().module().id.as_str(),
            if defined { "m" } else { "e" }
        );
        assert!(inventory.owner().module().required_capabilities.is_empty());
        assert!(inventory.owner().module().kernels.is_empty());
        assert_eq!(inventory.functions().len(), count);
        assert_eq!(inventory.blocks().len(), count);
        assert!(inventory.operations().is_empty());
        assert!(inventory.definitions().is_empty());
        assert!(inventory.uses().is_empty());
        assert!(inventory.edges().is_empty());
        assert!(inventory.edge_arguments().is_empty());
        if defined {
            let function = inventory.functions()[0].function;
            assert_eq!(function.id.as_str(), "f");
            assert!(function.signature.parameters.is_empty());
            assert!(function.signature.results.is_empty());
            assert!(function.required_capabilities.is_empty());
            assert!(
                matches!(inventory.blocks()[0].terminator, Terminator::Return { values } if values.is_empty())
            );
        }
    }
}

fn exact_storage(defined: bool) -> (usize, usize) {
    let word = size_of::<usize>();
    // The private State header consists of two inventory references, the nine
    // borrowed row slices in Candidate, and eighteen Vec headers. Derive this
    // independently, without sampling a successful solver's peak.
    assert_eq!(size_of::<Candidate<'_>>(), 9 * 2 * word);
    assert_eq!(size_of::<Vec<usize>>(), 3 * word);
    assert_eq!(size_of::<Control<'_, '_, '_>>(), 2 * word + 4 * 3 * word);
    let state_header = 2 * word + size_of::<Candidate<'_>>() + 18 * size_of::<Vec<usize>>();
    let retained = size_of::<Control<'_, '_, '_>>()
        + usize::from(defined) * size_of::<CanonicalKirBlockControlV1>();
    // The void CFG has eight usize slots (two function maps, four block maps,
    // incoming head and pending queue) and one reachable byte. All other State
    // vectors are empty. The index retains exactly one block-control row.
    let scratch = state_header + usize::from(defined) * (8 * word + size_of::<u8>());
    (retained + scratch, retained)
}

fn exact_work(defined: bool) -> usize {
    if !defined {
        // Index entry + State entry + module check + two one-byte ID visits
        // + empty capability check + one fixed-point loop = 7.
        1 + 1 + 1 + 2 + 1 + 1
    } else {
        // Index entry/block initialization 2; State entry/nine slots 10;
        // exact module/function metadata 11; block installation 8;
        // final output-block definition visit 1; solver loop/reachability/phi
        // visits 5; control checks 4; retained block-fact copy 1 = 42.
        2 + 10 + 11 + 8 + 1 + 5 + 4 + 1
    }
}

#[test]
fn empty_and_void_cfg_have_independent_exact_and_one_under_index_budgets() {
    for (module, defined) in [(Module::new("e"), false), (void_cfg(), true)] {
        with_transition(&module, |observed, input, output, budget| {
            assert_exact_census(input, output, defined);
            let rows = observed.occurrences().candidate();
            assert_eq!(rows.functions.len(), usize::from(defined));
            assert_eq!(rows.blocks.len(), usize::from(defined));
            assert_eq!(rows.segments.len(), usize::from(defined));
            assert!(rows.operations.is_empty());
            assert!(rows.definitions.is_empty());
            assert!(rows.definition_outputs.is_empty());
            assert!(rows.uses.is_empty());
            assert!(rows.edges.is_empty());
            assert!(rows.edge_arguments.is_empty());
            let checked_storage = {
                let (checked, checked_storage) = check(input, output, rows, budget).unwrap();
                budget
                    .reserve_storage(checked_storage.retained_storage())
                    .unwrap();
                let floor = budget.storage();
                assert!(floor > 0);
                let (peak, retained) = exact_storage(defined);
                let expected_work = exact_work(defined);
                assert_eq!(expected_work, if defined { 42 } else { 7 });
                for (work_under, storage_under) in [(0, 0), (1, 0), (0, 1)] {
                    let mut work =
                        CanonicalKernelIrWorkBudgetV1::new(7 + expected_work - work_under);
                    let mut bounded = Budget::new(&mut work, floor + peak - storage_under);
                    bounded.charge_work(7).unwrap();
                    bounded.reserve_storage(floor).unwrap();
                    match Control::derive(&checked, &mut bounded) {
                        Ok((control, storage)) => {
                            assert_eq!((work_under, storage_under), (0, 0));
                            assert_eq!(bounded.work(), 7 + expected_work);
                            assert_eq!(bounded.peak_storage(), floor + peak);
                            assert_eq!(storage.retained_storage(), retained);
                            assert!(std::ptr::eq(control.input(), input));
                            assert!(std::ptr::eq(control.output(), output));
                            bounded.reserve_storage(retained).unwrap();
                            drop(control);
                            bounded.release_storage(retained).unwrap();
                        }
                        Err(CheckError::Resource(Resource::Work(error))) => {
                            assert_eq!((work_under, storage_under), (1, 0));
                            assert_eq!(error.actual(), 7 + expected_work);
                            assert_eq!(bounded.work(), 7 + expected_work - 1);
                            assert_eq!(bounded.peak_storage(), floor + peak);
                            assert_eq!(bounded.failed_storage(), None);
                        }
                        Err(CheckError::Resource(Resource::Storage(error))) => {
                            assert_eq!((work_under, storage_under), (0, 1));
                            assert_eq!(error.actual(), floor + peak);
                            assert_eq!(bounded.failed_storage(), Some(floor + peak));
                            if defined {
                                // The last State Vec is pending[B]. Its reserve
                                // fails before its initialization charge: the
                                // other eight nonempty vectors are already live.
                                assert_eq!(bounded.work(), 7 + 11);
                                assert_eq!(
                                    bounded.peak_storage(),
                                    floor + peak - size_of::<usize>()
                                );
                            } else {
                                // Empty State has no vector payload; its header
                                // reservation follows the complete index header.
                                assert_eq!(bounded.work(), 7 + 2);
                                assert_eq!(bounded.peak_storage(), floor + retained);
                            }
                        }
                        other => panic!("unexpected exact control boundary: {other:?}"),
                    }
                    assert_eq!(bounded.storage(), floor);
                    assert_eq!(
                        work.failed_work(),
                        if work_under != 0 {
                            Some(7 + expected_work)
                        } else {
                            None
                        }
                    );
                }
                checked_storage
            };
            budget
                .release_storage(checked_storage.retained_storage())
                .unwrap();
        });
    }
}

#[test]
fn solver_phase_exhaustion_drops_fully_allocated_state_before_restoring_floor() {
    with_transition(&void_cfg(), |observed, input, output, budget| {
        assert_exact_census(input, output, true);
        let checked_storage = {
            let (checked, checked_storage) =
                check(input, output, observed.occurrences().candidate(), budget).unwrap();
            budget
                .reserve_storage(checked_storage.retained_storage())
                .unwrap();
            let floor = budget.storage();
            let (peak, _) = exact_storage(true);
            // Build+structure uses 32. The solver's fixed loop costs 1,
            // reachable[B] fill 1, function-entry visit 1 and queue pop 1.
            // The next phi-alias block visit requests unit 37, even though
            // this void fixture has no value/alias premise to fabricate.
            let mut work = CanonicalKernelIrWorkBudgetV1::new(7 + 32 + 1 + 3);
            let mut bounded = Budget::new(&mut work, floor + peak);
            bounded.charge_work(7).unwrap();
            bounded.reserve_storage(floor).unwrap();
            let error = Control::derive(&checked, &mut bounded).unwrap_err();
            let CheckError::Resource(Resource::Work(error)) = error else {
                panic!("expected solver work exhaustion");
            };
            assert_eq!(error.actual(), 7 + 37);
            assert_eq!(bounded.work(), 7 + 36);
            assert_eq!(bounded.peak_storage(), floor + peak);
            assert_eq!(bounded.storage(), floor);
            assert_eq!(bounded.failed_storage(), None);
            assert_eq!(work.failed_work(), Some(7 + 37));
            checked_storage
        };
        budget
            .release_storage(checked_storage.retained_storage())
            .unwrap();
    });
}

use super::*;
#[derive(Debug)]
struct Observation {
    result: Result<()>,
    work: usize,
    peak: usize,
    failed_work: Option<usize>,
    failed_storage: Option<usize>,
}
fn measure(input: &Owner, floor: usize, w: usize, s: usize) -> Observation {
    let mut work = Work::new(w);
    let (result, accepted, peak, failed_storage) = {
        let mut budget = Budget::new(&mut work, s);
        budget.reserve_storage(floor).unwrap();
        let ledger = budget.work_ledger_identity_v1();
        let result = prepare_owned_induction_refinement_v1(input, Limits::default(), &mut budget)
            .map(|owner| {
                assert_eq!(
                    owner.retained_storage(),
                    header().unwrap()
                        + owner.output_storage.retained_storage()
                        + owner.origins.capacity() * size_of::<Row>()
                );
                budget.reserve_storage(owner.retained_storage()).unwrap();
                release(owner, &mut budget);
            });
        assert_eq!(budget.storage(), floor);
        assert!(budget.work_ledger_identity_v1() == ledger);
        (
            result,
            budget.work(),
            budget.peak_storage(),
            budget.failed_storage(),
        )
    };
    Observation {
        result,
        work: accepted,
        peak,
        failed_work: work.failed_work(),
        failed_storage,
    }
}
fn final_output_loop_tail_work(input: &Owner, floor: usize) -> usize {
    let mut work = Work::new(W);
    let mut budget = Budget::new(&mut work, S);
    budget.reserve_storage(floor).unwrap();
    let ledger = budget.work_ledger_identity_v1();
    let owner =
        prepare_owned_induction_refinement_v1(input, Limits::default(), &mut budget).unwrap();
    budget.reserve_storage(owner.retained_storage()).unwrap();
    let (inventory, inventory_size) = Inventory::derive(owner.output(), &mut budget).unwrap();
    budget
        .reserve_storage(inventory_size.retained_storage())
        .unwrap();
    let (loops, loops_size) = Loops::derive(&inventory, Limits::default(), &mut budget).unwrap();
    budget
        .reserve_storage(loops_size.retained_storage())
        .unwrap();
    assert_eq!(loops.loop_count(), 1);
    let header = loops.natural_loop(0, &mut budget).unwrap().header();
    assert_eq!(
        (inventory.functions().len(), inventory.blocks().len()),
        (1, 4)
    );
    assert_eq!((header.function.0, header.block), (0, 1));
    let remaining_headers = inventory.blocks().len() - header.block as usize - 1;
    let start = budget.work();
    loops
        .replay(&inventory, Limits::default(), &mut budget)
        .unwrap();
    let replay = budget.work() - start;
    drop(loops);
    budget
        .release_storage(loops_size.retained_storage())
        .unwrap();
    drop(inventory);
    budget
        .release_storage(inventory_size.retained_storage())
        .unwrap();
    release(owner, &mut budget);
    assert_eq!(budget.storage(), floor);
    assert!(budget.work_ledger_identity_v1() == ledger);
    // After the pair's first NaturalLoop reserve: later headers, replay, two transfers.
    remaining_headers + replay + 2
}
#[test]
fn owned_induction_refinement_exact_work_and_storage_preserve_denial_history() {
    for mutation in [false, true] {
        let (input, bytes) = admit(&fixture(
            ScalarType::U32,
            None,
            if mutation { 1 } else { 2 },
            true,
        ));
        let sibling = [0x93u8; FLOOR];
        let floor = bytes + sibling.len();
        let measured = measure(&input, floor, W, S);
        assert!(measured.result.is_ok());
        let tail_work = final_output_loop_tail_work(&input, floor);
        let exact = measure(&input, floor, measured.work, measured.peak);
        assert!(exact.result.is_ok());
        assert_eq!(
            (
                exact.work,
                exact.peak,
                exact.failed_work,
                exact.failed_storage
            ),
            (measured.work, measured.peak, None, None)
        );
        let short = measure(&input, floor, measured.work - 1, measured.peak);
        let Err(Error::Resource(Resource::Work(limit))) = &short.result else {
            panic!("exact final owner transfer refusal: {short:?}")
        };
        assert_eq!(
            (limit.actual(), limit.limit()),
            (measured.work, measured.work - 1)
        );
        assert_eq!(
            (
                short.work,
                short.peak,
                short.failed_work,
                short.failed_storage
            ),
            (measured.work - 1, measured.peak, Some(measured.work), None)
        );
        let initial = floor + size_of::<Meter<'_, '_>>();
        let short = measure(&input, floor, W, initial - 1);
        let Err(Error::Resource(Resource::Storage(limit))) = &short.result else {
            panic!("exact owner-scope header refusal: {short:?}")
        };
        assert_eq!((limit.actual(), limit.limit()), (initial, initial - 1));
        assert_eq!(
            (
                short.work,
                short.peak,
                short.failed_work,
                short.failed_storage
            ),
            (0, floor, None, Some(initial))
        );
        let short = measure(&input, floor, W, measured.peak - 1);
        let Err(Error::Pair(PairError::Loops(LoopError::Resource(Resource::Storage(limit))))) =
            &short.result
        else {
            panic!("exact final-output NaturalLoop backing refusal: {short:?}")
        };
        assert_eq!(
            (limit.actual(), limit.limit()),
            (measured.peak, measured.peak - 1)
        );
        assert_eq!(
            (
                short.work,
                short.peak,
                short.failed_work,
                short.failed_storage
            ),
            (
                measured.work - tail_work,
                measured.peak - size_of::<fe2o3_kernel_analysis::CanonicalKirNaturalLoopV1>(),
                None,
                Some(measured.peak)
            )
        );
        let mut work = Work::new(W);
        {
            let mut budget = Budget::new(&mut work, S);
            budget.reserve_storage(floor).unwrap();
            budget.charge_work(17).unwrap();
            assert!(matches!(budget.charge_work(W), Err(Resource::Work(_))));
            assert!(matches!(
                budget.reserve_storage(S),
                Err(Resource::Storage(_))
            ));
            let owner =
                prepare_owned_induction_refinement_v1(&input, Limits::default(), &mut budget)
                    .unwrap();
            assert_eq!(
                (
                    budget.work(),
                    budget.peak_storage(),
                    budget.failed_storage()
                ),
                (measured.work + 17, measured.peak, Some(floor + S))
            );
            budget.reserve_storage(owner.retained_storage()).unwrap();
            release(owner, &mut budget);
            assert_eq!(budget.storage(), floor);
        }
        assert_eq!(work.failed_work(), Some(W + 17));
        assert_eq!(sibling, [0x93; FLOOR]);
    }
}
#[test]
fn owned_induction_refinement_output_cap_precedes_candidate_mutation() {
    with_input(fixture(ScalarType::U32, None, 1, true), |input, budget| {
        let before = input.canonical().canonical_bytes().to_vec();
        let floor = budget.storage();
        assert!(matches!(
            prepare_owned_induction_refinement_v1(
                input,
                Limits {
                    operations: 3,
                    ..Limits::default()
                },
                budget
            ),
            Err(Error::OutputLimit {
                actual: 4,
                limit: 3
            })
        ));
        assert_eq!(budget.storage(), floor);
        assert_eq!(input.canonical().canonical_bytes(), before);
        let owner = prepare_owned_induction_refinement_v1(
            input,
            Limits {
                operations: 4,
                ..Limits::default()
            },
            budget,
        )
        .unwrap();
        budget.reserve_storage(owner.retained_storage()).unwrap();
        assert_eq!(selected(&owner), 1);
        verify(&owner, input, budget);
        release(owner, budget);
    });
}
#[test]
fn owned_induction_refinement_actual_capacity_and_old_new_coexistence_are_paid() {
    with_input(fixture(ScalarType::U32, None, 1, true), |input, budget| {
        let owner =
            prepare_owned_induction_refinement_v1(input, Limits::default(), budget).unwrap();
        budget.reserve_storage(owner.retained_storage()).unwrap();
        let floor = budget.storage();
        resources::scoped(budget, |meter| -> Result<()> {
            let (inventory, ir) = meter.derive(|b| Ok(Inventory::derive(input, b)?))?;
            meter.reserve(ir.retained_storage())?;
            let (mut candidate, cr) =
                meter.derive(|b| Ok(input.copy_module_for_transformation_v12(b)?))?;
            meter.reserve(cr.retained_storage())?;
            let old_capacity = candidate.functions[0].body.as_ref().unwrap().blocks[2].operations
                [0]
            .results
            .capacity();
            let before = meter.budget_for_test().storage();
            let extra = build::materialize(&inventory, owner.origins(), &mut candidate, meter)?;
            let block = &candidate.functions[0].body.as_ref().unwrap().blocks[2];
            let actual = size_of::<Vec<Operation>>()
                + block.operations.capacity() * size_of::<Operation>()
                + size_of::<Vec<ValueDef>>()
                + block.operations[1].results.capacity() * size_of::<ValueDef>();
            assert_eq!(extra, actual);
            assert_eq!(block.operations[0].results.capacity(), old_capacity);
            assert_eq!(meter.budget_for_test().storage(), before + actual);
            assert_eq!(&candidate, owner.output().module());
            drop(candidate);
            meter.release(cr.retained_storage() + extra)?;
            drop(inventory);
            meter.release(ir.retained_storage())?;
            Ok(())
        })
        .unwrap();
        assert_eq!(budget.storage(), floor);
        release(owner, budget);
    });
}
#[test]
fn owned_induction_refinement_error_panic_and_accounting_mutation_drop_before_refund() {
    use std::{cell::Cell, rc::Rc};
    struct Candidate {
        module: Module,
        dropped: Rc<Cell<bool>>,
    }
    impl Drop for Candidate {
        fn drop(&mut self) {
            assert!(!self.module.functions.is_empty());
            self.dropped.set(true);
        }
    }
    with_input(fixture(ScalarType::U32, None, 1, true), |input, budget| {
        let floor = budget.storage();
        let ledger = budget.work_ledger_identity_v1();
        for unwind in [false, true] {
            let dropped = Rc::new(Cell::new(false));
            let result: Result<()> = resources::scoped(budget, |meter| {
                let (module, receipt) =
                    meter.derive(|b| Ok(input.copy_module_for_transformation_v12(b)?))?;
                meter.reserve(receipt.retained_storage())?;
                meter.reserve(size_of::<Candidate>())?;
                let _candidate = Candidate {
                    module,
                    dropped: dropped.clone(),
                };
                if unwind {
                    panic!("partial actual induction candidate")
                }
                Err(Error::Recipe("partial actual candidate"))
            });
            if unwind {
                assert!(matches!(result, Err(Error::Panicked)));
            } else {
                assert!(matches!(
                    result,
                    Err(Error::Recipe("partial actual candidate"))
                ));
            }
            assert!(dropped.get());
            assert_eq!(budget.storage(), floor);
            assert!(budget.work_ledger_identity_v1() == ledger);
        }
        let result: Result<()> = resources::scoped(budget, |meter| {
            let (mut rows, _) = meter.table::<usize>(1)?;
            meter.push(&mut rows, 17)?;
            meter.budget_for_test().release_storage(1)?;
            Err(Error::Recipe("masked by corrupt accounting"))
        });
        assert!(matches!(result, Err(Error::Resource(Resource::Accounting))));
        assert!(
            budget.storage() > floor,
            "corrupted accounting must not refund caller or unknown credit"
        );
        budget.release_storage(budget.storage() - floor).unwrap();
        assert!(budget.work_ledger_identity_v1() == ledger);
    });
}

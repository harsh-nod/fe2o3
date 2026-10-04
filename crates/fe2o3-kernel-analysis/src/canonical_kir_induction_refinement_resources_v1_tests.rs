use super::*;
#[derive(Debug)]
struct Observation {
    result: Result<()>,
    work: usize,
    peak: usize,
    failed_work: Option<usize>,
    failed_storage: Option<usize>,
}
fn measure(a: &Owner, b: &Owner, rows: &[Row], floor: usize, w: usize, s: usize) -> Observation {
    let mut work = Work::new(w);
    let (result, accepted, peak, failed_storage) = {
        let mut budget = Budget::new(&mut work, s);
        budget.reserve_storage(floor).unwrap();
        let ledger = budget.work_ledger_identity_v1();
        let result =
            check_canonical_kir_induction_refinement_v1(a, b, rows, Limits::default(), &mut budget)
                .map(|_| ());
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
fn final_output_loop_tail_work(output: &Owner, floor: usize) -> usize {
    let mut work = Work::new(W);
    let mut budget = Budget::new(&mut work, S);
    budget.reserve_storage(floor).unwrap();
    let ledger = budget.work_ledger_identity_v1();
    let (inventory, inventory_size) = Inventory::derive(output, &mut budget).unwrap();
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
    assert_eq!(budget.storage(), floor);
    assert!(budget.work_ledger_identity_v1() == ledger);
    // After the first NaturalLoop vector reserve: later headers, replay, pair transfer.
    remaining_headers + replay + 1
}
#[test]
fn induction_refinement_pair_exact_work_and_storage_preserve_floor_and_history() {
    for mutation in [false, true] {
        let input = fixture(ScalarType::U32, if mutation { 1 } else { 2 }, false);
        let (output, rows) = expected(&input, if mutation { &[(0, 2, 0, 0)] } else { &[] });
        let (a, aa) = admit(&input);
        let (b, bb) = admit(&output);
        let sibling = [0x95u8; FLOOR];
        let floor = aa + bb + rows.capacity() * size_of::<Row>() + sibling.len();
        let measured = measure(&a, &b, &rows, floor, W, S);
        assert!(measured.result.is_ok());
        let tail_work = final_output_loop_tail_work(&b, floor);
        let exact = measure(&a, &b, &rows, floor, measured.work, measured.peak);
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
        let short = measure(&a, &b, &rows, floor, measured.work - 1, measured.peak);
        let Err(Error::Resource(Resource::Work(limit))) = &short.result else {
            panic!("exact final pair-transfer refusal: {short:?}")
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
        let initial_short = measure(&a, &b, &rows, floor, W, initial - 1);
        let Err(Error::Resource(Resource::Storage(limit))) = &initial_short.result else {
            panic!("exact pair-scope header refusal: {initial_short:?}")
        };
        assert_eq!((limit.actual(), limit.limit()), (initial, initial - 1));
        assert_eq!(
            (
                initial_short.work,
                initial_short.peak,
                initial_short.failed_work,
                initial_short.failed_storage
            ),
            (0, floor, None, Some(initial))
        );
        let peak_short = measure(&a, &b, &rows, floor, W, measured.peak - 1);
        let Err(Error::Loops(LoopError::Resource(Resource::Storage(limit)))) = &peak_short.result
        else {
            panic!("exact final-output NaturalLoop backing refusal: {peak_short:?}")
        };
        assert_eq!(
            (limit.actual(), limit.limit()),
            (measured.peak, measured.peak - 1)
        );
        assert_eq!(
            (
                peak_short.work,
                peak_short.peak,
                peak_short.failed_work,
                peak_short.failed_storage
            ),
            (
                measured.work - tail_work,
                measured.peak - size_of::<crate::CanonicalKirNaturalLoopV1>(),
                None,
                Some(measured.peak)
            )
        );
        assert_eq!(sibling, [0x95; FLOOR]);
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
            let retained = {
                let (checked, receipt) = check_canonical_kir_induction_refinement_v1(
                    &a,
                    &b,
                    &rows,
                    Limits::default(),
                    &mut budget,
                )
                .unwrap();
                budget.reserve_storage(receipt.retained_storage()).unwrap();
                assert!(std::ptr::eq(checked.input(), &a));
                receipt.retained_storage()
            };
            budget.release_storage(retained).unwrap();
            assert_eq!(
                (
                    budget.work(),
                    budget.peak_storage(),
                    budget.storage(),
                    budget.failed_storage()
                ),
                (measured.work + 17, measured.peak, floor, Some(floor + S))
            );
        }
        assert_eq!(work.failed_work(), Some(W + 17));
    }
}
#[test]
fn induction_refinement_pair_output_growth_limits_and_arithmetic_refuse() {
    let input = fixture(ScalarType::U32, 1, false);
    let (output, rows) = expected(&input, &[(0, 2, 0, 0)]);
    let (a, aa) = admit(&input);
    let (b, bb) = admit(&output);
    let mut work = Work::new(W);
    let mut budget = Budget::new(&mut work, S);
    let floor = aa + bb + rows.capacity() * size_of::<Row>() + FLOOR;
    budget.reserve_storage(floor).unwrap();
    let limits = Limits {
        operations: 3,
        ..Limits::default()
    };
    assert!(matches!(
        check_canonical_kir_induction_refinement_v1(&a, &b, &rows, limits, &mut budget),
        Err(Error::OutputLimit {
            actual: 4,
            limit: 3
        })
    ));
    assert_eq!(budget.storage(), floor);
    let limits = Limits {
        operations: 4,
        ..Limits::default()
    };
    let retained = {
        let (pair, receipt) =
            check_canonical_kir_induction_refinement_v1(&a, &b, &rows, limits, &mut budget)
                .unwrap();
        budget.reserve_storage(receipt.retained_storage()).unwrap();
        assert_eq!(pair.limits(), limits);
        assert!(receipt.retained_storage() > 0);
        receipt.retained_storage()
    };
    budget.release_storage(retained).unwrap();
    assert_eq!(budget.storage(), floor);
    let failure: Result<()> = resources::scoped(&mut budget, |meter| {
        let _ = meter.table::<Row>(usize::MAX)?;
        Ok(())
    });
    assert!(matches!(
        failure,
        Err(Error::Resource(Resource::Arithmetic))
    ));
    assert_eq!(budget.storage(), floor);
}
#[test]
fn induction_refinement_pair_failed_candidate_and_unwind_keep_live_siblings() {
    use std::{cell::Cell, rc::Rc};
    struct Probe {
        data: Vec<usize>,
        dropped: Rc<Cell<bool>>,
    }
    impl Drop for Probe {
        fn drop(&mut self) {
            assert_eq!(self.data, [17; 3]);
            self.dropped.set(true);
        }
    }
    for unwind in [false, true] {
        let dropped = Rc::new(Cell::new(false));
        let mut work = Work::new(W);
        let mut budget = Budget::new(&mut work, S);
        let sibling = [0x87u8; FLOOR];
        budget.reserve_storage(sibling.len()).unwrap();
        let ledger = budget.work_ledger_identity_v1();
        let result: Result<()> = resources::scoped(&mut budget, |meter| {
            meter.reserve(size_of::<Probe>())?;
            let (mut data, _) = meter.table::<usize>(3)?;
            for _ in 0..3 {
                meter.push(&mut data, 17)?;
            }
            let _probe = Probe {
                data,
                dropped: dropped.clone(),
            };
            if unwind {
                panic!("induction pair partial scratch")
            }
            Err(Error::Mismatch("partial scratch"))
        });
        if unwind {
            assert!(matches!(result, Err(Error::Panicked)));
        } else {
            assert!(matches!(result, Err(Error::Mismatch("partial scratch"))));
        }
        assert!(dropped.get());
        assert_eq!(budget.storage(), FLOOR);
        assert!(budget.work_ledger_identity_v1() == ledger);
        assert_eq!(sibling, [0x87; FLOOR]);
    }
}

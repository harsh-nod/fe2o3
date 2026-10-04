use super::*;

const ACCEPTED_PREFIX: usize = 17;

struct Measurement<E> {
    result: std::result::Result<(), E>,
    work: usize,
    peak: usize,
    failed_work: Option<usize>,
    failed_storage: Option<usize>,
}

fn begin_work(limit: usize, prior_denial: bool) -> Work {
    let mut work = Work::new(limit);
    work.charge_work(ACCEPTED_PREFIX).unwrap();
    if prior_denial {
        let denied = work.charge_work(limit + 11 - ACCEPTED_PREFIX).unwrap_err();
        assert_eq!((denied.actual(), denied.limit()), (limit + 11, limit));
        assert_eq!(work.work(), ACCEPTED_PREFIX);
        assert_eq!(work.failed_work(), Some(limit + 11));
    }
    work
}

fn reserve_floor(budget: &mut Budget<'_>, floor: usize, limit: usize, prior_denial: bool) {
    budget.reserve_storage(floor).unwrap();
    if prior_denial {
        let denied = budget.reserve_storage(limit + 13 - floor).unwrap_err();
        assert!(matches!(denied, Resource::Storage(error)
            if error.actual() == limit + 13 && error.limit() == limit));
        assert_eq!(budget.storage(), floor);
        assert_eq!(budget.peak_storage(), floor);
        assert_eq!(budget.failed_storage(), Some(limit + 13));
    }
}

fn measure(
    input: &Owner,
    floor: usize,
    work_limit: usize,
    storage_limit: usize,
    prior_denial: bool,
) -> Measurement<Error> {
    let mut work = begin_work(work_limit, prior_denial);
    let (result, accepted, peak, failed_storage) = {
        let mut budget = Budget::new(&mut work, storage_limit);
        reserve_floor(&mut budget, floor, storage_limit, prior_denial);
        let ledger = budget.work_ledger_identity_v1();
        let result = prepare_owned_loop_preheaders_v1(input, &mut budget).map(drop);
        assert_eq!(budget.storage(), floor);
        assert!(budget.work_ledger_identity_v1() == ledger);
        (
            result,
            budget.work(),
            budget.peak_storage(),
            budget.failed_storage(),
        )
    };
    assert_eq!(work.work(), accepted);
    Measurement {
        result,
        work: accepted,
        peak,
        failed_work: work.failed_work(),
        failed_storage,
    }
}

#[test]
fn exact_one_short_mutation_noop_resources_and_deterministic_bytes_preserve_live_floor() {
    // Exact remaining work after each frozen fixture's observed storage denial.
    for (incoming, denied_work_suffix) in [
        (Incoming::Branch, 411),
        (Incoming::Conditional, 557),
        (Incoming::IntegerSwitch, 610),
    ] {
        with_input(fixture(incoming), |input, budget| {
            let floor = budget.storage();
            let measured = measure(input, floor, WORK, STORAGE, false);
            assert!(measured.result.is_ok());
            assert_eq!(
                (measured.failed_work, measured.failed_storage),
                (None, None)
            );
            let (work, peak) = (measured.work, measured.peak);
            for prior_denial in [false, true] {
                let exact = measure(input, floor, work, peak, prior_denial);
                assert!(exact.result.is_ok(), "{:?}", exact.result);
                assert_eq!((exact.work, exact.peak), (work, peak));
                assert_eq!(exact.failed_work, prior_denial.then_some(work + 11));
                assert_eq!(exact.failed_storage, prior_denial.then_some(peak + 13));

                let short = measure(input, floor, work - 1, peak, prior_denial);
                assert!(
                    matches!(&short.result,
                    Err(Error::Pair(PairError::Loops(LoopError::Resource(Resource::Work(error)))))
                    if error.actual() == work && error.limit() == work - 1),
                    "{:?}",
                    short.result
                );
                assert_eq!((short.work, short.peak), (work - 4, peak));
                assert_eq!(
                    short.failed_work,
                    Some(if prior_denial { work + 10 } else { work })
                );
                assert_eq!(short.failed_storage, prior_denial.then_some(peak + 13));

                let short = measure(input, floor, work, peak - 1, prior_denial);
                assert!(
                    matches!(&short.result,
                    Err(Error::Pair(PairError::Loops(LoopError::Resource(Resource::Storage(error)))))
                    if error.actual() == peak && error.limit() == peak - 1),
                    "{:?}",
                    short.result
                );
                assert_eq!(
                    (short.work, short.peak),
                    (work - denied_work_suffix, peak - 112)
                );
                assert_eq!(
                    short.failed_storage,
                    Some(if prior_denial { peak + 12 } else { peak })
                );
                assert_eq!(short.failed_work, prior_denial.then_some(work + 11));
            }
            let again = measure(input, floor, WORK, STORAGE, false);
            assert!(again.result.is_ok());
            assert_eq!((work, peak), (again.work, again.peak));
            assert_eq!((again.failed_work, again.failed_storage), (None, None));
            let first = prepare_owned_loop_preheaders_v1(input, budget).unwrap();
            budget.reserve_storage(first.retained_storage()).unwrap();
            let before = budget.work();
            let second = prepare_owned_loop_preheaders_v1(input, budget).unwrap();
            assert_eq!(budget.work() - before, work - ACCEPTED_PREFIX);
            budget.reserve_storage(second.retained_storage()).unwrap();
            assert_eq!(
                first.output().canonical().canonical_bytes(),
                second.output().canonical().canonical_bytes()
            );
            assert_eq!(first.preheaders(), second.preheaders());
            assert_eq!(first.retained_storage(), second.retained_storage());
            release(second, budget);
            release(first, budget);
        });
    }
}

#[test]
fn independent_pair_exact_and_one_short_resource_boundaries_keep_both_owners_live() {
    with_input(nested(), |input, budget| {
        let owner = prepare_owned_loop_preheaders_v1(input, budget).unwrap();
        budget.reserve_storage(owner.retained_storage()).unwrap();
        let floor = budget.storage();
        let run = |work_limit, storage_limit, prior_denial| {
            let mut work = begin_work(work_limit, prior_denial);
            let (result, accepted, peak, failed_storage) = {
                let mut local = Budget::new(&mut work, storage_limit);
                reserve_floor(&mut local, floor, storage_limit, prior_denial);
                let ledger = local.work_ledger_identity_v1();
                let result = check_canonical_kir_loop_preheaders_v1(
                    input,
                    owner.output(),
                    owner.preheaders(),
                    Limits::default(),
                    &mut local,
                )
                .map(drop);
                assert_eq!(local.storage(), floor);
                assert!(local.work_ledger_identity_v1() == ledger);
                (
                    result,
                    local.work(),
                    local.peak_storage(),
                    local.failed_storage(),
                )
            };
            assert_eq!(work.work(), accepted);
            Measurement {
                result,
                work: accepted,
                peak,
                failed_work: work.failed_work(),
                failed_storage,
            }
        };
        let measured = run(WORK, STORAGE, false);
        assert!(measured.result.is_ok());
        assert_eq!(
            (measured.failed_work, measured.failed_storage),
            (None, None)
        );
        let (work, peak) = (measured.work, measured.peak);
        for prior_denial in [false, true] {
            let exact = run(work, peak, prior_denial);
            assert!(exact.result.is_ok(), "{:?}", exact.result);
            assert_eq!((exact.work, exact.peak), (work, peak));
            assert_eq!(exact.failed_work, prior_denial.then_some(work + 11));
            assert_eq!(exact.failed_storage, prior_denial.then_some(peak + 13));

            let short = run(work - 1, peak, prior_denial);
            assert!(
                matches!(&short.result,
                Err(PairError::Loops(LoopError::Resource(Resource::Work(error))))
                if error.actual() == work && error.limit() == work - 1),
                "{:?}",
                short.result
            );
            assert_eq!((short.work, short.peak), (work - 4, peak));
            assert_eq!(
                short.failed_work,
                Some(if prior_denial { work + 10 } else { work })
            );
            assert_eq!(short.failed_storage, prior_denial.then_some(peak + 13));

            let short = run(work, peak - 1, prior_denial);
            assert!(
                matches!(&short.result,
                Err(PairError::Loops(LoopError::Resource(Resource::Storage(error))))
                if error.actual() == peak && error.limit() == peak - 1),
                "{:?}",
                short.result
            );
            // Exact accepted prefix and prior peak of the frozen nested fixture.
            assert_eq!((short.work, short.peak), (work - 1_109, peak - 300));
            assert_eq!(
                short.failed_storage,
                Some(if prior_denial { peak + 12 } else { peak })
            );
            assert_eq!(short.failed_work, prior_denial.then_some(work + 11));
        }
        let limits = Limits {
            blocks: input.module().functions[0]
                .body
                .as_ref()
                .unwrap()
                .blocks
                .len(),
            ..Limits::default()
        };
        let ledger = budget.work_ledger_identity_v1();
        let result = check_canonical_kir_loop_preheaders_v1(
            input,
            owner.output(),
            owner.preheaders(),
            limits,
            budget,
        )
        .map(drop);
        assert!(
            matches!(
                &result,
                Err(PairError::Loops(LoopError::InputLimit {
                    kind: "blocks",
                    actual: 8,
                    limit: 6,
                }))
            ),
            "fresh output loop inventory must obey the supplied output limit: {result:?}"
        );
        assert_eq!(budget.storage(), floor);
        assert!(budget.work_ledger_identity_v1() == ledger);
        release(owner, budget);
    });
}

#[test]
fn underreserved_corrupt_receipts_and_same_identity_changed_rows_refuse() {
    with_input(nested(), |input, budget| {
        let owner = prepare_owned_loop_preheaders_v1(input, budget).unwrap();
        let mut work = Work::new(WORK);
        let mut short = Budget::new(&mut work, STORAGE);
        short.reserve_storage(owner.retained_storage() - 1).unwrap();
        assert!(matches!(
            owner.replay_against(input, &mut short),
            Err(Error::Resource(Resource::Accounting))
        ));
        assert_eq!(short.work(), 0);
        drop(owner);
        for mode in 0..3 {
            let mut bad = prepare_owned_loop_preheaders_v1(input, budget).unwrap();
            match mode {
                0 => bad.retained += 1,
                1 => {
                    let capacity = bad.rows.capacity();
                    bad.rows.reserve_exact(capacity + 1);
                }
                _ => bad.rows.swap(0, 1),
            }
            let receipt = bad.retained_storage();
            budget.reserve_storage(receipt).unwrap();
            let floor = budget.storage();
            let result = bad.replay_against(input, budget);
            assert!(if mode < 2 {
                matches!(result, Err(Error::Resource(Resource::Accounting)))
            } else {
                matches!(result, Err(Error::Pair(_)))
            });
            assert_eq!(budget.storage(), floor);
            release(bad, budget);
        }
    });
}

#[test]
fn stale_private_candidate_is_refused_and_partial_candidate_panic_cleans_up_after_drop() {
    for panic_after_mutation in [false, true] {
        with_input(fixture(Incoming::Conditional), |input, budget| {
            let floor = budget.storage();
            let result: Result<()> = scoped(budget, |meter| {
                let (inventory, is) = meter.derive(|b| Ok(Inventory::derive(input, b)?))?;
                meter.reserve(is.retained_storage())?;
                let (loops, ls) =
                    meter.derive(|b| Ok(Loops::derive(&inventory, Limits::default(), b)?))?;
                meter.reserve(ls.retained_storage())?;
                let (mut candidate, receipt) =
                    meter.derive(|b| Ok(input.copy_module_for_transformation_v12(b)?))?;
                meter.reserve(receipt.retained_storage())?;
                if !panic_after_mutation {
                    candidate.id = "stale".into();
                }
                let result = build::apply(&inventory, &loops, &mut candidate, meter);
                if panic_after_mutation {
                    let (_rows, _) = result?;
                    assert_ne!(&candidate, input.module());
                    panic!("private candidate unwinds before ledger cleanup");
                }
                assert!(matches!(result, Err(Error::Recipe(_))));
                assert_eq!(candidate.functions, input.module().functions);
                result.map(|_| ())
            });
            assert!(if panic_after_mutation {
                matches!(result, Err(Error::Panicked))
            } else {
                matches!(result, Err(Error::Recipe(_)))
            });
            assert_eq!(budget.storage(), floor);
        });
    }
}

#[test]
fn error_panic_extra_storage_and_foreign_ledger_never_refund_live_siblings() {
    for panic in [false, true] {
        let mut work = Work::new(WORK);
        let mut budget = Budget::new(&mut work, STORAGE);
        budget.reserve_storage(SIBLING).unwrap();
        let result: Result<()> = scoped(&mut budget, |meter| {
            let (_rows, _) = meter.table::<u64>(16)?;
            if panic {
                panic!("meter cleanup");
            } else {
                Err(Error::Recipe("failure"))
            }
        });
        assert!(result.is_err());
        assert_eq!(budget.storage(), SIBLING);
        assert!(budget.work() > 0);
        assert!(budget.peak_storage() > SIBLING);
    }
    let mut work = Work::new(WORK);
    let mut budget = Budget::new(&mut work, STORAGE);
    budget.reserve_storage(SIBLING).unwrap();
    let result: Result<()> = scoped(&mut budget, |meter| {
        let (_rows, _) = meter.table::<u64>(16)?;
        meter.budget_for_test().reserve_storage(11)?;
        Ok(())
    });
    assert!(matches!(result, Err(Error::Resource(Resource::Accounting))));
    assert_eq!(budget.storage(), SIBLING + 11);
    let mut foreign_work = Work::new(WORK);
    let mut held = 0;
    let result: Result<()> = scoped(&mut budget, |meter| {
        let (_rows, _) = meter.table::<u64>(16)?;
        held = meter.budget_for_test().storage();
        let mut foreign = Budget::new(&mut foreign_work, STORAGE);
        foreign.reserve_storage(held)?;
        *meter.budget_for_test() = foreign;
        meter.work(1)
    });
    assert!(matches!(result, Err(Error::Resource(Resource::Accounting))));
    assert_eq!(budget.storage(), held);
    assert_eq!(budget.work(), 0);
}

use super::fixture::*;
use super::*;

// Public constituent analyses plus the checker's source-defined entry/table
// charges stop before coverage's allocation. No pair or failed run is queried.
fn coverage_prefix(input: &Owner, output: &Owner, b: &mut Budget<'_>) -> (usize, usize) {
    let floor = b.storage();
    b.charge_work(11).unwrap();
    let limits = Limits::default();
    let (inventory, storage) = Inventory::derive(input, b).unwrap();
    b.reserve_storage(storage.retained_storage()).unwrap();
    let (loops, storage) = Loops::derive(&inventory, limits.loops, b).unwrap();
    b.reserve_storage(storage.retained_storage()).unwrap();
    let (facts, storage) = Facts::derive(&loops, limits.loops, b).unwrap();
    b.reserve_storage(storage.retained_storage()).unwrap();
    facts.replay(&loops, limits.loops, b).unwrap();
    let (final_inventory, storage) = Inventory::derive(output, b).unwrap();
    b.reserve_storage(storage.retained_storage()).unwrap();
    b.charge_work(12).unwrap();
    b.charge_work(
        input
            .canonical()
            .canonical_bytes()
            .len()
            .checked_add(output.canonical().canonical_bytes().len())
            .unwrap(),
    )
    .unwrap();
    // Scratch::new calls table four times before initializing any elements.
    // table pays 4 work before reserve, including the refused coverage table.
    b.charge_work(4 * 4).unwrap();
    let coverage = inventory
        .blocks()
        .len()
        .checked_mul(size_of::<usize>())
        .unwrap();
    assert!(coverage > 0);
    let accepted = b.work();
    drop(final_inventory);
    drop(facts);
    drop(loops);
    drop(inventory);
    b.release_storage(b.storage() - floor).unwrap();
    (accepted, coverage)
}

fn factory_coverage_prefix(input: &Owner, floor: usize) -> (usize, usize) {
    let mut work = Work::new(W);
    let mut b = Budget::new(&mut work, S);
    b.reserve_storage(floor).unwrap();
    // Build the real prefix only, stopping before independent pair admission;
    // do not call unroll or use the failed operation's reported work as oracle.
    let expected = resources::scoped(&mut b, |m: &mut Meter<'_, '_>| -> Result<_> {
        let limits = Limits::default();
        m.work(11)?;
        m.reserve(header()?)?;
        let (inventory, storage) = m.derive(|b| Ok(Inventory::derive(input, b)?))?;
        m.reserve(storage.retained_storage())?;
        let (loops, storage) = m.derive(|b| Ok(Loops::derive(&inventory, limits.loops, b)?))?;
        m.reserve(storage.retained_storage())?;
        let (facts, storage) = m.derive(|b| Ok(Facts::derive(&loops, limits.loops, b)?))?;
        m.reserve(storage.retained_storage())?;
        m.derive(|b| Ok(facts.replay(&loops, limits.loops, b)?))?;
        let (plan, _) = selection::plan(&inventory, &facts, limits, m)?;
        let (mut candidate, storage) =
            m.derive(|b| Ok(input.copy_module_for_transformation_v12(b)?))?;
        m.reserve(storage.retained_storage())?;
        let (rows, extra) = build::materialize(&inventory, &facts, &plan, &mut candidate, m)?;
        let (output, storage) = m.derive(|b| {
            Ok(Owner::from_module_ref_with_verification_budget_v12(
                &candidate, b,
            )?)
        })?;
        m.reserve(storage.retained_storage())?;
        let expected = m.derive(|b| Ok(coverage_prefix(input, &output, b)))?;
        m.work(
            extra
                .checked_add(input.canonical().canonical_bytes().len())
                .ok_or(Resource::Arithmetic)?,
        )?;
        drop(output);
        drop(rows);
        drop(candidate);
        drop(plan);
        drop(facts);
        drop(loops);
        drop(inventory);
        Ok(expected)
    })
    .unwrap();
    assert_eq!((b.storage(), b.failed_storage()), (floor, None));
    expected
}

fn replay_coverage_prefix(
    input: &Owner,
    owner: &OwnedLoopUnrollV1,
    floor: usize,
) -> (usize, usize) {
    let mut work = Work::new(W);
    let mut b = Budget::new(&mut work, S);
    b.reserve_storage(floor).unwrap();
    b.charge_work(11 + size_of::<Identity>() + 3).unwrap();
    let expected = coverage_prefix(input, owner.output(), &mut b);
    assert_eq!((b.storage(), b.failed_storage()), (floor, None));
    expected
}

fn assert_coverage_denial(short: &Observation, peak: usize, expected: (usize, usize)) {
    let Err(Error::Pair(PairError::Resource(Resource::Storage(error)))) = &short.result else {
        panic!("exact independent checker coverage-table Storage phase: {short:?}");
    };
    assert_eq!((error.actual(), error.limit()), (peak, peak - 1));
    assert_eq!(
        (
            short.work,
            short.peak,
            short.failed_work,
            short.failed_storage
        ),
        (
            expected.0,
            peak.checked_sub(expected.1).unwrap(),
            None,
            Some(peak)
        )
    );
}

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
        let mut b = Budget::new(&mut work, s);
        b.reserve_storage(floor).unwrap();
        let ledger = b.work_ledger_identity_v1();
        let result =
            unroll_canonical_kir_loops_v1(input, Limits::default(), &mut b).map(|(o, receipt)| {
                assert_eq!(
                    receipt.retained_storage(),
                    header().unwrap()
                        + o.output_storage.retained_storage()
                        + o.rows.backing().unwrap()
                );
                b.reserve_storage(receipt.retained_storage()).unwrap();
                release(o, &mut b);
            });
        assert_eq!(b.storage(), floor);
        assert!(b.work_ledger_identity_v1() == ledger);
        (result, b.work(), b.peak_storage(), b.failed_storage())
    };
    Observation {
        result,
        work: accepted,
        peak,
        failed_work: work.failed_work(),
        failed_storage,
    }
}
#[test]
fn bounded_unroll_owner_exact_work_peak_and_final_work_short() {
    for literals in [None, Some((0, 0)), Some((0, 3))] {
        let (input, bytes) = admit(&fixture(ScalarType::U32, literals, 1, true));
        let sibling = vec![0x93u8; FLOOR];
        let floor = bytes + size_of::<Vec<u8>>() + sibling.capacity();
        let full = measure(&input, floor, W, S);
        assert!(full.result.is_ok(), "{full:?}");
        let exact = measure(&input, floor, full.work, full.peak);
        assert!(exact.result.is_ok(), "{exact:?}");
        assert_eq!(
            (
                exact.work,
                exact.peak,
                exact.failed_work,
                exact.failed_storage
            ),
            (full.work, full.peak, None, None)
        );
        let short = measure(&input, floor, full.work - 1, full.peak);
        let Err(Error::Resource(Resource::Work(e))) = &short.result else {
            panic!("final owning transfer: {short:?}");
        };
        assert_eq!(
            (
                e.actual(),
                e.limit(),
                short.work,
                short.peak,
                short.failed_work,
                short.failed_storage
            ),
            (
                full.work,
                full.work - 1,
                full.work - 1,
                full.peak,
                Some(full.work),
                None
            )
        );
        assert_eq!(sibling, [0x93; FLOOR]);
    }
}
#[test]
fn bounded_unroll_owner_exact_scope_and_retained_headers_refuse_before_inventory() {
    let (input, bytes) = admit(&fixture(ScalarType::U32, Some((0, 3)), 1, true));
    let sibling = [0x81u8; FLOOR];
    let floor = bytes + sibling.len();
    let scope = size_of::<Meter<'_, '_>>();
    let h = header().unwrap();
    for (limit, actual, work, peak) in [
        (floor + scope - 1, floor + scope, 0, floor),
        (floor + scope + h - 1, floor + scope + h, 11, floor + scope),
    ] {
        let short = measure(&input, floor, W, limit);
        let Err(Error::Resource(Resource::Storage(e))) = &short.result else {
            panic!("exact new header: {short:?}");
        };
        assert_eq!(
            (
                e.actual(),
                e.limit(),
                short.work,
                short.peak,
                short.failed_work,
                short.failed_storage
            ),
            (actual, limit, work, peak, None, Some(actual))
        );
    }
    assert_eq!(sibling, [0x81; FLOOR]);
}
#[test]
fn bounded_unroll_owner_peak_minus_one_phase_observation_requires_strict_successor() {
    for literals in [None, Some((0, 0)), Some((0, 3))] {
        let (input, bytes) = admit(&fixture(ScalarType::U32, literals, 1, true));
        let sibling = vec![0x77u8; FLOOR];
        let floor = bytes + sibling.capacity() + size_of::<Vec<u8>>();
        let full = measure(&input, floor, W, S);
        assert!(full.result.is_ok());
        let short = measure(&input, floor, W, full.peak - 1);
        assert!(short.result.is_err());
        assert_eq!(short.failed_storage, Some(full.peak));
        assert_eq!(short.failed_work, None);
        assert!(short.work < full.work);
        assert!(short.peak < full.peak);
        assert_coverage_denial(&short, full.peak, factory_coverage_prefix(&input, floor));
        assert_eq!(sibling, [0x77; FLOOR]);
    }
}
#[test]
fn bounded_unroll_replay_exact_limits_identity_and_reserved_owner_required() {
    with_input(
        fixture(ScalarType::U32, Some((0, 2)), 1, true),
        |input, b| {
            let o = run(input, b);
            let floor = b.storage();
            for changed in 0..11 {
                let mut l = o.limits();
                match changed {
                    0 => l.loops.functions += 1,
                    1 => l.loops.blocks += 1,
                    2 => l.loops.edges += 1,
                    3 => l.loops.definitions += 1,
                    4 => l.loops.operations += 1,
                    5 => l.loops.loops += 1,
                    6 => l.loops.rows += 1,
                    7 => l.max_iterations -= 1,
                    8 => l.max_output_operand_uses += 1,
                    9 => l.max_output_edge_arguments += 1,
                    10 => l.max_origin_rows += 1,
                    _ => unreachable!(),
                }
                assert!(matches!(o.replay(input, l, b), Err(Error::LimitsMismatch)));
                assert_eq!(b.storage(), floor);
            }
            let (other, bytes) = admit(&fixture(ScalarType::U32, Some((0, 1)), 1, true));
            b.reserve_storage(bytes).unwrap();
            assert!(matches!(
                o.replay(&other, o.limits(), b),
                Err(Error::ForeignInput)
            ));
            drop(other);
            b.release_storage(bytes).unwrap();
            let mut w = Work::new(W);
            {
                let mut empty = Budget::new(&mut w, S);
                assert!(matches!(
                    o.replay(input, o.limits(), &mut empty),
                    Err(Error::Resource(Resource::Accounting))
                ));
            }
            replay(&o, input, b);
            release(o, b);
        },
    );
}
#[test]
fn bounded_unroll_output_caps_and_fixed_iteration_ceiling_are_not_noops() {
    with_input(
        fixture(ScalarType::U32, Some((0, 3)), 1, true),
        |input, b| {
            for kind in 0..8 {
                let mut l = Limits::default();
                match kind {
                    0 => l.max_iterations = 9,
                    1 => l.loops.blocks = 4,
                    2 => l.loops.operations = 5,
                    3 => l.loops.definitions = 12,
                    4 => l.loops.edges = 4,
                    5 => l.max_output_operand_uses = 1,
                    6 => l.max_output_edge_arguments = 1,
                    7 => l.max_origin_rows = 1,
                    _ => unreachable!(),
                }
                let error = unroll_canonical_kir_loops_v1(input, l, b).unwrap_err();
                if kind == 0 {
                    assert!(matches!(error, Error::InvalidIterationLimit(9)));
                } else {
                    let expected = [
                        ("blocks", 9, 4),
                        ("operations", 10, 5),
                        ("definitions", 24, 12),
                        ("edges", 8, 4),
                        ("uses", 22, 1),
                        ("edge arguments", 8, 1),
                        ("origin rows", 76, 1),
                    ][kind - 1];
                    let Error::OutputLimit {
                        kind,
                        actual,
                        limit,
                    } = error
                    else {
                        panic!("pre-copy output cap");
                    };
                    assert_eq!((kind, actual, limit), expected);
                }
            }
            let (o, s) = unroll_canonical_kir_loops_v1(
                input,
                Limits {
                    max_iterations: 2,
                    ..Limits::default()
                },
                b,
            )
            .unwrap();
            b.reserve_storage(s.retained_storage()).unwrap();
            assert_eq!(o.origins().selection, None);
            assert_eq!(input.module(), o.output().module());
            release(o, b);
        },
    );
}
#[test]
fn bounded_unroll_prior_denials_and_foreign_ledger_unwind_preserve_floor() {
    let (input, bytes) = admit(&fixture(ScalarType::U32, Some((0, 2)), 1, true));
    let sibling = vec![0x31u8; FLOOR];
    let floor = bytes + sibling.capacity() + size_of::<Vec<u8>>();
    let full = measure(&input, floor, W, S);
    assert!(full.result.is_ok());
    let mut work = Work::new(W);
    {
        let mut b = Budget::new(&mut work, S);
        b.reserve_storage(floor).unwrap();
        let ledger = b.work_ledger_identity_v1();
        b.charge_work(17).unwrap();
        assert!(matches!(b.charge_work(W), Err(Resource::Work(_))));
        assert!(matches!(b.reserve_storage(S), Err(Resource::Storage(_))));
        let o = run(&input, &mut b);
        assert_eq!(b.work(), full.work + 17);
        replay(&o, &input, &mut b);
        release(o, &mut b);
        assert_eq!(b.storage(), floor);
        assert_eq!(b.failed_storage(), Some(floor + S));
        assert!(b.work_ledger_identity_v1() == ledger);
        let result = resources::scoped(&mut b, |m| -> Result<()> {
            m.reserve(31)?;
            panic!("intentional metered unwind")
        });
        assert!(matches!(result, Err(Error::Panicked)));
        assert_eq!(b.storage(), floor);
    }
    assert_eq!(work.failed_work(), Some(W + 17));
    assert_eq!(sibling, [0x31; FLOOR]);
    let mut original_work = Work::new(W);
    let mut foreign_work = Work::new(W);
    let mut b = Budget::new(&mut original_work, S);
    let floor = FLOOR;
    b.reserve_storage(floor).unwrap();
    let mut saved = None;
    let result = resources::scoped(&mut b, |m| -> Result<()> {
        let mut foreign = Budget::new(&mut foreign_work, S);
        foreign.reserve_storage(floor + size_of::<Meter<'_, '_>>())?;
        saved = Some(std::mem::replace(m.budget_for_test(), foreign));
        m.work(1)
    });
    assert!(matches!(result, Err(Error::Resource(Resource::Accounting))));
    let original = saved.take().unwrap();
    let displaced = std::mem::replace(&mut b, original);
    assert_eq!(displaced.storage(), floor + size_of::<Meter<'_, '_>>());
    assert_eq!(b.storage(), floor + size_of::<Meter<'_, '_>>());
    b.release_storage(size_of::<Meter<'_, '_>>()).unwrap();
    assert_eq!(b.storage(), floor);
}

#[test]
fn bounded_unroll_replay_exact_resources_and_peak_phase_observation() {
    for literals in [None, Some((0, 0)), Some((0, 3))] {
        let (input, bytes) = admit(&fixture(ScalarType::U32, literals, 1, true));
        let owner = {
            let mut work = Work::new(W);
            let mut budget = Budget::new(&mut work, S);
            budget.reserve_storage(bytes).unwrap();
            let (owner, receipt) =
                unroll_canonical_kir_loops_v1(&input, Limits::default(), &mut budget).unwrap();
            assert_eq!(owner.retained, receipt.retained_storage());
            owner
        };
        let sibling = vec![0x71u8; FLOOR];
        let floor = bytes + owner.retained + size_of::<Vec<u8>>() + sibling.capacity();
        let measure = |wc, sc| {
            let mut work = Work::new(wc);
            let (result, accepted, peak, storage) = {
                let mut b = Budget::new(&mut work, sc);
                b.reserve_storage(floor).unwrap();
                let result = match owner.replay(&input, owner.limits(), &mut b) {
                    Ok((pair, s)) => {
                        b.reserve_storage(s.retained_storage()).unwrap();
                        assert!(!pair.grants_authority());
                        Ok(s.retained_storage())
                    }
                    Err(e) => Err(e),
                };
                let result = result.map(|s| b.release_storage(s).unwrap());
                assert_eq!(b.storage(), floor);
                (result, b.work(), b.peak_storage(), b.failed_storage())
            };
            Observation {
                result,
                work: accepted,
                peak,
                failed_work: work.failed_work(),
                failed_storage: storage,
            }
        };
        let full = measure(W, S);
        assert!(full.result.is_ok());
        let exact = measure(full.work, full.peak);
        assert!(exact.result.is_ok());
        assert_eq!(
            (
                exact.work,
                exact.peak,
                exact.failed_work,
                exact.failed_storage
            ),
            (full.work, full.peak, None, None)
        );
        let short = measure(full.work - 1, full.peak);
        let Err(Error::Pair(PairError::Resource(Resource::Work(e)))) = &short.result else {
            panic!("final independent pair transfer: {short:?}");
        };
        assert_eq!(
            (
                e.actual(),
                e.limit(),
                short.work,
                short.peak,
                short.failed_work,
                short.failed_storage
            ),
            (
                full.work,
                full.work - 1,
                full.work - 1,
                full.peak,
                Some(full.work),
                None
            )
        );
        let short = measure(W, full.peak - 1);
        assert!(short.result.is_err());
        assert_eq!(short.failed_storage, Some(full.peak));
        assert_eq!(short.failed_work, None);
        assert!(short.work < full.work);
        assert!(short.peak < full.peak);
        assert_coverage_denial(
            &short,
            full.peak,
            replay_coverage_prefix(&input, &owner, floor),
        );
        assert_eq!(sibling, [0x71; FLOOR]);
        drop(owner);
    }
}

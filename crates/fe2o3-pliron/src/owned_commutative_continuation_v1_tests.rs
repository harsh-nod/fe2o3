use super::super::owned;
use super::*;

#[test]
fn detached_actual_output_and_rows_move_once_without_a_borrow_or_copy() {
    let input = own(&pair(ScalarType::U32, BinaryOp::BitOr, true, true).0);
    let mut work = Work::new(WORK);
    let mut budget = Budget::new(&mut work, STORAGE);
    let floor = FLOOR + input.storage;
    budget.reserve_storage(floor).unwrap();
    let result = resources::scoped(&mut budget, |budget, _| {
        let borrowed = optimize_checked_commutative_bitwise_cse_v1(&input.owner, budget)?;
        let pointers = (
            borrowed.owner().canonical().canonical_bytes().as_ptr(),
            borrowed.occurrences().candidate().operations.as_ptr(),
        );
        let old = borrowed.retained_storage();
        budget.reserve_storage(old)?;
        let output = owned::detach(&input.owner, borrowed, budget)?;
        assert_eq!(
            pointers,
            (
                output.output().canonical().canonical_bytes().as_ptr(),
                output.occurrences().candidate().operations.as_ptr()
            )
        );
        assert_eq!(output.proved_pairs(), 1);
        assert!(budget.peak_storage() > floor + output.retained_storage());
        Ok(output)
    })
    .unwrap();
    assert_eq!(budget.storage(), floor);
    let storage = result.retained_storage();
    budget.reserve_storage(storage).unwrap();
    result.replay_against(&input.owner, &mut budget).unwrap();
    assert!(!result.grants_authority());
    drop(input);
    assert_eq!(result.proved_pairs(), 1);
    drop(result);
    budget.release_storage(storage).unwrap();
    assert_eq!(budget.storage(), floor);
}

#[test]
fn detacher_refuses_equal_bytes_from_a_foreign_producer() {
    let module = pair(ScalarType::I64, BinaryOp::BitXor, true, false).0;
    let input = own(&module);
    let foreign = own(&module);
    assert_eq!(
        input.owner.canonical().canonical_bytes(),
        foreign.owner.canonical().canonical_bytes()
    );
    let mut work = Work::new(WORK);
    let mut budget = Budget::new(&mut work, STORAGE);
    let floor = FLOOR + input.storage + foreign.storage;
    budget.reserve_storage(floor).unwrap();
    let result = resources::scoped(&mut budget, |budget, _| {
        let borrowed = optimize_checked_commutative_bitwise_cse_v1(&foreign.owner, budget)?;
        assert_eq!(borrowed.proved_pairs(), 1);
        budget.reserve_storage(borrowed.retained_storage())?;
        owned::detach(&input.owner, borrowed, budget)
    });
    assert!(matches!(result, Err(Error::Resource(Resource::Accounting))));
    assert_eq!(budget.storage(), floor);
}

#[test]
fn owning_actual_reverse_nested_chains_and_noop_are_independently_replayed() {
    for depth in [2, 8, 32] {
        let (module, expected) = reversed_chain(depth);
        let input = own(&module);
        let mut work = Work::new(WORK);
        let mut budget = Budget::new(&mut work, STORAGE);
        let floor = FLOOR + input.storage;
        budget.reserve_storage(floor).unwrap();
        let output =
            prepare_owned_commutative_bitwise_continuation_v1(&input.owner, &mut budget).unwrap();
        assert_eq!(output.output().module(), &expected);
        assert_eq!(output.proved_pairs(), depth as usize);
        budget.reserve_storage(output.retained_storage()).unwrap();
        assert_eq!(
            output.replay_against(&input.owner, &mut budget).unwrap(),
            depth as usize
        );
    }
    let input = own(&pair(ScalarType::U32, BinaryOp::Subtract, true, true).0);
    let mut work = Work::new(WORK);
    let mut budget = Budget::new(&mut work, STORAGE);
    budget.reserve_storage(FLOOR + input.storage).unwrap();
    let output =
        prepare_owned_commutative_bitwise_continuation_v1(&input.owner, &mut budget).unwrap();
    assert_eq!(output.output().module(), input.owner.module());
    assert_eq!(output.proved_pairs(), 0);
    assert!(!output.execution().changed());
}

#[test]
fn owning_constructor_exact_and_one_short_limits_keep_unrelated_floor() {
    let input = own(&reversed_chain(8).0);
    let run = |work_limit, storage_limit| {
        let mut work = Work::new(work_limit);
        let mut budget = Budget::new(&mut work, storage_limit);
        let floor = FLOOR + input.storage;
        budget.reserve_storage(floor).unwrap();
        let ledger = budget.work_ledger_identity_v1();
        let result = prepare_owned_commutative_bitwise_continuation_v1(&input.owner, &mut budget);
        let ok = result.is_ok();
        drop(result);
        assert_eq!(budget.storage(), floor);
        assert!(budget.work_ledger_identity_v1() == ledger);
        (ok, budget.work(), budget.peak_storage())
    };
    let (ok, work, peak) = run(WORK, STORAGE);
    assert!(ok);
    assert!(run(work, peak).0);
    assert!(!run(work - 1, peak).0);
    assert!(!run(work, peak - 1).0);
    assert!(!run(0, peak).0);
}

#[test]
fn owning_replay_inventory_custody_and_exact_short_budgets() {
    let input = own(&pair(ScalarType::U32, BinaryOp::BitAnd, true, true).0);
    let mut initial_work = Work::new(WORK);
    let mut initial = Budget::new(&mut initial_work, STORAGE);
    initial.reserve_storage(FLOOR + input.storage).unwrap();
    let output =
        prepare_owned_commutative_bitwise_continuation_v1(&input.owner, &mut initial).unwrap();
    let floor = FLOOR + input.storage + output.retained_storage();
    let run = |work_limit, storage_limit| {
        let mut work = Work::new(work_limit);
        let mut budget = Budget::new(&mut work, storage_limit);
        budget.reserve_storage(floor).unwrap();
        let result = output.replay_against(&input.owner, &mut budget);
        assert_eq!(budget.storage(), floor);
        (result.is_ok(), budget.work(), budget.peak_storage())
    };
    let (ok, work, peak) = run(WORK, STORAGE);
    assert!(ok && run(work, peak).0);
    assert!(!run(work - 1, peak).0);
    assert!(!run(work, peak - 1).0);
    let foreign = own(output.output().module());
    let mut work = Work::new(WORK);
    let mut budget = Budget::new(&mut work, STORAGE);
    budget.reserve_storage(floor + foreign.storage).unwrap();
    let (a, sa) = Inventory::derive(&input.owner, &mut budget).unwrap();
    budget.reserve_storage(sa.retained_storage()).unwrap();
    let (b, sb) = Inventory::derive(&foreign.owner, &mut budget).unwrap();
    budget.reserve_storage(sb.retained_storage()).unwrap();
    let before = budget.storage();
    assert!(matches!(
        output.check_inventories_v1(&a, &b, &mut budget),
        Err(Error::Resource(Resource::Accounting))
    ));
    assert_eq!(budget.storage(), before);
}

#[test]
fn owning_first_physical_mutation_error_and_panic_discard_candidate() {
    let input = own(&pair(ScalarType::U32, BinaryOp::BitOr, true, false).0);
    for fault in [1, 2] {
        let mut work = Work::new(WORK);
        let mut budget = Budget::new(&mut work, STORAGE);
        let floor = FLOOR + input.storage;
        budget.reserve_storage(floor).unwrap();
        // The underlying real transaction hook asserts actual_mutation_seen()
        // before returning this refusal, rather than trusting a pass counter.
        assert!(owned::prepare(&input.owner, &mut budget, fault).is_err());
        assert_eq!(budget.storage(), floor);
    }
}

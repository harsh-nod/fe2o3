//! Ledger/residency tests only. Empty inert inputs never enter Runtime or mint
//! a source owner, source-use token, binding, or emission acceptance result.
use super::*;

const INLINE: usize = std::mem::size_of::<Mutex<usize>>();

fn inert_input() -> PhaseEmissionInputV1 {
    PhaseEmissionInputV1 {
        root: SemanticFunctionIdV1::from_index(0),
        semantic: [0; 32],
        expansion: [0; 32],
        expanded_root: [0; 32],
        source_protocol: [0; 32],
        bindings: Vec::new(),
        phases: Vec::new(),
        rows: Vec::new(),
    }
}

fn ledger(remaining: usize) -> PhaseReplayV1 {
    let mut work = INLINE + remaining;
    let replay = PhaseReplayV1::retain_after_emission(inert_input(), &mut work).unwrap();
    assert_eq!(work, 0);
    replay
}

#[test]
fn transfer_retains_only_unspent_source_work_and_empties_external_counter() {
    let mut source = INLINE + 31;
    spend(&mut source, 12).unwrap();
    let replay = PhaseReplayV1::retain_after_emission(inert_input(), &mut source).unwrap();
    assert_eq!(source, 0);
    assert_eq!(*replay.remaining.lock().unwrap(), 19);
    with_replay_work(Some(&replay), |phase| {
        let (_, work) = phase.unwrap();
        assert_eq!(*work, 18);
        spend(work, 7)
    })
    .unwrap();
    assert_eq!(*replay.remaining.lock().unwrap(), 11);
    assert!(spend(&mut source, 1).is_err());
}

#[test]
fn repeated_replays_cannot_replenish_the_live_or_previous_replay_allowance() {
    let replay = ledger(12);
    for expected in [6, 0] {
        with_replay_work(Some(&replay), |phase| spend(phase.unwrap().1, 5)).unwrap();
        assert_eq!(*replay.remaining.lock().unwrap(), expected);
    }
    assert!(with_replay_work(Some(&replay), |_| panic!("exhausted replay ran")).is_err());
    assert_eq!(*replay.remaining.lock().unwrap(), 0);
}

#[test]
fn emission_retention_and_replay_entry_each_need_their_own_remaining_debit() {
    let mut too_small = INLINE - 1;
    assert!(PhaseReplayV1::retain_after_emission(inert_input(), &mut too_small).is_err());
    assert_eq!(too_small, INLINE - 1);

    let mut exact_retention = INLINE;
    let replay = PhaseReplayV1::retain_after_emission(inert_input(), &mut exact_retention).unwrap();
    assert_eq!(exact_retention, 0);
    assert!(with_replay_work(Some(&replay), |_| panic!("unfunded replay ran")).is_err());
}

#[test]
fn error_after_lowering_or_comparison_keeps_all_prior_debits() {
    let replay = ledger(10);
    assert!(
        with_replay_work(Some(&replay), |phase| {
            spend(phase.unwrap().1, 6)?;
            Err(rejected("controlled post-lowering comparison failure"))
        })
        .is_err()
    );
    assert_eq!(*replay.remaining.lock().unwrap(), 3);
    with_replay_work(Some(&replay), |phase| spend(phase.unwrap().1, 2)).unwrap();
    assert_eq!(*replay.remaining.lock().unwrap(), 0);
}

#[test]
fn recursive_replay_is_rejected_without_lending_or_resetting_outer_work() {
    let replay = ledger(10);
    with_replay_work(Some(&replay), |phase| {
        let (_, work) = phase.unwrap();
        assert_eq!(*work, 9);
        assert!(with_replay_work(Some(&replay), |_| panic!("reentrant replay ran")).is_err());
        assert_eq!(*work, 9);
        spend(work, 4)
    })
    .unwrap();
    assert_eq!(*replay.remaining.lock().unwrap(), 5);
}

#[test]
fn concurrent_replay_cannot_overlap_the_outer_validation_interval() {
    let replay = ledger(10);
    with_replay_work(Some(&replay), |phase| {
        let (_, work) = phase.unwrap();
        std::thread::scope(|scope| {
            scope
                .spawn(|| {
                    assert!(
                        with_replay_work(Some(&replay), |_| { panic!("concurrent replay ran") })
                            .is_err()
                    );
                })
                .join()
                .unwrap();
        });
        assert_eq!(*work, 9);
        spend(work, 4)
    })
    .unwrap();
    assert_eq!(*replay.remaining.lock().unwrap(), 5);
}

#[test]
fn panic_poison_is_not_recovered_as_a_fresh_replay_budget() {
    let replay = ledger(10);
    let panic = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let _ = with_replay_work(Some(&replay), |phase| {
            spend(phase.unwrap().1, 4)?;
            panic!("controlled replay panic")
        });
    }));
    assert!(panic.is_err());
    assert!(replay.remaining.is_poisoned());
    assert!(with_replay_work(Some(&replay), |_| panic!("poisoned replay ran")).is_err());
}

#[test]
fn input_is_moved_and_borrowed_without_cloning_spare_vector_capacities() {
    let mut input = inert_input();
    input.bindings.reserve_exact(3);
    input.phases.reserve_exact(4);
    input.rows.reserve_exact(7);
    let pointers = (
        input.bindings.as_ptr(),
        input.phases.as_ptr(),
        input.rows.as_ptr(),
    );
    let retained = input.retained_bytes().unwrap();
    assert_eq!(
        retained,
        std::mem::size_of::<PhaseEmissionInputV1>()
            + input.bindings.capacity()
                * std::mem::size_of::<SemanticExpandedDefinedCapabilityV1>()
            + input.phases.capacity() * std::mem::size_of::<PhaseOccurrenceEmissionInputV1>()
            + input.rows.capacity() * std::mem::size_of::<PhaseEmissionRowV1>()
    );
    // Model CheckedRows' live retained-input debit, then the same debit in
    // each replay. All allocations remain the original moved allocations.
    let mut source = INLINE + 3 * retained + 2;
    spend(&mut source, retained).unwrap();
    let replay = PhaseReplayV1::retain_after_emission(input, &mut source).unwrap();
    assert_eq!(source, 0);
    for _ in 0..2 {
        with_replay_work(Some(&replay), |phase| {
            let (input, work) = phase.unwrap();
            assert!(std::ptr::eq(input, &replay.input));
            assert_eq!(
                pointers,
                (
                    input.bindings.as_ptr(),
                    input.phases.as_ptr(),
                    input.rows.as_ptr()
                )
            );
            spend(work, input.retained_bytes().unwrap())
        })
        .unwrap();
    }
    assert_eq!(*replay.remaining.lock().unwrap(), 0);
}

#[test]
fn live_and_replay_auxiliary_capacities_share_one_nonrefundable_ledger() {
    let mut source = 4096;
    let initial = source;
    let live = reserve::<u64>(9, &mut source).unwrap();
    let live_bytes = live.capacity() * std::mem::size_of::<u64>();
    assert_eq!(initial - source, live_bytes);
    let replay = PhaseReplayV1::retain_after_emission(inert_input(), &mut source).unwrap();
    assert_eq!(source, 0);
    let before = *replay.remaining.lock().unwrap();
    let mut replay_bytes = 0;
    with_replay_work(Some(&replay), |phase| {
        let (_, work) = phase.unwrap();
        let temporary = reserve::<u64>(11, work)?;
        replay_bytes = temporary.capacity() * std::mem::size_of::<u64>();
        assert_eq!(live.capacity() * std::mem::size_of::<u64>(), live_bytes);
        assert_eq!(*work, before - 1 - replay_bytes);
        drop(temporary);
        assert_eq!(*work, before - 1 - replay_bytes);
        // Guard extends beyond transient lowering allocations to comparison.
        assert!(replay.remaining.try_lock().is_err());
        Ok(())
    })
    .unwrap();
    assert_eq!(
        initial - *replay.remaining.lock().unwrap(),
        live_bytes + INLINE + 1 + replay_bytes
    );
    drop(live);
    assert_eq!(*replay.remaining.lock().unwrap(), before - 1 - replay_bytes);
}

#[test]
fn unphased_verification_keeps_the_existing_no_phase_path() {
    let mut called = false;
    with_replay_work(None, |phase| {
        assert!(phase.is_none());
        called = true;
        Ok(())
    })
    .unwrap();
    assert!(called);
}

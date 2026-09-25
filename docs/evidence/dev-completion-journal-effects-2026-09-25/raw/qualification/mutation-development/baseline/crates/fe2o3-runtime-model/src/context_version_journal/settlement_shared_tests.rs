use super::*;
use crate::context_version_journal::retained as retained_execution;

#[allow(unused_macros)]
#[macro_use]
mod templates {
    include!("settlement_wrapper_bodies.rs");
}

macro_rules! settlement_test_expr {
    ($body:expr) => {
        $body
    };
}

fn compare(
    j: &mut Journal,
    writer: Reference,
    evidence: Reference,
    success: bool,
    expected: Result<(), Error>,
    accesses: usize,
) {
    let copy = j.guard_copy_for_test_v1();
    let before = snapshot(j);
    j.reset_access_count_for_test_v1();
    let result = if success {
        j.baseline_settle_success_v1(writer, &Success { writer: evidence })
    } else {
        j.baseline_settle_no_effect_v1(writer, &NoEffect { writer: evidence })
    };
    assert_eq!(result, expected);
    assert_eq!(j.guard_accesses_for_test_v1(), accesses);
    let frozen = snapshot(j);
    if result.is_ok() {
        j.restore_settlement_for_test_v1(&copy, writer);
    }
    assert_eq!(snapshot(j), before);
    j.reset_access_count_for_test_v1();
    assert_eq!(settle(j, writer, evidence, success), expected);
    assert_eq!(j.guard_accesses_for_test_v1(), accesses);
    assert_eq!(snapshot(j), frozen);
    assert_eq!(storage(j), before.storage);
    if expected.is_err() {
        assert_eq!(snapshot(j), before);
    }
}

#[test]
fn settlement_shared_frozen_ranked_faults_and_touched_work() {
    for kind in [Kind::Synchronous, Kind::Submission] {
        for success in [false, true] {
            for fault in 0..11 {
                let (mut j, mut writer, roster) = pending_kind(3, kind);
                let slots = chain(&j, writer);
                let mut evidence = writer;
                let (expected, accesses) = match fault {
                    0 => (Ok(()), 30),
                    1 => {
                        writer.slot = usize::MAX;
                        (Err(Error::InvalidReference), 1)
                    }
                    2 => {
                        evidence.slot = usize::MAX;
                        (Err(Error::SettlementEvidenceMismatch), 1)
                    }
                    3 => {
                        evidence.key.local += 1;
                        (Err(Error::SettlementEvidenceMismatch), 1)
                    }
                    4 => {
                        evidence.key.context_generation += 1;
                        (Err(Error::SettlementEvidenceMismatch), 1)
                    }
                    5 => {
                        evidence.key.kind = if kind == Kind::Synchronous {
                            Kind::Submission
                        } else {
                            Kind::Synchronous
                        };
                        (Err(Error::SettlementEvidenceMismatch), 1)
                    }
                    6 => {
                        j.members[slots[2]] = None;
                        (Err(Error::InvalidState), 6)
                    }
                    7 => {
                        j.allocations[roster[2].allocation.slot] = None;
                        (Err(Error::InvalidState), 7)
                    }
                    8 => {
                        j.members[slots[2]].as_mut().unwrap().next = Some(usize::MAX);
                        (Err(Error::InvalidState), 7)
                    }
                    9 => {
                        j.writer_capacity = j.free.len();
                        (Err(Error::InvalidState), 7)
                    }
                    _ => {
                        j.scratch[2] = Some(BeginMemberPlanV1 {
                            member_slot: usize::MAX,
                            allocation: roster[2].allocation,
                            prior_lineage: 0,
                            attempt_epoch: 0,
                        });
                        (Err(Error::InvalidState), 10)
                    }
                };
                compare(&mut j, writer, evidence, success, expected, accesses);
            }
        }
    }
}

#[allow(clippy::question_mark)]
fn observed(
    journal: &Journal,
    writer: Reference,
    evidence: Reference,
    seen: &Cell<u8>,
    writer_capacity: usize,
    member_capacity: usize,
) -> Result<(Option<usize>, usize), Error> {
    settlement_preflight_body!(
        settlement_test_expr,
        journal,
        writer,
        evidence,
        {
            seen.set(seen.get() * 10 + 1);
            writer_capacity
        },
        {
            seen.set(seen.get() * 10 + 2);
            member_capacity
        },
        retained_execution::shared_retained_header_v1,
        retained_execution::shared_retained_writer_key_v1,
        retained_execution::shared_retained_chain_v1,
        settlement_scratch::shared_settlement_scratch_scan_v1
    )
}

#[test]
fn settlement_shared_capacity_observations_follow_chain_and_are_both_eager() {
    for fault in 0..9 {
        let (mut j, mut writer, roster) = pending(3);
        let head = Some(chain(&j, writer)[0]);
        let mut evidence = writer;
        let mut capacities = (j.free.capacity(), j.member_free.capacity());
        match fault {
            1 => writer.slot = usize::MAX,
            2 => evidence.key.local += 1,
            3 => j.allocations[roster[2].allocation.slot] = None,
            4 => j.writer_capacity = j.free.len(),
            5 => capacities.0 = 0,
            6 => capacities.1 = 0,
            7 => j.scratch.truncate(2),
            8 => {
                j.scratch[2] = Some(BeginMemberPlanV1 {
                    member_slot: 0,
                    allocation: roster[2].allocation,
                    prior_lineage: 0,
                    attempt_epoch: 1,
                })
            }
            _ => {}
        }
        let before = snapshot(&j);
        let seen = Cell::new(0);
        j.reset_access_count_for_test_v1();
        let result = observed(&j, writer, evidence, &seen, capacities.0, capacities.1);
        assert_eq!(
            j.guard_accesses_for_test_v1(),
            [10, 1, 1, 7, 7, 7, 7, 7, 10][fault]
        );
        assert_eq!(seen.get(), if (1..=3).contains(&fault) { 0 } else { 12 });
        assert_eq!(
            result,
            match fault {
                0 => Ok((head, 3)),
                1 => Err(Error::InvalidReference),
                2 => Err(Error::SettlementEvidenceMismatch),
                _ => Err(Error::InvalidState),
            }
        );
        assert_eq!(snapshot(&j), before);
    }
}

#[test]
fn settlement_shared_empty_raw_owner_ignores_unrelated_malformed_storage() {
    for success in [false, true] {
        for rejected in [false, true] {
            let (mut j, writer, _) = pending(0);
            j.allocations.clear();
            j.members.clear();
            j.scratch.clear();
            j.member_free.clear();
            j.allocation_capacity = 0;
            j.registration_watermark = u64::MAX;
            j.reserved_count = usize::MAX;
            if rejected {
                j.member_free.push(usize::MAX);
            }
            compare(
                &mut j,
                writer,
                writer,
                success,
                if rejected {
                    Err(Error::InvalidState)
                } else {
                    Ok(())
                },
                if rejected { 1 } else { 3 },
            );
        }
    }
}

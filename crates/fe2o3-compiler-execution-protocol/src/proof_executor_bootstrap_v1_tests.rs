use super::{
    PROOF_EXECUTOR_BOOTSTRAP_BYTES_V1 as BYTES, PROOF_EXECUTOR_BOOTSTRAP_STORAGE_V1 as STORAGE,
    PROOF_EXECUTOR_BOOTSTRAP_WORK_V1 as WORK, ProofExecutorBootstrapErrorV1 as Failure,
    ProofExecutorBootstrapFramingErrorV1 as Framing, ProofExecutorBootstrapKindV1 as Kind,
    ProofExecutorBootstrapRecordV1 as Record, *,
};
use fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1 as Work;
use std::panic::{AssertUnwindSafe, catch_unwind};

const PID: u32 = 1234;
const SESSION: [u8; 32] = [0x51; 32];
const RUNTIME: [u8; 32] = [0x62; 32];
const KINDS: [Kind; 4] = [Kind::Initial, Kind::Ready, Kind::Finish, Kind::Finished];

// Independent wire oracle, not the private constructor or encoder.
fn wire(kind: u32, pid: u32, session: [u8; 32], runtime: [u8; 32]) -> [u8; 88] {
    let mut bytes = [0; 88];
    bytes[..16].copy_from_slice(b"F2O3PROOFBOOTV1\0");
    bytes[16..20].copy_from_slice(&kind.to_le_bytes());
    bytes[20..24].copy_from_slice(&pid.to_le_bytes());
    bytes[24..56].copy_from_slice(&session);
    bytes[56..88].copy_from_slice(&runtime);
    bytes
}

fn framing(bytes: &[u8]) -> Option<Framing> {
    if bytes.len() != 88 {
        return Some(Framing::Length);
    }
    if &bytes[..16] != b"F2O3PROOFBOOTV1\0" {
        return Some(Framing::Magic);
    }
    if !(1..=4).contains(&u32::from_le_bytes(bytes[16..20].try_into().unwrap())) {
        return Some(Framing::Kind);
    }
    let pid = u32::from_le_bytes(bytes[20..24].try_into().unwrap());
    if pid == 0 || pid > 0x7fff_ffff {
        return Some(Framing::Pid);
    }
    if bytes[24..56].iter().all(|byte| *byte == 0) {
        return Some(Framing::Session);
    }
    if bytes[56..88].iter().all(|byte| *byte == 0) {
        return Some(Framing::RuntimeIdentity);
    }
    None
}

#[test]
fn all_kinds_and_pid_boundaries_round_trip_independent_bytes() {
    assert_eq!(BYTES, 88);
    let floor = BYTES + 13;
    let mut work = Work::new(24 * WORK);
    let mut budget = Budget::new(&mut work, floor + 2 * RETAINED + STORAGE);
    budget.reserve_storage(floor).unwrap();
    for (index, kind) in KINDS.into_iter().enumerate() {
        assert_eq!(kind as u32, index as u32 + 1);
        for pid in [1, i32::MAX as u32] {
            let external = wire(index as u32 + 1, pid, SESSION, RUNTIME);
            let before_work = budget.work();
            {
                let (record, charge) =
                    Record::new(kind, pid, SESSION, RUNTIME, &mut budget).unwrap();
                assert_eq!(budget.storage(), floor);
                budget.reserve_storage(charge.additional_storage()).unwrap();
                assert_eq!(charge.additional_storage(), record.retained_storage());
                assert_eq!(
                    record.retained_storage(),
                    size_of::<(Record, ProofExecutorBootstrapStorageV1)>()
                );
                assert_eq!(record.kind(), kind);
                assert_eq!(record.pid(), pid);
                assert_eq!(record.session(), &SESSION);
                assert_eq!(record.runtime_identity(), &RUNTIME);
                assert_eq!(record.canonical_bytes(), &external);

                let (decoded, charge) = Record::decode(&external, &mut budget).unwrap();
                assert_eq!(budget.storage(), floor + RETAINED);
                budget.reserve_storage(charge.additional_storage()).unwrap();
                assert_eq!(record, decoded);
                assert_eq!(decoded.canonical_bytes(), &external);
                assert!(
                    decoded
                        .matches_association(kind, pid, SESSION, RUNTIME, &mut budget)
                        .unwrap()
                );
                assert_eq!(budget.storage(), floor + 2 * RETAINED);
            }
            budget.release_storage(2 * RETAINED).unwrap();
            assert_eq!(budget.storage(), floor);
            assert_eq!(budget.work(), before_work + 3 * WORK);
        }
    }
    assert_eq!(budget.work(), 24 * WORK);
}

#[test]
fn every_byte_substitution_obeys_canonical_acceptance_not_authentication() {
    let original = wire(1, PID, SESSION, RUNTIME);
    let mut work = Work::new(88 * 256 * WORK);
    let mut budget = Budget::new(&mut work, BYTES + STORAGE + RETAINED);
    budget.reserve_storage(BYTES).unwrap();
    let mut accepted_changes = 0;
    for index in 0..88 {
        for byte in 0..=255 {
            let mut changed = original;
            changed[index] = byte;
            let before_work = budget.work();
            match Record::decode(&changed, &mut budget) {
                Ok((record, charge)) => {
                    assert_eq!(framing(&changed), None, "byte {index}: {byte}");
                    assert_eq!(budget.storage(), BYTES);
                    budget.reserve_storage(charge.additional_storage()).unwrap();
                    assert_eq!(record.canonical_bytes(), &changed);
                    assert_eq!(
                        record.kind() as u32,
                        u32::from_le_bytes(changed[16..20].try_into().unwrap())
                    );
                    assert_eq!(
                        record.pid(),
                        u32::from_le_bytes(changed[20..24].try_into().unwrap())
                    );
                    assert_eq!(record.session().as_slice(), &changed[24..56]);
                    assert_eq!(record.runtime_identity().as_slice(), &changed[56..88]);
                    accepted_changes += usize::from(changed != original);
                    drop((record, charge));
                    budget.release_storage(RETAINED).unwrap();
                }
                Err(Failure::Framing(error)) => {
                    assert_eq!(framing(&changed), Some(error), "byte {index}: {byte}");
                }
                Err(error) => panic!("unexpected resource failure: {error}"),
            }
            assert_eq!(budget.storage(), BYTES);
            assert_eq!(budget.work(), before_work + WORK);
        }
    }
    assert!(accepted_changes > 64 * 255);
}

#[test]
fn invalid_pid_zero_id_and_unknown_kind_are_closed_framing_errors() {
    let mut work = Work::new(100 * WORK);
    let mut budget = Budget::new(&mut work, BYTES + 31 + STORAGE);
    budget.reserve_storage(BYTES + 31).unwrap();
    for (pid, session, runtime, expected) in [
        (0, SESSION, RUNTIME, Framing::Pid),
        (i32::MAX as u32 + 1, SESSION, RUNTIME, Framing::Pid),
        (u32::MAX, SESSION, RUNTIME, Framing::Pid),
        (PID, [0; 32], RUNTIME, Framing::Session),
        (PID, SESSION, [0; 32], Framing::RuntimeIdentity),
        (PID, [0; 32], [0; 32], Framing::Session),
    ] {
        let before = budget.work();
        assert!(
            matches!(Record::new(Kind::Initial, pid, session, runtime, &mut budget),
            Err(Failure::Framing(error)) if error == expected)
        );
        assert!(
            matches!(Record::decode(&wire(1, pid, session, runtime), &mut budget),
            Err(Failure::Framing(error)) if error == expected)
        );
        assert_eq!(budget.storage(), BYTES + 31);
        assert_eq!(budget.work(), before + 2 * WORK);
    }
    for kind in [0, 5, 0x0100_0000, u32::MAX] {
        let before = budget.work();
        assert!(matches!(
            Record::decode(&wire(kind, PID, SESSION, RUNTIME), &mut budget),
            Err(Failure::Framing(Framing::Kind))
        ));
        assert_eq!(budget.storage(), BYTES + 31);
        assert_eq!(budget.work(), before + WORK);
    }
    // An otherwise zero ID with any one nonzero byte remains inert valid content.
    for position in [0, 31] {
        let mut id = [0; 32];
        id[position] = 1;
        let (record, charge) = Record::new(Kind::Ready, PID, id, id, &mut budget).unwrap();
        budget.reserve_storage(charge.additional_storage()).unwrap();
        assert_eq!(record.session(), &id);
        assert_eq!(record.runtime_identity(), &id);
        drop((record, charge));
        budget.release_storage(RETAINED).unwrap();
    }
}

#[test]
fn each_association_field_must_match_but_matching_does_not_approve_it() {
    let mut work = Work::new(12 * WORK);
    let mut budget = Budget::new(&mut work, RETAINED + STORAGE);
    let (record, charge) = Record::new(Kind::Initial, PID, SESSION, RUNTIME, &mut budget).unwrap();
    budget.reserve_storage(charge.additional_storage()).unwrap();
    for (kind, pid, session, runtime, expected) in [
        (Kind::Initial, PID, SESSION, RUNTIME, true),
        (Kind::Ready, PID, SESSION, RUNTIME, false),
        (Kind::Finish, PID, SESSION, RUNTIME, false),
        (Kind::Finished, PID, SESSION, RUNTIME, false),
        (Kind::Initial, PID + 1, SESSION, RUNTIME, false),
        (Kind::Initial, 0, SESSION, RUNTIME, false),
        (Kind::Initial, PID, [0; 32], RUNTIME, false),
        (Kind::Initial, PID, SESSION, [0; 32], false),
        (Kind::Initial, PID, RUNTIME, SESSION, false),
    ] {
        let before = budget.work();
        assert_eq!(
            record
                .matches_association(kind, pid, session, runtime, &mut budget)
                .unwrap(),
            expected
        );
        assert_eq!(budget.storage(), RETAINED);
        assert_eq!(budget.work(), before + WORK);
    }
    budget.release_storage(1).unwrap();
    assert!(matches!(
        record.matches_association(Kind::Initial, PID, SESSION, RUNTIME, &mut budget),
        Err(Failure::Resource(Resource::Accounting))
    ));
}

#[test]
fn original_ledger_exact_and_one_short_boundaries_cover_all_operations() {
    for operation in 0..3 {
        for mode in 0..5 {
            if operation == 0 && mode == 0 {
                continue; // A constructor's fixed by-value inputs have zero borrowed floor.
            }
            let setup = if operation == 2 { WORK } else { 0 };
            let expected_floor = match operation {
                0 => 0,
                1 => BYTES,
                _ => RETAINED,
            };
            let available = if mode == 1 {
                resources::ENTRY_WORK - 1
            } else {
                WORK - usize::from(mode == 2)
            };
            let limit = expected_floor + STORAGE - usize::from(mode == 3);
            let mut work = Work::new(13 + setup + available);
            work.charge_work(13).unwrap();
            assert!(work.charge_work(4 * WORK).is_err());
            {
                let mut budget = Budget::new(&mut work, limit);
                let owner = if operation == 2 {
                    let pair =
                        Record::new(Kind::Initial, PID, SESSION, RUNTIME, &mut budget).unwrap();
                    budget.reserve_storage(pair.1.additional_storage()).unwrap();
                    Some(pair)
                } else {
                    budget.reserve_storage(expected_floor).unwrap();
                    None
                };
                if mode == 0 {
                    budget.release_storage(1).unwrap();
                }
                let floor = budget.storage();
                let peak = budget.peak_storage();
                assert!(budget.reserve_storage(limit + 1).is_err());
                let result = match operation {
                    0 => {
                        Record::new(Kind::Initial, PID, SESSION, RUNTIME, &mut budget).map(|_| true)
                    }
                    1 => Record::decode(&wire(1, PID, SESSION, RUNTIME), &mut budget).map(|_| true),
                    _ => owner.as_ref().unwrap().0.matches_association(
                        Kind::Initial,
                        PID,
                        SESSION,
                        RUNTIME,
                        &mut budget,
                    ),
                };
                assert_eq!(budget.storage(), floor);
                assert_eq!(budget.failed_storage(), Some(floor + limit + 1));
                let charged = match mode {
                    0 => {
                        assert!(matches!(
                            result,
                            Err(Failure::Resource(Resource::Accounting))
                        ));
                        resources::ENTRY_WORK
                    }
                    1 | 2 => {
                        assert!(matches!(result, Err(Failure::Resource(Resource::Work(_)))));
                        if mode == 1 { 0 } else { resources::ENTRY_WORK }
                    }
                    3 => {
                        assert!(matches!(
                            result,
                            Err(Failure::Resource(Resource::Storage(_)))
                        ));
                        WORK
                    }
                    _ => {
                        assert!(result.unwrap());
                        WORK
                    }
                };
                assert_eq!(budget.work(), 13 + setup + charged);
                assert_eq!(
                    budget.peak_storage(),
                    if mode == 4 { floor + STORAGE } else { peak }
                );
            }
            assert_eq!(work.failed_work(), Some(13 + 4 * WORK));
        }
    }
}

#[test]
fn malformed_lengths_and_fields_cannot_bypass_resource_preflight() {
    let oversized = [0u8; 4096];
    for bytes in [&[][..], &oversized[..87], &oversized[..89], &oversized[..]] {
        for mode in 0..3 {
            let mut work = Work::new(WORK - usize::from(mode == 0));
            let mut budget = Budget::new(&mut work, STORAGE - usize::from(mode == 1));
            let result = Record::decode(bytes, &mut budget);
            match mode {
                0 => assert!(matches!(result, Err(Failure::Resource(Resource::Work(_))))),
                1 => assert!(matches!(
                    result,
                    Err(Failure::Resource(Resource::Storage(_)))
                )),
                _ => assert!(matches!(result, Err(Failure::Framing(Framing::Length)))),
            }
            assert_eq!(budget.storage(), 0);
            assert_eq!(
                budget.work(),
                if mode == 0 {
                    resources::ENTRY_WORK
                } else {
                    WORK
                }
            );
        }
    }
    for constructor in [false, true] {
        let mut work = Work::new(WORK - 1);
        let mut budget = Budget::new(&mut work, BYTES + STORAGE);
        budget.reserve_storage(BYTES).unwrap();
        let result = if constructor {
            Record::new(Kind::Initial, 0, [0; 32], [0; 32], &mut budget)
        } else {
            Record::decode(&[0; BYTES], &mut budget)
        };
        assert!(matches!(result, Err(Failure::Resource(Resource::Work(_)))));
        assert_eq!(budget.storage(), BYTES);
        assert_eq!(budget.work(), resources::ENTRY_WORK);
    }
}

#[test]
fn repeated_operations_do_not_reset_work_or_implicitly_reserve_results() {
    let bytes = wire(1, PID, SESSION, RUNTIME);
    let mut work = Work::new(2 * WORK);
    let mut budget = Budget::new(&mut work, BYTES + RETAINED + STORAGE);
    budget.reserve_storage(BYTES).unwrap();
    {
        let (first, charge) = Record::decode(&bytes, &mut budget).unwrap();
        assert_eq!(budget.storage(), BYTES);
        budget.reserve_storage(charge.additional_storage()).unwrap();
        let (second, charge) = Record::decode(&bytes, &mut budget).unwrap();
        assert_eq!(budget.storage(), BYTES + RETAINED);
        budget.reserve_storage(charge.additional_storage()).unwrap();
        assert_eq!(first, second);
        for _ in 0..3 {
            assert!(matches!(
                Record::decode(&bytes, &mut budget),
                Err(Failure::Resource(Resource::Work(_)))
            ));
            assert_eq!(budget.work(), 2 * WORK);
            assert_eq!(budget.storage(), BYTES + 2 * RETAINED);
        }
    }
    budget.release_storage(2 * RETAINED).unwrap();
    assert_eq!(budget.storage(), BYTES);
    assert_eq!(budget.peak_storage(), BYTES + RETAINED + STORAGE);
}

#[test]
fn success_error_and_unwind_restore_storage_without_refunding_work() {
    for mode in 0..3 {
        let mut work = Work::new(13 + WORK);
        work.charge_work(13).unwrap();
        assert!(work.charge_work(WORK + 1).is_err());
        {
            let mut budget = Budget::new(&mut work, 31 + STORAGE);
            budget.reserve_storage(31).unwrap();
            assert!(budget.reserve_storage(STORAGE + 1).is_err());
            let result = catch_unwind(AssertUnwindSafe(|| {
                metered::<()>(&mut budget, 31, || match mode {
                    0 => Ok(()),
                    1 => Err(Framing::Kind.into()),
                    _ => panic!("inert bootstrap fixed scope"),
                })
            }));
            match mode {
                0 => assert!(result.unwrap().is_ok()),
                1 => assert!(matches!(
                    result.unwrap(),
                    Err(Failure::Framing(Framing::Kind))
                )),
                _ => assert!(result.is_err()),
            }
            assert_eq!(budget.storage(), 31);
            assert_eq!(budget.work(), 13 + WORK);
            assert_eq!(budget.peak_storage(), 31 + STORAGE);
            assert_eq!(budget.failed_storage(), Some(32 + STORAGE));
        }
        assert_eq!(work.failed_work(), Some(14 + WORK));
    }
}

use super::binding_tests::references;
use super::*;
use crate::native_capability::tests::{failure, policy, run, transfer_boundaries};
use fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1 as Work;
use std::{os::fd::AsRawFd, panic::AssertUnwindSafe};

type Cap = CompilerExecutionSigningKeyCapabilityV2;
const UNRELATED: usize = 19;

#[test]
fn journal_signing_revalidates_custody_and_charges_original_work() {
    let p = policy(7);
    let cap = key(&p);
    let floor = cap.retained_storage() + p.retained_storage() + 32;
    let work = 2 * Cap::IO_WORK + Cap::SIGN_WORK;
    let (result, accepted, live, _) = run(floor, work, floor + Cap::IO_STORAGE, |b| {
        cap.sign_journal_digest(&p, &[0x42; 32], b)
    });
    let (signature, storage) = result.unwrap();
    cap.key
        .verifying_key()
        .verify_strict(
            &[0x42; 32],
            &ed25519_dalek::Signature::from_bytes(&signature),
        )
        .unwrap();
    assert_eq!(
        storage.additional_storage(),
        size_of::<([u8; 64], Storage)>()
    );
    assert_eq!((accepted, live), (work, floor));
    for limit in [ENTRY_WORK - 1, 2 * Cap::IO_WORK - 1, work - 1] {
        let (result, _, live, _) = run(floor, limit, floor + Cap::IO_STORAGE, |b| {
            cap.sign_journal_digest(&p, &[0x42; 32], b)
        });
        assert!(matches!(
            failure(result),
            Error::Resource(Resource::Work(_))
        ));
        assert_eq!(live, floor);
    }
    let other = policy(8);
    assert!(
        run(floor, work, floor + Cap::IO_STORAGE, |b| {
            cap.sign_journal_digest(&other, &[0x42; 32], b)
        })
        .0
        .is_err()
    );
}

fn key(policy: &Policy) -> Cap {
    let mut seed = [7; KEY_BYTES];
    run(
        KEY_BYTES + policy.retained_storage(),
        1_000_000,
        1_000_000,
        |b| Cap::create_and_zeroize(&mut seed, policy, b),
    )
    .0
    .unwrap()
    .0
}

fn transfer(key: &Cap) -> File {
    run(key.retained_storage(), 1_000_000, 1_000_000, |b| {
        key.try_clone_for_transfer(b)
    })
    .0
    .unwrap()
    .0
}

#[derive(Clone, Copy, Debug)]
enum Limit {
    Entry,
    Floor,
    Work,
    Storage,
    Exact,
}
const LIMITS: [Limit; 5] = [
    Limit::Entry,
    Limit::Floor,
    Limit::Work,
    Limit::Storage,
    Limit::Exact,
];

fn boundary<T>(
    floor: usize,
    work: usize,
    limit: Limit,
    op: impl FnOnce(&mut Budget<'_>) -> Result<T>,
) {
    let prepaid = match limit {
        Limit::Floor => floor - 1,
        _ => floor + UNRELATED,
    };
    let work_limit = match limit {
        Limit::Entry => ENTRY_WORK - 1,
        Limit::Work => work - 1,
        _ => work,
    };
    let storage_limit = prepaid + Cap::IO_STORAGE - usize::from(matches!(limit, Limit::Storage));
    let mut meter = Work::new(work_limit);
    let mut budget = Budget::new(&mut meter, storage_limit);
    budget.reserve_storage(prepaid).unwrap();
    let result = op(&mut budget);
    assert_eq!(budget.storage(), prepaid, "{limit:?}");
    let accepted = budget.work();
    match limit {
        Limit::Entry | Limit::Work => {
            assert!(matches!(
                failure(result),
                Error::Resource(Resource::Work(_))
            ));
            assert_eq!(
                accepted,
                if matches!(limit, Limit::Entry) {
                    0
                } else {
                    ENTRY_WORK
                }
            );
            assert_eq!(budget.peak_storage(), prepaid);
        }
        Limit::Floor => {
            assert!(matches!(
                failure(result),
                Error::Resource(Resource::Accounting)
            ));
            assert_eq!(accepted, ENTRY_WORK);
            assert_eq!(budget.peak_storage(), prepaid);
        }
        Limit::Storage => {
            assert!(matches!(
                failure(result),
                Error::Resource(Resource::Storage(_))
            ));
            assert_eq!(accepted, work);
            assert_eq!(budget.failed_storage(), Some(prepaid + Cap::IO_STORAGE));
            assert_eq!(budget.peak_storage(), prepaid);
        }
        Limit::Exact => {
            assert!(result.is_ok());
            assert_eq!(accepted, work);
            assert_eq!(budget.peak_storage(), storage_limit);
        }
    }
    match limit {
        Limit::Entry => assert_eq!(meter.failed_work(), Some(ENTRY_WORK)),
        Limit::Work => assert_eq!(meter.failed_work(), Some(work)),
        _ => assert_eq!(meter.failed_work(), None),
    }
}

#[test]
fn creation_clears_caller_seed_on_every_resource_boundary_and_key_refusal() {
    let policy = policy(7);
    for limit in LIMITS {
        let mut seed = [7; KEY_BYTES];
        boundary(
            KEY_BYTES + policy.retained_storage(),
            Cap::ADMISSION_WORK,
            limit,
            |budget| {
                let (cap, delta) = Cap::create_and_zeroize(&mut seed, &policy, budget)?;
                assert_eq!(delta.additional_storage(), cap.retained_storage());
                Ok(cap)
            },
        );
        assert_eq!(seed, [0; KEY_BYTES]);
    }
    let mut seed = [8; KEY_BYTES];
    let (result, work, live, _) = run(
        KEY_BYTES + policy.retained_storage(),
        Cap::ADMISSION_WORK,
        1_000_000,
        |b| Cap::create_and_zeroize(&mut seed, &policy, b),
    );
    assert!(matches!(
        failure(result),
        Error::Rejected("signing key does not match the pinned native policy")
    ));
    assert_eq!(work, Cap::ADMISSION_WORK);
    assert_eq!(live, KEY_BYTES + policy.retained_storage());
    assert_eq!(seed, [0; KEY_BYTES]);
}

#[test]
fn admission_and_borrowed_operations_have_exact_shared_ledger_boundaries() {
    let policy = policy(7);
    let cap = key(&policy);
    for limit in LIMITS {
        let before = references(cap.image.as_file());
        let file = transfer(&cap);
        boundary(
            Cap::FILE_STORAGE + policy.retained_storage(),
            Cap::ADMISSION_WORK,
            limit,
            |b| {
                let (admitted, delta) = Cap::from_file(file, &policy, b)?;
                assert_eq!(
                    delta.additional_storage(),
                    admitted.retained_storage() - Cap::FILE_STORAGE
                );
                Ok(admitted)
            },
        );
        assert_eq!(references(cap.image.as_file()), before);
        let inherited = transfer(&cap);
        let before = references(&inherited);
        rustix::io::fcntl_setfd(&inherited, rustix::io::FdFlags::empty()).unwrap();
        boundary(
            Cap::FILE_STORAGE + policy.retained_storage(),
            Cap::ADMISSION_WORK,
            limit,
            |b| {
                let (admitted, delta) = Cap::from_inherited_at(inherited.as_raw_fd(), &policy, b)?;
                assert_eq!(delta.additional_storage(), admitted.retained_storage());
                Ok(admitted)
            },
        );
        assert_eq!(references(&inherited), before);
        assert_eq!(
            rustix::io::fcntl_getfd(&inherited).unwrap(),
            rustix::io::FdFlags::empty()
        );
        boundary(
            cap.retained_storage() + policy.retained_storage(),
            Cap::IO_WORK,
            limit,
            |b| cap.revalidate(&policy, b),
        );
        boundary(cap.retained_storage(), Cap::IO_WORK, limit, |b| {
            let (file, delta) = cap.try_clone_for_transfer(b)?;
            assert_eq!(delta.additional_storage(), Cap::FILE_STORAGE);
            Ok(file)
        });
    }
}

#[test]
fn exact_borrowed_transfer_requires_owner_file_policy_and_preserves_sticky_denials() {
    let policy = policy(7);
    let cap = key(&policy);
    let file = transfer(&cap);
    assert_eq!(references(&file), 2);
    transfer_boundaries(
        cap.retained_storage() + Cap::FILE_STORAGE + policy.retained_storage(),
        Cap::IO_WORK,
        Cap::IO_STORAGE,
        |budget| {
            let result = cap.validate_transfer(&file, &policy, budget);
            assert_eq!(references(&file), 2);
            result
        },
    );
    assert_eq!(references(&file), 2);
    assert_eq!(
        rustix::io::fcntl_getfd(&file).unwrap(),
        rustix::io::FdFlags::CLOEXEC
    );
}

#[test]
fn guard_clears_live_buffer_after_partial_read_error_refusal_and_unwind() {
    for case in 0..7 {
        let mut seed = [0; KEY_BYTES];
        let result = std::panic::catch_unwind(AssertUnwindSafe(|| {
            with_secret(
                &mut seed,
                |bytes| {
                    bytes[..17].fill(9);
                    match case {
                        0 => return Err(Error::Rejected("short sealed image read")),
                        1 => return Err(Error::io("read sealed image", rustix::io::Errno::INTR)),
                        2 => return Err(Error::io("read sealed image", rustix::io::Errno::IO)),
                        _ => bytes[17..].fill(9),
                    }
                    match case {
                        3 => Err(Error::Rejected("post-read metadata refusal")),
                        4 => panic!("read callback unwound"),
                        _ => Ok(()),
                    }
                },
                |bytes| {
                    assert_eq!(bytes, &[9; KEY_BYTES]);
                    if case == 5 {
                        panic!("seed consumer unwound");
                    }
                    Ok(())
                },
            )
        }));
        assert_eq!(seed, [0; KEY_BYTES], "case {case}");
        match case {
            0..=3 => assert!(result.unwrap().is_err()),
            4..=5 => assert!(result.is_err()),
            _ => assert!(result.unwrap().is_ok()),
        }
    }
}

#[test]
fn seed_guard_and_scope_restore_storage_without_refunding_work_on_unwind() {
    let mut seed = [7; KEY_BYTES];
    let mut meter = Work::new(Cap::ADMISSION_WORK);
    let mut budget = Budget::new(&mut meter, KEY_BYTES + UNRELATED + Cap::IO_STORAGE);
    budget.reserve_storage(KEY_BYTES + UNRELATED).unwrap();
    let result = std::panic::catch_unwind(AssertUnwindSafe(|| {
        let seed = SeedGuard(&mut seed);
        Cap::scope::<()>(&mut budget, KEY_BYTES, Cap::ADMISSION_WORK, |_| {
            assert_eq!(seed.0, &[7; KEY_BYTES]);
            panic!("admission callback unwound")
        })
    }));
    assert!(result.is_err());
    assert_eq!(seed, [0; KEY_BYTES]);
    assert_eq!(budget.storage(), KEY_BYTES + UNRELATED);
    assert_eq!(budget.work(), Cap::ADMISSION_WORK);
    assert_eq!(
        budget.peak_storage(),
        KEY_BYTES + UNRELATED + Cap::IO_STORAGE
    );
}

#[test]
fn named_logical_quotas_are_frozen_independently() {
    assert_eq!(Cap::IO_WORK, 66_568);
    assert_eq!(Cap::DERIVATION_WORK, 65_536);
    assert_eq!(Cap::ADMISSION_WORK, 132_104);
    assert!(Cap::IO_STORAGE >= 4 * size_of::<SigningKey>() + 4 * KEY_BYTES + 8192);
}

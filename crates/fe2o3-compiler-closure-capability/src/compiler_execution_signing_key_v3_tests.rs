use super::*;
use crate::native_capability::tests::{failure, run, transfer_boundaries};
use fe2o3_compiler_execution_protocol::{
    CompilerExecutionIssuerMeasurementV1 as Measurement,
    CompilerExecutionNativeJournalErrorV3 as JournalError,
};
use fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1 as Work;
use std::{error::Error as _, os::fd::AsRawFd, panic::AssertUnwindSafe};

type Cap = CompilerExecutionSigningKeyCapabilityV3;

fn policy(generation: u64, budget: &mut Budget<'_>) -> Policy {
    let (policy, storage) = Policy::new(
        generation,
        Measurement::new([11; 32], 123).unwrap(),
        Measurement::new([12; 32], 456).unwrap(),
        SigningKey::from_bytes(&[7; 32]).verifying_key().to_bytes(),
        SigningKey::from_bytes(&[8; 32]).verifying_key().to_bytes(),
        budget,
    )
    .unwrap();
    budget
        .reserve_storage(storage.additional_storage())
        .unwrap();
    policy
}

fn key(policy: &Policy, budget: &mut Budget<'_>) -> Cap {
    let mut seed = [7; KEY_BYTES];
    budget.reserve_storage(KEY_BYTES).unwrap();
    let (cap, storage) = Cap::create_and_zeroize(&mut seed, policy, budget).unwrap();
    assert_eq!(seed, [0; KEY_BYTES]);
    assert_eq!(storage.additional_storage(), cap.retained_storage());
    budget
        .reserve_storage(storage.additional_storage())
        .unwrap();
    budget.release_storage(KEY_BYTES).unwrap();
    cap
}

#[test]
fn key_transfer_readmission_and_signing_preserve_one_account() {
    let mut work = Work::new(3_000_000);
    let mut budget = Budget::new(&mut work, 1_000_000);
    budget.reserve_storage(19).unwrap();
    let ledger = budget.work_ledger_identity_v1();
    let policy = policy(7, &mut budget);
    let policy_charge = policy.retained_storage();
    let cap = key(&policy, &mut budget);
    let retained = cap.retained_storage();
    let (file, storage) = cap.try_clone_for_transfer(&mut budget).unwrap();
    assert_eq!(storage.additional_storage(), Cap::FILE_STORAGE);
    budget
        .reserve_storage(storage.additional_storage())
        .unwrap();
    assert_eq!(
        rustix::io::fcntl_getfd(&file).unwrap(),
        rustix::io::FdFlags::CLOEXEC
    );
    assert_eq!(
        rustix::fs::fcntl_getfl(&file).unwrap() & rustix::fs::OFlags::ACCMODE,
        rustix::fs::OFlags::RDONLY
    );
    rustix::fs::seek(&file, rustix::fs::SeekFrom::Start(17)).unwrap();
    cap.validate_transfer(&file, &policy, &mut budget).unwrap();
    assert_eq!(
        rustix::fs::seek(&file, rustix::fs::SeekFrom::Current(0)).unwrap(),
        17
    );
    let (readmitted, delta) = Cap::from_file(file, &policy, &mut budget).unwrap();
    assert_eq!(delta.additional_storage(), retained - Cap::FILE_STORAGE);
    budget.reserve_storage(delta.additional_storage()).unwrap();
    assert_eq!(budget.storage(), 19 + policy_charge + 2 * retained);
    let floor = budget.storage();
    let before = budget.work();
    budget.reserve_storage(32).unwrap();
    let (signature, storage) = readmitted
        .sign_journal_digest(&policy, &[0x42; 32], &mut budget)
        .unwrap();
    assert_eq!(budget.work() - before, 2 * Cap::IO_WORK + Cap::SIGN_WORK);
    assert_eq!(budget.storage(), floor + 32);
    budget
        .reserve_storage(storage.additional_storage())
        .unwrap();
    ed25519_dalek::VerifyingKey::from_bytes(&cap.verifying_key())
        .unwrap()
        .verify_strict(
            &[0x42; 32],
            &ed25519_dalek::Signature::from_bytes(&signature),
        )
        .unwrap();
    assert!(budget.work_ledger_identity_v1() == ledger);
    assert_eq!(readmitted.policy_identity(), policy.identity());
    drop((readmitted, cap, policy));
    budget
        .release_storage(2 * retained + policy_charge + 32 + storage.additional_storage())
        .unwrap();
    assert_eq!(budget.storage(), 19);
}

#[test]
fn creation_wipes_seed_on_exact_resource_boundaries_and_policy_refusal() {
    let mut work = Work::new(1_000_000);
    let mut budget = Budget::new(&mut work, 1_000_000);
    let policy = policy(7, &mut budget);
    let floor = KEY_BYTES + policy.retained_storage();
    transfer_boundaries(floor, Cap::ADMISSION_WORK, Cap::IO_STORAGE, |b| {
        let mut seed = [7; KEY_BYTES];
        let result = Cap::create_and_zeroize(&mut seed, &policy, b);
        assert_eq!(seed, [0; KEY_BYTES]);
        result.map(|(cap, charge)| assert_eq!(charge.additional_storage(), cap.retained_storage()))
    });
    let mut seed = [8; KEY_BYTES];
    let (result, accepted, live, _) =
        run(floor, Cap::ADMISSION_WORK, floor + Cap::IO_STORAGE, |b| {
            Cap::create_and_zeroize(&mut seed, &policy, b)
        });
    assert_eq!(seed, [0; KEY_BYTES]);
    assert_eq!((accepted, live), (Cap::ADMISSION_WORK, floor));
    assert!(matches!(
        failure(result),
        Error::Rejected("signing key does not match the pinned native policy")
    ));
}

#[test]
fn key_admission_and_transfer_require_prepaid_owner_file_and_policy() {
    let mut work = Work::new(1_000_000);
    let mut budget = Budget::new(&mut work, 1_000_000);
    let policy = policy(7, &mut budget);
    let cap = key(&policy, &mut budget);
    let (file, storage) = cap.try_clone_for_transfer(&mut budget).unwrap();
    budget
        .reserve_storage(storage.additional_storage())
        .unwrap();
    transfer_boundaries(
        cap.retained_storage() + Cap::FILE_STORAGE + policy.retained_storage(),
        Cap::IO_WORK,
        Cap::IO_STORAGE,
        |b| cap.validate_transfer(&file, &policy, b),
    );
    transfer_boundaries(
        cap.retained_storage() + policy.retained_storage(),
        Cap::IO_WORK,
        Cap::IO_STORAGE,
        |b| cap.revalidate(&policy, b),
    );
    transfer_boundaries(cap.retained_storage(), Cap::IO_WORK, Cap::IO_STORAGE, |b| {
        cap.try_clone_for_transfer(b)
            .map(|(_, storage)| assert_eq!(storage.additional_storage(), Cap::FILE_STORAGE))
    });
    transfer_boundaries(
        Cap::FILE_STORAGE + policy.retained_storage(),
        Cap::ADMISSION_WORK,
        Cap::IO_STORAGE,
        |b| {
            let file = cap.image.clone_fixed().unwrap();
            Cap::from_file(file, &policy, b).map(|(admitted, delta)| {
                assert_eq!(
                    delta.additional_storage(),
                    admitted.retained_storage() - Cap::FILE_STORAGE
                )
            })
        },
    );
    rustix::io::fcntl_setfd(&file, rustix::io::FdFlags::empty()).unwrap();
    transfer_boundaries(
        Cap::FILE_STORAGE + policy.retained_storage(),
        Cap::ADMISSION_WORK,
        Cap::IO_STORAGE,
        |b| {
            Cap::from_inherited_at(file.as_raw_fd(), &policy, b).map(|(admitted, delta)| {
                assert_eq!(delta.additional_storage(), admitted.retained_storage())
            })
        },
    );
    assert_eq!(
        rustix::io::fcntl_getfd(&file).unwrap(),
        rustix::io::FdFlags::empty()
    );
    assert!(file.metadata().is_ok());
}

#[test]
fn key_rejects_same_seed_in_another_object_and_same_key_in_another_policy() {
    let mut work = Work::new(2_000_000);
    let mut budget = Budget::new(&mut work, 1_000_000);
    let original = policy(7, &mut budget);
    let different = policy(8, &mut budget);
    let cap = key(&original, &mut budget);
    let other = key(&original, &mut budget);
    let (alias, charge) = cap.try_clone_for_transfer(&mut budget).unwrap();
    budget.reserve_storage(charge.additional_storage()).unwrap();
    let (substitute, charge) = other.try_clone_for_transfer(&mut budget).unwrap();
    budget.reserve_storage(charge.additional_storage()).unwrap();
    let floor = budget.storage();
    assert!(matches!(
        cap.validate_transfer(&substitute, &original, &mut budget),
        Err(Error::Rejected("sealed image identity or length changed"))
    ));
    assert!(matches!(
        cap.revalidate(&different, &mut budget),
        Err(Error::Rejected(
            "signing key is pinned to another native policy"
        ))
    ));
    assert!(matches!(
        cap.validate_transfer(&alias, &different, &mut budget),
        Err(Error::Rejected(
            "signing key is pinned to another native policy"
        ))
    ));
    cap.validate_transfer(&alias, &original, &mut budget)
        .unwrap();
    assert_eq!(budget.storage(), floor);
    assert!(substitute.metadata().is_ok());
}

#[test]
fn secret_staging_is_wiped_after_success_read_error_and_unwind() {
    for case in 0..4 {
        let mut seed = [0; KEY_BYTES];
        let result = std::panic::catch_unwind(AssertUnwindSafe(|| {
            with_secret(
                &mut seed,
                |bytes| {
                    bytes[..17].fill(7);
                    if case == 0 {
                        return Err(Error::Rejected("short secret read"));
                    }
                    if case == 1 {
                        panic!("read unwound");
                    }
                    bytes[17..].fill(7);
                    Ok(())
                },
                |bytes| {
                    assert_eq!(bytes, &[7; KEY_BYTES]);
                    if case == 2 {
                        panic!("consumer unwound");
                    }
                    Ok(())
                },
            )
        }));
        assert_eq!(seed, [0; KEY_BYTES]);
        match case {
            0 => assert!(result.unwrap().is_err()),
            1 | 2 => assert!(result.is_err()),
            _ => result.unwrap().unwrap(),
        }
    }
    let mut seed = [7; KEY_BYTES];
    let mut work = Work::new(Cap::ADMISSION_WORK);
    let mut budget = Budget::new(&mut work, KEY_BYTES + 19 + Cap::IO_STORAGE);
    budget.reserve_storage(KEY_BYTES + 19).unwrap();
    let result = std::panic::catch_unwind(AssertUnwindSafe(|| {
        let seed = SeedGuard(&mut seed);
        Cap::scope::<()>(&mut budget, KEY_BYTES, Cap::ADMISSION_WORK, |_| {
            assert_eq!(seed.0, &[7; KEY_BYTES]);
            panic!("admission unwound")
        })
    }));
    assert!(result.is_err());
    assert_eq!(seed, [0; KEY_BYTES]);
    assert_eq!(budget.storage(), KEY_BYTES + 19);
    assert_eq!(budget.work(), Cap::ADMISSION_WORK);
    assert_eq!(budget.peak_storage(), KEY_BYTES + 19 + Cap::IO_STORAGE);
}

#[test]
fn signing_errors_keep_original_work_storage_floor_and_typed_source() {
    let mut setup_work = Work::new(1_000_000);
    let mut setup_budget = Budget::new(&mut setup_work, 1_000_000);
    let policy = policy(7, &mut setup_budget);
    let cap = key(&policy, &mut setup_budget);
    let floor = cap.retained_storage() + policy.retained_storage() + 32;
    let required = 2 * Cap::IO_WORK + Cap::SIGN_WORK;
    for work in [ENTRY_WORK - 1, 2 * Cap::IO_WORK - 1, required - 1, required] {
        let (result, accepted, live, _) = run(floor, work, floor + Cap::IO_STORAGE, |b| {
            cap.sign_journal_digest(&policy, &[0x42; 32], b)
        });
        assert_eq!(live, floor);
        if work == required {
            assert!(result.is_ok());
            assert_eq!(accepted, required);
        } else {
            assert!(matches!(
                failure(result),
                Error::Resource(Resource::Work(_))
            ));
            assert_eq!(
                accepted,
                if work < ENTRY_WORK {
                    0
                } else if work < 2 * Cap::IO_WORK {
                    ENTRY_WORK
                } else {
                    2 * Cap::IO_WORK
                }
            );
        }
    }
    let mut work = Work::new(required);
    let mut budget = Budget::new(&mut work, floor + Cap::IO_STORAGE + 17);
    budget.reserve_storage(floor).unwrap();
    let ledger = budget.work_ledger_identity_v1();
    let error = failure(cap.signing_scope::<()>(&policy, 32, &mut budget, |b| {
        assert!(b.work_ledger_identity_v1() == ledger);
        b.charge_work(17)?;
        b.reserve_storage(17)?;
        Err(
            fe2o3_compiler_execution_protocol::CompilerExecutionAttestationErrorV3::Resource(
                Resource::Accounting,
            )
            .into(),
        )
    }));
    assert!(matches!(
        &error,
        Error::PolicyV3(
            fe2o3_compiler_execution_protocol::CompilerExecutionAttestationErrorV3::Resource(
                Resource::Accounting
            )
        )
    ));
    assert!(
        error
            .source()
            .unwrap()
            .is::<fe2o3_compiler_execution_protocol::CompilerExecutionAttestationErrorV3>()
    );
    assert!(budget.work_ledger_identity_v1() == ledger);
    assert_eq!(budget.storage(), floor);
    assert_eq!(budget.work(), 2 * Cap::IO_WORK + 17);
}

#[test]
fn native_currentness_error_keeps_typed_cause_and_resource_denial() {
    let error = Error::from(JournalError::Mismatch("V3 currentness mismatch"));
    assert!(matches!(
        &error,
        Error::JournalV3(JournalError::Mismatch(_))
    ));
    assert!(error.source().unwrap().is::<JournalError>());
    assert!(matches!(
        Error::from(JournalError::Resource(Resource::Accounting)),
        Error::Resource(Resource::Accounting)
    ));
}

#[test]
fn v2_and_v3_named_key_quotas_and_layout_are_identical() {
    type V2 = crate::CompilerExecutionSigningKeyCapabilityV2;
    assert_eq!(size_of::<Cap>(), size_of::<V2>());
    assert_eq!(Cap::FILE_STORAGE, V2::FILE_STORAGE);
    assert_eq!(Cap::IO_STORAGE, V2::IO_STORAGE);
    assert_eq!(Cap::IO_WORK, 66_568);
    assert_eq!(Cap::IO_WORK, V2::IO_WORK);
    assert_eq!(Cap::ADMISSION_WORK, 132_104);
    assert_eq!(Cap::ADMISSION_WORK, V2::ADMISSION_WORK);
    assert_eq!(Cap::DERIVATION_WORK, V2::DERIVATION_WORK);
    assert_eq!(Cap::SIGN_WORK, V2::SIGN_WORK);
}

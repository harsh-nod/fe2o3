use super::*;
use crate::native_capability::{
    CompilerExecutionCapabilityErrorV2 as Error, ENTRY_WORK,
    tests::{failure, policy as policy_v2, run, sealed, transfer_boundaries},
};
use ed25519_dalek::SigningKey;
use fe2o3_compiler_execution_protocol::{
    COMPILER_EXECUTION_ISSUER_POLICY_STORAGE_V3 as POLICY_STORAGE,
    COMPILER_EXECUTION_ISSUER_POLICY_WORK_V3 as POLICY_WORK,
    CompilerExecutionAttestationErrorV3 as PolicyError,
    CompilerExecutionIssuerMeasurementV1 as Measurement,
};
use fe2o3_kernel_ir::{
    CanonicalKernelIrVerificationResourceErrorV1 as Resource, CanonicalKernelIrWorkBudgetV1 as Work,
};
use std::{error::Error as _, os::fd::AsRawFd};

type Cap = CompilerExecutionPolicyCapabilityV3;

fn policy(budget: &mut Budget<'_>) -> Policy {
    let (policy, charge) = Policy::new(
        7,
        Measurement::new([11; 32], 123).unwrap(),
        Measurement::new([12; 32], 456).unwrap(),
        SigningKey::from_bytes(&[7; 32]).verifying_key().to_bytes(),
        SigningKey::from_bytes(&[8; 32]).verifying_key().to_bytes(),
        budget,
    )
    .unwrap();
    budget.reserve_storage(charge.additional_storage()).unwrap();
    policy
}

#[test]
fn policy_transfers_keep_original_ledger_inode_and_exact_charges() {
    let mut work = Work::new(2_000_000);
    let mut budget = Budget::new(&mut work, 1_000_000);
    budget.reserve_storage(19).unwrap();
    let ledger = budget.work_ledger_identity_v1();
    let policy = policy(&mut budget);
    let bytes = *policy.canonical_bytes();
    let (cap, delta) = Cap::create(policy, &mut budget).unwrap();
    budget.reserve_storage(delta.additional_storage()).unwrap();
    let retained = cap.retained_storage();
    assert_eq!(budget.storage(), 19 + retained);
    let (file, charge) = cap.try_clone_for_transfer(&mut budget).unwrap();
    assert_eq!(charge.additional_storage(), Cap::FILE_STORAGE);
    budget.reserve_storage(charge.additional_storage()).unwrap();
    rustix::fs::seek(&file, rustix::fs::SeekFrom::Start(17)).unwrap();
    cap.validate_transfer(&file, &mut budget).unwrap();
    assert_eq!(
        rustix::fs::seek(&file, rustix::fs::SeekFrom::Current(0)).unwrap(),
        17
    );
    let (readmitted, delta) = Cap::from_file(file, &mut budget).unwrap();
    assert_eq!(delta.additional_storage(), retained - Cap::FILE_STORAGE);
    budget.reserve_storage(delta.additional_storage()).unwrap();
    assert_eq!(readmitted.policy().canonical_bytes(), &bytes);
    readmitted.revalidate(&mut budget).unwrap();
    drop(readmitted);
    budget.release_storage(retained).unwrap();
    let (inherited, charge) = cap.try_clone_for_transfer(&mut budget).unwrap();
    budget.reserve_storage(charge.additional_storage()).unwrap();
    rustix::io::fcntl_setfd(&inherited, rustix::io::FdFlags::empty()).unwrap();
    let (recovered, charge) = Cap::from_inherited_at(inherited.as_raw_fd(), &mut budget).unwrap();
    assert_eq!(charge.additional_storage(), retained);
    budget.reserve_storage(charge.additional_storage()).unwrap();
    assert_eq!(recovered.policy().canonical_bytes(), &bytes);
    assert_eq!(
        rustix::io::fcntl_getfd(&inherited).unwrap(),
        rustix::io::FdFlags::empty()
    );
    assert!(budget.work_ledger_identity_v1() == ledger);
    drop((inherited, recovered, cap));
    budget
        .release_storage(Cap::FILE_STORAGE + 2 * retained)
        .unwrap();
    assert_eq!(budget.storage(), 19);
    assert_eq!(budget.work(), 7 * Cap::IO_WORK + 3 * POLICY_WORK);
}

#[test]
fn policy_transfer_requires_exact_object_and_prepaid_floor() {
    let mut work = Work::new(1_000_000);
    let mut budget = Budget::new(&mut work, 1_000_000);
    let p = policy(&mut budget);
    let (cap, delta) = Cap::create(p, &mut budget).unwrap();
    budget.reserve_storage(delta.additional_storage()).unwrap();
    let (alias, delta) = cap.try_clone_for_transfer(&mut budget).unwrap();
    budget.reserve_storage(delta.additional_storage()).unwrap();
    transfer_boundaries(
        cap.retained_storage() + Cap::FILE_STORAGE,
        Cap::IO_WORK,
        Cap::IO_STORAGE,
        |b| cap.validate_transfer(&alias, b),
    );
    budget.reserve_storage(Cap::FILE_STORAGE).unwrap();
    let replacement = sealed(cap.policy().canonical_bytes());
    assert!(matches!(
        cap.validate_transfer(&replacement, &mut budget),
        Err(Error::Rejected("sealed image identity or length changed"))
    ));
    assert!(replacement.metadata().is_ok());
    cap.validate_transfer(&alias, &mut budget).unwrap();
}

#[test]
fn nested_v3_decoder_has_exact_resource_boundaries_and_typed_errors() {
    let mut work = Work::new(1_000_000);
    let mut budget = Budget::new(&mut work, 1_000_000);
    let p = policy(&mut budget);
    let floor = 19 + Cap::FILE_STORAGE;
    let peak = floor + Cap::IO_STORAGE + POLICY_STORAGE;
    for case in 0..3 {
        let limit = Cap::IO_WORK + POLICY_WORK - usize::from(case == 0);
        let (result, accepted, live, observed_peak) =
            run(floor, limit, peak - usize::from(case == 1), |b| {
                Cap::from_file(sealed(p.canonical_bytes()), b)
            });
        assert_eq!(live, floor);
        match case {
            0 => {
                assert!(matches!(
                    failure(result),
                    Error::PolicyV3(PolicyError::Resource(Resource::Work(_)))
                ));
                assert_eq!(accepted, Cap::IO_WORK + ENTRY_WORK);
            }
            1 => {
                assert!(matches!(
                    failure(result),
                    Error::PolicyV3(PolicyError::Resource(Resource::Storage(_)))
                ));
                assert_eq!(accepted, Cap::IO_WORK + POLICY_WORK);
            }
            _ => {
                let (cap, delta) = result.unwrap();
                assert_eq!(
                    delta.additional_storage(),
                    cap.retained_storage() - Cap::FILE_STORAGE
                );
                assert_eq!(observed_peak, peak);
            }
        }
    }
    let legacy = policy_v2(7);
    let error = failure(
        run(floor, 1_000_000, 1_000_000, |b| {
            Cap::from_file(sealed(legacy.canonical_bytes()), b)
        })
        .0,
    );
    assert!(matches!(&error, Error::PolicyV3(PolicyError::Framing(_))));
    assert!(error.source().unwrap().is::<PolicyError>());
    let reverse = failure(
        run(floor, 1_000_000, 1_000_000, |b| {
            crate::CompilerExecutionPolicyCapabilityV2::from_file(sealed(p.canonical_bytes()), b)
        })
        .0,
    );
    assert!(matches!(reverse, Error::Policy(_)));
}

#[test]
fn policy_outer_admission_floors_and_v2_quotas_remain_fixed() {
    let mut work = Work::new(1_000_000);
    let mut budget = Budget::new(&mut work, 1_000_000);
    let p = policy(&mut budget);
    let floor = p.retained_storage();
    transfer_boundaries(floor, Cap::IO_WORK, Cap::IO_STORAGE, |b| {
        let mut fixture_work = Work::new(1_000_000);
        let mut fixture_budget = Budget::new(&mut fixture_work, 1_000_000);
        let p = policy(&mut fixture_budget);
        Cap::create(p, b).map(|(cap, delta)| {
            assert_eq!(delta.additional_storage(), cap.retained_storage() - floor);
        })
    });
    type V2 = crate::CompilerExecutionPolicyCapabilityV2;
    assert_eq!(Cap::FILE_STORAGE, V2::FILE_STORAGE);
    assert_eq!(Cap::IO_WORK, V2::IO_WORK);
    assert_eq!(Cap::IO_STORAGE, V2::IO_STORAGE);
}

use super::*;
use crate::native_capability::{
    CompilerExecutionCapabilityErrorV2 as Error,
    tests::{failure, legacy_policy, run, sealed, transfer_boundaries},
};
use fe2o3_compiler_execution_protocol::{
    CompilerExecutionClientProcessIdentityV1 as Client,
    CompilerExecutionExternalAnchorServiceIdentityV1 as Service,
};
use fe2o3_kernel_ir::{
    CanonicalKernelIrVerificationResourceErrorV1 as Resource, CanonicalKernelIrWorkBudgetV1 as Work,
};
use std::os::{fd::AsRawFd, unix::fs::MetadataExt};

fn policy(seed: u8) -> Policy {
    let p = legacy_policy(seed);
    let mut work = Work::new(100_000);
    let mut b = Budget::new(&mut work, 100_000);
    Policy::new(
        p.generation(),
        p.executable(),
        p.runtime(),
        *p.verifying_key(),
        *p.external_anchor_verifying_key(),
        &mut b,
    )
    .unwrap()
    .0
}

fn manifest() -> Manifest {
    let p = policy(7);
    let mut w = Work::new(WORK);
    let mut b = Budget::new(&mut w, 100_000);
    b.reserve_storage(p.retained_storage()).unwrap();
    Manifest::new(
        Client::new(1234, 5678, 9012).unwrap(),
        Service::new(6001, 7001).unwrap(),
        &p,
        &mut b,
    )
    .unwrap()
    .0
}

#[test]
fn native_manifest_survives_sealed_transfer_and_borrowed_inheritance() {
    let m = manifest();
    let expected = *m.canonical_bytes();
    let mut w = Work::new(1_000_000);
    let mut b = Budget::new(&mut w, 1_000_000);
    b.reserve_storage(m.retained_storage()).unwrap();
    let (cap, charge) = Cap::create(m, &mut b).unwrap();
    b.reserve_storage(charge.additional_storage()).unwrap();
    assert_eq!(b.storage(), cap.retained_storage());
    let (file, charge) = cap.try_clone_for_transfer(&mut b).unwrap();
    b.reserve_storage(charge.additional_storage()).unwrap();
    let inode = file.metadata().unwrap().ino();
    let floor = b.storage();
    cap.validate_transfer(&file, &mut b).unwrap();
    assert_eq!(b.storage(), floor);
    transfer_boundaries(floor, Capability::IO_WORK, Capability::IO_STORAGE, |b| {
        cap.validate_transfer(&file, b)
    });
    let denied = Cap::from_inherited_at(file.as_raw_fd(), &mut b);
    assert!(denied.is_err());
    assert_eq!(b.storage(), floor);
    rustix::io::fcntl_setfd(&file, rustix::io::FdFlags::empty()).unwrap();
    let (inherited, charge) = Cap::from_inherited_at(file.as_raw_fd(), &mut b).unwrap();
    assert_eq!(charge.additional_storage(), inherited.retained_storage());
    b.reserve_storage(charge.additional_storage()).unwrap();
    inherited.revalidate(&mut b).unwrap();
    assert_eq!(inherited.manifest().canonical_bytes(), &expected);
    assert_eq!(file.metadata().unwrap().ino(), inode);
    assert!(rustix::io::fcntl_getfd(&file).unwrap().is_empty());
    drop(inherited);
    b.release_storage(Capability::RETAINED).unwrap();
    rustix::io::fcntl_setfd(&file, rustix::io::FdFlags::CLOEXEC).unwrap();
    drop(cap);
    b.release_storage(Capability::RETAINED).unwrap();
    let (recovered, charge) = Cap::from_file(file, &mut b).unwrap();
    b.reserve_storage(charge.additional_storage()).unwrap();
    assert_eq!(b.storage(), recovered.retained_storage());
    assert_eq!(recovered.manifest().canonical_bytes(), &expected);
    drop(recovered);
    b.release_storage(Capability::RETAINED).unwrap();
    assert_eq!(b.storage(), 0);
}

#[test]
fn launch_admission_nested_decoder_uses_exact_shared_budget() {
    let bytes = *manifest().canonical_bytes();
    let floor = Capability::FILE_STORAGE;
    let peak = floor + Capability::IO_STORAGE + SCRATCH;
    for mode in 0..3 {
        let (result, work, live, observed_peak) = run(
            floor,
            Capability::IO_WORK + WORK - usize::from(mode == 0),
            peak - usize::from(mode == 1),
            |b| Cap::from_file(sealed(&bytes), b),
        );
        assert_eq!(live, floor);
        match mode {
            0 => {
                assert!(matches!(
                    failure(result),
                    LaunchError(ManifestError::Resource(Resource::Work(_)))
                ));
                assert_eq!(work, Capability::IO_WORK + 8);
            }
            1 => {
                assert!(matches!(
                    failure(result),
                    LaunchError(ManifestError::Resource(Resource::Storage(_)))
                ));
                assert_eq!(work, Capability::IO_WORK + WORK);
            }
            _ => {
                let (cap, delta) = result.unwrap();
                assert_eq!(delta.additional_storage() + floor, cap.retained_storage());
                assert_eq!(observed_peak, peak);
            }
        }
    }
}

#[test]
fn structural_admission_does_not_claim_a_native_policy_match() {
    let p = crate::native_capability::tests::legacy_policy(7);
    let old = fe2o3_compiler_execution_protocol::CompilerExecutionServiceLaunchManifestV1::new(
        Client::new(1234, 5678, 9012).unwrap(),
        Service::new(6001, 7001).unwrap(),
        &p,
    );
    let p = policy(7);
    let mut w = Work::new(1_000_000);
    let mut b = Budget::new(&mut w, 1_000_000);
    b.reserve_storage(Capability::FILE_STORAGE + p.retained_storage())
        .unwrap();
    let (cap, delta) = Cap::from_file(sealed(old.canonical_bytes()), &mut b).unwrap();
    b.reserve_storage(delta.additional_storage()).unwrap();
    assert!(!cap.manifest().matches_policy(&p, &mut b).unwrap());
    let mut bad = *old.canonical_bytes();
    bad[80] ^= 1;
    b.reserve_storage(Capability::FILE_STORAGE).unwrap();
    assert!(matches!(
        failure(Cap::from_file(sealed(&bad), &mut b)),
        LaunchError(_)
    ));
}

#[test]
fn identical_launch_bytes_do_not_allow_sealed_object_replacement() {
    let m = manifest();
    let bytes = *m.canonical_bytes();
    let (result, _, _, _) = run(m.retained_storage(), 1_000_000, 1_000_000, |b| {
        Cap::create(m, b)
    });
    let (mut cap, _) = result.unwrap();
    let (transfer, _) = run(cap.retained_storage(), 1_000_000, 1_000_000, |b| {
        cap.try_clone_for_transfer(b)
    })
    .0
    .unwrap();
    let replacement = sealed(&bytes);
    let floor = cap.retained_storage() + 2 * Capability::FILE_STORAGE;
    let (result, _, live, _) = run(floor, Capability::IO_WORK, 1_000_000, |b| {
        cap.validate_transfer(&replacement, b)
    });
    assert!(matches!(
        failure(result),
        Error::Rejected("sealed image identity or length changed")
    ));
    assert_eq!(live, floor);
    drop(replacement);
    cap.0.image.replace_file_for_test(sealed(&bytes));
    let (result, _, live, _) = run(cap.retained_storage(), 1_000_000, 1_000_000, |b| {
        cap.revalidate(b)
    });
    assert!(result.is_err());
    assert_eq!(live, cap.retained_storage());
    let floor = cap.retained_storage() + Capability::FILE_STORAGE;
    let (result, _, live, _) = run(floor, Capability::IO_WORK, 1_000_000, |b| {
        cap.validate_transfer(&transfer, b)
    });
    assert!(matches!(
        failure(result),
        Error::Rejected("sealed image identity or length changed")
    ));
    assert_eq!(live, floor);
    assert_eq!(transfer.metadata().unwrap().len(), BYTES as u64);
}

#[test]
fn other_native_policy_is_not_admitted_by_a_shared_launch_wire() {
    let mut work = Work::new(2_000_000);
    let mut budget = Budget::new(&mut work, 1_000_000);
    let legacy = legacy_policy(7);
    let (other, charge) = OtherPolicy::new(
        legacy.generation(),
        legacy.executable(),
        legacy.runtime(),
        *legacy.verifying_key(),
        *legacy.external_anchor_verifying_key(),
        &mut budget,
    )
    .unwrap();
    budget.reserve_storage(charge.additional_storage()).unwrap();
    let (other_manifest, charge) = OtherManifest::new(
        Client::new(1234, 5678, 9012).unwrap(),
        Service::new(6001, 7001).unwrap(),
        &other,
        &mut budget,
    )
    .unwrap();
    budget.reserve_storage(charge.additional_storage()).unwrap();
    let (policy, charge) = Policy::new(
        legacy.generation(),
        legacy.executable(),
        legacy.runtime(),
        *legacy.verifying_key(),
        *legacy.external_anchor_verifying_key(),
        &mut budget,
    )
    .unwrap();
    budget.reserve_storage(charge.additional_storage()).unwrap();
    let ledger = budget.work_ledger_identity_v1();
    budget.reserve_storage(Cap::FILE_STORAGE).unwrap();
    let (cap, charge) =
        Cap::from_file(sealed(other_manifest.canonical_bytes()), &mut budget).unwrap();
    budget.reserve_storage(charge.additional_storage()).unwrap();
    assert_eq!(
        cap.manifest().canonical_bytes(),
        other_manifest.canonical_bytes()
    );
    assert!(!cap.manifest().matches_policy(&policy, &mut budget).unwrap());
    assert!(budget.work_ledger_identity_v1() == ledger);
}

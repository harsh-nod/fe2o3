use ed25519_dalek::SigningKey;
use fe2o3_compiler_closure_capability::CompilerExecutionCapabilityErrorV2 as Error;
use fe2o3_compiler_execution_protocol::{
    CompilerExecutionExternalAnchorServiceIdentityV1 as Service,
    CompilerExecutionIssuerMeasurementV1 as Measurement,
    CompilerExecutionIssuerPolicyV1 as OldPolicy,
    CompilerExecutionSupervisorDeploymentV1 as OldDeployment,
};
use fe2o3_kernel_ir::{
    CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
    CanonicalKernelIrVerificationResourceErrorV1 as Resource,
    CanonicalKernelIrWorkBudgetV1 as Work,
};
use rustix::fs::{Mode, SealFlags};
use std::{
    fs::File,
    os::{fd::AsRawFd, unix::fs::MetadataExt},
};

const LIMIT: usize = 4_000_000;
const EXTRA: usize = 19;
const ENTRY: usize = 8;
const SEALS: SealFlags = SealFlags::WRITE
    .union(SealFlags::GROW)
    .union(SealFlags::SHRINK)
    .union(SealFlags::SEAL);

fn measurement(seed: u8, len: u64) -> Measurement {
    Measurement::new([seed; 32], len).unwrap()
}
fn key(seed: u8) -> [u8; 32] {
    SigningKey::from_bytes(&[seed; 32])
        .verifying_key()
        .to_bytes()
}
fn policy(axis: u8, b: &mut Budget<'_>) -> Policy {
    let (p, d) = Policy::new(
        if axis == 1 { 8 } else { 7 },
        measurement(if axis == 2 { 21 } else { 11 }, 123),
        measurement(if axis == 3 { 22 } else { 12 }, 456),
        key(if axis == 4 { 17 } else { 7 }),
        key(if axis == 5 { 18 } else { 8 }),
        b,
    )
    .unwrap();
    b.reserve_storage(d.additional_storage()).unwrap();
    p
}
fn deployment(p: &Policy, b: &mut Budget<'_>) -> Deployment {
    let (d, s) = Deployment::new(
        1001,
        1002,
        Service::new(1003, 1004).unwrap(),
        measurement(31, 4096),
        measurement(32, 8192),
        p,
        b,
    )
    .unwrap();
    b.reserve_storage(s.additional_storage()).unwrap();
    d
}
fn sealed(bytes: &[u8]) -> File {
    let file = File::from(
        rustix::fs::memfd_create(
            "native-deployment-test",
            rustix::fs::MemfdFlags::CLOEXEC | rustix::fs::MemfdFlags::ALLOW_SEALING,
        )
        .unwrap(),
    );
    assert_eq!(rustix::io::pwrite(&file, bytes, 0).unwrap(), bytes.len());
    rustix::fs::fchmod(&file, Mode::RUSR).unwrap();
    rustix::fs::fcntl_add_seals(&file, SEALS).unwrap();
    file
}
fn object(file: &File) -> (u64, u64) {
    let m = file.metadata().unwrap();
    (m.dev(), m.ino())
}
fn references(witness: &File) -> usize {
    let identity = object(witness);
    std::fs::read_dir("/proc/self/fd")
        .unwrap()
        .filter_map(|entry| std::fs::metadata(entry.ok()?.path()).ok())
        .filter(|m| (m.dev(), m.ino()) == identity)
        .count()
}
fn failure<T>(result: Result<T, Error>) -> Error {
    match result {
        Err(error) => error,
        Ok(_) => panic!("unexpected acceptance"),
    }
}

#[test]
fn ownership_chain_preserves_context_inode_offsets_and_complete_reservations() {
    let mut w = Work::new(LIMIT);
    let mut b = Budget::new(&mut w, LIMIT);
    b.reserve_storage(EXTRA).unwrap();
    let p = policy(0, &mut b);
    let d = deployment(&p, &mut b);
    let expected = *d.canonical_bytes();
    let record_charge = d.retained_storage();
    let before = b.work();
    let ledger = b.work_ledger_identity_v1();
    let (cap, growth) = Cap::create(d, &mut b).unwrap();
    assert_eq!(
        record_charge + growth.additional_storage(),
        cap.retained_storage()
    );
    b.reserve_storage(growth.additional_storage()).unwrap();
    let cap_charge = cap.retained_storage();
    let (file, delta) = cap.try_clone_for_transfer(&mut b).unwrap();
    assert_eq!(delta.additional_storage(), Cap::FILE_STORAGE);
    b.reserve_storage(delta.additional_storage()).unwrap();
    b.reserve_storage(Cap::FILE_STORAGE).unwrap();
    let witness = file.try_clone().unwrap();
    assert_eq!(references(&witness), 3);
    assert_eq!(file.metadata().unwrap().mode(), libc::S_IFREG | 0o400);
    assert_eq!(rustix::fs::fcntl_get_seals(&file).unwrap(), SEALS);
    rustix::fs::seek(&file, rustix::fs::SeekFrom::Start(17)).unwrap();
    cap.validate_transfer(&file, &mut b).unwrap();
    assert_eq!(
        rustix::fs::seek(&file, rustix::fs::SeekFrom::Current(0)).unwrap(),
        17
    );
    assert_eq!(
        rustix::io::pwrite(&file, &[0], 0),
        Err(rustix::io::Errno::PERM)
    );

    rustix::io::fcntl_setfd(&file, rustix::io::FdFlags::empty()).unwrap();
    let (inherited, full) = Cap::from_inherited_at(file.as_raw_fd(), &p, &mut b).unwrap();
    assert_eq!(full.additional_storage(), cap_charge);
    b.reserve_storage(full.additional_storage()).unwrap();
    assert_eq!(references(&witness), 4);
    assert_eq!(inherited.deployment().canonical_bytes(), &expected);
    assert_eq!(
        rustix::io::fcntl_getfd(&file).unwrap(),
        rustix::io::FdFlags::empty()
    );
    inherited.revalidate(&mut b).unwrap();
    drop(inherited);
    b.release_storage(cap_charge).unwrap();
    rustix::io::fcntl_setfd(&file, rustix::io::FdFlags::CLOEXEC).unwrap();
    drop(cap);
    b.release_storage(cap_charge).unwrap();
    let (recovered, growth) = Cap::from_file(file, &p, &mut b).unwrap();
    assert_eq!(growth.additional_storage() + Cap::FILE_STORAGE, cap_charge);
    b.reserve_storage(growth.additional_storage()).unwrap();
    assert_eq!(references(&witness), 2);
    assert_eq!(recovered.deployment().canonical_bytes(), &expected);
    assert_eq!(
        b.work(),
        before + 4 * Cap::IO_WORK + 2 * Cap::ADMISSION_WORK
    );
    assert!(b.work_ledger_identity_v1() == ledger);
    assert_eq!(
        b.storage(),
        EXTRA + p.retained_storage() + cap_charge + Cap::FILE_STORAGE
    );
    drop((recovered, witness));
    b.release_storage(cap_charge + Cap::FILE_STORAGE).unwrap();
    let policy_charge = p.retained_storage();
    drop(p);
    b.release_storage(policy_charge).unwrap();
    assert_eq!(b.storage(), EXTRA);
}

#[test]
fn every_actual_policy_axis_is_required_when_recovering_the_file() {
    let mut w = Work::new(LIMIT);
    let mut b = Budget::new(&mut w, LIMIT);
    let pinned = policy(0, &mut b);
    let d = deployment(&pinned, &mut b);
    for axis in 1..=5 {
        let wrong = policy(axis, &mut b);
        b.reserve_storage(2 * Cap::FILE_STORAGE).unwrap();
        let file = sealed(d.canonical_bytes());
        let witness = file.try_clone().unwrap();
        let (floor, before) = (b.storage(), b.work());
        let error = failure(Cap::from_file(file, &wrong, &mut b));
        assert!(matches!(decode_error(&error), DecodeError::PolicyMismatch));
        assert_eq!(
            (b.storage(), b.work()),
            (floor, before + Cap::ADMISSION_WORK)
        );
        assert_eq!(references(&witness), 1);
        let retired = 2 * Cap::FILE_STORAGE + wrong.retained_storage();
        drop((witness, wrong));
        b.release_storage(retired).unwrap();
    }
}

#[test]
fn legacy_foreign_and_resealed_foreign_policy_bindings_never_recover() {
    use sha2::{Digest, Sha256};
    let mut w = Work::new(LIMIT);
    let mut b = Budget::new(&mut w, LIMIT);
    let p = policy(0, &mut b);
    let old_policy = OldPolicy::new(
        7,
        measurement(11, 123),
        measurement(12, 456),
        key(7),
        key(8),
    )
    .unwrap();
    let old = OldDeployment::new(
        1001,
        1002,
        Service::new(1003, 1004).unwrap(),
        measurement(31, 4096),
        measurement(32, 8192),
        &old_policy,
    )
    .unwrap();
    let (other_policy, charge) = OtherPolicy::new(
        7,
        measurement(11, 123),
        measurement(12, 456),
        key(7),
        key(8),
        &mut b,
    )
    .unwrap();
    b.reserve_storage(charge.additional_storage()).unwrap();
    let (other, charge) = OtherDeployment::new(
        1001,
        1002,
        Service::new(1003, 1004).unwrap(),
        measurement(31, 4096),
        measurement(32, 8192),
        &other_policy,
        &mut b,
    )
    .unwrap();
    b.reserve_storage(charge.additional_storage()).unwrap();
    let mut relabeled = *other.canonical_bytes();
    relabeled[..8].copy_from_slice(MAGIC);
    relabeled[8..10].copy_from_slice(&VERSION.to_le_bytes());
    let mut hash = Sha256::new();
    hash.update(DOMAIN);
    hash.update(((BYTES - 32) as u64).to_le_bytes());
    hash.update(&relabeled[..BYTES - 32]);
    relabeled[BYTES - 32..].copy_from_slice(&hash.finalize());
    for (case, bytes) in [old.canonical_bytes(), other.canonical_bytes(), &relabeled]
        .into_iter()
        .enumerate()
    {
        b.reserve_storage(Cap::FILE_STORAGE).unwrap();
        let error = failure(Cap::from_file(sealed(bytes), &p, &mut b));
        if case == 2 {
            assert!(matches!(decode_error(&error), DecodeError::PolicyMismatch));
        } else {
            assert!(matches!(decode_error(&error), DecodeError::Framing(_)));
        }
        b.release_storage(Cap::FILE_STORAGE).unwrap();
    }
}

#[test]
fn contextual_decode_boundaries_close_consumed_images_and_keep_exact_history() {
    let mut setup_work = Work::new(LIMIT);
    let mut setup = Budget::new(&mut setup_work, LIMIT);
    let p = policy(0, &mut setup);
    let d = deployment(&p, &mut setup);
    let input = Cap::FILE_STORAGE + p.retained_storage();
    for mode in 0..7 {
        let floor = if mode == 1 { input - 1 } else { input };
        let work_limit = match mode {
            0 => ENTRY - 1,
            2 => Cap::IO_WORK - 1,
            4 => Cap::ADMISSION_WORK - 1,
            _ => Cap::ADMISSION_WORK,
        };
        let scratch = match mode {
            3 => Cap::IO_STORAGE - 1,
            5 => Cap::ADMISSION_STORAGE - 1,
            _ => Cap::ADMISSION_STORAGE,
        };
        let mut w = Work::new(work_limit);
        let mut b = Budget::new(&mut w, floor + scratch);
        b.reserve_storage(floor).unwrap();
        let file = sealed(d.canonical_bytes());
        let witness = file.try_clone().unwrap(); // External test witness, not an operation input.
        let ledger = b.work_ledger_identity_v1();
        let result = Cap::from_file(file, &p, &mut b);
        assert_eq!(b.storage(), floor, "mode {mode}");
        assert!(b.work_ledger_identity_v1() == ledger);
        if mode == 6 {
            let (cap, delta) = result.unwrap();
            assert_eq!(b.work(), Cap::ADMISSION_WORK);
            assert_eq!(b.peak_storage(), floor + Cap::ADMISSION_STORAGE);
            assert_eq!(
                delta.additional_storage() + Cap::FILE_STORAGE,
                cap.retained_storage()
            );
            b.reserve_storage(delta.additional_storage()).unwrap();
            let retained = cap.retained_storage();
            drop(cap);
            b.release_storage(retained).unwrap();
            assert_eq!(b.storage(), floor - Cap::FILE_STORAGE);
        } else {
            let error = failure(result);
            match mode {
                0 | 2 => {
                    assert!(matches!(error, Error::Resource(Resource::Work(_))));
                    assert_eq!(b.work(), if mode == 0 { 0 } else { ENTRY });
                    assert_eq!(
                        b.failed_work(),
                        Some(if mode == 0 { ENTRY } else { Cap::IO_WORK })
                    );
                    assert_eq!(b.peak_storage(), floor);
                }
                1 => {
                    assert!(matches!(error, Error::Resource(Resource::Accounting)));
                    assert_eq!((b.work(), b.peak_storage()), (ENTRY, floor));
                }
                3 => {
                    assert!(matches!(error, Error::Resource(Resource::Storage(_))));
                    assert_eq!((b.work(), b.peak_storage()), (Cap::IO_WORK, floor));
                    assert_eq!(b.failed_storage(), Some(floor + Cap::IO_STORAGE));
                }
                4 => {
                    assert!(matches!(
                        decode_error(&error),
                        DecodeError::Resource(Resource::Work(_))
                    ));
                    assert_eq!(b.work(), Cap::IO_WORK + ENTRY);
                    assert_eq!(b.failed_work(), Some(Cap::ADMISSION_WORK));
                    assert_eq!(b.peak_storage(), floor + Cap::IO_STORAGE);
                }
                5 => {
                    assert!(matches!(
                        decode_error(&error),
                        DecodeError::Resource(Resource::Storage(_))
                    ));
                    assert_eq!(b.work(), Cap::ADMISSION_WORK);
                    assert_eq!(b.failed_storage(), Some(floor + Cap::ADMISSION_STORAGE));
                    assert_eq!(b.peak_storage(), floor + Cap::IO_STORAGE);
                }
                _ => unreachable!(),
            }
        }
        assert_eq!(references(&witness), 1, "mode {mode}");
    }
    assert_eq!(Cap::ADMISSION_WORK, Cap::IO_WORK + DECODE_WORK);
    assert_eq!(Cap::ADMISSION_STORAGE, Cap::IO_STORAGE + DECODE_STORAGE);
}

#[test]
fn inherited_refusals_preserve_the_original_slot_and_its_flags() {
    let mut setup_work = Work::new(LIMIT);
    let mut setup = Budget::new(&mut setup_work, LIMIT);
    let p = policy(0, &mut setup);
    let d = deployment(&p, &mut setup);
    let wrong = policy(1, &mut setup);
    let file = sealed(d.canonical_bytes());
    for mode in 0..4 {
        let mut w = Work::new(if mode == 1 { ENTRY - 1 } else { LIMIT });
        let mut b = Budget::new(&mut w, LIMIT);
        let chosen = if mode == 2 { &wrong } else { &p };
        b.reserve_storage(Cap::FILE_STORAGE + chosen.retained_storage())
            .unwrap();
        let flags = if mode == 0 {
            rustix::io::FdFlags::CLOEXEC
        } else {
            rustix::io::FdFlags::empty()
        };
        rustix::io::fcntl_setfd(&file, flags).unwrap();
        let fd = if mode == 3 { -1 } else { file.as_raw_fd() };
        let floor = b.storage();
        let error = failure(Cap::from_inherited_at(fd, chosen, &mut b));
        match mode {
            0 => assert!(matches!(
                error,
                Error::Rejected("inherited descriptor is close-on-exec")
            )),
            1 => assert!(matches!(error, Error::Resource(Resource::Work(_)))),
            2 => assert!(matches!(decode_error(&error), DecodeError::PolicyMismatch)),
            3 => assert!(matches!(
                error,
                Error::Rejected("inherited descriptor overlaps stdio")
            )),
            _ => unreachable!(),
        }
        assert_eq!(
            b.work(),
            match mode {
                1 => 0,
                2 => Cap::ADMISSION_WORK,
                _ => Cap::IO_WORK,
            }
        );
        assert_eq!(b.storage(), floor);
        assert_eq!(references(&file), 1);
        assert_eq!(rustix::io::fcntl_getfd(&file).unwrap(), flags);
    }
}

#[test]
fn transfers_require_the_original_object_and_recheck_mutable_metadata() {
    let mut w = Work::new(LIMIT);
    let mut b = Budget::new(&mut w, LIMIT);
    let p = policy(0, &mut b);
    let d = deployment(&p, &mut b);
    let wire = *d.canonical_bytes();
    let (cap, delta) = Cap::create(d, &mut b).unwrap();
    b.reserve_storage(delta.additional_storage()).unwrap();
    let (file, delta) = cap.try_clone_for_transfer(&mut b).unwrap();
    b.reserve_storage(delta.additional_storage()).unwrap();
    b.reserve_storage(Cap::FILE_STORAGE).unwrap();
    let other = sealed(&wire);
    assert_ne!(object(&file), object(&other));
    assert!(cap.validate_transfer(&other, &mut b).is_err());
    cap.validate_transfer(&file, &mut b).unwrap();
    rustix::io::fcntl_setfd(&file, rustix::io::FdFlags::empty()).unwrap();
    assert!(cap.validate_transfer(&file, &mut b).is_err());
    cap.revalidate(&mut b).unwrap();
    rustix::io::fcntl_setfd(&file, rustix::io::FdFlags::CLOEXEC).unwrap();
    rustix::fs::fchmod(&file, Mode::RUSR | Mode::WUSR).unwrap();
    assert!(cap.revalidate(&mut b).is_err());
    assert!(cap.try_clone_for_transfer(&mut b).is_err());
    rustix::fs::fchmod(&file, Mode::RUSR).unwrap();
    cap.validate_transfer(&file, &mut b).unwrap();
}

#[test]
fn contextual_admission_preserves_first_denials_across_success_and_later_refusal() {
    let mut setup_work = Work::new(LIMIT);
    let mut setup = Budget::new(&mut setup_work, LIMIT);
    let p = policy(0, &mut setup);
    let d = deployment(&p, &mut setup);
    let floor = p.retained_storage() + Cap::FILE_STORAGE + EXTRA;
    let limit = 2 * Cap::ADMISSION_WORK - 1;
    let mut w = Work::new(limit);
    let mut b = Budget::new(&mut w, floor + Cap::ADMISSION_STORAGE);
    b.reserve_storage(floor).unwrap();
    assert!(b.charge_work(limit + 1).is_err());
    assert!(b.reserve_storage(Cap::ADMISSION_STORAGE + 1).is_err());
    let ledger = b.work_ledger_identity_v1();
    for turn in 0..2 {
        let result = Cap::from_file(sealed(d.canonical_bytes()), &p, &mut b);
        assert_eq!(b.storage(), floor);
        if turn == 0 {
            drop(result.unwrap());
        } else {
            assert!(matches!(
                decode_error(&failure(result)),
                DecodeError::Resource(Resource::Work(_))
            ));
        }
        assert_eq!(b.failed_work(), Some(limit + 1));
        assert_eq!(b.failed_storage(), Some(floor + Cap::ADMISSION_STORAGE + 1));
        assert_eq!(b.peak_storage(), floor + Cap::ADMISSION_STORAGE);
        assert!(b.work_ledger_identity_v1() == ledger);
        b.release_storage(Cap::FILE_STORAGE).unwrap();
        if turn == 0 {
            b.reserve_storage(Cap::FILE_STORAGE).unwrap();
        }
    }
    assert_eq!(b.work(), Cap::ADMISSION_WORK + Cap::IO_WORK + ENTRY);
}

#[test]
fn malformed_descriptors_fail_before_contextual_decoding_and_close_the_input() {
    let mut setup_work = Work::new(LIMIT);
    let mut setup = Budget::new(&mut setup_work, LIMIT);
    let p = policy(0, &mut setup);
    let d = deployment(&p, &mut setup);
    for case in 0..5 {
        let file = match case {
            0 => sealed(&d.canonical_bytes()[..BYTES - 1]),
            1 => sealed(&[0; BYTES + 1]),
            4 => {
                let file = File::from(
                    rustix::fs::memfd_create(
                        "native-deployment-unsealed",
                        rustix::fs::MemfdFlags::CLOEXEC | rustix::fs::MemfdFlags::ALLOW_SEALING,
                    )
                    .unwrap(),
                );
                assert_eq!(
                    rustix::io::pwrite(&file, d.canonical_bytes(), 0).unwrap(),
                    BYTES
                );
                rustix::fs::fchmod(&file, Mode::RUSR).unwrap();
                file
            }
            _ => sealed(d.canonical_bytes()),
        };
        if case == 2 {
            rustix::io::fcntl_setfd(&file, rustix::io::FdFlags::empty()).unwrap();
        }
        if case == 3 {
            rustix::fs::fchmod(&file, Mode::RUSR | Mode::WUSR).unwrap();
        }
        let witness = file.try_clone().unwrap();
        let mut w = Work::new(LIMIT);
        let mut b = Budget::new(&mut w, LIMIT);
        let floor = Cap::FILE_STORAGE + p.retained_storage();
        b.reserve_storage(floor).unwrap();
        let error = failure(Cap::from_file(file, &p, &mut b));
        assert!(
            matches!(error, Error::Rejected(_)),
            "case {case}: {error:?}"
        );
        assert_eq!((b.storage(), b.work()), (floor, Cap::IO_WORK));
        assert_eq!(b.peak_storage(), floor + Cap::IO_STORAGE);
        assert_eq!(references(&witness), 1);
    }
}

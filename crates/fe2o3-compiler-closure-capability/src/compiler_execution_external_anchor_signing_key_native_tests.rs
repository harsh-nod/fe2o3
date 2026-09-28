use super::*;
use fe2o3_compiler_execution_protocol::{
    CompilerExecutionExternalAnchorServiceIdentityV1 as Service,
    CompilerExecutionIssuerMeasurementV1 as Measurement,
};
use fe2o3_external_anchor_protocol::{
    AnchorDecisionV1, AnchoredStateV1, CallerNonceV1, HashChainHeadV1, TransactionDigestV1,
};
use fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1 as Work;
use rustix::fs::{Mode, OFlags, SealFlags};
use std::{
    os::{fd::AsRawFd, unix::fs::MetadataExt},
    panic::{AssertUnwindSafe, catch_unwind},
};

const LIMIT: usize = 100_000_000;
const EXTRA: usize = 19;
fn public(seed: u8) -> [u8; 32] {
    SigningKey::from_bytes(&[seed; 32])
        .verifying_key()
        .to_bytes()
}
fn measure(seed: u8) -> Measurement {
    Measurement::new([seed; 32], 4096).unwrap()
}
fn credentials() -> (u32, u32) {
    let pair = (
        rustix::process::geteuid().as_raw(),
        rustix::process::getegid().as_raw(),
    );
    assert!(
        pair.0 != 0 && pair.1 != 0,
        "rootless mechanical fixtures require nonroot credentials"
    );
    pair
}
fn deployment(axis: u8, b: &mut Budget<'_>) -> Deployment {
    let (uid, gid) = credentials();
    let (p, c) = Policy::new(
        if axis == 1 { 8 } else { 7 },
        measure(1),
        measure(2),
        public(3),
        public(if axis == 2 { 8 } else { 7 }),
        b,
    )
    .unwrap();
    b.reserve_storage(c.additional_storage()).unwrap();
    let (s, c) = Supervisor::new(
        uid + 10,
        gid + 10,
        Service::new(uid + u32::from(axis == 3), gid + u32::from(axis == 4)).unwrap(),
        measure(if axis == 5 { 6 } else { 5 }),
        measure(4),
        &p,
        b,
    )
    .unwrap();
    b.reserve_storage(c.additional_storage()).unwrap();
    let (d, c) = Deployment::new(&s, &p, measure(if axis == 6 { 9 } else { 8 }), b).unwrap();
    b.reserve_storage(c.additional_storage()).unwrap();
    let retired = p.retained_storage() + s.retained_storage();
    drop((p, s));
    b.release_storage(retired).unwrap();
    d
}
fn key(d: &Deployment, b: &mut Budget<'_>) -> Cap {
    b.reserve_storage(32).unwrap();
    let mut seed = [7; 32];
    let (cap, c) = Cap::create_and_zeroize(&mut seed, d, b).unwrap();
    assert_eq!(seed, [0; 32]);
    b.reserve_storage(c.additional_storage()).unwrap();
    b.release_storage(32).unwrap();
    cap
}
fn wire(d: &Deployment, seed: u8) -> [u8; 88] {
    let mut bytes = [0; 88];
    bytes[..8].copy_from_slice(if VERSION == 2 {
        b"F2O3EAK2"
    } else {
        b"F2O3EAK3"
    });
    bytes[8..10].copy_from_slice(&VERSION.to_le_bytes());
    bytes[12..16].copy_from_slice(&88u32.to_le_bytes());
    bytes[24..56].copy_from_slice(d.identity().as_bytes());
    bytes[56..].fill(seed);
    bytes
}
fn image(bytes: &[u8], readonly: bool) -> File {
    let f = File::from(
        rustix::fs::memfd_create(
            "anchor-key-test",
            rustix::fs::MemfdFlags::CLOEXEC | rustix::fs::MemfdFlags::ALLOW_SEALING,
        )
        .unwrap(),
    );
    assert_eq!(rustix::io::pwrite(&f, bytes, 0).unwrap(), bytes.len());
    rustix::fs::fchmod(&f, Mode::RUSR).unwrap();
    rustix::fs::fcntl_add_seals(
        &f,
        SealFlags::WRITE | SealFlags::SHRINK | SealFlags::GROW | SealFlags::SEAL,
    )
    .unwrap();
    if readonly {
        File::open(format!("/proc/self/fd/{}", f.as_raw_fd())).unwrap()
    } else {
        f
    }
}
fn refs(f: &File) -> usize {
    let m = f.metadata().unwrap();
    std::fs::read_dir("/proc/self/fd")
        .unwrap()
        .filter_map(|e| std::fs::metadata(e.ok()?.path()).ok())
        .filter(|other| other.dev() == m.dev() && other.ino() == m.ino())
        .count()
}
fn failure<T>(r: Result<T>) -> Error {
    match r {
        Err(e) => e,
        Ok(_) => panic!("unexpected acceptance"),
    }
}

#[test]
fn exact_native_key_roundtrip_preserves_object_flags_offsets_and_charges() {
    let mut w = Work::new(LIMIT);
    let mut b = Budget::new(&mut w, LIMIT);
    b.reserve_storage(EXTRA).unwrap();
    let d = deployment(0, &mut b);
    let cap = key(&d, &mut b);
    assert_eq!(cap.verifying_key(), public(7));
    assert_eq!(cap.deployment_identity(), d.identity());
    assert!(!format!("{cap:?}").contains(&format!("{:?}", [7; 32])));
    let retained = cap.retained_storage();
    let (f, c) = cap.try_clone_for_transfer(&mut b).unwrap();
    assert_eq!(c.additional_storage(), Cap::FILE_STORAGE);
    b.reserve_storage(c.additional_storage()).unwrap();
    assert_eq!(
        rustix::fs::fcntl_getfl(&f).unwrap() & OFlags::ACCMODE,
        OFlags::RDONLY
    );
    let mut bytes = [0; 88];
    assert_eq!(rustix::io::pread(&f, &mut bytes, 0).unwrap(), 88);
    assert_eq!(bytes, wire(&d, 7));
    assert_eq!(f.metadata().unwrap().mode(), libc::S_IFREG | 0o400);
    rustix::fs::seek(&f, rustix::fs::SeekFrom::Start(17)).unwrap();
    cap.validate_transfer(&f, &d, &mut b).unwrap();
    assert_eq!(
        rustix::fs::seek(&f, rustix::fs::SeekFrom::Current(0)).unwrap(),
        17
    );
    assert_eq!(
        rustix::io::pwrite(&f, &[0], 0),
        Err(rustix::io::Errno::BADF)
    );
    rustix::io::fcntl_setfd(&f, rustix::io::FdFlags::empty()).unwrap();
    let floor = b.storage();
    let (inherited, c) = Cap::from_inherited_at(f.as_raw_fd(), &d, &mut b).unwrap();
    assert_eq!(b.storage(), floor);
    assert_eq!(c.additional_storage(), retained);
    b.reserve_storage(c.additional_storage()).unwrap();
    assert_eq!(refs(&f), 3);
    assert_eq!(
        rustix::io::fcntl_getfd(&f).unwrap(),
        rustix::io::FdFlags::empty()
    );
    drop(inherited);
    b.release_storage(retained).unwrap();
    rustix::io::fcntl_setfd(&f, rustix::io::FdFlags::CLOEXEC).unwrap();
    drop(cap);
    b.release_storage(retained).unwrap();
    let floor = b.storage();
    let (cap, c) = Cap::from_file(f, &d, &mut b).unwrap();
    assert_eq!(b.storage(), floor);
    assert_eq!(c.additional_storage() + Cap::FILE_STORAGE, retained);
    b.reserve_storage(c.additional_storage()).unwrap();
    cap.revalidate(&d, &mut b).unwrap();
    let full = retained + d.retained_storage();
    drop((cap, d));
    b.release_storage(full).unwrap();
    assert_eq!(b.storage(), EXTRA);
}

#[test]
fn all_wire_bytes_and_foreign_roles_refuse_with_consumed_descriptor_cleanup() {
    let mut w = Work::new(LIMIT);
    let mut b = Budget::new(&mut w, LIMIT);
    let d = deployment(0, &mut b);
    let original = wire(&d, 7);
    for offset in 0..88 {
        let mut bytes = original;
        bytes[offset] ^= 1;
        let f = image(&bytes, true);
        let witness = f.try_clone().unwrap();
        b.reserve_storage(Cap::FILE_STORAGE).unwrap();
        let (floor, before) = (b.storage(), b.work());
        assert!(matches!(
            failure(Cap::from_file(f, &d, &mut b)),
            Error::Rejected(_)
        ));
        assert_eq!(
            (b.storage(), b.work()),
            (floor, before + Cap::ADMISSION_WORK)
        );
        assert_eq!(refs(&witness), 1);
        b.release_storage(Cap::FILE_STORAGE).unwrap();
    }
    for version in [1u16, if VERSION == 2 { 3 } else { 2 }] {
        let mut bytes = original;
        bytes[7] = b'0' + version as u8;
        bytes[8..10].copy_from_slice(&version.to_le_bytes());
        b.reserve_storage(Cap::FILE_STORAGE).unwrap();
        assert!(matches!(
            failure(Cap::from_file(image(&bytes, true), &d, &mut b)),
            Error::Rejected(_)
        ));
        b.release_storage(Cap::FILE_STORAGE).unwrap();
    }
    for bytes in [original[..87].to_vec(), [7; 32].to_vec(), [0; 89].to_vec()] {
        b.reserve_storage(Cap::FILE_STORAGE).unwrap();
        assert!(matches!(
            failure(Cap::from_file(image(&bytes, true), &d, &mut b)),
            Error::Rejected(_)
        ));
        b.release_storage(Cap::FILE_STORAGE).unwrap();
    }
}

#[test]
fn every_deployment_axis_binds_recovery_revalidation_and_key_use() {
    let mut w = Work::new(LIMIT);
    let mut b = Budget::new(&mut w, LIMIT);
    let d = deployment(0, &mut b);
    let cap = key(&d, &mut b);
    for axis in 1..=6 {
        let wrong = deployment(axis, &mut b);
        let charge = wrong.retained_storage();
        assert!(matches!(
            cap.revalidate(&wrong, &mut b),
            Err(Error::Rejected(_))
        ));
        b.reserve_storage(Cap::FILE_STORAGE).unwrap();
        assert!(matches!(
            failure(Cap::from_file(image(&wire(&d, 7), true), &wrong, &mut b)),
            Error::Rejected(_)
        ));
        b.release_storage(Cap::FILE_STORAGE).unwrap();
        drop(wrong);
        b.release_storage(charge).unwrap();
    }
    b.reserve_storage(32).unwrap();
    let mut wrong_seed = [8; 32];
    assert!(matches!(
        failure(Cap::create_and_zeroize(&mut wrong_seed, &d, &mut b)),
        Error::Rejected(_)
    ));
    assert_eq!(wrong_seed, [0; 32]);
}

#[test]
fn native_signatures_verify_with_existing_anchor_state_machine_without_key_export() {
    let mut w = Work::new(LIMIT);
    let mut b = Budget::new(&mut w, LIMIT);
    let d = deployment(0, &mut b);
    let cap = key(&d, &mut b);
    let pin = PinnedAnchorKeyV1::from_bytes(public(7)).unwrap();
    for position in [AnchorPositionV1::Prior, AnchorPositionV1::Proposed] {
        let pending = AnchoredStateV1::from_local_state(3, HashChainHeadV1::from_bytes([1; 32]))
            .prepare(TransactionDigestV1::from_bytes([2; 32]), &pin)
            .unwrap()
            .begin_advance(CallerNonceV1::from_bytes([3; 32]), &pin)
            .unwrap();
        b.reserve_storage(size_of::<AnchorChallengeV1>()).unwrap();
        let (before, floor, ledger) = (b.work(), b.storage(), b.work_ledger_identity_v1());
        let (signed, c) = cap
            .sign_observation(&d, pending.challenge(), position, &mut b)
            .unwrap();
        assert_eq!(
            (b.work(), b.storage()),
            (before + Cap::OBSERVATION_WORK, floor)
        );
        assert!(ledger == b.work_ledger_identity_v1());
        assert_eq!(c.additional_storage(), size_of::<([u8; 288], Storage)>());
        b.reserve_storage(c.additional_storage()).unwrap();
        assert_eq!(
            matches!(
                pending.verify(&signed).unwrap(),
                AnchorDecisionV1::Commit(_)
            ),
            position == AnchorPositionV1::Proposed
        );
        b.release_storage(c.additional_storage() + size_of::<AnchorChallengeV1>())
            .unwrap();
    }
    let wrong = PinnedAnchorKeyV1::from_bytes(public(8)).unwrap();
    let pending = AnchoredStateV1::from_local_state(3, HashChainHeadV1::from_bytes([1; 32]))
        .prepare(TransactionDigestV1::from_bytes([2; 32]), &wrong)
        .unwrap()
        .begin_advance(CallerNonceV1::from_bytes([3; 32]), &wrong)
        .unwrap();
    b.reserve_storage(size_of::<AnchorChallengeV1>()).unwrap();
    assert!(matches!(
        cap.sign_observation(&d, pending.challenge(), AnchorPositionV1::Proposed, &mut b),
        Err(Error::Rejected("anchor challenge names another key"))
    ));
}

#[test]
fn exact_and_one_short_quota_boundaries_cover_every_public_operation() {
    let mut setup_w = Work::new(LIMIT);
    let mut setup = Budget::new(&mut setup_w, LIMIT);
    let d = deployment(0, &mut setup);
    let cap = key(&d, &mut setup);
    let pin = PinnedAnchorKeyV1::from_bytes(public(7)).unwrap();
    let pending = AnchoredStateV1::from_local_state(3, HashChainHeadV1::from_bytes([1; 32]))
        .prepare(TransactionDigestV1::from_bytes([2; 32]), &pin)
        .unwrap()
        .begin_advance(CallerNonceV1::from_bytes([3; 32]), &pin)
        .unwrap();
    assert_eq!(Cap::IO_WORK, 8 + 64 * 1024 + 32 * 88);
    for operation in 0..8 {
        let input = match operation {
            0 => 32 + d.retained_storage(),
            1 | 2 | 3 => Cap::FILE_STORAGE + d.retained_storage(),
            4 => cap.retained_storage() + d.retained_storage(),
            5 => cap.retained_storage(),
            6 => cap.retained_storage() + Cap::FILE_STORAGE + d.retained_storage(),
            _ => cap.retained_storage() + d.retained_storage() + size_of::<AnchorChallengeV1>(),
        };
        let work = match operation {
            0..=2 => Cap::ADMISSION_WORK,
            3 => Cap::REISSUE_WORK,
            7 => Cap::OBSERVATION_WORK,
            _ => Cap::IO_WORK,
        };
        for mode in 0..5 {
            let floor = input - usize::from(mode == 1);
            let work_limit = match mode {
                2 => ENTRY_WORK - 1,
                3 => work - 1,
                _ => work,
            };
            let mut w = Work::new(work_limit);
            let mut b = Budget::new(&mut w, floor + Cap::IO_STORAGE - usize::from(mode == 4));
            b.reserve_storage(floor).unwrap();
            let ledger = b.work_ledger_identity_v1();
            let f = if operation == 6 {
                cap.image.clone_fixed().unwrap()
            } else {
                image(&wire(&d, 7), true)
            };
            let witness = f.try_clone().unwrap(); // Test observer, not an operation input.
            let mut seed = [7; 32];
            let mut staged = [9; 88];
            let mut source = None;
            let result = match operation {
                0 => Cap::create_and_zeroize(&mut seed, &d, &mut b).map(drop),
                1 => Cap::from_file(f, &d, &mut b).map(drop),
                2 => {
                    rustix::io::fcntl_setfd(&f, rustix::io::FdFlags::empty()).unwrap();
                    let result = Cap::from_inherited_at(f.as_raw_fd(), &d, &mut b).map(drop);
                    source = Some(f);
                    result
                }
                3 => Cap::reissue_template(f, &d, credentials(), &mut staged, &mut b, |_| Ok(()))
                    .map(drop),
                4 => cap.revalidate(&d, &mut b),
                5 => cap.try_clone_for_transfer(&mut b).map(drop),
                6 => cap.validate_transfer(&f, &d, &mut b),
                _ => cap
                    .sign_observation(&d, pending.challenge(), AnchorPositionV1::Proposed, &mut b)
                    .map(drop),
            };
            if operation == 0 {
                assert_eq!(seed, [0; 32]);
            }
            if operation == 3 {
                assert_eq!(staged, [0; 88]);
            }
            assert!(ledger == b.work_ledger_identity_v1());
            assert_eq!(b.storage(), floor);
            let expected_work = match mode {
                1 | 3 => ENTRY_WORK,
                2 => 0,
                _ => work,
            };
            assert_eq!(b.work(), expected_work, "op={operation}, mode={mode}");
            match mode {
                0 => {
                    result.unwrap();
                    assert_eq!(b.peak_storage(), floor + Cap::IO_STORAGE);
                }
                1 => assert!(matches!(result, Err(Error::Resource(Resource::Accounting)))),
                2 | 3 => assert!(matches!(result, Err(Error::Resource(Resource::Work(_))))),
                4 => assert!(matches!(result, Err(Error::Resource(Resource::Storage(_))))),
                _ => unreachable!(),
            }
            if operation == 1 || operation == 3 {
                assert_eq!(refs(&witness), 1);
            }
            if let Some(f) = source {
                assert_eq!(refs(&witness), 2);
                assert_eq!(
                    rustix::io::fcntl_getfd(&f).unwrap(),
                    rustix::io::FdFlags::empty()
                );
            }
        }
    }
}

#[test]
fn private_reissue_checks_both_template_owners_and_exact_current_service_credentials() {
    let mut w = Work::new(LIMIT);
    let mut b = Budget::new(&mut w, LIMIT);
    let d = deployment(0, &mut b);
    let (uid, gid) = credentials();
    for owner in [(0, 0), (uid + 1, gid), (uid, gid + 1)] {
        let f = image(&wire(&d, 7), true);
        let witness = f.try_clone().unwrap();
        b.reserve_storage(Cap::FILE_STORAGE).unwrap();
        let mut staged = [9; 88];
        assert!(matches!(
            failure(Cap::reissue_template(
                f,
                &d,
                owner,
                &mut staged,
                &mut b,
                |_| Ok(())
            )),
            Error::Rejected(_)
        ));
        assert_eq!(staged, [0; 88]);
        assert_eq!(refs(&witness), 1);
        b.release_storage(Cap::FILE_STORAGE).unwrap();
    }
    for axis in [3, 4] {
        let wrong = deployment(axis, &mut b);
        let f = image(&wire(&wrong, 7), true);
        b.reserve_storage(Cap::FILE_STORAGE).unwrap();
        assert!(matches!(
            failure(Cap::reissue_root_template_for_current_service(
                f, &wrong, &mut b
            )),
            Error::Rejected("anchor key reissue requires exact nonroot service credentials")
        ));
        b.release_storage(Cap::FILE_STORAGE).unwrap();
        let charge = wrong.retained_storage();
        drop(wrong);
        b.release_storage(charge).unwrap();
    }
    b.reserve_storage(Cap::FILE_STORAGE).unwrap();
    assert!(matches!(
        failure(Cap::reissue_root_template_for_current_service(
            image(&wire(&d, 7), true),
            &d,
            &mut b
        )),
        Error::Rejected(_)
    ));
}

#[test]
fn private_reissue_creates_fresh_custody_with_exact_growth_and_zeroed_staging() {
    let mut w = Work::new(LIMIT);
    let mut b = Budget::new(&mut w, LIMIT);
    let d = deployment(0, &mut b);
    b.reserve_storage(EXTRA + Cap::FILE_STORAGE).unwrap();
    let source = image(&wire(&d, 7), true);
    let witness = source.try_clone().unwrap();
    let old = witness.metadata().unwrap();
    let (before, floor, ledger) = (b.work(), b.storage(), b.work_ledger_identity_v1());
    let prior_peak = b.peak_storage();
    let mut staged = [9; 88];
    let (cap, growth) =
        Cap::reissue_template(source, &d, credentials(), &mut staged, &mut b, |_| Ok(())).unwrap();
    assert_eq!(staged, [0; 88]);
    assert_eq!((b.storage(), b.work()), (floor, before + Cap::REISSUE_WORK));
    assert_eq!(b.peak_storage(), prior_peak.max(floor + Cap::IO_STORAGE));
    assert!(ledger == b.work_ledger_identity_v1());
    assert_eq!(
        growth.additional_storage() + Cap::FILE_STORAGE,
        cap.retained_storage()
    );
    b.reserve_storage(growth.additional_storage()).unwrap();
    assert_eq!(refs(&witness), 1);
    let fresh = cap.image.clone_fixed().unwrap();
    let new = fresh.metadata().unwrap();
    assert_ne!((new.dev(), new.ino()), (old.dev(), old.ino()));
    assert_eq!((new.uid(), new.gid()), credentials());
    assert_eq!(new.mode(), libc::S_IFREG | 0o400);
    assert_eq!(
        rustix::fs::fcntl_getfl(&fresh).unwrap() & OFlags::ACCMODE,
        OFlags::RDONLY
    );
    cap.revalidate(&d, &mut b).unwrap();
    assert_eq!(cap.verifying_key(), public(7));
    assert_eq!(cap.deployment_identity(), d.identity());
    let charge = cap.retained_storage() + d.retained_storage();
    drop((fresh, cap, d));
    b.release_storage(charge).unwrap();
    assert_eq!(b.storage(), EXTRA);
}

#[test]
fn late_reissue_failure_and_unwind_wipe_staging_and_retire_fresh_and_source_images() {
    let mut w = Work::new(LIMIT);
    let mut b = Budget::new(&mut w, LIMIT);
    let d = deployment(0, &mut b);
    for mode in 0..4 {
        b.reserve_storage(Cap::FILE_STORAGE).unwrap();
        let template = image(&wire(&d, 7), true);
        let witness = template.try_clone().unwrap();
        let (floor, before, ledger) = (b.storage(), b.work(), b.work_ledger_identity_v1());
        assert!(b.charge_work(LIMIT).is_err());
        assert!(b.reserve_storage(LIMIT).is_err());
        let failures = (b.failed_work(), b.failed_storage());
        let mut staged = [9; 88];
        let mut fresh = None;
        let result = catch_unwind(AssertUnwindSafe(|| {
            Cap::reissue_template(template, &d, credentials(), &mut staged, &mut b, |cap| {
                fresh = Some(cap.image.clone_fixed().unwrap());
                match mode {
                    0 => Err(Error::Rejected("late observation")),
                    1 => {
                        rustix::fs::fchmod(&witness, Mode::RUSR | Mode::WUSR).unwrap();
                        Ok(())
                    }
                    2 => {
                        rustix::fs::fchmod(fresh.as_ref().unwrap(), Mode::RUSR | Mode::WUSR)
                            .unwrap();
                        Ok(())
                    }
                    _ => panic!("late anchor key unwind"),
                }
            })
        }));
        if mode == 3 {
            assert!(result.is_err());
        } else {
            assert!(matches!(failure(result.unwrap()), Error::Rejected(_)));
        }
        assert_eq!(staged, [0; 88]);
        assert_eq!(refs(&witness), 1);
        assert_eq!(refs(fresh.as_ref().unwrap()), 1);
        assert_eq!((b.storage(), b.work()), (floor, before + Cap::REISSUE_WORK));
        assert_eq!((b.failed_work(), b.failed_storage()), failures);
        assert!(ledger == b.work_ledger_identity_v1());
        assert!(b.peak_storage() >= floor + Cap::IO_STORAGE);
        b.release_storage(Cap::FILE_STORAGE).unwrap();
    }
}

#[test]
fn secret_read_and_caller_guards_wipe_partial_reads_refusal_and_unwind() {
    for unwind in [false, true] {
        let mut staged = [9; 88];
        let result = catch_unwind(AssertUnwindSafe(|| {
            with_secret(
                &mut staged,
                |bytes| {
                    bytes[50..60].fill(7);
                    if unwind {
                        panic!("short secret read unwind");
                    }
                    Err(Error::Rejected("partial secret read"))
                },
                |_| Ok(()),
            )
        }));
        if unwind {
            assert!(result.is_err());
        } else {
            assert!(result.unwrap().is_err());
        }
        assert_eq!(staged, [0; 88]);
    }
}

#[test]
fn secret_images_and_transfers_reject_access_flags_metadata_and_inode_substitution() {
    let mut w = Work::new(LIMIT);
    let mut b = Budget::new(&mut w, LIMIT);
    let d = deployment(0, &mut b);
    let cap = key(&d, &mut b);
    let (f, c) = cap.try_clone_for_transfer(&mut b).unwrap();
    b.reserve_storage(c.additional_storage()).unwrap();
    b.reserve_storage(Cap::FILE_STORAGE).unwrap();
    for source in [image(&wire(&d, 7), false), image(&wire(&d, 7), true)] {
        assert!(matches!(
            cap.validate_transfer(&source, &d, &mut b),
            Err(Error::Rejected(_))
        ));
        assert_eq!(refs(&source), 1);
    }
    assert!(matches!(
        failure(Cap::from_file(image(&wire(&d, 7), false), &d, &mut b)),
        Error::Rejected(_)
    ));
    assert!(matches!(
        failure(Cap::from_inherited_at(f.as_raw_fd(), &d, &mut b)),
        Error::Rejected(_)
    ));
    for fd in [-1, 2, i32::MAX] {
        assert!(Cap::from_inherited_at(fd, &d, &mut b).is_err());
    }
    assert_eq!(refs(&f), 2);
    rustix::io::fcntl_setfd(&f, rustix::io::FdFlags::empty()).unwrap();
    assert!(matches!(
        cap.validate_transfer(&f, &d, &mut b),
        Err(Error::Rejected(_))
    ));
    cap.revalidate(&d, &mut b).unwrap();
    rustix::io::fcntl_setfd(&f, rustix::io::FdFlags::CLOEXEC).unwrap();
    rustix::fs::fchmod(&f, Mode::RUSR | Mode::WUSR).unwrap();
    assert!(matches!(
        cap.revalidate(&d, &mut b),
        Err(Error::Rejected(_))
    ));
    assert!(cap.try_clone_for_transfer(&mut b).is_err());
    assert_eq!(refs(&f), 2);
}

#[test]
fn reservation_and_work_overflow_preserve_history_without_leaking_consumed_file() {
    let mut setup_w = Work::new(LIMIT);
    let mut setup = Budget::new(&mut setup_w, LIMIT);
    let d = deployment(0, &mut setup);
    for storage_overflow in [false, true] {
        let mut w = Work::new(usize::MAX);
        let mut b = Budget::new(&mut w, usize::MAX);
        let floor = if storage_overflow {
            usize::MAX - Cap::IO_STORAGE + 1
        } else {
            Cap::FILE_STORAGE + d.retained_storage()
        };
        b.reserve_storage(floor).unwrap();
        if !storage_overflow {
            b.charge_work(usize::MAX - 7).unwrap();
        }
        let ledger = b.work_ledger_identity_v1();
        let f = image(&wire(&d, 7), true);
        let witness = f.try_clone().unwrap();
        let e = failure(Cap::from_file(f, &d, &mut b));
        if storage_overflow {
            assert!(matches!(e, Error::Resource(Resource::Storage(_))));
            assert_eq!(b.failed_storage(), Some(usize::MAX));
            assert_eq!(b.work(), Cap::ADMISSION_WORK);
        } else {
            assert!(matches!(e, Error::Resource(Resource::Work(_))));
            assert_eq!(b.failed_work(), Some(usize::MAX));
            assert_eq!(b.work(), usize::MAX - 7);
        }
        assert_eq!(b.storage(), floor);
        assert_eq!(refs(&witness), 1);
        assert!(ledger == b.work_ledger_identity_v1());
    }
}

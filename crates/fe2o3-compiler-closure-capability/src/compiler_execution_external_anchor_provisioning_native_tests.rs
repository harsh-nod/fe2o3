use super::*;
use crate::{CompilerExecutionCapabilityErrorV2 as Error, sealed_image::SealedCapabilityImage};
use ed25519_dalek::SigningKey;
use fe2o3_compiler_execution_protocol::{
    CompilerExecutionExternalAnchorServiceIdentityV1 as Service,
    CompilerExecutionIssuerMeasurementV1 as Measurement,
};
use fe2o3_kernel_ir::{
    CanonicalKernelIrVerificationResourceErrorV1 as Resource, CanonicalKernelIrWorkBudgetV1 as Work,
};
use sha2::{Digest, Sha256};
use std::os::{fd::AsRawFd, unix::fs::MetadataExt};

const LIMIT: usize = 10_000_000;
fn measurement(n: u8) -> Measurement {
    Measurement::new([n; 32], 4096).unwrap()
}
fn public(n: u8) -> [u8; 32] {
    SigningKey::from_bytes(&[n; 32]).verifying_key().to_bytes()
}
fn deployment(n: u8, b: &mut Budget<'_>) -> Deployment {
    let (p, c) = Policy::new(7, measurement(1), measurement(2), public(3), public(4), b).unwrap();
    b.reserve_storage(c.additional_storage()).unwrap();
    let (s, c) = Supervisor::new(
        1001,
        1002,
        Service::new(1003, 1004).unwrap(),
        measurement(5),
        measurement(6),
        &p,
        b,
    )
    .unwrap();
    b.reserve_storage(c.additional_storage()).unwrap();
    let (d, c) = Deployment::new(&s, &p, measurement(n), b).unwrap();
    b.reserve_storage(c.additional_storage()).unwrap();
    let retired = s.retained_storage() + p.retained_storage();
    drop((s, p));
    b.release_storage(retired).unwrap();
    d
}
fn provisioning(d: &Deployment, b: &mut Budget<'_>) -> Provisioning {
    let (p, c) = Provisioning::new(d, measurement(8), b).unwrap();
    b.reserve_storage(c.additional_storage()).unwrap();
    p
}
fn image(bytes: &[u8; BYTES]) -> File {
    SealedCapabilityImage::create_fixed(bytes, <Provisioning as Record<BYTES>>::ROLE)
        .unwrap()
        .clone_fixed()
        .unwrap()
}
fn refs(f: &File) -> usize {
    let m = f.metadata().unwrap();
    std::fs::read_dir("/proc/self/fd")
        .unwrap()
        .filter_map(|e| std::fs::metadata(e.ok()?.path()).ok())
        .filter(|other| (other.dev(), other.ino()) == (m.dev(), m.ino()))
        .count()
}
fn failure<T>(r: Result<T>) -> Error {
    match r {
        Err(e) => e,
        Ok(_) => panic!("unexpected acceptance"),
    }
}
fn reseal(bytes: &mut [u8; BYTES]) {
    let domain = if VERSION == 2 {
        b"FE2O3/COMPILER-EXECUTION-EXTERNAL-ANCHOR-PROVISIONING/V2\0"
    } else {
        b"FE2O3/COMPILER-EXECUTION-EXTERNAL-ANCHOR-PROVISIONING/V3\0"
    };
    let mut h = Sha256::new();
    h.update(domain);
    h.update(96u64.to_le_bytes());
    h.update(&bytes[..96]);
    bytes[96..].copy_from_slice(&h.finalize());
}

#[test]
fn provisioning_roundtrip_preserves_exact_context_object_and_retained_charges() {
    let mut w = Work::new(LIMIT);
    let mut b = Budget::new(&mut w, LIMIT);
    b.reserve_storage(19).unwrap();
    let d = deployment(7, &mut b);
    let p = provisioning(&d, &mut b);
    let original = *p.canonical_bytes();
    let record_charge = p.retained_storage();
    let (cap, c) = Cap::create(p, &mut b).unwrap();
    assert_eq!(
        record_charge + c.additional_storage(),
        cap.retained_storage()
    );
    b.reserve_storage(c.additional_storage()).unwrap();
    let retained = cap.retained_storage();
    let (f, c) = cap.try_clone_for_transfer(&mut b).unwrap();
    b.reserve_storage(c.additional_storage()).unwrap();
    assert_eq!(c.additional_storage(), Cap::FILE_STORAGE);
    rustix::fs::seek(&f, rustix::fs::SeekFrom::Start(17)).unwrap();
    cap.validate_transfer(&f, &mut b).unwrap();
    assert_eq!(
        rustix::fs::seek(&f, rustix::fs::SeekFrom::Current(0)).unwrap(),
        17
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
    assert_eq!(inherited.provisioning().canonical_bytes(), &original);
    inherited.revalidate(&mut b).unwrap();
    drop(inherited);
    b.release_storage(retained).unwrap();
    drop(cap);
    b.release_storage(retained).unwrap();
    rustix::io::fcntl_setfd(&f, rustix::io::FdFlags::CLOEXEC).unwrap();
    let (cap, c) = Cap::from_file(f, &d, &mut b).unwrap();
    assert_eq!(c.additional_storage() + Cap::FILE_STORAGE, retained);
    b.reserve_storage(c.additional_storage()).unwrap();
    assert_eq!(cap.provisioning().deployment_identity(), d.identity());
    assert_eq!(cap.provisioning().canonical_bytes(), &original);
    let retired = retained + d.retained_storage();
    drop((cap, d));
    b.release_storage(retired).unwrap();
    assert_eq!(b.storage(), 19);
}

#[test]
fn native_context_and_resealed_substitutions_refuse_with_typed_errors_and_cleanup() {
    let mut w = Work::new(LIMIT);
    let mut b = Budget::new(&mut w, LIMIT);
    let d = deployment(7, &mut b);
    let wrong = deployment(9, &mut b);
    let p = provisioning(&d, &mut b);
    for inherited in [false, true] {
        for mode in 0..4 {
            let mut bytes = *p.canonical_bytes();
            let context = if mode == 0 { &wrong } else { &d };
            match mode {
                1 => {
                    bytes[24..56].copy_from_slice(wrong.identity().as_bytes());
                    reseal(&mut bytes);
                }
                2 => {
                    bytes[7] = b'1';
                    bytes[8..10].copy_from_slice(&1u16.to_le_bytes());
                }
                3 => {
                    let other = if VERSION == 2 { 3u16 } else { 2 };
                    bytes[7] = b'0' + other as u8;
                    bytes[8..10].copy_from_slice(&other.to_le_bytes());
                }
                _ => {}
            }
            b.reserve_storage(Cap::FILE_STORAGE).unwrap();
            let f = image(&bytes);
            let witness = f.try_clone().unwrap();
            let (floor, before, ledger) = (b.storage(), b.work(), b.work_ledger_identity_v1());
            let (r, source) = if inherited {
                rustix::io::fcntl_setfd(&f, rustix::io::FdFlags::empty()).unwrap();
                (
                    Cap::from_inherited_at(f.as_raw_fd(), context, &mut b),
                    Some(f),
                )
            } else {
                (Cap::from_file(f, context, &mut b), None)
            };
            let e = failure(r);
            if mode < 2 {
                assert!(matches!(decode_error(&e), DecodeError::ContextMismatch));
            } else {
                assert!(matches!(decode_error(&e), DecodeError::Framing(_)));
            }
            assert_eq!(e.to_string(), decode_error(&e).to_string());
            assert!(std::error::Error::source(&e).is_some());
            assert_eq!(
                (b.storage(), b.work()),
                (floor, before + Cap::ADMISSION_WORK)
            );
            assert!(ledger == b.work_ledger_identity_v1());
            assert_eq!(refs(&witness), 1 + usize::from(inherited));
            if let Some(f) = source {
                assert_eq!(
                    rustix::io::fcntl_getfd(&f).unwrap(),
                    rustix::io::FdFlags::empty()
                );
            }
            b.release_storage(Cap::FILE_STORAGE).unwrap();
        }
    }
}

#[test]
fn helper_rehash_is_configuration_not_independently_pinned_executable_authority() {
    let mut w = Work::new(LIMIT);
    let mut b = Budget::new(&mut w, LIMIT);
    let d = deployment(7, &mut b);
    let p = provisioning(&d, &mut b);
    let mut bytes = *p.canonical_bytes();
    bytes[56] ^= 1;
    reseal(&mut bytes);
    b.reserve_storage(Cap::FILE_STORAGE).unwrap();
    let (cap, c) = Cap::from_file(image(&bytes), &d, &mut b).unwrap();
    b.reserve_storage(c.additional_storage()).unwrap();
    assert!(
        !cap.provisioning()
            .matches_deployment_and_helper(&d, measurement(8), &mut b)
            .unwrap()
    );
    assert_ne!(cap.provisioning().identity(), p.identity());
}

#[test]
fn exact_outer_and_nested_resource_boundaries_keep_storage_history_and_source_custody() {
    let mut setup_w = Work::new(LIMIT);
    let mut setup = Budget::new(&mut setup_w, LIMIT);
    let d = deployment(7, &mut setup);
    let p = provisioning(&d, &mut setup);
    assert_eq!(Cap::IO_WORK, 36872);
    assert_eq!(Cap::ADMISSION_WORK, 40976);
    let input = Cap::FILE_STORAGE + d.retained_storage();
    for inherited in [false, true] {
        for mode in 0..8 {
            let floor = input - usize::from(mode == 1);
            let work_limit = match mode {
                2 => 7,
                3 => Cap::IO_WORK - 1,
                5 => Cap::IO_WORK + 7,
                6 => Cap::ADMISSION_WORK - 1,
                _ => Cap::ADMISSION_WORK,
            };
            let scratch = match mode {
                4 => Cap::IO_STORAGE - 1,
                7 => Cap::ADMISSION_STORAGE - 1,
                _ => Cap::ADMISSION_STORAGE,
            };
            let mut w = Work::new(work_limit);
            let mut b = Budget::new(&mut w, floor + scratch);
            b.reserve_storage(floor).unwrap();
            let ledger = b.work_ledger_identity_v1();
            let f = image(p.canonical_bytes());
            let witness = f.try_clone().unwrap();
            let (r, source) = if inherited {
                rustix::io::fcntl_setfd(&f, rustix::io::FdFlags::empty()).unwrap();
                (Cap::from_inherited_at(f.as_raw_fd(), &d, &mut b), Some(f))
            } else {
                (Cap::from_file(f, &d, &mut b), None)
            };
            assert_eq!(b.storage(), floor);
            assert!(ledger == b.work_ledger_identity_v1());
            let used = match mode {
                1 | 3 => 8,
                2 => 0,
                4 | 5 => Cap::IO_WORK,
                6 => Cap::IO_WORK + 8,
                _ => Cap::ADMISSION_WORK,
            };
            assert_eq!(b.work(), used);
            if mode == 0 {
                let (cap, c) = r.unwrap();
                assert_eq!(b.peak_storage(), floor + Cap::ADMISSION_STORAGE);
                assert_eq!(
                    c.additional_storage(),
                    cap.retained_storage() - if inherited { 0 } else { Cap::FILE_STORAGE }
                );
                b.reserve_storage(c.additional_storage()).unwrap();
                let retained = cap.retained_storage();
                drop(cap);
                b.release_storage(retained).unwrap();
            } else {
                let e = failure(r);
                let resource = match &e {
                    Error::Resource(r) => r,
                    _ => match decode_error(&e) {
                        DecodeError::Resource(r) => r,
                        other => panic!("lost resource: {other}"),
                    },
                };
                match mode {
                    1 => assert!(matches!(resource, Resource::Accounting)),
                    2 | 3 | 5 | 6 => {
                        assert!(matches!(resource, Resource::Work(_)));
                        assert!(b.failed_work().is_some());
                    }
                    _ => {
                        assert!(matches!(resource, Resource::Storage(_)));
                        assert!(b.failed_storage().is_some());
                    }
                }
            }
            assert_eq!(refs(&witness), 1 + usize::from(inherited));
            if let Some(f) = source {
                assert_eq!(
                    rustix::io::fcntl_getfd(&f).unwrap(),
                    rustix::io::FdFlags::empty()
                );
            }
        }
    }
}

#[test]
fn transfers_require_exact_object_and_recheck_mutable_metadata() {
    let mut w = Work::new(LIMIT);
    let mut b = Budget::new(&mut w, LIMIT);
    let d = deployment(7, &mut b);
    let p = provisioning(&d, &mut b);
    let bytes = *p.canonical_bytes();
    let (cap, c) = Cap::create(p, &mut b).unwrap();
    b.reserve_storage(c.additional_storage()).unwrap();
    let (f, c) = cap.try_clone_for_transfer(&mut b).unwrap();
    b.reserve_storage(c.additional_storage()).unwrap();
    b.reserve_storage(Cap::FILE_STORAGE).unwrap();
    let other = image(&bytes);
    assert!(matches!(
        cap.validate_transfer(&other, &mut b),
        Err(Error::Rejected(_))
    ));
    assert_eq!(refs(&other), 1);
    rustix::io::fcntl_setfd(&f, rustix::io::FdFlags::empty()).unwrap();
    assert!(matches!(
        cap.validate_transfer(&f, &mut b),
        Err(Error::Rejected(_))
    ));
    cap.revalidate(&mut b).unwrap();
    rustix::io::fcntl_setfd(&f, rustix::io::FdFlags::CLOEXEC).unwrap();
    rustix::fs::fchmod(&f, rustix::fs::Mode::RUSR | rustix::fs::Mode::WUSR).unwrap();
    assert!(matches!(cap.revalidate(&mut b), Err(Error::Rejected(_))));
    assert!(cap.try_clone_for_transfer(&mut b).is_err());
    assert_eq!(refs(&f), 2);
}

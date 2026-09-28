use ed25519_dalek::SigningKey;
use fe2o3_compiler_closure_capability::CompilerExecutionCapabilityErrorV2 as Error;
use fe2o3_compiler_execution_protocol::{
    CompilerExecutionExternalAnchorDeploymentErrorV1 as Framing,
    CompilerExecutionExternalAnchorDeploymentV1 as OldDeployment,
    CompilerExecutionExternalAnchorServiceIdentityV1 as Service,
    CompilerExecutionIssuerMeasurementV1 as Measurement,
    CompilerExecutionIssuerPolicyV1 as OldPolicy,
    CompilerExecutionSupervisorDeploymentV1 as OldSupervisor,
};
use fe2o3_kernel_ir::{
    CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
    CanonicalKernelIrVerificationResourceErrorV1 as Resource,
    CanonicalKernelIrWorkBudgetV1 as Work,
};
use rustix::fs::{Mode, SealFlags};
use sha2::{Digest, Sha256};
use std::{
    fs::File,
    os::{fd::AsRawFd, unix::fs::MetadataExt},
};

const LIMIT: usize = 10_000_000;
const EXTRA: usize = 19;
const ENTRY: usize = 8;
const LOCAL_WORK: usize = DECODE_WORK - SUPERVISOR_WORK;
const LOCAL_STORAGE: usize = DECODE_STORAGE - SUPERVISOR_STORAGE;
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
fn executable() -> Measurement {
    measurement(33, 16384)
}
fn policy(axis: u8, b: &mut Budget<'_>) -> Policy {
    let (p, d) = Policy::new(
        if axis == 1 { 8 } else { 7 },
        measurement(
            if axis == 2 { 21 } else { 11 },
            if axis == 3 { 124 } else { 123 },
        ),
        measurement(
            if axis == 4 { 22 } else { 12 },
            if axis == 5 { 457 } else { 456 },
        ),
        key(if axis == 6 { 17 } else { 7 }),
        key(if axis == 7 { 18 } else { 8 }),
        b,
    )
    .unwrap();
    b.reserve_storage(d.additional_storage()).unwrap();
    p
}
fn supervisor(p: &Policy, axis: u8, b: &mut Budget<'_>) -> Supervisor {
    let (s, d) = Supervisor::new(
        if axis == 1 { 1101 } else { 1001 },
        if axis == 2 { 1102 } else { 1002 },
        Service::new(
            if axis == 3 { 1103 } else { 1003 },
            if axis == 4 { 1104 } else { 1004 },
        )
        .unwrap(),
        measurement(
            if axis == 5 { 41 } else { 31 },
            if axis == 6 { 4097 } else { 4096 },
        ),
        measurement(
            if axis == 7 { 42 } else { 32 },
            if axis == 8 { 8193 } else { 8192 },
        ),
        p,
        b,
    )
    .unwrap();
    b.reserve_storage(d.additional_storage()).unwrap();
    s
}
fn deployment(s: &Supervisor, p: &Policy, b: &mut Budget<'_>) -> Deployment {
    let (d, growth) = Deployment::new(s, p, executable(), b).unwrap();
    b.reserve_storage(growth.additional_storage()).unwrap();
    d
}
fn image(bytes: &[u8], seal: bool) -> File {
    let f = File::from(
        rustix::fs::memfd_create(
            "native-anchor-deployment-test",
            rustix::fs::MemfdFlags::CLOEXEC | rustix::fs::MemfdFlags::ALLOW_SEALING,
        )
        .unwrap(),
    );
    assert_eq!(rustix::io::pwrite(&f, bytes, 0).unwrap(), bytes.len());
    rustix::fs::fchmod(&f, Mode::RUSR).unwrap();
    if seal {
        rustix::fs::fcntl_add_seals(&f, SEALS).unwrap();
    }
    f
}
fn sealed(bytes: &[u8]) -> File {
    image(bytes, true)
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
        Err(e) => e,
        Ok(_) => panic!("unexpected acceptance"),
    }
}
fn resource_error(error: &Error) -> &Resource {
    match error {
        Error::Resource(e) => e,
        e => match decode_error(e) {
            DecodeError::Resource(e) => e,
            e => panic!("lost resource error: {e:?}"),
        },
    }
}
fn reseal(bytes: &mut [u8; BYTES], domain: &[u8]) {
    let mut hash = Sha256::new();
    hash.update(domain);
    hash.update(136u64.to_le_bytes());
    hash.update(&bytes[..136]);
    bytes[136..].copy_from_slice(&hash.finalize());
}
fn contextual_refusal(
    bytes: &[u8],
    s: &Supervisor,
    p: &Policy,
    inherited: bool,
    b: &mut Budget<'_>,
) -> Error {
    b.reserve_storage(2 * Cap::FILE_STORAGE).unwrap();
    let file = sealed(bytes);
    let witness = file.try_clone().unwrap();
    let (floor, before, ledger) = (b.storage(), b.work(), b.work_ledger_identity_v1());
    let (result, source) = if inherited {
        rustix::io::fcntl_setfd(&file, rustix::io::FdFlags::empty()).unwrap();
        (
            Cap::from_inherited_at(file.as_raw_fd(), s, p, b),
            Some(file),
        )
    } else {
        (Cap::from_file(file, s, p, b), None)
    };
    let error = failure(result);
    assert_eq!(
        (b.storage(), b.work()),
        (floor, before + Cap::ADMISSION_WORK)
    );
    assert!(b.work_ledger_identity_v1() == ledger);
    assert_eq!(references(&witness), 1 + usize::from(inherited));
    if let Some(file) = &source {
        assert_eq!(
            rustix::io::fcntl_getfd(file).unwrap(),
            rustix::io::FdFlags::empty()
        );
    }
    let native = decode_error(&error);
    assert_eq!(error.to_string(), native.to_string());
    assert!(std::error::Error::source(&error).is_some());
    drop((source, witness));
    b.release_storage(2 * Cap::FILE_STORAGE).unwrap();
    error
}

#[test]
fn roundtrip_transfers_and_inheritance_preserve_context_object_and_exact_retention() {
    assert_eq!(BYTES, 168);
    assert_eq!(Cap::IO_WORK, 38152);
    assert_eq!(Cap::ADMISSION_WORK, Cap::IO_WORK + DECODE_WORK);
    assert_eq!(Cap::ADMISSION_STORAGE, Cap::IO_STORAGE + DECODE_STORAGE);
    let mut w = Work::new(LIMIT);
    let mut b = Budget::new(&mut w, LIMIT);
    b.reserve_storage(EXTRA).unwrap();
    let p = policy(0, &mut b);
    let s = supervisor(&p, 0, &mut b);
    let d = deployment(&s, &p, &mut b);
    let expected = *d.canonical_bytes();
    let record_charge = d.retained_storage();
    let (before, floor, ledger) = (b.work(), b.storage(), b.work_ledger_identity_v1());
    let (cap, growth) = Cap::create(d, &mut b).unwrap();
    assert_eq!(b.storage(), floor);
    assert_eq!(
        record_charge + growth.additional_storage(),
        cap.retained_storage()
    );
    b.reserve_storage(growth.additional_storage()).unwrap();
    let retained = cap.retained_storage();
    let (file, growth) = cap.try_clone_for_transfer(&mut b).unwrap();
    assert_eq!(growth.additional_storage(), Cap::FILE_STORAGE);
    b.reserve_storage(growth.additional_storage()).unwrap();
    b.reserve_storage(Cap::FILE_STORAGE).unwrap();
    let witness = file.try_clone().unwrap();
    assert_eq!(references(&witness), 3);
    assert_eq!(file.metadata().unwrap().mode(), libc::S_IFREG | 0o400);
    assert_eq!(rustix::fs::fcntl_get_seals(&file).unwrap(), SEALS);
    assert_eq!(
        rustix::io::fcntl_getfd(&file).unwrap(),
        rustix::io::FdFlags::CLOEXEC
    );
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
    let floor = b.storage();
    let (inherited, full) = Cap::from_inherited_at(file.as_raw_fd(), &s, &p, &mut b).unwrap();
    assert_eq!(b.storage(), floor);
    assert_eq!(full.additional_storage(), retained);
    b.reserve_storage(full.additional_storage()).unwrap();
    assert_eq!(references(&witness), 4);
    assert_eq!(inherited.deployment().canonical_bytes(), &expected);
    assert_eq!(
        rustix::io::fcntl_getfd(&file).unwrap(),
        rustix::io::FdFlags::empty()
    );
    inherited.revalidate(&mut b).unwrap();
    drop(inherited);
    b.release_storage(retained).unwrap();
    rustix::io::fcntl_setfd(&file, rustix::io::FdFlags::CLOEXEC).unwrap();
    drop(cap);
    b.release_storage(retained).unwrap();
    let floor = b.storage();
    let (recovered, growth) = Cap::from_file(file, &s, &p, &mut b).unwrap();
    assert_eq!(b.storage(), floor);
    assert_eq!(growth.additional_storage() + Cap::FILE_STORAGE, retained);
    b.reserve_storage(growth.additional_storage()).unwrap();
    assert_eq!(references(&witness), 2);
    assert_eq!(recovered.deployment().canonical_bytes(), &expected);
    assert_eq!(
        recovered.deployment().supervisor_deployment_identity(),
        s.identity()
    );
    assert_eq!(
        recovered.deployment().verifying_key(),
        p.external_anchor_verifying_key()
    );
    assert_eq!(recovered.deployment().executable(), executable());
    assert_eq!(
        b.work(),
        before + 4 * Cap::IO_WORK + 2 * Cap::ADMISSION_WORK
    );
    assert!(b.work_ledger_identity_v1() == ledger);
    let context = s.retained_storage() + p.retained_storage();
    assert_eq!(b.storage(), EXTRA + context + retained + Cap::FILE_STORAGE);
    drop((recovered, witness, s, p));
    b.release_storage(context + retained + Cap::FILE_STORAGE)
        .unwrap();
    assert_eq!(b.storage(), EXTRA);
}

#[test]
fn recovery_requires_every_policy_axis_and_the_actual_supervisor_policy_relationship() {
    let mut w = Work::new(LIMIT);
    let mut b = Budget::new(&mut w, LIMIT);
    let pinned = policy(0, &mut b);
    let s = supervisor(&pinned, 0, &mut b);
    let d = deployment(&s, &pinned, &mut b);
    for axis in 1..=7 {
        let wrong = policy(axis, &mut b);
        let other_s = supervisor(&wrong, 0, &mut b);
        for inherited in [false, true] {
            let e = contextual_refusal(d.canonical_bytes(), &s, &wrong, inherited, &mut b);
            assert!(matches!(
                decode_error(&e),
                DecodeError::SupervisorPolicyMismatch
            ));
            let e = contextual_refusal(d.canonical_bytes(), &other_s, &wrong, inherited, &mut b);
            assert!(matches!(decode_error(&e), DecodeError::ContextMismatch));
        }
        let retired = wrong.retained_storage() + other_s.retained_storage();
        drop((wrong, other_s));
        b.release_storage(retired).unwrap();
    }
}

#[test]
fn recovery_requires_the_complete_supervisor_identity_and_exact_anchor_credentials() {
    let mut w = Work::new(LIMIT);
    let mut b = Budget::new(&mut w, LIMIT);
    let p = policy(0, &mut b);
    let s = supervisor(&p, 0, &mut b);
    let d = deployment(&s, &p, &mut b);
    for axis in 1..=8 {
        let wrong = supervisor(&p, axis, &mut b);
        for inherited in [false, true] {
            let e = contextual_refusal(d.canonical_bytes(), &wrong, &p, inherited, &mut b);
            assert!(matches!(decode_error(&e), DecodeError::ContextMismatch));
        }
        let retired = wrong.retained_storage();
        drop(wrong);
        b.release_storage(retired).unwrap();
    }
}

#[test]
fn sealed_legacy_foreign_and_rehashed_context_substitutions_cannot_upgrade() {
    let mut w = Work::new(LIMIT);
    let mut b = Budget::new(&mut w, LIMIT);
    let p = policy(0, &mut b);
    let s = supervisor(&p, 0, &mut b);
    let d = deployment(&s, &p, &mut b);
    let old_p = OldPolicy::new(
        7,
        measurement(11, 123),
        measurement(12, 456),
        key(7),
        key(8),
    )
    .unwrap();
    let old_s = OldSupervisor::new(
        1001,
        1002,
        Service::new(1003, 1004).unwrap(),
        measurement(31, 4096),
        measurement(32, 8192),
        &old_p,
    )
    .unwrap();
    let old = OldDeployment::new(&old_s, &old_p, executable()).unwrap();
    let (other_p, charge) = OtherPolicy::new(
        7,
        measurement(11, 123),
        measurement(12, 456),
        key(7),
        key(8),
        &mut b,
    )
    .unwrap();
    b.reserve_storage(charge.additional_storage()).unwrap();
    let (other_s, charge) = OtherSupervisor::new(
        1001,
        1002,
        Service::new(1003, 1004).unwrap(),
        measurement(31, 4096),
        measurement(32, 8192),
        &other_p,
        &mut b,
    )
    .unwrap();
    b.reserve_storage(charge.additional_storage()).unwrap();
    let (other, charge) = OtherDeployment::new(&other_s, &other_p, executable(), &mut b).unwrap();
    b.reserve_storage(charge.additional_storage()).unwrap();
    for foreign in [old.canonical_bytes(), other.canonical_bytes()] {
        for inherited in [false, true] {
            let e = contextual_refusal(foreign, &s, &p, inherited, &mut b);
            assert!(matches!(
                decode_error(&e),
                DecodeError::Framing(Framing::Magic)
            ));
            let mut bytes = *foreign;
            bytes[..8].copy_from_slice(MAGIC);
            bytes[8..10].copy_from_slice(&VERSION.to_le_bytes());
            reseal(&mut bytes, DOMAIN);
            let e = contextual_refusal(&bytes, &s, &p, inherited, &mut b);
            assert!(matches!(decode_error(&e), DecodeError::ContextMismatch));
        }
    }
    let mut bytes = *d.canonical_bytes();
    reseal(&mut bytes, OTHER_DOMAIN);
    let e = contextual_refusal(&bytes, &s, &p, false, &mut b);
    assert!(matches!(
        decode_error(&e),
        DecodeError::Framing(Framing::Identity)
    ));
    for offset in [24, 28, 32, 64] {
        let mut bytes = *d.canonical_bytes();
        bytes[offset] ^= 1;
        reseal(&mut bytes, DOMAIN);
        for inherited in [false, true] {
            let e = contextual_refusal(&bytes, &s, &p, inherited, &mut b);
            assert!(matches!(decode_error(&e), DecodeError::ContextMismatch));
        }
    }
    let mut bytes = *d.canonical_bytes();
    bytes[32..64].fill(0);
    bytes[32] = 1; // Weak Ed25519 identity point, despite valid framing and digest.
    reseal(&mut bytes, DOMAIN);
    let e = contextual_refusal(&bytes, &s, &p, false, &mut b);
    assert!(matches!(decode_error(&e), DecodeError::ContextMismatch));
}

#[test]
fn sealed_executable_measurement_remains_configuration_and_requires_exact_pinning() {
    let mut w = Work::new(LIMIT);
    let mut b = Budget::new(&mut w, LIMIT);
    let p = policy(0, &mut b);
    let s = supervisor(&p, 0, &mut b);
    let d = deployment(&s, &p, &mut b);
    for offset in [96, 128] {
        let mut bytes = *d.canonical_bytes();
        bytes[offset] ^= 1;
        reseal(&mut bytes, DOMAIN);
        b.reserve_storage(Cap::FILE_STORAGE).unwrap();
        let (cap, growth) = Cap::from_file(sealed(&bytes), &s, &p, &mut b).unwrap();
        b.reserve_storage(growth.additional_storage()).unwrap();
        assert_ne!(cap.deployment().identity(), d.identity());
        assert!(
            !cap.deployment()
                .matches_supervisor_policy_and_executable(&s, &p, executable(), &mut b)
                .unwrap()
        );
        let retained = cap.retained_storage();
        drop(cap);
        b.release_storage(retained).unwrap();
    }
    for length in [0u64, 128 * 1024 * 1024 + 1, u64::MAX] {
        let mut bytes = *d.canonical_bytes();
        bytes[128..136].copy_from_slice(&length.to_le_bytes());
        reseal(&mut bytes, DOMAIN);
        let e = contextual_refusal(&bytes, &s, &p, false, &mut b);
        assert!(matches!(
            decode_error(&e),
            DecodeError::Framing(Framing::ExecutableMeasurement)
        ));
    }
}

#[test]
fn exact_and_one_short_admission_at_all_three_stages_preserves_original_ledger() {
    let mut setup_work = Work::new(LIMIT);
    let mut setup = Budget::new(&mut setup_work, LIMIT);
    let p = policy(0, &mut setup);
    let s = supervisor(&p, 0, &mut setup);
    let d = deployment(&s, &p, &mut setup);
    let input = Cap::FILE_STORAGE + s.retained_storage() + p.retained_storage();
    for inherited in [false, true] {
        for mode in 0..13 {
            let floor = match mode {
                1 => input - 1,
                11 => input - p.retained_storage(),
                12 => input - s.retained_storage(),
                _ => input,
            };
            let work_limit = match mode {
                2 => ENTRY - 1,
                3 => Cap::IO_WORK - 1,
                5 => Cap::IO_WORK + ENTRY - 1,
                6 => Cap::IO_WORK + LOCAL_WORK - 1,
                8 => Cap::IO_WORK + LOCAL_WORK + ENTRY - 1,
                9 => Cap::ADMISSION_WORK - 1,
                _ => Cap::ADMISSION_WORK,
            };
            let scratch = match mode {
                4 => Cap::IO_STORAGE - 1,
                7 => Cap::IO_STORAGE + LOCAL_STORAGE - 1,
                10 => Cap::ADMISSION_STORAGE - 1,
                _ => Cap::ADMISSION_STORAGE,
            };
            let mut w = Work::new(work_limit);
            let mut b = Budget::new(&mut w, floor + scratch);
            b.reserve_storage(floor).unwrap();
            let file = sealed(d.canonical_bytes());
            let witness = file.try_clone().unwrap(); // Test observer, never an operation input.
            let ledger = b.work_ledger_identity_v1();
            let (result, source) = if inherited {
                rustix::io::fcntl_setfd(&file, rustix::io::FdFlags::empty()).unwrap();
                (
                    Cap::from_inherited_at(file.as_raw_fd(), &s, &p, &mut b),
                    Some(file),
                )
            } else {
                (Cap::from_file(file, &s, &p, &mut b), None)
            };
            assert_eq!(b.storage(), floor, "mode {mode}");
            assert!(b.work_ledger_identity_v1() == ledger);
            if mode == 0 {
                let (cap, growth) = result.unwrap();
                assert_eq!(b.work(), Cap::ADMISSION_WORK);
                assert_eq!(b.peak_storage(), floor + Cap::ADMISSION_STORAGE);
                assert_eq!(
                    growth.additional_storage(),
                    cap.retained_storage() - if inherited { 0 } else { Cap::FILE_STORAGE }
                );
                b.reserve_storage(growth.additional_storage()).unwrap();
                let retained = cap.retained_storage();
                drop(cap);
                b.release_storage(retained).unwrap();
                assert_eq!(
                    b.storage(),
                    floor - if inherited { 0 } else { Cap::FILE_STORAGE }
                );
            } else {
                let error = failure(result);
                let resource = resource_error(&error);
                let (used, peak) = match mode {
                    1 | 11 | 12 => {
                        assert!(matches!(resource, Resource::Accounting));
                        (ENTRY, floor)
                    }
                    2 | 3 | 5 | 6 | 8 | 9 => {
                        assert!(matches!(resource, Resource::Work(_)));
                        let (used, denied, peak) = match mode {
                            2 => (0, ENTRY, floor),
                            3 => (ENTRY, Cap::IO_WORK, floor),
                            5 => (Cap::IO_WORK, Cap::IO_WORK + ENTRY, floor + Cap::IO_STORAGE),
                            6 => (
                                Cap::IO_WORK + ENTRY,
                                Cap::IO_WORK + LOCAL_WORK,
                                floor + Cap::IO_STORAGE,
                            ),
                            8 => (
                                Cap::IO_WORK + LOCAL_WORK,
                                Cap::IO_WORK + LOCAL_WORK + ENTRY,
                                floor + Cap::IO_STORAGE + LOCAL_STORAGE,
                            ),
                            9 => (
                                Cap::IO_WORK + LOCAL_WORK + ENTRY,
                                Cap::ADMISSION_WORK,
                                floor + Cap::IO_STORAGE + LOCAL_STORAGE,
                            ),
                            _ => unreachable!(),
                        };
                        assert_eq!(b.failed_work(), Some(denied));
                        (used, peak)
                    }
                    4 | 7 | 10 => {
                        assert!(matches!(resource, Resource::Storage(_)));
                        let (used, denied, peak) = match mode {
                            4 => (Cap::IO_WORK, floor + Cap::IO_STORAGE, floor),
                            7 => (
                                Cap::IO_WORK + LOCAL_WORK,
                                floor + Cap::IO_STORAGE + LOCAL_STORAGE,
                                floor + Cap::IO_STORAGE,
                            ),
                            10 => (
                                Cap::ADMISSION_WORK,
                                floor + Cap::ADMISSION_STORAGE,
                                floor + Cap::IO_STORAGE + LOCAL_STORAGE,
                            ),
                            _ => unreachable!(),
                        };
                        assert_eq!(b.failed_storage(), Some(denied));
                        (used, peak)
                    }
                    _ => unreachable!(),
                };
                assert_eq!((b.work(), b.peak_storage()), (used, peak), "mode {mode}");
                if (5..=10).contains(&mode) {
                    assert!(matches!(decode_error(&error), DecodeError::Resource(_)));
                } else {
                    assert!(matches!(error, Error::Resource(_)));
                }
            }
            assert_eq!(references(&witness), 1 + usize::from(inherited));
            if let Some(source) = &source {
                assert_eq!(
                    rustix::io::fcntl_getfd(source).unwrap(),
                    rustix::io::FdFlags::empty()
                );
            }
            drop(source);
            assert_eq!(references(&witness), 1);
        }
    }
}

#[test]
fn first_denials_and_prior_peak_survive_success_context_error_and_nested_refusal() {
    let mut setup_work = Work::new(LIMIT);
    let mut setup = Budget::new(&mut setup_work, LIMIT);
    let p = policy(0, &mut setup);
    let wrong = policy(1, &mut setup);
    let s = supervisor(&p, 0, &mut setup);
    let d = deployment(&s, &p, &mut setup);
    let floor =
        Cap::FILE_STORAGE + s.retained_storage() + p.retained_storage() + wrong.retained_storage();
    let limit = 3 * Cap::ADMISSION_WORK - 1;
    let ceiling = floor + Cap::ADMISSION_STORAGE + EXTRA;
    let mut w = Work::new(limit);
    let mut b = Budget::new(&mut w, ceiling);
    assert!(b.charge_work(limit + 1).is_err());
    assert!(b.reserve_storage(ceiling + 1).is_err());
    b.reserve_storage(ceiling).unwrap();
    b.release_storage(ceiling - floor).unwrap();
    let ledger = b.work_ledger_identity_v1();
    for turn in 0..3 {
        let chosen = if turn == 1 { &wrong } else { &p };
        let result = Cap::from_file(sealed(d.canonical_bytes()), &s, chosen, &mut b);
        assert_eq!(b.storage(), floor);
        if turn == 0 {
            let (cap, growth) = result.unwrap();
            b.reserve_storage(growth.additional_storage()).unwrap();
            let retained = cap.retained_storage();
            drop(cap);
            b.release_storage(retained).unwrap();
        } else {
            let e = failure(result);
            if turn == 1 {
                assert!(matches!(
                    decode_error(&e),
                    DecodeError::SupervisorPolicyMismatch
                ));
            } else {
                assert!(matches!(
                    decode_error(&e),
                    DecodeError::Resource(Resource::Work(_))
                ));
            }
            b.release_storage(Cap::FILE_STORAGE).unwrap();
        }
        assert_eq!(b.storage(), floor - Cap::FILE_STORAGE);
        assert_eq!(
            (b.failed_work(), b.failed_storage()),
            (Some(limit + 1), Some(ceiling + 1))
        );
        assert_eq!(b.peak_storage(), ceiling);
        assert!(b.work_ledger_identity_v1() == ledger);
        if turn != 2 {
            b.reserve_storage(Cap::FILE_STORAGE).unwrap();
        }
    }
    assert_eq!(
        b.work(),
        2 * Cap::ADMISSION_WORK + Cap::IO_WORK + LOCAL_WORK + ENTRY
    );
}

#[test]
fn overflow_at_each_reservation_and_work_entry_keeps_accepted_history_and_closes_files() {
    let mut setup_work = Work::new(LIMIT);
    let mut setup = Budget::new(&mut setup_work, LIMIT);
    let p = policy(0, &mut setup);
    let s = supervisor(&p, 0, &mut setup);
    let d = deployment(&s, &p, &mut setup);
    for (scratch, used, peak_delta) in [
        (Cap::IO_STORAGE, Cap::IO_WORK, 0),
        (
            Cap::IO_STORAGE + LOCAL_STORAGE,
            Cap::IO_WORK + LOCAL_WORK,
            Cap::IO_STORAGE,
        ),
        (
            Cap::ADMISSION_STORAGE,
            Cap::ADMISSION_WORK,
            Cap::IO_STORAGE + LOCAL_STORAGE,
        ),
    ] {
        let floor = usize::MAX - scratch + 1;
        let mut w = Work::new(Cap::ADMISSION_WORK);
        let mut b = Budget::new(&mut w, usize::MAX);
        b.reserve_storage(floor).unwrap();
        let file = sealed(d.canonical_bytes());
        let witness = file.try_clone().unwrap();
        let e = failure(Cap::from_file(file, &s, &p, &mut b));
        assert!(matches!(resource_error(&e), Resource::Storage(_)));
        assert_eq!(
            (b.storage(), b.work(), b.peak_storage()),
            (floor, used, floor + peak_delta)
        );
        assert_eq!(b.failed_storage(), Some(usize::MAX));
        assert_eq!(references(&witness), 1);
    }
    let floor = Cap::FILE_STORAGE + s.retained_storage() + p.retained_storage();
    for prior in [
        usize::MAX - 7,
        usize::MAX - Cap::IO_WORK,
        usize::MAX - (Cap::IO_WORK + LOCAL_WORK),
    ] {
        let mut w = Work::new(usize::MAX);
        let mut b = Budget::new(&mut w, floor + Cap::ADMISSION_STORAGE);
        b.reserve_storage(floor).unwrap();
        b.charge_work(prior).unwrap();
        let file = sealed(d.canonical_bytes());
        let witness = file.try_clone().unwrap();
        let e = failure(Cap::from_file(file, &s, &p, &mut b));
        assert!(matches!(resource_error(&e), Resource::Work(_)));
        assert_eq!(b.storage(), floor);
        assert_eq!(
            b.work(),
            if prior == usize::MAX - 7 {
                prior
            } else {
                usize::MAX
            }
        );
        assert_eq!(b.failed_work(), Some(usize::MAX));
        assert_eq!(references(&witness), 1);
    }
}

#[test]
fn inherited_io_refusals_never_consume_or_change_the_source_slot() {
    let mut setup_work = Work::new(LIMIT);
    let mut setup = Budget::new(&mut setup_work, LIMIT);
    let p = policy(0, &mut setup);
    let s = supervisor(&p, 0, &mut setup);
    let d = deployment(&s, &p, &mut setup);
    let file = sealed(d.canonical_bytes());
    for mode in 0..5 {
        let flags = if mode == 0 {
            rustix::io::FdFlags::CLOEXEC
        } else {
            rustix::io::FdFlags::empty()
        };
        rustix::io::fcntl_setfd(&file, flags).unwrap();
        let mut w = Work::new(if mode == 1 { ENTRY - 1 } else { LIMIT });
        let mut b = Budget::new(&mut w, LIMIT);
        let floor = Cap::FILE_STORAGE + s.retained_storage() + p.retained_storage();
        b.reserve_storage(floor).unwrap();
        let fd = match mode {
            2 => -1,
            3 => 2,
            4 => i32::MAX,
            _ => file.as_raw_fd(),
        };
        let e = failure(Cap::from_inherited_at(fd, &s, &p, &mut b));
        match mode {
            0 => assert!(matches!(
                e,
                Error::Rejected("inherited descriptor is close-on-exec")
            )),
            1 => assert!(matches!(e, Error::Resource(Resource::Work(_)))),
            2 | 3 => assert!(matches!(
                e,
                Error::Rejected("inherited descriptor overlaps stdio")
            )),
            4 => assert!(matches!(
                e,
                Error::Io {
                    operation: "inspect inherited descriptor",
                    errno: libc::EBADF
                }
            )),
            _ => unreachable!(),
        }
        assert_eq!(
            (b.storage(), b.work()),
            (floor, if mode == 1 { 0 } else { Cap::IO_WORK })
        );
        assert_eq!(references(&file), 1);
        assert_eq!(rustix::io::fcntl_getfd(&file).unwrap(), flags);
    }
}

#[test]
fn transfers_require_exact_object_flags_and_metadata_and_preserve_borrowed_files() {
    let mut w = Work::new(LIMIT);
    let mut b = Budget::new(&mut w, LIMIT);
    let p = policy(0, &mut b);
    let s = supervisor(&p, 0, &mut b);
    let d = deployment(&s, &p, &mut b);
    let bytes = *d.canonical_bytes();
    let (cap, growth) = Cap::create(d, &mut b).unwrap();
    b.reserve_storage(growth.additional_storage()).unwrap();
    let (file, growth) = cap.try_clone_for_transfer(&mut b).unwrap();
    b.reserve_storage(growth.additional_storage()).unwrap();
    b.reserve_storage(Cap::FILE_STORAGE).unwrap();
    let other = sealed(&bytes);
    assert_ne!(object(&file), object(&other));
    let floor = b.storage();
    let before = b.work();
    assert!(matches!(
        cap.validate_transfer(&other, &mut b),
        Err(Error::Rejected(_))
    ));
    assert_eq!(references(&other), 1);
    cap.validate_transfer(&file, &mut b).unwrap();
    rustix::io::fcntl_setfd(&file, rustix::io::FdFlags::empty()).unwrap();
    assert!(matches!(
        cap.validate_transfer(&file, &mut b),
        Err(Error::Rejected(_))
    ));
    cap.revalidate(&mut b).unwrap();
    rustix::io::fcntl_setfd(&file, rustix::io::FdFlags::CLOEXEC).unwrap();
    rustix::fs::fchmod(&file, Mode::RUSR | Mode::WUSR).unwrap();
    assert!(matches!(cap.revalidate(&mut b), Err(Error::Rejected(_))));
    assert!(matches!(
        cap.try_clone_for_transfer(&mut b),
        Err(Error::Rejected(_))
    ));
    assert!(matches!(
        cap.validate_transfer(&file, &mut b),
        Err(Error::Rejected(_))
    ));
    assert_eq!(references(&file), 2);
    rustix::fs::fchmod(&file, Mode::RUSR).unwrap();
    cap.validate_transfer(&file, &mut b).unwrap();
    assert_eq!((b.storage(), b.work()), (floor, before + 8 * Cap::IO_WORK));
}

#[test]
fn malformed_images_refuse_before_native_decode_and_close_consumed_files() {
    let mut setup_work = Work::new(LIMIT);
    let mut setup = Budget::new(&mut setup_work, LIMIT);
    let p = policy(0, &mut setup);
    let s = supervisor(&p, 0, &mut setup);
    let d = deployment(&s, &p, &mut setup);
    for case in 0..5 {
        let file = match case {
            0 => sealed(&d.canonical_bytes()[..BYTES - 1]),
            1 => sealed(&[0; BYTES + 1]),
            4 => image(d.canonical_bytes(), false),
            _ => sealed(d.canonical_bytes()),
        };
        if case == 2 {
            rustix::io::fcntl_setfd(&file, rustix::io::FdFlags::empty()).unwrap();
        }
        if case == 3 {
            rustix::fs::fchmod(&file, Mode::RUSR | Mode::WUSR).unwrap();
        }
        let witness = file.try_clone().unwrap();
        let mut w = Work::new(Cap::IO_WORK);
        let floor = Cap::FILE_STORAGE + s.retained_storage() + p.retained_storage();
        let mut b = Budget::new(&mut w, floor + Cap::IO_STORAGE);
        b.reserve_storage(floor).unwrap();
        assert!(matches!(
            failure(Cap::from_file(file, &s, &p, &mut b)),
            Error::Rejected(_)
        ));
        assert_eq!(
            (b.storage(), b.work(), b.peak_storage()),
            (floor, Cap::IO_WORK, floor + Cap::IO_STORAGE)
        );
        assert_eq!(references(&witness), 1);
    }
}

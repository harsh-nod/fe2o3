use super::*;
use crate::native_capability::tests::{failure, policy, run, sealed};
use crate::sealed_image::REQUIRED_SEALS;
use fe2o3_compiler_execution_protocol::CompilerExecutionIssuerMeasurementV1 as Measurement;
use fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1 as Work;
use rustix::fs::{MemfdFlags, Mode, OFlags, SealFlags};
use rustix::io::FdFlags;
use std::{
    io::{Seek, SeekFrom},
    os::{fd::AsRawFd, unix::fs::MetadataExt},
};

type Cap = CompilerExecutionSigningKeyCapabilityV2;
const UNRELATED: usize = 19;

fn key(policy: &Policy) -> Cap {
    let mut seed = [7; KEY_BYTES];
    let (result, _, _, _) = run(
        KEY_BYTES + policy.retained_storage(),
        1_000_000,
        1_000_000,
        |b| Cap::create_and_zeroize(&mut seed, policy, b),
    );
    assert_eq!(seed, [0; KEY_BYTES]);
    result.unwrap().0
}

fn readonly_secret(seed: u8) -> File {
    SealedCapabilityImage::create_fixed(&[seed; KEY_BYTES], ROLE)
        .unwrap()
        .into_read_only_fixed::<KEY_BYTES>()
        .unwrap()
        .clone_fixed()
        .unwrap()
}

fn reopen(file: &File, access: OFlags) -> File {
    rustix::fs::open(
        format!("/proc/self/fd/{}", file.as_raw_fd()),
        access | OFlags::CLOEXEC,
        Mode::empty(),
    )
    .map(File::from)
    .unwrap()
}

fn identity(file: &File) -> (u64, u64) {
    let metadata = file.metadata().unwrap();
    (metadata.dev(), metadata.ino())
}

// Keep a witness alive: parallel tests may reuse descriptor numbers, not this inode.
pub(super) fn references(witness: &File) -> usize {
    let expected = identity(witness);
    std::fs::read_dir("/proc/self/fd")
        .unwrap()
        .filter_map(|entry| std::fs::metadata(entry.ok()?.path()).ok())
        .filter(|metadata| (metadata.dev(), metadata.ino()) == expected)
        .count()
}

fn assert_secret_file(file: &File, cloexec: bool) {
    assert_eq!(
        rustix::io::fcntl_getfd(file)
            .unwrap()
            .contains(FdFlags::CLOEXEC),
        cloexec
    );
    let flags = rustix::fs::fcntl_getfl(file).unwrap();
    assert_eq!(flags & OFlags::ACCMODE, OFlags::RDONLY);
    assert!(!flags.contains(OFlags::PATH));
    assert_eq!(rustix::fs::fcntl_get_seals(file).unwrap(), REQUIRED_SEALS);
    let metadata = file.metadata().unwrap();
    assert!(metadata.is_file());
    assert_eq!(metadata.len(), KEY_BYTES as u64);
    assert_eq!(metadata.mode() & 0o7777, 0o400);
    assert_eq!(metadata.nlink(), 0);
    assert_eq!(metadata.uid(), rustix::process::geteuid().as_raw());
    assert_eq!(metadata.gid(), rustix::process::getegid().as_raw());
}

#[test]
fn revalidation_pins_every_policy_component_even_with_the_same_signing_key() {
    let original = policy(7);
    let cap = key(&original);
    let (transfer, _) = run(cap.retained_storage(), Cap::IO_WORK, 1_000_000, |b| {
        cap.try_clone_for_transfer(b)
    })
    .0
    .unwrap();
    for changed in 0..4 {
        let (variant, _) = run(0, 100_000, 100_000, |budget| {
            Ok(Policy::new(
                original.generation() + u64::from(changed == 0),
                if changed == 1 {
                    Measurement::new([21; KEY_BYTES], 124).unwrap()
                } else {
                    original.executable()
                },
                if changed == 2 {
                    Measurement::new([22; KEY_BYTES], 457).unwrap()
                } else {
                    original.runtime()
                },
                *original.verifying_key(),
                if changed == 3 {
                    SigningKey::from_bytes(&[23; KEY_BYTES])
                        .verifying_key()
                        .to_bytes()
                } else {
                    *original.external_anchor_verifying_key()
                },
                budget,
            )?)
        })
        .0
        .unwrap();
        assert_eq!(original.verifying_key(), variant.verifying_key());
        assert_ne!(original.identity(), variant.identity());
        let floor =
            UNRELATED + cap.retained_storage() + variant.retained_storage() + Cap::FILE_STORAGE;
        let (result, work, live, _) = run(floor, 1_000_000, 1_000_000, |budget| {
            cap.revalidate(&variant, budget)
        });
        assert!(matches!(
            failure(result),
            Error::Rejected("signing key is pinned to another native policy")
        ));
        assert_eq!(work, Cap::IO_WORK);
        assert_eq!(live, floor);
        let (result, work, live, _) = run(floor, Cap::IO_WORK, 1_000_000, |budget| {
            cap.validate_transfer(&transfer, &variant, budget)
        });
        assert!(matches!(
            failure(result),
            Error::Rejected("signing key is pinned to another native policy")
        ));
        assert_eq!(work, Cap::IO_WORK);
        assert_eq!(live, floor);
        assert_eq!(references(&transfer), 2);
        assert_eq!(cap.policy_identity(), original.identity());
    }
    let floor = cap.retained_storage() + original.retained_storage();
    assert!(
        run(floor, 1_000_000, 1_000_000, |b| cap
            .revalidate(&original, b))
        .0
        .is_ok()
    );
}

#[test]
fn transfer_and_consuming_readmission_preserve_custody_on_one_ledger() {
    let mut work = Work::new(2_000_000);
    let mut budget = Budget::new(&mut work, 1_000_000);
    let charge = {
        let policy = policy(7);
        let policy_floor = UNRELATED + policy.retained_storage();
        budget.reserve_storage(policy_floor + KEY_BYTES).unwrap();
        let (cap, charge) = {
            let mut seed = [7; KEY_BYTES];
            let admitted = Cap::create_and_zeroize(&mut seed, &policy, &mut budget).unwrap();
            assert_eq!(seed, [0; KEY_BYTES]);
            admitted
        };
        assert_eq!(charge.additional_storage(), cap.retained_storage());
        budget.reserve_storage(charge.additional_storage()).unwrap();
        budget.release_storage(KEY_BYTES).unwrap();
        let retained = cap.retained_storage();
        assert_eq!(budget.storage(), policy_floor + retained);
        assert_secret_file(cap.image.as_file(), true);
        let (mut file, charge) = cap.try_clone_for_transfer(&mut budget).unwrap();
        assert_eq!(charge.additional_storage(), Cap::FILE_STORAGE);
        budget.reserve_storage(charge.additional_storage()).unwrap();
        assert_secret_file(&file, true);
        // File transport deliberately permits its trusted recipient to read the seed.
        {
            let mut seed = [0; KEY_BYTES];
            let guard = SeedGuard(&mut seed);
            assert_eq!(
                rustix::io::pread(&file, guard.0.as_mut_slice(), 0).unwrap(),
                KEY_BYTES
            );
            assert_eq!(guard.0, &[7; KEY_BYTES]);
        }
        assert_eq!(identity(&file), identity(cap.image.as_file()));
        assert_eq!(references(&file), 2);
        file.seek(SeekFrom::Start(17)).unwrap();
        cap.revalidate(&policy, &mut budget).unwrap();
        cap.validate_transfer(&file, &policy, &mut budget).unwrap();
        assert_eq!(
            budget.storage(),
            policy_floor + retained + Cap::FILE_STORAGE
        );
        assert_eq!(references(&file), 2);
        assert_eq!(file.stream_position().unwrap(), 17);
        let expected_object = identity(&file);
        drop(cap);
        budget.release_storage(retained).unwrap();
        assert_eq!(references(&file), 1);
        let (recovered, growth) = Cap::from_file(file, &policy, &mut budget).unwrap();
        assert_eq!(growth.additional_storage(), retained - Cap::FILE_STORAGE);
        assert_eq!(budget.storage(), policy_floor + Cap::FILE_STORAGE);
        budget.reserve_storage(growth.additional_storage()).unwrap();
        assert_eq!(budget.storage(), policy_floor + retained);
        assert_eq!(identity(recovered.image.as_file()), expected_object);
        assert_eq!(recovered.verifying_key(), *policy.verifying_key());
        assert_eq!(recovered.policy_identity(), policy.identity());
        assert_secret_file(recovered.image.as_file(), true);
        assert_eq!(recovered.image.as_file().stream_position().unwrap(), 17);
        assert!(rustix::io::pwrite(recovered.image.as_file(), &[0], 0).is_err());
        assert!(rustix::fs::ftruncate(recovered.image.as_file(), 0).is_err());
        recovered.revalidate(&policy, &mut budget).unwrap();
        assert_eq!(budget.work(), 2 * Cap::ADMISSION_WORK + 4 * Cap::IO_WORK);
        drop(recovered);
        budget.release_storage(retained).unwrap();
        policy.retained_storage()
    };
    budget.release_storage(charge).unwrap();
    assert_eq!(budget.storage(), UNRELATED);
}

#[test]
fn inherited_admission_owns_only_private_cloexec_duplicates_without_moving_offsets() {
    let mut work = Work::new(2_000_000);
    let mut budget = Budget::new(&mut work, 1_000_000);
    let charge = {
        let policy = policy(7);
        let mut source = readonly_secret(7);
        source.seek(SeekFrom::Start(29)).unwrap();
        let floor = UNRELATED + policy.retained_storage() + Cap::FILE_STORAGE;
        budget.reserve_storage(floor).unwrap();
        assert!(matches!(
            failure(Cap::from_inherited_at(
                source.as_raw_fd(),
                &policy,
                &mut budget
            )),
            Error::Rejected("inherited descriptor is close-on-exec")
        ));
        assert_eq!(references(&source), 1);
        assert_eq!(budget.storage(), floor);
        rustix::io::fcntl_setfd(&source, FdFlags::empty()).unwrap();
        let (first, charge) =
            Cap::from_inherited_at(source.as_raw_fd(), &policy, &mut budget).unwrap();
        assert_eq!(charge.additional_storage(), first.retained_storage());
        budget.reserve_storage(charge.additional_storage()).unwrap();
        let (second, charge) =
            Cap::from_inherited_at(source.as_raw_fd(), &policy, &mut budget).unwrap();
        let retained = second.retained_storage();
        assert_eq!(charge.additional_storage(), retained);
        budget.reserve_storage(charge.additional_storage()).unwrap();
        assert_eq!(budget.storage(), floor + 2 * retained);
        assert_eq!(references(&source), 3);
        assert_secret_file(&source, false);
        for cap in [&first, &second] {
            assert_secret_file(cap.image.as_file(), true);
            assert_ne!(cap.image.as_file().as_raw_fd(), source.as_raw_fd());
            assert_eq!(identity(cap.image.as_file()), identity(&source));
            assert_eq!(cap.policy_identity(), policy.identity());
            cap.revalidate(&policy, &mut budget).unwrap();
        }
        assert_eq!(source.stream_position().unwrap(), 29);
        drop(first);
        budget.release_storage(retained).unwrap();
        assert_eq!(references(&source), 2);
        assert_secret_file(&source, false);
        drop(source);
        budget.release_storage(Cap::FILE_STORAGE).unwrap();
        assert_eq!(references(second.image.as_file()), 1);
        second.revalidate(&policy, &mut budget).unwrap();
        assert_eq!(second.image.as_file().stream_position().unwrap(), 29);
        drop(second);
        budget.release_storage(retained).unwrap();
        policy.retained_storage()
    };
    budget.release_storage(charge).unwrap();
    assert_eq!(budget.storage(), UNRELATED);
}

fn assert_hostile_image_closes_only_owned_descriptors(file: File, policy: &Policy) {
    let witness = file.try_clone().unwrap();
    assert_eq!(references(&witness), 2);
    let mut work = Work::new(1_000_000);
    let mut budget = Budget::new(&mut work, 1_000_000);
    let floor = UNRELATED + policy.retained_storage() + 2 * Cap::FILE_STORAGE;
    budget.reserve_storage(floor).unwrap();
    assert!(matches!(
        failure(Cap::from_file(file, policy, &mut budget)),
        Error::Rejected(_) | Error::Io { .. }
    ));
    assert_eq!(budget.storage(), floor);
    assert_eq!(references(&witness), 1);
    budget.release_storage(Cap::FILE_STORAGE).unwrap();
    rustix::io::fcntl_setfd(&witness, FdFlags::empty()).unwrap();
    assert!(matches!(
        failure(Cap::from_inherited_at(
            witness.as_raw_fd(),
            policy,
            &mut budget
        )),
        Error::Rejected(_) | Error::Io { .. }
    ));
    assert_eq!(budget.storage(), floor - Cap::FILE_STORAGE);
    assert_eq!(references(&witness), 1);
    assert_eq!(rustix::io::fcntl_getfd(&witness).unwrap(), FdFlags::empty());
    drop(witness);
    budget.release_storage(Cap::FILE_STORAGE).unwrap();
    assert_eq!(budget.storage(), UNRELATED + policy.retained_storage());
}

#[test]
fn hostile_secret_images_refuse_before_retention_without_leaking_descriptors() {
    let policy = policy(7);
    assert_hostile_image_closes_only_owned_descriptors(readonly_secret(9), &policy);
    assert_hostile_image_closes_only_owned_descriptors(sealed(&[7; KEY_BYTES]), &policy);
    for length in [0, KEY_BYTES - 1, KEY_BYTES + 1] {
        let file = sealed(&vec![7; length]);
        let readonly = reopen(&file, OFlags::RDONLY);
        drop(file);
        assert_hostile_image_closes_only_owned_descriptors(readonly, &policy);
    }
    for missing in [
        SealFlags::WRITE,
        SealFlags::GROW,
        SealFlags::SHRINK,
        SealFlags::SEAL,
    ] {
        let file = rustix::fs::memfd_create(
            "native-key-missing-seal",
            MemfdFlags::CLOEXEC | MemfdFlags::ALLOW_SEALING,
        )
        .map(File::from)
        .unwrap();
        assert_eq!(
            rustix::io::pwrite(&file, &[7; KEY_BYTES], 0).unwrap(),
            KEY_BYTES
        );
        rustix::fs::fchmod(&file, Mode::RUSR).unwrap();
        rustix::fs::fcntl_add_seals(&file, REQUIRED_SEALS & !missing).unwrap();
        let readonly = reopen(&file, OFlags::RDONLY);
        drop(file);
        assert_hostile_image_closes_only_owned_descriptors(readonly, &policy);
    }
    for access in [OFlags::WRONLY, OFlags::PATH] {
        let file = sealed(&[7; KEY_BYTES]);
        rustix::fs::fchmod(&file, Mode::RUSR | Mode::WUSR).unwrap();
        let hostile = reopen(&file, access);
        rustix::fs::fchmod(&file, Mode::RUSR).unwrap();
        drop(file);
        assert_hostile_image_closes_only_owned_descriptors(hostile, &policy);
    }
    let wrong_mode = readonly_secret(7);
    rustix::fs::fchmod(&wrong_mode, Mode::RUSR | Mode::WUSR).unwrap();
    assert_hostile_image_closes_only_owned_descriptors(wrong_mode, &policy);
}

#[test]
fn inherited_invalid_slots_and_consuming_inheritable_files_are_rejected() {
    let policy = policy(7);
    let floor = UNRELATED + policy.retained_storage() + Cap::FILE_STORAGE;
    for fd in [-1, 0, 1, 2, i32::MAX] {
        let (result, _, live, _) = run(floor, 1_000_000, 1_000_000, |budget| {
            Cap::from_inherited_at(fd, &policy, budget)
        });
        assert!(matches!(
            failure(result),
            Error::Rejected(_) | Error::Io { .. }
        ));
        assert_eq!(live, floor);
    }
    let file = readonly_secret(7);
    let witness = file.try_clone().unwrap();
    rustix::io::fcntl_setfd(&file, FdFlags::empty()).unwrap();
    let (result, _, live, _) = run(floor + Cap::FILE_STORAGE, 1_000_000, 1_000_000, |b| {
        Cap::from_file(file, &policy, b)
    });
    assert!(matches!(
        failure(result),
        Error::Rejected(" descriptor is unexpectedly inheritable")
    ));
    assert_eq!(live, floor + Cap::FILE_STORAGE);
    assert_eq!(references(&witness), 1);
    assert_secret_file(&witness, true);
}

#[test]
fn retained_metadata_access_and_inode_drift_refuse_revalidation_and_transfer() {
    let policy = policy(7);
    for change in 0..4 {
        let mut cap = key(&policy);
        match change {
            0 => rustix::fs::fchmod(cap.image.as_file(), Mode::RUSR | Mode::WUSR).unwrap(),
            1 => rustix::io::fcntl_setfd(cap.image.as_file(), FdFlags::empty()).unwrap(),
            2 => cap.image.replace_file_for_test(readonly_secret(7)),
            _ => {
                rustix::fs::fchmod(cap.image.as_file(), Mode::RUSR | Mode::WUSR).unwrap();
                let writable = reopen(cap.image.as_file(), OFlags::RDWR);
                rustix::fs::fchmod(&writable, Mode::RUSR).unwrap();
                cap.image.replace_file_for_test(writable);
            }
        }
        let witness = cap.image.as_file().try_clone().unwrap();
        let mut work = Work::new(1_000_000);
        let mut budget = Budget::new(&mut work, 1_000_000);
        let floor =
            UNRELATED + cap.retained_storage() + policy.retained_storage() + Cap::FILE_STORAGE;
        budget.reserve_storage(floor).unwrap();
        assert_eq!(references(&witness), 2);
        assert!(matches!(
            failure(cap.revalidate(&policy, &mut budget)),
            Error::Rejected(_)
        ));
        assert!(matches!(
            failure(cap.try_clone_for_transfer(&mut budget)),
            Error::Rejected(_)
        ));
        assert_eq!(budget.work(), 2 * Cap::IO_WORK);
        assert_eq!(budget.storage(), floor);
        assert_eq!(references(&witness), 2);
        let retained = cap.retained_storage();
        drop(cap);
        budget.release_storage(retained).unwrap();
        assert_eq!(references(&witness), 1);
        drop(witness);
        budget.release_storage(Cap::FILE_STORAGE).unwrap();
        assert_eq!(budget.storage(), UNRELATED + policy.retained_storage());
    }
}

#[test]
fn borrowed_secret_transfer_rejects_replacement_inheritable_and_non_readonly_files() {
    let policy = policy(7);
    let cap = key(&policy);
    for change in 0..7 {
        let mut file = cap.image.clone_fixed().unwrap();
        match change {
            0 => file = readonly_secret(7),
            1 => file = readonly_secret(9),
            2 => rustix::io::fcntl_setfd(&file, FdFlags::empty()).unwrap(),
            3..=5 => {
                rustix::fs::fchmod(&file, Mode::RUSR | Mode::WUSR).unwrap();
                file = reopen(
                    &file,
                    [OFlags::RDWR, OFlags::WRONLY, OFlags::PATH][change - 3],
                );
                rustix::fs::fchmod(cap.image.as_file(), Mode::RUSR).unwrap();
            }
            _ => file = sealed(&[7; KEY_BYTES - 1]),
        }
        let floor =
            UNRELATED + cap.retained_storage() + policy.retained_storage() + Cap::FILE_STORAGE;
        let before = references(&file);
        let (result, work, live, _) = run(floor, Cap::IO_WORK, 1_000_000, |b| {
            cap.validate_transfer(&file, &policy, b)
        });
        match (change, failure(result)) {
            (0 | 1, Error::Rejected("sealed image identity or length changed"))
            | (2, Error::Rejected(" descriptor is unexpectedly inheritable"))
            | (
                3 | 4,
                Error::Rejected(
                    "sealed secret image is not an anonymous current-owner read-only image",
                ),
            )
            | (5, Error::Io { .. })
            | (6, Error::Rejected(" has an invalid length")) => {}
            (_, error) => panic!("unexpected transfer refusal for case {change}: {error:?}"),
        }
        assert_eq!(work, Cap::IO_WORK);
        assert_eq!(live, floor);
        assert_eq!(references(&file), before);
        assert_secret_file(cap.image.as_file(), true);
        assert!(
            run(floor, Cap::IO_WORK, 1_000_000, |b| cap
                .revalidate(&policy, b))
            .0
            .is_ok()
        );
    }
}

#[test]
fn borrowed_secret_transfer_rechecks_owner_identity_seed_and_descriptor_flags() {
    let policy = policy(7);
    for change in 0..3 {
        let mut cap = key(&policy);
        let transfer = cap.image.clone_fixed().unwrap();
        match change {
            0 => cap.image.replace_file_for_test(readonly_secret(7)),
            1 => cap.key = SigningKey::from_bytes(&[8; KEY_BYTES]),
            _ => rustix::io::fcntl_setfd(cap.image.as_file(), FdFlags::empty()).unwrap(),
        }
        let floor = cap.retained_storage() + policy.retained_storage() + Cap::FILE_STORAGE;
        let before = references(&transfer);
        let (result, work, live, _) = run(floor, Cap::IO_WORK, 1_000_000, |b| {
            cap.validate_transfer(&transfer, &policy, b)
        });
        let expected = [
            "sealed image identity or length changed",
            "signing-key bytes changed",
            " descriptor is unexpectedly inheritable",
        ][change];
        assert!(matches!(failure(result), Error::Rejected(reason) if reason == expected));
        assert_eq!(work, Cap::IO_WORK);
        assert_eq!(live, floor);
        assert_eq!(references(&transfer), before);
        assert_secret_file(&transfer, true);
    }
}

#[test]
fn debug_exposes_policy_custody_but_no_seed_descriptor_or_internal_key() {
    let policy = policy(7);
    let cap = key(&policy);
    let debug = format!("{cap:?}");
    assert!(debug.contains("CompilerExecutionSigningKeyCapabilityV2"));
    assert!(debug.contains("signing-key-custody-only"));
    assert!(debug.contains(&format!("{:?}", policy.identity())));
    for secret in [
        format!("{:?}", [7; KEY_BYTES]),
        "07".repeat(KEY_BYTES),
        "/proc/self/fd/".to_owned(),
        "secret_key".to_owned(),
        "File {".to_owned(),
        "image:".to_owned(),
    ] {
        assert!(!debug.contains(&secret));
    }
}

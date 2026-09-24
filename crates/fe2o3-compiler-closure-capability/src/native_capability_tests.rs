use super::*;
use crate::sealed_image::REQUIRED_SEALS;
use crate::{CompilerExecutionClientProfileCapabilityV1, CompilerExecutionPolicyCapabilityV1};
use ed25519_dalek::SigningKey;
use fe2o3_compiler_execution_protocol::{
    COMPILER_EXECUTION_ISSUER_POLICY_STORAGE_V2 as POLICY_STORAGE,
    COMPILER_EXECUTION_ISSUER_POLICY_WORK_V2 as POLICY_WORK,
    CompilerExecutionClientProfileV1 as LegacyProfile, CompilerExecutionClientProfileV2 as Profile,
    CompilerExecutionExternalAnchorServiceIdentityV1 as Service,
    CompilerExecutionIssuerMeasurementV1 as Measurement,
    CompilerExecutionIssuerPolicyV1 as LegacyPolicy, CompilerExecutionIssuerPolicyV2 as Policy,
};
use fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1 as Work;
use std::{
    io::{Seek, SeekFrom},
    os::{
        fd::AsRawFd,
        unix::fs::{MetadataExt, PermissionsExt},
    },
};

type Cap = NativeCapability<Policy, 216>;
type ProfileCap = NativeCapability<Profile, 280>;
pub(crate) fn policy(seed: u8) -> Policy {
    let legacy = legacy_policy(seed);
    let mut work = Work::new(100_000);
    let mut budget = Budget::new(&mut work, 100_000);
    Policy::new(
        legacy.generation(),
        legacy.executable(),
        legacy.runtime(),
        *legacy.verifying_key(),
        *legacy.external_anchor_verifying_key(),
        &mut budget,
    )
    .unwrap()
    .0
}
pub(crate) fn legacy_policy(seed: u8) -> LegacyPolicy {
    LegacyPolicy::new(
        u64::from(seed),
        Measurement::new([seed; 32], 123).unwrap(),
        Measurement::new([seed + 1; 32], 456).unwrap(),
        SigningKey::from_bytes(&[seed; 32])
            .verifying_key()
            .to_bytes(),
        SigningKey::from_bytes(&[seed + 1; 32])
            .verifying_key()
            .to_bytes(),
    )
    .unwrap()
}
pub(crate) fn profile(seed: u8) -> Profile {
    let policy = policy(seed);
    let mut work = Work::new(100_000);
    let mut budget = Budget::new(&mut work, 100_000);
    budget.reserve_storage(policy.retained_storage()).unwrap();
    Profile::new(
        1000,
        1000,
        Service::new(1001, 1001).unwrap(),
        policy,
        &mut budget,
    )
    .unwrap()
    .0
}
pub(crate) fn legacy_profile(seed: u8) -> LegacyProfile {
    LegacyProfile::new(
        1000,
        1000,
        Service::new(1001, 1001).unwrap(),
        legacy_policy(seed),
    )
    .unwrap()
}
pub(crate) fn sealed(bytes: &[u8]) -> File {
    let file = rustix::fs::memfd_create(
        "native-capability-test",
        rustix::fs::MemfdFlags::CLOEXEC | rustix::fs::MemfdFlags::ALLOW_SEALING,
    )
    .map(File::from)
    .unwrap();
    assert_eq!(rustix::io::pwrite(&file, bytes, 0).unwrap(), bytes.len());
    rustix::fs::fchmod(&file, rustix::fs::Mode::RUSR).unwrap();
    rustix::fs::fcntl_add_seals(&file, REQUIRED_SEALS).unwrap();
    file
}
pub(crate) fn failure<T>(result: Result<T>) -> CompilerExecutionCapabilityErrorV2 {
    match result {
        Ok(_) => panic!("expected refusal"),
        Err(error) => error,
    }
}
pub(crate) fn run<T>(
    floor: usize,
    work: usize,
    storage: usize,
    operation: impl FnOnce(&mut Budget<'_>) -> Result<T>,
) -> (Result<T>, usize, usize, usize) {
    let mut meter = Work::new(work);
    let mut budget = Budget::new(&mut meter, storage);
    budget.reserve_storage(floor).unwrap();
    let result = operation(&mut budget);
    (
        result,
        budget.work(),
        budget.storage(),
        budget.peak_storage(),
    )
}

pub(crate) fn transfer_boundaries(
    floor: usize,
    work: usize,
    scratch: usize,
    validate: impl Fn(&mut Budget<'_>) -> Result<()>,
) {
    for case in 0..6 {
        let prepaid = match case {
            1 => floor - 1,
            5 => floor + 19,
            _ => floor,
        };
        let work_limit = match case {
            0 => ENTRY_WORK - 1,
            2 => work - 1,
            _ => work,
        };
        let storage_limit = prepaid + scratch - usize::from(case == 3);
        let mut meter = Work::new(work_limit);
        {
            let mut budget = Budget::new(&mut meter, storage_limit);
            budget.reserve_storage(prepaid).unwrap();
            let ledger = budget.work_ledger_identity_v1();
            let result = validate(&mut budget);
            assert!(budget.work_ledger_identity_v1() == ledger);
            assert_eq!(budget.storage(), prepaid);
            assert_eq!(
                budget.work(),
                match case {
                    0 => 0,
                    1 | 2 => ENTRY_WORK,
                    _ => work,
                }
            );
            assert_eq!(
                budget.peak_storage(),
                prepaid + if case >= 4 { scratch } else { 0 }
            );
            match case {
                0 | 2 => {
                    assert!(matches!(
                        failure(result),
                        CompilerExecutionCapabilityErrorV2::Resource(Resource::Work(_))
                    ));
                    assert!(budget.charge_work(work + 1).is_err());
                }
                1 => assert!(matches!(
                    failure(result),
                    CompilerExecutionCapabilityErrorV2::Resource(Resource::Accounting)
                )),
                3 => {
                    assert!(matches!(
                        failure(result),
                        CompilerExecutionCapabilityErrorV2::Resource(Resource::Storage(_))
                    ));
                    assert_eq!(budget.failed_storage(), Some(prepaid + scratch));
                    assert!(budget.reserve_storage(scratch + 1).is_err());
                }
                _ => result.unwrap(),
            }
            budget.release_storage(prepaid).unwrap();
            budget.reserve_storage(1).unwrap();
            budget.charge_work(0).unwrap();
            assert_eq!(
                budget.failed_storage(),
                (case == 3).then_some(prepaid + scratch)
            );
        }
        assert_eq!(
            meter.failed_work(),
            match case {
                0 => Some(ENTRY_WORK),
                2 => Some(work),
                _ => None,
            }
        );
    }
}

#[test]
fn policy_ownership_chain_preserves_inode_bytes_offsets_and_exact_charges() {
    let p = policy(7);
    let expected = *p.canonical_bytes();
    let mut work = Work::new(1_000_000);
    let mut budget = Budget::new(&mut work, 1_000_000);
    budget.reserve_storage(13 + p.retained_storage()).unwrap();
    let (cap, growth) = Cap::create(p, &mut budget).unwrap();
    budget.reserve_storage(growth.additional_storage()).unwrap();
    assert_eq!(budget.storage(), 13 + Cap::RETAINED);
    assert_eq!(budget.work(), Cap::IO_WORK);
    assert_eq!(
        rustix::fs::fcntl_getfl(cap.image.as_file()).unwrap() & rustix::fs::OFlags::ACCMODE,
        rustix::fs::OFlags::RDWR
    );
    let (mut transfer, charge) = cap.try_clone_for_transfer(&mut budget).unwrap();
    assert_eq!(charge.additional_storage(), Cap::FILE_STORAGE);
    budget.reserve_storage(charge.additional_storage()).unwrap();
    assert!(
        rustix::io::fcntl_getfd(&transfer)
            .unwrap()
            .contains(rustix::io::FdFlags::CLOEXEC)
    );
    let identity = (
        transfer.metadata().unwrap().dev(),
        transfer.metadata().unwrap().ino(),
    );
    assert_eq!(
        identity,
        (
            cap.image.as_file().metadata().unwrap().dev(),
            cap.image.as_file().metadata().unwrap().ino()
        )
    );
    transfer.seek(SeekFrom::Start(77)).unwrap();
    cap.revalidate(&mut budget).unwrap();
    assert_eq!(transfer.stream_position().unwrap(), 77);
    drop(cap);
    budget.release_storage(Cap::RETAINED).unwrap();
    let (recovered, growth) = Cap::from_file(transfer, &mut budget).unwrap();
    budget.reserve_storage(growth.additional_storage()).unwrap();
    assert_eq!(budget.storage(), 13 + Cap::RETAINED);
    assert_eq!(budget.work(), 4 * Cap::IO_WORK + POLICY_WORK);
    assert_eq!(recovered.record.canonical_bytes(), &expected);
    assert_eq!(
        identity,
        (
            recovered.image.as_file().metadata().unwrap().dev(),
            recovered.image.as_file().metadata().unwrap().ino()
        )
    );
    assert_eq!(
        rustix::io::pwrite(recovered.image.as_file(), &[0], 0),
        Err(rustix::io::Errno::PERM)
    );
    assert_eq!(
        rustix::fs::ftruncate(recovered.image.as_file(), 1),
        Err(rustix::io::Errno::PERM)
    );
    drop(recovered);
    budget.release_storage(Cap::RETAINED).unwrap();
    assert_eq!(budget.storage(), 13);
}

#[test]
fn exact_create_work_and_storage_denials_preserve_the_input_floor() {
    for which in 0..5 {
        let p = policy(7);
        let full = p.retained_storage();
        let floor = if which == 0 { full - 1 } else { full + 13 };
        let work = if which == 1 {
            Cap::IO_WORK - 1
        } else {
            Cap::IO_WORK
        };
        let storage = floor + Cap::IO_STORAGE - usize::from(which == 2);
        let (result, accepted, live, peak) = run(floor, work, storage, |b| Cap::create(p, b));
        assert_eq!(live, floor);
        match which {
            0 => {
                assert!(matches!(
                    failure(result),
                    CompilerExecutionCapabilityErrorV2::Resource(Resource::Accounting)
                ));
                assert_eq!(accepted, ENTRY_WORK);
            }
            1 => {
                assert!(matches!(
                    failure(result),
                    CompilerExecutionCapabilityErrorV2::Resource(Resource::Work(_))
                ));
                assert_eq!(accepted, ENTRY_WORK);
            }
            2 => {
                assert!(matches!(
                    failure(result),
                    CompilerExecutionCapabilityErrorV2::Resource(Resource::Storage(_))
                ));
                assert_eq!(accepted, Cap::IO_WORK);
            }
            _ => {
                let (_, delta) = result.unwrap();
                assert_eq!(delta.additional_storage(), Cap::RETAINED - full);
                assert_eq!(peak, storage);
            }
        }
    }
}

#[test]
fn nested_decode_charges_the_real_shared_ledger_at_exact_boundaries() {
    let bytes = *policy(9).canonical_bytes();
    let floor = 13 + Cap::FILE_STORAGE;
    let peak = floor + Cap::IO_STORAGE + POLICY_STORAGE;
    for which in 0..4 {
        let work = Cap::IO_WORK + POLICY_WORK - usize::from(which == 0);
        let storage = peak - usize::from(which == 1);
        let (result, accepted, live, observed_peak) =
            run(floor, work, storage, |b| Cap::from_file(sealed(&bytes), b));
        assert_eq!(live, floor);
        match which {
            0 => {
                assert!(matches!(
                    failure(result),
                    CompilerExecutionCapabilityErrorV2::Policy(
                        CompilerExecutionAttestationErrorV2::Resource(Resource::Work(_))
                    )
                ));
                assert_eq!(accepted, Cap::IO_WORK + ENTRY_WORK);
            }
            1 => {
                assert!(matches!(
                    failure(result),
                    CompilerExecutionCapabilityErrorV2::Policy(
                        CompilerExecutionAttestationErrorV2::Resource(Resource::Storage(_))
                    )
                ));
                assert_eq!(accepted, Cap::IO_WORK + POLICY_WORK);
            }
            _ => {
                let (_, delta) = result.unwrap();
                assert_eq!(
                    delta.additional_storage(),
                    Cap::RETAINED - Cap::FILE_STORAGE
                );
                assert_eq!(observed_peak, peak);
            }
        }
    }
}

#[test]
fn both_same_length_legacy_families_are_rejected_in_both_directions() {
    let p1 = legacy_policy(7);
    let p2 = policy(7);
    let c1 = legacy_profile(7);
    let c2 = profile(7);
    assert_eq!(p1.canonical_bytes().len(), p2.canonical_bytes().len());
    assert_eq!(c1.canonical_bytes().len(), c2.canonical_bytes().len());
    let (result, _, live, _) = run(Cap::FILE_STORAGE, 1_000_000, 1_000_000, |b| {
        Cap::from_file(sealed(p1.canonical_bytes()), b)
    });
    assert!(matches!(
        failure(result),
        CompilerExecutionCapabilityErrorV2::Policy(_)
    ));
    assert_eq!(live, Cap::FILE_STORAGE);
    assert!(CompilerExecutionPolicyCapabilityV1::from_file(sealed(p2.canonical_bytes())).is_err());
    let (result, _, _, _) = run(ProfileCap::FILE_STORAGE, 1_000_000, 1_000_000, |b| {
        ProfileCap::from_file(sealed(c1.canonical_bytes()), b)
    });
    assert!(matches!(
        failure(result),
        CompilerExecutionCapabilityErrorV2::Profile(_)
    ));
    assert!(
        CompilerExecutionClientProfileCapabilityV1::from_file(sealed(c2.canonical_bytes()))
            .is_err()
    );
}

#[test]
fn even_identical_bytes_cannot_replace_a_retained_sealed_object() {
    for seed in [7, 9] {
        let p = policy(7);
        let (result, _, _, _) = run(p.retained_storage(), 1_000_000, 1_000_000, |b| {
            Cap::create(p, b)
        });
        let (mut cap, _) = result.unwrap();
        cap.image
            .replace_file_for_test(sealed(policy(seed).canonical_bytes()));
        let (result, _, live, _) = run(Cap::RETAINED, 1_000_000, 1_000_000, |b| cap.revalidate(b));
        assert!(matches!(
            failure(result),
            CompilerExecutionCapabilityErrorV2::Rejected("sealed image identity or length changed")
        ));
        assert_eq!(live, Cap::RETAINED);
    }
}

#[test]
fn seal_mode_length_and_descriptor_precedence_matches_legacy_validation() {
    for (mode, len, seals, cloexec, expected) in [
        (
            0o600,
            215,
            false,
            false,
            " is not an exact regular mode-0400 file",
        ),
        (0o400, 215, false, false, " has an invalid length"),
        (0o400, 216, false, false, " is not exactly immutable"),
        (
            0o400,
            216,
            true,
            false,
            " descriptor is unexpectedly inheritable",
        ),
    ] {
        let file = rustix::fs::memfd_create(
            "paired-defect",
            rustix::fs::MemfdFlags::CLOEXEC | rustix::fs::MemfdFlags::ALLOW_SEALING,
        )
        .map(File::from)
        .unwrap();
        rustix::fs::ftruncate(&file, len).unwrap();
        file.set_permissions(std::fs::Permissions::from_mode(mode))
            .unwrap();
        if seals {
            rustix::fs::fcntl_add_seals(&file, REQUIRED_SEALS).unwrap();
        }
        if !cloexec {
            rustix::io::fcntl_setfd(&file, rustix::io::FdFlags::empty()).unwrap();
        }
        let legacy_error = failure_legacy(CompilerExecutionPolicyCapabilityV1::from_file(
            file.try_clone().unwrap(),
        ));
        // try_clone restores CLOEXEC, so freeze the first three shared faults.
        if cloexec || !seals {
            assert!(legacy_error.ends_with(expected));
        }
        let (result, _, _, _) = run(Cap::FILE_STORAGE, 1_000_000, 1_000_000, |b| {
            Cap::from_file(file, b)
        });
        assert!(
            matches!(failure(result), CompilerExecutionCapabilityErrorV2::Rejected(reason) if reason == expected)
        );
    }
}
fn failure_legacy<T>(result: std::result::Result<T, String>) -> String {
    match result {
        Ok(_) => panic!("expected legacy refusal"),
        Err(error) => error,
    }
}

#[test]
fn inherited_policy_owns_only_its_private_duplicate() {
    let source = sealed(policy(7).canonical_bytes());
    let fd = source.as_raw_fd();
    let floor = 13 + Cap::FILE_STORAGE;
    let (result, _, _, _) = run(floor, 1_000_000, 1_000_000, |b| {
        Cap::from_inherited_at(fd, b)
    });
    assert!(matches!(
        failure(result),
        CompilerExecutionCapabilityErrorV2::Rejected("inherited descriptor is close-on-exec")
    ));
    rustix::io::fcntl_setfd(&source, rustix::io::FdFlags::empty()).unwrap();
    let (result, work, live, _) = run(floor, 1_000_000, 1_000_000, |b| {
        Cap::from_inherited_at(fd, b)
    });
    let (cap, charge) = result.unwrap();
    assert_eq!(charge.additional_storage(), Cap::RETAINED);
    assert_eq!(live, floor);
    assert_eq!(work, Cap::IO_WORK + POLICY_WORK);
    assert!(
        !rustix::io::fcntl_getfd(&source)
            .unwrap()
            .contains(rustix::io::FdFlags::CLOEXEC)
    );
    assert!(
        rustix::io::fcntl_getfd(cap.image.as_file())
            .unwrap()
            .contains(rustix::io::FdFlags::CLOEXEC)
    );
    assert_ne!(fd, cap.image.as_file().as_raw_fd());
    drop(source);
    assert!(
        run(Cap::RETAINED, 1_000_000, 1_000_000, |b| cap.revalidate(b))
            .0
            .is_ok()
    );
    for invalid in [-1, 0, 1, 2, i32::MAX] {
        let (result, _, _, _) = run(floor, 1_000_000, 1_000_000, |b| {
            Cap::from_inherited_at(invalid, b)
        });
        assert!(result.is_err());
    }
}

#[test]
fn revalidation_and_transfer_refuse_mutable_metadata_without_redecoding() {
    for change_flags in [false, true] {
        let p = policy(7);
        let (cap, _) = run(p.retained_storage(), 1_000_000, 1_000_000, |b| {
            Cap::create(p, b)
        })
        .0
        .unwrap();
        if change_flags {
            rustix::io::fcntl_setfd(cap.image.as_file(), rustix::io::FdFlags::empty()).unwrap();
        } else {
            rustix::fs::fchmod(
                cap.image.as_file(),
                rustix::fs::Mode::RUSR | rustix::fs::Mode::WUSR,
            )
            .unwrap();
        }
        let (result, work, floor, _) = run(Cap::RETAINED, 1_000_000, 1_000_000, |b| {
            cap.try_clone_for_transfer(b)
        });
        assert!(matches!(
            failure(result),
            CompilerExecutionCapabilityErrorV2::Rejected(_)
        ));
        assert_eq!(work, Cap::IO_WORK);
        assert_eq!(floor, Cap::RETAINED);
    }
}

#[test]
fn nonregular_and_unreadable_descriptors_fail_without_blocking_content_reads() {
    let (socket, _) = std::os::unix::net::UnixStream::pair().unwrap();
    let ordinary = File::open("/dev/null").unwrap();
    for file in [
        File::from(std::os::fd::OwnedFd::from(socket)),
        File::open("/").unwrap(),
        ordinary,
    ] {
        let (result, _, live, _) = run(Cap::FILE_STORAGE, 1_000_000, 1_000_000, |b| {
            Cap::from_file(file, b)
        });
        assert!(matches!(
            failure(result),
            CompilerExecutionCapabilityErrorV2::Rejected(" is not an exact regular mode-0400 file")
        ));
        assert_eq!(live, Cap::FILE_STORAGE);
    }
}

#[test]
fn borrowed_operations_require_the_complete_owner_and_exact_outer_budget() {
    let p = policy(7);
    let (cap, _) = run(p.retained_storage(), 1_000_000, 1_000_000, |b| {
        Cap::create(p, b)
    })
    .0
    .unwrap();
    for transfer in [false, true] {
        for case in 0..4 {
            let floor = Cap::RETAINED - usize::from(case == 0);
            let work = Cap::IO_WORK - usize::from(case == 1);
            let storage = floor + Cap::IO_STORAGE - usize::from(case == 2);
            let (result, used, live, _) = run(floor, work, storage, |budget| {
                if transfer {
                    cap.try_clone_for_transfer(budget).map(|_| ())
                } else {
                    cap.revalidate(budget)
                }
            });
            assert_eq!(live, floor);
            match case {
                0 => assert!(matches!(
                    failure(result),
                    CompilerExecutionCapabilityErrorV2::Resource(Resource::Accounting)
                )),
                1 => assert!(matches!(
                    failure(result),
                    CompilerExecutionCapabilityErrorV2::Resource(Resource::Work(_))
                )),
                2 => assert!(matches!(
                    failure(result),
                    CompilerExecutionCapabilityErrorV2::Resource(Resource::Storage(_))
                )),
                _ => assert!(result.is_ok()),
            }
            assert_eq!(used, if case < 2 { ENTRY_WORK } else { Cap::IO_WORK });
        }
    }
}

#[test]
fn borrowed_transfer_rechecks_retained_owner_before_accepting_original_alias() {
    for replacement_seed in [7, 9] {
        let p = policy(7);
        let (mut cap, _) = run(p.retained_storage(), Cap::IO_WORK, 1_000_000, |b| {
            Cap::create(p, b)
        })
        .0
        .unwrap();
        let (transfer, _) = run(Cap::RETAINED, Cap::IO_WORK, 1_000_000, |b| {
            cap.try_clone_for_transfer(b)
        })
        .0
        .unwrap();
        cap.image
            .replace_file_for_test(sealed(policy(replacement_seed).canonical_bytes()));
        let floor = Cap::RETAINED + Cap::FILE_STORAGE;
        let (result, work, live, _) = run(floor, Cap::IO_WORK, 1_000_000, |b| {
            cap.validate_transfer(&transfer, b)
        });
        assert!(matches!(
            failure(result),
            CompilerExecutionCapabilityErrorV2::Rejected("sealed image identity or length changed")
        ));
        assert_eq!(work, Cap::IO_WORK);
        assert_eq!(live, floor);
        assert_eq!(transfer.metadata().unwrap().len(), 216);
    }
}

#[test]
fn every_required_seal_is_mandatory_and_unreadable_aliases_are_rejected() {
    let bytes = *policy(7).canonical_bytes();
    for missing in [
        rustix::fs::SealFlags::WRITE,
        rustix::fs::SealFlags::GROW,
        rustix::fs::SealFlags::SHRINK,
        rustix::fs::SealFlags::SEAL,
    ] {
        let file = rustix::fs::memfd_create(
            "missing-seal",
            rustix::fs::MemfdFlags::CLOEXEC | rustix::fs::MemfdFlags::ALLOW_SEALING,
        )
        .map(File::from)
        .unwrap();
        assert_eq!(rustix::io::pwrite(&file, &bytes, 0).unwrap(), bytes.len());
        rustix::fs::fchmod(&file, rustix::fs::Mode::RUSR).unwrap();
        rustix::fs::fcntl_add_seals(&file, REQUIRED_SEALS & !missing).unwrap();
        assert!(matches!(
            failure(
                run(Cap::FILE_STORAGE, 1_000_000, 1_000_000, |b| Cap::from_file(
                    file, b
                ))
                .0
            ),
            CompilerExecutionCapabilityErrorV2::Rejected(" is not exactly immutable")
        ));
    }
    let file = sealed(&bytes);
    let path = format!("/proc/self/fd/{}", file.as_raw_fd());
    for flags in [rustix::fs::OFlags::PATH, rustix::fs::OFlags::WRONLY] {
        // O_WRONLY cannot be reopened by an unprivileged user at mode0400.
        rustix::fs::fchmod(&file, rustix::fs::Mode::RUSR | rustix::fs::Mode::WUSR).unwrap();
        let alias = rustix::fs::open(
            &path,
            flags | rustix::fs::OFlags::CLOEXEC,
            rustix::fs::Mode::empty(),
        )
        .map(File::from)
        .unwrap();
        rustix::fs::fchmod(&file, rustix::fs::Mode::RUSR).unwrap();
        assert!(matches!(
            failure(
                run(Cap::FILE_STORAGE, 1_000_000, 1_000_000, |b| Cap::from_file(
                    alias, b
                ))
                .0
            ),
            CompilerExecutionCapabilityErrorV2::Io { .. }
        ));
    }
}

#[test]
fn consuming_refusals_close_inputs_and_inherited_refusals_drop_only_duplicates() {
    // A witness keeps this unique inode alive. Other parallel tests may reuse
    // fd numbers, but cannot acquire this inode; count identity, not fd slots.
    fn aliases(witness: &File) -> usize {
        let identity = witness.metadata().unwrap();
        std::fs::read_dir("/proc/self/fd")
            .unwrap()
            .filter_map(|entry| {
                entry
                    .ok()
                    .and_then(|entry| std::fs::metadata(entry.path()).ok())
            })
            .filter(|m| m.dev() == identity.dev() && m.ino() == identity.ino())
            .count()
    }
    for work in [ENTRY_WORK - 1, 1_000_000] {
        let file = sealed(legacy_policy(7).canonical_bytes());
        let witness = file.try_clone().unwrap();
        assert_eq!(aliases(&witness), 2);
        assert!(
            run(Cap::FILE_STORAGE, work, 1_000_000, |b| Cap::from_file(
                file, b
            ))
            .0
            .is_err()
        );
        assert_eq!(aliases(&witness), 1);
    }
    let source = sealed(legacy_policy(7).canonical_bytes());
    rustix::io::fcntl_setfd(&source, rustix::io::FdFlags::empty()).unwrap();
    assert_eq!(aliases(&source), 1);
    assert!(
        run(Cap::FILE_STORAGE, 1_000_000, 1_000_000, |b| {
            Cap::from_inherited_at(source.as_raw_fd(), b)
        })
        .0
        .is_err()
    );
    assert_eq!(aliases(&source), 1);
    assert_eq!(
        rustix::io::fcntl_getfd(&source).unwrap(),
        rustix::io::FdFlags::empty()
    );
}

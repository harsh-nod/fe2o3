// Included by both nominal families. Private expected-owner tests are rootless
// mechanics only: they do not exercise a root-provisioned deployment or process.
use super::*;
use fe2o3_compiler_execution_protocol::{
    CompilerExecutionExternalAnchorServiceIdentityV1 as Anchor,
    CompilerExecutionIssuerMeasurementV1 as Measurement,
    CompilerExecutionSupervisorDeploymentErrorV2 as DeploymentErrorV2,
    CompilerExecutionSupervisorDeploymentErrorV3 as DeploymentErrorV3,
};
use fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1 as Work;
use rustix::fs::{Mode, OFlags};
use std::{error::Error as _, os::unix::fs::MetadataExt};

const EXTRA: usize = 19;
const LIMIT: usize = 4_000_000;
const OUTER_WORK: usize = Cap::REISSUE_WORK - DEPLOYMENT_WORK;

fn rootless_service() -> Option<(u32, u32)> {
    let uid = rustix::process::geteuid().as_raw();
    let gid = rustix::process::getegid().as_raw();
    (uid != 0 && gid != 0).then_some((uid, gid))
}

fn different(value: u32) -> u32 {
    if value == 1 { 2 } else { 1 }
}

fn policy(variation: u8, budget: &mut Budget<'_>) -> Policy {
    let (policy, delta) = Policy::new(
        if variation == 1 { 8 } else { 7 },
        Measurement::new([if variation == 2 { 21 } else { 11 }; 32], 123).unwrap(),
        Measurement::new([if variation == 3 { 22 } else { 12 }; 32], 456).unwrap(),
        SigningKey::from_bytes(&[7; KEY_BYTES])
            .verifying_key()
            .to_bytes(),
        SigningKey::from_bytes(&[if variation == 4 { 9 } else { 8 }; KEY_BYTES])
            .verifying_key()
            .to_bytes(),
        budget,
    )
    .unwrap();
    budget.reserve_storage(delta.additional_storage()).unwrap();
    policy
}

fn deployment(policy: &Policy, uid: u32, gid: u32, budget: &mut Budget<'_>) -> Deployment {
    let (deployment, delta) = Deployment::new(
        uid,
        gid,
        Anchor::new(different(uid), 1).unwrap(),
        Measurement::new([31; 32], 4096).unwrap(),
        Measurement::new([32; 32], 8192).unwrap(),
        policy,
        budget,
    )
    .unwrap();
    budget.reserve_storage(delta.additional_storage()).unwrap();
    deployment
}

// Fixed local fixtures, not root templates or admitted provisioning provenance.
fn template(seed: u8) -> File {
    SealedCapabilityImage::create_fixed(&[seed; KEY_BYTES], ROLE)
        .unwrap()
        .into_read_only_fixed::<KEY_BYTES>()
        .unwrap()
        .clone_fixed()
        .unwrap()
}

fn references(witness: &File) -> usize {
    let expected = witness.metadata().unwrap();
    std::fs::read_dir("/proc/self/fd")
        .unwrap()
        .filter_map(|entry| std::fs::metadata(entry.ok()?.path()).ok())
        .filter(|m| (m.dev(), m.ino()) == (expected.dev(), expected.ino()))
        .count()
}

fn assert_rejected<T>(result: Result<T>, expected: &'static str) {
    match result {
        Err(Error::Rejected(reason)) => assert_eq!(reason, expected),
        Err(error) => panic!("unexpected rejection: {error:?}"),
        Ok(_) => panic!("unexpected acceptance"),
    }
}

fn assert_deployment_resource(error: &Error, storage: bool) {
    let resource = match error {
        Error::Deployment(DeploymentErrorV2::Resource(resource)) => resource,
        Error::DeploymentV3(DeploymentErrorV3::Resource(resource)) => resource,
        _ => panic!("native deployment cause was lost: {error:?}"),
    };
    if storage {
        assert!(matches!(resource, Resource::Storage(_)));
    } else {
        assert!(matches!(resource, Resource::Work(_)));
    }
    let source = error.source().unwrap();
    assert!(source.is::<DeploymentErrorV2>() || source.is::<DeploymentErrorV3>());
}

#[test]
fn private_expected_owner_reissue_creates_fresh_custody_on_the_original_ledger() {
    let Some((uid, gid)) = rootless_service() else {
        return;
    };
    let mut work = Work::new(LIMIT);
    let mut budget = Budget::new(&mut work, LIMIT);
    budget.reserve_storage(EXTRA).unwrap();
    let policy = policy(0, &mut budget);
    let deployment = deployment(&policy, uid, gid, &mut budget);
    let image = template(7);
    let witness = image.try_clone().unwrap();
    rustix::fs::seek(&image, rustix::fs::SeekFrom::Start(17)).unwrap();
    budget.reserve_storage(Cap::FILE_STORAGE).unwrap();
    let floor = budget.storage();
    let before = budget.work();
    let ledger = budget.work_ledger_identity_v1();
    let mut seed = [0xa5; KEY_BYTES];
    let (cap, delta) = Cap::reissue_template_for_current_service(
        image,
        &deployment,
        &policy,
        (uid, gid),
        &mut seed,
        &mut budget,
    )
    .unwrap();
    assert_eq!(seed, [0; KEY_BYTES]);
    assert_eq!(
        (budget.storage(), budget.work()),
        (floor, before + Cap::REISSUE_WORK)
    );
    assert!(budget.work_ledger_identity_v1() == ledger);
    assert_eq!(references(&witness), 1);
    assert_eq!(
        rustix::fs::seek(&witness, rustix::fs::SeekFrom::Current(0)).unwrap(),
        17
    );
    assert_eq!(
        delta.additional_storage() + Cap::FILE_STORAGE,
        cap.retained_storage()
    );
    budget.reserve_storage(delta.additional_storage()).unwrap();
    assert_eq!(cap.policy_identity(), policy.identity());
    assert_eq!(&cap.verifying_key(), policy.verifying_key());
    let metadata = cap.image.as_file().metadata().unwrap();
    let source = witness.metadata().unwrap();
    assert_ne!(
        (metadata.dev(), metadata.ino()),
        (source.dev(), source.ino())
    );
    assert_eq!(
        (metadata.uid(), metadata.gid(), metadata.nlink()),
        (uid, gid, 0)
    );
    assert_eq!(metadata.mode(), libc::S_IFREG | 0o400);
    assert_eq!(metadata.len(), KEY_BYTES as u64);
    assert_eq!(
        rustix::fs::fcntl_getfl(cap.image.as_file()).unwrap() & OFlags::ACCMODE,
        OFlags::RDONLY
    );
    assert_eq!(
        rustix::io::fcntl_getfd(cap.image.as_file()).unwrap(),
        rustix::io::FdFlags::CLOEXEC
    );
    assert_eq!(
        rustix::fs::fcntl_get_seals(cap.image.as_file()).unwrap(),
        crate::sealed_image::REQUIRED_SEALS
    );
    cap.revalidate(&policy, &mut budget).unwrap();
    let retained =
        cap.retained_storage() + deployment.retained_storage() + policy.retained_storage();
    drop((cap, deployment, policy));
    budget.release_storage(retained).unwrap();
    assert_eq!(budget.storage(), EXTRA);
}

#[test]
fn public_reissue_rejects_a_service_owned_template_even_for_exact_inert_records() {
    let Some((uid, gid)) = rootless_service() else {
        return;
    };
    let mut work = Work::new(LIMIT);
    let mut budget = Budget::new(&mut work, LIMIT);
    let policy = policy(0, &mut budget);
    let deployment = deployment(&policy, uid, gid, &mut budget);
    let image = template(7);
    let witness = image.try_clone().unwrap();
    budget.reserve_storage(Cap::FILE_STORAGE).unwrap();
    let floor = budget.storage();
    let before = budget.work();
    assert_rejected(
        Cap::reissue_root_template_for_current_service(image, &deployment, &policy, &mut budget),
        "sealed secret image is not an anonymous expected-owner read-only image",
    );
    assert_eq!(
        (budget.storage(), budget.work()),
        (floor, before + Cap::REISSUE_WORK)
    );
    assert_eq!(references(&witness), 1);
    budget.release_storage(Cap::FILE_STORAGE).unwrap();
}

#[test]
fn private_reissue_boundaries_wipe_staging_close_input_and_preserve_both_scope_floors() {
    let Some((uid, gid)) = rootless_service() else {
        return;
    };
    let mut setup_work = Work::new(LIMIT);
    let mut setup = Budget::new(&mut setup_work, LIMIT);
    let policy = policy(0, &mut setup);
    let deployment = deployment(&policy, uid, gid, &mut setup);
    let input = Cap::FILE_STORAGE + deployment.retained_storage() + policy.retained_storage();
    for mode in 0..7 {
        let floor = if mode == 1 { input - 1 } else { input + EXTRA };
        let work_limit = match mode {
            0 => ENTRY_WORK - 1,
            2 => OUTER_WORK - 1,
            4 => Cap::REISSUE_WORK - 1,
            _ => Cap::REISSUE_WORK,
        };
        let storage_limit = floor
            + match mode {
                3 => Cap::IO_STORAGE - 1,
                5 => Cap::REISSUE_STORAGE - 1,
                _ => Cap::REISSUE_STORAGE,
            };
        let image = template(7);
        let witness = image.try_clone().unwrap();
        let mut seed = [0xa5; KEY_BYTES];
        let mut work = Work::new(work_limit);
        let mut budget = Budget::new(&mut work, storage_limit);
        budget.reserve_storage(floor).unwrap();
        let ledger = budget.work_ledger_identity_v1();
        let result = Cap::reissue_template_for_current_service(
            image,
            &deployment,
            &policy,
            (uid, gid),
            &mut seed,
            &mut budget,
        );
        assert_eq!(seed, [0; KEY_BYTES], "mode {mode}");
        assert_eq!(budget.storage(), floor, "mode {mode}");
        assert!(budget.work_ledger_identity_v1() == ledger);
        assert_eq!(references(&witness), 1);
        match mode {
            0 | 2 => {
                assert!(matches!(result, Err(Error::Resource(Resource::Work(_)))));
                assert_eq!(budget.work(), if mode == 0 { 0 } else { ENTRY_WORK });
                assert_eq!(
                    budget.failed_work(),
                    Some(if mode == 0 { ENTRY_WORK } else { OUTER_WORK })
                );
                assert_eq!(budget.peak_storage(), floor);
            }
            1 => {
                assert!(matches!(result, Err(Error::Resource(Resource::Accounting))));
                assert_eq!((budget.work(), budget.peak_storage()), (ENTRY_WORK, floor));
            }
            3 => {
                assert!(matches!(result, Err(Error::Resource(Resource::Storage(_)))));
                assert_eq!(budget.work(), OUTER_WORK);
                assert_eq!(budget.failed_storage(), Some(floor + Cap::IO_STORAGE));
                assert_eq!(budget.peak_storage(), floor);
            }
            4 => {
                assert_deployment_resource(&result.unwrap_err(), false);
                assert_eq!(budget.work(), OUTER_WORK + 8);
                assert_eq!(budget.failed_work(), Some(Cap::REISSUE_WORK));
                assert_eq!(budget.peak_storage(), floor + Cap::IO_STORAGE);
            }
            5 => {
                assert_deployment_resource(&result.unwrap_err(), true);
                assert_eq!(budget.work(), Cap::REISSUE_WORK);
                assert_eq!(budget.failed_storage(), Some(floor + Cap::REISSUE_STORAGE));
                assert_eq!(budget.peak_storage(), floor + Cap::IO_STORAGE);
            }
            6 => {
                let (cap, delta) = result.unwrap();
                assert_eq!(budget.work(), Cap::REISSUE_WORK);
                assert_eq!(budget.peak_storage(), floor + Cap::REISSUE_STORAGE);
                assert_eq!(
                    delta.additional_storage(),
                    cap.retained_storage() - Cap::FILE_STORAGE
                );
                budget.reserve_storage(delta.additional_storage()).unwrap();
                let retained = cap.retained_storage();
                drop(cap);
                budget.release_storage(retained).unwrap();
                assert_eq!(budget.storage(), floor - Cap::FILE_STORAGE);
            }
            _ => unreachable!(),
        }
    }
}

#[test]
fn reissue_checks_current_uid_gid_and_the_complete_policy_before_reading_the_seed() {
    let Some((uid, gid)) = rootless_service() else {
        return;
    };
    let mut setup_work = Work::new(LIMIT);
    let mut setup = Budget::new(&mut setup_work, LIMIT);
    let pinned = policy(0, &mut setup);
    let exact = deployment(&pinned, uid, gid, &mut setup);
    let wrong_uid = deployment(&pinned, different(uid), gid, &mut setup);
    let wrong_gid = deployment(&pinned, uid, different(gid), &mut setup);
    for record in [&wrong_uid, &wrong_gid] {
        // Invalid image bytes would fail a read/derivation; credentials must win first.
        let image = template(9);
        let mut seed = [0xa5; KEY_BYTES];
        setup.reserve_storage(Cap::FILE_STORAGE).unwrap();
        assert_rejected(
            Cap::reissue_template_for_current_service(
                image,
                record,
                &pinned,
                (uid, gid),
                &mut seed,
                &mut setup,
            ),
            "key reissue process does not have the nonroot deployment credentials",
        );
        assert_eq!(seed, [0; KEY_BYTES]);
        setup.release_storage(Cap::FILE_STORAGE).unwrap();
    }
    for changed in 1..=4 {
        let other = policy(changed, &mut setup);
        assert_eq!(other.verifying_key(), pinned.verifying_key());
        let image = template(9);
        let mut seed = [0xa5; KEY_BYTES];
        setup.reserve_storage(Cap::FILE_STORAGE).unwrap();
        assert_rejected(
            Cap::reissue_template_for_current_service(
                image,
                &exact,
                &other,
                (uid, gid),
                &mut seed,
                &mut setup,
            ),
            "key reissue deployment names another native policy",
        );
        assert_eq!(seed, [0; KEY_BYTES]);
        let released = other.retained_storage() + Cap::FILE_STORAGE;
        drop(other);
        setup.release_storage(released).unwrap();
    }
}

#[test]
fn private_expected_owner_requires_both_template_credentials_and_the_signing_seed() {
    let Some((uid, gid)) = rootless_service() else {
        return;
    };
    let mut work = Work::new(LIMIT);
    let mut budget = Budget::new(&mut work, LIMIT);
    let policy = policy(0, &mut budget);
    let deployment = deployment(&policy, uid, gid, &mut budget);
    for (owner, key, expected) in [
        (
            (different(uid), gid),
            7,
            "sealed secret image is not an anonymous expected-owner read-only image",
        ),
        (
            (uid, different(gid)),
            7,
            "sealed secret image is not an anonymous expected-owner read-only image",
        ),
        (
            (uid, gid),
            9,
            "signing key does not match the pinned native policy",
        ),
    ] {
        let image = template(key);
        let witness = image.try_clone().unwrap();
        let mut seed = [0xa5; KEY_BYTES];
        budget.reserve_storage(Cap::FILE_STORAGE).unwrap();
        let floor = budget.storage();
        assert_rejected(
            Cap::reissue_template_for_current_service(
                image,
                &deployment,
                &policy,
                owner,
                &mut seed,
                &mut budget,
            ),
            expected,
        );
        assert_eq!(seed, [0; KEY_BYTES]);
        assert_eq!(budget.storage(), floor);
        assert_eq!(references(&witness), 1);
        budget.release_storage(Cap::FILE_STORAGE).unwrap();
    }
}

#[test]
fn private_reissue_rejects_wrong_access_mode_length_permissions_and_seals() {
    let Some((uid, gid)) = rootless_service() else {
        return;
    };
    let mut work = Work::new(LIMIT);
    let mut budget = Budget::new(&mut work, LIMIT);
    let policy = policy(0, &mut budget);
    let deployment = deployment(&policy, uid, gid, &mut budget);
    for case in 0..5 {
        let image = match case {
            0 => SealedCapabilityImage::create_fixed(&[7; KEY_BYTES], ROLE)
                .unwrap()
                .clone_fixed()
                .unwrap(),
            1 => SealedCapabilityImage::create_fixed(&[7; KEY_BYTES - 1], ROLE)
                .unwrap()
                .clone_fixed()
                .unwrap(),
            4 => {
                let file = rustix::fs::memfd_create(
                    "native-key-unsealed-test",
                    rustix::fs::MemfdFlags::CLOEXEC | rustix::fs::MemfdFlags::ALLOW_SEALING,
                )
                .map(File::from)
                .unwrap();
                rustix::fs::fchmod(&file, Mode::RUSR).unwrap();
                assert_eq!(
                    rustix::io::pwrite(&file, &[7; KEY_BYTES], 0).unwrap(),
                    KEY_BYTES
                );
                file
            }
            _ => template(7),
        };
        if case == 2 {
            rustix::fs::fchmod(&image, Mode::RUSR | Mode::WUSR).unwrap();
        }
        if case == 3 {
            rustix::io::fcntl_setfd(&image, rustix::io::FdFlags::empty()).unwrap();
        }
        let witness = image.try_clone().unwrap();
        let mut seed = [0xa5; KEY_BYTES];
        budget.reserve_storage(Cap::FILE_STORAGE).unwrap();
        let floor = budget.storage();
        let result = Cap::reissue_template_for_current_service(
            image,
            &deployment,
            &policy,
            (uid, gid),
            &mut seed,
            &mut budget,
        );
        assert!(matches!(result, Err(Error::Rejected(_))), "case {case}");
        assert_eq!(seed, [0; KEY_BYTES]);
        assert_eq!(budget.storage(), floor);
        assert_eq!(references(&witness), 1);
        budget.release_storage(Cap::FILE_STORAGE).unwrap();
    }
}

#[test]
fn reissue_keeps_first_denials_across_success_and_nested_refusal() {
    let Some((uid, gid)) = rootless_service() else {
        return;
    };
    let mut setup_work = Work::new(LIMIT);
    let mut setup = Budget::new(&mut setup_work, LIMIT);
    let policy = policy(0, &mut setup);
    let deployment = deployment(&policy, uid, gid, &mut setup);
    let input = Cap::FILE_STORAGE + deployment.retained_storage() + policy.retained_storage();
    let work_limit = Cap::REISSUE_WORK + OUTER_WORK + DEPLOYMENT_WORK - 1;
    let storage_limit = input + EXTRA + Cap::REISSUE_STORAGE;
    let mut work = Work::new(work_limit);
    let mut budget = Budget::new(&mut work, storage_limit);
    budget.reserve_storage(input + EXTRA).unwrap();
    assert!(budget.charge_work(work_limit + 1).is_err());
    assert!(budget.reserve_storage(storage_limit).is_err());
    let failed_storage = budget.failed_storage();
    for iteration in 0..2 {
        let mut seed = [0xa5; KEY_BYTES];
        let result = Cap::reissue_template_for_current_service(
            template(7),
            &deployment,
            &policy,
            (uid, gid),
            &mut seed,
            &mut budget,
        );
        assert_eq!(seed, [0; KEY_BYTES]);
        if iteration == 0 {
            drop(result.unwrap());
        } else {
            assert_deployment_resource(&result.unwrap_err(), false);
        }
        assert_eq!(budget.storage(), input + EXTRA);
        assert_eq!(budget.failed_work(), Some(work_limit + 1));
        assert_eq!(budget.failed_storage(), failed_storage);
        budget.release_storage(Cap::FILE_STORAGE).unwrap();
        if iteration == 0 {
            budget.reserve_storage(Cap::FILE_STORAGE).unwrap();
        }
    }
}

#[test]
fn deployment_error_mappings_keep_nominal_sources_and_exact_resource_failures() {
    for error in [
        Error::from(DeploymentErrorV2::Resource(Resource::Accounting)),
        Error::from(DeploymentErrorV3::Resource(Resource::Accounting)),
    ] {
        match &error {
            Error::Deployment(DeploymentErrorV2::Resource(Resource::Accounting)) => {
                assert!(error.source().unwrap().is::<DeploymentErrorV2>());
            }
            Error::DeploymentV3(DeploymentErrorV3::Resource(Resource::Accounting)) => {
                assert!(error.source().unwrap().is::<DeploymentErrorV3>());
            }
            _ => panic!("deployment error was flattened: {error:?}"),
        }
        assert_eq!(error.to_string(), Resource::Accounting.to_string());
    }
}

#[test]
fn reissue_named_quotas_are_finite_and_family_consistent() {
    assert_eq!(Cap::REISSUE_WORK, 146_192);
    assert_eq!(Cap::REISSUE_STORAGE, Cap::IO_STORAGE + DEPLOYMENT_STORAGE);
    assert_eq!(
        crate::CompilerExecutionSigningKeyCapabilityV2::REISSUE_WORK,
        crate::CompilerExecutionSigningKeyCapabilityV3::REISSUE_WORK
    );
    assert_eq!(
        crate::CompilerExecutionSigningKeyCapabilityV2::REISSUE_STORAGE,
        crate::CompilerExecutionSigningKeyCapabilityV3::REISSUE_STORAGE
    );
}

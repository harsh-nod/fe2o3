use super::super::tests::{key, policy};
use super::*;
use fe2o3_compiler_execution_protocol::{
    CompilerExecutionExternalAnchorServiceIdentityV1 as Anchor,
    CompilerExecutionIssuerMeasurementV1 as Measurement,
    CompilerExecutionSupervisorDeploymentErrorV3 as DeploymentError,
};
use fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1 as Work;
use rustix::fs::{Mode, OFlags};
use std::panic::{AssertUnwindSafe, catch_unwind};

const LIMIT: usize = 8_000_000;
const EXTRA: usize = 19;
const OUTER_WORK: usize = Transfer::WORK - DEPLOYMENT_WORK;

// Native configuration fixtures only, never installed provisioning provenance.
fn deployment(policy: &Policy, axis: u8, budget: &mut Budget<'_>) -> Deployment {
    let (deployment, charge) = Deployment::new(
        if axis == 1 { 60003 } else { 60001 },
        if axis == 2 { 60004 } else { 60001 },
        Anchor::new(60002, if axis == 3 { 60005 } else { 60002 }).unwrap(),
        Measurement::new([if axis == 4 { 33 } else { 31 }; 32], 4096).unwrap(),
        Measurement::new([if axis == 5 { 34 } else { 32 }; 32], 8192).unwrap(),
        policy,
        budget,
    )
    .unwrap();
    budget.reserve_storage(charge.additional_storage()).unwrap();
    deployment
}

fn rejected<T>(result: Result<T>, expected: &str) {
    match result {
        Err(Error::Rejected(reason)) => assert_eq!(reason, expected),
        Err(error) => panic!("unexpected error: {error:?}"),
        Ok(_) => panic!("unexpected acceptance"),
    }
}

fn references(file: &File) -> usize {
    let metadata = file.metadata().unwrap();
    std::fs::read_dir("/proc/self/fd")
        .unwrap()
        .filter_map(|e| std::fs::metadata(e.ok()?.path()).ok())
        .filter(|m| (m.dev(), m.ino()) == (metadata.dev(), metadata.ino()))
        .count()
}

// Test corruption with CHOWN alone; fchmod needs ownership, not DAC_OVERRIDE.
fn change_service_mode(file: &File, mode: Mode, deployment: &Deployment) {
    rustix::fs::fchown(file, Some(rustix::process::Uid::from_raw(0)), None).unwrap();
    rustix::fs::fchmod(file, mode).unwrap();
    rustix::fs::fchown(
        file,
        Some(rustix::process::Uid::from_raw(deployment.service_uid())),
        None,
    )
    .unwrap();
}

#[test]
fn service_transfer_rootless_refusal_preserves_source_and_wipes_staging() {
    assert!(rustix::process::geteuid().as_raw() != 0);
    let mut work = Work::new(LIMIT);
    let mut budget = Budget::new(&mut work, LIMIT);
    let policy = policy(7, &mut budget);
    let deployment = deployment(&policy, 0, &mut budget);
    let key = key(&policy, &mut budget);
    let source = key.image.as_file().metadata().unwrap();
    let floor = budget.storage();
    let before = budget.work();
    let ledger = budget.work_ledger_identity_v1();
    let mut seed = [0xa5; KEY_BYTES];
    rejected(
        key.reissue_for_deployed_service_with(&deployment, &policy, &mut seed, &mut budget, |_| {
            panic!("rootless allocation")
        }),
        "service key transfer requires root UID/GID 0:0",
    );
    assert_eq!(seed, [0; KEY_BYTES]);
    assert_eq!(
        (budget.storage(), budget.work()),
        (floor, before + Transfer::WORK)
    );
    assert!(budget.work_ledger_identity_v1() == ledger);
    assert_eq!(references(key.image.as_file()), 1);
    let after = key.image.as_file().metadata().unwrap();
    assert_eq!(
        (after.dev(), after.ino(), after.uid(), after.gid()),
        (source.dev(), source.ino(), source.uid(), source.gid())
    );
    key.revalidate(&policy, &mut budget).unwrap();
    rejected(
        key.reissue_for_deployed_service(&deployment, &policy, &mut budget),
        "service key transfer requires root UID/GID 0:0",
    );
}

#[test]
fn service_transfer_rejects_same_key_different_native_policy() {
    let mut work = Work::new(LIMIT);
    let mut budget = Budget::new(&mut work, LIMIT);
    let original = policy(7, &mut budget);
    let other = policy(8, &mut budget);
    let deployment = deployment(&original, 0, &mut budget);
    let key = key(&original, &mut budget);
    rejected(
        key.reissue_for_deployed_service(&deployment, &other, &mut budget),
        "service key transfer deployment names another native policy",
    );
}

#[test]
fn service_transfer_resource_boundaries_keep_original_ledger_and_wipe_seed() {
    let mut setup_work = Work::new(LIMIT);
    let mut setup = Budget::new(&mut setup_work, LIMIT);
    let policy = policy(7, &mut setup);
    let deployment = deployment(&policy, 0, &mut setup);
    let key = key(&policy, &mut setup);
    let inputs = key.retained_storage() + policy.retained_storage() + deployment.retained_storage();
    denied_boundaries(inputs, |budget| {
        let mut seed = [0xa5; KEY_BYTES];
        let result =
            key.reissue_for_deployed_service_with(&deployment, &policy, &mut seed, budget, |_| {
                panic!("unpaid allocation")
            });
        assert_eq!(seed, [0; KEY_BYTES]);
        result.map(|_| ())
    });
}

fn denied_boundaries(inputs: usize, mut operation: impl FnMut(&mut Budget<'_>) -> Result<()>) {
    for case in 0..6 {
        let floor = if case == 1 {
            inputs - 1
        } else {
            inputs + EXTRA
        };
        let work_limit = match case {
            0 => ENTRY_WORK - 1,
            2 => OUTER_WORK - 1,
            4 => Transfer::WORK - 1,
            _ => Transfer::WORK,
        };
        let scratch = match case {
            3 => Transfer::IO_STORAGE - 1,
            5 => Transfer::STORAGE - 1,
            _ => Transfer::STORAGE,
        };
        let mut work = Work::new(work_limit);
        let mut budget = Budget::new(&mut work, floor + scratch);
        budget.reserve_storage(floor).unwrap();
        let ledger = budget.work_ledger_identity_v1();
        let result = operation(&mut budget);
        assert_eq!(budget.storage(), floor);
        assert!(budget.work_ledger_identity_v1() == ledger);
        match case {
            0 | 2 => {
                assert!(matches!(result, Err(Error::Resource(Resource::Work(_)))));
                assert_eq!(budget.work(), if case == 0 { 0 } else { ENTRY_WORK });
                assert_eq!(
                    budget.failed_work(),
                    Some(if case == 0 { ENTRY_WORK } else { OUTER_WORK })
                );
            }
            1 => assert!(matches!(result, Err(Error::Resource(Resource::Accounting)))),
            3 => {
                assert!(matches!(result, Err(Error::Resource(Resource::Storage(_)))));
                assert_eq!(budget.failed_storage(), Some(floor + Transfer::IO_STORAGE));
            }
            4 => {
                assert!(matches!(
                    result,
                    Err(Error::DeploymentV3(DeploymentError::Resource(
                        Resource::Work(_)
                    )))
                ));
                assert_eq!(budget.failed_work(), Some(Transfer::WORK));
            }
            5 => {
                assert!(matches!(
                    result,
                    Err(Error::DeploymentV3(DeploymentError::Resource(
                        Resource::Storage(_)
                    )))
                ));
                assert_eq!(budget.failed_storage(), Some(floor + Transfer::STORAGE));
            }
            _ => unreachable!(),
        }
    }
}

#[test]
#[ignore = "requires isolated root UID/GID 0:0; run this test filter with --ignored --test-threads=1"]
fn service_transfer_isolated_root_positive_and_owner_checks() {
    require_root().unwrap();
    let mut work = Work::new(LIMIT);
    let mut budget = Budget::new(&mut work, LIMIT);
    budget.reserve_storage(EXTRA).unwrap();
    let policy = policy(7, &mut budget);
    let deployment = deployment(&policy, 0, &mut budget);
    let key = key(&policy, &mut budget);
    let (source_alias, charge) = key.try_clone_for_transfer(&mut budget).unwrap();
    budget.reserve_storage(charge.additional_storage()).unwrap();
    let source = source_alias.metadata().unwrap();
    let floor = budget.storage();
    let before = budget.work();
    let peak = budget.peak_storage();
    let ledger = budget.work_ledger_identity_v1();
    let mut seed = [0xa5; KEY_BYTES];
    let (transfer, charge) = key
        .reissue_for_deployed_service_with(&deployment, &policy, &mut seed, &mut budget, |_| Ok(()))
        .unwrap();
    assert_eq!(seed, [0; KEY_BYTES]);
    assert_eq!(
        (budget.storage(), budget.work()),
        (floor, before + Transfer::WORK)
    );
    assert!(budget.work_ledger_identity_v1() == ledger);
    assert_eq!(budget.peak_storage(), peak.max(floor + Transfer::STORAGE));
    assert_eq!(charge.additional_storage(), transfer.retained_storage());
    budget.reserve_storage(charge.additional_storage()).unwrap();
    let (file, charge) = transfer
        .try_clone_for_transfer(&key, &deployment, &policy, &mut budget)
        .unwrap();
    assert_eq!(charge.additional_storage(), Transfer::FILE_STORAGE);
    budget.reserve_storage(charge.additional_storage()).unwrap();
    let fresh = file.metadata().unwrap();
    assert_ne!((fresh.dev(), fresh.ino()), (source.dev(), source.ino()));
    assert_eq!(
        (
            fresh.uid(),
            fresh.gid(),
            fresh.nlink(),
            fresh.len(),
            fresh.mode()
        ),
        (
            deployment.service_uid(),
            deployment.service_gid(),
            0,
            KEY_BYTES as u64,
            libc::S_IFREG | 0o400
        )
    );
    assert_eq!(
        rustix::fs::fcntl_getfl(&file).unwrap() & OFlags::ACCMODE,
        OFlags::RDONLY
    );
    assert_eq!(
        rustix::io::fcntl_getfd(&file).unwrap(),
        rustix::io::FdFlags::CLOEXEC
    );
    assert_eq!(
        rustix::fs::fcntl_get_seals(&file).unwrap(),
        crate::sealed_image::REQUIRED_SEALS
    );
    rustix::fs::seek(&file, rustix::fs::SeekFrom::Start(17)).unwrap();
    transfer
        .validate_transfer(&file, &key, &deployment, &policy, &mut budget)
        .unwrap();
    assert_eq!(
        rustix::fs::seek(&file, rustix::fs::SeekFrom::Current(0)).unwrap(),
        17
    );
    let unchanged = source_alias.metadata().unwrap();
    assert_eq!(
        (
            unchanged.dev(),
            unchanged.ino(),
            unchanged.uid(),
            unchanged.gid(),
            unchanged.mode()
        ),
        (source.dev(), source.ino(), 0, 0, libc::S_IFREG | 0o400)
    );
    key.validate_transfer(&source_alias, &policy, &mut budget)
        .unwrap();
    // Current-owner admission is unchanged: root cannot admit the service image.
    rejected(
        Key::from_file(file, &policy, &mut budget),
        "sealed secret image is not an anonymous current-owner read-only image",
    );
    budget.release_storage(Transfer::FILE_STORAGE).unwrap();
    let retained = key.retained_storage()
        + transfer.retained_storage()
        + policy.retained_storage()
        + deployment.retained_storage()
        + Key::FILE_STORAGE;
    drop((key, transfer, policy, deployment, source_alias));
    budget.release_storage(retained).unwrap();
    assert_eq!(budget.storage(), EXTRA);
}

#[test]
#[ignore = "requires isolated root UID/GID 0:0; run this test filter with --ignored --test-threads=1"]
fn service_transfer_isolated_root_substitution_and_staged_metadata_refuse() {
    require_root().unwrap();
    let mut work = Work::new(LIMIT);
    let mut budget = Budget::new(&mut work, LIMIT);
    let original = policy(7, &mut budget);
    let deployment = deployment(&original, 0, &mut budget);
    let root_key = key(&original, &mut budget);
    let other_key = key(&original, &mut budget);
    let (transfer, charge) = root_key
        .reissue_for_deployed_service(&deployment, &original, &mut budget)
        .unwrap();
    budget.reserve_storage(charge.additional_storage()).unwrap();
    let (file, charge) = transfer
        .try_clone_for_transfer(&root_key, &deployment, &original, &mut budget)
        .unwrap();
    budget.reserve_storage(charge.additional_storage()).unwrap();
    rejected(
        transfer.validate_transfer(&file, &other_key, &deployment, &original, &mut budget),
        "service key transfer names another root key inode",
    );
    let other_policy = policy(8, &mut budget);
    let other_deployment = self::deployment(&other_policy, 0, &mut budget);
    rejected(
        transfer.validate_transfer(
            &file,
            &root_key,
            &other_deployment,
            &other_policy,
            &mut budget,
        ),
        "service key transfer names another deployment or policy",
    );
    rejected(
        root_key.reissue_for_deployed_service(&other_deployment, &other_policy, &mut budget),
        "signing key is pinned to another native policy",
    );
    for axis in 1..6 {
        let changed = self::deployment(&original, axis, &mut budget);
        rejected(
            transfer.validate_transfer(&file, &root_key, &changed, &original, &mut budget),
            "service key transfer names another deployment or policy",
        );
    }
    let (other, charge) = root_key
        .reissue_for_deployed_service(&deployment, &original, &mut budget)
        .unwrap();
    budget.reserve_storage(charge.additional_storage()).unwrap();
    let (substitute, charge) = other
        .try_clone_for_transfer(&root_key, &deployment, &original, &mut budget)
        .unwrap();
    budget.reserve_storage(charge.additional_storage()).unwrap();
    rejected(
        transfer.validate_transfer(&substitute, &root_key, &deployment, &original, &mut budget),
        "sealed image identity or length changed",
    );
    for axis in 0..4 {
        match axis {
            0 => change_service_mode(&file, Mode::RUSR | Mode::WUSR, &deployment),
            1 => rustix::io::fcntl_setfd(&file, rustix::io::FdFlags::empty()).unwrap(),
            2 => rustix::fs::fchown(&file, Some(rustix::process::Uid::from_raw(0)), None).unwrap(),
            3 => rustix::fs::fchown(&file, None, Some(rustix::process::Gid::from_raw(0))).unwrap(),
            _ => unreachable!(),
        }
        assert!(
            transfer
                .validate_transfer(&file, &root_key, &deployment, &original, &mut budget)
                .is_err()
        );
        change_service_mode(&file, Mode::RUSR, &deployment);
        rustix::io::fcntl_setfd(&file, rustix::io::FdFlags::CLOEXEC).unwrap();
        rustix::fs::fchown(
            &file,
            Some(rustix::process::Uid::from_raw(deployment.service_uid())),
            Some(rustix::process::Gid::from_raw(deployment.service_gid())),
        )
        .unwrap();
    }
    transfer
        .validate_transfer(&file, &root_key, &deployment, &original, &mut budget)
        .unwrap();
    let floor = root_key.retained_storage()
        + original.retained_storage()
        + deployment.retained_storage()
        + transfer.retained_storage();
    denied_boundaries(floor, |b| {
        transfer
            .try_clone_for_transfer(&root_key, &deployment, &original, b)
            .map(|_| ())
    });
    denied_boundaries(floor + Transfer::FILE_STORAGE, |b| {
        transfer.validate_transfer(&file, &root_key, &deployment, &original, b)
    });
    let mut exact_work = Work::new(Transfer::WORK);
    let mut exact = Budget::new(
        &mut exact_work,
        floor + Transfer::FILE_STORAGE + Transfer::STORAGE,
    );
    exact
        .reserve_storage(floor + Transfer::FILE_STORAGE)
        .unwrap();
    transfer
        .validate_transfer(&file, &root_key, &deployment, &original, &mut exact)
        .unwrap();
    assert_eq!(exact.work(), Transfer::WORK);
    assert_eq!(
        exact.peak_storage(),
        floor + Transfer::FILE_STORAGE + Transfer::STORAGE
    );
}

#[test]
#[ignore = "requires isolated root UID/GID 0:0; run this test filter with --ignored --test-threads=1"]
fn service_transfer_isolated_root_late_error_and_unwind_wipe_close_restore() {
    require_root().unwrap();
    let mut work = Work::new(LIMIT);
    let mut budget = Budget::new(&mut work, LIMIT);
    let policy = policy(7, &mut budget);
    let deployment = deployment(&policy, 0, &mut budget);
    let root_key = key(&policy, &mut budget);
    budget.reserve_storage(Transfer::FILE_STORAGE).unwrap(); // Test witness.
    assert!(budget.charge_work(LIMIT).is_err());
    assert!(budget.reserve_storage(LIMIT).is_err());
    let denied = (budget.failed_work(), budget.failed_storage());
    for case in 0..3 {
        let floor = budget.storage();
        let before = budget.work();
        let ledger = budget.work_ledger_identity_v1();
        let mut seed = [0xa5; KEY_BYTES];
        let mut witness = None;
        let result = catch_unwind(AssertUnwindSafe(|| {
            root_key.reissue_for_deployed_service_with(
                &deployment,
                &policy,
                &mut seed,
                &mut budget,
                |transfer| {
                    witness = Some(transfer.image.as_file().try_clone().unwrap());
                    match case {
                        0 => Err(Error::Rejected("injected late refusal")),
                        1 => panic!("injected late unwind"),
                        2 => {
                            change_service_mode(
                                transfer.image.as_file(),
                                Mode::RUSR | Mode::WUSR,
                                &deployment,
                            );
                            Ok(())
                        }
                        _ => unreachable!(),
                    }
                },
            )
        }));
        assert_eq!(seed, [0; KEY_BYTES]);
        assert_eq!(
            (budget.storage(), budget.work()),
            (floor, before + Transfer::WORK)
        );
        assert!(budget.work_ledger_identity_v1() == ledger);
        assert_eq!((budget.failed_work(), budget.failed_storage()), denied);
        if case == 1 {
            assert!(result.is_err());
        } else {
            assert!(result.unwrap().is_err());
        }
        assert_eq!(references(witness.as_ref().unwrap()), 1);
        assert_eq!(references(root_key.image.as_file()), 1);
        root_key.revalidate(&policy, &mut budget).unwrap();
    }
}

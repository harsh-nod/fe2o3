//! Real image/namespace custody tests, not fabricated protected service owners.
use super::*;
use fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1 as Work;
use fe2o3_protected_static_executable::ProtectedStaticExecutableOperationV2 as Operation;
use sha2::{Digest, Sha256};
use std::{
    fs,
    os::unix::fs::{MetadataExt, PermissionsExt},
    panic::{AssertUnwindSafe, catch_unwind},
};

const LIMIT: usize = 1 << 34;
const EXTRA: usize = 27;

#[test]
fn native_prepared_types_satisfy_retained_owner_send_bound() {
    fn retained<T: Send + 'static>() {
        let floor = std::mem::size_of::<T>();
        assert!(fe2o3_protected_service_spawn::native_spawn::RootOwnedRetainedServiceChildV2::<T>::storage_for(floor).unwrap() > floor);
    }
    retained::<crate::PreparedCompilerExecutionSupervisorV2>();
    retained::<crate::PreparedCompilerExecutionSupervisorV3>();
}
struct Fixture {
    dir: tempfile::TempDir,
    expected: [Measurement; 3],
}
impl Fixture {
    fn new() -> Self {
        let dir = tempfile::tempdir().unwrap();
        let expected = std::array::from_fn(|index| {
            let bytes = crate::provisioning_entrypoint::static_pause_elf(index as u8);
            let path = dir.path().join(index.to_string());
            fs::write(&path, &bytes).unwrap();
            fs::set_permissions(&path, fs::Permissions::from_mode(0o555)).unwrap();
            Measurement::new(
                Sha256::digest(&bytes).into(),
                bytes.len() as u64,
                128 * 1024 * 1024,
            )
            .unwrap()
        });
        Self { dir, expected }
    }
    fn sources(&self) -> Sources {
        Sources::new(self.open(0), self.open(1), self.open(2))
    }
    fn open(&self, index: usize) -> File {
        File::open(self.dir.path().join(index.to_string())).unwrap()
    }
    fn floor(&self) -> usize {
        self.expected
            .iter()
            .map(|m| Image::file_storage(*m).unwrap())
            .sum()
    }
    fn work(&self, operation: Operation) -> usize {
        self.expected
            .iter()
            .map(|m| Image::quota(*m, operation).unwrap().work())
            .sum()
    }
    fn peak(&self) -> usize {
        self.expected
            .iter()
            .enumerate()
            .map(|(index, m)| {
                index * IMAGE_GROWTH + Image::quota(*m, Operation::Admit).unwrap().scratch()
            })
            .max()
            .unwrap()
            .max(3 * IMAGE_GROWTH)
    }
    fn source_objects(&self) -> [(u64, u64); 3] {
        std::array::from_fn(|index| {
            let m = fs::metadata(self.dir.path().join(index.to_string())).unwrap();
            (m.dev(), m.ino())
        })
    }
}
fn refs(object: (u64, u64)) -> usize {
    fs::read_dir("/proc/self/fd")
        .unwrap()
        .filter_map(|e| e.ok())
        .filter_map(|e| fs::metadata(e.path()).ok())
        .filter(|m| (m.dev(), m.ino()) == object)
        .count()
}
fn credentials() -> Credentials {
    let uid = rustix::process::geteuid().as_raw();
    let gid = rustix::process::getegid().as_raw();
    Credentials::new(
        if uid == 0 { 65534 } else { uid },
        if gid == 0 { 65534 } else { gid },
    )
    .unwrap()
}
fn owner() -> Owner {
    Owner::new(credentials().uid(), credentials().gid()).unwrap()
}
fn seal(f: &Fixture, b: &mut Budget<'_>) -> Result<[Image; 3]> {
    let sources = f.sources();
    b.with_prepaid_scope(f.floor(), 0, 0, 0, |b| {
        seal_programs(sources, f.expected, owner(), b)
    })
}

#[test]
fn three_roles_seal_distinct_objects_with_complete_original_storage() {
    let f = Fixture::new();
    let floor = EXTRA + f.floor();
    let mut work = Work::new(LIMIT);
    let mut b = Budget::new(&mut work, LIMIT);
    b.reserve_storage(floor).unwrap();
    let ledger = b.work_ledger_identity_v1();
    let images = seal(&f, &mut b).unwrap();
    assert_eq!(b.storage(), floor);
    assert_eq!(b.work(), f.work(Operation::Admit));
    assert_eq!(b.peak_storage(), floor + f.peak());
    b.reserve_storage(3 * IMAGE_GROWTH).unwrap();
    let objects = images.each_ref().map(|image| {
        (
            image.object_identity().device(),
            image.object_identity().inode(),
        )
    });
    for index in 0..3 {
        assert_eq!(images[index].measurement(), f.expected[index]);
        assert_eq!(
            images[index].retained_storage(),
            Image::file_storage(f.expected[index]).unwrap() + IMAGE_GROWTH
        );
        assert!(!objects[..index].contains(&objects[index]));
        assert!(!f.source_objects().contains(&objects[index]));
    }
    check_programs(&images, f.expected, credentials(), &mut b).unwrap();
    assert_eq!(
        b.work(),
        f.work(Operation::Admit) + f.work(Operation::Revalidate)
    );
    assert!(b.work_ledger_identity_v1() == ledger);
    drop(images);
    b.release_storage(f.floor() + 3 * IMAGE_GROWTH).unwrap();
    assert_eq!(b.storage(), EXTRA);
    for object in objects.into_iter().chain(f.source_objects()) {
        assert_eq!(refs(object), 0);
    }
}

#[test]
fn exact_and_one_short_image_composition_work_storage_and_floor() {
    let f = Fixture::new();
    for mode in 0..4 {
        let floor = f.floor() - usize::from(mode == 1);
        let mut work = Work::new(f.work(Operation::Admit) - usize::from(mode == 2));
        let mut b = Budget::new(&mut work, floor + f.peak() - usize::from(mode == 3));
        b.reserve_storage(floor).unwrap();
        let result = seal(&f, &mut b);
        assert_eq!(b.storage(), floor);
        match mode {
            0 => {
                let images = result.unwrap();
                b.reserve_storage(3 * IMAGE_GROWTH).unwrap();
                drop(images);
                b.release_storage(3 * IMAGE_GROWTH).unwrap();
                assert_eq!(b.work(), f.work(Operation::Admit));
                assert_eq!(b.peak_storage(), floor + f.peak());
            }
            1 => assert!(matches!(
                result,
                Err(Failure::Resource(Resource::Accounting))
            )),
            2 => assert!(matches!(
                result,
                Err(Failure::Executable(ImageError::Resource(Resource::Work(_))))
            )),
            _ => assert!(matches!(
                result,
                Err(Failure::Executable(ImageError::Resource(
                    Resource::Storage(_)
                )))
            )),
        }
        for object in f.source_objects() {
            assert_eq!(refs(object), 0);
        }
    }
}

#[test]
fn role_substitution_fails_without_retiring_inputs_or_leaking_sources() {
    let f = Fixture::new();
    for bad in 0..3 {
        let mut expected = f.expected;
        expected[bad] = f.expected[(bad + 1) % 3];
        let mut work = Work::new(LIMIT);
        let mut b = Budget::new(&mut work, LIMIT);
        b.reserve_storage(f.floor() + EXTRA).unwrap();
        let sources = f.sources();
        let result = b.with_prepaid_scope(f.floor(), 0, 0, 0, |b| {
            seal_programs(sources, expected, owner(), b)
        });
        assert!(matches!(result, Err(Failure::Executable(_))));
        assert_eq!(b.storage(), f.floor() + EXTRA);
        assert!(b.work() > 0);
        for object in f.source_objects() {
            assert_eq!(refs(object), 0);
        }
    }
}

#[test]
fn image_checks_reject_each_role_owner_and_metadata_change() {
    let f = Fixture::new();
    let mut work = Work::new(LIMIT);
    let mut b = Budget::new(&mut work, LIMIT);
    b.reserve_storage(f.floor()).unwrap();
    let images = seal(&f, &mut b).unwrap();
    b.reserve_storage(3 * IMAGE_GROWTH).unwrap();
    for bad in 0..3 {
        let mut expected = f.expected;
        expected[bad] = f.expected[(bad + 1) % 3];
        assert!(matches!(
            check_programs(&images, expected, credentials(), &mut b),
            Err(Failure::ExecutableBindingMismatch)
        ));
    }
    let wrong = Credentials::new(credentials().uid() + 1, credentials().gid()).unwrap();
    assert!(matches!(
        check_programs(&images, f.expected, wrong, &mut b),
        Err(Failure::ExecutableBindingMismatch)
    ));
    drop(images);
    b.release_storage(f.floor() + 3 * IMAGE_GROWTH).unwrap();
    for role in 0..3 {
        b.reserve_storage(f.floor()).unwrap();
        let images = seal(&f, &mut b).unwrap();
        b.reserve_storage(3 * IMAGE_GROWTH).unwrap();
        let (alias, charge) = images[role].try_clone_for_exec(&mut b).unwrap();
        b.reserve_storage(charge.additional_storage()).unwrap();
        // F_SEAL_EXEC prevents changing execute bits, but writable-mode drift
        // remains possible and must be rejected by exact image revalidation.
        assert_eq!(
            rustix::fs::fchmod(&alias, rustix::fs::Mode::from_bits_truncate(0o400)),
            Err(rustix::io::Errno::PERM)
        );
        check_programs(&images, f.expected, credentials(), &mut b).unwrap();
        rustix::fs::fchmod(&alias, rustix::fs::Mode::from_bits_truncate(0o755)).unwrap();
        assert!(matches!(
            check_programs(&images, f.expected, credentials(), &mut b),
            Err(Failure::Executable(_))
        ));
        rustix::fs::fchmod(&alias, rustix::fs::Mode::from_bits_truncate(0o555)).unwrap();
        // Rapid chmod calls can share a ctime tick. After restoring the mode,
        // change mtime explicitly so this tests observable snapshot drift.
        let before = alias.metadata().unwrap().modified().unwrap();
        let changed = if before == std::time::UNIX_EPOCH {
            std::time::UNIX_EPOCH + std::time::Duration::from_secs(1)
        } else {
            std::time::UNIX_EPOCH
        };
        alias.set_modified(changed).unwrap();
        assert_eq!(alias.metadata().unwrap().modified().unwrap(), changed);
        assert_ne!(before, changed);
        assert!(matches!(
            check_programs(&images, f.expected, credentials(), &mut b),
            Err(Failure::Executable(_))
        ));
        drop(alias);
        b.release_storage(charge.additional_storage()).unwrap();
        drop(images);
        b.release_storage(f.floor() + 3 * IMAGE_GROWTH).unwrap();
    }
}

#[test]
fn image_scope_preserves_denials_and_closes_on_caller_unwind() {
    let f = Fixture::new();
    let floor = f.floor() + EXTRA;
    let mut work = Work::new(LIMIT);
    let mut b = Budget::new(&mut work, LIMIT);
    b.reserve_storage(floor).unwrap();
    assert!(b.charge_work(LIMIT + 1).is_err());
    assert!(b.reserve_storage(LIMIT).is_err());
    let history = (b.failed_work(), b.failed_storage());
    let ledger = b.work_ledger_identity_v1();
    let mut objects = None;
    let result = catch_unwind(AssertUnwindSafe(|| {
        let _: Result<()> = b.with_prepaid_scope(floor, 0, 0, 0, |b| {
            let images = seal_programs(f.sources(), f.expected, owner(), b)?;
            objects = Some(
                images
                    .each_ref()
                    .map(|i| (i.object_identity().device(), i.object_identity().inode())),
            );
            panic!("after sealing compiler program chain");
        });
    }));
    assert!(result.is_err());
    assert_eq!(b.storage(), floor);
    assert_eq!(b.work(), f.work(Operation::Admit));
    assert_eq!((b.failed_work(), b.failed_storage()), history);
    assert!(b.work_ledger_identity_v1() == ledger);
    for object in objects.unwrap().into_iter().chain(f.source_objects()) {
        assert_eq!(refs(object), 0);
    }
}

#[test]
fn fixed_arithmetic_and_namespace_receipt_match_native_contracts() {
    use fe2o3_protected_service_profile::{
        ProtectedServiceNamespaceSetV2 as Namespaces, ProtectedServiceProfileStorageV2,
    };
    assert_eq!(sum(&[1, 2, 3]).unwrap(), 6);
    assert!(matches!(
        sum(&[usize::MAX, 1]),
        Err(Failure::Resource(Resource::Arithmetic))
    ));
    assert_eq!(maximum(&[9, 2, 8]), 9);
    assert_eq!(maximum(&[]), 0);
    assert_eq!(
        require_root().is_ok(),
        fe2o3_protected_service_spawn::require_exact_root_identity_v1().is_ok()
    );
    let mut work = Work::new(Namespaces::CAPTURE_WORK);
    let mut b = Budget::new(&mut work, Namespaces::CAPTURE_SCRATCH);
    let (namespaces, storage) = Namespaces::capture_self(&mut b).unwrap();
    assert_eq!(
        storage.additional_storage(),
        size_of::<(Namespaces, ProtectedServiceProfileStorageV2)>()
    );
    assert_eq!(namespaces.retained_storage(), storage.additional_storage());
    assert_eq!(b.storage(), 0);
    b.reserve_storage(storage.additional_storage()).unwrap();
    drop(namespaces);
    b.release_storage(storage.additional_storage()).unwrap();
}

#[test]
fn cleanup_guard_apis_and_precise_preparation_errors() {
    type Quota = CompilerExecutionPreparationQuotaV2;
    macro_rules! api {
        ($prepared:ty) => {
            let _: fn(&$prepared) -> Result<Quota> = <$prepared>::cleanup_guard_quota;
            let _: fn(&$prepared, &mut Cleanup, &mut Budget<'_>) -> Result<()> =
                <$prepared>::validate_cleanup_guard;
        };
    }
    api!(crate::PreparedCompilerExecutionSupervisorV2);
    api!(crate::PreparedCompilerExecutionSupervisorV3);
    for error in [
        Failure::from(CleanupError::State),
        Failure::from(LifecycleError::ParentChanged),
    ] {
        assert_eq!(error.to_string(), error.source().unwrap().to_string());
    }
    assert!(matches!(
        Failure::from(CleanupError::State),
        Failure::Cleanup(CleanupError::State)
    ));
    assert!(matches!(
        Failure::from(LifecycleError::ParentChanged),
        Failure::Lifecycle(LifecycleError::ParentChanged)
    ));
}

#[allow(unsafe_code)]
fn guard_pool() -> Cleanup {
    use fe2o3_kernel_ir::CanonicalKernelIrOwnedVerificationResourceBudgetV1 as Account;
    // SAFETY: this isolated pool only holds a test lock; no child is ever submitted.
    unsafe {
        fe2o3_protected_service_spawn::cleanup_bridge::isolated_cleanup_for_test(Account::new(
            Work::new(LIMIT),
            Cleanup::STORAGE,
        ))
    }
}

#[test]
fn cleanup_guard_refuses_before_pool_access_on_original_account() {
    let mut cleanup = guard_pool();
    let before = cleanup.report().unwrap();
    for mode in 0..4 {
        let floor = EXTRA - usize::from(mode == 0);
        let mut work = Work::new(LOCAL_WORK - usize::from(mode == 1));
        let mut b = Budget::new(&mut work, floor + 64 - usize::from(mode == 2));
        b.reserve_storage(floor).unwrap();
        let ledger = b.work_ledger_identity_v1();
        let result = with_cleanup_guard(
            EXTRA,
            64,
            &mut cleanup,
            &mut b,
            |_| Err(Failure::CoordinatorChanged),
            |_, _| panic!("guard validation reached after refusal"),
        );
        match mode {
            0 => assert!(matches!(
                result,
                Err(Failure::Resource(Resource::Accounting))
            )),
            1 => assert!(matches!(result, Err(Failure::Resource(Resource::Work(_))))),
            2 => assert!(matches!(
                result,
                Err(Failure::Resource(Resource::Storage(_)))
            )),
            _ => assert!(matches!(result, Err(Failure::CoordinatorChanged))),
        }
        assert_eq!(b.storage(), floor);
        assert_eq!(b.work(), if mode < 2 { ENTRY } else { LOCAL_WORK });
        assert!(b.work_ledger_identity_v1() == ledger);
        assert_eq!(cleanup.report().unwrap(), before);
    }
    cleanup.shutdown().unwrap();
}

#[test]
fn cleanup_guard_requires_an_existing_guard_without_installing_one() {
    let mut cleanup = guard_pool();
    let before = cleanup.report().unwrap();
    let work_limit = LOCAL_WORK + Cleanup::GUARD_CLONE_WORK;
    let mut work = Work::new(work_limit);
    let mut b = Budget::new(
        &mut work,
        EXTRA + 64 + Cleanup::GUARD_CLONE_SCRATCH + Cleanup::GUARD_FILE_STORAGE,
    );
    b.reserve_storage(EXTRA).unwrap();
    assert!(matches!(
        with_cleanup_guard(
            EXTRA,
            64,
            &mut cleanup,
            &mut b,
            |_| Ok(()),
            |_, _| panic!("missing guard must not reach validation")
        ),
        Err(Failure::Cleanup(CleanupError::State))
    ));
    assert_eq!(b.storage(), EXTRA);
    assert_eq!(b.work(), work_limit);
    let after = cleanup.report().unwrap();
    assert_eq!(after.storage, before.storage);
    assert_eq!(after.work, before.work + Cleanup::GUARD_CLONE_WORK);
    cleanup.shutdown().unwrap();
}

fn root_lease(b: &mut Budget<'_>) -> (tempfile::TempDir, File, Lease) {
    use fe2o3_compiler_execution_protocol::{
        COMPILER_EXECUTION_LIFECYCLE_LOCK_MODE_V1 as MODE,
        COMPILER_EXECUTION_LIFECYCLE_LOCK_PATH_V1 as PATH,
    };
    let dir = tempfile::tempdir().unwrap();
    fs::set_permissions(dir.path(), fs::Permissions::from_mode(0o755)).unwrap();
    let root = dir.path().join("state");
    fs::create_dir(&root).unwrap();
    let lock = dir
        .path()
        .join(std::path::Path::new(PATH).file_name().unwrap());
    fs::write(&lock, []).unwrap();
    fs::set_permissions(&lock, fs::Permissions::from_mode(MODE)).unwrap();
    let root = File::open(root).unwrap();
    b.reserve_storage(Lease::STATE_ROOT_STORAGE).unwrap();
    let (lease, charge) = Lease::open(&root, b).unwrap();
    b.reserve_storage(charge.additional_storage()).unwrap();
    (dir, root, lease)
}

#[test]
fn cleanup_guard_real_lease_join_exact_short_substituted_and_unwind() {
    // Exercise the shared schedule with genuine root leases, not a fabricated
    // Prepared or a protected launch. Public Prepared wiring is checked above.
    if let Err(error) = require_root() {
        eprintln!(
            "SKIP cleanup_guard_real_lease_join_exact_short_substituted_and_unwind: \
             {error}; real-lease accounting, substitution and unwind cases were not \
             exercised. Rootless accounting tests remain independent; no launch or \
             protected-validation credit."
        );
        return;
    }
    let frame = crate::PreparedCompilerExecutionSupervisorV2::FRAME_STORAGE;
    let quota = cleanup_guard_quota(
        CompilerExecutionPreparationQuotaV2 {
            work: Lease::ROOT_BINDING_WORK,
            scratch: Lease::ROOT_BINDING_SCRATCH,
        },
        frame,
    )
    .unwrap();
    let prefix = 2 * Lease::ADMISSION_WORK + Lease::TRANSFER_WORK + Cleanup::GUARD_WORK;
    for mode in 0..6 {
        let mut cleanup = guard_pool();
        let mut work = Work::new(prefix + quota.work() - usize::from(mode == 1));
        let mut b = Budget::new(&mut work, LIMIT);
        let (dir, root, lease) = root_lease(&mut b);
        let (other_dir, other_root, other) = root_lease(&mut b);
        let (guard, charge) = lease.try_clone_for_transfer(&mut b).unwrap();
        b.reserve_storage(charge.additional_storage()).unwrap();
        cleanup.retain_deployment_guard(guard, &mut b).unwrap();
        b.release_storage(charge.additional_storage()).unwrap();
        assert_eq!(b.work(), prefix);
        let owners = b.storage();
        // Prepay unrelated caller storage, leaving exactly the queried allowance.
        let padding = LIMIT - owners - quota.scratch() + usize::from(mode == 2);
        b.reserve_storage(padding).unwrap();
        let floor = b.storage();
        let ledger = b.work_ledger_identity_v1();
        assert!(b.charge_work(LIMIT).is_err());
        assert!(b.reserve_storage(LIMIT).is_err());
        let history = (b.failed_work(), b.failed_storage());
        let pool = cleanup.report().unwrap();
        let object = lease_object(&dir);
        let before = refs(object);
        let result = catch_unwind(AssertUnwindSafe(|| {
            with_cleanup_guard(
                floor,
                frame,
                &mut cleanup,
                &mut b,
                |b| Ok(lease.revalidate_for_root(if mode == 3 { &other_root } else { &root }, b)?),
                |alias, b| {
                    if mode == 5 {
                        panic!("charged cleanup alias unwinds");
                    }
                    let actual = if mode == 4 { &other } else { &lease };
                    Ok(actual.validate_transfer(alias, b)?)
                },
            )
        }));
        match mode {
            0 => {
                result.unwrap().unwrap();
                assert_eq!(b.work(), prefix + quota.work());
                assert_eq!(b.peak_storage(), floor + quota.scratch());
            }
            1 => assert!(matches!(
                result.unwrap(),
                Err(Failure::Lifecycle(LifecycleError::Resource(
                    Resource::Work(_)
                )))
            )),
            2 => assert!(matches!(
                result.unwrap(),
                Err(Failure::Lifecycle(LifecycleError::Resource(
                    Resource::Storage(_)
                )))
            )),
            3 => assert!(matches!(
                result.unwrap(),
                Err(Failure::Lifecycle(LifecycleError::ParentChanged))
            )),
            4 => assert!(matches!(
                result.unwrap(),
                Err(Failure::Lifecycle(LifecycleError::Lease(
                    fe2o3_compiler_execution_lifecycle::LifecycleLeaseErrorV1::FileChanged
                )))
            )),
            _ => assert!(result.is_err()),
        }
        assert_eq!(b.storage(), floor);
        assert_eq!((b.failed_work(), b.failed_storage()), history);
        assert!(b.work_ledger_identity_v1() == ledger);
        assert_eq!(refs(object), before);
        let after = cleanup.report().unwrap();
        assert_eq!(after.storage, pool.storage);
        assert_eq!(after.admission_open, pool.admission_open);
        assert_eq!(
            after.work,
            pool.work
                + if matches!(mode, 2 | 3) {
                    0
                } else {
                    Cleanup::GUARD_CLONE_WORK
                }
        );
        drop((lease, other, root, other_root));
        b.release_storage(owners + padding).unwrap();
        assert_eq!(b.storage(), 0);
        assert_eq!(refs(object), 1);
        cleanup.shutdown().unwrap();
        assert_eq!(refs(object), 0);
        drop((dir, other_dir));
    }
}

fn lease_object(dir: &tempfile::TempDir) -> (u64, u64) {
    let name = std::path::Path::new(
        fe2o3_compiler_execution_protocol::COMPILER_EXECUTION_LIFECYCLE_LOCK_PATH_V1,
    )
    .file_name()
    .unwrap();
    let m = fs::metadata(dir.path().join(name)).unwrap();
    (m.dev(), m.ino())
}

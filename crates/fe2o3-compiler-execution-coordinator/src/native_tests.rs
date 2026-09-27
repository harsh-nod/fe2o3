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
        // Restoring permissions does not restore the pinned inode change time.
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

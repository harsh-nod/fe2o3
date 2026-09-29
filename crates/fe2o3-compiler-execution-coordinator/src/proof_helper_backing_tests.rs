//! Inert selection/accounting and real sealed-image mechanics only. No test
//! manufactures a retained runtime, compiler approval or ProofHelperBacking.
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

fn helper() -> Entry<'static> {
    Entry {
        role: Role::ProofExecutorHelper,
        path: "bin/helper",
        length: 512,
        sha256: [7; 32],
    }
}

#[test]
fn selection_uses_only_the_unique_helper_measurement() {
    let expected = helper();
    let other = Entry {
        role: Role::Rustc,
        sha256: [8; 32],
        ..expected
    };
    let measurement = select_helper([other, expected].into_iter()).unwrap();
    assert_eq!(measurement.sha256(), expected.sha256);
    assert_eq!(measurement.byte_len(), expected.length);
    assert_eq!(measurement.maximum_byte_len(), MAX_IMAGE);
    assert_eq!(
        select_helper(
            [Entry {
                path: "another/approved/helper",
                ..expected
            }]
            .into_iter()
        )
        .unwrap(),
        measurement,
    );
}

#[test]
fn selection_refuses_missing_duplicate_or_invalid_helper_measurement() {
    let entry = helper();
    assert!(matches!(
        select_helper(
            [Entry {
                role: Role::SharedLibrary,
                ..entry
            }]
            .into_iter()
        ),
        Err(ProofHelperBackingError::Manifest(
            "proof helper role is absent"
        )),
    ));
    assert!(matches!(
        select_helper([entry, entry].into_iter()),
        Err(ProofHelperBackingError::Manifest(
            "proof helper role is not unique"
        )),
    ));
    for bad in [
        Entry { length: 0, ..entry },
        Entry {
            length: MAX_IMAGE + 1,
            ..entry
        },
        Entry {
            sha256: [0; 32],
            ..entry
        },
    ] {
        assert!(matches!(
            select_helper([bad].into_iter()),
            Err(ProofHelperBackingError::Image(_))
        ));
    }
}

#[test]
fn selection_has_a_finite_entry_bound() {
    let library = Entry {
        role: Role::SharedLibrary,
        ..helper()
    };
    let exact = std::iter::once(helper()).chain(std::iter::repeat_n(library, MAX_ENTRIES - 1));
    assert!(select_helper(exact).is_ok());
    let mut observed = 0;
    let unbounded = std::iter::once(helper())
        .chain(std::iter::repeat(library))
        .inspect(|_| observed += 1);
    assert!(matches!(
        select_helper(unbounded),
        Err(ProofHelperBackingError::Manifest(
            "proof helper inventory exceeds bound"
        ))
    ));
    assert_eq!(observed, MAX_ENTRIES + 1);
}

#[test]
fn account_requires_original_ledger_address_and_full_reservation() {
    let mut first_work = Work::new(LIMIT);
    let mut second_work = Work::new(LIMIT);
    let mut first = Budget::new(&mut first_work, LIMIT);
    let mut second = Budget::new(&mut second_work, LIMIT);
    first.reserve_storage(512).unwrap();
    second.reserve_storage(512).unwrap();
    let account = Account::capture(&first);
    account.require(&first, 512).unwrap();
    assert!(matches!(
        account.require(&first, 513),
        Err(ProofHelperBackingError::Resource(Resource::Accounting))
    ));
    assert!(matches!(
        account.require(&second, 512),
        Err(ProofHelperBackingError::Resource(Resource::Accounting))
    ));
    std::mem::swap(&mut first, &mut second);
    assert!(matches!(
        account.require(&first, 512),
        Err(ProofHelperBackingError::Resource(Resource::Accounting))
    ));
    assert!(matches!(
        account.require(&second, 512),
        Err(ProofHelperBackingError::Resource(Resource::Accounting))
    ));
    std::mem::swap(&mut first, &mut second);
    account.require(&first, 512).unwrap();
}

#[test]
fn retained_accounting_includes_both_owners_and_checked_growth() {
    fn move_only_custody<T: Send + 'static>() {}
    move_only_custody::<ProofHelperBacking>();
    let (full, growth) = retained_storage_for(1234, 5678).unwrap();
    assert_eq!(growth, 5678 + ProofHelperBacking::ENVELOPE);
    assert_eq!(full, 1234 + growth);
    for (runtime, image) in [(usize::MAX, 1), (1, usize::MAX)] {
        assert!(matches!(
            retained_storage_for(runtime, image),
            Err(ProofHelperBackingError::Resource(Resource::Arithmetic))
        ));
    }
}

struct Fixture {
    directory: tempfile::TempDir,
    measurement: Measurement,
}
impl Fixture {
    fn new() -> Self {
        let directory = tempfile::tempdir().unwrap();
        let bytes = crate::provisioning_entrypoint::static_pause_elf(71);
        let path = directory.path().join("helper");
        fs::write(&path, &bytes).unwrap();
        fs::set_permissions(&path, fs::Permissions::from_mode(0o555)).unwrap();
        let measurement =
            Measurement::new(Sha256::digest(&bytes).into(), bytes.len() as u64, MAX_IMAGE).unwrap();
        Self {
            directory,
            measurement,
        }
    }
    fn open(&self) -> File {
        File::open(self.directory.path().join("helper")).unwrap()
    }
    fn floor(&self) -> usize {
        Image::file_storage(self.measurement).unwrap()
    }
    fn seal(&self, source_storage: usize, b: &mut Budget<'_>) -> Result<Image> {
        let source = self.open();
        b.with_prepaid_scope(source_storage, 0, 0, 0, |b| {
            seal_transferred_source(source, source_storage, self.measurement, credentials(), b)
        })
    }
    fn source_object(&self) -> (u64, u64) {
        let m = fs::metadata(self.directory.path().join("helper")).unwrap();
        (m.dev(), m.ino())
    }
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
fn object(image: &Image) -> (u64, u64) {
    let v = image.object_identity();
    (v.device(), v.inode())
}
fn references(object: (u64, u64)) -> usize {
    fs::read_dir("/proc/self/fd")
        .unwrap()
        .filter_map(|e| e.ok())
        .filter_map(|e| fs::metadata(e.path()).ok())
        .filter(|m| (m.dev(), m.ino()) == object)
        .count()
}

#[test]
fn real_seal_normalizes_source_envelopes_and_retains_a_distinct_image() {
    let fixture = Fixture::new();
    for source_storage in [fixture.floor() - 4, fixture.floor(), fixture.floor() + 47] {
        let mut work = Work::new(LIMIT);
        let mut b = Budget::new(&mut work, LIMIT);
        let floor = EXTRA + source_storage;
        b.reserve_storage(floor).unwrap();
        let source = fixture.open();
        let image = b
            .with_prepaid_scope(source_storage, 0, 0, 0, |b| {
                let image = seal_transferred_source(
                    source,
                    source_storage,
                    fixture.measurement,
                    credentials(),
                    b,
                )?;
                assert_eq!(b.storage(), EXTRA + image.retained_storage());
                Ok::<_, ProofHelperBackingError>(image)
            })
            .unwrap();
        assert_eq!(b.storage(), floor);
        assert_eq!(
            b.work(),
            Image::quota(fixture.measurement, Operation::Admit)
                .unwrap()
                .work()
        );
        assert_ne!(object(&image), fixture.source_object());
        assert_eq!(image.measurement(), fixture.measurement);
        assert_eq!(references(fixture.source_object()), 0);
        // Returned images are unreserved. Preserve all unrelated live storage.
        b.reserve_storage(image.retained_storage()).unwrap();
        check_image(&image, fixture.measurement, credentials(), &mut b).unwrap();
        let sealed_object = object(&image);
        assert_eq!(references(sealed_object), 1);
        drop(image);
        assert_eq!(references(sealed_object), 0);
    }
}

#[test]
fn sealing_requires_exact_work_peak_and_input_floor() {
    let f = Fixture::new();
    let quota = Image::quota(f.measurement, Operation::Admit).unwrap();
    for mode in 0..4 {
        let floor = f.floor() - usize::from(mode == 1);
        let mut work = Work::new(quota.work() - usize::from(mode == 2));
        let mut b = Budget::new(&mut work, floor + quota.scratch() - usize::from(mode == 3));
        b.reserve_storage(floor).unwrap();
        let result = f.seal(f.floor(), &mut b);
        assert_eq!(b.storage(), floor);
        match mode {
            0 => {
                let image = result.unwrap();
                assert_eq!(b.work(), quota.work());
                assert_eq!(b.peak_storage(), floor + quota.scratch());
                b.reserve_storage(image.retained_storage() - f.floor())
                    .unwrap();
                drop(image);
            }
            1 => assert!(matches!(
                result,
                Err(ProofHelperBackingError::Resource(Resource::Accounting))
            )),
            2 => assert!(matches!(
                result,
                Err(ProofHelperBackingError::Image(ImageError::Resource(
                    Resource::Work(_)
                )))
            )),
            _ => assert!(matches!(
                result,
                Err(ProofHelperBackingError::Image(ImageError::Resource(
                    Resource::Storage(_)
                )))
            )),
        }
        assert_eq!(references(f.source_object()), 0);
    }
}

#[test]
fn image_binding_refuses_changed_measurement_or_staging_credentials() {
    let f = Fixture::new();
    let mut work = Work::new(LIMIT);
    let mut b = Budget::new(&mut work, LIMIT);
    b.reserve_storage(f.floor()).unwrap();
    let image = f.seal(f.floor(), &mut b).unwrap();
    b.reserve_storage(image.retained_storage() - f.floor())
        .unwrap();
    let wrong_hash = Measurement::new([19; 32], f.measurement.byte_len(), MAX_IMAGE).unwrap();
    assert!(matches!(
        check_image(&image, wrong_hash, credentials(), &mut b),
        Err(ProofHelperBackingError::BindingMismatch)
    ));
    let wrong_ids = Credentials::new(
        if credentials().uid() == 1 { 2 } else { 1 },
        credentials().gid(),
    )
    .unwrap();
    assert!(matches!(
        check_image(&image, f.measurement, wrong_ids, &mut b),
        Err(ProofHelperBackingError::BindingMismatch)
    ));
    check_image(&image, f.measurement, credentials(), &mut b).unwrap();
}

#[test]
fn exec_transfer_requires_the_sealed_inode_not_source_or_identical_bytes() {
    let f = Fixture::new();
    let mut work = Work::new(LIMIT);
    let mut b = Budget::new(&mut work, LIMIT);
    b.reserve_storage(f.floor()).unwrap();
    let image = f.seal(f.floor(), &mut b).unwrap();
    b.reserve_storage(image.retained_storage() - f.floor())
        .unwrap();
    let (clone, charge) = image.try_clone_for_exec(&mut b).unwrap();
    assert_eq!(charge.additional_storage(), f.floor());
    b.reserve_storage(charge.additional_storage() - 1).unwrap();
    assert!(matches!(
        image.revalidate_exec_clone(&clone, &mut b),
        Err(ImageError::Resource(Resource::Accounting))
    ));
    b.reserve_storage(1).unwrap();
    image.revalidate_exec_clone(&clone, &mut b).unwrap();
    b.reserve_storage(f.floor()).unwrap();
    let source = f.open();
    assert!(matches!(
        image.revalidate_exec_clone(&source, &mut b),
        Err(ImageError::Image(_))
    ));
    drop(source);
    let other = f.seal(f.floor(), &mut b).unwrap();
    b.reserve_storage(other.retained_storage() - f.floor())
        .unwrap();
    assert_eq!(image.measurement(), other.measurement());
    assert_ne!(object(&image), object(&other));
    let (other_clone, charge) = other.try_clone_for_exec(&mut b).unwrap();
    b.reserve_storage(charge.additional_storage()).unwrap();
    assert!(matches!(
        image.revalidate_exec_clone(&other_clone, &mut b),
        Err(ImageError::Image(_))
    ));
    image.revalidate_exec_clone(&clone, &mut b).unwrap();
}

#[test]
fn failed_or_unwound_sealing_restores_storage_and_closes_consumed_files() {
    let f = Fixture::new();
    let mut work = Work::new(LIMIT);
    let mut b = Budget::new(&mut work, LIMIT);
    b.reserve_storage(EXTRA + f.floor()).unwrap();
    let source = f.open();
    let wrong_hash = Measurement::new([20; 32], f.measurement.byte_len(), MAX_IMAGE).unwrap();
    let result = b.with_prepaid_scope(f.floor(), 0, 0, 0, |b| {
        seal_transferred_source(source, f.floor(), wrong_hash, credentials(), b)
    });
    assert!(matches!(
        result,
        Err(ProofHelperBackingError::Image(ImageError::Image(_)))
    ));
    assert_eq!(b.storage(), EXTRA + f.floor());
    assert_eq!(references(f.source_object()), 0);
    let mut sealed = None;
    let panic = catch_unwind(AssertUnwindSafe(|| {
        let source = f.open();
        let _: Result<()> = b.with_prepaid_scope(f.floor(), 0, 0, 0, |b| {
            let image =
                seal_transferred_source(source, f.floor(), f.measurement, credentials(), b)?;
            sealed = Some(object(&image));
            panic!("private post-seal unwind fixture");
        });
    }));
    assert!(panic.is_err());
    assert_eq!(b.storage(), EXTRA + f.floor());
    assert_eq!(references(f.source_object()), 0);
    assert_eq!(references(sealed.unwrap()), 0);
}

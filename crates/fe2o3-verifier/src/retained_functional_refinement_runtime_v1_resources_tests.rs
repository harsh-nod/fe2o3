//! Local denial/accounting tests use only private filesystem mechanics. They
//! cannot construct a protected Runtime or an approved compiler/process owner.
//! The explicitly ignored test at the end requires real protected admission.

use super::super::{
    FUNCTIONAL_REFINEMENT_RUNTIME_V1_MANIFEST_NAME as MANIFEST_NAME, FileSpecV2,
    open_retained_generated_verus_runtime_v1,
};
use super::*;
use fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1 as Work;
use sha2::{Digest, Sha256};
use std::{
    fs,
    os::unix::fs::{PermissionsExt, symlink},
    panic::{AssertUnwindSafe, catch_unwind},
    path::PathBuf,
    sync::atomic::{AtomicU64, Ordering},
};

const PREFIX: usize = 17;
const FLOOR: usize = 29;
const ROOT: &str = "/opt/fe2o3/verus-runtime-v2/functional-refinement-0.2026.08.02-b677dd5";

#[test]
fn entry_work_denial_preserves_original_prefix_and_storage() {
    let mut work = Work::new(PREFIX + ENTRY_WORK - 1);
    let mut b = Budget::new(&mut work, usize::MAX);
    b.charge_work(PREFIX).unwrap();
    b.reserve_storage(FLOOR).unwrap();
    let error = open_retained_generated_verus_runtime_bounded_v1(Path::new(ROOT), &mut b)
        .err()
        .unwrap();
    assert!(matches!(error, Error::Resource(Resource::Work(_))));
    assert_eq!(b.work(), PREFIX);
    assert_eq!(b.failed_work(), Some(PREFIX + ENTRY_WORK));
    assert_eq!((b.storage(), b.peak_storage()), (FLOOR, FLOOR));
}

#[test]
fn preparation_storage_denial_precedes_path_validation() {
    let mut work = Work::new(usize::MAX);
    let mut b = Budget::new(&mut work, FLOOR + PREPARATION_SCRATCH - 1);
    b.charge_work(PREFIX).unwrap();
    b.reserve_storage(FLOOR).unwrap();
    let error = open_retained_generated_verus_runtime_bounded_v1(Path::new("relative"), &mut b)
        .err()
        .unwrap();
    assert!(matches!(error, Error::Resource(Resource::Storage(_))));
    assert_eq!(b.work(), PREFIX + PREPARATION_WORK);
    assert_eq!((b.storage(), b.peak_storage()), (FLOOR, FLOOR));
    assert_eq!(b.failed_storage(), Some(FLOOR + PREPARATION_SCRATCH));
}

#[test]
fn invalid_origins_use_shared_refusals_and_never_reset_work() {
    let mut work = Work::new(usize::MAX);
    let mut b = Budget::new(&mut work, usize::MAX);
    b.charge_work(PREFIX).unwrap();
    b.reserve_storage(FLOOR).unwrap();
    for (index, path) in [
        "relative",
        "/tmp/runtime",
        "/opt/fe2o3/verus-runtime-v2/../wrong",
    ]
    .into_iter()
    .enumerate()
    {
        let path = Path::new(path);
        let legacy = open_retained_generated_verus_runtime_v1(path)
            .err()
            .unwrap();
        let error = open_retained_generated_verus_runtime_bounded_v1(path, &mut b)
            .err()
            .unwrap();
        assert_eq!(error, Error::Runtime(legacy.kind()));
        assert_eq!(b.work(), PREFIX + (index + 1) * PREPARATION_WORK);
        assert_eq!(b.storage(), FLOOR);
        assert_eq!(b.peak_storage(), FLOOR + PREPARATION_SCRATCH);
    }
}

#[test]
fn overlong_root_is_rejected_before_scanning_its_components() {
    let root = PathBuf::from("/".repeat(MAX_ABSOLUTE_PATH_BYTES + 1));
    let mut work = Work::new(usize::MAX);
    let mut b = Budget::new(&mut work, usize::MAX);
    assert_eq!(
        open_retained_generated_verus_runtime_bounded_v1(&root, &mut b)
            .err()
            .unwrap(),
        Error::Runtime(RuntimeKind::InvalidManifest),
    );
    assert_eq!(b.storage(), 0);
}

#[test]
fn absent_protected_origin_fails_after_admission_prepayment() {
    let root = PathBuf::from(format!(
        "/opt/fe2o3/verus-runtime-v2/absent-resource-test-{}",
        std::process::id()
    ));
    assert!(!root.try_exists().unwrap());
    let expected = open_retained_generated_verus_runtime_v1(&root)
        .err()
        .unwrap()
        .kind();
    let manifest = runtime_manifest(&root).unwrap();
    let quota = AdmissionQuota::new(&manifest).unwrap();
    let mut work = Work::new(usize::MAX);
    let mut b = Budget::new(&mut work, usize::MAX);
    b.charge_work(PREFIX).unwrap();
    b.reserve_storage(FLOOR).unwrap();
    let error = open_retained_generated_verus_runtime_bounded_v1(&root, &mut b)
        .err()
        .unwrap();
    assert_eq!(error, Error::Runtime(expected));
    assert_eq!(b.work(), PREFIX + PREPARATION_WORK + quota.work);
    assert_eq!(b.storage(), FLOOR);
    assert_eq!(
        b.peak_storage(),
        FLOOR + PREPARATION_SCRATCH + quota.scratch
    );
}

#[test]
fn backing_and_scan_overlap_are_admitted_before_opening_the_runtime() {
    let manifest = runtime_manifest(Path::new(ROOT)).unwrap();
    let quota = AdmissionQuota::new(&manifest).unwrap();
    for work_denial in [true, false] {
        let mut work = Work::new(if work_denial {
            PREFIX + PREPARATION_WORK + quota.work - 1
        } else {
            usize::MAX
        });
        let mut b = Budget::new(&mut work, FLOOR + PREPARATION_SCRATCH + quota.scratch - 1);
        b.charge_work(PREFIX).unwrap();
        b.reserve_storage(FLOOR).unwrap();
        let error = open_retained_generated_verus_runtime_bounded_v1(Path::new(ROOT), &mut b)
            .err()
            .unwrap();
        if work_denial {
            assert!(matches!(error, Error::Resource(Resource::Work(_))));
            assert_eq!(b.work(), PREFIX + PREPARATION_WORK);
            assert_eq!(
                b.failed_work(),
                Some(PREFIX + PREPARATION_WORK + quota.work)
            );
            assert_eq!(b.failed_storage(), None);
        } else {
            assert!(matches!(error, Error::Resource(Resource::Storage(_))));
            assert_eq!(b.work(), PREFIX + PREPARATION_WORK + quota.work);
            assert_eq!(
                b.failed_storage(),
                Some(FLOOR + PREPARATION_SCRATCH + quota.scratch)
            );
        }
        assert_eq!(b.storage(), FLOOR);
        assert_eq!(b.peak_storage(), FLOOR + PREPARATION_SCRATCH);
    }
}

static NEXT: AtomicU64 = AtomicU64::new(0);
const LOCAL_MANIFEST: &[u8] = b"private-accounting-fixture\n";
const LOCAL_FILE: &[u8] = b"local-retained-file";
struct Tree {
    root: PathBuf,
    manifest: ManifestV2,
}
impl Tree {
    fn new() -> Self {
        // Match the retained-closure fixture's writer/fork coordination. A child
        // must not inherit a setup writer and close it after the journal starts.
        let _process_guard = linux::RUNTIME_CLOSURE_PROCESS_TEST_LOCK
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let root = std::env::temp_dir().join(format!(
            "fe2o3-retained-runtime-account-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed),
        ));
        fs::create_dir(&root).unwrap();
        fs::write(root.join(MANIFEST_NAME), LOCAL_MANIFEST).unwrap();
        fs::write(root.join("data"), LOCAL_FILE).unwrap();
        for name in [MANIFEST_NAME, "data"] {
            fs::set_permissions(root.join(name), fs::Permissions::from_mode(0o444)).unwrap();
        }
        fs::set_permissions(&root, fs::Permissions::from_mode(0o555)).unwrap();
        let manifest = ManifestV2::synthetic(
            LOCAL_MANIFEST,
            vec![],
            vec![FileSpecV2 {
                path: PathBuf::from("data"),
                mode: 0o444,
                size: Some(LOCAL_FILE.len() as u64),
                sha256: Sha256::digest(LOCAL_FILE).into(),
            }],
        );
        Self { root, manifest }
    }
    fn open(
        &self,
        b: &mut Budget<'_>,
    ) -> Result<(linux::RetainedRuntimeClosureV2, RuntimeAccountV1)> {
        b.with_prepaid_scope(0, ENTRY_WORK, PREPARATION_WORK, PREPARATION_SCRATCH, |b| {
            let quota = AdmissionQuota::new(&self.manifest)?;
            quota.admit(b, |b| {
                let retained =
                    linux::RetainedRuntimeClosureV2::open_for_test(&self.root, &self.manifest)?;
                let account = RuntimeAccountV1::bind(&retained, &quota, b)?;
                Ok((retained, account))
            })
        })
    }
}
impl Drop for Tree {
    fn drop(&mut self) {
        let _ = fs::set_permissions(&self.root, fs::Permissions::from_mode(0o755));
        let _ = fs::remove_dir_all(&self.root);
    }
}

#[test]
fn local_filesystem_failures_preserve_floor_and_charge_the_attempt() {
    for replace_with_symlink in [false, true] {
        let tree = Tree::new();
        let data = tree.root.join("data");
        if replace_with_symlink {
            fs::set_permissions(&tree.root, fs::Permissions::from_mode(0o755)).unwrap();
            fs::remove_file(&data).unwrap();
            symlink(MANIFEST_NAME, &data).unwrap();
            fs::set_permissions(&tree.root, fs::Permissions::from_mode(0o555)).unwrap();
        } else {
            fs::set_permissions(&data, fs::Permissions::from_mode(0o644)).unwrap();
            fs::write(&data, vec![b'x'; LOCAL_FILE.len()]).unwrap();
            fs::set_permissions(&data, fs::Permissions::from_mode(0o444)).unwrap();
        }
        let mut work = Work::new(usize::MAX);
        let mut b = Budget::new(&mut work, usize::MAX);
        b.charge_work(PREFIX).unwrap();
        b.reserve_storage(FLOOR).unwrap();
        let error = tree.open(&mut b).err().unwrap();
        assert_eq!(
            error,
            Error::Runtime(if replace_with_symlink {
                RuntimeKind::ObjectType
            } else {
                RuntimeKind::ContentMismatch
            })
        );
        let quota = AdmissionQuota::new(&tree.manifest).unwrap();
        assert_eq!(b.work(), PREFIX + PREPARATION_WORK + quota.work);
        assert_eq!(b.storage(), FLOOR);
        assert_eq!(
            b.peak_storage(),
            FLOOR + PREPARATION_SCRATCH + quota.scratch
        );
    }
}

#[test]
fn local_retention_requires_full_backing_and_preserves_mutation_refusal() {
    let tree = Tree::new();
    let mut work = Work::new(usize::MAX);
    let mut b = Budget::new(&mut work, usize::MAX);
    let (retained, account) = tree.open(&mut b).unwrap();
    assert_eq!(b.storage(), 0, "returned backing is unreserved");
    assert_eq!(
        account.storage,
        linux::RETAINED_METADATA_STORAGE + LOCAL_MANIFEST.len() + LOCAL_FILE.len()
    );
    b.reserve_storage(account.storage - 1).unwrap();
    assert_eq!(
        account.revalidate(&mut b, || panic!("unpaid backing")),
        Err::<(), _>(Resource::Accounting.into())
    );
    b.reserve_storage(1).unwrap();
    account
        .revalidate(&mut b, || retained.revalidate().map_err(Error::from))
        .unwrap();
    assert_eq!(b.storage(), account.storage);
    fs::set_permissions(tree.root.join("data"), fs::Permissions::from_mode(0o644)).unwrap();
    assert_eq!(
        account.revalidate(&mut b, || retained.revalidate().map_err(Error::from)),
        Err(Error::Runtime(RuntimeKind::ClosureChanged)),
    );
    assert_eq!(b.storage(), account.storage);
    drop(retained);
    b.release_storage(account.storage).unwrap();
    assert_eq!(b.storage(), 0);
}

#[test]
fn local_revalidation_denies_replacement_ledger_and_short_scratch_before_io() {
    let tree = Tree::new();
    let mut work = Work::new(usize::MAX);
    let mut b = Budget::new(&mut work, usize::MAX);
    let (retained, account) = tree.open(&mut b).unwrap();
    b.reserve_storage(account.storage).unwrap();
    let mut other_work = Work::new(usize::MAX);
    let mut other = Budget::new(&mut other_work, usize::MAX);
    other.reserve_storage(account.storage).unwrap();
    std::mem::swap(&mut b, &mut other);
    assert_eq!(
        account.revalidate(&mut b, || panic!("replacement ledger")),
        Err::<(), _>(Resource::Accounting.into())
    );
    assert_eq!(
        account.revalidate(&mut other, || panic!("moved budget")),
        Err::<(), _>(Resource::Accounting.into())
    );
    std::mem::swap(&mut b, &mut other);
    let fill = b.storage_limit() - account.storage - account.revalidation_scratch + 1;
    b.reserve_storage(fill).unwrap();
    let error = account
        .revalidate::<()>(&mut b, || panic!("unpaid scan"))
        .unwrap_err();
    assert!(matches!(error, Error::Resource(Resource::Storage(_))));
    assert_eq!(b.storage(), account.storage + fill);
    b.release_storage(fill).unwrap();
    account
        .revalidate(&mut b, || retained.revalidate().map_err(Error::from))
        .unwrap();
}

#[test]
fn unwind_drops_local_custody_before_releasing_the_admission_scope() {
    let tree = Tree::new();
    let quota = AdmissionQuota::new(&tree.manifest).unwrap();
    let mut work = Work::new(usize::MAX);
    let mut b = Budget::new(&mut work, usize::MAX);
    b.reserve_storage(FLOOR).unwrap();
    let callback_entered = std::cell::Cell::new(false);
    let retained_opened = std::cell::Cell::new(false);
    let refusal_detail = std::cell::RefCell::new(None);
    let outcome = catch_unwind(AssertUnwindSafe(|| {
        quota.admit::<()>(&mut b, |_| {
            callback_entered.set(true);
            let _retained =
                linux::RetainedRuntimeClosureV2::open_for_test(&tree.root, &tree.manifest)
                    .map_err(|error| {
                        *refusal_detail.borrow_mut() = Some(error.to_string());
                        error
                    })?;
            retained_opened.set(true);
            panic!("local admission unwind");
        })
    }));
    match outcome {
        Err(payload) => assert_eq!(
            payload.downcast_ref::<&str>().copied(),
            Some("local admission unwind"),
            "only the intentional post-retention panic exercises this cleanup path",
        ),
        Ok(Err(error)) => panic!(
            "admission returned before intended unwind: {error:?}; callback_entered={}, retained_opened={}, detail={:?}",
            callback_entered.get(),
            retained_opened.get(),
            refusal_detail.borrow(),
        ),
        Ok(Ok(())) => panic!("admission unexpectedly returned without the intended unwind"),
    }
    assert!(callback_entered.get());
    assert!(retained_opened.get());
    assert_eq!(b.storage(), FLOOR);
    assert_eq!(b.peak_storage(), FLOOR + quota.scratch);
    assert_eq!(b.work(), quota.work);
}

#[test]
fn accounting_fixture_parallel_setup_preserves_clean_admission_and_mutation_refusal() {
    const REPETITIONS: usize = 32;
    let start = std::sync::Barrier::new(2);
    std::thread::scope(|scope| {
        scope.spawn(|| {
            start.wait();
            for _ in 0..REPETITIONS {
                let _process_guard = linux::RUNTIME_CLOSURE_PROCESS_TEST_LOCK
                    .lock()
                    .unwrap_or_else(|poisoned| poisoned.into_inner());
                let mut command = std::process::Command::new("/bin/true");
                assert!(
                    crate::executor::status_artifact_coordinated_child(&mut command)
                        .unwrap()
                        .success()
                );
            }
        });
        start.wait();
        for _ in 0..REPETITIONS {
            let tree = Tree::new();
            let mut work = Work::new(usize::MAX);
            let mut b = Budget::new(&mut work, usize::MAX);
            b.reserve_storage(FLOOR).unwrap();
            let (retained, account) = tree.open(&mut b).unwrap();
            assert_eq!(b.storage(), FLOOR);
            b.reserve_storage(account.storage).unwrap();
            account
                .revalidate(&mut b, || retained.revalidate().map_err(Error::from))
                .unwrap();

            let path = tree.root.join("data");
            fs::set_permissions(&path, fs::Permissions::from_mode(0o644)).unwrap();
            fs::write(&path, LOCAL_FILE).unwrap();
            fs::set_permissions(&path, fs::Permissions::from_mode(0o444)).unwrap();
            assert_eq!(
                account.revalidate(&mut b, || retained.revalidate().map_err(Error::from)),
                Err(Error::Runtime(RuntimeKind::ClosureChanged)),
            );
            drop(retained);
            b.release_storage(account.storage).unwrap();
            assert_eq!(b.storage(), FLOOR);
        }
    });
}

#[test]
#[ignore = "requires actual installed root-protected pinned runtime and admitted execution environment"]
fn protected_runtime_retains_complete_backing_on_original_budget() {
    let mut work = Work::new(usize::MAX);
    let mut b = Budget::new(&mut work, usize::MAX);
    let (runtime, charge) =
        open_retained_generated_verus_runtime_bounded_v1(Path::new(ROOT), &mut b)
            .expect("real protected admission, no synthetic owner");
    assert_eq!(b.storage(), 0);
    assert_eq!(charge.additional_storage(), charge.retained_storage());
    assert_eq!(
        runtime.required_retained_storage_v1().unwrap(),
        charge.retained_storage()
    );
    let manifest = runtime_manifest(Path::new(ROOT)).unwrap();
    let interpreter = manifest.interpreter.as_ref().unwrap().size as usize;
    let files: usize = manifest
        .files
        .iter()
        .map(|file| {
            fs::metadata(Path::new(ROOT).join(&file.path))
                .unwrap()
                .len() as usize
        })
        .sum();
    assert_eq!(
        charge.retained_storage(),
        linux::RETAINED_METADATA_STORAGE + manifest.manifest_bytes.len() + files + 2 * interpreter
    );
    assert_eq!(
        runtime.revalidate_bounded_v1(&mut b),
        Err(Error::Resource(Resource::Accounting)),
        "the returned runtime's complete backing must first be reserved",
    );
    b.reserve_storage(charge.retained_storage()).unwrap();
    runtime.revalidate_bounded_v1(&mut b).unwrap();
    assert_eq!(b.storage(), charge.retained_storage());

    let before_legacy = (b.work(), b.storage());
    assert_eq!(
        runtime.revalidate().unwrap_err().kind(),
        RuntimeKind::Protection
    );
    assert_eq!(
        runtime
            .begin_attempt()
            .err()
            .expect("legacy attempt refused")
            .kind(),
        RuntimeKind::Protection
    );
    // Exercise the execution entry gate without proof input or attempt custody.
    assert_eq!(
        runtime
            .with_legacy_access::<()>(|| panic!("legacy execution dispatch reached"))
            .unwrap_err()
            .kind(),
        RuntimeKind::Protection
    );
    assert_eq!((b.work(), b.storage()), before_legacy);

    let original_ledger = b.work_ledger_identity_v1();
    let original_work = b.work();
    let mut other_work = Work::new(usize::MAX);
    let mut other = Budget::new(&mut other_work, usize::MAX);
    other.charge_work(PREFIX).unwrap();
    other.reserve_storage(charge.retained_storage()).unwrap();
    std::mem::swap(&mut b, &mut other);
    assert!(b.work_ledger_identity_v1() != original_ledger);
    assert_eq!(
        runtime.revalidate_bounded_v1(&mut b),
        Err(Error::Resource(Resource::Accounting)),
        "equal storage at the original address cannot replace the original ledger",
    );
    assert!(other.work_ledger_identity_v1() == original_ledger);
    assert_eq!(
        runtime.revalidate_bounded_v1(&mut other),
        Err(Error::Resource(Resource::Accounting)),
        "the original ledger cannot move to another Budget address",
    );
    assert_eq!(b.work(), PREFIX + ENTRY_WORK);
    assert_eq!(other.work(), original_work + ENTRY_WORK);
    assert_eq!(b.storage(), charge.retained_storage());
    assert_eq!(other.storage(), charge.retained_storage());
    std::mem::swap(&mut b, &mut other);
    other.release_storage(charge.retained_storage()).unwrap();
    runtime.revalidate_bounded_v1(&mut b).unwrap();
    assert_eq!(b.storage(), charge.retained_storage());
    // This is static protected runtime custody only, never proof/execution credit.
    drop(runtime);
    b.release_storage(charge.retained_storage()).unwrap();
    assert_eq!(b.storage(), 0);
}

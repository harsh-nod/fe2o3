//! Private Inventory fixtures only: no public retained runtime or approval is
//! fabricated. The immutability stub is not evidence of production FS_IMMUTABLE.
use super::*;
use fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1 as Work;
use std::{
    cell::Cell,
    fs,
    os::unix::fs::{PermissionsExt, symlink},
    path::PathBuf,
    sync::atomic::{AtomicU64, Ordering},
};

const APPROVAL_STORAGE: usize = 4096;
const LIMIT: usize = 100_000_000;
const PATHS: [&str; 6] = [
    "bin/helper",
    "bin/rustc",
    "lib/ld.so",
    "lib/libc.so",
    "lib/libfe2o3.so",
    "lib/libfe2o3_macros.so",
];
const ROLES: [CompilerRuntimeRoleV1; 6] = [
    CompilerRuntimeRoleV1::ProofExecutorHelper,
    CompilerRuntimeRoleV1::Rustc,
    CompilerRuntimeRoleV1::ElfInterpreter,
    CompilerRuntimeRoleV1::SharedLibrary,
    CompilerRuntimeRoleV1::CodegenBackend,
    CompilerRuntimeRoleV1::Fe2o3ProcMacro,
];
thread_local! { static PROBES: Cell<usize> = const { Cell::new(0) }; }
fn synthetic_immutable(_: &File) -> Result<()> {
    PROBES.with(|v| v.set(v.get() + 1));
    Ok(())
}
fn owners() -> (u32, u32) {
    (
        rustix::process::getuid().as_raw(),
        rustix::process::getgid().as_raw(),
    )
}
fn chmod(path: &Path, mode: u32) {
    fs::set_permissions(path, fs::Permissions::from_mode(mode)).unwrap();
}

struct Tree {
    root: PathBuf,
    policy: CompilerApprovalPolicyV1,
}
impl Tree {
    fn new(helper_length: usize) -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let root = std::env::temp_dir().join(format!(
            "compiler-helper-transfer-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed),
        ));
        fs::create_dir(&root).unwrap();
        chmod(&root, 0o700);
        for path in [
            "etc",
            "etc/fe2o3",
            "etc/fe2o3/build-authority",
            "opt",
            "opt/fe2o3",
            "opt/fe2o3/compiler-runtime-v1",
            "opt/fe2o3/compiler-runtime-v1/bin",
            "opt/fe2o3/compiler-runtime-v1/lib",
        ] {
            fs::create_dir(root.join(path)).unwrap();
            chmod(&root.join(path), 0o755);
        }
        let bytes: [Vec<u8>; 6] =
            std::array::from_fn(|i| vec![i as u8 + 1; if i == 0 { helper_length } else { 64 }]);
        let digests: [[u8; 32]; 6] = std::array::from_fn(|i| Sha256::digest(&bytes[i]).into());
        let closure =
            CompilerClosureV2::new([1; 32], [2; 32], [3; 32], digests[1], [5; 32], digests[4])
                .unwrap();
        let entries: [_; 6] = std::array::from_fn(|i| CompilerRuntimeEntryV1 {
            role: ROLES[i],
            path: PATHS[i],
            length: bytes[i].len() as u64,
            sha256: digests[i],
        });
        let manifest =
            CompilerRuntimeManifestV1::new(closure, [18; 32], &entries, |_| Ok::<_, Resource>(()))
                .unwrap();
        let policy =
            CompilerApprovalPolicyV1::new(closure, [17; 32], *manifest.identity(), 1, |_| {
                Ok::<_, Resource>(())
            })
            .unwrap();
        let tree = Self { root, policy };
        for (i, entry) in manifest.entries().enumerate() {
            fs::write(tree.code(entry.path), &bytes[i]).unwrap();
            chmod(&tree.code(entry.path), entry.role.protected_mode());
        }
        fs::write(tree.manifest_path(), manifest.canonical_bytes()).unwrap();
        chmod(&tree.manifest_path(), 0o444);
        tree
    }
    fn code(&self, path: &str) -> PathBuf {
        self.root
            .join(COMPILER_RUNTIME_ROOT_V1.strip_prefix('/').unwrap())
            .join(path)
    }
    fn manifest_path(&self) -> PathBuf {
        self.root
            .join(COMPILER_RUNTIME_MANIFEST_PATH_V1.strip_prefix('/').unwrap())
    }
    fn open(&self) -> Result<File> {
        rustix::fs::open(&self.root, tree::DIRECTORY_FLAGS, Mode::empty())
            .map(File::from)
            .map_err(|e| io("open synthetic transfer root", e))
    }
    fn retain(&self, b: &mut Budget<'_>) -> Inventory {
        let (uid, gid) = owners();
        let inventory = Inventory::load_using(
            &self.policy,
            APPROVAL_STORAGE,
            || self.open(),
            uid,
            gid,
            synthetic_immutable,
            b,
        )
        .unwrap();
        b.reserve_storage(inventory.additional_storage()).unwrap();
        inventory
    }
    fn revalidate(&self, v: &Inventory, b: &mut Budget<'_>) -> Result<()> {
        let (uid, gid) = owners();
        v.revalidate_using(
            &self.policy,
            || self.open(),
            uid,
            gid,
            synthetic_immutable,
            b,
        )
    }
    fn duplicate(
        &self,
        v: &Inventory,
        b: &mut Budget<'_>,
    ) -> Result<(File, RetainedCompilerRuntimeExecTransferChargeV1)> {
        let (uid, gid) = owners();
        v.clone_proof_executor_using(uid, gid, synthetic_immutable, b, |b| self.revalidate(v, b))
    }
    fn validate(&self, v: &Inventory, file: &File, b: &mut Budget<'_>) -> Result<()> {
        let (uid, gid) = owners();
        v.validate_proof_executor_using(file, uid, gid, synthetic_immutable, b, |b| {
            self.revalidate(v, b)
        })
    }
    fn replace_helper(&self) {
        let source = self.code(PATHS[0]);
        let bytes = fs::read(&source).unwrap();
        fs::rename(&source, self.root.join("previous-helper")).unwrap();
        fs::write(&source, bytes).unwrap();
        chmod(&source, 0o555);
    }
}
impl Drop for Tree {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.root).unwrap();
    }
}
fn accounting<T>(result: Result<T>) {
    assert!(matches!(
        result,
        Err(RetainedCompilerRuntimeErrorV1::Resource(
            Resource::Accounting
        ))
    ));
}

#[test]
fn synthetic_helper_transfer_has_full_actual_length_charge_and_preserves_inventory() {
    for length in [64, 137, CHUNK + 1] {
        let t = Tree::new(length);
        let mut work = Work::new(LIMIT);
        let mut b = Budget::new(&mut work, LIMIT);
        b.reserve_storage(APPROVAL_STORAGE).unwrap();
        let v = t.retain(&mut b);
        let before = b.storage();
        let original = &v.files[0].as_ref().unwrap().file;
        rustix::fs::seek(original, rustix::fs::SeekFrom::Start(9)).unwrap();
        let (file, charge) = t.duplicate(&v, &mut b).unwrap();
        assert_eq!(
            charge.full_storage(),
            size_of::<(File, RetainedCompilerRuntimeExecTransferChargeV1)>() + length
        );
        assert_eq!(b.storage(), before);
        assert_eq!(
            Snapshot::read(&file).unwrap(),
            v.files[0].as_ref().unwrap().snapshot
        );
        assert!(
            rustix::io::fcntl_getfd(&file)
                .unwrap()
                .contains(rustix::io::FdFlags::CLOEXEC)
        );
        assert_eq!(
            rustix::fs::fcntl_getfl(&file).unwrap() & OFlags::ACCMODE,
            OFlags::RDONLY
        );
        b.reserve_storage(charge.full_storage()).unwrap();
        t.validate(&v, &file, &mut b).unwrap();
        assert_eq!(
            rustix::fs::seek(&file, rustix::fs::SeekFrom::Current(0)).unwrap(),
            9
        );
        assert_eq!(
            rustix::fs::seek(original, rustix::fs::SeekFrom::Current(0)).unwrap(),
            9
        );
        assert_eq!(b.storage(), before + charge.full_storage());
        drop(file);
        b.release_storage(charge.full_storage()).unwrap();
        t.revalidate(&v, &mut b).unwrap();
        assert_eq!(b.storage(), before);
        assert!(!v.manifest.grants_authority());
    }
}

#[test]
fn transfer_rejects_wrong_role_same_content_other_inode_and_descriptor_flags() {
    let t = Tree::new(64);
    let mut work = Work::new(LIMIT);
    let mut b = Budget::new(&mut work, LIMIT);
    b.reserve_storage(APPROVAL_STORAGE).unwrap();
    let v = t.retain(&mut b);
    let (file, charge) = t.duplicate(&v, &mut b).unwrap();
    b.reserve_storage(charge.full_storage()).unwrap();
    let wrong_role = &v.files[1].as_ref().unwrap().file;
    assert!(matches!(
        t.validate(&v, wrong_role, &mut b),
        Err(RetainedCompilerRuntimeErrorV1::Mismatch(
            "proof executor transfer origin differs from retained entry"
        ))
    ));
    let copy_path = t.root.join("same-content-other-inode");
    fs::write(&copy_path, [1; 64]).unwrap();
    chmod(&copy_path, 0o555);
    let copy = File::open(&copy_path).unwrap();
    assert!(t.validate(&v, &copy, &mut b).is_err());
    rustix::io::fcntl_setfd(&file, rustix::io::FdFlags::empty()).unwrap();
    assert!(matches!(
        t.validate(&v, &file, &mut b),
        Err(RetainedCompilerRuntimeErrorV1::Mismatch(
            "invalid protected code file"
        ))
    ));
    assert_eq!(b.storage(), v.required_storage() + charge.full_storage());
}

#[test]
fn transfer_rejects_replaced_or_symlinked_fixed_helper_and_manifest_paths() {
    for attack in 0..3 {
        let t = Tree::new(64);
        let mut work = Work::new(LIMIT);
        let mut b = Budget::new(&mut work, LIMIT);
        b.reserve_storage(APPROVAL_STORAGE).unwrap();
        let v = t.retain(&mut b);
        let (file, charge) = t.duplicate(&v, &mut b).unwrap();
        b.reserve_storage(charge.full_storage()).unwrap();
        let before = b.storage();
        match attack {
            0 => t.replace_helper(),
            1 => {
                fs::rename(t.code(PATHS[0]), t.root.join("previous-helper")).unwrap();
                symlink(t.root.join("previous-helper"), t.code(PATHS[0])).unwrap();
            }
            _ => {
                let bytes = fs::read(t.manifest_path()).unwrap();
                fs::remove_file(t.manifest_path()).unwrap();
                fs::write(t.manifest_path(), bytes).unwrap();
                chmod(&t.manifest_path(), 0o444);
            }
        }
        assert!(t.duplicate(&v, &mut b).is_err());
        assert!(t.validate(&v, &file, &mut b).is_err());
        assert_eq!(b.storage(), before);
    }
}

#[test]
fn private_transfer_check_requires_digest_and_protection_not_only_matching_inode() {
    let t = Tree::new(64);
    let mut work = Work::new(LIMIT);
    let mut b = Budget::new(&mut work, LIMIT);
    b.reserve_storage(APPROVAL_STORAGE).unwrap();
    let v = t.retain(&mut b);
    b.reserve_storage(TRANSFER_SCRATCH).unwrap();
    let mut selected = v.proof_executor(&mut b).unwrap();
    let file = &selected.retained.file;
    let (uid, gid) = owners();
    // Alter only the private test comparison, not public approval or custody.
    selected.entry.sha256[0] ^= 1;
    assert!(matches!(
        selected.check_file(file, uid, gid, synthetic_immutable, &mut b),
        Err(RetainedCompilerRuntimeErrorV1::Mismatch(
            "code bytes differ from approved digest"
        ))
    ));
    selected.entry.sha256[0] ^= 1;
    fn refused_immutable(_: &File) -> Result<()> {
        Err(mismatch("synthetic transfer protection refusal"))
    }
    assert!(matches!(
        selected.check_file(file, uid, gid, refused_immutable, &mut b),
        Err(RetainedCompilerRuntimeErrorV1::Mismatch(
            "synthetic transfer protection refusal"
        ))
    ));
}

#[test]
fn post_duplicate_origin_revalidation_is_required_and_custody_charge_rolls_back() {
    let t = Tree::new(64);
    let mut work = Work::new(LIMIT);
    let mut b = Budget::new(&mut work, LIMIT);
    b.reserve_storage(APPROVAL_STORAGE).unwrap();
    let v = t.retain(&mut b);
    let before = b.storage();
    let charge = size_of::<(File, RetainedCompilerRuntimeExecTransferChargeV1)>() + 64;
    let (uid, gid) = owners();
    let mut calls = 0;
    let result = v.clone_proof_executor_using(uid, gid, synthetic_immutable, &mut b, |b| {
        calls += 1;
        assert!(b.storage() >= before + charge + TRANSFER_SCRATCH);
        if calls == 2 {
            t.replace_helper();
        }
        t.revalidate(&v, b)
    });
    assert!(result.is_err());
    assert_eq!(calls, 2);
    assert_eq!(b.storage(), before);
}

#[test]
fn final_transfer_validation_rechecks_origins_after_file_inspection() {
    let t = Tree::new(64);
    let mut work = Work::new(LIMIT);
    let mut b = Budget::new(&mut work, LIMIT);
    b.reserve_storage(APPROVAL_STORAGE).unwrap();
    let v = t.retain(&mut b);
    let (file, charge) = t.duplicate(&v, &mut b).unwrap();
    b.reserve_storage(charge.full_storage()).unwrap();
    let before = b.storage();
    let (uid, gid) = owners();
    let mut calls = 0;
    let result =
        v.validate_proof_executor_using(&file, uid, gid, synthetic_immutable, &mut b, |b| {
            calls += 1;
            if calls == 2 {
                t.replace_helper();
            }
            t.revalidate(&v, b)
        });
    assert!(result.is_err());
    assert_eq!(calls, 2);
    assert_eq!(b.storage(), before);
}

#[test]
fn missing_retained_helper_refuses_before_origin_io() {
    let t = Tree::new(64);
    let mut work = Work::new(LIMIT);
    let mut b = Budget::new(&mut work, LIMIT);
    b.reserve_storage(APPROVAL_STORAGE).unwrap();
    let mut v = t.retain(&mut b);
    v.files[0] = None;
    PROBES.with(|v| v.set(0));
    assert!(matches!(
        t.duplicate(&v, &mut b),
        Err(RetainedCompilerRuntimeErrorV1::Mismatch(
            "proof executor custody absent"
        ))
    ));
    assert_eq!(PROBES.with(Cell::get), 0);
}

#[test]
fn source_and_transfer_each_require_their_complete_live_reservation() {
    let t = Tree::new(64);
    let mut work = Work::new(LIMIT);
    let mut b = Budget::new(&mut work, LIMIT);
    b.reserve_storage(APPROVAL_STORAGE).unwrap();
    let v = t.retain(&mut b);
    let (file, charge) = t.duplicate(&v, &mut b).unwrap();
    PROBES.with(|v| v.set(0));
    accounting(t.validate(&v, &file, &mut b));
    b.reserve_storage(charge.full_storage() - 1).unwrap();
    accounting(t.validate(&v, &file, &mut b));
    assert_eq!(PROBES.with(Cell::get), 0);
    b.reserve_storage(1).unwrap();
    t.validate(&v, &file, &mut b).unwrap();
    drop(file);
    b.release_storage(charge.full_storage() + 1).unwrap();
    PROBES.with(|v| v.set(0));
    accounting(t.duplicate(&v, &mut b));
    assert_eq!(PROBES.with(Cell::get), 0);
}

#[test]
fn transfer_refuses_cross_ledger_and_moved_original_budget_before_io() {
    let t = Tree::new(64);
    let mut work = Work::new(LIMIT);
    let mut other = Work::new(LIMIT);
    let mut b = Budget::new(&mut work, LIMIT);
    b.reserve_storage(APPROVAL_STORAGE).unwrap();
    let v = t.retain(&mut b);
    let (file, charge) = t.duplicate(&v, &mut b).unwrap();
    b.reserve_storage(charge.full_storage()).unwrap();
    let mut moved = Box::new(std::mem::replace(&mut b, Budget::new(&mut other, LIMIT)));
    b.reserve_storage(moved.storage()).unwrap();
    PROBES.with(|v| v.set(0));
    for account in [&mut b, &mut *moved] {
        accounting(t.duplicate(&v, account));
        accounting(t.validate(&v, &file, account));
    }
    assert_eq!(PROBES.with(Cell::get), 0);
}

struct Usage {
    work: usize,
    peak: usize,
    failed_work: Option<usize>,
    failed_storage: Option<usize>,
}
fn bounded_operation(work: usize, storage: usize, validation: bool) -> (Result<()>, Usage) {
    let t = Tree::new(137);
    let mut work = Work::new(work);
    let mut b = Budget::new(&mut work, storage);
    b.reserve_storage(APPROVAL_STORAGE).unwrap();
    let v = t.retain(&mut b);
    let transfer = if validation {
        let (file, charge) = t.duplicate(&v, &mut b).unwrap();
        b.reserve_storage(charge.full_storage()).unwrap();
        Some(file)
    } else {
        None
    };
    // Make the measured operation's peak strictly exceed fixture setup's peak.
    b.reserve_storage(FRAME).unwrap();
    let before = b.storage();
    let ledger = b.work_ledger_identity_v1();
    let address = &b as *const Budget<'_> as usize;
    let result = match transfer {
        Some(ref file) => t.validate(&v, file, &mut b),
        None => t.duplicate(&v, &mut b).map(drop),
    };
    assert_eq!(b.storage(), before);
    assert_eq!(&b as *const Budget<'_> as usize, address);
    assert!(b.work_ledger_identity_v1() == ledger);
    (
        result,
        Usage {
            work: b.work(),
            peak: b.peak_storage(),
            failed_work: b.failed_work(),
            failed_storage: b.failed_storage(),
        },
    )
}

#[test]
fn transfer_and_final_validation_exact_and_one_short_budgets() {
    for validation in [false, true] {
        let (result, used) = bounded_operation(LIMIT, LIMIT, validation);
        result.unwrap();
        let (result, exact) = bounded_operation(used.work, used.peak, validation);
        result.unwrap();
        assert_eq!(exact.work, used.work);
        assert_eq!(exact.peak, used.peak);
        let (result, short) = bounded_operation(used.work - 1, used.peak, validation);
        assert!(result.is_err());
        assert!(short.failed_work.is_some());
        let (result, short) = bounded_operation(used.work, used.peak - 1, validation);
        assert!(result.is_err());
        assert!(short.failed_storage.is_some());
    }
}

#[test]
fn post_duplicate_unwind_preserves_original_account_and_work() {
    let t = Tree::new(64);
    let mut work = Work::new(LIMIT);
    let mut b = Budget::new(&mut work, LIMIT);
    b.reserve_storage(APPROVAL_STORAGE).unwrap();
    let v = t.retain(&mut b);
    let before = b.storage();
    let work_before = b.work();
    let (uid, gid) = owners();
    let mut calls = 0;
    let unwind = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let _ = v.clone_proof_executor_using(uid, gid, synthetic_immutable, &mut b, |b| {
            calls += 1;
            assert_ne!(calls, 2, "synthetic post-duplicate unwind");
            t.revalidate(&v, b)
        });
    }));
    assert!(unwind.is_err());
    assert_eq!(calls, 2);
    assert_eq!(b.storage(), before);
    assert!(b.work() > work_before);
    t.revalidate(&v, &mut b).unwrap();
}

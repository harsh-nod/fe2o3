//! Private synthetic inventory intake only. Ordinary fixture bytes are not ELF,
//! and the private immutability probe is not FS_IMMUTABLE evidence. These tests
//! never create production approval, RetainedCompilerRuntimeV1, or a guard.
use super::*;
use fe2o3_build_authority::{
    COMPILER_RUNTIME_MANIFEST_WORK_V1 as CODEC_WORK, CompilerRuntimeRoleV1 as Role,
};
use fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1 as Work;
use std::{
    cell::Cell,
    fs,
    os::unix::fs::{PermissionsExt, symlink},
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
    time::{Duration, UNIX_EPOCH},
};

const APPROVAL_STORAGE: usize = 4096;
const FLOOR: usize = APPROVAL_STORAGE + 19;
const LIMIT: usize = 32_000_000;
const PATHS: [&str; 6] = [
    "bin/helper",
    "bin/rustc",
    "lib/ld.so",
    "lib/libc.so",
    "lib/libfe2o3.so",
    "lib/libfe2o3_macros.so",
];
const ROLES: [Role; 6] = [
    Role::ProofExecutorHelper,
    Role::Rustc,
    Role::ElfInterpreter,
    Role::SharedLibrary,
    Role::CodegenBackend,
    Role::Fe2o3ProcMacro,
];
thread_local! { static PROBES: Cell<usize> = const { Cell::new(0) }; }
fn synthetic_immutable(_: &File) -> Result<()> {
    PROBES.with(|v| v.set(v.get() + 1));
    Ok(())
}
fn refused_immutable(_: &File) -> Result<()> {
    Err(mismatch("synthetic immutable probe refused"))
}
fn mutate_during_probe(file: &File) -> Result<()> {
    synthetic_immutable(file)?;
    if PROBES.with(Cell::get) == 2 {
        file.set_times(fs::FileTimes::new().set_modified(UNIX_EPOCH + Duration::from_secs(1)))
            .unwrap();
    }
    Ok(())
}
fn chmod(path: &Path, mode: u32) {
    fs::set_permissions(path, fs::Permissions::from_mode(mode)).unwrap();
}
fn owners() -> (u32, u32) {
    (
        rustix::process::getuid().as_raw(),
        rustix::process::getgid().as_raw(),
    )
}
fn policy(manifest: &CompilerRuntimeManifestV1) -> CompilerApprovalPolicyV2 {
    CompilerApprovalPolicyV2::new(
        manifest.compiler_closure(),
        [17; 32],
        *manifest.identity(),
        1,
        9603,
        9603,
        |_| Ok::<_, Resource>(()),
    )
    .unwrap()
}
struct Tree {
    root: PathBuf,
    manifest: CompilerRuntimeManifestV1,
    policy: CompilerApprovalPolicyV2,
}
impl Tree {
    fn new() -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let root = std::env::temp_dir().join(format!(
            "compiler-runtime-inventory-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&root).unwrap();
        chmod(&root, 0o700);
        for p in [
            "etc",
            "etc/fe2o3",
            "etc/fe2o3/build-authority",
            "opt",
            "opt/fe2o3",
            "opt/fe2o3/compiler-runtime-v1",
            "opt/fe2o3/compiler-runtime-v1/bin",
            "opt/fe2o3/compiler-runtime-v1/lib",
        ] {
            fs::create_dir(root.join(p)).unwrap();
            chmod(&root.join(p), 0o755);
        }
        let digests: [[u8; 32]; 6] =
            std::array::from_fn(|i| Sha256::digest([i as u8 + 1; 64]).into());
        let closure =
            CompilerClosureV2::new([1; 32], [2; 32], [3; 32], digests[1], [5; 32], digests[4])
                .unwrap();
        let entries: [_; 6] = std::array::from_fn(|i| CompilerRuntimeEntryV1 {
            role: ROLES[i],
            path: PATHS[i],
            length: 64,
            sha256: digests[i],
        });
        let manifest =
            CompilerRuntimeManifestV1::new(closure, [18; 32], &entries, |_| Ok::<_, Resource>(()))
                .unwrap();
        let tree = Self {
            root,
            policy: policy(&manifest),
            manifest,
        };
        for (i, entry) in tree.manifest.entries().enumerate() {
            tree.write(
                &tree.code(entry.path),
                &[i as u8 + 1; 64],
                entry.role.protected_mode(),
            );
        }
        tree.write(
            &tree.manifest_path(),
            tree.manifest.canonical_bytes(),
            0o444,
        );
        tree
    }
    fn fixed(&self, path: &str) -> PathBuf {
        self.root.join(path.strip_prefix('/').unwrap())
    }
    fn manifest_path(&self) -> PathBuf {
        self.fixed(COMPILER_RUNTIME_MANIFEST_PATH_V1)
    }
    fn runtime_path(&self) -> PathBuf {
        self.fixed(COMPILER_RUNTIME_ROOT_V1)
    }
    fn code(&self, path: &str) -> PathBuf {
        self.runtime_path().join(path)
    }
    fn write(&self, path: &Path, bytes: &[u8], mode: u32) {
        if fs::symlink_metadata(path).is_ok() {
            fs::remove_file(path).unwrap();
        }
        fs::write(path, bytes).unwrap();
        chmod(path, mode);
    }
    fn open(&self) -> Result<File> {
        rustix::fs::open(&self.root, tree::DIRECTORY_FLAGS, Mode::empty())
            .map(File::from)
            .map_err(|e| io("open synthetic root", e))
    }
    fn load(&self, b: &mut Budget<'_>) -> Result<Inventory> {
        let (uid, gid) = owners();
        Inventory::load_using(
            &self.policy,
            APPROVAL_STORAGE,
            || self.open(),
            uid,
            gid,
            synthetic_immutable,
            b,
        )
    }
    fn retain(&self, b: &mut Budget<'_>) -> Inventory {
        let before = b.storage();
        let v = self.load(b).unwrap();
        assert_eq!(b.storage(), before);
        b.reserve_storage(v.additional_storage()).unwrap();
        v
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
}
impl Drop for Tree {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.root).unwrap();
    }
}
fn failure<T>(r: Result<T>) -> RetainedCompilerRuntimeErrorV1 {
    match r {
        Ok(_) => panic!("expected synthetic refusal"),
        Err(e) => e,
    }
}
fn is_resource<T>(r: Result<T>) -> bool {
    matches!(
        failure(r),
        RetainedCompilerRuntimeErrorV1::Resource(_)
            | RetainedCompilerRuntimeErrorV1::Codec(CompilerRuntimeManifestErrorV1::Charge(_))
    )
}
#[derive(Debug)]
struct Usage {
    work: usize,
    peak: usize,
    storage: usize,
    failed_work: Option<usize>,
    failed_storage: Option<usize>,
}
fn observe<T>(
    work: usize,
    storage: usize,
    f: impl FnOnce(&mut Budget<'_>) -> Result<T>,
) -> (Result<T>, Usage) {
    let mut w = Work::new(work);
    let mut b = Budget::new(&mut w, storage);
    b.reserve_storage(FLOOR).unwrap();
    let ledger = b.work_ledger_identity_v1();
    let address = &b as *const Budget<'_> as usize;
    let r = f(&mut b);
    assert!(ledger == b.work_ledger_identity_v1());
    assert_eq!(address, &b as *const Budget<'_> as usize);
    (
        r,
        Usage {
            work: b.work(),
            peak: b.peak_storage(),
            storage: b.storage(),
            failed_work: b.failed_work(),
            failed_storage: b.failed_storage(),
        },
    )
}
fn load_work() -> usize {
    IO_WORK + 2 * CODEC_WORK + 512 + MAX_BYTES + 12 * ENTRY_IO_WORK + 6 * 64 * 8
}
fn revalidate_work() -> usize {
    8 + IO_WORK + CODEC_WORK + 512 + MAX_BYTES + 12 * ENTRY_IO_WORK + 6 * 64 * 8
}

#[test]
fn shared_runtime_quotes_preserve_the_closed_schedule() {
    let code = usize::try_from(MAX_CODE_BYTES).unwrap();
    let files = MAX_ENTRIES * ENTRY_IO_WORK + code * 8;
    assert_eq!(
        RetainedCompilerRuntimeV1::maximum_file_pass_work().unwrap(),
        files
    );
    assert_eq!(
        RetainedCompilerRuntimeV1::maximum_operation_work().unwrap(),
        3 * ApprovedCompilerPolicyV2::MAX_OPERATION_WORK
            + IO_WORK
            + 2048
            + 2 * CODEC_WORK
            + MAX_BYTES
            + MAX_ENTRIES * ENTRY_IO_WORK
            + files
    );
    assert!(RetainedCompilerRuntimeV1::MAX_OPERATION_SCRATCH >= FRAME);
    assert_eq!(
        RetainedCompilerRuntimeV1::maximum_additional_storage().unwrap(),
        size_of::<RetainedCompilerRuntimeV1>() - size_of::<ApprovedCompilerPolicyV2>()
            + size_of::<RetainedCompilerRuntimeStorageV1>()
            + MAX_BYTES
            + code
    );
}

#[test]
fn shared_runtime_work_quote_covers_synthetic_inventory_operations() {
    let t = Tree::new();
    let limit = 2 * RetainedCompilerRuntimeV1::maximum_operation_work().unwrap();
    let (result, usage) = observe(limit, LIMIT, |b| {
        let inventory = t.retain(b);
        t.revalidate(&inventory, b)?;
        let retained = inventory.additional_storage();
        assert!(retained <= RetainedCompilerRuntimeV1::maximum_additional_storage()?);
        drop(inventory);
        b.release_storage(retained)?;
        Ok(())
    });
    result.unwrap();
    assert_eq!(usage.work, load_work() + revalidate_work());
    assert_eq!(usage.storage, FLOOR);
    assert_eq!(usage.failed_work, None);
    assert_eq!(usage.failed_storage, None);
}

#[test]
fn synthetic_intake_binds_full_policy_and_retains_every_exact_origin() {
    let t = Tree::new();
    let mut w = Work::new(LIMIT);
    let mut b = Budget::new(&mut w, LIMIT);
    b.reserve_storage(FLOOR).unwrap();
    let v = t.retain(&mut b);
    assert_eq!(v.manifest, t.manifest);
    assert_eq!(v.approval_identity, *t.policy.identity());
    assert_eq!(v.address, &b as *const Budget<'_> as usize);
    assert!(v.ledger == b.work_ledger_identity_v1());
    assert_eq!(b.work(), load_work());
    for (entry, retained) in v.manifest.entries().zip(&v.files) {
        let retained = retained.as_ref().unwrap();
        let m = fs::metadata(t.code(entry.path)).unwrap();
        assert_eq!(
            (retained.snapshot.dev, retained.snapshot.ino),
            (m.dev(), m.ino())
        );
        rustix::fs::seek(&retained.file, rustix::fs::SeekFrom::Start(11)).unwrap();
    }
    let live = b.storage();
    t.revalidate(&v, &mut b).unwrap();
    assert_eq!(b.work(), load_work() + revalidate_work());
    assert_eq!(b.storage(), live);
    for retained in v.files.iter().flatten() {
        assert_eq!(
            rustix::fs::seek(&retained.file, rustix::fs::SeekFrom::Current(0)).unwrap(),
            11
        );
    }
    assert!(!v.manifest.grants_authority());
    let charge = v.additional_storage();
    drop(v);
    b.release_storage(charge).unwrap();
    assert_eq!(b.storage(), FLOOR);
}

#[test]
fn retained_charge_includes_original_approval_full_capacity_and_all_file_backing() {
    let t = Tree::new();
    let (result, used) = observe(LIMIT, LIMIT, |b| t.load(b));
    let v = result.unwrap();
    let extra = size_of::<RetainedCompilerRuntimeV1>() - size_of::<ApprovedCompilerPolicyV2>()
        + size_of::<RetainedCompilerRuntimeStorageV1>()
        + t.manifest.canonical_bytes().len()
        + 6 * 64;
    assert_eq!(v.additional_storage(), extra);
    assert_eq!(v.required_storage(), APPROVAL_STORAGE + extra);
    assert_eq!(used.peak, FLOOR + FRAME + extra);
    assert_eq!(used.storage, FLOOR);
    assert_eq!(used.work, load_work());
}

#[test]
fn synthetic_exact_and_short_constructor_budgets_refund_only_storage() {
    let t = Tree::new();
    let (r, u) = observe(LIMIT, LIMIT, |b| t.load(b));
    drop(r.unwrap());
    for (work, storage, ok) in [
        (u.work, u.peak, true),
        (u.work - 1, u.peak, false),
        (u.work, u.peak - 1, false),
    ] {
        let (r, actual) = observe(work, storage, |b| t.load(b));
        assert_eq!(actual.storage, FLOOR);
        if ok {
            r.unwrap();
            assert_eq!((actual.work, actual.peak), (u.work, u.peak));
        } else {
            assert!(is_resource(r));
            assert!(actual.failed_work.is_some() || actual.failed_storage.is_some());
        }
    }
}

#[test]
fn constructor_prepayment_and_input_floor_precede_root_open() {
    for (work, storage, called) in [
        (7, LIMIT, false),
        (IO_WORK - 1, LIMIT, false),
        (IO_WORK, FLOOR + FRAME - 1, false),
        (IO_WORK, FLOOR + FRAME, true),
    ] {
        let seen = Cell::new(false);
        let p = Tree::new().policy;
        let (r, u) = observe(work, storage, |b| {
            Inventory::load_using(
                &p,
                APPROVAL_STORAGE,
                || {
                    seen.set(true);
                    Err(mismatch("synthetic root refused"))
                },
                0,
                0,
                synthetic_immutable,
                b,
            )
        });
        assert_eq!(seen.get(), called);
        assert_eq!(u.storage, FLOOR);
        if called {
            assert!(matches!(
                failure(r),
                RetainedCompilerRuntimeErrorV1::Mismatch("synthetic root refused")
            ));
        } else {
            assert!(is_resource(r));
        }
    }
    let t = Tree::new();
    let mut w = Work::new(LIMIT);
    let mut b = Budget::new(&mut w, LIMIT);
    assert!(matches!(
        failure(t.load(&mut b)),
        RetainedCompilerRuntimeErrorV1::Resource(Resource::Accounting)
    ));
    assert_eq!(b.work(), 8);
    assert_eq!(b.storage(), 0);
}

#[test]
fn synthetic_revalidation_exact_and_short_work_storage_preserve_live_account() {
    let t = Tree::new();
    let (r, used) = observe(LIMIT, LIMIT, |b| {
        let v = t.retain(b);
        // Reach the revalidation high-water mark independently of construction.
        b.reserve_storage(FRAME).unwrap();
        t.revalidate(&v, b)
    });
    r.unwrap();
    for (work, storage, ok) in [
        (load_work() + revalidate_work(), used.peak, true),
        (load_work() + revalidate_work() - 1, used.peak, false),
        (load_work() + revalidate_work(), used.peak - 1, false),
    ] {
        let (r, u) = observe(work, storage, |b| {
            let v = t.retain(b);
            b.reserve_storage(FRAME).unwrap();
            let before = b.storage();
            let r = t.revalidate(&v, b);
            assert_eq!(b.storage(), before);
            r
        });
        if ok {
            r.unwrap();
            assert_eq!(u.work, work);
        } else {
            assert!(is_resource(r));
        }
    }
}

#[test]
fn wrong_ledger_moved_budget_and_short_live_reservation_refuse() {
    let t = Tree::new();
    let mut w = Work::new(LIMIT);
    let mut other = Work::new(LIMIT);
    let mut b = Budget::new(&mut w, LIMIT);
    b.reserve_storage(FLOOR).unwrap();
    let v = t.retain(&mut b);
    let mut moved = Box::new(std::mem::replace(&mut b, Budget::new(&mut other, LIMIT)));
    b.reserve_storage(moved.storage()).unwrap();
    for account in [&mut b, &mut *moved] {
        assert!(matches!(
            failure(t.revalidate(&v, account)),
            RetainedCompilerRuntimeErrorV1::Resource(Resource::Accounting)
        ));
    }
    let t = Tree::new();
    let mut w = Work::new(LIMIT);
    let mut b = Budget::new(&mut w, LIMIT);
    b.reserve_storage(FLOOR).unwrap();
    let v = t.retain(&mut b);
    b.release_storage(b.storage() - v.required_storage() + 1)
        .unwrap();
    assert!(matches!(
        failure(t.revalidate(&v, &mut b)),
        RetainedCompilerRuntimeErrorV1::Resource(Resource::Accounting)
    ));
}

#[test]
fn independently_approved_digest_and_all_six_pins_are_required() {
    let mut t = Tree::new();
    let original = t.policy;
    t.policy = CompilerApprovalPolicyV2::new(
        original.compiler_closure(),
        [17; 32],
        [99; 32],
        1,
        9603,
        9603,
        |_| Ok::<_, Resource>(()),
    )
    .unwrap();
    assert!(matches!(
        failure(observe(LIMIT, LIMIT, |b| t.load(b)).0),
        RetainedCompilerRuntimeErrorV1::Mismatch(_)
    ));
    let c = original.compiler_closure();
    let pins = [
        c.cargo_executable_sha256(),
        c.cargo_binding_trampoline_sha256(),
        c.cargo_fe2o3_binding_wrapper_sha256(),
        c.rustc_executable_sha256(),
        c.rustc_runtime_tree_sha256(),
        c.codegen_backend_sha256(),
    ];
    for i in 0..6 {
        let mut changed = pins;
        changed[i][0] ^= 1;
        let c = CompilerClosureV2::new(
            changed[0], changed[1], changed[2], changed[3], changed[4], changed[5],
        )
        .unwrap();
        t.policy = CompilerApprovalPolicyV2::new(
            c,
            [17; 32],
            *original.runtime_manifest_identity(),
            1,
            9603,
            9603,
            |_| Ok::<_, Resource>(()),
        )
        .unwrap();
        assert!(
            matches!(
                failure(observe(LIMIT, LIMIT, |b| t.load(b)).0),
                RetainedCompilerRuntimeErrorV1::Mismatch(_)
            ),
            "pin {i}"
        );
    }
}

#[test]
fn coherently_resealed_manifest_cannot_replace_approved_inventory() {
    let t = Tree::new();
    let mut entries: Vec<_> = t.manifest.entries().collect();
    entries[5].sha256 = [33; 32];
    let substitute =
        CompilerRuntimeManifestV1::new(t.manifest.compiler_closure(), [18; 32], &entries, |_| {
            Ok::<_, Resource>(())
        })
        .unwrap();
    t.write(&t.manifest_path(), substitute.canonical_bytes(), 0o444);
    assert!(matches!(
        failure(observe(LIMIT, LIMIT, |b| t.load(b)).0),
        RetainedCompilerRuntimeErrorV1::Mismatch(
            "inventory differs from root-approved digest or full compiler closure"
        )
    ));
}

#[test]
fn every_code_role_requires_exact_bytes_length_and_protected_mode() {
    for i in 0..6 {
        for attack in 0..4 {
            let t = Tree::new();
            let p = t.code(PATHS[i]);
            match attack {
                0 => t.write(&p, &[99; 64], ROLES[i].protected_mode()),
                1 => t.write(&p, &[i as u8 + 1; 63], ROLES[i].protected_mode()),
                2 => chmod(&p, 0o644),
                _ => {
                    fs::hard_link(&p, p.with_extension("alias")).unwrap();
                }
            }
            let (r, u) = observe(LIMIT, LIMIT, |b| t.load(b));
            assert!(r.is_err(), "role {i}, attack {attack}");
            assert_eq!(u.storage, FLOOR);
        }
    }
}

#[test]
fn missing_entries_symlinks_and_untrusted_directories_refuse() {
    for i in 0..6 {
        let t = Tree::new();
        fs::remove_file(t.code(PATHS[i])).unwrap();
        assert!(observe(LIMIT, LIMIT, |b| t.load(b)).0.is_err());
        let t = Tree::new();
        let p = t.code(PATHS[i]);
        let other = p.with_extension("other");
        fs::rename(&p, &other).unwrap();
        symlink(&other, &p).unwrap();
        assert!(observe(LIMIT, LIMIT, |b| t.load(b)).0.is_err());
    }
    for p in [
        "etc",
        "etc/fe2o3/build-authority",
        "opt",
        "opt/fe2o3",
        "opt/fe2o3/compiler-runtime-v1",
        "opt/fe2o3/compiler-runtime-v1/lib",
    ] {
        let t = Tree::new();
        chmod(&t.root.join(p), 0o777);
        assert!(observe(LIMIT, LIMIT, |b| t.load(b)).0.is_err());
        let t = Tree::new();
        let original = t.root.join(p);
        let moved = original.with_extension("moved");
        fs::rename(&original, &moved).unwrap();
        symlink(&moved, &original).unwrap();
        assert!(observe(LIMIT, LIMIT, |b| t.load(b)).0.is_err());
    }
}

#[test]
fn immutable_prerequisite_is_checked_before_decode_and_after_measurement() {
    let t = Tree::new();
    t.write(&t.manifest_path(), &[0; 512], 0o444);
    let (uid, gid) = owners();
    let (r, u) = observe(LIMIT, LIMIT, |b| {
        Inventory::load_using(
            &t.policy,
            APPROVAL_STORAGE,
            || t.open(),
            uid,
            gid,
            refused_immutable,
            b,
        )
    });
    assert!(matches!(
        failure(r),
        RetainedCompilerRuntimeErrorV1::Mismatch("synthetic immutable probe refused")
    ));
    assert_eq!(u.work, IO_WORK);
    assert_eq!(u.storage, FLOOR);
    let t = Tree::new();
    PROBES.with(|v| v.set(0));
    assert!(
        observe(LIMIT, LIMIT, |b| Inventory::load_using(
            &t.policy,
            APPROVAL_STORAGE,
            || t.open(),
            uid,
            gid,
            mutate_during_probe,
            b
        ))
        .0
        .is_err()
    );
    let mut w = Work::new(LIMIT);
    let mut b = Budget::new(&mut w, LIMIT);
    b.reserve_storage(FLOOR).unwrap();
    let v = t.retain(&mut b);
    assert!(
        v.revalidate_using(&t.policy, || t.open(), uid, gid, refused_immutable, &mut b)
            .is_err()
    );
}

#[test]
fn inode_replacement_and_retained_origin_drift_refuse_even_with_identical_bytes() {
    for target in 0..7 {
        let t = Tree::new();
        let mut w = Work::new(LIMIT);
        let mut b = Budget::new(&mut w, LIMIT);
        b.reserve_storage(FLOOR).unwrap();
        let v = t.retain(&mut b);
        let p = if target == 6 {
            t.manifest_path()
        } else {
            t.code(PATHS[target])
        };
        let bytes = fs::read(&p).unwrap();
        let mode = fs::metadata(&p).unwrap().mode() & 0o7777;
        fs::rename(&p, p.with_extension("retained")).unwrap();
        t.write(&p, &bytes, mode);
        let live = b.storage();
        assert!(t.revalidate(&v, &mut b).is_err());
        assert_eq!(b.storage(), live);
    }
    let t = Tree::new();
    let mut w = Work::new(LIMIT);
    let mut b = Budget::new(&mut w, LIMIT);
    b.reserve_storage(FLOOR).unwrap();
    let v = t.retain(&mut b);
    let p = t.runtime_path();
    fs::rename(&p, p.with_extension("moved")).unwrap();
    fs::create_dir(&p).unwrap();
    chmod(&p, 0o755);
    assert!(t.revalidate(&v, &mut b).is_err());
}

#[test]
fn retained_writer_and_policy_rotation_refuse_without_mutating_cached_manifest() {
    let t = Tree::new();
    let p = t.code(PATHS[5]);
    chmod(&p, 0o644);
    let writer = fs::OpenOptions::new().write(true).open(&p).unwrap();
    chmod(&p, 0o444);
    let mut w = Work::new(LIMIT);
    let mut b = Budget::new(&mut w, LIMIT);
    b.reserve_storage(FLOOR).unwrap();
    let v = t.retain(&mut b);
    rustix::io::pwrite(&writer, &[0], 31).unwrap();
    assert!(t.revalidate(&v, &mut b).is_err());
    assert_eq!(v.manifest, t.manifest);
    let t = Tree::new();
    let mut w = Work::new(LIMIT);
    let mut b = Budget::new(&mut w, LIMIT);
    b.reserve_storage(FLOOR).unwrap();
    let v = t.retain(&mut b);
    let rotated = CompilerApprovalPolicyV2::new(
        t.policy.compiler_closure(),
        [88; 32],
        *t.policy.runtime_manifest_identity(),
        1,
        9603,
        9603,
        |_| Ok::<_, Resource>(()),
    )
    .unwrap();
    let (uid, gid) = owners();
    assert!(matches!(
        failure(v.revalidate_using(&rotated, || t.open(), uid, gid, synthetic_immutable, &mut b)),
        RetainedCompilerRuntimeErrorV1::Mismatch("approved policy rotated")
    ));
}

#[test]
fn synthetic_owner_gid_manifest_mode_length_and_hardlink_checks_are_closed() {
    let t = Tree::new();
    let (uid, gid) = owners();
    for (expected_uid, expected_gid) in [(uid.wrapping_add(1), gid), (uid, gid.wrapping_add(1))] {
        assert!(
            observe(LIMIT, LIMIT, |b| Inventory::load_using(
                &t.policy,
                APPROVAL_STORAGE,
                || t.open(),
                expected_uid,
                expected_gid,
                synthetic_immutable,
                b,
            ))
            .0
            .is_err()
        );
    }
    for attack in 0..4 {
        let t = Tree::new();
        let path = t.manifest_path();
        match attack {
            0 => chmod(&path, 0o644),
            1 => t.write(&path, &[0; MAX_BYTES + 1], 0o444),
            2 => {
                fs::hard_link(&path, path.with_extension("alias")).unwrap();
            }
            _ => {
                let moved = path.with_extension("moved");
                fs::rename(&path, &moved).unwrap();
                symlink(&moved, &path).unwrap();
            }
        }
        assert!(observe(LIMIT, LIMIT, |b| t.load(b)).0.is_err());
    }
}

#[test]
fn actual_code_acl_is_rejected_when_the_fixture_filesystem_supports_it() {
    for index in [1, 5] {
        let t = Tree::new();
        let path = t.code(PATHS[index]);
        let permissions = if ROLES[index].protected_mode() == 0o555 {
            5_u16
        } else {
            4_u16
        };
        let mut acl = 2_u32.to_le_bytes().to_vec();
        // UID + 1 may be unmapped. Naming the mapped owner with ACL_USER still
        // requires an extended ACL, even though ACL_USER_OBJ names that owner too.
        for (tag, value, id) in [
            (1_u16, permissions, u32::MAX),
            (2, 4, owners().0),
            (4, permissions, u32::MAX),
            (16, permissions, u32::MAX),
            (32, permissions, u32::MAX),
        ] {
            acl.extend_from_slice(&tag.to_le_bytes());
            acl.extend_from_slice(&value.to_le_bytes());
            acl.extend_from_slice(&id.to_le_bytes());
        }
        match rustix::fs::setxattr(
            &path,
            "system.posix_acl_access",
            &acl,
            rustix::fs::XattrFlags::empty(),
        ) {
            Ok(()) => {}
            Err(rustix::io::Errno::OPNOTSUPP) => {
                eprintln!("synthetic filesystem lacks ACL support; no code-ACL coverage claimed");
                continue;
            }
            Err(e) => panic!("set fixture code ACL: {e}"),
        }
        let mut actual = [0_u8; 44];
        let length = rustix::fs::getxattr(&path, "system.posix_acl_access", &mut actual).unwrap();
        assert_eq!(&actual[..length], acl.as_slice());
        assert_eq!(
            fs::metadata(&path).unwrap().mode() & 0o7777,
            ROLES[index].protected_mode()
        );
        assert!(matches!(
            failure(observe(LIMIT, LIMIT, |b| t.load(b)).0),
            RetainedCompilerRuntimeErrorV1::Mismatch("code file has a forbidden xattr")
        ));
        rustix::fs::removexattr(&path, "system.posix_acl_access").unwrap();
        assert!(observe(LIMIT, LIMIT, |b| t.load(b)).0.is_ok());
    }
}

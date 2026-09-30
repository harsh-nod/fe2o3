//! Synthetic private inventory mechanics, never a fabricated approved runtime.
use super::super::inventory::{INVENTORY_SCRATCH, INVENTORY_WORK, inventory_transfer_storage};
use super::*;
use std::panic::{AssertUnwindSafe, catch_unwind};

type Transfer = RetainedCompilerRuntimeInventoryTransferV1;
const WORK_LIMIT: usize = 1_000_000_000;

fn duplicate(
    t: &Tree,
    v: &Inventory,
    b: &mut Budget<'_>,
) -> Result<(Transfer, RetainedCompilerRuntimeExecTransferChargeV1)> {
    let (uid, gid) = owners();
    v.clone_inventory_using(uid, gid, synthetic_immutable, b, |b| t.revalidate(v, b))
}

fn validate(t: &Tree, v: &Inventory, transfer: &Transfer, b: &mut Budget<'_>) -> Result<()> {
    let (uid, gid) = owners();
    v.validate_inventory_using(transfer, uid, gid, synthetic_immutable, b, |b| {
        t.revalidate(v, b)
    })
}

fn refs(file: &File) -> usize {
    let original = file.metadata().unwrap();
    fs::read_dir("/proc/self/fd")
        .unwrap()
        .filter_map(|entry| fs::metadata(entry.ok()?.path()).ok())
        .filter(|m| (m.dev(), m.ino()) == (original.dev(), original.ino()))
        .count()
}

fn all_refs(v: &Inventory, expected: usize) {
    for file in v.files.iter().flatten() {
        assert_eq!(refs(&file.file), expected);
    }
}

fn maximum_tree() -> Tree {
    let mut t = Tree::new(64);
    let manifest = CompilerRuntimeManifestV1::decode(&fs::read(t.manifest_path()).unwrap(), |_| {
        Ok::<_, Resource>(())
    })
    .unwrap();
    let paths: Vec<_> = (6..MAX_ENTRIES)
        .map(|i| format!("lib/z-shared-{i:03}.so"))
        .collect();
    let mut entries: Vec<_> = manifest.entries().collect();
    for path in &paths {
        fs::write(t.code(path), [9; 17]).unwrap();
        chmod(&t.code(path), 0o444);
        entries.push(CompilerRuntimeEntryV1 {
            role: CompilerRuntimeRoleV1::SharedLibrary,
            path,
            length: 17,
            sha256: Sha256::digest([9; 17]).into(),
        });
    }
    let manifest =
        CompilerRuntimeManifestV1::new(manifest.compiler_closure(), [18; 32], &entries, |_| {
            Ok::<_, Resource>(())
        })
        .unwrap();
    t.policy = CompilerApprovalPolicyV2::new(
        manifest.compiler_closure(),
        [17; 32],
        *manifest.identity(),
        1,
        9603,
        9603,
        |_| Ok::<_, Resource>(()),
    )
    .unwrap();
    chmod(&t.manifest_path(), 0o644);
    fs::write(t.manifest_path(), manifest.canonical_bytes()).unwrap();
    chmod(&t.manifest_path(), 0o444);
    t
}

#[test]
fn complete_transfer_includes_every_shared_library_and_full_overlap() {
    for t in [
        Tree::with_lengths([137, CHUNK + 1, 67, 71, 73, 79]),
        maximum_tree(),
    ] {
        let mut work = Work::new(WORK_LIMIT);
        let mut b = Budget::new(&mut work, LIMIT);
        b.reserve_storage(APPROVAL_STORAGE).unwrap();
        let v = t.retain(&mut b);
        let floor = b.storage();
        let ledger = b.work_ledger_identity_v1();
        let (transfer, charge) = duplicate(&t, &v, &mut b).unwrap();
        assert_eq!(b.storage(), floor);
        assert_eq!(
            charge.full_storage(),
            size_of::<(Transfer, RetainedCompilerRuntimeExecTransferChargeV1)>()
                + v.manifest.total_file_bytes() as usize
        );
        assert_eq!(transfer.retained_storage(), charge.full_storage());
        assert_eq!(transfer.entries().len(), v.manifest.entries().len());
        let mut compiler = 0;
        for ((entry, file), original) in transfer.entries().zip(v.files.iter().flatten()) {
            assert_eq!(Snapshot::read(file).unwrap(), original.snapshot);
            assert_eq!(
                rustix::io::fcntl_getfd(file).unwrap(),
                rustix::io::FdFlags::CLOEXEC
            );
            assert_eq!(
                rustix::fs::fcntl_getfl(file).unwrap() & OFlags::ACCMODE,
                OFlags::RDONLY
            );
            if !matches!(
                entry.role,
                CompilerRuntimeRoleV1::SharedLibrary | CompilerRuntimeRoleV1::ProofExecutorHelper
            ) {
                compiler += size_of::<(File, RetainedCompilerRuntimeExecTransferChargeV1)>()
                    + entry.length as usize;
            }
        }
        assert_eq!(transfer.compiler_staging_storage(), compiler);
        for (file, role) in [
            (transfer.rustc_source(), CompilerRuntimeRoleV1::Rustc),
            (
                transfer.elf_interpreter_source(),
                CompilerRuntimeRoleV1::ElfInterpreter,
            ),
            (
                transfer.codegen_backend_source(),
                CompilerRuntimeRoleV1::CodegenBackend,
            ),
            (
                transfer.fe2o3_proc_macro_source(),
                CompilerRuntimeRoleV1::Fe2o3ProcMacro,
            ),
        ] {
            let expected = transfer
                .entries()
                .find(|(entry, _)| entry.role == role)
                .unwrap()
                .1;
            assert!(std::ptr::eq(file, expected));
        }
        all_refs(&v, 2);
        b.reserve_storage(charge.full_storage()).unwrap();
        validate(&t, &v, &transfer, &mut b).unwrap();
        let state = (b.work(), b.storage(), b.failed_work(), b.failed_storage());
        drop(transfer);
        all_refs(&v, 1);
        assert_eq!(
            (b.work(), b.storage(), b.failed_work(), b.failed_storage()),
            state
        );
        assert!(b.work_ledger_identity_v1() == ledger);
        b.release_storage(charge.full_storage()).unwrap();
        t.revalidate(&v, &mut b).unwrap();
        assert_eq!(b.storage(), floor);
    }
}

#[test]
fn incomplete_swapped_same_bytes_and_changed_shared_descriptors_refuse() {
    for fault in 0..5 {
        let t = Tree::new(64);
        let mut work = Work::new(WORK_LIMIT);
        let mut b = Budget::new(&mut work, LIMIT);
        b.reserve_storage(APPROVAL_STORAGE).unwrap();
        let v = t.retain(&mut b);
        let (mut transfer, charge) = duplicate(&t, &v, &mut b).unwrap();
        b.reserve_storage(charge.full_storage()).unwrap();
        match fault {
            0 => {
                transfer.files[3] = None;
            }
            1 => transfer.files.swap(0, 3),
            2 => {
                let copy = t.root.join("different-inode.so");
                fs::write(&copy, [4; 64]).unwrap();
                chmod(&copy, 0o444);
                transfer.files[3].as_mut().unwrap().0 = File::open(copy).unwrap();
            }
            3 => rustix::io::fcntl_setfd(
                &transfer.files[3].as_ref().unwrap().0,
                rustix::io::FdFlags::empty(),
            )
            .unwrap(),
            _ => transfer.singleton[TransferRole::Rustc.index()] = 3,
        }
        let floor = b.storage();
        assert!(validate(&t, &v, &transfer, &mut b).is_err());
        assert_eq!(b.storage(), floor);
        drop(transfer);
        all_refs(&v, 1);
        t.revalidate(&v, &mut b).unwrap();
    }
}

#[test]
fn missing_inventory_entry_refuses_before_origin_checks() {
    let t = Tree::new(64);
    let mut work = Work::new(WORK_LIMIT);
    let mut b = Budget::new(&mut work, LIMIT);
    b.reserve_storage(APPROVAL_STORAGE).unwrap();
    let mut v = t.retain(&mut b);
    v.files[3] = None;
    PROBES.with(|p| p.set(0));
    assert!(duplicate(&t, &v, &mut b).is_err());
    assert_eq!(PROBES.with(Cell::get), 0);
}

#[test]
fn complete_transfer_requires_both_floors_and_original_account_address() {
    let t = Tree::new(64);
    let mut work = Work::new(WORK_LIMIT);
    let mut other = Work::new(WORK_LIMIT);
    let mut b = Budget::new(&mut work, LIMIT);
    b.reserve_storage(APPROVAL_STORAGE).unwrap();
    let v = t.retain(&mut b);
    let (transfer, charge) = duplicate(&t, &v, &mut b).unwrap();
    PROBES.with(|p| p.set(0));
    accounting(validate(&t, &v, &transfer, &mut b));
    b.reserve_storage(charge.full_storage() - 1).unwrap();
    accounting(validate(&t, &v, &transfer, &mut b));
    assert_eq!(PROBES.with(Cell::get), 0);
    b.reserve_storage(1).unwrap();
    validate(&t, &v, &transfer, &mut b).unwrap();
    let mut moved = Box::new(std::mem::replace(&mut b, Budget::new(&mut other, LIMIT)));
    b.reserve_storage(moved.storage()).unwrap();
    PROBES.with(|p| p.set(0));
    for account in [&mut b, &mut *moved] {
        accounting(duplicate(&t, &v, account));
        accounting(validate(&t, &v, &transfer, account));
    }
    assert_eq!(PROBES.with(Cell::get), 0);
    drop(transfer);
    all_refs(&v, 1);
}

#[test]
fn equal_manifest_from_another_inventory_cannot_validate_the_actual_files() {
    let first = Tree::new(64);
    let second = Tree::new(64);
    let mut work = Work::new(WORK_LIMIT);
    let mut b = Budget::new(&mut work, LIMIT);
    b.reserve_storage(APPROVAL_STORAGE).unwrap();
    let v = first.retain(&mut b);
    let (transfer, charge) = duplicate(&first, &v, &mut b).unwrap();
    b.reserve_storage(charge.full_storage() + APPROVAL_STORAGE)
        .unwrap();
    let other = second.retain(&mut b);
    assert_eq!(v.manifest, other.manifest);
    let floor = b.storage();
    assert!(validate(&second, &other, &transfer, &mut b).is_err());
    assert_eq!(b.storage(), floor);
    validate(&first, &v, &transfer, &mut b).unwrap();
    drop(transfer);
    all_refs(&v, 1);
    all_refs(&other, 1);
}

#[test]
fn entry_work_and_complete_constructor_storage_refuse_before_origin_io() {
    for short_work in [false, true] {
        let t = Tree::new(64);
        let mut work = Work::new(WORK_LIMIT);
        let mut b = Budget::new(&mut work, LIMIT);
        b.reserve_storage(APPROVAL_STORAGE).unwrap();
        let v = t.retain(&mut b);
        let charge = inventory_transfer_storage(v.manifest.total_file_bytes() as usize).unwrap();
        if short_work {
            b.charge_work(WORK_LIMIT - b.work() - (8 + INVENTORY_WORK - 1))
                .unwrap();
        } else {
            let scratch = INVENTORY_SCRATCH + charge.full_storage();
            b.reserve_storage(LIMIT - b.storage() - scratch + 1)
                .unwrap();
        }
        let floor = b.storage();
        PROBES.with(|p| p.set(0));
        assert!(duplicate(&t, &v, &mut b).is_err());
        assert_eq!(b.storage(), floor);
        assert_eq!(PROBES.with(Cell::get), 0);
        if short_work {
            assert!(b.failed_work().is_some());
        } else {
            assert!(b.failed_storage().is_some());
        }
        all_refs(&v, 1);
    }
}

fn bounded(work: usize, storage: usize, validation: bool) -> (Result<()>, Usage) {
    let t = Tree::new(137);
    let mut work = Work::new(work);
    let mut b = Budget::new(&mut work, storage);
    b.reserve_storage(APPROVAL_STORAGE).unwrap();
    let v = t.retain(&mut b);
    let transfer = if validation {
        let (transfer, charge) = duplicate(&t, &v, &mut b).unwrap();
        b.reserve_storage(charge.full_storage()).unwrap();
        Some(transfer)
    } else {
        None
    };
    // Ensure this operation, not fixture construction, determines the peak.
    b.reserve_storage(FRAME + INVENTORY_SCRATCH).unwrap();
    let floor = b.storage();
    let address = &b as *const Budget<'_> as usize;
    let ledger = b.work_ledger_identity_v1();
    let result = match &transfer {
        Some(transfer) => validate(&t, &v, transfer, &mut b),
        None => duplicate(&t, &v, &mut b).map(drop),
    };
    assert_eq!(b.storage(), floor);
    assert_eq!(&b as *const Budget<'_> as usize, address);
    assert!(b.work_ledger_identity_v1() == ledger);
    drop(transfer);
    all_refs(&v, 1);
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
fn complete_transfer_and_revalidation_exact_and_one_short_work_storage() {
    for validation in [false, true] {
        let (result, used) = bounded(WORK_LIMIT, LIMIT, validation);
        result.unwrap();
        let (result, exact) = bounded(used.work, used.peak, validation);
        result.unwrap();
        assert_eq!((exact.work, exact.peak), (used.work, used.peak));
        let (result, short) = bounded(used.work - 1, used.peak, validation);
        assert!(result.is_err());
        assert!(short.failed_work.is_some());
        let (result, short) = bounded(used.work, used.peak - 1, validation);
        assert!(result.is_err());
        assert!(short.failed_storage.is_some());
    }
}

thread_local! {
    static DUPLICATE_PROBES: Cell<usize> = const { Cell::new(0) };
    static PANIC: Cell<bool> = const { Cell::new(false) };
}
fn partial_failure(_: &File) -> Result<()> {
    let n = DUPLICATE_PROBES.with(|p| {
        let n = p.get() + 1;
        p.set(n);
        n
    });
    if n == 5 {
        assert!(!PANIC.with(Cell::get), "partial transfer unwind");
        return Err(mismatch("partial transfer refusal"));
    }
    Ok(())
}

#[test]
fn partial_duplicate_error_and_unwind_close_only_new_files_without_refund() {
    for panic in [false, true] {
        let t = Tree::new(64);
        let mut work = Work::new(WORK_LIMIT);
        let mut b = Budget::new(&mut work, LIMIT);
        b.reserve_storage(APPROVAL_STORAGE).unwrap();
        let v = t.retain(&mut b);
        let floor = b.storage();
        let prefix = b.work();
        assert!(b.charge_work(WORK_LIMIT).is_err());
        assert!(b.reserve_storage(LIMIT).is_err());
        let denials = (b.failed_work(), b.failed_storage());
        DUPLICATE_PROBES.with(|p| p.set(0));
        PANIC.with(|p| p.set(panic));
        let (uid, gid) = owners();
        let result = catch_unwind(AssertUnwindSafe(|| {
            v.clone_inventory_using(uid, gid, partial_failure, &mut b, |b| t.revalidate(&v, b))
        }));
        if panic {
            assert!(result.is_err());
        } else {
            assert!(result.unwrap().is_err());
        }
        assert_eq!(DUPLICATE_PROBES.with(Cell::get), 5);
        assert_eq!(b.storage(), floor);
        assert!(b.work() > prefix);
        assert_eq!((b.failed_work(), b.failed_storage()), denials);
        all_refs(&v, 1);
        t.revalidate(&v, &mut b).unwrap();
    }
}

#[test]
fn full_set_outer_scope_accounting_refusal_drops_duplicates_and_preserves_original_account() {
    let t = Tree::with_lengths([137, 67, 71, 73, 79, 83]);
    let mut work = Work::new(WORK_LIMIT);
    let mut b = Budget::new(&mut work, LIMIT);
    b.reserve_storage(APPROVAL_STORAGE).unwrap();
    let v = t.retain(&mut b);
    let floor = b.storage();
    assert_eq!(floor, v.required_storage());
    let prefix = b.work();
    let address = &b as *const Budget<'_> as usize;
    let ledger = b.work_ledger_identity_v1();
    assert!(b.charge_work(WORK_LIMIT).is_err());
    assert!(b.reserve_storage(LIMIT).is_err());
    let denials = (b.failed_work(), b.failed_storage());
    assert_eq!(denials, (Some(prefix + WORK_LIMIT), Some(floor + LIMIT)));
    let charge = inventory_transfer_storage(v.manifest.total_file_bytes() as usize).unwrap();
    let protected = floor + INVENTORY_SCRATCH + charge.full_storage();
    let final_work: usize = v
        .manifest
        .entries()
        .map(|entry| ENTRY_IO_WORK + entry.length as usize * 8)
        .sum();
    let (uid, gid) = owners();
    let mut calls = 0;
    let mut before_final_checks = None;
    let result = v.clone_inventory_using(uid, gid, synthetic_immutable, &mut b, |b| {
        calls += 1;
        assert_eq!(b.storage(), protected);
        all_refs(&v, if calls == 1 { 1 } else { 2 });
        t.revalidate(&v, b)?;
        if calls == 2 {
            // Fail only the outer frame check, after every duplicate and both
            // origin revalidations succeed. Never release an original owner byte.
            assert!(b.charge_work(WORK_LIMIT).is_err());
            assert!(b.reserve_storage(LIMIT).is_err());
            assert_eq!((b.failed_work(), b.failed_storage()), denials);
            b.release_storage(1)?;
            assert_eq!(b.storage(), protected - 1);
            assert!(b.storage() >= floor + charge.full_storage());
            before_final_checks = Some((b.work(), b.peak_storage(), PROBES.with(Cell::get)));
        }
        Ok(())
    });
    accounting(result);
    assert_eq!(calls, 2);
    let (checked_work, peak, probes) = before_final_checks.unwrap();
    // Every final duplicate passed both metadata/protection checks. The failure
    // therefore came from prepaid-scope exit, not constructor or validation refusal.
    assert_eq!(b.work(), checked_work + final_work);
    assert_eq!(
        PROBES.with(Cell::get),
        probes + 2 * v.manifest.entries().len()
    );
    assert!(b.work() > prefix);
    assert!(b.work() < WORK_LIMIT);
    assert_eq!(b.storage(), floor);
    assert_eq!(b.storage_limit(), LIMIT);
    assert_eq!(b.peak_storage(), peak);
    assert_eq!((b.failed_work(), b.failed_storage()), denials);
    assert_eq!(&b as *const Budget<'_> as usize, address);
    assert!(b.work_ledger_identity_v1() == ledger);
    all_refs(&v, 1);
    for original in v.files.iter().flatten() {
        assert_eq!(Snapshot::read(&original.file).unwrap(), original.snapshot);
    }
    t.revalidate(&v, &mut b).unwrap();
    assert_eq!(b.storage(), floor);
    assert_eq!((b.failed_work(), b.failed_storage()), denials);
    all_refs(&v, 1);
}

#[test]
fn final_origin_recheck_rejects_replaced_shared_library_and_preserves_live_set() {
    for validation in [false, true] {
        let t = Tree::new(64);
        let mut work = Work::new(WORK_LIMIT);
        let mut b = Budget::new(&mut work, LIMIT);
        b.reserve_storage(APPROVAL_STORAGE).unwrap();
        let v = t.retain(&mut b);
        let existing = if validation {
            let (transfer, charge) = duplicate(&t, &v, &mut b).unwrap();
            b.reserve_storage(charge.full_storage()).unwrap();
            Some(transfer)
        } else {
            None
        };
        let floor = b.storage();
        let (uid, gid) = owners();
        let mut calls = 0;
        let mut revalidate = |b: &mut Budget<'_>| {
            calls += 1;
            if calls == 2 {
                fs::rename(t.code(PATHS[3]), t.root.join("old-shared.so")).unwrap();
                fs::write(t.code(PATHS[3]), [4; 64]).unwrap();
                chmod(&t.code(PATHS[3]), 0o444);
            }
            t.revalidate(&v, b)
        };
        let result = match &existing {
            Some(transfer) => v.validate_inventory_using(
                transfer,
                uid,
                gid,
                synthetic_immutable,
                &mut b,
                &mut revalidate,
            ),
            None => v
                .clone_inventory_using(uid, gid, synthetic_immutable, &mut b, &mut revalidate)
                .map(drop),
        };
        assert!(result.is_err());
        assert_eq!(calls, 2);
        assert_eq!(b.storage(), floor);
        all_refs(&v, if validation { 2 } else { 1 });
        drop(existing);
        all_refs(&v, 1);
    }
}

#[test]
fn inventory_transfer_charge_overflow_and_send_ownership() {
    assert!(matches!(
        inventory_transfer_storage(usize::MAX),
        Err(RetainedCompilerRuntimeErrorV1::Resource(
            Resource::Arithmetic
        ))
    ));
    fn send_static<T: Send + 'static>() {}
    send_static::<Transfer>();
}

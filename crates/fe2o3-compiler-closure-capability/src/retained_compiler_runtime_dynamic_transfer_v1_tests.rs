//! Reuse private synthetic inventory mechanics, never public runtime approval.
use super::*;

const ROLES: [(usize, TransferRole); 4] = [
    (1, TransferRole::Rustc),
    (2, TransferRole::Interpreter),
    (4, TransferRole::CodegenBackend),
    (5, TransferRole::Fe2o3ProcMacro),
];

fn duplicate(
    t: &Tree,
    v: &Inventory,
    role: TransferRole,
    b: &mut Budget<'_>,
) -> Result<(File, RetainedCompilerRuntimeExecTransferChargeV1)> {
    let (uid, gid) = owners();
    v.clone_image_using(role, uid, gid, synthetic_immutable, b, |b| {
        t.revalidate(v, b)
    })
}

fn validate(
    t: &Tree,
    v: &Inventory,
    role: TransferRole,
    file: &File,
    b: &mut Budget<'_>,
) -> Result<()> {
    let (uid, gid) = owners();
    v.validate_image_using(role, file, uid, gid, synthetic_immutable, b, |b| {
        t.revalidate(v, b)
    })
}

#[test]
fn exec_and_load_transfers_keep_separate_full_actual_length_charges() {
    let lengths = [64, 137, 193, 64, CHUNK + 1, 251];
    let t = Tree::with_lengths(lengths);
    let mut work = Work::new(ROLES.len() * LIMIT);
    let mut b = Budget::new(&mut work, LIMIT);
    b.reserve_storage(APPROVAL_STORAGE).unwrap();
    let v = t.retain(&mut b);
    let floor = b.storage();
    let mut transfers = Vec::new();
    for (index, role) in ROLES {
        let before = b.storage();
        let (file, charge) = duplicate(&t, &v, role, &mut b).unwrap();
        assert_eq!(b.storage(), before);
        assert_eq!(
            charge.full_storage(),
            size_of::<(File, RetainedCompilerRuntimeExecTransferChargeV1)>() + lengths[index]
        );
        b.reserve_storage(charge.full_storage()).unwrap();
        validate(&t, &v, role, &file, &mut b).unwrap();
        assert_eq!(
            Snapshot::read(&file).unwrap(),
            v.files[index].as_ref().unwrap().snapshot
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
        transfers.push((role, file, charge));
    }
    assert_eq!(
        b.storage(),
        floor
            + transfers
                .iter()
                .map(|(_, _, c)| c.full_storage())
                .sum::<usize>()
    );
    for (role, file, charge) in transfers {
        validate(&t, &v, role, &file, &mut b).unwrap();
        drop(file);
        b.release_storage(charge.full_storage()).unwrap();
    }
    t.revalidate(&v, &mut b).unwrap();
    assert_eq!(b.storage(), floor);
}

#[test]
fn dynamic_transfers_refuse_every_other_role_and_equal_bytes_at_another_inode() {
    for (index, role) in ROLES {
        let t = Tree::new(64);
        let mut work = Work::new(LIMIT);
        let mut b = Budget::new(&mut work, LIMIT);
        b.reserve_storage(APPROVAL_STORAGE).unwrap();
        let v = t.retain(&mut b);
        let (file, charge) = duplicate(&t, &v, role, &mut b).unwrap();
        b.reserve_storage(charge.full_storage()).unwrap();
        for (other, (_, retained)) in v.manifest.entries().zip(&v.files).enumerate() {
            if other != index {
                assert!(validate(&t, &v, role, &retained.as_ref().unwrap().file, &mut b).is_err());
            }
        }
        let copy_path = t.root.join("same-bytes");
        fs::write(&copy_path, vec![index as u8 + 1; 64]).unwrap();
        chmod(&copy_path, role.role().protected_mode());
        assert!(validate(&t, &v, role, &File::open(copy_path).unwrap(), &mut b).is_err());
        validate(&t, &v, role, &file, &mut b).unwrap();
    }
}

#[test]
fn transfers_enforce_distinct_exec_and_load_modes() {
    for (index, role) in ROLES {
        let t = Tree::new(64);
        let mut work = Work::new(LIMIT);
        let mut b = Budget::new(&mut work, LIMIT);
        b.reserve_storage(APPROVAL_STORAGE).unwrap();
        let v = t.retain(&mut b);
        let (file, charge) = duplicate(&t, &v, role, &mut b).unwrap();
        b.reserve_storage(charge.full_storage()).unwrap();
        let before = b.storage();
        let expected = if index < 3 { 0o555 } else { 0o444 };
        assert_eq!(Snapshot::read(&file).unwrap().mode & 0o7777, expected);
        validate(&t, &v, role, &file, &mut b).unwrap();
        chmod(&t.code(PATHS[index]), expected ^ 0o111);
        assert!(matches!(
            validate(&t, &v, role, &file, &mut b),
            Err(RetainedCompilerRuntimeErrorV1::Mismatch(
                "invalid protected code file"
            ))
        ));
        assert!(duplicate(&t, &v, role, &mut b).is_err());
        assert_eq!(b.storage(), before);
    }
}

#[test]
fn transfers_refuse_writable_path_only_and_non_cloexec_descriptors() {
    for (index, role) in ROLES {
        let t = Tree::new(64);
        // The synthetic fixture allows a writable alias before admission. That
        // alias is never eligible for transfer, even with the same inode/bytes.
        let path = t.code(PATHS[index]);
        chmod(&path, 0o600);
        let writable = fs::OpenOptions::new()
            .read(true)
            .write(true)
            .open(&path)
            .unwrap();
        chmod(&path, role.role().protected_mode());
        let path_only = File::from(
            rustix::fs::open(&path, OFlags::PATH | OFlags::CLOEXEC, Mode::empty()).unwrap(),
        );
        let mut work = Work::new(LIMIT);
        let mut b = Budget::new(&mut work, LIMIT);
        b.reserve_storage(APPROVAL_STORAGE).unwrap();
        let v = t.retain(&mut b);
        let (file, charge) = duplicate(&t, &v, role, &mut b).unwrap();
        b.reserve_storage(charge.full_storage()).unwrap();
        let before = b.storage();
        for invalid in [&writable, &path_only] {
            assert!(matches!(
                validate(&t, &v, role, invalid, &mut b),
                Err(RetainedCompilerRuntimeErrorV1::Mismatch(
                    "invalid protected code file"
                ))
            ));
        }
        rustix::io::fcntl_setfd(&file, rustix::io::FdFlags::empty()).unwrap();
        assert!(matches!(
            validate(&t, &v, role, &file, &mut b),
            Err(RetainedCompilerRuntimeErrorV1::Mismatch(
                "invalid protected code file"
            ))
        ));
        assert_eq!(b.storage(), before);
    }
}

#[test]
fn load_transfers_recheck_unselected_inventory_and_fixed_manifest_origins() {
    for (index, role) in ROLES.into_iter().filter(|(i, _)| *i >= 4) {
        for attack in 0..3 {
            let t = Tree::new(64);
            let mut work = Work::new(LIMIT);
            let mut b = Budget::new(&mut work, LIMIT);
            b.reserve_storage(APPROVAL_STORAGE).unwrap();
            let v = t.retain(&mut b);
            let (file, charge) = duplicate(&t, &v, role, &mut b).unwrap();
            b.reserve_storage(charge.full_storage()).unwrap();
            let before = b.storage();
            let path = match attack {
                0 => t.code(PATHS[3]), // Unselected shared library remains required.
                1 => t.manifest_path(),
                _ => t.code(PATHS[index]),
            };
            let previous = t.root.join("previous-origin");
            fs::rename(&path, &previous).unwrap();
            if attack == 2 {
                symlink(&previous, &path).unwrap();
            } else {
                fs::write(&path, fs::read(&previous).unwrap()).unwrap();
                chmod(&path, 0o444);
            }
            assert!(duplicate(&t, &v, role, &mut b).is_err());
            assert!(validate(&t, &v, role, &file, &mut b).is_err());
            assert_eq!(b.storage(), before);
        }
    }
}

#[test]
fn load_transfer_checks_digest_and_immutability_in_addition_to_snapshot() {
    for (_, role) in ROLES.into_iter().filter(|(i, _)| *i >= 4) {
        let t = Tree::new(64);
        let mut work = Work::new(LIMIT);
        let mut b = Budget::new(&mut work, LIMIT);
        b.reserve_storage(APPROVAL_STORAGE).unwrap();
        let v = t.retain(&mut b);
        b.reserve_storage(TRANSFER_SCRATCH).unwrap();
        let mut image = v.image(role, &mut b).unwrap();
        let file = &image.retained.file;
        let (uid, gid) = owners();
        image.entry.sha256[0] ^= 1;
        assert!(matches!(
            image.check_file(file, uid, gid, synthetic_immutable, &mut b),
            Err(RetainedCompilerRuntimeErrorV1::Mismatch(
                "code bytes differ from approved digest"
            ))
        ));
        image.entry.sha256[0] ^= 1;
        fn refuse_immutable(_: &File) -> Result<()> {
            Err(mismatch("synthetic load protection refusal"))
        }
        assert!(matches!(
            image.check_file(file, uid, gid, refuse_immutable, &mut b),
            Err(RetainedCompilerRuntimeErrorV1::Mismatch(
                "synthetic load protection refusal"
            ))
        ));
    }
}

#[test]
fn dynamic_transfer_revalidates_fixed_origins_after_duplication_and_final_check() {
    for (index, role) in ROLES {
        for final_check in [false, true] {
            let t = Tree::new(64);
            let mut work = Work::new(LIMIT);
            let mut b = Budget::new(&mut work, LIMIT);
            b.reserve_storage(APPROVAL_STORAGE).unwrap();
            let v = t.retain(&mut b);
            let (file, charge) = duplicate(&t, &v, role, &mut b).unwrap();
            b.reserve_storage(charge.full_storage()).unwrap();
            let before = b.storage();
            let (uid, gid) = owners();
            let mut calls = 0;
            let recheck = |b: &mut Budget<'_>| {
                calls += 1;
                if calls == 2 {
                    let source = t.code(PATHS[index]);
                    let bytes = fs::read(&source).unwrap();
                    fs::rename(&source, t.root.join("old-source")).unwrap();
                    fs::write(&source, bytes).unwrap();
                    chmod(&source, role.role().protected_mode());
                }
                t.revalidate(&v, b)
            };
            let result = if final_check {
                v.validate_image_using(role, &file, uid, gid, synthetic_immutable, &mut b, recheck)
            } else {
                v.clone_image_using(role, uid, gid, synthetic_immutable, &mut b, recheck)
                    .map(drop)
            };
            assert!(result.is_err());
            assert_eq!(calls, 2);
            assert_eq!(b.storage(), before);
        }
    }
}

#[test]
fn missing_dynamic_source_refuses_before_origin_io() {
    for (index, role) in ROLES {
        let t = Tree::new(64);
        let mut work = Work::new(LIMIT);
        let mut b = Budget::new(&mut work, LIMIT);
        b.reserve_storage(APPROVAL_STORAGE).unwrap();
        let mut v = t.retain(&mut b);
        v.files[index] = None;
        PROBES.with(|v| v.set(0));
        assert!(matches!(
            duplicate(&t, &v, role, &mut b),
            Err(RetainedCompilerRuntimeErrorV1::Mismatch(
                "image custody absent"
            ))
        ));
        assert_eq!(PROBES.with(Cell::get), 0);
    }
}

#[test]
fn dynamic_transfers_require_original_account_and_full_reservation_before_io() {
    for (_, role) in ROLES {
        let t = Tree::new(64);
        let mut work = Work::new(LIMIT);
        let mut other = Work::new(LIMIT);
        let mut b = Budget::new(&mut work, LIMIT);
        b.reserve_storage(APPROVAL_STORAGE).unwrap();
        let v = t.retain(&mut b);
        let (file, charge) = duplicate(&t, &v, role, &mut b).unwrap();
        b.reserve_storage(charge.full_storage() - 1).unwrap();
        PROBES.with(|v| v.set(0));
        accounting(validate(&t, &v, role, &file, &mut b));
        assert_eq!(PROBES.with(Cell::get), 0);
        b.reserve_storage(1).unwrap();
        validate(&t, &v, role, &file, &mut b).unwrap();
        let mut moved = Box::new(std::mem::replace(&mut b, Budget::new(&mut other, LIMIT)));
        b.reserve_storage(moved.storage()).unwrap();
        PROBES.with(|v| v.set(0));
        for account in [&mut b, &mut *moved] {
            accounting(duplicate(&t, &v, role, account));
            accounting(validate(&t, &v, role, &file, account));
        }
        assert_eq!(PROBES.with(Cell::get), 0);
    }
}

fn bounded(
    role: TransferRole,
    validation: bool,
    work: usize,
    storage: usize,
) -> (Result<()>, Usage) {
    let t = Tree::new(64);
    let mut work = Work::new(work);
    let mut b = Budget::new(&mut work, storage);
    b.reserve_storage(APPROVAL_STORAGE).unwrap();
    let v = t.retain(&mut b);
    let transfer = if validation {
        let (file, charge) = duplicate(&t, &v, role, &mut b).unwrap();
        b.reserve_storage(charge.full_storage()).unwrap();
        Some(file)
    } else {
        None
    };
    b.reserve_storage(FRAME).unwrap();
    let before = b.storage();
    let result = match transfer {
        Some(ref file) => validate(&t, &v, role, file, &mut b),
        None => duplicate(&t, &v, role, &mut b).map(drop),
    };
    assert_eq!(b.storage(), before);
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
fn dynamic_transfer_and_final_check_exact_and_one_short_limits() {
    for (_, role) in ROLES {
        for validation in [false, true] {
            let (result, used) = bounded(role, validation, LIMIT, LIMIT);
            result.unwrap();
            let (result, exact) = bounded(role, validation, used.work, used.peak);
            result.unwrap();
            assert_eq!(exact.work, used.work);
            assert_eq!(exact.peak, used.peak);
            let (result, short) = bounded(role, validation, used.work - 1, used.peak);
            assert!(result.is_err());
            assert!(short.failed_work.is_some());
            let (result, short) = bounded(role, validation, used.work, used.peak - 1);
            assert!(result.is_err());
            assert!(short.failed_storage.is_some());
        }
    }
}

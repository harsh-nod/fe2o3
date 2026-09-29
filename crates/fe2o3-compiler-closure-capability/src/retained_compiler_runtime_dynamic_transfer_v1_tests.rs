//! Reuse private synthetic inventory mechanics, never public runtime approval.
use super::*;

const ROLES: [ExecRole; 2] = [ExecRole::Rustc, ExecRole::Interpreter];

fn duplicate(
    t: &Tree,
    v: &Inventory,
    role: ExecRole,
    b: &mut Budget<'_>,
) -> Result<(File, RetainedCompilerRuntimeExecTransferChargeV1)> {
    let (uid, gid) = owners();
    v.clone_executable_using(role, uid, gid, synthetic_immutable, b, |b| {
        t.revalidate(v, b)
    })
}

fn validate(
    t: &Tree,
    v: &Inventory,
    role: ExecRole,
    file: &File,
    b: &mut Budget<'_>,
) -> Result<()> {
    let (uid, gid) = owners();
    v.validate_executable_using(role, file, uid, gid, synthetic_immutable, b, |b| {
        t.revalidate(v, b)
    })
}

#[test]
fn rustc_and_interpreter_transfers_keep_separate_full_backing_charges() {
    let t = Tree::new(64);
    let mut work = Work::new(LIMIT);
    let mut b = Budget::new(&mut work, LIMIT);
    b.reserve_storage(APPROVAL_STORAGE).unwrap();
    let v = t.retain(&mut b);
    let floor = b.storage();
    let mut transfers = Vec::new();
    for (index, role) in ROLES.into_iter().enumerate() {
        let before = b.storage();
        let (file, charge) = duplicate(&t, &v, role, &mut b).unwrap();
        assert_eq!(b.storage(), before);
        assert_eq!(
            charge.full_storage(),
            size_of::<(File, RetainedCompilerRuntimeExecTransferChargeV1)>() + 64
        );
        b.reserve_storage(charge.full_storage()).unwrap();
        validate(&t, &v, role, &file, &mut b).unwrap();
        assert_eq!(
            Snapshot::read(&file).unwrap(),
            v.files[index + 1].as_ref().unwrap().snapshot
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
    for (index, role) in ROLES.into_iter().enumerate() {
        let t = Tree::new(64);
        let mut work = Work::new(LIMIT);
        let mut b = Budget::new(&mut work, LIMIT);
        b.reserve_storage(APPROVAL_STORAGE).unwrap();
        let v = t.retain(&mut b);
        let (file, charge) = duplicate(&t, &v, role, &mut b).unwrap();
        b.reserve_storage(charge.full_storage()).unwrap();
        for (other, (_, retained)) in v.manifest.entries().zip(&v.files).enumerate() {
            if other != index + 1 {
                assert!(validate(&t, &v, role, &retained.as_ref().unwrap().file, &mut b).is_err());
            }
        }
        let copy_path = t.root.join("same-bytes");
        fs::write(&copy_path, vec![index as u8 + 2; 64]).unwrap();
        chmod(&copy_path, 0o555);
        assert!(validate(&t, &v, role, &File::open(copy_path).unwrap(), &mut b).is_err());
        validate(&t, &v, role, &file, &mut b).unwrap();
    }
}

#[test]
fn dynamic_transfer_revalidates_fixed_origins_after_duplication_and_final_check() {
    for (index, role) in ROLES.into_iter().enumerate() {
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
                    let source = t.code(PATHS[index + 1]);
                    let bytes = fs::read(&source).unwrap();
                    fs::rename(&source, t.root.join("old-source")).unwrap();
                    fs::write(&source, bytes).unwrap();
                    chmod(&source, 0o555);
                }
                t.revalidate(&v, b)
            };
            let result = if final_check {
                v.validate_executable_using(
                    role,
                    &file,
                    uid,
                    gid,
                    synthetic_immutable,
                    &mut b,
                    recheck,
                )
            } else {
                v.clone_executable_using(role, uid, gid, synthetic_immutable, &mut b, recheck)
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
    for (index, role) in ROLES.into_iter().enumerate() {
        let t = Tree::new(64);
        let mut work = Work::new(LIMIT);
        let mut b = Budget::new(&mut work, LIMIT);
        b.reserve_storage(APPROVAL_STORAGE).unwrap();
        let mut v = t.retain(&mut b);
        v.files[index + 1] = None;
        PROBES.with(|v| v.set(0));
        assert!(matches!(
            duplicate(&t, &v, role, &mut b),
            Err(RetainedCompilerRuntimeErrorV1::Mismatch(
                "executable custody absent"
            ))
        ));
        assert_eq!(PROBES.with(Cell::get), 0);
    }
}

#[test]
fn dynamic_transfers_require_original_account_and_full_reservation_before_io() {
    for role in ROLES {
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

fn bounded(role: ExecRole, validation: bool, work: usize, storage: usize) -> (Result<()>, Usage) {
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
    for role in ROLES {
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

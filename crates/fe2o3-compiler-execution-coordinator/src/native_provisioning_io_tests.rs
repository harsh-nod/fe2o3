use super::*;
use fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1 as Work;
use std::os::unix::fs::PermissionsExt;

fn owner() -> Owner {
    Owner {
        uid: rustix::process::geteuid().as_raw(),
        gid: rustix::process::getegid().as_raw(),
    }
}
fn mode(path: &Path, mode: u32) {
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(mode)).unwrap();
}
fn fixture() -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    mode(dir.path(), 0o755);
    dir
}
fn image(path: &Path, tag: u8) -> Vec<u8> {
    let bytes = crate::provisioning_entrypoint::static_pause_elf(tag);
    std::fs::write(path, &bytes).unwrap();
    mode(path, 0o555);
    bytes
}
const MAX: usize = 8192;

#[test]
fn measured_image_matches_real_bytes_and_bounded_revalidation() {
    let dir = fixture();
    let path = dir.path().join("image");
    let bytes = image(&path, 1);
    let (work, scratch) = Image::quota(MAX).unwrap();
    let (rw, rs) = Image::revalidation_quota(bytes.len()).unwrap();
    let mut w = Work::new(work + rw);
    let mut b = Budget::new(&mut w, 19 + scratch);
    b.reserve_storage(19).unwrap();
    let ledger = b.work_ledger_identity_v1();
    let measured = Image::measure(&path, MAX, owner(), &mut b).unwrap();
    assert_eq!(b.work(), work);
    assert_eq!(b.storage(), 19);
    assert_eq!(b.peak_storage(), 19 + scratch);
    assert_eq!(
        measured.measurement.sha256(),
        <[u8; 32]>::from(Sha256::digest(&bytes))
    );
    assert_eq!(measured.measurement.byte_len(), bytes.len() as u64);
    let retained = measured.retained_storage();
    b.reserve_storage(retained).unwrap();
    measured.revalidate(&mut b).unwrap();
    assert_eq!(b.work(), work + rw);
    assert_eq!(b.peak_storage(), 19 + scratch.max(retained + rs));
    assert!(b.work_ledger_identity_v1() == ledger);
    drop(measured);
    b.release_storage(retained).unwrap();
    assert_eq!(b.storage(), 19);
}

#[test]
fn image_budget_refusal_precedes_open_and_overflow_precedes_all_work() {
    let (work, scratch) = Image::quota(MAX).unwrap();
    for (w, s) in [(work - 1, scratch), (work, scratch - 1)] {
        let mut w = Work::new(w);
        let mut b = Budget::new(&mut w, s);
        assert!(matches!(
            Image::measure(Path::new("/absent-native-image"), MAX, owner(), &mut b),
            Err(Failure::Resource(_))
        ));
        assert_eq!(b.storage(), 0);
    }
    for n in [0, usize::MAX] {
        assert!(Image::quota(n).is_err());
    }
}

#[test]
fn image_revalidation_short_floor_work_and_scratch_refuse_on_the_original_ledger() {
    let dir = fixture();
    let path = dir.path().join("image");
    image(&path, 1);
    let mut setup_work = Work::new(usize::MAX);
    let mut setup = Budget::new(&mut setup_work, usize::MAX);
    let measured = Image::measure(&path, MAX, owner(), &mut setup).unwrap();
    let (work, scratch) =
        Image::revalidation_quota(measured.measurement.byte_len() as usize).unwrap();
    for case in 0..3 {
        let floor = measured.retained_storage() - usize::from(case == 0);
        let mut w = Work::new(work - usize::from(case == 1));
        let mut b = Budget::new(&mut w, floor + scratch - usize::from(case == 2));
        b.reserve_storage(floor).unwrap();
        let identity = b.work_ledger_identity_v1();
        assert!(matches!(
            measured.revalidate(&mut b),
            Err(Failure::Resource(_))
        ));
        assert_eq!(b.storage(), floor);
        assert!(b.work_ledger_identity_v1() == identity);
    }
    assert!(Image::revalidation_quota(0).is_err());
    assert!(Image::revalidation_quota(usize::MAX).is_err());
}

#[test]
fn retained_seed_rejects_same_inode_rewrite_and_same_bytes_path_replacement() {
    for replaced in [false, true] {
        let dir = fixture();
        let mut w = Work::new(usize::MAX);
        let mut b = Budget::new(&mut w, usize::MAX);
        let directory = Directory::open(dir.path(), owner(), &mut b).unwrap();
        b.reserve_storage(Directory::STORAGE).unwrap();
        let seed = directory.seed("seed", &mut b).unwrap();
        b.reserve_storage(SEED_STORAGE).unwrap();
        directory.verify_seed(&seed, &mut b).unwrap();
        let path = dir.path().join("seed");
        if replaced {
            std::fs::rename(&path, dir.path().join("moved")).unwrap();
            std::fs::copy(dir.path().join("moved"), &path).unwrap();
        } else {
            mode(&path, 0o600);
            let mut changed = Zeroizing::new(*seed);
            changed[0] ^= 1;
            std::fs::write(&path, &changed[..]).unwrap();
        }
        mode(&path, 0o400);
        assert!(directory.verify_seed(&seed, &mut b).is_err());
    }
}

#[test]
fn changed_path_inode_and_changed_retained_bytes_are_both_rejected() {
    for replaced in [false, true] {
        let dir = fixture();
        let path = dir.path().join("image");
        image(&path, 1);
        let mut w = Work::new(usize::MAX);
        let mut b = Budget::new(&mut w, usize::MAX);
        let measured = Image::measure(&path, MAX, owner(), &mut b).unwrap();
        b.reserve_storage(measured.retained_storage()).unwrap();
        if replaced {
            std::fs::rename(&path, dir.path().join("old")).unwrap();
        } else {
            mode(&path, 0o755);
        }
        image(&path, 2);
        assert!(measured.revalidate(&mut b).is_err());
    }
}

#[test]
fn image_provenance_and_static_profile_refuse_invalid_sources() {
    for case in 0..5 {
        let dir = fixture();
        let path = dir.path().join("image");
        let mut data = image(&path, 1);
        match case {
            0 => mode(&path, 0o755),
            1 => {
                std::fs::hard_link(&path, dir.path().join("alias")).unwrap();
            }
            2 => {
                std::fs::rename(&path, dir.path().join("target")).unwrap();
                std::os::unix::fs::symlink("target", &path).unwrap();
            }
            3 => {
                mode(&path, 0o755);
                data[0] = 0;
                std::fs::write(&path, data).unwrap();
                mode(&path, 0o555);
            }
            _ => {}
        }
        let mut w = Work::new(usize::MAX);
        let mut b = Budget::new(&mut w, usize::MAX);
        let result = Image::measure(&path, if case == 4 { 64 } else { MAX }, owner(), &mut b);
        assert!(result.is_err(), "case {case}");
        assert_eq!(b.storage(), 0);
    }
}

#[test]
fn seed_and_record_publication_are_durable_idempotent_and_no_replace() {
    let dir = fixture();
    let mut w = Work::new(usize::MAX);
    let mut b = Budget::new(&mut w, usize::MAX);
    let directory = Directory::open(dir.path(), owner(), &mut b).unwrap();
    b.reserve_storage(Directory::STORAGE + 32 + 32).unwrap();
    let first = directory.seed("seed", &mut b).unwrap();
    let repeated = directory.seed("seed", &mut b).unwrap();
    assert_eq!(*first, *repeated);
    assert_eq!(
        std::fs::metadata(dir.path().join("seed"))
            .unwrap()
            .permissions()
            .mode()
            & 0o7777,
        0o400
    );
    let record = [0x41; 32];
    directory.publish("record", &record, &mut b).unwrap();
    directory.publish("record", &record, &mut b).unwrap();
    assert_eq!(std::fs::read(dir.path().join("record")).unwrap(), record);
    assert_eq!(
        std::fs::metadata(dir.path().join("record"))
            .unwrap()
            .permissions()
            .mode()
            & 0o7777,
        0o444
    );
    assert!(directory.publish("record", &[0x42; 32], &mut b).is_err());
    assert_eq!(std::fs::read(dir.path().join("record")).unwrap(), record);
    assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), 2);
    directory.revalidate(&mut b).unwrap();
}

#[test]
fn directory_lock_excludes_an_independent_installer_and_rejects_path_replacement() {
    let outer = fixture();
    let path = outer.path().join("config");
    std::fs::create_dir(&path).unwrap();
    mode(&path, 0o755);
    let mut w = Work::new(usize::MAX);
    let mut b = Budget::new(&mut w, usize::MAX);
    let directory = Directory::open(&path, owner(), &mut b).unwrap();
    b.reserve_storage(Directory::STORAGE).unwrap();
    assert!(Directory::open(&path, owner(), &mut b).is_err());
    std::fs::rename(&path, outer.path().join("old")).unwrap();
    std::fs::create_dir(&path).unwrap();
    mode(&path, 0o755);
    assert!(directory.revalidate(&mut b).is_err());
    assert!(directory.seed("seed", &mut b).is_err());
    assert!(!outer.path().join("old/seed").exists());
    assert!(!path.join("seed").exists());
}

#[test]
fn temporary_collision_and_substitution_are_never_unlinked() {
    let dir = fixture();
    let mut w = Work::new(usize::MAX);
    let mut b = Budget::new(&mut w, usize::MAX);
    let directory = Directory::open(dir.path(), owner(), &mut b).unwrap();
    let path = dir.path().join("temporary");
    std::fs::write(&path, b"other owner").unwrap();
    assert!(
        directory
            .publish_new_named("record", b"payload", 0o444, "temporary")
            .is_err()
    );
    assert_eq!(std::fs::read(&path).unwrap(), b"other owner");
    let pending = Pending {
        directory: &directory.file,
        name: "temporary",
        file: File::open(&path).unwrap(),
        renamed: false,
    };
    std::fs::rename(&path, dir.path().join("moved")).unwrap();
    std::fs::write(&path, b"replacement").unwrap();
    drop(pending);
    assert_eq!(std::fs::read(&path).unwrap(), b"replacement");
    assert!(dir.path().join("moved").exists());
}

#[test]
fn pending_owned_temporary_is_removed_and_no_replace_keeps_the_original() {
    let dir = fixture();
    let mut w = Work::new(usize::MAX);
    let mut b = Budget::new(&mut w, usize::MAX);
    let directory = Directory::open(dir.path(), owner(), &mut b).unwrap();
    std::fs::write(dir.path().join("record"), b"original").unwrap();
    assert!(
        directory
            .publish_new_named("record", b"new", 0o444, "temporary")
            .is_err()
    );
    assert!(!dir.path().join("temporary").exists());
    assert_eq!(
        std::fs::read(dir.path().join("record")).unwrap(),
        b"original"
    );
}

#[test]
fn rerun_after_rename_sync_failure_verifies_and_syncs_the_existing_inode() {
    let dir = fixture();
    let mut w = Work::new(usize::MAX);
    let mut b = Budget::new(&mut w, usize::MAX);
    let directory = Directory::open(dir.path(), owner(), &mut b).unwrap();
    b.reserve_storage(Directory::STORAGE + 32).unwrap();
    let expected = [0x41; 32];
    let mut synced = false;
    let result = directory.publish_with_sync("record", &expected, 0o444, "temporary", |_| {
        synced = true;
        assert_eq!(std::fs::read(dir.path().join("record")).unwrap(), expected);
        Err(rustix::io::Errno::IO)
    });
    assert!(synced);
    assert!(matches!(
        result,
        Err(Failure::Io {
            operation: "sync provisioning directory",
            ..
        })
    ));
    assert!(!dir.path().join("temporary").exists());
    let file = directory.existing("record").unwrap().unwrap();
    let before = snapshot(&file, owner().policy(0o444, 32)).unwrap();
    directory.publish("record", &expected, &mut b).unwrap();
    let named = directory.existing("record").unwrap().unwrap();
    assert_eq!(snapshot(&named, owner().policy(0o444, 32)).unwrap(), before);
    assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), 1);
}

#[test]
fn record_short_budgets_refuse_before_publication_and_preserve_first_denials() {
    for case in 0..3 {
        let dir = fixture();
        let mut setup = Work::new(FILE_WORK);
        let mut setup_b = Budget::new(&mut setup, FILE_SCRATCH);
        let directory = Directory::open(dir.path(), owner(), &mut setup_b).unwrap();
        let floor = Directory::STORAGE + 32 - usize::from(case == 0);
        let work = record_work(32).unwrap() - usize::from(case == 1);
        let limit = floor + record_scratch(32).unwrap() - usize::from(case == 2);
        let mut w = Work::new(work);
        let mut b = Budget::new(&mut w, limit);
        b.reserve_storage(floor).unwrap();
        let ledger = b.work_ledger_identity_v1();
        assert!(directory.publish("record", &[0; 32], &mut b).is_err());
        assert!(!dir.path().join("record").exists());
        assert_eq!(b.storage(), floor);
        assert!(b.work_ledger_identity_v1() == ledger);
    }
}

#[test]
fn listener_presence_including_dangling_symlink_prevents_provisioning() {
    let dir = fixture();
    let path = dir.path().join("listener");
    let mut w = Work::new(3 * FILE_WORK);
    let mut b = Budget::new(&mut w, FILE_SCRATCH);
    listener_absent(&path, &mut b).unwrap();
    std::os::unix::fs::symlink("missing", &path).unwrap();
    assert!(listener_absent(&path, &mut b).is_err());
    std::fs::remove_file(&path).unwrap();
    std::fs::write(&path, b"present").unwrap();
    assert!(listener_absent(&path, &mut b).is_err());
}

#[test]
fn writes_and_fixed_temporary_names_have_no_retry_or_growth() {
    assert!(exact_write(3, Ok(3)).is_ok());
    for result in [Ok(0), Ok(2), Ok(4), Err(rustix::io::Errno::INTR)] {
        assert!(exact_write(3, result).is_err());
    }
    assert_eq!(
        temporary_name([0; 16]),
        *b".provisioning-00000000000000000000000000000000"
    );
    assert_eq!(
        temporary_name([255; 16]),
        *b".provisioning-ffffffffffffffffffffffffffffffff"
    );
    assert!(record_work(0).is_err());
    assert!(record_work(usize::MAX).is_err());
    assert!(record_scratch(usize::MAX).is_err());
}

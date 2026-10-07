//! Genuine same-owner filesystem composition; lifecycle observations are injected.
//! These tests grant no root startup, NSS, C-environment or protected boot credit.
use super::*;
use std::{fs, os::unix::fs::PermissionsExt, path::PathBuf};

struct Fixture {
    _root: tempfile::TempDir,
    config: PathBuf,
    listener: PathBuf,
    images: [PathBuf; 5],
}
fn mode(path: &Path, value: u32) {
    fs::set_permissions(path, fs::Permissions::from_mode(value)).unwrap();
}
fn write_image(path: &Path, tag: u8) {
    if path.exists() {
        mode(path, 0o755);
    }
    fs::write(path, crate::provisioning_entrypoint::static_pause_elf(tag)).unwrap();
    mode(path, 0o555);
}
impl Fixture {
    fn new() -> Self {
        let root = tempfile::tempdir().unwrap();
        let config = root.path().join("config");
        fs::create_dir(&config).unwrap();
        mode(&config, 0o755);
        let listener = root.path().join("listener");
        let images = std::array::from_fn(|i| {
            let path = root.path().join(format!("image-{i}"));
            write_image(&path, i as u8);
            path
        });
        Self {
            _root: root,
            config,
            listener,
            images,
        }
    }
    fn layout(&self) -> Layout<'_> {
        Layout {
            config: &self.config,
            listener: &self.listener,
            images: self.images.each_ref().map(PathBuf::as_path),
            limits: [8192; 5],
        }
    }
    fn owner(&self) -> io::Owner {
        io::Owner {
            uid: rustix::process::geteuid().as_raw(),
            gid: rustix::process::getegid().as_raw(),
        }
    }
    fn count(&self) -> usize {
        fs::read_dir(&self.config).unwrap().count()
    }
    fn run(&self, mut observe: impl FnMut(usize) -> Result<()>) -> Result<(usize, usize)> {
        let (work, storage) = quota().unwrap();
        let mut w = Work::new(work + 19);
        let mut b = Budget::new(&mut w, storage + 23);
        b.charge_work(19).unwrap();
        b.reserve_storage(23).unwrap();
        let identity = b.work_ledger_identity_v1();
        let mut calls = 0;
        let result = b.with_prepaid_scope(0, 8, LOCAL_WORK, FRAME, |b| {
            provision(
                &self.layout(),
                7,
                (1001, 1002),
                (2001, 2002),
                &mut |b| {
                    calls += 1;
                    b.with_prepaid_scope(0, 8, Lease::REVALIDATION_WORK, Lease::IO_STORAGE, |_| {
                        observe(calls)
                    })
                },
                self.owner(),
                b,
            )
        });
        assert_eq!(b.storage(), 23);
        assert!(b.work_ledger_identity_v1() == identity);
        assert!(b.work() <= work + 19);
        assert!(b.peak_storage() <= storage + 23);
        assert_eq!(b.failed_work(), None);
        assert_eq!(b.failed_storage(), None);
        result.map(|()| {
            assert_eq!(calls, 3);
            (b.work(), b.peak_storage())
        })
    }
}

#[test]
fn complete_filesystem_graph_is_native_and_idempotent_on_one_ledger() {
    crate::eof_test_process::isolated(
        module_path!(),
        "complete_filesystem_graph_is_native_and_idempotent_on_one_ledger",
        complete_filesystem_graph_is_native_and_idempotent_on_one_ledger_isolated,
    );
}

fn complete_filesystem_graph_is_native_and_idempotent_on_one_ledger_isolated() {
    let f = Fixture::new();
    let first = f.run(|_| Ok(())).unwrap();
    assert_eq!(f.count(), 7);
    let before = fs::read(f.config.join("client-profile-v3")).unwrap();
    let repeated = f.run(|_| Ok(())).unwrap();
    assert_eq!(first, repeated);
    assert_eq!(f.count(), 7);
    assert_eq!(
        fs::read(f.config.join("client-profile-v3")).unwrap(),
        before
    );
    let mut w = Work::new(COMPILER_EXECUTION_CLIENT_PROFILE_WORK_V3);
    let mut b = Budget::new(
        &mut w,
        before.len() + COMPILER_EXECUTION_CLIENT_PROFILE_STORAGE_V3,
    );
    b.reserve_storage(before.len()).unwrap();
    let (profile, _) = CompilerExecutionClientProfileV3::decode(&before, &mut b).unwrap();
    assert_eq!(profile.policy().generation(), 7);
    assert_eq!(profile.supervisor_uid(), 1001);
    assert_eq!(profile.supervisor_gid(), 1002);
    assert_eq!(
        profile.policy().canonical_bytes().as_slice(),
        fs::read(f.config.join("issuer-policy-v3")).unwrap()
    );
}

#[test]
fn prepublication_image_change_refuses_without_installing_stale_public_records() {
    crate::eof_test_process::isolated(
        module_path!(),
        "prepublication_image_change_refuses_without_installing_stale_public_records",
        prepublication_image_change_refuses_without_installing_stale_public_records_isolated,
    );
}

fn prepublication_image_change_refuses_without_installing_stale_public_records_isolated() {
    let f = Fixture::new();
    assert!(
        f.run(|step| {
            if step == 2 {
                write_image(&f.images[0], 23);
            }
            Ok(())
        })
        .is_err()
    );
    assert_eq!(f.count(), 2);
    f.run(|_| Ok(())).unwrap();
    assert_eq!(f.count(), 7);
}

#[test]
fn seed_path_replacement_is_rejected_before_publication_even_with_equal_bytes() {
    crate::eof_test_process::isolated(
        module_path!(),
        "seed_path_replacement_is_rejected_before_publication_even_with_equal_bytes",
        seed_path_replacement_is_rejected_before_publication_even_with_equal_bytes_isolated,
    );
}

fn seed_path_replacement_is_rejected_before_publication_even_with_equal_bytes_isolated() {
    let f = Fixture::new();
    assert!(
        f.run(|step| {
            if step == 2 {
                let name = f.config.join("issuer-signing-key-seed-v3");
                let moved = f._root.path().join("moved-seed");
                fs::rename(&name, &moved).unwrap();
                fs::copy(&moved, &name).unwrap();
                mode(&name, 0o400);
            }
            Ok(())
        })
        .is_err()
    );
    assert_eq!(f.count(), 2);
    f.run(|_| Ok(())).unwrap();
}

#[test]
fn each_lifecycle_refusal_restores_storage_and_allows_an_idempotent_rerun() {
    crate::eof_test_process::isolated(
        module_path!(),
        "each_lifecycle_refusal_restores_storage_and_allows_an_idempotent_rerun",
        each_lifecycle_refusal_restores_storage_and_allows_an_idempotent_rerun_isolated,
    );
}

fn each_lifecycle_refusal_restores_storage_and_allows_an_idempotent_rerun_isolated() {
    for failed_step in 1..=3 {
        let f = Fixture::new();
        assert!(
            f.run(|step| if step == failed_step {
                Err(Failure::Invalid("fixture lifecycle refusal"))
            } else {
                Ok(())
            })
            .is_err()
        );
        assert_eq!(f.count(), [0, 2, 7][failed_step - 1]);
        f.run(|_| Ok(())).unwrap();
        assert_eq!(f.count(), 7);
    }
}

#[test]
fn existing_listener_and_record_mismatch_are_not_repaired() {
    crate::eof_test_process::isolated(
        module_path!(),
        "existing_listener_and_record_mismatch_are_not_repaired",
        existing_listener_and_record_mismatch_are_not_repaired_isolated,
    );
}

fn existing_listener_and_record_mismatch_are_not_repaired_isolated() {
    let f = Fixture::new();
    fs::write(&f.listener, b"occupied").unwrap();
    assert!(
        f.run(|_| panic!("listener refusal precedes lifecycle callback"))
            .is_err()
    );
    assert_eq!(f.count(), 0);
    fs::remove_file(&f.listener).unwrap();
    f.run(|_| Ok(())).unwrap();
    let record = f.config.join("issuer-policy-v3");
    mode(&record, 0o644);
    let corrupt = [0; COMPILER_EXECUTION_ISSUER_POLICY_BYTES_V3];
    fs::write(&record, corrupt).unwrap();
    mode(&record, 0o444);
    assert!(f.run(|_| Ok(())).is_err());
    assert_eq!(fs::read(&record).unwrap(), corrupt);
}

#[test]
fn unwind_closes_the_directory_lock_and_restores_the_original_account() {
    crate::eof_test_process::isolated(
        module_path!(),
        "unwind_closes_the_directory_lock_and_restores_the_original_account",
        unwind_closes_the_directory_lock_and_restores_the_original_account_isolated,
    );
}

fn unwind_closes_the_directory_lock_and_restores_the_original_account_isolated() {
    let f = Fixture::new();
    let (work, storage) = quota().unwrap();
    let mut w = Work::new(work);
    let mut b = Budget::new(&mut w, storage);
    b.reserve_storage(23).unwrap();
    let identity = b.work_ledger_identity_v1();
    let mut steps = 0;
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let _ = b.with_prepaid_scope(0, 8, LOCAL_WORK, FRAME, |b| {
            provision(
                &f.layout(),
                7,
                (1001, 1002),
                (2001, 2002),
                &mut |_| {
                    steps += 1;
                    if steps == 2 {
                        panic!("fixture publication unwind");
                    }
                    Ok(())
                },
                f.owner(),
                b,
            )
        });
    }));
    assert!(result.is_err());
    assert_eq!(b.storage(), 23);
    assert!(b.work_ledger_identity_v1() == identity);
    assert_eq!(f.count(), 2);
    f.run(|_| Ok(())).unwrap();
}

#[test]
fn fixed_native_profile_path_and_checked_quota_have_no_legacy_fallback() {
    assert_eq!(
        Path::new(CONFIG).join("client-profile-v3"),
        Path::new(COMPILER_EXECUTION_CLIENT_PROFILE_PATH_V3)
    );
    assert!(sum(&[usize::MAX, 1]).is_err());
    let (work, storage) = quota().unwrap();
    assert!(work > LOCAL_WORK + Bundle::WORK);
    assert!(storage > FRAME + Bundle::SCRATCH);
    assert!(IMAGES[2].ends_with("-issuer-conditional"));
}

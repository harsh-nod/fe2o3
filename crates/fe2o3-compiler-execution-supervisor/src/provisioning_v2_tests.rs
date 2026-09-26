use super::*;
use crate::tests::{Fixture, bound_named_seqpacket_socket};
use fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1 as Work;
use std::os::{
    fd::AsFd,
    unix::fs::{MetadataExt, PermissionsExt},
};

const EXTRA: usize = 19;
fn credentials() -> Credentials {
    Credentials::new(
        rustix::process::geteuid().as_raw(),
        rustix::process::getegid().as_raw(),
    )
    .unwrap()
}

fn invalid_inputs() -> (OwnedFd, File) {
    let (read, write) = rustix::pipe::pipe_with(rustix::pipe::PipeFlags::CLOEXEC).unwrap();
    (write, File::from(read))
}

fn identity(fd: &impl AsFd) -> (u64, u64) {
    let stat = rustix::fs::fstat(fd).unwrap();
    (stat.st_dev, stat.st_ino)
}

fn assert_closed(object: (u64, u64)) {
    for entry in std::fs::read_dir("/proc/self/fd").unwrap() {
        if let Ok(metadata) = std::fs::metadata(entry.unwrap().path()) {
            assert_ne!((metadata.dev(), metadata.ino()), object);
        }
    }
}

#[test]
fn admission_requires_full_pair_and_closes_consumed_inputs() {
    let (listener, root) = invalid_inputs();
    let objects = [identity(&listener), identity(&root)];
    let mut work = Work::new(Inputs::WORK);
    let mut b = Budget::new(&mut work, usize::MAX);
    b.reserve_storage(Inputs::PAIR_STORAGE - 1).unwrap();
    assert!(matches!(
        Inputs::admit(listener, root, credentials(), &mut b),
        Err(Failure::Resource(Resource::Accounting))
    ));
    assert_eq!(b.storage(), Inputs::PAIR_STORAGE - 1);
    assert_eq!(b.work(), ENTRY);
    for object in objects {
        assert_closed(object);
    }
}

#[test]
fn admission_work_and_storage_refuse_before_io_and_restore_entry() {
    let floor = Inputs::PAIR_STORAGE + EXTRA;
    let peak = floor + Inputs::SCRATCH + Inputs::OWNER_GROWTH;
    for (work_limit, storage_limit) in [(Inputs::WORK - 1, peak), (Inputs::WORK, peak - 1)] {
        let (listener, root) = invalid_inputs();
        let objects = [identity(&listener), identity(&root)];
        let mut work = Work::new(work_limit);
        let mut b = Budget::new(&mut work, storage_limit);
        b.reserve_storage(floor).unwrap();
        let ledger = b.work_ledger_identity_v1();
        let result = Inputs::admit(listener, root, credentials(), &mut b);
        assert!(matches!(result, Err(Failure::Resource(_))), "{result:?}");
        assert_eq!(b.storage(), floor);
        assert!(b.work_ledger_identity_v1() == ledger);
        for object in objects {
            assert_closed(object);
        }
    }
}

#[test]
fn exact_admission_quota_reaches_root_shape_check() {
    let floor = Inputs::PAIR_STORAGE + EXTRA;
    let peak = floor + Inputs::SCRATCH + Inputs::OWNER_GROWTH;
    let (listener, root) = invalid_inputs();
    let mut work = Work::new(Inputs::WORK);
    let mut b = Budget::new(&mut work, peak);
    b.reserve_storage(floor).unwrap();
    assert!(matches!(
        Inputs::admit(listener, root, credentials(), &mut b),
        Err(Failure::Root("object is not a directory"))
    ));
    assert_eq!(b.storage(), floor);
    assert_eq!(b.peak_storage(), peak);
    assert_eq!(b.work(), Inputs::WORK);
}

#[test]
fn oversized_path_refuses_before_filesystem_queries() {
    let (listener, root) = invalid_inputs();
    let path = format!("/{}", "x".repeat(MAX_PATH_BYTES));
    let mut work = Work::new(Inputs::WORK);
    let mut b = Budget::new(&mut work, usize::MAX);
    b.reserve_storage(Inputs::PAIR_STORAGE).unwrap();
    assert!(matches!(
        Inputs::admit_at(
            listener,
            root,
            credentials(),
            Path::new(&path),
            ListenerFilesystemPolicyV1::fixture(credentials()),
            &mut b
        ),
        Err(Failure::Listener(
            "listener pathname exceeds Unix socket capacity"
        ))
    ));
    assert_eq!(b.storage(), Inputs::PAIR_STORAGE);
}

#[test]
fn valid_root_does_not_admit_a_non_socket_descriptor() {
    let f = Fixture::new("np-shape");
    let mut work = Work::new(Inputs::WORK);
    let mut b = Budget::new(&mut work, 1_000_000);
    b.reserve_storage(Inputs::PAIR_STORAGE).unwrap();
    let listener = File::open("/dev/null").unwrap().into();
    let root = File::open(&f.root).unwrap();
    assert!(matches!(
        Inputs::admit_at(
            listener,
            root,
            credentials(),
            &f.root.join("s.sock"),
            ListenerFilesystemPolicyV1::fixture(credentials()),
            &mut b
        ),
        Err(Failure::Listener(_))
    ));
    assert_eq!(b.storage(), Inputs::PAIR_STORAGE);
    assert_eq!(b.work(), Inputs::WORK);
}

fn admitted(f: &Fixture, b: &mut Budget<'_>) -> Inputs {
    let path = f.root.join("s.sock");
    let socket = bound_named_seqpacket_socket(&path);
    b.reserve_storage(Inputs::PAIR_STORAGE).unwrap();
    let floor = b.storage();
    let (owner, delta) = Inputs::admit_at(
        socket,
        File::open(&f.root).unwrap(),
        credentials(),
        &path,
        ListenerFilesystemPolicyV1::fixture(credentials()),
        b,
    )
    .unwrap();
    assert_eq!(b.storage(), floor);
    assert_eq!(delta.additional_storage(), Inputs::OWNER_GROWTH);
    b.reserve_storage(delta.additional_storage()).unwrap();
    owner
}

#[test]
fn native_pair_pins_final_files_and_refuses_retransfer_after_activation() {
    let f = Fixture::new("np-pair");
    let other = Fixture::new("np-other");
    let mut work = Work::new(20 * Inputs::WORK);
    let mut b = Budget::new(&mut work, 1_000_000);
    b.reserve_storage(EXTRA).unwrap();
    let owner = admitted(&f, &mut b);
    assert_eq!(owner.credentials(), credentials());
    assert!(!format!("{owner:?}").contains("fd:"));
    let floor = b.storage();
    let ((listener, root), delta) = owner.try_clone_ordered_for_spawn(&mut b).unwrap();
    assert_eq!(b.storage(), floor);
    assert_eq!(delta.additional_storage(), Inputs::PAIR_STORAGE);
    b.reserve_storage(delta.additional_storage()).unwrap();
    let listener = File::from(listener);
    owner
        .validate_transfer(listener.as_fd(), &root, &mut b)
        .unwrap();
    b.reserve_storage(Inputs::PAIR_STORAGE).unwrap();
    let wrong_root = File::open(&other.root).unwrap();
    assert!(matches!(
        owner.validate_transfer(listener.as_fd(), &wrong_root, &mut b),
        Err(Failure::Root("root descriptor identity changed"))
    ));
    let wrong_listener = bound_named_seqpacket_socket(&other.root.join("s.sock"));
    assert!(matches!(
        owner.validate_transfer(wrong_listener.as_fd(), &root, &mut b),
        Err(Failure::Listener(_))
    ));
    drop((wrong_root, wrong_listener));
    b.release_storage(Inputs::PAIR_STORAGE).unwrap();
    rustix::net::listen(&listener, 16).unwrap();
    owner.revalidate(&mut b).unwrap();
    owner.revalidate(&mut b).unwrap();
    assert!(matches!(
        owner.validate_transfer(listener.as_fd(), &root, &mut b),
        Err(Failure::Listener(_))
    ));
    assert!(matches!(
        owner.try_clone_ordered_for_spawn(&mut b),
        Err(Failure::Listener(_))
    ));
    drop((listener, root));
    b.release_storage(delta.additional_storage()).unwrap();
    let retained = owner.retained_storage();
    drop(owner);
    b.release_storage(retained).unwrap();
    assert_eq!(b.storage(), EXTRA);
}

#[test]
fn native_owner_clone_and_final_validation_require_complete_floors() {
    let f = Fixture::new("np-quota");
    let mut work = Work::new(20 * Inputs::WORK);
    let mut b = Budget::new(&mut work, 1_000_000);
    let owner = admitted(&f, &mut b);
    let floor = b.storage();
    let ((listener, root), delta) = owner.try_clone_ordered_for_spawn(&mut b).unwrap();
    b.reserve_storage(delta.additional_storage() - 1).unwrap();
    assert!(matches!(
        owner.validate_transfer(listener.as_fd(), &root, &mut b),
        Err(Failure::Resource(Resource::Accounting))
    ));
    b.reserve_storage(1).unwrap();
    owner
        .validate_transfer(listener.as_fd(), &root, &mut b)
        .unwrap();
    drop((listener, root));
    b.release_storage(delta.additional_storage()).unwrap();
    b.release_storage(1).unwrap();
    assert!(matches!(
        owner.revalidate(&mut b),
        Err(Failure::Resource(Resource::Accounting))
    ));
    assert!(matches!(
        owner.try_clone_ordered_for_spawn(&mut b),
        Err(Failure::Resource(Resource::Accounting))
    ));
    assert_eq!(b.storage(), floor - 1);
    b.reserve_storage(1).unwrap();
    std::fs::set_permissions(&f.root, std::fs::Permissions::from_mode(0o755)).unwrap();
    assert!(matches!(owner.revalidate(&mut b), Err(Failure::Root(_))));
    assert_eq!(b.storage(), floor);
    drop(owner);
    b.release_storage(floor).unwrap();
}

#[test]
fn clone_exact_and_one_short_quotas_use_original_account() {
    const LIMIT: usize = 4 * Inputs::WORK;
    let retained = Inputs::PAIR_STORAGE + Inputs::OWNER_GROWTH;
    let peak = retained + Inputs::SCRATCH + Inputs::PAIR_STORAGE;
    for mode in 0..3 {
        let f = Fixture::new(&format!("np-clone-{mode}"));
        let mut work = Work::new(LIMIT);
        let mut b = Budget::new(&mut work, peak - usize::from(mode == 2));
        let owner = admitted(&f, &mut b);
        let ledger = b.work_ledger_identity_v1();
        b.charge_work(LIMIT - b.work() - Inputs::WORK + usize::from(mode == 1))
            .unwrap();
        let result = owner.try_clone_ordered_for_spawn(&mut b);
        assert_eq!(b.storage(), retained);
        assert!(b.work_ledger_identity_v1() == ledger);
        match mode {
            0 => {
                let (pair, delta) = result.unwrap();
                b.reserve_storage(delta.additional_storage()).unwrap();
                drop(pair);
                b.release_storage(delta.additional_storage()).unwrap();
                assert_eq!(b.work(), LIMIT);
                assert_eq!(b.peak_storage(), peak);
            }
            1 => assert!(matches!(result, Err(Failure::Resource(Resource::Work(_))))),
            _ => {
                assert!(matches!(
                    result,
                    Err(Failure::Resource(Resource::Storage(_)))
                ));
                assert_eq!(b.failed_storage(), Some(peak));
            }
        }
        drop(owner);
        b.release_storage(retained).unwrap();
    }
}

#[test]
fn final_validation_exact_short_quotas_and_descriptor_flags() {
    const LIMIT: usize = 20 * Inputs::WORK;
    let f = Fixture::new("np-final");
    let retained = Inputs::PAIR_STORAGE + Inputs::OWNER_GROWTH;
    let floor = retained + Inputs::PAIR_STORAGE;
    let peak = floor + Inputs::SCRATCH;
    let mut work = Work::new(LIMIT);
    let mut b = Budget::new(&mut work, peak);
    let owner = admitted(&f, &mut b);
    let ((listener, root), delta) = owner.try_clone_ordered_for_spawn(&mut b).unwrap();
    b.reserve_storage(delta.additional_storage()).unwrap();
    let listener = File::from(listener);
    let ledger = b.work_ledger_identity_v1();
    b.reserve_storage(1).unwrap();
    assert!(matches!(
        owner.validate_transfer(listener.as_fd(), &root, &mut b),
        Err(Failure::Resource(Resource::Storage(_)))
    ));
    assert_eq!(b.storage(), floor + 1);
    b.release_storage(1).unwrap();
    owner
        .validate_transfer(listener.as_fd(), &root, &mut b)
        .unwrap();
    assert_eq!(b.failed_storage(), Some(peak + 1));
    for fd in [&listener, &root] {
        rustix::io::fcntl_setfd(fd, rustix::io::FdFlags::empty()).unwrap();
        assert!(
            owner
                .validate_transfer(listener.as_fd(), &root, &mut b)
                .is_err()
        );
        rustix::io::fcntl_setfd(fd, rustix::io::FdFlags::CLOEXEC).unwrap();
        owner
            .validate_transfer(listener.as_fd(), &root, &mut b)
            .unwrap();
    }
    b.charge_work(LIMIT - b.work() - Inputs::WORK + 1).unwrap();
    assert!(matches!(
        owner.validate_transfer(listener.as_fd(), &root, &mut b),
        Err(Failure::Resource(Resource::Work(_)))
    ));
    assert_eq!(b.storage(), floor);
    assert_eq!(b.failed_storage(), Some(peak + 1));
    assert_eq!(b.failed_work(), Some(LIMIT + 1));
    assert!(b.work_ledger_identity_v1() == ledger);
    drop((listener, root, owner));
    b.release_storage(floor).unwrap();
}

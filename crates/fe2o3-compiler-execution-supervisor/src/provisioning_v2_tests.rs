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

fn admitted_with_lifecycle(f: &Fixture, b: &mut Budget<'_>) -> (Inputs, Lease) {
    use fe2o3_compiler_execution_protocol::{
        COMPILER_EXECUTION_LIFECYCLE_LOCK_MODE_V1, COMPILER_EXECUTION_LIFECYCLE_LOCK_PATH_V1,
    };
    std::fs::set_permissions(&f.root, std::fs::Permissions::from_mode(0o755)).unwrap();
    let state = f.root.join("state");
    std::fs::create_dir(&state).unwrap();
    std::fs::set_permissions(&state, std::fs::Permissions::from_mode(0o700)).unwrap();
    let lock = f.root.join(
        Path::new(COMPILER_EXECUTION_LIFECYCLE_LOCK_PATH_V1)
            .file_name()
            .unwrap(),
    );
    std::fs::write(&lock, []).unwrap();
    std::fs::set_permissions(
        &lock,
        std::fs::Permissions::from_mode(COMPILER_EXECUTION_LIFECYCLE_LOCK_MODE_V1),
    )
    .unwrap();
    let root = File::open(&state).unwrap();
    b.reserve_storage(Lease::STATE_ROOT_STORAGE).unwrap();
    let (lease, charge) = Lease::open_non_authoritative_same_owner_test(&root, b).unwrap();
    b.reserve_storage(charge.additional_storage()).unwrap();
    // The same root File now becomes part of the prepaid listener/root pair.
    b.reserve_storage(Inputs::PAIR_STORAGE - Lease::STATE_ROOT_STORAGE)
        .unwrap();
    let path = state.join("s.sock");
    let socket = bound_named_seqpacket_socket(&path);
    let (inputs, charge) = Inputs::admit_at(
        socket,
        root,
        credentials(),
        &path,
        ListenerFilesystemPolicyV1::fixture(credentials()),
        b,
    )
    .unwrap();
    b.reserve_storage(charge.additional_storage()).unwrap();
    (inputs, lease)
}

#[test]
fn lifecycle_join_rejects_an_independently_valid_unrelated_lease() {
    let f = Fixture::new("np-join");
    let other = Fixture::new("np-join-other");
    let mut work = Work::new(20 * Inputs::LIFECYCLE_WORK);
    let mut b = Budget::new(&mut work, 1_000_000);
    let (inputs, lease) = admitted_with_lifecycle(&f, &mut b);
    let (other_inputs, other_lease) = admitted_with_lifecycle(&other, &mut b);
    let floor = b.storage();
    inputs.validate_lifecycle(&lease, &mut b).unwrap();
    other_inputs
        .validate_lifecycle(&other_lease, &mut b)
        .unwrap();
    assert!(matches!(
        inputs.validate_lifecycle(&other_lease, &mut b),
        Err(Failure::Lifecycle(_))
    ));
    inputs.validate_lifecycle(&lease, &mut b).unwrap();
    std::fs::set_permissions(f.root.join("state"), std::fs::Permissions::from_mode(0o755)).unwrap();
    assert!(matches!(
        inputs.validate_lifecycle(&lease, &mut b),
        Err(Failure::Root(_))
    ));
    assert_eq!(b.storage(), floor);
    drop((inputs, lease, other_inputs, other_lease));
    b.release_storage(floor).unwrap();
}

#[test]
fn lifecycle_join_exact_short_quotas_and_complete_owner_floor() {
    const LIMIT: usize = 10 * Inputs::LIFECYCLE_WORK;
    for mode in 0..4 {
        let f = Fixture::new(&format!("np-join-quota-{mode}"));
        let mut work = Work::new(LIMIT);
        // Fill the remaining storage below the known limit after admission; the
        // original ledger then has exactly (or one less than) join scratch left.
        let mut b = Budget::new(&mut work, 1_000_000);
        let (inputs, lease) = admitted_with_lifecycle(&f, &mut b);
        let owners = b.storage();
        assert_eq!(owners, inputs.retained_storage() + lease.retained_storage());
        let padding = if mode == 3 {
            0
        } else {
            1_000_000 - owners - Inputs::LIFECYCLE_SCRATCH + usize::from(mode == 2)
        };
        b.reserve_storage(padding).unwrap();
        if mode == 3 {
            b.release_storage(1).unwrap();
        }
        let floor = b.storage();
        let ledger = b.work_ledger_identity_v1();
        b.charge_work(LIMIT - b.work() - Inputs::LIFECYCLE_WORK + usize::from(mode == 1))
            .unwrap();
        let result = inputs.validate_lifecycle(&lease, &mut b);
        assert_eq!(b.storage(), floor);
        assert!(b.work_ledger_identity_v1() == ledger);
        match mode {
            0 => {
                result.unwrap();
                assert_eq!(b.work(), LIMIT);
                assert_eq!(b.peak_storage(), 1_000_000);
            }
            1 => assert!(matches!(
                result,
                Err(Failure::Lifecycle(LifecycleLeaseErrorV2::Resource(
                    Resource::Work(_)
                )))
            )),
            2 => {
                assert!(matches!(
                    result,
                    Err(Failure::Lifecycle(LifecycleLeaseErrorV2::Resource(
                        Resource::Storage(_)
                    )))
                ));
                assert_eq!(b.failed_storage(), Some(1_000_001));
            }
            _ => assert!(matches!(
                result,
                Err(Failure::Resource(Resource::Accounting))
            )),
        }
        drop((inputs, lease));
        b.release_storage(floor).unwrap();
    }
}

#[test]
fn root_only_alias_survives_activation_without_reexporting_listener() {
    let f = Fixture::new("np-root-only");
    let other = Fixture::new("np-root-wrong");
    let mut work = Work::new(30 * Inputs::WORK);
    let mut b = Budget::new(&mut work, 1_000_000);
    b.reserve_storage(EXTRA).unwrap();
    let owner = admitted(&f, &mut b);
    let ((listener, root), pair) = owner.try_clone_ordered_for_spawn(&mut b).unwrap();
    b.reserve_storage(pair.additional_storage()).unwrap();
    rustix::net::listen(&listener, 16).unwrap();
    owner.revalidate(&mut b).unwrap();
    drop((listener, root));
    b.release_storage(pair.additional_storage()).unwrap();
    let floor = b.storage();
    let ledger = b.work_ledger_identity_v1();
    let (root, delta) = owner.try_clone_root_for_spawn(&mut b).unwrap();
    assert_eq!(delta.additional_storage(), Inputs::ROOT_STORAGE);
    assert_eq!(b.storage(), floor);
    b.reserve_storage(delta.additional_storage()).unwrap();
    owner.validate_root_transfer(&root, &mut b).unwrap();
    assert!(matches!(
        owner.try_clone_ordered_for_spawn(&mut b),
        Err(Failure::Listener(_))
    ));
    b.reserve_storage(Inputs::ROOT_STORAGE).unwrap();
    let wrong = File::open(&other.root).unwrap();
    assert!(matches!(
        owner.validate_root_transfer(&wrong, &mut b),
        Err(Failure::Root("root descriptor identity changed"))
    ));
    drop(wrong);
    b.release_storage(Inputs::ROOT_STORAGE).unwrap();
    rustix::io::fcntl_setfd(&root, rustix::io::FdFlags::empty()).unwrap();
    assert!(matches!(
        owner.validate_root_transfer(&root, &mut b),
        Err(Failure::Root(_))
    ));
    rustix::io::fcntl_setfd(&root, rustix::io::FdFlags::CLOEXEC).unwrap();
    owner.validate_root_transfer(&root, &mut b).unwrap();
    std::fs::remove_file(f.root.join("s.sock")).unwrap();
    assert!(owner.validate_root_transfer(&root, &mut b).is_err());
    assert!(owner.try_clone_root_for_spawn(&mut b).is_err());
    assert!(b.work_ledger_identity_v1() == ledger);
    drop(root);
    b.release_storage(delta.additional_storage()).unwrap();
    let retained = owner.retained_storage();
    drop(owner);
    b.release_storage(retained).unwrap();
    assert_eq!(b.storage(), EXTRA);
}

#[test]
fn root_only_alias_exact_short_and_unwind_keep_original_funding() {
    const LIMIT: usize = 20 * Inputs::WORK;
    for mode in 0..5 {
        let f = Fixture::new(&format!("np-root-quota-{mode}"));
        let mut work = Work::new(LIMIT);
        let mut b = Budget::new(&mut work, 1_000_000);
        let owner = admitted(&f, &mut b);
        let retained = owner.retained_storage();
        let padding = if mode == 3 {
            0
        } else {
            b.storage_limit() - retained - Inputs::SCRATCH - Inputs::ROOT_STORAGE
                + usize::from(mode == 2)
        };
        b.reserve_storage(padding).unwrap();
        if mode == 3 {
            b.release_storage(1).unwrap();
        }
        let floor = b.storage();
        let ledger = b.work_ledger_identity_v1();
        if mode <= 1 {
            b.charge_work(LIMIT - b.work() - Inputs::WORK + usize::from(mode == 1))
                .unwrap();
        }
        let result = owner.try_clone_root_for_spawn(&mut b);
        assert_eq!(b.storage(), floor);
        assert!(b.work_ledger_identity_v1() == ledger);
        match mode {
            0 | 4 => {
                let (root, delta) = result.unwrap();
                assert_eq!(b.peak_storage(), 1_000_000);
                b.reserve_storage(delta.additional_storage()).unwrap();
                let before = b.work();
                if mode == 4 {
                    let failure = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                        let result: Result<()> = b.with_prepaid_scope(
                            retained + Inputs::ROOT_STORAGE,
                            ENTRY,
                            ENTRY,
                            0,
                            |b| {
                                owner.validate_root_transfer(&root, b)?;
                                panic!("root alias scope unwind");
                            },
                        );
                        result
                    }));
                    assert!(failure.is_err());
                    assert_eq!(b.work(), before + ENTRY + Inputs::WORK);
                } else {
                    assert_eq!(b.work(), LIMIT);
                }
                assert_eq!(b.storage(), floor + Inputs::ROOT_STORAGE);
                drop(root);
                b.release_storage(delta.additional_storage()).unwrap();
            }
            1 => assert!(matches!(result, Err(Failure::Resource(Resource::Work(_))))),
            2 => {
                assert!(matches!(
                    result,
                    Err(Failure::Resource(Resource::Storage(_)))
                ));
                assert_eq!(b.failed_storage(), Some(1_000_001));
            }
            _ => assert!(matches!(
                result,
                Err(Failure::Resource(Resource::Accounting))
            )),
        }
        drop(owner);
        b.release_storage(floor).unwrap();
    }
}

#[test]
fn root_only_final_alias_requires_its_full_retained_charge() {
    let f = Fixture::new("np-root-floor");
    let mut work = Work::new(10 * Inputs::WORK);
    let mut b = Budget::new(&mut work, 1_000_000);
    let owner = admitted(&f, &mut b);
    let (root, delta) = owner.try_clone_root_for_spawn(&mut b).unwrap();
    b.reserve_storage(delta.additional_storage() - 1).unwrap();
    let floor = b.storage();
    assert!(matches!(
        owner.validate_root_transfer(&root, &mut b),
        Err(Failure::Resource(Resource::Accounting))
    ));
    assert_eq!(b.storage(), floor);
    b.reserve_storage(1).unwrap();
    owner.validate_root_transfer(&root, &mut b).unwrap();
    drop((root, owner));
    b.release_storage(floor + 1).unwrap();
}

fn connect_root(f: &Fixture) -> OwnedFd {
    let client = rustix::net::socket_with(
        rustix::net::AddressFamily::UNIX,
        rustix::net::SocketType::SEQPACKET,
        rustix::net::SocketFlags::CLOEXEC,
        None,
    )
    .unwrap();
    let address = rustix::net::SocketAddrUnix::new(&f.root.join("s.sock")).unwrap();
    rustix::net::connect(&client, &address).unwrap();
    client
}

#[test]
fn original_root_listener_accepts_without_a_listener_alias() {
    let f = Fixture::new("np-root-listen");
    let mut work = Work::new(20 * Inputs::WORK);
    let mut b = Budget::new(&mut work, 1_000_000);
    let mut owner = admitted(&f, &mut b);
    assert!(matches!(
        owner.try_accept_original_root(&mut b),
        Err(Failure::Listener(_))
    ));
    owner.activate_original_root_listener(&mut b).unwrap();
    assert!(owner.activate_original_root_listener(&mut b).is_err());
    assert!(owner.try_clone_ordered_for_spawn(&mut b).is_err());
    assert!(owner.try_accept_original_root(&mut b).unwrap().is_none());
    let client = connect_root(&f);
    // Queue the first record before accept: PASSCRED must precede listening.
    rustix::net::send(&client, b"inert", rustix::net::SendFlags::NOSIGNAL).unwrap();
    let retained = owner.retained_storage();
    let (connection, delta) = owner.try_accept_original_root(&mut b).unwrap().unwrap();
    assert_eq!(b.storage(), retained);
    assert_eq!(delta.additional_storage(), Inputs::CONNECTION_STORAGE);
    b.reserve_storage(delta.additional_storage()).unwrap();
    assert!(rustix::net::sockopt::socket_passcred(&connection).unwrap());
    assert_eq!(
        rustix::io::fcntl_getfd(&connection).unwrap(),
        rustix::io::FdFlags::CLOEXEC
    );
    assert!(
        rustix::fs::fcntl_getfl(&connection)
            .unwrap()
            .contains(rustix::fs::OFlags::NONBLOCK)
    );
    let peer = rustix::net::sockopt::socket_peercred(&connection).unwrap();
    assert_eq!(peer.pid, rustix::process::getpid());
    let mut bytes = [0; 5];
    let mut iov = [std::io::IoSliceMut::new(&mut bytes)];
    let mut space = [std::mem::MaybeUninit::uninit(); rustix::cmsg_space!(ScmCredentials(1))];
    let mut ancillary = rustix::net::RecvAncillaryBuffer::new(&mut space);
    let received = rustix::net::recvmsg(
        &connection,
        &mut iov,
        &mut ancillary,
        rustix::net::RecvFlags::CMSG_CLOEXEC,
    )
    .unwrap();
    assert_eq!(received.bytes, 5);
    assert!(
        !received
            .flags
            .intersects(rustix::net::ReturnFlags::TRUNC | rustix::net::ReturnFlags::CTRUNC)
    );
    let mut credentials = 0;
    for message in ancillary.drain() {
        if let rustix::net::RecvAncillaryMessage::ScmCredentials(sender) = message {
            assert_eq!(sender.pid, peer.pid);
            assert_eq!(sender.uid, peer.uid);
            assert_eq!(sender.gid, peer.gid);
            credentials += 1;
        } else {
            panic!("unexpected ancillary");
        }
    }
    assert_eq!(credentials, 1);
    assert_eq!(&bytes, b"inert");
    assert!(owner.try_accept_original_root(&mut b).unwrap().is_none());
    drop((client, connection));
    b.release_storage(delta.additional_storage()).unwrap();
    drop(owner);
    b.release_storage(retained).unwrap();
}

#[test]
fn exported_listener_cannot_become_original_root_intake_even_after_alias_drop() {
    for activate in [false, true] {
        let f = Fixture::new(if activate {
            "np-indirect"
        } else {
            "np-exported"
        });
        let mut work = Work::new(20 * Inputs::WORK);
        let mut b = Budget::new(&mut work, 1_000_000);
        let mut owner = admitted(&f, &mut b);
        let ((listener, root), delta) = owner.try_clone_ordered_for_spawn(&mut b).unwrap();
        b.reserve_storage(delta.additional_storage()).unwrap();
        if activate {
            rustix::net::listen(&listener, 16).unwrap();
        }
        drop((listener, root));
        b.release_storage(delta.additional_storage()).unwrap();
        owner.revalidate(&mut b).unwrap();
        assert!(matches!(
            owner.activate_original_root_listener(&mut b),
            Err(Failure::Listener(
                "listener was exported for indirect activation"
            ))
        ));
        assert!(matches!(
            owner.try_accept_original_root(&mut b),
            Err(Failure::Listener("not activated by original root"))
        ));
        let retained = owner.retained_storage();
        drop(owner);
        b.release_storage(retained).unwrap();
    }
}

#[test]
fn unfunded_root_accept_does_not_dequeue_a_connection_or_change_account() {
    let f = Fixture::new("np-accept-fund");
    let mut work = Work::new(20 * Inputs::WORK);
    let mut b = Budget::new(&mut work, 1_000_000);
    let mut owner = admitted(&f, &mut b);
    owner.activate_original_root_listener(&mut b).unwrap();
    let client = connect_root(&f);
    let retained = owner.retained_storage();
    let padding = b.storage_limit() - retained - Inputs::SCRATCH - Inputs::CONNECTION_STORAGE;
    b.reserve_storage(padding + 1).unwrap();
    let floor = b.storage();
    let ledger = b.work_ledger_identity_v1();
    assert!(matches!(
        owner.try_accept_original_root(&mut b),
        Err(Failure::Resource(Resource::Storage(_)))
    ));
    assert_eq!(b.storage(), floor);
    assert_eq!(b.failed_storage(), Some(1_000_001));
    b.release_storage(1).unwrap();
    let (connection, delta) = owner.try_accept_original_root(&mut b).unwrap().unwrap();
    assert_eq!(b.storage(), floor - 1);
    assert_eq!(b.peak_storage(), 1_000_000);
    assert_eq!(b.failed_storage(), Some(1_000_001));
    assert!(b.work_ledger_identity_v1() == ledger);
    b.reserve_storage(delta.additional_storage()).unwrap();
    drop((client, connection));
    b.release_storage(delta.additional_storage()).unwrap();
    drop(owner);
    b.release_storage(retained + padding).unwrap();
}

#[test]
fn root_accept_exact_and_one_short_work_do_not_replace_the_ledger() {
    const LIMIT: usize = 20 * Inputs::WORK;
    for short in [false, true] {
        let f = Fixture::new(if short {
            "np-accept-short"
        } else {
            "np-accept-exact"
        });
        let mut work = Work::new(LIMIT);
        let mut b = Budget::new(&mut work, 1_000_000);
        let mut owner = admitted(&f, &mut b);
        owner.activate_original_root_listener(&mut b).unwrap();
        let client = connect_root(&f);
        b.charge_work(LIMIT - b.work() - Inputs::WORK + usize::from(short))
            .unwrap();
        let floor = b.storage();
        let ledger = b.work_ledger_identity_v1();
        let result = owner.try_accept_original_root(&mut b);
        assert_eq!(b.storage(), floor);
        assert!(b.work_ledger_identity_v1() == ledger);
        if short {
            assert!(matches!(result, Err(Failure::Resource(Resource::Work(_)))));
            assert_eq!(b.failed_work(), Some(LIMIT + 1));
            // The pure transport observation sees the still-queued connection;
            // no second Budget or refund is used to recover the failed operation.
            drop(owner.listener.try_accept_original_root().unwrap().unwrap());
        } else {
            let (connection, delta) = result.unwrap().unwrap();
            b.reserve_storage(delta.additional_storage()).unwrap();
            assert_eq!(b.work(), LIMIT);
            drop(connection);
            b.release_storage(delta.additional_storage()).unwrap();
        }
        drop((client, owner));
        b.release_storage(floor).unwrap();
    }
}

#[test]
fn outer_pair_failure_or_unwind_does_not_reset_original_listener_export() {
    for unwind in [false, true] {
        let f = Fixture::new(if unwind {
            "np-export-unwind"
        } else {
            "np-export-denial"
        });
        let mut work = Work::new(20 * Inputs::WORK);
        let mut b = Budget::new(&mut work, 1_000_000);
        let mut owner = admitted(&f, &mut b);
        let floor = b.storage();
        let ledger = b.work_ledger_identity_v1();
        let caught = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            b.with_prepaid_scope::<(), Failure>(floor, ENTRY, ENTRY, 0, |b| {
                let (pair, delta) = owner.try_clone_ordered_for_spawn(b)?;
                b.reserve_storage(delta.additional_storage())?;
                assert!(!unwind, "injected outer pair unwind");
                b.reserve_storage(b.storage_limit())?;
                drop(pair);
                Ok(())
            })
        }));
        if unwind {
            assert!(caught.is_err());
        } else {
            assert!(matches!(
                caught.unwrap(),
                Err(Failure::Resource(Resource::Storage(_)))
            ));
        }
        assert_eq!(b.storage(), floor);
        assert!(b.work_ledger_identity_v1() == ledger);
        let denied = b.failed_storage();
        assert!(matches!(
            owner.activate_original_root_listener(&mut b),
            Err(Failure::Listener(
                "listener was exported for indirect activation"
            ))
        ));
        assert_eq!(b.failed_storage(), denied);
        drop(owner);
        b.release_storage(floor).unwrap();
    }
}

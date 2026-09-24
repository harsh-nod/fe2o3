use super::super::tests::{
    accepted_named_connection, nonblocking_seqpacket as pair, pidfd_for, receive_descriptor,
    send_descriptor,
};
use super::*;
use fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1 as Work;
use rustix::{
    event::{PollFd, PollFlags, Timespec, poll},
    fs::OFlags,
    net::{AddressFamily, SocketAddrUnix, SocketFlags, SocketType, socket_with, socketpair},
};
use std::{
    fs::{self, File},
    os::{
        fd::AsRawFd,
        unix::{
            fs::{MetadataExt, PermissionsExt},
            process::CommandExt,
        },
    },
    process::{Child, Command, ExitStatus, Stdio},
    time::{Duration, Instant},
};

const EXTRA: usize = 19;
const ROLE: &str = "FE2O3_NATIVE_SERVICE_TEST_ROLE";
const OPT_IN: &str = "FE2O3_RUN_PRIVILEGED_NATIVE_SERVICE_TEST";
const CHILD_TEST: &str = "linux::service_native::tests::native_client_helper";

struct ChildGuard(Child);
impl ChildGuard {
    fn wait(&mut self) -> std::io::Result<ExitStatus> {
        let deadline = Instant::now() + Duration::from_secs(5);
        loop {
            if let Some(status) = self.0.try_wait()? {
                return Ok(status);
            }
            if Instant::now() >= deadline {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::TimedOut,
                    "native service fixture child timed out",
                ));
            }
            std::thread::sleep(Duration::from_millis(5));
        }
    }
}
impl Drop for ChildGuard {
    fn drop(&mut self) {
        if matches!(self.0.try_wait(), Ok(Some(_))) {
            return;
        }
        let _ = self.0.kill();
        if let Err(error) = self.wait() {
            eprintln!("native service child cleanup: {error}");
        }
    }
}

fn readable(fd: &OwnedFd) {
    let mut events = [PollFd::new(fd, PollFlags::IN)];
    assert_eq!(
        poll(
            &mut events,
            Some(&Timespec {
                tv_sec: 15,
                tv_nsec: 0
            })
        )
        .unwrap(),
        1
    );
    assert!(events[0].revents().contains(PollFlags::IN));
}

#[test]
#[ignore = "private client role spawned only by native service custody fixtures"]
fn native_client_helper() {
    let role = std::env::var(ROLE).expect("explicit native service client role required");
    assert!(matches!(role.as_str(), "same-uid" | "distinct-uid"));
    if role == "distinct-uid" {
        assert_eq!(rustix::process::geteuid().as_raw(), 65_534);
        assert_eq!(rustix::process::getegid().as_raw(), 65_534);
    }
    // Stdio owns stdin; this local CLOEXEC duplicate needs no raw-FD adoption.
    let control = rustix::io::fcntl_dupfd_cloexec(std::io::stdin(), 3).unwrap();
    let (endpoint, held_peer) = pair();
    send_descriptor(control.as_raw_fd(), endpoint.as_raw_fd()).unwrap();
    drop(endpoint);
    readable(&control);
    let mut ack = [0];
    assert_eq!(rustix::io::read(&control, &mut ack).unwrap(), 1);
    assert_eq!(ack, [0x51]);
    drop(held_peer);
}

fn protected_root() -> (tempfile::TempDir, OwnedFd) {
    let directory = tempfile::tempdir().unwrap();
    fs::set_permissions(directory.path(), fs::Permissions::from_mode(0o700)).unwrap();
    let root = File::open(directory.path()).unwrap().into();
    (directory, root)
}

struct Fixture {
    // Terminate/reap before closing control, avoiding a spurious child EOF panic.
    child: ChildGuard,
    directory: tempfile::TempDir,
    root: OwnedFd,
    peer: OwnedFd,
    control: OwnedFd,
    expected: ExpectedClientProcessIdentityV1,
}
impl Fixture {
    fn new() -> Self {
        Self::spawn(false)
    }
    fn spawn(distinct: bool) -> Self {
        let (control, child_control) = pair();
        let mut command = Command::new(std::env::current_exe().unwrap());
        command
            .args([
                "--exact",
                CHILD_TEST,
                "--ignored",
                "--nocapture",
                "--test-threads=1",
            ])
            .env(ROLE, if distinct { "distinct-uid" } else { "same-uid" })
            .env_remove(OPT_IN)
            .stdin(Stdio::from(child_control))
            .stdout(Stdio::null())
            .stderr(Stdio::inherit());
        if distinct {
            // Only the explicitly opted-in container fixture selects this branch.
            // Command performs child-local credential changes, never pre_exec.
            command
                .uid(65_534)
                .gid(65_534)
                .current_dir("/tmp")
                .env("TMPDIR", "/tmp");
        }
        let child = ChildGuard(crate::test_process_execution::spawn(&mut command).unwrap());
        drop(command);
        readable(&control);
        let peer = receive_descriptor(control.as_raw_fd()).unwrap();
        let expected = ExpectedClientProcessIdentityV1::new(
            child.0.id(),
            if distinct {
                65_534
            } else {
                rustix::process::geteuid().as_raw()
            },
            if distinct {
                65_534
            } else {
                rustix::process::getegid().as_raw()
            },
        )
        .unwrap();
        let (directory, root) = protected_root();
        Self {
            directory,
            root,
            peer,
            control,
            child,
            expected,
        }
    }
    fn inputs(&self) -> (OwnedFd, OwnedFd, Client) {
        (
            duplicate(&self.root),
            duplicate(&self.peer),
            client(self.expected),
        )
    }
    fn admitted(&self) -> Service {
        let (root, peer, client) = self.inputs();
        run(Service::INPUT_STORAGE, Service::ADMISSION_WORK, |budget| {
            Service::admit_inner::<false>(root, peer, client, budget)
        })
        .unwrap()
        .0
    }
    fn finish(&mut self) {
        assert_eq!(rustix::io::write(&self.control, &[0x51]).unwrap(), 1);
        assert!(self.child.wait().unwrap().success());
    }
}

fn duplicate(fd: &impl AsFd) -> OwnedFd {
    rustix::io::fcntl_dupfd_cloexec(fd, 3).unwrap()
}

fn client(expected: ExpectedClientProcessIdentityV1) -> Client {
    let mut work = Work::new(Client::ADMISSION_WORK);
    let mut budget = Budget::new(&mut work, Client::FD_STORAGE + Client::IO_STORAGE);
    budget.reserve_storage(Client::FD_STORAGE).unwrap();
    let (client, delta) = Client::admit(pidfd_for(expected.pid()), expected, &mut budget).unwrap();
    budget.reserve_storage(delta.additional_storage()).unwrap();
    client
}

fn run<T>(
    floor: usize,
    work: usize,
    operation: impl FnOnce(&mut Budget<'_>) -> Result<T>,
) -> Result<T> {
    let mut meter = Work::new(work);
    let mut budget = Budget::new(&mut meter, floor + EXTRA + Service::IO_STORAGE);
    budget.reserve_storage(floor + EXTRA).unwrap();
    let result = operation(&mut budget);
    assert_eq!(budget.storage(), floor + EXTRA);
    assert!(budget.work() <= work);
    result
}

fn revalidate(service: &Service) -> Result<()> {
    run(
        service.retained_storage(),
        Service::REVALIDATION_WORK,
        |budget| service.validate_continuity(budget),
    )
}

fn references(fd: &OwnedFd) -> usize {
    let stat = rustix::fs::fstat(fd).unwrap();
    fs::read_dir("/proc/self/fd")
        .unwrap()
        .filter_map(|entry| fs::metadata(entry.ok()?.path()).ok())
        .filter(|meta| (meta.dev(), meta.ino()) == (stat.st_dev, stat.st_ino))
        .count()
}

fn pidfd_references(pid: u32) -> usize {
    // Unique fixture child, including on kernels sharing pidfd anon-inodes.
    fs::read_dir("/proc/self/fdinfo")
        .unwrap()
        .filter_map(|entry| {
            let record = fs::read_to_string(entry.ok()?.path()).ok()?;
            checks::parse_pidfd_fdinfo(&record).ok()
        })
        .filter(|actual| *actual == pid)
        .count()
}

#[test]
fn fixed_schedule_and_storage_are_independently_bounded() {
    assert_eq!(Service::ADMISSION_WORK, 36_184_216);
    assert_eq!(Service::REVALIDATION_WORK, 36_184_216);
    assert_eq!(
        Service::INPUT_STORAGE,
        2 * size_of::<(OwnedFd, Storage)>()
            + size_of::<(Client, super::super::LiveClientPidfdStorageV2)>()
    );
    assert_eq!(
        Service::IO_STORAGE,
        8 * size_of::<(Service, Storage)>()
            + 4 * Service::FD_PAIR_STORAGE
            + 2 * 4097
            + 8192
            + Client::IO_STORAGE
    );
    assert!(size_of::<Error>() <= 128);
    assert_eq!(size_of::<Storage>(), size_of::<usize>());
}

#[test]
fn one_ledger_retains_exact_inputs_and_releases_only_after_owner_drop() {
    let mut fixture = Fixture::new();
    let mut meter = Work::new(
        EXTRA + Client::ADMISSION_WORK + Service::ADMISSION_WORK + Service::REVALIDATION_WORK,
    );
    let mut budget = Budget::new(&mut meter, EXTRA + Service::RETAINED + Service::IO_STORAGE);
    budget.charge_work(EXTRA).unwrap();
    budget
        .reserve_storage(EXTRA + Service::FD_PAIR_STORAGE + Client::FD_STORAGE)
        .unwrap();
    let ledger = budget.work_ledger_identity_v1();
    let (client, delta) = Client::admit(
        pidfd_for(fixture.expected.pid()),
        fixture.expected,
        &mut budget,
    )
    .unwrap();
    budget.reserve_storage(delta.additional_storage()).unwrap();
    let client_identity = client.process_identity();
    let (root, peer) = (duplicate(&fixture.root), duplicate(&fixture.peer));
    let raw = (
        root.as_raw_fd(),
        peer.as_raw_fd(),
        client.pidfd().as_raw_fd(),
    );
    let (service, delta) = Service::admit_inner::<false>(root, peer, client, &mut budget).unwrap();
    assert!(ledger == budget.work_ledger_identity_v1());
    assert_eq!(budget.storage(), EXTRA + Service::INPUT_STORAGE);
    assert_eq!(
        delta.additional_storage(),
        service.retained_storage() - Service::INPUT_STORAGE
    );
    budget.reserve_storage(delta.additional_storage()).unwrap();
    assert_eq!(
        (
            service.root.as_raw_fd(),
            service.service_peer().as_raw_fd(),
            service.client_pidfd().as_raw_fd()
        ),
        raw
    );
    assert_eq!(service.client_process_identity(), client_identity);
    assert_eq!(service.expected_client(), fixture.expected);
    assert!(client_identity.1 > 0);
    service.validate_continuity(&mut budget).unwrap();
    assert_eq!(
        budget.work(),
        EXTRA + Client::ADMISSION_WORK + Service::ADMISSION_WORK + Service::REVALIDATION_WORK
    );
    assert_eq!(
        budget.peak_storage(),
        EXTRA + Service::RETAINED + Service::IO_STORAGE
    );
    assert_eq!(
        (
            references(&fixture.root),
            references(&fixture.peer),
            pidfd_references(fixture.expected.pid())
        ),
        (2, 2, 1)
    );
    let retained = service.retained_storage();
    drop(service);
    budget.release_storage(retained).unwrap();
    assert_eq!(budget.storage(), EXTRA);
    assert_eq!(
        (
            references(&fixture.root),
            references(&fixture.peer),
            pidfd_references(fixture.expected.pid())
        ),
        (1, 1, 0)
    );
    fixture.finish();
}

fn boundary<T>(floor: usize, case: usize, operation: impl FnOnce(&mut Budget<'_>) -> Result<T>) {
    let prepaid = if case == 0 { floor - 1 } else { floor + EXTRA };
    let work_limit = match case {
        1 => native::ENTRY_WORK - 1,
        2 => Service::ADMISSION_WORK - 1,
        5 => Service::OUTER_WORK - 1,
        _ => Service::ADMISSION_WORK,
    };
    let storage_limit = match case {
        3 => prepaid + Service::IO_STORAGE - 1,
        6 => prepaid + Service::OUTER_STORAGE - 1,
        _ => prepaid + Service::IO_STORAGE,
    };
    let mut meter = Work::new(work_limit);
    let mut budget = Budget::new(&mut meter, storage_limit);
    budget.reserve_storage(prepaid).unwrap();
    let result = operation(&mut budget);
    assert_eq!(budget.storage(), prepaid);
    if case == 4 {
        assert!(result.is_ok());
        assert_eq!(budget.work(), work_limit);
        assert_eq!(budget.peak_storage(), storage_limit);
        assert_eq!(budget.failed_storage(), None);
        return;
    }
    let error = result.err().expect("expected budget refusal");
    assert_eq!((error.kind(), error.errno()), (None, None));
    match case {
        0 => {
            assert_eq!(error.resource(), Some(Resource::Accounting));
            assert_eq!(budget.work(), native::ENTRY_WORK);
            assert_eq!(budget.peak_storage(), prepaid);
        }
        1 | 2 | 5 => {
            assert!(matches!(error.resource(), Some(Resource::Work(_))));
            let used = match case {
                1 => 0,
                2 => Service::OUTER_WORK + Client::REVALIDATION_WORK + native::ENTRY_WORK,
                _ => native::ENTRY_WORK,
            };
            assert_eq!(budget.work(), used);
            assert_eq!(
                budget.peak_storage(),
                if case == 2 {
                    prepaid + Service::IO_STORAGE
                } else {
                    prepaid
                }
            );
        }
        3 | 6 => {
            assert!(matches!(error.resource(), Some(Resource::Storage(_))));
            assert_eq!(
                budget.work(),
                Service::OUTER_WORK
                    + if case == 3 {
                        Client::REVALIDATION_WORK
                    } else {
                        0
                    }
            );
            assert_eq!(budget.failed_storage(), Some(storage_limit + 1));
            assert_eq!(
                budget.peak_storage(),
                prepaid + if case == 3 { Service::OUTER_STORAGE } else { 0 }
            );
        }
        _ => unreachable!(),
    }
    assert_eq!(
        meter.failed_work(),
        match case {
            1 => Some(native::ENTRY_WORK),
            2 => Some(Service::ADMISSION_WORK),
            5 => Some(Service::OUTER_WORK),
            _ => None,
        }
    );
}

#[test]
fn exact_and_one_short_budgets_close_consumed_inputs_and_preserve_borrowed_custody() {
    let fixture = Fixture::new();
    let service = fixture.admitted();
    let baseline = (
        references(&fixture.root),
        references(&fixture.peer),
        pidfd_references(fixture.expected.pid()),
    );
    for case in 0..7 {
        let (root, peer, client) = fixture.inputs();
        boundary(Service::INPUT_STORAGE, case, |budget| {
            Service::admit_inner::<false>(root, peer, client, budget)
        });
        assert_eq!(
            (
                references(&fixture.root),
                references(&fixture.peer),
                pidfd_references(fixture.expected.pid())
            ),
            baseline
        );
        boundary(service.retained_storage(), case, |budget| {
            service.validate_continuity(budget)
        });
        assert_eq!(
            (
                references(&fixture.root),
                references(&fixture.peer),
                pidfd_references(fixture.expected.pid())
            ),
            baseline
        );
    }
    revalidate(&service).unwrap();
}

#[test]
fn cumulative_exhaustion_does_not_reset_for_the_next_service_check() {
    let fixture = Fixture::new();
    let service = fixture.admitted();
    let mut meter = Work::new(Service::REVALIDATION_WORK);
    let mut budget = Budget::new(&mut meter, Service::RETAINED + Service::IO_STORAGE);
    budget.reserve_storage(Service::RETAINED).unwrap();
    service.validate_continuity(&mut budget).unwrap();
    let peak = budget.peak_storage();
    let error = service.validate_continuity(&mut budget).unwrap_err();
    assert!(matches!(error.resource(), Some(Resource::Work(_))));
    assert_eq!(budget.work(), Service::REVALIDATION_WORK);
    assert_eq!(budget.storage(), Service::RETAINED);
    assert_eq!(budget.peak_storage(), peak);
}

#[test]
fn unwind_closes_consumed_inputs_restores_storage_and_keeps_work() {
    let fixture = Fixture::new();
    let (root, peer, client) = fixture.inputs();
    let mut work = Work::new(Service::ADMISSION_WORK);
    let mut budget = Budget::new(
        &mut work,
        EXTRA + Service::INPUT_STORAGE + Service::IO_STORAGE,
    );
    budget
        .reserve_storage(EXTRA + Service::INPUT_STORAGE)
        .unwrap();
    let panic = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        Service::scope::<()>(&mut budget, Service::INPUT_STORAGE, move |budget| {
            let _inputs = (root, peer);
            client.validate_liveness(budget)?;
            panic!("injected inside the original funded custody frame");
        })
    }));
    assert!(panic.is_err());
    assert_eq!(budget.storage(), EXTRA + Service::INPUT_STORAGE);
    assert_eq!(
        budget.work(),
        Service::OUTER_WORK + Client::REVALIDATION_WORK
    );
    assert_eq!(
        budget.peak_storage(),
        EXTRA + Service::INPUT_STORAGE + Service::IO_STORAGE
    );
    assert_eq!(
        (
            references(&fixture.root),
            references(&fixture.peer),
            pidfd_references(fixture.expected.pid())
        ),
        (1, 1, 0)
    );
}

#[test]
fn production_never_accepts_the_same_uid_fixture_bypass() {
    let fixture = Fixture::new();
    let (root, peer, client) = fixture.inputs();
    let error = run(Service::INPUT_STORAGE, Service::ADMISSION_WORK, |b| {
        Service::admit(root, peer, client, b)
    })
    .unwrap_err();
    assert_eq!(error.kind(), Some(Kind::SameUidClient));
    let mut service = fixture.admitted();
    service.same_uid_fixture = false;
    assert_eq!(
        revalidate(&service).unwrap_err().kind(),
        Some(Kind::SameUidClient)
    );
}

fn refuses_input(fixture: &Fixture, root: OwnedFd, peer: OwnedFd, kind: Kind) {
    let client = client(fixture.expected);
    let error = run(Service::INPUT_STORAGE, Service::ADMISSION_WORK, |b| {
        Service::admit(root, peer, client, b)
    })
    .unwrap_err();
    assert_eq!(error.kind(), Some(kind));
    assert_eq!(error.resource(), None);
}

#[test]
fn root_shape_mode_and_link_predicates_refuse_independently() {
    let fixture = Fixture::new();
    refuses_input(
        &fixture,
        tempfile::tempfile().unwrap().into(),
        duplicate(&fixture.peer),
        Kind::RootNotDirectory,
    );
    for mode in [0o000, 0o600, 0o710, 0o777, 0o1700, 0o2700, 0o4700] {
        fs::set_permissions(fixture.directory.path(), fs::Permissions::from_mode(mode)).unwrap();
        refuses_input(
            &fixture,
            duplicate(&fixture.root),
            duplicate(&fixture.peer),
            Kind::RootMode,
        );
    }
    fs::set_permissions(fixture.directory.path(), fs::Permissions::from_mode(0o700)).unwrap();
    let identity = checks::inspect_object(&fixture.root, Kind::InspectRoot, "root").unwrap();
    assert_eq!(
        require_root_identity(
            ObjectIdentityV1 {
                uid: identity.uid ^ 1,
                ..identity
            },
            identity.uid
        )
        .unwrap_err()
        .kind(),
        Kind::RootOwner
    );
    fs::remove_dir(fixture.directory.path()).unwrap();
    refuses_input(
        &fixture,
        duplicate(&fixture.root),
        duplicate(&fixture.peer),
        Kind::RootUnlinked,
    );
}

#[test]
fn every_cloexec_role_is_checked_on_admission_and_revalidation() {
    let fixture = Fixture::new();
    for (role, kind) in [
        (0, Kind::RootCloseOnExec),
        (1, Kind::PeerCloseOnExec),
        (2, Kind::ClientPidfdCloseOnExec),
    ] {
        let (root, peer, client) = fixture.inputs();
        let selected = match role {
            0 => root.as_fd(),
            1 => peer.as_fd(),
            _ => client.pidfd(),
        };
        rustix::io::fcntl_setfd(selected, rustix::io::FdFlags::empty()).unwrap();
        let error = run(Service::INPUT_STORAGE, Service::ADMISSION_WORK, |b| {
            Service::admit(root, peer, client, b)
        })
        .unwrap_err();
        assert_eq!(error.kind(), Some(kind));
        let service = fixture.admitted();
        let selected = match role {
            0 => service.root.as_fd(),
            1 => service.service_peer(),
            _ => service.client_pidfd(),
        };
        rustix::io::fcntl_setfd(selected, rustix::io::FdFlags::empty()).unwrap();
        assert_eq!(revalidate(&service).unwrap_err().kind(), Some(kind));
    }
}

#[test]
fn peer_requires_unix_connected_unnamed_seqpacket_and_exact_nonblocking_status() {
    let fixture = Fixture::new();
    for (domain, kind, expected) in [
        (AddressFamily::INET, SocketType::STREAM, Kind::PeerDomain),
        (
            AddressFamily::UNIX,
            SocketType::STREAM,
            Kind::PeerSocketType,
        ),
        (
            AddressFamily::UNIX,
            SocketType::SEQPACKET,
            Kind::PeerNotConnected,
        ),
    ] {
        let peer = socket_with(
            domain,
            kind,
            SocketFlags::CLOEXEC | SocketFlags::NONBLOCK,
            None,
        )
        .unwrap();
        refuses_input(&fixture, duplicate(&fixture.root), peer, expected);
    }
    refuses_input(
        &fixture,
        duplicate(&fixture.root),
        tempfile::tempfile().unwrap().into(),
        Kind::PeerDomain,
    );
    let names = tempfile::tempdir().unwrap();
    let server = SocketAddrUnix::new(names.path().join("server")).unwrap();
    let (peer, held) = accepted_named_connection(&server, None);
    refuses_input(
        &fixture,
        duplicate(&fixture.root),
        peer,
        Kind::PeerLocalAddress,
    );
    refuses_input(
        &fixture,
        duplicate(&fixture.root),
        held,
        Kind::PeerRemoteAddress,
    );
    let (peer, _held) = socketpair(
        AddressFamily::UNIX,
        SocketType::SEQPACKET,
        SocketFlags::CLOEXEC,
        None,
    )
    .unwrap();
    refuses_input(
        &fixture,
        duplicate(&fixture.root),
        peer,
        Kind::PeerStatusFlags,
    );
    let service = fixture.admitted();
    rustix::fs::fcntl_setfl(&fixture.peer, OFlags::RDWR).unwrap();
    assert_eq!(
        revalidate(&service).unwrap_err().kind(),
        Some(Kind::PeerStatusFlags)
    );
}

#[test]
fn distinct_object_roles_are_required_even_with_distinct_descriptor_numbers() {
    let fixture = Fixture::new();
    refuses_input(
        &fixture,
        duplicate(&fixture.root),
        duplicate(&fixture.root),
        Kind::DuplicateDescriptors,
    );
    let client = client(fixture.expected);
    let peer = duplicate(&client.pidfd());
    let error = run(Service::INPUT_STORAGE, Service::ADMISSION_WORK, |b| {
        Service::admit(duplicate(&fixture.root), peer, client, b)
    })
    .unwrap_err();
    assert_eq!(error.kind(), Some(Kind::DuplicateDescriptors));
    let root = checks::inspect_object(&fixture.root, Kind::InspectRoot, "root").unwrap();
    let peer = checks::inspect_object(&fixture.peer, Kind::InspectPeer, "peer").unwrap();
    assert_eq!(
        require_distinct_pidfd_descriptor(root, peer, root)
            .unwrap_err()
            .kind(),
        Kind::DuplicateDescriptors
    );
    assert_eq!(
        require_distinct_pidfd_descriptor(root, peer, peer)
            .unwrap_err()
            .kind(),
        Kind::DuplicateDescriptors
    );
}

#[test]
fn socket_credentials_bind_each_of_pid_uid_gid_not_just_caller_claims() {
    let fixture = Fixture::new();
    for expected in [
        ExpectedClientProcessIdentityV1::new(
            std::process::id(),
            fixture.expected.uid(),
            fixture.expected.gid(),
        )
        .unwrap(),
        ExpectedClientProcessIdentityV1::new(
            fixture.expected.pid(),
            fixture.expected.uid() ^ 1,
            fixture.expected.gid(),
        )
        .unwrap(),
        ExpectedClientProcessIdentityV1::new(
            fixture.expected.pid(),
            fixture.expected.uid(),
            fixture.expected.gid() ^ 1,
        )
        .unwrap(),
    ] {
        let claimed_client = client(expected);
        let error = run(Service::INPUT_STORAGE, Service::ADMISSION_WORK, |b| {
            Service::admit_inner::<false>(
                duplicate(&fixture.root),
                duplicate(&fixture.peer),
                claimed_client,
                b,
            )
        })
        .unwrap_err();
        assert_eq!(error.kind(), Some(Kind::PeerCredentialsMismatch));
        let mut service = fixture.admitted();
        service.live_client = client(expected);
        assert_eq!(
            revalidate(&service).unwrap_err().kind(),
            Some(Kind::PeerCredentialsChanged)
        );
    }
}

#[test]
fn revalidation_compares_complete_snapshots_not_only_inode_numbers() {
    let fixture = Fixture::new();
    for root in [true, false] {
        for field in 0..6 {
            let mut service = fixture.admitted();
            let snapshot = if root {
                &mut service.root_identity
            } else {
                &mut service.peer_identity
            };
            match field {
                0 => snapshot.device ^= 1,
                1 => snapshot.inode ^= 1,
                2 => snapshot.mode ^= 1,
                3 => snapshot.uid ^= 1,
                4 => snapshot.gid ^= 1,
                _ => snapshot.links ^= 1,
            }
            assert_eq!(
                revalidate(&service).unwrap_err().kind(),
                Some(if root {
                    Kind::RootIdentityChanged
                } else {
                    Kind::PeerIdentityChanged
                })
            );
        }
    }
    let mut service = fixture.admitted();
    service.service_uid ^= 1;
    assert_eq!(
        revalidate(&service).unwrap_err().kind(),
        Some(Kind::ServiceIdentityChanged)
    );
}

#[test]
fn changed_live_roots_and_sockets_cannot_replace_admitted_objects() {
    let fixture = Fixture::new();
    let mut service = fixture.admitted();
    let (_other_directory, other_root) = protected_root();
    service.root = other_root;
    assert_eq!(
        revalidate(&service).unwrap_err().kind(),
        Some(Kind::RootIdentityChanged)
    );
    let mut service = fixture.admitted();
    let (other_peer, _held) = pair();
    service.peer = other_peer;
    assert_eq!(
        revalidate(&service).unwrap_err().kind(),
        Some(Kind::PeerIdentityChanged)
    );
    let service = fixture.admitted();
    fs::set_permissions(fixture.directory.path(), fs::Permissions::from_mode(0o710)).unwrap();
    assert_eq!(
        revalidate(&service).unwrap_err().kind(),
        Some(Kind::RootMode)
    );
    fs::set_permissions(fixture.directory.path(), fs::Permissions::from_mode(0o700)).unwrap();
    fs::remove_dir(fixture.directory.path()).unwrap();
    assert_eq!(
        revalidate(&service).unwrap_err().kind(),
        Some(Kind::RootUnlinked)
    );
}

#[test]
fn dead_client_refuses_both_consuming_admission_and_retained_revalidation() {
    let mut fixture = Fixture::new();
    let service = fixture.admitted();
    let (root, peer, client) = fixture.inputs();
    fixture.finish();
    let error = run(Service::INPUT_STORAGE, Service::ADMISSION_WORK, |b| {
        Service::admit(root, peer, client, b)
    })
    .unwrap_err();
    assert_eq!(error.kind(), Some(Kind::ClientAlreadyDead));
    assert_eq!(
        revalidate(&service).unwrap_err().kind(),
        Some(Kind::ClientAlreadyDead)
    );
}

#[test]
fn exit_after_peer_credentials_is_refused_by_the_final_liveness_check() {
    let mut fixture = Fixture::new();
    let service = fixture.admitted();
    let mut reached = false;
    let mut work = Work::new(Service::REVALIDATION_WORK);
    let mut budget = Budget::new(&mut work, EXTRA + Service::RETAINED + Service::IO_STORAGE);
    budget.reserve_storage(EXTRA + Service::RETAINED).unwrap();
    let error = Service::scope(&mut budget, Service::RETAINED, |budget| {
        service.inspect_with::<false>(Kind::PeerCredentialsChanged, budget, || {
            reached = true;
            fixture.finish();
        })
    })
    .unwrap_err();
    assert!(reached);
    assert_eq!(error.kind(), Some(Kind::ClientAlreadyDead));
    assert_eq!(budget.work(), Service::REVALIDATION_WORK);
    assert_eq!(budget.storage(), EXTRA + Service::RETAINED);
    assert_eq!(
        budget.peak_storage(),
        EXTRA + Service::RETAINED + Service::IO_STORAGE
    );
}

#[test]
#[ignore = "explicit root-container opt-in; exercises the production distinct-UID constructor"]
fn isolated_distinct_uid_public_admission_and_revalidation() {
    assert_eq!(
        std::env::var(OPT_IN).as_deref(),
        Ok("1"),
        "explicit opt-in required, not a skipped success"
    );
    assert!(
        std::path::Path::new("/.dockerenv").exists(),
        "isolated Docker fixture only"
    );
    assert_eq!(rustix::process::geteuid().as_raw(), 0);
    let mut fixture = Fixture::spawn(true);
    let mut work = Work::new(
        Client::ADMISSION_WORK + Service::ADMISSION_WORK + Service::REVALIDATION_WORK * 3,
    );
    let mut budget = Budget::new(&mut work, Service::RETAINED + Service::IO_STORAGE);
    budget
        .reserve_storage(Service::FD_PAIR_STORAGE + Client::FD_STORAGE)
        .unwrap();
    let (client, delta) = Client::admit(
        pidfd_for(fixture.expected.pid()),
        fixture.expected,
        &mut budget,
    )
    .unwrap();
    budget.reserve_storage(delta.additional_storage()).unwrap();
    let (service, delta) = Service::admit(
        duplicate(&fixture.root),
        duplicate(&fixture.peer),
        client,
        &mut budget,
    )
    .unwrap();
    budget.reserve_storage(delta.additional_storage()).unwrap();
    assert_eq!(service.expected_client().uid(), 65_534);
    assert_eq!(service.client_process_identity().0, fixture.child.0.id());
    service.validate_continuity(&mut budget).unwrap();
    rustix::fs::fchown(
        &fixture.root,
        Some(rustix::process::Uid::from_raw(65_534)),
        None,
    )
    .unwrap();
    assert_eq!(
        service.validate_continuity(&mut budget).unwrap_err().kind(),
        Some(Kind::RootOwner)
    );
    rustix::fs::fchown(&fixture.root, Some(rustix::process::Uid::from_raw(0)), None).unwrap();
    fixture.finish();
    assert_eq!(
        service.validate_continuity(&mut budget).unwrap_err().kind(),
        Some(Kind::ClientAlreadyDead)
    );
    let retained = service.retained_storage();
    drop(service);
    budget.release_storage(retained).unwrap();
    assert_eq!(budget.storage(), 0);
}

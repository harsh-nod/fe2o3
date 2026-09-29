//! Privileged socket lifecycle tests, not launch-history or authority qualification.
//! Every channel comes from the production constructor. No compiler occurrence,
//! held-exec callback, issuer admission, or root session is manufactured here.
//! Opt in with FE2O3_RUN_NATIVE_TESTS=1, filter lifecycle_tests::privileged_,
//! and pass --ignored.
//! Missing prerequisites fail that explicit run; ordinary runs report ignored.
use super::*;
use std::{
    fs::File,
    io::{Read, Seek, SeekFrom, Write},
    os::fd::{AsRawFd, RawFd},
    process::{Child, Command, Stdio},
    sync::mpsc,
    time::{Duration, Instant},
};

type Channel<'work> = RootLaunchChannelV3<'work>;
const LIMIT: usize = 2_000_000;
const MARKER: &str = "FE2O3_ROOT_CHANNEL_LIFECYCLE_CASE";
const REPORT: &str = "FE2O3_ROOT_CHANNEL_LIFECYCLE_REPORT";
const HELPER: &str = "compiler_execution_root_channel::lifecycle_tests::lifecycle_subprocess";

#[test]
fn creation_resource_refusals_preserve_prefix_floor_and_denial_history() {
    for shortage in 0..3 {
        let work_limit = 11
            + if shortage == 0 {
                ENTRY - 1
            } else {
                WORK - usize::from(shortage == 1)
            };
        let storage_limit = 37 + FRAME - usize::from(shortage == 2);
        let mut work = Work::new(work_limit);
        let mut b = Budget::new(&mut work, storage_limit);
        b.charge_work(11).unwrap();
        b.reserve_storage(37).unwrap();
        assert!(b.charge_work(work_limit + 1).is_err());
        assert!(b.reserve_storage(storage_limit + 1).is_err());
        let ledger = b.work_ledger_identity_v1();
        let result = Channel::create(&mut b);
        if shortage == 2 {
            assert!(matches!(result, Err(Error::Resource(Resource::Storage(e)))
                if e.actual() == 37 + FRAME && e.limit() == storage_limit));
            assert_eq!(b.work(), 11 + WORK);
        } else {
            let attempted = 11 + if shortage == 0 { ENTRY } else { WORK };
            assert!(matches!(result, Err(Error::Resource(Resource::Work(e)))
                if e.actual() == attempted && e.limit() == work_limit));
            assert_eq!(b.work(), 11 + if shortage == 0 { 0 } else { ENTRY });
        }
        assert_eq!(b.storage(), 37);
        assert_eq!(b.peak_storage(), 37);
        assert_eq!(b.failed_work(), Some(11 + work_limit + 1));
        assert_eq!(b.failed_storage(), Some(37 + storage_limit + 1));
        assert!(b.work_ledger_identity_v1() == ledger);
    }
}

#[test]
#[ignore = "requires isolated exact root and Unix seqpacket permission; no default qualification"]
fn privileged_creation_checks_both_endpoints_and_drop() {
    run("creation");
}

#[test]
#[ignore = "requires isolated exact root and Unix seqpacket permission; no default qualification"]
fn privileged_alias_closure_preserves_external_duplicate_and_retained_charge() {
    run("aliases");
}

#[test]
#[ignore = "requires isolated exact root and Unix seqpacket permission; no default qualification"]
fn privileged_endpoint_mutations_refuse_without_losing_custody() {
    run("shape");
}

#[test]
#[ignore = "requires isolated exact root and Unix seqpacket permission; no default qualification"]
fn privileged_account_replacement_and_moved_budget_refuse() {
    run("account");
}

#[test]
#[ignore = "requires isolated exact root and Unix seqpacket permission; no default qualification"]
fn privileged_live_foreign_identity_faults_refuse() {
    run("identities");
}

#[test]
#[ignore = "requires isolated exact root and Unix seqpacket permission; no default qualification"]
fn privileged_exact_and_short_lifecycle_quotas_preserve_cleanup() {
    run("quotas");
}

#[test]
#[ignore = "requires isolated exact root, setgroups/setresgid/setresuid, and Unix seqpacket"]
fn privileged_credential_loss_refuses_access_but_drop_still_closes() {
    run("lose-root");
}

fn run(mode: &'static str) {
    assert_eq!(
        std::env::var("FE2O3_RUN_NATIVE_TESTS").as_deref(),
        Ok("1"),
        "UNAVAILABLE: privileged lifecycle lane not enabled; no positive qualification"
    );
    Subprocess::start(mode).finish();
}

// The completion marker detects an incorrect libtest filter as well as partial
// execution. File-backed diagnostics cannot fill a pipe while the parent polls.
struct Subprocess {
    child: Child,
    mode: &'static str,
    report: tempfile::NamedTempFile,
    diagnostics: tempfile::NamedTempFile,
}

impl Subprocess {
    fn start(mode: &'static str) -> Self {
        let report = tempfile::NamedTempFile::new().unwrap();
        let diagnostics = tempfile::NamedTempFile::new().unwrap();
        let child = Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                HELPER,
                "--ignored",
                "--nocapture",
                "--test-threads=1",
            ])
            .env_clear()
            .env(MARKER, mode)
            .env(REPORT, report.path())
            .stdin(Stdio::piped())
            .stdout(Stdio::null())
            .stderr(Stdio::from(diagnostics.as_file().try_clone().unwrap()))
            .spawn()
            .expect("UNAVAILABLE: cannot exec isolated lifecycle helper");
        Self {
            child,
            mode,
            report,
            diagnostics,
        }
    }

    fn finish(&mut self) {
        let deadline = Instant::now() + Duration::from_secs(20);
        let status = loop {
            if let Some(status) = self.child.try_wait().unwrap() {
                break status;
            }
            assert!(Instant::now() < deadline, "{} helper timed out", self.mode);
            std::thread::sleep(Duration::from_millis(2));
        };
        assert!(
            status.success(),
            "{}: {}",
            self.mode,
            std::fs::read_to_string(self.diagnostics.path()).unwrap()
        );
        assert_eq!(
            std::fs::read(self.report.path()).unwrap(),
            format!("complete:{}", self.mode).as_bytes(),
            "helper did not complete the selected lifecycle case"
        );
    }

    fn wait_ready(&mut self) {
        let deadline = Instant::now() + Duration::from_secs(5);
        while std::fs::read(self.report.path()).unwrap() != b"ready" {
            assert!(
                self.child.try_wait().unwrap().is_none(),
                "identity peer exited early"
            );
            assert!(
                Instant::now() < deadline,
                "identity peer readiness timed out"
            );
            std::thread::sleep(Duration::from_millis(2));
        }
    }
}

impl Drop for Subprocess {
    fn drop(&mut self) {
        if !matches!(self.child.try_wait(), Ok(Some(_))) {
            let _ = self.child.kill();
            let _ = self.child.wait();
        }
    }
}

fn report(file: &mut File, bytes: &[u8]) {
    file.set_len(0).unwrap();
    file.seek(SeekFrom::Start(0)).unwrap();
    file.write_all(bytes).unwrap();
}

#[test]
#[ignore = "subprocess-only helper; requires an explicit case and completion marker"]
fn lifecycle_subprocess() {
    let mode = std::env::var(MARKER).expect("subprocess-only: no lifecycle case selected");
    // Open before any credential drop; a root-owned build/report path need not
    // be traversable by the eventual unprivileged helper identity.
    let mut completion = File::options()
        .write(true)
        .open(std::env::var_os(REPORT).expect("missing completion marker"))
        .unwrap();
    if mode == "identity-peer" {
        report(&mut completion, b"ready");
        let mut release = [0];
        std::io::stdin().read_exact(&mut release).unwrap();
        assert_eq!(release, [1]);
    } else {
        assert!(
            require_root().is_ok(),
            "UNAVAILABLE: exact root identity required; no positive channel qualification"
        );
        match mode.as_str() {
            "creation" => creation(),
            "aliases" => aliases(),
            "shape" => shape(),
            "account" => account(),
            "identities" => identities(),
            "quotas" => quotas(),
            "lose-root" => lose_root(),
            _ => panic!("unknown lifecycle case: {mode}"),
        }
    }
    report(&mut completion, format!("complete:{mode}").as_bytes());
}

fn create<'work>(b: &mut Budget<'work>) -> Channel<'work> {
    let floor = b.storage();
    let prefix = b.work();
    let (channel, charge) = Channel::create(b).unwrap_or_else(|error| match error {
        Error::Io(
            io::Errno::PERM
            | io::Errno::ACCESS
            | io::Errno::NOSYS
            | io::Errno::AFNOSUPPORT
            | io::Errno::PROTONOSUPPORT,
        ) => panic!("UNAVAILABLE: root socket prerequisites refused: {error}; no qualification"),
        _ => panic!("production root channel creation failed: {error}"),
    });
    assert_eq!(b.storage(), floor);
    assert_eq!(b.work(), prefix + WORK);
    assert_eq!(charge.additional_storage(), channel.retained_storage());
    b.reserve_storage(charge.additional_storage()).unwrap();
    channel
}

fn descriptors(channel: &Channel<'_>) -> [RawFd; 2] {
    [
        channel.root.as_raw_fd(),
        channel.issuer.as_ref().unwrap().as_raw_fd(),
    ]
}

fn assert_closed(fd: RawFd) {
    // SAFETY: fcntl takes a scalar descriptor; no BorrowedFd is forged after
    // closure. The isolated helper opens no descriptors between drop and probe.
    assert_eq!(unsafe { libc::fcntl(fd, libc::F_GETFD) }, -1);
    assert_eq!(
        std::io::Error::last_os_error().raw_os_error(),
        Some(libc::EBADF)
    );
}

fn drop_channel(channel: Channel<'_>, b: &mut Budget<'_>, fds: [RawFd; 2]) {
    let retained = channel.retained_storage();
    let floor = b.storage();
    let prefix = b.work();
    drop(channel);
    for fd in fds {
        assert_closed(fd);
    }
    assert_eq!((b.storage(), b.work()), (floor, prefix));
    b.release_storage(retained).unwrap();
}

fn creation() {
    let mut work = Work::new(2 * WORK);
    let mut b = Budget::new(&mut work, 37 + Channel::STORAGE + FRAME);
    b.reserve_storage(37).unwrap();
    let channel = create(&mut b);
    let fds = descriptors(&channel);
    assert_ne!(fds[0], fds[1]);
    let issuer = channel.issuer_endpoint(&mut b).unwrap();
    for fd in [channel.root.as_fd(), issuer] {
        assert_eq!(io::fcntl_getfd(fd).unwrap(), io::FdFlags::CLOEXEC);
        assert_eq!(
            fs::fcntl_getfl(fd).unwrap(),
            fs::OFlags::RDWR | fs::OFlags::NONBLOCK
        );
        assert_eq!(
            net::sockopt::socket_type(fd).unwrap(),
            net::SocketType::SEQPACKET
        );
        assert!(net::sockopt::socket_passcred(fd).unwrap());
        assert_eq!(
            net::getsockname(fd).unwrap(),
            net::SocketAddrUnix::new_unnamed().into()
        );
        assert_eq!(
            net::getpeername(fd).unwrap(),
            Some(net::SocketAddrUnix::new_unnamed().into())
        );
        let peer = net::sockopt::socket_peercred(fd).unwrap();
        assert_eq!(peer.pid, process::getpid());
        assert_eq!((peer.uid.as_raw(), peer.gid.as_raw()), (0, 0));
    }
    let mut byte = [0];
    for (sender, receiver) in [
        (channel.root.as_fd(), issuer),
        (issuer, channel.root.as_fd()),
    ] {
        assert_eq!(
            net::send(
                sender,
                b"x",
                net::SendFlags::DONTWAIT | net::SendFlags::NOSIGNAL
            )
            .unwrap(),
            1
        );
        assert_eq!(io::read(receiver, &mut byte).unwrap(), 1);
        assert_eq!(byte, [b'x']);
    }
    assert_eq!(b.work(), 2 * WORK);
    assert_eq!(b.peak_storage(), 37 + Channel::STORAGE + FRAME);
    drop_channel(channel, &mut b, fds);
    assert_eq!(b.storage(), 37);
}

fn aliases() {
    let mut work = Work::new(LIMIT);
    let mut b = Budget::new(&mut work, LIMIT);
    let mut channel = create(&mut b);
    let fds = descriptors(&channel);
    b.reserve_storage(FD_STORAGE).unwrap();
    let duplicate = io::fcntl_dupfd_cloexec(channel.issuer_endpoint(&mut b).unwrap(), 0).unwrap();
    let duplicate_fd = duplicate.as_raw_fd();
    let floor = b.storage();
    let prefix = b.work();
    for attempt in 1..=2 {
        channel.close_parent_issuer_endpoint(&mut b).unwrap();
        assert!(channel.issuer.is_none());
        assert_closed(fds[1]);
        assert_eq!(b.storage(), floor);
        assert_eq!(b.work(), prefix + attempt * WORK);
        assert_eq!(channel.retained_storage(), Channel::STORAGE);
    }
    assert!(matches!(
        channel.issuer_endpoint(&mut b),
        Err(Error::Refused("issuer endpoint closed"))
    ));
    let mut byte = [0];
    assert_eq!(io::read(&channel.root, &mut byte), Err(io::Errno::AGAIN));
    assert_eq!(
        net::send(
            &duplicate,
            b"x",
            net::SendFlags::DONTWAIT | net::SendFlags::NOSIGNAL
        )
        .unwrap(),
        1
    );
    assert_eq!(io::read(&channel.root, &mut byte).unwrap(), 1);
    assert_eq!(byte, [b'x']);
    drop(duplicate);
    assert_closed(duplicate_fd);
    b.release_storage(FD_STORAGE).unwrap();
    assert_eq!(io::read(&channel.root, &mut byte).unwrap(), 0);
    drop_channel(channel, &mut b, fds);
    assert_eq!(b.storage(), 0);
}

fn shape() {
    let mut work = Work::new(20 * WORK);
    let mut b = Budget::new(&mut work, LIMIT);
    let channel = create(&mut b);
    let fds = descriptors(&channel);
    for fd in [
        channel.root.as_fd(),
        channel.issuer.as_ref().unwrap().as_fd(),
    ] {
        for field in 0..3 {
            match field {
                0 => io::fcntl_setfd(fd, io::FdFlags::empty()).unwrap(),
                1 => fs::fcntl_setfl(fd, fs::OFlags::RDWR).unwrap(),
                _ => net::sockopt::set_socket_passcred(fd, false).unwrap(),
            }
            let floor = b.storage();
            let prefix = b.work();
            assert!(matches!(
                channel.issuer_endpoint(&mut b),
                Err(Error::Refused("root control endpoint shape"))
            ));
            assert_eq!((b.storage(), b.work()), (floor, prefix + WORK));
            match field {
                0 => io::fcntl_setfd(fd, io::FdFlags::CLOEXEC).unwrap(),
                1 => fs::fcntl_setfl(fd, fs::OFlags::RDWR | fs::OFlags::NONBLOCK).unwrap(),
                _ => net::sockopt::set_socket_passcred(fd, true).unwrap(),
            }
            channel.issuer_endpoint(&mut b).unwrap();
        }
    }
    drop_channel(channel, &mut b, fds);
}

fn accounting_refusal(channel: &mut Channel<'_>, b: &mut Budget<'_>) {
    let floor = b.storage();
    let prefix = b.work();
    assert!(matches!(
        channel.issuer_endpoint(b),
        Err(Error::Resource(Resource::Accounting))
    ));
    assert!(matches!(
        channel.close_parent_issuer_endpoint(b),
        Err(Error::Resource(Resource::Accounting))
    ));
    assert!(channel.issuer.is_some());
    assert_eq!((b.storage(), b.work()), (floor, prefix + 2 * WORK));
    assert_eq!(
        io::fcntl_getfd(&channel.root).unwrap(),
        io::FdFlags::CLOEXEC
    );
    assert_eq!(
        io::fcntl_getfd(channel.issuer.as_ref().unwrap()).unwrap(),
        io::FdFlags::CLOEXEC
    );
}

fn account() {
    let mut work = Work::new(LIMIT);
    let mut foreign_work = Work::new(LIMIT);
    let mut b = Budget::new(&mut work, LIMIT);
    let mut foreign = Budget::new(&mut foreign_work, LIMIT);
    foreign.reserve_storage(Channel::STORAGE).unwrap();
    assert!(b.reserve_storage(LIMIT + 1).is_err());
    let mut channel = create(&mut b);
    let fds = descriptors(&channel);
    let original = b.work_ledger_identity_v1();
    let prefix = b.work();
    accounting_refusal(&mut channel, &mut foreign);
    assert_eq!(b.work(), prefix);
    std::mem::swap(&mut b, &mut foreign);
    // Foreign ledger in the original slot, then original ledger at a new address.
    assert!(b.work_ledger_identity_v1() != original);
    accounting_refusal(&mut channel, &mut b);
    assert!(foreign.work_ledger_identity_v1() == original);
    accounting_refusal(&mut channel, &mut foreign);
    std::mem::swap(&mut b, &mut foreign);
    assert_eq!(b.failed_storage(), Some(LIMIT + 1));
    channel.issuer_endpoint(&mut b).unwrap();
    channel.close_parent_issuer_endpoint(&mut b).unwrap();
    drop_channel(channel, &mut b, fds);
}

fn identities() {
    let mut work = Work::new(LIMIT);
    let mut b = Budget::new(&mut work, LIMIT);
    let mut channel = create(&mut b);
    let fds = descriptors(&channel);
    // Fault injection into a real owner, matching root-trace test practice.
    // This is not a Send bypass or a claim of post-fork execution coverage.
    std::thread::scope(|scope| {
        let (send_tid, receive_tid) = mpsc::channel();
        let (release, hold) = mpsc::channel();
        let worker = scope.spawn(move || {
            send_tid.send(rustix::thread::gettid()).unwrap();
            let _ = hold.recv();
        });
        let tid = receive_tid.recv_timeout(Duration::from_secs(5)).unwrap();
        assert_ne!(tid, rustix::thread::gettid());
        let original = std::mem::replace(&mut channel.creator, tid);
        accounting_refusal(&mut channel, &mut b);
        channel.creator = original;
        channel.issuer_endpoint(&mut b).unwrap();
        release.send(()).unwrap();
        worker.join().unwrap();
    });
    let mut peer = Subprocess::start("identity-peer");
    peer.wait_ready();
    let pid = process::Pid::from_raw(peer.child.id().try_into().unwrap()).unwrap();
    assert_ne!(pid, process::getpid());
    let original = std::mem::replace(&mut channel.process, pid);
    accounting_refusal(&mut channel, &mut b);
    channel.process = original;
    channel.issuer_endpoint(&mut b).unwrap();
    peer.child.stdin.take().unwrap().write_all(&[1]).unwrap();
    peer.finish();
    // Reap/drop the helper before capturing/probing any new descriptor numbers.
    drop(peer);
    drop_channel(channel, &mut b, fds);
}

fn quotas() {
    for close in [false, true] {
        for shortage in 0..4 {
            let mut work = Work::new(LIMIT);
            let mut b = Budget::new(&mut work, LIMIT);
            assert!(b.charge_work(LIMIT + 1).is_err());
            assert!(b.reserve_storage(LIMIT + 1).is_err());
            let mut channel = create(&mut b);
            let fds = descriptors(&channel);
            if shortage == 3 {
                b.release_storage(1).unwrap();
            } else {
                b.reserve_storage(LIMIT - FRAME + usize::from(shortage == 2) - b.storage())
                    .unwrap();
            }
            let remaining = WORK - usize::from(shortage == 1);
            b.charge_work(LIMIT - remaining - b.work()).unwrap();
            let floor = b.storage();
            let prefix = b.work();
            let result = if close {
                channel.close_parent_issuer_endpoint(&mut b)
            } else {
                channel.issuer_endpoint(&mut b).map(|_| ())
            };
            match shortage {
                0 => {
                    result.unwrap();
                    assert_eq!(b.work(), prefix + WORK);
                    assert_eq!(b.peak_storage(), LIMIT);
                }
                1 => {
                    assert!(matches!(result, Err(Error::Resource(Resource::Work(_)))));
                    assert_eq!(b.work(), prefix + ENTRY);
                }
                2 => {
                    assert!(matches!(result, Err(Error::Resource(Resource::Storage(_)))));
                    assert_eq!(b.work(), prefix + WORK);
                }
                _ => {
                    assert!(matches!(result, Err(Error::Resource(Resource::Accounting))));
                    assert_eq!(b.work(), prefix + ENTRY);
                }
            }
            assert_eq!(b.storage(), floor);
            assert_eq!(b.failed_work(), Some(LIMIT + 1));
            assert_eq!(b.failed_storage(), Some(LIMIT + 1));
            assert_eq!(channel.issuer.is_none(), close && shortage == 0);
            if shortage == 3 {
                b.reserve_storage(1).unwrap();
            }
            // Drop remains available after work/scratch denial; it refunds no ledger.
            drop_channel(channel, &mut b, fds);
        }
    }
}

fn lose_root() {
    let mut work = Work::new(3 * WORK);
    let mut b = Budget::new(&mut work, Channel::STORAGE + FRAME);
    let mut channel = create(&mut b);
    let fds = descriptors(&channel);
    // SAFETY: only the explicitly selected, deadline-bounded subprocess changes
    // credentials, after exec and after obtaining the genuine root-created pair.
    assert_eq!(
        unsafe { libc::setgroups(0, std::ptr::null()) },
        0,
        "UNAVAILABLE: setgroups prerequisite refused"
    );
    assert_eq!(
        unsafe { libc::setresgid(65534, 65534, 65534) },
        0,
        "UNAVAILABLE: setresgid prerequisite refused"
    );
    assert_eq!(
        unsafe { libc::setresuid(65534, 65534, 65534) },
        0,
        "UNAVAILABLE: setresuid prerequisite refused"
    );
    assert!(require_root().is_err());
    assert!(matches!(
        channel.issuer_endpoint(&mut b),
        Err(Error::Refused("root control requires exact root identity"))
    ));
    assert!(matches!(
        channel.close_parent_issuer_endpoint(&mut b),
        Err(Error::Refused("root control requires exact root identity"))
    ));
    assert!(channel.issuer.is_some());
    assert_eq!(b.work(), 3 * WORK);
    drop_channel(channel, &mut b, fds);
    assert_eq!(b.storage(), 0);
}

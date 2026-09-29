//! Packet custody tests only: no occurrence, admission, or launch history is
//! manufactured. Every owner comes from RootLaunchChannelV3::create.
//! Native lane: FE2O3_RUN_NATIVE_TESTS=1, filter packet_tests::privileged_,
//! and pass --ignored. Missing prerequisites fail the explicitly selected lane.
use super::*;
use fe2o3_protected_service_spawn::launch_io::{self, MessageSender};
use std::{
    fs::File,
    io::{IoSlice, Write},
    mem::MaybeUninit,
    os::{fd::AsRawFd, unix::fs::MetadataExt},
    process::{Child, Command, Stdio},
    sync::mpsc,
    time::{Duration, Instant},
};

type Channel<'work> = RootLaunchChannelV3<'work>;
type Packet = [u8; 4096];
const BYTES: usize = size_of::<Packet>();
const WORK_LIMIT: usize = 128 * Channel::PACKET_WORK;
const STORAGE_LIMIT: usize = Channel::STORAGE + FD_STORAGE + BYTES + 2 * Channel::PACKET_SCRATCH;
const MARKER: &str = "FE2O3_ROOT_CHANNEL_PACKET_CASE";
const REPORT: &str = "FE2O3_ROOT_CHANNEL_PACKET_REPORT";
const HELPER: &str = "compiler_execution_root_channel::packet_tests::packet_subprocess";
const FRAMING: &str = "root control packet framing or credentials";

#[test]
fn packet_quotes_cover_validation_transport_and_borrowed_send_bytes() {
    assert_eq!(BYTES, launch_io::AUTHENTICATED_PACKET_MAX_BYTES);
    assert_eq!(
        Channel::PACKET_WORK,
        Channel::WORK
            .checked_add(launch_io::packet_receive_work(BYTES))
            .unwrap()
    );
    assert_eq!(
        Channel::PACKET_SCRATCH,
        Channel::SCRATCH
            .checked_add(launch_io::packet_receive_scratch(BYTES))
            .unwrap()
    );
    assert!(Channel::PACKET_WORK > ENTRY);
    assert!(
        Channel::STORAGE
            .checked_add(BYTES)
            .unwrap()
            .checked_add(Channel::PACKET_SCRATCH)
            .unwrap()
            < STORAGE_LIMIT
    );
}

#[test]
#[ignore = "FE2O3_RUN_NATIVE_TESTS=1; requires isolated exact root and Unix seqpacket"]
fn privileged_packet_roundtrip_and_backpressure_are_not_admission() {
    run("roundtrip");
}

#[test]
#[ignore = "FE2O3_RUN_NATIVE_TESTS=1; requires isolated exact root, seqpacket, and /proc/self/fd"]
fn privileged_packet_credentials_framing_rights_and_eof_refuse() {
    run("framing");
}

#[test]
#[ignore = "FE2O3_RUN_NATIVE_TESTS=1; requires exact root, seqpacket, setgroups/setresgid/setresuid"]
fn privileged_packet_guards_preserve_queued_input_and_owner() {
    run("guards");
    run("accounts");
    run("lose-root");
}

#[test]
#[ignore = "FE2O3_RUN_NATIVE_TESTS=1; requires isolated exact root and Unix seqpacket"]
fn privileged_packet_exact_and_short_quotas_preserve_history_and_custody() {
    run("quotas");
}

struct Subprocess(Child);

impl Drop for Subprocess {
    fn drop(&mut self) {
        if !matches!(self.0.try_wait(), Ok(Some(_))) {
            let _ = self.0.kill();
            let _ = self.0.wait();
        }
    }
}

fn run(mode: &str) {
    assert_eq!(
        std::env::var("FE2O3_RUN_NATIVE_TESTS").as_deref(),
        Ok("1"),
        "UNAVAILABLE: native packet lane not enabled; no positive qualification"
    );
    let report = tempfile::NamedTempFile::new().unwrap();
    let diagnostics = tempfile::NamedTempFile::new().unwrap();
    let mut child = Subprocess(
        Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                HELPER,
                "--ignored",
                "--nocapture",
                "--test-threads=1",
            ])
            .env_clear()
            .env("FE2O3_RUN_NATIVE_TESTS", "1")
            .env(MARKER, mode)
            .env(REPORT, report.path())
            .stdin(Stdio::null())
            .stdout(Stdio::from(diagnostics.as_file().try_clone().unwrap()))
            .stderr(Stdio::from(diagnostics.as_file().try_clone().unwrap()))
            .spawn()
            .expect("UNAVAILABLE: cannot exec isolated packet helper"),
    );
    let deadline = Instant::now() + Duration::from_secs(20);
    let status = loop {
        if let Some(status) = child.0.try_wait().unwrap() {
            break status;
        }
        assert!(Instant::now() < deadline, "{mode}: packet helper timed out");
        std::thread::sleep(Duration::from_millis(2));
    };
    assert!(
        status.success(),
        "{mode}: {}",
        std::fs::read_to_string(diagnostics.path()).unwrap()
    );
    assert_eq!(
        std::fs::read(report.path()).unwrap(),
        format!("complete:{mode}").as_bytes(),
        "helper did not complete the selected packet case"
    );
}

#[test]
#[ignore = "subprocess-only; FE2O3_RUN_NATIVE_TESTS=1, explicit case, and completion marker required"]
fn packet_subprocess() {
    assert_eq!(std::env::var("FE2O3_RUN_NATIVE_TESTS").as_deref(), Ok("1"));
    let mode = std::env::var(MARKER).expect("subprocess-only: no packet case selected");
    // Open the report before dropping credentials; its path may be root-only.
    let mut report = File::options()
        .write(true)
        .open(std::env::var_os(REPORT).expect("missing completion marker"))
        .unwrap();
    assert!(
        require_root().is_ok(),
        "UNAVAILABLE: exact root required; no positive packet qualification"
    );
    match mode.as_str() {
        "roundtrip" => roundtrip(),
        "framing" => framing(),
        "guards" => guards(),
        "accounts" => accounts(),
        "quotas" => quotas(),
        "lose-root" => lose_root(),
        _ => panic!("unknown packet case: {mode}"),
    }
    write!(report, "complete:{mode}").unwrap();
}

fn this_sender() -> MessageSender {
    MessageSender::new(
        process::getpid().as_raw_pid(),
        process::getuid().as_raw(),
        process::getgid().as_raw(),
    )
}

fn create<'work>(b: &mut Budget<'work>, close_parent: bool) -> (Channel<'work>, OwnedFd) {
    assert!(b.charge_work(WORK_LIMIT + 1).is_err());
    assert!(b.reserve_storage(STORAGE_LIMIT + 1).is_err());
    b.charge_work(11).unwrap();
    b.reserve_storage(37).unwrap();
    let (mut channel, charge) = Channel::create(b)
        .unwrap_or_else(|error| panic!("UNAVAILABLE: production root channel creation: {error}"));
    assert_eq!(b.work(), 11 + Channel::WORK);
    assert_eq!(b.storage(), 37);
    assert_eq!(charge.additional_storage(), Channel::STORAGE);
    // Reserve the real owner, the test's issuer duplicate, and borrowed send bytes.
    b.reserve_storage(charge.additional_storage() + FD_STORAGE + BYTES)
        .unwrap();
    let peer = io::fcntl_dupfd_cloexec(channel.issuer_endpoint(b).unwrap(), 0).unwrap();
    if close_parent {
        channel.close_parent_issuer_endpoint(b).unwrap();
    }
    assert_eq!(channel.retained_storage(), Channel::STORAGE);
    (channel, peer)
}

fn attempt<'work, T>(
    channel: &Channel<'_>,
    b: &mut Budget<'work>,
    spent: usize,
    operation: impl FnOnce(&mut Budget<'work>) -> Result<T>,
) -> Result<T> {
    let (prefix, floor, peak) = (b.work(), b.storage(), b.peak_storage());
    let history = (b.failed_work(), b.failed_storage());
    let ledger = b.work_ledger_identity_v1();
    let result = operation(b);
    assert_eq!((b.work(), b.storage()), (prefix + spent, floor));
    assert!(b.peak_storage() >= peak);
    assert_eq!((b.failed_work(), b.failed_storage()), history);
    assert!(b.work_ledger_identity_v1() == ledger);
    assert_eq!(channel.retained_storage(), Channel::STORAGE);
    assert!(io::fcntl_getfd(&channel.root).is_ok());
    result
}

fn receive(
    channel: &Channel<'_>,
    sender: MessageSender,
    b: &mut Budget<'_>,
) -> Result<Option<Packet>> {
    attempt(channel, b, Channel::PACKET_WORK, |b| {
        channel.receive_packet(sender, b)
    })
}

fn send(channel: &Channel<'_>, bytes: &Packet, b: &mut Budget<'_>) -> Result<Option<()>> {
    attempt(channel, b, Channel::PACKET_WORK, |b| {
        channel.send_packet(bytes, b)
    })
}

fn refused<T: fmt::Debug>(result: Result<T>, reason: &str) {
    assert!(
        matches!(&result, Err(Error::Refused(actual)) if *actual == reason),
        "{result:?}"
    );
}

fn enqueue(peer: &OwnedFd, bytes: &[u8]) {
    assert_eq!(
        net::send(
            peer,
            bytes,
            net::SendFlags::DONTWAIT | net::SendFlags::NOSIGNAL
        )
        .unwrap(),
        bytes.len()
    );
}

fn peer_receive(peer: &OwnedFd) -> Option<Packet> {
    launch_io::receive_authenticated_packet(peer.as_fd(), this_sender()).unwrap()
}

fn drop_pair(channel: Channel<'_>, peer: OwnedFd, b: &mut Budget<'_>) {
    assert!(channel.issuer.is_none());
    assert_eq!(channel.retained_storage(), Channel::STORAGE);
    let fds = [channel.root.as_raw_fd(), peer.as_raw_fd()];
    let state = (b.work(), b.storage(), b.failed_work(), b.failed_storage());
    drop(channel);
    drop(peer);
    for fd in fds {
        // SAFETY: scalar fcntl probe in the isolated process, with no descriptor
        // allocation between dropping the actual owners and probing their numbers.
        assert_eq!(unsafe { libc::fcntl(fd, libc::F_GETFD) }, -1);
        assert_eq!(
            std::io::Error::last_os_error().raw_os_error(),
            Some(libc::EBADF)
        );
    }
    assert_eq!(
        (b.work(), b.storage(), b.failed_work(), b.failed_storage()),
        state
    );
    b.release_storage(b.storage()).unwrap();
}

fn roundtrip() {
    let mut work = Work::new(WORK_LIMIT);
    let mut b = Budget::new(&mut work, STORAGE_LIMIT);
    let (channel, peer) = create(&mut b, true);
    assert_eq!(receive(&channel, this_sender(), &mut b).unwrap(), None);
    let first: Packet = std::array::from_fn(|i| (i % 251) as u8);
    let second = [0x5a; BYTES];
    enqueue(&peer, &first);
    enqueue(&peer, &second);
    for expected in [first, second] {
        assert_eq!(
            receive(&channel, this_sender(), &mut b).unwrap(),
            Some(expected)
        );
    }
    assert_eq!(receive(&channel, this_sender(), &mut b).unwrap(), None);
    for packet in [&first, &second] {
        assert_eq!(send(&channel, packet, &mut b).unwrap(), Some(()));
        assert_eq!(peer_receive(&peer), Some(*packet));
    }
    assert_eq!(peer_receive(&peer), None);

    net::sockopt::set_socket_send_buffer_size(&channel.root, 4096).unwrap();
    let capacity = net::sockopt::socket_send_buffer_size(&channel.root).unwrap();
    assert!(
        (4096..=8192).contains(&capacity),
        "UNAVAILABLE: unexpected send buffer"
    );
    let filler = [0xa1; BYTES];
    let mut queued = 0;
    let mut full = false;
    for _ in 0..64 {
        match net::send(
            &channel.root,
            &filler,
            net::SendFlags::DONTWAIT | net::SendFlags::NOSIGNAL,
        ) {
            Ok(n) => {
                assert_eq!(n, BYTES);
                queued += 1;
            }
            Err(io::Errno::INTR) => (),
            Err(io::Errno::AGAIN) => {
                full = true;
                break;
            }
            Err(error) => panic!("backpressure prerequisite failed: {error}"),
        }
    }
    assert!(
        full && queued > 0,
        "UNAVAILABLE: could not establish bounded backpressure"
    );
    let pending = [0xe1; BYTES];
    assert_eq!(send(&channel, &pending, &mut b).unwrap(), None);
    assert_eq!(send(&channel, &pending, &mut b).unwrap(), None);
    for _ in 0..queued {
        assert_eq!(peer_receive(&peer), Some(filler));
    }
    assert_eq!(peer_receive(&peer), None);
    assert_eq!(send(&channel, &pending, &mut b).unwrap(), Some(()));
    assert_eq!(peer_receive(&peer), Some(pending));
    assert_eq!(peer_receive(&peer), None);
    assert_eq!(b.peak_storage(), b.storage() + Channel::PACKET_SCRATCH);
    drop_pair(channel, peer, &mut b);
}

fn framing() {
    let mut work = Work::new(WORK_LIMIT);
    let mut b = Budget::new(&mut work, STORAGE_LIMIT);
    let (channel, peer) = create(&mut b, true);
    let pid = process::getpid().as_raw_pid();
    let uid = process::getuid().as_raw();
    let gid = process::getgid().as_raw();
    for expected in [
        MessageSender::new(pid ^ 1, uid, gid),
        MessageSender::new(pid, uid ^ 1, gid),
        MessageSender::new(pid, uid, gid ^ 1),
    ] {
        enqueue(&peer, &[0xa5; BYTES]);
        enqueue(&peer, &[0x5a; BYTES]);
        refused(receive(&channel, expected, &mut b), FRAMING);
        assert_eq!(
            receive(&channel, this_sender(), &mut b).unwrap(),
            Some([0x5a; BYTES])
        );
    }
    // Representative wrapper checks; launch_io owns the full parser/rights matrix.
    for length in [0, 1, BYTES - 1, BYTES + 1] {
        enqueue(&peer, &[0xa5; BYTES + 1][..length]);
        refused(receive(&channel, this_sender(), &mut b), FRAMING);
    }
    let file = tempfile::tempfile().unwrap();
    let metadata = file.metadata().unwrap();
    let refs = || {
        std::fs::read_dir("/proc/self/fd")
            .expect("UNAVAILABLE: /proc/self/fd required to check rights disposal")
            .filter_map(|entry| std::fs::metadata(entry.ok()?.path()).ok())
            .filter(|m| (m.dev(), m.ino()) == (metadata.dev(), metadata.ino()))
            .count()
    };
    assert_eq!(refs(), 1);
    let fds = [file.as_fd()];
    let mut space = [MaybeUninit::uninit(); rustix::cmsg_space!(ScmRights(1))];
    let mut control = net::SendAncillaryBuffer::new(&mut space);
    assert!(control.push(net::SendAncillaryMessage::ScmRights(&fds)));
    assert_eq!(
        net::sendmsg(
            &peer,
            &[IoSlice::new(&[0xa5; BYTES])],
            &mut control,
            net::SendFlags::DONTWAIT | net::SendFlags::NOSIGNAL,
        )
        .unwrap(),
        BYTES
    );
    refused(receive(&channel, this_sender(), &mut b), FRAMING);
    assert_eq!(refs(), 1, "packet refusal leaked a received descriptor");
    assert_eq!(receive(&channel, this_sender(), &mut b).unwrap(), None);
    // Shutdown preserves both real owners while establishing EOF and EPIPE.
    net::shutdown(&peer, net::Shutdown::Both).unwrap();
    refused(receive(&channel, this_sender(), &mut b), FRAMING);
    assert!(matches!(
        send(&channel, &[0xa5; BYTES], &mut b),
        Err(Error::Io(io::Errno::PIPE))
    ));
    drop_pair(channel, peer, &mut b);
}

fn guards() {
    let mut work = Work::new(WORK_LIMIT);
    let mut b = Budget::new(&mut work, STORAGE_LIMIT);
    let (mut channel, peer) = create(&mut b, false);
    let packet = [0xa5; BYTES];
    enqueue(&peer, &packet);
    refused(
        receive(&channel, this_sender(), &mut b),
        "parent issuer endpoint still open",
    );
    refused(
        send(&channel, &packet, &mut b),
        "parent issuer endpoint still open",
    );
    assert!(channel.issuer.is_some());
    assert_eq!(peer_receive(&peer), None);
    channel.close_parent_issuer_endpoint(&mut b).unwrap();
    assert_eq!(
        receive(&channel, this_sender(), &mut b).unwrap(),
        Some(packet)
    );

    for field in 0..3 {
        enqueue(&peer, &packet);
        match field {
            0 => io::fcntl_setfd(&channel.root, io::FdFlags::empty()).unwrap(),
            1 => fs::fcntl_setfl(&channel.root, fs::OFlags::RDWR).unwrap(),
            _ => net::sockopt::set_socket_passcred(&channel.root, false).unwrap(),
        }
        refused(
            receive(&channel, this_sender(), &mut b),
            "root control endpoint shape",
        );
        refused(
            send(&channel, &packet, &mut b),
            "root control endpoint shape",
        );
        assert_eq!(peer_receive(&peer), None);
        match field {
            0 => io::fcntl_setfd(&channel.root, io::FdFlags::CLOEXEC).unwrap(),
            1 => fs::fcntl_setfl(&channel.root, fs::OFlags::RDWR | fs::OFlags::NONBLOCK).unwrap(),
            _ => net::sockopt::set_socket_passcred(&channel.root, true).unwrap(),
        }
        assert_eq!(
            receive(&channel, this_sender(), &mut b).unwrap(),
            Some(packet)
        );
    }
    assert_eq!(send(&channel, &packet, &mut b).unwrap(), Some(()));
    assert_eq!(peer_receive(&peer), Some(packet));
    drop_pair(channel, peer, &mut b);
}

fn accounting_refusal(channel: &Channel<'_>, peer: &OwnedFd, b: &mut Budget<'_>) {
    assert!(matches!(
        receive(channel, this_sender(), b),
        Err(Error::Resource(Resource::Accounting))
    ));
    assert!(matches!(
        send(channel, &[0xa5; BYTES], b),
        Err(Error::Resource(Resource::Accounting))
    ));
    assert_eq!(peer_receive(peer), None);
}

fn accounts() {
    let mut work = Work::new(WORK_LIMIT);
    let mut foreign_work = Work::new(WORK_LIMIT);
    let mut b = Budget::new(&mut work, STORAGE_LIMIT);
    let mut foreign = Budget::new(&mut foreign_work, STORAGE_LIMIT);
    let (mut channel, peer) = create(&mut b, true);
    foreign.reserve_storage(b.storage()).unwrap();
    let original = b.work_ledger_identity_v1();
    let packet = [0xa5; BYTES];
    enqueue(&peer, &packet);
    let prefix = b.work();
    accounting_refusal(&channel, &peer, &mut foreign);
    assert_eq!(b.work(), prefix);
    std::mem::swap(&mut b, &mut foreign);
    assert!(b.work_ledger_identity_v1() != original);
    accounting_refusal(&channel, &peer, &mut b);
    assert!(foreign.work_ledger_identity_v1() == original);
    accounting_refusal(&channel, &peer, &mut foreign);
    std::mem::swap(&mut b, &mut foreign);
    assert_eq!(
        receive(&channel, this_sender(), &mut b).unwrap(),
        Some(packet)
    );

    // Fault the recorded identity of a constructor-created owner, as lifecycle
    // tests do; this neither bypasses !Send nor claims execution after fork.
    std::thread::scope(|scope| {
        let (send_tid, receive_tid) = mpsc::channel();
        let (release, hold) = mpsc::channel();
        let worker = scope.spawn(move || {
            send_tid.send(rustix::thread::gettid()).unwrap();
            let _ = hold.recv_timeout(Duration::from_secs(5));
        });
        let tid = receive_tid.recv_timeout(Duration::from_secs(5)).unwrap();
        assert_ne!(tid, rustix::thread::gettid());
        enqueue(&peer, &packet);
        let creator = std::mem::replace(&mut channel.creator, tid);
        accounting_refusal(&channel, &peer, &mut b);
        channel.creator = creator;
        assert_eq!(
            receive(&channel, this_sender(), &mut b).unwrap(),
            Some(packet)
        );
        release.send(()).unwrap();
        worker.join().unwrap();
    });
    let parent = process::getppid().expect("subprocess requires its live harness parent");
    assert_ne!(parent, process::getpid());
    enqueue(&peer, &packet);
    let pid = std::mem::replace(&mut channel.process, parent);
    accounting_refusal(&channel, &peer, &mut b);
    channel.process = pid;
    assert_eq!(
        receive(&channel, this_sender(), &mut b).unwrap(),
        Some(packet)
    );
    assert_eq!(send(&channel, &packet, &mut b).unwrap(), Some(()));
    assert_eq!(peer_receive(&peer), Some(packet));
    drop_pair(channel, peer, &mut b);
}

fn quotas() {
    for sending in [false, true] {
        // Exact, short entry work, short full work, short scratch, short owner/input floor.
        for shortage in 0..5 {
            let mut work = Work::new(WORK_LIMIT);
            let mut b = Budget::new(&mut work, STORAGE_LIMIT);
            let (channel, peer) = create(&mut b, true);
            let packet = [0xa5; BYTES];
            enqueue(&peer, &packet);
            if shortage == 4 {
                let required = Channel::STORAGE + if sending { BYTES } else { 0 };
                b.release_storage(b.storage() - (required - 1)).unwrap();
            } else {
                b.reserve_storage(
                    STORAGE_LIMIT - Channel::PACKET_SCRATCH + usize::from(shortage == 3)
                        - b.storage(),
                )
                .unwrap();
            }
            let remaining = match shortage {
                1 => ENTRY - 1,
                2 => Channel::PACKET_WORK - 1,
                _ => Channel::PACKET_WORK,
            };
            b.charge_work(WORK_LIMIT - remaining - b.work()).unwrap();
            let prefix = b.work();
            let floor = b.storage();
            let peak = b.peak_storage();
            let spent = match shortage {
                1 => 0,
                2 | 4 => ENTRY,
                _ => Channel::PACKET_WORK,
            };
            let result = attempt(&channel, &mut b, spent, |b| {
                if sending {
                    channel.send_packet(&packet, b)
                } else {
                    channel
                        .receive_packet(this_sender(), b)
                        .map(|received| received.map(|bytes| assert_eq!(bytes, packet)))
                }
            });
            match shortage {
                0 => {
                    assert_eq!(result.unwrap(), Some(()));
                    assert_eq!(b.peak_storage(), STORAGE_LIMIT);
                }
                1 | 2 => {
                    let attempted = prefix
                        + if shortage == 1 {
                            ENTRY
                        } else {
                            Channel::PACKET_WORK
                        };
                    assert!(matches!(result, Err(Error::Resource(Resource::Work(e)))
                        if e.actual() == attempted && e.limit() == WORK_LIMIT));
                    assert_eq!(b.peak_storage(), peak);
                }
                3 => {
                    assert!(matches!(result, Err(Error::Resource(Resource::Storage(e)))
                        if e.actual() == floor + Channel::PACKET_SCRATCH && e.limit() == STORAGE_LIMIT));
                    assert_eq!(b.peak_storage(), peak);
                }
                _ => {
                    assert!(matches!(result, Err(Error::Resource(Resource::Accounting))));
                    assert_eq!(b.peak_storage(), peak);
                }
            }
            assert_eq!(b.failed_work(), Some(WORK_LIMIT + 1));
            assert_eq!(b.failed_storage(), Some(STORAGE_LIMIT + 1));
            // The exhausted/short account cannot authorize another packet call.
            // Inspect the genuine endpoint directly to prove denial did no I/O.
            assert_eq!(
                launch_io::receive_authenticated_packet::<BYTES>(
                    channel.root.as_fd(),
                    this_sender()
                )
                .unwrap(),
                if sending || shortage != 0 {
                    Some(packet)
                } else {
                    None
                }
            );
            assert_eq!(
                peer_receive(&peer),
                if sending && shortage == 0 {
                    Some(packet)
                } else {
                    None
                }
            );
            assert!(io::fcntl_getfd(&peer).is_ok());
            drop_pair(channel, peer, &mut b);
        }
    }
}

fn lose_root() {
    let mut work = Work::new(WORK_LIMIT);
    let mut b = Budget::new(&mut work, STORAGE_LIMIT);
    let (channel, peer) = create(&mut b, true);
    let sender = this_sender();
    let packet = [0xa5; BYTES];
    enqueue(&peer, &packet);
    // SAFETY: only the selected, deadline-bounded subprocess changes its own
    // credentials, after exec and production channel creation.
    assert_eq!(
        unsafe { libc::setgroups(0, std::ptr::null()) },
        0,
        "UNAVAILABLE: setgroups"
    );
    assert_eq!(
        unsafe { libc::setresgid(65534, 65534, 65534) },
        0,
        "UNAVAILABLE: setresgid"
    );
    assert_eq!(
        unsafe { libc::setresuid(65534, 65534, 65534) },
        0,
        "UNAVAILABLE: setresuid"
    );
    assert!(require_root().is_err());
    refused(
        receive(&channel, sender, &mut b),
        "root control requires exact root identity",
    );
    refused(
        send(&channel, &packet, &mut b),
        "root control requires exact root identity",
    );
    assert_eq!(
        launch_io::receive_authenticated_packet::<BYTES>(channel.root.as_fd(), sender).unwrap(),
        Some(packet)
    );
    assert_eq!(peer_receive(&peer), None);
    drop_pair(channel, peer, &mut b);
}

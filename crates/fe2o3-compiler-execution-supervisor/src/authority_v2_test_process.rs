//! Private, explicitly opted-in distinct-UID process fixtures, never activation.
#![cfg(test)]
#![allow(unsafe_code)]

use rustix::event::{PollFd, PollFlags, Timespec, poll};
use rustix::net::{
    AddressFamily, RecvAncillaryBuffer, RecvAncillaryMessage, RecvFlags, ReturnFlags,
    SendAncillaryBuffer, SendAncillaryMessage, SendFlags, SocketFlags, SocketType, recvmsg,
    sendmsg, socketpair,
};
use std::io::{self, IoSlice, IoSliceMut};
use std::mem::MaybeUninit;
use std::os::fd::{AsFd, FromRawFd, OwnedFd};
use std::os::unix::process::CommandExt;
use std::process::{Child, Command, ExitStatus, Stdio};
use std::time::{Duration, Instant};

const OPT_IN: &str = "FE2O3_RUN_PRIVILEGED_SUPERVISOR_V2_TEST";
const ROLE: &str = "FE2O3_SUPERVISOR_V2_FIXTURE_ROLE";
const ANCHOR_UID: u32 = 65_534;
const SUPERVISOR_UID: u32 = 65_533;
const IO_TIMEOUT: Duration = Duration::from_secs(5);
const CHILD_TIMEOUT: Duration = Duration::from_secs(30);
const ANCHOR_TIMEOUT: Duration = Duration::from_secs(45);
const CLEANUP_TIMEOUT: Duration = Duration::from_secs(5);
const ANCHOR_HELPER: &str = "authority_v2_test_process::anchor_process_helper";
const SUPERVISOR_HELPER: &str = "authority_v2_test_process::supervisor_process_helper";

struct ChildGuard(Child);

impl ChildGuard {
    fn wait_until(&mut self, deadline: Instant) -> io::Result<ExitStatus> {
        loop {
            if let Some(status) = self.0.try_wait()? {
                return Ok(status);
            }
            if Instant::now() >= deadline {
                return Err(io::Error::new(
                    io::ErrorKind::TimedOut,
                    "fixture child timed out",
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
        let kill = self.0.kill();
        let reap = self.wait_until(Instant::now() + CLEANUP_TIMEOUT);
        if reap.is_err() {
            // Do not double-panic or block indefinitely while unwinding. The
            // isolated container's init/outer deadline remains the final guard.
            eprintln!(
                "fixture child {} cleanup failed: kill={kill:?}, reap={reap:?}",
                self.0.id()
            );
        }
    }
}

fn pair() -> (OwnedFd, OwnedFd) {
    socketpair(
        AddressFamily::UNIX,
        SocketType::SEQPACKET,
        SocketFlags::CLOEXEC | SocketFlags::NONBLOCK,
        None,
    )
    .unwrap()
}

fn spawn_role(name: &str, role: &str, uid: u32, control: OwnedFd) -> ChildGuard {
    let mut command = Command::new(std::env::current_exe().unwrap());
    command
        .args([
            "--exact",
            name,
            "--ignored",
            "--nocapture",
            "--test-threads=1",
        ])
        .env(ROLE, role)
        .env_remove(OPT_IN)
        .env("TMPDIR", "/tmp")
        .current_dir("/tmp")
        .uid(uid)
        .gid(uid)
        .stdin(Stdio::from(control))
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit());
    // CommandExt::uid also clears supplementary groups when none are specified.
    // No pre_exec allocation, raw fork, or parent-wide inheritable FD window.
    let child =
        fe2o3_artifact_transaction::with_artifact_process_spawn_v1(|| command.spawn()).unwrap();
    // Command owns the parent's copy of child stdin until it is dropped.
    drop(command);
    ChildGuard(child)
}

fn require_child_credentials(role: &str, uid: u32) {
    assert_eq!(std::env::var(ROLE).as_deref(), Ok(role));
    let (mut real, mut effective, mut saved) = (0, 0, 0);
    // SAFETY: these scalar outputs are writable for the duration of each call.
    assert_eq!(
        unsafe { libc::getresuid(&mut real, &mut effective, &mut saved) },
        0
    );
    assert_eq!((real, effective, saved), (uid, uid, uid));
    assert_eq!(
        unsafe { libc::getresgid(&mut real, &mut effective, &mut saved) },
        0
    );
    assert_eq!((real, effective, saved), (uid, uid, uid));
    // SAFETY: zero requests only the group count, with no output array.
    assert_eq!(unsafe { libc::getgroups(0, std::ptr::null_mut()) }, 0);
}

fn inherited_control() -> OwnedFd {
    // SAFETY: spawn_role installs the dedicated control socket as stdin. Each
    // exact helper takes that descriptor's sole Rust ownership once.
    let control = unsafe { OwnedFd::from_raw_fd(libc::STDIN_FILENO) };
    rustix::io::fcntl_setfd(&control, rustix::io::FdFlags::CLOEXEC).unwrap();
    control
}

fn malformed() -> io::Error {
    io::Error::new(
        io::ErrorKind::InvalidData,
        "malformed fixture descriptor packet",
    )
}

fn frame(tag: &[u8; 4], pid: u32) -> [u8; 8] {
    let mut bytes = [0; 8];
    bytes[..4].copy_from_slice(tag);
    bytes[4..].copy_from_slice(&pid.to_le_bytes());
    bytes
}

fn send_packet(
    control: &OwnedFd,
    payload: &[u8],
    descriptors: &[std::os::fd::BorrowedFd<'_>],
) -> io::Result<()> {
    let mut space = [MaybeUninit::uninit(); rustix::cmsg_space!(ScmRights(3))];
    let mut ancillary = SendAncillaryBuffer::new(&mut space);
    if !descriptors.is_empty() && !ancillary.push(SendAncillaryMessage::ScmRights(descriptors)) {
        return Err(malformed());
    }
    let sent = sendmsg(
        control,
        &[IoSlice::new(payload)],
        &mut ancillary,
        SendFlags::NOSIGNAL | SendFlags::DONTWAIT,
    )?;
    if sent != payload.len() {
        return Err(malformed());
    }
    Ok(())
}

fn wait_readable(control: &OwnedFd, deadline: Instant) -> io::Result<()> {
    loop {
        let remaining = deadline.saturating_duration_since(Instant::now());
        if remaining.is_zero() {
            return Err(io::Error::new(
                io::ErrorKind::TimedOut,
                "fixture receive timed out",
            ));
        }
        let timeout = Timespec {
            tv_sec: i64::try_from(remaining.as_secs()).unwrap(),
            tv_nsec: i64::from(remaining.subsec_nanos()),
        };
        let mut descriptors = [PollFd::new(control, PollFlags::IN)];
        match poll(&mut descriptors, Some(&timeout)) {
            Ok(0) | Err(rustix::io::Errno::INTR) => continue,
            Ok(_) if descriptors[0].revents().contains(PollFlags::IN) => return Ok(()),
            Ok(_) => {
                return Err(io::Error::new(
                    io::ErrorKind::BrokenPipe,
                    "fixture control closed",
                ));
            }
            Err(error) => return Err(error.into()),
        }
    }
}

fn receive_packet<const N: usize>(
    control: &OwnedFd,
    deadline: Instant,
) -> io::Result<([u8; 8], [OwnedFd; N])> {
    loop {
        wait_readable(control, deadline)?;
        let mut payload = [0; 8];
        let mut vectors = [IoSliceMut::new(&mut payload)];
        let mut space = [MaybeUninit::uninit(); rustix::cmsg_space!(ScmRights(3))];
        let mut ancillary = RecvAncillaryBuffer::new(&mut space);
        let received = match recvmsg(
            control,
            &mut vectors,
            &mut ancillary,
            RecvFlags::DONTWAIT | RecvFlags::CMSG_CLOEXEC,
        ) {
            Ok(received) => received,
            Err(rustix::io::Errno::INTR | rustix::io::Errno::AGAIN) => continue,
            Err(error) => return Err(error.into()),
        };
        let mut descriptors: [Option<OwnedFd>; N] = std::array::from_fn(|_| None);
        let mut count = 0;
        let mut messages = 0;
        let mut invalid = received.bytes != payload.len()
            || received
                .flags
                .intersects(ReturnFlags::TRUNC | ReturnFlags::CTRUNC);
        // Drain before checking payload/ancillary validity: every installed FD
        // must acquire RAII ownership, including surplus and truncated packets.
        for message in ancillary.drain() {
            messages += 1;
            match message {
                RecvAncillaryMessage::ScmRights(rights) => {
                    for descriptor in rights {
                        if count < N {
                            descriptors[count] = Some(descriptor);
                        }
                        count += 1;
                    }
                }
                _ => invalid = true,
            }
        }
        if invalid || count != N || messages != usize::from(N != 0) {
            return Err(malformed());
        }
        return Ok((
            payload,
            descriptors.map(|fd| fd.expect("exact count checked")),
        ));
    }
}

#[test]
#[ignore = "opt-in isolated-container root coordinator for native supervisor custody"]
fn native_distinct_uid_supervisor_fixture() {
    assert_eq!(std::env::var(OPT_IN).as_deref(), Ok("1"));
    assert_eq!(rustix::process::geteuid().as_raw(), 0);
    assert_eq!(rustix::process::getegid().as_raw(), 0);
    assert!(
        std::path::Path::new("/.dockerenv").is_file(),
        "isolated Docker fixture only"
    );
    let status = std::fs::read_to_string("/proc/self/status").unwrap();
    let caps = status
        .lines()
        .find_map(|line| line.strip_prefix("CapEff:\t"))
        .unwrap();
    let caps = u64::from_str_radix(caps, 16).unwrap();
    // KILL is essential for cleanup after both children leave the root UID.
    let required = (1 << 5) | (1 << 6) | (1 << 7); // KILL, SETGID, SETUID.
    assert_eq!(
        caps & required,
        required,
        "root fixture lacks cleanup/credential capabilities"
    );

    let (anchor_control, child_control) = pair();
    let mut anchor = spawn_role(ANCHOR_HELPER, "anchor", ANCHOR_UID, child_control);
    let pid = anchor.0.id();
    let process = rustix::process::Pid::from_raw(i32::try_from(pid).unwrap()).unwrap();
    let pidfd = rustix::process::pidfd_open(process, rustix::process::PidfdFlags::empty()).unwrap();
    let (payload, [peer]) =
        receive_packet::<1>(&anchor_control, Instant::now() + IO_TIMEOUT).unwrap();
    assert_eq!(payload, frame(b"ANC2", pid));

    let (supervisor_control, child_control) = pair();
    let mut supervisor = spawn_role(
        SUPERVISOR_HELPER,
        "supervisor",
        SUPERVISOR_UID,
        child_control,
    );
    let deadline = Instant::now() + CHILD_TIMEOUT;
    send_packet(
        &supervisor_control,
        &payload,
        &[peer.as_fd(), pidfd.as_fd()],
    )
    .unwrap();
    drop((peer, pidfd));
    // A filtered libtest invocation can exit zero without running any tests.
    // Require this packet, emitted only after the actual owner cases return.
    let (completed, []) = receive_packet::<0>(&supervisor_control, deadline).unwrap();
    assert_eq!(completed, frame(b"DONE", pid));
    let supervisor_status = supervisor.wait_until(deadline).unwrap();
    send_packet(&anchor_control, &frame(b"STOP", pid), &[]).unwrap();
    let anchor_status = anchor.wait_until(Instant::now() + IO_TIMEOUT).unwrap();
    assert!(
        supervisor_status.success(),
        "supervisor fixture failed: {supervisor_status}"
    );
    assert!(
        anchor_status.success(),
        "anchor fixture failed: {anchor_status}"
    );
}

#[test]
#[ignore = "private anchor role, executed only by native_distinct_uid_supervisor_fixture"]
fn anchor_process_helper() {
    require_child_credentials("anchor", ANCHOR_UID);
    let control = inherited_control();
    // SO_PEERCRED records this process's final non-root credentials, not root's.
    let (transferred, held_peer) = pair();
    let pid = std::process::id();
    send_packet(&control, &frame(b"ANC2", pid), &[transferred.as_fd()]).unwrap();
    drop(transferred);
    let (payload, []) = receive_packet::<0>(&control, Instant::now() + ANCHOR_TIMEOUT).unwrap();
    assert_eq!(payload, frame(b"STOP", pid));
    drop(held_peer);
}

#[test]
#[ignore = "private non-root supervisor role, executed only by the root coordinator"]
fn supervisor_process_helper() {
    require_child_credentials("supervisor", SUPERVISOR_UID);
    let control = inherited_control();
    let (payload, [peer, pidfd]) =
        receive_packet::<2>(&control, Instant::now() + IO_TIMEOUT).unwrap();
    assert_eq!(&payload[..4], b"ANC2");
    let pid = u32::from_le_bytes(payload[4..].try_into().unwrap());
    assert_ne!(pid, 0);
    // Existing tests::Fixture creates and removes its service-owned 0700 root
    // under TMPDIR=/tmp after this process has dropped all root credentials.
    crate::authority_v2::tests::exercise(peer, pidfd);
    send_packet(&control, &frame(b"DONE", pid), &[]).unwrap();
}

#[test]
fn descriptor_packets_preserve_order_and_set_cloexec() {
    let (sender, receiver) = pair();
    let (first, second) = pair();
    send_packet(
        &sender,
        &frame(b"ANC2", 42),
        &[first.as_fd(), second.as_fd()],
    )
    .unwrap();
    let (payload, descriptors) =
        receive_packet::<2>(&receiver, Instant::now() + IO_TIMEOUT).unwrap();
    assert_eq!(payload, frame(b"ANC2", 42));
    for (received, expected) in descriptors.iter().zip([&first, &second]) {
        assert!(
            rustix::io::fcntl_getfd(received)
                .unwrap()
                .contains(rustix::io::FdFlags::CLOEXEC)
        );
        let actual = rustix::fs::fstat(received).unwrap();
        let expected = rustix::fs::fstat(expected).unwrap();
        assert_eq!(
            (actual.st_dev, actual.st_ino),
            (expected.st_dev, expected.st_ino)
        );
    }
}

#[test]
fn malformed_descriptor_packets_close_received_rights() {
    use std::os::unix::fs::MetadataExt;
    let (source, _held) = pair();
    let identity = rustix::fs::fstat(&source).unwrap();
    let references = || {
        std::fs::read_dir("/proc/self/fd")
            .unwrap()
            .filter_map(|entry| std::fs::metadata(entry.ok()?.path()).ok())
            .filter(|metadata| {
                (metadata.dev(), metadata.ino()) == (identity.st_dev, identity.st_ino)
            })
            .count()
    };
    let baseline = references();
    for (bytes, count) in [(7, 1), (9, 1), (8, 0), (8, 2), (8, 3)] {
        let (sender, receiver) = pair();
        send_packet(&sender, &vec![0; bytes], &vec![source.as_fd(); count]).unwrap();
        let error = receive_packet::<1>(&receiver, Instant::now() + IO_TIMEOUT).unwrap_err();
        assert_eq!(error.kind(), io::ErrorKind::InvalidData);
        assert_eq!(references(), baseline);
    }
}

#[test]
fn missing_packet_has_an_absolute_deadline() {
    let (_sender, receiver) = pair();
    let error = receive_packet::<0>(&receiver, Instant::now()).unwrap_err();
    assert_eq!(error.kind(), io::ErrorKind::TimedOut);
}

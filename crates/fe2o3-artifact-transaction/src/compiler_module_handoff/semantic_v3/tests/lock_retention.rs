use super::*;
use crate::CompilerModuleHandoffLockRetentionV3;
use std::io::{IoSlice, IoSliceMut};
use std::mem::MaybeUninit;
use std::os::fd::{AsRawFd, FromRawFd, OwnedFd};
use std::os::unix::process::{CommandExt, ExitStatusExt};
use std::process::{Child, Command, Stdio};

use rustix::net::{
    AddressFamily, RecvAncillaryBuffer, RecvAncillaryMessage, RecvFlags, ReturnFlags,
    SendAncillaryBuffer, SendAncillaryMessage, SendFlags, SocketFlags, SocketType, recvmsg,
    sendmsg, socketpair,
};

fn observe(
    path: &Path,
    producer: &ProducerIdentity,
    attempt: BuildAttempt,
) -> (
    CompilerModuleHandoffCurrentnessLeaseV3,
    CompilerModuleHandoffConsumptionTokenV3,
) {
    try_observe_compiler_module_handoff_currentness_in_slot_v3(
        path,
        producer,
        attempt,
        CompilerModuleHandoffSlotV3::Production,
    )
    .unwrap()
}

fn assert_busy(lease: &CompilerModuleHandoffCurrentnessLeaseV3) {
    assert!(matches!(
        lease.acquire_current_token(),
        Err(CompilerModuleHandoffErrorV3::Busy)
    ));
}

#[test]
fn retention_requires_nonrepairing_descriptor_root_and_preserves_both_locks() {
    let temp = TestDirectory::new();
    let producer = producer("observer_retention_v3");
    let attempt = begin(&temp.0, &producer, 31);
    let lease =
        publish_with_currentness(&temp.0, &producer, attempt, &outer(131)).into_current_lease();
    let regular = acquire_token(&lease);
    assert!(regular.retain_observed_lock_descriptors().is_err());
    drop(regular);
    let (_named, token) = observe(&temp.0, &producer, attempt);
    assert!(token.retain_observed_lock_descriptors().is_err());
    drop(token);
    let root = fs::File::open(&temp.0).unwrap();
    let procfd = PathBuf::from(format!("/proc/self/fd/{}", root.as_raw_fd()));
    let (_observed, token) = observe(&procfd, &producer, attempt);
    let retention = token.retain_observed_lock_descriptors().unwrap();
    let originals = [&token._lock.fd, &token._lock.root_guard];
    for (original, duplicate) in originals.into_iter().zip(retention.transfer_descriptors()) {
        let original = rustix::fs::fstat(original.as_ref().unwrap()).unwrap();
        let duplicate_stat = rustix::fs::fstat(duplicate).unwrap();
        assert_eq!(
            (original.st_dev, original.st_ino),
            (duplicate_stat.st_dev, duplicate_stat.st_ino)
        );
        assert_eq!(
            rustix::io::fcntl_getfd(duplicate).unwrap(),
            rustix::io::FdFlags::CLOEXEC
        );
    }
    drop(token);
    assert_busy(&lease);
    // Probe each independent lock, so keeping only the named-file lock cannot pass this test.
    let named_probe = fs::OpenOptions::new()
        .read(true)
        .write(true)
        .open(temp.0.join(crate::LOCK_FILE))
        .unwrap();
    let named_probe: OwnedFd = named_probe.into();
    assert!(!crate::acquire_linux_ofd_exclusive_lock(&named_probe, true).unwrap());
    let root_probe: OwnedFd = fs::File::open(&temp.0).unwrap().into();
    assert!(!crate::acquire_linux_descriptor_flock(&root_probe, true).unwrap());
    drop(retention);
    assert!(crate::acquire_linux_ofd_exclusive_lock(&named_probe, true).unwrap());
    assert!(crate::acquire_linux_descriptor_flock(&root_probe, true).unwrap());
    drop(named_probe);
    drop(root_probe);
    lease.acquire_current_token().unwrap();
}

#[test]
fn retention_drop_obeys_the_artifact_spawn_barrier() {
    let temp = TestDirectory::new();
    let producer = producer("observer_retention_spawn_v3");
    let attempt = begin(&temp.0, &producer, 32);
    let lease =
        publish_with_currentness(&temp.0, &producer, attempt, &outer(132)).into_current_lease();
    let root = fs::File::open(&temp.0).unwrap();
    let procfd = PathBuf::from(format!("/proc/self/fd/{}", root.as_raw_fd()));
    let (_observed, token) = observe(&procfd, &producer, attempt);
    let retention = token.retain_observed_lock_descriptors().unwrap();
    drop(token);
    let spawn = crate::ArtifactProcessSpawnCoordinatorV1::global().begin_spawn();
    let (started_tx, started_rx) = std::sync::mpsc::channel();
    let (done_tx, done_rx) = std::sync::mpsc::channel();
    let thread = std::thread::spawn(move || {
        started_tx.send(()).unwrap();
        drop(retention);
        done_tx.send(()).unwrap();
    });
    started_rx
        .recv_timeout(std::time::Duration::from_secs(5))
        .unwrap();
    assert!(
        done_rx
            .recv_timeout(std::time::Duration::from_millis(100))
            .is_err()
    );
    assert_busy(&lease);
    drop(spawn);
    done_rx
        .recv_timeout(std::time::Duration::from_secs(5))
        .unwrap();
    thread.join().unwrap();
    lease.acquire_current_token().unwrap();
}

fn unlocked_descriptors(path: &Path) -> [OwnedFd; 2] {
    [
        fs::OpenOptions::new()
            .read(true)
            .write(true)
            .open(path.join(crate::LOCK_FILE))
            .unwrap()
            .into(),
        fs::File::open(path).unwrap().into(),
    ]
}

#[test]
fn receiving_checks_shape_not_lock_or_semantic_authority() {
    let temp = TestDirectory::new();
    let producer = producer("observer_retention_import_v3");
    let attempt = begin(&temp.0, &producer, 33);
    let lease =
        publish_with_currentness(&temp.0, &producer, attempt, &outer(133)).into_current_lease();
    // Shape-valid reopened descriptions are deliberately not evidence of retained locks.
    let inert = CompilerModuleHandoffLockRetentionV3::from_received_descriptors(
        unlocked_descriptors(&temp.0),
    )
    .unwrap();
    lease.acquire_current_token().unwrap();
    drop(inert);
    let [named, root] = unlocked_descriptors(&temp.0);
    assert!(
        CompilerModuleHandoffLockRetentionV3::from_received_descriptors([root, named]).is_err()
    );
    let descriptors = unlocked_descriptors(&temp.0);
    rustix::io::fcntl_setfd(&descriptors[0], rustix::io::FdFlags::empty()).unwrap();
    assert!(CompilerModuleHandoffLockRetentionV3::from_received_descriptors(descriptors).is_err());
    let [named, root] = unlocked_descriptors(&temp.0);
    rustix::fs::fcntl_setfl(&named, rustix::fs::OFlags::APPEND).unwrap();
    assert!(
        CompilerModuleHandoffLockRetentionV3::from_received_descriptors([named, root]).is_err()
    );
    let read_only: OwnedFd = fs::File::open(temp.0.join(crate::LOCK_FILE))
        .unwrap()
        .into();
    let directory: OwnedFd = fs::File::open(&temp.0).unwrap().into();
    assert!(
        CompilerModuleHandoffLockRetentionV3::from_received_descriptors([read_only, directory])
            .is_err()
    );
    let (socket, _) = socketpair(
        AddressFamily::UNIX,
        SocketType::SEQPACKET,
        SocketFlags::CLOEXEC,
        None,
    )
    .unwrap();
    let [_, root] = unlocked_descriptors(&temp.0);
    assert!(
        CompilerModuleHandoffLockRetentionV3::from_received_descriptors([socket, root]).is_err()
    );
    lease.acquire_current_token().unwrap();
}

#[test]
fn rejected_import_releases_last_lock_aliases() {
    let temp = TestDirectory::new();
    let producer = producer("observer_retention_rejected_import_v3");
    let attempt = begin(&temp.0, &producer, 35);
    let lease =
        publish_with_currentness(&temp.0, &producer, attempt, &outer(135)).into_current_lease();
    let root = fs::File::open(&temp.0).unwrap();
    let procfd = PathBuf::from(format!("/proc/self/fd/{}", root.as_raw_fd()));
    let (_observed, token) = observe(&procfd, &producer, attempt);
    let retention = token.retain_observed_lock_descriptors().unwrap();
    let descriptors = retention
        .transfer_descriptors()
        .map(|fd| rustix::io::fcntl_dupfd_cloexec(fd, 0).unwrap());
    drop(token);
    drop(retention);
    assert_busy(&lease);
    let [named, root] = descriptors;
    assert!(
        CompilerModuleHandoffLockRetentionV3::from_received_descriptors([root, named]).is_err()
    );
    lease.acquire_current_token().unwrap();
}

fn send_retention(peer: &OwnedFd, retention: &CompilerModuleHandoffLockRetentionV3) {
    let descriptors = retention.transfer_descriptors();
    let mut space = [MaybeUninit::uninit(); rustix::cmsg_space!(ScmRights(2))];
    let mut ancillary = SendAncillaryBuffer::new(&mut space);
    assert!(ancillary.push(SendAncillaryMessage::ScmRights(&descriptors)));
    assert_eq!(
        sendmsg(
            peer,
            &[IoSlice::new(b"retained")],
            &mut ancillary,
            SendFlags::NOSIGNAL | SendFlags::DONTWAIT
        )
        .unwrap(),
        8
    );
}

fn receive_retention(peer: &OwnedFd) -> CompilerModuleHandoffLockRetentionV3 {
    let mut bytes = [0; 8];
    let mut space = [MaybeUninit::uninit(); rustix::cmsg_space!(ScmRights(2))];
    let mut ancillary = RecvAncillaryBuffer::new(&mut space);
    let received = recvmsg(
        peer,
        &mut [IoSliceMut::new(&mut bytes)],
        &mut ancillary,
        RecvFlags::CMSG_CLOEXEC | RecvFlags::DONTWAIT,
    )
    .unwrap();
    assert_eq!(received.bytes, 8);
    assert_eq!(&bytes, b"retained");
    assert!(
        !received
            .flags
            .intersects(ReturnFlags::TRUNC | ReturnFlags::CTRUNC)
    );
    let mut descriptors = Vec::new();
    for message in ancillary.drain() {
        match message {
            RecvAncillaryMessage::ScmRights(rights) => descriptors.extend(rights),
            _ => panic!("unexpected ancillary message"),
        }
    }
    CompilerModuleHandoffLockRetentionV3::from_received_descriptors(descriptors.try_into().unwrap())
        .unwrap()
}

fn await_packet(peer: &OwnedFd) {
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
    loop {
        let remaining = deadline
            .checked_duration_since(std::time::Instant::now())
            .expect("observer helper did not transfer locks before the deadline");
        let mut event = libc::pollfd {
            fd: peer.as_raw_fd(),
            events: libc::POLLIN,
            revents: 0,
        };
        // SAFETY: one initialized pollfd remains exclusively borrowed during this bounded poll.
        let result = unsafe { libc::poll(&mut event, 1, remaining.as_millis().max(1) as i32) };
        if result < 0 && std::io::Error::last_os_error().kind() == std::io::ErrorKind::Interrupted {
            continue;
        }
        assert!(result > 0, "observer helper did not transfer locks");
        assert_ne!(event.revents & libc::POLLIN, 0);
        assert_eq!(event.revents & (libc::POLLERR | libc::POLLNVAL), 0);
        return;
    }
}

struct OwnedChild(Child);

impl Drop for OwnedChild {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

#[test]
#[ignore = "owned subprocess helper for the abrupt observer-death test"]
fn observer_lock_sender() {
    let attempt = BuildAttempt::from_env_value(
        &std::env::var("FE2O3_TEST_OBSERVER_ATTEMPT").expect("parent-owned helper invocation"),
    )
    .unwrap();
    // SAFETY: the parent installs these exact owned descriptors only in this helper process.
    let (peer, root) = unsafe { (OwnedFd::from_raw_fd(197), OwnedFd::from_raw_fd(198)) };
    rustix::io::fcntl_setfd(&peer, rustix::io::FdFlags::CLOEXEC).unwrap();
    rustix::io::fcntl_setfd(&root, rustix::io::FdFlags::CLOEXEC).unwrap();
    let (_lease, token) = observe(
        Path::new("/proc/self/fd/198"),
        &producer("observer_transfer_v3"),
        attempt,
    );
    let retention = token.retain_observed_lock_descriptors().unwrap();
    send_retention(&peer, &retention);
    loop {
        std::thread::park();
    }
}

#[test]
fn transferred_and_queued_rights_survive_abrupt_observer_death_and_lock_replacement() {
    enum Delivery {
        Received,
        Queued,
        Discard,
    }
    for delivery in [Delivery::Received, Delivery::Queued, Delivery::Discard] {
        let temp = TestDirectory::new();
        let producer = producer("observer_transfer_v3");
        let attempt = begin(&temp.0, &producer, 34);
        let lease =
            publish_with_currentness(&temp.0, &producer, attempt, &outer(134)).into_current_lease();
        let root = fs::File::open(&temp.0).unwrap();
        let (receiver, sender) = socketpair(
            AddressFamily::UNIX,
            SocketType::SEQPACKET,
            SocketFlags::CLOEXEC | SocketFlags::NONBLOCK,
            None,
        )
        .unwrap();
        // Stage above the fixed destinations to avoid descriptor-number collisions in pre_exec.
        let sender = rustix::io::fcntl_dupfd_cloexec(&sender, 256).unwrap();
        let root = rustix::io::fcntl_dupfd_cloexec(&root, 256).unwrap();
        let sender_raw = sender.as_raw_fd();
        let root_raw = root.as_raw_fd();
        let mut command = Command::new(std::env::current_exe().unwrap());
        command
            .args([
                "--exact",
                "compiler_module_handoff::semantic_v3::tests::lock_retention::observer_lock_sender",
                "--ignored",
                "--nocapture",
                "--test-threads=1",
            ])
            .env("FE2O3_TEST_OBSERVER_ATTEMPT", attempt.to_env_value())
            .stdin(Stdio::null())
            .stdout(Stdio::null());
        // SAFETY: only async-signal-safe dup2 runs in the child before exec.
        unsafe {
            command.pre_exec(move || {
                if libc::dup2(sender_raw, 197) != 197 || libc::dup2(root_raw, 198) != 198 {
                    return Err(std::io::Error::last_os_error());
                }
                Ok(())
            });
        }
        let mut child =
            OwnedChild(crate::with_artifact_process_spawn_v1(|| command.spawn()).unwrap());
        drop(sender);
        drop(root);
        await_packet(&receiver);
        let retention =
            matches!(delivery, Delivery::Received).then(|| receive_retention(&receiver));
        child.0.kill().unwrap();
        assert_eq!(child.0.wait().unwrap().signal(), Some(libc::SIGKILL));
        assert_busy(&lease);
        if matches!(delivery, Delivery::Discard) {
            drop(receiver);
            lease.acquire_current_token().unwrap();
            continue;
        }
        let retention = retention.unwrap_or_else(|| receive_retention(&receiver));
        assert_busy(&lease);
        // If the named lock is replaced, the independent root flock must still exclude writers.
        fs::remove_file(temp.0.join(crate::LOCK_FILE)).unwrap();
        assert_busy(&lease);
        drop(retention);
        lease.acquire_current_token().unwrap();
    }
}

//! Real ancillary-message checks; no compiler or deployment authority is made.
use super::*;
use rustix::net::{SendAncillaryBuffer, SendAncillaryMessage, SendFlags};
use std::{
    io::IoSlice,
    os::fd::{AsFd, BorrowedFd},
};

fn control() -> (OwnedFd, OwnedFd) {
    let (receiver, sender) = seqpacket_pair().unwrap();
    rustix::net::sockopt::set_socket_passcred(&receiver, true).unwrap();
    (receiver, sender)
}

fn payload() -> [u8; TRANSFER_BYTES] {
    let mut bytes = [0; TRANSFER_BYTES];
    bytes[..8].copy_from_slice(&TRANSFER_MAGIC);
    bytes[8..12].copy_from_slice(&TRANSFER_VERSION.to_le_bytes());
    bytes[12..16].copy_from_slice(&std::process::id().to_le_bytes());
    bytes[16..20].copy_from_slice(&COMPILER_EXECUTION_SERVICE_CHILD_FD_V1.to_le_bytes());
    bytes[20..24].copy_from_slice(
        &rustix::process::getppid()
            .unwrap()
            .as_raw_pid()
            .to_le_bytes(),
    );
    bytes
}

fn send(sender: &OwnedFd, bytes: &[u8], descriptors: &[BorrowedFd<'_>]) {
    let mut space = [MaybeUninit::uninit(); rustix::cmsg_space!(ScmRights(4))];
    let mut ancillary = SendAncillaryBuffer::new(&mut space);
    if !descriptors.is_empty() {
        assert!(ancillary.push(SendAncillaryMessage::ScmRights(descriptors)));
    }
    assert_eq!(
        rustix::net::sendmsg(
            sender,
            &[IoSlice::new(bytes)],
            &mut ancillary,
            SendFlags::NOSIGNAL
        )
        .unwrap(),
        bytes.len()
    );
}

fn eof(peer: &OwnedFd) {
    let mut byte = [0];
    assert_eq!(
        rustix::net::recv(peer, &mut byte, RecvFlags::DONTWAIT).unwrap(),
        (0, 0)
    );
}

#[test]
fn transfer_binds_payload_and_socket_to_actual_sender() {
    let (receiver, sender) = control();
    let (peer, remote) = seqpacket_pair().unwrap();
    send(&sender, &payload(), &[peer.as_fd()]);
    drop(peer);
    let (received, pid, parent) = receive_service_peer(&receiver).unwrap();
    assert_eq!(pid, std::process::id());
    assert_eq!(
        parent,
        rustix::process::getppid().unwrap().as_raw_pid() as u32
    );
    require_close_on_exec(&received).unwrap();
    assert_eq!(peer_identity(&received).unwrap().pid(), pid);
    drop(received);
    eof(&remote);
}

#[test]
fn claimed_sender_mismatch_closes_received_rights() {
    let (receiver, sender) = control();
    let (peer, remote) = seqpacket_pair().unwrap();
    let mut bytes = payload();
    // A valid nonzero PID in the record cannot replace kernel sender credentials.
    bytes[12..16].copy_from_slice(&u32::MAX.to_le_bytes());
    send(&sender, &bytes, &[peer.as_fd()]);
    drop(peer);
    assert!(matches!(
        receive_service_peer(&receiver),
        Err(CompilerExecutionChildChannelErrorV1::TransferCredentialsMismatch)
    ));
    eof(&remote);
}

#[test]
fn relaying_another_process_socket_is_refused() {
    let (receiver, sender) = control();
    let (peer, remote) = seqpacket_pair().unwrap();
    let mut command = Command::new("/bin/true");
    // SAFETY: the closure owns both descriptors and the existing sender performs
    // only stack/scalar async-signal-safe operations before exec. It creates no
    // new socket: the relayed socket still identifies this parent, not the sender.
    unsafe {
        command.pre_exec(move || send_service_peer(sender.as_raw_fd(), peer.as_raw_fd()));
    }
    assert!(command.status().unwrap().success());
    drop(command);
    assert!(matches!(
        receive_service_peer(&receiver),
        Err(CompilerExecutionChildChannelErrorV1::TransferCredentialsMismatch)
    ));
    eof(&remote);
}

#[test]
fn malformed_transfer_closes_all_received_rights() {
    for case in 0..7 {
        let (receiver, sender) = control();
        let pairs: Vec<_> = (0..4).map(|_| seqpacket_pair().unwrap()).collect();
        let mut bytes = payload().to_vec();
        match case {
            0 => {
                bytes.pop();
            }
            1 => bytes.push(0),
            2 => bytes[0] ^= 1,
            3 => bytes[8] ^= 1,
            4 => bytes[16] ^= 1,
            _ => {}
        }
        let count = match case {
            5 => 2,
            6 => 4,
            _ => 1,
        };
        let descriptors: Vec<_> = pairs.iter().take(count).map(|(fd, _)| fd.as_fd()).collect();
        send(&sender, &bytes, &descriptors);
        drop(descriptors);
        let remotes: Vec<_> = pairs
            .into_iter()
            .map(|(fd, remote)| {
                drop(fd);
                remote
            })
            .collect();
        assert!(
            matches!(
                receive_service_peer(&receiver),
                Err(CompilerExecutionChildChannelErrorV1::MalformedTransfer)
            ),
            "case {case}"
        );
        for remote in &remotes {
            eof(remote);
        }
    }
}

#[test]
fn missing_rights_or_disabled_credentials_refuse() {
    let (receiver, sender) = control();
    send(&sender, &payload(), &[]);
    assert!(matches!(
        receive_service_peer(&receiver),
        Err(CompilerExecutionChildChannelErrorV1::MalformedTransfer)
    ));
    rustix::net::sockopt::set_socket_passcred(&receiver, false).unwrap();
    assert!(matches!(
        receive_service_peer(&receiver),
        Err(CompilerExecutionChildChannelErrorV1::TransferCredentialsMismatch)
    ));
}

#[test]
fn credentials_require_every_identity_field_and_presence() {
    let (peer, _) = seqpacket_pair().unwrap();
    let actual = rustix::net::sockopt::socket_peercred(&peer).unwrap();
    let client = peer_identity(&peer).unwrap();
    require_transfer_credentials(Some(actual), client.pid(), client).unwrap();
    assert!(require_transfer_credentials(None, client.pid(), client).is_err());
    for (pid, uid, gid) in [
        (u32::MAX, client.uid(), client.gid()),
        (client.pid(), client.uid() ^ 1, client.gid()),
        (client.pid(), client.uid(), client.gid() ^ 1),
    ] {
        let different = CompilerExecutionClientProcessIdentityV1::new(pid, uid, gid).unwrap();
        assert!(matches!(
            require_transfer_credentials(Some(actual), client.pid(), different),
            Err(CompilerExecutionChildChannelErrorV1::TransferCredentialsMismatch)
        ));
    }
}

#[test]
fn receiver_swap_keeps_message_credentials_enabled() {
    const CASE: &str = "FE2O3_CHILD_CHANNEL_RECEIVER_SWAP";
    if std::env::var_os(CASE).is_none() {
        let output = Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "child_channel::transfer_tests::receiver_swap_keeps_message_credentials_enabled",
                "--nocapture",
                "--test-threads=1",
            ])
            .env(CASE, "1")
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(String::from_utf8_lossy(&output.stdout).contains("1 passed"));
        return;
    }
    let source = std::fs::File::open("/dev/null").unwrap();
    let mut fillers = Vec::new();
    loop {
        let fd = rustix::io::fcntl_dupfd_cloexec(&source, 3).unwrap();
        let raw = fd.as_raw_fd();
        if raw == COMPILER_EXECUTION_SERVICE_CHILD_FD_V1 {
            drop(fd);
            break;
        }
        assert!(raw < COMPILER_EXECUTION_SERVICE_CHILD_FD_V1);
        fillers.push(fd);
    }
    let mut command = Command::new("/bin/sleep");
    command.arg("30");
    let pending = PendingCompilerExecutionChildChannelV1::prepare(&mut command).unwrap();
    assert_eq!(
        pending.receiver.as_raw_fd(),
        COMPILER_EXECUTION_SERVICE_CHILD_FD_V1 + 1
    );
    assert!(rustix::net::sockopt::socket_passcred(&pending.receiver).unwrap());
    let mut child = command.spawn().unwrap();
    let result = pending.finish(child.id(), Duration::from_secs(2));
    // Clean the owned diagnostic child even when admission fails.
    child.kill().unwrap();
    child.wait().unwrap();
    assert_eq!(result.unwrap().client().pid(), child.id());
}

#[test]
#[ignore = "requires an isolated root process with SETUID/SETGID; no deployment authority"]
fn dropped_child_credentials_do_not_admit_root_handoff() {
    assert_eq!(rustix::process::getuid().as_raw(), 0);
    assert_eq!(rustix::process::geteuid().as_raw(), 0);
    let mut command = Command::new("/bin/sleep");
    command.arg("30").uid(65534).gid(65534);
    let pending = PendingCompilerExecutionChildChannelV1::prepare(&mut command).unwrap();
    let mut child = command.spawn().unwrap();
    let result = pending.finish(child.id(), Duration::from_secs(2));
    child.kill().unwrap();
    child.wait().unwrap();
    assert!(
        matches!(
            result,
            Err(CompilerExecutionChildChannelErrorV1::ParentCredentialsMismatch)
        ),
        "{result:?}"
    );
}

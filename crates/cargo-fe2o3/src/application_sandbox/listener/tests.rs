use super::*;
use rustix::io::{FdFlags, fcntl_getfd, read};
use rustix::net::{SendAncillaryBuffer, SendAncillaryMessage, SendFlags, sendmsg};
use rustix::pipe::{PipeFlags, pipe_with};
use std::io::IoSlice;
use std::os::fd::AsFd;
use std::os::unix::net::UnixDatagram;

fn isolated_case(name: &str, run: impl FnOnce()) {
    const ENV: &str = "FE2O3_LISTENER_OWNERSHIP_TEST";
    // This module is also included by the compiler-execution-client tests.
    let (_, module) = module_path!().split_once("::").unwrap();
    let name = format!("{module}::{name}");
    let marker = format!("listener-ownership-completed:{name}");
    if let Some(selected) = std::env::var_os(ENV) {
        assert_eq!(selected, std::ffi::OsStr::new(&name));
        run();
        println!("\n{marker}");
        return;
    }
    // Create pipe owners only after exec: unrelated test forks can otherwise
    // retain transient CLOEXEC aliases and prevent the expected EOF.
    let output = std::process::Command::new(std::env::current_exe().unwrap())
        .args(["--exact", &name, "--nocapture", "--test-threads=1"])
        .env(ENV, &name)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "listener fixture failed: {}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr),
    );
    assert_eq!(
        std::str::from_utf8(&output.stdout)
            .unwrap()
            .lines()
            .filter(|line| *line == marker)
            .count(),
        1,
    );
}

fn packet() -> [u8; 16] {
    let mut bytes = [0; 16];
    bytes[..8].copy_from_slice(&LISTENER_MESSAGE_MAGIC);
    bytes[8..12].copy_from_slice(&std::process::id().to_ne_bytes());
    bytes
}

fn send(socket: &UnixDatagram, bytes: &[u8], rights: &[BorrowedFd<'_>]) {
    let mut storage = [MaybeUninit::uninit(); rustix::cmsg_space!(ScmRights(4))];
    let mut ancillary = SendAncillaryBuffer::new(&mut storage);
    if !rights.is_empty() {
        assert!(ancillary.push(SendAncillaryMessage::ScmRights(rights)));
    }
    assert_eq!(
        sendmsg(
            socket,
            &[IoSlice::new(bytes)],
            &mut ancillary,
            SendFlags::empty()
        )
        .unwrap(),
        bytes.len()
    );
}

#[test]
fn rejected_listener_packets_close_every_received_pipe_writer() {
    isolated_case(
        "rejected_listener_packets_close_every_received_pipe_writer",
        rejected_packets,
    );
}

fn rejected_packets() {
    let good = packet();
    let mut bad_magic = good;
    bad_magic[0] ^= 1;
    let mut zero_pid = good;
    zero_pid[8..12].fill(0);
    let mut reserved = good;
    reserved[12] = 1;
    let cases = [
        (bad_magic.to_vec(), 1),
        (zero_pid.to_vec(), 1),
        (reserved.to_vec(), 1),
        (good[..15].to_vec(), 1),
        ([good.as_slice(), &[0]].concat(), 1),
        (good.to_vec(), 2),
        (good.to_vec(), 4),
        (good.to_vec(), 0),
    ];
    for (bytes, count) in cases {
        let (parent, child) = UnixDatagram::pair().unwrap();
        let (readers, writers): (Vec<_>, Vec<_>) = (0..count)
            .map(|_| pipe_with(PipeFlags::CLOEXEC | PipeFlags::NONBLOCK).unwrap())
            .unzip();
        let rights: Vec<_> = writers.iter().map(AsFd::as_fd).collect();
        send(&child, &bytes, &rights);
        drop(rights);
        drop(writers);
        assert!(receive_listener(parent.as_fd()).is_err());
        for reader in readers {
            assert_eq!(
                read(&reader, &mut [0]).unwrap(),
                0,
                "received writer leaked"
            );
        }
    }
}

#[test]
fn accepted_listener_keeps_one_cloexec_owner_until_drop() {
    isolated_case(
        "accepted_listener_keeps_one_cloexec_owner_until_drop",
        accepted_packet,
    );
}

fn accepted_packet() {
    let (parent, child) = UnixDatagram::pair().unwrap();
    let (reader, writer) = pipe_with(PipeFlags::CLOEXEC | PipeFlags::NONBLOCK).unwrap();
    send(&child, &packet(), &[writer.as_fd()]);
    drop(writer);
    let (listener, pid) = receive_listener(parent.as_fd()).unwrap();
    assert_eq!(pid, std::process::id());
    assert!(fcntl_getfd(&listener).unwrap().contains(FdFlags::CLOEXEC));
    assert_eq!(read(&reader, &mut [0]), Err(rustix::io::Errno::AGAIN));
    drop(listener);
    assert_eq!(read(&reader, &mut [0]).unwrap(), 0);
}

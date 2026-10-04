use super::*;
use fe2o3_protected_service_profile::{
    validate_proof_controller_process_v1, validate_protected_service_process_v1,
};
use rustix::net::{
    self, RecvAncillaryBuffer, RecvAncillaryMessage, RecvFlags, SendAncillaryBuffer,
    SendAncillaryMessage, SendFlags,
};
use std::io::{IoSlice, IoSliceMut};
use std::os::unix::process::CommandExt;
use std::time::{Duration, Instant};

std::thread_local! {
    static FORCE_LEGACY: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
    static USED_LEGACY: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

pub(super) fn take_force_legacy() -> bool {
    FORCE_LEGACY.replace(false)
}
pub(super) fn record_legacy_route() {
    USED_LEGACY.set(true);
}

core::arch::global_asm!(
    include_str!("parent_death_test_x86_64.S"),
    options(att_syntax)
);
unsafe extern "C" {
    static fe2o3_parent_death_test_start: u8;
    static fe2o3_parent_death_test_end: u8;
}

fn parent_death_image() -> File {
    let start = &raw const fe2o3_parent_death_test_start;
    let end = &raw const fe2o3_parent_death_test_end;
    let size = end.addr().checked_sub(start.addr()).unwrap();
    assert!((32..4096).contains(&size));
    // SAFETY: these symbols bound the single position-independent test assembly section.
    tests::static_image(unsafe { std::slice::from_raw_parts(start, size) })
}

fn wait_readable(fd: BorrowedFd<'_>) {
    let mut poll = [rustix::event::PollFd::new(
        &fd,
        rustix::event::PollFlags::IN,
    )];
    assert_eq!(
        rustix::event::poll(
            &mut poll,
            Some(&rustix::event::Timespec {
                tv_sec: 10,
                tv_nsec: 0,
            })
        )
        .unwrap(),
        1,
        "fixture descriptor timed out"
    );
}

fn read_byte(fd: BorrowedFd<'_>) -> u8 {
    wait_readable(fd);
    let mut byte = [0];
    assert_eq!(rustix::io::read(fd, &mut byte).unwrap(), 1);
    byte[0]
}

fn send_pidfd(socket: BorrowedFd<'_>, child: &RootOwnedProtectedServiceChildV1) {
    let pid = child.pid().as_raw_pid().to_le_bytes();
    let fds = [child.pidfd().as_fd()];
    let mut space = [std::mem::MaybeUninit::uninit(); rustix::cmsg_space!(ScmRights(1))];
    let mut ancillary = SendAncillaryBuffer::new(&mut space);
    assert!(ancillary.push(SendAncillaryMessage::ScmRights(&fds)));
    assert_eq!(
        net::sendmsg(
            socket,
            &[IoSlice::new(&pid)],
            &mut ancillary,
            SendFlags::NOSIGNAL
        )
        .unwrap(),
        4
    );
}

fn receive_pidfd(socket: BorrowedFd<'_>) -> (i32, OwnedFd) {
    wait_readable(socket);
    let mut pid = [0; 4];
    let mut space = [std::mem::MaybeUninit::uninit(); rustix::cmsg_space!(ScmRights(1))];
    let mut ancillary = RecvAncillaryBuffer::new(&mut space);
    let message = net::recvmsg(
        socket,
        &mut [IoSliceMut::new(&mut pid)],
        &mut ancillary,
        RecvFlags::CMSG_CLOEXEC,
    )
    .unwrap();
    assert_eq!(message.bytes, 4);
    assert!(
        !message
            .flags
            .intersects(net::ReturnFlags::TRUNC | net::ReturnFlags::CTRUNC)
    );
    let mut fds = Vec::new();
    for value in ancillary.drain() {
        match value {
            RecvAncillaryMessage::ScmRights(rights) => fds.extend(rights),
            _ => panic!("unexpected ancillary"),
        }
    }
    assert_eq!(fds.len(), 1);
    (i32::from_le_bytes(pid), fds.pop().unwrap())
}

#[test]
#[ignore = "private real-root helper; invoked by root_parent_death_matrix"]
fn root_parent_death_spawn_helper() {
    assert!(has_exact_root_identity());
    // SAFETY: the private parent installs sole owned control descriptor 198 at exec.
    let control = unsafe { OwnedFd::from_raw_fd(198) };
    let role = std::env::var("FE2O3_PDEATH_ROLE").unwrap();
    let legacy = std::env::var("FE2O3_PDEATH_ROUTE").unwrap() == "legacy";
    let image = if role.ends_with("-static") {
        let bytes = std::fs::read(
            std::env::var_os("FE2O3_PROOF_PROFILE_FIXTURE").expect("static fixture required"),
        )
        .unwrap();
        assert!(bytes.len() < 32 * 1024 * 1024);
        tests::sealed_executable(&bytes)
    } else {
        parent_death_image()
    };
    let (ready_read, ready_write) =
        rustix::pipe::pipe_with(rustix::pipe::PipeFlags::CLOEXEC).unwrap();
    let (gate_read, gate_write) =
        rustix::pipe::pipe_with(rustix::pipe::PipeFlags::CLOEXEC).unwrap();
    let (status_read, status_write) =
        rustix::pipe::pipe_with(rustix::pipe::PipeFlags::CLOEXEC).unwrap();
    let bindings = [ProtectedServiceDescriptorBindingV1::new(control.as_fd(), 3).unwrap()];
    FORCE_LEGACY.set(legacy);
    USED_LEGACY.set(false);
    let child = if role.starts_with("proof") {
        let staged = crate::StagedProofControllerExecV1::new(
            &image,
            &bindings,
            ready_write.as_fd(),
            gate_read.as_fd(),
            status_write.as_fd(),
        )
        .unwrap();
        staged
            .spawn(ProofControllerCredentialProfileV1::new(61000, 61000).unwrap())
            .unwrap()
            .inner
    } else {
        assert!(role == "locked" || role == "locked-static");
        let staged = crate::StagedProtectedServiceExecV1::new(
            &image,
            &bindings,
            ready_write.as_fd(),
            gate_read.as_fd(),
            status_write.as_fd(),
        )
        .unwrap();
        staged
            .spawn(ProtectedServiceCredentialProfileV1::new(61000, 61000).unwrap())
            .unwrap()
            .inner
    };
    assert_eq!(
        USED_LEGACY.get(),
        legacy,
        "native case must actually use clone3"
    );
    drop((ready_write, gate_read, status_write));
    assert_eq!(
        read_byte(ready_read.as_fd()),
        PROTECTED_SERVICE_PROFILE_READY_V1
    );
    if role.starts_with("proof") {
        validate_proof_controller_process_v1(
            ProofControllerCredentialProfileV1::new(61000, 61000).unwrap(),
            child.pid(),
        )
        .unwrap();
    } else {
        validate_protected_service_process_v1(
            ProtectedServiceCredentialProfileV1::new(61000, 61000).unwrap(),
            child.pid(),
        )
        .unwrap();
    }
    send_pidfd(control.as_fd(), &child);
    assert_eq!(
        rustix::io::write(&gate_write, &[PROTECTED_SERVICE_GATE_RELEASE_V1]).unwrap(),
        1
    );
    wait_readable(status_read.as_fd());
    assert_eq!(rustix::io::read(&status_read, &mut [0]).unwrap(), 0);
    assert_eq!(read_byte(control.as_fd()), 0xac);
    // SAFETY: intentional abrupt direct-parent death; no Drop, pipe EOF or orderly cancellation.
    unsafe { libc::_exit(0) }
}

#[test]
#[ignore = "requires private real-root PID namespace; four post-exec parent-death cases"]
fn root_parent_death_matrix() {
    parent_death_matrix(&["locked", "proof"]);
}

#[test]
#[ignore = "requires isolated root and an independently built static secure-entry profile fixture"]
fn root_static_proof_profile_matrix() {
    parent_death_matrix(&["proof-static", "locked-static"]);
}

fn parent_death_matrix(roles: &[&str]) {
    assert!(has_exact_root_identity());
    // SAFETY: this disposable test subprocess owns all descendants in a private namespace.
    assert_eq!(
        unsafe { libc::prctl(libc::PR_SET_CHILD_SUBREAPER, 1, 0, 0, 0) },
        0
    );
    for role in roles {
        for route in ["native", "legacy"] {
            let (controller, helper) = net::socketpair(
                net::AddressFamily::UNIX,
                net::SocketType::SEQPACKET,
                net::SocketFlags::CLOEXEC,
                None,
            )
            .unwrap();
            let fd = helper.as_raw_fd();
            let mut command = std::process::Command::new(std::env::current_exe().unwrap());
            command
                .args([
                    "--exact",
                    "syscall::proof_controller_tests::root_parent_death_spawn_helper",
                    "--ignored",
                    "--nocapture",
                    "--test-threads=1",
                ])
                .env("FE2O3_PDEATH_ROLE", role)
                .env("FE2O3_PDEATH_ROUTE", route);
            // SAFETY: only async-signal-safe descriptor duplication runs in the pre-exec child.
            unsafe {
                command.pre_exec(move || {
                    if libc::dup3(fd, 198, 0) != 198 {
                        return Err(io::Error::last_os_error());
                    }
                    Ok(())
                });
            }
            let mut process = command.spawn().unwrap();
            drop(helper);
            let (pid, pidfd) = receive_pidfd(controller.as_fd());
            // Keep cleanup armed even when a subsequent assertion fails.
            let mut custody = RootOwnedProtectedServiceChildV1 {
                pid: rustix::process::Pid::from_raw(pid).unwrap(),
                pidfd: Some(pidfd),
                reaping_ownership_lost: false,
            };
            if *role == "locked-static" {
                wait_readable(custody.pidfd().as_fd());
            } else {
                assert_eq!(
                    read_byte(controller.as_fd()),
                    9,
                    "post-exec PDEATHSIG readback"
                );
            }
            assert_eq!(rustix::io::write(&controller, &[0xac]).unwrap(), 1);
            let deadline = Instant::now() + Duration::from_secs(10);
            loop {
                if let Some(status) = process.try_wait().unwrap() {
                    assert!(status.success());
                    break;
                }
                if Instant::now() >= deadline {
                    process.kill().unwrap();
                    process.wait().unwrap();
                    panic!("launch parent timed out");
                }
                std::thread::sleep(Duration::from_millis(10));
            }
            wait_readable(custody.pidfd().as_fd());
            let status = rustix::process::waitid(
                rustix::process::WaitId::PidFd(custody.pidfd().as_fd()),
                rustix::process::WaitIdOptions::EXITED,
            )
            .unwrap()
            .unwrap();
            custody.pidfd.take();
            if *role == "locked-static" {
                assert_eq!(
                    status.exit_status(),
                    Some(98),
                    "proof entry must reject locked service profile"
                );
                eprintln!("{role}/{route}: static proof entry rejected locked service profile");
            } else {
                assert_eq!(status.terminating_signal(), Some(libc::SIGKILL));
                eprintln!(
                    "{role}/{route}: post-exec SIGKILL after abrupt parent exit, exact original pidfd reaped"
                );
            }
        }
    }
    assert!(matches!(
        rustix::process::waitpid(None, rustix::process::WaitOptions::NOHANG),
        Err(rustix::io::Errno::CHILD)
    ));
}

#[test]
#[ignore = "irreversible thread-local confinement; private root subprocess only"]
fn root_incompatible_proof_parent_is_rejected() {
    assert!(has_exact_root_identity());
    let case = std::env::var("FE2O3_PROOF_PARENT_CASE").unwrap();
    let image = tests::static_image(&[0x31, 0xff, 0xb8, 60, 0, 0, 0, 0x0f, 0x05]);
    let (reader, writer) = rustix::pipe::pipe_with(rustix::pipe::PipeFlags::CLOEXEC).unwrap();
    let staged = crate::StagedProofControllerExecV1::new(
        &image,
        &[ProtectedServiceDescriptorBindingV1::new(reader.as_fd(), 3).unwrap()],
        writer.as_fd(),
        reader.as_fd(),
        writer.as_fd(),
    )
    .unwrap();
    let before = std::fs::read_to_string("/proc/thread-self/children").unwrap();
    match case.as_str() {
        "seccomp" => {
            // libtest invokes this on a worker, not its unfiltered process leader.
            assert_ne!(rustix::process::getpid(), rustix::thread::gettid());
            tests::deny_calls(&[]);
            let leader = std::fs::read_to_string("/proc/self/status").unwrap();
            assert!(leader.lines().any(|line| line == "Seccomp:\t0"));
        }
        "securebits" => {
            // SAFETY: only this disposable root test thread installs irreversible locked bits.
            assert_eq!(
                unsafe {
                    libc::prctl(
                        PR_SET_SECUREBITS,
                        fe2o3_protected_service_profile::PROTECTED_SERVICE_SECUREBITS_V1,
                        0,
                        0,
                        0,
                    )
                },
                0
            );
        }
        _ => panic!("unknown parent rejection case"),
    }
    assert!(matches!(
        staged.spawn(ProofControllerCredentialProfileV1::new(61000, 61000).unwrap()),
        Err(crate::ProtectedServiceSpawnErrorV1::ParentProfile(_))
    ));
    assert_eq!(
        std::fs::read_to_string("/proc/thread-self/children").unwrap(),
        before
    );
}

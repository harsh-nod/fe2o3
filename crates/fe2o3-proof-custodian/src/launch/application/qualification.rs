#![cfg(test)]

//! Component qualification, not authenticated Cargo registration or GPU settlement.
use super::*;
use crate::application::{
    tests::{binding, pidfd},
    transport,
};
use fe2o3_runtime_protocol::{
    WorkerV3ApplicationProofInputsV1 as Inputs, WorkerV3ApplicationProofKindV1 as Kind,
    WorkerV3ApplicationProofMessageV1 as Message,
};
use rustix::net;
use std::{
    fs,
    io::{IoSlice, IoSliceMut},
    mem::MaybeUninit,
    os::{
        fd::{AsRawFd, FromRawFd},
        unix::process::CommandExt,
    },
    path::PathBuf,
    process::{Child, Command},
};

const ROLE: &str = "FE2O3_APPLICATION_PROOF_QUAL_ROLE";
const HELPER: &str = "launch::application::qualification::application_role";

fn ipc_send(fd: BorrowedFd<'_>, bytes: &[u8], rights: &[BorrowedFd<'_>]) {
    let mut space = [MaybeUninit::uninit(); rustix::cmsg_space!(ScmRights(3))];
    let mut ancillary = net::SendAncillaryBuffer::new(&mut space);
    if !rights.is_empty() {
        assert!(ancillary.push(net::SendAncillaryMessage::ScmRights(rights)));
    }
    wire::wait(
        fd,
        rustix::event::PollFlags::OUT,
        Instant::now() + Duration::from_secs(300),
    )
    .unwrap();
    assert_eq!(
        net::sendmsg(
            fd,
            &[IoSlice::new(bytes)],
            &mut ancillary,
            net::SendFlags::NOSIGNAL
        )
        .unwrap(),
        bytes.len()
    );
}

fn ipc_receive(fd: BorrowedFd<'_>) -> (Vec<u8>, Vec<OwnedFd>) {
    wire::wait(
        fd,
        rustix::event::PollFlags::IN,
        Instant::now() + Duration::from_secs(300),
    )
    .unwrap();
    let mut bytes = [0; 4096];
    let mut space = [MaybeUninit::uninit(); rustix::cmsg_space!(ScmRights(3), ScmCredentials(1))];
    let mut ancillary = net::RecvAncillaryBuffer::new(&mut space);
    let message = net::recvmsg(
        fd,
        &mut [IoSliceMut::new(&mut bytes)],
        &mut ancillary,
        net::RecvFlags::CMSG_CLOEXEC,
    )
    .unwrap();
    assert!(message.bytes > 0 && (message.flags - net::ReturnFlags::CMSG_CLOEXEC).is_empty());
    let mut rights = Vec::new();
    for item in ancillary.drain() {
        match item {
            net::RecvAncillaryMessage::ScmRights(fds) => rights.extend(fds),
            net::RecvAncillaryMessage::ScmCredentials(_) => (),
            _ => panic!("unexpected test IPC ancillary"),
        }
    }
    (bytes[..message.bytes].to_vec(), rights)
}

fn claim(slot: i32) -> OwnedFd {
    // SAFETY: this helper is explicitly launched once with the named exclusive test slots.
    let fd = unsafe { OwnedFd::from_raw_fd(slot) };
    rustix::io::fcntl_setfd(&fd, rustix::io::FdFlags::CLOEXEC).unwrap();
    fd
}

fn spawn(role: &str, slots: &[(BorrowedFd<'_>, i32)]) -> Child {
    let executable = std::env::current_exe().unwrap();
    let root = rustix::process::getuid().is_root();
    let mut command = if root {
        let mut command = Command::new("/usr/bin/setpriv");
        command
            .args([
                "--reuid=1000",
                "--regid=1000",
                "--clear-groups",
                "--inh-caps=-all",
                "--ambient-caps=-all",
                "--bounding-set=-all",
            ])
            .arg(executable);
        command.process_group(0);
        command
    } else {
        Command::new(executable)
    };
    command
        .args([
            "--exact",
            HELPER,
            "--ignored",
            "--nocapture",
            "--test-threads=1",
        ])
        .env(ROLE, role);
    let slots = slots
        .iter()
        .map(|(fd, slot)| (fd.as_raw_fd(), *slot))
        .collect::<Vec<_>>();
    assert!(slots.iter().all(|(fd, _)| *fd < 196));
    // SAFETY: child pre-exec performs only async-signal-safe descriptor syscalls.
    unsafe {
        command.pre_exec(move || {
            for &(fd, slot) in &slots {
                if libc::dup3(fd, slot, 0) < 0 {
                    return Err(io::Error::last_os_error());
                }
            }
            Ok(())
        });
    }
    command.spawn().unwrap()
}

#[test]
#[ignore = "private application proof qualification helper only"]
fn application_role() {
    assert_eq!(rustix::process::getuid().as_raw(), 1000);
    let role = std::env::var(ROLE).unwrap();
    if role == "cargo" {
        let ipc = claim(196);
        let (application, peer) = wire::control_pair().unwrap();
        let (app_ipc, root_ipc) = wire::control_pair().unwrap();
        let mut app = spawn("app", &[(application.as_fd(), 197), (app_ipc.as_fd(), 198)]);
        drop((application, app_ipc));
        let app_pidfd = pidfd(app.id());
        ipc_send(
            ipc.as_fd(),
            &app.id().to_le_bytes(),
            &[app_pidfd.as_fd(), peer.as_fd(), root_ipc.as_fd()],
        );
        drop((app_pidfd, peer, root_ipc));
        assert_eq!(ipc_receive(ipc.as_fd()).0, b"STOP");
        assert!(app.wait().unwrap().success());
        ipc_send(ipc.as_fd(), b"APP-REAPED", &[]);
        assert_eq!(ipc_receive(ipc.as_fd()).0, b"EXIT");
        return;
    }
    assert_eq!(role, "app");
    let peer = claim(197);
    let ipc = claim(198);
    let session = Session::decode(&ipc_receive(ipc.as_fd()).0).unwrap();
    let case = std::env::var("FE2O3_CUSTODIAN_CASE").unwrap();
    let source = PathBuf::from(std::env::var_os("FE2O3_CUSTODIAN_INPUTS").unwrap());
    let envelope = fs::read(source.join("envelope.bin")).unwrap();
    let mut payload = fs::read(source.join("payload.hsaco")).unwrap();
    let kernel = fs::read(source.join("kernel.bin"))
        .unwrap()
        .try_into()
        .unwrap();
    if case == "app-payload" {
        payload.push(0);
    }
    let envelope_fd = wire::seal(&envelope).unwrap();
    let payload_fd = wire::seal(&payload).unwrap();
    let inputs = Inputs::new(
        kernel,
        (wire::digest(&envelope), envelope.len() as u64),
        (wire::digest(&payload), payload.len() as u64),
    )
    .unwrap();
    let identity = if case == "app-stale" {
        [99; 32]
    } else {
        session.identity()
    };
    let request = Message::new(Kind::Request, identity, 1, inputs.canonical_bytes()).unwrap();
    let rights = if case == "app-duplicate" {
        [envelope_fd.as_fd(), envelope_fd.as_fd()]
    } else {
        [envelope_fd.as_fd(), payload_fd.as_fd()]
    };
    assert!(transport::try_send(peer.as_fd(), &request, &rights).unwrap());
    ipc_send(ipc.as_fd(), b"QUEUED", &[]);
    let sender = session.controller();
    let sender = (sender.0 as i32, sender.1, sender.2);
    let receive = || {
        let deadline = Instant::now() + Duration::from_secs(300);
        loop {
            wire::wait(peer.as_fd(), rustix::event::PollFlags::IN, deadline).unwrap();
            if let Some((message, rights)) =
                transport::try_receive(peer.as_fd(), sender, session.identity()).unwrap()
            {
                assert!(rights.is_empty());
                return message;
            }
        }
    };
    assert_eq!(receive().kind(), Kind::Active);
    if case != "app-good" {
        assert_eq!(ipc_receive(ipc.as_fd()).0, b"STOP");
        return;
    }
    let proved = receive();
    assert_eq!(proved.kind(), Kind::Proved);
    ipc_send(ipc.as_fd(), proved.body(), &[]);
    assert_eq!(ipc_receive(ipc.as_fd()).0, b"PROBE");
    assert!(
        transport::try_send(
            peer.as_fd(),
            &Message::new(Kind::Probe, session.identity(), 2, &[]).unwrap(),
            &[]
        )
        .unwrap()
    );
    let retained = receive();
    assert_eq!(retained.kind(), Kind::Retained);
    assert_eq!(retained.sequence(), 2);
    assert_eq!(retained.body(), proved.body());
    ipc_send(ipc.as_fd(), retained.body(), &[]);
    assert_eq!(ipc_receive(ipc.as_fd()).0, b"CLOSE");
    drop(peer);
    ipc_send(ipc.as_fd(), b"CLOSED", &[]);
    assert_eq!(ipc_receive(ipc.as_fd()).0, b"STOP");
}

struct Fixture {
    cargo: Option<Child>,
    root: OwnedFd,
    app: OwnedFd,
    controller: Option<RootStagedApplicationProofControllerV1>,
}
impl Fixture {
    fn finish(&mut self) {
        ipc_send(self.app.as_fd(), b"STOP", &[]);
        ipc_send(self.root.as_fd(), b"STOP", &[]);
        assert_eq!(ipc_receive(self.root.as_fd()).0, b"APP-REAPED");
        ipc_send(self.root.as_fd(), b"EXIT", &[]);
        let cargo = self.cargo.as_mut().unwrap();
        let pid = cargo.id();
        let deadline = Instant::now() + Duration::from_secs(5);
        loop {
            if let Some(status) = cargo.try_wait().unwrap() {
                self.cargo.take();
                assert!(status.success(), "Cargo fixture failed: {status}");
                break;
            }
            assert!(Instant::now() < deadline, "Cargo fixture did not exit");
            std::thread::sleep(Duration::from_millis(5));
        }
        // SAFETY: signal 0 only observes the private qualification process group.
        assert_eq!(unsafe { libc::kill(-(pid as i32), 0) }, -1);
        assert_eq!(io::Error::last_os_error().raw_os_error(), Some(libc::ESRCH));
        self.controller
            .as_mut()
            .unwrap()
            .contain_qualification()
            .unwrap();
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let containment = self
            .controller
            .as_mut()
            .map(|controller| controller.contain_qualification());
        if let Some(cargo) = self.cargo.as_mut() {
            // SAFETY: the original unreaped Cargo child leads this fixture-only group.
            let killed = unsafe { libc::kill(-(cargo.id() as i32), libc::SIGKILL) };
            if killed != 0 {
                eprintln!("helper group cleanup: {}", io::Error::last_os_error());
            }
            if let Err(error) = cargo.wait() {
                eprintln!("Cargo cleanup: {error}");
            }
        }
        if let Some(Err(error)) = containment {
            eprintln!("controller containment failed after helper cleanup: {error}");
            std::process::abort();
        }
    }
}

#[test]
#[ignore = "requires private real-root namespace and independently installed application controller"]
fn root_application_controller() {
    assert!(rustix::process::getuid().is_root());
    assert_eq!(
        std::env::var("FE2O3_CUSTODIAN_PRIVATE_INSTALL").unwrap(),
        "1"
    );
    assert_ne!(
        fs::read_link("/proc/self/ns/pid").unwrap().as_os_str(),
        std::env::var_os("FE2O3_CUSTODIAN_HOST_PID_NAMESPACE").unwrap()
    );
    for path in ["/etc", "/usr/libexec"] {
        assert_eq!(rustix::fs::statfs(path).unwrap().f_type, libc::TMPFS_MAGIC);
    }
    assert!(
        fs::read_to_string("/proc/thread-self/cgroup")
            .unwrap()
            .contains("/fe2o3-custodian-qual-")
    );
    let case = std::env::var("FE2O3_CUSTODIAN_CASE").unwrap();
    assert!(matches!(
        case.as_str(),
        "app-good" | "app-payload" | "app-duplicate" | "app-stale"
    ));
    let output = PathBuf::from(std::env::var_os("FE2O3_CUSTODIAN_CAPTURE").unwrap());
    fs::create_dir(&output).unwrap();
    fs::write(
        output.join("pid-namespace.txt"),
        fs::read_link("/proc/self/ns/pid")
            .unwrap()
            .as_os_str()
            .as_encoded_bytes(),
    )
    .unwrap();
    let (root, cargo_ipc) = wire::control_pair().unwrap();
    let cargo = spawn("cargo", &[(cargo_ipc.as_fd(), 196)]);
    drop(cargo_ipc);
    let (app_pid, rights) = ipc_receive(root.as_fd());
    let app_pid = u32::from_le_bytes(app_pid.try_into().unwrap());
    let [app_pidfd, peer, app]: [OwnedFd; 3] = rights.try_into().unwrap();
    let alias = rustix::io::fcntl_dupfd_cloexec(&peer, 0).unwrap();
    let source = PathBuf::from(std::env::var_os("FE2O3_CUSTODIAN_INPUTS").unwrap());
    let binding = binding(
        app_pid,
        cargo.id(),
        1000,
        1000,
        &fs::read(source.join("envelope.bin")).unwrap(),
    );
    let transcript = Transcript::new([12; 32], [13; 32], *binding.identity().as_bytes()).unwrap();
    let cargo_pidfd = pidfd(cargo.id());
    let mut fixture = Fixture {
        cargo: Some(cargo),
        root,
        app,
        controller: None,
    };
    let mut sentinel = Command::new("/bin/sleep").arg("400").spawn().unwrap();
    let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let mut pending = ProductionApplicationProofCustodianDeploymentV1::open()
            .unwrap()
            .begin_unregistered_qualification(binding, transcript, app_pidfd, cargo_pidfd, peer)
            .unwrap();
        while !pending.poll().unwrap() {
            std::thread::sleep(Duration::from_millis(2));
        }
        fixture.controller = Some(pending.take_ready().unwrap().unwrap());
        let controller = fixture.controller.as_mut().unwrap();
        ipc_send(
            fixture.app.as_fd(),
            controller.session().canonical_bytes(),
            &[],
        );
        assert_eq!(ipc_receive(fixture.app.as_fd()).0, b"QUEUED");
        let mut bytes = [0; 2112];
        let mut space =
            [MaybeUninit::uninit(); rustix::cmsg_space!(ScmRights(2), ScmCredentials(1))];
        let mut ancillary = net::RecvAncillaryBuffer::new(&mut space);
        let peek = net::recvmsg(
            &alias,
            &mut [IoSliceMut::new(&mut bytes)],
            &mut ancillary,
            net::RecvFlags::PEEK | net::RecvFlags::DONTWAIT | net::RecvFlags::CMSG_CLOEXEC,
        )
        .unwrap();
        assert_eq!(
            Message::decode(&bytes[..peek.bytes]).unwrap().kind(),
            Kind::Request
        );
        drop(ancillary);
        drop(alias);
        fs::write(output.join("preactivation.txt"), b"application request queued and unread; all root receive aliases dropped before Activate\n").unwrap();
        while !controller.poll_activate().unwrap() {
            std::thread::sleep(Duration::from_millis(2));
        }
        assert!(controller.poll_probe().unwrap().is_none());
        assert_eq!(
            controller.probe_deadline,
            Some(controller.controller.deadline)
        );
        let proof = loop {
            match controller.poll_probe() {
                Ok(Some(value)) => break Ok(value),
                Ok(None) => std::thread::sleep(Duration::from_millis(5)),
                Err(error) => break Err(error),
            }
        };
        if case == "app-good" {
            let (subject, quarantined) = proof.unwrap();
            assert!(!quarantined);
            assert_eq!(ipc_receive(fixture.app.as_fd()).0, subject);
            ipc_send(fixture.app.as_fd(), b"PROBE", &[]);
            assert_eq!(ipc_receive(fixture.app.as_fd()).0, subject);
            ipc_send(fixture.app.as_fd(), b"CLOSE", &[]);
            assert_eq!(ipc_receive(fixture.app.as_fd()).0, b"CLOSED");
            let deadline = Instant::now() + Duration::from_secs(5);
            loop {
                assert!(
                    Instant::now() < deadline,
                    "EOF did not quarantine original proof"
                );
                if let Some((retained, quarantined)) = controller.poll_probe().unwrap() {
                    assert_eq!(retained, subject);
                    if quarantined {
                        break;
                    }
                }
                std::thread::sleep(Duration::from_millis(5));
            }
            fs::write(output.join("subject.bin"), subject).unwrap();
            fs::write(output.join("quarantine.txt"), b"app EOF while original app/Cargo remain alive: original proof recomputed and retained\n").unwrap();
        } else {
            let error = proof.unwrap_err().to_string();
            let expected = match case.as_str() {
                "app-payload" => "FinalizedLengthMismatch",
                "app-duplicate" => "aliased application proof inputs",
                "app-stale" => "application proof session mismatch",
                _ => unreachable!(),
            };
            assert!(error.contains(expected), "expected {expected}, got {error}");
            fs::write(output.join("rejection.txt"), error).unwrap();
        }
        fixture.finish();
    }));
    assert!(
        sentinel.try_wait().unwrap().is_none(),
        "unrelated sibling was killed"
    );
    sentinel.kill().unwrap();
    sentinel.wait().unwrap();
    drop(fixture);
    if let Err(panic) = outcome {
        std::panic::resume_unwind(panic);
    }
    fs::write(
        output.join("cleanup.txt"),
        b"original controller contained; app and Cargo exited successfully and were reaped; helper group absent; unrelated sibling survived\n",
    )
    .unwrap();
}

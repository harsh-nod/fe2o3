use super::*;
use std::io::{IoSliceMut, Write};
use std::mem::MaybeUninit;
use std::os::fd::{AsRawFd, OwnedFd};
use std::os::unix::fs::PermissionsExt;
use std::os::unix::process::CommandExt;
use std::path::Path;
use std::process::{Child, Command, Stdio};

use rustix::net::{
    AddressFamily, RecvAncillaryBuffer, RecvAncillaryMessage, RecvFlags, SocketFlags, SocketType,
};
use rustix::process::{Gid, Uid};

const CLIENT_ID: u32 = 1000;
const ENVELOPE_BYTES: &[u8] = b"exact bytes; deliberately not a canonical V2 envelope";

#[test]
fn proof_counterpart_requires_the_exact_pair_even_with_the_same_cargo_creator() {
    let pair = || {
        let (child, peer) = rustix::net::socketpair(
            AddressFamily::UNIX,
            SocketType::SEQPACKET,
            SocketFlags::CLOEXEC | SocketFlags::NONBLOCK,
            None,
        )
        .unwrap();
        for fd in [&child, &peer] {
            rustix::net::sockopt::set_socket_passcred(fd, true).unwrap();
            rustix::net::bind(fd, &SocketAddrUnix::new_unnamed()).unwrap();
        }
        (child, peer)
    };
    let creator = ExpectedClientProcessIdentityV1::new(
        std::process::id(),
        rustix::process::geteuid().as_raw(),
        rustix::process::getegid().as_raw(),
    )
    .unwrap();
    let (child, peer) = pair();
    let (_foreign_child, foreign_peer) = pair();
    let child = inspect_proof_endpoint(child.as_fd(), creator).unwrap();
    let peer = inspect_proof_endpoint(peer.as_fd(), creator).unwrap();
    let foreign = inspect_proof_endpoint(foreign_peer.as_fd(), creator).unwrap();
    child.require_counterpart(&peer).unwrap();
    assert!(child.require_counterpart(&child).is_err());
    assert!(child.require_counterpart(&foreign).is_err());
}

#[test]
fn descriptor_coordinates_are_distinct_non_stdio() {
    for values in [
        (0, 4, 5),
        (3, -1, 5),
        (3, 4, 2),
        (3, 3, 4),
        (3, 4, 3),
        (3, 4, 4),
    ] {
        assert!(WorkerV3ApplicationDescriptorNumbersV1::new(values.0, values.1, values.2).is_err());
    }
    assert!(WorkerV3ApplicationDescriptorNumbersV1::new(3, 4, i32::MAX).is_ok());
}

#[test]
fn process_credential_record_requires_all_four_ids_and_unique_fields() {
    let expected = ExpectedClientProcessIdentityV1::new(1, 1000, 1001).unwrap();
    assert!(
        validate_credentials(
            b"Name:\ttest\nUid:\t1000 1000 1000 1000\nGid:\t1001 1001 1001 1001\n",
            expected
        )
        .is_ok()
    );
    for bytes in [
        "Uid: 1000 1000 1000 0\nGid: 1001 1001 1001 1001\n",
        "Uid: 1000 1000 1000 1000\nGid: 1001 1001 0 1001\n",
        "Uid: 1000 1000 1000\nGid: 1001 1001 1001 1001\n",
        "Uid: 1000 1000 1000 1000\nUid: 1000 1000 1000 1000\nGid: 1001 1001 1001 1001\n",
        "Uid: 1000 1000 1000 1000\n",
    ] {
        assert!(validate_credentials(bytes.as_bytes(), expected).is_err());
    }
}

#[test]
fn source_flags_are_one_bounded_octal_field() {
    assert_eq!(
        parse_source_flags(b"pos:\t0\nflags:\t02000001\nmnt_id:\t1\n").unwrap(),
        libc::O_CLOEXEC as u32 | libc::O_WRONLY as u32
    );
    for bytes in [
        "flags: 08",
        "flags:",
        "flags: 1 2",
        "flags: 1\nflags: 1",
        "flags: 7777777777777777777777",
        "pos: 0",
    ] {
        assert!(parse_source_flags(bytes.as_bytes()).is_err());
    }
}

fn own(file: &impl AsFd) {
    rustix::fs::fchown(
        file,
        Some(Uid::from_raw(CLIENT_ID)),
        Some(Gid::from_raw(CLIENT_ID)),
    )
    .unwrap();
}

fn private_directory(path: &Path) -> File {
    std::fs::create_dir(path).unwrap();
    let file = File::open(path).unwrap();
    file.set_permissions(std::fs::Permissions::from_mode(0o700))
        .unwrap();
    own(&file);
    file
}

fn private_envelope(path: &Path) -> File {
    std::fs::write(path, ENVELOPE_BYTES).unwrap();
    let file = File::open(path).unwrap();
    file.set_permissions(std::fs::Permissions::from_mode(0o600))
        .unwrap();
    own(&file);
    file
}

fn pair(kind: SocketType) -> (OwnedFd, OwnedFd) {
    rustix::net::socketpair(AddressFamily::UNIX, kind, SocketFlags::CLOEXEC, None).unwrap()
}

fn pipe() -> (OwnedFd, OwnedFd) {
    let (read, write) = rustix::pipe::pipe_with(
        rustix::pipe::PipeFlags::CLOEXEC | rustix::pipe::PipeFlags::NONBLOCK,
    )
    .unwrap();
    own(&read);
    (read, write)
}

fn poll_readable(fd: &impl AsFd) {
    let mut poll = libc::pollfd {
        fd: fd.as_fd().as_raw_fd(),
        events: libc::POLLIN,
        revents: 0,
    };
    // SAFETY: the borrowed descriptor and one initialized pollfd remain valid for this call.
    assert_eq!(
        unsafe { libc::poll(&mut poll, 1, 5000) },
        1,
        "fixture timed out"
    );
}

fn read_ready(control: &mut File) {
    poll_readable(control);
    let mut byte = [0];
    control.read_exact(&mut byte).unwrap();
    assert_eq!(byte, [b'r']);
}

fn process_identity(pidfd: &OwnedFd, pid: u32, id: u32) -> LiveClientPidfdIdentityV1 {
    LiveClientPidfdIdentityV1::admit(
        rustix::io::fcntl_dupfd_cloexec(pidfd, 0).unwrap(),
        ExpectedClientProcessIdentityV1::new(pid, id, id).unwrap(),
    )
    .unwrap()
}

pub(crate) struct Fixture {
    parent: GroupChild,
    application_pidfd: OwnedFd,
    application_pid: u32,
    parent_pidfd: OwnedFd,
    parent_control: File,
    control: File,
    ack: File,
    _replacement_ack: OwnedFd,
    expected: WorkerV3ApplicationOccurrenceV1,
    registered_expected: WorkerV3ApplicationOccurrenceV1,
    proof_peer: OwnedFd,
    files: tempfile::TempDir,
}

struct GroupChild(Child, bool);

impl Drop for GroupChild {
    fn drop(&mut self) {
        // Kill the still-owned process group before reaping, including failures before app exit.
        if !self.1 {
            // SAFETY: setsid in this fixture's pre_exec made its unreaped child the group leader.
            unsafe {
                libc::kill(-(self.0.id() as i32), libc::SIGKILL);
            }
            let _ = self.0.wait();
        }
    }
}

impl Fixture {
    pub(crate) fn spawn(helper: &Path, sealed: bool) -> Self {
        let files = tempfile::tempdir().unwrap();
        std::fs::set_permissions(files.path(), std::fs::Permissions::from_mode(0o755)).unwrap();
        let directory = private_directory(&files.path().join("original"));
        let envelope = private_envelope(&files.path().join("original/envelope"));
        let replacement_directory = private_directory(&files.path().join("replacement"));
        let replacement_envelope = private_envelope(&files.path().join("replacement/envelope"));
        let (ack, ack_writer) = pipe();
        let (replacement_ack, replacement_ack_writer) = pipe();
        let (control, child_control) = pair(SocketType::STREAM);
        let (parent_control, child_parent_control) = pair(SocketType::SEQPACKET);
        let image_bytes = std::fs::read(helper).unwrap();
        let image_identity =
            WorkerV3ApplicationIdentityV1::from_sealed_static_elf_v1(&image_bytes).unwrap();
        let mut image = File::from(
            rustix::fs::memfd_create(
                "fe2o3-test-application",
                rustix::fs::MemfdFlags::CLOEXEC | rustix::fs::MemfdFlags::ALLOW_SEALING,
            )
            .unwrap(),
        );
        image.write_all(&image_bytes).unwrap();
        image
            .set_permissions(std::fs::Permissions::from_mode(0o500))
            .unwrap();
        own(&image);
        if sealed {
            rustix::fs::fcntl_add_seals(&image, REQUIRED_SEALS).unwrap();
        }
        let mut second_image = File::from(
            rustix::fs::memfd_create(
                "fe2o3-test-second-application",
                rustix::fs::MemfdFlags::CLOEXEC | rustix::fs::MemfdFlags::ALLOW_SEALING,
            )
            .unwrap(),
        );
        second_image.write_all(&image_bytes).unwrap();
        second_image
            .set_permissions(std::fs::Permissions::from_mode(0o500))
            .unwrap();
        own(&second_image);
        rustix::fs::fcntl_add_seals(&second_image, REQUIRED_SEALS).unwrap();
        let ack_file = File::from(ack_writer);
        let expected = WorkerV3ApplicationOccurrenceV1::new(
            image_identity,
            [9; 32],
            &[
                Snapshot::new(envelope.metadata().unwrap())
                    .input(1)
                    .unwrap(),
                Snapshot::new(directory.metadata().unwrap())
                    .input(2)
                    .unwrap(),
                Snapshot::new(ack_file.metadata().unwrap())
                    .input(3)
                    .unwrap(),
            ],
        )
        .unwrap();
        let transfers = [
            (envelope.as_fd(), 180),
            (directory.as_fd(), 181),
            (ack_file.as_fd(), 182),
            (child_control.as_fd(), 183),
            (replacement_envelope.as_fd(), 184),
            (replacement_directory.as_fd(), 185),
            (replacement_ack_writer.as_fd(), 186),
            (child_parent_control.as_fd(), 187),
            (image.as_fd(), 188),
            (second_image.as_fd(), 189),
        ];
        let sources: Vec<_> = transfers
            .iter()
            .map(|(fd, number)| (rustix::io::fcntl_dupfd_cloexec(fd, 240).unwrap(), *number))
            .collect();
        let raw: Vec<_> = sources
            .iter()
            .map(|(fd, number)| (fd.as_raw_fd(), *number))
            .collect();
        let mut command = Command::new(helper);
        command.arg("parent").stdin(Stdio::null());
        // SAFETY: only credential/session/descriptor syscalls run in the owned child before exec.
        unsafe {
            command.pre_exec(move || {
                if libc::setsid() < 0
                    || libc::setgroups(0, std::ptr::null()) != 0
                    || libc::setresgid(CLIENT_ID, CLIENT_ID, CLIENT_ID) != 0
                    || libc::setresuid(CLIENT_ID, CLIENT_ID, CLIENT_ID) != 0
                {
                    return Err(io::Error::last_os_error());
                }
                let header = [0x20080522_u32, 0];
                let capabilities = [0_u32; 6];
                if libc::syscall(libc::SYS_capset, header.as_ptr(), capabilities.as_ptr()) != 0 {
                    return Err(io::Error::last_os_error());
                }
                for (source, target) in &raw {
                    if libc::dup2(*source, *target) != *target
                        || libc::fcntl(*target, libc::F_SETFD, 0) != 0
                    {
                        return Err(io::Error::last_os_error());
                    }
                }
                Ok(())
            });
        }
        let parent = GroupChild(
            crate::test_process_execution::spawn(&mut command).unwrap(),
            false,
        );
        drop(command);
        drop(sources);
        drop(ack_file);
        drop(child_control);
        drop(child_parent_control);
        let parent_pidfd = rustix::process::pidfd_open(
            rustix::process::Pid::from_raw(parent.0.id() as i32).unwrap(),
            rustix::process::PidfdFlags::empty(),
        )
        .unwrap();
        poll_readable(&parent_control);
        let mut bytes = [0; 32];
        let mut space = [MaybeUninit::uninit(); rustix::cmsg_space!(ScmRights(2))];
        let mut ancillary = RecvAncillaryBuffer::new(&mut space);
        let received = rustix::net::recvmsg(
            &parent_control,
            &mut [IoSliceMut::new(&mut bytes)],
            &mut ancillary,
            RecvFlags::CMSG_CLOEXEC,
        )
        .unwrap();
        assert_eq!(received.bytes, 32);
        assert!(
            !received
                .flags
                .intersects(rustix::net::ReturnFlags::TRUNC | rustix::net::ReturnFlags::CTRUNC)
        );
        let mut rights = Vec::new();
        for message in ancillary.drain() {
            match message {
                RecvAncillaryMessage::ScmRights(fds) => rights.extend(fds),
                _ => panic!("unexpected fixture ancillary"),
            }
        }
        assert_eq!(rights.len(), 2);
        let proof_peer = rights.pop().unwrap();
        let application_pidfd = rights.pop().unwrap();
        let facts: Vec<_> = bytes
            .chunks_exact(8)
            .map(|field| u64::from_ne_bytes(field.try_into().unwrap()))
            .collect();
        let application_pid = u32::try_from(facts[0]).unwrap();
        let mut inputs = expected.inputs().to_vec();
        inputs.push(
            WorkerV3ApplicationInputOccurrenceV1::from_linux_descriptor_v1(
                4,
                facts[1],
                facts[2],
                u32::try_from(facts[3]).unwrap(),
            )
            .unwrap(),
        );
        let registered_expected = WorkerV3ApplicationOccurrenceV1::new(
            expected.application(),
            expected.spawn_identity(),
            &inputs,
        )
        .unwrap();
        let mut control = File::from(control);
        read_ready(&mut control);
        Self {
            parent,
            parent_pidfd,
            parent_control: File::from(parent_control),
            application_pidfd,
            application_pid,
            control,
            ack: File::from(ack),
            _replacement_ack: replacement_ack,
            expected,
            registered_expected,
            proof_peer,
            files,
        }
    }

    fn observe(&self) -> Result<RetainedWorkerV3ApplicationObservationV1> {
        self.observe_expected(
            &self.expected,
            WorkerV3LoadEnvelopeIdentityV1::from_exact_bytes(ENVELOPE_BYTES).unwrap(),
        )
    }

    pub(crate) fn registration(&self) -> WorkerV3ApplicationRegistrationBindingV1 {
        use fe2o3_compiler_execution_protocol::{
            CompilerExecutionClientProcessIdentityV1,
            CompilerExecutionExternalAnchorServiceIdentityV1, CompilerExecutionIssuerMeasurementV1,
            CompilerExecutionIssuerPolicyV1, CompilerExecutionServiceLaunchManifestV1,
            CompilerExecutionSupervisorHandoffV1,
        };
        use fe2o3_runtime_protocol::{
            WorkerV3ApplicationHandoffChallengeV1, WorkerV3ApplicationRegistrationDescriptorsV1,
        };
        let key = |hex: &str| {
            std::array::from_fn(|i| u8::from_str_radix(&hex[i * 2..i * 2 + 2], 16).unwrap())
        };
        let policy = CompilerExecutionIssuerPolicyV1::new(
            1,
            CompilerExecutionIssuerMeasurementV1::new([1; 32], 123).unwrap(),
            CompilerExecutionIssuerMeasurementV1::new([2; 32], 456).unwrap(),
            key("d75a980182b10ab7d54bfed3c964073a0ee172f3daa62325af021a68f707511a"),
            key("3d4017c3e843895a92b70aa74d1b7ebc9c982ccf2ec4968cc0cd55f12af4660c"),
        )
        .unwrap();
        let handoff = CompilerExecutionSupervisorHandoffV1::new(
            CompilerExecutionClientProcessIdentityV1::new(self.parent.0.id(), CLIENT_ID, CLIENT_ID)
                .unwrap(),
            CompilerExecutionServiceLaunchManifestV1::new(
                CompilerExecutionClientProcessIdentityV1::new(
                    self.application_pid,
                    CLIENT_ID,
                    CLIENT_ID,
                )
                .unwrap(),
                CompilerExecutionExternalAnchorServiceIdentityV1::new(6000, 7000).unwrap(),
                &policy,
            ),
        )
        .unwrap();
        WorkerV3ApplicationRegistrationBindingV1::new(
            handoff,
            self.registered_expected.clone(),
            WorkerV3ApplicationRegistrationDescriptorsV1::new(180, 181, 182, 190).unwrap(),
            WorkerV3ApplicationHandoffExpectationV1::new(
                WorkerV3LoadEnvelopeIdentityV1::from_exact_bytes(ENVELOPE_BYTES).unwrap(),
                &self.registered_expected,
            ),
            WorkerV3ApplicationHandoffChallengeV1::from_bytes([7; 32]).unwrap(),
        )
        .unwrap()
    }

    fn observe_registered(&self) -> Result<RetainedWorkerV3ApplicationObservationV1> {
        RetainedWorkerV3ApplicationObservationV1::observe_registered_pre_ack(
            process_identity(&self.application_pidfd, self.application_pid, CLIENT_ID),
            process_identity(&self.parent_pidfd, self.parent.0.id(), CLIENT_ID),
            &self.registration(),
            self.proof_peer.as_fd(),
        )
    }

    fn assert_proof_hangup(&self) {
        let mut poll = libc::pollfd {
            fd: self.proof_peer.as_raw_fd(),
            events: libc::POLLIN,
            revents: 0,
        };
        // SAFETY: poll points to one initialized entry borrowing the retained root counterpart.
        assert_eq!(unsafe { libc::poll(&mut poll, 1, 5000) }, 1);
        assert_ne!(
            poll.revents & libc::POLLHUP,
            0,
            "application proof alias leaked"
        );
    }

    fn observe_expected(
        &self,
        expected: &WorkerV3ApplicationOccurrenceV1,
        envelope: WorkerV3LoadEnvelopeIdentityV1,
    ) -> Result<RetainedWorkerV3ApplicationObservationV1> {
        RetainedWorkerV3ApplicationObservationV1::observe_pre_ack(
            process_identity(&self.application_pidfd, self.application_pid, CLIENT_ID),
            process_identity(&self.parent_pidfd, self.parent.0.id(), CLIENT_ID),
            WorkerV3ApplicationDescriptorNumbersV1::new(180, 181, 182).unwrap(),
            expected,
            envelope,
        )
    }

    pub(crate) fn command(&mut self, byte: u8) {
        self.control.write_all(&[byte]).unwrap();
        read_ready(&mut self.control);
    }

    pub(crate) fn acknowledge(&mut self) {
        use fe2o3_runtime_protocol::WorkerV3ApplicationHandoffChallengeV1;
        let expected = WorkerV3ApplicationHandoffExpectationV1::new(
            WorkerV3LoadEnvelopeIdentityV1::from_exact_bytes(ENVELOPE_BYTES).unwrap(),
            &self.expected,
        );
        let ack = expected
            .acknowledgment(WorkerV3ApplicationHandoffChallengeV1::from_bytes([7; 32]).unwrap())
            .encode_canonical()
            .unwrap();
        self.control.write_all(b"a").unwrap();
        self.control
            .write_all(&(ack.len() as u32).to_ne_bytes())
            .unwrap();
        self.control.write_all(&ack).unwrap();
        read_ready(&mut self.control);
        let mut actual = Vec::new();
        loop {
            poll_readable(&self.ack);
            let mut buffer = [0; 512];
            let count = self.ack.read(&mut buffer).unwrap();
            if count == 0 {
                break;
            }
            actual.extend_from_slice(&buffer[..count]);
            assert!(actual.len() <= ack.len());
        }
        assert_eq!(actual, ack);
    }

    pub(crate) fn exit(&mut self) {
        self.control.write_all(b"x").unwrap();
        self.reap();
    }

    fn reap(&mut self) {
        self.parent_control.write_all(b"q").unwrap();
        poll_readable(&self.parent_pidfd);
        // SAFETY: the group leader has exited but remains unreaped and owned by this guard.
        unsafe {
            libc::kill(-(self.parent.0.id() as i32), libc::SIGKILL);
        }
        let status = self.parent.0.wait().unwrap();
        self.parent.1 = true;
        assert!(
            status.success(),
            "fixture supervisor did not reap a successful original child"
        );
    }

    pub(crate) fn application_identity(&self) -> LiveClientPidfdIdentityV1 {
        process_identity(&self.application_pidfd, self.application_pid, CLIENT_ID)
    }

    pub(crate) fn parent_identity(&self) -> LiveClientPidfdIdentityV1 {
        process_identity(&self.parent_pidfd, self.parent.0.id(), CLIENT_ID)
    }

    pub(crate) fn proof_peer(&self) -> OwnedFd {
        rustix::io::fcntl_dupfd_cloexec(&self.proof_peer, 0).unwrap()
    }

    pub(crate) fn send_session(&mut self, bytes: &[u8], extra_right: bool) {
        self.control
            .write_all(if extra_right { b"I" } else { b"H" })
            .unwrap();
        self.control
            .write_all(&(bytes.len() as u32).to_ne_bytes())
            .unwrap();
        self.control.write_all(bytes).unwrap();
        read_ready(&mut self.control);
    }

    pub(crate) fn receive_session(&mut self) -> (Vec<u8>, u32) {
        self.control.write_all(b"R").unwrap();
        poll_readable(&self.control);
        let mut fields = [0; 8];
        self.control.read_exact(&mut fields).unwrap();
        let length = u32::from_ne_bytes(fields[..4].try_into().unwrap()) as usize;
        let rights = u32::from_ne_bytes(fields[4..].try_into().unwrap());
        assert!(length <= 952);
        let mut bytes = vec![0; length];
        self.control.read_exact(&mut bytes).unwrap();
        read_ready(&mut self.control);
        (bytes, rights)
    }

    pub(crate) fn compiler_peer(&mut self) -> OwnedFd {
        self.control.write_all(b"C").unwrap();
        poll_readable(&self.control);
        let mut bytes = [0];
        let mut space = [MaybeUninit::uninit(); rustix::cmsg_space!(ScmRights(1))];
        let mut ancillary = RecvAncillaryBuffer::new(&mut space);
        let received = rustix::net::recvmsg(
            &self.control,
            &mut [IoSliceMut::new(&mut bytes)],
            &mut ancillary,
            RecvFlags::CMSG_CLOEXEC,
        )
        .unwrap();
        assert_eq!(received.bytes, 1);
        assert!(
            !received
                .flags
                .intersects(rustix::net::ReturnFlags::TRUNC | rustix::net::ReturnFlags::CTRUNC)
        );
        let mut rights = Vec::new();
        for message in ancillary.drain() {
            match message {
                RecvAncillaryMessage::ScmRights(fds) => rights.extend(fds),
                _ => panic!("unexpected compiler fixture ancillary"),
            }
        }
        read_ready(&mut self.control);
        let [peer]: [OwnedFd; 1] = rights.try_into().unwrap();
        peer
    }
}

pub(crate) fn build_helper(directory: &Path) -> std::path::PathBuf {
    std::fs::set_permissions(directory, std::fs::Permissions::from_mode(0o755)).unwrap();
    let helper = directory.join("application-observation-fixture");
    let source =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("src/linux/application_observation/fixture.c");
    let mut command = Command::new("cc");
    command
        .args([
            "-static",
            "-nostdlib",
            "-no-pie",
            "-fno-pie",
            "-fno-stack-protector",
            "-fno-builtin",
            "-fcf-protection=none",
            "-Wl,--build-id=none",
            "-O2",
            "-Wall",
            "-Wextra",
            "-Werror",
        ])
        .arg(&source)
        .arg("-o")
        .arg(&helper);
    assert!(
        crate::test_process_execution::status(&mut command)
            .unwrap()
            .success()
    );
    std::fs::set_permissions(&helper, std::fs::Permissions::from_mode(0o755)).unwrap();
    helper
}

#[test]
#[ignore = "requires root, SYS_PTRACE, credential capabilities, and a static C toolchain in a private namespace"]
fn root_application_observation_campaign() {
    assert_eq!(rustix::process::geteuid().as_raw(), 0);
    let build = tempfile::tempdir().unwrap();
    let helper = build_helper(build.path());

    let mut fixture = Fixture::spawn(&helper, true);
    let observed = fixture.observe_registered().unwrap();
    assert_eq!(observed.occurrence(), &fixture.registered_expected);
    fixture.command(b'm');
    let mut bytes = [0; 1];
    let mut space = [MaybeUninit::uninit(); rustix::cmsg_space!(ScmCredentials(1))];
    let mut ancillary = RecvAncillaryBuffer::new(&mut space);
    let message = rustix::net::recvmsg(
        &fixture.proof_peer,
        &mut [IoSliceMut::new(&mut bytes)],
        &mut ancillary,
        RecvFlags::CMSG_CLOEXEC,
    )
    .unwrap();
    assert_eq!(message.bytes, 1);
    assert!(
        !message
            .flags
            .intersects(rustix::net::ReturnFlags::TRUNC | rustix::net::ReturnFlags::CTRUNC)
    );
    assert_eq!(bytes, [b'p']);
    let messages: Vec<_> = ancillary.drain().collect();
    assert_eq!(messages.len(), 1);
    let RecvAncillaryMessage::ScmCredentials(sender) = &messages[0] else {
        panic!("unexpected proof message rights");
    };
    assert_eq!(
        sender.pid.as_raw_nonzero().get() as u32,
        fixture.application_pid
    );
    assert_eq!(
        (sender.uid.as_raw(), sender.gid.as_raw()),
        (CLIENT_ID, CLIENT_ID)
    );
    observed.revalidate_retained_inputs().unwrap();
    fixture.acknowledge();
    observed.revalidate_retained_inputs().unwrap();
    fixture.command(b't');
    fixture.assert_proof_hangup();
    assert!(observed.revalidate_retained_inputs().is_err());
    fixture.exit();
    println!("PASS: four-input observation, ACK EOF and proof HUP with retained observation alive");

    for command in [b'c', b'n', b'v', b's', b'o', b't'] {
        let mut fixture = Fixture::spawn(&helper, true);
        fixture.command(command);
        assert!(
            fixture.observe_registered().is_err(),
            "accepted proof mutation {}",
            command as char
        );
        fixture.command(b't');
        fixture.assert_proof_hangup();
        fixture.acknowledge();
        fixture.exit();
        println!(
            "PASS: proof pre-ACK mutation {}; failed admission leaks no alias",
            command as char
        );
    }
    for command in [b'c', b'n', b'v', b's', b'o'] {
        let mut fixture = Fixture::spawn(&helper, true);
        let observed = fixture.observe_registered().unwrap();
        fixture.acknowledge();
        fixture.command(command);
        assert!(observed.revalidate_retained_inputs().is_err());
        fixture.command(b't');
        fixture.assert_proof_hangup();
        fixture.exit();
        println!(
            "PASS: retained proof continuity rejects mutation {}",
            command as char
        );
    }

    let mut fixture = Fixture::spawn(&helper, true);
    let mut foreign = Fixture::spawn(&helper, true);
    let observe = |binding: &WorkerV3ApplicationRegistrationBindingV1, peer: BorrowedFd<'_>| {
        RetainedWorkerV3ApplicationObservationV1::observe_registered_pre_ack(
            process_identity(
                &fixture.application_pidfd,
                fixture.application_pid,
                CLIENT_ID,
            ),
            process_identity(&fixture.parent_pidfd, fixture.parent.0.id(), CLIENT_ID),
            binding,
            peer,
        )
    };
    assert!(observe(&foreign.registration(), fixture.proof_peer.as_fd()).is_err());
    assert!(observe(&fixture.registration(), foreign.proof_peer.as_fd()).is_err());
    let registration = fixture.registration();
    let mut inputs = fixture.registered_expected.inputs().to_vec();
    inputs[3] = foreign.registered_expected.inputs()[3];
    let occurrence = WorkerV3ApplicationOccurrenceV1::new(
        fixture.expected.application(),
        fixture.expected.spawn_identity(),
        &inputs,
    )
    .unwrap();
    let substituted = WorkerV3ApplicationRegistrationBindingV1::new(
        registration.compiler_handoff().clone(),
        occurrence.clone(),
        registration.descriptors(),
        WorkerV3ApplicationHandoffExpectationV1::new(
            registration.expectation().envelope(),
            &occurrence,
        ),
        registration.challenge(),
    )
    .unwrap();
    assert!(observe(&substituted, fixture.proof_peer.as_fd()).is_err());
    fixture.command(b't');
    fixture.assert_proof_hangup();
    fixture.acknowledge();
    fixture.exit();
    foreign.exit();
    println!(
        "PASS: foreign process binding, counterpart and slot4 occurrence reject without retained aliases"
    );

    let mut fixture = Fixture::spawn(&helper, true);
    let observed = fixture.observe().unwrap();
    assert_eq!(observed.occurrence(), &fixture.expected);
    assert_eq!(observed.exact_envelope_bytes(), ENVELOPE_BYTES);
    assert_eq!(
        observed.expectation(),
        WorkerV3ApplicationHandoffExpectationV1::new(
            WorkerV3LoadEnvelopeIdentityV1::from_exact_bytes(ENVELOPE_BYTES).unwrap(),
            &fixture.expected
        )
    );
    fixture.acknowledge();
    observed.revalidate_retained_inputs().unwrap();
    fixture.exit();
    assert!(observed.revalidate_retained_inputs().is_err());
    println!(
        "PASS: original cross-UID app, exact ACK and EOF, ACK slot reuse, post-ACK continuity, supervised exit"
    );

    for command in [b'e', b'd', b'k', b'f', b'g', b'h', b'w', b'p', b'b', b'j'] {
        let mut fixture = Fixture::spawn(&helper, true);
        fixture.command(command);
        assert!(
            fixture.observe().is_err(),
            "accepted pre-ACK mutation {}",
            command as char
        );
        fixture.exit();
        println!(
            "PASS: pre-ACK source substitution/flags {}",
            command as char
        );
    }
    for command in [b'e', b'd', b'f', b'g', b'w', b'p', b'j'] {
        let mut fixture = Fixture::spawn(&helper, true);
        let observed = fixture.observe().unwrap();
        fixture.acknowledge();
        fixture.command(command);
        assert!(observed.revalidate_retained_inputs().is_err());
        fixture.exit();
        println!(
            "PASS: post-ACK retained source mutation {}",
            command as char
        );
    }
    let mut fixture = Fixture::spawn(&helper, false);
    assert!(fixture.observe().is_err(), "unsealed executable accepted");
    fixture.exit();
    println!("PASS: unsealed executable rejected");

    let mut fixture = Fixture::spawn(&helper, true);
    let foreign = Fixture::spawn(&helper, true);
    let result = RetainedWorkerV3ApplicationObservationV1::observe_pre_ack(
        process_identity(
            &fixture.application_pidfd,
            fixture.application_pid,
            CLIENT_ID,
        ),
        process_identity(&foreign.parent_pidfd, foreign.parent.0.id(), CLIENT_ID),
        WorkerV3ApplicationDescriptorNumbersV1::new(180, 181, 182).unwrap(),
        &fixture.expected,
        WorkerV3LoadEnvelopeIdentityV1::from_exact_bytes(ENVELOPE_BYTES).unwrap(),
    );
    assert!(result.is_err());
    assert!(
        fixture
            .observe_expected(
                &foreign.expected,
                WorkerV3LoadEnvelopeIdentityV1::from_exact_bytes(ENVELOPE_BYTES).unwrap()
            )
            .is_err()
    );
    assert!(
        fixture
            .observe_expected(
                &fixture.expected,
                WorkerV3LoadEnvelopeIdentityV1::from_exact_bytes(b"foreign envelope").unwrap()
            )
            .is_err()
    );
    fixture.acknowledge();
    fixture.exit();
    println!(
        "PASS: foreign parent/occurrence/envelope rejected; failed admission leaks no ACK writer"
    );

    let mut fixture = Fixture::spawn(&helper, true);
    let observed = fixture.observe().unwrap();
    std::fs::write(
        fixture.files.path().join("original/envelope"),
        vec![b'x'; ENVELOPE_BYTES.len()],
    )
    .unwrap();
    assert!(observed.revalidate_retained_inputs().is_err());
    fixture.exit();
    println!("PASS: same-length envelope mutation rejected");

    let mut fixture = Fixture::spawn(&helper, true);
    let result = RetainedWorkerV3ApplicationObservationV1::observe_pre_ack(
        process_identity(&fixture.application_pidfd, fixture.application_pid, 1001),
        process_identity(&fixture.parent_pidfd, fixture.parent.0.id(), 1001),
        WorkerV3ApplicationDescriptorNumbersV1::new(180, 181, 182).unwrap(),
        &fixture.expected,
        WorkerV3LoadEnvelopeIdentityV1::from_exact_bytes(ENVELOPE_BYTES).unwrap(),
    );
    assert!(result.is_err(), "unmeasured asserted UID/GID accepted");
    let application = process_identity(
        &fixture.application_pidfd,
        fixture.application_pid,
        CLIENT_ID,
    );
    let parent = process_identity(&fixture.parent_pidfd, fixture.parent.0.id(), CLIENT_ID);
    fixture.exit();
    assert!(
        RetainedWorkerV3ApplicationObservationV1::observe_pre_ack(
            application,
            parent,
            WorkerV3ApplicationDescriptorNumbersV1::new(180, 181, 182).unwrap(),
            &fixture.expected,
            WorkerV3LoadEnvelopeIdentityV1::from_exact_bytes(ENVELOPE_BYTES).unwrap(),
        )
        .is_err()
    );
    println!("PASS: independently measured credentials and exit-before-observation rejection");

    let mut fixture = Fixture::spawn(&helper, true);
    let observed = fixture.observe().unwrap();
    fixture.command(b'r');
    assert!(matches!(
        observed.revalidate_retained_inputs(),
        Err(WorkerV3ApplicationObservationErrorV1::InvalidObservation(
            "application executable object changed"
        ))
    ));
    fixture.exit();
    println!("PASS: re-exec of identical sealed bytes from another object rejected");

    let mut fixture = Fixture::spawn(&helper, true);
    let observed = fixture.observe().unwrap();
    fixture.control.write_all(b"x").unwrap();
    poll_readable(&fixture.application_pidfd);
    observed.parent.validate_liveness().unwrap();
    assert!(observed.revalidate_retained_inputs().is_err());
    fixture.reap();
    println!(
        "PASS: application-only death rejects while original parent remains alive and unreaped"
    );

    let mut fixture = Fixture::spawn(&helper, true);
    for name in ["original", "original/envelope"] {
        let file = File::open(fixture.files.path().join(name)).unwrap();
        rustix::fs::fchown(&file, None, Some(Gid::from_raw(1001))).unwrap();
    }
    let observed = fixture.observe().unwrap();
    fixture.acknowledge();
    observed.revalidate_retained_inputs().unwrap();
    fixture.exit();
    println!(
        "PASS: private artifact objects with inherited foreign GID match existing handoff policy"
    );
}

//! Actual post-exec analyzer/Verus qualification, without application or GPU authority.

use super::conditional_fill_host::{FILL_BINDING, GeneratedFillMarker};
use super::*;
#[path = "proof_controller_protocol.rs"]
#[allow(dead_code)]
mod protocol;
use fe2o3_kernel_analysis::{
    AuthenticatedPhysicalMachineEffectLimitsV1, inspect_physical_machine_effect_worker_candidate_v1,
};
use fe2o3_protected_service_profile::{
    ProofControllerCredentialProfileV1, validate_proof_controller_process_v1,
};
use fe2o3_protected_service_spawn::{
    PROTECTED_SERVICE_GATE_RELEASE_V1, PROTECTED_SERVICE_PROFILE_READY_V1,
    ProtectedServiceDescriptorBindingV1, StagedProofControllerExecV1,
};
use protocol::{Config, Message};
use std::{
    io::{Read, Seek, SeekFrom, Write},
    os::fd::AsFd,
    time::{Duration, Instant},
};

struct Capture;
impl WorkerV3AuditorV1<GeneratedFillMarker> for Capture {
    type Error = Infallible;
    type Evidence = (Vec<u8>, Vec<u8>);
    fn audit(
        &mut self,
        request: &WorkerV3VerificationRequestV1<'_, GeneratedFillMarker>,
    ) -> Result<Self::Evidence, Self::Error> {
        let _closure = super::compiler_closure::check_request(request);
        Ok((
            request
                .load_envelope_evidence_view()
                .exact_canonical_bytes()
                .to_vec(),
            request.finalized_hsaco_bytes().to_vec(),
        ))
    }
}

#[test]
#[ignore = "requires native Worker, isolated pinned runtime and an explicit fresh capture directory"]
fn prepare_fixed_controller_inputs() {
    let destination = PathBuf::from(std::env::var_os("FE2O3_PROOF_CONTROLLER_INPUTS").unwrap());
    fs::create_dir(&destination).unwrap();
    let fixture = worker_v3_fixture::published_genuine_conditional_fill_fixture();
    let (_directory, recovered) = recover_published_worker_v3_fixture(fixture);
    let admission =
        admit_recovered_worker_v3_descriptor_v1(recovered, KernelId::from_bytes(FILL_BINDING))
            .unwrap();
    let (envelope, payload) = audit_recovered_worker_v3_verification_v1::<GeneratedFillMarker, _>(
        &admission,
        &mut Capture,
    )
    .unwrap();
    let limits = AuthenticatedPhysicalMachineEffectLimitsV1::new(
        Duration::from_secs(60),
        1024 * 1024,
        16384,
    )
    .unwrap();
    let policy = inspect_physical_machine_effect_worker_candidate_v1(protocol::WORKER_PATH, limits)
        .unwrap()
        .policy();
    let runtime =
        fe2o3_verifier::FunctionalRefinementVerusRuntimeLeaseV1::open(protocol::RUNTIME_PATH)
            .unwrap();
    let config = Config {
        version: 1,
        nonce: [1; 32],
        parent_pid: std::process::id() as i32,
        kernel: FILL_BINDING,
        envelope_hash: protocol::digest(&envelope),
        envelope_len: envelope.len() as u64,
        payload_hash: protocol::digest(&payload),
        payload_len: payload.len() as u64,
        executable_hash: policy.executable().sha256(),
        executable_len: policy.executable().byte_len(),
        closure_hash: policy.runtime_closure().sha256(),
        closure_len: policy.runtime_closure().byte_len(),
        analyzer: policy.analyzer().as_bytes(),
        toolchain: policy.toolchain().as_bytes(),
        runtime: runtime.identity().as_bytes(),
    };
    fs::write(destination.join("envelope.bin"), envelope).unwrap();
    fs::write(destination.join("payload.hsaco"), payload).unwrap();
    fs::write(
        destination.join("config.json"),
        serde_json::to_vec(&config).unwrap(),
    )
    .unwrap();
}

#[test]
#[ignore = "requires private real-root PID namespace, fresh inputs, static fixture and protected runtime"]
fn root_fixed_controller_executes_and_retains_fill() {
    use fe2o3_process_identity::pidfd::ReceivedProcessPidfdV1;
    use rustix::net::{self, AddressFamily, SocketFlags, SocketType};
    assert!(rustix::process::getuid().is_root());
    let case = std::env::var("FE2O3_PROOF_CONTROLLER_CASE").unwrap();
    assert!(matches!(
        case.as_str(),
        "good" | "worker" | "runtime" | "payload" | "cancel"
    ));
    let source = PathBuf::from(std::env::var_os("FE2O3_PROOF_CONTROLLER_INPUTS").unwrap());
    let output = PathBuf::from(std::env::var_os("FE2O3_PROOF_CONTROLLER_CAPTURE").unwrap());
    fs::create_dir(&output).unwrap();
    fs::write(
        output.join("pid-namespace.txt"),
        fs::read_link("/proc/self/ns/pid")
            .unwrap()
            .as_os_str()
            .as_encoded_bytes(),
    )
    .unwrap();
    let mut config: Config =
        serde_json::from_slice(&fs::read(source.join("config.json")).unwrap()).unwrap();
    std::fs::File::open("/dev/urandom")
        .unwrap()
        .read_exact(&mut config.nonce)
        .unwrap();
    config.parent_pid = std::process::id() as i32;
    let mut payload = fs::read(source.join("payload.hsaco")).unwrap();
    match case.as_str() {
        "worker" => config.executable_hash[0] ^= 1,
        "runtime" => config.runtime[0] ^= 1,
        "payload" => {
            payload.push(0);
            config.payload_hash = protocol::digest(&payload);
            config.payload_len += 1;
        }
        _ => (),
    }
    let mut envelope =
        protocol::seal(&fs::read(source.join("envelope.bin")).unwrap(), false).unwrap();
    let mut payload = protocol::seal(&payload, false).unwrap();
    let encoded_config = serde_json::to_vec(&config).unwrap();
    fs::write(output.join("config.json"), &encoded_config).unwrap();
    let mut config_file = protocol::seal(&encoded_config, false).unwrap();
    for file in [&mut envelope, &mut payload, &mut config_file] {
        file.seek(SeekFrom::End(0)).unwrap();
    }
    let image_path = std::env::var_os("FE2O3_PROOF_CONTROLLER_FIXTURE").unwrap();
    let image = protocol::seal(&fs::read(image_path).unwrap(), true).unwrap();
    let (parent, child_end) = net::socketpair(
        AddressFamily::UNIX,
        SocketType::SEQPACKET,
        SocketFlags::CLOEXEC | SocketFlags::NONBLOCK,
        None,
    )
    .unwrap();
    net::sockopt::set_socket_passcred(&parent, true).unwrap();
    net::sockopt::set_socket_passcred(&child_end, true).unwrap();
    protocol::admit_control(parent.as_fd()).unwrap();
    protocol::admit_control(child_end.as_fd()).unwrap();
    let (ready_read, ready_write) =
        rustix::pipe::pipe_with(rustix::pipe::PipeFlags::CLOEXEC).unwrap();
    let (gate_read, gate_write) =
        rustix::pipe::pipe_with(rustix::pipe::PipeFlags::CLOEXEC).unwrap();
    let (status_read, status_write) = net::socketpair(
        AddressFamily::UNIX,
        SocketType::SEQPACKET,
        SocketFlags::CLOEXEC,
        None,
    )
    .unwrap();
    let bindings = [
        (envelope.as_fd(), 3),
        (payload.as_fd(), 4),
        (child_end.as_fd(), 5),
        (config_file.as_fd(), 6),
    ]
    .map(|(fd, destination)| ProtectedServiceDescriptorBindingV1::new(fd, destination).unwrap());
    let staged = StagedProofControllerExecV1::new(
        &image,
        &bindings,
        ready_write.as_fd(),
        gate_read.as_fd(),
        status_write.as_fd(),
    )
    .unwrap();
    let credentials =
        ProofControllerCredentialProfileV1::new(protocol::UID, protocol::GID).unwrap();
    let mut child = staged.spawn(credentials).unwrap();
    let completion = child.try_clone_pidfd().unwrap();
    let observed = ReceivedProcessPidfdV1::admit_received(
        child.try_clone_pidfd().unwrap(),
        child.pid().as_raw_pid() as u32,
    )
    .unwrap();
    drop((staged, child_end, ready_write, gate_read, status_write));
    let deadline = Instant::now() + Duration::from_secs(300);
    protocol::wait(ready_read.as_fd(), rustix::event::PollFlags::IN, deadline).unwrap();
    let mut byte = [0];
    assert_eq!(rustix::io::read(&ready_read, &mut byte).unwrap(), 1);
    assert_eq!(byte[0], PROTECTED_SERVICE_PROFILE_READY_V1);
    validate_proof_controller_process_v1(credentials, child.pid()).unwrap();
    assert_eq!(
        rustix::io::write(&gate_write, &[PROTECTED_SERVICE_GATE_RELEASE_V1]).unwrap(),
        1
    );
    protocol::wait(status_read.as_fd(), rustix::event::PollFlags::IN, deadline).unwrap();
    assert_eq!(
        rustix::io::read(&status_read, &mut byte).unwrap(),
        0,
        "exec failed"
    );
    let sender = (child.pid().as_raw_pid(), protocol::UID, protocol::GID);
    let receive = || {
        observed.revalidate().unwrap();
        let message = protocol::receive(parent.as_fd(), sender, config.nonce, deadline).unwrap();
        observed.revalidate().unwrap();
        message
    };
    let send = |message| protocol::send(parent.as_fd(), config.nonce, message, deadline).unwrap();
    let first = receive();
    if matches!(case.as_str(), "worker" | "runtime") {
        check_rejection(
            first,
            if case == "worker" {
                "WorkerIdentityMismatch"
            } else {
                "wrong protected runtime identity"
            },
            &output,
        );
        send(Message::Release);
        check_exit(&completion, 98, deadline);
        child.cancel_and_reap().unwrap();
        return;
    }
    assert!(
        matches!(first, Message::Ready { runtime } if runtime == config.runtime),
        "{first:?}"
    );
    send(Message::Start);
    assert!(matches!(receive(), Message::Executing));
    if case == "cancel" {
        let verifier = wait_for_verifier_child(child.pid().as_raw_pid(), &observed, deadline);
        fs::write(
            output.join("observed-verifier.json"),
            serde_json::to_vec(&verifier).unwrap(),
        )
        .unwrap();
        child.cancel_and_reap().unwrap();
        fs::write(
            output.join("cancelled.txt"),
            "exact controller pidfd killed and reaped; outer PID namespace contains descendants\n",
        )
        .unwrap();
        return;
    }
    let proved = receive();
    if case == "payload" {
        check_rejection(proved, "FinalizedLengthMismatch", &output);
        send(Message::Release);
        check_exit(&completion, 98, deadline);
        child.cancel_and_reap().unwrap();
        return;
    }
    let Message::Proved { evidence } = proved else {
        panic!("{proved:?}")
    };
    assert_eq!(evidence.boundary, fe2o3_functional_proof::FunctionalRefinementBoundaryV2::SemanticMirToGfx942FillDispatchConditional as u8);
    assert!(evidence.signed_and_imported && evidence.analyzer_authenticated);
    assert_eq!(evidence.authority, [false; 4]);
    assert!(evidence.pointers.iter().all(|value| *value != 0));
    let report = serde_json::to_vec(&evidence).unwrap();
    let expected_digest = protocol::digest(&report);
    fs::write(output.join("report.json"), report).unwrap();
    fs::write(output.join("proof.key"), evidence.proof_key).unwrap();
    for (index, content) in evidence.artifacts.iter().enumerate() {
        assert!((1..=128 * 1024 * 1024).contains(&content.len));
        let mut file =
            std::fs::File::create_new(output.join(protocol::ARTIFACT_NAMES[index])).unwrap();
        let mut hash = Sha256::new();
        let mut offset = 0;
        while offset < content.len {
            send(Message::ArtifactRequest {
                index: index as u8,
                offset,
            });
            let message = receive();
            let Message::Artifact {
                index: actual,
                offset: position,
                bytes,
            } = message
            else {
                panic!("{message:?}")
            };
            assert_eq!((actual as usize, position), (index, offset));
            assert_eq!(
                bytes.len(),
                (content.len - offset).min(protocol::CHUNK_BYTES as u64) as usize
            );
            file.write_all(&bytes).unwrap();
            hash.update(&bytes);
            offset += bytes.len() as u64;
        }
        assert_eq!(<[u8; 32]>::from(hash.finalize()), content.sha256);
    }
    std::thread::sleep(Duration::from_millis(50));
    send(Message::Probe);
    let retained = receive();
    assert!(
        matches!(retained, Message::Retained { digest, pointers } if digest == expected_digest && pointers == evidence.pointers),
        "{retained:?}"
    );
    fs::write(
        output.join("retention.json"),
        serde_json::to_vec(&retained).unwrap(),
    )
    .unwrap();
    send(Message::Release);
    assert!(matches!(
        protocol::receive(parent.as_fd(), sender, config.nonce, deadline).unwrap(),
        Message::Released
    ));
    check_exit(&completion, 0, deadline);
    child.cancel_and_reap().unwrap();
}

fn wait_for_verifier_child(
    controller: i32,
    observed: &fe2o3_process_identity::pidfd::ReceivedProcessPidfdV1,
    deadline: Instant,
) -> (i32, Vec<u8>) {
    loop {
        observed.revalidate().unwrap();
        assert!(
            Instant::now() < deadline,
            "no live Verus child observed before cancellation"
        );
        let mut pending = vec![controller];
        let mut seen = std::collections::BTreeSet::new();
        while let Some(pid) = pending.pop() {
            if !seen.insert(pid) {
                continue;
            }
            assert!(seen.len() <= 64, "unexpected fixture process tree size");
            if pid != controller
                && let Ok(file) = std::fs::File::open(format!("/proc/{pid}/cmdline"))
            {
                let mut command = Vec::new();
                file.take(16 * 1024).read_to_end(&mut command).unwrap();
                if command
                    .split(|byte| *byte == 0)
                    .any(|arg| arg == b"--no-cheating")
                {
                    return (pid, command);
                }
            }
            if let Ok(tasks) = fs::read_dir(format!("/proc/{pid}/task")) {
                for task in tasks.flatten() {
                    if let Ok(children) = fs::read_to_string(task.path().join("children")) {
                        pending.extend(
                            children
                                .split_whitespace()
                                .map(|value| value.parse::<i32>().unwrap()),
                        );
                    }
                }
            }
        }
        std::thread::sleep(Duration::from_millis(10));
    }
}

fn check_rejection(message: Message, expected: &str, output: &Path) {
    let Message::Rejected { ref detail, .. } = message else {
        panic!("{message:?}")
    };
    assert!(
        detail.contains(expected),
        "expected {expected}, got {detail}"
    );
    fs::write(
        output.join("rejection.json"),
        serde_json::to_vec(&message).unwrap(),
    )
    .unwrap();
}

fn check_exit(pidfd: &std::os::fd::OwnedFd, code: i32, deadline: Instant) {
    protocol::wait(pidfd.as_fd(), rustix::event::PollFlags::IN, deadline).unwrap();
    let status = rustix::process::waitid(
        rustix::process::WaitId::PidFd(pidfd.as_fd()),
        rustix::process::WaitIdOptions::EXITED | rustix::process::WaitIdOptions::NOWAIT,
    )
    .unwrap()
    .unwrap();
    assert_eq!(status.exit_status(), Some(code));
}

use super::*;

use ed25519_dalek::{Signer, SigningKey};
use fe2o3_artifact_transaction::{
    NoRetainedDurableDirectoryHooksV1, RetainedDurableDirectoryHooksV1,
    RetainedDurableRecoveryBoundaryV1,
};
use fe2o3_build_authority::{
    BrokerTranscriptValidatorV4, CapabilityBindingV4, HostLinkCommitV4, HostLinkGrantV4,
    HostLinkPrepareV4, ProcessIdentityV4,
};
use fe2o3_external_anchor_protocol::{
    AnchorPositionV1, AnchoredStateV1, HashChainHeadV1, TransactionDigestV1,
    UnsignedAnchorObservationV1,
};
use fe2o3_host_link_closure::{
    ApprovedStaticHostLldV1, ArtifactProvenanceV1, ElfClassV1, ElfEndianV1, ElfProfileV1,
    ExecutableToolchainV1, FixedRootSetV1, HostArtifactCatalogV1, HostArtifactKindV1,
    HostLinkClosureV1, HostLinkHandoffV1, HostLinkPlanSpecV1, HostLinkPlanV1, OutputTypeV1,
    PlanArgumentV1, ProducerArtifactSpecV1, PublishedHostArtifactV1, ReleaseNonceV1,
    RuntimeDsoClosureV1, TargetTripleV1,
};
use rustix::net::{AddressFamily, SocketFlags, SocketType, socketpair};
use std::fs::{self, File, OpenOptions};
use std::io::{self, Write};
use std::os::fd::{FromRawFd, OwnedFd};
use std::os::unix::fs::OpenOptionsExt;
use std::os::unix::fs::{PermissionsExt, symlink};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::OnceLock;
use std::thread;
use std::time::{Duration, Instant};
use tempfile::TempDir;

use crate::{
    AdmissionErrorKindV1, BrokerAnchorModeV1, BrokerAnchorPreparedSessionV1, BrokerHostLinkPollV1,
    BrokerHostOutputObservationV1, BrokerOwnedHostLinkExecutionV1, BrokerReservedHostLinkSessionV1,
    BrokerSessionErrorKindV1, BrokerSessionIdV1, BrokerSessionMachineV1, BrokerSessionNonceV1,
    BrokerSessionStageV1, ExpectedClientProcessIdentityV1, LiveClientPidfdIdentityV1,
    ProtectedServiceAdmissionV1,
};

const RECORD_BOUNDARIES: [RetainedDurableRecordBoundaryV1; 7] = [
    RetainedDurableRecordBoundaryV1::CreateTemp,
    RetainedDurableRecordBoundaryV1::WriteTemp,
    RetainedDurableRecordBoundaryV1::SyncTemp,
    RetainedDurableRecordBoundaryV1::RenameTempToRedo,
    RetainedDurableRecordBoundaryV1::SyncRedoName,
    RetainedDurableRecordBoundaryV1::RenameRedoToCanonical,
    RetainedDurableRecordBoundaryV1::SyncCanonicalName,
];
const TIMINGS: [RetainedDurableFaultTimingV1; 2] = [
    RetainedDurableFaultTimingV1::Before,
    RetainedDurableFaultTimingV1::After,
];
const STAGE_BOUNDARIES: [RetainedDurableArtifactBoundaryV1; 5] = [
    RetainedDurableArtifactBoundaryV1::CreateTemp,
    RetainedDurableArtifactBoundaryV1::WriteTemp,
    RetainedDurableArtifactBoundaryV1::SyncTemp,
    RetainedDurableArtifactBoundaryV1::RenameTempToStaged,
    RetainedDurableArtifactBoundaryV1::SyncStagedName,
];
const PUBLISH_BOUNDARIES: [RetainedDurableArtifactBoundaryV1; 4] = [
    RetainedDurableArtifactBoundaryV1::SetFinalMode,
    RetainedDurableArtifactBoundaryV1::SyncFinalMode,
    RetainedDurableArtifactBoundaryV1::RenameStagedToFinal,
    RetainedDurableArtifactBoundaryV1::SyncFinalName,
];

struct RecoveryBarrierHook {
    fail_at: Option<RetainedDurableFaultTimingV1>,
    after_sync: Option<Box<dyn FnMut() -> io::Result<()>>>,
    events: Vec<RetainedDurableFaultTimingV1>,
}

impl RecoveryBarrierHook {
    fn tracing() -> Self {
        Self {
            fail_at: None,
            after_sync: None,
            events: Vec::new(),
        }
    }
}

impl RetainedDurableDirectoryHooksV1 for RecoveryBarrierHook {
    fn recovery(
        &mut self,
        boundary: RetainedDurableRecoveryBoundaryV1,
        timing: RetainedDurableFaultTimingV1,
    ) -> io::Result<()> {
        assert_eq!(boundary, RetainedDurableRecoveryBoundaryV1::SyncDirectory);
        self.events.push(timing);
        if timing == RetainedDurableFaultTimingV1::After
            && let Some(after_sync) = &mut self.after_sync
        {
            after_sync()?;
        }
        if self.fail_at == Some(timing) {
            Err(io::Error::other("injected recovery barrier failure"))
        } else {
            Ok(())
        }
    }
}

struct Fixture {
    directory: TempDir,
    plan: DurableBrokerPublicationPlanV1,
    names: DurableNames,
    prepared: DurableRecordV1,
    output: Vec<u8>,
    commit_observation: [u8; ANCHOR_OBSERVATION_WIRE_LEN_V1],
    abort_observation: [u8; ANCHOR_OBSERVATION_WIRE_LEN_V1],
}

impl Fixture {
    fn new() -> Self {
        let directory = tempfile::tempdir().unwrap();
        fs::set_permissions(directory.path(), fs::Permissions::from_mode(0o700)).unwrap();
        let signing = test_signing_key();
        let key = PinnedAnchorKeyV1::from_bytes(signing.verifying_key().to_bytes()).unwrap();
        let plan = DurableBrokerPublicationPlanV1::new("linked-host-output.bin", &key).unwrap();
        let names = DurableNames::new(plan.identity);
        let output = b"exact reservation-bound admitted W0 host output".repeat(31);
        let binding = binding_fixture(plan.identity, &output);
        let service_attempt_nonce = [0xa7; 32];
        let stable = AnchoredStateV1::from_local_state(41, HashChainHeadV1::from_bytes([0x44; 32]));
        let pending = stable
            .prepare(
                TransactionDigestV1::from_bytes(anchor_transaction(binding, service_attempt_nonce)),
                &key,
            )
            .unwrap()
            .begin_advance(CallerNonceV1::from_bytes(service_attempt_nonce), &key)
            .unwrap();
        let challenge = pending.challenge().clone();
        let commit_observation =
            signed_observation(&challenge, AnchorPositionV1::Proposed, &signing);
        let abort_observation = signed_observation(&challenge, AnchorPositionV1::Prior, &signing);
        let mut challenge_bytes = [0_u8; ANCHOR_CHALLENGE_WIRE_LEN_V1];
        challenge_bytes.copy_from_slice(challenge.as_bytes());
        let prepared = DurableRecordV1 {
            state: RecordStateV1::Prepared,
            plan_identity: plan.identity,
            destination: plan.destination.clone(),
            binding,
            challenge: challenge_bytes,
            anchor_key_bytes: key.to_bytes(),
            observation: None,
        };
        validate_record(&prepared, &plan).unwrap();
        Self {
            directory,
            plan,
            names,
            prepared,
            output,
            commit_observation,
            abort_observation,
        }
    }

    fn root(&self) -> OwnedFd {
        root(self.directory.path())
    }

    fn store(&self) -> RetainedDurableDirectoryV1 {
        RetainedDurableDirectoryV1::admit_service_owned(self.root()).unwrap()
    }

    fn persist_prepared(&self) {
        persist_prepared(&self.store(), &self.names, &self.prepared, &self.output);
    }
}

struct RealPreparedFixture {
    directory: TempDir,
    plan: DurableBrokerPublicationPlanV1,
    prepared_session: BrokerAnchorPreparedSessionV1,
    transcript: CompletedBrokerTranscriptV4,
    signing: SigningKey,
    output_bytes: Vec<u8>,
    _client_peer: OwnedFd,
}

#[derive(Clone, Copy)]
struct RealHostLinkCommitContext {
    process: ProcessIdentityV4,
    binding: CapabilityBindingV4,
    request_identity: [u8; 32],
    plan_identity: [u8; 32],
    closure_identity: [u8; 32],
    grant_identity: [u8; 32],
    durable_plan_identity: [u8; 32],
}

impl RealHostLinkCommitContext {
    fn commit(
        self,
        output_sha256: [u8; 32],
        output_length: u64,
        output_mode: u32,
        grant_identity: Option<[u8; 32]>,
    ) -> HostLinkCommitV4 {
        HostLinkCommitV4::new(
            self.process,
            self.binding.identity_sha256(),
            self.request_identity,
            self.plan_identity,
            self.closure_identity,
            grant_identity.unwrap_or(self.grant_identity),
            output_sha256,
            output_length,
            output_mode,
            self.durable_plan_identity,
        )
        .unwrap()
    }
}

fn release_nonce() -> ReleaseNonceV1 {
    ReleaseNonceV1::new([0x73; 32]).unwrap()
}

fn host_target() -> TargetTripleV1 {
    TargetTripleV1::new("x86_64-unknown-linux-gnu").unwrap()
}

fn minimal_relocatable_elf() -> Vec<u8> {
    let mut bytes = vec![0_u8; 64];
    bytes[..4].copy_from_slice(b"\x7fELF");
    bytes[4] = 2;
    bytes[5] = 1;
    bytes[6] = 1;
    bytes[16..18].copy_from_slice(&1_u16.to_le_bytes());
    bytes[18..20].copy_from_slice(&62_u16.to_le_bytes());
    bytes[20..24].copy_from_slice(&1_u32.to_le_bytes());
    bytes[52..54].copy_from_slice(&64_u16.to_le_bytes());
    bytes[54..56].copy_from_slice(&56_u16.to_le_bytes());
    bytes[58..60].copy_from_slice(&64_u16.to_le_bytes());
    bytes
}

fn expected_static_output_profile() -> ElfProfileV1 {
    ElfProfileV1 {
        class: ElfClassV1::Elf64,
        endian: ElfEndianV1::Little,
        elf_type: 2,
        machine: 62,
        interpreter: None,
        soname: None,
        needed: vec![],
        has_writable_executable_segment: false,
        has_executable_stack: false,
    }
}

fn expected_real_host_output() -> Vec<u8> {
    let mut output = vec![0_u8; 121];
    output[..4].copy_from_slice(b"\x7fELF");
    output[4] = 2;
    output[5] = 1;
    output[6] = 1;
    output[16..18].copy_from_slice(&2_u16.to_le_bytes());
    output[18..20].copy_from_slice(&62_u16.to_le_bytes());
    output[20..24].copy_from_slice(&1_u32.to_le_bytes());
    output[24..32].copy_from_slice(&0x400078_u64.to_le_bytes());
    output[32..40].copy_from_slice(&64_u64.to_le_bytes());
    output[52..54].copy_from_slice(&64_u16.to_le_bytes());
    output[54..56].copy_from_slice(&56_u16.to_le_bytes());
    output[56..58].copy_from_slice(&1_u16.to_le_bytes());
    output[58..60].copy_from_slice(&64_u16.to_le_bytes());
    output[64..68].copy_from_slice(&1_u32.to_le_bytes());
    output[68..72].copy_from_slice(&5_u32.to_le_bytes());
    output[80..88].copy_from_slice(&0x400000_u64.to_le_bytes());
    output[88..96].copy_from_slice(&0x400000_u64.to_le_bytes());
    output[96..104].copy_from_slice(&121_u64.to_le_bytes());
    output[104..112].copy_from_slice(&121_u64.to_le_bytes());
    output[112..120].copy_from_slice(&0x1000_u64.to_le_bytes());
    output[120] = 0xc3;
    output
}

fn real_worker_path() -> &'static Path {
    static WORKER: OnceLock<PathBuf> = OnceLock::new();
    WORKER
        .get_or_init(|| {
            if let Some(path) = std::env::var_os("FE2O3_TEST_HOST_LINK_WORKER") {
                return PathBuf::from(path);
            }
            let directory = TempDir::new().unwrap().keep();
            let output = directory.join("fe2o3-broker-durable-host-link-worker");
            let source = Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../fe2o3-host-link-closure/tests/fixtures/host_link_worker.c");
            let mut command = Command::new("cc");
            command
                .args(["-std=c11", "-O2", "-static", "-Wall", "-Wextra", "-Werror"])
                .arg(&source)
                .arg("-o")
                .arg(&output);
            let result = crate::test_process_execution::capture_output(&mut command).unwrap();
            assert!(
                result.status.success(),
                "failed to compile static host-link worker fixture: {}",
                String::from_utf8_lossy(&result.stderr)
            );
            output
        })
        .as_path()
}

fn publish_host_file(label: &str, kind: HostArtifactKindV1, file: File) -> PublishedHostArtifactV1 {
    PublishedHostArtifactV1::from_producer_fd(
        file,
        ProducerArtifactSpecV1::new(
            label,
            kind,
            ArtifactProvenanceV1::Compiler,
            release_nonce(),
            host_target(),
        )
        .unwrap(),
    )
    .unwrap()
}

fn source_host_file(root: &TempDir, name: &str, bytes: &[u8], mode: u32) -> File {
    let mut file = OpenOptions::new()
        .create_new(true)
        .read(true)
        .write(true)
        .mode(mode)
        .open(root.path().join(name))
        .unwrap();
    file.write_all(bytes).unwrap();
    file
}

fn real_host_link_closure() -> HostLinkClosureV1 {
    let root = TempDir::new().unwrap();
    let worker = real_worker_path();
    let wrapper = publish_host_file(
        "wrapper",
        HostArtifactKindV1::StaticWrapper,
        File::open(worker).unwrap(),
    );
    let wrapper_id = wrapper.id();
    let lld = publish_host_file(
        "host-lld",
        HostArtifactKindV1::StaticHostLld,
        File::open(worker).unwrap(),
    );
    let lld_id = lld.id();
    let object = publish_host_file(
        "input.o",
        HostArtifactKindV1::Object,
        source_host_file(&root, "input.o", &minimal_relocatable_elf(), 0o644),
    );
    let object_id = object.id();
    let spec = HostLinkPlanSpecV1 {
        release_nonce: release_nonce(),
        target: host_target(),
        toolchain: ExecutableToolchainV1 {
            static_wrapper: wrapper_id,
            static_host_lld: lld_id,
            llvm_build_identity: "upstream-llvmorg-22.1.8-broker-durable-test".to_owned(),
        },
        output_type: OutputTypeV1::Executable,
        expected_output_mode: HOST_LINK_OUTPUT_MODE_V4,
        expected_output_elf: expected_static_output_profile(),
        arguments: vec![PlanArgumentV1::ProducerArtifact(object_id)],
        runtime_dsos: RuntimeDsoClosureV1::default(),
    };
    let handoff = HostLinkHandoffV1::new(spec, vec![object, lld, wrapper]).unwrap();
    let (plan, producers) = handoff.into_parts();
    let plan = HostLinkPlanV1::from_sealed_fd(plan, producers).unwrap();
    let mut closure = HostLinkClosureV1::prepare(
        plan,
        FixedRootSetV1::new(vec![]).unwrap(),
        HostArtifactCatalogV1::new(release_nonce(), host_target()),
    )
    .unwrap();
    closure.prevalidate().unwrap();
    closure
}

#[allow(unsafe_code)]
fn current_process_pidfd() -> OwnedFd {
    let pid = libc::pid_t::try_from(std::process::id()).unwrap();
    // SAFETY: pidfd_open receives the current positive process ID and zero flags. A nonnegative
    // result is a new close-on-exec descriptor owned by this test.
    let descriptor = unsafe { libc::syscall(libc::SYS_pidfd_open, pid, 0) };
    assert!(
        descriptor >= 0,
        "pidfd_open failed: {}",
        io::Error::last_os_error()
    );
    // SAFETY: successful pidfd_open returned one fresh owned descriptor.
    unsafe { OwnedFd::from_raw_fd(descriptor as i32) }
}

fn real_test_admission(directory: &TempDir) -> (ProtectedServiceAdmissionV1, OwnedFd) {
    let (service_peer, client_peer) = socketpair(
        AddressFamily::UNIX,
        SocketType::SEQPACKET,
        SocketFlags::CLOEXEC,
        None,
    )
    .unwrap();
    let expected = ExpectedClientProcessIdentityV1::new(
        std::process::id(),
        rustix::process::geteuid().as_raw(),
        rustix::process::getegid().as_raw(),
    )
    .unwrap();
    let live = LiveClientPidfdIdentityV1::admit(current_process_pidfd(), expected).unwrap();
    let admission = ProtectedServiceAdmissionV1::admit_non_authoritative_same_uid_session_test(
        File::open(directory.path()).unwrap().into(),
        service_peer,
        live,
    )
    .unwrap();
    (admission, client_peer)
}

#[allow(unsafe_code)]
fn reserved_real_host_link_for_rejection(
    seed: u8,
    wrong_tool_identity: bool,
    wrong_grant_request: bool,
) -> (
    BrokerReservedHostLinkSessionV1,
    HostLinkClosureV1,
    HostLinkGrantV4,
    ApprovedStaticHostLldV1,
    RealHostLinkCommitContext,
    OwnedFd,
) {
    let directory = TempDir::new().unwrap();
    fs::set_permissions(directory.path(), fs::Permissions::from_mode(0o700)).unwrap();
    let (admission, client_peer) = real_test_admission(&directory);
    let (client_pid, client_start_time_ticks) = admission.non_authoritative_test_process_identity();
    let process = ProcessIdentityV4::new(client_pid, client_start_time_ticks).unwrap();
    let closure = real_host_link_closure();
    let plan_identity = *closure.plan_digest().as_bytes();
    let closure_identity = *closure.closure_digest().as_bytes();
    let actual_tool_identity = *closure
        .static_host_lld_artifact_id()
        .unwrap()
        .sha256()
        .as_bytes();
    let approval = unsafe { ApprovedStaticHostLldV1::from_verified_evidence(&closure) }.unwrap();
    let static_host_lld_identity = if wrong_tool_identity {
        [seed.wrapping_add(0x51).max(1); 32]
    } else {
        actual_tool_identity
    };
    let binding =
        CapabilityBindingV4::new([0x41; 32], [0x42; 32], static_host_lld_identity).unwrap();
    let request_identity = [seed.wrapping_add(1).max(1); 32];
    let prepare = HostLinkPrepareV4::new(
        process,
        binding.identity_sha256(),
        request_identity,
        plan_identity,
        closure_identity,
    )
    .unwrap();
    let prepared = BrokerTranscriptValidatorV4::new(binding, process)
        .validate_prepare(prepare)
        .unwrap();
    let grant_request_identity = if wrong_grant_request {
        [seed.wrapping_add(2).max(1); 32]
    } else {
        request_identity
    };
    let grant_identity = [seed.wrapping_add(3).max(1); 32];
    let grant = HostLinkGrantV4::new(
        process,
        binding.identity_sha256(),
        grant_request_identity,
        plan_identity,
        closure_identity,
        grant_identity,
    )
    .unwrap();
    let durable_plan_identity = [seed.wrapping_add(6).max(1); 32];
    let reserved = BrokerSessionMachineV1::new()
        .reserve_prepared_link(
            admission,
            BrokerSessionIdV1::from_bytes([seed.wrapping_add(4).max(1); 32]).unwrap(),
            BrokerSessionNonceV1::from_bytes([seed.wrapping_add(5).max(1); 32]).unwrap(),
            prepared,
            DurablePublicationPlanIdentityV1::from_bytes(durable_plan_identity).unwrap(),
        )
        .unwrap();
    (
        reserved,
        closure,
        grant,
        approval,
        RealHostLinkCommitContext {
            process,
            binding,
            request_identity,
            plan_identity,
            closure_identity,
            grant_identity,
            durable_plan_identity,
        },
        client_peer,
    )
}

fn await_broker_owned_output(
    execution: &mut BrokerOwnedHostLinkExecutionV1,
) -> BrokerHostOutputObservationV1 {
    let deadline = Instant::now() + Duration::from_secs(35);
    loop {
        match execution.poll_output().unwrap() {
            BrokerHostLinkPollV1::Pending => {
                assert!(Instant::now() < deadline, "authenticated launch timed out");
                thread::sleep(Duration::from_millis(1));
            }
            BrokerHostLinkPollV1::Admitted(output) => return output,
        }
    }
}

#[allow(unsafe_code)]
fn real_prepared_fixture(seed: u8) -> RealPreparedFixture {
    let directory = TempDir::new().unwrap();
    fs::set_permissions(directory.path(), fs::Permissions::from_mode(0o700)).unwrap();
    let (admission, client_peer) = real_test_admission(&directory);
    assert_eq!(
        admission.validate_continuity().unwrap_err().kind(),
        AdmissionErrorKindV1::SameUidClient
    );
    let (client_pid, client_start_time_ticks) = admission.non_authoritative_test_process_identity();
    let process = ProcessIdentityV4::new(client_pid, client_start_time_ticks).unwrap();
    let signing = SigningKey::from_bytes(&[seed.max(1); 32]);
    let key = PinnedAnchorKeyV1::from_bytes(signing.verifying_key().to_bytes()).unwrap();
    let plan =
        DurableBrokerPublicationPlanV1::new(format!("real-host-output-{seed:02x}.bin"), &key)
            .unwrap();
    let closure = real_host_link_closure();
    let plan_identity = *closure.plan_digest().as_bytes();
    let closure_identity = *closure.closure_digest().as_bytes();
    let static_host_lld_identity = *closure
        .static_host_lld_artifact_id()
        .unwrap()
        .sha256()
        .as_bytes();
    let approval = unsafe { ApprovedStaticHostLldV1::from_verified_evidence(&closure) }.unwrap();
    let binding =
        CapabilityBindingV4::new([0x31; 32], [0x32; 32], static_host_lld_identity).unwrap();
    let request_identity = [seed.wrapping_add(1).max(1); 32];
    let grant_identity = [seed.wrapping_add(2).max(1); 32];
    let prepare = HostLinkPrepareV4::new(
        process,
        binding.identity_sha256(),
        request_identity,
        plan_identity,
        closure_identity,
    )
    .unwrap();
    let prepared_transcript = BrokerTranscriptValidatorV4::new(binding, process)
        .validate_prepare(prepare)
        .unwrap();
    let grant = HostLinkGrantV4::new(
        process,
        binding.identity_sha256(),
        request_identity,
        plan_identity,
        closure_identity,
        grant_identity,
    )
    .unwrap();
    let reserved = BrokerSessionMachineV1::new()
        .reserve_prepared_link(
            admission,
            BrokerSessionIdV1::from_bytes([seed.wrapping_add(3).max(1); 32]).unwrap(),
            BrokerSessionNonceV1::from_bytes([seed.wrapping_add(4).max(1); 32]).unwrap(),
            prepared_transcript,
            plan.broker_identity().unwrap(),
        )
        .unwrap();
    assert_eq!(reserved.stage(), BrokerSessionStageV1::Reserved);
    let mut execution = reserved.grant_and_launch(closure, grant, approval).unwrap();
    let output = await_broker_owned_output(&mut execution);
    assert_eq!(execution.output_observation(), Some(output));
    assert_eq!(
        execution.poll_output().unwrap(),
        BrokerHostLinkPollV1::Admitted(output)
    );
    let commit = HostLinkCommitV4::new(
        process,
        binding.identity_sha256(),
        request_identity,
        plan_identity,
        closure_identity,
        grant_identity,
        output.sha256(),
        output.length(),
        output.mode(),
        plan.identity_bytes(),
    )
    .unwrap();
    let completed = execution.complete(commit).unwrap();
    assert_eq!(completed.stage(), BrokerSessionStageV1::Completed);
    let (machine, transcript) = completed.into_parts();
    let prepared_session = machine
        .prepare_anchor(
            BrokerAnchorModeV1::Advance,
            AnchoredStateV1::from_local_state(
                u64::from(seed) + 5,
                HashChainHeadV1::from_bytes([seed.wrapping_add(5).max(1); 32]),
            ),
            &key,
        )
        .unwrap();
    assert_eq!(
        prepared_session.stage(),
        BrokerSessionStageV1::AnchorPrepared
    );
    assert_eq!(prepared_session.authority(), "none");
    let debug = format!("{prepared_session:?}");
    assert!(!debug.contains("challenge"));
    assert!(!debug.contains("nonce"));
    RealPreparedFixture {
        directory,
        plan,
        prepared_session,
        transcript,
        signing,
        output_bytes: expected_real_host_output(),
        _client_peer: client_peer,
    }
}

#[test]
fn broker_owned_link_rejects_static_tool_substitution_before_launch() {
    let (reserved, closure, grant, approval, _context, _client_peer) =
        reserved_real_host_link_for_rejection(0x21, true, false);
    assert_eq!(reserved.stage(), BrokerSessionStageV1::Reserved);
    assert_eq!(
        reserved
            .grant_and_launch(closure, grant, approval)
            .unwrap_err()
            .kind(),
        BrokerSessionErrorKindV1::HostLinkToolIdentityMismatch
    );
}

#[test]
fn broker_owned_link_rejects_grant_substitution_before_launch() {
    let (reserved, closure, grant, approval, _context, _client_peer) =
        reserved_real_host_link_for_rejection(0x31, false, true);
    assert_eq!(
        reserved
            .grant_and_launch(closure, grant, approval)
            .unwrap_err()
            .kind(),
        BrokerSessionErrorKindV1::HostLinkGrantMismatch
    );
}

#[test]
fn broker_owned_link_consumes_running_execution_on_premature_commit() {
    let (reserved, closure, grant, approval, context, _client_peer) =
        reserved_real_host_link_for_rejection(0x41, false, false);
    let execution = reserved.grant_and_launch(closure, grant, approval).unwrap();
    let commit = context.commit([0x91; 32], 121, HOST_LINK_OUTPUT_MODE_V4, None);
    assert_eq!(
        execution.complete(commit).unwrap_err().kind(),
        BrokerSessionErrorKindV1::HostLinkOutputPending
    );
}

#[test]
fn broker_owned_link_rejects_commit_substitution_after_admission() {
    let (reserved, closure, grant, approval, context, _client_peer) =
        reserved_real_host_link_for_rejection(0x51, false, false);
    let mut execution = reserved.grant_and_launch(closure, grant, approval).unwrap();
    let output = await_broker_owned_output(&mut execution);
    let commit = context.commit(
        output.sha256(),
        output.length(),
        output.mode(),
        Some([0xe7; 32]),
    );
    assert_eq!(
        execution.complete(commit).unwrap_err().kind(),
        BrokerSessionErrorKindV1::HostLinkCommitMismatch
    );
}

fn sign_transaction_challenge(
    transaction: &BrokerDurableSessionTransactionV1,
    position: AnchorPositionV1,
    signing: &SigningKey,
) -> [u8; ANCHOR_OBSERVATION_WIRE_LEN_V1] {
    let challenge = AnchorChallengeV1::decode(transaction.challenge_bytes()).unwrap();
    signed_observation(&challenge, position, signing)
}

fn read_record(directory: &Path, plan: &DurableBrokerPublicationPlanV1) -> DurableRecordV1 {
    let store = RetainedDurableDirectoryV1::admit_service_owned(root(directory)).unwrap();
    let names = DurableNames::new(plan.identity_bytes());
    let bytes = store
        .read_private(&names.record, MAX_BROKER_DURABLE_RECORD_BYTES_V1)
        .unwrap()
        .unwrap();
    DurableRecordV1::decode(&bytes).unwrap()
}

#[test]
fn caller_visible_inputs_cannot_reconstruct_service_nonce_bound_challenge() {
    let fixture = real_prepared_fixture(0x66);
    let service_attempt_nonce = [0xd4; 32];
    let transaction = prepare_durable_broker_session_v1_with_options(
        fixture.prepared_session,
        fixture.plan.clone(),
        BrokerDurableOptionsV1::with_test_service_nonce(service_attempt_nonce),
    )
    .unwrap();
    let actual = AnchorChallengeV1::decode(transaction.challenge_bytes()).unwrap();
    let record = read_record(fixture.directory.path(), &fixture.plan);
    let key = PinnedAnchorKeyV1::from_bytes(record.anchor_key_bytes).unwrap();

    // This binds every deterministic field the caller knows, including the full V4/W0/plan
    // binding, but substitutes the caller's session nonce for the unavailable service nonce.
    let guessed =
        AnchoredStateV1::from_local_state(actual.expected_sequence() - 1, actual.prior_head())
            .prepare(
                TransactionDigestV1::from_bytes(anchor_transaction(
                    record.binding,
                    record.binding.session_nonce,
                )),
                &key,
            )
            .unwrap()
            .begin_advance(
                CallerNonceV1::from_bytes(record.binding.session_nonce),
                &key,
            )
            .unwrap();

    assert_eq!(actual.nonce(), service_attempt_nonce);
    assert_eq!(
        actual.transaction().to_bytes(),
        anchor_transaction(record.binding, service_attempt_nonce)
    );
    assert_ne!(actual.as_bytes(), guessed.challenge().as_bytes());
    assert_ne!(actual.nonce(), record.binding.session_nonce);
}

#[test]
fn entropy_failure_consumes_prepared_capability_without_forming_a_record() {
    let fixture = real_prepared_fixture(0x67);
    let names = DurableNames::new(fixture.plan.identity_bytes());
    let result = prepare_durable_broker_session_v1_with_options(
        fixture.prepared_session,
        fixture.plan.clone(),
        BrokerDurableOptionsV1::with_test_entropy_failure(),
    );
    let error = result.unwrap_err();
    assert!(matches!(error, BrokerDurableSessionErrorV1::Entropy(_)));
    assert!(!format!("{error:?}").contains("challenge"));
    assert!(fixture.directory.path().join(&names.staged).is_file());
    assert!(!fixture.directory.path().join(&names.record).exists());
    assert!(!fixture.directory.path().join(&names.redo).exists());
    assert!(
        recover_prepared_durable_broker_session_v1(root(fixture.directory.path()), fixture.plan,)
            .is_err()
    );
}

#[test]
fn recovered_prepared_capability_reemits_exact_challenge_and_reconciles() {
    let fixture = real_prepared_fixture(0x68);
    let transaction =
        prepare_durable_broker_session_v1(fixture.prepared_session, fixture.plan.clone()).unwrap();
    let challenge = *transaction.challenge_bytes();
    drop(transaction);

    let recovered = recover_prepared_durable_broker_session_v1(
        root(fixture.directory.path()),
        fixture.plan.clone(),
    )
    .unwrap();
    assert_eq!(recovered.authority(), "none");
    assert_eq!(recovered.challenge_bytes(), &challenge);
    let debug = format!("{recovered:?}");
    assert!(!debug.contains("challenge"));
    assert!(!debug.contains("nonce"));
    let decoded = AnchorChallengeV1::decode(recovered.challenge_bytes()).unwrap();
    let observation = signed_observation(&decoded, AnchorPositionV1::Proposed, &fixture.signing);
    assert_eq!(
        recovered.observe_and_recover(&observation).unwrap(),
        BrokerDurableRecoveryV1::Published
    );
    assert_eq!(
        fs::read(fixture.directory.path().join(fixture.plan.destination())).unwrap(),
        fixture.output_bytes
    );
}

#[test]
fn real_public_lifecycle_releases_challenge_only_after_durable_prepared() {
    let fixture = real_prepared_fixture(0x61);
    let names = DurableNames::new(fixture.plan.identity_bytes());
    assert!(!fixture.directory.path().join(&names.record).exists());
    assert!(!fixture.directory.path().join(&names.staged).exists());
    assert!(
        !fixture
            .directory
            .path()
            .join(fixture.plan.destination())
            .exists()
    );

    let transaction =
        prepare_durable_broker_session_v1(fixture.prepared_session, fixture.plan.clone()).unwrap();
    let transaction_debug = format!("{transaction:?}");
    assert!(!transaction_debug.contains("challenge"));
    assert!(!transaction_debug.contains("nonce"));
    assert_eq!(
        inspect_durable_broker_session_v1(root(fixture.directory.path()), fixture.plan.clone())
            .unwrap(),
        BrokerDurableRecoveryV1::Prepared
    );
    assert!(fixture.directory.path().join(&names.record).is_file());
    assert!(fixture.directory.path().join(&names.staged).is_file());
    assert!(
        !fixture
            .directory
            .path()
            .join(fixture.plan.destination())
            .exists()
    );

    let observation =
        sign_transaction_challenge(&transaction, AnchorPositionV1::Proposed, &fixture.signing);
    assert_eq!(
        transaction
            .observe_and_consume(&observation, &fixture.transcript)
            .unwrap(),
        BrokerDurableOutcomeV1::Published
    );
    assert_eq!(
        fs::read(fixture.directory.path().join(fixture.plan.destination())).unwrap(),
        fixture.output_bytes
    );
    assert_eq!(
        inspect_durable_broker_session_v1(root(fixture.directory.path()), fixture.plan.clone())
            .unwrap(),
        BrokerDurableRecoveryV1::Published
    );
}

#[test]
fn real_prepare_fsync_failure_releases_no_transaction_or_challenge() {
    let fixture = real_prepared_fixture(0x65);
    let names = DurableNames::new(fixture.plan.identity_bytes());
    let result = prepare_durable_broker_session_v1_with_options(
        fixture.prepared_session,
        fixture.plan.clone(),
        BrokerDurableOptionsV1::inject_crash(BrokerDurableFaultPointV1::Record {
            stage: BrokerDurableRecordStageV1::Prepared,
            boundary: RetainedDurableRecordBoundaryV1::SyncCanonicalName,
            timing: RetainedDurableFaultTimingV1::Before,
        }),
    );
    let error = result.unwrap_err();
    assert!(matches!(
        &error,
        BrokerDurableSessionErrorV1::InjectedCrash { .. }
    ));
    assert!(!format!("{error:?}").contains("challenge"));
    assert!(fixture.directory.path().join(&names.record).is_file());
    assert!(fixture.directory.path().join(&names.staged).is_file());
    assert!(
        !fixture
            .directory
            .path()
            .join(fixture.plan.destination())
            .exists()
    );
    let mut failed_barrier = RecoveryBarrierHook {
        fail_at: Some(RetainedDurableFaultTimingV1::Before),
        ..RecoveryBarrierHook::tracing()
    };
    let recovery = recover_prepared_durable_broker_session_v1_with_hooks(
        root(fixture.directory.path()),
        fixture.plan.clone(),
        &mut failed_barrier,
    );
    let error = recovery.unwrap_err();
    assert!(!format!("{error:?}").contains("challenge"));
    assert_eq!(
        failed_barrier.events,
        [RetainedDurableFaultTimingV1::Before]
    );

    let mut successful_barrier = RecoveryBarrierHook::tracing();
    let recovered = recover_prepared_durable_broker_session_v1_with_hooks(
        root(fixture.directory.path()),
        fixture.plan.clone(),
        &mut successful_barrier,
    )
    .unwrap();
    assert_eq!(
        successful_barrier.events,
        [
            RetainedDurableFaultTimingV1::Before,
            RetainedDurableFaultTimingV1::After
        ]
    );
    assert!(AnchorChallengeV1::decode(recovered.challenge_bytes()).is_ok());
}

#[test]
fn recovery_barrier_rejects_post_sync_record_artifact_or_destination_changes() {
    #[derive(Clone, Copy)]
    enum Mutation {
        Canonical,
        Redo,
        Staged,
        Destination,
    }

    for (offset, mutation) in [
        Mutation::Canonical,
        Mutation::Redo,
        Mutation::Staged,
        Mutation::Destination,
    ]
    .into_iter()
    .enumerate()
    {
        let fixture = real_prepared_fixture(0x69 + offset as u8);
        let names = DurableNames::new(fixture.plan.identity_bytes());
        let transaction =
            prepare_durable_broker_session_v1(fixture.prepared_session, fixture.plan.clone())
                .unwrap();
        drop(transaction);
        let (target, replacement, mode) = match mutation {
            Mutation::Canonical => (
                fixture.directory.path().join(&names.record),
                b"hostile canonical replacement".to_vec(),
                0o600,
            ),
            Mutation::Redo => (
                fixture.directory.path().join(&names.redo),
                b"hostile redo replacement".to_vec(),
                0o600,
            ),
            Mutation::Staged => (
                fixture.directory.path().join(&names.staged),
                b"stale staged output".to_vec(),
                0o600,
            ),
            Mutation::Destination => (
                fixture.directory.path().join(fixture.plan.destination()),
                fixture.output_bytes.clone(),
                HOST_LINK_OUTPUT_MODE_V4,
            ),
        };
        let mut hook = RecoveryBarrierHook {
            after_sync: Some(Box::new(move || {
                fs::write(&target, &replacement)?;
                fs::set_permissions(&target, fs::Permissions::from_mode(mode))
            })),
            ..RecoveryBarrierHook::tracing()
        };

        let result = recover_prepared_durable_broker_session_v1_with_hooks(
            root(fixture.directory.path()),
            fixture.plan.clone(),
            &mut hook,
        );
        let error = result.unwrap_err();
        assert!(!format!("{error:?}").contains("challenge"));
        assert_eq!(
            hook.events,
            [
                RetainedDurableFaultTimingV1::Before,
                RetainedDurableFaultTimingV1::After
            ]
        );
    }
}

#[test]
fn real_public_lifecycle_durably_aborts_without_publication() {
    let fixture = real_prepared_fixture(0x62);
    let transaction =
        prepare_durable_broker_session_v1(fixture.prepared_session, fixture.plan.clone()).unwrap();
    let observation =
        sign_transaction_challenge(&transaction, AnchorPositionV1::Prior, &fixture.signing);
    assert_eq!(
        transaction
            .observe_and_consume(&observation, &fixture.transcript)
            .unwrap(),
        BrokerDurableOutcomeV1::Aborted
    );
    assert_eq!(
        inspect_durable_broker_session_v1(root(fixture.directory.path()), fixture.plan.clone())
            .unwrap(),
        BrokerDurableRecoveryV1::Aborted
    );
    assert!(
        !fixture
            .directory
            .path()
            .join(fixture.plan.destination())
            .exists()
    );
}

#[test]
fn real_public_lifecycle_recovers_both_cross_system_crash_windows() {
    let before_local_commit = real_prepared_fixture(0x63);
    let transaction = prepare_durable_broker_session_v1(
        before_local_commit.prepared_session,
        before_local_commit.plan.clone(),
    )
    .unwrap();
    let observation = sign_transaction_challenge(
        &transaction,
        AnchorPositionV1::Proposed,
        &before_local_commit.signing,
    );
    let result = transaction.observe_and_consume_with_options(
        &observation,
        &before_local_commit.transcript,
        BrokerDurableOptionsV1::inject_crash(BrokerDurableFaultPointV1::Record {
            stage: BrokerDurableRecordStageV1::AnchorCommitted,
            boundary: RetainedDurableRecordBoundaryV1::CreateTemp,
            timing: RetainedDurableFaultTimingV1::Before,
        }),
    );
    assert!(matches!(
        result,
        Err(BrokerDurableSessionErrorV1::InjectedCrash { .. })
    ));
    assert_eq!(
        inspect_durable_broker_session_v1(
            root(before_local_commit.directory.path()),
            before_local_commit.plan.clone(),
        )
        .unwrap(),
        BrokerDurableRecoveryV1::Prepared
    );
    assert_eq!(
        recover_durable_broker_session_v1(
            root(before_local_commit.directory.path()),
            before_local_commit.plan.clone(),
            Some(&observation),
        )
        .unwrap(),
        BrokerDurableRecoveryV1::Published
    );

    let after_local_commit = real_prepared_fixture(0x64);
    let transaction = prepare_durable_broker_session_v1(
        after_local_commit.prepared_session,
        after_local_commit.plan.clone(),
    )
    .unwrap();
    let observation = sign_transaction_challenge(
        &transaction,
        AnchorPositionV1::Proposed,
        &after_local_commit.signing,
    );
    let result = transaction.observe_and_consume_with_options(
        &observation,
        &after_local_commit.transcript,
        BrokerDurableOptionsV1::inject_crash(BrokerDurableFaultPointV1::Record {
            stage: BrokerDurableRecordStageV1::AnchorCommitted,
            boundary: RetainedDurableRecordBoundaryV1::SyncCanonicalName,
            timing: RetainedDurableFaultTimingV1::After,
        }),
    );
    assert!(matches!(
        result,
        Err(BrokerDurableSessionErrorV1::InjectedCrash { .. })
    ));
    assert_eq!(
        recover_durable_broker_session_v1(
            root(after_local_commit.directory.path()),
            after_local_commit.plan.clone(),
            None,
        )
        .unwrap(),
        BrokerDurableRecoveryV1::Published
    );
}

fn test_signing_key() -> SigningKey {
    SigningKey::from_bytes(&[0x5a; 32])
}

fn test_anchor_key() -> PinnedAnchorKeyV1 {
    PinnedAnchorKeyV1::from_bytes(test_signing_key().verifying_key().to_bytes()).unwrap()
}

fn root(path: &Path) -> OwnedFd {
    File::open(path).unwrap().into()
}

fn binding_fixture(plan_identity: [u8; 32], output: &[u8]) -> BrokerDurableBindingV1 {
    const CLAIM_DOMAIN: &[u8] = b"FE2O3/BROKER-V4/SESSION-CLAIM-DIGEST/V1\0";
    const TRANSCRIPT_DOMAIN: &[u8] = b"FE2O3/BROKER-V4/COMPLETED-TRANSCRIPT-DIGEST/V1\0";
    const RESERVATION_DOMAIN: &[u8] = b"FE2O3/BROKER-SESSION/LINK-RESERVATION/V1\0";
    let mut binding = BrokerDurableBindingV1 {
        session_id: [0x11; 32],
        session_nonce: [0x12; 32],
        reservation_digest: [0; 32],
        request_nonce_sha256: [0x13; 32],
        client_pid: 0x1020_3040,
        client_start_time_ticks: 0x0102_0304_0506_0708,
        claim_digest: [0; 32],
        transcript_digest: [0; 32],
        transcript_binding_identity: [0x21; 32],
        transcript_request_identity: [0x22; 32],
        transcript_plan_identity: [0x23; 32],
        transcript_closure_identity: [0x24; 32],
        transcript_grant_identity: [0x25; 32],
        output_digest: Sha256::digest(output).into(),
        output_length: output.len() as u64,
        output_mode: HOST_LINK_OUTPUT_MODE_V4,
        durable_plan: plan_identity,
    };
    binding.claim_digest = sha256_parts(&[
        CLAIM_DOMAIN,
        &binding.transcript_binding_identity,
        &binding.client_pid.to_le_bytes(),
        &binding.client_start_time_ticks.to_le_bytes(),
        &binding.transcript_request_identity,
        &binding.transcript_plan_identity,
        &binding.transcript_closure_identity,
    ]);
    binding.transcript_digest = sha256_parts(&[
        TRANSCRIPT_DOMAIN,
        &binding.transcript_binding_identity,
        &binding.client_pid.to_le_bytes(),
        &binding.client_start_time_ticks.to_le_bytes(),
        &binding.transcript_request_identity,
        &binding.transcript_plan_identity,
        &binding.transcript_closure_identity,
        &binding.transcript_grant_identity,
        &binding.output_digest,
        &binding.output_length.to_le_bytes(),
        &binding.output_mode.to_le_bytes(),
        &binding.durable_plan,
    ]);
    binding.reservation_digest = sha256_parts(&[
        RESERVATION_DOMAIN,
        &binding.session_id,
        &binding.session_nonce,
        &binding.claim_digest,
        &binding.client_pid.to_le_bytes(),
        &binding.client_start_time_ticks.to_le_bytes(),
        &binding.transcript_plan_identity,
        &binding.transcript_closure_identity,
        &binding.durable_plan,
    ]);
    binding
}

fn signed_observation(
    challenge: &AnchorChallengeV1,
    position: AnchorPositionV1,
    signing: &SigningKey,
) -> [u8; ANCHOR_OBSERVATION_WIRE_LEN_V1] {
    let unsigned = UnsignedAnchorObservationV1::from_challenge(challenge, position);
    let signature = signing.sign(&unsigned.signing_bytes()).to_bytes();
    unsigned.attach_signature(signature)
}

fn persist_prepared(
    store: &RetainedDurableDirectoryV1,
    names: &DurableNames,
    prepared: &DurableRecordV1,
    output: &[u8],
) {
    store
        .stage_artifact(
            &names.staged,
            output,
            MAX_BROKER_DURABLE_OUTPUT_BYTES_V1,
            &mut NoRetainedDurableDirectoryHooksV1,
        )
        .unwrap();
    let mut faults = FaultInjector::new(None, BrokerDurableRecordStageV1::Prepared);
    commit_record(store, names, prepared, &mut faults).unwrap();
}

#[test]
fn plan_is_one_canonical_non_authority_component() {
    let key = test_anchor_key();
    let plan = DurableBrokerPublicationPlanV1::new("output-v1.bin", &key).unwrap();
    assert_eq!(plan.destination(), "output-v1.bin");
    assert_ne!(plan.identity_bytes(), [0; 32]);
    assert_eq!(plan.anchor_key_identity(), key.identity().to_bytes());
    assert_eq!(
        plan.broker_identity().unwrap().as_bytes(),
        plan.identity_bytes()
    );
    let other_key = PinnedAnchorKeyV1::from_bytes(
        SigningKey::from_bytes(&[0x6b; 32])
            .verifying_key()
            .to_bytes(),
    )
    .unwrap();
    assert_ne!(
        plan.identity_bytes(),
        DurableBrokerPublicationPlanV1::new("output-v1.bin", &other_key)
            .unwrap()
            .identity_bytes()
    );
    assert_eq!(BROKER_DURABLE_SESSION_AUTHORITY_V1, "none");
    for rejected in [
        "",
        ".",
        "..",
        "../escape",
        "/absolute",
        "nested/output",
        ".fe2o3-owned",
        "has space",
    ] {
        assert!(
            DurableBrokerPublicationPlanV1::new(rejected, &key).is_err(),
            "{rejected}"
        );
    }
}

#[test]
fn record_round_trip_and_signed_positions_are_exact() {
    let fixture = Fixture::new();
    let bytes = fixture.prepared.encode().unwrap();
    assert!(bytes.len() <= MAX_BROKER_DURABLE_RECORD_BYTES_V1);
    let decoded = DurableRecordV1::decode(&bytes).unwrap();
    assert_eq!(decoded, fixture.prepared);
    assert!(matches!(
        verify_record_observation(&decoded, &fixture.commit_observation).unwrap(),
        AnchorDecisionV1::Commit(_)
    ));
    assert!(matches!(
        verify_record_observation(&decoded, &fixture.abort_observation).unwrap(),
        AnchorDecisionV1::Abort(_)
    ));
    for index in 0..ANCHOR_OBSERVATION_WIRE_LEN_V1 {
        let mut mutated = fixture.commit_observation;
        mutated[index] ^= 1;
        assert!(
            verify_record_observation(&decoded, &mutated).is_err(),
            "byte {index}"
        );
    }
}

#[test]
fn every_canonical_binding_field_is_checked_beyond_the_checksum() {
    let fixture = Fixture::new();
    let mut variants = Vec::new();
    macro_rules! variant {
        ($field:ident) => {{
            let mut record = fixture.prepared.clone();
            record.binding.$field[0] ^= 1;
            variants.push(record);
        }};
    }
    variant!(session_id);
    variant!(session_nonce);
    variant!(reservation_digest);
    variant!(request_nonce_sha256);
    variant!(claim_digest);
    variant!(transcript_digest);
    variant!(transcript_binding_identity);
    variant!(transcript_request_identity);
    variant!(transcript_plan_identity);
    variant!(transcript_closure_identity);
    variant!(transcript_grant_identity);
    variant!(output_digest);
    variant!(durable_plan);
    let mut pid = fixture.prepared.clone();
    pid.binding.client_pid ^= 1;
    variants.push(pid);
    let mut start = fixture.prepared.clone();
    start.binding.client_start_time_ticks ^= 1;
    variants.push(start);
    let mut length = fixture.prepared.clone();
    length.binding.output_length ^= 1;
    variants.push(length);
    let mut mode = fixture.prepared.clone();
    mode.binding.output_mode ^= 1;
    variants.push(mode);
    let mut key = fixture.prepared.clone();
    key.anchor_key_bytes[0] ^= 1;
    variants.push(key);
    let mut challenge = fixture.prepared.clone();
    challenge.challenge[0] ^= 1;
    variants.push(challenge);
    let mut destination = fixture.prepared.clone();
    destination.destination = "substitute.bin".into();
    variants.push(destination);
    for variant in variants {
        let rechecksummed = DurableRecordV1::decode(&variant.encode().unwrap()).unwrap();
        assert!(validate_record(&rechecksummed, &fixture.plan).is_err());
    }
}

#[test]
fn prepared_commit_and_abort_recover_deterministically() {
    let commit = Fixture::new();
    commit.persist_prepared();
    assert_eq!(
        recover_durable_broker_session_v1(commit.root(), commit.plan.clone(), None).unwrap(),
        BrokerDurableRecoveryV1::Prepared
    );
    assert_eq!(
        recover_durable_broker_session_v1(
            commit.root(),
            commit.plan.clone(),
            Some(&commit.commit_observation),
        )
        .unwrap(),
        BrokerDurableRecoveryV1::Published
    );
    let published = commit.directory.path().join(commit.plan.destination());
    assert_eq!(fs::read(&published).unwrap(), commit.output);
    assert_eq!(
        fs::metadata(&published).unwrap().permissions().mode() & 0o777,
        HOST_LINK_OUTPUT_MODE_V4
    );
    assert_eq!(
        recover_durable_broker_session_v1(commit.root(), commit.plan.clone(), None).unwrap(),
        BrokerDurableRecoveryV1::Published
    );

    let abort = Fixture::new();
    abort.persist_prepared();
    assert_eq!(
        recover_durable_broker_session_v1(
            abort.root(),
            abort.plan.clone(),
            Some(&abort.abort_observation),
        )
        .unwrap(),
        BrokerDurableRecoveryV1::Aborted
    );
    assert!(
        !abort
            .directory
            .path()
            .join(abort.plan.destination())
            .exists()
    );
}

#[test]
fn inspection_distinguishes_every_durable_state_without_advancing_it() {
    let fixture = Fixture::new();
    fixture.persist_prepared();
    assert_eq!(
        inspect_durable_broker_session_v1(fixture.root(), fixture.plan.clone()).unwrap(),
        BrokerDurableRecoveryV1::Prepared
    );
    let committed = fixture
        .prepared
        .successor(
            RecordStateV1::AnchorCommitted,
            Some(fixture.commit_observation),
        )
        .unwrap();
    let mut faults = FaultInjector::new(None, BrokerDurableRecordStageV1::AnchorCommitted);
    commit_record(&fixture.store(), &fixture.names, &committed, &mut faults).unwrap();
    assert_eq!(
        inspect_durable_broker_session_v1(fixture.root(), fixture.plan.clone()).unwrap(),
        BrokerDurableRecoveryV1::AnchorCommitted
    );
    recover_durable_broker_session_v1(fixture.root(), fixture.plan.clone(), None).unwrap();
    assert_eq!(
        inspect_durable_broker_session_v1(fixture.root(), fixture.plan.clone()).unwrap(),
        BrokerDurableRecoveryV1::Published
    );

    let aborted = Fixture::new();
    aborted.persist_prepared();
    recover_durable_broker_session_v1(
        aborted.root(),
        aborted.plan.clone(),
        Some(&aborted.abort_observation),
    )
    .unwrap();
    assert_eq!(
        inspect_durable_broker_session_v1(aborted.root(), aborted.plan.clone()).unwrap(),
        BrokerDurableRecoveryV1::Aborted
    );

    let invalid = Fixture::new();
    invalid.persist_prepared();
    let invalid_record = invalid
        .prepared
        .successor(RecordStateV1::Invalid, None)
        .unwrap();
    let mut faults = FaultInjector::new(None, BrokerDurableRecordStageV1::Invalid);
    commit_record(
        &invalid.store(),
        &invalid.names,
        &invalid_record,
        &mut faults,
    )
    .unwrap();
    assert_eq!(
        inspect_durable_broker_session_v1(invalid.root(), invalid.plan.clone()).unwrap(),
        BrokerDurableRecoveryV1::Invalid
    );
}

#[test]
fn every_record_fault_boundary_recovers_or_fails_closed() {
    for stage in [
        BrokerDurableRecordStageV1::Prepared,
        BrokerDurableRecordStageV1::Invalid,
    ] {
        for boundary in RECORD_BOUNDARIES {
            for timing in TIMINGS {
                let fixture = Fixture::new();
                let store = fixture.store();
                store
                    .stage_artifact(
                        &fixture.names.staged,
                        &fixture.output,
                        MAX_BROKER_DURABLE_OUTPUT_BYTES_V1,
                        &mut NoRetainedDurableDirectoryHooksV1,
                    )
                    .unwrap();
                let record = if stage == BrokerDurableRecordStageV1::Prepared {
                    fixture.prepared.clone()
                } else {
                    fixture
                        .prepared
                        .successor(RecordStateV1::Invalid, None)
                        .unwrap()
                };
                if stage == BrokerDurableRecordStageV1::Invalid {
                    let mut no_fault =
                        FaultInjector::new(None, BrokerDurableRecordStageV1::Prepared);
                    commit_record(&store, &fixture.names, &fixture.prepared, &mut no_fault)
                        .unwrap();
                }
                let point = BrokerDurableFaultPointV1::Record {
                    stage,
                    boundary,
                    timing,
                };
                let mut faults = FaultInjector::new(Some(point), stage);
                assert!(matches!(
                    commit_record(&store, &fixture.names, &record, &mut faults),
                    Err(BrokerDurableSessionErrorV1::InjectedCrash { .. })
                ));
                let recovered =
                    recover_durable_broker_session_v1(fixture.root(), fixture.plan.clone(), None);
                assert!(
                    matches!(
                        recovered,
                        Ok(BrokerDurableRecoveryV1::Prepared)
                            | Ok(BrokerDurableRecoveryV1::Invalid)
                            | Err(_)
                    ),
                    "{stage:?} {boundary:?} {timing:?}: {recovered:?}"
                );
                assert!(
                    !fixture
                        .directory
                        .path()
                        .join(fixture.plan.destination())
                        .exists()
                );
            }
        }
    }

    for stage in [
        BrokerDurableRecordStageV1::AnchorCommitted,
        BrokerDurableRecordStageV1::Published,
        BrokerDurableRecordStageV1::Aborted,
    ] {
        for boundary in RECORD_BOUNDARIES {
            for timing in TIMINGS {
                let fixture = Fixture::new();
                fixture.persist_prepared();
                let observation = if stage == BrokerDurableRecordStageV1::Aborted {
                    &fixture.abort_observation
                } else {
                    &fixture.commit_observation
                };
                let result = recover_durable_broker_session_v1_with_options(
                    fixture.root(),
                    fixture.plan.clone(),
                    Some(observation),
                    BrokerDurableOptionsV1::inject_crash(BrokerDurableFaultPointV1::Record {
                        stage,
                        boundary,
                        timing,
                    }),
                );
                assert!(
                    matches!(
                        result,
                        Err(BrokerDurableSessionErrorV1::InjectedCrash { .. })
                    ),
                    "{stage:?} {boundary:?} {timing:?}: {result:?}"
                );
                let recovered = recover_durable_broker_session_v1(
                    fixture.root(),
                    fixture.plan.clone(),
                    Some(observation),
                )
                .unwrap();
                let expected = if stage == BrokerDurableRecordStageV1::Aborted {
                    BrokerDurableRecoveryV1::Aborted
                } else {
                    BrokerDurableRecoveryV1::Published
                };
                assert_eq!(recovered, expected, "{stage:?} {boundary:?} {timing:?}");
            }
        }
    }
}

#[test]
fn redo_replay_rename_and_fsync_boundaries_are_fault_injected() {
    for (boundary, timing) in [
        (
            RetainedDurableRecordBoundaryV1::RenameRedoToCanonical,
            RetainedDurableFaultTimingV1::Before,
        ),
        (
            RetainedDurableRecordBoundaryV1::RenameRedoToCanonical,
            RetainedDurableFaultTimingV1::After,
        ),
        (
            RetainedDurableRecordBoundaryV1::SyncCanonicalName,
            RetainedDurableFaultTimingV1::Before,
        ),
        (
            RetainedDurableRecordBoundaryV1::SyncCanonicalName,
            RetainedDurableFaultTimingV1::After,
        ),
    ] {
        let fixture = Fixture::new();
        fixture.persist_prepared();
        let first = recover_durable_broker_session_v1_with_options(
            fixture.root(),
            fixture.plan.clone(),
            Some(&fixture.commit_observation),
            BrokerDurableOptionsV1::inject_crash(BrokerDurableFaultPointV1::Record {
                stage: BrokerDurableRecordStageV1::AnchorCommitted,
                boundary: RetainedDurableRecordBoundaryV1::RenameTempToRedo,
                timing: RetainedDurableFaultTimingV1::After,
            }),
        );
        assert!(matches!(
            first,
            Err(BrokerDurableSessionErrorV1::InjectedCrash { .. })
        ));
        let replay = recover_durable_broker_session_v1_with_options(
            fixture.root(),
            fixture.plan.clone(),
            Some(&fixture.commit_observation),
            BrokerDurableOptionsV1::inject_crash(BrokerDurableFaultPointV1::Record {
                stage: BrokerDurableRecordStageV1::AnchorCommitted,
                boundary,
                timing,
            }),
        );
        assert!(
            matches!(
                replay,
                Err(BrokerDurableSessionErrorV1::InjectedCrash { .. })
            ),
            "{boundary:?} {timing:?}: {replay:?}"
        );
        assert_eq!(
            recover_durable_broker_session_v1(
                fixture.root(),
                fixture.plan.clone(),
                Some(&fixture.commit_observation),
            )
            .unwrap(),
            BrokerDurableRecoveryV1::Published
        );
    }
}

#[test]
fn every_artifact_fault_boundary_is_exercised() {
    for boundary in STAGE_BOUNDARIES {
        for timing in TIMINGS {
            let fixture = Fixture::new();
            let store = fixture.store();
            let point = BrokerDurableFaultPointV1::Artifact { boundary, timing };
            let mut faults = FaultInjector::new(Some(point), BrokerDurableRecordStageV1::Prepared);
            let result = store.stage_artifact(
                &fixture.names.staged,
                &fixture.output,
                MAX_BROKER_DURABLE_OUTPUT_BYTES_V1,
                &mut faults,
            );
            assert!(result.is_err(), "{boundary:?} {timing:?}");
            assert!(
                !fixture
                    .directory
                    .path()
                    .join(fixture.plan.destination())
                    .exists()
            );
        }
    }
    for boundary in PUBLISH_BOUNDARIES {
        for timing in TIMINGS {
            let fixture = Fixture::new();
            fixture.persist_prepared();
            let result = recover_durable_broker_session_v1_with_options(
                fixture.root(),
                fixture.plan.clone(),
                Some(&fixture.commit_observation),
                BrokerDurableOptionsV1::inject_crash(BrokerDurableFaultPointV1::Artifact {
                    boundary,
                    timing,
                }),
            );
            assert!(matches!(
                result,
                Err(BrokerDurableSessionErrorV1::InjectedCrash { .. })
            ));
            assert_eq!(
                recover_durable_broker_session_v1(
                    fixture.root(),
                    fixture.plan.clone(),
                    Some(&fixture.commit_observation),
                )
                .unwrap(),
                BrokerDurableRecoveryV1::Published
            );
        }
    }

    let fixture = Fixture::new();
    fixture.persist_prepared();
    let renamed = recover_durable_broker_session_v1_with_options(
        fixture.root(),
        fixture.plan.clone(),
        Some(&fixture.commit_observation),
        BrokerDurableOptionsV1::inject_crash(BrokerDurableFaultPointV1::Artifact {
            boundary: RetainedDurableArtifactBoundaryV1::RenameStagedToFinal,
            timing: RetainedDurableFaultTimingV1::After,
        }),
    );
    assert!(matches!(
        renamed,
        Err(BrokerDurableSessionErrorV1::InjectedCrash { .. })
    ));
    let final_sync = recover_durable_broker_session_v1_with_options(
        fixture.root(),
        fixture.plan.clone(),
        Some(&fixture.commit_observation),
        BrokerDurableOptionsV1::inject_crash(BrokerDurableFaultPointV1::Artifact {
            boundary: RetainedDurableArtifactBoundaryV1::SyncFinalName,
            timing: RetainedDurableFaultTimingV1::Before,
        }),
    );
    assert!(matches!(
        final_sync,
        Err(BrokerDurableSessionErrorV1::InjectedCrash { .. })
    ));
    assert_eq!(
        recover_durable_broker_session_v1(
            fixture.root(),
            fixture.plan.clone(),
            Some(&fixture.commit_observation),
        )
        .unwrap(),
        BrokerDurableRecoveryV1::Published
    );
}

#[test]
fn conflicting_writer_redo_is_not_an_advisory_lock_or_accepted_successor() {
    let fixture = Fixture::new();
    fixture.persist_prepared();
    let mut conflicting = fixture
        .prepared
        .successor(
            RecordStateV1::AnchorCommitted,
            Some(fixture.commit_observation),
        )
        .unwrap();
    conflicting.binding.transcript_grant_identity[0] ^= 1;
    fs::write(
        fixture.directory.path().join(&fixture.names.redo),
        conflicting.encode().unwrap(),
    )
    .unwrap();
    assert!(
        recover_durable_broker_session_v1(
            fixture.root(),
            fixture.plan.clone(),
            Some(&fixture.commit_observation),
        )
        .is_err()
    );
    assert_eq!(
        DurableRecordV1::decode(
            &fs::read(fixture.directory.path().join(&fixture.names.record)).unwrap()
        )
        .unwrap()
        .state,
        RecordStateV1::Prepared
    );
    assert!(
        !fixture
            .directory
            .path()
            .join(fixture.plan.destination())
            .exists()
    );
}

#[test]
fn partial_replaced_stale_symlink_and_hardlink_state_fail_closed() {
    let partial = Fixture::new();
    partial.persist_prepared();
    let record_path = partial.directory.path().join(&partial.names.record);
    let bytes = fs::read(&record_path).unwrap();
    fs::write(&record_path, &bytes[..bytes.len() / 2]).unwrap();
    assert!(recover_durable_broker_session_v1(partial.root(), partial.plan.clone(), None).is_err());

    let partial_redo = Fixture::new();
    partial_redo.persist_prepared();
    fs::write(
        partial_redo.directory.path().join(&partial_redo.names.redo),
        b"partial-redo",
    )
    .unwrap();
    assert!(
        recover_durable_broker_session_v1(partial_redo.root(), partial_redo.plan.clone(), None,)
            .is_err()
    );

    let stale = Fixture::new();
    stale.persist_prepared();
    fs::write(stale.directory.path().join(&stale.names.staged), b"stale").unwrap();
    assert!(recover_durable_broker_session_v1(stale.root(), stale.plan.clone(), None).is_err());

    let hardlink = Fixture::new();
    hardlink.persist_prepared();
    fs::hard_link(
        hardlink.directory.path().join(&hardlink.names.staged),
        hardlink.directory.path().join("hostile-hardlink"),
    )
    .unwrap();
    assert!(
        recover_durable_broker_session_v1(hardlink.root(), hardlink.plan.clone(), None).is_err()
    );

    let record_hardlink = Fixture::new();
    record_hardlink.persist_prepared();
    fs::hard_link(
        record_hardlink
            .directory
            .path()
            .join(&record_hardlink.names.record),
        record_hardlink.directory.path().join("hostile-record-link"),
    )
    .unwrap();
    assert!(
        recover_durable_broker_session_v1(
            record_hardlink.root(),
            record_hardlink.plan.clone(),
            None,
        )
        .is_err()
    );

    let symlinked = Fixture::new();
    symlinked.persist_prepared();
    let staged = symlinked.directory.path().join(&symlinked.names.staged);
    fs::remove_file(&staged).unwrap();
    symlink("/dev/null", &staged).unwrap();
    assert!(
        recover_durable_broker_session_v1(symlinked.root(), symlinked.plan.clone(), None).is_err()
    );

    let record_symlink = Fixture::new();
    record_symlink.persist_prepared();
    let record = record_symlink
        .directory
        .path()
        .join(&record_symlink.names.record);
    fs::remove_file(&record).unwrap();
    symlink("/dev/null", &record).unwrap();
    assert!(
        recover_durable_broker_session_v1(
            record_symlink.root(),
            record_symlink.plan.clone(),
            None,
        )
        .is_err()
    );

    let rollback = Fixture::new();
    rollback.persist_prepared();
    let prepared_bytes = fs::read(rollback.directory.path().join(&rollback.names.record)).unwrap();
    recover_durable_broker_session_v1(
        rollback.root(),
        rollback.plan.clone(),
        Some(&rollback.commit_observation),
    )
    .unwrap();
    fs::write(
        rollback.directory.path().join(&rollback.names.record),
        prepared_bytes,
    )
    .unwrap();
    assert!(
        recover_durable_broker_session_v1(rollback.root(), rollback.plan.clone(), None).is_err()
    );

    let prepared_with_public = Fixture::new();
    prepared_with_public.persist_prepared();
    fs::write(
        prepared_with_public
            .directory
            .path()
            .join(prepared_with_public.plan.destination()),
        &prepared_with_public.output,
    )
    .unwrap();
    fs::set_permissions(
        prepared_with_public
            .directory
            .path()
            .join(prepared_with_public.plan.destination()),
        fs::Permissions::from_mode(HOST_LINK_OUTPUT_MODE_V4),
    )
    .unwrap();
    assert!(
        recover_prepared_durable_broker_session_v1(
            prepared_with_public.root(),
            prepared_with_public.plan.clone(),
        )
        .is_err()
    );

    let invalid_with_public = Fixture::new();
    invalid_with_public.persist_prepared();
    let invalid = invalid_with_public
        .prepared
        .successor(RecordStateV1::Invalid, None)
        .unwrap();
    let mut faults = FaultInjector::new(None, BrokerDurableRecordStageV1::Invalid);
    commit_record(
        &invalid_with_public.store(),
        &invalid_with_public.names,
        &invalid,
        &mut faults,
    )
    .unwrap();
    fs::write(
        invalid_with_public
            .directory
            .path()
            .join(invalid_with_public.plan.destination()),
        &invalid_with_public.output,
    )
    .unwrap();
    fs::set_permissions(
        invalid_with_public
            .directory
            .path()
            .join(invalid_with_public.plan.destination()),
        fs::Permissions::from_mode(HOST_LINK_OUTPUT_MODE_V4),
    )
    .unwrap();
    assert!(
        inspect_durable_broker_session_v1(
            invalid_with_public.root(),
            invalid_with_public.plan.clone(),
        )
        .is_err()
    );
}

#[test]
fn duplicate_and_mismatched_plan_or_observation_do_not_publish() {
    let fixture = Fixture::new();
    fixture.persist_prepared();
    assert!(
        fixture
            .store()
            .read_private(&fixture.names.record, MAX_BROKER_DURABLE_RECORD_BYTES_V1)
            .unwrap()
            .is_some()
    );
    let other_plan = DurableBrokerPublicationPlanV1::new("other.bin", &test_anchor_key()).unwrap();
    assert!(recover_durable_broker_session_v1(fixture.root(), other_plan, None).is_err());
    let mut wrong_observation = fixture.commit_observation;
    wrong_observation[40] ^= 1;
    assert!(
        recover_durable_broker_session_v1(
            fixture.root(),
            fixture.plan.clone(),
            Some(&wrong_observation),
        )
        .is_err()
    );
    assert!(
        !fixture
            .directory
            .path()
            .join(fixture.plan.destination())
            .exists()
    );
}

#[test]
fn sigkill_restart_recovers_from_the_retained_root() {
    const CHILD_ENV: &str = "FE2O3_DURABLE_SIGKILL_CHILD_V1";
    const READY_ENV: &str = "FE2O3_DURABLE_SIGKILL_READY_V1";
    const SEED: u8 = 0x69;
    if std::env::var_os(CHILD_ENV).is_some() {
        let ready = PathBuf::from(std::env::var_os(READY_ENV).unwrap());
        let observation_path = ready.with_extension("observation");
        let fixture = real_prepared_fixture(SEED);
        let transaction =
            prepare_durable_broker_session_v1(fixture.prepared_session, fixture.plan.clone())
                .unwrap();
        let observation =
            sign_transaction_challenge(&transaction, AnchorPositionV1::Proposed, &fixture.signing);
        fs::write(
            &ready,
            fixture.directory.path().as_os_str().as_encoded_bytes(),
        )
        .unwrap();
        fs::write(&observation_path, observation).unwrap();
        thread::sleep(Duration::from_secs(30));
        std::hint::black_box(transaction);
        std::hint::black_box(fixture._client_peer);
        return;
    }

    let rendezvous = tempfile::tempdir().unwrap();
    let ready = rendezvous.path().join("prepared-root");
    let observation_path = ready.with_extension("observation");
    let test_name =
        "durable_session_consume::tests::sigkill_restart_recovers_from_the_retained_root";
    let mut command = Command::new(std::env::current_exe().unwrap());
    command
        .arg("--exact")
        .arg(test_name)
        .arg("--nocapture")
        .env(CHILD_ENV, "1")
        .env(READY_ENV, &ready)
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    let mut child = crate::test_process_execution::spawn(&mut command).unwrap();
    let deadline = Instant::now() + Duration::from_secs(10);
    while (!ready.exists() || !observation_path.exists()) && Instant::now() < deadline {
        thread::sleep(Duration::from_millis(10));
    }
    assert!(ready.exists(), "child did not commit prepared state");
    child.kill().unwrap();
    let status = child.wait().unwrap();
    assert!(!status.success());
    let service_root = PathBuf::from(String::from_utf8(fs::read(&ready).unwrap()).unwrap());
    let signing = SigningKey::from_bytes(&[SEED; 32]);
    let key = PinnedAnchorKeyV1::from_bytes(signing.verifying_key().to_bytes()).unwrap();
    let plan =
        DurableBrokerPublicationPlanV1::new(format!("real-host-output-{SEED:02x}.bin"), &key)
            .unwrap();
    let recovered =
        recover_prepared_durable_broker_session_v1(root(&service_root), plan.clone()).unwrap();
    let observation = fs::read(&observation_path).unwrap();
    assert_eq!(
        recovered.observe_and_recover(&observation).unwrap(),
        BrokerDurableRecoveryV1::Published
    );
    assert!(service_root.join(plan.destination()).is_file());
    fs::remove_dir_all(service_root).unwrap();
}

//! Fixed-entry qualification only: no deployed approval, application lease or GPU authority.

#[path = "proof_controller_protocol.rs"]
#[allow(dead_code)]
mod protocol;

use fe2o3_host::{
    RetainedWorkerV3ConditionalFillProofV1, check_worker_v3_compiler_closure_v1,
    execute_retained_worker_v3_conditional_fill_v1,
};
use fe2o3_kernel_analysis::{
    AuthenticatedPhysicalMachineEffectLimitsV1, AuthenticatedPhysicalMachineEffectWorkerV1,
};
use fe2o3_kernel_descriptor::KernelId;
use fe2o3_protected_service_profile::{
    ProofControllerCredentialProfileV1, ProofControllerProcessProfileV1,
    protected_service_secure_start_address_v1,
};
use fe2o3_verifier::FunctionalRefinementVerusRuntimeLeaseV1;
use protocol::{Config, Evidence, Message, Result, require};
use std::{
    fs::File,
    os::{
        fd::{AsFd, FromRawFd, OwnedFd},
        unix::fs::MetadataExt,
    },
    time::{Duration, Instant},
};

fn main() {
    std::hint::black_box(protected_service_secure_start_address_v1());
    if run().is_err() {
        std::process::exit(98);
    }
}

fn run() -> Result<()> {
    require(
        std::env::args_os().collect::<Vec<_>>()
            == [std::ffi::OsString::from("fe2o3-protected-service")]
            && std::env::vars_os().next().is_none(),
        "fixed argv and empty environment required",
    )?;
    let profile = ProofControllerProcessProfileV1::capture(
        ProofControllerCredentialProfileV1::new(protocol::UID, protocol::GID)?,
    )?;
    std::env::set_current_dir("/")?;
    // SAFETY: this feature-gated fixed executable claims each installed descriptor exactly once.
    let (mut envelope, mut payload, control, mut config) = unsafe {
        (
            File::from_raw_fd(3),
            File::from_raw_fd(4),
            OwnedFd::from_raw_fd(5),
            File::from_raw_fd(6),
        )
    };
    for fd in [
        envelope.as_fd(),
        payload.as_fd(),
        control.as_fd(),
        config.as_fd(),
    ] {
        rustix::io::fcntl_setfd(fd, rustix::io::FdFlags::CLOEXEC)?;
    }
    let identities = [&envelope, &payload, &config]
        .map(|file| file.metadata().map(|m| (m.dev(), m.ino())))
        .into_iter()
        .collect::<std::io::Result<Vec<_>>>()?;
    require(
        identities[0] != identities[1]
            && identities[0] != identities[2]
            && identities[1] != identities[2],
        "aliased fixture inputs",
    )?;
    let config: Config =
        serde_json::from_slice(&protocol::read_sealed(&mut config, protocol::MAX_CONFIG)?)?;
    config.policy()?;
    require(
        rustix::process::getppid().map(|pid| pid.as_raw_pid()) == Some(config.parent_pid),
        "fixture parent changed",
    )?;
    protocol::admit_control(control.as_fd())?;
    let peer = rustix::net::sockopt::socket_peercred(&control)?;
    require(
        (peer.pid.as_raw_pid(), peer.uid.as_raw(), peer.gid.as_raw()) == (config.parent_pid, 0, 0),
        "fixture control peer mismatch",
    )?;
    let deadline = Instant::now() + Duration::from_secs(300);
    let result = execute(
        &profile,
        &mut envelope,
        &mut payload,
        &control,
        &config,
        deadline,
    );
    if let Err(error) = &result {
        let detail: String = error.to_string().chars().take(2048).collect();
        let _ = protocol::send(
            control.as_fd(),
            config.nonce,
            Message::Rejected {
                stage: "execution".into(),
                detail,
            },
            deadline,
        );
        require(
            matches!(
                protocol::receive(
                    control.as_fd(),
                    (config.parent_pid, 0, 0),
                    config.nonce,
                    deadline,
                )?,
                Message::Release
            ),
            "expected Release after rejection",
        )?;
    }
    result
}

fn execute(
    profile: &ProofControllerProcessProfileV1,
    envelope_file: &mut File,
    payload_file: &mut File,
    control: &OwnedFd,
    config: &Config,
    deadline: Instant,
) -> Result<()> {
    let envelope = protocol::read_sealed(
        envelope_file,
        fe2o3_runtime_protocol::MAX_WORKER_V3_LOAD_ENVELOPE_BYTES_V2,
    )?;
    let payload = protocol::read_sealed(
        payload_file,
        fe2o3_kernel_analysis::MAX_PHYSICAL_MACHINE_EFFECT_PAYLOAD_BYTES_V1,
    )?;
    require(
        envelope.len() as u64 == config.envelope_len
            && protocol::digest(&envelope) == config.envelope_hash
            && payload.len() as u64 == config.payload_len
            && protocol::digest(&payload) == config.payload_hash,
        "sealed input/config mismatch",
    )?;
    let envelope_pointer = envelope.as_ptr();
    let payload_pointer = payload.as_ptr();
    let mut death_signal = 0;
    // SAFETY: fixed prctl readback writes one live scalar; no credential mutation.
    require(
        unsafe { libc::prctl(libc::PR_GET_PDEATHSIG, &raw mut death_signal, 0, 0, 0) } == 0
            && death_signal == libc::SIGKILL,
        "missing parent-death signal",
    )?;
    let limits = AuthenticatedPhysicalMachineEffectLimitsV1::new(
        Duration::from_secs(60),
        1024 * 1024,
        16384,
    )?;
    let worker = AuthenticatedPhysicalMachineEffectWorkerV1::open(
        protocol::WORKER_PATH,
        config.policy()?,
        limits,
    )?;
    let runtime = FunctionalRefinementVerusRuntimeLeaseV1::open(protocol::RUNTIME_PATH)?;
    require(
        runtime.identity().as_bytes() == config.runtime,
        "wrong protected runtime identity",
    )?;
    profile.revalidate_current()?;
    runtime.revalidate()?;
    protocol::send(
        control.as_fd(),
        config.nonce,
        Message::Ready {
            runtime: runtime.identity().as_bytes(),
        },
        deadline,
    )?;
    let parent = (config.parent_pid, 0, 0);
    require(
        matches!(
            protocol::receive(control.as_fd(), parent, config.nonce, deadline)?,
            Message::Start
        ),
        "expected Start",
    )?;
    profile.revalidate_current()?;
    runtime.revalidate()?;
    protocol::send(control.as_fd(), config.nonce, Message::Executing, deadline)?;
    let proof = Box::new(execute_retained_worker_v3_conditional_fill_v1(
        envelope,
        payload,
        KernelId::from_bytes(config.kernel),
        &worker,
        &runtime,
        deadline,
    )?);
    require(
        proof.exact_canonical_envelope_bytes().as_ptr() == envelope_pointer
            && proof.finalized_hsaco_bytes().as_ptr() == payload_pointer,
        "input custody changed",
    )?;
    profile.revalidate_current()?;
    runtime.revalidate()?;
    let first = evidence(&proof)?;
    let first_hash = protocol::digest(&serde_json::to_vec(&first)?);
    let first_pointers = first.pointers;
    protocol::send(
        control.as_fd(),
        config.nonce,
        Message::Proved {
            evidence: Box::new(first),
        },
        deadline,
    )?;
    for (index, bytes) in artifacts(&proof).iter().enumerate() {
        for (chunk, data) in bytes.chunks(protocol::CHUNK_BYTES).enumerate() {
            let offset = (chunk * protocol::CHUNK_BYTES) as u64;
            require(
                matches!(protocol::receive(control.as_fd(), parent, config.nonce, deadline)?,
                Message::ArtifactRequest { index: requested, offset: position } if requested as usize == index && position == offset),
                "out-of-order artifact request",
            )?;
            protocol::send(
                control.as_fd(),
                config.nonce,
                Message::Artifact {
                    index: index as u8,
                    offset,
                    bytes: data.to_vec(),
                },
                deadline,
            )?;
        }
    }
    require(
        matches!(
            protocol::receive(control.as_fd(), parent, config.nonce, deadline)?,
            Message::Probe
        ),
        "expected Probe",
    )?;
    profile.revalidate_current()?;
    runtime.revalidate()?;
    let later = evidence(&proof)?;
    let later_hash = protocol::digest(&serde_json::to_vec(&later)?);
    require(
        later_hash == first_hash && later.pointers == first_pointers,
        "original proof owner changed",
    )?;
    protocol::send(
        control.as_fd(),
        config.nonce,
        Message::Retained {
            digest: later_hash,
            pointers: later.pointers,
        },
        deadline,
    )?;
    require(
        matches!(
            protocol::receive(control.as_fd(), parent, config.nonce, deadline)?,
            Message::Release
        ),
        "expected Release",
    )?;
    drop(proof);
    drop((worker, runtime));
    protocol::send(control.as_fd(), config.nonce, Message::Released, deadline)
}

fn evidence(proof: &RetainedWorkerV3ConditionalFillProofV1) -> Result<Evidence> {
    let refinement = proof.refinement();
    let analysis = refinement.analysis_execution();
    let closure = check_worker_v3_compiler_closure_v1(
        proof.exact_canonical_envelope_bytes(),
        proof.finalized_hsaco_bytes(),
        proof.kernel_id(),
    )?;
    require(
        closure.check_conditional_fill_refinement_v1(refinement)? == *proof.subject(),
        "retained subject mismatch",
    )?;
    require(
        analysis.request().exact_payload_bytes() == proof.finalized_hsaco_bytes(),
        "analysis payload mismatch",
    )?;
    require(
        refinement.retains_strictly_imported_signed_receipt()
            && analysis.authenticates_analyzer_execution()
            && !proof.authenticates_compiler_origin()
            && !proof.grants_currentness_authority()
            && !proof.grants_load_authority()
            && !proof.grants_launch_authority(),
        "unexpected proof boundary",
    )?;
    Ok(Evidence {
        artifacts: artifacts(proof).map(|bytes| protocol::Content {
            sha256: protocol::digest(bytes),
            len: bytes.len() as u64,
        }),
        proof_key: *refinement.receipt_verifying_key(),
        boundary: refinement.boundary() as u8,
        signed_and_imported: refinement.retains_strictly_imported_signed_receipt(),
        analyzer_authenticated: analysis.authenticates_analyzer_execution(),
        authority: [
            proof.authenticates_compiler_origin(),
            proof.grants_currentness_authority(),
            proof.grants_load_authority(),
            proof.grants_launch_authority(),
        ],
        pointers: [
            proof as *const _ as u64,
            refinement as *const _ as u64,
            proof.exact_canonical_envelope_bytes().as_ptr() as u64,
            proof.finalized_hsaco_bytes().as_ptr() as u64,
            refinement.generated_source().as_ptr() as u64,
            refinement.obligation_preimage().as_ptr() as u64,
            refinement.signed_receipt_wire().as_ptr() as u64,
            analysis.canonical_receipt_bytes().as_ptr() as u64,
        ],
    })
}

fn artifacts(proof: &RetainedWorkerV3ConditionalFillProofV1) -> [&[u8]; 7] {
    let refinement = proof.refinement();
    let analysis = refinement.analysis_execution();
    [
        proof.subject().canonical_bytes(),
        refinement.generated_source(),
        refinement.obligation_preimage(),
        refinement.signed_receipt_wire(),
        analysis.canonical_receipt_bytes(),
        analysis.request().canonical_bytes(),
        analysis.analysis().canonical_bytes(),
    ]
}

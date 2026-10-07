use crate::{
    deployment::{DEPLOYMENT_BYTES, ProofCustodianDeploymentV1, RUNTIME_PATH, WORKER_PATH},
    other, require, wire,
};
use fe2o3_host::{
    RetainedWorkerV3ConditionalFillProofV1, check_worker_v3_compiler_closure_v1,
    execute_retained_worker_v3_conditional_fill_v1,
};
use fe2o3_kernel_analysis::{
    AuthenticatedPhysicalMachineEffectLimitsV1, AuthenticatedPhysicalMachineEffectWorkerV1,
};
use fe2o3_kernel_descriptor::KernelId;
use fe2o3_protected_service_profile::ProofControllerProcessProfileV1;
use fe2o3_verifier::FunctionalRefinementVerusRuntimeLeaseV1;
use std::{
    fs::File,
    io,
    os::{
        fd::{AsFd, FromRawFd, OwnedFd},
        unix::fs::MetadataExt,
    },
    time::{Duration, Instant},
};

/// Runs only the fixed descriptor-only keyless controller after its secure ELF entry.
/// The five inherited slots are root-sealed deployment/request/envelope/payload
/// and a private root control endpoint, never application authority or signing keys.
///
/// # Safety
/// Invoke exactly once in the single-threaded fixed staged executable, before
/// unrelated descriptor use, handlers or descendants. Slots 3 through 7 must be
/// exclusively transferred to this entry, with no existing Rust owners or borrows.
#[doc(hidden)]
pub unsafe fn run_inherited_conditional_fill_proof_controller_v1() -> io::Result<()> {
    require(
        std::env::args_os().collect::<Vec<_>>()
            == [std::ffi::OsString::from("fe2o3-protected-service")]
            && std::env::vars_os().next().is_none(),
        "controller requires fixed argv and empty environment",
    )?;
    for fd in 3..=7 {
        // SAFETY: fcntl inspects an integer descriptor without assuming it is valid.
        require(
            unsafe { libc::fcntl(fd, libc::F_GETFD) } >= 0,
            "missing inherited controller slot",
        )?;
    }
    // SAFETY: the sole fixed entry claims each installed slot once, before any fallible use.
    let (config_file, request_file, envelope_file, payload_file, control) = unsafe {
        (
            File::from_raw_fd(3),
            File::from_raw_fd(4),
            File::from_raw_fd(5),
            File::from_raw_fd(6),
            OwnedFd::from_raw_fd(7),
        )
    };
    for fd in [
        config_file.as_fd(),
        request_file.as_fd(),
        envelope_file.as_fd(),
        payload_file.as_fd(),
        control.as_fd(),
    ] {
        rustix::io::fcntl_setfd(fd, rustix::io::FdFlags::CLOEXEC)?;
    }
    let mut identities = std::collections::BTreeSet::new();
    for file in [&config_file, &request_file, &envelope_file, &payload_file] {
        let m = file.metadata()?;
        require(
            identities.insert((m.dev(), m.ino())),
            "aliased controller input slots",
        )?;
    }
    let config =
        ProofCustodianDeploymentV1::decode(&wire::read_sealed(&config_file, DEPLOYMENT_BYTES)?)?;
    let profile = ProofControllerProcessProfileV1::capture(config.credentials()?).map_err(other)?;
    std::env::set_current_dir("/")?;
    let request = wire::Request::decode(&wire::read_sealed(&request_file, wire::REQUEST_BYTES)?)?;
    require(
        rustix::process::getppid().map(|pid| pid.as_raw_pid()) == Some(request.parent),
        "controller parent changed",
    )?;
    let mut signal = 0;
    // SAFETY: read-only prctl writes one initialized scalar.
    require(
        unsafe { libc::prctl(libc::PR_GET_PDEATHSIG, &raw mut signal, 0, 0, 0) } == 0
            && signal == libc::SIGKILL,
        "controller parent-death signal missing",
    )?;
    let control = wire::ControlEndpoint::admit(control)?;
    let peer = rustix::net::sockopt::socket_peercred(&control)?;
    require(
        (peer.pid.as_raw_pid(), peer.uid.as_raw(), peer.gid.as_raw()) == (request.parent, 0, 0),
        "controller root endpoint creator mismatch",
    )?;
    let deadline = Instant::now() + Duration::from_secs(300);
    let result = execute(
        config,
        request,
        profile,
        envelope_file,
        payload_file,
        &control,
        deadline,
    );
    if let Err(error) = &result {
        // Session nonce is still available only from its original sealed request.
        let request =
            wire::Request::decode(&wire::read_sealed(&request_file, wire::REQUEST_BYTES)?)?;
        reject_and_retain(error, &request, &control, ());
    }
    result
}

fn execute(
    config: ProofCustodianDeploymentV1,
    request: wire::Request,
    profile: ProofControllerProcessProfileV1,
    envelope_file: File,
    payload_file: File,
    control: &wire::ControlEndpoint,
    deadline: Instant,
) -> io::Result<()> {
    let envelope = wire::read_sealed(
        &envelope_file,
        fe2o3_runtime_protocol::MAX_WORKER_V3_LOAD_ENVELOPE_BYTES_V2,
    )?;
    let payload = wire::read_sealed(
        &payload_file,
        fe2o3_kernel_analysis::MAX_PHYSICAL_MACHINE_EFFECT_PAYLOAD_BYTES_V1,
    )?;
    require(
        (wire::digest(&envelope), envelope.len() as u64) == request.envelope
            && (wire::digest(&payload), payload.len() as u64) == request.payload,
        "controller input hashes differ",
    )?;
    let input_pointers = (envelope.as_ptr(), payload.as_ptr());
    let Resources {
        worker,
        runtime,
        profile,
    } = Resources::open(&config, profile)?;
    wire::send(
        control.as_fd(),
        request.nonce,
        wire::READY,
        &config.identity(),
        deadline,
    )?;
    let parent = (request.parent, 0, 0);
    require(
        wire::receive(control.as_fd(), parent, request.nonce, deadline)? == (wire::START, vec![]),
        "expected root Start",
    )?;
    profile.revalidate_current().map_err(other)?;
    runtime.revalidate().map_err(other)?;
    let proof = Box::new(
        execute_retained_worker_v3_conditional_fill_v1(
            envelope,
            payload,
            KernelId::from_bytes(request.kernel),
            &worker,
            &runtime,
            deadline,
        )
        .map_err(other)?,
    );
    let proof_pointer = &*proof as *const _;
    let retained = (|| {
        require(
            (
                proof.exact_canonical_envelope_bytes().as_ptr(),
                proof.finalized_hsaco_bytes().as_ptr(),
            ) == input_pointers,
            "original input custody changed",
        )?;
        let subject = evidence(&proof)?;
        wire::send(
            control.as_fd(),
            request.nonce,
            wire::PROVED,
            &subject,
            deadline,
        )?;
        serve_retained(
            control,
            parent,
            request.nonce,
            &subject,
            Duration::from_secs(30),
            || {
                profile.revalidate_current().map_err(other)?;
                runtime.revalidate().map_err(other)?;
                require(
                    std::ptr::eq(&*proof, proof_pointer) && evidence(&proof)? == subject,
                    "original proof custody changed",
                )
            },
        )
    })();
    if let Err(error) = retained {
        // A failed send may already have published PROVED. Never unwind original custody.
        reject_and_retain(
            &error,
            &request,
            control,
            (&proof, &runtime, &worker, &profile),
        );
    }
    drop(proof);
    drop((runtime, worker));
    wire::send(
        control.as_fd(),
        request.nonce,
        wire::RELEASED,
        &[],
        Instant::now() + Duration::from_secs(30),
    )
}

pub(crate) struct Resources {
    pub(crate) worker: AuthenticatedPhysicalMachineEffectWorkerV1,
    pub(crate) runtime: FunctionalRefinementVerusRuntimeLeaseV1,
    pub(crate) profile: ProofControllerProcessProfileV1,
}
impl Resources {
    pub(crate) fn open(
        config: &ProofCustodianDeploymentV1,
        profile: ProofControllerProcessProfileV1,
    ) -> io::Result<Self> {
        let limits = AuthenticatedPhysicalMachineEffectLimitsV1::new(
            Duration::from_secs(60),
            1024 * 1024,
            16384,
        )
        .map_err(other)?;
        let worker = AuthenticatedPhysicalMachineEffectWorkerV1::open(
            WORKER_PATH,
            config.analyzer_policy()?,
            limits,
        )
        .map_err(other)?;
        let runtime =
            FunctionalRefinementVerusRuntimeLeaseV1::open_closed_conditional_fill_v1(RUNTIME_PATH)
                .map_err(other)?;
        require(
            runtime.identity().as_bytes() == config.verus_identity(),
            "protected Verus runtime identity differs",
        )?;
        profile.revalidate_current().map_err(other)?;
        runtime.revalidate().map_err(other)?;
        Ok(Self {
            worker,
            runtime,
            profile,
        })
    }
    pub(crate) fn revalidate(&self) -> io::Result<()> {
        self.profile.revalidate_current().map_err(other)?;
        self.runtime.revalidate().map_err(other)
    }
}

fn reject_and_retain(
    error: &io::Error,
    request: &wire::Request,
    control: &wire::ControlEndpoint,
    custody: impl Sized,
) -> ! {
    let text: String = error.to_string().chars().take(512).collect();
    let _ = wire::send(
        control.as_fd(),
        request.nonce,
        wire::REJECTED,
        text.as_bytes(),
        Instant::now() + Duration::from_secs(1),
    );
    loop {
        std::hint::black_box(&custody);
        std::thread::park_timeout(Duration::from_secs(1));
    }
}

fn serve_retained(
    control: &wire::ControlEndpoint,
    parent: (i32, u32, u32),
    nonce: [u8; 32],
    subject: &[u8],
    timeout: Duration,
    mut revalidate: impl FnMut() -> io::Result<()>,
) -> io::Result<()> {
    loop {
        control.revalidate()?;
        let packet = wire::receive(control.as_fd(), parent, nonce, Instant::now() + timeout);
        let (kind, body) = match packet {
            Err(error) if error.kind() == io::ErrorKind::TimedOut => continue,
            result => result?,
        };
        control.revalidate()?;
        require(body.is_empty(), "unexpected root control body")?;
        revalidate()?;
        match kind {
            wire::PROBE => wire::send(
                control.as_fd(),
                nonce,
                wire::RETAINED,
                subject,
                Instant::now() + timeout,
            )?,
            wire::RELEASE => return Ok(()),
            _ => return Err(io::Error::other("unexpected root control transition")),
        }
    }
}

pub(crate) fn evidence(proof: &RetainedWorkerV3ConditionalFillProofV1) -> io::Result<Vec<u8>> {
    let refinement = proof.refinement();
    let closure = check_worker_v3_compiler_closure_v1(
        proof.exact_canonical_envelope_bytes(),
        proof.finalized_hsaco_bytes(),
        proof.kernel_id(),
    )
    .map_err(other)?;
    require(
        closure
            .check_conditional_fill_refinement_v1(refinement)
            .map_err(other)?
            == *proof.subject()
            && refinement.retains_strictly_imported_signed_receipt()
            && refinement
                .analysis_execution()
                .authenticates_analyzer_execution()
            && refinement
                .analysis_execution()
                .request()
                .exact_payload_bytes()
                == proof.finalized_hsaco_bytes()
            && !proof.authenticates_compiler_origin()
            && !proof.grants_currentness_authority()
            && !proof.grants_load_authority()
            && !proof.grants_launch_authority(),
        "retained proof association differs",
    )?;
    Ok(proof.subject().canonical_bytes().to_vec())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn retained_control_survives_idle_deadlines_and_revalidates_each_command() {
        let (parent, child) = wire::control_pair().unwrap();
        let parent = wire::ControlEndpoint::admit(parent).unwrap();
        let child = wire::ControlEndpoint::admit(child).unwrap();
        let sender = (
            std::process::id() as i32,
            rustix::process::getuid().as_raw(),
            rustix::process::getgid().as_raw(),
        );
        let server = std::thread::spawn(move || {
            let mut validations = 0;
            serve_retained(
                &child,
                sender,
                [9; 32],
                b"subject",
                Duration::from_millis(10),
                || {
                    validations += 1;
                    Ok(())
                },
            )
            .unwrap();
            validations
        });
        std::thread::sleep(Duration::from_millis(80));
        let deadline = Instant::now() + Duration::from_secs(2);
        wire::send(parent.as_fd(), [9; 32], wire::PROBE, &[], deadline).unwrap();
        assert_eq!(
            wire::receive(parent.as_fd(), sender, [9; 32], deadline).unwrap(),
            (wire::RETAINED, b"subject".to_vec())
        );
        wire::send(parent.as_fd(), [9; 32], wire::RELEASE, &[], deadline).unwrap();
        assert_eq!(server.join().unwrap(), 2);
    }
}

use super::{ActiveApplication, CAPSULE_BYTES, Capsule, StagedApplication, transport};
use crate::{
    deployment::{DEPLOYMENT_BYTES, ProofCustodianDeploymentV1},
    other, require, wire,
    worker::{Resources, evidence},
};
use fe2o3_host::{
    RetainedWorkerV3ConditionalFillProofV1, execute_retained_worker_v3_conditional_fill_v1,
};
use fe2o3_kernel_descriptor::KernelId;
use fe2o3_protected_service_profile::ProofControllerProcessProfileV1;
use fe2o3_runtime_protocol::{
    MAX_WORKER_V3_LOAD_ENVELOPE_BYTES_V2, WORKER_V3_APPLICATION_PROOF_MAX_PAYLOAD_BYTES_V1,
    WorkerV3ApplicationProofInputsV1 as Inputs, WorkerV3ApplicationProofKindV1 as Kind,
    WorkerV3ApplicationProofMessageV1 as Message, WorkerV3ApplicationProofSessionV1 as Session,
    WorkerV3LoadEnvelopeIdentityV1,
};
use std::{
    fs::File,
    io,
    os::{
        fd::{AsFd, FromRawFd, OwnedFd},
        unix::fs::MetadataExt,
    },
    time::{Duration, Instant},
};

const _: () = assert!(
    WORKER_V3_APPLICATION_PROOF_MAX_PAYLOAD_BYTES_V1
        == fe2o3_kernel_analysis::MAX_PHYSICAL_MACHINE_EFFECT_PAYLOAD_BYTES_V1
);

/// Fixed application-controller entry. Opens approved resources before resource Ready,
/// and exposes application receive only after an authenticated root Activate.
///
/// # Safety
/// Invoke once in the fixed single-threaded secure-entry executable. Descriptors 3..=8
/// must be exclusively transferred configuration, capsule, app pidfd, Cargo pidfd,
/// application proof peer and root control, with no preexisting Rust owners/borrows.
#[doc(hidden)]
pub unsafe fn run_inherited_application_proof_controller_v1() -> io::Result<()> {
    require(
        std::env::args_os().collect::<Vec<_>>()
            == [std::ffi::OsString::from("fe2o3-protected-service")]
            && std::env::vars_os().next().is_none(),
        "application controller entry profile",
    )?;
    for fd in 3..=8 {
        // SAFETY: descriptor inspection does not assume ownership or validity.
        require(
            unsafe { libc::fcntl(fd, libc::F_GETFD) } >= 0,
            "missing application controller slot",
        )?;
    }
    // SAFETY: the fixed entry claims the exact inherited table once before any FD allocation.
    let [config, capsule, app, cargo, peer, control] =
        unsafe { [3, 4, 5, 6, 7, 8].map(|fd| OwnedFd::from_raw_fd(fd)) };
    for fd in [&config, &capsule, &app, &cargo, &peer, &control] {
        rustix::io::fcntl_setfd(fd, rustix::io::FdFlags::CLOEXEC)?;
    }
    let config_file = File::from(config);
    let capsule_file = File::from(capsule);
    let config =
        ProofCustodianDeploymentV1::decode(&wire::read_sealed(&config_file, DEPLOYMENT_BYTES)?)?;
    let profile = ProofControllerProcessProfileV1::capture(config.credentials()?).map_err(other)?;
    std::env::set_current_dir("/")?;
    let capsule = Capsule::decode(&wire::read_sealed(&capsule_file, CAPSULE_BYTES)?)?;
    let nonce = capsule.nonce;
    let parent = (capsule.parent, 0, 0);
    require(
        rustix::process::getppid().map(|p| p.as_raw_pid()) == Some(parent.0),
        "application controller parent changed",
    )?;
    let mut signal = 0;
    // SAFETY: read-only prctl writes one initialized scalar.
    require(
        unsafe { libc::prctl(libc::PR_GET_PDEATHSIG, &raw mut signal, 0, 0, 0) } == 0
            && signal == libc::SIGKILL,
        "application controller parent-death signal missing",
    )?;
    let control = wire::ControlEndpoint::admit(control)?;
    let creator = rustix::net::sockopt::socket_peercred(&control)?;
    require(
        (
            creator.pid.as_raw_pid(),
            creator.uid.as_raw(),
            creator.gid.as_raw(),
        ) == parent,
        "application controller root endpoint creator",
    )?;
    let credentials = config.credentials()?;
    let session = Session::new(
        capsule.transcript,
        config.identity(),
        nonce,
        (std::process::id(), credentials.uid(), credentials.gid()),
    )
    .map_err(other)?;
    let staged = StagedApplication::admit(capsule, app, cargo, peer, credentials.uid())?;
    require(
        staged.peer.fingerprint() != control.fingerprint(),
        "aliased application/root endpoints",
    )?;
    let resources = Resources::open(&config, profile)?;
    let deadline = Instant::now() + Duration::from_secs(300);
    wire::send(
        control.as_fd(),
        nonce,
        wire::READY,
        session.canonical_bytes(),
        deadline,
    )?;
    if let Err(error) = staged.await_activation(&control, &session, deadline) {
        quarantine_without_proof(
            &error,
            &control,
            nonce,
            (&staged, &resources, &config_file, &capsule_file),
        );
    }
    let application = ActiveApplication(staged);
    let result = (|| {
        resources.revalidate()?;
        application.revalidate()?;
        wire::send(
            control.as_fd(),
            nonce,
            wire::ACTIVATED,
            &session.identity(),
            deadline,
        )?;
        transport::send(
            application.0.peer.as_fd(),
            &Message::new(Kind::Active, session.identity(), 1, &[]).map_err(other)?,
            deadline,
        )?;
        receive_inputs(&application, &session, deadline)
    })();
    let (inputs, envelope_file, payload_file) = match result {
        Ok(value) => value,
        Err(error) => quarantine_without_proof(
            &error,
            &control,
            nonce,
            (&application, &resources, &config_file, &capsule_file),
        ),
    };
    let result = (|| {
        let sender = application.sender();
        let envelope = wire::read_application_sealed(
            &envelope_file,
            (sender.1, sender.2),
            inputs.envelope(),
            MAX_WORKER_V3_LOAD_ENVELOPE_BYTES_V2,
        )?;
        let payload = wire::read_application_sealed(
            &payload_file,
            (sender.1, sender.2),
            inputs.payload(),
            WORKER_V3_APPLICATION_PROOF_MAX_PAYLOAD_BYTES_V1,
        )?;
        require(
            WorkerV3LoadEnvelopeIdentityV1::from_exact_bytes(&envelope).map_err(other)?
                == application.0.capsule.binding.expectation().envelope(),
            "proof envelope differs from registered original application input",
        )?;
        application.revalidate()?;
        resources.revalidate()?;
        let pointers = (envelope.as_ptr(), payload.as_ptr());
        let proof = Box::new(
            execute_retained_worker_v3_conditional_fill_v1(
                envelope,
                payload,
                KernelId::from_bytes(inputs.kernel()),
                &resources.worker,
                &resources.runtime,
                deadline,
            )
            .map_err(other)?,
        );
        Ok((proof, pointers))
    })();
    let (proof, pointers) = match result {
        Ok(value) => value,
        Err(error) => quarantine_without_proof(
            &error,
            &control,
            nonce,
            (
                &application,
                &resources,
                &envelope_file,
                &payload_file,
                &config_file,
                &capsule_file,
            ),
        ),
    };
    // All fallible post-proof work borrows the original owner; neither error nor panic unwinds it.
    let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| -> io::Result<()> {
        require(
            (
                proof.exact_canonical_envelope_bytes().as_ptr(),
                proof.finalized_hsaco_bytes().as_ptr(),
            ) == pointers,
            "application proof input custody changed",
        )?;
        let subject = evidence(&proof)?;
        application.revalidate()?;
        resources.revalidate()?;
        transport::send(
            application.0.peer.as_fd(),
            &Message::new(Kind::Proved, session.identity(), 1, &subject).map_err(other)?,
            deadline,
        )?;
        serve_retained(
            &application,
            &session,
            &control,
            parent,
            &resources,
            &proof,
            &subject,
        )
    }));
    let reason = match outcome {
        Ok(Err(error)) => error,
        Ok(Ok(())) => io::Error::other("application retained loop returned"),
        Err(_) => io::Error::other("application retained loop panicked"),
    };
    // Reporting failure cannot surrender the original proof; root may still inspect quarantine.
    let mut detail = reason.to_string();
    while detail.len() > 512 {
        detail.pop();
    }
    let _ = wire::send(
        control.as_fd(),
        nonce,
        wire::REJECTED,
        detail.as_bytes(),
        Instant::now() + Duration::from_secs(1),
    );
    loop {
        std::hint::black_box((
            &proof,
            &application,
            &resources,
            &envelope_file,
            &payload_file,
            &config_file,
            &capsule_file,
        ));
        let _ = answer_root(&control, parent, &session, &resources, &proof, true);
        std::thread::park_timeout(Duration::from_millis(50));
    }
}

fn receive_inputs(
    application: &ActiveApplication,
    session: &Session,
    deadline: Instant,
) -> io::Result<(Inputs, File, File)> {
    loop {
        require(
            Instant::now() < deadline,
            "application proof request deadline",
        )?;
        application.revalidate()?;
        if let Some((message, rights)) = transport::try_receive(
            application.0.peer.as_fd(),
            application.sender(),
            session.identity(),
        )? {
            application.revalidate()?;
            require(
                message.kind() == Kind::Request && message.sequence() == 1,
                "expected one application proof request",
            )?;
            let inputs = Inputs::decode(message.body()).map_err(other)?;
            let [envelope, payload]: [OwnedFd; 2] = rights
                .try_into()
                .map_err(|_| io::Error::other("application input rights"))?;
            let envelope = File::from(envelope);
            let payload = File::from(payload);
            let a = envelope.metadata()?;
            let b = payload.metadata()?;
            require(
                (a.dev(), a.ino()) != (b.dev(), b.ino()),
                "aliased application proof inputs",
            )?;
            return Ok((inputs, envelope, payload));
        }
        match wire::wait(
            application.0.peer.as_fd(),
            rustix::event::PollFlags::IN,
            deadline.min(Instant::now() + Duration::from_millis(100)),
        ) {
            Err(error) if error.kind() == io::ErrorKind::TimedOut => (),
            result => result?,
        }
    }
}

fn serve_retained(
    application: &ActiveApplication,
    session: &Session,
    control: &wire::ControlEndpoint,
    parent: (i32, u32, u32),
    resources: &Resources,
    proof: &RetainedWorkerV3ConditionalFillProofV1,
    subject: &[u8],
) -> io::Result<()> {
    let mut sequence = 2;
    let mut quarantined = false;
    loop {
        if !quarantined {
            let result = (|| {
                application.revalidate()?;
                if let Some((message, rights)) = transport::try_receive(
                    application.0.peer.as_fd(),
                    application.sender(),
                    session.identity(),
                )? {
                    application.revalidate()?;
                    require(
                        message.kind() == Kind::Probe
                            && message.sequence() == sequence
                            && rights.is_empty(),
                        "application proof phase or replay",
                    )?;
                    resources.revalidate()?;
                    require(
                        evidence(proof)? == subject,
                        "application retained subject changed",
                    )?;
                    transport::send(
                        application.0.peer.as_fd(),
                        &Message::new(Kind::Retained, session.identity(), sequence, subject)
                            .map_err(other)?,
                        Instant::now() + Duration::from_secs(30),
                    )?;
                    sequence = sequence
                        .checked_add(1)
                        .ok_or_else(|| io::Error::other("application sequence exhausted"))?;
                }
                Ok::<(), io::Error>(())
            })();
            if result.is_err() {
                quarantined = true;
            }
        }
        if answer_root(control, parent, session, resources, proof, quarantined).is_err() {
            quarantined = true;
        }
        std::thread::park_timeout(Duration::from_millis(10));
    }
}

fn answer_root(
    control: &wire::ControlEndpoint,
    parent: (i32, u32, u32),
    session: &Session,
    resources: &Resources,
    proof: &RetainedWorkerV3ConditionalFillProofV1,
    quarantined: bool,
) -> io::Result<()> {
    control.revalidate()?;
    if let Some((kind, body)) = wire::try_receive(control.as_fd(), parent, session.nonce())? {
        control.revalidate()?;
        require(
            kind == wire::PROBE && body.is_empty(),
            "application mode has no root release",
        )?;
        resources.revalidate()?;
        let subject = evidence(proof)?;
        wire::send(
            control.as_fd(),
            session.nonce(),
            if quarantined {
                wire::QUARANTINED
            } else {
                wire::RETAINED
            },
            &subject,
            Instant::now() + Duration::from_secs(30),
        )?;
    }
    Ok(())
}

fn quarantine_without_proof(
    error: &io::Error,
    control: &wire::ControlEndpoint,
    nonce: [u8; 32],
    custody: impl Sized,
) -> ! {
    let mut text = error.to_string();
    while text.len() > 512 {
        text.pop();
    }
    let _ = wire::send(
        control.as_fd(),
        nonce,
        wire::REJECTED,
        text.as_bytes(),
        Instant::now() + Duration::from_secs(1),
    );
    loop {
        std::hint::black_box(&custody);
        std::thread::park_timeout(Duration::from_secs(1));
    }
}

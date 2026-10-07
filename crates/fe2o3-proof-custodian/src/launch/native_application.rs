//! Private native launch mechanics. Only authenticated manager registration may call.
#[path = "native_application_startup.rs"]
pub(crate) mod startup;
use crate::native_application::{
    CAPSULE_BYTES, Capsule, control, resources::seal_native_control_bytes,
};
use crate::{
    ProductionNativeApplicationProofCustodianDeploymentV1 as Deployment, other, require, wire,
};
use fe2o3_broker_authority_service::{
    ApplicationCurrentnessCustodyV3 as Currentness, LiveClientPidfdIdentityV2 as Client,
};
use fe2o3_compiler_closure_capability::RootProductionCompilerExecutionDeploymentV3 as CompilerDeployment;
use fe2o3_kernel_ir::{
    CanonicalKernelIrStorageAccountIdentityV1 as Account,
    CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
    CanonicalKernelIrWorkLedgerIdentityV1 as Ledger,
};
use fe2o3_protected_service_profile::{
    ProtectedServiceNamespaceSetV1 as Namespaces, observations,
    validate_proof_controller_process_v1,
};
use fe2o3_protected_service_spawn::{
    PROTECTED_SERVICE_GATE_RELEASE_V1, PROTECTED_SERVICE_PROFILE_READY_V1,
    ProtectedServiceCleanupServiceV2 as Cleanup, ProtectedServiceDescriptorBindingV1 as BindingFd,
    launch_io,
    native_spawn::proof_controller::{
        NativeProofControllerCancellationV1 as Cancellation,
        RootOwnedNativeProofControllerV1 as Child, StagedNativeProofControllerV1 as Stage,
    },
};
use fe2o3_protected_static_executable::ProtectedStaticExecutableV2 as Image;
use fe2o3_runtime_protocol::{
    NativeApplicationProofSessionV1 as Session, NativeApplicationRegistrationBindingV1 as Binding,
    NativeApplicationSessionTranscriptV1 as Transcript,
};
use std::{
    fs::File,
    io,
    os::fd::{AsFd, OwnedFd},
    time::{Duration, Instant},
};

const WORK: usize = 128 * 1024
    + observations::PROCESS_VALIDATE_WORK
    + observations::NAMESPACE_CAPTURE_WORK
    + observations::NAMESPACE_PROCESS_WORK;
const SCRATCH: usize = 128 * 1024
    + observations::PROCESS_VALIDATE_SCRATCH
    + observations::NAMESPACE_CAPTURE_SCRATCH
    + observations::NAMESPACE_PROCESS_SCRATCH;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Phase {
    Profile,
    Gate,
    Exec,
    Resources,
    Ready,
}

/// Not public until the actual root manager's consuming registration route exists.
/// Drop before offer delegates the funded child/domain cleanup; after offer or a
/// possibly delivered Activate it fail-stops instead of pretending GPU settlement.
pub(crate) struct PendingNativeApplication<'root, 'work> {
    child: Child<'work>,
    deployment: Deployment<'work>,
    compiler_deployment: &'root CompilerDeployment<'work>,
    currentness: Currentness<'root, 'work>,
    application: Client,
    cargo: Client,
    capsule: Capsule,
    control: wire::ControlEndpoint,
    namespaces: Namespaces,
    ready: OwnedFd,
    gate: Option<OwnedFd>,
    status: OwnedFd,
    session: Session,
    deadline: Instant,
    execution_deadline: Instant,
    phase: Phase,
    offered: bool,
    activation_sent: bool,
    activated: bool,
    poisoned: bool,
    probe_sent: bool,
    probe_deadline: Option<Instant>,
    proof_observed: bool,
    application_terminal: bool,
    domain_retired: bool,
    ledger: Ledger,
    account: Option<Account>,
    retained: usize,
}

impl<'root, 'work> PendingNativeApplication<'root, 'work> {
    /// Call solely from the authenticated ReceivedNativeApplication staging join.
    /// This private parts constructor deliberately grants no registration authority.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn begin(
        deployment: Deployment<'work>,
        compiler_deployment: &'root CompilerDeployment<'work>,
        currentness: Currentness<'root, 'work>,
        binding: Binding,
        transcript: Transcript,
        application: Client,
        cargo: Client,
        proof_peer: OwnedFd,
        deadline: Instant,
        cleanup: &mut Cleanup,
        budget: &mut Budget<'work>,
    ) -> io::Result<(Self, usize)> {
        let floor = budget.storage();
        budget.charge_work(WORK).map_err(other)?;
        budget.reserve_storage(SCRATCH).map_err(other)?;
        require(
            Instant::now() < deadline,
            "native controller startup deadline",
        )?;
        deployment.revalidate(budget)?;
        compiler_deployment.revalidate(budget).map_err(other)?;
        currentness.revalidate_application(&application, &cargo, &binding, budget)?;
        application.validate_parent(&cargo, budget).map_err(other)?;
        let credentials = deployment.deployment().credentials()?;
        let app_identity = application.expected_client();
        let cargo_identity = cargo.expected_client();
        require(
            deployment.deployment().compiler_policy_identity()
                == *binding
                    .compiler_handoff()
                    .launch_manifest()
                    .policy_identity()
                    .as_bytes(),
            "native proof/compiler policy differs before spawn",
        )?;
        require(
            credentials.uid() != app_identity.uid()
                && credentials.uid() != cargo_identity.uid()
                && credentials.gid() != app_identity.gid()
                && credentials.gid() != cargo_identity.gid(),
            "native proof/application roles overlap before spawn",
        )?;
        require(
            deployment.deployment().compiler_policy_identity()
                == *compiler_deployment.policy().identity().as_bytes(),
            "native proof/root-installed compiler policy differs",
        )?;
        for (uid, gid) in [
            (
                compiler_deployment.profile().supervisor_uid(),
                compiler_deployment.profile().supervisor_gid(),
            ),
            (
                compiler_deployment.supervisor().service_uid(),
                compiler_deployment.supervisor().service_gid(),
            ),
            (
                compiler_deployment.anchor().service().uid(),
                compiler_deployment.anchor().service().gid(),
            ),
        ] {
            require(
                credentials.uid() != uid && credentials.gid() != gid,
                "native proof/signing role credentials overlap before spawn",
            )?;
        }
        let namespaces = Namespaces::capture_current_thread().map_err(other)?;
        let peer = wire::ControlEndpoint::admit(proof_peer)?;
        budget
            .reserve_storage(size_of::<wire::ControlEndpoint>())
            .map_err(other)?;
        let (capsule, s) =
            Capsule::capture(binding, transcript, &application, &cargo, &peer, budget)?;
        budget.reserve_storage(s).map_err(other)?;
        let (capsule_bytes, s) = capsule.encode(budget)?;
        budget.reserve_storage(s).map_err(other)?;
        let config = deployment.deployment();
        let (config_file, s) = seal_native_control_bytes(
            config.canonical_bytes(),
            config.canonical_bytes().len(),
            budget,
        )?;
        budget.reserve_storage(s).map_err(other)?;
        let (capsule_file, s) = seal_native_control_bytes(&capsule_bytes, CAPSULE_BYTES, budget)?;
        budget.reserve_storage(s).map_err(other)?;
        let (policy_file, s) = deployment.clone_policy_for_handoff(budget)?;
        budget.reserve_storage(s).map_err(other)?;
        let (app_fd, s) = application.try_clone_for_transfer(budget).map_err(other)?;
        budget
            .reserve_storage(s.additional_storage())
            .map_err(other)?;
        let (cargo_fd, s) = cargo.try_clone_for_transfer(budget).map_err(other)?;
        budget
            .reserve_storage(s.additional_storage())
            .map_err(other)?;
        let (image, s) = deployment
            .executable
            .try_clone_for_exec(budget)
            .map_err(other)?;
        let image_storage = s.additional_storage();
        budget.reserve_storage(image_storage).map_err(other)?;
        let (root, child_peer) = wire::control_pair()?;
        let root = wire::ControlEndpoint::admit(root)?;
        let (ready, ready_child) = rustix::pipe::pipe_with(rustix::pipe::PipeFlags::CLOEXEC)?;
        let (gate_child, gate) = rustix::pipe::pipe_with(rustix::pipe::PipeFlags::CLOEXEC)?;
        let (status, status_child) = rustix::net::socketpair(
            rustix::net::AddressFamily::UNIX,
            rustix::net::SocketType::SEQPACKET,
            rustix::net::SocketFlags::CLOEXEC,
            None,
        )?;
        for fd in [&ready, &gate, &status] {
            let flags = rustix::fs::fcntl_getfl(fd)?;
            rustix::fs::fcntl_setfl(fd, flags | rustix::fs::OFlags::NONBLOCK)?;
        }
        let descriptors = [
            config_file.as_fd(),
            capsule_file.as_fd(),
            app_fd.as_fd(),
            cargo_fd.as_fd(),
            peer.as_fd(),
            child_peer.as_fd(),
            policy_file.as_fd(),
        ];
        let bindings = descriptors
            .into_iter()
            .zip(3..10)
            .map(|(fd, slot)| BindingFd::new(fd, slot).map_err(other))
            .collect::<io::Result<Vec<_>>>()?;
        let source_storage = image_storage
            + config.canonical_bytes().len()
            + CAPSULE_BYTES
            + usize::try_from(config.semantic_policy_byte_len()).map_err(other)?
            + 64 * 1024;
        budget.reserve_storage(source_storage).map_err(other)?;
        // SAFETY: every source is the original admitted/sealed file; staged copies
        // are checked again below. The fresh-domain controls are never inherited.
        let (stage, s) = unsafe {
            Stage::stage(
                &image,
                &bindings,
                ready_child.as_fd(),
                gate_child.as_fd(),
                status_child.as_fd(),
                source_storage,
                budget,
            )
        }
        .map_err(other)?;
        budget
            .reserve_storage(s.additional_storage())
            .map_err(other)?;
        deployment.revalidate(budget)?;
        compiler_deployment.revalidate(budget).map_err(other)?;
        currentness.revalidate_application(&application, &cargo, &capsule.binding, budget)?;
        deployment
            .executable
            .revalidate_exec_clone(stage.executable(), budget)
            .map_err(other)?;
        application
            .validate_transfer(
                stage
                    .binding(5)
                    .ok_or_else(|| io::Error::other("native app stage missing"))?,
                budget,
            )
            .map_err(other)?;
        cargo
            .validate_transfer(
                stage
                    .binding(6)
                    .ok_or_else(|| io::Error::other("native Cargo stage missing"))?,
                budget,
            )
            .map_err(other)?;
        for (slot, source) in [(3, &config_file), (4, &capsule_file), (9, &policy_file)] {
            exact_file(
                stage
                    .binding(slot)
                    .ok_or_else(|| io::Error::other("native sealed stage missing"))?,
                source,
            )?;
        }
        for (slot, source) in [(7, peer.as_fd()), (8, child_peer.as_fd())] {
            let staged = stage
                .binding(slot)
                .ok_or_else(|| io::Error::other("native peer stage missing"))?;
            require(
                rustix::fs::fstat(staged)?.st_ino == rustix::fs::fstat(source)?.st_ino
                    && rustix::fs::fstat(staged)?.st_dev == rustix::fs::fstat(source)?.st_dev,
                "native staged peer differs",
            )?;
        }
        let credentials = config.credentials()?;
        // SAFETY: same independently admitted deployment, original account/cleanup
        // pool, original root thread; gate stays closed through profile validation.
        let (child, s) =
            unsafe { stage.spawn_in_fresh_domain(credentials, cleanup, budget) }.map_err(other)?;
        budget
            .reserve_storage(s.additional_storage())
            .map_err(other)?;
        let (session, s) = Session::new(
            transcript,
            config.identity(),
            capsule.nonce,
            (
                child.pid().as_raw_pid() as u32,
                credentials.uid(),
                credentials.gid(),
            ),
            budget,
        )
        .map_err(other)?;
        budget
            .reserve_storage(s.additional_storage())
            .map_err(other)?;
        let retained = [
            deployment.retained_storage()?,
            child.retained_storage(),
            session.retained_storage(),
            currentness.retained_storage(),
            application.retained_storage(),
            cargo.retained_storage(),
            capsule.retained_storage(),
            size_of::<Self>(),
            SCRATCH,
        ]
        .into_iter()
        .try_fold(0_usize, |total, bytes| total.checked_add(bytes))
        .ok_or_else(|| io::Error::other("native pending owner accounting overflow"))?;
        let value = Self {
            child,
            deployment,
            compiler_deployment,
            currentness,
            application,
            cargo,
            capsule,
            control: root,
            namespaces,
            ready,
            gate: Some(gate),
            status,
            session,
            deadline,
            execution_deadline: Instant::now() + Duration::from_secs(300),
            phase: Phase::Profile,
            offered: false,
            activation_sent: false,
            activated: false,
            poisoned: false,
            probe_sent: false,
            probe_deadline: None,
            proof_observed: false,
            application_terminal: false,
            domain_retired: false,
            ledger: budget.work_ledger_identity_v1(),
            account: budget.storage_account_identity_v1(),
            retained,
        };
        // This manager retains no receive alias of the application's proof peer.
        drop(stage);
        drop(bindings);
        drop((
            peer,
            child_peer,
            app_fd,
            cargo_fd,
            config_file,
            capsule_file,
            policy_file,
            image,
            ready_child,
            gate_child,
            status_child,
        ));
        budget
            .release_storage(budget.storage() - floor)
            .map_err(other)?;
        Ok((value, retained))
    }

    fn revalidate(&self, budget: &mut Budget<'work>) -> io::Result<()> {
        require(
            !self.poisoned
                && self.ledger == budget.work_ledger_identity_v1()
                && self.account == budget.storage_account_identity_v1()
                && budget.storage()
                    >= self
                        .retained
                        .checked_add(self.compiler_deployment.retained_storage().map_err(other)?)
                        .ok_or_else(|| {
                            io::Error::other("native root deployment accounting overflow")
                        })?,
            "native launcher account or state differs",
        )?;
        budget.charge_work(WORK).map_err(other)?;
        budget.reserve_storage(SCRATCH).map_err(other)?;
        self.deployment.revalidate(budget)?;
        self.compiler_deployment.revalidate(budget).map_err(other)?;
        if self.activated {
            self.currentness.revalidate_continuous_application(
                &self.application,
                &self.cargo,
                &self.capsule.binding,
                budget,
            )?;
        } else {
            self.currentness.revalidate_application(
                &self.application,
                &self.cargo,
                &self.capsule.binding,
                budget,
            )?;
        }
        require(
            self.child.is_live(budget).map_err(other)?,
            "native controller exited",
        )?;
        if self.phase != Phase::Profile {
            validate_proof_controller_process_v1(
                self.deployment.deployment().credentials()?,
                self.child.pid(),
            )
            .map_err(other)?;
        }
        self.namespaces
            .revalidate_process(self.child.pid())
            .map_err(other)?;
        self.control.revalidate()?;
        if matches!(self.phase, Phase::Resources | Phase::Ready) {
            let charge =
                Image::file_storage(self.deployment.executable.measurement()).map_err(other)?;
            budget.reserve_storage(charge).map_err(other)?;
            let image = File::from(rustix::fs::open(
                format!("/proc/{}/exe", self.child.pid().as_raw_pid()),
                rustix::fs::OFlags::RDONLY | rustix::fs::OFlags::CLOEXEC,
                rustix::fs::Mode::empty(),
            )?);
            self.deployment
                .executable
                .revalidate_exec_clone(&image, budget)
                .map_err(other)?;
            require(
                self.child.is_live(budget).map_err(other)?,
                "native controller exited during image inspection",
            )?;
            drop(image);
            budget.release_storage(charge).map_err(other)?;
        }
        budget.release_storage(SCRATCH).map_err(other)
    }
    pub(crate) fn poll_ready(&mut self, budget: &mut Budget<'work>) -> io::Result<bool> {
        let result = self.poll_ready_inner(budget);
        if result.is_err() {
            self.poisoned = true;
        }
        result
    }
    fn poll_ready_inner(&mut self, budget: &mut Budget<'work>) -> io::Result<bool> {
        require(
            Instant::now() < self.deadline,
            "native controller startup deadline",
        )?;
        self.revalidate(budget)?;
        match self.phase {
            Phase::Profile => {
                let Some((n, value)) = read_byte(&self.ready)? else {
                    return Ok(false);
                };
                require(
                    n == 1 && value == PROTECTED_SERVICE_PROFILE_READY_V1,
                    "native profile readiness differs",
                )?;
                self.phase = Phase::Gate;
                self.revalidate(budget)?;
            }
            Phase::Gate => {
                match rustix::io::write(
                    self.gate
                        .as_ref()
                        .ok_or_else(|| io::Error::other("native gate missing"))?,
                    &[PROTECTED_SERVICE_GATE_RELEASE_V1],
                ) {
                    Ok(1) => (),
                    Err(rustix::io::Errno::AGAIN | rustix::io::Errno::INTR) => return Ok(false),
                    _ => return Err(io::Error::other("native gate release failed")),
                }
                self.gate = None;
                self.phase = Phase::Exec;
            }
            Phase::Exec => {
                budget
                    .reserve_storage(launch_io::ATTEMPT_SCRATCH)
                    .map_err(other)?;
                launch_io::await_exec_eof(
                    self.status.as_fd(),
                    &mut ExecObserver {
                        child: &self.child,
                        budget,
                    },
                    self.deadline,
                )
                .map_err(|error| match error {
                    launch_io::Error::Observer(error) => error,
                    launch_io::Error::Failure(_) => {
                        io::Error::other("native authenticated exec EOF refused")
                    }
                })?;
                budget
                    .release_storage(launch_io::ATTEMPT_SCRATCH)
                    .map_err(other)?;
                self.phase = Phase::Resources;
                // Validate the original live running image before releasing any
                // inherited artifact-lock lease, even after canonical socket EOF.
                self.revalidate(budget)?;
                // SAFETY: this exact original child's exclusive exec-status
                // endpoint reported authenticated EOF and its original running
                // image/profile/namespaces were revalidated above.
                unsafe { self.child.confirm_exec(budget) }.map_err(other)?;
            }
            Phase::Resources => {
                let Some(kind) = self.receive(budget)? else {
                    return Ok(false);
                };
                require(
                    kind == control::Kind::Ready,
                    "native resources Ready differs",
                )?;
                self.phase = Phase::Ready;
            }
            Phase::Ready => return Ok(true),
        }
        Ok(false)
    }
    fn receive(&self, budget: &mut Budget<'work>) -> io::Result<Option<control::Kind>> {
        let c = self.deployment.deployment().credentials()?;
        control::try_receive(
            &self.control,
            (self.child.pid().as_raw_pid(), c.uid(), c.gid()),
            self.session.nonce(),
            self.session.identity(),
            budget,
        )
    }
    pub(crate) fn session(&self) -> &Session {
        &self.session
    }
    pub(crate) fn offer_ready(
        &mut self,
        budget: &mut Budget<'work>,
    ) -> io::Result<(OwnedFd, usize)> {
        require(
            self.phase == Phase::Ready && !self.offered && Instant::now() < self.deadline,
            "native Ready offer phase",
        )?;
        self.revalidate(budget)?;
        let (pidfd, s) = self.child.try_clone_pidfd(budget).map_err(other)?;
        self.offered = true;
        Ok((pidfd, s.additional_storage()))
    }
    /// Caller must have transferred CustodianReady and dropped every external
    /// application-proof receive alias. No socket query proves that invariant.
    pub(crate) fn poll_activate(&mut self, budget: &mut Budget<'work>) -> io::Result<bool> {
        let result = (|| {
            require(self.offered, "native activation before Ready offer")?;
            self.revalidate(budget)?;
            if self.activated {
                return Ok(true);
            }
            require(Instant::now() < self.deadline, "native activation deadline")?;
            if !self.activation_sent {
                self.activation_sent = true;
                if !control::try_send(
                    &self.control,
                    control::Kind::Activate,
                    self.session.nonce(),
                    self.session.identity(),
                    budget,
                )? {
                    self.activation_sent = false;
                }
                return Ok(false);
            }
            let Some(kind) = self.receive(budget)? else {
                return Ok(false);
            };
            require(
                kind == control::Kind::Activated,
                "native activation acknowledgment differs",
            )?;
            self.activated = true;
            Ok(true)
        })();
        if result.is_err() {
            self.poisoned = true;
        }
        result
    }
    pub(crate) fn poll_probe(&mut self, budget: &mut Budget<'work>) -> io::Result<bool> {
        let result = (|| {
            require(self.activated, "native probe before activation")?;
            self.revalidate(budget)?;
            let deadline = *self.probe_deadline.get_or_insert_with(|| {
                if self.proof_observed {
                    Instant::now() + Duration::from_secs(30)
                } else {
                    self.execution_deadline
                }
            });
            require(
                Instant::now() < deadline,
                "native controller probe deadline",
            )?;
            if !self.probe_sent {
                self.probe_sent = control::try_send(
                    &self.control,
                    control::Kind::Probe,
                    self.session.nonce(),
                    self.session.identity(),
                    budget,
                )?;
                return Ok(false);
            }
            let Some(kind) = self.receive(budget)? else {
                return Ok(false);
            };
            require(
                kind == control::Kind::Retained,
                "native retained observation differs",
            )?;
            self.probe_sent = false;
            self.probe_deadline = None;
            self.proof_observed = true;
            Ok(true)
        })();
        if result.is_err() {
            self.poisoned = true;
        }
        result
    }
    pub(crate) fn cancel_before_offer(&mut self) -> io::Result<Cancellation> {
        require(
            may_cancel(self.offered, self.activation_sent),
            "native cancellation is not settlement",
        )?;
        self.poisoned = true;
        Ok(self.child.cancel())
    }

    // Application exit only permits CPU-controller cancellation. All original
    // proof dependencies remain in this owner until this exact isolated domain
    // and its root have actually retired through the original cleanup record.
    fn poll_terminal_cleanup(&mut self, budget: &mut Budget<'work>) -> io::Result<bool> {
        budget.charge_work(128).map_err(other)?;
        require(
            self.activated
                && self.ledger == budget.work_ledger_identity_v1()
                && self.account == budget.storage_account_identity_v1()
                && budget.storage() >= self.retained,
            "native terminal cleanup phase/account",
        )?;
        if !self.application_terminal {
            let Some((terminal, charge)) =
                self.application.observe_terminal(budget).map_err(other)?
            else {
                return Ok(false);
            };
            budget
                .reserve_storage(charge.additional_storage())
                .map_err(other)?;
            require(
                terminal.is_for(&self.application),
                "native terminal owner changed",
            )?;
            drop(terminal);
            budget
                .release_storage(charge.additional_storage())
                .map_err(other)?;
            self.application_terminal = true;
            self.poisoned = true;
            let _ = self.child.cancel();
        }
        if let Some((retirement, charge)) = self.child.observe_retirement(budget).map_err(other)? {
            budget
                .reserve_storage(charge.additional_storage())
                .map_err(other)?;
            require(
                retirement.is_for(&self.child),
                "native cleanup domain changed",
            )?;
            drop(retirement);
            budget
                .release_storage(charge.additional_storage())
                .map_err(other)?;
            self.domain_retired = true;
        }
        Ok(self.domain_retired)
    }
}
impl Drop for PendingNativeApplication<'_, '_> {
    fn drop(&mut self) {
        if (self.offered || self.activation_sent)
            && !(self.application_terminal && self.domain_retired)
        {
            std::process::abort();
        }
    }
}
fn exact_file(a: &File, b: &File) -> io::Result<()> {
    require(
        rustix::io::fcntl_getfd(a)? == rustix::io::FdFlags::CLOEXEC
            && rustix::fs::fcntl_getfl(a)? == rustix::fs::fcntl_getfl(b)?
            && rustix::fs::fcntl_get_seals(a)? == rustix::fs::fcntl_get_seals(b)?,
        "native staged file flags differ",
    )?;
    let a = rustix::fs::fstat(a)?;
    let b = rustix::fs::fstat(b)?;
    require(
        a.st_dev == b.st_dev
            && a.st_ino == b.st_ino
            && a.st_mode == b.st_mode
            && a.st_size == b.st_size,
        "native staged file differs",
    )
}
fn may_cancel(offered: bool, activation_sent: bool) -> bool {
    !offered && !activation_sent
}
struct ExecObserver<'a, 'work> {
    child: &'a Child<'work>,
    budget: &'a mut Budget<'work>,
}
impl launch_io::Observer for ExecObserver<'_, '_> {
    type Error = io::Error;
    fn before_attempt(&mut self, boundary: launch_io::Boundary) -> io::Result<()> {
        self.budget.charge_work(boundary.work()).map_err(other)
    }
    fn is_live(&mut self) -> io::Result<bool> {
        self.child.is_live(self.budget).map_err(other)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn native_controller_cancellation_never_means_post_offer_settlement() {
        assert!(may_cancel(false, false));
        assert!(!may_cancel(true, false));
        assert!(!may_cancel(false, true));
        assert!(!may_cancel(true, true));
    }
    #[test]
    fn native_root_sealed_stage_requires_original_object_not_equal_bytes() {
        let first = wire::seal(b"identical inert control").unwrap();
        let clone = first.try_clone().unwrap();
        let other = wire::seal(b"identical inert control").unwrap();
        exact_file(&clone, &first).unwrap();
        assert!(exact_file(&other, &first).is_err());
        rustix::io::fcntl_setfd(&clone, rustix::io::FdFlags::empty()).unwrap();
        assert!(exact_file(&clone, &first).is_err());
    }
}
fn read_byte(fd: &OwnedFd) -> io::Result<Option<(usize, u8)>> {
    // A second byte makes a malformed profile packet visible instead of silently
    // accepting its first byte. This helper is never used for exec socket EOF.
    let mut byte = [0; 2];
    match rustix::io::read(fd, &mut byte) {
        Ok(n) => Ok(Some((n, byte[0]))),
        Err(rustix::io::Errno::AGAIN | rustix::io::Errno::INTR) => Ok(None),
        Err(error) => Err(error.into()),
    }
}

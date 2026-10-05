//! Consuming preparation on the original root account; still refusal-only.
use super::*;
use crate::compiler_invocation_backing::{
    CompilerInvocationBacking as Backing, CompilerInvocationBackingError as BackingError,
};
use crate::proof_helper_backing::{ProofHelperBacking, ProofHelperBackingError};
use crate::proof_helper_launch::{self, ManagedProofHelper, ProofHelperLaunchError};
use fe2o3_compiler_closure_capability::{
    ApprovedCompilerPolicyV2 as Approval, CompilerApprovalErrorV2 as ApprovalError,
    RetainedCompilerRuntimeErrorV1 as RuntimeError, RetainedCompilerRuntimeV1 as Runtime,
};
use fe2o3_kernel_ir::{
    CanonicalKernelIrVerificationResourceErrorV1 as Resource,
    CanonicalKernelIrWorkLedgerIdentityV1 as Ledger,
};
use fe2o3_protected_service_profile::ProtectedServiceCredentialProfileV1 as Credentials;
use fe2o3_protected_service_spawn::ProtectedServiceCleanupServiceV2 as Cleanup;
use std::sync::Arc;

#[path = "native_compiler_attempt.rs"]
mod compiler_attempt;

#[path = "native_root_request_quota.rs"]
mod quota;

#[cfg(test)]
#[path = "native_root_request_fault_tests.rs"]
mod faults;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum State {
    Received,
    Prepared,
    HelperReady,
    CompilerGated,
    Refused,
    Failed,
    Cancelled,
}

/// Owns the ORIGINAL preparation and complete receiver before any compiler
/// preparation can fail. No constructor accepts a descriptor, runtime approval,
/// substitute Prepared, or a reconstructed transport identity.
pub(crate) struct RootCompilerRequest<'work> {
    // Cancel the anchor before receiver/backing retirement on unwind too.
    prepared: Option<Prepared>,
    receiver: Arc<Receiver>,
    backing: Option<Backing>,
    helper: Option<ManagedProofHelper>,
    attempt: Option<compiler_attempt::Attempt<'work>>,
    state: State,
    ledger: Ledger,
    address: usize,
    reserved: usize,
}

impl<'work> RootCompilerRequest<'work> {
    // Prepared and Receiver already have full reservations. The new optional
    // backing is inline here; its heap/file growth is charged by its constructor.
    pub(crate) const ENVELOPE: usize =
        size_of::<Self>() - size_of::<Prepared>() + 2 * size_of::<usize>();
    const LOCAL_WORK: usize = root::LOCAL_WORK + Backing::LOCAL_WORK;

    pub(crate) fn preparation_quota() -> Result<root::CompilerExecutionRootAdmissionQuotaV2> {
        quota::preparation()
    }

    pub(crate) fn refusal_quota() -> Result<root::CompilerExecutionRootAdmissionQuotaV2> {
        quota::refusal()
    }

    pub(crate) fn launch_quota() -> Result<root::CompilerExecutionRootAdmissionQuotaV2> {
        quota::launch()
    }

    pub(crate) fn cleanup_growth() -> Result<(usize, usize)> {
        quota::cleanup_growth()
    }

    /// Infallible ownership installation, BEFORE any fallible validation or
    /// admission. Native prepaid ENVELOPE alongside the whole Receiver before
    /// accept. Preserve every existing reservation until this request drops.
    pub(crate) fn install(prepared: Prepared, receiver: Receiver, b: &Budget<'_>) -> Self {
        Self {
            prepared: Some(prepared),
            receiver: Arc::new(receiver),
            backing: None,
            helper: None,
            attempt: None,
            state: State::Received,
            ledger: b.work_ledger_identity_v1(),
            address: b as *const Budget<'_> as usize,
            reserved: b.storage(),
        }
    }

    pub(crate) fn continuity(&self, b: &mut Budget<'_>) -> Result<()> {
        self.check_account(b)?;
        self.prepared
            .as_ref()
            .ok_or_else(|| rejected("original preparation already cancelled"))?
            .revalidate(b)?;
        Ok(())
    }

    /// Native supplies its original creator's pool. Helper exec is permitted by
    /// that dedicated deployment contract, but the compiler gate NEVER opens.
    ///
    /// # Safety
    /// Preserve the entrypoint's actual outside whole-domain custodian, unique
    /// creator/reaper and mutation exclusions through aggregate pool shutdown.
    #[allow(unsafe_code)]
    pub(crate) unsafe fn step(
        &mut self,
        cleanup: &mut Cleanup,
        b: &mut Budget<'work>,
    ) -> Result<bool> {
        let state = self.state;
        // Set before even entry charging: failure AND unwind consume this turn.
        self.state = State::Failed;
        b.charge_work(Self::LOCAL_WORK)?;
        self.check_account(b)?;
        self.check_complete()?;
        match state {
            State::Received => {
                self.prepare(b)?;
                self.check_complete()?;
                self.state = State::Prepared;
                Ok(false)
            }
            State::Prepared => {
                self.require_cleanup_guard(cleanup, b)?;
                let backing = self
                    .backing
                    .take()
                    .ok_or_else(|| rejected("missing original backing"))?;
                let (backing, growth) =
                    ProofHelperBacking::prepare(backing, b).map_err(helper_backing_error)?;
                self.reserve_growth(growth, b)?;
                let peer =
                    net::sockopt::socket_peercred(self.receiver.connection.as_ref().unwrap())
                        .map_err(|source| Error::Io {
                            operation: "original compiler peer",
                            source,
                        })?;
                if self.receiver.sender
                    != Some(io::MessageSender::new(
                        peer.pid.as_raw_pid(),
                        peer.uid.as_raw(),
                        peer.gid.as_raw(),
                    ))
                {
                    return Err(rejected("original compiler peer changed"));
                }
                let credentials =
                    fe2o3_protected_service_profile::ProtectedServiceCredentialProfileV1::new(
                        peer.uid.as_raw(),
                        peer.gid.as_raw(),
                    )
                    .map_err(|_| rejected("compiler peer credentials refused"))?;
                require_separate_helper_peer(backing.credentials(), credentials)?;
                let deadline = self.receiver.deadline.unwrap();
                let remaining = deadline
                    .checked_duration_since(Instant::now())
                    .ok_or_else(|| rejected("intake deadline exceeded"))?;
                let session = *self.receiver.challenge.as_ref().unwrap().identity();
                // The additional shared handle is a new payload field, not a
                // second reservation of the receiver or consumed capture.
                self.reserve_growth(size_of::<Arc<Receiver>>(), b)?;
                // SAFETY: the original complete receiver is published in the
                // helper's funded payload before clone. Native supplies the sole
                // dedicated creator and actual outside-domain custody contract.
                let (helper, growth) = unsafe {
                    proof_helper_launch::launch(
                        backing,
                        Arc::clone(&self.receiver),
                        session,
                        credentials,
                        remaining,
                        cleanup,
                        b,
                    )
                }
                .map_err(helper_error)?;
                self.helper = Some(helper);
                self.reserve_growth(growth, b)?;
                #[cfg(test)]
                Self::postclone_checkpoint_for_test(
                    "helper-ready",
                    self.helper.as_ref().unwrap().pid_for_test(),
                    b,
                )?;
                self.state = State::HelperReady;
                Ok(false)
            }
            State::HelperReady => {
                self.require_cleanup_guard(cleanup, b)?;
                let peer =
                    net::sockopt::socket_peercred(self.receiver.connection.as_ref().unwrap())
                        .map_err(|source| Error::Io {
                            operation: "original compiler peer",
                            source,
                        })?;
                if self.receiver.sender
                    != Some(io::MessageSender::new(
                        peer.pid.as_raw_pid(),
                        peer.uid.as_raw(),
                        peer.gid.as_raw(),
                    ))
                {
                    return Err(rejected("original compiler peer changed"));
                }
                let credentials =
                    fe2o3_protected_service_profile::ProtectedServiceCredentialProfileV1::new(
                        peer.uid.as_raw(),
                        peer.gid.as_raw(),
                    )
                    .map_err(|_| rejected("compiler peer credentials refused"))?;
                let helper = self
                    .helper
                    .take()
                    .ok_or_else(|| rejected("missing original helper"))?;
                // SAFETY: same dedicated creator/pool as helper launch; only the
                // original receiver supplies cwd/stdio. The gate stays closed.
                let (attempt, growth) = unsafe {
                    compiler_attempt::launch(
                        helper,
                        &self.receiver,
                        credentials,
                        self.receiver.deadline.unwrap(),
                        cleanup,
                        b,
                    )
                }
                .map_err(helper_error)?;
                self.attempt = Some(attempt);
                self.reserve_growth(growth, b)?;
                self.state = State::CompilerGated;
                Ok(false)
            }
            State::CompilerGated => {
                let finished = b.with_prepaid_scope(
                    self.reserved,
                    8,
                    EXCHANGE_WORK,
                    FRAME + io::packet_receive_scratch(N),
                    |b| {
                        self.attempt
                            .as_ref()
                            .ok_or_else(|| rejected("missing original compiler attempt"))?
                            .revalidate(&self.receiver, b)
                            .map_err(helper_error)?;
                        self.check_complete()?;
                        self.receiver.send_refusal_packet()
                    },
                )?;
                self.state = if finished {
                    State::Refused
                } else {
                    State::CompilerGated
                };
                Ok(finished)
            }
            _ => Err(rejected("compiler request cannot be reused")),
        }
    }

    fn require_cleanup_guard(&self, cleanup: &mut Cleanup, b: &mut Budget<'_>) -> Result<()> {
        let prepared = self
            .prepared
            .as_ref()
            .ok_or_else(|| rejected("missing original preparation"))?;
        // This existing join validates the actual pool guard against the retained
        // root-bound lifecycle; the same request and cleanup accounts fund it.
        prepared.validate_cleanup_guard(cleanup, b)?;
        Ok(())
    }

    fn prepare(&mut self, b: &mut Budget<'_>) -> Result<()> {
        let prepared = self
            .prepared
            .as_ref()
            .ok_or_else(|| rejected("missing original preparation"))?;
        let floor = root::sum(&[
            prepared.retained_storage(),
            Receiver::STORAGE,
            Self::ENVELOPE,
        ])?;
        if b.storage() < floor {
            return Err(Resource::Accounting.into());
        }
        let (approval, charge) = Approval::from_production_policy(b).map_err(approval_error)?;
        self.reserve_growth(charge.retained_storage(), b)?;
        let prepared = self.prepared.as_ref().unwrap();
        let profile = approval.profile().profile();
        let deployment = prepared.trust.deployment().deployment();
        if profile.policy().canonical_bytes() != prepared.trust.policy().policy().canonical_bytes()
            || profile.supervisor_uid() != deployment.service_uid()
            || profile.supervisor_gid() != deployment.service_gid()
            || profile.external_anchor_service() != deployment.external_anchor_service()
        {
            return Err(rejected(
                "fixed compiler approval differs from original root",
            ));
        }
        let (runtime, charge) =
            Runtime::from_production_runtime(approval, b).map_err(runtime_error)?;
        self.reserve_growth(charge.additional_storage(), b)?;
        // Both are the original decoder/capture owners, not reconstructions from
        // the retained raw rights. Failed consumption never attempts to recover them.
        let receiver = Arc::get_mut(&mut self.receiver)
            .ok_or_else(|| rejected("received inputs were already transferred"))?;
        let capture = receiver
            .invocation
            .take()
            .ok_or_else(|| rejected("missing received invocation"))?;
        let output = receiver
            .output
            .take()
            .ok_or_else(|| rejected("missing received output"))?;
        let (backing, charge) =
            Backing::prepare(runtime, capture, output, b).map_err(backing_error)?;
        self.reserve_growth(charge.additional_storage(), b)?;
        self.backing = Some(backing);
        Ok(())
    }

    fn reserve_growth(&mut self, growth: usize, b: &mut Budget<'_>) -> Result<()> {
        let reserved = root::sum(&[self.reserved, growth])?;
        b.reserve_storage(growth)?;
        self.reserved = reserved;
        Ok(())
    }

    fn check_account(&self, b: &Budget<'_>) -> Result<()> {
        if b.work_ledger_identity_v1() != self.ledger
            || b as *const Budget<'_> as usize != self.address
            || b.storage() < self.reserved
        {
            return Err(Resource::Accounting.into());
        }
        Ok(())
    }

    fn check_complete(&self) -> Result<()> {
        if self.receiver.phase != Phase::Ack
            || self.receiver.connection.is_none()
            || self.receiver.sender.is_none()
            || self.receiver.ack.is_none()
            || self.receiver.last.is_none()
        {
            return Err(rejected(
                "compiler request requires complete original intake",
            ));
        }
        let challenge = self
            .receiver
            .challenge
            .as_ref()
            .ok_or_else(|| rejected("missing original challenge"))?;
        let prepared = self
            .prepared
            .as_ref()
            .ok_or_else(|| rejected("missing original preparation"))?;
        if challenge.policy_identity() != prepared.trust.policy().policy().identity().as_bytes() {
            return Err(rejected(
                "received request differs from original root policy",
            ));
        }
        let count = challenge.roles().count();
        if self.receiver.files[..count].iter().any(Option::is_none)
            || self.receiver.files[count..].iter().any(Option::is_some)
        {
            return Err(rejected("complete intake lost an original received right"));
        }
        if self
            .receiver
            .deadline
            .is_none_or(|deadline| Instant::now() >= deadline)
        {
            return Err(rejected("intake deadline exceeded"));
        }
        Ok(())
    }

    pub(crate) fn cancel(&mut self) {
        self.state = State::Cancelled;
        if let Some(mut attempt) = self.attempt.take() {
            let _ = attempt.cancel();
        }
        if let Some(helper) = self.helper.take() {
            let _ = helper.cancel();
        }
        // The original pool must reap the anchor before signal restoration.
        // Raw received rights and any prepared compiler backing stay through drain.
        drop(self.prepared.take());
    }

    #[cfg(test)]
    pub(crate) fn arm_postclone_fault_for_test(
        &self,
        phase: &'static str,
        action: &'static str,
        b: &Budget<'_>,
    ) {
        assert!(matches!(self.state, State::Prepared | State::HelperReady));
        faults::arm(&self.receiver, phase, action, b);
    }

    #[cfg(test)]
    pub(crate) fn postclone_fault_observation_for_test() -> faults::Observation {
        faults::observation()
    }

    #[cfg(test)]
    pub(crate) fn postclone_checkpoint_for_test(
        phase: &'static str,
        pid: rustix::process::Pid,
        b: &mut Budget<'_>,
    ) -> std::result::Result<(), Resource> {
        faults::checkpoint(phase, pid, b)
    }

    #[cfg(test)]
    pub(crate) fn received_for_test(&self) -> (&Receiver, Option<&Backing>) {
        (&self.receiver, self.backing.as_ref())
    }

    #[cfg(test)]
    pub(crate) fn failed_for_test(&self) -> bool {
        self.state == State::Failed
    }

    #[cfg(test)]
    pub(crate) fn helper_pid_for_test(&self) -> Option<rustix::process::Pid> {
        (self.state == State::HelperReady).then(|| self.helper.as_ref().unwrap().pid_for_test())
    }

    #[cfg(test)]
    pub(crate) fn compiler_pid_for_test(&self) -> Option<rustix::process::Pid> {
        (self.state == State::CompilerGated).then(|| self.attempt.as_ref().unwrap().pid())
    }
}

fn require_separate_helper_peer(helper: Credentials, peer: Credentials) -> Result<()> {
    if helper.uid() == peer.uid() || helper.gid() == peer.gid() {
        return Err(rejected(
            "proof helper and original compiler peer credentials overlap",
        ));
    }
    Ok(())
}

fn helper_backing_error(error: ProofHelperBackingError) -> Error {
    match error {
        ProofHelperBackingError::Resource(e) => e.into(),
        ProofHelperBackingError::Compiler(e) => backing_error(e),
        ProofHelperBackingError::Runtime(e) => runtime_error(e),
        ProofHelperBackingError::Image(
            fe2o3_protected_static_executable::ProtectedStaticExecutableErrorV2::Resource(e),
        ) => e.into(),
        _ => rejected("proof helper backing refused"),
    }
}

fn helper_error(error: ProofHelperLaunchError) -> Error {
    match error {
        ProofHelperLaunchError::Resource(e) => e.into(),
        ProofHelperLaunchError::Backing(e) => helper_backing_error(e),
        ProofHelperLaunchError::Inventory(crate::native_runtime_inventory::Error::Resource(e)) => {
            e.into()
        }
        ProofHelperLaunchError::Inventory(crate::native_runtime_inventory::Error::Backing(e)) => {
            backing_error(e)
        }
        ProofHelperLaunchError::Native(
            crate::native_launch::CompilerExecutionLaunchErrorV2::Resource(e),
        ) => e.into(),
        ProofHelperLaunchError::Native(e) => e.into(),
        ProofHelperLaunchError::Record(
            fe2o3_compiler_execution_protocol::ProofExecutorBootstrapErrorV1::Resource(e),
        ) => e.into(),
        _ => rejected("native pre-exec attempt refused"),
    }
}

fn approval_error(error: ApprovalError) -> Error {
    match error {
        ApprovalError::Resource(e) => e.into(),
        ApprovalError::Capability(e) => e.into(),
        ApprovalError::Codec(fe2o3_build_authority::CompilerApprovalPolicyErrorV2::Framing(
            fe2o3_build_authority::CompilerApprovalPolicyErrorV1::Charge(e),
        )) => e.into(),
        ApprovalError::Mismatch(reason) => root::invalid("compiler approval", reason),
        _ => rejected("fixed-origin compiler approval refused"),
    }
}

fn runtime_error(error: RuntimeError) -> Error {
    match error {
        RuntimeError::Resource(e) => e.into(),
        RuntimeError::Approval(e) => approval_error(e),
        RuntimeError::Capability(e) => e.into(),
        RuntimeError::Codec(fe2o3_build_authority::CompilerRuntimeManifestErrorV1::Charge(e)) => {
            e.into()
        }
        RuntimeError::Mismatch(reason) => root::invalid("compiler runtime", reason),
        _ => rejected("fixed-origin compiler runtime refused"),
    }
}

fn backing_error(error: BackingError) -> Error {
    use crate::compiler_invocation_staging::RustcInvocationStagingErrorV1 as StagingError;
    match error {
        BackingError::Resource(e) | BackingError::Staging(StagingError::Resource(e)) => e.into(),
        BackingError::Capture(e) => e.into(),
        BackingError::Runtime(e) => runtime_error(e),
        BackingError::Output(e) => output_error(e),
        _ => rejected("received compiler invocation preparation refused"),
    }
}

#[cfg(test)]
#[path = "native_root_request_tests.rs"]
mod tests;

//! Consuming preparation on the original root account; still refusal-only.
use super::*;
use crate::compiler_invocation_backing::{
    CompilerInvocationBacking as Backing, CompilerInvocationBackingError as BackingError,
};
use fe2o3_compiler_closure_capability::{
    ApprovedCompilerPolicyV2 as Approval, CompilerApprovalErrorV2 as ApprovalError,
    RetainedCompilerRuntimeErrorV1 as RuntimeError, RetainedCompilerRuntimeV1 as Runtime,
};
use fe2o3_kernel_ir::{
    CanonicalKernelIrVerificationResourceErrorV1 as Resource,
    CanonicalKernelIrWorkLedgerIdentityV1 as Ledger,
};

#[path = "native_root_request_quota.rs"]
mod quota;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum State {
    Received,
    Prepared,
    Refused,
    Failed,
    Cancelled,
}

/// Owns the ORIGINAL preparation and complete receiver before any compiler
/// preparation can fail. No constructor accepts a descriptor, runtime approval,
/// substitute Prepared, or a reconstructed transport identity.
pub(crate) struct RootCompilerRequest {
    // Cancel the anchor before receiver/backing retirement on unwind too.
    prepared: Option<Prepared>,
    receiver: Receiver,
    backing: Option<Backing>,
    state: State,
    ledger: Ledger,
    address: usize,
    reserved: usize,
}

impl RootCompilerRequest {
    // Prepared and Receiver already have full reservations. The new optional
    // backing is inline here; its heap/file growth is charged by its constructor.
    pub(crate) const ENVELOPE: usize =
        size_of::<Self>() - size_of::<Prepared>() - size_of::<Receiver>();
    const LOCAL_WORK: usize = root::LOCAL_WORK + Backing::LOCAL_WORK;

    pub(crate) fn preparation_quota() -> Result<root::CompilerExecutionRootAdmissionQuotaV2> {
        quota::preparation()
    }

    pub(crate) fn refusal_quota() -> Result<root::CompilerExecutionRootAdmissionQuotaV2> {
        quota::refusal()
    }

    /// Infallible ownership installation, BEFORE any fallible validation or
    /// admission. Native prepaid ENVELOPE alongside the whole Receiver before
    /// accept. Preserve every existing reservation until this request drops.
    pub(crate) fn install(prepared: Prepared, receiver: Receiver, b: &Budget<'_>) -> Self {
        Self {
            prepared: Some(prepared),
            receiver,
            backing: None,
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

    /// Native checks original Prepared continuity before every turn. Neither
    /// successful preparation nor the terminal V4 refusal permits execution.
    pub(crate) fn step(&mut self, b: &mut Budget<'_>) -> Result<bool> {
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
                let finished = b.with_prepaid_scope(
                    self.reserved,
                    8,
                    EXCHANGE_WORK,
                    FRAME + io::packet_receive_scratch(N),
                    |b| {
                        self.backing
                            .as_ref()
                            .ok_or_else(|| rejected("missing consumed compiler backing"))?
                            .revalidate(b)
                            .map_err(backing_error)?;
                        self.check_complete()?;
                        self.receiver.send_refusal()
                    },
                )?;
                self.state = if finished {
                    State::Refused
                } else {
                    State::Prepared
                };
                Ok(finished)
            }
            _ => Err(rejected("compiler request cannot be reused")),
        }
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
        let capture = self
            .receiver
            .invocation
            .take()
            .ok_or_else(|| rejected("missing received invocation"))?;
        let output = self
            .receiver
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
        // The original pool must reap the anchor before signal restoration.
        // Raw received rights and any prepared compiler backing stay through drain.
        drop(self.prepared.take());
    }

    #[cfg(test)]
    pub(crate) fn received_for_test(&self) -> (&Receiver, Option<&Backing>) {
        (&self.receiver, self.backing.as_ref())
    }

    #[cfg(test)]
    pub(crate) fn failed_for_test(&self) -> bool {
        self.state == State::Failed
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

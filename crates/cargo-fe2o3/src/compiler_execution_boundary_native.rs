//! Native Cargo custody on the existing child channel and artifact transaction.
//! Version selection remains with the production migration; there is no fallback.
use fe2o3_artifact_transaction::{
    BuildAttempt, CompilerExecutionReceiptTransportErrorV3 as TransportError,
    CompilerExecutionSubjectErrorV3 as SubjectError,
    CompilerModuleHandoffConsumptionTokenV5 as Token,
    CompilerModuleHandoffCurrentnessLeaseV5 as Lease, CompilerModuleHandoffErrorV5 as HandoffError,
    InertCompilerExecutionSubjectStorageV3 as SubjectStorage,
    InertCompilerExecutionSubjectV3 as Subject, ProducerIdentity,
    acquire_compiler_module_handoff_currentness_lease_v5 as acquire_lease,
    recover_compiler_execution_receipt_transport_with_currentness_v3 as recover,
    recover_compiler_module_handoff_receipt_v5 as recover_publication,
};
use fe2o3_compiler_closure_capability::{
    CompilerExecutionCapabilityErrorV2 as CapabilityError,
    CompilerExecutionClientProfileCapabilityV3 as Profile,
    CompilerExecutionPolicyCapabilityV3 as Policy,
};
use fe2o3_compiler_execution_client::{
    CompilerExecutionHandoffErrorV3 as SupervisorError, CompilerExecutionServiceLaunchV1 as Launch,
    CompilerExecutionSupervisorReadinessV3 as Received,
};
use fe2o3_compiler_execution_protocol::{
    CompilerExecutionAttestationStorageV3 as Storage,
    CompilerExecutionReceiptCarriageV3 as Carriage,
    CompilerExecutionReceiptPublicationErrorV3 as ReceiptError,
    CompilerExecutionServiceLaunchManifestErrorV3 as ManifestError,
    CompilerExecutionServiceLaunchManifestV3 as Manifest,
    CompilerExecutionServiceReadyErrorV3 as ReadyError, CompilerExecutionServiceReadyV3 as Ready,
};
use fe2o3_kernel_ir::{
    CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
    CanonicalKernelIrVerificationResourceErrorV1 as Resource,
};
use std::{error::Error, fmt, mem::size_of, path::Path, time::Instant};

type Result<T> = std::result::Result<T, Failure>;
const LOCAL_WORK: usize = 8192;
const FRAME: usize = 4 * size_of::<Failure>() + 4096;
const SUBJECT_STORAGE: usize = size_of::<(Subject, SubjectStorage)>();

/// The exact selected child, sealed configuration and original account remain
/// owned together through receipt admission. Neither readiness nor a signed
/// carriage grants compiler, publication, load or launch authority.
///
/// Profile provenance, Command/spawn accounting and the incoming launch owner
/// are obligations of the existing broker/child-channel boundary. This does not
/// admit a caller-selected profile as protected configuration.
pub(crate) struct ParentCompilerExecutionReadinessCustodyV3<'b, 'w> {
    profile: Profile,
    policy: Policy,
    received: Received,
    child_pid: u32,
    budget: &'b mut Budget<'w>,
}

impl<'b, 'w> ParentCompilerExecutionReadinessCustodyV3<'b, 'w> {
    /// Prepay both sealed owners, CHILD_LAUNCH_STORAGE and OWNER_STORAGE. Keep
    /// those reservations on refusal; successful transfer reserves its growth.
    /// The account stays exclusively borrowed until this custody is dropped.
    pub(crate) const OWNER_STORAGE: usize =
        size_of::<Self>() - size_of::<Profile>() - size_of::<Policy>() - size_of::<Received>();

    pub(crate) fn finish(
        profile: Profile,
        policy: Policy,
        launch: Launch,
        child_pid: u32,
        deadline: Instant,
        budget: &'b mut Budget<'w>,
    ) -> Result<Self> {
        let floor = profile.retained_storage()
            + policy.retained_storage()
            + Received::CHILD_LAUNCH_STORAGE
            + Self::OWNER_STORAGE;
        let (received, storage) = budget.with_prepaid_scope(floor, 0, 0, FRAME, |b| {
            validate_configuration(&profile, &policy, b)?;
            if child_pid == 0 || launch.client().pid() != child_pid {
                return Err(Failure::Mismatch("launch differs from the selected child"));
            }
            let (received, storage) =
                launch.handoff_to_supervisor_v3_until(profile.profile(), deadline, b)?;
            b.reserve_storage(storage.additional_storage())?;
            validate_readiness(
                &profile,
                &policy,
                child_pid,
                received.manifest(),
                received.readiness(),
                b,
            )?;
            Ok::<_, Failure>((received, storage))
        })?;
        budget.reserve_storage(storage.additional_storage())?;
        Ok(Self {
            profile,
            policy,
            received,
            child_pid,
            budget,
        })
    }

    fn retained_storage(&self) -> usize {
        self.profile.retained_storage()
            + self.policy.retained_storage()
            + self.received.retained_storage()
            + Self::OWNER_STORAGE
    }

    pub(crate) fn revalidate(&mut self) -> Result<()> {
        self.budget
            .with_prepaid_scope(self.retained_storage(), 0, 0, FRAME, |b| {
                validate_readiness(
                    &self.profile,
                    &self.policy,
                    self.child_pid,
                    self.received.manifest(),
                    self.received.readiness(),
                    b,
                )
            })
    }

    pub(crate) fn retain_through<T>(mut self, operation: impl FnOnce(&mut Self) -> T) -> T {
        operation(&mut self)
    }

    /// After the child completes, acquire its publication without releasing the
    /// original account borrow. Both returned owners are fully reserved here.
    /// The enclosing Cargo attempt must prepay and retain its output path and
    /// producer before `finish`; deriving those inputs is outside this boundary.
    pub(crate) fn acquire_current_publication(
        &mut self,
        output_dir: &Path,
        producer: &ProducerIdentity,
        attempt: BuildAttempt,
    ) -> Result<(Lease, Token)> {
        self.revalidate()?;
        let (lease, token) =
            self.budget
                .with_prepaid_scope(self.retained_storage(), 0, 0, FRAME, |b| {
                    let receipt = recover_publication(output_dir, producer, attempt, b)?;
                    if receipt.attempt() != attempt {
                        return Err(Failure::Mismatch(
                            "publication differs from the selected attempt",
                        ));
                    }
                    let (lease, storage) = acquire_lease(output_dir, producer, receipt, b)?;
                    b.reserve_storage(storage.retained_storage())?;
                    let (token, storage) = lease.acquire_current_token(b)?;
                    b.reserve_storage(storage.retained_storage())?;
                    lease.validate_current_token(&token)?;
                    Ok::<_, Failure>((lease, token))
                })?;
        let storage = lease
            .storage()
            .retained_storage()
            .checked_add(token.storage().retained_storage())
            .ok_or(Resource::Arithmetic)?;
        self.budget.reserve_storage(storage)?;
        Ok((lease, token))
    }

    /// Reconstruct the subject from the locked V5 publication, never caller
    /// bytes. Decode and authenticate the native carriage on the same account.
    /// The caller prepays the full lease/token. This owner reserves the returned
    /// carriage's full charge on its retained account; retire that charge only
    /// after the carriage drops. Intermediate subject/transport storage is
    /// retired on refusal or unwind; work and denial history are never refunded.
    /// Parent invocation custody and sealed verifier admission remain required.
    pub(crate) fn admit_current_receipt(
        &mut self,
        lease: &Lease,
        token: &Token,
    ) -> Result<Carriage> {
        let floor = self
            .retained_storage()
            .checked_add(lease.storage().retained_storage())
            .and_then(|n| n.checked_add(token.storage().retained_storage()))
            .ok_or(Resource::Arithmetic)?;
        let (carriage, storage) = self.budget.with_prepaid_scope(floor, 0, 0, FRAME, |b| {
            validate_readiness(
                &self.profile,
                &self.policy,
                self.child_pid,
                self.received.manifest(),
                self.received.readiness(),
                b,
            )?;
            lease.validate_current_token(token)?;
            let (subject, storage) =
                Subject::from_publication(token.receipt(), token.handoff(), b)?;
            b.reserve_storage(storage.retained_storage())?;
            let (transport, storage) = recover(lease, token, &subject, b)?;
            b.reserve_storage(storage.retained_storage())?;
            if transport.receipt().subject() != subject.identity()
                || transport.receipt().length() != transport.exact_bytes().len()
            {
                return Err(Failure::Mismatch(
                    "receipt transport differs from the locked subject",
                ));
            }
            let (carriage, storage) =
                decode_receipt(&self.profile, &subject, transport.exact_bytes(), b)?;
            b.reserve_storage(storage.additional_storage())?;
            validate_readiness(
                &self.profile,
                &self.policy,
                self.child_pid,
                self.received.manifest(),
                self.received.readiness(),
                b,
            )?;
            token.revalidate_locked_currentness(b)?;
            Ok((carriage, storage))
        })?;
        self.budget.reserve_storage(storage.additional_storage())?;
        Ok(carriage)
    }
}

fn validate_configuration(profile: &Profile, policy: &Policy, b: &mut Budget<'_>) -> Result<()> {
    let floor = profile.retained_storage() + policy.retained_storage();
    b.with_prepaid_scope(floor, 0, LOCAL_WORK, FRAME, |b| {
        profile.revalidate(b)?;
        policy.revalidate(b)?;
        if profile.profile().policy().canonical_bytes() != policy.policy().canonical_bytes() {
            return Err(Failure::Mismatch(
                "inherited policy differs from retained profile",
            ));
        }
        Ok(())
    })
}

fn validate_readiness(
    profile: &Profile,
    policy: &Policy,
    child_pid: u32,
    manifest: &Manifest,
    readiness: &Ready,
    b: &mut Budget<'_>,
) -> Result<()> {
    let floor = profile.retained_storage()
        + policy.retained_storage()
        + manifest.retained_storage()
        + readiness.retained_storage();
    b.with_prepaid_scope(floor, 0, LOCAL_WORK, FRAME, |b| {
        validate_configuration(profile, policy, b)?;
        if child_pid == 0
            || manifest.client().pid() != child_pid
            || manifest.client().uid() == profile.profile().supervisor_uid()
            || manifest.external_anchor_service() != profile.profile().external_anchor_service()
            || !manifest.matches_policy(policy.policy(), b)?
            || !readiness.matches_launch(readiness.issuer_pid(), manifest, policy.policy(), b)?
        {
            return Err(Failure::Mismatch(
                "readiness differs from the selected child, anchor or policy",
            ));
        }
        Ok(())
    })
}

fn decode_receipt(
    profile: &Profile,
    subject: &Subject,
    bytes: &[u8],
    b: &mut Budget<'_>,
) -> Result<(Carriage, Storage)> {
    let floor = profile
        .retained_storage()
        .checked_add(SUBJECT_STORAGE)
        .and_then(|n| n.checked_add(bytes.len()))
        .ok_or(Resource::Arithmetic)?;
    b.with_prepaid_scope(floor, 0, LOCAL_WORK, FRAME, |b| {
        profile.revalidate(b)?;
        let (carriage, storage) = Carriage::decode(bytes, b)?;
        b.reserve_storage(storage.additional_storage())?;
        if carriage.policy().canonical_bytes() != profile.profile().policy().canonical_bytes()
            || carriage.request().subject().canonical_bytes() != subject.canonical_bytes()
        {
            return Err(Failure::Mismatch(
                "receipt differs from retained policy or exact subject",
            ));
        }
        profile.revalidate(b)?;
        Ok((carriage, storage))
    })
}

#[derive(Debug)]
pub(crate) enum Failure {
    Resource(Resource),
    Capability(CapabilityError),
    Supervisor(SupervisorError),
    Manifest(ManifestError),
    Ready(ReadyError),
    Handoff(HandoffError),
    Subject(SubjectError),
    Transport(TransportError),
    Receipt(ReceiptError),
    Mismatch(&'static str),
}
macro_rules! causes {
    ($($ty:ty => $variant:ident),+ $(,)?) => {
        $(impl From<$ty> for Failure { fn from(e: $ty) -> Self { Self::$variant(e) } })+
        impl fmt::Display for Failure { fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            match self { $(Self::$variant(e) => e.fmt(f),)+ Self::Mismatch(s) => f.write_str(s) }
        } }
        impl Error for Failure { fn source(&self) -> Option<&(dyn Error + 'static)> {
            match self { $(Self::$variant(e) => Some(e),)+ Self::Mismatch(_) => None }
        } }
    };
}
causes!(Resource=>Resource, CapabilityError=>Capability, SupervisorError=>Supervisor,
    ManifestError=>Manifest, ReadyError=>Ready, HandoffError=>Handoff, SubjectError=>Subject,
    TransportError=>Transport, ReceiptError=>Receipt);

#[cfg(test)]
#[path = "compiler_execution_boundary_native_tests.rs"]
mod tests;

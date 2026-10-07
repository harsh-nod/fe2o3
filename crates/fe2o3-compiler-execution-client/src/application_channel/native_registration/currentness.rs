//! The native client is invoked only with independently pinned registration policy.
use super::*;
use crate::CompilerExecutionClientV3 as Client;
use fe2o3_compiler_execution_protocol::{
    CompilerExecutionReceiptCarriageV3 as Carriage,
    VerifiedCompilerExecutionCurrentRecordV3 as Verified,
};
use fe2o3_runtime_protocol::NativeConditionalApplicationBindingV1 as Association;

/// Fresh native V3 exchange retaining original registered endpoint and deployment.
///
/// No bare cryptographic record or legacy client result can construct this owner.
/// The pinned deployment authenticates configured key provenance, while the
/// actual native exchange verifies fresh challenge, carriage and external anchor.
/// This is not a durable-publication lock, proof custodian, machine refinement,
/// continuous service-liveness lease, or a load/launch permit.
///
/// ```compile_fail
/// use fe2o3_compiler_execution_client::NativeApplicationCurrentRecordV1 as Native;
/// use fe2o3_compiler_execution_protocol::VerifiedCompilerExecutionCurrentRecordV3 as Bare;
/// fn promote<'work>(record: Bare) -> Native<'work> { record.into() }
/// ```
/// ```compile_fail
/// use fe2o3_compiler_execution_client::NativeApplicationCurrentRecordV1 as Native;
/// fn clone(record: Native<'_>) { let _ = record.clone(); }
/// ```
pub struct NativeApplicationCurrentRecordV1<'work> {
    registration: RegisteredNativeApplicationProofEndpointV1<'work>,
    verified: Verified,
}
impl fmt::Debug for NativeApplicationCurrentRecordV1<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("NativeApplicationCurrentRecordV1")
            .field("registration", &self.registration)
            .finish_non_exhaustive()
    }
}
impl<'work> NativeApplicationCurrentRecordV1<'work> {
    pub const fn verified(&self) -> &Verified {
        &self.verified
    }
    pub const fn registration(&self) -> &RegisteredNativeApplicationProofEndpointV1<'work> {
        &self.registration
    }
    pub fn retained_storage(&self) -> Result<usize> {
        sum(
            self.registration.retained_storage()?,
            size_of::<Self>() - size_of::<RegisteredNativeApplicationProofEndpointV1<'work>>(),
        )
    }
    /// Rechecks original installed configuration and registration/root liveness.
    /// It does not repeat or extend the challenge-bound native currentness exchange.
    pub fn revalidate_provenance(&self, budget: &mut Budget<'work>) -> Result<()> {
        if budget.storage() < self.retained_storage()? {
            return Err(Resource::Accounting.into());
        }
        self.registration.revalidate(budget)
    }
    pub const fn grants_load_authority(&self) -> bool {
        false
    }
    pub const fn grants_launch_authority(&self) -> bool {
        false
    }
}

impl<'work> RegisteredNativeApplicationProofEndpointV1<'work> {
    /// Consumes the canonical native current-record service FD and exact registration.
    ///
    /// Prepay this owner, actual carriage, exact original readiness bytes and
    /// `CompilerExecutionClientV3::PEER_STORAGE`. Success returns only growth over
    /// this owner, and the native client retires the consumed peer reservation.
    /// Failure closes the endpoint on every path and retains terminal local charges.
    /// No supplied `OwnedFd`, caller policy, legacy transport or bare record can
    /// replace this transition. One absolute deadline covers all checks and I/O.
    ///
    /// # Safety
    /// Transfer exclusive ownership of inherited FD195, without another Rust owner
    /// or outstanding borrow. No thread, signal handler or foreign code may close,
    /// replace or acquire that slot during this call. If absent, keep it absent.
    /// This consumes the transfer once, including resource refusal and unwind.
    pub unsafe fn verify_inherited_native_currentness(
        self,
        readiness: &[u8],
        carriage: &Carriage,
        deadline: Instant,
        budget: &mut Budget<'work>,
    ) -> Result<(
        NativeApplicationCurrentRecordV1<'work>,
        NativeApplicationChannelStorageV1,
    )> {
        // SAFETY: this method forwards the caller's exclusive inherited-slot transfer once.
        let (registration, verified) =
            unsafe { verify(self, readiness, carriage, deadline, budget) }?;
        let value = NativeApplicationCurrentRecordV1 {
            registration,
            verified,
        };
        let additional = value
            .retained_storage()?
            .checked_sub(value.registration.retained_storage()?)
            .ok_or(Resource::Arithmetic)?;
        Ok((value, NativeApplicationChannelStorageV1(additional)))
    }
}

pub(super) trait Source<'work>: Sized {
    fn binding(&self) -> &Binding;
    fn deployment(&self) -> &Deployment<'work>;
    fn retained_storage(&self) -> Result<usize>;
    fn revalidate(&self, budget: &mut Budget<'work>) -> Result<()>;
    fn require_currentness_ready(&self) -> Result<()> {
        Ok(())
    }
}
impl<'work> Source<'work> for RegisteredNativeApplicationProofEndpointV1<'work> {
    fn binding(&self) -> &Binding {
        &self.binding
    }
    fn deployment(&self) -> &Deployment<'work> {
        &self.deployment
    }
    fn retained_storage(&self) -> Result<usize> {
        self.retained_storage()
    }
    fn revalidate(&self, budget: &mut Budget<'work>) -> Result<()> {
        self.revalidate(budget)
    }
}

pub(super) unsafe fn verify<'work, P: Source<'work>>(
    source: P,
    readiness: &[u8],
    carriage: &Carriage,
    deadline: Instant,
    budget: &mut Budget<'work>,
) -> Result<(P, Verified)> {
    // SAFETY: the caller transfers the canonical slot on every exit, before any
    // resource or provenance check can fail. This guard closes it on unwind.
    let pending = unsafe { crate::inherited_admission::PendingInheritedPeer::new() };
    let floor = budget.storage();
    let ledger = budget.work_ledger_identity_v1();
    let account = budget.storage_account_identity_v1();
    budget.charge_work(ATTEMPT_WORK)?;
    let retained = source.retained_storage()?;
    let required = sum(
        retained,
        sum(
            carriage.retained_storage(),
            sum(readiness.len(), Client::PEER_STORAGE)?,
        )?,
    )?;
    if floor < required {
        return Err(Resource::Accounting.into());
    }
    budget.reserve_storage(IO_SCRATCH)?;
    source.require_currentness_ready()?;
    transport::check_deadline(deadline)?;
    source.revalidate(budget)?;
    let association = Association::bind(
        readiness,
        source.binding().compiler_handoff(),
        carriage,
        budget,
    )
    .map_err(NativeApplicationChannelErrorV1::Association)?;
    budget.reserve_storage(size_of::<Association>())?;
    if association.canonical_bytes() != source.binding().association().canonical_bytes()
        || carriage.policy().canonical_bytes() != source.deployment().policy().canonical_bytes()
    {
        return Err(NativeApplicationChannelErrorV1::Binding(
            "original native readiness or policy",
        ));
    }
    let timeout = deadline
        .saturating_duration_since(Instant::now())
        .min(Duration::from_secs(30));
    let peer = pending
        .retain()
        .map_err(|error| NativeApplicationChannelErrorV1::Client(error.into()))?;
    let client =
        Client::admit(peer, timeout, budget).map_err(NativeApplicationChannelErrorV1::Client)?;
    let (verified, charge) = client
        .verify_current_only(source.deployment().policy(), carriage)
        .map_err(NativeApplicationChannelErrorV1::Client)?;
    budget.reserve_storage(charge.additional_storage())?;
    source.revalidate(budget)?;
    transport::check_deadline(deadline)?;
    if ledger != budget.work_ledger_identity_v1() || account != budget.storage_account_identity_v1()
    {
        return Err(Resource::Accounting.into());
    }
    let destination_floor = floor
        .checked_sub(Client::PEER_STORAGE)
        .ok_or(Resource::Accounting)?;
    budget.release_storage(
        budget
            .storage()
            .checked_sub(destination_floor)
            .ok_or(Resource::Accounting)?,
    )?;
    Ok((source, verified))
}

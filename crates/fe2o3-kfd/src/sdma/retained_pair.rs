//! Explicit ordinary-lifetime peer copies, not fresh whole-host observations.

use super::*;
use crate::SharedMemorySessionPhaseV1;
use std::panic::{AssertUnwindSafe, catch_unwind, resume_unwind};

/// Identifies the narrower observation contract; it grants no device authority.
pub const GFX942_XGMI_RETAINED_PAIR_PROFILE_V1: &str =
    "fe2o3.gfx942-xgmi-retained-pair-ordinary-lifetime.v1";
pub const GFX942_XGMI_RETAINED_PAIR_POLICY_V1: &str = include_str!("retained_pair_policy_v1.txt");
pub const GFX942_XGMI_RETAINED_PAIR_POLICY_SHA256_V1: &str =
    "18cfe1c56d270d9cab1cdc2f67a2b26b35e7cf1540a2962e4dc2f5cb42155b61";

/// An explicit external assumption, not a detected or authenticated driver.
///
/// The reviewed installation used Linux 6.8.0-124 and AMDGPU DKMS 6.16.13 on
/// MI300X. The caller must independently qualify that environment contract.
/// No administrative repartition, hive reconfiguration, hotplug, privileged
/// CRIU, or foreign same-process raw KFD/DRM mutation is allowed while active.
/// This value grants no allocation, mapping, queue, or completion authority.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Gfx942XgmiRetainedPairEnvironmentAssumptionV1 {
    ReviewedMi300xAmdgpu61613OrdinaryLifetime,
}

/// A copy whose exact completion and both closing operational fences succeeded.
///
/// No fresh whole-host topology observation or all-reset proof is implied.
/// Scope finish is not a deferred witness for this result.
///
/// ```no_run
/// use fe2o3_kfd::{Gfx942XgmiCompletedCopyV1, Gfx942XgmiRetainedPairCompletedCopyV1,
///     Gfx942XgmiMappedDeviceMemoryV1};
/// fn old_stays_old(value: Gfx942XgmiCompletedCopyV1) -> Gfx942XgmiCompletedCopyV1 { value }
/// fn retained_public_access(value: Gfx942XgmiRetainedPairCompletedCopyV1)
///     -> (Gfx942XgmiMappedDeviceMemoryV1, Gfx942XgmiMappedDeviceMemoryV1) {
///     let _ = (value.profile(), value.copy_bytes());
///     value.into_mappings()
/// }
/// ```
///
/// ```compile_fail,E0308
/// use fe2o3_kfd::{Gfx942XgmiCompletedCopyV1, Gfx942XgmiRetainedPairCompletedCopyV1};
/// fn cannot_relabel(value: Gfx942XgmiRetainedPairCompletedCopyV1) -> Gfx942XgmiCompletedCopyV1 {
///     value
/// }
/// ```
///
/// ```compile_fail,E0616
/// use fe2o3_kfd::{Gfx942XgmiCompletedCopyV1, Gfx942XgmiRetainedPairCompletedCopyV1};
/// fn cannot_unwrap(value: Gfx942XgmiRetainedPairCompletedCopyV1) -> Gfx942XgmiCompletedCopyV1 {
///     value.inner
/// }
/// ```
pub struct Gfx942XgmiRetainedPairCompletedCopyV1 {
    inner: Gfx942XgmiCompletedCopyV1,
}

impl Gfx942XgmiRetainedPairCompletedCopyV1 {
    pub const fn profile(&self) -> &'static str {
        GFX942_XGMI_RETAINED_PAIR_PROFILE_V1
    }

    pub const fn copy_bytes(&self) -> u32 {
        self.inner.copy_bytes()
    }

    pub fn into_mappings(
        self,
    ) -> (Gfx942XgmiMappedDeviceMemoryV1, Gfx942XgmiMappedDeviceMemoryV1) {
        self.inner.into_mappings()
    }
}

/// A fully completed roster under the explicitly retained-pair profile.
pub struct Gfx942XgmiRetainedPairCompletedBatchV1 {
    inner: Vec<Gfx942XgmiCompletedCopyV1>,
}

impl Gfx942XgmiRetainedPairCompletedBatchV1 {
    pub const fn profile(&self) -> &'static str {
        GFX942_XGMI_RETAINED_PAIR_PROFILE_V1
    }

    pub fn len(&self) -> usize {
        self.inner.len()
    }

    pub fn is_empty(&self) -> bool {
        self.inner.is_empty()
    }

    pub fn into_copies(self) -> impl ExactSizeIterator<Item = Gfx942XgmiRetainedPairCompletedCopyV1> {
        self.inner.into_iter().map(|inner| Gfx942XgmiRetainedPairCompletedCopyV1 { inner })
    }
}

impl fmt::Debug for Gfx942XgmiRetainedPairCompletedBatchV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.debug_struct("Gfx942XgmiRetainedPairCompletedBatchV1")
            .field("profile", &self.profile()).field("copies", &self.len()).finish()
    }
}

/// Pending tickets or unconfirmed mappings, never a full-fresh completion.
#[must_use = "recover the exact tickets or retain indeterminate mappings under terminal custody"]
pub struct Gfx942XgmiRetainedPairWaitFailureV1 {
    inner: Gfx942XgmiBatchWaitFailureV1,
}

impl Gfx942XgmiRetainedPairWaitFailureV1 {
    pub const fn error(&self) -> &Gfx942SdmaErrorV1 {
        self.inner.error()
    }

    pub fn into_retained_tickets(self) -> Option<Vec<Gfx942SdmaCopyTicketV1>> {
        self.inner.into_retained_tickets()
    }

    /// These mappings do not authorize reuse, release, or successful completion.
    /// The queue and both endpoint sessions have already been quarantined.
    pub fn into_indeterminate_mappings(self) -> Option<impl ExactSizeIterator<Item = (
        Gfx942XgmiMappedDeviceMemoryV1, Gfx942XgmiMappedDeviceMemoryV1,
    )>> {
        self.inner.into_indeterminate_completions()
            .map(|copies| copies.into_iter().map(Gfx942XgmiCompletedCopyV1::into_mappings))
    }
}

impl fmt::Debug for Gfx942XgmiRetainedPairWaitFailureV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.debug_struct("Gfx942XgmiRetainedPairWaitFailureV1")
            .field("profile", &GFX942_XGMI_RETAINED_PAIR_PROFILE_V1)
            .field("error", self.error()).finish_non_exhaustive()
    }
}

impl fmt::Display for Gfx942XgmiRetainedPairWaitFailureV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.error().fmt(formatter)
    }
}

impl std::error::Error for Gfx942XgmiRetainedPairWaitFailureV1 {}

// Private injection boundary for CPU tests, not a public authority provider.
trait Custody {
    fn terminal(&self) -> bool;
    fn quarantine(&mut self);
}

trait TerminalOutcome {
    fn refuse_terminal_success(self) -> Self;
}

enum Settled<R, P> {
    Return(R),
    Resume(P),
}

include!("retained_pair_operation_body.rs");

fn terminal_success_error() -> Gfx942SdmaErrorV1 {
    Gfx942SdmaErrorV1::Contract("retained XGMI success has terminal custody")
}

impl TerminalOutcome for Result<(), Gfx942SdmaErrorV1> {
    fn refuse_terminal_success(self) -> Self {
        retained_pair_unit_outcome_body!(self)
    }
}

impl TerminalOutcome for Result<Vec<Gfx942SdmaCopyTicketV1>, Gfx942XgmiBatchSubmissionFailureV1> {
    fn refuse_terminal_success(self) -> Self {
        retained_pair_tickets_outcome_body!(self)
    }
}

impl TerminalOutcome for Result<Vec<Gfx942XgmiCompletedCopyV1>, Gfx942XgmiBatchWaitFailureV1> {
    fn refuse_terminal_success(self) -> Self {
        retained_pair_completed_outcome_body!(self)
    }
}

fn settle_operation<C: Custody, R: TerminalOutcome, P>(context: &mut C, outcome: Result<R, P>) -> Settled<R, P> {
    retained_pair_settle_body!(context, outcome)
}

fn run_operation<C: Custody, R: TerminalOutcome>(context: &mut C, operation: impl FnOnce(&mut C) -> R) -> R {
    retained_pair_operation_body!(context, operation)
}

struct Scope<C: Custody> {
    context: C,
    finished: bool,
}

impl<C: Custody> Scope<C> {
    fn finish(mut self, close: impl FnOnce(&mut C) -> Result<(), Gfx942SdmaErrorV1>) -> Result<(), Gfx942SdmaErrorV1> {
        retained_pair_close_body!(self, close)
    }

    fn finish_terminal(mut self) {
        retained_pair_finish_terminal_body!(self)
    }
}

impl<C: Custody> Drop for Scope<C> {
    fn drop(&mut self) {
        retained_pair_drop_body!(self)
    }
}

struct Pair<'a> {
    queue: &'a mut Gfx942NativeXgmiSdmaQueueV1,
    source: &'a mut SharedGttMemorySessionV1,
    destination: &'a mut SharedGttMemorySessionV1,
}

impl Custody for Pair<'_> {
    fn terminal(&self) -> bool {
        retained_pair_terminal_body!(self)
    }

    fn quarantine(&mut self) {
        self.queue.quarantine_batch_v1(self.source, self.destination);
    }
}

fn require_drained(owner: &Gfx942SdmaQueueOwnerV1) -> Result<(), Gfx942SdmaErrorV1> {
    owner.require_live()?;
    let slots = GFX942_SDMA_RING_SLOT_COUNT_V1;
    if owner.records.len() != slots
        || owner.xgmi_records.len() != slots
        || owner.persistent_window_slots.len() != slots
        || owner.persistent_window_records.len() != slots
        || owner.uncertain_xgmi_ticket.is_some()
    {
        return Err(Gfx942SdmaErrorV1::Contract("retained XGMI queue roster or uncertainty"));
    }
    if owner.records.iter().any(Option::is_some)
        || owner.xgmi_records.iter().any(Option::is_some)
        || owner.persistent_window_slots.iter().any(Option::is_some)
        || owner.persistent_window_records.iter().any(Option::is_some)
    {
        return Err(Gfx942SdmaErrorV1::Pending);
    }
    Ok(())
}

fn required_resources(
    owner: &Gfx942SdmaQueueOwnerV1,
    engine: u32,
) -> Result<(&SdmaRingAuthorityV1, &SdmaControlAuthorityV1, &MappedHostBufferV1), Gfx942SdmaErrorV1> {
    if owner.engine_index != Some(engine) || owner.doorbell.is_none() {
        return Err(Gfx942SdmaErrorV1::Contract("retained XGMI engine or doorbell binding"));
    }
    Ok((
        owner.ring.as_ref().ok_or(Gfx942SdmaErrorV1::Contract("missing XGMI SDMA ring authority"))?,
        owner.control.as_ref().ok_or(Gfx942SdmaErrorV1::Contract("missing XGMI SDMA control authority"))?,
        owner.completions.as_ref().ok_or(Gfx942SdmaErrorV1::Contract("missing XGMI SDMA completion arena"))?,
    ))
}

impl Pair<'_> {
    fn require_drained(&self) -> Result<(), Gfx942SdmaErrorV1> {
        self.queue.require_live_queue_state_v1()?;
        require_drained(self.queue.owner.as_ref()
            .ok_or(Gfx942SdmaErrorV1::Contract("missing XGMI SDMA queue owner"))?)
    }

    fn admit_binding(&self) -> Result<(), Gfx942SdmaErrorV1> {
        self.require_drained()?;
        let owner = self.queue.owner.as_ref()
            .ok_or(Gfx942SdmaErrorV1::Contract("missing XGMI SDMA queue owner"))?;
        self.source.validate_gfx942_retained_pair_binding_v1(
            self.destination, self.queue.route, owner.owner,
        )?;
        let (ring, control, completions) = required_resources(owner, self.queue.route.recommended_engine_id())?;
        self.source.validate_retained_queue_resource_v1(ring)?;
        self.source.validate_retained_queue_resource_v1(control)?;
        self.source.mapped_resource_facts(completions)?;
        Ok(())
    }

    fn operational(&mut self) -> Result<(), Gfx942SdmaErrorV1> {
        self.source.validate_gfx942_retained_pair_operational_v1(self.destination, self.queue.route)?;
        Ok(())
    }
}

/// Opt-in Linux/gfx942 ordinary-lifetime copying through held kernel objects.
///
/// Entry performs full admission. Each operation independently closes BOTH
/// endpoint operational reset/VRAM checks before returning a conclusive result.
/// It does not rediscover whole-host topology for each operation or at finish.
///
/// Both sessions and the queue remain exclusively borrowed. Pending records own
/// their exact mappings. Forgetting this scope cannot release pending resources
/// or undo eager terminalization; prior successes need no later finish witness.
/// Dropping an unfinished scope quarantines the queue, both sessions and gate.
///
/// The external ordinary-driver-lifetime contract excludes administrative
/// hotplug/hive reconfiguration, privileged CRIU, and foreign same-process
/// KFD/DRM mutation. Kernel/firmware correctness and existing reset-subscription
/// gaps remain trusted limits, not universal reset or hotplug coverage.
///
/// The endpoint and queue borrows last through the final scope use:
///
/// ```no_run
/// use fe2o3_kfd::{Gfx942NativeXgmiSdmaQueueV1, SharedGttMemorySessionV1,
///     Gfx942XgmiRetainedPairEnvironmentAssumptionV1 as Environment};
/// fn finished_borrows(queue: &mut Gfx942NativeXgmiSdmaQueueV1,
///     source: &mut SharedGttMemorySessionV1, peer: &mut SharedGttMemorySessionV1) {
///     let scope = queue.begin_ordinary_retained_pair_v1(source, peer,
///         Environment::ReviewedMi300xAmdgpu61613OrdinaryLifetime).unwrap();
///     scope.finish().unwrap();
///     let _ = (queue.observation(), source.phase(), peer.phase());
///     queue.destroy_and_release(source, peer).unwrap();
/// }
/// ```
///
/// ```compile_fail,E0502
/// use fe2o3_kfd::{Gfx942NativeXgmiSdmaQueueV1, SharedGttMemorySessionV1,
///     Gfx942XgmiRetainedPairEnvironmentAssumptionV1 as Environment};
/// fn cannot_reborrow_queue(queue: &mut Gfx942NativeXgmiSdmaQueueV1,
///     source: &mut SharedGttMemorySessionV1, peer: &mut SharedGttMemorySessionV1) {
///     let scope = queue.begin_ordinary_retained_pair_v1(source, peer,
///         Environment::ReviewedMi300xAmdgpu61613OrdinaryLifetime).unwrap();
///     let _ = queue.observation();
///     scope.finish().unwrap();
/// }
/// ```
///
/// ```compile_fail,E0502
/// use fe2o3_kfd::{Gfx942NativeXgmiSdmaQueueV1, SharedGttMemorySessionV1,
///     Gfx942XgmiRetainedPairEnvironmentAssumptionV1 as Environment};
/// fn cannot_reborrow_source(queue: &mut Gfx942NativeXgmiSdmaQueueV1,
///     source: &mut SharedGttMemorySessionV1, peer: &mut SharedGttMemorySessionV1) {
///     let scope = queue.begin_ordinary_retained_pair_v1(source, peer,
///         Environment::ReviewedMi300xAmdgpu61613OrdinaryLifetime).unwrap();
///     let _ = source.phase();
///     scope.finish().unwrap();
/// }
/// ```
///
/// ```compile_fail,E0502
/// use fe2o3_kfd::{Gfx942NativeXgmiSdmaQueueV1, SharedGttMemorySessionV1,
///     Gfx942XgmiRetainedPairEnvironmentAssumptionV1 as Environment};
/// fn cannot_reborrow_peer(queue: &mut Gfx942NativeXgmiSdmaQueueV1,
///     source: &mut SharedGttMemorySessionV1, peer: &mut SharedGttMemorySessionV1) {
///     let scope = queue.begin_ordinary_retained_pair_v1(source, peer,
///         Environment::ReviewedMi300xAmdgpu61613OrdinaryLifetime).unwrap();
///     let _ = peer.phase();
///     scope.finish().unwrap();
/// }
/// ```
#[must_use = "finish a drained retained pair or explicitly quarantine it"]
pub struct Gfx942NativeXgmiSdmaRetainedPairV1<'a> {
    scope: Scope<Pair<'a>>,
}

impl Gfx942NativeXgmiSdmaQueueV1 {
    /// Starts the separately named retained profile without changing default APIs.
    /// No implicit fallback from a failed full-fresh operation is performed.
    pub fn begin_ordinary_retained_pair_v1<'a>(
        &'a mut self,
        source: &'a mut SharedGttMemorySessionV1,
        destination: &'a mut SharedGttMemorySessionV1,
        _environment_assumption: Gfx942XgmiRetainedPairEnvironmentAssumptionV1,
    ) -> Result<Gfx942NativeXgmiSdmaRetainedPairV1<'a>, Gfx942SdmaErrorV1> {
        let mut pair = Pair { queue: self, source, destination };
        pair.admit_binding()?;
        run_operation(&mut pair, |pair| {
            pair.source.validate_gfx942_xgmi_route_with_peer(pair.destination, pair.queue.route)?;
            pair.queue.require_live_queue_state_v1()
        })?;
        Ok(Gfx942NativeXgmiSdmaRetainedPairV1 { scope: Scope { context: pair, finished: false } })
    }
}

impl Gfx942NativeXgmiSdmaRetainedPairV1<'_> {
    pub const fn profile(&self) -> &'static str {
        GFX942_XGMI_RETAINED_PAIR_PROFILE_V1
    }

    pub fn submit_batch(
        &mut self,
        requests: Vec<Gfx942XgmiSdmaCopyRequestV1>,
    ) -> Result<Vec<Gfx942SdmaCopyTicketV1>, Gfx942XgmiBatchSubmissionFailureV1> {
        run_operation(&mut self.scope.context, |pair| pair.queue.submit_batch_with_currentness(
            pair.source, pair.destination, requests, XgmiRouteCurrentnessV1::OrdinaryRetainedPair,
        ))
    }

    /// Uses the original deadline; timeout retains the exact queue-owned roster.
    pub fn wait_batch_until(
        &mut self,
        tickets: Vec<Gfx942SdmaCopyTicketV1>,
        deadline: Instant,
    ) -> Result<Gfx942XgmiRetainedPairCompletedBatchV1, Gfx942XgmiRetainedPairWaitFailureV1> {
        self.wait(tickets, XgmiBatchDeadlineV1::Absolute(deadline))
    }

    pub fn wait_batch_for(
        &mut self,
        tickets: Vec<Gfx942SdmaCopyTicketV1>,
        timeout: Duration,
    ) -> Result<Gfx942XgmiRetainedPairCompletedBatchV1, Gfx942XgmiRetainedPairWaitFailureV1> {
        self.wait(tickets, XgmiBatchDeadlineV1::Relative(timeout))
    }

    fn wait(
        &mut self,
        tickets: Vec<Gfx942SdmaCopyTicketV1>,
        deadline: XgmiBatchDeadlineV1,
    ) -> Result<Gfx942XgmiRetainedPairCompletedBatchV1, Gfx942XgmiRetainedPairWaitFailureV1> {
        run_operation(&mut self.scope.context, |pair| pair.queue.wait_batch_for_with_currentness(
            pair.source, pair.destination, tickets, deadline,
            XgmiRouteCurrentnessV1::OrdinaryRetainedPair,
        ))
        .map(|inner| Gfx942XgmiRetainedPairCompletedBatchV1 { inner })
        .map_err(|inner| Gfx942XgmiRetainedPairWaitFailureV1 { inner })
    }

    /// Ends a drained scope with operational checks, not fresh host discovery.
    /// Consuming finish with pending work or a failed check quarantines custody.
    pub fn finish(self) -> Result<(), Gfx942SdmaErrorV1> {
        self.scope.finish(|pair| {
            pair.require_drained()?;
            pair.operational()
        })
    }

    /// Irreversibly quarantines this scope; grants no cleanup authority.
    pub fn finish_terminal(self) {
        self.scope.finish_terminal();
    }
}

#[cfg(test)]
mod tests;

//! Exact-owner V22 entry into the shared budgeted Engine capture.
use super::*;
use fe2o3_kernel_ir::{
    CanonicalKernelIrOwnedVerificationResourceBudgetV1 as Owned,
    CanonicalKernelIrVerificationResourceErrorV1 as Resource, VerifiedCanonicalKernelIrModuleV22,
};

pub const MAX_PHYSICAL_LDS_EXCHANGE_DEBUG_RECORDS_V22: usize = MAX_PHYSICAL_ENTRY_DEBUG_RECORDS_V20;
/// Profile-independent accounting/outcome vocabulary is intentionally shared.
pub type PhysicalLdsExchangeDebugOutcomeV22 = PhysicalEntryDebugOutcomeV20;
pub type PhysicalLdsExchangeDebugCaptureStopV22 = PhysicalEntryDebugCaptureStopV20;
pub type PhysicalLdsExchangeDebugUsageV22 = PhysicalEntryDebugUsageV20;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PhysicalLdsExchangeDebugOptionsV22(PhysicalEntryDebugOptionsV20);
impl PhysicalLdsExchangeDebugOptionsV22 {
    pub fn new(
        simulation: SimulationLimitsV1,
        capture: SimulationDebugCaptureLimitsV1,
        records: usize,
    ) -> Option<Self> {
        PhysicalEntryDebugOptionsV20::new(simulation, capture, records).map(Self)
    }
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PhysicalLdsExchangeDebugCaptureErrorV22 {
    OwnerMismatch,
    NotPhysicalLdsExchange,
    Resource(Resource),
}

/// Move-only CPU observations retain the original cumulative ledger. No raw
/// transcript/source-owner constructor or resumable interpreter state is exposed.
/// Pending write queues and per-byte publication/writer metadata are not projected;
/// these snapshots cannot resume execution or discharge hardware synchronization.
/// Numeric pending-load values are unavailable until the real Engine wait resolves
/// the same SSA binding. Allocation snapshots are logical CPU memory, not GPU reads.
///
/// ```compile_fail
/// use fe2o3_kir_sim::PhysicalLdsExchangeDebugCaptureV22;
/// fn clone_capture(value: PhysicalLdsExchangeDebugCaptureV22) { let _ = value.clone(); }
/// ```
pub struct PhysicalLdsExchangeDebugCaptureV22 {
    inner: PhysicalEntryDebugCaptureV20,
}
impl PhysicalLdsExchangeDebugCaptureV22 {
    pub const fn identity(&self) -> SimulationKernelIrIdentityV1 {
        self.inner.identity()
    }
    pub const fn outcome(&self) -> PhysicalLdsExchangeDebugOutcomeV22 {
        self.inner.outcome()
    }
    pub const fn error(&self) -> Option<PhysicalLdsExchangeDebugCaptureErrorV22> {
        use PhysicalLdsExchangeDebugCaptureErrorV22 as E;
        match self.inner.error() {
            None => None,
            Some(PhysicalEntryDebugCaptureErrorV20::OwnerMismatch) => Some(E::OwnerMismatch),
            Some(PhysicalEntryDebugCaptureErrorV20::NotPhysicalEntry) => {
                Some(E::NotPhysicalLdsExchange)
            }
            Some(PhysicalEntryDebugCaptureErrorV20::Resource(error)) => Some(E::Resource(error)),
        }
    }
    pub const fn stop(&self) -> Option<PhysicalLdsExchangeDebugCaptureStopV22> {
        self.inner.stop()
    }
    pub fn len(&self) -> usize {
        self.inner.len()
    }
    pub fn is_empty(&self) -> bool {
        self.inner.is_empty()
    }
    pub fn record(&self, index: usize) -> Option<PhysicalLdsExchangeDebugRecordRefV22<'_>> {
        Some(PhysicalLdsExchangeDebugRecordRefV22 {
            record: self.inner.record(index)?,
        })
    }
    pub fn usage(&self) -> PhysicalLdsExchangeDebugUsageV22 {
        self.inner.usage()
    }
    pub fn charge_navigation(&mut self, work: usize) -> Result<(), Resource> {
        self.inner.charge_navigation(work)
    }
    /// Drops retained payloads before returning the same ledger at its original
    /// floor; cumulative work, peak and denial history are never reset.
    pub fn into_budget(self) -> Owned {
        self.inner.into_budget()
    }
}
impl AdmittedSimulationModuleV1 {
    /// Exact V22 typed owner only; uses existing preflight, alias/initialization
    /// checks, Engine and real LGKM/VM readiness. Legacy debug entrypoints remain
    /// refused. The caller's existing storage floor remains caller-owned.
    pub fn capture_physical_lds_exchange_debug_v22(
        &self,
        canonical: &VerifiedCanonicalKernelIrModuleV22,
        request: &SimulationRequestV1,
        options: PhysicalLdsExchangeDebugOptionsV22,
        ledger: Owned,
    ) -> PhysicalLdsExchangeDebugCaptureV22 {
        PhysicalLdsExchangeDebugCaptureV22 {
            inner: self.capture_physical_debug_owned(
                debug_physical_capture_v20::CanonicalOwner::LdsExchange(canonical),
                request,
                options.0,
                ledger,
            ),
        }
    }
}

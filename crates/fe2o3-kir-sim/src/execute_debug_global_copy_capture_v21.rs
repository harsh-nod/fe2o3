//! Exact-owner V21 entry into the shared budgeted Engine capture.
use super::*;
use fe2o3_kernel_ir::{
    CanonicalKernelIrOwnedVerificationResourceBudgetV1 as Owned,
    CanonicalKernelIrVerificationResourceErrorV1 as Resource, VerifiedCanonicalKernelIrModuleV21,
};

pub const MAX_PHYSICAL_GLOBAL_COPY_DEBUG_RECORDS_V21: usize = MAX_PHYSICAL_ENTRY_DEBUG_RECORDS_V20;
/// Profile-independent accounting/outcome vocabulary is intentionally shared.
pub type PhysicalGlobalCopyDebugOutcomeV21 = PhysicalEntryDebugOutcomeV20;
pub type PhysicalGlobalCopyDebugCaptureStopV21 = PhysicalEntryDebugCaptureStopV20;
pub type PhysicalGlobalCopyDebugUsageV21 = PhysicalEntryDebugUsageV20;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PhysicalGlobalCopyDebugOptionsV21(PhysicalEntryDebugOptionsV20);
impl PhysicalGlobalCopyDebugOptionsV21 {
    pub fn new(
        simulation: SimulationLimitsV1,
        capture: SimulationDebugCaptureLimitsV1,
        records: usize,
    ) -> Option<Self> {
        PhysicalEntryDebugOptionsV20::new(simulation, capture, records).map(Self)
    }
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PhysicalGlobalCopyDebugCaptureErrorV21 {
    OwnerMismatch,
    NotPhysicalGlobalCopy,
    Resource(Resource),
}

/// Move-only CPU observations retain the original cumulative ledger. No raw
/// transcript/source-owner constructor or resumable interpreter state is exposed.
/// Numeric pending-load values are unavailable until the real Engine wait resolves
/// the same SSA binding. Allocation snapshots are logical CPU memory, not GPU reads.
///
/// ```compile_fail
/// use fe2o3_kir_sim::PhysicalGlobalCopyDebugCaptureV21;
/// fn clone_capture(value: PhysicalGlobalCopyDebugCaptureV21) { let _ = value.clone(); }
/// ```
pub struct PhysicalGlobalCopyDebugCaptureV21 {
    inner: PhysicalEntryDebugCaptureV20,
}
impl PhysicalGlobalCopyDebugCaptureV21 {
    pub const fn identity(&self) -> SimulationKernelIrIdentityV1 {
        self.inner.identity()
    }
    pub const fn outcome(&self) -> PhysicalGlobalCopyDebugOutcomeV21 {
        self.inner.outcome()
    }
    pub const fn error(&self) -> Option<PhysicalGlobalCopyDebugCaptureErrorV21> {
        use PhysicalGlobalCopyDebugCaptureErrorV21 as E;
        match self.inner.error() {
            None => None,
            Some(PhysicalEntryDebugCaptureErrorV20::OwnerMismatch) => Some(E::OwnerMismatch),
            Some(PhysicalEntryDebugCaptureErrorV20::NotPhysicalEntry) => {
                Some(E::NotPhysicalGlobalCopy)
            }
            Some(PhysicalEntryDebugCaptureErrorV20::Resource(error)) => Some(E::Resource(error)),
        }
    }
    pub const fn stop(&self) -> Option<PhysicalGlobalCopyDebugCaptureStopV21> {
        self.inner.stop()
    }
    pub fn len(&self) -> usize {
        self.inner.len()
    }
    pub fn is_empty(&self) -> bool {
        self.inner.is_empty()
    }
    pub fn record(&self, index: usize) -> Option<PhysicalGlobalCopyDebugRecordRefV21<'_>> {
        Some(PhysicalGlobalCopyDebugRecordRefV21 {
            record: self.inner.record(index)?,
        })
    }
    pub fn usage(&self) -> PhysicalGlobalCopyDebugUsageV21 {
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
    /// Exact V21 typed owner only; uses existing preflight, alias/initialization
    /// checks, Engine and real LGKM/VM readiness. Legacy debug entrypoints remain
    /// refused. The caller's existing storage floor remains caller-owned.
    pub fn capture_physical_global_copy_debug_v21(
        &self,
        canonical: &VerifiedCanonicalKernelIrModuleV21,
        request: &SimulationRequestV1,
        options: PhysicalGlobalCopyDebugOptionsV21,
        ledger: Owned,
    ) -> PhysicalGlobalCopyDebugCaptureV21 {
        PhysicalGlobalCopyDebugCaptureV21 {
            inner: self.capture_physical_debug_owned(
                debug_physical_capture_v20::CanonicalOwner::GlobalCopy(canonical),
                request,
                options.0,
                ledger,
            ),
        }
    }
}

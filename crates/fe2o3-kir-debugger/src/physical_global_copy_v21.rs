//! Move-only cursor over exact-owner V21 CPU checkpoints.
//! Reverse navigation selects retained observations; it cannot resume execution.
use crate::physical_cursor_v1::Cursor;
use fe2o3_kernel_ir::{
    CanonicalKernelIrOwnedVerificationResourceBudgetV1,
    CanonicalKernelIrVerificationResourceErrorV1, VerifiedCanonicalKernelIrModuleV21,
};
use fe2o3_kir_sim::{
    AdmittedSimulationModuleV1, PhysicalGlobalCopyDebugCaptureErrorV21,
    PhysicalGlobalCopyDebugCaptureStopV21, PhysicalGlobalCopyDebugCaptureV21,
    PhysicalGlobalCopyDebugOptionsV21, PhysicalGlobalCopyDebugOutcomeV21,
    PhysicalGlobalCopyDebugRecordRefV21, PhysicalGlobalCopyDebugUsageV21, SimulationRequestV1,
};
pub type PhysicalGlobalCopyDebugNavigationV21 = crate::PhysicalEntryDebugNavigationV20;

/// No constructor accepts raw snapshots, transcripts, hashes or source metadata.
/// Both read-only input and output allocations remain immutable CPU observations.
pub struct PhysicalGlobalCopyDebugSessionV21 {
    capture: PhysicalGlobalCopyDebugCaptureV21,
    cursor: Cursor,
}
impl PhysicalGlobalCopyDebugSessionV21 {
    pub fn capture(
        module: &AdmittedSimulationModuleV1,
        owner: &VerifiedCanonicalKernelIrModuleV21,
        request: &SimulationRequestV1,
        options: PhysicalGlobalCopyDebugOptionsV21,
        ledger: CanonicalKernelIrOwnedVerificationResourceBudgetV1,
    ) -> Self {
        Self {
            capture: module.capture_physical_global_copy_debug_v21(owner, request, options, ledger),
            cursor: Cursor::default(),
        }
    }
    pub fn usage(&self) -> PhysicalGlobalCopyDebugUsageV21 {
        self.capture.usage()
    }
    pub fn identity(&self) -> fe2o3_kir_sim::SimulationKernelIrIdentityV1 {
        self.capture.identity()
    }
    pub fn outcome(&self) -> PhysicalGlobalCopyDebugOutcomeV21 {
        self.capture.outcome()
    }
    pub fn charge_query_work(
        &mut self,
        work: usize,
    ) -> Result<(), CanonicalKernelIrVerificationResourceErrorV1> {
        self.capture.charge_navigation(work)
    }
    pub fn records_len(&self) -> usize {
        self.capture.len()
    }
    pub fn cursor(&self) -> Option<usize> {
        self.cursor.index()
    }
    pub fn record(&self, index: usize) -> Option<PhysicalGlobalCopyDebugRecordRefV21<'_>> {
        self.capture.record(index)
    }
    pub fn current(&self) -> Option<PhysicalGlobalCopyDebugRecordRefV21<'_>> {
        self.capture.record(self.cursor.index()?)
    }
    pub fn capture_error(&self) -> Option<PhysicalGlobalCopyDebugCaptureErrorV21> {
        self.capture.error()
    }
    pub fn capture_stop(&self) -> Option<PhysicalGlobalCopyDebugCaptureStopV21> {
        self.capture.stop()
    }
    pub fn rewind(&mut self) -> PhysicalGlobalCopyDebugNavigationV21 {
        self.cursor.rewind(&mut self.capture)
    }
    pub fn seek(&mut self, index: usize) -> PhysicalGlobalCopyDebugNavigationV21 {
        self.cursor.seek(&mut self.capture, index)
    }
    pub fn step_forward(&mut self) -> PhysicalGlobalCopyDebugNavigationV21 {
        self.cursor.forward(&mut self.capture)
    }
    pub fn step_reverse(&mut self) -> PhysicalGlobalCopyDebugNavigationV21 {
        self.cursor.reverse(&mut self.capture)
    }
    pub fn into_budget(self) -> CanonicalKernelIrOwnedVerificationResourceBudgetV1 {
        self.capture.into_budget()
    }
}

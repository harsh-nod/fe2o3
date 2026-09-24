//! Move-only cursor over actual budget-owned V20 CPU checkpoints.
//! Navigation selects prior observations; it cannot restore/resume an Engine.
use crate::physical_cursor_v1::Cursor;
use fe2o3_kernel_ir::{
    CanonicalKernelIrOwnedVerificationResourceBudgetV1, VerifiedCanonicalKernelIrModuleV20,
};
use fe2o3_kir_sim::{
    AdmittedSimulationModuleV1, PhysicalEntryDebugCaptureV20, PhysicalEntryDebugOptionsV20,
    PhysicalEntryDebugOutcomeV20, PhysicalEntryDebugRecordRefV20, PhysicalEntryDebugUsageV20,
    SimulationRequestV1,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PhysicalEntryDebugNavigationV20 {
    Beginning,
    Record { index: usize, ordinal: u64 },
    End,
    Incomplete,
    Unavailable,
}

/// The capture and its ledger stay inseparable for the complete session lifetime.
/// No constructor accepts records, hashes, a caller transcript, or source metadata.
pub struct PhysicalEntryDebugSessionV20 {
    capture: PhysicalEntryDebugCaptureV20,
    cursor: Cursor,
}
impl PhysicalEntryDebugSessionV20 {
    pub fn capture(
        module: &AdmittedSimulationModuleV1,
        owner: &VerifiedCanonicalKernelIrModuleV20,
        request: &SimulationRequestV1,
        options: PhysicalEntryDebugOptionsV20,
        ledger: CanonicalKernelIrOwnedVerificationResourceBudgetV1,
    ) -> Self {
        Self {
            capture: module.capture_physical_entry_debug_v20(owner, request, options, ledger),
            cursor: Cursor::default(),
        }
    }
    pub fn usage(&self) -> PhysicalEntryDebugUsageV20 {
        self.capture.usage()
    }
    pub fn identity(&self) -> fe2o3_kir_sim::SimulationKernelIrIdentityV1 {
        self.capture.identity()
    }
    pub fn outcome(&self) -> PhysicalEntryDebugOutcomeV20 {
        self.capture.outcome()
    }
    /// Debit bounded CLI inspection/parsing/serialization work without exposing
    /// a mutable ledger or releasing any retained capture/scratch storage.
    pub fn charge_query_work(
        &mut self,
        work: usize,
    ) -> Result<(), fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceErrorV1> {
        self.capture.charge_navigation(work)
    }
    pub fn records_len(&self) -> usize {
        self.capture.len()
    }
    pub fn cursor(&self) -> Option<usize> {
        self.cursor.index()
    }
    pub fn record(&self, index: usize) -> Option<PhysicalEntryDebugRecordRefV20<'_>> {
        self.capture.record(index)
    }
    pub fn capture_error(&self) -> Option<fe2o3_kir_sim::PhysicalEntryDebugCaptureErrorV20> {
        self.capture.error()
    }
    pub fn capture_stop(&self) -> Option<fe2o3_kir_sim::PhysicalEntryDebugCaptureStopV20> {
        self.capture.stop()
    }
    /// Select the position before the first observation, without resuming execution.
    pub fn rewind(&mut self) -> PhysicalEntryDebugNavigationV20 {
        self.cursor.rewind(&mut self.capture)
    }
    pub fn current(&self) -> Option<PhysicalEntryDebugRecordRefV20<'_>> {
        self.capture.record(self.cursor.index()?)
    }
    pub fn seek(&mut self, index: usize) -> PhysicalEntryDebugNavigationV20 {
        self.cursor.seek(&mut self.capture, index)
    }
    pub fn step_forward(&mut self) -> PhysicalEntryDebugNavigationV20 {
        self.cursor.forward(&mut self.capture)
    }
    pub fn step_reverse(&mut self) -> PhysicalEntryDebugNavigationV20 {
        self.cursor.reverse(&mut self.capture)
    }
    /// Explicitly destroy all retained records before returning the original ledger.
    pub fn into_budget(self) -> CanonicalKernelIrOwnedVerificationResourceBudgetV1 {
        self.capture.into_budget()
    }
}

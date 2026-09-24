//! Move-only cursor over actual budget-owned V20 CPU checkpoints.
//! Navigation selects prior observations; it cannot restore/resume an Engine.
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
    cursor: Option<usize>,
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
            cursor: None,
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
        self.cursor
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
        if self.capture.error().is_some()
            || self.capture.outcome() == PhysicalEntryDebugOutcomeV20::PreflightRefused
            || self.capture.charge_navigation(1).is_err()
        {
            return PhysicalEntryDebugNavigationV20::Unavailable;
        }
        self.cursor = None;
        PhysicalEntryDebugNavigationV20::Beginning
    }
    pub fn current(&self) -> Option<PhysicalEntryDebugRecordRefV20<'_>> {
        self.capture.record(self.cursor?)
    }
    pub fn seek(&mut self, index: usize) -> PhysicalEntryDebugNavigationV20 {
        if self.capture.charge_navigation(1).is_err()
            || self.capture.error().is_some()
            || index > self.capture.len()
        {
            return PhysicalEntryDebugNavigationV20::Unavailable;
        }
        if let Some(record) = self.capture.record(index) {
            let ordinal = record.ordinal();
            self.cursor = Some(index);
            return PhysicalEntryDebugNavigationV20::Record { index, ordinal };
        }
        if self.capture.stop().is_some() {
            return PhysicalEntryDebugNavigationV20::Incomplete;
        }
        if self.capture.outcome() != PhysicalEntryDebugOutcomeV20::Completed {
            return PhysicalEntryDebugNavigationV20::Unavailable;
        }
        self.cursor = Some(index);
        PhysicalEntryDebugNavigationV20::End
    }
    pub fn step_forward(&mut self) -> PhysicalEntryDebugNavigationV20 {
        match self.cursor.map_or(Some(0), |n| n.checked_add(1)) {
            Some(n) if n <= self.capture.len() => self.seek(n),
            _ => PhysicalEntryDebugNavigationV20::End,
        }
    }
    pub fn step_reverse(&mut self) -> PhysicalEntryDebugNavigationV20 {
        if self.capture.error().is_some()
            || self.capture.outcome() == PhysicalEntryDebugOutcomeV20::PreflightRefused
        {
            return PhysicalEntryDebugNavigationV20::Unavailable;
        }
        if let Some(previous) = self.cursor.and_then(|n| n.checked_sub(1)) {
            return self.seek(previous);
        }
        if self.capture.charge_navigation(1).is_err() {
            return PhysicalEntryDebugNavigationV20::Unavailable;
        }
        self.cursor = None;
        PhysicalEntryDebugNavigationV20::Beginning
    }
    /// Explicitly destroy all retained records before returning the original ledger.
    pub fn into_budget(self) -> CanonicalKernelIrOwnedVerificationResourceBudgetV1 {
        self.capture.into_budget()
    }
}

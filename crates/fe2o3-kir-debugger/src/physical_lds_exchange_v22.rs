//! Move-only cursor over exact-owner V22 CPU checkpoints.
//! Reverse navigation selects retained observations; it cannot resume execution.
use crate::physical_cursor_v1::Cursor;
use fe2o3_kernel_ir::{
    CanonicalKernelIrOwnedVerificationResourceBudgetV1,
    CanonicalKernelIrVerificationResourceErrorV1, VerifiedCanonicalKernelIrModuleV22,
};
use fe2o3_kir_sim::{
    AdmittedSimulationModuleV1, PhysicalLdsExchangeDebugCaptureErrorV22,
    PhysicalLdsExchangeDebugCaptureStopV22, PhysicalLdsExchangeDebugCaptureV22,
    PhysicalLdsExchangeDebugOptionsV22, PhysicalLdsExchangeDebugOutcomeV22,
    PhysicalLdsExchangeDebugRecordRefV22, PhysicalLdsExchangeDebugUsageV22, SimulationRequestV1,
};
pub type PhysicalLdsExchangeDebugNavigationV22 = crate::PhysicalEntryDebugNavigationV20;

/// No constructor accepts raw snapshots, transcripts, hashes or source metadata.
/// Input, output and the context-owned LDS frame remain immutable CPU observations.
/// Barrier records do not expose hidden publication bitmaps or resumable state.
pub struct PhysicalLdsExchangeDebugSessionV22 {
    capture: PhysicalLdsExchangeDebugCaptureV22,
    cursor: Cursor,
}
impl PhysicalLdsExchangeDebugSessionV22 {
    pub fn capture(
        module: &AdmittedSimulationModuleV1,
        owner: &VerifiedCanonicalKernelIrModuleV22,
        request: &SimulationRequestV1,
        options: PhysicalLdsExchangeDebugOptionsV22,
        ledger: CanonicalKernelIrOwnedVerificationResourceBudgetV1,
    ) -> Self {
        Self {
            capture: module
                .capture_physical_lds_exchange_debug_v22(owner, request, options, ledger),
            cursor: Cursor::default(),
        }
    }
    pub fn usage(&self) -> PhysicalLdsExchangeDebugUsageV22 {
        self.capture.usage()
    }
    pub fn identity(&self) -> fe2o3_kir_sim::SimulationKernelIrIdentityV1 {
        self.capture.identity()
    }
    pub fn outcome(&self) -> PhysicalLdsExchangeDebugOutcomeV22 {
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
    pub fn record(&self, index: usize) -> Option<PhysicalLdsExchangeDebugRecordRefV22<'_>> {
        self.capture.record(index)
    }
    pub fn current(&self) -> Option<PhysicalLdsExchangeDebugRecordRefV22<'_>> {
        self.capture.record(self.cursor.index()?)
    }
    pub fn capture_error(&self) -> Option<PhysicalLdsExchangeDebugCaptureErrorV22> {
        self.capture.error()
    }
    pub fn capture_stop(&self) -> Option<PhysicalLdsExchangeDebugCaptureStopV22> {
        self.capture.stop()
    }
    pub fn rewind(&mut self) -> PhysicalLdsExchangeDebugNavigationV22 {
        self.cursor.rewind(&mut self.capture)
    }
    pub fn seek(&mut self, index: usize) -> PhysicalLdsExchangeDebugNavigationV22 {
        self.cursor.seek(&mut self.capture, index)
    }
    pub fn step_forward(&mut self) -> PhysicalLdsExchangeDebugNavigationV22 {
        self.cursor.forward(&mut self.capture)
    }
    pub fn step_reverse(&mut self) -> PhysicalLdsExchangeDebugNavigationV22 {
        self.cursor.reverse(&mut self.capture)
    }
    pub fn into_budget(self) -> CanonicalKernelIrOwnedVerificationResourceBudgetV1 {
        self.capture.into_budget()
    }
}

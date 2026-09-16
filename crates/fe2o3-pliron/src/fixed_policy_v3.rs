//! Trusted fixed policy selection and observed execution evidence for policy 3.
//! The graph schema remains V12; this is not a new semantic checker policy.

use crate::{
    KIR_PLIRON_PRODUCTION_PASSES_V12, PlironOptimizationPassV1 as PassKind,
    PlironOptimizationReportV1, PlironOptimizationResourcesV12,
};
use dialect_gpu::dominance_cse_v1::{DominanceCseBudgetV1, DominanceCseErrorV1};
use fe2o3_kernel_ir::{
    CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
    CanonicalKernelIrVerificationResourceErrorV1 as Resource,
    VerifiedCanonicalKernelIrModuleV12 as Owner,
};

pub(crate) const POLICY3_PASSES: [PassKind; 8] = [
    PassKind::SparseConditionalConstantPropagation,
    PassKind::SimplifyControlFlow,
    PassKind::SelectSameValueCanonicalization,
    PassKind::DeadCodeElimination,
    PassKind::LocalPureCommonSubexpressionElimination,
    PassKind::DominancePureCommonSubexpressionElimination,
    PassKind::DeadCodeElimination,
    PassKind::SimplifyControlFlow,
];

pub(crate) const POLICY3_CANONICAL_CAP: usize = 16_777_216;
pub(crate) const POLICY3_MAX_PASSES: usize = 256;
pub(crate) const POLICY3_GRAPH_CAP: usize = 32_768;
pub(crate) const POLICY3_SESSION_WORK_CAP: usize = 25_268_224;

/// This selector is private to the common execution/capture implementation.
/// External report contents never construct it or choose a roster.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum FixedPolicy {
    Historical2,
    Checked3,
}

impl FixedPolicy {
    pub(crate) const fn passes(self) -> &'static [PassKind] {
        match self {
            Self::Historical2 => &KIR_PLIRON_PRODUCTION_PASSES_V12,
            Self::Checked3 => &POLICY3_PASSES,
        }
    }

    pub(crate) const fn map_domain(self) -> &'static [u8] {
        match self {
            Self::Historical2 => b"FE2O3/KIR-OPTIMIZATION-MAP/V12/POLICY-2/OBSERVED-V1\0",
            Self::Checked3 => b"FE2O3/KIR-OPTIMIZATION-MAP/V12/POLICY-3/OBSERVED-V1\0",
        }
    }
}

/// Bridge to the existing canonical ledger, never a second allowance.
pub(crate) struct CseLedger<'budget, 'work> {
    budget: &'budget mut Budget<'work>,
    first_failure: Option<Resource>,
    local_live: usize,
    start_work: usize,
}

impl<'budget, 'work> CseLedger<'budget, 'work> {
    pub(crate) fn new(budget: &'budget mut Budget<'work>) -> Self {
        let start_work = budget.work();
        Self {
            budget,
            first_failure: None,
            local_live: 0,
            start_work,
        }
    }

    fn remember(&mut self, error: Resource) -> Resource {
        *self.first_failure.get_or_insert(error)
    }

    pub(crate) fn record_core_error(&mut self, error: DominanceCseErrorV1<Resource>) -> Resource {
        self.remember(match error {
            DominanceCseErrorV1::Budget(error) => error,
            DominanceCseErrorV1::Overflow => Resource::Arithmetic,
            DominanceCseErrorV1::Allocation => Resource::Allocation,
        })
    }

    pub(crate) fn failure(&self) -> Option<Resource> {
        self.first_failure
    }

    pub(crate) fn finish(&mut self) -> Result<usize, Resource> {
        if self.local_live != 0 {
            self.remember(Resource::Accounting);
        }
        if let Some(error) = self.failure() {
            return Err(error);
        }
        self.budget
            .work()
            .checked_sub(self.start_work)
            .ok_or(Resource::Accounting)
    }
}

impl DominanceCseBudgetV1 for CseLedger<'_, '_> {
    type Error = Resource;

    fn charge_work(&mut self, work: usize) -> Result<(), Resource> {
        if let Some(error) = self.first_failure {
            return Err(error);
        }
        self.budget
            .charge_work(work)
            .map_err(|error| self.remember(error))
    }

    fn reserve_storage(&mut self, bytes: usize) -> Result<(), Resource> {
        if let Some(error) = self.first_failure {
            return Err(error);
        }
        let live = self
            .local_live
            .checked_add(bytes)
            .ok_or_else(|| self.remember(Resource::Arithmetic))?;
        self.budget
            .reserve_storage(bytes)
            .map_err(|error| self.remember(error))?;
        self.local_live = live;
        Ok(())
    }

    fn release_storage(&mut self, bytes: usize) {
        // Reject an invalid local release even when the caller's larger live
        // prefix could hide it. Cleanup continues after a prior work denial.
        let Some(live) = self.local_live.checked_sub(bytes) else {
            self.remember(Resource::Accounting);
            return;
        };
        match self.budget.release_storage(bytes) {
            Ok(()) => self.local_live = live,
            Err(error) => {
                self.remember(error);
            }
        }
    }
}

/// Canonical execution record size, independent of Rust usize/layout.
/// 8 control + 80 B/O identities + 48 structural profile + 40 fixed caps + 32 report
/// totals + 56 final graph identity + 32 map digest + 8 * 60 pass bytes.
pub const POLICY3_EXECUTION_RECORD_BYTES_V1: usize = 776;

/// Domain-separated diagnostic identity only. Full frame equality, the sealed
/// execution witness, and independent semantic checking remain mandatory.
pub fn policy3_execution_receipt_digest_v1(
    bytes: &[u8],
    budget: &mut Budget<'_>,
) -> Result<[u8; 32], Resource> {
    use sha2::{Digest, Sha256};
    const DOMAIN: &[u8] = b"FE2O3/CHECKED-KIR-OPTIMIZER-EXECUTION/V12/POLICY-3/RECEIPT-V1\0";
    budget.charge_work(2)?;
    if bytes.len() > fe2o3_kernel_ir::MAX_CANONICAL_KIR_TRANSITION_RECEIPT_BYTES_V1 {
        return Err(Resource::Arithmetic);
    }
    let length = u64::try_from(bytes.len()).map_err(|_| Resource::Arithmetic)?;
    budget.charge_work(
        bytes
            .len()
            .checked_add(DOMAIN.len() + 8)
            .ok_or(Resource::Arithmetic)?,
    )?;
    let mut digest = Sha256::new();
    digest.update(DOMAIN);
    digest.update(length.to_le_bytes());
    digest.update(bytes);
    Ok(digest.finalize().into())
}

/// Move-only evidence minted only by the actual fixed policy-3 execution.
/// This is distinct from the independently checked F2NTR1 semantic relation.
/// It does not authorize ranked/formal checks, code generation or execution.
pub struct Policy3ExecutionWitnessV1 {
    canonical: [u8; POLICY3_EXECUTION_RECORD_BYTES_V1],
}

pub(crate) struct ExecutionProfileV1 {
    pub(crate) resources: PlironOptimizationResourcesV12,
    pub(crate) registered_nodes: usize,
    pub(crate) cse_work: usize,
}

impl Policy3ExecutionWitnessV1 {
    /// Immutable exact record; bytes alone remain untrusted outside this owner.
    pub const fn canonical_bytes(&self) -> &[u8; POLICY3_EXECUTION_RECORD_BYTES_V1] {
        &self.canonical
    }

    pub const fn policy_version(&self) -> u16 {
        3
    }

    pub const fn grants_authority(&self) -> bool {
        false
    }

    pub(crate) fn from_execution(
        input: &Owner,
        output: &Owner,
        report: &PlironOptimizationReportV1,
        map: &crate::KirOptimizationMapPolicy3V12,
        execution: ExecutionProfileV1,
        budget: &mut Budget<'_>,
    ) -> Result<Self, Resource> {
        let ExecutionProfileV1 {
            resources: profile,
            registered_nodes,
            cse_work,
        } = execution;
        // Count/terminal/endpoint checks plus both eight-row roster traversals.
        budget.charge_work(4 + 2 * POLICY3_PASSES.len())?;
        if report.passes().len() != POLICY3_PASSES.len()
            || report
                .passes()
                .iter()
                .zip(POLICY3_PASSES)
                .any(|(actual, expected)| actual.pass() != expected)
            || !map.matches_execution(report)
            || map.input_identity() != input.canonical().identity()
            || map.output_identity() != output.canonical().identity()
        {
            return Err(Resource::Accounting);
        }
        budget.charge_work(POLICY3_EXECUTION_RECORD_BYTES_V1)?;
        // Caller prepays the enclosing output wrapper before this fixed owned
        // record is constructed. No Vec or graph clone is introduced here.
        let mut canonical = [0u8; POLICY3_EXECUTION_RECORD_BYTES_V1];
        let mut writer = RecordWriter {
            bytes: &mut canonical,
            cursor: 0,
        };
        writer.u16(3);
        writer.u16(1);
        writer.u16(8);
        writer.u16(0);
        for owner in [input, output] {
            writer.raw(owner.canonical().identity().digest());
            writer.u64(owner.canonical().identity().canonical_length());
        }
        for value in [
            registered_nodes,
            profile.work(),
            profile.persistent_storage(),
            profile.temporary_storage(),
            POLICY3_PASSES.len(),
            cse_work,
            POLICY3_CANONICAL_CAP,
            POLICY3_CANONICAL_CAP,
            POLICY3_MAX_PASSES,
            POLICY3_GRAPH_CAP,
            POLICY3_SESSION_WORK_CAP,
            report.initial_graph_work(),
            report.final_graph_work(),
            report.invalidated_handle_count(),
            report.work_units(),
        ] {
            writer.usize(value)?;
        }
        let final_graph = report.final_graph_identity();
        writer.raw(&final_graph.canonical_digest());
        writer.u64(final_graph.epoch().sequence());
        writer.usize(final_graph.tree_work())?;
        writer.usize(final_graph.operation_count())?;
        writer.raw(map.digest());
        for pass in report.passes() {
            writer.raw(&[pass_tag(pass.pass()), u8::from(pass.changed())]);
            writer.u16(0);
            writer.usize(pass.input_graph_work())?;
            writer.usize(pass.output_graph_work())?;
            writer.usize(pass.work_units())?;
            writer.u64(pass.input_epoch().sequence());
            writer.u64(pass.output_epoch().sequence());
            writer.usize(pass.invalidated_analysis_count())?;
            writer.usize(pass.preserved_analysis_count())?;
        }
        assert_eq!(writer.cursor, POLICY3_EXECUTION_RECORD_BYTES_V1);
        Ok(Self { canonical })
    }
}

fn pass_tag(pass: PassKind) -> u8 {
    match pass {
        PassKind::DeadCodeElimination => 1,
        PassKind::SparseConditionalConstantPropagation => 2,
        PassKind::SelectSameValueCanonicalization => 3,
        PassKind::LocalPureCommonSubexpressionElimination => 4,
        PassKind::SimplifyControlFlow => 5,
        PassKind::DominancePureCommonSubexpressionElimination => 6,
    }
}

struct RecordWriter<'a> {
    bytes: &'a mut [u8; POLICY3_EXECUTION_RECORD_BYTES_V1],
    cursor: usize,
}

#[cfg(test)]
#[path = "fixed_policy_v3_tests.rs"]
mod tests;

impl RecordWriter<'_> {
    fn raw(&mut self, value: &[u8]) {
        self.bytes[self.cursor..self.cursor + value.len()].copy_from_slice(value);
        self.cursor += value.len();
    }
    fn u16(&mut self, value: u16) {
        self.raw(&value.to_le_bytes());
    }
    fn u64(&mut self, value: u64) {
        self.raw(&value.to_le_bytes());
    }
    fn usize(&mut self, value: usize) -> Result<(), Resource> {
        self.u64(u64::try_from(value).map_err(|_| Resource::Arithmetic)?);
        Ok(())
    }
}

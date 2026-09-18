//! Sealed execution custody for the Policy6 integer/DCE continuation only.
//! Semantic checking is a separate complete occurrence relation.
use crate::fixed_policy_v3::{
    ExecutionProfileV1, POLICY3_CANONICAL_CAP, POLICY3_GRAPH_CAP, POLICY3_MAX_PASSES,
    POLICY3_SESSION_WORK_CAP, pass_tag,
};
use crate::{
    KirOptimizationMapIntegerContinuationV12, PlironOptimizationPassV1 as PassKind,
    PlironOptimizationReportV1,
};
use fe2o3_kernel_ir::{
    CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
    CanonicalKernelIrVerificationResourceErrorV1 as Resource,
    VerifiedCanonicalKernelIrModuleV12 as Owner,
};

pub(crate) const INTEGER_CONTINUATION_PASSES: [PassKind; 2] = [
    PassKind::IntegerNeutralCanonicalization,
    PassKind::DeadCodeElimination,
];
/// Fixed header/subjects/profile/report/map plus exactly two pass rows.
pub const INTEGER_CONTINUATION_EXECUTION_RECORD_BYTES_V1: usize = 416;

/// Constructible only by the actual fixed continuation. No decoder or authority.
pub struct IntegerContinuationExecutionWitnessV1 {
    canonical: [u8; INTEGER_CONTINUATION_EXECUTION_RECORD_BYTES_V1],
    profile: ExecutionProfileV1,
}
impl IntegerContinuationExecutionWitnessV1 {
    pub const fn canonical_bytes(&self) -> &[u8; INTEGER_CONTINUATION_EXECUTION_RECORD_BYTES_V1] {
        &self.canonical
    }
    pub const fn policy_version(&self) -> u16 {
        6
    }
    pub const fn grants_authority(&self) -> bool {
        false
    }
    pub(crate) fn from_execution(
        input: &Owner,
        output: &Owner,
        report: &PlironOptimizationReportV1,
        map: &KirOptimizationMapIntegerContinuationV12,
        profile: ExecutionProfileV1,
        budget: &mut Budget<'_>,
    ) -> Result<Self, Resource> {
        let canonical = record(input, output, report, map, &profile, budget)?;
        Ok(Self { canonical, profile })
    }
    pub fn check_against(
        &self,
        input: &Owner,
        output: &Owner,
        report: &PlironOptimizationReportV1,
        map: &KirOptimizationMapIntegerContinuationV12,
        budget: &mut Budget<'_>,
    ) -> Result<(), Resource> {
        let expected = record(input, output, report, map, &self.profile, budget)?;
        budget.charge_work(INTEGER_CONTINUATION_EXECUTION_RECORD_BYTES_V1)?;
        if expected != self.canonical {
            return Err(Resource::Accounting);
        }
        Ok(())
    }
}
fn record(
    input: &Owner,
    output: &Owner,
    report: &PlironOptimizationReportV1,
    map: &KirOptimizationMapIntegerContinuationV12,
    execution: &ExecutionProfileV1,
    budget: &mut Budget<'_>,
) -> Result<[u8; INTEGER_CONTINUATION_EXECUTION_RECORD_BYTES_V1], Resource> {
    let profile = execution.resources;
    let registered_nodes = execution.registered_nodes;
    let cse_work = execution.cse_work;
    // Count/terminal/endpoint checks plus both fixed two-row roster traversals.
    budget.charge_work(4 + 2 * INTEGER_CONTINUATION_PASSES.len())?;
    if report.passes().len() != INTEGER_CONTINUATION_PASSES.len()
        || report
            .passes()
            .iter()
            .zip(INTEGER_CONTINUATION_PASSES)
            .any(|(actual, expected)| actual.pass() != expected)
        || !map.matches_execution(report)
        || map.input_identity() != input.canonical().identity()
        || map.output_identity() != output.canonical().identity()
    {
        return Err(Resource::Accounting);
    }
    budget.charge_work(INTEGER_CONTINUATION_EXECUTION_RECORD_BYTES_V1)?;
    // Caller prepays the enclosing output wrapper before this fixed owned
    // record is constructed. No Vec or graph clone is introduced here.
    let mut canonical = [0u8; INTEGER_CONTINUATION_EXECUTION_RECORD_BYTES_V1];
    let mut writer = RecordWriter {
        bytes: &mut canonical,
        cursor: 0,
    };
    writer.u16(6);
    writer.u16(1);
    writer.u16(2);
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
        INTEGER_CONTINUATION_PASSES.len(),
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
    assert_eq!(
        writer.cursor,
        INTEGER_CONTINUATION_EXECUTION_RECORD_BYTES_V1
    );
    Ok(canonical)
}
struct RecordWriter<'a> {
    bytes: &'a mut [u8; INTEGER_CONTINUATION_EXECUTION_RECORD_BYTES_V1],
    cursor: usize,
}
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

//! Nominal evidence for actual Policy10 mixed pure-CSE execution on a V18 graph.
use super::*;

/// Four observed pass rows and the exact V18 storage-table identity.
pub const MIXED_PURE_CSE_EXECUTION_RECORD_BYTES_V18: usize =
    INTEGER_CONTINUATION_EXECUTION_RECORD_BYTES_V18 + 2 * 60;

/// Move-only evidence from the private fixed Policy10 execution. Serialized
/// identity bytes cannot mint this witness or replace historical Policy9.
///
/// ```compile_fail
/// use fe2o3_pliron::{MixedPureCseExecutionWitnessV18, IntegerWorklistExecutionWitnessV18};
/// fn substitute(old: IntegerWorklistExecutionWitnessV18) -> MixedPureCseExecutionWitnessV18 { old }
/// ```
pub struct MixedPureCseExecutionWitnessV18 {
    canonical: [u8; MIXED_PURE_CSE_EXECUTION_RECORD_BYTES_V18],
}

impl MixedPureCseExecutionWitnessV18 {
    pub const fn canonical_bytes(&self) -> &[u8; MIXED_PURE_CSE_EXECUTION_RECORD_BYTES_V18] {
        &self.canonical
    }
    pub const fn policy_version(&self) -> u16 {
        10
    }
    pub const fn graph_schema(&self) -> u16 {
        18
    }
    pub const fn grants_authority(&self) -> bool {
        false
    }

    pub(crate) fn from_execution(
        input: &Owner18,
        output: &Owner18,
        table: CanonicalStorageTableIdentityV18,
        report: &PlironOptimizationReportV1,
        map: &crate::KirOptimizationMapMixedPureCseV18,
        execution: ExecutionProfileV1,
        budget: &mut Budget<'_>,
    ) -> Result<Self, Resource> {
        let passes = &POLICY10_PASSES;
        budget.charge_work(4 + 2 * passes.len())?;
        if report.passes().len() != passes.len()
            || report
                .passes()
                .iter()
                .zip(passes)
                .any(|(row, pass)| row.pass() != *pass)
            || !map.matches_execution(report)
            || map.input_identity() != input.identity()
            || map.output_identity() != output.identity()
        {
            return Err(Resource::Accounting);
        }
        budget.charge_work(MIXED_PURE_CSE_EXECUTION_RECORD_BYTES_V18)?;
        let mut canonical = [0; MIXED_PURE_CSE_EXECUTION_RECORD_BYTES_V18];
        let mut writer = RecordWriter {
            bytes: &mut canonical,
            cursor: 0,
        };
        for control in [10, 1, 4, 18] {
            writer.u16(control);
        }
        for owner in [input, output] {
            writer.raw(owner.identity().digest());
            writer.u64(owner.identity().canonical_length());
        }
        writer.raw(table.digest());
        writer.u64(table.encoded_length());
        write_execution_tail_for_pass_count(
            &mut writer,
            report,
            map.digest(),
            execution,
            passes.len(),
        )?;
        assert_eq!(writer.cursor, MIXED_PURE_CSE_EXECUTION_RECORD_BYTES_V18);
        Ok(Self { canonical })
    }
}

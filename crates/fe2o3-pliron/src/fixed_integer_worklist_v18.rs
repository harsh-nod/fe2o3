//! Nominal evidence for actual Policy9 worklist execution on a V18 graph.
use super::*;

/// Two observed pass rows and the exact V18 storage-table identity.
pub const INTEGER_WORKLIST_EXECUTION_RECORD_BYTES_V18: usize =
    INTEGER_CONTINUATION_EXECUTION_RECORD_BYTES_V18;

/// Move-only evidence from the private fixed Policy9 execution. Serialized
/// identity bytes cannot mint this witness or replace historical Policy6.
///
/// ```compile_fail
/// use fe2o3_pliron::{IntegerWorklistExecutionWitnessV18, IntegerContinuationExecutionWitnessV18};
/// fn substitute(old: IntegerContinuationExecutionWitnessV18) -> IntegerWorklistExecutionWitnessV18 { old }
/// ```
pub struct IntegerWorklistExecutionWitnessV18 {
    canonical: [u8; INTEGER_WORKLIST_EXECUTION_RECORD_BYTES_V18],
}

impl IntegerWorklistExecutionWitnessV18 {
    pub const fn canonical_bytes(&self) -> &[u8; INTEGER_WORKLIST_EXECUTION_RECORD_BYTES_V18] {
        &self.canonical
    }
    pub const fn policy_version(&self) -> u16 {
        9
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
        map: &crate::KirOptimizationMapIntegerWorklistV18,
        execution: ExecutionProfileV1,
        budget: &mut Budget<'_>,
    ) -> Result<Self, Resource> {
        let passes = &POLICY9_PASSES;
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
        budget.charge_work(INTEGER_WORKLIST_EXECUTION_RECORD_BYTES_V18)?;
        let mut canonical = [0; INTEGER_WORKLIST_EXECUTION_RECORD_BYTES_V18];
        let mut writer = RecordWriter {
            bytes: &mut canonical,
            cursor: 0,
        };
        for control in [9, 1, 2, 18] {
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
        assert_eq!(writer.cursor, INTEGER_WORKLIST_EXECUTION_RECORD_BYTES_V18);
        Ok(Self { canonical })
    }
}

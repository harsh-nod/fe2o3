//! Sealed actual execution evidence with a nominal V18 canonical frame.
use super::*;
use fe2o3_kernel_ir::{
    CanonicalStorageTableIdentityV18, VerifiedCanonicalKernelIrModuleV18 as Owner18,
};

#[path = "fixed_integer_worklist_v18.rs"]
mod worklist;
pub use worklist::{
    INTEGER_WORKLIST_EXECUTION_RECORD_BYTES_V18, IntegerWorklistExecutionWitnessV18,
};

#[path = "fixed_mixed_pure_cse_v18.rs"]
mod mixed_pure_cse;
pub use mixed_pure_cse::{
    MIXED_PURE_CSE_EXECUTION_RECORD_BYTES_V18, MixedPureCseExecutionWitnessV18,
};

#[path = "fixed_mixed_fixedpoint_v18.rs"]
mod mixed_fixedpoint;
pub use mixed_fixedpoint::MixedFixedpointExecutionWitnessV18;
pub(crate) use mixed_fixedpoint::validate_fixedpoint_report;

/// Existing policy-3 frame plus the exact 40-byte storage-table identity.
/// The final control word names graph schema 18, not V12's reserved zero.
pub const POLICY3_EXECUTION_RECORD_BYTES_V18: usize = POLICY3_EXECUTION_RECORD_BYTES_V1 + 40;

/// Minted only by actual fixed execution on the private V18 graph. Canonical
/// bytes remain inert and cannot construct this move-only execution witness.
///
/// ```compile_fail
/// use fe2o3_pliron::Policy3ExecutionWitnessV18;
/// fn fabricate(bytes: [u8; 816]) -> Policy3ExecutionWitnessV18 {
///     Policy3ExecutionWitnessV18 { canonical: bytes }
/// }
/// ```
pub struct Policy3ExecutionWitnessV18 {
    canonical: [u8; POLICY3_EXECUTION_RECORD_BYTES_V18],
}

impl Policy3ExecutionWitnessV18 {
    pub const fn canonical_bytes(&self) -> &[u8; POLICY3_EXECUTION_RECORD_BYTES_V18] {
        &self.canonical
    }
    pub const fn policy_version(&self) -> u16 {
        3
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
        map: &crate::KirOptimizationMapPolicy3V18,
        execution: ExecutionProfileV1,
        budget: &mut Budget<'_>,
    ) -> Result<Self, Resource> {
        budget.charge_work(4 + 2 * POLICY3_PASSES.len())?;
        if report.passes().len() != POLICY3_PASSES.len()
            || report
                .passes()
                .iter()
                .zip(POLICY3_PASSES)
                .any(|(actual, expected)| actual.pass() != expected)
            || !map.matches_execution(report)
            || map.input_identity() != input.identity()
            || map.output_identity() != output.identity()
        {
            return Err(Resource::Accounting);
        }
        budget.charge_work(POLICY3_EXECUTION_RECORD_BYTES_V18)?;
        let mut canonical = [0; POLICY3_EXECUTION_RECORD_BYTES_V18];
        let mut writer = RecordWriter {
            bytes: &mut canonical,
            cursor: 0,
        };
        for control in [3, 1, 8, 18] {
            writer.u16(control);
        }
        for owner in [input, output] {
            writer.raw(owner.identity().digest());
            writer.u64(owner.identity().canonical_length());
        }
        writer.raw(table.digest());
        writer.u64(table.encoded_length());
        write_execution_tail(&mut writer, report, map.digest(), execution)?;
        assert_eq!(writer.cursor, POLICY3_EXECUTION_RECORD_BYTES_V18);
        Ok(Self { canonical })
    }
}

#[cfg(test)]
#[path = "fixed_policy_v18_tests.rs"]
mod tests;

/// Policy6's two-row frame with the nominal V18 storage-table identity.
pub const INTEGER_CONTINUATION_EXECUTION_RECORD_BYTES_V18: usize =
    crate::fixed_integer_continuation_v1::INTEGER_CONTINUATION_EXECUTION_RECORD_BYTES_V1 + 40;

/// Move-only actual V18 integer-neutral execution evidence; no authority conversion.
pub struct IntegerContinuationExecutionWitnessV18 {
    canonical: [u8; INTEGER_CONTINUATION_EXECUTION_RECORD_BYTES_V18],
}

impl IntegerContinuationExecutionWitnessV18 {
    pub const fn canonical_bytes(&self) -> &[u8; INTEGER_CONTINUATION_EXECUTION_RECORD_BYTES_V18] {
        &self.canonical
    }
    pub const fn policy_version(&self) -> u16 {
        6
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
        map: &crate::KirOptimizationMapIntegerContinuationV18,
        execution: ExecutionProfileV1,
        budget: &mut Budget<'_>,
    ) -> Result<Self, Resource> {
        let passes = &crate::fixed_integer_continuation_v1::INTEGER_CONTINUATION_PASSES;
        budget.charge_work(4 + 2 * passes.len())?;
        if report.passes().len() != passes.len()
            || report
                .passes()
                .iter()
                .zip(passes)
                .any(|(actual, expected)| actual.pass() != *expected)
            || !map.matches_execution(report)
            || map.input_identity() != input.identity()
            || map.output_identity() != output.identity()
        {
            return Err(Resource::Accounting);
        }
        budget.charge_work(INTEGER_CONTINUATION_EXECUTION_RECORD_BYTES_V18)?;
        let mut canonical = [0; INTEGER_CONTINUATION_EXECUTION_RECORD_BYTES_V18];
        let mut writer = RecordWriter {
            bytes: &mut canonical,
            cursor: 0,
        };
        for control in [6, 1, 2, 18] {
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
        assert_eq!(
            writer.cursor,
            INTEGER_CONTINUATION_EXECUTION_RECORD_BYTES_V18
        );
        Ok(Self { canonical })
    }
}

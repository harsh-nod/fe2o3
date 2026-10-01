//! Bounded retained Policy5 composition; stored transfer receipts are unchanged.
use super::{CheckedCanonicalKernelIrOwnerPolicy5V1, Policy5ExecutionWitnessV1};
use fe2o3_kernel_ir::{LogicalStorageCounterV1 as Counter, LogicalStorageErrorV1 as Error};

impl CheckedCanonicalKernelIrOwnerPolicy5V1 {
    /// Charges actual retained heap only: complete Policy4 C/S/history, distinct
    /// O, and Load-forwarding row capacity. Excludes inline headers and this
    /// wrapper's root visit; includes nested collection/Module visits.
    /// Borrowed B is excluded. On refusal discard the entire enclosing counter,
    /// which may already be partial. V11 Module grammar only, not all V12.
    pub fn charge_retained_heap_v11(&self, counter: &mut Counter) -> Result<(), Error> {
        let Self {
            prefix,
            output,
            rows,
            execution,
            retained,
        } = self;
        let Policy5ExecutionWitnessV1 { bytes } = execution;
        let _: &[u8; super::POLICY5_EXECUTION_RECORD_BYTES_V1] = bytes;
        let _: &usize = retained;
        fn fixed_rows<T: Copy>(_: &Vec<T>) {}
        fixed_rows(rows);
        prefix.charge_retained_heap_v11(counter)?;
        output.charge_retained_heap_v11(counter)?;
        counter.vector(rows)
    }
}

#[cfg(test)]
#[path = "checked_optimization_policy5_retained_storage_v1_tests.rs"]
mod tests;

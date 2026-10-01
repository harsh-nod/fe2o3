//! Bounded retained Policy4 composition; stored transfer receipts are unchanged.
use super::{CheckedCanonicalKernelIrOwnerPolicy4V1, Policy4ExecutionWitnessV1};
use fe2o3_kernel_ir::{LogicalStorageCounterV1 as Counter, LogicalStorageErrorV1 as Error};

impl CheckedCanonicalKernelIrOwnerPolicy4V1 {
    /// Charges actual retained heap only: Policy3 C/history, distinct S, and
    /// Store-forwarding row capacity. Excludes all nested inline headers and
    /// this wrapper's root visit; includes nested collection/Module visits.
    /// Borrowed B is excluded. On refusal discard the entire enclosing counter,
    /// which may already be partial. V11 Module grammar only, not all V12.
    pub fn charge_retained_heap_v11(&self, counter: &mut Counter) -> Result<(), Error> {
        let Self {
            intermediate,
            output,
            rows,
            execution,
            retained,
        } = self;
        let Policy4ExecutionWitnessV1 { bytes } = execution;
        let _: &[u8; super::POLICY4_EXECUTION_RECORD_BYTES_V1] = bytes;
        let _: &usize = retained;
        fn fixed_rows<T: Copy>(_: &Vec<T>) {}
        fixed_rows(rows);
        intermediate.charge_retained_heap_v11(counter)?;
        output.charge_retained_heap_v11(counter)?;
        counter.vector(rows)
    }
}

#[cfg(test)]
#[path = "checked_optimization_policy4_retained_storage_v1_tests.rs"]
mod tests;

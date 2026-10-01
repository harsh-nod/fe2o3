//! Bounded actual heap walks for the two selected checked native owners.
use super::{CheckedParts, KirCheckedNeutralOptimizationStorageV1};
use crate::{
    KirOptimizationMapIntegerContinuationV12, KirOptimizationMapPolicy3V12,
    fixed_integer_continuation_v1::IntegerContinuationExecutionWitnessV1,
    fixed_policy_v3::Policy3ExecutionWitnessV1,
};
use fe2o3_kernel_ir::{LogicalStorageCounterV1 as Counter, LogicalStorageErrorV1 as Error};

macro_rules! checked_parts_heap {
    ($map:ty, $execution:ty) => {
        impl CheckedParts<$map, $execution> {
            pub(crate) fn charge_retained_heap_v11(
                &self,
                counter: &mut Counter,
            ) -> Result<(), Error> {
                let Self {
                    owner,
                    report,
                    bridge,
                    map,
                    occurrences,
                    input_history,
                    storage,
                    extra,
                } = self;
                let KirCheckedNeutralOptimizationStorageV1 { retained } = storage;
                let _: &usize = retained;
                extra.assert_inline_retained_storage_v1();
                // Every nested inline header is already part of the enclosing
                // checked owner header. No owner is cloned or re-encoded here.
                owner.charge_retained_heap_v11(counter)?;
                report.charge_retained_heap_storage_v1(counter)?;
                bridge.charge_retained_heap_storage_v1(counter)?;
                map.charge_retained_heap_storage_v1(counter)?;
                occurrences.charge_retained_heap_storage_v1(counter)?;
                let bytes: &Vec<u8> = input_history;
                counter.vector(bytes)
            }
        }
    };
}
checked_parts_heap!(KirOptimizationMapPolicy3V12, Policy3ExecutionWitnessV1);
checked_parts_heap!(
    KirOptimizationMapIntegerContinuationV12,
    IntegerContinuationExecutionWitnessV1
);

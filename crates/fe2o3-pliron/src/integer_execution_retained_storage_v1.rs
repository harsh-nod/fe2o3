//! Compile-time ownership shape for the retained integer execution record.
use super::IntegerContinuationExecutionWitnessV1;
use crate::fixed_policy_v3::ExecutionProfileV1;
impl IntegerContinuationExecutionWitnessV1 {
    pub(crate) fn assert_inline_retained_storage_v1(&self) {
        let Self { canonical, profile } = self;
        let _: &[u8; super::INTEGER_CONTINUATION_EXECUTION_RECORD_BYTES_V1] = canonical;
        let ExecutionProfileV1 {
            resources,
            registered_nodes,
            cse_work,
        } = profile;
        fn fixed<T: Copy>(_: &T) {}
        fixed(resources);
        let _: &usize = registered_nodes;
        let _: &usize = cse_work;
        // resources is an inline Copy reservation profile, not owned buffers.
        // Its numeric persistent/temporary/report allowances are not added.
    }
}

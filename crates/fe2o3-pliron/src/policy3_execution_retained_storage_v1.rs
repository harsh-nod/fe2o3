//! Compile-time ownership shape for the retained Policy3 execution record.
use super::Policy3ExecutionWitnessV1;
impl Policy3ExecutionWitnessV1 {
    pub(crate) fn assert_inline_retained_storage_v1(&self) {
        let Self { canonical } = self;
        let _: &[u8; super::POLICY3_EXECUTION_RECORD_BYTES_V1] = canonical;
    }
}

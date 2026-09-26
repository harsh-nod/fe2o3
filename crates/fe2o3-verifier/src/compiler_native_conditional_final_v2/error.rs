use super::*;
use crate::conditional_contract_request_v2::ConditionalContractRequestErrorV2;
use crate::conditional_ranked_formulas_v1::ConditionalFormulaImportCheckErrorV2;
use fe2o3_amdgcn_model::{NativeV12TextDescriptorReplayErrorV5, ProductionTargetCoordinateErrorV1};
use fe2o3_kernel_descriptor::DescriptorWireErrorV5;
use fe2o3_kernel_opt::CanonicalRefinedForwardingHistoryErrorV1;
use fe2o3_lower_mir_kernel::ProductionConditionalCheckedFinalErrorV1;
use std::fmt;

/// Opaque terminal error, even when an enclosing postcheck overrides a callback.
/// Deliberately does not expose a source chain to ordinary refund classifiers.
#[derive(Debug)]
pub(crate) struct NativeConditionalFinalErrorV2(pub(super) Cause);

#[derive(Debug)]
pub(super) enum Cause {
    Resource(Resource),
    Source(SourceError),
    Packet(crate::compiler_native_conditional_source_packet_v2::NativeConditionalPacketErrorV2),
    History(CanonicalRefinedForwardingHistoryErrorV1),
    Coordinates(ProductionTargetCoordinateErrorV1),
    Import(ConditionalFormulaImportCheckErrorV2),
    ConditionalFinal(ProductionConditionalCheckedFinalErrorV1),
    Contract(ConditionalContractRequestErrorV2),
    Descriptor(DescriptorWireErrorV5<Resource>),
    Text(NativeV12TextDescriptorReplayErrorV5),
    Mismatch(&'static str),
}
impl NativeConditionalFinalErrorV2 {
    pub(super) fn mismatch(rule: &'static str) -> Self {
        Self(Cause::Mismatch(rule))
    }
}
impl From<Resource> for NativeConditionalFinalErrorV2 {
    fn from(error: Resource) -> Self {
        Self(Cause::Resource(error))
    }
}
impl From<SourceError> for NativeConditionalFinalErrorV2 {
    fn from(error: SourceError) -> Self {
        Self(Cause::Source(error))
    }
}
impl fmt::Display for NativeConditionalFinalErrorV2 {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(out, "conditional source-through-F content: {:?}", self.0)
    }
}
impl std::error::Error for NativeConditionalFinalErrorV2 {}

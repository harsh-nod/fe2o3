use super::*;
use std::{error::Error, fmt};

/// Structured replay refusal, never a proof or source-origin capability.
#[derive(Debug)]
pub struct NativeConditionalSourceProofErrorV2(pub(super) Cause);

#[derive(Debug)]
pub(super) enum Cause {
    Resource(Resource),
    Invalid(&'static str),
    Packet(crate::compiler_native_conditional_source_packet_v2::NativeConditionalPacketErrorV2),
    Native(fe2o3_compiler_lineage::NativeNeutralModuleErrorV1),
    Source(fe2o3_lower_mir_kernel::NativeSourceReplayErrorV1),
    Recipe(crate::NativeCompilerSourceProofErrorV1),
    Rows(fe2o3_lower_mir_kernel::ProductionRankedSourceRowsWireErrorV1),
    Cpu(crate::portable_reference_v1::codec::NativeCpuCodecErrorV1),
    Correspondence(crate::conditional_reference_v1::ConditionalReferenceErrorV1),
    Formula(crate::ProductionConditionalFormulaErrorV2),
    Continuation(fe2o3_lower_mir_kernel::ProductionConditionalContinuationErrorV1),
    Session(fe2o3_pliron::ProductionSessionErrorV1),
    SessionCreate(fe2o3_pliron::ProductionRankedCompileErrorV1),
    Staging(fe2o3_pliron::ProductionFunctionalRefinementAdmissionErrorV2),
    Induction(fe2o3_mir_model::SemanticU32InductionAnalysisErrorV1),
    InductionWire(fe2o3_mir_model::SemanticU32InductionEvidenceErrorV1),
}
impl E {
    pub(super) fn invalid(reason: &'static str) -> Self {
        Self(Cause::Invalid(reason))
    }
    pub(super) fn refund_safe(&self) -> bool {
        self.source().is_none_or(refund_safe)
    }
}
impl From<Resource> for E {
    fn from(value: Resource) -> Self {
        Self(Cause::Resource(value))
    }
}
impl fmt::Display for E {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        if let Cause::Invalid(reason) = &self.0 {
            return write!(out, "native conditional source replay V2: {reason}");
        }
        write!(out, "native conditional source replay V2: {:?}", self.0)
    }
}
impl Error for E {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        Some(match &self.0 {
            Cause::Resource(e) => e,
            Cause::Invalid(_) => return None,
            Cause::Packet(e) => e,
            Cause::Native(e) => e,
            Cause::Source(e) => e,
            Cause::Recipe(e) => e,
            Cause::Rows(e) => e,
            Cause::Cpu(e) => e,
            Cause::Correspondence(e) => e,
            Cause::Formula(e) => e,
            Cause::Continuation(e) => e,
            Cause::Session(e) => e,
            Cause::SessionCreate(e) => e,
            Cause::Staging(e) => e,
            Cause::Induction(e) => e,
            Cause::InductionWire(e) => e,
        })
    }
}

// Some inherited errors intentionally omit Error::source. Inspect their typed
// variants, never text. Opaque CPU failures and arena-owning errors stay charged.
fn refund_safe(error: &(dyn Error + 'static)) -> bool {
    use crate::compiler_native_conditional_source_packet_v2::NativeConditionalPacketErrorV2 as Packet;
    use crate::{
        ProductionConditionalFormulaErrorV1 as Formula1,
        ProductionConditionalFormulaErrorV2 as Formula2,
    };
    use fe2o3_lower_mir_kernel::{
        NativeSourceReplayErrorV1 as Source, ProductionConditionalContinuationErrorV1 as Lower,
    };
    use fe2o3_pliron::ProductionConditionalAggregateErrorV1 as Aggregate;
    if let Some(error) = error.downcast_ref::<Resource>() {
        return !matches!(error, Resource::Accounting);
    }
    if let Some(error) = error.downcast_ref::<Packet>() {
        return match error {
            Packet::Resource(e) => refund_safe(e),
            Packet::Invalid(_) => true,
        };
    }
    if let Some(error) =
        error.downcast_ref::<crate::portable_reference_v1::codec::NativeCpuCodecErrorV1>()
    {
        return match error {
            crate::portable_reference_v1::codec::NativeCpuCodecErrorV1::Resource(e) => {
                refund_safe(e)
            }
            _ => true,
        };
    }
    if let Some(error) =
        error.downcast_ref::<crate::conditional_reference_v1::ConditionalReferenceErrorV1>()
    {
        return !matches!(
            error,
            crate::conditional_reference_v1::ConditionalReferenceErrorV1::ProofExecution(_)
        );
    }
    if let Some(error) = error.downcast_ref::<Formula2>() {
        return match error {
            Formula2::Formula(e) => refund_safe(e),
            Formula2::Codec(e) => refund_safe(e),
            Formula2::Correspondence(e) => refund_safe(e),
            Formula2::Subject(_) => true,
        };
    }
    if let Some(error) = error.downcast_ref::<Formula1>() {
        return match error {
            Formula1::Resource(e) => refund_safe(e),
            Formula1::Graph(e) => refund_safe(e.as_ref()),
            Formula1::Execution(_) | Formula1::Subject(_) => true,
        };
    }
    if let Some(error) = error.downcast_ref::<Aggregate>() {
        return match error {
            Aggregate::Resource(e) => refund_safe(e),
            Aggregate::Source(e) => refund_safe(e),
            Aggregate::Canonical(e) => refund_safe(e),
            Aggregate::Coverage(e) => refund_safe(e),
            Aggregate::Session(e) => refund_safe(e),
            Aggregate::Pipeline(_) => false,
            Aggregate::Subject(_) => true,
        };
    }
    if let Some(error) = error.downcast_ref::<Lower>() {
        return match error {
            Lower::Resource(e) => refund_safe(e),
            Lower::Source(e) => refund_safe(e),
            Lower::Canonical(e) => refund_safe(e),
            Lower::Binding(e) => refund_safe(e),
            Lower::Ranked(e) => refund_safe(e),
            Lower::Coverage(e) => refund_safe(e),
            Lower::Aggregate(e) => refund_safe(e),
            Lower::PipelineRejected | Lower::Subject(_) => true,
        };
    }
    if let Some(error) = error.downcast_ref::<Source>() {
        return match error {
            Source::Resource(e) => refund_safe(e),
            Source::Materialize(e) => refund_safe(e),
            Source::Catalog(e) => refund_safe(e),
            Source::Inventory(e) => refund_safe(e),
            Source::CatalogBinding(e) => refund_safe(e),
            Source::RankedSource(e) => refund_safe(e),
            Source::Semantic(_)
            | Source::Source(_)
            | Source::Ssa(_)
            | Source::Launch(_)
            | Source::Mismatch(_) => true,
        };
    }
    if let Some(error) = error.downcast_ref::<crate::NativeCompilerSourceProofErrorV1>() {
        return match error {
            crate::NativeCompilerSourceProofErrorV1::Resource(e) => refund_safe(e),
            crate::NativeCompilerSourceProofErrorV1::RankedRecipeWire(e) => refund_safe(e),
            crate::NativeCompilerSourceProofErrorV1::EffectReceipt(_)
            | crate::NativeCompilerSourceProofErrorV1::Mismatch(_) => true,
            _ => false,
        };
    }
    if let Some(error) =
        error.downcast_ref::<fe2o3_lower_mir_kernel::ProductionRankedSourceRowsWireErrorV1>()
    {
        use fe2o3_lower_mir_kernel::ProductionRankedSourceRowsWireErrorV1 as Rows;
        return match error {
            Rows::Resource(e) => refund_safe(e),
            Rows::Value(e) => refund_safe(e),
            Rows::Invalid(_) => true,
        };
    }
    if let Some(error) = error.downcast_ref::<fe2o3_pliron::ProductionRankedRecipeWireErrorV1>() {
        use fe2o3_pliron::ProductionRankedRecipeWireErrorV1 as Wire;
        return match error {
            Wire::Resource(e) => refund_safe(e),
            Wire::Invalid(_)
            | Wire::UnknownTag { .. }
            | Wire::Kernel(_)
            | Wire::Binding(_)
            | Wire::TensorEncode(_)
            | Wire::TensorDecode(_) => true,
        };
    }
    // Unknown leaf errors may hide accounting failures. A terminal retained
    // reservation is preferable to releasing an unprovably intact inner floor.
    error.source().is_some_and(refund_safe)
}

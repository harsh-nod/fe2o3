//! Variant-preserving errors shared with the live backend adapter.
use std::fmt;

/// Existing conditional join/conversion failures; no proof authority.
#[derive(Debug)]
pub enum ConditionalReferenceErrorV1 {
    /// An existing CPU correspondence/domain check refused the input.
    UnsupportedReference(&'static str),
    /// An existing ranked-index normalization check refused the input.
    UnsupportedGpuIndex(&'static str),
    /// The translated expression failed typed semantic validation.
    SemanticExpression(fe2o3_pliron::ProductionSemanticExpressionErrorV2),
    /// The retained descriptive subject tuple is invalid.
    Subjects(String),
    /// Existing replay/resource error context, not a claim of proof execution.
    ProofExecution(String),
}

impl fmt::Display for ConditionalReferenceErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnsupportedReference(detail) => write!(
                formatter,
                "source-to-proof V2 reference is unsupported: {detail}"
            ),
            Self::UnsupportedGpuIndex(detail) => write!(
                formatter,
                "source-to-proof V2 cannot normalize the GPU coordinate: {detail}"
            ),
            Self::SemanticExpression(error) => write!(
                formatter,
                "source-to-proof V2 semantic expression is invalid: {error}"
            ),
            Self::Subjects(detail) => write!(
                formatter,
                "source-to-proof V2 subject identity is invalid: {detail}"
            ),
            Self::ProofExecution(detail) => write!(
                formatter,
                "functional-refinement proof execution failed: {detail}; compilation stopped before artifact emission"
            ),
        }
    }
}

impl std::error::Error for ConditionalReferenceErrorV1 {}

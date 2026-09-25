//! Compatibility imports for the shared inert logical signature projection.

pub(crate) use fe2o3_verifier::portable_reference_v1::signature::*;

#[cfg(test)]
use super::{MAX_REFERENCE_POINT_AXES_V1, ReferenceArgumentRelationV1, ReferenceScalarTypeV1};
#[cfg(test)]
use fe2o3_mir_model::semantic_mir_v1::{
    SemanticExternAbiV1, SemanticFunctionSafetyV1, SemanticMutabilityV1,
};

#[cfg(test)]
#[path = "reference_signature_preimage_v1_tests.rs"]
mod tests;

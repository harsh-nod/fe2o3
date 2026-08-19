use fe2o3_llvm_handoff::{
    DeviceLibraryInputV1, EvidenceV2, Gfx942HandoffV2, IdentityV1, MAX_DEVICE_LIBRARIES_V1,
    MAX_OBLIGATIONS_V1, MAX_ORIGINS_V1, ObligationV1, OriginKindV1, OriginV1, StageIdentitiesV1,
};
use sha2::{Digest as _, Sha256};

use crate::model::{ConstructionStageV1, LoweringErrorV1, NonGraphEnvelopeIdentityV1};

const NON_GRAPH_ENVELOPE_IDENTITY_DOMAIN_V1: &[u8] =
    b"fe2o3.lower-amdgcn-llvm.non-graph-envelope.identity.v1\0";

/// Immutable bounded data that is deliberately not represented by the Pliron LLVM graph.
///
/// LLVM target, module, global, function, CFG, instruction, type, and item-evidence
/// semantics are excluded. This envelope contains only prior-stage identities,
/// measured device-library inputs, source-origin records, and obligations.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CanonicalNonGraphEnvelopeV1 {
    pub(crate) stage_identities: StageIdentitiesV1,
    pub(crate) device_libraries: Vec<DeviceLibraryInputV1>,
    pub(crate) origins: Vec<OriginV1>,
    pub(crate) obligations: Vec<ObligationV1>,
    pub(crate) graph_origin: OriginV1,
    pub(crate) graph_evidence: EvidenceV2,
    pub(crate) identity: NonGraphEnvelopeIdentityV1,
}

impl CanonicalNonGraphEnvelopeV1 {
    pub(crate) fn from_source(source: &Gfx942HandoffV2) -> Result<Self, LoweringErrorV1> {
        let stage_identities = *source.base().stage_identities();
        let device_libraries = bounded_clone(
            source.base().module().device_libraries(),
            MAX_DEVICE_LIBRARIES_V1,
        )?;
        let origins = bounded_clone(source.base().origins(), MAX_ORIGINS_V1 - 1)?;
        let obligations = bounded_clone(source.base().obligations(), MAX_OBLIGATIONS_V1)?;
        let identity =
            calculate_identity(stage_identities, &device_libraries, &origins, &obligations);
        let graph_source = IdentityV1::new(identity.0)
            .map_err(|_| LoweringErrorV1::Construction(ConstructionStageV1::Receipt))?;
        let graph_origin = OriginV1::new(OriginKindV1::AmdgcnIr, graph_source, None);
        let graph_evidence = EvidenceV2::new(graph_origin.identity(), Vec::new())
            .map_err(|_| LoweringErrorV1::Construction(ConstructionStageV1::Receipt))?;
        Ok(Self {
            stage_identities,
            device_libraries,
            origins,
            obligations,
            graph_origin,
            graph_evidence,
            identity,
        })
    }

    /// Returns the independent identity of exactly the allowed non-graph fields.
    pub const fn identity(&self) -> NonGraphEnvelopeIdentityV1 {
        self.identity
    }

    /// Returns the number of measured device-library inputs in the envelope.
    pub fn device_library_count(&self) -> usize {
        self.device_libraries.len()
    }

    /// Returns the number of retained source-origin records, excluding graph-generated evidence.
    pub fn origin_count(&self) -> usize {
        self.origins.len()
    }

    /// Returns the number of retained non-graph obligations.
    pub fn obligation_count(&self) -> usize {
        self.obligations.len()
    }
}

fn bounded_clone<T: Clone>(source: &[T], maximum: usize) -> Result<Vec<T>, LoweringErrorV1> {
    if source.len() > maximum {
        return Err(LoweringErrorV1::Construction(ConstructionStageV1::Receipt));
    }
    let mut values = Vec::new();
    values
        .try_reserve_exact(source.len())
        .map_err(|_| LoweringErrorV1::Construction(ConstructionStageV1::Receipt))?;
    values.extend_from_slice(source);
    Ok(values)
}

fn calculate_identity(
    stage: StageIdentitiesV1,
    libraries: &[DeviceLibraryInputV1],
    origins: &[OriginV1],
    obligations: &[ObligationV1],
) -> NonGraphEnvelopeIdentityV1 {
    let mut hasher = Sha256::new();
    hasher.update(NON_GRAPH_ENVELOPE_IDENTITY_DOMAIN_V1);
    for identity in [stage.semantic(), stage.schedule(), stage.target_plan()] {
        hasher.update(identity.as_bytes());
    }
    hash_count(&mut hasher, libraries.len());
    for library in libraries {
        hash_bytes(&mut hasher, library.kind().canonical_name().as_bytes());
        hasher.update(library.sha256().as_bytes());
        hasher.update(library.byte_len().to_le_bytes());
    }
    hash_count(&mut hasher, origins.len());
    for origin in origins {
        hasher.update(origin.identity().as_bytes());
    }
    hash_count(&mut hasher, obligations.len());
    for obligation in obligations {
        hasher.update(obligation.identity().as_bytes());
    }
    NonGraphEnvelopeIdentityV1(hasher.finalize().into())
}

fn hash_count(hasher: &mut Sha256, count: usize) {
    hasher.update((count as u64).to_le_bytes());
}

fn hash_bytes(hasher: &mut Sha256, bytes: &[u8]) {
    hasher.update((bytes.len() as u64).to_le_bytes());
    hasher.update(bytes);
}

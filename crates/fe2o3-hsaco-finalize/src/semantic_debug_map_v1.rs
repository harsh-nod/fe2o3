//! Finalizer admission for exact-artifact-bound semantic debug maps.

use std::{error::Error, fmt};

use fe2o3_kernel_ir::{
    SemanticDebugMapDocumentV1, SemanticDebugMapErrorV1, semantic_debug_map_identity_v1,
};

use crate::{
    ContentIdentityV1, PreparedFinalizedProtectedWorkerV3HsacoV1,
    inspect_and_bind_kernel_descriptors,
};

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct FinalizedSemanticDebugMapIdentityV1([u8; 32]);

impl FinalizedSemanticDebugMapIdentityV1 {
    pub const fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}

/// Canonical semantic map admitted against one independently inspected finalized HSACO.
///
/// This record is descriptive evidence. It does not authenticate compiler execution and grants
/// no publication, load, launch, attach, or dispatch authority.
#[derive(Debug)]
pub struct AdmittedFinalizedSemanticDebugMapV1 {
    identity: FinalizedSemanticDebugMapIdentityV1,
    artifact_identity: ContentIdentityV1,
    canonical_bytes: Vec<u8>,
    document: SemanticDebugMapDocumentV1,
}

impl AdmittedFinalizedSemanticDebugMapV1 {
    pub const fn identity(&self) -> FinalizedSemanticDebugMapIdentityV1 {
        self.identity
    }

    pub const fn artifact_identity(&self) -> ContentIdentityV1 {
        self.artifact_identity
    }

    pub fn canonical_bytes(&self) -> &[u8] {
        &self.canonical_bytes
    }

    pub const fn document(&self) -> &SemanticDebugMapDocumentV1 {
        &self.document
    }

    pub const fn authenticates_compiler_execution(&self) -> bool {
        false
    }

    pub const fn grants_publication_authority(&self) -> bool {
        false
    }

    pub const fn grants_load_authority(&self) -> bool {
        false
    }

    pub const fn grants_launch_authority(&self) -> bool {
        false
    }
}

/// Independently parses the finalized HSACO, then admits only symbol-relative ISA intervals that
/// fit the ordinal-selected kernel entries and a map whose artifact digest and length are exact.
pub fn admit_finalized_semantic_debug_map_v1(
    map_bytes: &[u8],
    finalized_hsaco: &[u8],
) -> Result<AdmittedFinalizedSemanticDebugMapV1, FinalizedSemanticDebugMapErrorV1> {
    let inspected = inspect_and_bind_kernel_descriptors(finalized_hsaco)
        .map_err(|_| FinalizedSemanticDebugMapErrorV1::ArtifactInspection)?;
    let entry_sizes = inspected
        .bindings()
        .iter()
        .map(|binding| binding.entry_size())
        .collect::<Vec<_>>();
    admit_with_entry_sizes(map_bytes, finalized_hsaco, &entry_sizes)
}

impl PreparedFinalizedProtectedWorkerV3HsacoV1 {
    /// Admits a canonical semantic map against the exact bytes retained by this finalization.
    pub fn admit_semantic_debug_map_v1(
        &self,
        map_bytes: &[u8],
    ) -> Result<AdmittedFinalizedSemanticDebugMapV1, FinalizedSemanticDebugMapErrorV1> {
        admit_finalized_semantic_debug_map_v1(map_bytes, self.exact_finalized_bytes())
    }
}

fn admit_with_entry_sizes(
    map_bytes: &[u8],
    finalized_hsaco: &[u8],
    entry_sizes: &[u64],
) -> Result<AdmittedFinalizedSemanticDebugMapV1, FinalizedSemanticDebugMapErrorV1> {
    let document = SemanticDebugMapDocumentV1::from_canonical_json_bytes(map_bytes)
        .map_err(FinalizedSemanticDebugMapErrorV1::SemanticMap)?;
    document
        .validate_finalized_artifact(finalized_hsaco, entry_sizes)
        .map_err(FinalizedSemanticDebugMapErrorV1::SemanticMap)?;
    let canonical_bytes = map_bytes.to_vec();
    Ok(AdmittedFinalizedSemanticDebugMapV1 {
        identity: FinalizedSemanticDebugMapIdentityV1(semantic_debug_map_identity_v1(map_bytes)),
        artifact_identity: ContentIdentityV1::calculate(finalized_hsaco),
        canonical_bytes,
        document,
    })
}

#[derive(Debug)]
pub enum FinalizedSemanticDebugMapErrorV1 {
    SemanticMap(SemanticDebugMapErrorV1),
    ArtifactInspection,
}

impl fmt::Display for FinalizedSemanticDebugMapErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "finalized semantic debug map admission failed: {self:?}"
        )
    }
}

impl Error for FinalizedSemanticDebugMapErrorV1 {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::SemanticMap(error) => Some(error),
            Self::ArtifactInspection => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use fe2o3_kernel_ir::{
        SemanticDebugContentIdentityV1, SemanticDebugLayerV1, SemanticDebugLocationV1,
        SemanticDebugMapBindingV1, SemanticDebugMappingOutputV1, SemanticDebugMappingV1,
        SemanticDebugNodeV1, SemanticDebugTransformationV1,
    };

    use super::*;

    fn id(byte: u8) -> [u8; 32] {
        [byte; 32]
    }

    fn content(bytes: &[u8]) -> SemanticDebugContentIdentityV1 {
        SemanticDebugContentIdentityV1::calculate(bytes).unwrap()
    }

    fn map(artifact: &[u8], byte_end: u64) -> Vec<u8> {
        let binding = SemanticDebugMapBindingV1::new(
            content(b"source-map"),
            content(b"mir"),
            content(b"kir"),
            content(b"schedule"),
            content(b"llvm"),
            content(artifact),
        )
        .unwrap();
        let llvm = SemanticDebugNodeV1::new(
            id(1),
            SemanticDebugLocationV1::Llvm {
                function_ordinal: 0,
                block_ordinal: 0,
                instruction_ordinal: 0,
            },
        )
        .unwrap();
        let isa = SemanticDebugNodeV1::new(
            id(2),
            SemanticDebugLocationV1::Isa {
                kernel_ordinal: 0,
                byte_start: 0,
                byte_end,
            },
        )
        .unwrap();
        let mapping = SemanticDebugMappingV1::new(
            id(3),
            SemanticDebugLayerV1::Llvm,
            SemanticDebugLayerV1::Isa,
            SemanticDebugTransformationV1::Preserved,
            vec![id(1)],
            SemanticDebugMappingOutputV1::available(vec![id(2)]),
        )
        .unwrap();
        SemanticDebugMapDocumentV1::new(binding, vec![llvm, isa], vec![mapping])
            .unwrap()
            .to_canonical_json_bytes()
            .unwrap()
    }

    #[test]
    fn exact_artifact_and_symbol_relative_ranges_are_required() {
        let artifact = b"finalized-artifact";
        let bytes = map(artifact, 8);
        let admitted = admit_with_entry_sizes(&bytes, artifact, &[16]).unwrap();
        assert!(admitted.artifact_identity().matches(artifact));
        assert!(!admitted.authenticates_compiler_execution());
        assert!(!admitted.grants_load_authority());

        let mut substituted = artifact.to_vec();
        substituted[0] ^= 1;
        assert!(matches!(
            admit_with_entry_sizes(&bytes, &substituted, &[16]),
            Err(FinalizedSemanticDebugMapErrorV1::SemanticMap(
                SemanticDebugMapErrorV1::ArtifactBindingMismatch
            ))
        ));
        assert!(matches!(
            admit_with_entry_sizes(&bytes, artifact, &[4]),
            Err(FinalizedSemanticDebugMapErrorV1::SemanticMap(
                SemanticDebugMapErrorV1::InvalidIsaInterval
            ))
        ));
    }

    #[test]
    fn stale_or_noncanonical_map_is_not_admitted() {
        let artifact = b"finalized-artifact";
        let mut bytes = map(artifact, 8);
        bytes.push(b'\n');
        assert!(matches!(
            admit_with_entry_sizes(&bytes, artifact, &[16]),
            Err(FinalizedSemanticDebugMapErrorV1::SemanticMap(
                SemanticDebugMapErrorV1::NonCanonicalEncoding
            ))
        ));
    }
}

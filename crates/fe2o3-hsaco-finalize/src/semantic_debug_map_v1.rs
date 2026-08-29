//! Finalizer admission for exact-artifact-bound semantic debug maps.

use std::{error::Error, fmt};

use fe2o3_kernel_ir::{
    SemanticDebugMapDocumentV1, SemanticDebugMapErrorV1, SemanticDebugMapInputsV1,
    semantic_debug_map_identity_v1,
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

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FinalizedSemanticDebugMapAdmissionStatusV1 {
    ArtifactOnly,
    ExactInputsAndArtifact,
}

/// Canonical semantic map admitted against one independently inspected finalized HSACO.
///
/// This record is descriptive evidence. It does not authenticate compiler execution and grants
/// no publication, load, launch, attach, or dispatch authority. Callers must inspect
/// `admission_status`: the legacy artifact-only path does not validate the document's source-map,
/// MIR, KIR, schedule, or LLVM content axes.
#[derive(Debug)]
pub struct AdmittedFinalizedSemanticDebugMapV1 {
    identity: FinalizedSemanticDebugMapIdentityV1,
    artifact_identity: ContentIdentityV1,
    status: FinalizedSemanticDebugMapAdmissionStatusV1,
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

    pub const fn admission_status(&self) -> FinalizedSemanticDebugMapAdmissionStatusV1 {
        self.status
    }

    pub const fn validates_artifact_axis(&self) -> bool {
        true
    }

    pub const fn validates_all_input_axes(&self) -> bool {
        matches!(
            self.status,
            FinalizedSemanticDebugMapAdmissionStatusV1::ExactInputsAndArtifact
        )
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
    let entry_sizes = metadata_order_entry_sizes(&inspected)?;
    admit_with_entry_sizes_and_status(
        map_bytes,
        finalized_hsaco,
        &entry_sizes,
        FinalizedSemanticDebugMapAdmissionStatusV1::ArtifactOnly,
        None,
    )
}

/// Admits the map only after joining every declared compiler input axis, the Source Map V2 inner
/// canonical-KIR binding, and the independently inspected finalized artifact.
pub fn admit_finalized_semantic_debug_map_with_inputs_v1(
    map_bytes: &[u8],
    inputs: SemanticDebugMapInputsV1<'_>,
    finalized_hsaco: &[u8],
) -> Result<AdmittedFinalizedSemanticDebugMapV1, FinalizedSemanticDebugMapErrorV1> {
    let inspected = inspect_and_bind_kernel_descriptors(finalized_hsaco)
        .map_err(|_| FinalizedSemanticDebugMapErrorV1::ArtifactInspection)?;
    let entry_sizes = metadata_order_entry_sizes(&inspected)?;
    admit_with_entry_sizes_and_status(
        map_bytes,
        finalized_hsaco,
        &entry_sizes,
        FinalizedSemanticDebugMapAdmissionStatusV1::ExactInputsAndArtifact,
        Some(inputs),
    )
}

impl PreparedFinalizedProtectedWorkerV3HsacoV1 {
    /// Admits a canonical semantic map against the exact bytes retained by this finalization.
    pub fn admit_semantic_debug_map_v1(
        &self,
        map_bytes: &[u8],
    ) -> Result<AdmittedFinalizedSemanticDebugMapV1, FinalizedSemanticDebugMapErrorV1> {
        admit_finalized_semantic_debug_map_v1(map_bytes, self.exact_finalized_bytes())
    }

    /// Admits a semantic map with every compiler input axis joined to exact bytes.
    pub fn admit_semantic_debug_map_with_inputs_v1(
        &self,
        map_bytes: &[u8],
        inputs: SemanticDebugMapInputsV1<'_>,
    ) -> Result<AdmittedFinalizedSemanticDebugMapV1, FinalizedSemanticDebugMapErrorV1> {
        admit_finalized_semantic_debug_map_with_inputs_v1(
            map_bytes,
            inputs,
            self.exact_finalized_bytes(),
        )
    }
}

fn metadata_order_entry_sizes(
    inspected: &fe2o3_hsaco::InspectedKernelBindings,
) -> Result<Vec<u64>, FinalizedSemanticDebugMapErrorV1> {
    let mut entry_sizes = Vec::new();
    entry_sizes
        .try_reserve_exact(inspected.bindings().len())
        .map_err(|_| FinalizedSemanticDebugMapErrorV1::AllocationFailure)?;
    entry_sizes.extend(
        inspected
            .bindings()
            .iter()
            .map(|binding| binding.entry_size()),
    );
    Ok(entry_sizes)
}

#[cfg(test)]
fn admit_with_entry_sizes(
    map_bytes: &[u8],
    finalized_hsaco: &[u8],
    entry_sizes: &[u64],
) -> Result<AdmittedFinalizedSemanticDebugMapV1, FinalizedSemanticDebugMapErrorV1> {
    admit_with_entry_sizes_and_status(
        map_bytes,
        finalized_hsaco,
        entry_sizes,
        FinalizedSemanticDebugMapAdmissionStatusV1::ArtifactOnly,
        None,
    )
}

fn admit_with_entry_sizes_and_status(
    map_bytes: &[u8],
    finalized_hsaco: &[u8],
    entry_sizes: &[u64],
    status: FinalizedSemanticDebugMapAdmissionStatusV1,
    inputs: Option<SemanticDebugMapInputsV1<'_>>,
) -> Result<AdmittedFinalizedSemanticDebugMapV1, FinalizedSemanticDebugMapErrorV1> {
    let document = SemanticDebugMapDocumentV1::from_canonical_json_bytes(map_bytes)
        .map_err(FinalizedSemanticDebugMapErrorV1::SemanticMap)?;
    if let Some(inputs) = inputs {
        document
            .validate_exact_inputs(inputs)
            .map_err(FinalizedSemanticDebugMapErrorV1::SemanticMap)?;
    }
    document
        .validate_finalized_artifact(finalized_hsaco, entry_sizes)
        .map_err(FinalizedSemanticDebugMapErrorV1::SemanticMap)?;
    let mut canonical_bytes = Vec::new();
    canonical_bytes
        .try_reserve_exact(map_bytes.len())
        .map_err(|_| FinalizedSemanticDebugMapErrorV1::AllocationFailure)?;
    canonical_bytes.extend_from_slice(map_bytes);
    Ok(AdmittedFinalizedSemanticDebugMapV1 {
        identity: FinalizedSemanticDebugMapIdentityV1(semantic_debug_map_identity_v1(map_bytes)),
        artifact_identity: ContentIdentityV1::calculate(finalized_hsaco),
        status,
        canonical_bytes,
        document,
    })
}

#[derive(Debug)]
pub enum FinalizedSemanticDebugMapErrorV1 {
    SemanticMap(SemanticDebugMapErrorV1),
    ArtifactInspection,
    AllocationFailure,
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
            Self::ArtifactInspection | Self::AllocationFailure => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use fe2o3_kernel_ir::{
        DebugSourceMapBindingV1, DebugSourceMapDocumentV2, DebugSourceMapFileV1, Module,
        SemanticDebugBoundaryDirectionV1, SemanticDebugBoundaryReasonV1, SemanticDebugBoundaryV1,
        SemanticDebugContentIdentityV1, SemanticDebugLayerV1, SemanticDebugLocationV1,
        SemanticDebugMapBindingV1, SemanticDebugMappingOutputV1, SemanticDebugMappingV1,
        SemanticDebugNodeV1, SemanticDebugTransformationV1, VerifiedCanonicalKernelIrIdentityV7,
        VerifiedCanonicalKernelIrV7,
    };

    use super::*;

    fn id(byte: u8) -> [u8; 32] {
        [byte; 32]
    }

    fn content(bytes: &[u8]) -> SemanticDebugContentIdentityV1 {
        SemanticDebugContentIdentityV1::calculate(bytes).unwrap()
    }

    fn canonical_kir(module_id: &str) -> (Vec<u8>, VerifiedCanonicalKernelIrIdentityV7) {
        let owner = VerifiedCanonicalKernelIrV7::from_module(Module::new(module_id)).unwrap();
        let identity = *owner.identity();
        (owner.into_canonical_bytes(), identity)
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
        SemanticDebugMapDocumentV1::new_partial(
            binding,
            vec![llvm, isa],
            vec![mapping],
            vec![
                SemanticDebugBoundaryV1::new(
                    id(1),
                    SemanticDebugBoundaryDirectionV1::PredecessorUnavailable,
                    SemanticDebugBoundaryReasonV1::ProducerBoundary,
                )
                .unwrap(),
            ],
        )
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
        assert_eq!(
            admitted.admission_status(),
            FinalizedSemanticDebugMapAdmissionStatusV1::ArtifactOnly
        );
        assert!(!admitted.validates_all_input_axes());
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

    #[test]
    fn fully_joined_admission_is_distinct_and_rejects_source_map_kir_substitution() {
        let artifact = b"finalized-artifact";
        let (kir, kir_identity) = canonical_kir("finalizer-semantic-debug-map-test");
        let source_map = DebugSourceMapDocumentV2::new(
            DebugSourceMapBindingV1::new(
                id(70),
                *kir_identity.digest(),
                kir_identity.canonical_length(),
            )
            .unwrap(),
            vec![DebugSourceMapFileV1::new(id(71), 32, "/src/kernel.rs".into()).unwrap()],
            Vec::new(),
            Vec::new(),
            Vec::new(),
            Vec::new(),
        )
        .unwrap()
        .to_canonical_json_bytes()
        .unwrap();
        let mir = b"mir";
        let schedule = b"schedule";
        let llvm = b"llvm";
        let binding = SemanticDebugMapBindingV1::new(
            content(&source_map),
            content(mir),
            content(&kir),
            content(schedule),
            content(llvm),
            content(artifact),
        )
        .unwrap();
        let original =
            SemanticDebugMapDocumentV1::from_canonical_json_bytes(&map(artifact, 8)).unwrap();
        let joined_map = SemanticDebugMapDocumentV1::new_partial(
            binding,
            original.nodes().to_vec(),
            original.mappings().to_vec(),
            original.boundaries().to_vec(),
        )
        .unwrap()
        .to_canonical_json_bytes()
        .unwrap();
        let inputs = SemanticDebugMapInputsV1 {
            source_map_v2: &source_map,
            semantic_mir: mir,
            canonical_kir: &kir,
            schedule,
            llvm_module: llvm,
            finalized_artifact: artifact,
        };
        let admitted = admit_with_entry_sizes_and_status(
            &joined_map,
            artifact,
            &[16],
            FinalizedSemanticDebugMapAdmissionStatusV1::ExactInputsAndArtifact,
            Some(inputs),
        )
        .unwrap();
        assert_eq!(
            admitted.admission_status(),
            FinalizedSemanticDebugMapAdmissionStatusV1::ExactInputsAndArtifact
        );
        assert!(admitted.validates_all_input_axes());

        let (_, other_identity) = canonical_kir("substituted-finalizer-semantic-debug-map-test");
        let substituted_source_map = DebugSourceMapDocumentV2::new(
            DebugSourceMapBindingV1::new(
                id(70),
                *other_identity.digest(),
                other_identity.canonical_length(),
            )
            .unwrap(),
            vec![DebugSourceMapFileV1::new(id(71), 32, "/src/kernel.rs".into()).unwrap()],
            Vec::new(),
            Vec::new(),
            Vec::new(),
            Vec::new(),
        )
        .unwrap()
        .to_canonical_json_bytes()
        .unwrap();
        let substituted_binding = SemanticDebugMapBindingV1::new(
            content(&substituted_source_map),
            content(mir),
            content(&kir),
            content(schedule),
            content(llvm),
            content(artifact),
        )
        .unwrap();
        let substituted_map = SemanticDebugMapDocumentV1::new_partial(
            substituted_binding,
            original.nodes().to_vec(),
            original.mappings().to_vec(),
            original.boundaries().to_vec(),
        )
        .unwrap()
        .to_canonical_json_bytes()
        .unwrap();
        assert!(matches!(
            admit_with_entry_sizes_and_status(
                &substituted_map,
                artifact,
                &[16],
                FinalizedSemanticDebugMapAdmissionStatusV1::ExactInputsAndArtifact,
                Some(SemanticDebugMapInputsV1 {
                    source_map_v2: &substituted_source_map,
                    ..inputs
                }),
            ),
            Err(FinalizedSemanticDebugMapErrorV1::SemanticMap(
                SemanticDebugMapErrorV1::SourceMapKirBindingMismatch
            ))
        ));
    }
}

//! Exact cross-layer cardinality and authenticated transformation observations.
//!
//! This additive sidecar binds one frozen Semantic Debug Map V1 without changing that format. It
//! deliberately separates an exact input/output relation from its transformation classification:
//! cardinality alone cannot prove duplication, fusion, outlining, inlining, or movement.

use std::{error::Error, fmt, io, io::Write};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::{
    SemanticDebugContentIdentityV1, SemanticDebugLayerV1, SemanticDebugMapBindingV1,
    SemanticDebugMapDocumentV1,
};

pub const SEMANTIC_DEBUG_TRANSFORMATION_MAP_SCHEMA_V2: &str =
    "fe2o3-semantic-debug-transformation-map-v2";
pub const MAX_SEMANTIC_DEBUG_TRANSFORMATION_MAP_BYTES_V2: usize = 64 * 1024 * 1024;
pub const MAX_SEMANTIC_DEBUG_TRANSFORMATION_EVIDENCE_V2: usize = 64;
pub const MAX_SEMANTIC_DEBUG_TRANSFORMATION_RELATIONS_V2: usize = 262_144;
pub const MAX_SEMANTIC_DEBUG_TRANSFORMATION_REFERENCES_V2: usize = 1_048_576;

const EVIDENCE_IDENTITY_DOMAIN_V2: &[u8] = b"FE2O3/SEMANTIC-DEBUG-TRANSFORMATION-EVIDENCE/V2\0";
const RELATION_IDENTITY_DOMAIN_V2: &[u8] = b"FE2O3/SEMANTIC-DEBUG-TRANSFORMATION-RELATION/V2\0";
const MAP_IDENTITY_DOMAIN_V2: &[u8] = b"FE2O3/SEMANTIC-DEBUG-TRANSFORMATION-MAP/V2\0";

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SemanticDebugTransformationEvidenceKindV2 {
    LegacySemanticDebugMapV1,
    SourceMirCorrespondence,
    MirKirCorrespondenceV4,
    MirKirCorrespondenceV5,
    ScheduledLowering,
    CompilerLlvmOrigin,
    FinalizerSourceIsaCatalogV1,
}

impl SemanticDebugTransformationEvidenceKindV2 {
    const fn code(self) -> u8 {
        match self {
            Self::LegacySemanticDebugMapV1 => 1,
            Self::SourceMirCorrespondence => 2,
            Self::MirKirCorrespondenceV4 => 3,
            Self::MirKirCorrespondenceV5 => 7,
            Self::ScheduledLowering => 4,
            Self::CompilerLlvmOrigin => 5,
            Self::FinalizerSourceIsaCatalogV1 => 6,
        }
    }
}

/// One exact producer evidence object. Its identity is derived, never caller-selected.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SemanticDebugTransformationEvidenceV2 {
    #[serde(with = "hex_identity_v2")]
    identity: [u8; 32],
    kind: SemanticDebugTransformationEvidenceKindV2,
    content: SemanticDebugContentIdentityV1,
}

impl SemanticDebugTransformationEvidenceV2 {
    pub fn from_exact_bytes(
        kind: SemanticDebugTransformationEvidenceKindV2,
        bytes: &[u8],
    ) -> Result<Self, SemanticDebugTransformationMapErrorV2> {
        let content = SemanticDebugContentIdentityV1::calculate(bytes)
            .map_err(|_| SemanticDebugTransformationMapErrorV2::InvalidEvidence)?;
        let identity = evidence_identity(kind, content);
        Ok(Self {
            identity,
            kind,
            content,
        })
    }

    pub const fn identity(self) -> [u8; 32] {
        self.identity
    }

    pub const fn kind(self) -> SemanticDebugTransformationEvidenceKindV2 {
        self.kind
    }

    pub const fn content(self) -> SemanticDebugContentIdentityV1 {
        self.content
    }

    pub fn matches_exact_bytes(self, bytes: &[u8]) -> bool {
        self.content.matches(bytes) && self.identity == evidence_identity(self.kind, self.content)
    }

    fn validate(self) -> Result<(), SemanticDebugTransformationMapErrorV2> {
        if self.identity == [0; 32]
            || self.content.sha256() == [0; 32]
            || self.content.byte_len() == 0
            || self.identity != evidence_identity(self.kind, self.content)
        {
            Err(SemanticDebugTransformationMapErrorV2::InvalidEvidence)
        } else {
            Ok(())
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SemanticDebugTransformationClassV2 {
    Duplicated,
    Fused,
    Outlined,
    Inlined,
    Moved,
    Eliminated,
}

impl SemanticDebugTransformationClassV2 {
    const ALL: [Self; 6] = [
        Self::Duplicated,
        Self::Fused,
        Self::Outlined,
        Self::Inlined,
        Self::Moved,
        Self::Eliminated,
    ];

    pub const fn all_v2() -> [Self; 6] {
        Self::ALL
    }

    const fn code(self) -> u8 {
        match self {
            Self::Duplicated => 1,
            Self::Fused => 2,
            Self::Outlined => 3,
            Self::Inlined => 4,
            Self::Moved => 5,
            Self::Eliminated => 6,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SemanticDebugTransformationAvailabilityV2 {
    AuthenticatedProducer,
    UnavailableNoAuthenticatedProducer,
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SemanticDebugTransformationCapabilityV2 {
    class: SemanticDebugTransformationClassV2,
    availability: SemanticDebugTransformationAvailabilityV2,
}

impl SemanticDebugTransformationCapabilityV2 {
    pub const fn new(
        class: SemanticDebugTransformationClassV2,
        availability: SemanticDebugTransformationAvailabilityV2,
    ) -> Self {
        Self {
            class,
            availability,
        }
    }

    pub const fn class(self) -> SemanticDebugTransformationClassV2 {
        self.class
    }

    pub const fn availability(self) -> SemanticDebugTransformationAvailabilityV2 {
        self.availability
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SemanticDebugRelationCardinalityV2 {
    OneToOne,
    OneToMany,
    ManyToOne,
    ManyToMany,
    Eliminated,
}

impl SemanticDebugRelationCardinalityV2 {
    const fn from_counts(inputs: usize, outputs: usize) -> Option<Self> {
        match (inputs, outputs) {
            (0, _) => None,
            (_, 0) => Some(Self::Eliminated),
            (1, 1) => Some(Self::OneToOne),
            (1, _) => Some(Self::OneToMany),
            (_, 1) => Some(Self::ManyToOne),
            (_, _) => Some(Self::ManyToMany),
        }
    }

    const fn code(self) -> u8 {
        match self {
            Self::OneToOne => 1,
            Self::OneToMany => 2,
            Self::ManyToOne => 3,
            Self::ManyToMany => 4,
            Self::Eliminated => 5,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SemanticDebugTransformationUnavailableReasonV2 {
    ProducerDidNotClassify,
    NoAuthenticatedProducer,
    LegacyClaimNotAuthenticated,
    UnsupportedLayer,
}

/// Classification is independent of the exact relation. `Unavailable` retains all endpoints.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Deserialize, Serialize)]
#[serde(tag = "status", rename_all = "snake_case", deny_unknown_fields)]
pub enum SemanticDebugTransformationClassificationV2 {
    Preserved,
    Observed {
        class: SemanticDebugTransformationClassV2,
    },
    Unavailable {
        reason: SemanticDebugTransformationUnavailableReasonV2,
    },
}

#[derive(Clone, Debug, Eq, PartialEq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SemanticDebugTransformationRelationV2 {
    #[serde(with = "hex_identity_v2")]
    identity: [u8; 32],
    input_layer: SemanticDebugLayerV1,
    output_layer: SemanticDebugLayerV1,
    cardinality: SemanticDebugRelationCardinalityV2,
    classification: SemanticDebugTransformationClassificationV2,
    #[serde(with = "hex_identity_v2")]
    evidence: [u8; 32],
    #[serde(with = "hex_identities_v2")]
    inputs: Vec<[u8; 32]>,
    #[serde(with = "hex_identities_v2")]
    outputs: Vec<[u8; 32]>,
}

impl SemanticDebugTransformationRelationV2 {
    pub fn new(
        input_layer: SemanticDebugLayerV1,
        output_layer: SemanticDebugLayerV1,
        evidence: [u8; 32],
        inputs: Vec<[u8; 32]>,
        outputs: Vec<[u8; 32]>,
        classification: SemanticDebugTransformationClassificationV2,
    ) -> Result<Self, SemanticDebugTransformationMapErrorV2> {
        let cardinality =
            SemanticDebugRelationCardinalityV2::from_counts(inputs.len(), outputs.len())
                .ok_or(SemanticDebugTransformationMapErrorV2::InvalidRelation)?;
        let mut relation = Self {
            identity: [0; 32],
            input_layer,
            output_layer,
            cardinality,
            classification,
            evidence,
            inputs,
            outputs,
        };
        relation.normalize()?;
        relation.identity = relation_identity(&relation);
        relation.validate_shape()?;
        Ok(relation)
    }

    pub const fn identity(&self) -> [u8; 32] {
        self.identity
    }
    pub const fn input_layer(&self) -> SemanticDebugLayerV1 {
        self.input_layer
    }
    pub const fn output_layer(&self) -> SemanticDebugLayerV1 {
        self.output_layer
    }
    pub const fn cardinality(&self) -> SemanticDebugRelationCardinalityV2 {
        self.cardinality
    }
    pub const fn classification(&self) -> SemanticDebugTransformationClassificationV2 {
        self.classification
    }
    pub const fn evidence(&self) -> [u8; 32] {
        self.evidence
    }
    pub fn inputs(&self) -> &[[u8; 32]] {
        &self.inputs
    }
    pub fn outputs(&self) -> &[[u8; 32]] {
        &self.outputs
    }

    fn normalize(&mut self) -> Result<(), SemanticDebugTransformationMapErrorV2> {
        let references = self
            .inputs
            .len()
            .checked_add(self.outputs.len())
            .ok_or(SemanticDebugTransformationMapErrorV2::ResourceLimit)?;
        if references > MAX_SEMANTIC_DEBUG_TRANSFORMATION_REFERENCES_V2 {
            return Err(SemanticDebugTransformationMapErrorV2::ResourceLimit);
        }
        self.inputs.sort_unstable();
        self.outputs.sort_unstable();
        Ok(())
    }

    fn validate_shape(&self) -> Result<(), SemanticDebugTransformationMapErrorV2> {
        if self.identity == [0; 32]
            || self.evidence == [0; 32]
            || self.input_layer.successor_v2() != Some(self.output_layer)
            || self.inputs.is_empty()
            || self.inputs.contains(&[0; 32])
            || self.outputs.contains(&[0; 32])
            || self.inputs.windows(2).any(|pair| pair[0] >= pair[1])
            || self.outputs.windows(2).any(|pair| pair[0] >= pair[1])
            || self.cardinality
                != SemanticDebugRelationCardinalityV2::from_counts(
                    self.inputs.len(),
                    self.outputs.len(),
                )
                .ok_or(SemanticDebugTransformationMapErrorV2::InvalidRelation)?
            || self.identity != relation_identity(self)
        {
            return Err(SemanticDebugTransformationMapErrorV2::InvalidRelation);
        }
        let shape_matches = match self.classification {
            SemanticDebugTransformationClassificationV2::Preserved => {
                self.cardinality == SemanticDebugRelationCardinalityV2::OneToOne
            }
            SemanticDebugTransformationClassificationV2::Observed { class } => match class {
                SemanticDebugTransformationClassV2::Eliminated => {
                    self.cardinality == SemanticDebugRelationCardinalityV2::Eliminated
                }
                SemanticDebugTransformationClassV2::Duplicated
                | SemanticDebugTransformationClassV2::Fused
                | SemanticDebugTransformationClassV2::Outlined
                | SemanticDebugTransformationClassV2::Inlined
                | SemanticDebugTransformationClassV2::Moved => !self.outputs.is_empty(),
            },
            SemanticDebugTransformationClassificationV2::Unavailable { .. } => true,
        };
        if shape_matches {
            Ok(())
        } else {
            Err(SemanticDebugTransformationMapErrorV2::InvalidRelation)
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SemanticDebugTransformationMapBindingV2 {
    semantic_debug_map_v1: SemanticDebugContentIdentityV1,
    semantic_debug_map_binding_v1: SemanticDebugMapBindingV1,
}

impl SemanticDebugTransformationMapBindingV2 {
    pub fn from_exact_map(map_bytes: &[u8]) -> Result<Self, SemanticDebugTransformationMapErrorV2> {
        let map = SemanticDebugMapDocumentV1::from_canonical_json_bytes(map_bytes)
            .map_err(|_| SemanticDebugTransformationMapErrorV2::InvalidSemanticMap)?;
        Ok(Self {
            semantic_debug_map_v1: SemanticDebugContentIdentityV1::calculate(map_bytes)
                .map_err(|_| SemanticDebugTransformationMapErrorV2::InvalidSemanticMap)?,
            semantic_debug_map_binding_v1: map.binding(),
        })
    }

    pub const fn semantic_debug_map_v1(self) -> SemanticDebugContentIdentityV1 {
        self.semantic_debug_map_v1
    }
    pub const fn semantic_debug_map_binding_v1(self) -> SemanticDebugMapBindingV1 {
        self.semantic_debug_map_binding_v1
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SemanticDebugTransformationMapDocumentV2 {
    schema: SemanticDebugTransformationMapSchemaV2,
    binding: SemanticDebugTransformationMapBindingV2,
    #[serde(deserialize_with = "bounded_capabilities_v2::deserialize")]
    capabilities: Vec<SemanticDebugTransformationCapabilityV2>,
    #[serde(deserialize_with = "bounded_evidence_v2::deserialize")]
    evidence: Vec<SemanticDebugTransformationEvidenceV2>,
    #[serde(deserialize_with = "bounded_relations_v2::deserialize")]
    relations: Vec<SemanticDebugTransformationRelationV2>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Deserialize, Serialize)]
enum SemanticDebugTransformationMapSchemaV2 {
    #[serde(rename = "fe2o3-semantic-debug-transformation-map-v2")]
    V2,
}

pub struct SemanticDebugTransformationEvidenceInputV2<'a> {
    pub kind: SemanticDebugTransformationEvidenceKindV2,
    pub canonical_bytes: &'a [u8],
}

impl SemanticDebugTransformationMapDocumentV2 {
    /// Projects a frozen V1 graph without treating its transformation labels as observations.
    ///
    /// V1 retains exact endpoints and content axes, but its canonical bytes do not identify an
    /// authenticated transformation producer. Every imported classification is therefore typed
    /// unavailable while its exact cardinality remains queryable.
    pub fn from_legacy_semantic_map_v1(
        semantic_map_bytes: &[u8],
    ) -> Result<Self, SemanticDebugTransformationMapErrorV2> {
        let semantic_map =
            SemanticDebugMapDocumentV1::from_canonical_json_bytes(semantic_map_bytes)
                .map_err(|_| SemanticDebugTransformationMapErrorV2::InvalidSemanticMap)?;
        let binding = SemanticDebugTransformationMapBindingV2::from_exact_map(semantic_map_bytes)?;
        let evidence = SemanticDebugTransformationEvidenceV2::from_exact_bytes(
            SemanticDebugTransformationEvidenceKindV2::LegacySemanticDebugMapV1,
            semantic_map_bytes,
        )?;
        let capabilities = SemanticDebugTransformationClassV2::all_v2()
            .into_iter()
            .map(|class| {
                SemanticDebugTransformationCapabilityV2::new(
                    class,
                    SemanticDebugTransformationAvailabilityV2::UnavailableNoAuthenticatedProducer,
                )
            })
            .collect();
        let mut relations = Vec::new();
        relations
            .try_reserve_exact(semantic_map.mappings().len())
            .map_err(|_| SemanticDebugTransformationMapErrorV2::AllocationFailure)?;
        for mapping in semantic_map.mappings() {
            let mut inputs = Vec::new();
            inputs
                .try_reserve_exact(mapping.inputs().len())
                .map_err(|_| SemanticDebugTransformationMapErrorV2::AllocationFailure)?;
            inputs.extend_from_slice(mapping.inputs());
            let mut outputs = Vec::new();
            outputs
                .try_reserve_exact(mapping.output().nodes().len())
                .map_err(|_| SemanticDebugTransformationMapErrorV2::AllocationFailure)?;
            outputs.extend_from_slice(mapping.output().nodes());
            relations.push(SemanticDebugTransformationRelationV2::new(
                mapping.input_layer(),
                mapping.output_layer(),
                evidence.identity(),
                inputs,
                outputs,
                SemanticDebugTransformationClassificationV2::Unavailable {
                    reason:
                        SemanticDebugTransformationUnavailableReasonV2::LegacyClaimNotAuthenticated,
                },
            )?);
        }
        Self::new(
            binding,
            capabilities,
            vec![evidence],
            relations,
            &semantic_map,
        )
    }

    pub fn new(
        binding: SemanticDebugTransformationMapBindingV2,
        mut capabilities: Vec<SemanticDebugTransformationCapabilityV2>,
        mut evidence: Vec<SemanticDebugTransformationEvidenceV2>,
        mut relations: Vec<SemanticDebugTransformationRelationV2>,
        semantic_map: &SemanticDebugMapDocumentV1,
    ) -> Result<Self, SemanticDebugTransformationMapErrorV2> {
        capabilities.sort_unstable();
        evidence.sort_unstable();
        relations.sort_unstable_by_key(SemanticDebugTransformationRelationV2::identity);
        let document = Self {
            schema: SemanticDebugTransformationMapSchemaV2::V2,
            binding,
            capabilities,
            evidence,
            relations,
        };
        document.validate(semantic_map)?;
        Ok(document)
    }

    pub fn from_canonical_json_bytes(
        bytes: &[u8],
        semantic_map_bytes: &[u8],
    ) -> Result<Self, SemanticDebugTransformationMapErrorV2> {
        if bytes.is_empty() || bytes.len() > MAX_SEMANTIC_DEBUG_TRANSFORMATION_MAP_BYTES_V2 {
            return Err(SemanticDebugTransformationMapErrorV2::InvalidLength);
        }
        let document = serde_json::from_slice::<Self>(bytes)
            .map_err(|_| SemanticDebugTransformationMapErrorV2::InvalidJson)?;
        let semantic_map =
            SemanticDebugMapDocumentV1::from_canonical_json_bytes(semantic_map_bytes)
                .map_err(|_| SemanticDebugTransformationMapErrorV2::InvalidSemanticMap)?;
        document.validate(&semantic_map)?;
        document.validate_map_binding(semantic_map_bytes)?;
        if document.to_canonical_json_bytes()? != bytes {
            return Err(SemanticDebugTransformationMapErrorV2::NonCanonicalEncoding);
        }
        Ok(document)
    }

    pub fn to_canonical_json_bytes(
        &self,
    ) -> Result<Vec<u8>, SemanticDebugTransformationMapErrorV2> {
        let mut writer = BoundedWriterV2::new(MAX_SEMANTIC_DEBUG_TRANSFORMATION_MAP_BYTES_V2);
        serde_json::to_writer(&mut writer, self).map_err(|_| writer.error())?;
        writer.finish()
    }

    pub const fn binding(&self) -> SemanticDebugTransformationMapBindingV2 {
        self.binding
    }
    pub fn capabilities(&self) -> &[SemanticDebugTransformationCapabilityV2] {
        &self.capabilities
    }
    pub fn evidence(&self) -> &[SemanticDebugTransformationEvidenceV2] {
        &self.evidence
    }
    pub fn relations(&self) -> &[SemanticDebugTransformationRelationV2] {
        &self.relations
    }

    pub fn relations_from(
        &self,
        identity: [u8; 32],
    ) -> impl Iterator<Item = &SemanticDebugTransformationRelationV2> {
        self.relations
            .iter()
            .filter(move |relation| relation.inputs.binary_search(&identity).is_ok())
    }

    pub fn relations_to(
        &self,
        identity: [u8; 32],
    ) -> impl Iterator<Item = &SemanticDebugTransformationRelationV2> {
        self.relations
            .iter()
            .filter(move |relation| relation.outputs.binary_search(&identity).is_ok())
    }

    pub fn validate_exact_inputs(
        &self,
        semantic_map_bytes: &[u8],
        evidence_inputs: &[SemanticDebugTransformationEvidenceInputV2<'_>],
    ) -> Result<(), SemanticDebugTransformationMapErrorV2> {
        self.validate_map_binding(semantic_map_bytes)?;
        if evidence_inputs.len() != self.evidence.len() {
            return Err(SemanticDebugTransformationMapErrorV2::EvidenceBindingMismatch);
        }
        for (claim, input) in self.evidence.iter().zip(evidence_inputs) {
            if claim.kind != input.kind || !claim.matches_exact_bytes(input.canonical_bytes) {
                return Err(SemanticDebugTransformationMapErrorV2::EvidenceBindingMismatch);
            }
        }
        Ok(())
    }

    pub fn identity(&self) -> Result<[u8; 32], SemanticDebugTransformationMapErrorV2> {
        let bytes = self.to_canonical_json_bytes()?;
        let mut digest = Sha256::new();
        digest.update(MAP_IDENTITY_DOMAIN_V2);
        digest.update((bytes.len() as u64).to_le_bytes());
        digest.update(bytes);
        Ok(digest.finalize().into())
    }

    pub const fn grants_compiler_authority(&self) -> bool {
        false
    }
    pub const fn grants_artifact_authority(&self) -> bool {
        false
    }
    pub const fn grants_runtime_authority(&self) -> bool {
        false
    }

    fn validate_map_binding(
        &self,
        semantic_map_bytes: &[u8],
    ) -> Result<(), SemanticDebugTransformationMapErrorV2> {
        let map = SemanticDebugMapDocumentV1::from_canonical_json_bytes(semantic_map_bytes)
            .map_err(|_| SemanticDebugTransformationMapErrorV2::InvalidSemanticMap)?;
        if !self
            .binding
            .semantic_debug_map_v1
            .matches(semantic_map_bytes)
            || self.binding.semantic_debug_map_binding_v1 != map.binding()
        {
            Err(SemanticDebugTransformationMapErrorV2::MapBindingMismatch)
        } else {
            Ok(())
        }
    }

    fn validate(
        &self,
        semantic_map: &SemanticDebugMapDocumentV1,
    ) -> Result<(), SemanticDebugTransformationMapErrorV2> {
        if self.binding.semantic_debug_map_binding_v1 != semantic_map.binding()
            || self.capabilities.len() != SemanticDebugTransformationClassV2::ALL.len()
            || self.evidence.is_empty()
            || self.evidence.len() > MAX_SEMANTIC_DEBUG_TRANSFORMATION_EVIDENCE_V2
            || self.relations.len() > MAX_SEMANTIC_DEBUG_TRANSFORMATION_RELATIONS_V2
            || self.capabilities.windows(2).any(|pair| pair[0] >= pair[1])
            || self.evidence.windows(2).any(|pair| pair[0] >= pair[1])
            || self
                .relations
                .windows(2)
                .any(|pair| pair[0].identity >= pair[1].identity)
            || self
                .capabilities
                .iter()
                .map(|capability| capability.class)
                .ne(SemanticDebugTransformationClassV2::ALL)
        {
            return Err(SemanticDebugTransformationMapErrorV2::InvalidDocument);
        }
        let total_references = self.relations.iter().try_fold(0_usize, |count, relation| {
            count
                .checked_add(relation.inputs.len())?
                .checked_add(relation.outputs.len())
        });
        if total_references
            .is_none_or(|count| count > MAX_SEMANTIC_DEBUG_TRANSFORMATION_REFERENCES_V2)
        {
            return Err(SemanticDebugTransformationMapErrorV2::ResourceLimit);
        }
        for evidence in &self.evidence {
            evidence.validate()?;
        }
        for relation in &self.relations {
            relation.validate_shape()?;
            if self
                .evidence
                .binary_search_by_key(&relation.evidence, |evidence| evidence.identity)
                .is_err()
            {
                return Err(SemanticDebugTransformationMapErrorV2::UnknownEvidence);
            }
            for input in &relation.inputs {
                if semantic_map
                    .node(*input)
                    .is_none_or(|node| node.layer() != relation.input_layer)
                {
                    return Err(SemanticDebugTransformationMapErrorV2::UnknownNode);
                }
            }
            for output in &relation.outputs {
                if semantic_map
                    .node(*output)
                    .is_none_or(|node| node.layer() != relation.output_layer)
                {
                    return Err(SemanticDebugTransformationMapErrorV2::UnknownNode);
                }
            }
            if let SemanticDebugTransformationClassificationV2::Observed { class } =
                relation.classification
            {
                let capability = self
                    .capabilities
                    .binary_search_by_key(&class, |capability| capability.class)
                    .ok()
                    .and_then(|index| self.capabilities.get(index));
                if capability.is_none_or(|capability| {
                    capability.availability
                        != SemanticDebugTransformationAvailabilityV2::AuthenticatedProducer
                }) {
                    return Err(SemanticDebugTransformationMapErrorV2::UnauthenticatedObservation);
                }
            }
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SemanticDebugTransformationMapErrorV2 {
    InvalidLength,
    InvalidJson,
    NonCanonicalEncoding,
    InvalidSemanticMap,
    InvalidEvidence,
    InvalidRelation,
    InvalidDocument,
    UnknownEvidence,
    UnknownNode,
    UnauthenticatedObservation,
    MapBindingMismatch,
    EvidenceBindingMismatch,
    ResourceLimit,
    AllocationFailure,
    Encoding,
}

impl fmt::Display for SemanticDebugTransformationMapErrorV2 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "semantic debug transformation map V2 failed: {self:?}"
        )
    }
}

impl Error for SemanticDebugTransformationMapErrorV2 {}

impl SemanticDebugLayerV1 {
    const fn successor_v2(self) -> Option<Self> {
        match self {
            Self::Source => Some(Self::Mir),
            Self::Mir => Some(Self::Kir),
            Self::Kir => Some(Self::Schedule),
            Self::Schedule => Some(Self::Llvm),
            Self::Llvm => Some(Self::Isa),
            Self::Isa => None,
        }
    }
}

fn evidence_identity(
    kind: SemanticDebugTransformationEvidenceKindV2,
    content: SemanticDebugContentIdentityV1,
) -> [u8; 32] {
    let mut digest = Sha256::new();
    digest.update(EVIDENCE_IDENTITY_DOMAIN_V2);
    digest.update([kind.code()]);
    digest.update(content.sha256());
    digest.update(content.byte_len().to_le_bytes());
    digest.finalize().into()
}

fn relation_identity(relation: &SemanticDebugTransformationRelationV2) -> [u8; 32] {
    let mut digest = Sha256::new();
    digest.update(RELATION_IDENTITY_DOMAIN_V2);
    digest.update([
        layer_code(relation.input_layer),
        layer_code(relation.output_layer),
    ]);
    digest.update([relation.cardinality.code()]);
    match relation.classification {
        SemanticDebugTransformationClassificationV2::Preserved => digest.update([1, 0]),
        SemanticDebugTransformationClassificationV2::Observed { class } => {
            digest.update([2, class.code()]);
        }
        SemanticDebugTransformationClassificationV2::Unavailable { reason } => {
            digest.update([3, unavailable_reason_code(reason)]);
        }
    }
    digest.update(relation.evidence);
    digest.update((relation.inputs.len() as u64).to_le_bytes());
    for input in &relation.inputs {
        digest.update(input);
    }
    digest.update((relation.outputs.len() as u64).to_le_bytes());
    for output in &relation.outputs {
        digest.update(output);
    }
    digest.finalize().into()
}

const fn layer_code(layer: SemanticDebugLayerV1) -> u8 {
    match layer {
        SemanticDebugLayerV1::Source => 1,
        SemanticDebugLayerV1::Mir => 2,
        SemanticDebugLayerV1::Kir => 3,
        SemanticDebugLayerV1::Schedule => 4,
        SemanticDebugLayerV1::Llvm => 5,
        SemanticDebugLayerV1::Isa => 6,
    }
}

const fn unavailable_reason_code(reason: SemanticDebugTransformationUnavailableReasonV2) -> u8 {
    match reason {
        SemanticDebugTransformationUnavailableReasonV2::ProducerDidNotClassify => 1,
        SemanticDebugTransformationUnavailableReasonV2::NoAuthenticatedProducer => 2,
        SemanticDebugTransformationUnavailableReasonV2::LegacyClaimNotAuthenticated => 3,
        SemanticDebugTransformationUnavailableReasonV2::UnsupportedLayer => 4,
    }
}

struct BoundedWriterV2 {
    bytes: Vec<u8>,
    limit: usize,
    failed: Option<SemanticDebugTransformationMapErrorV2>,
}

impl BoundedWriterV2 {
    fn new(limit: usize) -> Self {
        Self {
            bytes: Vec::new(),
            limit,
            failed: None,
        }
    }

    fn error(&self) -> SemanticDebugTransformationMapErrorV2 {
        self.failed
            .unwrap_or(SemanticDebugTransformationMapErrorV2::Encoding)
    }

    fn finish(self) -> Result<Vec<u8>, SemanticDebugTransformationMapErrorV2> {
        if let Some(error) = self.failed {
            return Err(error);
        }
        if self.bytes.is_empty() || self.bytes.len() > self.limit {
            return Err(SemanticDebugTransformationMapErrorV2::InvalidLength);
        }
        Ok(self.bytes)
    }
}

impl Write for BoundedWriterV2 {
    fn write(&mut self, buffer: &[u8]) -> io::Result<usize> {
        let Some(next) = self.bytes.len().checked_add(buffer.len()) else {
            self.failed = Some(SemanticDebugTransformationMapErrorV2::ResourceLimit);
            return Err(io::Error::other("transformation map length overflow"));
        };
        if next > self.limit {
            self.failed = Some(SemanticDebugTransformationMapErrorV2::InvalidLength);
            return Err(io::Error::other("transformation map exceeds limit"));
        }
        self.bytes.try_reserve_exact(buffer.len()).map_err(|_| {
            self.failed = Some(SemanticDebugTransformationMapErrorV2::AllocationFailure);
            io::Error::other("transformation map allocation failed")
        })?;
        self.bytes.extend_from_slice(buffer);
        Ok(buffer.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

mod hex_identity_v2 {
    use serde::{Deserialize, Deserializer, Serializer, de};

    pub fn serialize<S: Serializer>(identity: &[u8; 32], serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&hex(identity))
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(deserializer: D) -> Result<[u8; 32], D::Error> {
        let text = String::deserialize(deserializer)?;
        decode(&text).ok_or_else(|| de::Error::custom("expected lowercase 32-byte hex identity"))
    }

    pub(super) fn hex(identity: &[u8; 32]) -> String {
        const HEX: &[u8; 16] = b"0123456789abcdef";
        let mut output = String::with_capacity(64);
        for byte in identity {
            output.push(char::from(HEX[usize::from(byte >> 4)]));
            output.push(char::from(HEX[usize::from(byte & 0xf)]));
        }
        output
    }

    pub(super) fn decode(text: &str) -> Option<[u8; 32]> {
        if text.len() != 64 {
            return None;
        }
        let mut output = [0_u8; 32];
        for (index, pair) in text.as_bytes().chunks_exact(2).enumerate() {
            output[index] = nibble(pair[0])?
                .checked_mul(16)?
                .checked_add(nibble(pair[1])?)?;
        }
        Some(output)
    }

    const fn nibble(byte: u8) -> Option<u8> {
        match byte {
            b'0'..=b'9' => Some(byte - b'0'),
            b'a'..=b'f' => Some(byte - b'a' + 10),
            _ => None,
        }
    }
}

mod hex_identities_v2 {
    use std::fmt;

    use serde::{
        Deserializer, Serializer,
        de::{self, SeqAccess, Visitor},
        ser::SerializeSeq,
    };

    use super::{HexIdentityV2, MAX_SEMANTIC_DEBUG_TRANSFORMATION_REFERENCES_V2};

    pub fn serialize<S: Serializer>(
        identities: &[[u8; 32]],
        serializer: S,
    ) -> Result<S::Ok, S::Error> {
        let mut sequence = serializer.serialize_seq(Some(identities.len()))?;
        for identity in identities {
            sequence.serialize_element(&HexIdentityV2(*identity))?;
        }
        sequence.end()
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(
        deserializer: D,
    ) -> Result<Vec<[u8; 32]>, D::Error> {
        struct IdentityVisitorV2;

        impl<'de> Visitor<'de> for IdentityVisitorV2 {
            type Value = Vec<[u8; 32]>;

            fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                formatter.write_str("a bounded sequence of semantic node identities")
            }

            fn visit_seq<A>(self, mut sequence: A) -> Result<Self::Value, A::Error>
            where
                A: SeqAccess<'de>,
            {
                let mut values = Vec::new();
                while let Some(identity) = sequence.next_element::<HexIdentityV2>()? {
                    if values.len() == MAX_SEMANTIC_DEBUG_TRANSFORMATION_REFERENCES_V2 {
                        return Err(de::Error::custom(
                            "transformation reference count exceeds wire limit",
                        ));
                    }
                    if values.len() == values.capacity() {
                        let remaining =
                            MAX_SEMANTIC_DEBUG_TRANSFORMATION_REFERENCES_V2 - values.len();
                        values
                            .try_reserve_exact(remaining.min(1024))
                            .map_err(|_| de::Error::custom("identity allocation failed"))?;
                    }
                    values.push(identity.0);
                }
                Ok(values)
            }
        }

        deserializer.deserialize_seq(IdentityVisitorV2)
    }
}

fn deserialize_bounded_vec_v2<'de, D, T>(
    deserializer: D,
    max: usize,
    description: &'static str,
) -> Result<Vec<T>, D::Error>
where
    D: serde::Deserializer<'de>,
    T: Deserialize<'de>,
{
    use std::{fmt, marker::PhantomData};

    use serde::de::{self, SeqAccess, Visitor};

    struct BoundedVecVisitorV2<T> {
        max: usize,
        description: &'static str,
        marker: PhantomData<T>,
    }

    impl<'de, T> Visitor<'de> for BoundedVecVisitorV2<T>
    where
        T: Deserialize<'de>,
    {
        type Value = Vec<T>;

        fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
            write!(formatter, "a bounded sequence of {}", self.description)
        }

        fn visit_seq<A>(self, mut sequence: A) -> Result<Self::Value, A::Error>
        where
            A: SeqAccess<'de>,
        {
            let mut values = Vec::new();
            while let Some(value) = sequence.next_element()? {
                if values.len() == self.max {
                    return Err(de::Error::custom(
                        "transformation map sequence exceeds wire limit",
                    ));
                }
                if values.len() == values.capacity() {
                    let remaining = self.max - values.len();
                    values
                        .try_reserve_exact(remaining.min(256))
                        .map_err(|_| de::Error::custom("sequence allocation failed"))?;
                }
                values.push(value);
            }
            Ok(values)
        }
    }

    deserializer.deserialize_seq(BoundedVecVisitorV2 {
        max,
        description,
        marker: PhantomData,
    })
}

mod bounded_capabilities_v2 {
    use super::*;

    pub fn deserialize<'de, D>(
        deserializer: D,
    ) -> Result<Vec<SemanticDebugTransformationCapabilityV2>, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        deserialize_bounded_vec_v2(
            deserializer,
            SemanticDebugTransformationClassV2::ALL.len(),
            "transformation capabilities",
        )
    }
}

mod bounded_evidence_v2 {
    use super::*;

    pub fn deserialize<'de, D>(
        deserializer: D,
    ) -> Result<Vec<SemanticDebugTransformationEvidenceV2>, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        deserialize_bounded_vec_v2(
            deserializer,
            MAX_SEMANTIC_DEBUG_TRANSFORMATION_EVIDENCE_V2,
            "transformation evidence",
        )
    }
}

mod bounded_relations_v2 {
    use super::*;

    pub fn deserialize<'de, D>(
        deserializer: D,
    ) -> Result<Vec<SemanticDebugTransformationRelationV2>, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        deserialize_bounded_vec_v2(
            deserializer,
            MAX_SEMANTIC_DEBUG_TRANSFORMATION_RELATIONS_V2,
            "transformation relations",
        )
    }
}

#[derive(Clone, Copy, Deserialize, Serialize)]
#[serde(transparent)]
struct HexIdentityV2(#[serde(with = "hex_identity_v2")] [u8; 32]);

#[cfg(test)]
mod tests {
    use crate::{
        DebugSourceMapSpanV1, SemanticDebugBoundaryDirectionV1, SemanticDebugBoundaryReasonV1,
        SemanticDebugBoundaryV1, SemanticDebugLocationV1, SemanticDebugMapDocumentV1,
        SemanticDebugMappingOutputV1, SemanticDebugMappingV1, SemanticDebugNodeV1,
        SemanticDebugTransformationV1,
    };

    use super::*;

    fn id(byte: u8) -> [u8; 32] {
        [byte; 32]
    }

    fn content(bytes: &[u8]) -> SemanticDebugContentIdentityV1 {
        SemanticDebugContentIdentityV1::calculate(bytes).unwrap()
    }

    fn node(byte: u8, location: SemanticDebugLocationV1) -> SemanticDebugNodeV1 {
        SemanticDebugNodeV1::new(id(byte), location).unwrap()
    }

    fn fixture_map() -> SemanticDebugMapDocumentV1 {
        fixture_map_with_artifact(b"finalized-artifact")
    }

    fn fixture_map_with_artifact(artifact: &[u8]) -> SemanticDebugMapDocumentV1 {
        let nodes = vec![
            node(
                1,
                SemanticDebugLocationV1::Source {
                    span: DebugSourceMapSpanV1::new(id(90), 0, 4, 1, 1).unwrap(),
                },
            ),
            node(
                2,
                SemanticDebugLocationV1::Source {
                    span: DebugSourceMapSpanV1::new_eliminated(id(90), 4, 8, 1, 5).unwrap(),
                },
            ),
            node(
                3,
                SemanticDebugLocationV1::Mir {
                    body_ordinal: 0,
                    block_ordinal: 0,
                    statement_ordinal: 0,
                },
            ),
            node(
                4,
                SemanticDebugLocationV1::Mir {
                    body_ordinal: 0,
                    block_ordinal: 0,
                    statement_ordinal: 1,
                },
            ),
            node(
                5,
                SemanticDebugLocationV1::Kir {
                    function_ordinal: 0,
                    block_ordinal: 0,
                    operation_ordinal: 0,
                },
            ),
            node(
                6,
                SemanticDebugLocationV1::Kir {
                    function_ordinal: 0,
                    block_ordinal: 0,
                    operation_ordinal: 1,
                },
            ),
            node(
                7,
                SemanticDebugLocationV1::Schedule {
                    function_ordinal: 0,
                    region_ordinal: 0,
                    operation_ordinal: 0,
                },
            ),
            node(
                8,
                SemanticDebugLocationV1::Schedule {
                    function_ordinal: 0,
                    region_ordinal: 0,
                    operation_ordinal: 1,
                },
            ),
            node(
                9,
                SemanticDebugLocationV1::Llvm {
                    function_ordinal: 0,
                    block_ordinal: 0,
                    instruction_ordinal: 0,
                },
            ),
            node(
                10,
                SemanticDebugLocationV1::Llvm {
                    function_ordinal: 0,
                    block_ordinal: 0,
                    instruction_ordinal: 1,
                },
            ),
            node(
                11,
                SemanticDebugLocationV1::Isa {
                    kernel_ordinal: 0,
                    byte_start: 0,
                    byte_end: 4,
                },
            ),
            node(
                12,
                SemanticDebugLocationV1::Isa {
                    kernel_ordinal: 0,
                    byte_start: 4,
                    byte_end: 8,
                },
            ),
        ];
        let mut boundaries = Vec::new();
        for node in &nodes {
            if node.layer() != SemanticDebugLayerV1::Isa && node.identity() != id(1) {
                boundaries.push(
                    SemanticDebugBoundaryV1::new(
                        node.identity(),
                        SemanticDebugBoundaryDirectionV1::SuccessorUnavailable,
                        SemanticDebugBoundaryReasonV1::NotRepresented,
                    )
                    .unwrap(),
                );
            }
            if node.layer() != SemanticDebugLayerV1::Source && node.identity() != id(3) {
                boundaries.push(
                    SemanticDebugBoundaryV1::new(
                        node.identity(),
                        SemanticDebugBoundaryDirectionV1::PredecessorUnavailable,
                        SemanticDebugBoundaryReasonV1::NotRepresented,
                    )
                    .unwrap(),
                );
            }
        }
        SemanticDebugMapDocumentV1::new_partial(
            SemanticDebugMapBindingV1::new(
                content(b"source-map-v2"),
                content(b"semantic-mir"),
                content(b"canonical-kir"),
                content(b"schedule"),
                content(b"llvm-module"),
                content(artifact),
            )
            .unwrap(),
            nodes,
            vec![
                SemanticDebugMappingV1::new(
                    id(80),
                    SemanticDebugLayerV1::Source,
                    SemanticDebugLayerV1::Mir,
                    SemanticDebugTransformationV1::Preserved,
                    vec![id(1)],
                    SemanticDebugMappingOutputV1::available(vec![id(3)]),
                )
                .unwrap(),
            ],
            boundaries,
        )
        .unwrap()
    }

    fn all_available_capabilities() -> Vec<SemanticDebugTransformationCapabilityV2> {
        SemanticDebugTransformationClassV2::all_v2()
            .into_iter()
            .map(|class| {
                SemanticDebugTransformationCapabilityV2::new(
                    class,
                    SemanticDebugTransformationAvailabilityV2::AuthenticatedProducer,
                )
            })
            .collect()
    }

    fn observed(
        class: SemanticDebugTransformationClassV2,
    ) -> SemanticDebugTransformationClassificationV2 {
        SemanticDebugTransformationClassificationV2::Observed { class }
    }

    fn complete_fixture() -> (
        SemanticDebugMapDocumentV1,
        Vec<u8>,
        Vec<u8>,
        SemanticDebugTransformationMapDocumentV2,
    ) {
        let map = fixture_map();
        let map_bytes = map.to_canonical_json_bytes().unwrap();
        let evidence_bytes = b"authenticated-producer-evidence".to_vec();
        let evidence = SemanticDebugTransformationEvidenceV2::from_exact_bytes(
            SemanticDebugTransformationEvidenceKindV2::MirKirCorrespondenceV4,
            &evidence_bytes,
        )
        .unwrap();
        let evidence_id = evidence.identity();
        let relations = vec![
            SemanticDebugTransformationRelationV2::new(
                SemanticDebugLayerV1::Source,
                SemanticDebugLayerV1::Mir,
                evidence_id,
                vec![id(1)],
                vec![id(3), id(4)],
                observed(SemanticDebugTransformationClassV2::Duplicated),
            )
            .unwrap(),
            SemanticDebugTransformationRelationV2::new(
                SemanticDebugLayerV1::Mir,
                SemanticDebugLayerV1::Kir,
                evidence_id,
                vec![id(3), id(4)],
                vec![id(5)],
                observed(SemanticDebugTransformationClassV2::Fused),
            )
            .unwrap(),
            SemanticDebugTransformationRelationV2::new(
                SemanticDebugLayerV1::Kir,
                SemanticDebugLayerV1::Schedule,
                evidence_id,
                vec![id(5)],
                vec![id(7), id(8)],
                observed(SemanticDebugTransformationClassV2::Outlined),
            )
            .unwrap(),
            SemanticDebugTransformationRelationV2::new(
                SemanticDebugLayerV1::Schedule,
                SemanticDebugLayerV1::Llvm,
                evidence_id,
                vec![id(7), id(8)],
                vec![id(9), id(10)],
                observed(SemanticDebugTransformationClassV2::Inlined),
            )
            .unwrap(),
            SemanticDebugTransformationRelationV2::new(
                SemanticDebugLayerV1::Llvm,
                SemanticDebugLayerV1::Isa,
                evidence_id,
                vec![id(9)],
                vec![id(11)],
                observed(SemanticDebugTransformationClassV2::Moved),
            )
            .unwrap(),
            SemanticDebugTransformationRelationV2::new(
                SemanticDebugLayerV1::Source,
                SemanticDebugLayerV1::Mir,
                evidence_id,
                vec![id(2)],
                Vec::new(),
                observed(SemanticDebugTransformationClassV2::Eliminated),
            )
            .unwrap(),
        ];
        let document = SemanticDebugTransformationMapDocumentV2::new(
            SemanticDebugTransformationMapBindingV2::from_exact_map(&map_bytes).unwrap(),
            all_available_capabilities(),
            vec![evidence],
            relations,
            &map,
        )
        .unwrap();
        (map, map_bytes, evidence_bytes, document)
    }

    #[test]
    fn every_cardinality_and_transformation_round_trips_without_unique_owner_assumptions() {
        let (_map, map_bytes, evidence_bytes, document) = complete_fixture();
        let bytes = document.to_canonical_json_bytes().unwrap();
        let decoded =
            SemanticDebugTransformationMapDocumentV2::from_canonical_json_bytes(&bytes, &map_bytes)
                .unwrap();
        decoded
            .validate_exact_inputs(
                &map_bytes,
                &[SemanticDebugTransformationEvidenceInputV2 {
                    kind: SemanticDebugTransformationEvidenceKindV2::MirKirCorrespondenceV4,
                    canonical_bytes: &evidence_bytes,
                }],
            )
            .unwrap();
        assert_eq!(decoded, document);
        assert_eq!(decoded.relations_from(id(1)).count(), 1);
        assert_eq!(decoded.relations_to(id(5)).count(), 1);
        assert_eq!(
            decoded
                .relations()
                .iter()
                .map(SemanticDebugTransformationRelationV2::cardinality)
                .collect::<std::collections::BTreeSet<_>>(),
            [
                SemanticDebugRelationCardinalityV2::OneToOne,
                SemanticDebugRelationCardinalityV2::OneToMany,
                SemanticDebugRelationCardinalityV2::ManyToOne,
                SemanticDebugRelationCardinalityV2::ManyToMany,
                SemanticDebugRelationCardinalityV2::Eliminated,
            ]
            .into_iter()
            .collect()
        );
        assert_ne!(decoded.identity().unwrap(), [0; 32]);
        assert!(!decoded.grants_compiler_authority());
        assert!(!decoded.grants_artifact_authority());
        assert!(!decoded.grants_runtime_authority());
    }

    #[test]
    fn stale_map_evidence_nodes_and_claims_fail_closed() {
        let (map, map_bytes, evidence_bytes, document) = complete_fixture();
        let mut stale_map = map_bytes.clone();
        stale_map.push(b'\n');
        assert_eq!(
            document.validate_exact_inputs(
                &stale_map,
                &[SemanticDebugTransformationEvidenceInputV2 {
                    kind: SemanticDebugTransformationEvidenceKindV2::MirKirCorrespondenceV4,
                    canonical_bytes: &evidence_bytes,
                }]
            ),
            Err(SemanticDebugTransformationMapErrorV2::InvalidSemanticMap)
        );
        assert_eq!(
            document.validate_exact_inputs(
                &map_bytes,
                &[SemanticDebugTransformationEvidenceInputV2 {
                    kind: SemanticDebugTransformationEvidenceKindV2::MirKirCorrespondenceV4,
                    canonical_bytes: b"substituted-evidence",
                }]
            ),
            Err(SemanticDebugTransformationMapErrorV2::EvidenceBindingMismatch)
        );
        assert_eq!(
            document.validate_exact_inputs(
                &map_bytes,
                &[SemanticDebugTransformationEvidenceInputV2 {
                    kind: SemanticDebugTransformationEvidenceKindV2::CompilerLlvmOrigin,
                    canonical_bytes: &evidence_bytes,
                }]
            ),
            Err(SemanticDebugTransformationMapErrorV2::EvidenceBindingMismatch)
        );
        let substituted_map = fixture_map_with_artifact(b"other-finalized-artifact")
            .to_canonical_json_bytes()
            .unwrap();
        assert_eq!(
            document.validate_exact_inputs(
                &substituted_map,
                &[SemanticDebugTransformationEvidenceInputV2 {
                    kind: SemanticDebugTransformationEvidenceKindV2::MirKirCorrespondenceV4,
                    canonical_bytes: &evidence_bytes,
                }]
            ),
            Err(SemanticDebugTransformationMapErrorV2::MapBindingMismatch)
        );

        let evidence = document.evidence()[0];
        let unknown_node = SemanticDebugTransformationRelationV2::new(
            SemanticDebugLayerV1::Source,
            SemanticDebugLayerV1::Mir,
            evidence.identity(),
            vec![id(99)],
            vec![id(3)],
            SemanticDebugTransformationClassificationV2::Preserved,
        )
        .unwrap();
        assert_eq!(
            SemanticDebugTransformationMapDocumentV2::new(
                document.binding(),
                document.capabilities().to_vec(),
                document.evidence().to_vec(),
                vec![unknown_node],
                &map,
            ),
            Err(SemanticDebugTransformationMapErrorV2::UnknownNode)
        );

        let unavailable = SemanticDebugTransformationClassV2::all_v2()
            .into_iter()
            .map(|class| {
                SemanticDebugTransformationCapabilityV2::new(
                    class,
                    SemanticDebugTransformationAvailabilityV2::UnavailableNoAuthenticatedProducer,
                )
            })
            .collect();
        assert_eq!(
            SemanticDebugTransformationMapDocumentV2::new(
                document.binding(),
                unavailable,
                document.evidence().to_vec(),
                document.relations().to_vec(),
                &map,
            ),
            Err(SemanticDebugTransformationMapErrorV2::UnauthenticatedObservation)
        );
    }

    #[test]
    fn cardinality_does_not_silently_become_a_transformation_observation() {
        let map = fixture_map();
        let map_bytes = map.to_canonical_json_bytes().unwrap();
        let evidence = SemanticDebugTransformationEvidenceV2::from_exact_bytes(
            SemanticDebugTransformationEvidenceKindV2::MirKirCorrespondenceV4,
            b"cardinality-only",
        )
        .unwrap();
        let relation = SemanticDebugTransformationRelationV2::new(
            SemanticDebugLayerV1::Mir,
            SemanticDebugLayerV1::Kir,
            evidence.identity(),
            vec![id(3)],
            vec![id(5), id(6)],
            SemanticDebugTransformationClassificationV2::Unavailable {
                reason: SemanticDebugTransformationUnavailableReasonV2::ProducerDidNotClassify,
            },
        )
        .unwrap();
        assert_eq!(
            relation.cardinality(),
            SemanticDebugRelationCardinalityV2::OneToMany
        );
        assert_eq!(
            relation.classification(),
            SemanticDebugTransformationClassificationV2::Unavailable {
                reason: SemanticDebugTransformationUnavailableReasonV2::ProducerDidNotClassify,
            }
        );
        let capabilities = SemanticDebugTransformationClassV2::all_v2()
            .into_iter()
            .map(|class| {
                SemanticDebugTransformationCapabilityV2::new(
                    class,
                    SemanticDebugTransformationAvailabilityV2::UnavailableNoAuthenticatedProducer,
                )
            })
            .collect();
        SemanticDebugTransformationMapDocumentV2::new(
            SemanticDebugTransformationMapBindingV2::from_exact_map(&map_bytes).unwrap(),
            capabilities,
            vec![evidence],
            vec![relation],
            &map,
        )
        .unwrap();
    }

    #[test]
    fn hostile_canonical_fields_and_derived_limits_fail_closed() {
        let (_map, map_bytes, _evidence_bytes, document) = complete_fixture();
        let bytes = document.to_canonical_json_bytes().unwrap();
        let mut value = serde_json::from_slice::<serde_json::Value>(&bytes).unwrap();
        value["relations"][0]["identity"] = serde_json::Value::String("11".repeat(32));
        let resealed = serde_json::to_vec(&value).unwrap();
        assert_eq!(
            SemanticDebugTransformationMapDocumentV2::from_canonical_json_bytes(
                &resealed, &map_bytes
            ),
            Err(SemanticDebugTransformationMapErrorV2::InvalidRelation)
        );

        let mut reordered =
            serde_json::from_slice::<SemanticDebugTransformationMapDocumentV2>(&bytes).unwrap();
        let relation = reordered
            .relations
            .iter_mut()
            .find(|relation| relation.outputs.len() == 2)
            .unwrap();
        relation.outputs.swap(0, 1);
        relation.identity = relation_identity(relation);
        reordered
            .relations
            .sort_unstable_by_key(SemanticDebugTransformationRelationV2::identity);
        let resealed = serde_json::to_vec(&reordered).unwrap();
        assert_eq!(
            SemanticDebugTransformationMapDocumentV2::from_canonical_json_bytes(
                &resealed, &map_bytes
            ),
            Err(SemanticDebugTransformationMapErrorV2::InvalidRelation)
        );

        let mut value = serde_json::from_slice::<serde_json::Value>(&bytes).unwrap();
        value["relations"][0]["evidence"] = serde_json::Value::String("22".repeat(32));
        let resealed = serde_json::to_vec(&value).unwrap();
        assert_eq!(
            SemanticDebugTransformationMapDocumentV2::from_canonical_json_bytes(
                &resealed, &map_bytes
            ),
            Err(SemanticDebugTransformationMapErrorV2::InvalidRelation)
        );

        let mut value = serde_json::from_slice::<serde_json::Value>(&bytes).unwrap();
        let capabilities = value["capabilities"].as_array_mut().unwrap();
        capabilities.push(capabilities[0].clone());
        let oversized = serde_json::to_vec(&value).unwrap();
        assert_eq!(
            SemanticDebugTransformationMapDocumentV2::from_canonical_json_bytes(
                &oversized, &map_bytes
            ),
            Err(SemanticDebugTransformationMapErrorV2::InvalidJson)
        );

        let mut uppercase = bytes.clone();
        let position = uppercase
            .windows(64)
            .position(|window| window.iter().all(u8::is_ascii_hexdigit))
            .expect("fixture contains a hex identity");
        uppercase[position] = uppercase[position].to_ascii_uppercase();
        if uppercase[position].is_ascii_digit() {
            uppercase[position] = b'A';
        }
        assert!(matches!(
            SemanticDebugTransformationMapDocumentV2::from_canonical_json_bytes(
                &uppercase, &map_bytes
            ),
            Err(SemanticDebugTransformationMapErrorV2::InvalidJson)
                | Err(SemanticDebugTransformationMapErrorV2::InvalidEvidence)
                | Err(SemanticDebugTransformationMapErrorV2::InvalidRelation)
        ));
    }

    #[test]
    fn constructing_v2_does_not_change_frozen_v1_bytes() {
        let (_map, map_bytes, _evidence_bytes, document) = complete_fixture();
        let before = map_bytes.clone();
        let legacy_projection =
            SemanticDebugTransformationMapDocumentV2::from_legacy_semantic_map_v1(&map_bytes)
                .unwrap();
        let v2_bytes = document.to_canonical_json_bytes().unwrap();
        assert_eq!(map_bytes, before);
        assert_eq!(
            SemanticDebugMapDocumentV1::from_canonical_json_bytes(&before)
                .unwrap()
                .to_canonical_json_bytes()
                .unwrap(),
            before
        );
        assert!(SemanticDebugMapDocumentV1::from_canonical_json_bytes(&v2_bytes).is_err());
        assert!(
            SemanticDebugTransformationMapDocumentV2::from_canonical_json_bytes(
                &map_bytes, &map_bytes
            )
            .is_err()
        );
        assert!(
            SemanticDebugTransformationMapDocumentV2::from_canonical_json_bytes(
                &v2_bytes, &map_bytes
            )
            .is_ok()
        );
        assert!(legacy_projection.relations().iter().all(|relation| {
            relation.classification()
                == SemanticDebugTransformationClassificationV2::Unavailable {
                    reason:
                        SemanticDebugTransformationUnavailableReasonV2::LegacyClaimNotAuthenticated,
                }
        }));
        legacy_projection
            .validate_exact_inputs(
                &map_bytes,
                &[SemanticDebugTransformationEvidenceInputV2 {
                    kind: SemanticDebugTransformationEvidenceKindV2::LegacySemanticDebugMapV1,
                    canonical_bytes: &map_bytes,
                }],
            )
            .unwrap();
    }
}

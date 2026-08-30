//! Exact production Source/MIR/KIR-to-sparse-ISA correlation.
//!
//! This contract composes two independently admitted records: the frozen Source/MIR/KIR V7
//! debug projection and compiler-inserted target-KIR/Worker-input-LLVM/final-HSACO anchors. The
//! join is by authenticated ordinal coordinates only. It does not infer a schedule, optimized
//! LLVM custody, complete instruction coverage, live wave state, or semantic refinement.

use std::{error::Error, fmt};

use dialect_amdgcn::{
    CanonicalProductionKirToLlvmReplayEvidenceV1, ProductionReplayKernelIrVersionV1,
    ProductionTargetStructuralBindingV1,
};
use fe2o3_kernel_ir::{
    DebugSourceMapSpanV1, MAX_SEMANTIC_DEBUG_MAPPING_REFERENCES_V1, MAX_SEMANTIC_DEBUG_NODES_V1,
    ProductionSemanticDebugProducerGapV1, SemanticDebugLayerV1, SemanticDebugLocationV1,
};
use sha2::{Digest, Sha256};

use crate::{
    AdmittedProductionSemanticAnchorV1, ContentIdentityV1,
    FinalizedSemanticDebugMapAdmissionStatusV1, FinalizedSemanticDebugMapErrorV1,
    PreparedFinalizedProtectedWorkerV3HsacoV1, ProductionFinalizedSemanticDebugAdmissionV1,
    ProductionSemanticAnchorAdmissionV1, ProductionSemanticAnchorErrorV1,
    ProductionSemanticAnchorTransformationV1, ProductionSemanticAnchorUnavailableV1,
};

const CORRELATION_IDENTITY_DOMAIN_V1: &[u8] = b"FE2O3/PRODUCTION-SOURCE-ISA-CORRELATION/V1\0";
const MAX_CORRELATION_RECORDS_V1: usize =
    MAX_SEMANTIC_DEBUG_NODES_V1 + dialect_amdgcn::MAX_PRODUCTION_SEMANTIC_ANCHORS_V1;

/// Why an exact joint correlation cannot be admitted from otherwise valid retained evidence.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProductionSourceIsaCorrelationUnavailableV1 {
    SemanticDebugCarrier(ProductionSemanticDebugProducerGapV1),
    SemanticAnchors(ProductionSemanticAnchorUnavailableV1),
    /// The source carrier has an exact V7-to-V8 debug projection, but no authenticated V8-to-V9
    /// source projection exists. V1 therefore refuses to infer V9 source coordinates.
    SourceProjectionForKirV9,
}

#[derive(Debug)]
pub enum ProductionSourceIsaCorrelationAdmissionV1 {
    Admitted(Box<AdmittedProductionSourceIsaCorrelationV1>),
    Unavailable(ProductionSourceIsaCorrelationUnavailableV1),
}

/// Closed failures while joining independently admitted source and sparse-ISA evidence.
#[derive(Debug)]
pub enum ProductionSourceIsaCorrelationErrorV1 {
    SemanticDebugMap(FinalizedSemanticDebugMapErrorV1),
    SemanticAnchors(ProductionSemanticAnchorErrorV1),
    InvalidKirToLlvmReplay,
    NonExactSemanticMap,
    ArtifactIdentityMismatch,
    TargetKirIdentityMismatch,
    CoordinateShapeMismatch,
    InvalidSourceGraph,
    ResourceLimit,
    AllocationFailure,
}

impl fmt::Display for ProductionSourceIsaCorrelationErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "production Source-to-sparse-ISA correlation admission failed: {self:?}"
        )
    }
}

impl Error for ProductionSourceIsaCorrelationErrorV1 {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::SemanticDebugMap(error) => Some(error),
            Self::SemanticAnchors(error) => Some(error),
            _ => None,
        }
    }
}

/// The exact availability of one source/target coordinate record.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProductionSourceIsaRecordKindV1 {
    /// Source and MIR exist, but the exact production correspondence records no KIR operation.
    EliminatedBeforeKir,
    /// A source/MIR/KIR record joins one compiler anchor. Its ISA list may still be empty when the
    /// pseudo-probe was eliminated by the backend.
    SourceAnchored,
    /// The target KIR operation has a compiler anchor but no source node in the exact carrier.
    NoSourceProvenance,
}

/// One name-independent, admitted relationship. Optional fields are controlled by `kind()`.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AdmittedProductionSourceIsaRecordV1 {
    kind: ProductionSourceIsaRecordKindV1,
    source_node_identity: Option<[u8; 32]>,
    source_span: Option<DebugSourceMapSpanV1>,
    mir_node_identity: Option<[u8; 32]>,
    mir: Option<SemanticDebugLocationV1>,
    neutral_kir_node_identity: Option<[u8; 32]>,
    neutral_kir: Option<SemanticDebugLocationV1>,
    target_kir: Option<SemanticDebugLocationV1>,
    semantic_operation_id: Option<[u8; 32]>,
    compiler_handoff_llvm: Option<SemanticDebugLocationV1>,
    isa: Vec<SemanticDebugLocationV1>,
    anchor_transformation: Option<ProductionSemanticAnchorTransformationV1>,
}

impl AdmittedProductionSourceIsaRecordV1 {
    pub const fn kind(&self) -> ProductionSourceIsaRecordKindV1 {
        self.kind
    }

    pub const fn source_node_identity(&self) -> Option<[u8; 32]> {
        self.source_node_identity
    }

    pub const fn source_span(&self) -> Option<DebugSourceMapSpanV1> {
        self.source_span
    }

    pub const fn mir_node_identity(&self) -> Option<[u8; 32]> {
        self.mir_node_identity
    }

    pub const fn mir(&self) -> Option<SemanticDebugLocationV1> {
        self.mir
    }

    pub const fn neutral_kir_node_identity(&self) -> Option<[u8; 32]> {
        self.neutral_kir_node_identity
    }

    pub const fn neutral_kir(&self) -> Option<SemanticDebugLocationV1> {
        self.neutral_kir
    }

    pub const fn target_kir(&self) -> Option<SemanticDebugLocationV1> {
        self.target_kir
    }

    pub const fn semantic_operation_id(&self) -> Option<[u8; 32]> {
        self.semantic_operation_id
    }

    /// Exact coordinate in the compiler handoff consumed by the Worker, not optimized/final LLVM.
    pub const fn compiler_handoff_llvm(&self) -> Option<SemanticDebugLocationV1> {
        self.compiler_handoff_llvm
    }

    /// Sparse, exact four-byte final-HSACO anchor intervals. An empty slice is an admitted backend
    /// elimination outcome, not evidence that the target KIR operation did not execute.
    pub fn isa(&self) -> &[SemanticDebugLocationV1] {
        &self.isa
    }

    pub const fn anchor_transformation(&self) -> Option<ProductionSemanticAnchorTransformationV1> {
        self.anchor_transformation
    }
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct ProductionIsaPointV1 {
    kernel_ordinal: u64,
    symbol_relative_pc: u64,
}

impl ProductionIsaPointV1 {
    pub const fn new(kernel_ordinal: u64, symbol_relative_pc: u64) -> Self {
        Self {
            kernel_ordinal,
            symbol_relative_pc,
        }
    }

    pub const fn kernel_ordinal(self) -> u64 {
        self.kernel_ordinal
    }

    pub const fn symbol_relative_pc(self) -> u64 {
        self.symbol_relative_pc
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProductionSourceIsaQueryUnavailableV1 {
    UnknownSourceNode,
    UnknownSourceSpan,
    UnalignedProgramCounter,
    UnknownMetadataKernelOrdinal,
    ProgramCounterIsNotAnAdmittedAnchor,
}

#[derive(Debug)]
enum MatchIndexSliceV1<'a> {
    SourceNode(&'a [([u8; 32], usize)]),
    SourceSpan(&'a [(DebugSourceMapSpanV1, usize)]),
    Isa(&'a [(ProductionIsaPointV1, usize)]),
}

/// Allocation-free iterator over one exact, pre-indexed query result.
#[derive(Debug)]
pub struct ProductionSourceIsaMatchesV1<'a> {
    records: &'a [AdmittedProductionSourceIsaRecordV1],
    index: MatchIndexSliceV1<'a>,
    next: usize,
}

impl<'a> Iterator for ProductionSourceIsaMatchesV1<'a> {
    type Item = &'a AdmittedProductionSourceIsaRecordV1;

    fn next(&mut self) -> Option<Self::Item> {
        let record_index = match &self.index {
            MatchIndexSliceV1::SourceNode(entries) => entries.get(self.next).map(|entry| entry.1),
            MatchIndexSliceV1::SourceSpan(entries) => entries.get(self.next).map(|entry| entry.1),
            MatchIndexSliceV1::Isa(entries) => entries.get(self.next).map(|entry| entry.1),
        }?;
        self.next += 1;
        self.records.get(record_index)
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let length = match &self.index {
            MatchIndexSliceV1::SourceNode(entries) => entries.len(),
            MatchIndexSliceV1::SourceSpan(entries) => entries.len(),
            MatchIndexSliceV1::Isa(entries) => entries.len(),
        };
        let remaining = length.saturating_sub(self.next);
        (remaining, Some(remaining))
    }
}

impl ExactSizeIterator for ProductionSourceIsaMatchesV1<'_> {}

/// Joint source-to-sparse-ISA correlation admitted from one exact finalized Worker transaction.
#[derive(Debug)]
pub struct AdmittedProductionSourceIsaCorrelationV1 {
    identity: [u8; 32],
    artifact_identity: ContentIdentityV1,
    structural_binding: ProductionTargetStructuralBindingV1,
    records: Vec<AdmittedProductionSourceIsaRecordV1>,
    source_node_index: Vec<([u8; 32], usize)>,
    source_span_index: Vec<(DebugSourceMapSpanV1, usize)>,
    isa_index: Vec<(ProductionIsaPointV1, usize)>,
    metadata_kernel_ordinals: Vec<u64>,
}

impl AdmittedProductionSourceIsaCorrelationV1 {
    pub const fn identity(&self) -> &[u8; 32] {
        &self.identity
    }

    pub const fn artifact_identity(&self) -> ContentIdentityV1 {
        self.artifact_identity
    }

    pub const fn structural_binding(&self) -> ProductionTargetStructuralBindingV1 {
        self.structural_binding
    }

    pub fn records(&self) -> &[AdmittedProductionSourceIsaRecordV1] {
        &self.records
    }

    pub fn query_source_node(
        &self,
        source_node_identity: [u8; 32],
    ) -> Result<ProductionSourceIsaMatchesV1<'_>, ProductionSourceIsaQueryUnavailableV1> {
        let range = equal_range_by_key(&self.source_node_index, &source_node_identity, |entry| {
            &entry.0
        });
        if range.start == range.end {
            return Err(ProductionSourceIsaQueryUnavailableV1::UnknownSourceNode);
        }
        Ok(ProductionSourceIsaMatchesV1 {
            records: &self.records,
            index: MatchIndexSliceV1::SourceNode(&self.source_node_index[range]),
            next: 0,
        })
    }

    pub fn query_source_span(
        &self,
        source_span: DebugSourceMapSpanV1,
    ) -> Result<ProductionSourceIsaMatchesV1<'_>, ProductionSourceIsaQueryUnavailableV1> {
        let range = equal_range_by_key(&self.source_span_index, &source_span, |entry| &entry.0);
        if range.start == range.end {
            return Err(ProductionSourceIsaQueryUnavailableV1::UnknownSourceSpan);
        }
        Ok(ProductionSourceIsaMatchesV1 {
            records: &self.records,
            index: MatchIndexSliceV1::SourceSpan(&self.source_span_index[range]),
            next: 0,
        })
    }

    pub fn query_isa_pc(
        &self,
        point: ProductionIsaPointV1,
    ) -> Result<ProductionSourceIsaMatchesV1<'_>, ProductionSourceIsaQueryUnavailableV1> {
        if !point.symbol_relative_pc.is_multiple_of(4) {
            return Err(ProductionSourceIsaQueryUnavailableV1::UnalignedProgramCounter);
        }
        if self
            .metadata_kernel_ordinals
            .binary_search(&point.kernel_ordinal)
            .is_err()
        {
            return Err(ProductionSourceIsaQueryUnavailableV1::UnknownMetadataKernelOrdinal);
        }
        let range = equal_range_by_key(&self.isa_index, &point, |entry| &entry.0);
        if range.start == range.end {
            return Err(ProductionSourceIsaQueryUnavailableV1::ProgramCounterIsNotAnAdmittedAnchor);
        }
        Ok(ProductionSourceIsaMatchesV1 {
            records: &self.records,
            index: MatchIndexSliceV1::Isa(&self.isa_index[range]),
            next: 0,
        })
    }

    pub const fn proves_complete_machine_instruction_coverage(&self) -> bool {
        false
    }

    pub const fn proves_a_schedule(&self) -> bool {
        false
    }

    pub const fn proves_semantic_refinement(&self) -> bool {
        false
    }

    pub const fn proves_optimized_or_final_llvm_custody(&self) -> bool {
        false
    }

    pub const fn proves_live_program_counter_ownership(&self) -> bool {
        false
    }

    pub const fn grants_runtime_authority(&self) -> bool {
        false
    }
}

impl PreparedFinalizedProtectedWorkerV3HsacoV1 {
    /// Admits and joins exact Source/MIR/KIR V7 projection, deterministic neutral-to-target KIR
    /// V8 binding, Worker-input LLVM anchors, and final-HSACO sparse pseudo-probe intervals.
    #[allow(clippy::too_many_lines)]
    pub fn admit_production_source_isa_correlation_v1(
        &self,
    ) -> Result<ProductionSourceIsaCorrelationAdmissionV1, ProductionSourceIsaCorrelationErrorV1>
    {
        let receipts = self.outer_handoff().capsule().receipts();
        let semantic_map = match self.admit_production_semantic_debug_map_v1() {
            Ok(ProductionFinalizedSemanticDebugAdmissionV1::Admitted(map)) => map,
            Ok(ProductionFinalizedSemanticDebugAdmissionV1::Unavailable(reason)) => {
                return Ok(ProductionSourceIsaCorrelationAdmissionV1::Unavailable(
                    ProductionSourceIsaCorrelationUnavailableV1::SemanticDebugCarrier(reason),
                ));
            }
            Err(map_error) => {
                let replay = CanonicalProductionKirToLlvmReplayEvidenceV1::decode(
                    receipts.amdgpu_lowering().canonical_preimage(),
                )
                .and_then(|evidence| {
                    evidence.validate_against_neutral_kernel_ir(
                        receipts.kernel_ir().canonical_preimage(),
                    )
                });
                if replay.is_ok_and(|replay| {
                    replay.structural_binding().version() == ProductionReplayKernelIrVersionV1::V9
                }) {
                    match self
                        .admit_production_semantic_anchors_v1()
                        .map_err(ProductionSourceIsaCorrelationErrorV1::SemanticAnchors)?
                    {
                        ProductionSemanticAnchorAdmissionV1::Admitted(_) => {
                            return Ok(ProductionSourceIsaCorrelationAdmissionV1::Unavailable(
                                ProductionSourceIsaCorrelationUnavailableV1::SourceProjectionForKirV9,
                            ));
                        }
                        ProductionSemanticAnchorAdmissionV1::Unavailable(reason) => {
                            return Ok(ProductionSourceIsaCorrelationAdmissionV1::Unavailable(
                                ProductionSourceIsaCorrelationUnavailableV1::SemanticAnchors(
                                    reason,
                                ),
                            ));
                        }
                    }
                }
                return Err(ProductionSourceIsaCorrelationErrorV1::SemanticDebugMap(
                    map_error,
                ));
            }
        };
        let replay = CanonicalProductionKirToLlvmReplayEvidenceV1::decode(
            receipts.amdgpu_lowering().canonical_preimage(),
        )
        .and_then(|evidence| {
            evidence.validate_against_neutral_kernel_ir(receipts.kernel_ir().canonical_preimage())
        })
        .map_err(|_| ProductionSourceIsaCorrelationErrorV1::InvalidKirToLlvmReplay)?;
        let anchors = match self
            .admit_production_semantic_anchors_v1()
            .map_err(ProductionSourceIsaCorrelationErrorV1::SemanticAnchors)?
        {
            ProductionSemanticAnchorAdmissionV1::Admitted(anchors) => anchors,
            ProductionSemanticAnchorAdmissionV1::Unavailable(reason) => {
                return Ok(ProductionSourceIsaCorrelationAdmissionV1::Unavailable(
                    ProductionSourceIsaCorrelationUnavailableV1::SemanticAnchors(reason),
                ));
            }
        };
        if replay.structural_binding().version() == ProductionReplayKernelIrVersionV1::V9 {
            return Ok(ProductionSourceIsaCorrelationAdmissionV1::Unavailable(
                ProductionSourceIsaCorrelationUnavailableV1::SourceProjectionForKirV9,
            ));
        }
        if semantic_map.admission_status()
            != FinalizedSemanticDebugMapAdmissionStatusV1::ExactInputsAndArtifact
        {
            return Err(ProductionSourceIsaCorrelationErrorV1::NonExactSemanticMap);
        }
        if semantic_map.artifact_identity() != anchors.artifact_identity() {
            return Err(ProductionSourceIsaCorrelationErrorV1::ArtifactIdentityMismatch);
        }

        let structural_binding = replay.structural_binding();
        let target_identity = structural_binding.target_bound_kernel_ir();
        if anchors.target_bound_kir_version() != target_identity.version()
            || anchors.target_bound_kir_sha256() != &target_identity.sha256()
            || anchors.target_bound_kir_bytes() != target_identity.byte_len()
        {
            return Err(ProductionSourceIsaCorrelationErrorV1::TargetKirIdentityMismatch);
        }
        if structural_binding.counts().operations()
            != u64::try_from(anchors.anchors().len())
                .map_err(|_| ProductionSourceIsaCorrelationErrorV1::ResourceLimit)?
        {
            return Err(ProductionSourceIsaCorrelationErrorV1::CoordinateShapeMismatch);
        }

        let document = semantic_map.document();
        let anchor_records = anchors.anchors();
        let mut anchors_by_kir = Vec::new();
        anchors_by_kir
            .try_reserve_exact(anchor_records.len())
            .map_err(|_| ProductionSourceIsaCorrelationErrorV1::AllocationFailure)?;
        anchors_by_kir.extend(
            anchor_records
                .iter()
                .enumerate()
                .map(|(index, anchor)| (anchor.kir(), index)),
        );
        anchors_by_kir.sort_unstable_by_key(|entry| entry.0);
        if anchors_by_kir.windows(2).any(|pair| pair[0].0 == pair[1].0) {
            return Err(ProductionSourceIsaCorrelationErrorV1::CoordinateShapeMismatch);
        }
        let mut has_source = Vec::new();
        has_source
            .try_reserve_exact(anchor_records.len())
            .map_err(|_| ProductionSourceIsaCorrelationErrorV1::AllocationFailure)?;
        has_source.resize(anchor_records.len(), false);

        let requested_records = document
            .nodes()
            .len()
            .checked_add(anchor_records.len())
            .ok_or(ProductionSourceIsaCorrelationErrorV1::ResourceLimit)?;
        if requested_records > MAX_CORRELATION_RECORDS_V1 {
            return Err(ProductionSourceIsaCorrelationErrorV1::ResourceLimit);
        }
        let mut records = Vec::new();
        records
            .try_reserve_exact(requested_records)
            .map_err(|_| ProductionSourceIsaCorrelationErrorV1::AllocationFailure)?;

        for mapping in document.mappings().iter().filter(|mapping| {
            mapping.input_layer() == SemanticDebugLayerV1::Mir
                && mapping.output_layer() == SemanticDebugLayerV1::Kir
        }) {
            let [mir_node_identity] = mapping.inputs() else {
                return Err(ProductionSourceIsaCorrelationErrorV1::InvalidSourceGraph);
            };
            let mir_node = document
                .node(*mir_node_identity)
                .ok_or(ProductionSourceIsaCorrelationErrorV1::InvalidSourceGraph)?;
            let source_mapping = document
                .mapping_to(*mir_node_identity)
                .filter(|source_mapping| {
                    source_mapping.input_layer() == SemanticDebugLayerV1::Source
                        && source_mapping.output_layer() == SemanticDebugLayerV1::Mir
                        && source_mapping.inputs().len() == 1
                })
                .ok_or(ProductionSourceIsaCorrelationErrorV1::InvalidSourceGraph)?;
            let source_node_identity = source_mapping.inputs()[0];
            let source_node = document
                .node(source_node_identity)
                .ok_or(ProductionSourceIsaCorrelationErrorV1::InvalidSourceGraph)?;
            let SemanticDebugLocationV1::Source { span } = source_node.location() else {
                return Err(ProductionSourceIsaCorrelationErrorV1::InvalidSourceGraph);
            };
            if mapping.output().nodes().is_empty() {
                records.push(AdmittedProductionSourceIsaRecordV1 {
                    kind: ProductionSourceIsaRecordKindV1::EliminatedBeforeKir,
                    source_node_identity: Some(source_node_identity),
                    source_span: Some(span),
                    mir_node_identity: Some(*mir_node_identity),
                    mir: Some(mir_node.location()),
                    neutral_kir_node_identity: None,
                    neutral_kir: None,
                    target_kir: None,
                    semantic_operation_id: None,
                    compiler_handoff_llvm: None,
                    isa: Vec::new(),
                    anchor_transformation: None,
                });
                continue;
            }
            for kir_node_identity in mapping.output().nodes() {
                let kir_node = document
                    .node(*kir_node_identity)
                    .ok_or(ProductionSourceIsaCorrelationErrorV1::InvalidSourceGraph)?;
                let kir = kir_node.location();
                if kir.layer() != SemanticDebugLayerV1::Kir {
                    return Err(ProductionSourceIsaCorrelationErrorV1::InvalidSourceGraph);
                }
                let anchor_index = anchors_by_kir
                    .binary_search_by_key(&kir, |entry| entry.0)
                    .ok()
                    .map(|index| anchors_by_kir[index].1)
                    .ok_or(ProductionSourceIsaCorrelationErrorV1::CoordinateShapeMismatch)?;
                *has_source
                    .get_mut(anchor_index)
                    .ok_or(ProductionSourceIsaCorrelationErrorV1::CoordinateShapeMismatch)? = true;
                records.push(source_anchor_record(
                    source_node_identity,
                    span,
                    *mir_node_identity,
                    mir_node.location(),
                    *kir_node_identity,
                    kir,
                    anchor_records
                        .get(anchor_index)
                        .ok_or(ProductionSourceIsaCorrelationErrorV1::CoordinateShapeMismatch)?,
                )?);
            }
        }
        for (anchor_index, anchor) in anchor_records.iter().enumerate() {
            if !has_source
                .get(anchor_index)
                .copied()
                .ok_or(ProductionSourceIsaCorrelationErrorV1::CoordinateShapeMismatch)?
            {
                records.push(no_source_anchor_record(anchor)?);
            }
        }
        if records.len() > MAX_CORRELATION_RECORDS_V1 {
            return Err(ProductionSourceIsaCorrelationErrorV1::ResourceLimit);
        }

        let (source_node_index, source_span_index, isa_index, mut metadata_kernel_ordinals) =
            build_indices(&records)?;
        if metadata_kernel_ordinals.is_empty() {
            metadata_kernel_ordinals
                .try_reserve_exact(1)
                .map_err(|_| ProductionSourceIsaCorrelationErrorV1::AllocationFailure)?;
            metadata_kernel_ordinals.push(0);
        } else if metadata_kernel_ordinals != [0] {
            return Err(ProductionSourceIsaCorrelationErrorV1::CoordinateShapeMismatch);
        }
        let identity = correlation_identity(
            semantic_map.identity().as_bytes(),
            semantic_map.artifact_identity(),
            structural_binding,
            &records,
        );
        Ok(ProductionSourceIsaCorrelationAdmissionV1::Admitted(
            Box::new(AdmittedProductionSourceIsaCorrelationV1 {
                identity,
                artifact_identity: semantic_map.artifact_identity(),
                structural_binding,
                records,
                source_node_index,
                source_span_index,
                isa_index,
                metadata_kernel_ordinals,
            }),
        ))
    }
}

fn source_anchor_record(
    source_node_identity: [u8; 32],
    source_span: DebugSourceMapSpanV1,
    mir_node_identity: [u8; 32],
    mir: SemanticDebugLocationV1,
    neutral_kir_node_identity: [u8; 32],
    neutral_kir: SemanticDebugLocationV1,
    anchor: &AdmittedProductionSemanticAnchorV1,
) -> Result<AdmittedProductionSourceIsaRecordV1, ProductionSourceIsaCorrelationErrorV1> {
    Ok(AdmittedProductionSourceIsaRecordV1 {
        kind: ProductionSourceIsaRecordKindV1::SourceAnchored,
        source_node_identity: Some(source_node_identity),
        source_span: Some(source_span),
        mir_node_identity: Some(mir_node_identity),
        mir: Some(mir),
        neutral_kir_node_identity: Some(neutral_kir_node_identity),
        neutral_kir: Some(neutral_kir),
        target_kir: Some(anchor.kir()),
        semantic_operation_id: Some(*anchor.semantic_operation_id()),
        compiler_handoff_llvm: Some(anchor.compiler_handoff_llvm()),
        isa: bounded_copy_locations(anchor.isa())?,
        anchor_transformation: Some(anchor.transformation()),
    })
}

fn no_source_anchor_record(
    anchor: &AdmittedProductionSemanticAnchorV1,
) -> Result<AdmittedProductionSourceIsaRecordV1, ProductionSourceIsaCorrelationErrorV1> {
    Ok(AdmittedProductionSourceIsaRecordV1 {
        kind: ProductionSourceIsaRecordKindV1::NoSourceProvenance,
        source_node_identity: None,
        source_span: None,
        mir_node_identity: None,
        mir: None,
        neutral_kir_node_identity: None,
        neutral_kir: None,
        target_kir: Some(anchor.kir()),
        semantic_operation_id: Some(*anchor.semantic_operation_id()),
        compiler_handoff_llvm: Some(anchor.compiler_handoff_llvm()),
        isa: bounded_copy_locations(anchor.isa())?,
        anchor_transformation: Some(anchor.transformation()),
    })
}

fn bounded_copy_locations(
    locations: &[SemanticDebugLocationV1],
) -> Result<Vec<SemanticDebugLocationV1>, ProductionSourceIsaCorrelationErrorV1> {
    let mut copy = Vec::new();
    copy.try_reserve_exact(locations.len())
        .map_err(|_| ProductionSourceIsaCorrelationErrorV1::AllocationFailure)?;
    copy.extend_from_slice(locations);
    Ok(copy)
}

type CorrelationIndicesV1 = (
    Vec<([u8; 32], usize)>,
    Vec<(DebugSourceMapSpanV1, usize)>,
    Vec<(ProductionIsaPointV1, usize)>,
    Vec<u64>,
);

fn build_indices(
    records: &[AdmittedProductionSourceIsaRecordV1],
) -> Result<CorrelationIndicesV1, ProductionSourceIsaCorrelationErrorV1> {
    let isa_count = records.iter().try_fold(0_usize, |count, record| {
        count
            .checked_add(record.isa.len())
            .ok_or(ProductionSourceIsaCorrelationErrorV1::ResourceLimit)
    })?;
    if isa_count > MAX_SEMANTIC_DEBUG_MAPPING_REFERENCES_V1 {
        return Err(ProductionSourceIsaCorrelationErrorV1::ResourceLimit);
    }
    let mut source_node_index = Vec::new();
    let mut source_span_index = Vec::new();
    let mut isa_index = Vec::new();
    let mut metadata_kernel_ordinals = Vec::new();
    for (index, record) in records.iter().enumerate() {
        if record.source_node_identity.is_some() {
            source_node_index
                .try_reserve(1)
                .map_err(|_| ProductionSourceIsaCorrelationErrorV1::AllocationFailure)?;
            source_node_index.push((record.source_node_identity.unwrap_or([0; 32]), index));
        }
        if let Some(span) = record.source_span {
            source_span_index
                .try_reserve(1)
                .map_err(|_| ProductionSourceIsaCorrelationErrorV1::AllocationFailure)?;
            source_span_index.push((span, index));
        }
        for location in &record.isa {
            let SemanticDebugLocationV1::Isa {
                kernel_ordinal,
                byte_start,
                byte_end,
            } = *location
            else {
                return Err(ProductionSourceIsaCorrelationErrorV1::CoordinateShapeMismatch);
            };
            if byte_end
                != byte_start
                    .checked_add(4)
                    .ok_or(ProductionSourceIsaCorrelationErrorV1::CoordinateShapeMismatch)?
            {
                return Err(ProductionSourceIsaCorrelationErrorV1::CoordinateShapeMismatch);
            }
            isa_index
                .try_reserve(1)
                .map_err(|_| ProductionSourceIsaCorrelationErrorV1::AllocationFailure)?;
            isa_index.push((ProductionIsaPointV1::new(kernel_ordinal, byte_start), index));
            metadata_kernel_ordinals
                .try_reserve(1)
                .map_err(|_| ProductionSourceIsaCorrelationErrorV1::AllocationFailure)?;
            metadata_kernel_ordinals.push(kernel_ordinal);
        }
    }
    source_node_index.sort_unstable();
    source_span_index.sort_unstable();
    isa_index.sort_unstable();
    metadata_kernel_ordinals.sort_unstable();
    metadata_kernel_ordinals.dedup();
    Ok((
        source_node_index,
        source_span_index,
        isa_index,
        metadata_kernel_ordinals,
    ))
}

fn equal_range_by_key<T, K: Ord, F: Fn(&T) -> &K>(
    values: &[T],
    key: &K,
    field: F,
) -> std::ops::Range<usize> {
    let start = values.partition_point(|value| field(value) < key);
    let end = values.partition_point(|value| field(value) <= key);
    start..end
}

fn correlation_identity(
    semantic_map_identity: &[u8; 32],
    artifact_identity: ContentIdentityV1,
    structural_binding: ProductionTargetStructuralBindingV1,
    records: &[AdmittedProductionSourceIsaRecordV1],
) -> [u8; 32] {
    let mut digest = Sha256::new();
    digest.update((CORRELATION_IDENTITY_DOMAIN_V1.len() as u32).to_le_bytes());
    digest.update(CORRELATION_IDENTITY_DOMAIN_V1);
    digest.update(semantic_map_identity);
    digest.update(artifact_identity.sha256());
    digest.update(artifact_identity.byte_len().to_le_bytes());
    digest.update(structural_binding.identity());
    digest.update((records.len() as u64).to_le_bytes());
    for record in records {
        digest.update([match record.kind {
            ProductionSourceIsaRecordKindV1::EliminatedBeforeKir => 1,
            ProductionSourceIsaRecordKindV1::SourceAnchored => 2,
            ProductionSourceIsaRecordKindV1::NoSourceProvenance => 3,
        }]);
        digest.update(record.source_node_identity.unwrap_or([0; 32]));
        digest.update(record.mir_node_identity.unwrap_or([0; 32]));
        digest.update(record.neutral_kir_node_identity.unwrap_or([0; 32]));
        digest.update(record.semantic_operation_id.unwrap_or([0; 32]));
        digest.update((record.isa.len() as u64).to_le_bytes());
        for isa in &record.isa {
            if let SemanticDebugLocationV1::Isa {
                kernel_ordinal,
                byte_start,
                byte_end,
            } = isa
            {
                digest.update(kernel_ordinal.to_le_bytes());
                digest.update(byte_start.to_le_bytes());
                digest.update(byte_end.to_le_bytes());
            }
        }
    }
    digest.finalize().into()
}

#[cfg(test)]
mod tests {
    use dialect_amdgcn::{
        CanonicalProductionKirToLlvmReplayEvidenceV1, ProductionSemanticAnchorKirIdentityV1,
        bind_production_llvm22_worker_layout_v1, bind_production_target_v1,
        lower_kernel_to_gfx942_xnack_minus_llvm_ir_with_semantic_anchors_v1,
    };
    use fe2o3_amd_target::ProductionAmdTargetProfileV1;
    use fe2o3_kernel_ir::{
        BasicBlock, BlockId, Constant, Function, LaunchDomain, LaunchExtent, Module, Operation,
        OperationKind, ScalarType, Signature, Terminator, Type, ValueDef, ValueId,
        VerifiedCanonicalKernelIrV8, WorkgroupSize,
    };

    use super::*;

    fn span(seed: u8, start: u64) -> DebugSourceMapSpanV1 {
        DebugSourceMapSpanV1::new([seed; 32], start, start + 4, 1, 1).unwrap()
    }

    fn source_record(
        source: u8,
        source_span: DebugSourceMapSpanV1,
        operation: u64,
        isa: Vec<SemanticDebugLocationV1>,
    ) -> AdmittedProductionSourceIsaRecordV1 {
        AdmittedProductionSourceIsaRecordV1 {
            kind: ProductionSourceIsaRecordKindV1::SourceAnchored,
            source_node_identity: Some([source; 32]),
            source_span: Some(source_span),
            mir_node_identity: Some([source.wrapping_add(10); 32]),
            mir: Some(SemanticDebugLocationV1::Mir {
                body_ordinal: 0,
                block_ordinal: 0,
                statement_ordinal: operation,
            }),
            neutral_kir_node_identity: Some([source.wrapping_add(20); 32]),
            neutral_kir: Some(SemanticDebugLocationV1::Kir {
                function_ordinal: 0,
                block_ordinal: 0,
                operation_ordinal: operation,
            }),
            target_kir: Some(SemanticDebugLocationV1::Kir {
                function_ordinal: 0,
                block_ordinal: 0,
                operation_ordinal: operation,
            }),
            semantic_operation_id: Some([source.wrapping_add(30); 32]),
            compiler_handoff_llvm: Some(SemanticDebugLocationV1::Llvm {
                function_ordinal: 0,
                block_ordinal: 0,
                instruction_ordinal: operation,
            }),
            isa,
            anchor_transformation: Some(ProductionSemanticAnchorTransformationV1::Preserved),
        }
    }

    fn eliminated_record(
        source: u8,
        source_span: DebugSourceMapSpanV1,
    ) -> AdmittedProductionSourceIsaRecordV1 {
        AdmittedProductionSourceIsaRecordV1 {
            kind: ProductionSourceIsaRecordKindV1::EliminatedBeforeKir,
            source_node_identity: Some([source; 32]),
            source_span: Some(source_span),
            mir_node_identity: Some([source.wrapping_add(10); 32]),
            mir: Some(SemanticDebugLocationV1::Mir {
                body_ordinal: 0,
                block_ordinal: 0,
                statement_ordinal: 3,
            }),
            neutral_kir_node_identity: None,
            neutral_kir: None,
            target_kir: None,
            semantic_operation_id: None,
            compiler_handoff_llvm: None,
            isa: Vec::new(),
            anchor_transformation: None,
        }
    }

    fn synthetic_record() -> AdmittedProductionSourceIsaRecordV1 {
        AdmittedProductionSourceIsaRecordV1 {
            kind: ProductionSourceIsaRecordKindV1::NoSourceProvenance,
            source_node_identity: None,
            source_span: None,
            mir_node_identity: None,
            mir: None,
            neutral_kir_node_identity: None,
            neutral_kir: None,
            target_kir: Some(SemanticDebugLocationV1::Kir {
                function_ordinal: 0,
                block_ordinal: 0,
                operation_ordinal: 4,
            }),
            semantic_operation_id: Some([44; 32]),
            compiler_handoff_llvm: Some(SemanticDebugLocationV1::Llvm {
                function_ordinal: 0,
                block_ordinal: 0,
                instruction_ordinal: 4,
            }),
            isa: vec![SemanticDebugLocationV1::Isa {
                kernel_ordinal: 0,
                byte_start: 12,
                byte_end: 16,
            }],
            anchor_transformation: Some(ProductionSemanticAnchorTransformationV1::Preserved),
        }
    }

    fn structural_binding() -> ProductionTargetStructuralBindingV1 {
        let mut block = BasicBlock::new(BlockId(0));
        block.operations.push(Operation::effect_free(
            ValueDef::new(ValueId(0), Type::Scalar(ScalarType::U32)),
            OperationKind::Constant(Constant::U32(7)),
        ));
        block.terminator = Some(Terminator::Return { values: vec![] });
        let function =
            Function::kernel_entry("entry", Signature::new(vec![], vec![]), vec![], vec![block]);
        let mut kernel = fe2o3_kernel_ir::Kernel::new(
            "kernel",
            "entry",
            LaunchDomain::D1 {
                x: LaunchExtent::Static(1),
            },
        );
        kernel.workgroup_size = Some(WorkgroupSize::new(64, 1, 1));
        let mut module = Module::new("source-isa-index-test");
        module.functions.push(function);
        module.kernels.push(kernel);
        let neutral = VerifiedCanonicalKernelIrV8::from_module(module).unwrap();
        let neutral_bytes = neutral.into_canonical_bytes();
        let (_, neutral_module) =
            VerifiedCanonicalKernelIrV8::from_canonical_bytes_with_module(neutral_bytes.clone())
                .unwrap();
        let target =
            bind_production_target_v1(&neutral_module, ProductionAmdTargetProfileV1::Gfx942)
                .unwrap();
        let target_owner =
            VerifiedCanonicalKernelIrV8::from_module(target.module().clone()).unwrap();
        let dialect = lower_kernel_to_gfx942_xnack_minus_llvm_ir_with_semantic_anchors_v1(
            target.module(),
            target.kernel_id(),
            ProductionSemanticAnchorKirIdentityV1::from_v8(&target_owner),
        )
        .unwrap();
        let llvm = bind_production_llvm22_worker_layout_v1(&dialect).unwrap();
        CanonicalProductionKirToLlvmReplayEvidenceV1::from_live_inputs(
            &neutral_bytes,
            target.module(),
            ProductionAmdTargetProfileV1::Gfx942,
            &llvm,
        )
        .unwrap()
        .validate_against_neutral_kernel_ir(&neutral_bytes)
        .unwrap()
        .structural_binding()
    }

    fn indexed_fixture() -> AdmittedProductionSourceIsaCorrelationV1 {
        let shared_span = span(1, 0);
        let records = vec![
            source_record(
                1,
                shared_span,
                0,
                vec![
                    SemanticDebugLocationV1::Isa {
                        kernel_ordinal: 0,
                        byte_start: 0,
                        byte_end: 4,
                    },
                    SemanticDebugLocationV1::Isa {
                        kernel_ordinal: 0,
                        byte_start: 8,
                        byte_end: 12,
                    },
                ],
            ),
            source_record(1, shared_span, 1, Vec::new()),
            source_record(
                2,
                shared_span,
                2,
                vec![SemanticDebugLocationV1::Isa {
                    kernel_ordinal: 0,
                    byte_start: 0,
                    byte_end: 4,
                }],
            ),
            eliminated_record(3, span(2, 16)),
            synthetic_record(),
        ];
        let (source_node_index, source_span_index, isa_index, metadata_kernel_ordinals) =
            build_indices(&records).unwrap();
        AdmittedProductionSourceIsaCorrelationV1 {
            identity: [9; 32],
            artifact_identity: ContentIdentityV1::calculate(b"artifact"),
            structural_binding: structural_binding(),
            records,
            source_node_index,
            source_span_index,
            isa_index,
            metadata_kernel_ordinals,
        }
    }

    #[test]
    fn flat_indices_preserve_many_to_many_eliminated_and_synthetic_records() {
        let admitted = indexed_fixture();
        let source = admitted
            .query_source_node([1; 32])
            .unwrap()
            .collect::<Vec<_>>();
        assert_eq!(source.len(), 2);
        assert!(source.iter().any(|record| record.isa().len() == 2));
        assert!(source.iter().any(|record| {
            record.kind() == ProductionSourceIsaRecordKindV1::SourceAnchored
                && record.isa().is_empty()
        }));

        let same_span = admitted
            .query_source_span(span(1, 0))
            .unwrap()
            .collect::<Vec<_>>();
        assert_eq!(same_span.len(), 3);
        assert_eq!(
            same_span
                .iter()
                .filter_map(|record| record.source_node_identity())
                .collect::<std::collections::BTreeSet<_>>()
                .len(),
            2
        );

        let coalesced = admitted
            .query_isa_pc(ProductionIsaPointV1::new(0, 0))
            .unwrap()
            .collect::<Vec<_>>();
        assert_eq!(coalesced.len(), 2);
        assert_eq!(
            coalesced
                .iter()
                .filter_map(|record| record.source_node_identity())
                .collect::<std::collections::BTreeSet<_>>()
                .len(),
            2
        );
        let synthetic = admitted
            .query_isa_pc(ProductionIsaPointV1::new(0, 12))
            .unwrap()
            .collect::<Vec<_>>();
        assert_eq!(synthetic.len(), 1);
        assert_eq!(
            synthetic[0].kind(),
            ProductionSourceIsaRecordKindV1::NoSourceProvenance
        );
        assert!(synthetic[0].source_node_identity().is_none());

        let eliminated = admitted.query_source_node([3; 32]).unwrap().next().unwrap();
        assert_eq!(
            eliminated.kind(),
            ProductionSourceIsaRecordKindV1::EliminatedBeforeKir
        );
        assert!(eliminated.target_kir().is_none());
    }

    #[test]
    fn exact_pc_and_source_queries_fail_closed() {
        let admitted = indexed_fixture();
        assert_eq!(
            admitted.query_source_node([99; 32]).unwrap_err(),
            ProductionSourceIsaQueryUnavailableV1::UnknownSourceNode
        );
        assert_eq!(
            admitted.query_source_span(span(99, 0)).unwrap_err(),
            ProductionSourceIsaQueryUnavailableV1::UnknownSourceSpan
        );
        assert_eq!(
            admitted
                .query_isa_pc(ProductionIsaPointV1::new(0, 1))
                .unwrap_err(),
            ProductionSourceIsaQueryUnavailableV1::UnalignedProgramCounter
        );
        assert_eq!(
            admitted
                .query_isa_pc(ProductionIsaPointV1::new(9, 0))
                .unwrap_err(),
            ProductionSourceIsaQueryUnavailableV1::UnknownMetadataKernelOrdinal
        );
        assert_eq!(
            admitted
                .query_isa_pc(ProductionIsaPointV1::new(0, 4))
                .unwrap_err(),
            ProductionSourceIsaQueryUnavailableV1::ProgramCounterIsNotAnAdmittedAnchor
        );
    }
}

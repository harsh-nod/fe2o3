//! Finalizer admission for exact-artifact-bound semantic debug maps.

use std::{error::Error, fmt};

use dialect_amdgcn::CanonicalProductionKirToLlvmReplayEvidenceV1;
use fe2o3_compiler_lineage::{
    InertSemanticToLlvmAssociationInputsV3, InertSemanticToLlvmAssociationV3,
    InertSemanticToLlvmContentIdentityV3,
};
use fe2o3_kernel_ir::{
    DebugSourceMapDocumentV2, DebugSourceMapKirSiteV1, DebugSourceMapSpanV1,
    ProductionSemanticDebugAvailabilityV1, ProductionSemanticDebugCarrierV1,
    ProductionSemanticDebugFragmentErrorV1, ProductionSemanticDebugProducerGapV1,
    ProductionSemanticDebugReceiptExtensionV1, SemanticDebugContentIdentityV1,
    SemanticDebugLayerV1, SemanticDebugLocationV1, SemanticDebugMapDocumentV1,
    SemanticDebugMapErrorV1, SemanticDebugMapInputsV1, SemanticDebugTransformationV1,
    SemanticDebugUnavailableReasonV1, VerifiedCanonicalKernelIrV8, decode_module_v7,
    semantic_debug_map_identity_v1,
};
use fe2o3_lower_mir_kernel::{
    InertCanonicalMirToKirCorrespondenceEvidenceV4, ProductionCanonicalKernelIrVersionV1,
};
use fe2o3_mir_model::semantic_mir_v1::{AdmittedInertSemanticMirV1, SemanticMirLimitsV1};

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

/// Production finalizer result. A missing optional attachment remains explicit.
#[derive(Debug)]
pub enum ProductionFinalizedSemanticDebugAdmissionV1 {
    Admitted(Box<AdmittedFinalizedSemanticDebugMapV1>),
    Unavailable(ProductionSemanticDebugProducerGapV1),
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
    /// Joins the optional compiler debug attachment to the exact retained V3 lineage and
    /// independently finalized artifact. Exact admission replays whole-module KIR-to-LLVM
    /// custody and revalidates every produced Source-to-MIR-to-KIR edge against V4 evidence.
    pub fn admit_production_semantic_debug_map_v1(
        &self,
    ) -> Result<ProductionFinalizedSemanticDebugAdmissionV1, FinalizedSemanticDebugMapErrorV1> {
        let outer = self.outer_handoff();
        let receipts = outer.capsule().receipts();
        let receipt_bytes = receipts.semantic_to_llvm().canonical_preimage();
        let extension =
            match ProductionSemanticDebugReceiptExtensionV1::from_canonical_bytes(receipt_bytes) {
                Ok(extension) => extension,
                Err(error) => {
                    if InertSemanticToLlvmAssociationV3::decode(receipt_bytes).is_ok() {
                        return Ok(ProductionFinalizedSemanticDebugAdmissionV1::Unavailable(
                            ProductionSemanticDebugProducerGapV1::LegacyBareAssociationNoAttachment,
                        ));
                    }
                    return Err(FinalizedSemanticDebugMapErrorV1::ProductionFragment(error));
                }
            };
        let association = InertSemanticToLlvmAssociationV3::decode(extension.association_v3())
            .map_err(|_| FinalizedSemanticDebugMapErrorV1::ProductionAssociation)?;
        validate_production_association(outer, association.inputs())?;

        let carrier: &ProductionSemanticDebugCarrierV1 = extension.carrier_v1();
        let ProductionSemanticDebugAvailabilityV1::Available(fragment) = carrier.availability()
        else {
            let ProductionSemanticDebugAvailabilityV1::Unavailable(gap) = carrier.availability()
            else {
                unreachable!()
            };
            return Ok(ProductionFinalizedSemanticDebugAdmissionV1::Unavailable(
                *gap,
            ));
        };

        let validated_replay = CanonicalProductionKirToLlvmReplayEvidenceV1::decode(
            receipts.amdgpu_lowering().canonical_preimage(),
        )
        .and_then(|evidence| {
            evidence.validate_against_neutral_kernel_ir(receipts.kernel_ir().canonical_preimage())
        })
        .map_err(|_| FinalizedSemanticDebugMapErrorV1::InvalidKirToLlvmReplay)?;
        let replay_target = validated_replay.evidence().profile().device_target();
        if outer.module_handoff().target().to_string() != replay_target
            || self.target().to_string() != replay_target
        {
            return Err(FinalizedSemanticDebugMapErrorV1::KirToLlvmReplayTargetMismatch);
        }
        let llvm_to_hsaco = self.llvm_to_hsaco_derivation_evidence();
        if llvm_to_hsaco.hsaco() != self.raw_output_identity() {
            return Err(FinalizedSemanticDebugMapErrorV1::InvalidLlvmToHsacoCustody);
        }

        let semantic_mir = receipts.semantic_mir().canonical_preimage();
        let exact_kir_v8 = receipts.kernel_ir().canonical_preimage();
        let correspondence_bytes = receipts.mir_to_kir_correspondence().canonical_preimage();
        let map =
            SemanticDebugMapDocumentV1::from_canonical_json_bytes(fragment.pre_finalization_map())
                .map_err(FinalizedSemanticDebugMapErrorV1::SemanticMap)?;
        validate_exact_production_correspondence(
            &map,
            fragment.source_map_v2(),
            semantic_mir,
            exact_kir_v8,
            fragment.canonical_kir_v7(),
            correspondence_bytes,
        )?;

        let artifact_identity =
            SemanticDebugContentIdentityV1::calculate(self.exact_finalized_bytes())
                .map_err(FinalizedSemanticDebugMapErrorV1::SemanticMap)?;
        let map = map
            .with_finalized_artifact_identity(artifact_identity)
            .map_err(FinalizedSemanticDebugMapErrorV1::SemanticMap)?;
        let map_bytes = map
            .to_canonical_json_bytes()
            .map_err(FinalizedSemanticDebugMapErrorV1::SemanticMap)?;
        let admitted = self.admit_semantic_debug_map_with_inputs_v1(
            &map_bytes,
            SemanticDebugMapInputsV1 {
                source_map_v2: fragment.source_map_v2(),
                semantic_mir,
                canonical_kir: fragment.canonical_kir_v7(),
                schedule: fragment.schedule_status(),
                llvm_module: outer.module_handoff().module_bytes(),
                finalized_artifact: self.exact_finalized_bytes(),
            },
        )?;
        Ok(ProductionFinalizedSemanticDebugAdmissionV1::Admitted(
            Box::new(admitted),
        ))
    }

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

pub(crate) fn validate_production_association(
    outer: &fe2o3_compiler_ffi::InertSemanticCompilerModuleHandoffV3,
    actual: InertSemanticToLlvmAssociationInputsV3,
) -> Result<(), FinalizedSemanticDebugMapErrorV1> {
    let receipts = outer.capsule().receipts();
    let module = outer.module_handoff().module_identity();
    let expected = InertSemanticToLlvmAssociationInputsV3::new(
        association_identity(
            receipts.semantic_mir().identity().sha256(),
            receipts.semantic_mir().identity().byte_len(),
        )?,
        association_identity(
            receipts.middle_end().identity().sha256(),
            receipts.middle_end().identity().byte_len(),
        )?,
        association_identity(
            receipts.kernel_ir().identity().sha256(),
            receipts.kernel_ir().identity().byte_len(),
        )?,
        association_identity(
            receipts.mir_to_kir_correspondence().identity().sha256(),
            receipts.mir_to_kir_correspondence().identity().byte_len(),
        )?,
        association_identity(
            receipts.formal_memory().identity().sha256(),
            receipts.formal_memory().identity().byte_len(),
        )?,
        association_identity(
            receipts.proof_binding().identity().sha256(),
            receipts.proof_binding().identity().byte_len(),
        )?,
        association_identity(
            receipts.target_binding().identity().sha256(),
            receipts.target_binding().identity().byte_len(),
        )?,
        association_identity(
            receipts.data_layout().identity().sha256(),
            receipts.data_layout().identity().byte_len(),
        )?,
        association_identity(
            receipts.abi().identity().sha256(),
            receipts.abi().identity().byte_len(),
        )?,
        association_identity(
            receipts.export_manifest().identity().sha256(),
            receipts.export_manifest().identity().byte_len(),
        )?,
        association_identity(
            receipts.amdgpu_lowering().identity().sha256(),
            receipts.amdgpu_lowering().identity().byte_len(),
        )?,
        association_identity(module.sha256(), module.byte_len())?,
        association_identity(
            receipts
                .final_compiler_module_commitment()
                .identity()
                .sha256(),
            receipts
                .final_compiler_module_commitment()
                .identity()
                .byte_len(),
        )?,
    );
    if actual != expected {
        return Err(FinalizedSemanticDebugMapErrorV1::ProductionAssociationMismatch);
    }
    Ok(())
}

fn association_identity(
    sha256: &[u8; 32],
    byte_len: u64,
) -> Result<InertSemanticToLlvmContentIdentityV3, FinalizedSemanticDebugMapErrorV1> {
    InertSemanticToLlvmContentIdentityV3::new(*sha256, byte_len)
        .map_err(|_| FinalizedSemanticDebugMapErrorV1::ProductionAssociation)
}

fn bounded_copy(bytes: &[u8]) -> Result<Vec<u8>, FinalizedSemanticDebugMapErrorV1> {
    let mut copy = Vec::new();
    copy.try_reserve_exact(bytes.len())
        .map_err(|_| FinalizedSemanticDebugMapErrorV1::AllocationFailure)?;
    copy.extend_from_slice(bytes);
    Ok(copy)
}

#[allow(clippy::too_many_lines)]
fn validate_exact_production_correspondence(
    map: &SemanticDebugMapDocumentV1,
    source_map_bytes: &[u8],
    semantic_mir_bytes: &[u8],
    canonical_kir_v8_bytes: &[u8],
    canonical_kir_v7_bytes: &[u8],
    correspondence_bytes: &[u8],
) -> Result<(), FinalizedSemanticDebugMapErrorV1> {
    let source_map = DebugSourceMapDocumentV2::from_canonical_json_bytes(source_map_bytes)
        .map_err(|_| FinalizedSemanticDebugMapErrorV1::InvalidBoundSourceMap)?;
    let semantic_mir = AdmittedInertSemanticMirV1::decode_current_production_canonical(
        semantic_mir_bytes,
        SemanticMirLimitsV1::default(),
    )
    .map_err(|_| FinalizedSemanticDebugMapErrorV1::InvalidBoundSemanticMir)?;
    let correspondence =
        InertCanonicalMirToKirCorrespondenceEvidenceV4::decode(correspondence_bytes)
            .map_err(|_| FinalizedSemanticDebugMapErrorV1::InvalidBoundCorrespondenceV4)?;
    correspondence
        .revalidate()
        .map_err(|_| FinalizedSemanticDebugMapErrorV1::InvalidBoundCorrespondenceV4)?;
    let (canonical_kir_v8, module_v8) =
        VerifiedCanonicalKernelIrV8::from_canonical_bytes_with_module(bounded_copy(
            canonical_kir_v8_bytes,
        )?)
        .map_err(|_| FinalizedSemanticDebugMapErrorV1::InvalidBoundCanonicalKirV8)?;
    let module_v7 = decode_module_v7(canonical_kir_v7_bytes)
        .map_err(|_| FinalizedSemanticDebugMapErrorV1::InvalidBoundCanonicalKirV7)?;
    if module_v8 != module_v7 {
        return Err(FinalizedSemanticDebugMapErrorV1::CanonicalKirProjectionMismatch);
    }
    let correspondence_kir = correspondence.canonical_kernel_ir_identity();
    if correspondence_kir.version() != ProductionCanonicalKernelIrVersionV1::V8
        || correspondence_kir.digest() != canonical_kir_v8.identity().digest()
        || correspondence_kir.canonical_length() != canonical_kir_v8.identity().canonical_length()
        || correspondence.semantic_sha256() != semantic_mir.semantic_sha256().as_bytes()
    {
        return Err(FinalizedSemanticDebugMapErrorV1::CorrespondenceIdentityMismatch);
    }

    let mut bodies = module_v7
        .functions
        .iter()
        .enumerate()
        .filter_map(|(ordinal, function)| function.body.as_ref().map(|body| (ordinal, body)));
    let Some((function_ordinal, body)) = bodies.next() else {
        return Err(FinalizedSemanticDebugMapErrorV1::InvalidSemanticCorrespondence);
    };
    if bodies.next().is_some() {
        return Err(FinalizedSemanticDebugMapErrorV1::InvalidSemanticCorrespondence);
    }
    let function_ordinal = u64::try_from(function_ordinal)
        .map_err(|_| FinalizedSemanticDebugMapErrorV1::InvalidSemanticCorrespondence)?;
    let mut block_ordinals = Vec::new();
    block_ordinals
        .try_reserve_exact(body.blocks.len())
        .map_err(|_| FinalizedSemanticDebugMapErrorV1::AllocationFailure)?;
    block_ordinals.extend(
        body.blocks
            .iter()
            .enumerate()
            .map(|(ordinal, block)| (block.id.0, ordinal)),
    );
    block_ordinals.sort_unstable_by_key(|entry| entry.0);
    if block_ordinals.windows(2).any(|pair| pair[0].0 == pair[1].0) {
        return Err(FinalizedSemanticDebugMapErrorV1::InvalidSemanticCorrespondence);
    }

    let counts = correspondence
        .statement_spans()
        .iter()
        .try_fold((0_usize, 0_usize), |(statements, operations), span| {
            Some((
                statements.checked_add(1)?,
                operations.checked_add(span.operation_count() as usize)?,
            ))
        })
        .ok_or(FinalizedSemanticDebugMapErrorV1::InvalidSemanticCorrespondence)?;
    let expected_nodes = counts
        .0
        .checked_mul(2)
        .and_then(|count| count.checked_add(counts.1))
        .ok_or(FinalizedSemanticDebugMapErrorV1::InvalidSemanticCorrespondence)?;
    if map.nodes().len() != expected_nodes
        || map.mappings().len() != counts.0.saturating_mul(2)
        || map.boundaries().len() != counts.1
    {
        return Err(FinalizedSemanticDebugMapErrorV1::InvalidSemanticCorrespondence);
    }

    let mut nodes_by_location = Vec::new();
    nodes_by_location
        .try_reserve_exact(map.nodes().len())
        .map_err(|_| FinalizedSemanticDebugMapErrorV1::AllocationFailure)?;
    nodes_by_location.extend(map.nodes().iter().filter_map(|node| {
        (!matches!(node.location(), SemanticDebugLocationV1::Source { .. }))
            .then_some((node.location(), node.identity()))
    }));
    nodes_by_location.sort_unstable_by_key(|entry| entry.0);
    if nodes_by_location
        .windows(2)
        .any(|pair| pair[0].0 == pair[1].0)
    {
        return Err(FinalizedSemanticDebugMapErrorV1::InvalidSemanticCorrespondence);
    }
    let node_id = |location: SemanticDebugLocationV1| {
        nodes_by_location
            .binary_search_by_key(&location, |entry| entry.0)
            .ok()
            .map(|index| nodes_by_location[index].1)
    };

    for span in correspondence.statement_spans() {
        let function = semantic_mir
            .functions()
            .get(span.semantic_function() as usize)
            .ok_or(FinalizedSemanticDebugMapErrorV1::InvalidSemanticCorrespondence)?;
        let block = function
            .blocks()
            .get(span.semantic_block() as usize)
            .ok_or(FinalizedSemanticDebugMapErrorV1::InvalidSemanticCorrespondence)?;
        let statement = block
            .statements()
            .get(span.statement() as usize)
            .ok_or(FinalizedSemanticDebugMapErrorV1::InvalidSemanticCorrespondence)?;
        let origin = statement
            .source()
            .call_site()
            .ok_or(FinalizedSemanticDebugMapErrorV1::InvalidSemanticCorrespondence)?;
        let (byte_start, byte_end) = origin.byte_range();
        let (line, column) = origin.start_coordinate();
        let source_span = if span.operation_count() == 0 {
            DebugSourceMapSpanV1::new_eliminated(
                *origin.file().as_bytes(),
                byte_start,
                byte_end,
                line,
                column,
            )
        } else {
            DebugSourceMapSpanV1::new(
                *origin.file().as_bytes(),
                byte_start,
                byte_end,
                line,
                column,
            )
        }
        .map_err(|_| FinalizedSemanticDebugMapErrorV1::InvalidSemanticCorrespondence)?;
        let source_location = SemanticDebugLocationV1::Source { span: source_span };
        let mir_location = SemanticDebugLocationV1::Mir {
            body_ordinal: u64::from(span.semantic_function()),
            block_ordinal: u64::from(span.semantic_block()),
            statement_ordinal: u64::from(span.statement()),
        };
        let mir_id = node_id(mir_location)
            .ok_or(FinalizedSemanticDebugMapErrorV1::InvalidSemanticCorrespondence)?;
        let source_mapping = map
            .mapping_to(mir_id)
            .filter(|mapping| mapping.inputs().len() == 1)
            .ok_or(FinalizedSemanticDebugMapErrorV1::InvalidSemanticCorrespondence)?;
        let source_id = source_mapping.inputs()[0];
        if map.node(source_id).map(|node| node.location()) != Some(source_location) {
            return Err(FinalizedSemanticDebugMapErrorV1::InvalidSemanticCorrespondence);
        }
        require_mapping(
            map,
            source_id,
            &[mir_id],
            SemanticDebugLayerV1::Source,
            SemanticDebugLayerV1::Mir,
            SemanticDebugTransformationV1::Preserved,
        )?;

        if span.operation_count() == 0 {
            if source_map.eliminated().binary_search(&source_span).is_err() {
                return Err(FinalizedSemanticDebugMapErrorV1::InvalidSemanticCorrespondence);
            }
            require_unavailable_mapping(
                map,
                mir_id,
                SemanticDebugLayerV1::Mir,
                SemanticDebugLayerV1::Kir,
                SemanticDebugTransformationV1::Eliminated,
                SemanticDebugUnavailableReasonV1::Eliminated,
            )?;
            continue;
        }

        let block_index = block_ordinals
            .binary_search_by_key(&span.kernel_ir_block(), |entry| entry.0)
            .ok()
            .map(|index| block_ordinals[index].1)
            .ok_or(FinalizedSemanticDebugMapErrorV1::InvalidSemanticCorrespondence)?;
        let kir_block = body
            .blocks
            .get(block_index)
            .ok_or(FinalizedSemanticDebugMapErrorV1::InvalidSemanticCorrespondence)?;
        let end = span
            .first_operation()
            .checked_add(span.operation_count())
            .ok_or(FinalizedSemanticDebugMapErrorV1::InvalidSemanticCorrespondence)?;
        if end as usize > kir_block.operations.len() {
            return Err(FinalizedSemanticDebugMapErrorV1::InvalidSemanticCorrespondence);
        }
        let block_ordinal = u64::try_from(block_index)
            .map_err(|_| FinalizedSemanticDebugMapErrorV1::InvalidSemanticCorrespondence)?;
        let mut kir_ids = Vec::new();
        kir_ids
            .try_reserve_exact(span.operation_count() as usize)
            .map_err(|_| FinalizedSemanticDebugMapErrorV1::AllocationFailure)?;
        for operation in span.first_operation()..end {
            let site = DebugSourceMapKirSiteV1::operation(
                function_ordinal,
                block_ordinal,
                u64::from(operation),
            );
            let source_site = source_map
                .sites()
                .binary_search_by_key(&site, |entry| entry.site())
                .ok()
                .map(|index| &source_map.sites()[index])
                .ok_or(FinalizedSemanticDebugMapErrorV1::InvalidSemanticCorrespondence)?;
            if source_site.spans() != [source_span] {
                return Err(FinalizedSemanticDebugMapErrorV1::InvalidSemanticCorrespondence);
            }
            let kir_location = SemanticDebugLocationV1::Kir {
                function_ordinal,
                block_ordinal,
                operation_ordinal: u64::from(operation),
            };
            let kir_id = node_id(kir_location)
                .ok_or(FinalizedSemanticDebugMapErrorV1::InvalidSemanticCorrespondence)?;
            let boundary = map.boundaries().iter().filter(|boundary| {
                boundary.node() == kir_id
                    && boundary.direction()
                        == fe2o3_kernel_ir::SemanticDebugBoundaryDirectionV1::SuccessorUnavailable
                    && boundary.reason()
                        == fe2o3_kernel_ir::SemanticDebugBoundaryReasonV1::UnsupportedLayer
            });
            if boundary.count() != 1 {
                return Err(FinalizedSemanticDebugMapErrorV1::InvalidSemanticCorrespondence);
            }
            kir_ids.push(kir_id);
        }
        let transformation = if kir_ids.len() == 1 {
            SemanticDebugTransformationV1::Preserved
        } else {
            SemanticDebugTransformationV1::Duplicated
        };
        require_mapping(
            map,
            mir_id,
            &kir_ids,
            SemanticDebugLayerV1::Mir,
            SemanticDebugLayerV1::Kir,
            transformation,
        )?;
    }
    Ok(())
}

fn require_mapping(
    map: &SemanticDebugMapDocumentV1,
    input: [u8; 32],
    outputs: &[[u8; 32]],
    input_layer: SemanticDebugLayerV1,
    output_layer: SemanticDebugLayerV1,
    transformation: SemanticDebugTransformationV1,
) -> Result<(), FinalizedSemanticDebugMapErrorV1> {
    let mapping = map
        .mapping_from(input)
        .ok_or(FinalizedSemanticDebugMapErrorV1::InvalidSemanticCorrespondence)?;
    let mut expected_outputs = bounded_copy_identities(outputs)?;
    expected_outputs.sort_unstable();
    if mapping.inputs() != [input]
        || mapping.output().nodes() != expected_outputs
        || mapping.input_layer() != input_layer
        || mapping.output_layer() != output_layer
        || mapping.transformation() != transformation
    {
        return Err(FinalizedSemanticDebugMapErrorV1::InvalidSemanticCorrespondence);
    }
    Ok(())
}

fn require_unavailable_mapping(
    map: &SemanticDebugMapDocumentV1,
    input: [u8; 32],
    input_layer: SemanticDebugLayerV1,
    output_layer: SemanticDebugLayerV1,
    transformation: SemanticDebugTransformationV1,
    reason: SemanticDebugUnavailableReasonV1,
) -> Result<(), FinalizedSemanticDebugMapErrorV1> {
    let mapping = map
        .mapping_from(input)
        .ok_or(FinalizedSemanticDebugMapErrorV1::InvalidSemanticCorrespondence)?;
    if mapping.inputs() != [input]
        || !mapping.output().nodes().is_empty()
        || mapping.output().reason() != Some(reason)
        || mapping.input_layer() != input_layer
        || mapping.output_layer() != output_layer
        || mapping.transformation() != transformation
    {
        return Err(FinalizedSemanticDebugMapErrorV1::InvalidSemanticCorrespondence);
    }
    Ok(())
}

fn bounded_copy_identities(
    values: &[[u8; 32]],
) -> Result<Vec<[u8; 32]>, FinalizedSemanticDebugMapErrorV1> {
    let mut copy = Vec::new();
    copy.try_reserve_exact(values.len())
        .map_err(|_| FinalizedSemanticDebugMapErrorV1::AllocationFailure)?;
    copy.extend_from_slice(values);
    Ok(copy)
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
    ProductionFragment(ProductionSemanticDebugFragmentErrorV1),
    ProductionAssociation,
    ProductionAssociationMismatch,
    InvalidKirToLlvmReplay,
    KirToLlvmReplayTargetMismatch,
    InvalidLlvmToHsacoCustody,
    InvalidBoundSourceMap,
    InvalidBoundSemanticMir,
    InvalidBoundCorrespondenceV4,
    InvalidBoundCanonicalKirV8,
    InvalidBoundCanonicalKirV7,
    CanonicalKirProjectionMismatch,
    CorrespondenceIdentityMismatch,
    InvalidSemanticCorrespondence,
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
            Self::ProductionFragment(error) => Some(error),
            Self::ProductionAssociation
            | Self::ProductionAssociationMismatch
            | Self::InvalidKirToLlvmReplay
            | Self::KirToLlvmReplayTargetMismatch
            | Self::InvalidLlvmToHsacoCustody
            | Self::InvalidBoundSourceMap
            | Self::InvalidBoundSemanticMir
            | Self::InvalidBoundCorrespondenceV4
            | Self::InvalidBoundCanonicalKirV8
            | Self::InvalidBoundCanonicalKirV7
            | Self::CanonicalKirProjectionMismatch
            | Self::CorrespondenceIdentityMismatch
            | Self::InvalidSemanticCorrespondence
            | Self::ArtifactInspection
            | Self::AllocationFailure => None,
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

#[cfg(test)]
mod production_correspondence_tests {
    use std::collections::BTreeMap;

    use fe2o3_kernel_ir::{
        DebugSourceMapBindingV1, DebugSourceMapDocumentV2, DebugSourceMapFileV1,
        DebugSourceMapKirSiteV1, DebugSourceMapSiteV1, DebugSourceMapSpanV1,
        PRODUCTION_PREFINALIZED_ARTIFACT_STATUS_V1, ProductionScheduleStatusV1,
        SemanticDebugBoundaryDirectionV1, SemanticDebugBoundaryReasonV1, SemanticDebugBoundaryV1,
        SemanticDebugContentIdentityV1, SemanticDebugLayerV1, SemanticDebugLocationV1,
        SemanticDebugMapBindingV1, SemanticDebugMapDocumentV1, SemanticDebugMappingOutputV1,
        SemanticDebugMappingV1, SemanticDebugNodeV1, SemanticDebugTransformationV1,
        SemanticDebugUnavailableReasonV1, VerifiedCanonicalKernelIrV7, VerifiedCanonicalKernelIrV8,
    };
    use fe2o3_lower_mir_kernel::InertCanonicalMirToKirCorrespondenceEvidenceV4;
    use fe2o3_mir_model::semantic_mir_v1::{AdmittedInertSemanticMirV1, SemanticMirLimitsV1};
    use sha2::{Digest, Sha256};

    use super::*;

    #[allow(dead_code)]
    mod compiler_proof_inputs_v3 {
        include!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../tests/support/compiler_proof_inputs_v3.rs"
        ));
    }

    struct ExactMapFixture {
        map: SemanticDebugMapDocumentV1,
        source_map: Vec<u8>,
        semantic_mir: Vec<u8>,
        canonical_kir_v8: Vec<u8>,
        canonical_kir_v7: Vec<u8>,
        correspondence: Vec<u8>,
    }

    fn stable_id(tag: u8, values: &[u32]) -> [u8; 32] {
        let mut digest = Sha256::new();
        digest.update(b"FE2O3/SEMANTIC-DEBUG-FINALIZER-TEST-ID/V1\0");
        digest.update([tag]);
        for value in values {
            digest.update(value.to_le_bytes());
        }
        digest.finalize().into()
    }

    #[allow(clippy::too_many_lines)]
    fn exact_map_fixture() -> ExactMapFixture {
        let proof =
            compiler_proof_inputs_v3::canonical_compiler_proof_inputs_v4_with_sourceful_induction(
                0x20,
            );
        let semantic_mir = proof.semantic_mir().to_vec();
        let canonical_kir_v8 = proof.kernel_ir().to_vec();
        let correspondence = proof.correspondence().to_vec();
        let (_, module) =
            VerifiedCanonicalKernelIrV8::from_canonical_bytes_with_module(canonical_kir_v8.clone())
                .unwrap();
        let canonical_kir_v7 = VerifiedCanonicalKernelIrV7::from_module(module.clone())
            .unwrap()
            .into_canonical_bytes();
        let semantic = AdmittedInertSemanticMirV1::decode_current_production_canonical(
            &semantic_mir,
            SemanticMirLimitsV1::default(),
        )
        .unwrap();
        let correspondence =
            InertCanonicalMirToKirCorrespondenceEvidenceV4::decode(&correspondence).unwrap();
        let mut bodies = module
            .functions
            .iter()
            .enumerate()
            .filter_map(|(ordinal, function)| function.body.as_ref().map(|body| (ordinal, body)));
        let (function_ordinal, body) = bodies.next().unwrap();
        assert!(bodies.next().is_none());
        let block_ordinals = body
            .blocks
            .iter()
            .enumerate()
            .map(|(ordinal, block)| (block.id.0, ordinal))
            .collect::<BTreeMap<_, _>>();
        let mut sites = Vec::new();
        let mut nodes = Vec::new();
        let mut mappings = Vec::new();
        let mut boundaries = Vec::new();
        let mut eliminated = Vec::new();
        for span in correspondence.statement_spans() {
            let statement = &semantic.functions()[span.semantic_function() as usize].blocks()
                [span.semantic_block() as usize]
                .statements()[span.statement() as usize];
            let origin = statement.source().call_site().unwrap();
            let (byte_start, byte_end) = origin.byte_range();
            let (line, column) = origin.start_coordinate();
            let source_span = if span.operation_count() == 0 {
                DebugSourceMapSpanV1::new_eliminated(
                    *origin.file().as_bytes(),
                    byte_start,
                    byte_end,
                    line,
                    column,
                )
            } else {
                DebugSourceMapSpanV1::new(
                    *origin.file().as_bytes(),
                    byte_start,
                    byte_end,
                    line,
                    column,
                )
            }
            .unwrap();
            let source_id = stable_id(
                1,
                &[
                    span.semantic_function(),
                    span.semantic_block(),
                    span.statement(),
                ],
            );
            let mir_id = stable_id(
                2,
                &[
                    span.semantic_function(),
                    span.semantic_block(),
                    span.statement(),
                ],
            );
            nodes.push(
                SemanticDebugNodeV1::new(
                    source_id,
                    SemanticDebugLocationV1::Source { span: source_span },
                )
                .unwrap(),
            );
            nodes.push(
                SemanticDebugNodeV1::new(
                    mir_id,
                    SemanticDebugLocationV1::Mir {
                        body_ordinal: u64::from(span.semantic_function()),
                        block_ordinal: u64::from(span.semantic_block()),
                        statement_ordinal: u64::from(span.statement()),
                    },
                )
                .unwrap(),
            );
            mappings.push(
                SemanticDebugMappingV1::new(
                    stable_id(
                        4,
                        &[
                            span.semantic_function(),
                            span.semantic_block(),
                            span.statement(),
                        ],
                    ),
                    SemanticDebugLayerV1::Source,
                    SemanticDebugLayerV1::Mir,
                    SemanticDebugTransformationV1::Preserved,
                    vec![source_id],
                    SemanticDebugMappingOutputV1::available(vec![mir_id]),
                )
                .unwrap(),
            );
            if span.operation_count() == 0 {
                eliminated.push(source_span);
                mappings.push(
                    SemanticDebugMappingV1::new(
                        stable_id(
                            5,
                            &[
                                span.semantic_function(),
                                span.semantic_block(),
                                span.statement(),
                            ],
                        ),
                        SemanticDebugLayerV1::Mir,
                        SemanticDebugLayerV1::Kir,
                        SemanticDebugTransformationV1::Eliminated,
                        vec![mir_id],
                        SemanticDebugMappingOutputV1::unavailable(
                            SemanticDebugUnavailableReasonV1::Eliminated,
                        ),
                    )
                    .unwrap(),
                );
                continue;
            }
            let block_ordinal = block_ordinals[&span.kernel_ir_block()];
            let mut kir_ids = Vec::new();
            for operation in span.first_operation()
                ..span
                    .first_operation()
                    .checked_add(span.operation_count())
                    .unwrap()
            {
                let site = DebugSourceMapKirSiteV1::operation(
                    function_ordinal as u64,
                    block_ordinal as u64,
                    u64::from(operation),
                );
                sites.push(DebugSourceMapSiteV1::new(site, vec![source_span]).unwrap());
                let kir_id = stable_id(3, &[span.kernel_ir_block(), operation]);
                nodes.push(
                    SemanticDebugNodeV1::new(
                        kir_id,
                        SemanticDebugLocationV1::Kir {
                            function_ordinal: function_ordinal as u64,
                            block_ordinal: block_ordinal as u64,
                            operation_ordinal: u64::from(operation),
                        },
                    )
                    .unwrap(),
                );
                boundaries.push(
                    SemanticDebugBoundaryV1::new(
                        kir_id,
                        SemanticDebugBoundaryDirectionV1::SuccessorUnavailable,
                        SemanticDebugBoundaryReasonV1::UnsupportedLayer,
                    )
                    .unwrap(),
                );
                kir_ids.push(kir_id);
            }
            mappings.push(
                SemanticDebugMappingV1::new(
                    stable_id(
                        5,
                        &[
                            span.semantic_function(),
                            span.semantic_block(),
                            span.statement(),
                        ],
                    ),
                    SemanticDebugLayerV1::Mir,
                    SemanticDebugLayerV1::Kir,
                    if kir_ids.len() == 1 {
                        SemanticDebugTransformationV1::Preserved
                    } else {
                        SemanticDebugTransformationV1::Duplicated
                    },
                    vec![mir_id],
                    SemanticDebugMappingOutputV1::available(kir_ids),
                )
                .unwrap(),
            );
        }
        assert!(
            mappings
                .iter()
                .filter(|mapping| { mapping.input_layer() == SemanticDebugLayerV1::Source })
                .count()
                >= 2
        );
        let file = semantic
            .functions()
            .iter()
            .flat_map(|function| function.blocks())
            .flat_map(|block| block.statements())
            .find_map(|statement| statement.source().call_site())
            .unwrap()
            .file();
        let kir_owner =
            VerifiedCanonicalKernelIrV7::from_canonical_bytes(canonical_kir_v7.clone()).unwrap();
        let source_map = DebugSourceMapDocumentV2::new(
            DebugSourceMapBindingV1::new(
                [0x70; 32],
                *kir_owner.identity().digest(),
                kir_owner.identity().canonical_length(),
            )
            .unwrap(),
            vec![
                DebugSourceMapFileV1::new(*file.as_bytes(), 4096, "/src/sourceful.rs".into())
                    .unwrap(),
            ],
            sites,
            eliminated,
            Vec::new(),
            Vec::new(),
        )
        .unwrap()
        .to_canonical_json_bytes()
        .unwrap();
        let schedule = ProductionScheduleStatusV1::NoProductionScheduleStage.canonical_bytes();
        let content = |bytes: &[u8]| SemanticDebugContentIdentityV1::calculate(bytes).unwrap();
        let binding = SemanticDebugMapBindingV1::new(
            content(&source_map),
            content(&semantic_mir),
            content(&canonical_kir_v7),
            content(&schedule),
            content(b"whole-module-llvm"),
            content(PRODUCTION_PREFINALIZED_ARTIFACT_STATUS_V1),
        )
        .unwrap();
        ExactMapFixture {
            map: SemanticDebugMapDocumentV1::new_partial(binding, nodes, mappings, boundaries)
                .unwrap(),
            source_map,
            semantic_mir,
            canonical_kir_v8,
            canonical_kir_v7,
            correspondence: correspondence.canonical_bytes().to_vec(),
        }
    }

    #[test]
    fn exact_v4_edges_are_admitted_and_coordinated_valid_member_reseal_is_rejected() {
        let fixture = exact_map_fixture();
        validate_exact_production_correspondence(
            &fixture.map,
            &fixture.source_map,
            &fixture.semantic_mir,
            &fixture.canonical_kir_v8,
            &fixture.canonical_kir_v7,
            &fixture.correspondence,
        )
        .unwrap();

        let source_mapping_indices = fixture
            .map
            .mappings()
            .iter()
            .enumerate()
            .filter(|(_, mapping)| mapping.input_layer() == SemanticDebugLayerV1::Source)
            .map(|(index, _)| index)
            .take(2)
            .collect::<Vec<_>>();
        assert_eq!(source_mapping_indices.len(), 2);
        let mut mappings = fixture.map.mappings().to_vec();
        let first = source_mapping_indices[0];
        let second = source_mapping_indices[1];
        let first_output = mappings[first].output().nodes()[0];
        let second_output = mappings[second].output().nodes()[0];
        for (index, replacement) in [(first, second_output), (second, first_output)] {
            let original = &mappings[index];
            mappings[index] = SemanticDebugMappingV1::new(
                original.identity(),
                original.input_layer(),
                original.output_layer(),
                original.transformation(),
                original.inputs().to_vec(),
                SemanticDebugMappingOutputV1::available(vec![replacement]),
            )
            .unwrap();
        }
        let resealed = SemanticDebugMapDocumentV1::new_partial(
            fixture.map.binding(),
            fixture.map.nodes().to_vec(),
            mappings,
            fixture.map.boundaries().to_vec(),
        )
        .unwrap();
        assert!(matches!(
            validate_exact_production_correspondence(
                &resealed,
                &fixture.source_map,
                &fixture.semantic_mir,
                &fixture.canonical_kir_v8,
                &fixture.canonical_kir_v7,
                &fixture.correspondence,
            ),
            Err(FinalizedSemanticDebugMapErrorV1::InvalidSemanticCorrespondence)
        ));
    }
}

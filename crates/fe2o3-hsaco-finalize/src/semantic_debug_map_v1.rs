//! Finalizer admission for exact-artifact-bound semantic debug maps.

use std::{
    collections::{BTreeMap, BTreeSet},
    error::Error,
    fmt,
};

use dialect_amdgcn::CanonicalProductionKirToLlvmReplayEvidenceV1;
use fe2o3_compiler_lineage::{
    InertSemanticToLlvmAssociationInputsV3, InertSemanticToLlvmAssociationV3,
    InertSemanticToLlvmContentIdentityV3, MultiRootCanonicalKirVersionV2,
    MultiRootCorrespondenceFunctionRoleV2, MultiRootCorrespondencePayloadV2,
    MultiRootCorrespondenceSyntheticRuleV2, MultiRootProofRosterKindV2,
    MultiRootProofRosterTranscriptV2,
};
use fe2o3_kernel_ir::{
    DebugSourceMapDocumentV2, DebugSourceMapKirSiteV1, DebugSourceMapSpanV1,
    MAX_SEMANTIC_DEBUG_BOUNDARIES_V1, MAX_SEMANTIC_DEBUG_MAPPING_REFERENCES_V1,
    ProductionSemanticDebugAvailabilityV1, ProductionSemanticDebugCarrierV1,
    ProductionSemanticDebugFragmentErrorV1, ProductionSemanticDebugProducerGapV1,
    ProductionSemanticDebugReceiptExtensionV1, SemanticDebugBoundaryDirectionV1,
    SemanticDebugBoundaryV1, SemanticDebugContentIdentityV1, SemanticDebugLayerV1,
    SemanticDebugLocationV1, SemanticDebugMapDocumentV1, SemanticDebugMapErrorV1,
    SemanticDebugMapInputsV1, SemanticDebugMappingV1, SemanticDebugRelationCardinalityV2,
    SemanticDebugTransformationAvailabilityV2, SemanticDebugTransformationCapabilityV2,
    SemanticDebugTransformationClassV2, SemanticDebugTransformationClassificationV2,
    SemanticDebugTransformationEvidenceKindV2, SemanticDebugTransformationEvidenceV2,
    SemanticDebugTransformationMapBindingV2, SemanticDebugTransformationMapDocumentV2,
    SemanticDebugTransformationMapErrorV2, SemanticDebugTransformationRelationV2,
    SemanticDebugTransformationUnavailableReasonV2, SemanticDebugTransformationV1,
    SemanticDebugUnavailableReasonV1, VerifiedCanonicalKernelIrV8, decode_module_v7,
    semantic_debug_map_identity_v1,
};
use fe2o3_lower_mir_kernel::{
    InertCanonicalMirToKirCorrespondenceEvidenceV4, InertCanonicalMirToKirCorrespondenceEvidenceV5,
    MirToKirFunctionRoleEvidenceV5, ProductionCanonicalKernelIrVersionV1,
};
use fe2o3_mir_model::{
    InertCanonicalSemanticU32InductionEvidenceV1,
    semantic_mir_v1::{AdmittedInertSemanticMirV1, SemanticFunctionIdV1, SemanticMirLimitsV1},
};

use crate::{
    ContentIdentityV1, PreparedFinalizedProtectedWorkerV3HsacoV1,
    inspect_and_bind_kernel_descriptors,
    semantic_debug_instance_custody_v1::{
        ProductionSemanticDebugFunctionInstanceInputV1,
        ProductionSemanticDebugFunctionInstanceRoleV1,
        ProductionSemanticDebugInstanceCustodyAvailabilityV1,
        ProductionSemanticDebugInstanceCustodyBindingV1,
        ProductionSemanticDebugInstanceCustodyErrorV1,
        ProductionSemanticDebugInstanceCustodyUnavailableV1,
        ProductionSemanticDebugInstanceCustodyV1, ProductionSemanticDebugStatementInstanceInputV1,
    },
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
    graph_indices: SemanticDebugGraphIndicesV1,
    transformation_map_v2: Option<SemanticDebugTransformationMapDocumentV2>,
    instance_custody_v1: ProductionSemanticDebugInstanceCustodyAvailabilityV1,
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

    /// Returns exact cardinality and only producer-authenticated transformation classes.
    ///
    /// Legacy artifact-only admission has no correspondence evidence and returns `None`.
    pub const fn transformation_map_v2(&self) -> Option<&SemanticDebugTransformationMapDocumentV2> {
        self.transformation_map_v2.as_ref()
    }

    /// Returns exact root-instance ownership or a typed correspondence boundary.
    pub const fn instance_custody_v1(
        &self,
    ) -> &ProductionSemanticDebugInstanceCustodyAvailabilityV1 {
        &self.instance_custody_v1
    }

    pub(crate) const fn graph_indices(&self) -> &SemanticDebugGraphIndicesV1 {
        &self.graph_indices
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

#[derive(Debug)]
pub(crate) struct SemanticDebugGraphIndicesV1 {
    mapping_inputs: Vec<([u8; 32], usize)>,
    mapping_outputs: Vec<([u8; 32], usize)>,
    boundaries: Vec<(([u8; 32], SemanticDebugBoundaryDirectionV1), usize)>,
}

impl SemanticDebugGraphIndicesV1 {
    fn try_new(map: &SemanticDebugMapDocumentV1) -> Result<Self, FinalizedSemanticDebugMapErrorV1> {
        let (input_count, output_count) = map
            .mappings()
            .iter()
            .try_fold((0_usize, 0_usize), |(inputs, outputs), mapping| {
                Some((
                    inputs.checked_add(mapping.inputs().len())?,
                    outputs.checked_add(mapping.output().nodes().len())?,
                ))
            })
            .ok_or(FinalizedSemanticDebugMapErrorV1::InvalidSemanticCorrespondence)?;
        validate_graph_index_cardinalities(input_count, output_count, map.boundaries().len())?;

        let mut mapping_inputs = Vec::new();
        let mut mapping_outputs = Vec::new();
        let mut boundaries = Vec::new();
        mapping_inputs
            .try_reserve_exact(input_count)
            .map_err(|_| FinalizedSemanticDebugMapErrorV1::AllocationFailure)?;
        mapping_outputs
            .try_reserve_exact(output_count)
            .map_err(|_| FinalizedSemanticDebugMapErrorV1::AllocationFailure)?;
        boundaries
            .try_reserve_exact(map.boundaries().len())
            .map_err(|_| FinalizedSemanticDebugMapErrorV1::AllocationFailure)?;

        for (mapping_index, mapping) in map.mappings().iter().enumerate() {
            mapping_inputs.extend(
                mapping
                    .inputs()
                    .iter()
                    .map(|identity| (*identity, mapping_index)),
            );
            mapping_outputs.extend(
                mapping
                    .output()
                    .nodes()
                    .iter()
                    .map(|identity| (*identity, mapping_index)),
            );
        }
        boundaries.extend(
            map.boundaries()
                .iter()
                .enumerate()
                .map(|(boundary_index, boundary)| {
                    ((boundary.node(), boundary.direction()), boundary_index)
                }),
        );
        normalize_unique_owner_index(&mut mapping_inputs)?;
        normalize_unique_owner_index(&mut mapping_outputs)?;
        normalize_unique_owner_index(&mut boundaries)?;
        Ok(Self {
            mapping_inputs,
            mapping_outputs,
            boundaries,
        })
    }

    pub(crate) fn mapping_from<'a>(
        &self,
        map: &'a SemanticDebugMapDocumentV1,
        identity: [u8; 32],
    ) -> Option<&'a SemanticDebugMappingV1> {
        owner_lookup(&self.mapping_inputs, &identity).and_then(|index| map.mappings().get(index))
    }

    pub(crate) fn mapping_to<'a>(
        &self,
        map: &'a SemanticDebugMapDocumentV1,
        identity: [u8; 32],
    ) -> Option<&'a SemanticDebugMappingV1> {
        owner_lookup(&self.mapping_outputs, &identity).and_then(|index| map.mappings().get(index))
    }

    fn boundary<'a>(
        &self,
        map: &'a SemanticDebugMapDocumentV1,
        identity: [u8; 32],
        direction: SemanticDebugBoundaryDirectionV1,
    ) -> Option<&'a SemanticDebugBoundaryV1> {
        owner_lookup(&self.boundaries, &(identity, direction))
            .and_then(|index| map.boundaries().get(index))
    }
}

fn validate_graph_index_cardinalities(
    input_count: usize,
    output_count: usize,
    boundary_count: usize,
) -> Result<(), FinalizedSemanticDebugMapErrorV1> {
    let reference_count = input_count
        .checked_add(output_count)
        .ok_or(FinalizedSemanticDebugMapErrorV1::InvalidSemanticCorrespondence)?;
    if reference_count > MAX_SEMANTIC_DEBUG_MAPPING_REFERENCES_V1
        || boundary_count > MAX_SEMANTIC_DEBUG_BOUNDARIES_V1
    {
        return Err(FinalizedSemanticDebugMapErrorV1::InvalidSemanticCorrespondence);
    }
    Ok(())
}

fn normalize_unique_owner_index<K: Ord>(
    entries: &mut [(K, usize)],
) -> Result<(), FinalizedSemanticDebugMapErrorV1> {
    entries.sort_unstable_by(|left, right| left.0.cmp(&right.0));
    if entries.windows(2).any(|pair| pair[0].0 == pair[1].0) {
        return Err(FinalizedSemanticDebugMapErrorV1::InvalidSemanticCorrespondence);
    }
    Ok(())
}

fn owner_lookup<K: Ord + Copy>(entries: &[(K, usize)], key: &K) -> Option<usize> {
    entries
        .binary_search_by_key(key, |entry| entry.0)
        .ok()
        .map(|index| entries[index].1)
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
        let graph_indices = SemanticDebugGraphIndicesV1::try_new(&map)?;
        let replayed_instances = validate_exact_production_correspondence(
            &map,
            &graph_indices,
            fragment.source_map_v2(),
            semantic_mir,
            exact_kir_v8,
            fragment.canonical_kir_v7(),
            correspondence_bytes,
        )?;
        drop(graph_indices);

        let artifact_identity =
            SemanticDebugContentIdentityV1::calculate(self.exact_finalized_bytes())
                .map_err(FinalizedSemanticDebugMapErrorV1::SemanticMap)?;
        let map = map
            .with_finalized_artifact_identity(artifact_identity)
            .map_err(FinalizedSemanticDebugMapErrorV1::SemanticMap)?;
        let map_bytes = map
            .to_canonical_json_bytes()
            .map_err(FinalizedSemanticDebugMapErrorV1::SemanticMap)?;
        let mut admitted = self.admit_semantic_debug_map_with_inputs_v1(
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
        admitted.transformation_map_v2 = Some(build_production_transformation_map_v2(
            &admitted,
            correspondence_bytes,
        )?);
        admitted.instance_custody_v1 = match replayed_instances {
            Some(replayed) => {
                let binding = ProductionSemanticDebugInstanceCustodyBindingV1::from_exact_bytes(
                    fragment.source_map_v2(),
                    &map_bytes,
                    semantic_mir,
                    fragment.canonical_kir_v7(),
                    correspondence_bytes,
                )
                .map_err(FinalizedSemanticDebugMapErrorV1::InstanceCustodyV1)?;
                let custody = ProductionSemanticDebugInstanceCustodyV1::from_replayed_inputs(
                    binding,
                    replayed.functions,
                    replayed.statements,
                    admitted.document(),
                )
                .map_err(FinalizedSemanticDebugMapErrorV1::InstanceCustodyV1)?;
                ProductionSemanticDebugInstanceCustodyAvailabilityV1::Available(Box::new(custody))
            }
            None => ProductionSemanticDebugInstanceCustodyAvailabilityV1::Unavailable(
                ProductionSemanticDebugInstanceCustodyUnavailableV1::LegacyCorrespondenceV4,
            ),
        };
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

#[derive(Clone, Copy)]
struct ExactFinalizerStatementV1 {
    correspondence_owner: u32,
    semantic_function: u32,
    semantic_block: u32,
    statement: u32,
    kernel_ir_block: u32,
    first_operation: u32,
    operation_count: u32,
}

struct ExactFinalizerFunctionV1<'a> {
    correspondence_owner: u32,
    semantic_function: u32,
    function_ordinal: u64,
    instance_role: ProductionSemanticDebugFunctionInstanceRoleV1,
    body: &'a fe2o3_kernel_ir::FunctionBody,
    block_ordinals: Vec<(u32, usize)>,
}

struct ReplayedProductionSemanticDebugInstancesV1 {
    functions: Vec<ProductionSemanticDebugFunctionInstanceInputV1>,
    statements: Vec<ProductionSemanticDebugStatementInstanceInputV1>,
}

fn multi_root_finalizer_view_v1<'a>(
    roster: &MultiRootProofRosterTranscriptV2,
    semantic_mir: &AdmittedInertSemanticMirV1,
    module: &'a fe2o3_kernel_ir::Module,
) -> Result<
    (
        Vec<ExactFinalizerFunctionV1<'a>>,
        Vec<ExactFinalizerStatementV1>,
    ),
    FinalizedSemanticDebugMapErrorV1,
> {
    let mut layouts = Vec::new();
    let mut statements = Vec::new();
    let mut semantic_functions = BTreeMap::new();
    let mut physical_functions = BTreeMap::new();
    let mut kir_ordinals = BTreeSet::new();
    let mut previous_root = None;
    for root_ordinal in 0..roster.root_count() {
        let root = roster
            .root(root_ordinal)
            .ok_or(FinalizedSemanticDebugMapErrorV1::InvalidBoundMultiRootCorrespondenceV2)?;
        if previous_root.is_some_and(|previous| previous >= root.semantic_root()) {
            return Err(FinalizedSemanticDebugMapErrorV1::InvalidSemanticCorrespondence);
        }
        previous_root = Some(root.semantic_root());
        let root_function = semantic_mir
            .functions()
            .get(root.semantic_root() as usize)
            .ok_or(FinalizedSemanticDebugMapErrorV1::InvalidSemanticCorrespondence)?;
        if root_function.identity().as_bytes() != &root.semantic_root_identity() {
            return Err(FinalizedSemanticDebugMapErrorV1::InvalidSemanticCorrespondence);
        }
        let selected = semantic_mir
            .select_kernel_body_for_root_v1(SemanticFunctionIdV1::from_index(root.semantic_root()))
            .ok_or(FinalizedSemanticDebugMapErrorV1::InvalidSemanticCorrespondence)?;
        let payload = MultiRootCorrespondencePayloadV2::decode(root.payload())
            .map_err(|_| FinalizedSemanticDebugMapErrorV1::InvalidBoundMultiRootCorrespondenceV2)?;
        let expected_ordinal = u32::try_from(root_ordinal)
            .map_err(|_| FinalizedSemanticDebugMapErrorV1::InvalidSemanticCorrespondence)?;
        if payload.root_ordinal() != expected_ordinal
            || payload.correspondence_owner() != root.semantic_root()
        {
            return Err(FinalizedSemanticDebugMapErrorV1::InvalidSemanticCorrespondence);
        }
        let induction = InertCanonicalSemanticU32InductionEvidenceV1::decode(payload.induction())
            .map_err(|_| {
            FinalizedSemanticDebugMapErrorV1::InvalidBoundMultiRootCorrespondenceV2
        })?;
        if induction.semantic_mir_sha256() != semantic_mir.semantic_sha256().as_bytes()
            || induction.grants_authority()
        {
            return Err(FinalizedSemanticDebugMapErrorV1::CorrespondenceIdentityMismatch);
        }

        let mut local_functions = BTreeMap::new();
        let mut entry_count = 0_usize;
        for record in payload.functions() {
            let semantic = semantic_mir
                .functions()
                .get(record.semantic_function() as usize)
                .ok_or(FinalizedSemanticDebugMapErrorV1::InvalidSemanticCorrespondence)?;
            let mut matches = module
                .functions
                .iter()
                .enumerate()
                .filter(|(_, function)| function.id.as_str() == record.kernel_ir_function());
            let (ordinal, function) = matches
                .next()
                .ok_or(FinalizedSemanticDebugMapErrorV1::InvalidSemanticCorrespondence)?;
            if matches.next().is_some() {
                return Err(FinalizedSemanticDebugMapErrorV1::InvalidSemanticCorrespondence);
            }
            let role = record.role();
            if let Some((previous_ordinal, previous_role)) =
                semantic_functions.insert(record.semantic_function(), (ordinal, role))
                && (previous_ordinal != ordinal
                    || previous_role != MultiRootCorrespondenceFunctionRoleV2::InternalHelper
                    || role != MultiRootCorrespondenceFunctionRoleV2::InternalHelper)
            {
                return Err(FinalizedSemanticDebugMapErrorV1::InvalidSemanticCorrespondence);
            }
            if let Some((previous_semantic, previous_role)) =
                physical_functions.insert(ordinal, (record.semantic_function(), role))
                && (previous_semantic != record.semantic_function()
                    || previous_role != MultiRootCorrespondenceFunctionRoleV2::InternalHelper
                    || role != MultiRootCorrespondenceFunctionRoleV2::InternalHelper)
            {
                return Err(FinalizedSemanticDebugMapErrorV1::InvalidSemanticCorrespondence);
            }
            kir_ordinals.insert(ordinal);
            let body = function
                .body
                .as_ref()
                .ok_or(FinalizedSemanticDebugMapErrorV1::InvalidSemanticCorrespondence)?;
            let expected_role = match record.role() {
                MultiRootCorrespondenceFunctionRoleV2::KernelEntry => {
                    fe2o3_kernel_ir::FunctionRole::KernelEntry
                }
                MultiRootCorrespondenceFunctionRoleV2::InternalHelper => {
                    fe2o3_kernel_ir::FunctionRole::InternalHelper
                }
            };
            if function.role != expected_role
                || local_functions
                    .insert(record.semantic_function(), (semantic, ordinal, body))
                    .is_some()
            {
                return Err(FinalizedSemanticDebugMapErrorV1::InvalidSemanticCorrespondence);
            }
            if record.role() == MultiRootCorrespondenceFunctionRoleV2::KernelEntry {
                entry_count += 1;
                if record.semantic_function() != selected.body().index()
                    || record.kernel_ir_function() != root.kernel_id()
                {
                    return Err(FinalizedSemanticDebugMapErrorV1::InvalidSemanticCorrespondence);
                }
            }
            layouts.push(ExactFinalizerFunctionV1 {
                correspondence_owner: root.semantic_root(),
                semantic_function: record.semantic_function(),
                function_ordinal: u64::try_from(ordinal)
                    .map_err(|_| FinalizedSemanticDebugMapErrorV1::InvalidSemanticCorrespondence)?,
                instance_role: match record.role() {
                    MultiRootCorrespondenceFunctionRoleV2::KernelEntry => {
                        ProductionSemanticDebugFunctionInstanceRoleV1::KernelEntry
                    }
                    MultiRootCorrespondenceFunctionRoleV2::InternalHelper => {
                        ProductionSemanticDebugFunctionInstanceRoleV1::InternalHelper
                    }
                },
                body,
                block_ordinals: exact_block_ordinals_v1(body)?,
            });
        }
        if entry_count != 1 || local_functions.is_empty() {
            return Err(FinalizedSemanticDebugMapErrorV1::InvalidSemanticCorrespondence);
        }

        let expected_blocks = local_functions
            .values()
            .try_fold(0_usize, |total, (semantic, _, _)| {
                total.checked_add(semantic.blocks().len())
            })
            .ok_or(FinalizedSemanticDebugMapErrorV1::InvalidSemanticCorrespondence)?;
        if payload.blocks().len() != expected_blocks
            || payload.terminators().len() != expected_blocks
        {
            return Err(FinalizedSemanticDebugMapErrorV1::InvalidSemanticCorrespondence);
        }
        let mut semantic_to_kir = BTreeMap::new();
        let mut mapped_kir_blocks = BTreeSet::new();
        for record in payload.blocks() {
            let (semantic, _, body) = local_functions
                .get(&record.semantic_function())
                .ok_or(FinalizedSemanticDebugMapErrorV1::InvalidSemanticCorrespondence)?;
            let source = semantic
                .blocks()
                .get(record.semantic_block() as usize)
                .ok_or(FinalizedSemanticDebugMapErrorV1::InvalidSemanticCorrespondence)?;
            if source.statements().len() != record.source_statement_count() as usize
                || !body
                    .blocks
                    .iter()
                    .any(|block| block.id.0 == record.kernel_ir_block())
                || !mapped_kir_blocks.insert((record.semantic_function(), record.kernel_ir_block()))
                || semantic_to_kir
                    .insert(
                        (record.semantic_function(), record.semantic_block()),
                        record.kernel_ir_block(),
                    )
                    .is_some()
            {
                return Err(FinalizedSemanticDebugMapErrorV1::InvalidSemanticCorrespondence);
            }
        }
        let expected_statements = local_functions
            .values()
            .try_fold(0_usize, |total, (semantic, _, _)| {
                semantic.blocks().iter().try_fold(total, |total, block| {
                    total.checked_add(block.statements().len())
                })
            })
            .ok_or(FinalizedSemanticDebugMapErrorV1::InvalidSemanticCorrespondence)?;
        if payload.statements().len() != expected_statements {
            return Err(FinalizedSemanticDebugMapErrorV1::InvalidSemanticCorrespondence);
        }
        let statement_by_site = payload
            .statements()
            .iter()
            .map(|record| {
                (
                    (
                        record.semantic_function(),
                        record.semantic_block(),
                        record.statement(),
                    ),
                    *record,
                )
            })
            .collect::<BTreeMap<_, _>>();
        let terminator_by_site = payload
            .terminators()
            .iter()
            .map(|record| {
                (
                    (record.semantic_function(), record.semantic_block()),
                    *record,
                )
            })
            .collect::<BTreeMap<_, _>>();
        let mut synthetic_by_block = payload
            .synthetics()
            .iter()
            .map(|record| {
                (
                    (record.semantic_function(), record.kernel_ir_block()),
                    *record,
                )
            })
            .collect::<BTreeMap<_, _>>();
        if statement_by_site.len() != payload.statements().len()
            || terminator_by_site.len() != payload.terminators().len()
            || synthetic_by_block.len() != payload.synthetics().len()
        {
            return Err(FinalizedSemanticDebugMapErrorV1::InvalidSemanticCorrespondence);
        }
        for (&semantic_function, (semantic, _, body)) in &local_functions {
            let mut mapped = BTreeSet::new();
            for (semantic_block, source) in semantic.blocks().iter().enumerate() {
                let semantic_block = u32::try_from(semantic_block)
                    .map_err(|_| FinalizedSemanticDebugMapErrorV1::InvalidSemanticCorrespondence)?;
                let kir_block_id = *semantic_to_kir
                    .get(&(semantic_function, semantic_block))
                    .ok_or(FinalizedSemanticDebugMapErrorV1::InvalidSemanticCorrespondence)?;
                mapped.insert(kir_block_id);
                let block = body
                    .blocks
                    .iter()
                    .find(|block| block.id.0 == kir_block_id)
                    .ok_or(FinalizedSemanticDebugMapErrorV1::InvalidSemanticCorrespondence)?;
                let mut next = 0_u32;
                if let Some(synthetic) =
                    synthetic_by_block.remove(&(semantic_function, kir_block_id))
                {
                    if synthetic.rule()
                        != MultiRootCorrespondenceSyntheticRuleV2::EnumPayloadStorage
                        || synthetic.first_operation() != 0
                        || synthetic.operation_count() == 0
                    {
                        return Err(
                            FinalizedSemanticDebugMapErrorV1::InvalidSemanticCorrespondence,
                        );
                    }
                    next = synthetic.operation_count();
                }
                for statement in 0..source.statements().len() {
                    let statement = u32::try_from(statement).map_err(|_| {
                        FinalizedSemanticDebugMapErrorV1::InvalidSemanticCorrespondence
                    })?;
                    let span = statement_by_site
                        .get(&(semantic_function, semantic_block, statement))
                        .ok_or(FinalizedSemanticDebugMapErrorV1::InvalidSemanticCorrespondence)?;
                    if span.kernel_ir_block() != kir_block_id || span.first_operation() != next {
                        return Err(
                            FinalizedSemanticDebugMapErrorV1::InvalidSemanticCorrespondence,
                        );
                    }
                    next = next
                        .checked_add(span.operation_count())
                        .ok_or(FinalizedSemanticDebugMapErrorV1::InvalidSemanticCorrespondence)?;
                    statements.push(ExactFinalizerStatementV1 {
                        correspondence_owner: root.semantic_root(),
                        semantic_function,
                        semantic_block,
                        statement,
                        kernel_ir_block: kir_block_id,
                        first_operation: span.first_operation(),
                        operation_count: span.operation_count(),
                    });
                }
                let terminator = terminator_by_site
                    .get(&(semantic_function, semantic_block))
                    .ok_or(FinalizedSemanticDebugMapErrorV1::InvalidSemanticCorrespondence)?;
                if terminator.kernel_ir_block() != kir_block_id
                    || terminator.first_operation() != next
                {
                    return Err(FinalizedSemanticDebugMapErrorV1::InvalidSemanticCorrespondence);
                }
                next = next
                    .checked_add(terminator.operation_count())
                    .ok_or(FinalizedSemanticDebugMapErrorV1::InvalidSemanticCorrespondence)?;
                if next as usize != block.operations.len() || block.terminator.is_none() {
                    return Err(FinalizedSemanticDebugMapErrorV1::InvalidSemanticCorrespondence);
                }
            }
            for block in &body.blocks {
                if mapped.contains(&block.id.0) {
                    continue;
                }
                let synthetic = synthetic_by_block
                    .remove(&(semantic_function, block.id.0))
                    .ok_or(FinalizedSemanticDebugMapErrorV1::InvalidSemanticCorrespondence)?;
                if synthetic.rule()
                    != MultiRootCorrespondenceSyntheticRuleV2::RuntimeAssertFailureTrap
                    || synthetic.first_operation() != 0
                    || synthetic.operation_count() as usize != block.operations.len()
                    || block.operations.is_empty()
                    || block.terminator.is_none()
                {
                    return Err(FinalizedSemanticDebugMapErrorV1::InvalidSemanticCorrespondence);
                }
            }
        }
        if !synthetic_by_block.is_empty() {
            return Err(FinalizedSemanticDebugMapErrorV1::InvalidSemanticCorrespondence);
        }
        let mut parameters = BTreeSet::new();
        for parameter in payload.parameters() {
            let (semantic, _, body) = local_functions
                .get(&parameter.semantic_function())
                .ok_or(FinalizedSemanticDebugMapErrorV1::InvalidSemanticCorrespondence)?;
            if semantic
                .locals()
                .get(parameter.semantic_local() as usize)
                .is_none()
                || !body
                    .parameters
                    .iter()
                    .any(|value| value.0 == parameter.kernel_ir_value())
                || !parameters.insert((
                    parameter.semantic_function(),
                    parameter.semantic_local(),
                    parameter.kernel_ir_value(),
                ))
            {
                return Err(FinalizedSemanticDebugMapErrorV1::InvalidSemanticCorrespondence);
            }
        }
    }
    let defined_count = module
        .functions
        .iter()
        .filter(|function| function.body.is_some())
        .count();
    if kir_ordinals.len() != defined_count {
        return Err(FinalizedSemanticDebugMapErrorV1::InvalidSemanticCorrespondence);
    }
    layouts.sort_unstable_by_key(|layout| (layout.correspondence_owner, layout.semantic_function));
    statements.sort_unstable_by_key(|span| {
        (
            span.correspondence_owner,
            span.semantic_function,
            span.semantic_block,
            span.statement,
        )
    });
    if layouts.windows(2).any(|pair| {
        (pair[0].correspondence_owner, pair[0].semantic_function)
            >= (pair[1].correspondence_owner, pair[1].semantic_function)
    }) {
        return Err(FinalizedSemanticDebugMapErrorV1::InvalidSemanticCorrespondence);
    }
    Ok((layouts, statements))
}

#[allow(clippy::too_many_lines)]
fn validate_exact_production_correspondence(
    map: &SemanticDebugMapDocumentV1,
    graph_indices: &SemanticDebugGraphIndicesV1,
    source_map_bytes: &[u8],
    semantic_mir_bytes: &[u8],
    canonical_kir_v8_bytes: &[u8],
    canonical_kir_v7_bytes: &[u8],
    correspondence_bytes: &[u8],
) -> Result<Option<ReplayedProductionSemanticDebugInstancesV1>, FinalizedSemanticDebugMapErrorV1> {
    let source_map = DebugSourceMapDocumentV2::from_canonical_json_bytes(source_map_bytes)
        .map_err(|_| FinalizedSemanticDebugMapErrorV1::InvalidBoundSourceMap)?;
    let semantic_mir = AdmittedInertSemanticMirV1::decode_current_production_canonical(
        semantic_mir_bytes,
        SemanticMirLimitsV1::default(),
    )
    .map_err(|_| FinalizedSemanticDebugMapErrorV1::InvalidBoundSemanticMir)?;
    let multi_root = if correspondence_bytes.get(..8) == Some(b"F2MRCOR2") {
        Some(
            MultiRootProofRosterTranscriptV2::decode(correspondence_bytes).map_err(|_| {
                FinalizedSemanticDebugMapErrorV1::InvalidBoundMultiRootCorrespondenceV2
            })?,
        )
    } else {
        None
    };
    let correspondence_v5 = if multi_root.is_none()
        && correspondence_bytes.get(..8) == Some(b"F2M2K5\0\0")
    {
        let evidence = InertCanonicalMirToKirCorrespondenceEvidenceV5::decode(correspondence_bytes)
            .map_err(|_| FinalizedSemanticDebugMapErrorV1::InvalidBoundCorrespondenceV5)?;
        evidence
            .revalidate()
            .map_err(|_| FinalizedSemanticDebugMapErrorV1::InvalidBoundCorrespondenceV5)?;
        Some(evidence)
    } else {
        None
    };
    let legacy_v4 = if multi_root.is_none() && correspondence_v5.is_none() {
        let evidence = InertCanonicalMirToKirCorrespondenceEvidenceV4::decode(correspondence_bytes)
            .map_err(|_| FinalizedSemanticDebugMapErrorV1::InvalidBoundCorrespondenceV4)?;
        evidence
            .revalidate()
            .map_err(|_| FinalizedSemanticDebugMapErrorV1::InvalidBoundCorrespondenceV4)?;
        Some(evidence)
    } else {
        None
    };
    let correspondence = correspondence_v5
        .as_ref()
        .map(InertCanonicalMirToKirCorrespondenceEvidenceV5::nested_v4)
        .or(legacy_v4.as_ref());
    let retain_instances = multi_root.is_some() || correspondence_v5.is_some();
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
    if let Some(evidence) = correspondence_v5.as_ref() {
        evidence
            .validate_against_module(&module_v7)
            .map_err(|_| FinalizedSemanticDebugMapErrorV1::InvalidBoundCorrespondenceV5)?;
    }
    if let Some(roster) = multi_root.as_ref() {
        if roster.kind() != MultiRootProofRosterKindV2::Correspondence
            || roster.root_count() < 2
            || roster.semantic_mir_sha256() != *semantic_mir.semantic_sha256().as_bytes()
            || roster.neutral_kir().version() != MultiRootCanonicalKirVersionV2::V8
            || roster.neutral_kir().digest() != *canonical_kir_v8.identity().digest()
            || roster.neutral_kir().canonical_length()
                != canonical_kir_v8.identity().canonical_length()
        {
            return Err(FinalizedSemanticDebugMapErrorV1::CorrespondenceIdentityMismatch);
        }
    } else {
        let correspondence = correspondence
            .ok_or(FinalizedSemanticDebugMapErrorV1::InvalidSemanticCorrespondence)?;
        let correspondence_kir = correspondence.canonical_kernel_ir_identity();
        if correspondence_kir.version() != ProductionCanonicalKernelIrVersionV1::V8
            || correspondence_kir.digest() != canonical_kir_v8.identity().digest()
            || correspondence_kir.canonical_length()
                != canonical_kir_v8.identity().canonical_length()
            || correspondence.semantic_sha256() != semantic_mir.semantic_sha256().as_bytes()
        {
            return Err(FinalizedSemanticDebugMapErrorV1::CorrespondenceIdentityMismatch);
        }
    }

    let defined_function_count = module_v7
        .functions
        .iter()
        .filter(|function| function.body.is_some())
        .count();
    let mut function_layouts = Vec::new();
    let mut statement_spans = Vec::new();
    if let Some(roster) = multi_root.as_ref() {
        (function_layouts, statement_spans) =
            multi_root_finalizer_view_v1(roster, &semantic_mir, &module_v7)?;
    } else if let Some(evidence) = correspondence_v5.as_ref() {
        if defined_function_count != evidence.functions().len()
            || evidence.functions().is_empty()
            || evidence
                .functions()
                .iter()
                .map(|record| record.correspondence_owner())
                .collect::<BTreeSet<_>>()
                .len()
                != 1
            || evidence
                .functions()
                .iter()
                .filter(|record| record.role() == MirToKirFunctionRoleEvidenceV5::KernelEntry)
                .count()
                != 1
        {
            return Err(FinalizedSemanticDebugMapErrorV1::InvalidSemanticCorrespondence);
        }
        function_layouts
            .try_reserve_exact(evidence.functions().len())
            .map_err(|_| FinalizedSemanticDebugMapErrorV1::AllocationFailure)?;
        let mut ordinals = BTreeSet::new();
        for record in evidence.functions() {
            let ordinal = usize::try_from(record.kernel_ir_function_ordinal())
                .map_err(|_| FinalizedSemanticDebugMapErrorV1::InvalidSemanticCorrespondence)?;
            let function = module_v7
                .functions
                .get(ordinal)
                .ok_or(FinalizedSemanticDebugMapErrorV1::InvalidSemanticCorrespondence)?;
            let role = match record.role() {
                MirToKirFunctionRoleEvidenceV5::KernelEntry => {
                    fe2o3_kernel_ir::FunctionRole::KernelEntry
                }
                MirToKirFunctionRoleEvidenceV5::InternalHelper => {
                    fe2o3_kernel_ir::FunctionRole::InternalHelper
                }
            };
            let body = function
                .body
                .as_ref()
                .ok_or(FinalizedSemanticDebugMapErrorV1::InvalidSemanticCorrespondence)?;
            if !ordinals.insert(ordinal)
                || function.role != role
                || function.id.as_str() != record.kernel_ir_function()
            {
                return Err(FinalizedSemanticDebugMapErrorV1::InvalidSemanticCorrespondence);
            }
            function_layouts.push(ExactFinalizerFunctionV1 {
                correspondence_owner: record.correspondence_owner(),
                semantic_function: record.semantic_function(),
                function_ordinal: u64::from(record.kernel_ir_function_ordinal()),
                instance_role: match record.role() {
                    MirToKirFunctionRoleEvidenceV5::KernelEntry => {
                        ProductionSemanticDebugFunctionInstanceRoleV1::KernelEntry
                    }
                    MirToKirFunctionRoleEvidenceV5::InternalHelper => {
                        ProductionSemanticDebugFunctionInstanceRoleV1::InternalHelper
                    }
                },
                body,
                block_ordinals: exact_block_ordinals_v1(body)?,
            });
        }
        if ordinals.len() != defined_function_count {
            return Err(FinalizedSemanticDebugMapErrorV1::InvalidSemanticCorrespondence);
        }
    } else {
        let mut bodies = module_v7
            .functions
            .iter()
            .enumerate()
            .filter_map(|(ordinal, function)| function.body.as_ref().map(|body| (ordinal, body)));
        let Some((ordinal, body)) = bodies.next() else {
            return Err(FinalizedSemanticDebugMapErrorV1::InvalidSemanticCorrespondence);
        };
        let correspondence = correspondence
            .ok_or(FinalizedSemanticDebugMapErrorV1::InvalidSemanticCorrespondence)?;
        if bodies.next().is_some() || correspondence.function_count() != 1 {
            return Err(FinalizedSemanticDebugMapErrorV1::InvalidSemanticCorrespondence);
        }
        let semantic_function = correspondence
            .blocks()
            .first()
            .ok_or(FinalizedSemanticDebugMapErrorV1::InvalidSemanticCorrespondence)?
            .semantic_function();
        function_layouts.push(ExactFinalizerFunctionV1 {
            correspondence_owner: semantic_function,
            semantic_function,
            function_ordinal: u64::try_from(ordinal)
                .map_err(|_| FinalizedSemanticDebugMapErrorV1::InvalidSemanticCorrespondence)?,
            instance_role: ProductionSemanticDebugFunctionInstanceRoleV1::KernelEntry,
            body,
            block_ordinals: exact_block_ordinals_v1(body)?,
        });
    }
    if let Some(correspondence) = correspondence {
        statement_spans
            .try_reserve_exact(correspondence.statement_spans().len())
            .map_err(|_| FinalizedSemanticDebugMapErrorV1::AllocationFailure)?;
        statement_spans.extend(correspondence.statement_spans().iter().map(|span| {
            ExactFinalizerStatementV1 {
                correspondence_owner: function_layouts
                    .first()
                    .map_or(span.semantic_function(), |layout| {
                        layout.correspondence_owner
                    }),
                semantic_function: span.semantic_function(),
                semantic_block: span.semantic_block(),
                statement: span.statement(),
                kernel_ir_block: span.kernel_ir_block(),
                first_operation: span.first_operation(),
                operation_count: span.operation_count(),
            }
        }));
    }
    function_layouts
        .sort_unstable_by_key(|layout| (layout.correspondence_owner, layout.semantic_function));
    if function_layouts.windows(2).any(|pair| {
        (pair[0].correspondence_owner, pair[0].semantic_function)
            >= (pair[1].correspondence_owner, pair[1].semantic_function)
    }) {
        return Err(FinalizedSemanticDebugMapErrorV1::InvalidSemanticCorrespondence);
    }

    let mut statement_groups = BTreeMap::new();
    for span in &statement_spans {
        statement_groups
            .entry((span.semantic_function, span.semantic_block, span.statement))
            .or_insert_with(Vec::new)
            .push(span);
    }
    let mut instance_statements = Vec::new();
    if retain_instances {
        instance_statements
            .try_reserve_exact(statement_spans.len())
            .map_err(|_| FinalizedSemanticDebugMapErrorV1::AllocationFailure)?;
    }
    for instances in statement_groups.values() {
        let first = instances[0];
        let first_layout = function_layouts
            .binary_search_by_key(
                &(first.correspondence_owner, first.semantic_function),
                |layout| (layout.correspondence_owner, layout.semantic_function),
            )
            .ok()
            .and_then(|index| function_layouts.get(index))
            .ok_or(FinalizedSemanticDebugMapErrorV1::InvalidSemanticCorrespondence)?;
        for instance in &instances[1..] {
            let layout = function_layouts
                .binary_search_by_key(
                    &(instance.correspondence_owner, instance.semantic_function),
                    |layout| (layout.correspondence_owner, layout.semantic_function),
                )
                .ok()
                .and_then(|index| function_layouts.get(index))
                .ok_or(FinalizedSemanticDebugMapErrorV1::InvalidSemanticCorrespondence)?;
            if layout.function_ordinal != first_layout.function_ordinal
                || instance.kernel_ir_block != first.kernel_ir_block
                || instance.first_operation != first.first_operation
                || instance.operation_count != first.operation_count
            {
                return Err(FinalizedSemanticDebugMapErrorV1::InvalidSemanticCorrespondence);
            }
        }
    }

    let counts = statement_groups
        .values()
        .try_fold((0_usize, 0_usize), |(statements, operations), instances| {
            let span = instances[0];
            Some((
                statements.checked_add(1)?,
                operations.checked_add(span.operation_count as usize)?,
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

    for instances in statement_groups.values() {
        let span = instances[0];
        let layout = function_layouts
            .binary_search_by_key(
                &(span.correspondence_owner, span.semantic_function),
                |layout| (layout.correspondence_owner, layout.semantic_function),
            )
            .ok()
            .and_then(|index| function_layouts.get(index))
            .ok_or(FinalizedSemanticDebugMapErrorV1::InvalidSemanticCorrespondence)?;
        let function = semantic_mir
            .functions()
            .get(span.semantic_function as usize)
            .ok_or(FinalizedSemanticDebugMapErrorV1::InvalidSemanticCorrespondence)?;
        let block = function
            .blocks()
            .get(span.semantic_block as usize)
            .ok_or(FinalizedSemanticDebugMapErrorV1::InvalidSemanticCorrespondence)?;
        let statement = block
            .statements()
            .get(span.statement as usize)
            .ok_or(FinalizedSemanticDebugMapErrorV1::InvalidSemanticCorrespondence)?;
        let origin = statement
            .source()
            .call_site()
            .ok_or(FinalizedSemanticDebugMapErrorV1::InvalidSemanticCorrespondence)?;
        let (byte_start, byte_end) = origin.byte_range();
        let (line, column) = origin.start_coordinate();
        let source_span = if span.operation_count == 0 {
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
            body_ordinal: u64::from(span.semantic_function),
            block_ordinal: u64::from(span.semantic_block),
            statement_ordinal: u64::from(span.statement),
        };
        let mir_id = node_id(mir_location)
            .ok_or(FinalizedSemanticDebugMapErrorV1::InvalidSemanticCorrespondence)?;
        let source_mapping = graph_indices
            .mapping_to(map, mir_id)
            .filter(|mapping| mapping.inputs().len() == 1)
            .ok_or(FinalizedSemanticDebugMapErrorV1::InvalidSemanticCorrespondence)?;
        let source_id = source_mapping.inputs()[0];
        if map.node(source_id).map(|node| node.location()) != Some(source_location) {
            return Err(FinalizedSemanticDebugMapErrorV1::InvalidSemanticCorrespondence);
        }
        require_mapping(
            map,
            graph_indices,
            source_id,
            &[mir_id],
            SemanticDebugLayerV1::Source,
            SemanticDebugLayerV1::Mir,
            SemanticDebugTransformationV1::Preserved,
        )?;

        if span.operation_count == 0 {
            if source_map.eliminated().binary_search(&source_span).is_err() {
                return Err(FinalizedSemanticDebugMapErrorV1::InvalidSemanticCorrespondence);
            }
            require_unavailable_mapping(
                map,
                graph_indices,
                mir_id,
                SemanticDebugLayerV1::Mir,
                SemanticDebugLayerV1::Kir,
                SemanticDebugTransformationV1::Eliminated,
                SemanticDebugUnavailableReasonV1::Eliminated,
            )?;
            if retain_instances {
                for instance in instances {
                    instance_statements.push(ProductionSemanticDebugStatementInstanceInputV1 {
                        correspondence_owner: instance.correspondence_owner,
                        semantic_function: instance.semantic_function,
                        semantic_block: instance.semantic_block,
                        statement: instance.statement,
                        source_node: source_id,
                        mir_node: mir_id,
                        kir_nodes: Vec::new(),
                    });
                }
            }
            continue;
        }

        let block_index = layout
            .block_ordinals
            .binary_search_by_key(&span.kernel_ir_block, |entry| entry.0)
            .ok()
            .map(|index| layout.block_ordinals[index].1)
            .ok_or(FinalizedSemanticDebugMapErrorV1::InvalidSemanticCorrespondence)?;
        let kir_block = layout
            .body
            .blocks
            .get(block_index)
            .ok_or(FinalizedSemanticDebugMapErrorV1::InvalidSemanticCorrespondence)?;
        let end = span
            .first_operation
            .checked_add(span.operation_count)
            .ok_or(FinalizedSemanticDebugMapErrorV1::InvalidSemanticCorrespondence)?;
        if end as usize > kir_block.operations.len() {
            return Err(FinalizedSemanticDebugMapErrorV1::InvalidSemanticCorrespondence);
        }
        let block_ordinal = u64::try_from(block_index)
            .map_err(|_| FinalizedSemanticDebugMapErrorV1::InvalidSemanticCorrespondence)?;
        let mut kir_ids = Vec::new();
        kir_ids
            .try_reserve_exact(span.operation_count as usize)
            .map_err(|_| FinalizedSemanticDebugMapErrorV1::AllocationFailure)?;
        for operation in span.first_operation..end {
            let site = DebugSourceMapKirSiteV1::operation(
                layout.function_ordinal,
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
                function_ordinal: layout.function_ordinal,
                block_ordinal,
                operation_ordinal: u64::from(operation),
            };
            let kir_id = node_id(kir_location)
                .ok_or(FinalizedSemanticDebugMapErrorV1::InvalidSemanticCorrespondence)?;
            let boundary = graph_indices.boundary(
                map,
                kir_id,
                SemanticDebugBoundaryDirectionV1::SuccessorUnavailable,
            );
            if boundary.is_none_or(|boundary| {
                boundary.reason()
                    != fe2o3_kernel_ir::SemanticDebugBoundaryReasonV1::UnsupportedLayer
            }) {
                return Err(FinalizedSemanticDebugMapErrorV1::InvalidSemanticCorrespondence);
            }
            kir_ids.push(kir_id);
        }
        // This validates the frozen V1 carrier shape, not semantic duplication. Exact production
        // classification is emitted separately by Transformation Map V2.
        let transformation = if kir_ids.len() == 1 {
            SemanticDebugTransformationV1::Preserved
        } else {
            SemanticDebugTransformationV1::Duplicated
        };
        require_mapping(
            map,
            graph_indices,
            mir_id,
            &kir_ids,
            SemanticDebugLayerV1::Mir,
            SemanticDebugLayerV1::Kir,
            transformation,
        )?;
        if retain_instances {
            for instance in instances {
                instance_statements.push(ProductionSemanticDebugStatementInstanceInputV1 {
                    correspondence_owner: instance.correspondence_owner,
                    semantic_function: instance.semantic_function,
                    semantic_block: instance.semantic_block,
                    statement: instance.statement,
                    source_node: source_id,
                    mir_node: mir_id,
                    kir_nodes: bounded_copy_identities(&kir_ids)?,
                });
            }
        }
    }
    if !retain_instances {
        return Ok(None);
    }
    let mut instance_functions = Vec::new();
    instance_functions
        .try_reserve_exact(function_layouts.len())
        .map_err(|_| FinalizedSemanticDebugMapErrorV1::AllocationFailure)?;
    for layout in function_layouts {
        instance_functions.push(ProductionSemanticDebugFunctionInstanceInputV1 {
            correspondence_owner: layout.correspondence_owner,
            semantic_function: layout.semantic_function,
            kernel_ir_function_ordinal: u32::try_from(layout.function_ordinal)
                .map_err(|_| FinalizedSemanticDebugMapErrorV1::InvalidSemanticCorrespondence)?,
            role: layout.instance_role,
        });
    }
    Ok(Some(ReplayedProductionSemanticDebugInstancesV1 {
        functions: instance_functions,
        statements: instance_statements,
    }))
}

fn exact_block_ordinals_v1(
    body: &fe2o3_kernel_ir::FunctionBody,
) -> Result<Vec<(u32, usize)>, FinalizedSemanticDebugMapErrorV1> {
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
    Ok(block_ordinals)
}

fn require_mapping(
    map: &SemanticDebugMapDocumentV1,
    graph_indices: &SemanticDebugGraphIndicesV1,
    input: [u8; 32],
    outputs: &[[u8; 32]],
    input_layer: SemanticDebugLayerV1,
    output_layer: SemanticDebugLayerV1,
    transformation: SemanticDebugTransformationV1,
) -> Result<(), FinalizedSemanticDebugMapErrorV1> {
    let mapping = graph_indices
        .mapping_from(map, input)
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
    graph_indices: &SemanticDebugGraphIndicesV1,
    input: [u8; 32],
    input_layer: SemanticDebugLayerV1,
    output_layer: SemanticDebugLayerV1,
    transformation: SemanticDebugTransformationV1,
    reason: SemanticDebugUnavailableReasonV1,
) -> Result<(), FinalizedSemanticDebugMapErrorV1> {
    let mapping = graph_indices
        .mapping_from(map, input)
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
    let graph_indices = SemanticDebugGraphIndicesV1::try_new(&document)?;
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
        graph_indices,
        transformation_map_v2: None,
        instance_custody_v1: ProductionSemanticDebugInstanceCustodyAvailabilityV1::Unavailable(
            ProductionSemanticDebugInstanceCustodyUnavailableV1::CorrespondenceUnavailable,
        ),
    })
}

fn build_production_transformation_map_v2(
    admitted: &AdmittedFinalizedSemanticDebugMapV1,
    correspondence_bytes: &[u8],
) -> Result<SemanticDebugTransformationMapDocumentV2, FinalizedSemanticDebugMapErrorV1> {
    let correspondence_kind = if correspondence_bytes.get(..8) == Some(b"F2MRCOR2") {
        SemanticDebugTransformationEvidenceKindV2::MultiRootMirKirCorrespondenceRosterV2
    } else if correspondence_bytes.get(..8) == Some(b"F2M2K5\0\0") {
        SemanticDebugTransformationEvidenceKindV2::MirKirCorrespondenceV5
    } else {
        SemanticDebugTransformationEvidenceKindV2::MirKirCorrespondenceV4
    };
    let evidence = SemanticDebugTransformationEvidenceV2::from_exact_bytes(
        correspondence_kind,
        correspondence_bytes,
    )
    .map_err(FinalizedSemanticDebugMapErrorV1::TransformationMapV2)?;
    let binding =
        SemanticDebugTransformationMapBindingV2::from_exact_map(admitted.canonical_bytes())
            .map_err(FinalizedSemanticDebugMapErrorV1::TransformationMapV2)?;
    let capabilities = SemanticDebugTransformationClassV2::all_v2()
        .into_iter()
        .map(|class| {
            let availability = if class == SemanticDebugTransformationClassV2::Eliminated {
                SemanticDebugTransformationAvailabilityV2::AuthenticatedProducer
            } else {
                SemanticDebugTransformationAvailabilityV2::UnavailableNoAuthenticatedProducer
            };
            SemanticDebugTransformationCapabilityV2::new(class, availability)
        })
        .collect();
    let mut relations = Vec::new();
    relations
        .try_reserve_exact(admitted.document().mappings().len())
        .map_err(|_| FinalizedSemanticDebugMapErrorV1::AllocationFailure)?;
    for mapping in admitted.document().mappings() {
        if !matches!(
            (mapping.input_layer(), mapping.output_layer()),
            (SemanticDebugLayerV1::Source, SemanticDebugLayerV1::Mir)
                | (SemanticDebugLayerV1::Mir, SemanticDebugLayerV1::Kir)
        ) {
            continue;
        }
        let classification = match (mapping.input_layer(), mapping.output().nodes().len()) {
            (SemanticDebugLayerV1::Source, 1) => {
                SemanticDebugTransformationClassificationV2::Preserved
            }
            (SemanticDebugLayerV1::Mir, 0) => {
                SemanticDebugTransformationClassificationV2::Observed {
                    class: SemanticDebugTransformationClassV2::Eliminated,
                }
            }
            (SemanticDebugLayerV1::Mir, 1) => {
                SemanticDebugTransformationClassificationV2::Preserved
            }
            (SemanticDebugLayerV1::Mir, _) => {
                SemanticDebugTransformationClassificationV2::Unavailable {
                    reason: SemanticDebugTransformationUnavailableReasonV2::ProducerDidNotClassify,
                }
            }
            _ => return Err(FinalizedSemanticDebugMapErrorV1::InvalidSemanticCorrespondence),
        };
        let relation = SemanticDebugTransformationRelationV2::new(
            mapping.input_layer(),
            mapping.output_layer(),
            evidence.identity(),
            bounded_copy_identities(mapping.inputs())?,
            bounded_copy_identities(mapping.output().nodes())?,
            classification,
        )
        .map_err(FinalizedSemanticDebugMapErrorV1::TransformationMapV2)?;
        if mapping.output().nodes().len() > 1
            && relation.cardinality() != SemanticDebugRelationCardinalityV2::OneToMany
        {
            return Err(FinalizedSemanticDebugMapErrorV1::InvalidSemanticCorrespondence);
        }
        relations.push(relation);
    }
    SemanticDebugTransformationMapDocumentV2::new(
        binding,
        capabilities,
        vec![evidence],
        relations,
        admitted.document(),
    )
    .map_err(FinalizedSemanticDebugMapErrorV1::TransformationMapV2)
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
    InvalidBoundCorrespondenceV5,
    InvalidBoundMultiRootCorrespondenceV2,
    InvalidBoundCanonicalKirV8,
    InvalidBoundCanonicalKirV7,
    CanonicalKirProjectionMismatch,
    CorrespondenceIdentityMismatch,
    InvalidSemanticCorrespondence,
    TransformationMapV2(SemanticDebugTransformationMapErrorV2),
    InstanceCustodyV1(ProductionSemanticDebugInstanceCustodyErrorV1),
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
            Self::TransformationMapV2(error) => Some(error),
            Self::InstanceCustodyV1(error) => Some(error),
            Self::ProductionFragment(error) => Some(error),
            Self::ProductionAssociation
            | Self::ProductionAssociationMismatch
            | Self::InvalidKirToLlvmReplay
            | Self::KirToLlvmReplayTargetMismatch
            | Self::InvalidLlvmToHsacoCustody
            | Self::InvalidBoundSourceMap
            | Self::InvalidBoundSemanticMir
            | Self::InvalidBoundCorrespondenceV4
            | Self::InvalidBoundCorrespondenceV5
            | Self::InvalidBoundMultiRootCorrespondenceV2
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

    fn ordinal_id(ordinal: usize) -> [u8; 32] {
        let mut identity = [0_u8; 32];
        identity[..8].copy_from_slice(&(ordinal as u64).to_le_bytes());
        identity[31] = 1;
        identity
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
        assert!(matches!(
            admitted.instance_custody_v1(),
            ProductionSemanticDebugInstanceCustodyAvailabilityV1::Unavailable(
                ProductionSemanticDebugInstanceCustodyUnavailableV1::CorrespondenceUnavailable
            )
        ));

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
    fn graph_indices_are_bounded_unique_and_logarithmically_queryable_at_the_wire_limit() {
        let mut owners = Vec::new();
        owners
            .try_reserve_exact(MAX_SEMANTIC_DEBUG_MAPPING_REFERENCES_V1)
            .unwrap();
        owners.extend(
            (0..MAX_SEMANTIC_DEBUG_MAPPING_REFERENCES_V1)
                .rev()
                .map(|ordinal| (ordinal_id(ordinal), ordinal)),
        );
        normalize_unique_owner_index(&mut owners).unwrap();
        for ordinal in 0..MAX_SEMANTIC_DEBUG_MAPPING_REFERENCES_V1 {
            assert_eq!(owner_lookup(&owners, &ordinal_id(ordinal)), Some(ordinal));
        }

        assert!(matches!(
            validate_graph_index_cardinalities(MAX_SEMANTIC_DEBUG_MAPPING_REFERENCES_V1, 1, 0,),
            Err(FinalizedSemanticDebugMapErrorV1::InvalidSemanticCorrespondence)
        ));
        assert!(matches!(
            validate_graph_index_cardinalities(0, 0, MAX_SEMANTIC_DEBUG_BOUNDARIES_V1 + 1),
            Err(FinalizedSemanticDebugMapErrorV1::InvalidSemanticCorrespondence)
        ));
    }

    #[test]
    fn graph_owner_index_rejects_duplicate_and_missing_source_mapping() {
        let source = id(9);
        let mut duplicate_source_owners = vec![(source, 0), (source, 1)];
        assert!(matches!(
            normalize_unique_owner_index(&mut duplicate_source_owners),
            Err(FinalizedSemanticDebugMapErrorV1::InvalidSemanticCorrespondence)
        ));

        let mut exact_source_owners = vec![(source, 0)];
        normalize_unique_owner_index(&mut exact_source_owners).unwrap();
        assert_eq!(owner_lookup(&exact_source_owners, &source), Some(0));
        assert_eq!(owner_lookup(&exact_source_owners, &id(0xfe)), None);
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
        assert!(matches!(
            admitted.instance_custody_v1(),
            ProductionSemanticDebugInstanceCustodyAvailabilityV1::Unavailable(
                ProductionSemanticDebugInstanceCustodyUnavailableV1::CorrespondenceUnavailable
            )
        ));

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
        let fixture_indices = SemanticDebugGraphIndicesV1::try_new(&fixture.map).unwrap();
        validate_exact_production_correspondence(
            &fixture.map,
            &fixture_indices,
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
        let resealed_indices = SemanticDebugGraphIndicesV1::try_new(&resealed).unwrap();
        assert!(matches!(
            validate_exact_production_correspondence(
                &resealed,
                &resealed_indices,
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

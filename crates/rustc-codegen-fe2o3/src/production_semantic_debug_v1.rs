//! Compiler-owned production semantic debug fragment construction.

use std::collections::BTreeMap;

use fe2o3_compiler_lineage::{
    MultiRootCanonicalKirVersionV2, MultiRootCorrespondenceFunctionRoleV2,
    MultiRootCorrespondencePayloadV2, MultiRootCorrespondenceSyntheticRuleV2,
    MultiRootProofRosterKindV2, MultiRootProofRosterTranscriptV2,
};
use fe2o3_kernel_ir::{
    DebugSourceMapDocumentV2, DebugSourceMapErrorV2, DebugSourceMapKirSiteV1, DebugSourceMapSpanV1,
    MAX_PRODUCTION_SEMANTIC_DEBUG_CARRIER_BYTES_V1, MAX_SEMANTIC_DEBUG_BOUNDARIES_V1,
    MAX_SEMANTIC_DEBUG_MAPPINGS_V1, MAX_SEMANTIC_DEBUG_NODES_V1,
    PRODUCTION_PREFINALIZED_ARTIFACT_STATUS_V1, ProductionScheduleStatusV1,
    ProductionSemanticDebugAvailabilityV1, ProductionSemanticDebugFragmentErrorV1,
    ProductionSemanticDebugFragmentV1, ProductionSemanticDebugProducerGapV1,
    SemanticDebugBoundaryDirectionV1, SemanticDebugBoundaryReasonV1, SemanticDebugBoundaryV1,
    SemanticDebugContentIdentityV1, SemanticDebugLayerV1, SemanticDebugLocationV1,
    SemanticDebugMapBindingV1, SemanticDebugMapDocumentV1, SemanticDebugMapErrorV1,
    SemanticDebugMappingOutputV1, SemanticDebugMappingV1, SemanticDebugNodeV1,
    SemanticDebugTransformationV1, SemanticDebugUnavailableReasonV1, VerifiedCanonicalKernelIrV7,
};
use fe2o3_lower_mir_kernel::{
    InertCanonicalMirToKirCorrespondenceEvidenceV5, ProductionCanonicalKernelIrVersionV1,
    ProductionSemanticKirOwnerV1, SemanticKirFunctionRoleV1, SemanticKirSyntheticOperationRuleV1,
};
use fe2o3_mir_model::InertCanonicalSemanticU32InductionEvidenceV1;
use sha2::{Digest, Sha256};

use crate::production_pipeline::ProductionPipelineError;

const NODE_DOMAIN_V1: &[u8] = b"FE2O3/PRODUCTION-SEMANTIC-DEBUG-NODE/V1\0";
const MAPPING_DOMAIN_V1: &[u8] = b"FE2O3/PRODUCTION-SEMANTIC-DEBUG-MAPPING/V1\0";
const CONTEXT_DOMAIN_V1: &[u8] = b"FE2O3/PRODUCTION-SEMANTIC-DEBUG-CONTEXT/V1\0";

pub(crate) enum PreparedProductionSemanticDebugV1 {
    Available(PreparedProductionSemanticDebugFragmentV1),
    Unavailable(ProductionSemanticDebugProducerGapV1),
}

pub(crate) enum ProductionSemanticDebugInputsV1 {
    Available {
        source_map: Box<DebugSourceMapDocumentV2>,
        canonical_kir_v7: VerifiedCanonicalKernelIrV7,
    },
    Unavailable(ProductionSemanticDebugProducerGapV1),
}

impl ProductionSemanticDebugInputsV1 {
    pub(crate) const fn unavailable(gap: ProductionSemanticDebugProducerGapV1) -> Self {
        Self::Unavailable(gap)
    }
}

pub(crate) struct PreparedProductionSemanticDebugFragmentV1 {
    source_map_v2: Vec<u8>,
    canonical_kir_v7: Vec<u8>,
    nodes: Vec<SemanticDebugNodeV1>,
    mappings: Vec<SemanticDebugMappingV1>,
    boundaries: Vec<SemanticDebugBoundaryV1>,
}

impl PreparedProductionSemanticDebugV1 {
    pub(crate) fn finish(
        self,
        semantic_mir: &[u8],
        llvm_module: &[u8],
    ) -> ProductionSemanticDebugAvailabilityV1 {
        match self {
            Self::Unavailable(gap) => ProductionSemanticDebugAvailabilityV1::Unavailable(gap),
            Self::Available(prepared) => prepared.finish(semantic_mir, llvm_module),
        }
    }
}

impl PreparedProductionSemanticDebugFragmentV1 {
    fn finish(
        self,
        semantic_mir: &[u8],
        llvm_module: &[u8],
    ) -> ProductionSemanticDebugAvailabilityV1 {
        let schedule = ProductionScheduleStatusV1::NoProductionScheduleStage.canonical_bytes();
        let binding =
            SemanticDebugContentIdentityV1::calculate(&self.source_map_v2).and_then(|source_map| {
                SemanticDebugMapBindingV1::new(
                    source_map,
                    SemanticDebugContentIdentityV1::calculate(semantic_mir)?,
                    SemanticDebugContentIdentityV1::calculate(&self.canonical_kir_v7)?,
                    SemanticDebugContentIdentityV1::calculate(&schedule)?,
                    SemanticDebugContentIdentityV1::calculate(llvm_module)?,
                    SemanticDebugContentIdentityV1::calculate(
                        PRODUCTION_PREFINALIZED_ARTIFACT_STATUS_V1,
                    )?,
                )
            });
        let Ok(binding) = binding else {
            return unavailable(
                ProductionSemanticDebugProducerGapV1::SemanticMapConstructionUnavailable,
            );
        };
        let map = match SemanticDebugMapDocumentV1::new_partial(
            binding,
            self.nodes,
            self.mappings,
            self.boundaries,
        ) {
            Ok(map) => map,
            Err(error) if semantic_map_resource_error(error) => return resource_unavailable(),
            Err(_) => {
                return unavailable(
                    ProductionSemanticDebugProducerGapV1::SemanticMapConstructionUnavailable,
                );
            }
        };
        let map = match map.to_canonical_json_bytes() {
            Ok(map) => map,
            Err(error) if semantic_map_resource_error(error) => return resource_unavailable(),
            Err(_) => {
                return unavailable(
                    ProductionSemanticDebugProducerGapV1::SemanticMapEncodingUnavailable,
                );
            }
        };
        let carried_bytes = self
            .source_map_v2
            .len()
            .checked_add(self.canonical_kir_v7.len())
            .and_then(|length| length.checked_add(schedule.len()))
            .and_then(|length| length.checked_add(map.len()));
        if carried_bytes
            .is_none_or(|length| length >= MAX_PRODUCTION_SEMANTIC_DEBUG_CARRIER_BYTES_V1)
        {
            return resource_unavailable();
        }
        let fragment = match ProductionSemanticDebugFragmentV1::new(
            self.source_map_v2,
            self.canonical_kir_v7,
            schedule.to_vec(),
            map,
        ) {
            Ok(fragment) => fragment,
            Err(error) if semantic_fragment_resource_error(error) => {
                return resource_unavailable();
            }
            Err(_) => {
                return unavailable(
                    ProductionSemanticDebugProducerGapV1::FragmentConstructionUnavailable,
                );
            }
        };
        ProductionSemanticDebugAvailabilityV1::Available(fragment)
    }
}

const fn resource_unavailable() -> ProductionSemanticDebugAvailabilityV1 {
    unavailable(ProductionSemanticDebugProducerGapV1::ResourceLimit)
}

const fn unavailable(
    gap: ProductionSemanticDebugProducerGapV1,
) -> ProductionSemanticDebugAvailabilityV1 {
    ProductionSemanticDebugAvailabilityV1::Unavailable(gap)
}

const fn semantic_map_resource_error(error: SemanticDebugMapErrorV1) -> bool {
    matches!(
        error,
        SemanticDebugMapErrorV1::InvalidLength
            | SemanticDebugMapErrorV1::Encoding
            | SemanticDebugMapErrorV1::ResourceLimit
            | SemanticDebugMapErrorV1::AllocationFailure
    )
}

const fn semantic_fragment_resource_error(error: ProductionSemanticDebugFragmentErrorV1) -> bool {
    matches!(
        error,
        ProductionSemanticDebugFragmentErrorV1::ResourceLimit
            | ProductionSemanticDebugFragmentErrorV1::AllocationFailure
    )
}

#[allow(clippy::result_large_err)]
pub(crate) fn prepare_production_semantic_debug_v1(
    lowered: &ProductionSemanticKirOwnerV1,
    correspondence: &InertCanonicalMirToKirCorrespondenceEvidenceV5,
    source_map: DebugSourceMapDocumentV2,
    canonical_kir: &VerifiedCanonicalKernelIrV7,
) -> Result<PreparedProductionSemanticDebugV1, ProductionPipelineError> {
    correspondence
        .validate_against_module(lowered.module())
        .map_err(|_| correspondence_error("V5 correspondence differs from the live KIR roster"))?;
    let nested = correspondence.nested_v4();
    if nested.semantic_sha256() != lowered.semantic().semantic().semantic_sha256().as_bytes()
        || nested.canonical_kernel_ir_identity() != lowered.canonical_kernel_ir_identity()
    {
        return Err(correspondence_error(
            "V5 correspondence names different semantic MIR or production KIR",
        ));
    }
    prepare_production_semantic_debug_from_live_v1(lowered, source_map, canonical_kir)
}

#[allow(clippy::result_large_err)]
pub(crate) fn prepare_production_semantic_debug_multi_root_v1(
    lowered: &ProductionSemanticKirOwnerV1,
    correspondence_roster: &[u8],
    source_map: DebugSourceMapDocumentV2,
    canonical_kir: &VerifiedCanonicalKernelIrV7,
) -> Result<PreparedProductionSemanticDebugV1, ProductionPipelineError> {
    validate_multi_root_correspondence_v1(lowered, correspondence_roster)?;
    prepare_production_semantic_debug_from_live_v1(lowered, source_map, canonical_kir)
}

#[allow(clippy::result_large_err)]
fn prepare_production_semantic_debug_from_live_v1(
    lowered: &ProductionSemanticKirOwnerV1,
    source_map: DebugSourceMapDocumentV2,
    canonical_kir: &VerifiedCanonicalKernelIrV7,
) -> Result<PreparedProductionSemanticDebugV1, ProductionPipelineError> {
    macro_rules! semantic_map_value {
        ($result:expr) => {
            match $result {
                Ok(value) => value,
                Err(error) if semantic_map_resource_error(error) => {
                    return Ok(PreparedProductionSemanticDebugV1::Unavailable(
                        ProductionSemanticDebugProducerGapV1::ResourceLimit,
                    ));
                }
                Err(error) => return Err(ProductionPipelineError::SemanticDebugMap(error)),
            }
        };
    }

    let live = lowered.correspondence();
    if live.lowered_functions().is_empty() {
        return Ok(PreparedProductionSemanticDebugV1::Unavailable(
            ProductionSemanticDebugProducerGapV1::NoStatementCorrespondence,
        ));
    }
    let defined_function_count = lowered
        .module()
        .functions
        .iter()
        .filter(|function| function.body.is_some())
        .count();
    if defined_function_count != live.lowered_functions().len() {
        return Err(ProductionPipelineError::SimulationDebugMapCorrespondence(
            "live correspondence does not cover every defined KIR function",
        ));
    }
    let mut function_layouts = Vec::new();
    if function_layouts
        .try_reserve_exact(live.lowered_functions().len())
        .is_err()
    {
        return Ok(PreparedProductionSemanticDebugV1::Unavailable(
            ProductionSemanticDebugProducerGapV1::ResourceLimit,
        ));
    }
    let mut owners = BTreeMap::<u32, usize>::new();
    for record in live.lowered_functions() {
        let mut matches = lowered
            .module()
            .functions
            .iter()
            .enumerate()
            .filter(|(_, function)| &function.id == record.kernel_ir_function());
        let Some((ordinal, function)) = matches.next() else {
            return Err(correspondence_error(
                "correspondence names an unknown KIR function",
            ));
        };
        if matches.next().is_some() {
            return Err(correspondence_error(
                "correspondence KIR symbol is ambiguous",
            ));
        }
        let expected_role = match record.role() {
            SemanticKirFunctionRoleV1::KernelEntry => fe2o3_kernel_ir::FunctionRole::KernelEntry,
            SemanticKirFunctionRoleV1::InternalHelper => {
                fe2o3_kernel_ir::FunctionRole::InternalHelper
            }
        };
        let body = function.body.as_ref().ok_or(
            ProductionPipelineError::SimulationDebugMapCorrespondence(
                "V5 correspondence names a KIR declaration",
            ),
        )?;
        if function.role != expected_role {
            return Err(ProductionPipelineError::SimulationDebugMapCorrespondence(
                "correspondence KIR function role differs",
            ));
        }
        if record.role() == SemanticKirFunctionRoleV1::KernelEntry {
            *owners
                .entry(record.correspondence_owner().index())
                .or_default() += 1;
        } else {
            owners
                .entry(record.correspondence_owner().index())
                .or_default();
        }
        let block_ordinals = body
            .blocks
            .iter()
            .enumerate()
            .map(|(block_ordinal, block)| (block.id, block_ordinal))
            .collect::<BTreeMap<_, _>>();
        if block_ordinals.len() != body.blocks.len() {
            return Err(ProductionPipelineError::SimulationDebugMapCorrespondence(
                "KIR block identities are not unique within their function",
            ));
        }
        function_layouts.push((
            record.semantic_function().index(),
            u64::try_from(ordinal)
                .map_err(|_| correspondence_error("KIR function ordinal overflow"))?,
            body,
            block_ordinals,
        ));
    }
    if owners.values().any(|entry_count| *entry_count != 1) {
        return Err(correspondence_error(
            "each correspondence root must own exactly one KIR entry",
        ));
    }
    function_layouts.sort_unstable_by_key(|layout| layout.0);
    if function_layouts
        .windows(2)
        .any(|pair| pair[0].0 >= pair[1].0)
    {
        return Err(ProductionPipelineError::SimulationDebugMapCorrespondence(
            "semantic functions shared across correspondence roots are ambiguous",
        ));
    }

    let source_map_v2 = match source_map.to_canonical_json_bytes() {
        Ok(bytes) => bytes,
        Err(error) if source_map_resource_error(error) => {
            return Ok(PreparedProductionSemanticDebugV1::Unavailable(
                ProductionSemanticDebugProducerGapV1::ResourceLimit,
            ));
        }
        Err(error) => return Err(ProductionPipelineError::SimulationDebugMapV2(error)),
    };
    if source_map_v2.len() >= MAX_PRODUCTION_SEMANTIC_DEBUG_CARRIER_BYTES_V1
        || canonical_kir.canonical_bytes().len() >= MAX_PRODUCTION_SEMANTIC_DEBUG_CARRIER_BYTES_V1
    {
        return Ok(PreparedProductionSemanticDebugV1::Unavailable(
            ProductionSemanticDebugProducerGapV1::ResourceLimit,
        ));
    }
    let context = debug_identity_context(&source_map_v2, canonical_kir.canonical_bytes());

    let counts = live.statement_operation_spans().iter().try_fold(
        (0_usize, 0_usize),
        |(statements, operations), span| {
            Some((
                statements.checked_add(1)?,
                operations.checked_add(span.operation_count() as usize)?,
            ))
        },
    );
    let Some((statement_count, operation_count)) = counts else {
        return Ok(PreparedProductionSemanticDebugV1::Unavailable(
            ProductionSemanticDebugProducerGapV1::ResourceLimit,
        ));
    };
    let node_count = statement_count
        .checked_mul(2)
        .and_then(|count| count.checked_add(operation_count));
    let mapping_count = statement_count.checked_mul(2);
    if node_count.is_none_or(|count| count > MAX_SEMANTIC_DEBUG_NODES_V1)
        || mapping_count.is_none_or(|count| count > MAX_SEMANTIC_DEBUG_MAPPINGS_V1)
        || operation_count > MAX_SEMANTIC_DEBUG_BOUNDARIES_V1
    {
        return Ok(PreparedProductionSemanticDebugV1::Unavailable(
            ProductionSemanticDebugProducerGapV1::ResourceLimit,
        ));
    }
    let mut nodes = Vec::new();
    let mut mappings = Vec::new();
    let mut boundaries = Vec::new();
    if nodes
        .try_reserve_exact(node_count.expect("node count checked"))
        .is_err()
        || mappings
            .try_reserve_exact(mapping_count.expect("mapping count checked"))
            .is_err()
        || boundaries.try_reserve_exact(operation_count).is_err()
    {
        return Ok(PreparedProductionSemanticDebugV1::Unavailable(
            ProductionSemanticDebugProducerGapV1::ResourceLimit,
        ));
    }
    for span in live.statement_operation_spans() {
        let (_, function_ordinal, body, block_ordinals) = function_layouts
            .binary_search_by_key(&span.semantic_function().index(), |layout| layout.0)
            .ok()
            .and_then(|index| function_layouts.get(index))
            .ok_or(ProductionPipelineError::SimulationDebugMapCorrespondence(
                "statement debug correspondence has no exact KIR function owner",
            ))?;
        let source = lowered
            .semantic()
            .resolve_statement(
                fe2o3_mir_model::semantic_mir_v1::SemanticFunctionIdV1::from_index(
                    span.semantic_function().index(),
                ),
                fe2o3_mir_model::semantic_mir_v1::SemanticBlockIdV1::from_index(
                    span.semantic_block().index(),
                ),
                span.statement_ordinal(),
            )
            .ok_or(ProductionPipelineError::SimulationDebugMapCorrespondence(
                "statement debug correspondence does not resolve in semantic MIR",
            ))?
            .source();
        let origin =
            source
                .call_site()
                .ok_or(ProductionPipelineError::SimulationDebugMapCorrespondence(
                    "statement debug correspondence has no resolved call site",
                ))?;
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
        .map_err(ProductionPipelineError::SimulationDebugMap)?;
        let source_id = node_identity(
            context,
            1,
            u64::from(span.semantic_function().index()),
            u64::from(span.semantic_block().index()),
            u64::from(span.statement_ordinal()),
            0,
        );
        let mir_id = node_identity(
            context,
            2,
            u64::from(span.semantic_function().index()),
            u64::from(span.semantic_block().index()),
            u64::from(span.statement_ordinal()),
            0,
        );
        nodes.push(semantic_map_value!(SemanticDebugNodeV1::new(
            source_id,
            SemanticDebugLocationV1::Source { span: source_span },
        )));
        nodes.push(semantic_map_value!(SemanticDebugNodeV1::new(
            mir_id,
            SemanticDebugLocationV1::Mir {
                body_ordinal: u64::from(span.semantic_function().index()),
                block_ordinal: u64::from(span.semantic_block().index()),
                statement_ordinal: u64::from(span.statement_ordinal()),
            },
        )));
        mappings.push(semantic_map_value!(SemanticDebugMappingV1::new(
            mapping_identity(context, 1, source_id, 0),
            SemanticDebugLayerV1::Source,
            SemanticDebugLayerV1::Mir,
            SemanticDebugTransformationV1::Preserved,
            vec![source_id],
            SemanticDebugMappingOutputV1::available(vec![mir_id]),
        )));

        if span.operation_count() == 0 {
            if source_map.eliminated().binary_search(&source_span).is_err() {
                return Err(ProductionPipelineError::SimulationDebugMapCorrespondence(
                    "eliminated semantic statement span is absent from exact Source Map V2",
                ));
            }
            mappings.push(semantic_map_value!(SemanticDebugMappingV1::new(
                mapping_identity(context, 2, mir_id, 0),
                SemanticDebugLayerV1::Mir,
                SemanticDebugLayerV1::Kir,
                SemanticDebugTransformationV1::Eliminated,
                vec![mir_id],
                SemanticDebugMappingOutputV1::unavailable(
                    SemanticDebugUnavailableReasonV1::Eliminated,
                ),
            )));
            continue;
        }

        let block_index = *block_ordinals.get(&span.kernel_ir_block()).ok_or(
            ProductionPipelineError::SimulationDebugMapCorrespondence(
                "statement debug correspondence names an unknown KIR block",
            ),
        )?;
        let block_ordinal = u64::try_from(block_index).map_err(|_| {
            ProductionPipelineError::SimulationDebugMapCorrespondence(
                "KIR block ordinal exceeds the semantic debug wire",
            )
        })?;

        let mut kir_ids = Vec::new();
        if kir_ids
            .try_reserve_exact(span.operation_count() as usize)
            .is_err()
        {
            return Ok(PreparedProductionSemanticDebugV1::Unavailable(
                ProductionSemanticDebugProducerGapV1::ResourceLimit,
            ));
        }
        let operation_end = span
            .first_operation_ordinal()
            .checked_add(span.operation_count())
            .ok_or(ProductionPipelineError::SimulationDebugMapCorrespondence(
                "statement KIR operation range overflows",
            ))?;
        if usize::try_from(operation_end).map_or(true, |end| {
            body.blocks
                .get(block_index)
                .is_none_or(|block| end > block.operations.len())
        }) {
            return Err(ProductionPipelineError::SimulationDebugMapCorrespondence(
                "statement KIR operation range exceeds its exact function body",
            ));
        }
        for operation in span.first_operation_ordinal()..operation_end {
            let site = DebugSourceMapKirSiteV1::operation(
                *function_ordinal,
                block_ordinal,
                u64::from(operation),
            );
            let source_site = source_map
                .sites()
                .binary_search_by_key(&site, |entry| entry.site())
                .ok()
                .map(|index| &source_map.sites()[index])
                .ok_or(ProductionPipelineError::SimulationDebugMapCorrespondence(
                    "semantic debug operation is absent from exact Source Map V2",
                ))?;
            if source_site.spans() != [source_span] {
                return Err(ProductionPipelineError::SimulationDebugMapCorrespondence(
                    "semantic statement span differs from exact Source Map V2 operation span",
                ));
            }
            let kir_id = node_identity(
                context,
                3,
                *function_ordinal,
                block_ordinal,
                u64::from(operation),
                0,
            );
            nodes.push(semantic_map_value!(SemanticDebugNodeV1::new(
                kir_id,
                SemanticDebugLocationV1::Kir {
                    function_ordinal: *function_ordinal,
                    block_ordinal,
                    operation_ordinal: u64::from(operation),
                },
            )));
            boundaries.push(semantic_map_value!(SemanticDebugBoundaryV1::new(
                kir_id,
                SemanticDebugBoundaryDirectionV1::SuccessorUnavailable,
                SemanticDebugBoundaryReasonV1::UnsupportedLayer,
            )));
            kir_ids.push(kir_id);
        }
        let transformation = if kir_ids.len() == 1 {
            SemanticDebugTransformationV1::Preserved
        } else {
            SemanticDebugTransformationV1::Duplicated
        };
        mappings.push(semantic_map_value!(SemanticDebugMappingV1::new(
            mapping_identity(context, 2, mir_id, span.operation_count()),
            SemanticDebugLayerV1::Mir,
            SemanticDebugLayerV1::Kir,
            transformation,
            vec![mir_id],
            SemanticDebugMappingOutputV1::available(kir_ids),
        )));
    }
    if boundaries.is_empty() {
        return Ok(PreparedProductionSemanticDebugV1::Unavailable(
            ProductionSemanticDebugProducerGapV1::NoStatementCorrespondence,
        ));
    }
    let mut canonical_kir_v7 = Vec::new();
    if canonical_kir_v7
        .try_reserve_exact(canonical_kir.canonical_bytes().len())
        .is_err()
    {
        return Ok(PreparedProductionSemanticDebugV1::Unavailable(
            ProductionSemanticDebugProducerGapV1::ResourceLimit,
        ));
    }
    canonical_kir_v7.extend_from_slice(canonical_kir.canonical_bytes());
    Ok(PreparedProductionSemanticDebugV1::Available(
        PreparedProductionSemanticDebugFragmentV1 {
            source_map_v2,
            canonical_kir_v7,
            nodes,
            mappings,
            boundaries,
        },
    ))
}

#[allow(clippy::result_large_err)]
fn validate_multi_root_correspondence_v1(
    lowered: &ProductionSemanticKirOwnerV1,
    correspondence_roster: &[u8],
) -> Result<(), ProductionPipelineError> {
    let roster = MultiRootProofRosterTranscriptV2::decode(correspondence_roster)
        .map_err(|_| correspondence_error("invalid multi-root correspondence roster"))?;
    let kir = lowered.canonical_kernel_ir_identity();
    if roster.kind() != MultiRootProofRosterKindV2::Correspondence
        || roster.root_count() < 2
        || roster.semantic_mir_sha256()
            != *lowered.semantic().semantic().semantic_sha256().as_bytes()
        || roster.neutral_kir().version() != MultiRootCanonicalKirVersionV2::V8
        || kir.version() != ProductionCanonicalKernelIrVersionV1::V8
        || roster.neutral_kir().digest() != *kir.digest()
        || roster.neutral_kir().canonical_length() != kir.canonical_length()
    {
        return Err(correspondence_error(
            "multi-root correspondence roster names different semantic MIR or production KIR",
        ));
    }

    let live = lowered.correspondence();
    let mut owners = Vec::new();
    let mut semantic_functions = Vec::new();
    owners
        .try_reserve_exact(roster.root_count())
        .map_err(|_| correspondence_error("multi-root correspondence resource limit"))?;
    semantic_functions
        .try_reserve_exact(live.lowered_functions().len())
        .map_err(|_| correspondence_error("multi-root correspondence resource limit"))?;
    for ordinal in 0..roster.root_count() {
        let root = roster
            .root(ordinal)
            .ok_or_else(|| correspondence_error("missing multi-root correspondence root"))?;
        let payload = MultiRootCorrespondencePayloadV2::decode(root.payload())
            .map_err(|_| correspondence_error("invalid multi-root correspondence root payload"))?;
        let expected_ordinal = u32::try_from(ordinal)
            .map_err(|_| correspondence_error("multi-root correspondence ordinal overflow"))?;
        if payload.root_ordinal() != expected_ordinal
            || payload.correspondence_owner() != root.semantic_root()
        {
            return Err(correspondence_error(
                "multi-root correspondence root header was reordered or substituted",
            ));
        }
        let induction =
            InertCanonicalSemanticU32InductionEvidenceV1::decode(payload.induction())
                .map_err(|_| correspondence_error("invalid multi-root induction evidence"))?;
        if induction.semantic_mir_sha256()
            != lowered.semantic().semantic().semantic_sha256().as_bytes()
            || induction.grants_authority()
        {
            return Err(correspondence_error(
                "multi-root induction evidence changed semantic identity or authority",
            ));
        }
        let function = lowered
            .semantic()
            .semantic()
            .functions()
            .get(root.semantic_root() as usize)
            .ok_or_else(|| {
                correspondence_error("multi-root roster names an unknown semantic root")
            })?;
        if function.identity().as_bytes() != &root.semantic_root_identity() {
            return Err(correspondence_error(
                "multi-root roster substituted a semantic root identity",
            ));
        }
        if !owners.is_empty()
            && owners
                .last()
                .is_some_and(|owner| *owner >= root.semantic_root())
        {
            return Err(correspondence_error(
                "multi-root correspondence roots are duplicated or reordered",
            ));
        }
        owners.push(root.semantic_root());

        let mut payload_functions = payload
            .functions()
            .iter()
            .map(|record| {
                (
                    record.semantic_function(),
                    match record.role() {
                        MultiRootCorrespondenceFunctionRoleV2::KernelEntry => 1_u8,
                        MultiRootCorrespondenceFunctionRoleV2::InternalHelper => 2_u8,
                    },
                    record.kernel_ir_function(),
                )
            })
            .collect::<Vec<_>>();
        let mut live_functions = live
            .lowered_functions()
            .iter()
            .filter(|record| record.correspondence_owner().index() == root.semantic_root())
            .map(|record| {
                (
                    record.semantic_function().index(),
                    match record.role() {
                        SemanticKirFunctionRoleV1::KernelEntry => 1_u8,
                        SemanticKirFunctionRoleV1::InternalHelper => 2_u8,
                    },
                    record.kernel_ir_function().as_str(),
                )
            })
            .collect::<Vec<_>>();
        payload_functions.sort_unstable();
        live_functions.sort_unstable();
        if payload_functions != live_functions
            || payload_functions
                .iter()
                .filter(|(_, role, symbol)| *role == 1 && *symbol == root.kernel_id())
                .count()
                != 1
        {
            return Err(correspondence_error(
                "multi-root function closure differs from live correspondence",
            ));
        }
        for (semantic_function, _, _) in payload_functions {
            if semantic_functions.contains(&semantic_function) {
                return Err(correspondence_error(
                    "a semantic helper is shared across multi-root closures",
                ));
            }
            semantic_functions.push(semantic_function);
        }

        let mut payload_blocks = payload
            .blocks()
            .iter()
            .map(|record| {
                (
                    record.semantic_function(),
                    record.semantic_block(),
                    record.kernel_ir_block(),
                    record.source_statement_count(),
                )
            })
            .collect::<Vec<_>>();
        let mut live_blocks = live
            .blocks()
            .iter()
            .filter(|record| record.correspondence_owner().index() == root.semantic_root())
            .map(|record| {
                (
                    record.semantic_function().index(),
                    record.semantic_block().index(),
                    record.kernel_ir_block().0,
                    record.source_statement_count(),
                )
            })
            .collect::<Vec<_>>();
        payload_blocks.sort_unstable();
        live_blocks.sort_unstable();
        let mut payload_statements = payload.statements().to_vec();
        let mut live_statements = live
            .statement_operation_spans()
            .iter()
            .filter(|record| record.correspondence_owner().index() == root.semantic_root())
            .map(|record| {
                (
                    record.semantic_function().index(),
                    record.semantic_block().index(),
                    record.statement_ordinal(),
                    record.kernel_ir_block().0,
                    record.first_operation_ordinal(),
                    record.operation_count(),
                )
            })
            .collect::<Vec<_>>();
        payload_statements.sort_unstable();
        live_statements.sort_unstable();
        let payload_statement_tuples = payload_statements
            .iter()
            .map(|record| {
                (
                    record.semantic_function(),
                    record.semantic_block(),
                    record.statement(),
                    record.kernel_ir_block(),
                    record.first_operation(),
                    record.operation_count(),
                )
            })
            .collect::<Vec<_>>();
        let mut payload_terminators = payload
            .terminators()
            .iter()
            .map(|record| {
                (
                    record.semantic_function(),
                    record.semantic_block(),
                    record.kernel_ir_block(),
                    record.first_operation(),
                    record.operation_count(),
                )
            })
            .collect::<Vec<_>>();
        let mut live_terminators = live
            .terminator_operation_spans()
            .iter()
            .filter(|record| record.correspondence_owner().index() == root.semantic_root())
            .map(|record| {
                (
                    record.semantic_function().index(),
                    record.semantic_block().index(),
                    record.kernel_ir_block().0,
                    record.first_operation_ordinal(),
                    record.operation_count(),
                )
            })
            .collect::<Vec<_>>();
        payload_terminators.sort_unstable();
        live_terminators.sort_unstable();
        let mut payload_synthetics = payload
            .synthetics()
            .iter()
            .map(|record| {
                (
                    record.semantic_function(),
                    match record.rule() {
                        MultiRootCorrespondenceSyntheticRuleV2::EnumPayloadStorage => 1_u8,
                        MultiRootCorrespondenceSyntheticRuleV2::RuntimeAssertFailureTrap => 2_u8,
                    },
                    record.kernel_ir_block(),
                    record.first_operation(),
                    record.operation_count(),
                )
            })
            .collect::<Vec<_>>();
        let mut live_synthetics = live
            .synthetic_operation_spans()
            .iter()
            .filter(|record| record.correspondence_owner().index() == root.semantic_root())
            .map(|record| {
                (
                    record.semantic_function().index(),
                    match record.rule() {
                        SemanticKirSyntheticOperationRuleV1::EnumPayloadStorage => 1_u8,
                        SemanticKirSyntheticOperationRuleV1::RuntimeAssertFailureTrap => 2_u8,
                    },
                    record.kernel_ir_block().0,
                    record.first_operation_ordinal(),
                    record.operation_count(),
                )
            })
            .collect::<Vec<_>>();
        payload_synthetics.sort_unstable();
        live_synthetics.sort_unstable();
        let mut payload_parameters = payload
            .parameters()
            .iter()
            .map(|record| {
                (
                    record.semantic_function(),
                    record.semantic_local(),
                    record.kernel_ir_value(),
                )
            })
            .collect::<Vec<_>>();
        let mut live_parameters = live
            .parameter_bindings()
            .iter()
            .filter(|record| record.correspondence_owner().index() == root.semantic_root())
            .map(|record| {
                (
                    record.semantic_function().index(),
                    record.semantic_local().index(),
                    record.kernel_ir_value().0,
                )
            })
            .collect::<Vec<_>>();
        payload_parameters.sort_unstable();
        live_parameters.sort_unstable();
        if payload_blocks != live_blocks
            || payload_statement_tuples != live_statements
            || payload_terminators != live_terminators
            || payload_synthetics != live_synthetics
            || payload_parameters != live_parameters
        {
            return Err(correspondence_error(
                "multi-root block, operation, synthetic, or parameter custody differs from live correspondence",
            ));
        }
    }
    let mut live_owners = live
        .lowered_functions()
        .iter()
        .map(|record| record.correspondence_owner().index())
        .collect::<Vec<_>>();
    live_owners.sort_unstable();
    live_owners.dedup();
    if owners != live_owners {
        return Err(correspondence_error(
            "multi-root roster does not cover every live correspondence owner",
        ));
    }
    Ok(())
}

const fn correspondence_error(message: &'static str) -> ProductionPipelineError {
    ProductionPipelineError::SimulationDebugMapCorrespondence(message)
}

const fn source_map_resource_error(error: DebugSourceMapErrorV2) -> bool {
    matches!(
        error,
        DebugSourceMapErrorV2::InvalidLength
            | DebugSourceMapErrorV2::ResourceLimit
            | DebugSourceMapErrorV2::AllocationFailure
            | DebugSourceMapErrorV2::Encoding
    )
}

fn debug_identity_context(source_map_v2: &[u8], canonical_kir_v7: &[u8]) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(CONTEXT_DOMAIN_V1);
    hasher.update((source_map_v2.len() as u64).to_le_bytes());
    hasher.update(Sha256::digest(source_map_v2));
    hasher.update((canonical_kir_v7.len() as u64).to_le_bytes());
    hasher.update(Sha256::digest(canonical_kir_v7));
    hasher.finalize().into()
}

fn node_identity(context: [u8; 32], kind: u8, a: u64, b: u64, c: u64, d: u64) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(NODE_DOMAIN_V1);
    hasher.update(context);
    hasher.update([kind]);
    for value in [a, b, c, d] {
        hasher.update(value.to_le_bytes());
    }
    hasher.finalize().into()
}

fn mapping_identity(context: [u8; 32], kind: u8, input: [u8; 32], cardinality: u32) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(MAPPING_DOMAIN_V1);
    hasher.update(context);
    hasher.update([kind]);
    hasher.update(input);
    hasher.update(cardinality.to_le_bytes());
    hasher.finalize().into()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn identical_ordinals_in_distinct_exact_inputs_never_collide() {
        let first = debug_identity_context(b"source-a", b"kir-a");
        let second = debug_identity_context(b"source-b", b"kir-b");
        let first_node = node_identity(first, 3, 0, 0, 0, 0);
        let second_node = node_identity(second, 3, 0, 0, 0, 0);
        assert_ne!(first_node, second_node);
        assert_ne!(
            mapping_identity(first, 2, first_node, 1),
            mapping_identity(second, 2, second_node, 1)
        );
    }
}

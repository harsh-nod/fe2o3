//! Compiler-owned production semantic debug fragment construction.

use std::collections::BTreeMap;

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
    InertCanonicalMirToKirCorrespondenceEvidenceV4, ProductionSemanticKirOwnerV1,
};
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

pub(crate) fn prepare_production_semantic_debug_v1(
    lowered: &ProductionSemanticKirOwnerV1,
    correspondence: &InertCanonicalMirToKirCorrespondenceEvidenceV4,
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

    let mut bodies = lowered
        .module()
        .functions
        .iter()
        .enumerate()
        .filter_map(|(ordinal, function)| function.body.as_ref().map(|body| (ordinal, body)));
    let Some((function_ordinal, body)) = bodies.next() else {
        return Ok(PreparedProductionSemanticDebugV1::Unavailable(
            ProductionSemanticDebugProducerGapV1::NoStatementCorrespondence,
        ));
    };
    if bodies.next().is_some() {
        return Ok(PreparedProductionSemanticDebugV1::Unavailable(
            ProductionSemanticDebugProducerGapV1::MultipleKirFunctionBodies,
        ));
    }
    let function_ordinal = u64::try_from(function_ordinal).map_err(|_| {
        ProductionPipelineError::SimulationDebugMapCorrespondence(
            "KIR function ordinal exceeds the semantic debug wire",
        )
    })?;
    let block_ordinals = body
        .blocks
        .iter()
        .enumerate()
        .map(|(ordinal, block)| (block.id, ordinal))
        .collect::<BTreeMap<_, _>>();
    if block_ordinals.len() != body.blocks.len() {
        return Err(ProductionPipelineError::SimulationDebugMapCorrespondence(
            "KIR block identities are not unique",
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

    if *correspondence.semantic_sha256()
        != *lowered.semantic().semantic().semantic_sha256().as_bytes()
        || correspondence.canonical_kernel_ir_identity() != lowered.canonical_kernel_ir_identity()
    {
        return Err(ProductionPipelineError::SimulationDebugMapCorrespondence(
            "V4 correspondence names different semantic MIR or production KIR",
        ));
    }

    let counts = correspondence.statement_spans().iter().try_fold(
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
    for span in correspondence.statement_spans() {
        let source = lowered
            .semantic()
            .resolve_statement(
                fe2o3_mir_model::semantic_mir_v1::SemanticFunctionIdV1::from_index(
                    span.semantic_function(),
                ),
                fe2o3_mir_model::semantic_mir_v1::SemanticBlockIdV1::from_index(
                    span.semantic_block(),
                ),
                span.statement(),
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
            u64::from(span.semantic_function()),
            u64::from(span.semantic_block()),
            u64::from(span.statement()),
            0,
        );
        let mir_id = node_identity(
            context,
            2,
            u64::from(span.semantic_function()),
            u64::from(span.semantic_block()),
            u64::from(span.statement()),
            0,
        );
        nodes.push(semantic_map_value!(SemanticDebugNodeV1::new(
            source_id,
            SemanticDebugLocationV1::Source { span: source_span },
        )));
        nodes.push(semantic_map_value!(SemanticDebugNodeV1::new(
            mir_id,
            SemanticDebugLocationV1::Mir {
                body_ordinal: u64::from(span.semantic_function()),
                block_ordinal: u64::from(span.semantic_block()),
                statement_ordinal: u64::from(span.statement()),
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

        let block_ordinal = u64::try_from(
            *block_ordinals
                .get(&fe2o3_kernel_ir::BlockId(span.kernel_ir_block()))
                .ok_or(ProductionPipelineError::SimulationDebugMapCorrespondence(
                    "statement debug correspondence names an unknown KIR block",
                ))?,
        )
        .map_err(|_| {
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
        for operation in span.first_operation()
            ..span
                .first_operation()
                .checked_add(span.operation_count())
                .ok_or(ProductionPipelineError::SimulationDebugMapCorrespondence(
                    "statement KIR operation range overflows",
                ))?
        {
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
                function_ordinal,
                block_ordinal,
                u64::from(operation),
                0,
            );
            nodes.push(semantic_map_value!(SemanticDebugNodeV1::new(
                kir_id,
                SemanticDebugLocationV1::Kir {
                    function_ordinal,
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

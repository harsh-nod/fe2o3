use std::collections::BTreeMap;

use fe2o3_kernel_ir::{
    DebugSourceMapBindingV1, DebugSourceMapDocumentV2, DebugSourceMapFileV1,
    DebugSourceMapKirSiteV1, DebugSourceMapSiteV1, DebugSourceMapSpanV1,
    PRODUCTION_PREFINALIZED_ARTIFACT_STATUS_V1, ProductionScheduleStatusV1,
    ProductionSemanticDebugAvailabilityV1, ProductionSemanticDebugCarrierV1,
    ProductionSemanticDebugFragmentV1, SemanticDebugBoundaryDirectionV1,
    SemanticDebugBoundaryReasonV1, SemanticDebugBoundaryV1, SemanticDebugContentIdentityV1,
    SemanticDebugLayerV1, SemanticDebugLocationV1, SemanticDebugMapBindingV1,
    SemanticDebugMapDocumentV1, SemanticDebugMappingOutputV1, SemanticDebugMappingV1,
    SemanticDebugNodeV1, SemanticDebugTransformationV1, SemanticDebugUnavailableReasonV1,
    VerifiedCanonicalKernelIrV7, VerifiedCanonicalKernelIrV8,
};
use fe2o3_mir_kir_contracts::InertCanonicalMirToKirCorrespondenceEvidenceV4;
use fe2o3_mir_model::semantic_mir_v1::{AdmittedInertSemanticMirV1, SemanticMirLimitsV1};
use sha2::{Digest, Sha256};

fn stable_id(tag: u8, values: &[u32]) -> [u8; 32] {
    let mut digest = Sha256::new();
    digest.update(b"FE2O3/PRODUCTION-SEMANTIC-DEBUG-TEST-FIXTURE/V1\0");
    digest.update([tag]);
    for value in values {
        digest.update(value.to_le_bytes());
    }
    digest.finalize().into()
}

#[allow(clippy::too_many_lines)]
pub(crate) fn exact_source_mir_kir_carrier_v1(
    association: &[u8],
    semantic_mir: &[u8],
    canonical_kir_v8: &[u8],
    correspondence_v4: &[u8],
    llvm_module: &[u8],
) -> ProductionSemanticDebugCarrierV1 {
    exact_source_mir_kir_carrier_with_projection_v1(
        association,
        semantic_mir,
        canonical_kir_v8,
        correspondence_v4,
        llvm_module,
        canonical_kir_v8,
    )
}

#[allow(clippy::too_many_lines)]
pub(crate) fn exact_source_mir_kir_carrier_with_projection_v1(
    association: &[u8],
    semantic_mir: &[u8],
    _canonical_kir_v8: &[u8],
    correspondence_v4: &[u8],
    llvm_module: &[u8],
    projection_kir_v8: &[u8],
) -> ProductionSemanticDebugCarrierV1 {
    let (_, module) =
        VerifiedCanonicalKernelIrV8::from_canonical_bytes_with_module(projection_kir_v8.to_vec())
            .unwrap();
    let canonical_kir_v7 = VerifiedCanonicalKernelIrV7::from_module(module.clone())
        .unwrap()
        .into_canonical_bytes();
    let kir_v7 =
        VerifiedCanonicalKernelIrV7::from_canonical_bytes(canonical_kir_v7.clone()).unwrap();
    let semantic = AdmittedInertSemanticMirV1::decode_current_production_canonical(
        semantic_mir,
        SemanticMirLimitsV1::default(),
    )
    .unwrap();
    let correspondence =
        InertCanonicalMirToKirCorrespondenceEvidenceV4::decode(correspondence_v4).unwrap();
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
    let mut files = BTreeMap::<[u8; 32], u64>::new();
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
        files
            .entry(*origin.file().as_bytes())
            .and_modify(|length| *length = (*length).max(byte_end))
            .or_insert(byte_end);
        let coordinates = [
            span.semantic_function(),
            span.semantic_block(),
            span.statement(),
        ];
        let source_id = stable_id(1, &coordinates);
        let mir_id = stable_id(2, &coordinates);
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
                stable_id(4, &coordinates),
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
                    stable_id(5, &coordinates),
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
        let end = span
            .first_operation()
            .checked_add(span.operation_count())
            .unwrap();
        for operation in span.first_operation()..end {
            sites.push(
                DebugSourceMapSiteV1::new(
                    DebugSourceMapKirSiteV1::operation(
                        function_ordinal as u64,
                        block_ordinal as u64,
                        u64::from(operation),
                    ),
                    vec![source_span],
                )
                .unwrap(),
            );
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
                stable_id(5, &coordinates),
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
    let files = files
        .into_iter()
        .enumerate()
        .map(|(ordinal, (identity, length))| {
            DebugSourceMapFileV1::new(
                identity,
                length.max(1),
                format!("/src/sourceful-{ordinal}.rs"),
            )
            .unwrap()
        })
        .collect();
    let source_map = DebugSourceMapDocumentV2::new(
        DebugSourceMapBindingV1::new(
            [0x70; 32],
            *kir_v7.identity().digest(),
            kir_v7.identity().canonical_length(),
        )
        .unwrap(),
        files,
        sites,
        eliminated,
        Vec::new(),
        Vec::new(),
    )
    .unwrap()
    .to_canonical_json_bytes()
    .unwrap();
    let schedule = ProductionScheduleStatusV1::NoProductionScheduleStage
        .canonical_bytes()
        .to_vec();
    let content = |bytes: &[u8]| SemanticDebugContentIdentityV1::calculate(bytes).unwrap();
    let map = SemanticDebugMapDocumentV1::new_partial(
        SemanticDebugMapBindingV1::new(
            content(&source_map),
            content(semantic_mir),
            content(&canonical_kir_v7),
            content(&schedule),
            content(llvm_module),
            content(PRODUCTION_PREFINALIZED_ARTIFACT_STATUS_V1),
        )
        .unwrap(),
        nodes,
        mappings,
        boundaries,
    )
    .unwrap()
    .to_canonical_json_bytes()
    .unwrap();
    let fragment =
        ProductionSemanticDebugFragmentV1::new(source_map, canonical_kir_v7, schedule, map)
            .unwrap();
    ProductionSemanticDebugCarrierV1::new(
        association,
        ProductionSemanticDebugAvailabilityV1::Available(fragment),
    )
    .unwrap()
}

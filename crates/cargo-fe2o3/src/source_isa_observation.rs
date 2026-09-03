use dialect_amdgcn::ProductionReplayKernelIrVersionV1;
use fe2o3_amd_target::ProductionAmdTargetProfileV1;
use fe2o3_artifact_transaction::BuildAttempt;
use fe2o3_hsaco_finalize::{
    FinalizedSemanticDebugMapErrorV1, PreparedFinalizedProtectedWorkerV3HsacoV1,
    ProductionKirV7BridgeAdmissionV1, ProductionSemanticAnchorErrorV1,
    ProductionSemanticAnchorUnavailableV1, ProductionSourceIsaAcceptanceSummaryAdmissionV1,
    ProductionSourceIsaAcceptanceSummaryV1, ProductionSourceIsaCatalogAdmissionV1,
    ProductionSourceIsaCharacteristicAdmissionV1, ProductionSourceIsaCorrelationErrorV1,
    ProductionSourceIsaCorrelationUnavailableV1, admit_production_kir_v7_structural_bridge_v1,
    admit_production_source_isa_characteristics_v1,
    readmit_exact_production_source_isa_characteristic_projection_v1,
    release_production_source_isa_characteristic_projection_v1,
};
use fe2o3_kernel_ir::{
    ProductionSemanticDebugAvailabilityV1, ProductionSemanticDebugFragmentErrorV1,
    ProductionSemanticDebugProducerGapV1, ProductionSemanticDebugReceiptExtensionV1,
    SemanticDebugMapErrorV1,
};
pub(crate) use fe2o3_source_isa_observation::wire_v1::*;
use sha2::{Digest, Sha256};

pub(crate) const SOURCE_ISA_CHARACTERISTIC_BROKER_MAGIC_V3: &[u8] =
    b"FE2O3/SOURCE-ISA-CHARACTERISTIC-BROKER/V3\0";
const SOURCE_ISA_CHARACTERISTIC_BROKER_CONFIG_DOMAIN_V3: &[u8] =
    b"FE2O3/SOURCE-ISA-CHARACTERISTIC-BROKER-CONFIG/V3\0";
const SOURCE_ISA_CHARACTERISTIC_BODY_FORMAT_V1: &[u8] =
    b"fe2o3-source-isa-characteristic-collection-v1";

pub(crate) fn source_isa_characteristic_broker_config_identity_v3() -> [u8; 32] {
    let mut digest = Sha256::new();
    for field in [
        SOURCE_ISA_CHARACTERISTIC_BROKER_CONFIG_DOMAIN_V3,
        SOURCE_ISA_CHARACTERISTIC_BROKER_MAGIC_V3,
        SOURCE_ISA_CHARACTERISTIC_BODY_FORMAT_V1,
        &u64::try_from(
            fe2o3_source_isa_observation::characteristic_v1::MAX_SOURCE_ISA_CHARACTERISTIC_COLLECTION_BYTES_V1,
        )
        .expect("characteristic collection bound fits u64")
        .to_le_bytes(),
        b"u64-le-length-prefix",
        b"config-unit-target-cell-binding",
        b"exact-eof-required",
    ] {
        digest.update(
            u64::try_from(field.len())
                .expect("Broker identity field length fits u64")
                .to_le_bytes(),
        );
        digest.update(field);
    }
    digest.finalize().into()
}

pub(crate) fn encode_source_isa_characteristic_broker_v3(
    config: [u8; 32],
    unit: [u8; 32],
    body: &[u8],
) -> Result<Vec<u8>, String> {
    if config == [0; 32]
        || unit == [0; 32]
        || body.is_empty()
        || body.len()
            > fe2o3_source_isa_observation::characteristic_v1::MAX_SOURCE_ISA_CHARACTERISTIC_COLLECTION_BYTES_V1
    {
        return Err("invalid Source/ISA characteristic Broker V3 cell".to_owned());
    }
    let inert = fe2o3_source_isa_observation::characteristic_v1::InertSourceIsaCharacteristicCollectionV1::decode_canonical(body)
        .map_err(|error| format!("invalid Source/ISA characteristic body: {error}"))?;
    let target = match inert.claimed_binding().target_profile() {
        fe2o3_source_isa_observation::characteristic_v1::SourceIsaCharacteristicTargetProfileV1::Gfx942 => 1_u16,
        fe2o3_source_isa_observation::characteristic_v1::SourceIsaCharacteristicTargetProfileV1::Gfx950 => 2_u16,
    };
    let total = SOURCE_ISA_CHARACTERISTIC_BROKER_MAGIC_V3
        .len()
        .checked_add(32 + 32 + 32 + 2 + 8)
        .and_then(|length| length.checked_add(body.len()))
        .ok_or_else(|| "Source/ISA characteristic Broker V3 length overflow".to_owned())?;
    let mut encoded = Vec::new();
    encoded
        .try_reserve_exact(total)
        .map_err(|_| "cannot allocate Source/ISA characteristic Broker V3 cell".to_owned())?;
    encoded.extend_from_slice(SOURCE_ISA_CHARACTERISTIC_BROKER_MAGIC_V3);
    encoded.extend_from_slice(&source_isa_characteristic_broker_config_identity_v3());
    encoded.extend_from_slice(&config);
    encoded.extend_from_slice(&unit);
    encoded.extend_from_slice(&target.to_le_bytes());
    encoded.extend_from_slice(
        &u64::try_from(body.len())
            .map_err(|_| "Source/ISA characteristic body length exceeds u64".to_owned())?
            .to_le_bytes(),
    );
    encoded.extend_from_slice(body);
    Ok(encoded)
}

pub(crate) fn finalized_source_isa_characteristic_observation_v1(
    finalized: &PreparedFinalizedProtectedWorkerV3HsacoV1,
) -> Result<
    fe2o3_source_isa_observation::characteristic_v1::SourceIsaCharacteristicCollectionV1,
    String,
> {
    let extension = ProductionSemanticDebugReceiptExtensionV1::from_canonical_bytes(
        finalized
            .outer_handoff()
            .capsule()
            .receipts()
            .semantic_to_llvm()
            .canonical_preimage(),
    )
    .map_err(|error| format!("invalid production semantic-debug receipt: {error}"))?;
    let ProductionSemanticDebugAvailabilityV1::Available(fragment) =
        extension.carrier_v1().availability()
    else {
        return Err("production semantic-debug characteristic evidence is unavailable".to_owned());
    };
    let target_kir = finalized
        .outer_handoff()
        .capsule()
        .receipts()
        .kernel_ir()
        .canonical_preimage();
    let catalog = match finalized
        .admit_production_source_isa_catalog_v1()
        .map_err(|error| format!("production Source/ISA catalog admission failed: {error}"))?
    {
        ProductionSourceIsaCatalogAdmissionV1::Admitted(catalog) => catalog,
        ProductionSourceIsaCatalogAdmissionV1::Unavailable(reason) => {
            return Err(format!(
                "production Source/ISA catalog evidence is unavailable: {reason:?}"
            ));
        }
    };
    let bridge = match admit_production_kir_v7_structural_bridge_v1(
        fragment.canonical_kir_v7(),
        target_kir,
        fragment.source_map_v2(),
        finalized.exact_finalized_bytes(),
        &catalog,
    )
    .map_err(|error| format!("production KIR V7 structural bridge admission failed: {error}"))?
    {
        ProductionKirV7BridgeAdmissionV1::Admitted(bridge) => bridge,
        ProductionKirV7BridgeAdmissionV1::Unavailable(reason) => {
            return Err(format!(
                "production KIR V7 structural bridge is unavailable: {reason:?}"
            ));
        }
    };
    let producer =
        match admit_production_source_isa_characteristics_v1(target_kir, &catalog, &bridge)
            .map_err(|error| {
                format!("production Source/ISA characteristic admission failed: {error}")
            })? {
            ProductionSourceIsaCharacteristicAdmissionV1::Admitted(producer) => producer,
            ProductionSourceIsaCharacteristicAdmissionV1::Unavailable(reason) => {
                return Err(format!(
                    "production Source/ISA characteristic evidence is unavailable: {reason:?}"
                ));
            }
        };
    let released = release_production_source_isa_characteristic_projection_v1(&producer)
        .map_err(|error| format!("Source/ISA characteristic release failed: {error}"))?;
    let encoded = released
        .encode_canonical()
        .map_err(|error| format!("Source/ISA characteristic encoding failed: {error}"))?;
    let inert = fe2o3_source_isa_observation::characteristic_v1::InertSourceIsaCharacteristicCollectionV1::decode_canonical(&encoded)
        .map_err(|error| format!("Source/ISA characteristic self-decode failed: {error}"))?;
    readmit_exact_production_source_isa_characteristic_projection_v1(inert, &producer)
        .map_err(|error| format!("Source/ISA characteristic exact readmission failed: {error}"))
}

pub(crate) const fn inert_source_isa_session_v1(
    session: fe2o3_artifact_transaction::BuildSession,
) -> SourceIsaObservationSessionV1 {
    SourceIsaObservationSessionV1::from_bytes(*session.as_bytes())
}

pub(crate) fn inert_source_isa_attempt_v1(
    attempt: BuildAttempt,
) -> Result<SourceIsaObservationAttemptV1, SourceIsaObservationFrameErrorV1> {
    SourceIsaObservationAttemptV1::new(
        attempt.generation(),
        inert_source_isa_session_v1(attempt.session()),
        SourceIsaObservationInvocationV1::from_bytes(*attempt.invocation().as_bytes()),
    )
}

pub fn finalized_source_isa_observation_frame_v1(
    config: [u8; 32],
    unit: [u8; 32],
    finalized: &PreparedFinalizedProtectedWorkerV3HsacoV1,
) -> Result<SourceIsaObservationFrameV1, SourceIsaObservationFrameErrorV1> {
    let context = SourceIsaObservationContextV1::new(
        config,
        unit,
        inert_source_isa_attempt_v1(finalized.attempt())?,
        *finalized.identity().as_bytes(),
    )?;
    let outcome = match finalized.admit_production_source_isa_acceptance_summary_v1() {
        Ok(ProductionSourceIsaAcceptanceSummaryAdmissionV1::Admitted(summary)) => {
            SourceIsaObservationOutcomeV1::Admitted(map_acceptance_summary(summary)?)
        }
        Ok(ProductionSourceIsaAcceptanceSummaryAdmissionV1::Unavailable(reason)) => {
            SourceIsaObservationOutcomeV1::Unavailable(map_unavailable_reason(reason))
        }
        Err(error) => SourceIsaObservationOutcomeV1::Error(map_correlation_error(error)),
    };
    Ok(SourceIsaObservationFrameV1::new(context, outcome))
}

pub fn ready_source_isa_observation_frame_v1(
    config: [u8; 32],
    unit: [u8; 32],
    attempt: BuildAttempt,
    finalization: [u8; 32],
) -> Result<SourceIsaObservationFrameV1, SourceIsaObservationFrameErrorV1> {
    let context = SourceIsaObservationContextV1::new(
        config,
        unit,
        inert_source_isa_attempt_v1(attempt)?,
        finalization,
    )?;
    Ok(SourceIsaObservationFrameV1::new(
        context,
        SourceIsaObservationOutcomeV1::Unavailable(
            SourceIsaObservationUnavailableReasonV1::FinalizedEvidenceUnavailableFromReadyState,
        ),
    ))
}

fn map_acceptance_summary(
    summary: ProductionSourceIsaAcceptanceSummaryV1,
) -> Result<AdmittedSourceIsaObservationV1, SourceIsaObservationFrameErrorV1> {
    if summary.format_version() != 1
        || summary.proves_complete_machine_instruction_coverage()
        || summary.proves_a_schedule()
        || summary.proves_semantic_refinement()
        || summary.proves_optimized_or_final_llvm_custody()
        || summary.proves_live_program_counter_ownership()
        || summary.retains_correlation_records()
        || summary.grants_publication_authority()
        || summary.grants_runtime_authority()
    {
        return Err(SourceIsaObservationFrameErrorV1::TruthClaim);
    }
    let artifact = summary.artifact_identity();
    let structural = summary.structural_binding();
    let neutral = structural.neutral_kernel_ir();
    let target = structural.target_bound_kernel_ir();
    let structural_counts = structural.counts();
    let counts = summary.counts();
    let witness = summary
        .round_trip_witness()
        .map(|witness| {
            let span = witness.source_span();
            let point = witness.isa_point();
            SourceIsaObservationRoundTripWitnessV1::new(
                *witness.source_node_identity(),
                SourceIsaObservationSourceSpanV1::new(
                    span.file_identity(),
                    span.byte_start(),
                    span.byte_end(),
                    span.line(),
                    span.column(),
                )?,
                SourceIsaObservationIsaPointV1::new(
                    point.kernel_ordinal(),
                    point.symbol_relative_pc(),
                )?,
                witness.source_node_query_matches(),
                witness.source_span_query_matches(),
                witness.isa_point_query_matches(),
            )
        })
        .transpose()?;
    AdmittedSourceIsaObservationV1::new(
        *summary.correlation_identity(),
        SourceIsaObservationContentIdentityV1::new(*artifact.sha256(), artifact.byte_len())?,
        SourceIsaObservationStructuralBindingV1::new(
            structural.identity(),
            match structural.profile() {
                ProductionAmdTargetProfileV1::Gfx942 => SourceIsaObservationTargetProfileV1::Gfx942,
                ProductionAmdTargetProfileV1::Gfx950 => SourceIsaObservationTargetProfileV1::Gfx950,
            },
            match structural.version() {
                ProductionReplayKernelIrVersionV1::V8 => SourceIsaObservationKirVersionV1::V8,
                ProductionReplayKernelIrVersionV1::V9 => SourceIsaObservationKirVersionV1::V9,
            },
            SourceIsaObservationContentIdentityV1::new(neutral.sha256(), neutral.byte_len())?,
            SourceIsaObservationContentIdentityV1::new(target.sha256(), target.byte_len())?,
            SourceIsaObservationStructuralCountsV1 {
                functions: structural_counts.functions(),
                defined_bodies: structural_counts.defined_bodies(),
                blocks: structural_counts.blocks(),
                operations: structural_counts.operations(),
            },
        )?,
        SourceIsaObservationCountsV1::new(
            SourceIsaObservationRecordCountsV1 {
                records: counts.records(),
                source_anchored: counts.source_anchored_records(),
                eliminated: counts.eliminated_before_kir_records(),
                no_source: counts.no_source_provenance_records(),
                source_anchored_without_isa: counts.source_anchored_without_isa_records(),
                isa_references: counts.isa_references(),
            },
            SourceIsaObservationQueryCountsV1 {
                distinct_source_nodes: counts.distinct_source_node_queries(),
                distinct_source_spans: counts.distinct_source_span_queries(),
                distinct_isa_points: counts.distinct_isa_point_queries(),
                max_source_node_cardinality: counts.maximum_source_node_query_matches(),
                max_source_span_cardinality: counts.maximum_source_span_query_matches(),
                max_exact_pc_cardinality: counts.maximum_isa_point_query_matches(),
            },
        )?,
        witness,
    )
}

const fn map_unavailable_reason(
    reason: ProductionSourceIsaCorrelationUnavailableV1,
) -> SourceIsaObservationUnavailableReasonV1 {
    match reason {
        ProductionSourceIsaCorrelationUnavailableV1::SemanticDebugCarrier(reason) => match reason {
            ProductionSemanticDebugProducerGapV1::MultipleKirFunctionBodies => {
                SourceIsaObservationUnavailableReasonV1::CarrierMultipleKirFunctionBodies
            }
            ProductionSemanticDebugProducerGapV1::NoStatementCorrespondence => {
                SourceIsaObservationUnavailableReasonV1::CarrierNoStatementCorrespondence
            }
            ProductionSemanticDebugProducerGapV1::SourceMapUnavailable => {
                SourceIsaObservationUnavailableReasonV1::CarrierSourceMapUnavailable
            }
            ProductionSemanticDebugProducerGapV1::ResourceLimit => {
                SourceIsaObservationUnavailableReasonV1::CarrierResourceLimit
            }
            ProductionSemanticDebugProducerGapV1::CanonicalKirV7ProjectionUnavailable => {
                SourceIsaObservationUnavailableReasonV1::CarrierCanonicalKirV7ProjectionUnavailable
            }
            ProductionSemanticDebugProducerGapV1::SourceObservationUnrepresentable => {
                SourceIsaObservationUnavailableReasonV1::CarrierSourceObservationUnrepresentable
            }
            ProductionSemanticDebugProducerGapV1::SemanticMapConstructionUnavailable => {
                SourceIsaObservationUnavailableReasonV1::CarrierSemanticMapConstructionUnavailable
            }
            ProductionSemanticDebugProducerGapV1::SemanticMapEncodingUnavailable => {
                SourceIsaObservationUnavailableReasonV1::CarrierSemanticMapEncodingUnavailable
            }
            ProductionSemanticDebugProducerGapV1::FragmentConstructionUnavailable => {
                SourceIsaObservationUnavailableReasonV1::CarrierFragmentConstructionUnavailable
            }
            ProductionSemanticDebugProducerGapV1::CarrierConstructionUnavailable => {
                SourceIsaObservationUnavailableReasonV1::CarrierConstructionUnavailable
            }
            ProductionSemanticDebugProducerGapV1::ReceiptExtensionConstructionUnavailable => {
                SourceIsaObservationUnavailableReasonV1::CarrierReceiptExtensionConstructionUnavailable
            }
            ProductionSemanticDebugProducerGapV1::CorrespondenceValidationUnavailable => {
                SourceIsaObservationUnavailableReasonV1::CarrierCorrespondenceValidationUnavailable
            }
            ProductionSemanticDebugProducerGapV1::CanonicalKirModuleMismatch => {
                SourceIsaObservationUnavailableReasonV1::CarrierCanonicalKirModuleMismatch
            }
            ProductionSemanticDebugProducerGapV1::LegacyBareAssociationNoAttachment => {
                SourceIsaObservationUnavailableReasonV1::CarrierLegacyBareAssociationNoAttachment
            }
        },
        ProductionSourceIsaCorrelationUnavailableV1::SemanticAnchors(reason) => match reason {
            ProductionSemanticAnchorUnavailableV1::LegacySemanticAttachment => {
                SourceIsaObservationUnavailableReasonV1::AnchorLegacySemanticAttachment
            }
            ProductionSemanticAnchorUnavailableV1::LegacyUninstrumentedReplay => {
                SourceIsaObservationUnavailableReasonV1::AnchorLegacyUninstrumentedReplay
            }
            ProductionSemanticAnchorUnavailableV1::NoOperations => {
                SourceIsaObservationUnavailableReasonV1::AnchorNoOperations
            }
            ProductionSemanticAnchorUnavailableV1::MultipleDefinedBodies => {
                SourceIsaObservationUnavailableReasonV1::AnchorMultipleDefinedBodies
            }
            ProductionSemanticAnchorUnavailableV1::CompilerInstrumentationAbsent => {
                SourceIsaObservationUnavailableReasonV1::AnchorCompilerInstrumentationAbsent
            }
        },
        ProductionSourceIsaCorrelationUnavailableV1::SourceProjectionForKirV9 => {
            SourceIsaObservationUnavailableReasonV1::SourceProjectionForKirV9
        }
    }
}

fn map_correlation_error(
    error: ProductionSourceIsaCorrelationErrorV1,
) -> SourceIsaObservationErrorCodeV1 {
    match error {
        ProductionSourceIsaCorrelationErrorV1::SemanticDebugMap(error) => {
            map_semantic_debug_map_error(error)
        }
        ProductionSourceIsaCorrelationErrorV1::SemanticAnchors(error) => {
            map_semantic_anchor_error(error)
        }
        ProductionSourceIsaCorrelationErrorV1::InvalidKirToLlvmReplay => {
            SourceIsaObservationErrorCodeV1::InvalidKirToLlvmReplay
        }
        ProductionSourceIsaCorrelationErrorV1::NonExactSemanticMap => {
            SourceIsaObservationErrorCodeV1::NonExactSemanticMap
        }
        ProductionSourceIsaCorrelationErrorV1::ArtifactIdentityMismatch => {
            SourceIsaObservationErrorCodeV1::ArtifactIdentityMismatch
        }
        ProductionSourceIsaCorrelationErrorV1::TargetKirIdentityMismatch => {
            SourceIsaObservationErrorCodeV1::TargetKirIdentityMismatch
        }
        ProductionSourceIsaCorrelationErrorV1::CoordinateShapeMismatch => {
            SourceIsaObservationErrorCodeV1::CoordinateShapeMismatch
        }
        ProductionSourceIsaCorrelationErrorV1::InvalidSourceGraph => {
            SourceIsaObservationErrorCodeV1::InvalidSourceGraph
        }
        ProductionSourceIsaCorrelationErrorV1::ResourceLimit => {
            SourceIsaObservationErrorCodeV1::ResourceLimit
        }
        ProductionSourceIsaCorrelationErrorV1::AllocationFailure => {
            SourceIsaObservationErrorCodeV1::AllocationFailure
        }
    }
}

const fn map_semantic_debug_map_error(
    error: FinalizedSemanticDebugMapErrorV1,
) -> SourceIsaObservationErrorCodeV1 {
    match error {
        FinalizedSemanticDebugMapErrorV1::SemanticMap(error) => map_semantic_map_error(error),
        FinalizedSemanticDebugMapErrorV1::ProductionFragment(error) => {
            map_production_fragment_error(error)
        }
        FinalizedSemanticDebugMapErrorV1::ProductionAssociation => {
            SourceIsaObservationErrorCodeV1::FinalizedMapProductionAssociation
        }
        FinalizedSemanticDebugMapErrorV1::ProductionAssociationMismatch => {
            SourceIsaObservationErrorCodeV1::FinalizedMapProductionAssociationMismatch
        }
        FinalizedSemanticDebugMapErrorV1::InvalidKirToLlvmReplay => {
            SourceIsaObservationErrorCodeV1::FinalizedMapInvalidKirToLlvmReplay
        }
        FinalizedSemanticDebugMapErrorV1::KirToLlvmReplayTargetMismatch => {
            SourceIsaObservationErrorCodeV1::FinalizedMapKirToLlvmReplayTargetMismatch
        }
        FinalizedSemanticDebugMapErrorV1::InvalidLlvmToHsacoCustody => {
            SourceIsaObservationErrorCodeV1::FinalizedMapInvalidLlvmToHsacoCustody
        }
        FinalizedSemanticDebugMapErrorV1::InvalidBoundSourceMap => {
            SourceIsaObservationErrorCodeV1::FinalizedMapInvalidBoundSourceMap
        }
        FinalizedSemanticDebugMapErrorV1::InvalidBoundSemanticMir => {
            SourceIsaObservationErrorCodeV1::FinalizedMapInvalidBoundSemanticMir
        }
        FinalizedSemanticDebugMapErrorV1::InvalidBoundCorrespondenceV4 => {
            SourceIsaObservationErrorCodeV1::FinalizedMapInvalidBoundCorrespondenceV4
        }
        FinalizedSemanticDebugMapErrorV1::InvalidBoundCanonicalKirV8 => {
            SourceIsaObservationErrorCodeV1::FinalizedMapInvalidBoundCanonicalKirV8
        }
        FinalizedSemanticDebugMapErrorV1::InvalidBoundCanonicalKirV7 => {
            SourceIsaObservationErrorCodeV1::FinalizedMapInvalidBoundCanonicalKirV7
        }
        FinalizedSemanticDebugMapErrorV1::CanonicalKirProjectionMismatch => {
            SourceIsaObservationErrorCodeV1::FinalizedMapCanonicalKirProjectionMismatch
        }
        FinalizedSemanticDebugMapErrorV1::CorrespondenceIdentityMismatch => {
            SourceIsaObservationErrorCodeV1::FinalizedMapCorrespondenceIdentityMismatch
        }
        FinalizedSemanticDebugMapErrorV1::InvalidSemanticCorrespondence => {
            SourceIsaObservationErrorCodeV1::FinalizedMapInvalidSemanticCorrespondence
        }
        FinalizedSemanticDebugMapErrorV1::ArtifactInspection => {
            SourceIsaObservationErrorCodeV1::FinalizedMapArtifactInspection
        }
        FinalizedSemanticDebugMapErrorV1::AllocationFailure => {
            SourceIsaObservationErrorCodeV1::FinalizedMapAllocationFailure
        }
    }
}

const fn map_semantic_map_error(error: SemanticDebugMapErrorV1) -> SourceIsaObservationErrorCodeV1 {
    match error {
        SemanticDebugMapErrorV1::InvalidLength => {
            SourceIsaObservationErrorCodeV1::SemanticMapInvalidLength
        }
        SemanticDebugMapErrorV1::InvalidJson => {
            SourceIsaObservationErrorCodeV1::SemanticMapInvalidJson
        }
        SemanticDebugMapErrorV1::NonCanonicalEncoding => {
            SourceIsaObservationErrorCodeV1::SemanticMapNonCanonicalEncoding
        }
        SemanticDebugMapErrorV1::Encoding => SourceIsaObservationErrorCodeV1::SemanticMapEncoding,
        SemanticDebugMapErrorV1::InvalidBinding => {
            SourceIsaObservationErrorCodeV1::SemanticMapInvalidBinding
        }
        SemanticDebugMapErrorV1::InvalidKernelOrdinalBasis => {
            SourceIsaObservationErrorCodeV1::SemanticMapInvalidKernelOrdinalBasis
        }
        SemanticDebugMapErrorV1::InvalidNode => {
            SourceIsaObservationErrorCodeV1::SemanticMapInvalidNode
        }
        SemanticDebugMapErrorV1::InvalidMapping => {
            SourceIsaObservationErrorCodeV1::SemanticMapInvalidMapping
        }
        SemanticDebugMapErrorV1::DuplicateNode => {
            SourceIsaObservationErrorCodeV1::SemanticMapDuplicateNode
        }
        SemanticDebugMapErrorV1::DuplicateMapping => {
            SourceIsaObservationErrorCodeV1::SemanticMapDuplicateMapping
        }
        SemanticDebugMapErrorV1::DuplicateReference => {
            SourceIsaObservationErrorCodeV1::SemanticMapDuplicateReference
        }
        SemanticDebugMapErrorV1::UnknownNode => {
            SourceIsaObservationErrorCodeV1::SemanticMapUnknownNode
        }
        SemanticDebugMapErrorV1::LayerMismatch => {
            SourceIsaObservationErrorCodeV1::SemanticMapLayerMismatch
        }
        SemanticDebugMapErrorV1::ContradictoryMapping => {
            SourceIsaObservationErrorCodeV1::SemanticMapContradictoryMapping
        }
        SemanticDebugMapErrorV1::OrphanNode => {
            SourceIsaObservationErrorCodeV1::SemanticMapOrphanNode
        }
        SemanticDebugMapErrorV1::InvalidBoundary => {
            SourceIsaObservationErrorCodeV1::SemanticMapInvalidBoundary
        }
        SemanticDebugMapErrorV1::UntypedBoundary => {
            SourceIsaObservationErrorCodeV1::SemanticMapUntypedBoundary
        }
        SemanticDebugMapErrorV1::ResourceLimit => {
            SourceIsaObservationErrorCodeV1::SemanticMapResourceLimit
        }
        SemanticDebugMapErrorV1::AllocationFailure => {
            SourceIsaObservationErrorCodeV1::SemanticMapAllocationFailure
        }
        SemanticDebugMapErrorV1::ContentBindingMismatch => {
            SourceIsaObservationErrorCodeV1::SemanticMapContentBindingMismatch
        }
        SemanticDebugMapErrorV1::ArtifactBindingMismatch => {
            SourceIsaObservationErrorCodeV1::SemanticMapArtifactBindingMismatch
        }
        SemanticDebugMapErrorV1::InvalidBoundSourceMap => {
            SourceIsaObservationErrorCodeV1::SemanticMapInvalidBoundSourceMap
        }
        SemanticDebugMapErrorV1::InvalidBoundCanonicalKir => {
            SourceIsaObservationErrorCodeV1::SemanticMapInvalidBoundCanonicalKir
        }
        SemanticDebugMapErrorV1::SourceMapKirBindingMismatch => {
            SourceIsaObservationErrorCodeV1::SemanticMapSourceMapKirBindingMismatch
        }
        SemanticDebugMapErrorV1::InvalidSourceLocation => {
            SourceIsaObservationErrorCodeV1::SemanticMapInvalidSourceLocation
        }
        SemanticDebugMapErrorV1::InvalidMirLocation => {
            SourceIsaObservationErrorCodeV1::SemanticMapInvalidMirLocation
        }
        SemanticDebugMapErrorV1::InvalidKirLocation => {
            SourceIsaObservationErrorCodeV1::SemanticMapInvalidKirLocation
        }
        SemanticDebugMapErrorV1::InvalidIsaInterval => {
            SourceIsaObservationErrorCodeV1::SemanticMapInvalidIsaInterval
        }
    }
}

const fn map_production_fragment_error(
    error: ProductionSemanticDebugFragmentErrorV1,
) -> SourceIsaObservationErrorCodeV1 {
    match error {
        ProductionSemanticDebugFragmentErrorV1::InvalidEncoding => {
            SourceIsaObservationErrorCodeV1::ProductionFragmentInvalidEncoding
        }
        ProductionSemanticDebugFragmentErrorV1::InvalidAssociation => {
            SourceIsaObservationErrorCodeV1::ProductionFragmentInvalidAssociation
        }
        ProductionSemanticDebugFragmentErrorV1::InvalidGap => {
            SourceIsaObservationErrorCodeV1::ProductionFragmentInvalidGap
        }
        ProductionSemanticDebugFragmentErrorV1::InvalidScheduleStatus => {
            SourceIsaObservationErrorCodeV1::ProductionFragmentInvalidScheduleStatus
        }
        ProductionSemanticDebugFragmentErrorV1::InvalidSourceMap => {
            SourceIsaObservationErrorCodeV1::ProductionFragmentInvalidSourceMap
        }
        ProductionSemanticDebugFragmentErrorV1::InvalidCanonicalKir => {
            SourceIsaObservationErrorCodeV1::ProductionFragmentInvalidCanonicalKir
        }
        ProductionSemanticDebugFragmentErrorV1::InvalidSemanticMap => {
            SourceIsaObservationErrorCodeV1::ProductionFragmentInvalidSemanticMap
        }
        ProductionSemanticDebugFragmentErrorV1::AxisMismatch => {
            SourceIsaObservationErrorCodeV1::ProductionFragmentAxisMismatch
        }
        ProductionSemanticDebugFragmentErrorV1::ResourceLimit => {
            SourceIsaObservationErrorCodeV1::ProductionFragmentResourceLimit
        }
        ProductionSemanticDebugFragmentErrorV1::AllocationFailure => {
            SourceIsaObservationErrorCodeV1::ProductionFragmentAllocationFailure
        }
    }
}

const fn map_semantic_anchor_error(
    error: ProductionSemanticAnchorErrorV1,
) -> SourceIsaObservationErrorCodeV1 {
    match error {
        ProductionSemanticAnchorErrorV1::InvalidCompilerAttachment => {
            SourceIsaObservationErrorCodeV1::SemanticAnchorInvalidCompilerAttachment
        }
        ProductionSemanticAnchorErrorV1::InvalidProductionAssociation => {
            SourceIsaObservationErrorCodeV1::SemanticAnchorInvalidProductionAssociation
        }
        ProductionSemanticAnchorErrorV1::InvalidKirToLlvmReplay => {
            SourceIsaObservationErrorCodeV1::SemanticAnchorInvalidKirToLlvmReplay
        }
        ProductionSemanticAnchorErrorV1::TargetMismatch => {
            SourceIsaObservationErrorCodeV1::SemanticAnchorTargetMismatch
        }
        ProductionSemanticAnchorErrorV1::InvalidLlvm => {
            SourceIsaObservationErrorCodeV1::SemanticAnchorInvalidLlvm
        }
        ProductionSemanticAnchorErrorV1::ContradictoryLlvm => {
            SourceIsaObservationErrorCodeV1::SemanticAnchorContradictoryLlvm
        }
        ProductionSemanticAnchorErrorV1::BindingMismatch => {
            SourceIsaObservationErrorCodeV1::SemanticAnchorBindingMismatch
        }
        ProductionSemanticAnchorErrorV1::KirCoordinateMismatch => {
            SourceIsaObservationErrorCodeV1::SemanticAnchorKirCoordinateMismatch
        }
        ProductionSemanticAnchorErrorV1::KirToLlvmAnchorMismatch => {
            SourceIsaObservationErrorCodeV1::SemanticAnchorKirToLlvmAnchorMismatch
        }
        ProductionSemanticAnchorErrorV1::InvalidArtifact => {
            SourceIsaObservationErrorCodeV1::SemanticAnchorInvalidArtifact
        }
        ProductionSemanticAnchorErrorV1::MissingProbeSection => {
            SourceIsaObservationErrorCodeV1::SemanticAnchorMissingProbeSection
        }
        ProductionSemanticAnchorErrorV1::AmbiguousProbeSection => {
            SourceIsaObservationErrorCodeV1::SemanticAnchorAmbiguousProbeSection
        }
        ProductionSemanticAnchorErrorV1::InvalidProbeEncoding => {
            SourceIsaObservationErrorCodeV1::SemanticAnchorInvalidProbeEncoding
        }
        ProductionSemanticAnchorErrorV1::ProbeDescriptorMismatch => {
            SourceIsaObservationErrorCodeV1::SemanticAnchorProbeDescriptorMismatch
        }
        ProductionSemanticAnchorErrorV1::AmbiguousEntrySymbol => {
            SourceIsaObservationErrorCodeV1::SemanticAnchorAmbiguousEntrySymbol
        }
        ProductionSemanticAnchorErrorV1::UnexpectedProbe => {
            SourceIsaObservationErrorCodeV1::SemanticAnchorUnexpectedProbe
        }
        ProductionSemanticAnchorErrorV1::ProbeOutsideKernel => {
            SourceIsaObservationErrorCodeV1::SemanticAnchorProbeOutsideKernel
        }
        ProductionSemanticAnchorErrorV1::ResourceLimit => {
            SourceIsaObservationErrorCodeV1::SemanticAnchorResourceLimit
        }
        ProductionSemanticAnchorErrorV1::AllocationFailure => {
            SourceIsaObservationErrorCodeV1::SemanticAnchorAllocationFailure
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use fe2o3_artifact_transaction::{BuildInvocation, BuildSession};

    fn attempt(generation: u64, session: [u8; 16], invocation: [u8; 32]) -> BuildAttempt {
        BuildAttempt::from_env_value(&format!(
            "{generation}:{}:{}",
            BuildSession::from_bytes(session),
            BuildInvocation::from_bytes(invocation)
        ))
        .expect("valid producer attempt")
    }

    fn frame(outcome: SourceIsaObservationOutcomeV1) -> SourceIsaObservationFrameV1 {
        SourceIsaObservationFrameV1::new(
            SourceIsaObservationContextV1::new(
                [0x11; 32],
                [0x12; 32],
                inert_source_isa_attempt_v1(attempt(7, [0x13; 16], [0x14; 32])).unwrap(),
                [0x15; 32],
            )
            .unwrap(),
            outcome,
        )
    }

    #[test]
    fn characteristic_broker_v3_config_identity_is_frozen() {
        let identity = source_isa_characteristic_broker_config_identity_v3();
        let encoded = identity
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>();
        assert_eq!(
            encoded,
            "d8cda5df0538ddd552b4b93bff3d8f1b9fefc379a0e941e271f0ca508e51ae74"
        );
    }

    #[test]
    fn all_correlation_errors_map_to_distinct_canonical_codes() {
        fn assert_code(
            error: ProductionSourceIsaCorrelationErrorV1,
            expected: SourceIsaObservationErrorCodeV1,
        ) {
            let actual = map_correlation_error(error);
            assert_eq!(actual, expected);
            let expected = frame(SourceIsaObservationOutcomeV1::Error(actual));
            let encoded = expected.encode();
            assert_eq!(
                u16::from_le_bytes(encoded[169..171].try_into().unwrap()),
                actual as u16
            );
            assert!(encoded[176..648].iter().all(|byte| *byte == 0));
            assert_eq!(SourceIsaObservationFrameV1::decode(&encoded), Ok(expected));
        }

        for (error, code) in [
            (
                ProductionSourceIsaCorrelationErrorV1::InvalidKirToLlvmReplay,
                SourceIsaObservationErrorCodeV1::InvalidKirToLlvmReplay,
            ),
            (
                ProductionSourceIsaCorrelationErrorV1::NonExactSemanticMap,
                SourceIsaObservationErrorCodeV1::NonExactSemanticMap,
            ),
            (
                ProductionSourceIsaCorrelationErrorV1::ArtifactIdentityMismatch,
                SourceIsaObservationErrorCodeV1::ArtifactIdentityMismatch,
            ),
            (
                ProductionSourceIsaCorrelationErrorV1::TargetKirIdentityMismatch,
                SourceIsaObservationErrorCodeV1::TargetKirIdentityMismatch,
            ),
            (
                ProductionSourceIsaCorrelationErrorV1::CoordinateShapeMismatch,
                SourceIsaObservationErrorCodeV1::CoordinateShapeMismatch,
            ),
            (
                ProductionSourceIsaCorrelationErrorV1::InvalidSourceGraph,
                SourceIsaObservationErrorCodeV1::InvalidSourceGraph,
            ),
            (
                ProductionSourceIsaCorrelationErrorV1::ResourceLimit,
                SourceIsaObservationErrorCodeV1::ResourceLimit,
            ),
            (
                ProductionSourceIsaCorrelationErrorV1::AllocationFailure,
                SourceIsaObservationErrorCodeV1::AllocationFailure,
            ),
        ] {
            assert_code(error, code);
        }

        for (error, code) in [
            (
                FinalizedSemanticDebugMapErrorV1::ProductionAssociation,
                SourceIsaObservationErrorCodeV1::FinalizedMapProductionAssociation,
            ),
            (
                FinalizedSemanticDebugMapErrorV1::ProductionAssociationMismatch,
                SourceIsaObservationErrorCodeV1::FinalizedMapProductionAssociationMismatch,
            ),
            (
                FinalizedSemanticDebugMapErrorV1::InvalidKirToLlvmReplay,
                SourceIsaObservationErrorCodeV1::FinalizedMapInvalidKirToLlvmReplay,
            ),
            (
                FinalizedSemanticDebugMapErrorV1::KirToLlvmReplayTargetMismatch,
                SourceIsaObservationErrorCodeV1::FinalizedMapKirToLlvmReplayTargetMismatch,
            ),
            (
                FinalizedSemanticDebugMapErrorV1::InvalidLlvmToHsacoCustody,
                SourceIsaObservationErrorCodeV1::FinalizedMapInvalidLlvmToHsacoCustody,
            ),
            (
                FinalizedSemanticDebugMapErrorV1::InvalidBoundSourceMap,
                SourceIsaObservationErrorCodeV1::FinalizedMapInvalidBoundSourceMap,
            ),
            (
                FinalizedSemanticDebugMapErrorV1::InvalidBoundSemanticMir,
                SourceIsaObservationErrorCodeV1::FinalizedMapInvalidBoundSemanticMir,
            ),
            (
                FinalizedSemanticDebugMapErrorV1::InvalidBoundCorrespondenceV4,
                SourceIsaObservationErrorCodeV1::FinalizedMapInvalidBoundCorrespondenceV4,
            ),
            (
                FinalizedSemanticDebugMapErrorV1::InvalidBoundCanonicalKirV8,
                SourceIsaObservationErrorCodeV1::FinalizedMapInvalidBoundCanonicalKirV8,
            ),
            (
                FinalizedSemanticDebugMapErrorV1::InvalidBoundCanonicalKirV7,
                SourceIsaObservationErrorCodeV1::FinalizedMapInvalidBoundCanonicalKirV7,
            ),
            (
                FinalizedSemanticDebugMapErrorV1::CanonicalKirProjectionMismatch,
                SourceIsaObservationErrorCodeV1::FinalizedMapCanonicalKirProjectionMismatch,
            ),
            (
                FinalizedSemanticDebugMapErrorV1::CorrespondenceIdentityMismatch,
                SourceIsaObservationErrorCodeV1::FinalizedMapCorrespondenceIdentityMismatch,
            ),
            (
                FinalizedSemanticDebugMapErrorV1::InvalidSemanticCorrespondence,
                SourceIsaObservationErrorCodeV1::FinalizedMapInvalidSemanticCorrespondence,
            ),
            (
                FinalizedSemanticDebugMapErrorV1::ArtifactInspection,
                SourceIsaObservationErrorCodeV1::FinalizedMapArtifactInspection,
            ),
            (
                FinalizedSemanticDebugMapErrorV1::AllocationFailure,
                SourceIsaObservationErrorCodeV1::FinalizedMapAllocationFailure,
            ),
        ] {
            assert_code(
                ProductionSourceIsaCorrelationErrorV1::SemanticDebugMap(error),
                code,
            );
        }

        for (error, code) in [
            (
                SemanticDebugMapErrorV1::InvalidLength,
                SourceIsaObservationErrorCodeV1::SemanticMapInvalidLength,
            ),
            (
                SemanticDebugMapErrorV1::InvalidJson,
                SourceIsaObservationErrorCodeV1::SemanticMapInvalidJson,
            ),
            (
                SemanticDebugMapErrorV1::NonCanonicalEncoding,
                SourceIsaObservationErrorCodeV1::SemanticMapNonCanonicalEncoding,
            ),
            (
                SemanticDebugMapErrorV1::Encoding,
                SourceIsaObservationErrorCodeV1::SemanticMapEncoding,
            ),
            (
                SemanticDebugMapErrorV1::InvalidBinding,
                SourceIsaObservationErrorCodeV1::SemanticMapInvalidBinding,
            ),
            (
                SemanticDebugMapErrorV1::InvalidKernelOrdinalBasis,
                SourceIsaObservationErrorCodeV1::SemanticMapInvalidKernelOrdinalBasis,
            ),
            (
                SemanticDebugMapErrorV1::InvalidNode,
                SourceIsaObservationErrorCodeV1::SemanticMapInvalidNode,
            ),
            (
                SemanticDebugMapErrorV1::InvalidMapping,
                SourceIsaObservationErrorCodeV1::SemanticMapInvalidMapping,
            ),
            (
                SemanticDebugMapErrorV1::DuplicateNode,
                SourceIsaObservationErrorCodeV1::SemanticMapDuplicateNode,
            ),
            (
                SemanticDebugMapErrorV1::DuplicateMapping,
                SourceIsaObservationErrorCodeV1::SemanticMapDuplicateMapping,
            ),
            (
                SemanticDebugMapErrorV1::DuplicateReference,
                SourceIsaObservationErrorCodeV1::SemanticMapDuplicateReference,
            ),
            (
                SemanticDebugMapErrorV1::UnknownNode,
                SourceIsaObservationErrorCodeV1::SemanticMapUnknownNode,
            ),
            (
                SemanticDebugMapErrorV1::LayerMismatch,
                SourceIsaObservationErrorCodeV1::SemanticMapLayerMismatch,
            ),
            (
                SemanticDebugMapErrorV1::ContradictoryMapping,
                SourceIsaObservationErrorCodeV1::SemanticMapContradictoryMapping,
            ),
            (
                SemanticDebugMapErrorV1::OrphanNode,
                SourceIsaObservationErrorCodeV1::SemanticMapOrphanNode,
            ),
            (
                SemanticDebugMapErrorV1::InvalidBoundary,
                SourceIsaObservationErrorCodeV1::SemanticMapInvalidBoundary,
            ),
            (
                SemanticDebugMapErrorV1::UntypedBoundary,
                SourceIsaObservationErrorCodeV1::SemanticMapUntypedBoundary,
            ),
            (
                SemanticDebugMapErrorV1::ResourceLimit,
                SourceIsaObservationErrorCodeV1::SemanticMapResourceLimit,
            ),
            (
                SemanticDebugMapErrorV1::AllocationFailure,
                SourceIsaObservationErrorCodeV1::SemanticMapAllocationFailure,
            ),
            (
                SemanticDebugMapErrorV1::ContentBindingMismatch,
                SourceIsaObservationErrorCodeV1::SemanticMapContentBindingMismatch,
            ),
            (
                SemanticDebugMapErrorV1::ArtifactBindingMismatch,
                SourceIsaObservationErrorCodeV1::SemanticMapArtifactBindingMismatch,
            ),
            (
                SemanticDebugMapErrorV1::InvalidBoundSourceMap,
                SourceIsaObservationErrorCodeV1::SemanticMapInvalidBoundSourceMap,
            ),
            (
                SemanticDebugMapErrorV1::InvalidBoundCanonicalKir,
                SourceIsaObservationErrorCodeV1::SemanticMapInvalidBoundCanonicalKir,
            ),
            (
                SemanticDebugMapErrorV1::SourceMapKirBindingMismatch,
                SourceIsaObservationErrorCodeV1::SemanticMapSourceMapKirBindingMismatch,
            ),
            (
                SemanticDebugMapErrorV1::InvalidSourceLocation,
                SourceIsaObservationErrorCodeV1::SemanticMapInvalidSourceLocation,
            ),
            (
                SemanticDebugMapErrorV1::InvalidMirLocation,
                SourceIsaObservationErrorCodeV1::SemanticMapInvalidMirLocation,
            ),
            (
                SemanticDebugMapErrorV1::InvalidKirLocation,
                SourceIsaObservationErrorCodeV1::SemanticMapInvalidKirLocation,
            ),
            (
                SemanticDebugMapErrorV1::InvalidIsaInterval,
                SourceIsaObservationErrorCodeV1::SemanticMapInvalidIsaInterval,
            ),
        ] {
            assert_code(
                ProductionSourceIsaCorrelationErrorV1::SemanticDebugMap(
                    FinalizedSemanticDebugMapErrorV1::SemanticMap(error),
                ),
                code,
            );
        }

        for (error, code) in [
            (
                ProductionSemanticDebugFragmentErrorV1::InvalidEncoding,
                SourceIsaObservationErrorCodeV1::ProductionFragmentInvalidEncoding,
            ),
            (
                ProductionSemanticDebugFragmentErrorV1::InvalidAssociation,
                SourceIsaObservationErrorCodeV1::ProductionFragmentInvalidAssociation,
            ),
            (
                ProductionSemanticDebugFragmentErrorV1::InvalidGap,
                SourceIsaObservationErrorCodeV1::ProductionFragmentInvalidGap,
            ),
            (
                ProductionSemanticDebugFragmentErrorV1::InvalidScheduleStatus,
                SourceIsaObservationErrorCodeV1::ProductionFragmentInvalidScheduleStatus,
            ),
            (
                ProductionSemanticDebugFragmentErrorV1::InvalidSourceMap,
                SourceIsaObservationErrorCodeV1::ProductionFragmentInvalidSourceMap,
            ),
            (
                ProductionSemanticDebugFragmentErrorV1::InvalidCanonicalKir,
                SourceIsaObservationErrorCodeV1::ProductionFragmentInvalidCanonicalKir,
            ),
            (
                ProductionSemanticDebugFragmentErrorV1::InvalidSemanticMap,
                SourceIsaObservationErrorCodeV1::ProductionFragmentInvalidSemanticMap,
            ),
            (
                ProductionSemanticDebugFragmentErrorV1::AxisMismatch,
                SourceIsaObservationErrorCodeV1::ProductionFragmentAxisMismatch,
            ),
            (
                ProductionSemanticDebugFragmentErrorV1::ResourceLimit,
                SourceIsaObservationErrorCodeV1::ProductionFragmentResourceLimit,
            ),
            (
                ProductionSemanticDebugFragmentErrorV1::AllocationFailure,
                SourceIsaObservationErrorCodeV1::ProductionFragmentAllocationFailure,
            ),
        ] {
            assert_code(
                ProductionSourceIsaCorrelationErrorV1::SemanticDebugMap(
                    FinalizedSemanticDebugMapErrorV1::ProductionFragment(error),
                ),
                code,
            );
        }

        for (error, code) in [
            (
                ProductionSemanticAnchorErrorV1::InvalidCompilerAttachment,
                SourceIsaObservationErrorCodeV1::SemanticAnchorInvalidCompilerAttachment,
            ),
            (
                ProductionSemanticAnchorErrorV1::InvalidProductionAssociation,
                SourceIsaObservationErrorCodeV1::SemanticAnchorInvalidProductionAssociation,
            ),
            (
                ProductionSemanticAnchorErrorV1::InvalidKirToLlvmReplay,
                SourceIsaObservationErrorCodeV1::SemanticAnchorInvalidKirToLlvmReplay,
            ),
            (
                ProductionSemanticAnchorErrorV1::TargetMismatch,
                SourceIsaObservationErrorCodeV1::SemanticAnchorTargetMismatch,
            ),
            (
                ProductionSemanticAnchorErrorV1::InvalidLlvm,
                SourceIsaObservationErrorCodeV1::SemanticAnchorInvalidLlvm,
            ),
            (
                ProductionSemanticAnchorErrorV1::ContradictoryLlvm,
                SourceIsaObservationErrorCodeV1::SemanticAnchorContradictoryLlvm,
            ),
            (
                ProductionSemanticAnchorErrorV1::BindingMismatch,
                SourceIsaObservationErrorCodeV1::SemanticAnchorBindingMismatch,
            ),
            (
                ProductionSemanticAnchorErrorV1::KirCoordinateMismatch,
                SourceIsaObservationErrorCodeV1::SemanticAnchorKirCoordinateMismatch,
            ),
            (
                ProductionSemanticAnchorErrorV1::KirToLlvmAnchorMismatch,
                SourceIsaObservationErrorCodeV1::SemanticAnchorKirToLlvmAnchorMismatch,
            ),
            (
                ProductionSemanticAnchorErrorV1::InvalidArtifact,
                SourceIsaObservationErrorCodeV1::SemanticAnchorInvalidArtifact,
            ),
            (
                ProductionSemanticAnchorErrorV1::MissingProbeSection,
                SourceIsaObservationErrorCodeV1::SemanticAnchorMissingProbeSection,
            ),
            (
                ProductionSemanticAnchorErrorV1::AmbiguousProbeSection,
                SourceIsaObservationErrorCodeV1::SemanticAnchorAmbiguousProbeSection,
            ),
            (
                ProductionSemanticAnchorErrorV1::InvalidProbeEncoding,
                SourceIsaObservationErrorCodeV1::SemanticAnchorInvalidProbeEncoding,
            ),
            (
                ProductionSemanticAnchorErrorV1::ProbeDescriptorMismatch,
                SourceIsaObservationErrorCodeV1::SemanticAnchorProbeDescriptorMismatch,
            ),
            (
                ProductionSemanticAnchorErrorV1::AmbiguousEntrySymbol,
                SourceIsaObservationErrorCodeV1::SemanticAnchorAmbiguousEntrySymbol,
            ),
            (
                ProductionSemanticAnchorErrorV1::UnexpectedProbe,
                SourceIsaObservationErrorCodeV1::SemanticAnchorUnexpectedProbe,
            ),
            (
                ProductionSemanticAnchorErrorV1::ProbeOutsideKernel,
                SourceIsaObservationErrorCodeV1::SemanticAnchorProbeOutsideKernel,
            ),
            (
                ProductionSemanticAnchorErrorV1::ResourceLimit,
                SourceIsaObservationErrorCodeV1::SemanticAnchorResourceLimit,
            ),
            (
                ProductionSemanticAnchorErrorV1::AllocationFailure,
                SourceIsaObservationErrorCodeV1::SemanticAnchorAllocationFailure,
            ),
        ] {
            assert_code(
                ProductionSourceIsaCorrelationErrorV1::SemanticAnchors(error),
                code,
            );
        }
    }

    #[test]
    fn ready_state_has_a_typed_canonical_unavailable_frame() {
        let attempt = attempt(7, [0x31; 16], [0x32; 32]);
        let expected =
            ready_source_isa_observation_frame_v1([0x30; 32], [0x40; 32], attempt, [0x33; 32])
                .unwrap();
        assert_eq!(
            expected.outcome(),
            SourceIsaObservationOutcomeV1::Unavailable(
                SourceIsaObservationUnavailableReasonV1::FinalizedEvidenceUnavailableFromReadyState
            )
        );
        let encoded = expected.encode();
        assert_eq!(encoded.len(), SOURCE_ISA_OBSERVATION_FRAME_BYTES_V1);
        assert!(encoded[176..640].iter().all(|byte| *byte == 0));
        assert_eq!(SourceIsaObservationFrameV1::decode(&encoded), Ok(expected));
    }

    #[test]
    fn unavailable_mapping_is_exact_and_exhaustive() {
        let carrier = [
            (ProductionSemanticDebugProducerGapV1::MultipleKirFunctionBodies, SourceIsaObservationUnavailableReasonV1::CarrierMultipleKirFunctionBodies),
            (ProductionSemanticDebugProducerGapV1::NoStatementCorrespondence, SourceIsaObservationUnavailableReasonV1::CarrierNoStatementCorrespondence),
            (ProductionSemanticDebugProducerGapV1::SourceMapUnavailable, SourceIsaObservationUnavailableReasonV1::CarrierSourceMapUnavailable),
            (ProductionSemanticDebugProducerGapV1::ResourceLimit, SourceIsaObservationUnavailableReasonV1::CarrierResourceLimit),
            (ProductionSemanticDebugProducerGapV1::CanonicalKirV7ProjectionUnavailable, SourceIsaObservationUnavailableReasonV1::CarrierCanonicalKirV7ProjectionUnavailable),
            (ProductionSemanticDebugProducerGapV1::SourceObservationUnrepresentable, SourceIsaObservationUnavailableReasonV1::CarrierSourceObservationUnrepresentable),
            (ProductionSemanticDebugProducerGapV1::SemanticMapConstructionUnavailable, SourceIsaObservationUnavailableReasonV1::CarrierSemanticMapConstructionUnavailable),
            (ProductionSemanticDebugProducerGapV1::SemanticMapEncodingUnavailable, SourceIsaObservationUnavailableReasonV1::CarrierSemanticMapEncodingUnavailable),
            (ProductionSemanticDebugProducerGapV1::FragmentConstructionUnavailable, SourceIsaObservationUnavailableReasonV1::CarrierFragmentConstructionUnavailable),
            (ProductionSemanticDebugProducerGapV1::CarrierConstructionUnavailable, SourceIsaObservationUnavailableReasonV1::CarrierConstructionUnavailable),
            (ProductionSemanticDebugProducerGapV1::ReceiptExtensionConstructionUnavailable, SourceIsaObservationUnavailableReasonV1::CarrierReceiptExtensionConstructionUnavailable),
            (ProductionSemanticDebugProducerGapV1::CorrespondenceValidationUnavailable, SourceIsaObservationUnavailableReasonV1::CarrierCorrespondenceValidationUnavailable),
            (ProductionSemanticDebugProducerGapV1::CanonicalKirModuleMismatch, SourceIsaObservationUnavailableReasonV1::CarrierCanonicalKirModuleMismatch),
            (ProductionSemanticDebugProducerGapV1::LegacyBareAssociationNoAttachment, SourceIsaObservationUnavailableReasonV1::CarrierLegacyBareAssociationNoAttachment),
        ];
        for (input, expected) in carrier {
            assert_eq!(
                map_unavailable_reason(
                    ProductionSourceIsaCorrelationUnavailableV1::SemanticDebugCarrier(input)
                ),
                expected
            );
        }
        let anchors = [
            (
                ProductionSemanticAnchorUnavailableV1::LegacySemanticAttachment,
                SourceIsaObservationUnavailableReasonV1::AnchorLegacySemanticAttachment,
            ),
            (
                ProductionSemanticAnchorUnavailableV1::LegacyUninstrumentedReplay,
                SourceIsaObservationUnavailableReasonV1::AnchorLegacyUninstrumentedReplay,
            ),
            (
                ProductionSemanticAnchorUnavailableV1::NoOperations,
                SourceIsaObservationUnavailableReasonV1::AnchorNoOperations,
            ),
            (
                ProductionSemanticAnchorUnavailableV1::MultipleDefinedBodies,
                SourceIsaObservationUnavailableReasonV1::AnchorMultipleDefinedBodies,
            ),
            (
                ProductionSemanticAnchorUnavailableV1::CompilerInstrumentationAbsent,
                SourceIsaObservationUnavailableReasonV1::AnchorCompilerInstrumentationAbsent,
            ),
        ];
        for (input, expected) in anchors {
            assert_eq!(
                map_unavailable_reason(
                    ProductionSourceIsaCorrelationUnavailableV1::SemanticAnchors(input)
                ),
                expected
            );
        }
        assert_eq!(
            map_unavailable_reason(
                ProductionSourceIsaCorrelationUnavailableV1::SourceProjectionForKirV9
            ),
            SourceIsaObservationUnavailableReasonV1::SourceProjectionForKirV9
        );
    }
}

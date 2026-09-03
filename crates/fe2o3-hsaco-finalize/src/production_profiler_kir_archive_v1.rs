//! Self-contained replay archive for authority-free production profiler KIR queries.

use std::{collections::BTreeSet, error::Error, fmt, ops::Range};

use fe2o3_artifact_transaction::{BuildAttempt, BuildInvocation, BuildSession};
use fe2o3_compiler_ffi::MAX_INERT_SEMANTIC_COMPILER_MODULE_HANDOFF_BYTES_V3;
use fe2o3_hsaco::MAX_HSACO_BYTES;
use fe2o3_kernel_ir::{
    ProductionSemanticDebugAvailabilityV1, ProductionSemanticDebugProducerGapV1,
    ProductionSemanticDebugReceiptExtensionV1,
};
use sha2::{Digest, Sha256};

use crate::{
    MAX_LINK_INPUTS, MAX_PROTECTED_WORKER_V3_COMPACT_FINALIZER_REPLAY_BYTES_V1,
    MAX_WORKER_TOTAL_INPUT_BYTES, PreparedFinalizedProtectedWorkerV3HsacoV1,
    ProductionKirV7BridgeAdmissionV1, ProductionKirV7BridgeUnavailableV1,
    ProductionKirV7StructuralBridgeV1, ProductionSemanticAnchorUnavailableV1,
    ProductionSourceIsaCatalogAdmissionV1, ProductionSourceIsaCatalogV1,
    ProductionSourceIsaCharacteristicAdmissionV1, ProductionSourceIsaCharacteristicCollectionV1,
    ProductionSourceIsaCharacteristicUnavailableV1, ProductionSourceIsaCorrelationUnavailableV1,
    ProtectedWorkerV3CompactFinalizerReplayPartsV2, admit_production_kir_v7_structural_bridge_v1,
    admit_production_source_isa_characteristics_v1,
    prepare_protected_worker_v3_compact_finalizer_replay_v2,
    worker_v3_hsaco_publication::revalidate_protected_worker_v3_finalizer_for_structural_archive_v1,
};

pub const PRODUCTION_PROFILER_KIR_ARCHIVE_MAGIC_V1: &[u8; 8] = b"F2P3KRA1";
pub const PRODUCTION_PROFILER_KIR_ARCHIVE_VERSION_V1: u16 = 1;

const ARCHIVE_CHECKSUM_DOMAIN_V1: &[u8] = b"FE2O3/PRODUCTION-PROFILER-KIR-ARCHIVE-CHECKSUM/V1\0";
const ARCHIVE_IDENTITY_DOMAIN_V1: &[u8] = b"FE2O3/PRODUCTION-PROFILER-KIR-ARCHIVE-IDENTITY/V1\0";
const ARCHIVE_HEADER_BYTES_V1: usize = 8 + 2 + 2 + 8 + 8 + 16 + 32 + 2 + 2;
const ARCHIVE_SECTION_HEADER_BYTES_V1: usize = 1 + 1 + 2 + 8;
const ARCHIVE_CHECKSUM_BYTES_V1: usize = 32;
const ARCHIVE_REQUIRED_SECTIONS_V1: usize = 3;
const SECTION_OUTER_HANDOFF_V1: u8 = 1;
const SECTION_EXTERNAL_PROVIDER_V1: u8 = 2;
const SECTION_COMPACT_TRANSCRIPT_V1: u8 = 3;
const SECTION_FINALIZED_HSACO_V1: u8 = 4;

/// Maximum canonical archive bytes across every upstream replay attachment bound.
pub const MAX_PRODUCTION_PROFILER_KIR_ARCHIVE_BYTES_V1: usize = ARCHIVE_HEADER_BYTES_V1
    + ((MAX_LINK_INPUTS + ARCHIVE_REQUIRED_SECTIONS_V1) * ARCHIVE_SECTION_HEADER_BYTES_V1)
    + MAX_INERT_SEMANTIC_COMPILER_MODULE_HANDOFF_BYTES_V3
    + MAX_WORKER_TOTAL_INPUT_BYTES
    + MAX_PROTECTED_WORKER_V3_COMPACT_FINALIZER_REPLAY_BYTES_V1
    + MAX_HSACO_BYTES
    + ARCHIVE_CHECKSUM_BYTES_V1;

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ProductionProfilerKirArchiveIdentityV1([u8; 32]);

impl ProductionProfilerKirArchiveIdentityV1 {
    pub const fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ProductionProfilerKirArchiveUnavailableV1 {
    SemanticDebugCarrier(ProductionSemanticDebugProducerGapV1),
    SourceIsaCatalog(ProductionSourceIsaCorrelationUnavailableV1),
    StructuralBridge(ProductionKirV7BridgeUnavailableV1),
    CharacteristicProjection(ProductionSourceIsaCharacteristicUnavailableV1),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProductionProfilerKirArchiveUnavailableClassV1 {
    SemanticDebugCarrier,
    SourceIsaCatalog,
    StructuralBridge,
    CharacteristicProjection,
}

impl ProductionProfilerKirArchiveUnavailableV1 {
    pub const fn class(&self) -> ProductionProfilerKirArchiveUnavailableClassV1 {
        match self {
            Self::SemanticDebugCarrier(_) => {
                ProductionProfilerKirArchiveUnavailableClassV1::SemanticDebugCarrier
            }
            Self::SourceIsaCatalog(_) => {
                ProductionProfilerKirArchiveUnavailableClassV1::SourceIsaCatalog
            }
            Self::StructuralBridge(_) => {
                ProductionProfilerKirArchiveUnavailableClassV1::StructuralBridge
            }
            Self::CharacteristicProjection(_) => {
                ProductionProfilerKirArchiveUnavailableClassV1::CharacteristicProjection
            }
        }
    }

    pub const fn reason_code(&self) -> &'static str {
        match self {
            Self::SemanticDebugCarrier(reason) => semantic_debug_gap_code(*reason),
            Self::SourceIsaCatalog(reason) => source_isa_unavailable_code(*reason),
            Self::StructuralBridge(reason) => match reason {
                ProductionKirV7BridgeUnavailableV1::NonIdentityStructuralProjectionUnavailable => {
                    "non_identity_structural_projection_unavailable"
                }
                ProductionKirV7BridgeUnavailableV1::SiteCatalogLimit => "site_catalog_limit",
            },
            Self::CharacteristicProjection(reason) => match reason {
                ProductionSourceIsaCharacteristicUnavailableV1::CharacteristicLimit => {
                    "characteristic_limit"
                }
                ProductionSourceIsaCharacteristicUnavailableV1::CorrelationPerWitnessLimit => {
                    "correlation_per_witness_limit"
                }
                ProductionSourceIsaCharacteristicUnavailableV1::TotalCorrelationLimit => {
                    "total_correlation_limit"
                }
                ProductionSourceIsaCharacteristicUnavailableV1::EliminationFactLimit => {
                    "elimination_fact_limit"
                }
            },
        }
    }
}

const fn source_isa_unavailable_code(
    reason: ProductionSourceIsaCorrelationUnavailableV1,
) -> &'static str {
    match reason {
        ProductionSourceIsaCorrelationUnavailableV1::SemanticDebugCarrier(reason) => {
            semantic_debug_gap_code(reason)
        }
        ProductionSourceIsaCorrelationUnavailableV1::SemanticAnchors(reason) => match reason {
            ProductionSemanticAnchorUnavailableV1::LegacySemanticAttachment => {
                "semantic_anchors_legacy_semantic_attachment"
            }
            ProductionSemanticAnchorUnavailableV1::LegacyUninstrumentedReplay => {
                "semantic_anchors_legacy_uninstrumented_replay"
            }
            ProductionSemanticAnchorUnavailableV1::NoOperations => "semantic_anchors_no_operations",
            ProductionSemanticAnchorUnavailableV1::MultipleDefinedBodies => {
                "semantic_anchors_multiple_defined_bodies"
            }
            ProductionSemanticAnchorUnavailableV1::CompilerInstrumentationAbsent => {
                "semantic_anchors_compiler_instrumentation_absent"
            }
        },
        ProductionSourceIsaCorrelationUnavailableV1::SourceProjectionForKirV9 => {
            "source_projection_for_kir_v9"
        }
    }
}

const fn semantic_debug_gap_code(reason: ProductionSemanticDebugProducerGapV1) -> &'static str {
    match reason {
        ProductionSemanticDebugProducerGapV1::MultipleKirFunctionBodies => {
            "semantic_debug_multiple_kir_function_bodies"
        }
        ProductionSemanticDebugProducerGapV1::NoStatementCorrespondence => {
            "semantic_debug_no_statement_correspondence"
        }
        ProductionSemanticDebugProducerGapV1::SourceMapUnavailable => {
            "semantic_debug_source_map_unavailable"
        }
        ProductionSemanticDebugProducerGapV1::ResourceLimit => "semantic_debug_resource_limit",
        ProductionSemanticDebugProducerGapV1::CanonicalKirV7ProjectionUnavailable => {
            "semantic_debug_canonical_kir_v7_projection_unavailable"
        }
        ProductionSemanticDebugProducerGapV1::SourceObservationUnrepresentable => {
            "semantic_debug_source_observation_unrepresentable"
        }
        ProductionSemanticDebugProducerGapV1::SemanticMapConstructionUnavailable => {
            "semantic_debug_map_construction_unavailable"
        }
        ProductionSemanticDebugProducerGapV1::SemanticMapEncodingUnavailable => {
            "semantic_debug_map_encoding_unavailable"
        }
        ProductionSemanticDebugProducerGapV1::FragmentConstructionUnavailable => {
            "semantic_debug_fragment_construction_unavailable"
        }
        ProductionSemanticDebugProducerGapV1::CarrierConstructionUnavailable => {
            "semantic_debug_carrier_construction_unavailable"
        }
        ProductionSemanticDebugProducerGapV1::ReceiptExtensionConstructionUnavailable => {
            "semantic_debug_receipt_extension_construction_unavailable"
        }
        ProductionSemanticDebugProducerGapV1::CorrespondenceValidationUnavailable => {
            "semantic_debug_correspondence_validation_unavailable"
        }
        ProductionSemanticDebugProducerGapV1::CanonicalKirModuleMismatch => {
            "semantic_debug_canonical_kir_module_mismatch"
        }
        ProductionSemanticDebugProducerGapV1::LegacyBareAssociationNoAttachment => {
            "semantic_debug_legacy_bare_association_no_attachment"
        }
    }
}

#[derive(Debug)]
#[allow(
    clippy::large_enum_variant,
    reason = "the admitted variant owns one bounded collection; boxing would add an infallible allocation"
)]
pub enum ProductionProfilerKirArchiveAdmissionV1 {
    Admitted(AdmittedProductionProfilerKirArchiveV1),
    Unavailable(ProductionProfilerKirArchiveUnavailableV1),
}

/// Exact replay bytes prepared from an already-admitted finalizer owner.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PreparedProductionProfilerKirArchiveV1 {
    identity: ProductionProfilerKirArchiveIdentityV1,
    attempt: BuildAttempt,
    canonical_bytes: Vec<u8>,
}

impl PreparedProductionProfilerKirArchiveV1 {
    pub const fn identity(&self) -> ProductionProfilerKirArchiveIdentityV1 {
        self.identity
    }

    pub const fn attempt(&self) -> BuildAttempt {
        self.attempt
    }

    pub fn canonical_bytes(&self) -> &[u8] {
        &self.canonical_bytes
    }

    pub fn into_canonical_bytes(self) -> Vec<u8> {
        self.canonical_bytes
    }

    pub const fn authenticates_external_provenance(&self) -> bool {
        false
    }

    pub const fn grants_runtime_authority(&self) -> bool {
        false
    }
}

/// Strictly decoded bytes. No structural query owner exists until exact replay succeeds.
#[derive(Debug)]
pub struct InertProductionProfilerKirArchiveV1 {
    identity: ProductionProfilerKirArchiveIdentityV1,
    attempt: BuildAttempt,
    outer_handoff: Range<usize>,
    external_providers: Vec<Range<usize>>,
    transcript: Range<usize>,
    finalized_hsaco: Range<usize>,
    canonical_bytes: Vec<u8>,
}

impl InertProductionProfilerKirArchiveV1 {
    pub fn decode_canonical(bytes: &[u8]) -> Result<Self, ProductionProfilerKirArchiveErrorV1> {
        if bytes.len() < ARCHIVE_HEADER_BYTES_V1 + ARCHIVE_CHECKSUM_BYTES_V1
            || bytes.len() > MAX_PRODUCTION_PROFILER_KIR_ARCHIVE_BYTES_V1
        {
            return Err(ProductionProfilerKirArchiveErrorV1::Length);
        }
        let mut owned = Vec::new();
        owned
            .try_reserve_exact(bytes.len())
            .map_err(|_| ProductionProfilerKirArchiveErrorV1::AllocationFailure)?;
        owned.extend_from_slice(bytes);
        Self::decode_owned_canonical(owned)
    }

    pub fn decode_owned_canonical(
        canonical_bytes: Vec<u8>,
    ) -> Result<Self, ProductionProfilerKirArchiveErrorV1> {
        decode_archive(canonical_bytes)
    }

    pub const fn identity(&self) -> ProductionProfilerKirArchiveIdentityV1 {
        self.identity
    }

    pub const fn attempt(&self) -> BuildAttempt {
        self.attempt
    }

    pub fn canonical_bytes(&self) -> &[u8] {
        &self.canonical_bytes
    }

    pub fn admit_exact_replay_v1(
        self,
    ) -> Result<ProductionProfilerKirArchiveAdmissionV1, ProductionProfilerKirArchiveErrorV1> {
        let providers = self
            .external_providers
            .iter()
            .map(|range| &self.canonical_bytes[range.clone()]);
        let finalized = revalidate_protected_worker_v3_finalizer_for_structural_archive_v1(
            self.attempt,
            &self.canonical_bytes[self.outer_handoff.clone()],
            providers,
            &self.canonical_bytes[self.transcript.clone()],
            &self.canonical_bytes[self.finalized_hsaco.clone()],
        )
        .map_err(|_| ProductionProfilerKirArchiveErrorV1::FinalizerReplay)?;
        let structural = derive_structural_evidence(&finalized)?;
        let StructuralEvidenceAdmissionV1::Admitted {
            catalog,
            bridge,
            characteristic,
        } = structural
        else {
            let StructuralEvidenceAdmissionV1::Unavailable(reason) = structural else {
                unreachable!()
            };
            return Ok(ProductionProfilerKirArchiveAdmissionV1::Unavailable(reason));
        };
        Ok(ProductionProfilerKirArchiveAdmissionV1::Admitted(
            AdmittedProductionProfilerKirArchiveV1 {
                identity: self.identity,
                canonical_len: self.canonical_bytes.len() as u64,
                attempt: self.attempt,
                catalog,
                bridge,
                characteristic,
            },
        ))
    }

    pub const fn authenticates_external_provenance(&self) -> bool {
        false
    }

    pub const fn grants_runtime_authority(&self) -> bool {
        false
    }
}

/// Replayed structural owners with no retained compiler, publication, load, or launch handle.
#[derive(Debug)]
pub struct AdmittedProductionProfilerKirArchiveV1 {
    identity: ProductionProfilerKirArchiveIdentityV1,
    canonical_len: u64,
    attempt: BuildAttempt,
    catalog: ProductionSourceIsaCatalogV1,
    bridge: ProductionKirV7StructuralBridgeV1,
    characteristic: ProductionSourceIsaCharacteristicCollectionV1,
}

impl AdmittedProductionProfilerKirArchiveV1 {
    pub const fn identity(&self) -> ProductionProfilerKirArchiveIdentityV1 {
        self.identity
    }

    pub const fn canonical_len(&self) -> u64 {
        self.canonical_len
    }

    pub const fn attempt(&self) -> BuildAttempt {
        self.attempt
    }

    pub const fn catalog(&self) -> &ProductionSourceIsaCatalogV1 {
        &self.catalog
    }

    pub const fn bridge(&self) -> &ProductionKirV7StructuralBridgeV1 {
        &self.bridge
    }

    pub const fn characteristic(&self) -> &ProductionSourceIsaCharacteristicCollectionV1 {
        &self.characteristic
    }

    pub const fn authenticates_external_provenance(&self) -> bool {
        false
    }

    pub const fn grants_compiler_authority(&self) -> bool {
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

    pub const fn grants_profiler_collection_authority(&self) -> bool {
        false
    }

    pub const fn grants_runtime_authority(&self) -> bool {
        false
    }
}

pub fn prepare_production_profiler_kir_archive_v1(
    finalized: PreparedFinalizedProtectedWorkerV3HsacoV1,
) -> Result<PreparedProductionProfilerKirArchiveV1, ProductionProfilerKirArchiveErrorV1> {
    let attempt = finalized.attempt();
    let parts = prepare_protected_worker_v3_compact_finalizer_replay_v2(finalized)
        .map_err(|_| ProductionProfilerKirArchiveErrorV1::CompactReplay)?
        .into_parts();
    let canonical_bytes = encode_archive(attempt, parts)?;
    let inert = InertProductionProfilerKirArchiveV1::decode_owned_canonical(canonical_bytes)?;
    Ok(PreparedProductionProfilerKirArchiveV1 {
        identity: inert.identity,
        attempt: inert.attempt,
        canonical_bytes: inert.canonical_bytes,
    })
}

#[derive(Debug)]
#[allow(
    clippy::large_enum_variant,
    reason = "temporary replay owners stay allocation-free until final admission"
)]
enum StructuralEvidenceAdmissionV1 {
    Admitted {
        catalog: ProductionSourceIsaCatalogV1,
        bridge: ProductionKirV7StructuralBridgeV1,
        characteristic: ProductionSourceIsaCharacteristicCollectionV1,
    },
    Unavailable(ProductionProfilerKirArchiveUnavailableV1),
}

fn derive_structural_evidence(
    finalized: &PreparedFinalizedProtectedWorkerV3HsacoV1,
) -> Result<StructuralEvidenceAdmissionV1, ProductionProfilerKirArchiveErrorV1> {
    let extension = ProductionSemanticDebugReceiptExtensionV1::from_canonical_bytes(
        finalized
            .outer_handoff()
            .capsule()
            .receipts()
            .semantic_to_llvm()
            .canonical_preimage(),
    )
    .map_err(|_| ProductionProfilerKirArchiveErrorV1::SemanticDebugReceipt)?;
    let fragment = match extension.carrier_v1().availability() {
        ProductionSemanticDebugAvailabilityV1::Available(fragment) => fragment,
        ProductionSemanticDebugAvailabilityV1::Unavailable(reason) => {
            return Ok(StructuralEvidenceAdmissionV1::Unavailable(
                ProductionProfilerKirArchiveUnavailableV1::SemanticDebugCarrier(*reason),
            ));
        }
    };
    let target_kir = finalized
        .outer_handoff()
        .capsule()
        .receipts()
        .kernel_ir()
        .canonical_preimage();
    let catalog = match finalized
        .admit_production_source_isa_catalog_v1()
        .map_err(|_| ProductionProfilerKirArchiveErrorV1::CatalogAdmission)?
    {
        ProductionSourceIsaCatalogAdmissionV1::Admitted(value) => value,
        ProductionSourceIsaCatalogAdmissionV1::Unavailable(reason) => {
            return Ok(StructuralEvidenceAdmissionV1::Unavailable(
                ProductionProfilerKirArchiveUnavailableV1::SourceIsaCatalog(reason),
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
    .map_err(|_| ProductionProfilerKirArchiveErrorV1::StructuralBridgeAdmission)?
    {
        ProductionKirV7BridgeAdmissionV1::Admitted(value) => value,
        ProductionKirV7BridgeAdmissionV1::Unavailable(reason) => {
            return Ok(StructuralEvidenceAdmissionV1::Unavailable(
                ProductionProfilerKirArchiveUnavailableV1::StructuralBridge(reason),
            ));
        }
    };
    let characteristic =
        match admit_production_source_isa_characteristics_v1(target_kir, &catalog, &bridge)
            .map_err(|_| ProductionProfilerKirArchiveErrorV1::CharacteristicAdmission)?
        {
            ProductionSourceIsaCharacteristicAdmissionV1::Admitted(value) => value,
            ProductionSourceIsaCharacteristicAdmissionV1::Unavailable(reason) => {
                return Ok(StructuralEvidenceAdmissionV1::Unavailable(
                    ProductionProfilerKirArchiveUnavailableV1::CharacteristicProjection(reason),
                ));
            }
        };
    Ok(StructuralEvidenceAdmissionV1::Admitted {
        catalog,
        bridge,
        characteristic,
    })
}

fn encode_archive(
    attempt: BuildAttempt,
    parts: ProtectedWorkerV3CompactFinalizerReplayPartsV2,
) -> Result<Vec<u8>, ProductionProfilerKirArchiveErrorV1> {
    validate_component_lengths(&parts)?;
    let provider_count = parts.external_provider_payloads.len();
    let section_count = provider_count
        .checked_add(ARCHIVE_REQUIRED_SECTIONS_V1)
        .ok_or(ProductionProfilerKirArchiveErrorV1::Length)?;
    let payload_bytes = parts.external_provider_payloads.iter().try_fold(
        parts
            .outer_handoff
            .len()
            .checked_add(parts.transcript.len())
            .and_then(|value| value.checked_add(parts.finalized_hsaco.len()))
            .ok_or(ProductionProfilerKirArchiveErrorV1::Length)?,
        |total, payload| {
            total
                .checked_add(payload.len())
                .ok_or(ProductionProfilerKirArchiveErrorV1::Length)
        },
    )?;
    let total_len = ARCHIVE_HEADER_BYTES_V1
        .checked_add(
            section_count
                .checked_mul(ARCHIVE_SECTION_HEADER_BYTES_V1)
                .ok_or(ProductionProfilerKirArchiveErrorV1::Length)?,
        )
        .and_then(|value| value.checked_add(payload_bytes))
        .and_then(|value| value.checked_add(ARCHIVE_CHECKSUM_BYTES_V1))
        .ok_or(ProductionProfilerKirArchiveErrorV1::Length)?;
    if total_len > MAX_PRODUCTION_PROFILER_KIR_ARCHIVE_BYTES_V1 {
        return Err(ProductionProfilerKirArchiveErrorV1::Length);
    }
    let mut output = Vec::new();
    output
        .try_reserve_exact(total_len)
        .map_err(|_| ProductionProfilerKirArchiveErrorV1::AllocationFailure)?;
    output.extend_from_slice(PRODUCTION_PROFILER_KIR_ARCHIVE_MAGIC_V1);
    push_u16(&mut output, PRODUCTION_PROFILER_KIR_ARCHIVE_VERSION_V1);
    push_u16(&mut output, 0);
    push_u64(&mut output, total_len as u64);
    push_u64(&mut output, attempt.generation());
    output.extend_from_slice(attempt.session().as_bytes());
    output.extend_from_slice(attempt.invocation().as_bytes());
    push_u16(
        &mut output,
        u16::try_from(section_count).map_err(|_| ProductionProfilerKirArchiveErrorV1::Length)?,
    );
    push_u16(&mut output, 0);
    encode_section(
        &mut output,
        SECTION_OUTER_HANDOFF_V1,
        0,
        &parts.outer_handoff,
    )?;
    for (ordinal, payload) in parts.external_provider_payloads.iter().enumerate() {
        encode_section(
            &mut output,
            SECTION_EXTERNAL_PROVIDER_V1,
            u16::try_from(ordinal).map_err(|_| ProductionProfilerKirArchiveErrorV1::Length)?,
            payload,
        )?;
    }
    encode_section(
        &mut output,
        SECTION_COMPACT_TRANSCRIPT_V1,
        0,
        &parts.transcript,
    )?;
    encode_section(
        &mut output,
        SECTION_FINALIZED_HSACO_V1,
        0,
        &parts.finalized_hsaco,
    )?;
    let checksum = hash(ARCHIVE_CHECKSUM_DOMAIN_V1, &output);
    output.extend_from_slice(&checksum);
    if output.len() != total_len {
        return Err(ProductionProfilerKirArchiveErrorV1::Length);
    }
    Ok(output)
}

fn validate_component_lengths(
    parts: &ProtectedWorkerV3CompactFinalizerReplayPartsV2,
) -> Result<(), ProductionProfilerKirArchiveErrorV1> {
    if parts.outer_handoff.is_empty()
        || parts.outer_handoff.len() > MAX_INERT_SEMANTIC_COMPILER_MODULE_HANDOFF_BYTES_V3
        || parts.transcript.is_empty()
        || parts.transcript.len() > MAX_PROTECTED_WORKER_V3_COMPACT_FINALIZER_REPLAY_BYTES_V1
        || parts.finalized_hsaco.is_empty()
        || parts.finalized_hsaco.len() > MAX_HSACO_BYTES
        || parts.external_provider_payloads.len() > MAX_LINK_INPUTS
    {
        return Err(ProductionProfilerKirArchiveErrorV1::Length);
    }
    let provider_bytes =
        parts
            .external_provider_payloads
            .iter()
            .try_fold(0_usize, |total, payload| {
                if payload.is_empty() {
                    return Err(ProductionProfilerKirArchiveErrorV1::Length);
                }
                total
                    .checked_add(payload.len())
                    .ok_or(ProductionProfilerKirArchiveErrorV1::Length)
            })?;
    if provider_bytes > MAX_WORKER_TOTAL_INPUT_BYTES {
        return Err(ProductionProfilerKirArchiveErrorV1::Length);
    }
    Ok(())
}

fn encode_section(
    output: &mut Vec<u8>,
    tag: u8,
    ordinal: u16,
    bytes: &[u8],
) -> Result<(), ProductionProfilerKirArchiveErrorV1> {
    output.push(tag);
    output.push(0);
    push_u16(output, ordinal);
    push_u64(
        output,
        u64::try_from(bytes.len()).map_err(|_| ProductionProfilerKirArchiveErrorV1::Length)?,
    );
    output.extend_from_slice(bytes);
    Ok(())
}

fn decode_archive(
    canonical_bytes: Vec<u8>,
) -> Result<InertProductionProfilerKirArchiveV1, ProductionProfilerKirArchiveErrorV1> {
    if canonical_bytes.len() < ARCHIVE_HEADER_BYTES_V1 + ARCHIVE_CHECKSUM_BYTES_V1
        || canonical_bytes.len() > MAX_PRODUCTION_PROFILER_KIR_ARCHIVE_BYTES_V1
    {
        return Err(ProductionProfilerKirArchiveErrorV1::Length);
    }
    let mut reader = ArchiveReaderV1::new(&canonical_bytes);
    if reader.take(8)? != PRODUCTION_PROFILER_KIR_ARCHIVE_MAGIC_V1 {
        return Err(ProductionProfilerKirArchiveErrorV1::Magic);
    }
    if reader.u16()? != PRODUCTION_PROFILER_KIR_ARCHIVE_VERSION_V1 {
        return Err(ProductionProfilerKirArchiveErrorV1::Version);
    }
    if reader.u16()? != 0 {
        return Err(ProductionProfilerKirArchiveErrorV1::Header);
    }
    let declared_len =
        usize::try_from(reader.u64()?).map_err(|_| ProductionProfilerKirArchiveErrorV1::Length)?;
    if declared_len > canonical_bytes.len() {
        return Err(ProductionProfilerKirArchiveErrorV1::Truncated);
    }
    if declared_len < canonical_bytes.len() {
        return Err(ProductionProfilerKirArchiveErrorV1::TrailingData);
    }
    let generation = reader.u64()?;
    let session = BuildSession::from_bytes(reader.array()?);
    let invocation = BuildInvocation::from_bytes(reader.array()?);
    let attempt = BuildAttempt::from_env_value(&format!(
        "{generation}:{}:{}",
        session.to_hex(),
        invocation.to_hex()
    ))
    .map_err(|_| ProductionProfilerKirArchiveErrorV1::Attempt)?;
    let section_count = reader.u16()? as usize;
    if reader.u16()? != 0
        || !(ARCHIVE_REQUIRED_SECTIONS_V1..=MAX_LINK_INPUTS + ARCHIVE_REQUIRED_SECTIONS_V1)
            .contains(&section_count)
    {
        return Err(ProductionProfilerKirArchiveErrorV1::Header);
    }
    let checksum_offset = canonical_bytes
        .len()
        .checked_sub(ARCHIVE_CHECKSUM_BYTES_V1)
        .ok_or(ProductionProfilerKirArchiveErrorV1::Length)?;
    if hash(
        ARCHIVE_CHECKSUM_DOMAIN_V1,
        &canonical_bytes[..checksum_offset],
    ) != canonical_bytes[checksum_offset..]
    {
        return Err(ProductionProfilerKirArchiveErrorV1::Checksum);
    }
    let provider_count = section_count - ARCHIVE_REQUIRED_SECTIONS_V1;
    let mut seen = BTreeSet::new();
    let outer_handoff = decode_expected_section(
        &mut reader,
        checksum_offset,
        SECTION_OUTER_HANDOFF_V1,
        0,
        &mut seen,
        MAX_INERT_SEMANTIC_COMPILER_MODULE_HANDOFF_BYTES_V3,
    )?;
    let mut external_providers = Vec::new();
    external_providers
        .try_reserve_exact(provider_count)
        .map_err(|_| ProductionProfilerKirArchiveErrorV1::AllocationFailure)?;
    let mut provider_bytes = 0_usize;
    for ordinal in 0..provider_count {
        let range = decode_expected_section(
            &mut reader,
            checksum_offset,
            SECTION_EXTERNAL_PROVIDER_V1,
            u16::try_from(ordinal).map_err(|_| ProductionProfilerKirArchiveErrorV1::Header)?,
            &mut seen,
            MAX_WORKER_TOTAL_INPUT_BYTES,
        )?;
        provider_bytes = provider_bytes
            .checked_add(range.len())
            .ok_or(ProductionProfilerKirArchiveErrorV1::Length)?;
        if provider_bytes > MAX_WORKER_TOTAL_INPUT_BYTES {
            return Err(ProductionProfilerKirArchiveErrorV1::Length);
        }
        external_providers.push(range);
    }
    let transcript = decode_expected_section(
        &mut reader,
        checksum_offset,
        SECTION_COMPACT_TRANSCRIPT_V1,
        0,
        &mut seen,
        MAX_PROTECTED_WORKER_V3_COMPACT_FINALIZER_REPLAY_BYTES_V1,
    )?;
    let finalized_hsaco = decode_expected_section(
        &mut reader,
        checksum_offset,
        SECTION_FINALIZED_HSACO_V1,
        0,
        &mut seen,
        MAX_HSACO_BYTES,
    )?;
    if reader.position != checksum_offset {
        return Err(ProductionProfilerKirArchiveErrorV1::TrailingData);
    }
    let identity =
        ProductionProfilerKirArchiveIdentityV1(hash(ARCHIVE_IDENTITY_DOMAIN_V1, &canonical_bytes));
    Ok(InertProductionProfilerKirArchiveV1 {
        identity,
        attempt,
        outer_handoff,
        external_providers,
        transcript,
        finalized_hsaco,
        canonical_bytes,
    })
}

fn decode_expected_section(
    reader: &mut ArchiveReaderV1<'_>,
    checksum_offset: usize,
    expected_tag: u8,
    expected_ordinal: u16,
    seen: &mut BTreeSet<(u8, u16)>,
    maximum_bytes: usize,
) -> Result<Range<usize>, ProductionProfilerKirArchiveErrorV1> {
    let tag = reader.u8()?;
    if reader.u8()? != 0 {
        return Err(ProductionProfilerKirArchiveErrorV1::SectionHeader);
    }
    let ordinal = reader.u16()?;
    if !seen.insert((tag, ordinal)) {
        return Err(ProductionProfilerKirArchiveErrorV1::DuplicateSection);
    }
    if tag != expected_tag || ordinal != expected_ordinal {
        return Err(ProductionProfilerKirArchiveErrorV1::SectionOrder);
    }
    let length =
        usize::try_from(reader.u64()?).map_err(|_| ProductionProfilerKirArchiveErrorV1::Length)?;
    if length == 0 || length > maximum_bytes {
        return Err(ProductionProfilerKirArchiveErrorV1::Length);
    }
    let start = reader.position;
    let end = start
        .checked_add(length)
        .ok_or(ProductionProfilerKirArchiveErrorV1::Length)?;
    if end > checksum_offset {
        return Err(ProductionProfilerKirArchiveErrorV1::Truncated);
    }
    reader.take(length)?;
    Ok(start..end)
}

struct ArchiveReaderV1<'a> {
    bytes: &'a [u8],
    position: usize,
}

impl<'a> ArchiveReaderV1<'a> {
    const fn new(bytes: &'a [u8]) -> Self {
        Self { bytes, position: 0 }
    }

    fn take(&mut self, count: usize) -> Result<&'a [u8], ProductionProfilerKirArchiveErrorV1> {
        let end = self
            .position
            .checked_add(count)
            .ok_or(ProductionProfilerKirArchiveErrorV1::Length)?;
        let value = self
            .bytes
            .get(self.position..end)
            .ok_or(ProductionProfilerKirArchiveErrorV1::Truncated)?;
        self.position = end;
        Ok(value)
    }

    fn u8(&mut self) -> Result<u8, ProductionProfilerKirArchiveErrorV1> {
        Ok(self.take(1)?[0])
    }

    fn u16(&mut self) -> Result<u16, ProductionProfilerKirArchiveErrorV1> {
        Ok(u16::from_le_bytes(self.array()?))
    }

    fn u64(&mut self) -> Result<u64, ProductionProfilerKirArchiveErrorV1> {
        Ok(u64::from_le_bytes(self.array()?))
    }

    fn array<const N: usize>(&mut self) -> Result<[u8; N], ProductionProfilerKirArchiveErrorV1> {
        self.take(N)?
            .try_into()
            .map_err(|_| ProductionProfilerKirArchiveErrorV1::Truncated)
    }
}

fn push_u16(output: &mut Vec<u8>, value: u16) {
    output.extend_from_slice(&value.to_le_bytes());
}

fn push_u64(output: &mut Vec<u8>, value: u64) {
    output.extend_from_slice(&value.to_le_bytes());
}

fn hash(domain: &[u8], bytes: &[u8]) -> [u8; 32] {
    let mut digest = Sha256::new();
    digest.update(domain);
    digest.update(bytes);
    digest.finalize().into()
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProductionProfilerKirArchiveErrorV1 {
    Length,
    AllocationFailure,
    Magic,
    Version,
    Header,
    Checksum,
    Truncated,
    TrailingData,
    Attempt,
    SectionHeader,
    DuplicateSection,
    SectionOrder,
    CompactReplay,
    FinalizerReplay,
    SemanticDebugReceipt,
    CatalogAdmission,
    StructuralBridgeAdmission,
    CharacteristicAdmission,
}

impl fmt::Display for ProductionProfilerKirArchiveErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "production profiler KIR archive rejected: {self:?}"
        )
    }
}

impl Error for ProductionProfilerKirArchiveErrorV1 {}

#[cfg(test)]
mod tests {
    use super::*;

    fn attempt() -> BuildAttempt {
        BuildAttempt::from_env_value(
            "1:01010101010101010101010101010101:0202020202020202020202020202020202020202020202020202020202020202",
        )
        .unwrap()
    }

    fn encoded() -> Vec<u8> {
        encode_archive(
            attempt(),
            ProtectedWorkerV3CompactFinalizerReplayPartsV2 {
                outer_handoff: vec![1],
                external_provider_payloads: vec![vec![2]],
                transcript: vec![3],
                finalized_hsaco: vec![4],
            },
        )
        .unwrap()
    }

    fn reseal(bytes: &mut [u8]) {
        let checksum_offset = bytes.len() - ARCHIVE_CHECKSUM_BYTES_V1;
        let checksum = hash(ARCHIVE_CHECKSUM_DOMAIN_V1, &bytes[..checksum_offset]);
        bytes[checksum_offset..].copy_from_slice(&checksum);
    }

    #[test]
    fn canonical_archive_is_bounded_and_decodes_without_query_authority() {
        let bytes = encoded();
        let inert = InertProductionProfilerKirArchiveV1::decode_canonical(&bytes).unwrap();
        assert_eq!(inert.canonical_bytes(), bytes);
        assert_eq!(inert.attempt(), attempt());
        assert!(!inert.authenticates_external_provenance());
        assert!(!inert.grants_runtime_authority());
        assert!(bytes.len() <= MAX_PRODUCTION_PROFILER_KIR_ARCHIVE_BYTES_V1);
    }

    #[test]
    fn archive_rejects_truncation_trailing_data_and_checksum_substitution() {
        let bytes = encoded();
        assert_eq!(
            InertProductionProfilerKirArchiveV1::decode_canonical(&bytes[..bytes.len() - 1])
                .unwrap_err(),
            ProductionProfilerKirArchiveErrorV1::Truncated
        );
        let mut trailing = bytes.clone();
        trailing.push(0);
        assert_eq!(
            InertProductionProfilerKirArchiveV1::decode_owned_canonical(trailing).unwrap_err(),
            ProductionProfilerKirArchiveErrorV1::TrailingData
        );
        let mut substituted = bytes;
        substituted[ARCHIVE_HEADER_BYTES_V1 + ARCHIVE_SECTION_HEADER_BYTES_V1] ^= 1;
        assert_eq!(
            InertProductionProfilerKirArchiveV1::decode_owned_canonical(substituted).unwrap_err(),
            ProductionProfilerKirArchiveErrorV1::Checksum
        );
    }

    #[test]
    fn archive_rejects_duplicate_and_reordered_sections_after_valid_reseal() {
        let mut duplicate = encoded();
        let provider_header = ARCHIVE_HEADER_BYTES_V1 + ARCHIVE_SECTION_HEADER_BYTES_V1 + 1;
        duplicate[provider_header] = SECTION_OUTER_HANDOFF_V1;
        duplicate[provider_header + 2..provider_header + 4].copy_from_slice(&0_u16.to_le_bytes());
        reseal(&mut duplicate);
        assert_eq!(
            InertProductionProfilerKirArchiveV1::decode_owned_canonical(duplicate).unwrap_err(),
            ProductionProfilerKirArchiveErrorV1::DuplicateSection
        );

        let mut reordered = encoded();
        reordered[provider_header] = SECTION_COMPACT_TRANSCRIPT_V1;
        reseal(&mut reordered);
        assert_eq!(
            InertProductionProfilerKirArchiveV1::decode_owned_canonical(reordered).unwrap_err(),
            ProductionProfilerKirArchiveErrorV1::SectionOrder
        );
    }
}

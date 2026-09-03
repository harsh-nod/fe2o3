//! Bounded compiler-to-finalizer carriage for production semantic debug evidence.
//!
//! The carrier is inert. It binds exact compiler-produced bytes and explicit
//! producer gaps, but grants no artifact, publication, load, or execution
//! authority.

use std::{error::Error, fmt};

use crate::{
    DebugSourceMapDocumentV2, SemanticDebugBoundaryDirectionV1, SemanticDebugBoundaryReasonV1,
    SemanticDebugContentIdentityV1, SemanticDebugLayerV1, SemanticDebugMapDocumentV1,
    SemanticDebugMapStatusV1, VerifiedCanonicalKernelIrV7,
};

pub const MAX_PRODUCTION_SEMANTIC_DEBUG_CARRIER_BYTES_V1: usize = 4 * 1024 * 1024;
pub const PRODUCTION_PREFINALIZED_ARTIFACT_STATUS_V1: &[u8] =
    b"FE2O3/PRODUCTION-SEMANTIC-DEBUG-ARTIFACT-STATUS/V1\0pre-finalization";

const MAGIC_V1: [u8; 8] = *b"F2SDFV1\0";
const VERSION_V1: u16 = 1;
const HEADER_BYTES_V1: usize = 16;
const FIELD_HEADER_BYTES_V1: usize = 4;
const AVAILABLE_KIND_V1: u16 = 1;
const UNAVAILABLE_KIND_V1: u16 = 2;
const AVAILABLE_FIELDS_V1: u16 = 6;
const UNAVAILABLE_FIELDS_V1: u16 = 3;
const CONTENT_IDENTITY_BYTES_V1: usize = 40;

/// Magic of the independently versioned semantic-debug receipt extension.
pub const PRODUCTION_SEMANTIC_DEBUG_RECEIPT_EXTENSION_MAGIC_V1: [u8; 8] = *b"F2SDRE1\0";
/// Version of the semantic-debug receipt extension.
pub const PRODUCTION_SEMANTIC_DEBUG_RECEIPT_EXTENSION_VERSION_V1: u16 = 1;
const RECEIPT_EXTENSION_HEADER_BYTES_V1: usize = 16;
const RECEIPT_EXTENSION_FIELD_COUNT_V1: u16 = 3;
const RECEIPT_EXTENSION_DOMAIN_V1: &[u8] =
    b"FE2O3/PRODUCTION-SEMANTIC-DEBUG-RECEIPT-EXTENSION/V1\0";

const SCHEDULE_MAGIC_V1: [u8; 8] = *b"F2SDSV1\0";
const SCHEDULE_VERSION_V1: u16 = 1;
const SCHEDULE_UNAVAILABLE_KIND_V1: u16 = 1;
const SCHEDULE_BYTES_V1: usize = 16;

/// Exact reason the production compiler emitted no semantic fragment.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum ProductionSemanticDebugProducerGapV1 {
    MultipleKirFunctionBodies = 1,
    NoStatementCorrespondence = 2,
    SourceMapUnavailable = 3,
    ResourceLimit = 4,
    CanonicalKirV7ProjectionUnavailable = 5,
    SourceObservationUnrepresentable = 6,
    SemanticMapConstructionUnavailable = 7,
    SemanticMapEncodingUnavailable = 8,
    FragmentConstructionUnavailable = 9,
    CarrierConstructionUnavailable = 10,
    ReceiptExtensionConstructionUnavailable = 11,
    CorrespondenceValidationUnavailable = 12,
    CanonicalKirModuleMismatch = 13,
    LegacyBareAssociationNoAttachment = 14,
}

impl ProductionSemanticDebugProducerGapV1 {
    fn from_byte(value: u8) -> Result<Self, ProductionSemanticDebugFragmentErrorV1> {
        match value {
            1 => Ok(Self::MultipleKirFunctionBodies),
            2 => Ok(Self::NoStatementCorrespondence),
            3 => Ok(Self::SourceMapUnavailable),
            4 => Ok(Self::ResourceLimit),
            5 => Ok(Self::CanonicalKirV7ProjectionUnavailable),
            6 => Ok(Self::SourceObservationUnrepresentable),
            7 => Ok(Self::SemanticMapConstructionUnavailable),
            8 => Ok(Self::SemanticMapEncodingUnavailable),
            9 => Ok(Self::FragmentConstructionUnavailable),
            10 => Ok(Self::CarrierConstructionUnavailable),
            11 => Ok(Self::ReceiptExtensionConstructionUnavailable),
            12 => Ok(Self::CorrespondenceValidationUnavailable),
            13 => Ok(Self::CanonicalKirModuleMismatch),
            14 => Ok(Self::LegacyBareAssociationNoAttachment),
            _ => Err(ProductionSemanticDebugFragmentErrorV1::InvalidGap),
        }
    }
}

/// Independently versioned receipt payload that preserves the frozen V3
/// association bytes while attaching one optional debug carrier.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProductionSemanticDebugReceiptExtensionV1 {
    association_v3: Box<[u8]>,
    carrier_v1: ProductionSemanticDebugCarrierV1,
    canonical_bytes: Box<[u8]>,
}

impl ProductionSemanticDebugReceiptExtensionV1 {
    /// Constructs a canonical extension and requires its nested association axes to agree.
    pub fn new(
        association_v3: &[u8],
        carrier_v1: ProductionSemanticDebugCarrierV1,
    ) -> Result<Self, ProductionSemanticDebugFragmentErrorV1> {
        if association_v3.is_empty() || carrier_v1.association_v3() != association_v3 {
            return Err(ProductionSemanticDebugFragmentErrorV1::InvalidAssociation);
        }
        let fields = [
            RECEIPT_EXTENSION_DOMAIN_V1,
            association_v3,
            carrier_v1.canonical_bytes(),
        ];
        let mut length = RECEIPT_EXTENSION_HEADER_BYTES_V1;
        for field in fields {
            length = length
                .checked_add(FIELD_HEADER_BYTES_V1)
                .and_then(|value| value.checked_add(field.len()))
                .ok_or(ProductionSemanticDebugFragmentErrorV1::ResourceLimit)?;
        }
        if length > MAX_PRODUCTION_SEMANTIC_DEBUG_CARRIER_BYTES_V1 {
            return Err(ProductionSemanticDebugFragmentErrorV1::ResourceLimit);
        }
        let mut bytes = Vec::new();
        bytes
            .try_reserve_exact(length)
            .map_err(|_| ProductionSemanticDebugFragmentErrorV1::AllocationFailure)?;
        bytes.extend_from_slice(&PRODUCTION_SEMANTIC_DEBUG_RECEIPT_EXTENSION_MAGIC_V1);
        bytes.extend_from_slice(
            &PRODUCTION_SEMANTIC_DEBUG_RECEIPT_EXTENSION_VERSION_V1.to_le_bytes(),
        );
        bytes.extend_from_slice(&RECEIPT_EXTENSION_FIELD_COUNT_V1.to_le_bytes());
        bytes.extend_from_slice(&0_u32.to_le_bytes());
        for field in fields {
            bytes.extend_from_slice(
                &u32::try_from(field.len())
                    .map_err(|_| ProductionSemanticDebugFragmentErrorV1::ResourceLimit)?
                    .to_le_bytes(),
            );
            bytes.extend_from_slice(field);
        }
        Self::from_canonical_bytes(&bytes)
    }

    /// Strictly decodes one complete extension with no legacy fallback.
    pub fn from_canonical_bytes(
        bytes: &[u8],
    ) -> Result<Self, ProductionSemanticDebugFragmentErrorV1> {
        if bytes.len() < RECEIPT_EXTENSION_HEADER_BYTES_V1
            || bytes.len() > MAX_PRODUCTION_SEMANTIC_DEBUG_CARRIER_BYTES_V1
            || bytes[..8] != PRODUCTION_SEMANTIC_DEBUG_RECEIPT_EXTENSION_MAGIC_V1
            || u16::from_le_bytes([bytes[8], bytes[9]])
                != PRODUCTION_SEMANTIC_DEBUG_RECEIPT_EXTENSION_VERSION_V1
            || u16::from_le_bytes([bytes[10], bytes[11]]) != RECEIPT_EXTENSION_FIELD_COUNT_V1
            || bytes[12..16] != [0; 4]
        {
            return Err(ProductionSemanticDebugFragmentErrorV1::InvalidEncoding);
        }
        let fields = decode_fields(bytes, 3)?;
        if fields[0] != RECEIPT_EXTENSION_DOMAIN_V1 {
            return Err(ProductionSemanticDebugFragmentErrorV1::InvalidEncoding);
        }
        let association_v3 = copy_field(fields[1])?;
        let carrier_v1 = ProductionSemanticDebugCarrierV1::from_canonical_bytes(fields[2])?;
        if carrier_v1.association_v3() != association_v3 {
            return Err(ProductionSemanticDebugFragmentErrorV1::InvalidAssociation);
        }
        Ok(Self {
            association_v3: association_v3.into_boxed_slice(),
            carrier_v1,
            canonical_bytes: copy_field(bytes)?.into_boxed_slice(),
        })
    }

    /// Returns the unchanged frozen V3 association bytes.
    pub fn association_v3(&self) -> &[u8] {
        &self.association_v3
    }
    /// Returns the bounded debug carrier.
    pub const fn carrier_v1(&self) -> &ProductionSemanticDebugCarrierV1 {
        &self.carrier_v1
    }
    /// Returns the exact canonical extension bytes.
    pub fn canonical_bytes(&self) -> &[u8] {
        &self.canonical_bytes
    }
    /// Reports that the extension grants no artifact authority.
    pub const fn grants_artifact_authority(&self) -> bool {
        false
    }
}

/// Canonical schedule-axis record used when production has no schedule stage.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProductionScheduleStatusV1 {
    NoProductionScheduleStage,
}

/// Truthful correspondence surface of the current production producer.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProductionSemanticDebugProducerCapabilityV1 {
    ExactSourceMirKir,
    ScheduleUnavailableNoProductionStage,
    InstructionLlvmUnavailableNoCorrespondence,
    ExactCanonicalKirV7DebugProjection,
}

/// Transformation classes tracked by the debugger acceptance contract.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum ProductionSemanticDebugTransformationClassV1 {
    Duplicated,
    Fused,
    Outlined,
    Inlined,
    Moved,
    Eliminated,
}

/// Exact producer capability for one transformation class.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProductionSemanticDebugTransformationAvailabilityV1 {
    Representable,
    UnavailableNoProductionEmitter,
    UnavailableNoSchemaRepresentation,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ProductionSemanticDebugTransformationCapabilityV1 {
    class: ProductionSemanticDebugTransformationClassV1,
    availability: ProductionSemanticDebugTransformationAvailabilityV1,
}

impl ProductionSemanticDebugTransformationCapabilityV1 {
    pub const fn class(self) -> ProductionSemanticDebugTransformationClassV1 {
        self.class
    }

    pub const fn availability(self) -> ProductionSemanticDebugTransformationAvailabilityV1 {
        self.availability
    }
}

/// Reports the transformation vocabulary of the current production semantic-debug producer.
///
/// `Representable` means the schema and producer can retain an authenticated observation when that
/// shape occurs; it does not claim that any particular compilation observed the shape. In
/// particular, a V4 correspondence span with multiple KIR operations proves cardinality, not
/// semantic duplication.
pub const fn production_semantic_debug_transformation_capabilities_v1()
-> [ProductionSemanticDebugTransformationCapabilityV1; 6] {
    use ProductionSemanticDebugTransformationAvailabilityV1::{
        Representable, UnavailableNoProductionEmitter, UnavailableNoSchemaRepresentation,
    };
    use ProductionSemanticDebugTransformationClassV1::{
        Duplicated, Eliminated, Fused, Inlined, Moved, Outlined,
    };
    [
        ProductionSemanticDebugTransformationCapabilityV1 {
            class: Duplicated,
            availability: UnavailableNoProductionEmitter,
        },
        ProductionSemanticDebugTransformationCapabilityV1 {
            class: Fused,
            availability: UnavailableNoProductionEmitter,
        },
        ProductionSemanticDebugTransformationCapabilityV1 {
            class: Outlined,
            availability: UnavailableNoSchemaRepresentation,
        },
        ProductionSemanticDebugTransformationCapabilityV1 {
            class: Inlined,
            availability: UnavailableNoProductionEmitter,
        },
        ProductionSemanticDebugTransformationCapabilityV1 {
            class: Moved,
            availability: UnavailableNoProductionEmitter,
        },
        ProductionSemanticDebugTransformationCapabilityV1 {
            class: Eliminated,
            availability: Representable,
        },
    ]
}

impl ProductionScheduleStatusV1 {
    pub fn canonical_bytes(self) -> [u8; SCHEDULE_BYTES_V1] {
        let mut bytes = [0_u8; SCHEDULE_BYTES_V1];
        bytes[..8].copy_from_slice(&SCHEDULE_MAGIC_V1);
        bytes[8..10].copy_from_slice(&SCHEDULE_VERSION_V1.to_le_bytes());
        bytes[10..12].copy_from_slice(&SCHEDULE_UNAVAILABLE_KIND_V1.to_le_bytes());
        bytes[12..16].copy_from_slice(&0_u32.to_le_bytes());
        bytes
    }

    pub fn from_canonical_bytes(
        bytes: &[u8],
    ) -> Result<Self, ProductionSemanticDebugFragmentErrorV1> {
        if bytes.len() != SCHEDULE_BYTES_V1
            || bytes[..8] != SCHEDULE_MAGIC_V1
            || u16::from_le_bytes([bytes[8], bytes[9]]) != SCHEDULE_VERSION_V1
            || u16::from_le_bytes([bytes[10], bytes[11]]) != SCHEDULE_UNAVAILABLE_KIND_V1
            || bytes[12..] != [0; 4]
        {
            return Err(ProductionSemanticDebugFragmentErrorV1::InvalidScheduleStatus);
        }
        Ok(Self::NoProductionScheduleStage)
    }
}

/// Exact compiler-produced axes retained until final artifact admission.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProductionSemanticDebugFragmentV1 {
    source_map_v2: Box<[u8]>,
    canonical_kir_v7: Box<[u8]>,
    schedule_status: Box<[u8]>,
    pre_finalization_map: Box<[u8]>,
}

impl ProductionSemanticDebugFragmentV1 {
    pub fn new(
        source_map_v2: Vec<u8>,
        canonical_kir_v7: Vec<u8>,
        schedule_status: Vec<u8>,
        pre_finalization_map: Vec<u8>,
    ) -> Result<Self, ProductionSemanticDebugFragmentErrorV1> {
        let fragment = Self {
            source_map_v2: source_map_v2.into_boxed_slice(),
            canonical_kir_v7: canonical_kir_v7.into_boxed_slice(),
            schedule_status: schedule_status.into_boxed_slice(),
            pre_finalization_map: pre_finalization_map.into_boxed_slice(),
        };
        fragment.validate()?;
        Ok(fragment)
    }

    pub fn source_map_v2(&self) -> &[u8] {
        &self.source_map_v2
    }

    pub fn canonical_kir_v7(&self) -> &[u8] {
        &self.canonical_kir_v7
    }

    pub fn schedule_status(&self) -> &[u8] {
        &self.schedule_status
    }

    pub fn pre_finalization_map(&self) -> &[u8] {
        &self.pre_finalization_map
    }

    pub const fn schedule_capability(&self) -> ProductionScheduleStatusV1 {
        ProductionScheduleStatusV1::NoProductionScheduleStage
    }

    pub const fn has_instruction_level_llvm_correspondence(&self) -> bool {
        false
    }

    pub const fn producer_capabilities(&self) -> [ProductionSemanticDebugProducerCapabilityV1; 4] {
        [
            ProductionSemanticDebugProducerCapabilityV1::ExactSourceMirKir,
            ProductionSemanticDebugProducerCapabilityV1::ScheduleUnavailableNoProductionStage,
            ProductionSemanticDebugProducerCapabilityV1::InstructionLlvmUnavailableNoCorrespondence,
            ProductionSemanticDebugProducerCapabilityV1::ExactCanonicalKirV7DebugProjection,
        ]
    }

    /// Reports transformation expressiveness without fabricating observations for this fragment.
    pub const fn transformation_capabilities(
        &self,
    ) -> [ProductionSemanticDebugTransformationCapabilityV1; 6] {
        production_semantic_debug_transformation_capabilities_v1()
    }

    fn validate(&self) -> Result<(), ProductionSemanticDebugFragmentErrorV1> {
        let source_map = DebugSourceMapDocumentV2::from_canonical_json_bytes(&self.source_map_v2)
            .map_err(|_| ProductionSemanticDebugFragmentErrorV1::InvalidSourceMap)?;
        let kir =
            VerifiedCanonicalKernelIrV7::from_canonical_bytes(copy_field(&self.canonical_kir_v7)?)
                .map_err(|_| ProductionSemanticDebugFragmentErrorV1::InvalidCanonicalKir)?;
        ProductionScheduleStatusV1::from_canonical_bytes(&self.schedule_status)?;
        let map = SemanticDebugMapDocumentV1::from_canonical_json_bytes(&self.pre_finalization_map)
            .map_err(|_| ProductionSemanticDebugFragmentErrorV1::InvalidSemanticMap)?;
        if source_map.binding().canonical_kir().digest() != *kir.identity().digest()
            || source_map.binding().canonical_kir().canonical_bytes()
                != kir.identity().canonical_length()
            || !map.binding().source_map_v2().matches(&self.source_map_v2)
            || !map
                .binding()
                .canonical_kir()
                .matches(&self.canonical_kir_v7)
            || !map.binding().schedule().matches(&self.schedule_status)
            || !map
                .binding()
                .finalized_artifact()
                .matches(PRODUCTION_PREFINALIZED_ARTIFACT_STATUS_V1)
            || map.status() != SemanticDebugMapStatusV1::Partial
            || map.nodes().iter().any(|node| {
                matches!(
                    node.layer(),
                    SemanticDebugLayerV1::Schedule
                        | SemanticDebugLayerV1::Llvm
                        | SemanticDebugLayerV1::Isa
                )
            })
        {
            return Err(ProductionSemanticDebugFragmentErrorV1::AxisMismatch);
        }
        let mut has_source = false;
        let mut has_mir = false;
        let mut has_kir = false;
        for node in map.nodes() {
            match node.layer() {
                SemanticDebugLayerV1::Source => has_source = true,
                SemanticDebugLayerV1::Mir => has_mir = true,
                SemanticDebugLayerV1::Kir => has_kir = true,
                _ => {}
            }
        }
        if !(has_source
            && has_mir
            && has_kir
            && map.boundaries().iter().any(|boundary| {
                map.node(boundary.node()).is_some_and(|node| {
                    node.layer() == SemanticDebugLayerV1::Kir
                        && boundary.direction()
                            == SemanticDebugBoundaryDirectionV1::SuccessorUnavailable
                        && boundary.reason() == SemanticDebugBoundaryReasonV1::UnsupportedLayer
                })
            }))
        {
            return Err(ProductionSemanticDebugFragmentErrorV1::AxisMismatch);
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ProductionSemanticDebugAvailabilityV1 {
    Available(ProductionSemanticDebugFragmentV1),
    Unavailable(ProductionSemanticDebugProducerGapV1),
}

/// Versioned wrapper retaining the existing association transcript and one
/// optional semantic-debug fragment in the semantic-to-LLVM receipt preimage.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProductionSemanticDebugCarrierV1 {
    association_v3: Box<[u8]>,
    association_identity_v3: SemanticDebugContentIdentityV1,
    availability: ProductionSemanticDebugAvailabilityV1,
    canonical_bytes: Box<[u8]>,
}

impl ProductionSemanticDebugCarrierV1 {
    pub fn new(
        association_v3: &[u8],
        availability: ProductionSemanticDebugAvailabilityV1,
    ) -> Result<Self, ProductionSemanticDebugFragmentErrorV1> {
        if association_v3.is_empty() {
            return Err(ProductionSemanticDebugFragmentErrorV1::InvalidAssociation);
        }
        let association_identity_v3 = SemanticDebugContentIdentityV1::calculate(association_v3)
            .map_err(|_| ProductionSemanticDebugFragmentErrorV1::InvalidAssociation)?;
        let encoded_association_identity = encode_content_identity(association_identity_v3);
        let canonical_bytes = match &availability {
            ProductionSemanticDebugAvailabilityV1::Available(fragment) => encode(
                AVAILABLE_KIND_V1,
                &[
                    association_v3,
                    &encoded_association_identity,
                    fragment.source_map_v2(),
                    fragment.canonical_kir_v7(),
                    fragment.schedule_status(),
                    fragment.pre_finalization_map(),
                ],
            )?,
            ProductionSemanticDebugAvailabilityV1::Unavailable(gap) => {
                let gap = [*gap as u8];
                encode(
                    UNAVAILABLE_KIND_V1,
                    &[association_v3, &encoded_association_identity, &gap],
                )?
            }
        };
        Self::from_canonical_bytes(&canonical_bytes)
    }

    pub fn from_canonical_bytes(
        bytes: &[u8],
    ) -> Result<Self, ProductionSemanticDebugFragmentErrorV1> {
        if bytes.len() < HEADER_BYTES_V1
            || bytes.len() > MAX_PRODUCTION_SEMANTIC_DEBUG_CARRIER_BYTES_V1
            || bytes[..8] != MAGIC_V1
            || u16::from_le_bytes([bytes[8], bytes[9]]) != VERSION_V1
            || bytes[14..16] != [0; 2]
        {
            return Err(ProductionSemanticDebugFragmentErrorV1::InvalidEncoding);
        }
        let kind = u16::from_le_bytes([bytes[10], bytes[11]]);
        let field_count = u16::from_le_bytes([bytes[12], bytes[13]]);
        let expected_fields = match kind {
            AVAILABLE_KIND_V1 => AVAILABLE_FIELDS_V1,
            UNAVAILABLE_KIND_V1 => UNAVAILABLE_FIELDS_V1,
            _ => return Err(ProductionSemanticDebugFragmentErrorV1::InvalidEncoding),
        };
        if field_count != expected_fields {
            return Err(ProductionSemanticDebugFragmentErrorV1::InvalidEncoding);
        }
        let fields = decode_fields(bytes, usize::from(field_count))?;
        let association_v3 = copy_field(fields[0])?;
        if association_v3.is_empty() {
            return Err(ProductionSemanticDebugFragmentErrorV1::InvalidAssociation);
        }
        let association_identity_v3 = decode_content_identity(fields[1])?;
        if !association_identity_v3.matches(&association_v3) {
            return Err(ProductionSemanticDebugFragmentErrorV1::InvalidAssociation);
        }
        let availability = match kind {
            AVAILABLE_KIND_V1 => ProductionSemanticDebugAvailabilityV1::Available(
                ProductionSemanticDebugFragmentV1::new(
                    copy_field(fields[2])?,
                    copy_field(fields[3])?,
                    copy_field(fields[4])?,
                    copy_field(fields[5])?,
                )?,
            ),
            UNAVAILABLE_KIND_V1 => {
                if fields[2].len() != 1 {
                    return Err(ProductionSemanticDebugFragmentErrorV1::InvalidGap);
                }
                ProductionSemanticDebugAvailabilityV1::Unavailable(
                    ProductionSemanticDebugProducerGapV1::from_byte(fields[2][0])?,
                )
            }
            _ => unreachable!(),
        };
        Ok(Self {
            association_v3: association_v3.into_boxed_slice(),
            association_identity_v3,
            availability,
            canonical_bytes: copy_field(bytes)?.into_boxed_slice(),
        })
    }

    pub fn association_v3(&self) -> &[u8] {
        &self.association_v3
    }

    /// Exact identity of the structurally validated compiler-owned nested
    /// association bytes. It remains descriptive and grants no authority.
    pub const fn association_identity_v3(&self) -> SemanticDebugContentIdentityV1 {
        self.association_identity_v3
    }

    pub const fn availability(&self) -> &ProductionSemanticDebugAvailabilityV1 {
        &self.availability
    }

    pub fn canonical_bytes(&self) -> &[u8] {
        &self.canonical_bytes
    }

    pub const fn grants_artifact_authority(&self) -> bool {
        false
    }

    pub const fn grants_execution_authority(&self) -> bool {
        false
    }
}

fn encode_content_identity(
    identity: SemanticDebugContentIdentityV1,
) -> [u8; CONTENT_IDENTITY_BYTES_V1] {
    let mut encoded = [0_u8; CONTENT_IDENTITY_BYTES_V1];
    encoded[..32].copy_from_slice(&identity.sha256());
    encoded[32..].copy_from_slice(&identity.byte_len().to_le_bytes());
    encoded
}

fn decode_content_identity(
    encoded: &[u8],
) -> Result<SemanticDebugContentIdentityV1, ProductionSemanticDebugFragmentErrorV1> {
    if encoded.len() != CONTENT_IDENTITY_BYTES_V1 {
        return Err(ProductionSemanticDebugFragmentErrorV1::InvalidAssociation);
    }
    let mut sha256 = [0_u8; 32];
    sha256.copy_from_slice(&encoded[..32]);
    let byte_len = u64::from_le_bytes(
        encoded[32..]
            .try_into()
            .expect("content identity length was checked"),
    );
    SemanticDebugContentIdentityV1::new(sha256, byte_len)
        .map_err(|_| ProductionSemanticDebugFragmentErrorV1::InvalidAssociation)
}

fn encode(kind: u16, fields: &[&[u8]]) -> Result<Vec<u8>, ProductionSemanticDebugFragmentErrorV1> {
    let mut length = HEADER_BYTES_V1;
    for field in fields {
        u32::try_from(field.len())
            .map_err(|_| ProductionSemanticDebugFragmentErrorV1::ResourceLimit)?;
        length = length
            .checked_add(FIELD_HEADER_BYTES_V1)
            .and_then(|value| value.checked_add(field.len()))
            .ok_or(ProductionSemanticDebugFragmentErrorV1::ResourceLimit)?;
    }
    if length > MAX_PRODUCTION_SEMANTIC_DEBUG_CARRIER_BYTES_V1 {
        return Err(ProductionSemanticDebugFragmentErrorV1::ResourceLimit);
    }
    let mut bytes = Vec::new();
    bytes
        .try_reserve_exact(length)
        .map_err(|_| ProductionSemanticDebugFragmentErrorV1::AllocationFailure)?;
    bytes.extend_from_slice(&MAGIC_V1);
    bytes.extend_from_slice(&VERSION_V1.to_le_bytes());
    bytes.extend_from_slice(&kind.to_le_bytes());
    bytes.extend_from_slice(
        &u16::try_from(fields.len())
            .map_err(|_| ProductionSemanticDebugFragmentErrorV1::ResourceLimit)?
            .to_le_bytes(),
    );
    bytes.extend_from_slice(&0_u16.to_le_bytes());
    for field in fields {
        bytes.extend_from_slice(&(field.len() as u32).to_le_bytes());
        bytes.extend_from_slice(field);
    }
    Ok(bytes)
}

fn decode_fields(
    bytes: &[u8],
    count: usize,
) -> Result<Vec<&[u8]>, ProductionSemanticDebugFragmentErrorV1> {
    let mut fields = Vec::new();
    fields
        .try_reserve_exact(count)
        .map_err(|_| ProductionSemanticDebugFragmentErrorV1::AllocationFailure)?;
    let mut cursor = HEADER_BYTES_V1;
    for _ in 0..count {
        let header_end = cursor
            .checked_add(FIELD_HEADER_BYTES_V1)
            .ok_or(ProductionSemanticDebugFragmentErrorV1::InvalidEncoding)?;
        let header = bytes
            .get(cursor..header_end)
            .ok_or(ProductionSemanticDebugFragmentErrorV1::InvalidEncoding)?;
        let length = u32::from_le_bytes(header.try_into().expect("four-byte field header"));
        let end = header_end
            .checked_add(length as usize)
            .ok_or(ProductionSemanticDebugFragmentErrorV1::InvalidEncoding)?;
        fields.push(
            bytes
                .get(header_end..end)
                .ok_or(ProductionSemanticDebugFragmentErrorV1::InvalidEncoding)?,
        );
        cursor = end;
    }
    if cursor != bytes.len() {
        return Err(ProductionSemanticDebugFragmentErrorV1::InvalidEncoding);
    }
    Ok(fields)
}

fn copy_field(bytes: &[u8]) -> Result<Vec<u8>, ProductionSemanticDebugFragmentErrorV1> {
    let mut copy = Vec::new();
    copy.try_reserve_exact(bytes.len())
        .map_err(|_| ProductionSemanticDebugFragmentErrorV1::AllocationFailure)?;
    copy.extend_from_slice(bytes);
    Ok(copy)
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProductionSemanticDebugFragmentErrorV1 {
    InvalidEncoding,
    InvalidAssociation,
    InvalidGap,
    InvalidScheduleStatus,
    InvalidSourceMap,
    InvalidCanonicalKir,
    InvalidSemanticMap,
    AxisMismatch,
    ResourceLimit,
    AllocationFailure,
}

impl fmt::Display for ProductionSemanticDebugFragmentErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "invalid production semantic debug fragment: {self:?}"
        )
    }
}

impl Error for ProductionSemanticDebugFragmentErrorV1 {}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        DebugSourceMapBindingV1, DebugSourceMapFileV1, DebugSourceMapSiteV1, Module,
        SemanticDebugBoundaryV1, SemanticDebugContentIdentityV1, SemanticDebugLocationV1,
        SemanticDebugMapBindingV1, SemanticDebugMappingOutputV1, SemanticDebugMappingV1,
        SemanticDebugNodeV1, SemanticDebugTransformationV1,
    };

    fn id(byte: u8) -> [u8; 32] {
        [byte; 32]
    }

    fn available_fragment(module_id: &str) -> ProductionSemanticDebugFragmentV1 {
        let kir = VerifiedCanonicalKernelIrV7::from_module(Module::new(module_id)).unwrap();
        let source_map = DebugSourceMapDocumentV2::new(
            DebugSourceMapBindingV1::new(
                id(80),
                *kir.identity().digest(),
                kir.identity().canonical_length(),
            )
            .unwrap(),
            vec![DebugSourceMapFileV1::new(id(1), 64, "/src/kernel.rs".into()).unwrap()],
            Vec::<DebugSourceMapSiteV1>::new(),
            Vec::new(),
            Vec::new(),
            Vec::new(),
        )
        .unwrap()
        .to_canonical_json_bytes()
        .unwrap();
        let kir = kir.into_canonical_bytes();
        let schedule = ProductionScheduleStatusV1::NoProductionScheduleStage
            .canonical_bytes()
            .to_vec();
        let source = SemanticDebugNodeV1::new(
            id(10),
            SemanticDebugLocationV1::Source {
                span: crate::DebugSourceMapSpanV1::new(id(1), 4, 8, 1, 5).unwrap(),
            },
        )
        .unwrap();
        let mir = SemanticDebugNodeV1::new(
            id(11),
            SemanticDebugLocationV1::Mir {
                body_ordinal: 0,
                block_ordinal: 0,
                statement_ordinal: 0,
            },
        )
        .unwrap();
        let kir_node = SemanticDebugNodeV1::new(
            id(12),
            SemanticDebugLocationV1::Kir {
                function_ordinal: 0,
                block_ordinal: 0,
                operation_ordinal: 0,
            },
        )
        .unwrap();
        let mapping = |identity, input_layer, output_layer, input, output| {
            SemanticDebugMappingV1::new(
                identity,
                input_layer,
                output_layer,
                SemanticDebugTransformationV1::Preserved,
                vec![input],
                SemanticDebugMappingOutputV1::available(vec![output]),
            )
            .unwrap()
        };
        let binding = SemanticDebugMapBindingV1::new(
            SemanticDebugContentIdentityV1::calculate(&source_map).unwrap(),
            SemanticDebugContentIdentityV1::calculate(b"mir").unwrap(),
            SemanticDebugContentIdentityV1::calculate(&kir).unwrap(),
            SemanticDebugContentIdentityV1::calculate(&schedule).unwrap(),
            SemanticDebugContentIdentityV1::calculate(b"llvm").unwrap(),
            SemanticDebugContentIdentityV1::calculate(PRODUCTION_PREFINALIZED_ARTIFACT_STATUS_V1)
                .unwrap(),
        )
        .unwrap();
        let map = SemanticDebugMapDocumentV1::new_partial(
            binding,
            vec![source, mir, kir_node],
            vec![
                mapping(
                    id(20),
                    SemanticDebugLayerV1::Source,
                    SemanticDebugLayerV1::Mir,
                    id(10),
                    id(11),
                ),
                mapping(
                    id(21),
                    SemanticDebugLayerV1::Mir,
                    SemanticDebugLayerV1::Kir,
                    id(11),
                    id(12),
                ),
            ],
            vec![
                SemanticDebugBoundaryV1::new(
                    id(12),
                    SemanticDebugBoundaryDirectionV1::SuccessorUnavailable,
                    SemanticDebugBoundaryReasonV1::UnsupportedLayer,
                )
                .unwrap(),
            ],
        )
        .unwrap()
        .to_canonical_json_bytes()
        .unwrap();
        ProductionSemanticDebugFragmentV1::new(source_map, kir, schedule, map).unwrap()
    }

    #[test]
    fn available_and_unavailable_carriers_round_trip_canonically() {
        let available = ProductionSemanticDebugCarrierV1::new(
            b"association-v3",
            ProductionSemanticDebugAvailabilityV1::Available(available_fragment("first")),
        )
        .unwrap();
        assert_eq!(
            ProductionSemanticDebugCarrierV1::from_canonical_bytes(available.canonical_bytes())
                .unwrap()
                .canonical_bytes(),
            available.canonical_bytes()
        );
        let decoded =
            ProductionSemanticDebugCarrierV1::from_canonical_bytes(available.canonical_bytes())
                .unwrap();
        assert!(
            decoded
                .association_identity_v3()
                .matches(decoded.association_v3())
        );
        let ProductionSemanticDebugAvailabilityV1::Available(fragment) = decoded.availability()
        else {
            panic!("available fragment changed availability")
        };
        assert_eq!(
            fragment.transformation_capabilities(),
            [
                ProductionSemanticDebugTransformationCapabilityV1 {
                    class: ProductionSemanticDebugTransformationClassV1::Duplicated,
                    availability: ProductionSemanticDebugTransformationAvailabilityV1::UnavailableNoProductionEmitter,
                },
                ProductionSemanticDebugTransformationCapabilityV1 {
                    class: ProductionSemanticDebugTransformationClassV1::Fused,
                    availability: ProductionSemanticDebugTransformationAvailabilityV1::UnavailableNoProductionEmitter,
                },
                ProductionSemanticDebugTransformationCapabilityV1 {
                    class: ProductionSemanticDebugTransformationClassV1::Outlined,
                    availability: ProductionSemanticDebugTransformationAvailabilityV1::UnavailableNoSchemaRepresentation,
                },
                ProductionSemanticDebugTransformationCapabilityV1 {
                    class: ProductionSemanticDebugTransformationClassV1::Inlined,
                    availability: ProductionSemanticDebugTransformationAvailabilityV1::UnavailableNoProductionEmitter,
                },
                ProductionSemanticDebugTransformationCapabilityV1 {
                    class: ProductionSemanticDebugTransformationClassV1::Moved,
                    availability: ProductionSemanticDebugTransformationAvailabilityV1::UnavailableNoProductionEmitter,
                },
                ProductionSemanticDebugTransformationCapabilityV1 {
                    class: ProductionSemanticDebugTransformationClassV1::Eliminated,
                    availability: ProductionSemanticDebugTransformationAvailabilityV1::Representable,
                },
            ]
        );
        let map =
            SemanticDebugMapDocumentV1::from_canonical_json_bytes(fragment.pre_finalization_map())
                .unwrap();
        assert_eq!(
            map.mapping_from(id(10)).unwrap().output().nodes(),
            &[id(11)]
        );
        assert_eq!(
            map.mapping_from(id(11)).unwrap().output().nodes(),
            &[id(12)]
        );
        assert_eq!(map.mapping_to(id(12)).unwrap().inputs(), &[id(11)]);
        assert!(map.mapping_from(id(12)).is_none());
        assert!(map.boundaries().iter().any(|boundary| {
            boundary.node() == id(12)
                && boundary.reason() == SemanticDebugBoundaryReasonV1::UnsupportedLayer
        }));
        let unavailable = ProductionSemanticDebugCarrierV1::new(
            b"association-v3",
            ProductionSemanticDebugAvailabilityV1::Unavailable(
                ProductionSemanticDebugProducerGapV1::CanonicalKirV7ProjectionUnavailable,
            ),
        )
        .unwrap();
        assert!(matches!(
            ProductionSemanticDebugCarrierV1::from_canonical_bytes(unavailable.canonical_bytes())
                .unwrap()
                .availability(),
            ProductionSemanticDebugAvailabilityV1::Unavailable(
                ProductionSemanticDebugProducerGapV1::CanonicalKirV7ProjectionUnavailable
            )
        ));
    }

    #[test]
    fn substitutions_trailing_bytes_and_oversize_fail_closed() {
        let carrier = ProductionSemanticDebugCarrierV1::new(
            b"association-v3",
            ProductionSemanticDebugAvailabilityV1::Available(available_fragment("first")),
        )
        .unwrap();
        let mut trailing = carrier.canonical_bytes().to_vec();
        trailing.push(0);
        assert_eq!(
            ProductionSemanticDebugCarrierV1::from_canonical_bytes(&trailing),
            Err(ProductionSemanticDebugFragmentErrorV1::InvalidEncoding)
        );
        let mut substituted = carrier.canonical_bytes().to_vec();
        let source_field = HEADER_BYTES_V1
            + FIELD_HEADER_BYTES_V1
            + b"association-v3".len()
            + FIELD_HEADER_BYTES_V1
            + CONTENT_IDENTITY_BYTES_V1
            + FIELD_HEADER_BYTES_V1;
        substituted[source_field] ^= 1;
        assert!(ProductionSemanticDebugCarrierV1::from_canonical_bytes(&substituted).is_err());
        let mut substituted_association = carrier.canonical_bytes().to_vec();
        substituted_association[HEADER_BYTES_V1 + FIELD_HEADER_BYTES_V1] ^= 1;
        assert_eq!(
            ProductionSemanticDebugCarrierV1::from_canonical_bytes(&substituted_association),
            Err(ProductionSemanticDebugFragmentErrorV1::InvalidAssociation)
        );
        assert_eq!(
            ProductionSemanticDebugCarrierV1::from_canonical_bytes(&vec![
                0;
                MAX_PRODUCTION_SEMANTIC_DEBUG_CARRIER_BYTES_V1
                    + 1
            ]),
            Err(ProductionSemanticDebugFragmentErrorV1::InvalidEncoding)
        );
    }

    #[test]
    fn receipt_extension_preserves_nested_association_and_carrier_exactly() {
        let association = b"frozen-v3-association";
        let carrier = ProductionSemanticDebugCarrierV1::new(
            association,
            ProductionSemanticDebugAvailabilityV1::Unavailable(
                ProductionSemanticDebugProducerGapV1::SourceMapUnavailable,
            ),
        )
        .unwrap();
        let carrier_bytes = carrier.canonical_bytes().to_vec();
        let extension =
            ProductionSemanticDebugReceiptExtensionV1::new(association, carrier).unwrap();
        let decoded = ProductionSemanticDebugReceiptExtensionV1::from_canonical_bytes(
            extension.canonical_bytes(),
        )
        .unwrap();
        assert_eq!(decoded.association_v3(), association);
        assert_eq!(decoded.carrier_v1().canonical_bytes(), carrier_bytes);
        assert!(!decoded.grants_artifact_authority());

        let mut trailing = extension.canonical_bytes().to_vec();
        trailing.push(0);
        assert!(
            ProductionSemanticDebugReceiptExtensionV1::from_canonical_bytes(&trailing).is_err()
        );
        for field in 0..3 {
            let mut substituted = extension.canonical_bytes().to_vec();
            let offset = receipt_extension_field_payload_offset(&substituted, field);
            substituted[offset] ^= 1;
            assert!(
                ProductionSemanticDebugReceiptExtensionV1::from_canonical_bytes(&substituted)
                    .is_err()
            );
        }
    }

    fn receipt_extension_field_payload_offset(bytes: &[u8], field: usize) -> usize {
        let mut offset = RECEIPT_EXTENSION_HEADER_BYTES_V1;
        for index in 0..=field {
            let length = u32::from_le_bytes(bytes[offset..offset + 4].try_into().unwrap()) as usize;
            offset += FIELD_HEADER_BYTES_V1;
            if index == field {
                return offset;
            }
            offset += length;
        }
        unreachable!()
    }
}

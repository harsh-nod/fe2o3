//! Exact structural coordinates from simulator KIR V7 into admitted production KIR V8.
//!
//! The current production attachment proves this narrow relationship by retaining exact
//! canonical V7 and V8 encodings whose verified decoded modules are equal. This sidecar makes
//! that relationship durable and queryable, and binds it to the already-admitted Source/ISA
//! catalog. It is not a semantic-refinement proof. The V1 bridge accepts only the reachable V8
//! catalog contract; non-identity migrations remain explicitly unavailable.

use std::{error::Error, fmt};

use fe2o3_kernel_ir::{
    DebugSourceMapDocumentV2, MAX_MODULE_BYTES_V1, Module, VerifiedCanonicalKernelIrV7,
    VerifiedCanonicalKernelIrV8,
};
use sha2::{Digest, Sha256};

use crate::{
    ContentIdentityV1, ProductionSourceIsaCatalogContentIdentityV1,
    ProductionSourceIsaCatalogKirVersionV1, ProductionSourceIsaCatalogMatchesV1,
    ProductionSourceIsaCatalogQueryUnavailableV1, ProductionSourceIsaCatalogStructuralBindingV1,
    ProductionSourceIsaCatalogStructuralCountsV1, ProductionSourceIsaCatalogTargetV1,
    ProductionSourceIsaCatalogV1, ProductionSourceIsaKirCoordinateV1,
};

pub const PRODUCTION_KIR_V7_BRIDGE_MAGIC_V1: [u8; 8] = *b"F2K7BRG1";
pub const PRODUCTION_KIR_V7_BRIDGE_VERSION_V1: u16 = 1;
pub const MAX_PRODUCTION_KIR_V7_BRIDGE_BYTES_V1: usize = 64 * 1024 * 1024;
pub const MAX_PRODUCTION_KIR_V7_BRIDGE_RECORDS_V1: usize = 1_000_000;

const BRIDGE_IDENTITY_DOMAIN_V1: &[u8] = b"FE2O3/PRODUCTION-KIR-V7-STRUCTURAL-BRIDGE/V1\0";
const BRIDGE_HEADER_BYTES_V1: usize = 400;
const BRIDGE_RECORD_BYTES_V1: usize = 104;
const BRIDGE_IDENTITY_BYTES_V1: usize = 32;

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ProductionKirV7BridgeContentIdentityV1 {
    sha256: [u8; 32],
    byte_len: u64,
}

impl ProductionKirV7BridgeContentIdentityV1 {
    pub const fn sha256(self) -> [u8; 32] {
        self.sha256
    }

    pub const fn byte_len(self) -> u64 {
        self.byte_len
    }

    fn validate(self) -> bool {
        self.sha256 != [0; 32] && self.byte_len != 0
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum ProductionKirV7BridgeKirVersionV1 {
    V8,
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum ProductionKirV7BridgeTargetV1 {
    Gfx942,
    Gfx950,
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum ProductionKirV7BridgePointV1 {
    BlockEntry,
    Operation { operation_ordinal: u64 },
    Terminator,
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ProductionKirV7BridgeSiteV1 {
    function_ordinal: u64,
    block_ordinal: u64,
    point: ProductionKirV7BridgePointV1,
}

impl ProductionKirV7BridgeSiteV1 {
    pub const fn block_entry(function_ordinal: u64, block_ordinal: u64) -> Self {
        Self {
            function_ordinal,
            block_ordinal,
            point: ProductionKirV7BridgePointV1::BlockEntry,
        }
    }

    pub const fn operation(
        function_ordinal: u64,
        block_ordinal: u64,
        operation_ordinal: u64,
    ) -> Self {
        Self {
            function_ordinal,
            block_ordinal,
            point: ProductionKirV7BridgePointV1::Operation { operation_ordinal },
        }
    }

    pub const fn terminator(function_ordinal: u64, block_ordinal: u64) -> Self {
        Self {
            function_ordinal,
            block_ordinal,
            point: ProductionKirV7BridgePointV1::Terminator,
        }
    }

    pub const fn function_ordinal(self) -> u64 {
        self.function_ordinal
    }

    pub const fn block_ordinal(self) -> u64 {
        self.block_ordinal
    }

    pub const fn point(self) -> ProductionKirV7BridgePointV1 {
        self.point
    }
}

/// Current exact production projection. Non-identity transformations are not admitted by V1.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum ProductionKirV7BridgeMappingV1 {
    ExactCoordinateIdentity,
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ProductionKirV7BridgeRecordV1 {
    simulator_v7: ProductionKirV7BridgeSiteV1,
    neutral_production: ProductionKirV7BridgeSiteV1,
    target_production: ProductionKirV7BridgeSiteV1,
    mapping: ProductionKirV7BridgeMappingV1,
}

impl ProductionKirV7BridgeRecordV1 {
    pub const fn simulator_v7(self) -> ProductionKirV7BridgeSiteV1 {
        self.simulator_v7
    }

    pub const fn neutral_production(self) -> ProductionKirV7BridgeSiteV1 {
        self.neutral_production
    }

    pub const fn target_production(self) -> ProductionKirV7BridgeSiteV1 {
        self.target_production
    }

    pub const fn mapping(self) -> ProductionKirV7BridgeMappingV1 {
        self.mapping
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProductionKirV7BridgeUnavailableV1 {
    /// Exact verified modules differ; the current producer carries no migration evidence.
    NonIdentityStructuralProjectionUnavailable,
    /// The exact site catalog cannot be retained within the V1 bound.
    SiteCatalogLimit,
}

#[derive(Debug)]
#[allow(clippy::large_enum_variant)]
pub enum ProductionKirV7BridgeAdmissionV1 {
    Admitted(ProductionKirV7StructuralBridgeV1),
    Unavailable(ProductionKirV7BridgeUnavailableV1),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProductionKirV7BridgeQueryUnavailableV1 {
    UnknownSimulatorV7Site,
    UnknownNeutralProductionSite,
    UnknownTargetProductionSite,
}

/// Exact handoff boundary from a structural V7 site into the bound Source/ISA catalog.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProductionKirV7BridgeCatalogQueryUnavailableV1 {
    CatalogIdentityMismatch,
    UnknownTargetProductionSite,
    BlockEntryHasNoCatalogOperationCoordinate,
    TerminatorHasNoCatalogOperationCoordinate,
    InvalidCatalogOperationCoordinate,
    CatalogQuery(ProductionSourceIsaCatalogQueryUnavailableV1),
}

#[derive(Debug, Eq, PartialEq)]
pub enum ProductionKirV7BridgeErrorV1 {
    InvalidLength,
    InvalidMagic,
    UnsupportedVersion,
    InvalidHeader,
    InvalidIdentity,
    InvalidCanonicalKirV7,
    InvalidCanonicalProductionKir,
    UnsupportedProductionKirVersion,
    InvalidSourceMapV2,
    SourceMapV7IdentityMismatch,
    SourceMapCatalogIdentityMismatch,
    ProductionKirCatalogIdentityMismatch,
    ArtifactCatalogIdentityMismatch,
    CoordinateShapeMismatch,
    InvalidRecord,
    NonCanonicalRecordOrder,
    DuplicateRecord,
    ExactProjectionMismatch,
    ResourceLimit,
    AllocationFailure,
    SizeOverflow,
}

impl fmt::Display for ProductionKirV7BridgeErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "invalid production KIR V7 structural bridge: {self:?}"
        )
    }
}

impl Error for ProductionKirV7BridgeErrorV1 {}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct BridgeBindingV1 {
    target: ProductionKirV7BridgeTargetV1,
    production_version: ProductionKirV7BridgeKirVersionV1,
    simulator_v7: ProductionKirV7BridgeContentIdentityV1,
    neutral_production: ProductionKirV7BridgeContentIdentityV1,
    target_production: ProductionKirV7BridgeContentIdentityV1,
    structural_identity: [u8; 32],
    source_map_v2: ProductionKirV7BridgeContentIdentityV1,
    artifact: ProductionKirV7BridgeContentIdentityV1,
    catalog_identity: [u8; 32],
    correlation_identity: [u8; 32],
    semantic_map_identity: [u8; 32],
    counts: ProductionSourceIsaCatalogStructuralCountsV1,
}

/// Canonically decoded claims. It exposes no records until exact production inputs are replayed.
#[derive(Debug)]
pub struct InertProductionKirV7StructuralBridgeV1 {
    claimed: ProductionKirV7StructuralBridgeV1,
}

impl InertProductionKirV7StructuralBridgeV1 {
    pub fn from_canonical_bytes(bytes: &[u8]) -> Result<Self, ProductionKirV7BridgeErrorV1> {
        Ok(Self {
            claimed: ProductionKirV7StructuralBridgeV1::decode_claimed(bytes)?,
        })
    }

    pub const fn claimed_identity(&self) -> &[u8; 32] {
        self.claimed.identity()
    }

    pub const fn claimed_catalog_identity(&self) -> &[u8; 32] {
        self.claimed.catalog_identity()
    }

    pub fn admit_exact_projection_v1(
        self,
        canonical_kir_v7: &[u8],
        canonical_production_kir: &[u8],
        source_map_v2: &[u8],
        artifact: &[u8],
        catalog: &ProductionSourceIsaCatalogV1,
    ) -> Result<ProductionKirV7StructuralBridgeV1, ProductionKirV7BridgeErrorV1> {
        let exact = match admit_production_kir_v7_structural_bridge_v1(
            canonical_kir_v7,
            canonical_production_kir,
            source_map_v2,
            artifact,
            catalog,
        )? {
            ProductionKirV7BridgeAdmissionV1::Admitted(bridge) => bridge,
            ProductionKirV7BridgeAdmissionV1::Unavailable(_) => {
                return Err(ProductionKirV7BridgeErrorV1::ExactProjectionMismatch);
            }
        };
        if self.claimed != exact {
            return Err(ProductionKirV7BridgeErrorV1::ExactProjectionMismatch);
        }
        Ok(exact)
    }

    pub const fn proves_semantic_refinement(&self) -> bool {
        false
    }

    pub const fn grants_runtime_authority(&self) -> bool {
        false
    }
}

/// Exact coordinate bridge admitted against one production Source/ISA catalog.
#[derive(Debug, Eq, PartialEq)]
pub struct ProductionKirV7StructuralBridgeV1 {
    identity: [u8; 32],
    binding: BridgeBindingV1,
    records: Vec<ProductionKirV7BridgeRecordV1>,
}

impl ProductionKirV7StructuralBridgeV1 {
    pub const fn format_version(&self) -> u16 {
        PRODUCTION_KIR_V7_BRIDGE_VERSION_V1
    }

    pub const fn identity(&self) -> &[u8; 32] {
        &self.identity
    }

    pub const fn target(&self) -> ProductionKirV7BridgeTargetV1 {
        self.binding.target
    }

    pub const fn production_version(&self) -> ProductionKirV7BridgeKirVersionV1 {
        self.binding.production_version
    }

    pub const fn simulator_v7_identity(&self) -> ProductionKirV7BridgeContentIdentityV1 {
        self.binding.simulator_v7
    }

    pub const fn neutral_production_identity(&self) -> ProductionKirV7BridgeContentIdentityV1 {
        self.binding.neutral_production
    }

    pub const fn target_production_identity(&self) -> ProductionKirV7BridgeContentIdentityV1 {
        self.binding.target_production
    }

    pub const fn structural_identity(&self) -> &[u8; 32] {
        &self.binding.structural_identity
    }

    pub const fn source_map_v2_identity(&self) -> ProductionKirV7BridgeContentIdentityV1 {
        self.binding.source_map_v2
    }

    pub const fn artifact_identity(&self) -> ProductionKirV7BridgeContentIdentityV1 {
        self.binding.artifact
    }

    pub const fn catalog_identity(&self) -> &[u8; 32] {
        &self.binding.catalog_identity
    }

    pub const fn correlation_identity(&self) -> &[u8; 32] {
        &self.binding.correlation_identity
    }

    pub const fn semantic_map_identity(&self) -> &[u8; 32] {
        &self.binding.semantic_map_identity
    }

    pub const fn structural_counts(&self) -> ProductionSourceIsaCatalogStructuralCountsV1 {
        self.binding.counts
    }

    pub fn records(&self) -> &[ProductionKirV7BridgeRecordV1] {
        &self.records
    }

    pub fn query_simulator_v7(
        &self,
        site: ProductionKirV7BridgeSiteV1,
    ) -> Result<ProductionKirV7BridgeRecordV1, ProductionKirV7BridgeQueryUnavailableV1> {
        self.records
            .binary_search_by_key(&site, |record| record.simulator_v7)
            .ok()
            .and_then(|index| self.records.get(index))
            .copied()
            .ok_or(ProductionKirV7BridgeQueryUnavailableV1::UnknownSimulatorV7Site)
    }

    pub fn query_neutral_production(
        &self,
        site: ProductionKirV7BridgeSiteV1,
    ) -> Result<ProductionKirV7BridgeRecordV1, ProductionKirV7BridgeQueryUnavailableV1> {
        self.query_simulator_v7(site)
            .map_err(|_| ProductionKirV7BridgeQueryUnavailableV1::UnknownNeutralProductionSite)
    }

    pub fn query_target_production(
        &self,
        site: ProductionKirV7BridgeSiteV1,
    ) -> Result<ProductionKirV7BridgeRecordV1, ProductionKirV7BridgeQueryUnavailableV1> {
        self.query_simulator_v7(site)
            .map_err(|_| ProductionKirV7BridgeQueryUnavailableV1::UnknownTargetProductionSite)
    }

    /// Queries the exact target-KIR operation in the catalog bound into this bridge.
    ///
    /// Block entries and terminators are structural sites, not catalog operation coordinates.
    /// They remain explicitly unavailable rather than being attributed to a neighboring op.
    pub fn query_target_catalog<'a>(
        &self,
        catalog: &'a ProductionSourceIsaCatalogV1,
        site: ProductionKirV7BridgeSiteV1,
    ) -> Result<
        ProductionSourceIsaCatalogMatchesV1<'a>,
        ProductionKirV7BridgeCatalogQueryUnavailableV1,
    > {
        if catalog.identity() != self.catalog_identity() {
            return Err(ProductionKirV7BridgeCatalogQueryUnavailableV1::CatalogIdentityMismatch);
        }
        let record = self.query_target_production(site).map_err(|_| {
            ProductionKirV7BridgeCatalogQueryUnavailableV1::UnknownTargetProductionSite
        })?;
        let operation_ordinal = match record.target_production.point {
            ProductionKirV7BridgePointV1::Operation { operation_ordinal } => operation_ordinal,
            ProductionKirV7BridgePointV1::BlockEntry => {
                return Err(
                    ProductionKirV7BridgeCatalogQueryUnavailableV1::BlockEntryHasNoCatalogOperationCoordinate,
                );
            }
            ProductionKirV7BridgePointV1::Terminator => {
                return Err(
                    ProductionKirV7BridgeCatalogQueryUnavailableV1::TerminatorHasNoCatalogOperationCoordinate,
                );
            }
        };
        let coordinate = ProductionSourceIsaKirCoordinateV1::new(
            record.target_production.function_ordinal,
            record.target_production.block_ordinal,
            operation_ordinal,
        )
        .map_err(|_| {
            ProductionKirV7BridgeCatalogQueryUnavailableV1::InvalidCatalogOperationCoordinate
        })?;
        catalog
            .query_target_kir(coordinate)
            .map_err(ProductionKirV7BridgeCatalogQueryUnavailableV1::CatalogQuery)
    }

    pub fn to_canonical_bytes(&self) -> Result<Vec<u8>, ProductionKirV7BridgeErrorV1> {
        let mut bytes = self.canonical_preimage()?;
        bytes.extend_from_slice(&self.identity);
        Ok(bytes)
    }

    pub const fn proves_exact_coordinate_identity(&self) -> bool {
        true
    }

    pub const fn proves_semantic_refinement(&self) -> bool {
        false
    }

    pub const fn proves_source_attribution_for_every_site(&self) -> bool {
        false
    }

    pub const fn proves_a_schedule(&self) -> bool {
        false
    }

    pub const fn proves_complete_machine_instruction_coverage(&self) -> bool {
        false
    }

    pub const fn proves_live_program_counter_ownership(&self) -> bool {
        false
    }

    pub const fn proves_gpu_observation(&self) -> bool {
        false
    }

    pub const fn authenticates_compiler_execution(&self) -> bool {
        false
    }

    pub const fn grants_debugger_authority(&self) -> bool {
        false
    }

    pub const fn grants_profiler_authority(&self) -> bool {
        false
    }

    pub const fn grants_publication_authority(&self) -> bool {
        false
    }

    pub const fn grants_load_or_launch_authority(&self) -> bool {
        false
    }

    pub const fn grants_runtime_authority(&self) -> bool {
        false
    }

    fn canonical_preimage(&self) -> Result<Vec<u8>, ProductionKirV7BridgeErrorV1> {
        validate_bridge(self)?;
        let records_bytes = self
            .records
            .len()
            .checked_mul(BRIDGE_RECORD_BYTES_V1)
            .ok_or(ProductionKirV7BridgeErrorV1::SizeOverflow)?;
        let total = BRIDGE_HEADER_BYTES_V1
            .checked_add(records_bytes)
            .and_then(|bytes| bytes.checked_add(BRIDGE_IDENTITY_BYTES_V1))
            .ok_or(ProductionKirV7BridgeErrorV1::SizeOverflow)?;
        if total > MAX_PRODUCTION_KIR_V7_BRIDGE_BYTES_V1 {
            return Err(ProductionKirV7BridgeErrorV1::ResourceLimit);
        }
        let mut bytes = Vec::new();
        bytes
            .try_reserve_exact(total)
            .map_err(|_| ProductionKirV7BridgeErrorV1::AllocationFailure)?;
        bytes.extend_from_slice(&PRODUCTION_KIR_V7_BRIDGE_MAGIC_V1);
        bytes.extend_from_slice(&PRODUCTION_KIR_V7_BRIDGE_VERSION_V1.to_le_bytes());
        bytes.extend_from_slice(&(BRIDGE_HEADER_BYTES_V1 as u16).to_le_bytes());
        bytes.extend_from_slice(&0_u32.to_le_bytes());
        bytes.extend_from_slice(&(total as u64).to_le_bytes());
        bytes.extend_from_slice(&(self.records.len() as u64).to_le_bytes());
        encode_content(&mut bytes, self.binding.simulator_v7);
        bytes.push(encode_version(self.binding.production_version));
        bytes.push(encode_target(self.binding.target));
        bytes.extend_from_slice(&[0; 6]);
        encode_content(&mut bytes, self.binding.neutral_production);
        encode_content(&mut bytes, self.binding.target_production);
        bytes.extend_from_slice(&self.binding.structural_identity);
        encode_content(&mut bytes, self.binding.source_map_v2);
        encode_content(&mut bytes, self.binding.artifact);
        bytes.extend_from_slice(&self.binding.catalog_identity);
        bytes.extend_from_slice(&self.binding.correlation_identity);
        bytes.extend_from_slice(&self.binding.semantic_map_identity);
        encode_counts(&mut bytes, self.binding.counts);
        if bytes.len() != BRIDGE_HEADER_BYTES_V1 {
            return Err(ProductionKirV7BridgeErrorV1::InvalidHeader);
        }
        for record in &self.records {
            encode_site(&mut bytes, record.simulator_v7);
            encode_site(&mut bytes, record.neutral_production);
            encode_site(&mut bytes, record.target_production);
            bytes.push(0);
            bytes.extend_from_slice(&[0; 7]);
        }
        Ok(bytes)
    }

    fn decode_claimed(bytes: &[u8]) -> Result<Self, ProductionKirV7BridgeErrorV1> {
        if bytes.len() < BRIDGE_HEADER_BYTES_V1 + BRIDGE_IDENTITY_BYTES_V1
            || bytes.len() > MAX_PRODUCTION_KIR_V7_BRIDGE_BYTES_V1
        {
            return Err(ProductionKirV7BridgeErrorV1::InvalidLength);
        }
        let mut reader = ReaderV1::new(bytes);
        if reader.take(8)? != PRODUCTION_KIR_V7_BRIDGE_MAGIC_V1 {
            return Err(ProductionKirV7BridgeErrorV1::InvalidMagic);
        }
        if reader.u16()? != PRODUCTION_KIR_V7_BRIDGE_VERSION_V1 {
            return Err(ProductionKirV7BridgeErrorV1::UnsupportedVersion);
        }
        if usize::from(reader.u16()?) != BRIDGE_HEADER_BYTES_V1 || reader.u32()? != 0 {
            return Err(ProductionKirV7BridgeErrorV1::InvalidHeader);
        }
        let total = usize::try_from(reader.u64()?)
            .map_err(|_| ProductionKirV7BridgeErrorV1::ResourceLimit)?;
        let record_count = usize::try_from(reader.u64()?)
            .map_err(|_| ProductionKirV7BridgeErrorV1::ResourceLimit)?;
        if total != bytes.len() || record_count > MAX_PRODUCTION_KIR_V7_BRIDGE_RECORDS_V1 {
            return Err(ProductionKirV7BridgeErrorV1::ResourceLimit);
        }
        let binding = BridgeBindingV1 {
            simulator_v7: reader.content()?,
            production_version: decode_version(reader.u8()?)?,
            target: decode_target(reader.u8()?)?,
            neutral_production: {
                if reader.take(6)? != [0; 6] {
                    return Err(ProductionKirV7BridgeErrorV1::InvalidHeader);
                }
                reader.content()?
            },
            target_production: reader.content()?,
            structural_identity: reader.identity()?,
            source_map_v2: reader.content()?,
            artifact: reader.content()?,
            catalog_identity: reader.identity()?,
            correlation_identity: reader.identity()?,
            semantic_map_identity: reader.identity()?,
            counts: reader.counts()?,
        };
        if reader.position() != BRIDGE_HEADER_BYTES_V1 {
            return Err(ProductionKirV7BridgeErrorV1::InvalidHeader);
        }
        let expected = BRIDGE_HEADER_BYTES_V1
            .checked_add(
                record_count
                    .checked_mul(BRIDGE_RECORD_BYTES_V1)
                    .ok_or(ProductionKirV7BridgeErrorV1::SizeOverflow)?,
            )
            .and_then(|value| value.checked_add(BRIDGE_IDENTITY_BYTES_V1))
            .ok_or(ProductionKirV7BridgeErrorV1::SizeOverflow)?;
        if expected != total {
            return Err(ProductionKirV7BridgeErrorV1::InvalidLength);
        }
        let mut records = Vec::new();
        records
            .try_reserve_exact(record_count)
            .map_err(|_| ProductionKirV7BridgeErrorV1::AllocationFailure)?;
        for _ in 0..record_count {
            let simulator_v7 = reader.site()?;
            let neutral_production = reader.site()?;
            let target_production = reader.site()?;
            if reader.u8()? != 0 || reader.take(7)? != [0; 7] {
                return Err(ProductionKirV7BridgeErrorV1::InvalidRecord);
            }
            records.push(ProductionKirV7BridgeRecordV1 {
                simulator_v7,
                neutral_production,
                target_production,
                mapping: ProductionKirV7BridgeMappingV1::ExactCoordinateIdentity,
            });
        }
        let claimed_identity = reader.identity()?;
        if reader.remaining() != 0 {
            return Err(ProductionKirV7BridgeErrorV1::InvalidLength);
        }
        let calculated = bridge_identity(&bytes[..bytes.len() - BRIDGE_IDENTITY_BYTES_V1]);
        if claimed_identity != calculated {
            return Err(ProductionKirV7BridgeErrorV1::InvalidIdentity);
        }
        let bridge = Self {
            identity: claimed_identity,
            binding,
            records,
        };
        validate_bridge(&bridge)?;
        Ok(bridge)
    }
}

/// Reconstructs the complete exact structural projection from production-admitted inputs.
pub fn admit_production_kir_v7_structural_bridge_v1(
    canonical_kir_v7: &[u8],
    canonical_production_kir: &[u8],
    source_map_v2: &[u8],
    artifact: &[u8],
    catalog: &ProductionSourceIsaCatalogV1,
) -> Result<ProductionKirV7BridgeAdmissionV1, ProductionKirV7BridgeErrorV1> {
    preflight_kir(canonical_kir_v7)?;
    preflight_kir(canonical_production_kir)?;
    let (v7, module_v7) = verified_v7(canonical_kir_v7)?;
    let structural = catalog.structural_binding();
    let binding = binding_from_catalog(&v7, artifact, catalog, structural)?;
    admit_exact_bridge_with_v7(
        v7,
        module_v7,
        canonical_production_kir,
        source_map_v2,
        binding,
    )
}

fn binding_from_catalog(
    v7: &VerifiedCanonicalKernelIrV7,
    artifact: &[u8],
    catalog: &ProductionSourceIsaCatalogV1,
    structural: ProductionSourceIsaCatalogStructuralBindingV1,
) -> Result<BridgeBindingV1, ProductionKirV7BridgeErrorV1> {
    if artifact.is_empty() || artifact.len() > fe2o3_hsaco::MAX_HSACO_BYTES {
        return Err(ProductionKirV7BridgeErrorV1::InvalidLength);
    }
    let artifact_identity = ContentIdentityV1::calculate(artifact);
    if artifact_identity != catalog.artifact_identity() {
        return Err(ProductionKirV7BridgeErrorV1::ArtifactCatalogIdentityMismatch);
    }
    Ok(BridgeBindingV1 {
        target: match structural.target() {
            ProductionSourceIsaCatalogTargetV1::Gfx942 => ProductionKirV7BridgeTargetV1::Gfx942,
            ProductionSourceIsaCatalogTargetV1::Gfx950 => ProductionKirV7BridgeTargetV1::Gfx950,
        },
        production_version: match structural.kir_version() {
            ProductionSourceIsaCatalogKirVersionV1::V8 => ProductionKirV7BridgeKirVersionV1::V8,
            ProductionSourceIsaCatalogKirVersionV1::V9 => {
                return Err(ProductionKirV7BridgeErrorV1::UnsupportedProductionKirVersion);
            }
        },
        simulator_v7: ProductionKirV7BridgeContentIdentityV1 {
            sha256: *v7.identity().digest(),
            byte_len: v7.identity().canonical_length(),
        },
        neutral_production: content_from_catalog(structural.neutral_kernel_ir()),
        target_production: content_from_catalog(structural.target_bound_kernel_ir()),
        structural_identity: structural.identity(),
        source_map_v2: content_from_catalog(catalog.source_map_v2_identity()),
        artifact: ProductionKirV7BridgeContentIdentityV1 {
            sha256: *artifact_identity.sha256(),
            byte_len: artifact_identity.byte_len(),
        },
        catalog_identity: *catalog.identity(),
        correlation_identity: *catalog.correlation_identity(),
        semantic_map_identity: *catalog.semantic_map_identity(),
        counts: structural.counts(),
    })
}

#[cfg(test)]
fn admit_exact_bridge(
    canonical_kir_v7: &[u8],
    canonical_production_kir: &[u8],
    source_map_v2: &[u8],
    binding: BridgeBindingV1,
) -> Result<ProductionKirV7BridgeAdmissionV1, ProductionKirV7BridgeErrorV1> {
    preflight_kir(canonical_kir_v7)?;
    preflight_kir(canonical_production_kir)?;
    validate_binding(binding)?;
    let (v7, module_v7) = verified_v7(canonical_kir_v7)?;
    admit_exact_bridge_with_v7(
        v7,
        module_v7,
        canonical_production_kir,
        source_map_v2,
        binding,
    )
}

fn admit_exact_bridge_with_v7(
    v7: VerifiedCanonicalKernelIrV7,
    module_v7: Module,
    canonical_production_kir: &[u8],
    source_map_v2: &[u8],
    binding: BridgeBindingV1,
) -> Result<ProductionKirV7BridgeAdmissionV1, ProductionKirV7BridgeErrorV1> {
    validate_binding(binding)?;
    if binding.simulator_v7
        != (ProductionKirV7BridgeContentIdentityV1 {
            sha256: *v7.identity().digest(),
            byte_len: v7.identity().canonical_length(),
        })
    {
        return Err(ProductionKirV7BridgeErrorV1::SourceMapV7IdentityMismatch);
    }
    let source_map = DebugSourceMapDocumentV2::from_canonical_json_bytes(source_map_v2)
        .map_err(|_| ProductionKirV7BridgeErrorV1::InvalidSourceMapV2)?;
    let source_kir = source_map.binding().canonical_kir();
    if source_kir.digest() != binding.simulator_v7.sha256
        || source_kir.canonical_bytes() != binding.simulator_v7.byte_len
    {
        return Err(ProductionKirV7BridgeErrorV1::SourceMapV7IdentityMismatch);
    }
    let source_identity = raw_content(source_map_v2)?;
    if source_identity != binding.source_map_v2 {
        return Err(ProductionKirV7BridgeErrorV1::SourceMapCatalogIdentityMismatch);
    }

    preflight_kir(canonical_production_kir)?;
    let (owner, module_production) = VerifiedCanonicalKernelIrV8::from_canonical_bytes_with_module(
        copy_bytes(canonical_production_kir)?,
    )
    .map_err(|_| ProductionKirV7BridgeErrorV1::InvalidCanonicalProductionKir)?;
    let actual = ProductionKirV7BridgeContentIdentityV1 {
        sha256: *owner.identity().digest(),
        byte_len: owner.identity().canonical_length(),
    };
    if actual != binding.neutral_production {
        return Err(ProductionKirV7BridgeErrorV1::ProductionKirCatalogIdentityMismatch);
    }
    if module_v7 != module_production {
        return Ok(ProductionKirV7BridgeAdmissionV1::Unavailable(
            ProductionKirV7BridgeUnavailableV1::NonIdentityStructuralProjectionUnavailable,
        ));
    }
    if !counts_match(&module_v7, binding.counts)? {
        return Err(ProductionKirV7BridgeErrorV1::CoordinateShapeMismatch);
    }
    let records = match exact_site_records(&module_v7)? {
        Some(records) => records,
        None => {
            return Ok(ProductionKirV7BridgeAdmissionV1::Unavailable(
                ProductionKirV7BridgeUnavailableV1::SiteCatalogLimit,
            ));
        }
    };
    let mut bridge = ProductionKirV7StructuralBridgeV1 {
        identity: [0; 32],
        binding,
        records,
    };
    let preimage = bridge.canonical_preimage()?;
    bridge.identity = bridge_identity(&preimage);
    Ok(ProductionKirV7BridgeAdmissionV1::Admitted(bridge))
}

fn verified_v7(
    bytes: &[u8],
) -> Result<(VerifiedCanonicalKernelIrV7, Module), ProductionKirV7BridgeErrorV1> {
    preflight_kir(bytes)?;
    VerifiedCanonicalKernelIrV7::from_canonical_bytes_with_module(copy_bytes(bytes)?)
        .map_err(|_| ProductionKirV7BridgeErrorV1::InvalidCanonicalKirV7)
}

fn preflight_kir(bytes: &[u8]) -> Result<(), ProductionKirV7BridgeErrorV1> {
    if bytes.is_empty() || bytes.len() > MAX_MODULE_BYTES_V1 {
        return Err(ProductionKirV7BridgeErrorV1::InvalidLength);
    }
    Ok(())
}

fn exact_site_records(
    module: &Module,
) -> Result<Option<Vec<ProductionKirV7BridgeRecordV1>>, ProductionKirV7BridgeErrorV1> {
    let count = module
        .functions
        .iter()
        .try_fold(0_usize, |total, function| {
            let Some(body) = &function.body else {
                return Some(total);
            };
            body.blocks.iter().try_fold(total, |total, block| {
                total.checked_add(block.operations.len().checked_add(2)?)
            })
        });
    let Some(count) = count else {
        return Err(ProductionKirV7BridgeErrorV1::SizeOverflow);
    };
    let encoded = BRIDGE_HEADER_BYTES_V1
        .checked_add(
            count
                .checked_mul(BRIDGE_RECORD_BYTES_V1)
                .ok_or(ProductionKirV7BridgeErrorV1::SizeOverflow)?,
        )
        .and_then(|bytes| bytes.checked_add(BRIDGE_IDENTITY_BYTES_V1))
        .ok_or(ProductionKirV7BridgeErrorV1::SizeOverflow)?;
    if count > MAX_PRODUCTION_KIR_V7_BRIDGE_RECORDS_V1
        || encoded > MAX_PRODUCTION_KIR_V7_BRIDGE_BYTES_V1
    {
        return Ok(None);
    }
    let mut records = Vec::new();
    records
        .try_reserve_exact(count)
        .map_err(|_| ProductionKirV7BridgeErrorV1::AllocationFailure)?;
    for (function_ordinal, function) in module.functions.iter().enumerate() {
        let Some(body) = &function.body else {
            continue;
        };
        for (block_ordinal, block) in body.blocks.iter().enumerate() {
            let function_ordinal = u64::try_from(function_ordinal)
                .map_err(|_| ProductionKirV7BridgeErrorV1::ResourceLimit)?;
            let block_ordinal = u64::try_from(block_ordinal)
                .map_err(|_| ProductionKirV7BridgeErrorV1::ResourceLimit)?;
            push_exact_record(
                &mut records,
                ProductionKirV7BridgeSiteV1::block_entry(function_ordinal, block_ordinal),
            );
            for operation_ordinal in 0..block.operations.len() {
                push_exact_record(
                    &mut records,
                    ProductionKirV7BridgeSiteV1::operation(
                        function_ordinal,
                        block_ordinal,
                        u64::try_from(operation_ordinal)
                            .map_err(|_| ProductionKirV7BridgeErrorV1::ResourceLimit)?,
                    ),
                );
            }
            push_exact_record(
                &mut records,
                ProductionKirV7BridgeSiteV1::terminator(function_ordinal, block_ordinal),
            );
        }
    }
    Ok(Some(records))
}

fn push_exact_record(
    records: &mut Vec<ProductionKirV7BridgeRecordV1>,
    site: ProductionKirV7BridgeSiteV1,
) {
    records.push(ProductionKirV7BridgeRecordV1 {
        simulator_v7: site,
        neutral_production: site,
        target_production: site,
        mapping: ProductionKirV7BridgeMappingV1::ExactCoordinateIdentity,
    });
}

fn validate_bridge(
    bridge: &ProductionKirV7StructuralBridgeV1,
) -> Result<(), ProductionKirV7BridgeErrorV1> {
    validate_binding(bridge.binding)?;
    if bridge.records.len() > MAX_PRODUCTION_KIR_V7_BRIDGE_RECORDS_V1 {
        return Err(ProductionKirV7BridgeErrorV1::ResourceLimit);
    }
    let expected_records = bridge
        .binding
        .counts
        .blocks()
        .checked_mul(2)
        .and_then(|count| count.checked_add(bridge.binding.counts.operations()))
        .ok_or(ProductionKirV7BridgeErrorV1::SizeOverflow)?;
    if u64::try_from(bridge.records.len())
        .map_err(|_| ProductionKirV7BridgeErrorV1::ResourceLimit)?
        != expected_records
    {
        return Err(ProductionKirV7BridgeErrorV1::CoordinateShapeMismatch);
    }
    for record in &bridge.records {
        if record.mapping != ProductionKirV7BridgeMappingV1::ExactCoordinateIdentity
            || record.simulator_v7 != record.neutral_production
            || record.simulator_v7 != record.target_production
        {
            return Err(ProductionKirV7BridgeErrorV1::InvalidRecord);
        }
    }
    if bridge
        .records
        .windows(2)
        .any(|pair| pair[0].simulator_v7 > pair[1].simulator_v7)
    {
        return Err(ProductionKirV7BridgeErrorV1::NonCanonicalRecordOrder);
    }
    if bridge
        .records
        .windows(2)
        .any(|pair| pair[0].simulator_v7 == pair[1].simulator_v7)
    {
        return Err(ProductionKirV7BridgeErrorV1::DuplicateRecord);
    }
    Ok(())
}

fn validate_binding(binding: BridgeBindingV1) -> Result<(), ProductionKirV7BridgeErrorV1> {
    if !binding.simulator_v7.validate()
        || !binding.neutral_production.validate()
        || !binding.target_production.validate()
        || !binding.source_map_v2.validate()
        || !binding.artifact.validate()
        || binding.structural_identity == [0; 32]
        || binding.catalog_identity == [0; 32]
        || binding.correlation_identity == [0; 32]
        || binding.semantic_map_identity == [0; 32]
        || binding.counts.functions() == 0
        || binding.counts.defined_bodies() == 0
        || binding.counts.blocks() == 0
        || binding.counts.defined_bodies() > binding.counts.functions()
        || binding.counts.blocks() < binding.counts.defined_bodies()
    {
        return Err(ProductionKirV7BridgeErrorV1::InvalidIdentity);
    }
    Ok(())
}

fn counts_match(
    module: &Module,
    expected: ProductionSourceIsaCatalogStructuralCountsV1,
) -> Result<bool, ProductionKirV7BridgeErrorV1> {
    let functions = u64::try_from(module.functions.len())
        .map_err(|_| ProductionKirV7BridgeErrorV1::ResourceLimit)?;
    let mut defined_bodies = 0_u64;
    let mut blocks = 0_u64;
    let mut operations = 0_u64;
    for function in &module.functions {
        let Some(body) = &function.body else {
            continue;
        };
        defined_bodies = defined_bodies
            .checked_add(1)
            .ok_or(ProductionKirV7BridgeErrorV1::SizeOverflow)?;
        blocks = blocks
            .checked_add(
                u64::try_from(body.blocks.len())
                    .map_err(|_| ProductionKirV7BridgeErrorV1::ResourceLimit)?,
            )
            .ok_or(ProductionKirV7BridgeErrorV1::SizeOverflow)?;
        for block in &body.blocks {
            operations = operations
                .checked_add(
                    u64::try_from(block.operations.len())
                        .map_err(|_| ProductionKirV7BridgeErrorV1::ResourceLimit)?,
                )
                .ok_or(ProductionKirV7BridgeErrorV1::SizeOverflow)?;
        }
    }
    Ok(functions == expected.functions()
        && defined_bodies == expected.defined_bodies()
        && blocks == expected.blocks()
        && operations == expected.operations())
}

fn content_from_catalog(
    identity: ProductionSourceIsaCatalogContentIdentityV1,
) -> ProductionKirV7BridgeContentIdentityV1 {
    ProductionKirV7BridgeContentIdentityV1 {
        sha256: identity.sha256(),
        byte_len: identity.byte_len(),
    }
}

fn raw_content(
    bytes: &[u8],
) -> Result<ProductionKirV7BridgeContentIdentityV1, ProductionKirV7BridgeErrorV1> {
    if bytes.is_empty() {
        return Err(ProductionKirV7BridgeErrorV1::InvalidLength);
    }
    Ok(ProductionKirV7BridgeContentIdentityV1 {
        sha256: Sha256::digest(bytes).into(),
        byte_len: u64::try_from(bytes.len())
            .map_err(|_| ProductionKirV7BridgeErrorV1::ResourceLimit)?,
    })
}

fn copy_bytes(bytes: &[u8]) -> Result<Vec<u8>, ProductionKirV7BridgeErrorV1> {
    let mut copy = Vec::new();
    copy.try_reserve_exact(bytes.len())
        .map_err(|_| ProductionKirV7BridgeErrorV1::AllocationFailure)?;
    copy.extend_from_slice(bytes);
    Ok(copy)
}

fn bridge_identity(preimage: &[u8]) -> [u8; 32] {
    let mut digest = Sha256::new();
    digest.update((BRIDGE_IDENTITY_DOMAIN_V1.len() as u32).to_le_bytes());
    digest.update(BRIDGE_IDENTITY_DOMAIN_V1);
    digest.update(preimage);
    digest.finalize().into()
}

const fn encode_version(version: ProductionKirV7BridgeKirVersionV1) -> u8 {
    match version {
        ProductionKirV7BridgeKirVersionV1::V8 => 8,
    }
}

fn decode_version(
    value: u8,
) -> Result<ProductionKirV7BridgeKirVersionV1, ProductionKirV7BridgeErrorV1> {
    match value {
        8 => Ok(ProductionKirV7BridgeKirVersionV1::V8),
        _ => Err(ProductionKirV7BridgeErrorV1::InvalidHeader),
    }
}

const fn encode_target(target: ProductionKirV7BridgeTargetV1) -> u8 {
    match target {
        ProductionKirV7BridgeTargetV1::Gfx942 => 1,
        ProductionKirV7BridgeTargetV1::Gfx950 => 2,
    }
}

fn decode_target(value: u8) -> Result<ProductionKirV7BridgeTargetV1, ProductionKirV7BridgeErrorV1> {
    match value {
        1 => Ok(ProductionKirV7BridgeTargetV1::Gfx942),
        2 => Ok(ProductionKirV7BridgeTargetV1::Gfx950),
        _ => Err(ProductionKirV7BridgeErrorV1::InvalidHeader),
    }
}

fn encode_content(output: &mut Vec<u8>, value: ProductionKirV7BridgeContentIdentityV1) {
    output.extend_from_slice(&value.sha256);
    output.extend_from_slice(&value.byte_len.to_le_bytes());
}

fn encode_counts(output: &mut Vec<u8>, value: ProductionSourceIsaCatalogStructuralCountsV1) {
    output.extend_from_slice(&value.functions().to_le_bytes());
    output.extend_from_slice(&value.defined_bodies().to_le_bytes());
    output.extend_from_slice(&value.blocks().to_le_bytes());
    output.extend_from_slice(&value.operations().to_le_bytes());
}

fn encode_site(output: &mut Vec<u8>, site: ProductionKirV7BridgeSiteV1) {
    output.extend_from_slice(&site.function_ordinal.to_le_bytes());
    output.extend_from_slice(&site.block_ordinal.to_le_bytes());
    let (tag, ordinal) = match site.point {
        ProductionKirV7BridgePointV1::BlockEntry => (0, 0),
        ProductionKirV7BridgePointV1::Operation { operation_ordinal } => (1, operation_ordinal),
        ProductionKirV7BridgePointV1::Terminator => (2, 0),
    };
    output.push(tag);
    output.extend_from_slice(&[0; 7]);
    output.extend_from_slice(&ordinal.to_le_bytes());
}

struct ReaderV1<'a> {
    bytes: &'a [u8],
    position: usize,
}

impl<'a> ReaderV1<'a> {
    const fn new(bytes: &'a [u8]) -> Self {
        Self { bytes, position: 0 }
    }

    fn take(&mut self, count: usize) -> Result<&'a [u8], ProductionKirV7BridgeErrorV1> {
        let end = self
            .position
            .checked_add(count)
            .ok_or(ProductionKirV7BridgeErrorV1::SizeOverflow)?;
        let value = self
            .bytes
            .get(self.position..end)
            .ok_or(ProductionKirV7BridgeErrorV1::InvalidLength)?;
        self.position = end;
        Ok(value)
    }

    fn u8(&mut self) -> Result<u8, ProductionKirV7BridgeErrorV1> {
        Ok(self.take(1)?[0])
    }

    fn u16(&mut self) -> Result<u16, ProductionKirV7BridgeErrorV1> {
        Ok(u16::from_le_bytes(self.take(2)?.try_into().unwrap()))
    }

    fn u32(&mut self) -> Result<u32, ProductionKirV7BridgeErrorV1> {
        Ok(u32::from_le_bytes(self.take(4)?.try_into().unwrap()))
    }

    fn u64(&mut self) -> Result<u64, ProductionKirV7BridgeErrorV1> {
        Ok(u64::from_le_bytes(self.take(8)?.try_into().unwrap()))
    }

    fn identity(&mut self) -> Result<[u8; 32], ProductionKirV7BridgeErrorV1> {
        Ok(self.take(32)?.try_into().unwrap())
    }

    fn content(
        &mut self,
    ) -> Result<ProductionKirV7BridgeContentIdentityV1, ProductionKirV7BridgeErrorV1> {
        Ok(ProductionKirV7BridgeContentIdentityV1 {
            sha256: self.identity()?,
            byte_len: self.u64()?,
        })
    }

    fn counts(
        &mut self,
    ) -> Result<ProductionSourceIsaCatalogStructuralCountsV1, ProductionKirV7BridgeErrorV1> {
        Ok(
            ProductionSourceIsaCatalogStructuralCountsV1::new_for_bridge_v1(
                self.u64()?,
                self.u64()?,
                self.u64()?,
                self.u64()?,
            ),
        )
    }

    fn site(&mut self) -> Result<ProductionKirV7BridgeSiteV1, ProductionKirV7BridgeErrorV1> {
        let function_ordinal = self.u64()?;
        let block_ordinal = self.u64()?;
        let tag = self.u8()?;
        if self.take(7)? != [0; 7] {
            return Err(ProductionKirV7BridgeErrorV1::InvalidRecord);
        }
        let ordinal = self.u64()?;
        let point = match (tag, ordinal) {
            (0, 0) => ProductionKirV7BridgePointV1::BlockEntry,
            (1, operation_ordinal) => ProductionKirV7BridgePointV1::Operation { operation_ordinal },
            (2, 0) => ProductionKirV7BridgePointV1::Terminator,
            _ => return Err(ProductionKirV7BridgeErrorV1::InvalidRecord),
        };
        Ok(ProductionKirV7BridgeSiteV1 {
            function_ordinal,
            block_ordinal,
            point,
        })
    }

    const fn position(&self) -> usize {
        self.position
    }

    const fn remaining(&self) -> usize {
        self.bytes.len() - self.position
    }
}

#[cfg(test)]
mod tests {
    use fe2o3_kernel_ir::{
        AddressSpace, BarrierSemantics, BasicBlock, BlockId, Constant, Convergence,
        DebugSourceMapBindingV1, DebugSourceMapDocumentV2, DebugSourceMapFileV1, Function,
        MemoryOrdering, Module, Operation, OperationKind, Signature, SynchronizationScope,
        Terminator, Type, ValueDef, ValueId, VerifiedCanonicalKernelIrV7,
        VerifiedCanonicalKernelIrV8, WorkgroupBarrier,
    };

    use super::*;

    struct FixtureV1 {
        canonical_v7: Vec<u8>,
        canonical_v8: Vec<u8>,
        source_map: Vec<u8>,
        binding: BridgeBindingV1,
        bridge: ProductionKirV7StructuralBridgeV1,
    }

    fn module(tag: u64) -> Module {
        let mut entry = BasicBlock::new(BlockId(0));
        entry.operations.push(Operation::new(
            vec![ValueDef::new(ValueId(0), Type::INDEX)],
            OperationKind::Constant(Constant::Index(tag)),
        ));
        entry.operations.push(Operation::new(
            Vec::new(),
            OperationKind::WorkgroupBarrier(WorkgroupBarrier {
                memory_scope: SynchronizationScope::Workgroup,
                semantics: BarrierSemantics::new(
                    MemoryOrdering::AcquireRelease,
                    [AddressSpace::Workgroup],
                ),
                convergence: Convergence::uniform(SynchronizationScope::Workgroup),
            }),
        ));
        entry.terminator = Some(Terminator::Branch {
            target: BlockId(1),
            arguments: Vec::new(),
        });
        let mut exit = BasicBlock::new(BlockId(1));
        exit.terminator = Some(Terminator::Return { values: Vec::new() });
        let mut module = Module::new(format!("bridge-{tag}"));
        module.functions.push(Function::definition(
            "kernel",
            Signature::new(Vec::new(), Vec::new()),
            Vec::new(),
            vec![entry, exit],
        ));
        module.functions.push(Function::declaration(
            "external",
            Signature::new(Vec::new(), Vec::new()),
        ));
        module
    }

    fn source_map(v7: &VerifiedCanonicalKernelIrV7) -> Vec<u8> {
        DebugSourceMapDocumentV2::new(
            DebugSourceMapBindingV1::new(
                [0x11; 32],
                *v7.identity().digest(),
                v7.identity().canonical_length(),
            )
            .unwrap(),
            vec![DebugSourceMapFileV1::new([0x22; 32], 64, "/src/kernel.rs".into()).unwrap()],
            Vec::new(),
            Vec::new(),
            Vec::new(),
            Vec::new(),
        )
        .unwrap()
        .to_canonical_json_bytes()
        .unwrap()
    }

    fn fixture() -> FixtureV1 {
        let module = module(7);
        let v7 = VerifiedCanonicalKernelIrV7::from_module(module.clone()).unwrap();
        let v8 = VerifiedCanonicalKernelIrV8::from_module(module).unwrap();
        let source_map = source_map(&v7);
        let canonical_v7 = v7.into_canonical_bytes();
        let canonical_v8 = v8.canonical_bytes().to_vec();
        let binding = BridgeBindingV1 {
            target: ProductionKirV7BridgeTargetV1::Gfx942,
            production_version: ProductionKirV7BridgeKirVersionV1::V8,
            simulator_v7: ProductionKirV7BridgeContentIdentityV1 {
                sha256: *VerifiedCanonicalKernelIrV7::from_canonical_bytes(canonical_v7.clone())
                    .unwrap()
                    .identity()
                    .digest(),
                byte_len: canonical_v7.len() as u64,
            },
            neutral_production: ProductionKirV7BridgeContentIdentityV1 {
                sha256: *v8.identity().digest(),
                byte_len: v8.identity().canonical_length(),
            },
            target_production: ProductionKirV7BridgeContentIdentityV1 {
                sha256: [0x33; 32],
                byte_len: canonical_v8.len() as u64,
            },
            structural_identity: [0x44; 32],
            source_map_v2: raw_content(&source_map).unwrap(),
            artifact: raw_content(b"exact-artifact").unwrap(),
            catalog_identity: [0x55; 32],
            correlation_identity: [0x66; 32],
            semantic_map_identity: [0x77; 32],
            counts: ProductionSourceIsaCatalogStructuralCountsV1::new_for_bridge_v1(2, 1, 2, 2),
        };
        let ProductionKirV7BridgeAdmissionV1::Admitted(bridge) =
            admit_exact_bridge(&canonical_v7, &canonical_v8, &source_map, binding).unwrap()
        else {
            panic!("exact V8 bridge must be admitted")
        };
        FixtureV1 {
            canonical_v7,
            canonical_v8,
            source_map,
            binding,
            bridge,
        }
    }

    fn remint(mut bridge: ProductionKirV7StructuralBridgeV1) -> Vec<u8> {
        let preimage = bridge.canonical_preimage().unwrap();
        bridge.identity = bridge_identity(&preimage);
        bridge.to_canonical_bytes().unwrap()
    }

    fn resign(bytes: &mut [u8]) {
        let identity_offset = bytes.len() - BRIDGE_IDENTITY_BYTES_V1;
        let identity = bridge_identity(&bytes[..identity_offset]);
        bytes[identity_offset..].copy_from_slice(&identity);
    }

    #[test]
    fn exact_bridge_round_trips_block_operation_barrier_and_return_coordinates() {
        let fixture = fixture();
        let block_entry = ProductionKirV7BridgeSiteV1::block_entry(0, 0);
        let operation = ProductionKirV7BridgeSiteV1::operation(0, 0, 0);
        let barrier = ProductionKirV7BridgeSiteV1::operation(0, 0, 1);
        let branch = ProductionKirV7BridgeSiteV1::terminator(0, 0);
        let return_terminator = ProductionKirV7BridgeSiteV1::terminator(0, 1);
        assert_eq!(fixture.bridge.records().len(), 6);
        assert_eq!(
            fixture
                .bridge
                .query_simulator_v7(block_entry)
                .unwrap()
                .neutral_production(),
            block_entry
        );
        assert_eq!(
            fixture
                .bridge
                .query_simulator_v7(operation)
                .unwrap()
                .target_production(),
            operation
        );
        assert_eq!(
            fixture
                .bridge
                .query_neutral_production(barrier)
                .unwrap()
                .simulator_v7(),
            barrier
        );
        assert_eq!(
            fixture
                .bridge
                .query_target_production(branch)
                .unwrap()
                .simulator_v7(),
            branch
        );
        assert_eq!(
            fixture
                .bridge
                .query_target_production(return_terminator)
                .unwrap()
                .neutral_production(),
            return_terminator
        );
        assert_eq!(
            fixture
                .bridge
                .query_simulator_v7(ProductionKirV7BridgeSiteV1::operation(0, 1, 0)),
            Err(ProductionKirV7BridgeQueryUnavailableV1::UnknownSimulatorV7Site)
        );
        assert_eq!(
            fixture
                .bridge
                .query_neutral_production(ProductionKirV7BridgeSiteV1::block_entry(1, 0)),
            Err(ProductionKirV7BridgeQueryUnavailableV1::UnknownNeutralProductionSite)
        );
        assert_eq!(
            fixture
                .bridge
                .query_target_production(ProductionKirV7BridgeSiteV1::terminator(0, 2)),
            Err(ProductionKirV7BridgeQueryUnavailableV1::UnknownTargetProductionSite)
        );
        assert!(!fixture.bridge.proves_source_attribution_for_every_site());
        assert!(!fixture.bridge.proves_semantic_refinement());

        let bytes = fixture.bridge.to_canonical_bytes().unwrap();
        let inert = InertProductionKirV7StructuralBridgeV1::from_canonical_bytes(&bytes).unwrap();
        assert_eq!(inert.claimed, fixture.bridge);
        assert!(!inert.proves_semantic_refinement());
    }

    #[test]
    fn stale_v7_v8_source_map_and_shape_fail_closed() {
        let fixture = fixture();
        let other = module(9);
        let other_v7 = VerifiedCanonicalKernelIrV7::from_module(other.clone())
            .unwrap()
            .into_canonical_bytes();
        assert_eq!(
            admit_exact_bridge(
                &other_v7,
                &fixture.canonical_v8,
                &fixture.source_map,
                fixture.binding,
            )
            .unwrap_err(),
            ProductionKirV7BridgeErrorV1::SourceMapV7IdentityMismatch
        );

        let other_v8 = VerifiedCanonicalKernelIrV8::from_module(other)
            .unwrap()
            .into_canonical_bytes();
        assert_eq!(
            admit_exact_bridge(
                &fixture.canonical_v7,
                &other_v8,
                &fixture.source_map,
                fixture.binding,
            )
            .unwrap_err(),
            ProductionKirV7BridgeErrorV1::ProductionKirCatalogIdentityMismatch
        );

        let mut source_binding = fixture.binding;
        source_binding.source_map_v2.sha256[0] ^= 1;
        assert_eq!(
            admit_exact_bridge(
                &fixture.canonical_v7,
                &fixture.canonical_v8,
                &fixture.source_map,
                source_binding,
            )
            .unwrap_err(),
            ProductionKirV7BridgeErrorV1::SourceMapCatalogIdentityMismatch
        );

        let mut count_binding = fixture.binding;
        count_binding.counts =
            ProductionSourceIsaCatalogStructuralCountsV1::new_for_bridge_v1(1, 1, 1, 2);
        assert_eq!(
            admit_exact_bridge(
                &fixture.canonical_v7,
                &fixture.canonical_v8,
                &fixture.source_map,
                count_binding,
            )
            .unwrap_err(),
            ProductionKirV7BridgeErrorV1::CoordinateShapeMismatch
        );
    }

    #[test]
    fn non_identity_migrations_are_typed_unavailable() {
        let fixture = fixture();
        let other_v8 = VerifiedCanonicalKernelIrV8::from_module(module(9))
            .unwrap()
            .into_canonical_bytes();
        let other_owner =
            VerifiedCanonicalKernelIrV8::from_canonical_bytes(other_v8.clone()).unwrap();
        let mut non_identity = fixture.binding;
        non_identity.neutral_production = ProductionKirV7BridgeContentIdentityV1 {
            sha256: *other_owner.identity().digest(),
            byte_len: other_owner.identity().canonical_length(),
        };
        assert!(matches!(
            admit_exact_bridge(
                &fixture.canonical_v7,
                &other_v8,
                &fixture.source_map,
                non_identity,
            )
            .unwrap(),
            ProductionKirV7BridgeAdmissionV1::Unavailable(
                ProductionKirV7BridgeUnavailableV1::NonIdentityStructuralProjectionUnavailable
            )
        ));
    }

    #[test]
    fn kir_inputs_are_rejected_one_byte_over_bound_before_decode() {
        let fixture = fixture();
        let oversized = vec![0_u8; MAX_MODULE_BYTES_V1 + 1];
        assert_eq!(
            admit_exact_bridge(
                &oversized,
                &fixture.canonical_v8,
                &fixture.source_map,
                fixture.binding,
            )
            .unwrap_err(),
            ProductionKirV7BridgeErrorV1::InvalidLength
        );
        assert_eq!(
            admit_exact_bridge(
                &fixture.canonical_v7,
                &oversized,
                &fixture.source_map,
                fixture.binding,
            )
            .unwrap_err(),
            ProductionKirV7BridgeErrorV1::InvalidLength
        );
    }

    #[test]
    fn substituted_target_artifact_catalog_and_semantic_identities_are_not_exact() {
        let fixture = fixture();
        let exact = fixture.bridge;
        for mutate in [0_u8, 1, 2, 3, 4] {
            let mut claimed = ProductionKirV7StructuralBridgeV1 {
                identity: exact.identity,
                binding: exact.binding,
                records: exact.records.clone(),
            };
            match mutate {
                0 => claimed.binding.target_production.sha256[0] ^= 1,
                1 => claimed.binding.artifact.sha256[0] ^= 1,
                2 => claimed.binding.catalog_identity[0] ^= 1,
                3 => claimed.binding.correlation_identity[0] ^= 1,
                _ => claimed.binding.semantic_map_identity[0] ^= 1,
            }
            let bytes = remint(claimed);
            let decoded =
                InertProductionKirV7StructuralBridgeV1::from_canonical_bytes(&bytes).unwrap();
            assert_ne!(decoded.claimed, exact);
        }
    }

    #[test]
    fn reordered_duplicate_non_identity_and_over_limit_wires_are_rejected() {
        let fixture = fixture();
        let exact = fixture.bridge.to_canonical_bytes().unwrap();

        let mut reordered = exact.clone();
        let first = reordered
            [BRIDGE_HEADER_BYTES_V1..BRIDGE_HEADER_BYTES_V1 + BRIDGE_RECORD_BYTES_V1]
            .to_vec();
        let second = reordered[BRIDGE_HEADER_BYTES_V1 + BRIDGE_RECORD_BYTES_V1
            ..BRIDGE_HEADER_BYTES_V1 + 2 * BRIDGE_RECORD_BYTES_V1]
            .to_vec();
        reordered[BRIDGE_HEADER_BYTES_V1..BRIDGE_HEADER_BYTES_V1 + BRIDGE_RECORD_BYTES_V1]
            .copy_from_slice(&second);
        reordered[BRIDGE_HEADER_BYTES_V1 + BRIDGE_RECORD_BYTES_V1
            ..BRIDGE_HEADER_BYTES_V1 + 2 * BRIDGE_RECORD_BYTES_V1]
            .copy_from_slice(&first);
        resign(&mut reordered);
        assert_eq!(
            InertProductionKirV7StructuralBridgeV1::from_canonical_bytes(&reordered).unwrap_err(),
            ProductionKirV7BridgeErrorV1::NonCanonicalRecordOrder
        );

        let mut duplicate = exact.clone();
        duplicate[BRIDGE_HEADER_BYTES_V1 + BRIDGE_RECORD_BYTES_V1
            ..BRIDGE_HEADER_BYTES_V1 + 2 * BRIDGE_RECORD_BYTES_V1]
            .copy_from_slice(&first);
        resign(&mut duplicate);
        assert_eq!(
            InertProductionKirV7StructuralBridgeV1::from_canonical_bytes(&duplicate).unwrap_err(),
            ProductionKirV7BridgeErrorV1::DuplicateRecord
        );

        for tag in 1..=4 {
            let mut non_identity = exact.clone();
            non_identity[BRIDGE_HEADER_BYTES_V1 + 96] = tag;
            resign(&mut non_identity);
            assert_eq!(
                InertProductionKirV7StructuralBridgeV1::from_canonical_bytes(&non_identity)
                    .unwrap_err(),
                ProductionKirV7BridgeErrorV1::InvalidRecord
            );
        }

        let mut over_limit = exact;
        over_limit[24..32].copy_from_slice(
            &(u64::try_from(MAX_PRODUCTION_KIR_V7_BRIDGE_RECORDS_V1).unwrap() + 1).to_le_bytes(),
        );
        resign(&mut over_limit);
        assert_eq!(
            InertProductionKirV7StructuralBridgeV1::from_canonical_bytes(&over_limit).unwrap_err(),
            ProductionKirV7BridgeErrorV1::ResourceLimit
        );
    }
}

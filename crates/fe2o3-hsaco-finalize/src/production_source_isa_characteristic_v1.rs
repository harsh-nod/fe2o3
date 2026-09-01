//! Structurally classified target-KIR characteristics bound to exact Source/ISA records.
//!
//! Classification is deliberately name-free: only verified target-KIR operation structure is
//! inspected. The resulting records are producer-internal evidence. They grant no observation,
//! debugger, profiler, publication, load, or launch authority.

use std::{error::Error, fmt};

use fe2o3_kernel_ir::{
    AddressSpace, MatrixMultiplyProfile, MatrixOperationKind, Module, Operation, OperationKind,
    SynchronizationScope, VerifiedCanonicalKernelIrV8,
};

use crate::{
    ContentIdentityV1, ProductionKirV7BridgeSiteV1, ProductionKirV7StructuralBridgeV1,
    ProductionSourceIsaCatalogContentIdentityV1, ProductionSourceIsaCatalogKirVersionV1,
    ProductionSourceIsaCatalogRecordKindV1, ProductionSourceIsaCatalogRecordV1,
    ProductionSourceIsaCatalogStructuralCountsV1, ProductionSourceIsaCatalogTargetV1,
    ProductionSourceIsaCatalogV1, ProductionSourceIsaKirCoordinateV1,
};

/// Maximum number of structurally characteristic target-KIR operations retained by V1.
pub const MAX_PRODUCTION_SOURCE_ISA_CHARACTERISTICS_V1: usize = 65_536;
/// Maximum correlations retained for one characteristic operation.
pub const MAX_PRODUCTION_SOURCE_ISA_CHARACTERISTIC_CORRELATIONS_PER_WITNESS_V1: usize = 4_096;
/// Maximum correlations retained across all characteristic operations.
pub const MAX_PRODUCTION_SOURCE_ISA_CHARACTERISTIC_CORRELATIONS_V1: usize = 262_144;
/// Maximum catalog elimination facts retained by V1.
pub const MAX_PRODUCTION_SOURCE_ISA_CHARACTERISTIC_ELIMINATIONS_V1: usize = 65_536;

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum ProductionSourceIsaCharacteristicMemoryFormV1 {
    Plain,
    Guarded,
    MatrixTile,
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum ProductionSourceIsaCharacteristicKindV1 {
    GlobalStore {
        form: ProductionSourceIsaCharacteristicMemoryFormV1,
    },
    WorkgroupLoad {
        form: ProductionSourceIsaCharacteristicMemoryFormV1,
    },
    WorkgroupStore {
        form: ProductionSourceIsaCharacteristicMemoryFormV1,
    },
    WorkgroupBarrier,
    Bf16F32M16N16K16Wave64MatrixMultiplyAccumulate,
}

/// Source-attribution shape of the exact records matching one target-KIR operation.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum ProductionSourceIsaCharacteristicAttributionV1 {
    StructuralOnly {
        record_count: u64,
    },
    UniqueSource {
        record_count: u64,
    },
    AmbiguousSources {
        distinct_source_count: u64,
        record_count: u64,
    },
    MixedSourceAndStructural {
        distinct_source_count: u64,
        structural_record_count: u64,
        record_count: u64,
    },
}

#[derive(Debug, Eq, PartialEq)]
pub struct ProductionSourceIsaCharacteristicCorrelationV1 {
    catalog_record_ordinal: u64,
    record: ProductionSourceIsaCatalogRecordV1,
}

impl ProductionSourceIsaCharacteristicCorrelationV1 {
    pub const fn catalog_record_ordinal(&self) -> u64 {
        self.catalog_record_ordinal
    }

    pub const fn record(&self) -> &ProductionSourceIsaCatalogRecordV1 {
        &self.record
    }
}

#[derive(Debug, Eq, PartialEq)]
pub struct ProductionSourceIsaCharacteristicWitnessV1 {
    target_kir: ProductionSourceIsaKirCoordinateV1,
    kind: ProductionSourceIsaCharacteristicKindV1,
    attribution: ProductionSourceIsaCharacteristicAttributionV1,
    correlations: Vec<ProductionSourceIsaCharacteristicCorrelationV1>,
}

impl ProductionSourceIsaCharacteristicWitnessV1 {
    pub const fn target_kir(&self) -> ProductionSourceIsaKirCoordinateV1 {
        self.target_kir
    }

    pub const fn kind(&self) -> ProductionSourceIsaCharacteristicKindV1 {
        self.kind
    }

    pub const fn attribution(&self) -> ProductionSourceIsaCharacteristicAttributionV1 {
        self.attribution
    }

    /// Returns every exact admitted catalog record for this target operation.
    pub fn correlations(&self) -> &[ProductionSourceIsaCharacteristicCorrelationV1] {
        &self.correlations
    }

    pub fn preserves_one_to_many_correlation(&self) -> bool {
        self.correlations.len() > 1
    }
}

/// Bounded producer-side records suitable for a one-way conversion into an inert observer schema.
#[derive(Debug, Eq, PartialEq)]
pub struct ProductionSourceIsaCharacteristicCollectionV1 {
    target: ProductionSourceIsaCatalogTargetV1,
    kir_version: ProductionSourceIsaCatalogKirVersionV1,
    neutral_kir: ProductionSourceIsaCatalogContentIdentityV1,
    target_kir: ProductionSourceIsaCatalogContentIdentityV1,
    source_map_v2: ProductionSourceIsaCatalogContentIdentityV1,
    artifact: ContentIdentityV1,
    structural_binding_identity: [u8; 32],
    structural_counts: ProductionSourceIsaCatalogStructuralCountsV1,
    catalog_identity: [u8; 32],
    catalog_byte_len: u64,
    correlation_identity: [u8; 32],
    semantic_map_identity: [u8; 32],
    structural_bridge_identity: [u8; 32],
    structural_bridge_byte_len: u64,
    catalog_record_count: u64,
    classified_target_operation_count: u64,
    retained_correlation_count: u64,
    pre_kir_elimination_count: u64,
    characteristics: Vec<ProductionSourceIsaCharacteristicWitnessV1>,
    pre_kir_eliminated_catalog_records: Vec<ProductionSourceIsaCharacteristicCorrelationV1>,
}

impl ProductionSourceIsaCharacteristicCollectionV1 {
    pub const fn target(&self) -> ProductionSourceIsaCatalogTargetV1 {
        self.target
    }

    pub const fn kir_version(&self) -> ProductionSourceIsaCatalogKirVersionV1 {
        self.kir_version
    }

    pub const fn target_kir_identity(&self) -> ProductionSourceIsaCatalogContentIdentityV1 {
        self.target_kir
    }

    pub const fn neutral_kir_identity(&self) -> ProductionSourceIsaCatalogContentIdentityV1 {
        self.neutral_kir
    }

    pub const fn source_map_v2_identity(&self) -> ProductionSourceIsaCatalogContentIdentityV1 {
        self.source_map_v2
    }

    pub const fn artifact_identity(&self) -> ContentIdentityV1 {
        self.artifact
    }

    pub const fn catalog_identity(&self) -> &[u8; 32] {
        &self.catalog_identity
    }

    pub const fn structural_binding_identity(&self) -> &[u8; 32] {
        &self.structural_binding_identity
    }

    pub const fn structural_counts(&self) -> ProductionSourceIsaCatalogStructuralCountsV1 {
        self.structural_counts
    }

    pub const fn catalog_byte_len(&self) -> u64 {
        self.catalog_byte_len
    }

    pub const fn correlation_identity(&self) -> &[u8; 32] {
        &self.correlation_identity
    }

    pub const fn semantic_map_identity(&self) -> &[u8; 32] {
        &self.semantic_map_identity
    }

    pub const fn structural_bridge_identity(&self) -> &[u8; 32] {
        &self.structural_bridge_identity
    }

    pub const fn structural_bridge_byte_len(&self) -> u64 {
        self.structural_bridge_byte_len
    }

    pub const fn catalog_record_count(&self) -> u64 {
        self.catalog_record_count
    }

    pub const fn examined_target_operation_count(&self) -> u64 {
        self.structural_counts.operations()
    }

    pub const fn classified_target_operation_count(&self) -> u64 {
        self.classified_target_operation_count
    }

    pub const fn retained_correlation_count(&self) -> u64 {
        self.retained_correlation_count
    }

    pub const fn pre_kir_elimination_count(&self) -> u64 {
        self.pre_kir_elimination_count
    }

    pub fn characteristics(&self) -> &[ProductionSourceIsaCharacteristicWitnessV1] {
        &self.characteristics
    }

    /// Exact admitted records eliminated before KIR. These are not attributed to a survivor.
    pub fn pre_kir_eliminated_catalog_records(
        &self,
    ) -> &[ProductionSourceIsaCharacteristicCorrelationV1] {
        &self.pre_kir_eliminated_catalog_records
    }

    /// An admitted collection always represents a complete bounded characteristic scan.
    pub const fn scan_is_complete(&self) -> bool {
        true
    }

    pub const fn classifies_by_kernel_or_workload_name(&self) -> bool {
        false
    }

    pub const fn proves_structurally_classified_target_kir_catalog_association(&self) -> bool {
        true
    }

    pub const fn proves_complete_source_or_machine_coverage(&self) -> bool {
        false
    }

    pub const fn proves_semantic_refinement(&self) -> bool {
        false
    }

    pub const fn proves_optimized_or_final_llvm_custody(&self) -> bool {
        false
    }

    pub const fn proves_final_isa_decoding(&self) -> bool {
        false
    }

    pub const fn proves_final_isa_opcode_semantics(&self) -> bool {
        false
    }

    pub const fn proves_a_schedule(&self) -> bool {
        false
    }

    pub const fn proves_gpu_execution(&self) -> bool {
        false
    }

    pub const fn proves_hardware_performance(&self) -> bool {
        false
    }

    pub const fn grants_debugger_authority(&self) -> bool {
        false
    }

    pub const fn grants_profiler_authority(&self) -> bool {
        false
    }

    pub const fn grants_runtime_authority(&self) -> bool {
        false
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProductionSourceIsaCharacteristicUnavailableV1 {
    CharacteristicLimit,
    CorrelationPerWitnessLimit,
    TotalCorrelationLimit,
    EliminationFactLimit,
}

#[derive(Debug, Eq, PartialEq)]
#[allow(
    clippy::large_enum_variant,
    reason = "the admitted variant owns one bounded collection; boxing would add an infallible allocation"
)]
pub enum ProductionSourceIsaCharacteristicAdmissionV1 {
    Admitted(ProductionSourceIsaCharacteristicCollectionV1),
    Unavailable(ProductionSourceIsaCharacteristicUnavailableV1),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProductionSourceIsaCharacteristicErrorV1 {
    InvalidLength,
    InvalidCanonicalTargetKir,
    UnsupportedCatalogKirVersion,
    CatalogBridgeIdentityMismatch,
    CatalogBridgeStructuralMismatch,
    TargetKirIdentityMismatch,
    StructuralCountMismatch,
    UnknownBridgeCoordinate,
    MissingCatalogCorrelation,
    InvalidCatalogCorrelation,
    SizeOverflow,
    AllocationFailure,
}

impl fmt::Display for ProductionSourceIsaCharacteristicErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "invalid production Source/ISA characteristic collection: {self:?}"
        )
    }
}

impl Error for ProductionSourceIsaCharacteristicErrorV1 {}

/// Classifies verified target-KIR operations and retains their exact admitted catalog records.
///
/// An observer adapter must copy these inert fields into its authority-free schema. Observer input
/// must never reconstruct this admitted producer collection or any compiler/runtime authority.
pub fn admit_production_source_isa_characteristics_v1(
    canonical_target_kir: &[u8],
    catalog: &ProductionSourceIsaCatalogV1,
    bridge: &ProductionKirV7StructuralBridgeV1,
) -> Result<ProductionSourceIsaCharacteristicAdmissionV1, ProductionSourceIsaCharacteristicErrorV1>
{
    validate_catalog_bridge_binding(catalog, bridge)?;
    let module = verified_target_module(canonical_target_kir, catalog, bridge)?;
    validate_structural_counts(&module, bridge)?;

    let pre_kir_eliminated_catalog_records = match collect_pre_kir_eliminations(catalog)? {
        Ok(records) => records,
        Err(unavailable) => {
            return Ok(ProductionSourceIsaCharacteristicAdmissionV1::Unavailable(
                unavailable,
            ));
        }
    };
    let characteristics = match collect_characteristics(&module, catalog, bridge)? {
        Ok(records) => records,
        Err(unavailable) => {
            return Ok(ProductionSourceIsaCharacteristicAdmissionV1::Unavailable(
                unavailable,
            ));
        }
    };
    let structural = catalog.structural_binding();
    let catalog_record_count = u64::try_from(catalog.records().len())
        .map_err(|_| ProductionSourceIsaCharacteristicErrorV1::SizeOverflow)?;
    let classified_target_operation_count = u64::try_from(characteristics.len())
        .map_err(|_| ProductionSourceIsaCharacteristicErrorV1::SizeOverflow)?;
    let retained_correlation_count = characteristics.iter().try_fold(0_u64, |count, witness| {
        count
            .checked_add(
                u64::try_from(witness.correlations.len())
                    .map_err(|_| ProductionSourceIsaCharacteristicErrorV1::SizeOverflow)?,
            )
            .ok_or(ProductionSourceIsaCharacteristicErrorV1::SizeOverflow)
    })?;
    let catalog_byte_len = catalog
        .canonical_byte_len()
        .map_err(|_| ProductionSourceIsaCharacteristicErrorV1::SizeOverflow)?;
    let structural_bridge_byte_len = bridge
        .canonical_byte_len()
        .map_err(|_| ProductionSourceIsaCharacteristicErrorV1::SizeOverflow)?;
    let pre_kir_elimination_count = u64::try_from(pre_kir_eliminated_catalog_records.len())
        .map_err(|_| ProductionSourceIsaCharacteristicErrorV1::SizeOverflow)?;
    Ok(ProductionSourceIsaCharacteristicAdmissionV1::Admitted(
        ProductionSourceIsaCharacteristicCollectionV1 {
            target: structural.target(),
            kir_version: structural.kir_version(),
            neutral_kir: structural.neutral_kernel_ir(),
            target_kir: structural.target_bound_kernel_ir(),
            source_map_v2: catalog.source_map_v2_identity(),
            artifact: catalog.artifact_identity(),
            structural_binding_identity: structural.identity(),
            structural_counts: structural.counts(),
            catalog_identity: *catalog.identity(),
            catalog_byte_len,
            correlation_identity: *catalog.correlation_identity(),
            semantic_map_identity: *catalog.semantic_map_identity(),
            structural_bridge_identity: *bridge.identity(),
            structural_bridge_byte_len,
            catalog_record_count,
            classified_target_operation_count,
            retained_correlation_count,
            pre_kir_elimination_count,
            characteristics,
            pre_kir_eliminated_catalog_records,
        },
    ))
}

fn validate_catalog_bridge_binding(
    catalog: &ProductionSourceIsaCatalogV1,
    bridge: &ProductionKirV7StructuralBridgeV1,
) -> Result<(), ProductionSourceIsaCharacteristicErrorV1> {
    if catalog.identity() != bridge.catalog_identity()
        || catalog.correlation_identity() != bridge.correlation_identity()
        || catalog.semantic_map_identity() != bridge.semantic_map_identity()
    {
        return Err(ProductionSourceIsaCharacteristicErrorV1::CatalogBridgeIdentityMismatch);
    }
    let structural = catalog.structural_binding();
    if !matches!(
        structural.kir_version(),
        ProductionSourceIsaCatalogKirVersionV1::V8
    ) {
        return Err(ProductionSourceIsaCharacteristicErrorV1::UnsupportedCatalogKirVersion);
    }
    let target = structural.target_bound_kernel_ir();
    let bridge_target = bridge.target_production_identity();
    let neutral = structural.neutral_kernel_ir();
    let bridge_neutral = bridge.neutral_production_identity();
    let source_map = catalog.source_map_v2_identity();
    let bridge_source_map = bridge.source_map_v2_identity();
    let artifact = catalog.artifact_identity();
    let bridge_artifact = bridge.artifact_identity();
    if structural.identity() != *bridge.structural_identity()
        || structural.counts() != bridge.structural_counts()
        || target.sha256() != bridge_target.sha256()
        || target.byte_len() != bridge_target.byte_len()
        || neutral.sha256() != bridge_neutral.sha256()
        || neutral.byte_len() != bridge_neutral.byte_len()
        || source_map.sha256() != bridge_source_map.sha256()
        || source_map.byte_len() != bridge_source_map.byte_len()
        || artifact.sha256() != &bridge_artifact.sha256()
        || artifact.byte_len() != bridge_artifact.byte_len()
        || !target_matches(structural.target(), bridge.target())
    {
        return Err(ProductionSourceIsaCharacteristicErrorV1::CatalogBridgeStructuralMismatch);
    }
    Ok(())
}

fn target_matches(
    catalog: ProductionSourceIsaCatalogTargetV1,
    bridge: crate::ProductionKirV7BridgeTargetV1,
) -> bool {
    matches!(
        (catalog, bridge),
        (
            ProductionSourceIsaCatalogTargetV1::Gfx942,
            crate::ProductionKirV7BridgeTargetV1::Gfx942
        ) | (
            ProductionSourceIsaCatalogTargetV1::Gfx950,
            crate::ProductionKirV7BridgeTargetV1::Gfx950
        )
    )
}

fn verified_target_module(
    bytes: &[u8],
    catalog: &ProductionSourceIsaCatalogV1,
    bridge: &ProductionKirV7StructuralBridgeV1,
) -> Result<Module, ProductionSourceIsaCharacteristicErrorV1> {
    if bytes.is_empty() || bytes.len() > fe2o3_kernel_ir::MAX_MODULE_BYTES_V1 {
        return Err(ProductionSourceIsaCharacteristicErrorV1::InvalidLength);
    }
    let (owner, module) =
        VerifiedCanonicalKernelIrV8::from_canonical_bytes_with_module(copy_bytes(bytes)?)
            .map_err(|_| ProductionSourceIsaCharacteristicErrorV1::InvalidCanonicalTargetKir)?;
    let expected = catalog.structural_binding().target_bound_kernel_ir();
    let bridge_expected = bridge.target_production_identity();
    if !matches_target_identity(&owner, expected.sha256(), expected.byte_len())
        || !matches_target_identity(&owner, bridge_expected.sha256(), bridge_expected.byte_len())
    {
        return Err(ProductionSourceIsaCharacteristicErrorV1::TargetKirIdentityMismatch);
    }
    Ok(module)
}

fn matches_target_identity(
    owner: &VerifiedCanonicalKernelIrV8,
    expected_digest: [u8; 32],
    expected_length: u64,
) -> bool {
    owner.identity().digest() == &expected_digest
        && owner.identity().canonical_length() == expected_length
}

fn copy_bytes(bytes: &[u8]) -> Result<Vec<u8>, ProductionSourceIsaCharacteristicErrorV1> {
    let mut owned = Vec::new();
    owned
        .try_reserve_exact(bytes.len())
        .map_err(|_| ProductionSourceIsaCharacteristicErrorV1::AllocationFailure)?;
    owned.extend_from_slice(bytes);
    Ok(owned)
}

fn validate_structural_counts(
    module: &Module,
    bridge: &ProductionKirV7StructuralBridgeV1,
) -> Result<(), ProductionSourceIsaCharacteristicErrorV1> {
    let functions = u64::try_from(module.functions.len())
        .map_err(|_| ProductionSourceIsaCharacteristicErrorV1::SizeOverflow)?;
    let mut defined_bodies = 0_u64;
    let mut blocks = 0_u64;
    let mut operations = 0_u64;
    for function in &module.functions {
        let Some(body) = &function.body else {
            continue;
        };
        defined_bodies = defined_bodies
            .checked_add(1)
            .ok_or(ProductionSourceIsaCharacteristicErrorV1::SizeOverflow)?;
        blocks = blocks
            .checked_add(
                u64::try_from(body.blocks.len())
                    .map_err(|_| ProductionSourceIsaCharacteristicErrorV1::SizeOverflow)?,
            )
            .ok_or(ProductionSourceIsaCharacteristicErrorV1::SizeOverflow)?;
        for block in &body.blocks {
            operations = operations
                .checked_add(
                    u64::try_from(block.operations.len())
                        .map_err(|_| ProductionSourceIsaCharacteristicErrorV1::SizeOverflow)?,
                )
                .ok_or(ProductionSourceIsaCharacteristicErrorV1::SizeOverflow)?;
        }
    }
    let expected = bridge.structural_counts();
    if functions != expected.functions()
        || defined_bodies != expected.defined_bodies()
        || blocks != expected.blocks()
        || operations != expected.operations()
    {
        return Err(ProductionSourceIsaCharacteristicErrorV1::StructuralCountMismatch);
    }
    Ok(())
}

fn collect_pre_kir_eliminations(
    catalog: &ProductionSourceIsaCatalogV1,
) -> Result<
    Result<
        Vec<ProductionSourceIsaCharacteristicCorrelationV1>,
        ProductionSourceIsaCharacteristicUnavailableV1,
    >,
    ProductionSourceIsaCharacteristicErrorV1,
> {
    let count = catalog
        .records()
        .iter()
        .filter(|record| is_pre_kir_eliminated(record))
        .count();
    if count > MAX_PRODUCTION_SOURCE_ISA_CHARACTERISTIC_ELIMINATIONS_V1 {
        return Ok(Err(
            ProductionSourceIsaCharacteristicUnavailableV1::EliminationFactLimit,
        ));
    }
    let mut records = Vec::new();
    records
        .try_reserve_exact(count)
        .map_err(|_| ProductionSourceIsaCharacteristicErrorV1::AllocationFailure)?;
    for (ordinal, record) in catalog.records().iter().enumerate() {
        if is_pre_kir_eliminated(record) {
            records.push(try_copy_correlation(ordinal, record)?);
        }
    }
    Ok(Ok(records))
}

fn is_pre_kir_eliminated(record: &ProductionSourceIsaCatalogRecordV1) -> bool {
    matches!(
        record.kind(),
        ProductionSourceIsaCatalogRecordKindV1::EliminatedBeforeKir
    )
}

fn try_copy_correlation(
    catalog_record_ordinal: usize,
    record: &ProductionSourceIsaCatalogRecordV1,
) -> Result<ProductionSourceIsaCharacteristicCorrelationV1, ProductionSourceIsaCharacteristicErrorV1>
{
    Ok(ProductionSourceIsaCharacteristicCorrelationV1 {
        catalog_record_ordinal: u64::try_from(catalog_record_ordinal)
            .map_err(|_| ProductionSourceIsaCharacteristicErrorV1::SizeOverflow)?,
        record: record
            .try_clone_bounded()
            .map_err(|_| ProductionSourceIsaCharacteristicErrorV1::AllocationFailure)?,
    })
}

fn collect_characteristics(
    module: &Module,
    catalog: &ProductionSourceIsaCatalogV1,
    bridge: &ProductionKirV7StructuralBridgeV1,
) -> Result<
    Result<
        Vec<ProductionSourceIsaCharacteristicWitnessV1>,
        ProductionSourceIsaCharacteristicUnavailableV1,
    >,
    ProductionSourceIsaCharacteristicErrorV1,
> {
    let mut witnesses = Vec::new();
    let mut correlation_total = 0_usize;
    for (function_ordinal, function) in module.functions.iter().enumerate() {
        let Some(body) = &function.body else {
            continue;
        };
        for (block_ordinal, block) in body.blocks.iter().enumerate() {
            for (operation_ordinal, operation) in block.operations.iter().enumerate() {
                let Some(kind) = classify_operation(operation) else {
                    continue;
                };
                if witnesses.len() == MAX_PRODUCTION_SOURCE_ISA_CHARACTERISTICS_V1 {
                    return Ok(Err(
                        ProductionSourceIsaCharacteristicUnavailableV1::CharacteristicLimit,
                    ));
                }
                witnesses
                    .try_reserve(1)
                    .map_err(|_| ProductionSourceIsaCharacteristicErrorV1::AllocationFailure)?;
                let coordinate = ProductionSourceIsaKirCoordinateV1::new(
                    u64::try_from(function_ordinal)
                        .map_err(|_| ProductionSourceIsaCharacteristicErrorV1::SizeOverflow)?,
                    u64::try_from(block_ordinal)
                        .map_err(|_| ProductionSourceIsaCharacteristicErrorV1::SizeOverflow)?,
                    u64::try_from(operation_ordinal)
                        .map_err(|_| ProductionSourceIsaCharacteristicErrorV1::SizeOverflow)?,
                )
                .map_err(|_| ProductionSourceIsaCharacteristicErrorV1::SizeOverflow)?;
                bridge
                    .query_target_production(ProductionKirV7BridgeSiteV1::operation(
                        coordinate.function_ordinal(),
                        coordinate.block_ordinal(),
                        coordinate.operation_ordinal(),
                    ))
                    .map_err(|_| {
                        ProductionSourceIsaCharacteristicErrorV1::UnknownBridgeCoordinate
                    })?;
                let mut matches = catalog.query_target_kir(coordinate).map_err(|_| {
                    ProductionSourceIsaCharacteristicErrorV1::MissingCatalogCorrelation
                })?;
                let count = matches.len();
                if count > MAX_PRODUCTION_SOURCE_ISA_CHARACTERISTIC_CORRELATIONS_PER_WITNESS_V1 {
                    return Ok(Err(
                        ProductionSourceIsaCharacteristicUnavailableV1::CorrelationPerWitnessLimit,
                    ));
                }
                correlation_total = correlation_total
                    .checked_add(count)
                    .ok_or(ProductionSourceIsaCharacteristicErrorV1::SizeOverflow)?;
                if correlation_total > MAX_PRODUCTION_SOURCE_ISA_CHARACTERISTIC_CORRELATIONS_V1 {
                    return Ok(Err(
                        ProductionSourceIsaCharacteristicUnavailableV1::TotalCorrelationLimit,
                    ));
                }
                let mut correlations = Vec::new();
                correlations
                    .try_reserve_exact(count)
                    .map_err(|_| ProductionSourceIsaCharacteristicErrorV1::AllocationFailure)?;
                while let Some((catalog_record_ordinal, record)) = matches.next_with_ordinal() {
                    if record.target_kir() != Some(coordinate) || is_pre_kir_eliminated(record) {
                        return Err(
                            ProductionSourceIsaCharacteristicErrorV1::InvalidCatalogCorrelation,
                        );
                    }
                    correlations.push(try_copy_correlation(catalog_record_ordinal, record)?);
                }
                let attribution = attribution(&correlations)?;
                witnesses.push(ProductionSourceIsaCharacteristicWitnessV1 {
                    target_kir: coordinate,
                    kind,
                    attribution,
                    correlations,
                });
            }
        }
    }
    Ok(Ok(witnesses))
}

fn attribution(
    records: &[ProductionSourceIsaCharacteristicCorrelationV1],
) -> Result<ProductionSourceIsaCharacteristicAttributionV1, ProductionSourceIsaCharacteristicErrorV1>
{
    let mut sources = Vec::new();
    sources
        .try_reserve_exact(records.len())
        .map_err(|_| ProductionSourceIsaCharacteristicErrorV1::AllocationFailure)?;
    let mut structural = 0_u64;
    for correlation in records {
        let record = correlation.record();
        if let Some(identity) = record.source_node_identity() {
            sources.push(identity);
        } else {
            structural = structural
                .checked_add(1)
                .ok_or(ProductionSourceIsaCharacteristicErrorV1::SizeOverflow)?;
        }
    }
    sources.sort_unstable();
    sources.dedup();
    let distinct_source_count = u64::try_from(sources.len())
        .map_err(|_| ProductionSourceIsaCharacteristicErrorV1::SizeOverflow)?;
    let record_count = u64::try_from(records.len())
        .map_err(|_| ProductionSourceIsaCharacteristicErrorV1::SizeOverflow)?;
    Ok(match (distinct_source_count, structural) {
        (0, _) => ProductionSourceIsaCharacteristicAttributionV1::StructuralOnly { record_count },
        (1, 0) => ProductionSourceIsaCharacteristicAttributionV1::UniqueSource { record_count },
        (source_count, 0) => ProductionSourceIsaCharacteristicAttributionV1::AmbiguousSources {
            distinct_source_count: source_count,
            record_count,
        },
        (source_count, structural_record_count) => {
            ProductionSourceIsaCharacteristicAttributionV1::MixedSourceAndStructural {
                distinct_source_count: source_count,
                structural_record_count,
                record_count,
            }
        }
    })
}

fn classify_operation(operation: &Operation) -> Option<ProductionSourceIsaCharacteristicKindV1> {
    let kind = match &operation.kind {
        OperationKind::Store { access, .. } if access.address_space == AddressSpace::Global => {
            ProductionSourceIsaCharacteristicKindV1::GlobalStore {
                form: ProductionSourceIsaCharacteristicMemoryFormV1::Plain,
            }
        }
        OperationKind::GuardedStore { access, .. }
            if access.address_space == AddressSpace::Global =>
        {
            ProductionSourceIsaCharacteristicKindV1::GlobalStore {
                form: ProductionSourceIsaCharacteristicMemoryFormV1::Guarded,
            }
        }
        OperationKind::Load { access, .. } if access.address_space == AddressSpace::Workgroup => {
            ProductionSourceIsaCharacteristicKindV1::WorkgroupLoad {
                form: ProductionSourceIsaCharacteristicMemoryFormV1::Plain,
            }
        }
        OperationKind::GuardedLoad { access, .. }
            if access.address_space == AddressSpace::Workgroup =>
        {
            ProductionSourceIsaCharacteristicKindV1::WorkgroupLoad {
                form: ProductionSourceIsaCharacteristicMemoryFormV1::Guarded,
            }
        }
        OperationKind::Store { access, .. } if access.address_space == AddressSpace::Workgroup => {
            ProductionSourceIsaCharacteristicKindV1::WorkgroupStore {
                form: ProductionSourceIsaCharacteristicMemoryFormV1::Plain,
            }
        }
        OperationKind::GuardedStore { access, .. }
            if access.address_space == AddressSpace::Workgroup =>
        {
            ProductionSourceIsaCharacteristicKindV1::WorkgroupStore {
                form: ProductionSourceIsaCharacteristicMemoryFormV1::Guarded,
            }
        }
        OperationKind::WorkgroupBarrier(_) => {
            ProductionSourceIsaCharacteristicKindV1::WorkgroupBarrier
        }
        OperationKind::Matrix(matrix) => match &matrix.kind {
            MatrixOperationKind::LdsLoad { .. } => {
                ProductionSourceIsaCharacteristicKindV1::WorkgroupLoad {
                    form: ProductionSourceIsaCharacteristicMemoryFormV1::MatrixTile,
                }
            }
            MatrixOperationKind::LdsStore { .. } => {
                ProductionSourceIsaCharacteristicKindV1::WorkgroupStore {
                    form: ProductionSourceIsaCharacteristicMemoryFormV1::MatrixTile,
                }
            }
            MatrixOperationKind::MultiplyAccumulate { profile, .. }
                if *profile == MatrixMultiplyProfile::bf16_f32_m16n16k16_wave64()
                    && matrix.active_lanes == 64
                    && matrix.convergence.scope() == SynchronizationScope::Subgroup =>
            {
                ProductionSourceIsaCharacteristicKindV1::Bf16F32M16N16K16Wave64MatrixMultiplyAccumulate
            }
            MatrixOperationKind::MultiplyAccumulate { .. }
            | MatrixOperationKind::ScaledMultiplyAccumulate { .. } => return None,
        },
        _ => return None,
    };
    Some(kind)
}

#[cfg(test)]
mod tests {
    use fe2o3_kernel_ir::{
        BarrierSemantics, BasicBlock, BlockId, Convergence, Function, MatrixElement,
        MatrixOperation, MemoryAccess, MemoryOrdering, Module, Operation, OperationKind, Signature,
        SynchronizationScope, Terminator, ValueId, VerifiedCanonicalKernelIrV8, WaveWidth,
        WorkgroupBarrier,
    };

    use crate::ProductionSourceIsaCatalogTransformationV1;

    use super::*;

    fn operation(kind: OperationKind) -> Operation {
        Operation::new(Vec::new(), kind)
    }

    #[test]
    fn classifier_is_structural_and_covers_characteristic_families() {
        let plain_global = operation(OperationKind::Store {
            pointer: ValueId(0),
            value: ValueId(1),
            access: MemoryAccess::new(AddressSpace::Global, 4),
        });
        let guarded_global = operation(OperationKind::GuardedStore {
            pointer: ValueId(0),
            predicate: ValueId(2),
            value: ValueId(1),
            access: MemoryAccess::new(AddressSpace::Global, 4),
        });
        let workgroup_load = operation(OperationKind::Load {
            pointer: ValueId(0),
            access: MemoryAccess::new(AddressSpace::Workgroup, 4),
        });
        let guarded_workgroup_store = operation(OperationKind::GuardedStore {
            pointer: ValueId(0),
            predicate: ValueId(2),
            value: ValueId(1),
            access: MemoryAccess::new(AddressSpace::Workgroup, 4),
        });
        let barrier = operation(OperationKind::WorkgroupBarrier(WorkgroupBarrier {
            memory_scope: SynchronizationScope::Workgroup,
            semantics: BarrierSemantics::new(
                MemoryOrdering::AcquireRelease,
                [AddressSpace::Workgroup],
            ),
            convergence: Convergence::uniform(SynchronizationScope::Workgroup),
        }));
        let matrix = operation(OperationKind::Matrix(MatrixOperation::multiply_accumulate(
            [ValueId(0); 4],
            [ValueId(1); 4],
            [ValueId(2); 4],
        )));

        assert_eq!(
            classify_operation(&plain_global),
            Some(ProductionSourceIsaCharacteristicKindV1::GlobalStore {
                form: ProductionSourceIsaCharacteristicMemoryFormV1::Plain,
            })
        );
        assert_eq!(
            classify_operation(&guarded_global),
            Some(ProductionSourceIsaCharacteristicKindV1::GlobalStore {
                form: ProductionSourceIsaCharacteristicMemoryFormV1::Guarded,
            })
        );
        assert_eq!(
            classify_operation(&workgroup_load),
            Some(ProductionSourceIsaCharacteristicKindV1::WorkgroupLoad {
                form: ProductionSourceIsaCharacteristicMemoryFormV1::Plain,
            })
        );
        assert_eq!(
            classify_operation(&guarded_workgroup_store),
            Some(ProductionSourceIsaCharacteristicKindV1::WorkgroupStore {
                form: ProductionSourceIsaCharacteristicMemoryFormV1::Guarded,
            })
        );
        assert_eq!(
            classify_operation(&barrier),
            Some(ProductionSourceIsaCharacteristicKindV1::WorkgroupBarrier)
        );
        assert_eq!(
            classify_operation(&matrix),
            Some(
                ProductionSourceIsaCharacteristicKindV1::Bf16F32M16N16K16Wave64MatrixMultiplyAccumulate
            )
        );
    }

    #[test]
    fn near_match_substitutions_do_not_change_characteristic_meaning() {
        let global_load = operation(OperationKind::Load {
            pointer: ValueId(0),
            access: MemoryAccess::new(AddressSpace::Global, 4),
        });
        let private_store = operation(OperationKind::Store {
            pointer: ValueId(0),
            value: ValueId(1),
            access: MemoryAccess::new(AddressSpace::Private, 4),
        });
        let mut wrong_lanes =
            MatrixOperation::multiply_accumulate([ValueId(0); 4], [ValueId(1); 4], [ValueId(2); 4]);
        wrong_lanes.active_lanes = 32;
        assert_eq!(classify_operation(&global_load), None);
        assert_eq!(classify_operation(&private_store), None);
        assert_eq!(
            classify_operation(&operation(OperationKind::Matrix(wrong_lanes))),
            None
        );
    }

    fn workgroup_barrier_operation() -> Operation {
        operation(OperationKind::WorkgroupBarrier(WorkgroupBarrier {
            memory_scope: SynchronizationScope::Workgroup,
            semantics: BarrierSemantics::new(
                MemoryOrdering::AcquireRelease,
                [AddressSpace::Workgroup],
            ),
            convergence: Convergence::uniform(SynchronizationScope::Workgroup),
        }))
    }

    fn barrier_module(barrier_count: usize) -> Module {
        let mut block = BasicBlock::new(BlockId(0));
        for _ in 0..barrier_count {
            block.operations.push(workgroup_barrier_operation());
        }
        block.terminator = Some(Terminator::Return { values: Vec::new() });
        let mut module = Module::new("structural-characteristic");
        module.functions.push(Function::definition(
            "entry",
            Signature::new(Vec::new(), Vec::new()),
            Vec::new(),
            vec![block],
        ));
        module
    }

    #[test]
    fn hostile_operation_substitution_changes_verified_target_identity() {
        let expected = VerifiedCanonicalKernelIrV8::from_module(barrier_module(1)).unwrap();
        let substituted = VerifiedCanonicalKernelIrV8::from_module(barrier_module(0)).unwrap();
        assert_ne!(expected.identity(), substituted.identity());
        assert_ne!(expected.canonical_bytes(), substituted.canonical_bytes());
        assert!(!matches_target_identity(
            &substituted,
            *expected.identity().digest(),
            expected.identity().canonical_length(),
        ));
    }

    fn admit_fixture(
        barrier_count: usize,
        records: Vec<ProductionSourceIsaCatalogRecordV1>,
    ) -> ProductionSourceIsaCharacteristicCollectionV1 {
        let verified =
            VerifiedCanonicalKernelIrV8::from_module(barrier_module(barrier_count)).unwrap();
        let counts = ProductionSourceIsaCatalogStructuralCountsV1::new_for_bridge_v1(
            1,
            1,
            1,
            barrier_count as u64,
        );
        let catalog = crate::production_source_isa_catalog_v1::characteristic_catalog_fixture_v1(
            *verified.identity().digest(),
            verified.identity().canonical_length(),
            counts,
            records,
        );
        let bridge =
            crate::production_kir_v7_structural_bridge_v1::characteristic_bridge_fixture_v1(
                verified.canonical_bytes(),
                &catalog,
            )
            .unwrap();
        let admission = admit_production_source_isa_characteristics_v1(
            verified.canonical_bytes(),
            &catalog,
            &bridge,
        )
        .unwrap();
        let ProductionSourceIsaCharacteristicAdmissionV1::Admitted(collection) = admission else {
            panic!("bounded characteristic fixture must be admitted")
        };
        assert_eq!(
            collection.catalog_byte_len(),
            catalog.canonical_byte_len().unwrap()
        );
        assert_eq!(
            collection.structural_bridge_byte_len(),
            bridge.canonical_byte_len().unwrap()
        );
        assert_eq!(
            collection.kir_version(),
            ProductionSourceIsaCatalogKirVersionV1::V8
        );
        collection
    }

    #[test]
    fn end_to_end_admission_preserves_mixed_partial_and_backend_eliminated_records() {
        let operation_zero = ProductionSourceIsaKirCoordinateV1::new(0, 0, 0).unwrap();
        let operation_one = ProductionSourceIsaKirCoordinateV1::new(0, 0, 1).unwrap();
        let records = vec![
            ProductionSourceIsaCatalogRecordV1::characteristic_fixture_v1(
                Some([1; 32]),
                Some(operation_zero),
                Some(ProductionSourceIsaCatalogTransformationV1::Preserved),
            ),
            ProductionSourceIsaCatalogRecordV1::characteristic_fixture_v1(
                None,
                Some(operation_zero),
                Some(ProductionSourceIsaCatalogTransformationV1::Preserved),
            ),
            ProductionSourceIsaCatalogRecordV1::characteristic_fixture_v1(
                Some([2; 32]),
                Some(operation_one),
                Some(ProductionSourceIsaCatalogTransformationV1::Eliminated),
            ),
            ProductionSourceIsaCatalogRecordV1::characteristic_fixture_v1(
                Some([3; 32]),
                None,
                None,
            ),
        ];
        let collection = admit_fixture(2, records);
        assert!(collection.scan_is_complete());
        assert_eq!(collection.examined_target_operation_count(), 2);
        assert_eq!(collection.classified_target_operation_count(), 2);
        assert_eq!(collection.retained_correlation_count(), 3);
        assert_eq!(collection.pre_kir_elimination_count(), 1);

        let first = &collection.characteristics()[0];
        assert_eq!(
            first.attribution(),
            ProductionSourceIsaCharacteristicAttributionV1::MixedSourceAndStructural {
                distinct_source_count: 1,
                structural_record_count: 1,
                record_count: 2,
            }
        );
        assert_ne!(
            first.correlations()[0].catalog_record_ordinal(),
            first.correlations()[1].catalog_record_ordinal()
        );
        let backend_eliminated = &collection.characteristics()[1].correlations()[0];
        assert_eq!(
            backend_eliminated.record().transformation(),
            Some(ProductionSourceIsaCatalogTransformationV1::Eliminated)
        );
        assert!(backend_eliminated.record().isa().is_empty());
        assert_eq!(
            collection.pre_kir_eliminated_catalog_records()[0]
                .record()
                .kind(),
            ProductionSourceIsaCatalogRecordKindV1::EliminatedBeforeKir
        );
    }

    #[test]
    fn end_to_end_complete_zero_characteristic_scan_is_admitted() {
        let collection = admit_fixture(0, Vec::new());
        assert!(collection.scan_is_complete());
        assert_eq!(collection.catalog_record_count(), 0);
        assert_eq!(collection.examined_target_operation_count(), 0);
        assert_eq!(collection.classified_target_operation_count(), 0);
        assert_eq!(collection.retained_correlation_count(), 0);
        assert_eq!(collection.pre_kir_elimination_count(), 0);
        assert!(collection.characteristics().is_empty());
    }

    #[test]
    fn one_to_many_ambiguity_and_elimination_remain_typed() {
        let coordinate = ProductionSourceIsaKirCoordinateV1::new(0, 0, 0).unwrap();
        let first = ProductionSourceIsaCharacteristicCorrelationV1 {
            catalog_record_ordinal: 3,
            record: ProductionSourceIsaCatalogRecordV1::characteristic_fixture_v1(
                Some([1; 32]),
                Some(coordinate),
                Some(ProductionSourceIsaCatalogTransformationV1::Duplicated),
            ),
        };
        let second = ProductionSourceIsaCharacteristicCorrelationV1 {
            catalog_record_ordinal: 7,
            record: ProductionSourceIsaCatalogRecordV1::characteristic_fixture_v1(
                Some([2; 32]),
                Some(coordinate),
                Some(ProductionSourceIsaCatalogTransformationV1::Coalesced),
            ),
        };
        assert_eq!(
            attribution(&[first, second]).unwrap(),
            ProductionSourceIsaCharacteristicAttributionV1::AmbiguousSources {
                distinct_source_count: 2,
                record_count: 2,
            }
        );

        let pre_kir_eliminated = ProductionSourceIsaCatalogRecordV1::characteristic_fixture_v1(
            Some([3; 32]),
            None,
            None,
        );
        assert!(is_pre_kir_eliminated(&pre_kir_eliminated));
        assert_eq!(pre_kir_eliminated.target_kir(), None);

        let backend_eliminated = ProductionSourceIsaCatalogRecordV1::characteristic_fixture_v1(
            Some([4; 32]),
            Some(coordinate),
            Some(ProductionSourceIsaCatalogTransformationV1::Eliminated),
        );
        assert!(!is_pre_kir_eliminated(&backend_eliminated));
        assert_eq!(backend_eliminated.target_kir(), Some(coordinate));
    }

    #[test]
    fn exact_matrix_profile_is_not_inferred_from_partial_shape() {
        let exact = MatrixMultiplyProfile::bf16_f32_m16n16k16_wave64();
        for profile in [
            MatrixMultiplyProfile { m: 32, ..exact },
            MatrixMultiplyProfile { n: 32, ..exact },
            MatrixMultiplyProfile { k: 8, ..exact },
            MatrixMultiplyProfile {
                input: MatrixElement::F32,
                ..exact
            },
            MatrixMultiplyProfile {
                wave_width: WaveWidth::Wave32,
                ..exact
            },
        ] {
            let matrix = MatrixOperation {
                kind: MatrixOperationKind::MultiplyAccumulate {
                    lhs: [ValueId(0); 4],
                    rhs: [ValueId(1); 4],
                    accumulator: [ValueId(2); 4],
                    profile,
                },
                active_lanes: 64,
                convergence: Convergence::uniform(SynchronizationScope::Subgroup),
                frontend_binding: None,
                tensor_layout: None,
            };
            assert_eq!(
                classify_operation(&operation(OperationKind::Matrix(matrix))),
                None
            );
        }
    }
}

//! One-way release of admitted producer characteristics into the authority-free observer schema.

use std::{error::Error, fmt};

use fe2o3_source_isa_observation::characteristic_v1::{
    InertSourceIsaCharacteristicCollectionV1, MAX_SOURCE_ISA_CHARACTERISTIC_CATALOG_RECORDS_V1,
    MAX_SOURCE_ISA_CHARACTERISTIC_CORRELATIONS_PER_TARGET_V1,
    MAX_SOURCE_ISA_CHARACTERISTIC_INTERVALS_V1,
    MAX_SOURCE_ISA_CHARACTERISTIC_PRE_KIR_ELIMINATIONS_V1,
    MAX_SOURCE_ISA_CHARACTERISTIC_TARGET_CORRELATIONS_V1, MAX_SOURCE_ISA_CHARACTERISTIC_TARGETS_V1,
    SourceIsaCharacteristicBindingV1, SourceIsaCharacteristicCollectionV1,
    SourceIsaCharacteristicCompilerHandoffLlvmCoordinateV1,
    SourceIsaCharacteristicContentIdentityV1, SourceIsaCharacteristicErrorV1,
    SourceIsaCharacteristicIsaIntervalV1, SourceIsaCharacteristicKindV1,
    SourceIsaCharacteristicKirCoordinateV1, SourceIsaCharacteristicKirVersionV1,
    SourceIsaCharacteristicMemoryFormV1, SourceIsaCharacteristicMirCoordinateV1,
    SourceIsaCharacteristicPreKirEliminationV1, SourceIsaCharacteristicRecordKindV1,
    SourceIsaCharacteristicScanStateV1, SourceIsaCharacteristicScanSummaryV1,
    SourceIsaCharacteristicSourceCoordinateV1, SourceIsaCharacteristicSourceSpanV1,
    SourceIsaCharacteristicStructuralCountsV1, SourceIsaCharacteristicTargetCorrelationV1,
    SourceIsaCharacteristicTargetProfileV1, SourceIsaCharacteristicTargetV1,
    SourceIsaCharacteristicTransformationV1,
};

use crate::{
    MAX_PRODUCTION_SOURCE_ISA_CATALOG_ISA_INTERVALS_V1,
    MAX_PRODUCTION_SOURCE_ISA_CATALOG_RECORDS_V1,
    MAX_PRODUCTION_SOURCE_ISA_CHARACTERISTIC_CORRELATIONS_PER_WITNESS_V1,
    MAX_PRODUCTION_SOURCE_ISA_CHARACTERISTIC_CORRELATIONS_V1,
    MAX_PRODUCTION_SOURCE_ISA_CHARACTERISTIC_ELIMINATIONS_V1,
    MAX_PRODUCTION_SOURCE_ISA_CHARACTERISTICS_V1, ProductionSourceIsaCatalogContentIdentityV1,
    ProductionSourceIsaCatalogKirVersionV1, ProductionSourceIsaCatalogRecordKindV1,
    ProductionSourceIsaCatalogRecordV1, ProductionSourceIsaCatalogTargetV1,
    ProductionSourceIsaCatalogTransformationV1, ProductionSourceIsaCharacteristicAttributionV1,
    ProductionSourceIsaCharacteristicCollectionV1, ProductionSourceIsaCharacteristicCorrelationV1,
    ProductionSourceIsaCharacteristicKindV1, ProductionSourceIsaCharacteristicMemoryFormV1,
    ProductionSourceIsaCharacteristicWitnessV1, ProductionSourceIsaKirCoordinateV1,
    ProductionSourceIsaLlvmCoordinateV1, ProductionSourceIsaMirCoordinateV1,
};

const _: () = assert!(
    MAX_PRODUCTION_SOURCE_ISA_CATALOG_RECORDS_V1
        == MAX_SOURCE_ISA_CHARACTERISTIC_CATALOG_RECORDS_V1
);
const _: () = assert!(
    MAX_PRODUCTION_SOURCE_ISA_CHARACTERISTICS_V1 == MAX_SOURCE_ISA_CHARACTERISTIC_TARGETS_V1
);
const _: () = assert!(
    MAX_PRODUCTION_SOURCE_ISA_CHARACTERISTIC_CORRELATIONS_V1
        == MAX_SOURCE_ISA_CHARACTERISTIC_TARGET_CORRELATIONS_V1
);
const _: () = assert!(
    MAX_PRODUCTION_SOURCE_ISA_CHARACTERISTIC_CORRELATIONS_PER_WITNESS_V1
        == MAX_SOURCE_ISA_CHARACTERISTIC_CORRELATIONS_PER_TARGET_V1
);
const _: () = assert!(
    MAX_PRODUCTION_SOURCE_ISA_CHARACTERISTIC_ELIMINATIONS_V1
        == MAX_SOURCE_ISA_CHARACTERISTIC_PRE_KIR_ELIMINATIONS_V1
);
const _: () = assert!(
    MAX_PRODUCTION_SOURCE_ISA_CATALOG_ISA_INTERVALS_V1
        == MAX_SOURCE_ISA_CHARACTERISTIC_INTERVALS_V1
);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProductionSourceIsaCharacteristicProjectionErrorV1 {
    InvalidProducerEvidence,
    SizeOverflow,
    AllocationFailure,
    Observation(SourceIsaCharacteristicErrorV1),
}

impl fmt::Display for ProductionSourceIsaCharacteristicProjectionErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidProducerEvidence => {
                formatter.write_str("invalid admitted producer characteristic evidence")
            }
            Self::SizeOverflow => formatter.write_str("producer characteristic size overflow"),
            Self::AllocationFailure => {
                formatter.write_str("cannot allocate bounded characteristic projection")
            }
            Self::Observation(error) => write!(formatter, "observer projection rejected: {error}"),
        }
    }
}

impl Error for ProductionSourceIsaCharacteristicProjectionErrorV1 {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Observation(error) => Some(error),
            Self::InvalidProducerEvidence | Self::SizeOverflow | Self::AllocationFailure => None,
        }
    }
}

impl From<SourceIsaCharacteristicErrorV1> for ProductionSourceIsaCharacteristicProjectionErrorV1 {
    fn from(error: SourceIsaCharacteristicErrorV1) -> Self {
        Self::Observation(error)
    }
}

/// Copies one admitted producer collection into the inert observer contract.
///
/// The returned value carries no compiler, proof, publication, runtime, debugger, profiler, or
/// hardware-observation authority. Producer-only derived attribution is checked before release
/// and can be reconstructed exactly from the retained correlations.
pub fn release_production_source_isa_characteristic_projection_v1(
    producer: &ProductionSourceIsaCharacteristicCollectionV1,
) -> Result<SourceIsaCharacteristicCollectionV1, ProductionSourceIsaCharacteristicProjectionErrorV1>
{
    if !producer.scan_is_complete() {
        return Err(ProductionSourceIsaCharacteristicProjectionErrorV1::InvalidProducerEvidence);
    }

    let binding = map_binding(producer)?;
    let mut targets = try_vec_capacity(producer.characteristics().len())?;
    for witness in producer.characteristics() {
        targets.push(map_witness(witness)?);
    }
    let mut pre_kir = try_vec_capacity(producer.pre_kir_eliminated_catalog_records().len())?;
    for correlation in producer.pre_kir_eliminated_catalog_records() {
        pre_kir.push(map_pre_kir(correlation)?);
    }

    validate_producer_counts(producer)?;
    let retained_count = producer
        .retained_correlation_count()
        .checked_add(producer.pre_kir_elimination_count())
        .ok_or(ProductionSourceIsaCharacteristicProjectionErrorV1::SizeOverflow)?;
    let scan = SourceIsaCharacteristicScanSummaryV1::new(
        producer.catalog_record_count(),
        producer.catalog_record_count(),
        producer.examined_target_operation_count(),
        producer.examined_target_operation_count(),
        producer.classified_target_operation_count(),
        producer.retained_correlation_count(),
        producer.pre_kir_elimination_count(),
        retained_count,
        SourceIsaCharacteristicScanStateV1::Complete,
    )?;
    Ok(SourceIsaCharacteristicCollectionV1::new(
        binding, scan, targets, pre_kir,
    )?)
}

/// Re-admits a decoded observer claim only when it equals a fresh projection of independently
/// admitted producer evidence.
pub fn readmit_exact_production_source_isa_characteristic_projection_v1(
    inert: InertSourceIsaCharacteristicCollectionV1,
    producer: &ProductionSourceIsaCharacteristicCollectionV1,
) -> Result<SourceIsaCharacteristicCollectionV1, ProductionSourceIsaCharacteristicProjectionErrorV1>
{
    let expected = release_production_source_isa_characteristic_projection_v1(producer)?;
    Ok(inert.admit_exact_projection_v1(&expected)?)
}

fn validate_producer_counts(
    producer: &ProductionSourceIsaCharacteristicCollectionV1,
) -> Result<(), ProductionSourceIsaCharacteristicProjectionErrorV1> {
    let target_count = u64::try_from(producer.characteristics().len())
        .map_err(|_| ProductionSourceIsaCharacteristicProjectionErrorV1::SizeOverflow)?;
    let pre_count = u64::try_from(producer.pre_kir_eliminated_catalog_records().len())
        .map_err(|_| ProductionSourceIsaCharacteristicProjectionErrorV1::SizeOverflow)?;
    let correlation_count =
        producer
            .characteristics()
            .iter()
            .try_fold(0_u64, |count, witness| {
                count
                    .checked_add(u64::try_from(witness.correlations().len()).map_err(|_| {
                        ProductionSourceIsaCharacteristicProjectionErrorV1::SizeOverflow
                    })?)
                    .ok_or(ProductionSourceIsaCharacteristicProjectionErrorV1::SizeOverflow)
            })?;
    if target_count != producer.classified_target_operation_count()
        || pre_count != producer.pre_kir_elimination_count()
        || correlation_count != producer.retained_correlation_count()
    {
        return Err(ProductionSourceIsaCharacteristicProjectionErrorV1::InvalidProducerEvidence);
    }
    Ok(())
}

fn map_binding(
    producer: &ProductionSourceIsaCharacteristicCollectionV1,
) -> Result<SourceIsaCharacteristicBindingV1, ProductionSourceIsaCharacteristicProjectionErrorV1> {
    let counts = producer.structural_counts();
    let artifact = producer.artifact_identity();
    Ok(SourceIsaCharacteristicBindingV1::new(
        match producer.target() {
            ProductionSourceIsaCatalogTargetV1::Gfx942 => {
                SourceIsaCharacteristicTargetProfileV1::Gfx942
            }
            ProductionSourceIsaCatalogTargetV1::Gfx950 => {
                SourceIsaCharacteristicTargetProfileV1::Gfx950
            }
        },
        match producer.kir_version() {
            ProductionSourceIsaCatalogKirVersionV1::V8 => SourceIsaCharacteristicKirVersionV1::V8,
            ProductionSourceIsaCatalogKirVersionV1::V9 => SourceIsaCharacteristicKirVersionV1::V9,
        },
        *producer.structural_binding_identity(),
        SourceIsaCharacteristicStructuralCountsV1 {
            functions: counts.functions(),
            defined_bodies: counts.defined_bodies(),
            blocks: counts.blocks(),
            operations: counts.operations(),
        },
        map_content(producer.source_map_v2_identity())?,
        map_content(producer.neutral_kir_identity())?,
        map_content(producer.target_kir_identity())?,
        SourceIsaCharacteristicContentIdentityV1::new(*artifact.sha256(), artifact.byte_len())?,
        SourceIsaCharacteristicContentIdentityV1::new(
            *producer.catalog_identity(),
            producer.catalog_byte_len(),
        )?,
        SourceIsaCharacteristicContentIdentityV1::new(
            *producer.structural_bridge_identity(),
            producer.structural_bridge_byte_len(),
        )?,
        *producer.correlation_identity(),
        *producer.semantic_map_identity(),
    )?)
}

fn map_content(
    identity: ProductionSourceIsaCatalogContentIdentityV1,
) -> Result<SourceIsaCharacteristicContentIdentityV1, SourceIsaCharacteristicErrorV1> {
    SourceIsaCharacteristicContentIdentityV1::new(identity.sha256(), identity.byte_len())
}

fn map_witness(
    witness: &ProductionSourceIsaCharacteristicWitnessV1,
) -> Result<SourceIsaCharacteristicTargetV1, ProductionSourceIsaCharacteristicProjectionErrorV1> {
    validate_attribution(witness)?;
    let target_kir = map_kir(witness.target_kir())?;
    let mut correlations = try_vec_capacity(witness.correlations().len())?;
    for correlation in witness.correlations() {
        correlations.push(map_target_correlation(correlation, witness.target_kir())?);
    }
    Ok(SourceIsaCharacteristicTargetV1::new(
        map_characteristic_kind(witness.kind()),
        target_kir,
        correlations,
    )?)
}

fn validate_attribution(
    witness: &ProductionSourceIsaCharacteristicWitnessV1,
) -> Result<(), ProductionSourceIsaCharacteristicProjectionErrorV1> {
    let mut source_identities = try_vec_capacity(witness.correlations().len())?;
    let mut structural_record_count = 0_u64;
    for correlation in witness.correlations() {
        match correlation.record().kind() {
            ProductionSourceIsaCatalogRecordKindV1::SourceAnchored => {
                source_identities.push(correlation.record().source_node_identity().ok_or(
                    ProductionSourceIsaCharacteristicProjectionErrorV1::InvalidProducerEvidence,
                )?);
            }
            ProductionSourceIsaCatalogRecordKindV1::NoSourceProvenance => {
                structural_record_count = structural_record_count
                    .checked_add(1)
                    .ok_or(ProductionSourceIsaCharacteristicProjectionErrorV1::SizeOverflow)?;
            }
            ProductionSourceIsaCatalogRecordKindV1::EliminatedBeforeKir => {
                return Err(
                    ProductionSourceIsaCharacteristicProjectionErrorV1::InvalidProducerEvidence,
                );
            }
        }
    }
    source_identities.sort_unstable();
    source_identities.dedup();
    let distinct_source_count = u64::try_from(source_identities.len())
        .map_err(|_| ProductionSourceIsaCharacteristicProjectionErrorV1::SizeOverflow)?;
    let record_count = u64::try_from(witness.correlations().len())
        .map_err(|_| ProductionSourceIsaCharacteristicProjectionErrorV1::SizeOverflow)?;
    let expected = match (distinct_source_count, structural_record_count) {
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
    };
    if witness.attribution() != expected {
        return Err(ProductionSourceIsaCharacteristicProjectionErrorV1::InvalidProducerEvidence);
    }
    Ok(())
}

fn map_target_correlation(
    correlation: &ProductionSourceIsaCharacteristicCorrelationV1,
    expected_target_kir: ProductionSourceIsaKirCoordinateV1,
) -> Result<
    SourceIsaCharacteristicTargetCorrelationV1,
    ProductionSourceIsaCharacteristicProjectionErrorV1,
> {
    let record = correlation.record();
    let kind = match record.kind() {
        ProductionSourceIsaCatalogRecordKindV1::SourceAnchored => {
            SourceIsaCharacteristicRecordKindV1::SourceAnchored
        }
        ProductionSourceIsaCatalogRecordKindV1::NoSourceProvenance => {
            SourceIsaCharacteristicRecordKindV1::NoSourceProvenance
        }
        ProductionSourceIsaCatalogRecordKindV1::EliminatedBeforeKir => {
            return Err(
                ProductionSourceIsaCharacteristicProjectionErrorV1::InvalidProducerEvidence,
            );
        }
    };
    let producer_target_kir = record
        .target_kir()
        .ok_or(ProductionSourceIsaCharacteristicProjectionErrorV1::InvalidProducerEvidence)?;
    if producer_target_kir != expected_target_kir {
        return Err(ProductionSourceIsaCharacteristicProjectionErrorV1::InvalidProducerEvidence);
    }
    let mut intervals = try_vec_capacity(record.isa().len())?;
    for interval in record.isa() {
        intervals.push(SourceIsaCharacteristicIsaIntervalV1::new(
            interval.kernel_ordinal(),
            interval.byte_start(),
            interval.byte_end(),
        )?);
    }
    Ok(SourceIsaCharacteristicTargetCorrelationV1::new(
        correlation.catalog_record_ordinal(),
        kind,
        map_source(record)?,
        record.mir_node_identity(),
        map_optional_mir(record.mir())?,
        record.neutral_kir_node_identity(),
        map_optional_kir(record.neutral_kir())?,
        map_kir(producer_target_kir)?,
        record
            .semantic_operation_id()
            .ok_or(ProductionSourceIsaCharacteristicProjectionErrorV1::InvalidProducerEvidence)?,
        map_llvm(
            record.compiler_handoff_llvm().ok_or(
                ProductionSourceIsaCharacteristicProjectionErrorV1::InvalidProducerEvidence,
            )?,
        )?,
        intervals,
        map_transformation(
            record.transformation().ok_or(
                ProductionSourceIsaCharacteristicProjectionErrorV1::InvalidProducerEvidence,
            )?,
        ),
    )?)
}

fn map_pre_kir(
    correlation: &ProductionSourceIsaCharacteristicCorrelationV1,
) -> Result<
    SourceIsaCharacteristicPreKirEliminationV1,
    ProductionSourceIsaCharacteristicProjectionErrorV1,
> {
    let record = correlation.record();
    if record.kind() != ProductionSourceIsaCatalogRecordKindV1::EliminatedBeforeKir
        || record.neutral_kir_node_identity().is_some()
        || record.neutral_kir().is_some()
        || record.target_kir().is_some()
        || record.semantic_operation_id().is_some()
        || record.compiler_handoff_llvm().is_some()
        || !record.isa().is_empty()
        || record.transformation().is_some()
    {
        return Err(ProductionSourceIsaCharacteristicProjectionErrorV1::InvalidProducerEvidence);
    }
    Ok(SourceIsaCharacteristicPreKirEliminationV1::new(
        correlation.catalog_record_ordinal(),
        map_source(record)?
            .ok_or(ProductionSourceIsaCharacteristicProjectionErrorV1::InvalidProducerEvidence)?,
        record
            .mir_node_identity()
            .ok_or(ProductionSourceIsaCharacteristicProjectionErrorV1::InvalidProducerEvidence)?,
        map_mir(
            record.mir().ok_or(
                ProductionSourceIsaCharacteristicProjectionErrorV1::InvalidProducerEvidence,
            )?,
        )?,
    )?)
}

fn map_source(
    record: &ProductionSourceIsaCatalogRecordV1,
) -> Result<
    Option<SourceIsaCharacteristicSourceCoordinateV1>,
    ProductionSourceIsaCharacteristicProjectionErrorV1,
> {
    match (record.source_node_identity(), record.source_span()) {
        (Some(node_identity), Some(span)) => {
            let span = SourceIsaCharacteristicSourceSpanV1::new(
                span.file_identity(),
                span.byte_start(),
                span.byte_end(),
                span.line(),
                span.column(),
            )?;
            Ok(Some(SourceIsaCharacteristicSourceCoordinateV1::new(
                node_identity,
                span,
            )?))
        }
        (None, None) => Ok(None),
        _ => Err(ProductionSourceIsaCharacteristicProjectionErrorV1::InvalidProducerEvidence),
    }
}

fn map_characteristic_kind(
    kind: ProductionSourceIsaCharacteristicKindV1,
) -> SourceIsaCharacteristicKindV1 {
    match kind {
        ProductionSourceIsaCharacteristicKindV1::GlobalStore { form } => {
            SourceIsaCharacteristicKindV1::GlobalStore {
                form: map_memory_form(form),
            }
        }
        ProductionSourceIsaCharacteristicKindV1::WorkgroupLoad { form } => {
            SourceIsaCharacteristicKindV1::WorkgroupLoad {
                form: map_memory_form(form),
            }
        }
        ProductionSourceIsaCharacteristicKindV1::WorkgroupStore { form } => {
            SourceIsaCharacteristicKindV1::WorkgroupStore {
                form: map_memory_form(form),
            }
        }
        ProductionSourceIsaCharacteristicKindV1::WorkgroupBarrier => {
            SourceIsaCharacteristicKindV1::WorkgroupBarrier
        }
        ProductionSourceIsaCharacteristicKindV1::Bf16F32M16N16K16Wave64MatrixMultiplyAccumulate => {
            SourceIsaCharacteristicKindV1::Bf16F32M16N16K16Wave64MatrixMultiplyAccumulate
        }
    }
}

fn map_memory_form(
    form: ProductionSourceIsaCharacteristicMemoryFormV1,
) -> SourceIsaCharacteristicMemoryFormV1 {
    match form {
        ProductionSourceIsaCharacteristicMemoryFormV1::Plain => {
            SourceIsaCharacteristicMemoryFormV1::Plain
        }
        ProductionSourceIsaCharacteristicMemoryFormV1::Guarded => {
            SourceIsaCharacteristicMemoryFormV1::Guarded
        }
        ProductionSourceIsaCharacteristicMemoryFormV1::MatrixTile => {
            SourceIsaCharacteristicMemoryFormV1::MatrixTile
        }
    }
}

fn map_transformation(
    transformation: ProductionSourceIsaCatalogTransformationV1,
) -> SourceIsaCharacteristicTransformationV1 {
    match transformation {
        ProductionSourceIsaCatalogTransformationV1::Preserved => {
            SourceIsaCharacteristicTransformationV1::Preserved
        }
        ProductionSourceIsaCatalogTransformationV1::Duplicated => {
            SourceIsaCharacteristicTransformationV1::Duplicated
        }
        ProductionSourceIsaCatalogTransformationV1::Coalesced => {
            SourceIsaCharacteristicTransformationV1::Coalesced
        }
        ProductionSourceIsaCatalogTransformationV1::DuplicatedAndCoalesced => {
            SourceIsaCharacteristicTransformationV1::DuplicatedAndCoalesced
        }
        ProductionSourceIsaCatalogTransformationV1::Eliminated => {
            SourceIsaCharacteristicTransformationV1::Eliminated
        }
    }
}

fn map_mir(
    coordinate: ProductionSourceIsaMirCoordinateV1,
) -> Result<SourceIsaCharacteristicMirCoordinateV1, SourceIsaCharacteristicErrorV1> {
    SourceIsaCharacteristicMirCoordinateV1::new(
        coordinate.body_ordinal(),
        coordinate.block_ordinal(),
        coordinate.statement_ordinal(),
    )
}

fn map_optional_mir(
    coordinate: Option<ProductionSourceIsaMirCoordinateV1>,
) -> Result<Option<SourceIsaCharacteristicMirCoordinateV1>, SourceIsaCharacteristicErrorV1> {
    coordinate.map(map_mir).transpose()
}

fn map_kir(
    coordinate: ProductionSourceIsaKirCoordinateV1,
) -> Result<SourceIsaCharacteristicKirCoordinateV1, SourceIsaCharacteristicErrorV1> {
    SourceIsaCharacteristicKirCoordinateV1::new(
        coordinate.function_ordinal(),
        coordinate.block_ordinal(),
        coordinate.operation_ordinal(),
    )
}

fn map_optional_kir(
    coordinate: Option<ProductionSourceIsaKirCoordinateV1>,
) -> Result<Option<SourceIsaCharacteristicKirCoordinateV1>, SourceIsaCharacteristicErrorV1> {
    coordinate.map(map_kir).transpose()
}

fn map_llvm(
    coordinate: ProductionSourceIsaLlvmCoordinateV1,
) -> Result<SourceIsaCharacteristicCompilerHandoffLlvmCoordinateV1, SourceIsaCharacteristicErrorV1>
{
    SourceIsaCharacteristicCompilerHandoffLlvmCoordinateV1::new(
        coordinate.function_ordinal(),
        coordinate.block_ordinal(),
        coordinate.instruction_ordinal(),
    )
}

fn try_vec_capacity<T>(
    capacity: usize,
) -> Result<Vec<T>, ProductionSourceIsaCharacteristicProjectionErrorV1> {
    let mut values = Vec::new();
    values
        .try_reserve_exact(capacity)
        .map_err(|_| ProductionSourceIsaCharacteristicProjectionErrorV1::AllocationFailure)?;
    Ok(values)
}

#[cfg(test)]
mod tests {
    use fe2o3_kernel_ir::{
        AddressSpace, BarrierSemantics, BasicBlock, BlockId, Constant, Convergence, Function,
        Module, Operation, OperationKind, ScalarType, Signature, SynchronizationScope, Terminator,
        Type, ValueDef, ValueId, VerifiedCanonicalKernelIrV8, WorkgroupBarrier,
    };
    use fe2o3_source_isa_observation::characteristic_v1::{
        SourceIsaCharacteristicQueryV1, SourceIsaCharacteristicTargetQueryV1,
    };

    use crate::{
        ProductionSourceIsaCatalogRecordV1, ProductionSourceIsaCatalogStructuralCountsV1,
        ProductionSourceIsaCharacteristicAdmissionV1,
        admit_production_source_isa_characteristics_v1,
    };

    use super::*;

    fn id(byte: u8) -> [u8; 32] {
        [byte; 32]
    }

    fn characteristic_module(with_constant: bool) -> Module {
        let mut block = BasicBlock::new(BlockId(0));
        if with_constant {
            block.operations.push(Operation::effect_free(
                ValueDef::new(ValueId(0), Type::Scalar(ScalarType::U32)),
                OperationKind::Constant(Constant::U32(7)),
            ));
        }
        block.operations.push(Operation::new(
            Vec::new(),
            OperationKind::WorkgroupBarrier(WorkgroupBarrier {
                memory_scope: SynchronizationScope::Workgroup,
                semantics: BarrierSemantics::new(
                    fe2o3_kernel_ir::MemoryOrdering::AcquireRelease,
                    [AddressSpace::Workgroup],
                ),
                convergence: Convergence::uniform(SynchronizationScope::Workgroup),
            }),
        ));
        block.terminator = Some(Terminator::Return { values: Vec::new() });
        let mut module = Module::new("observer-release-fixture");
        module.functions.push(Function::definition(
            "entry",
            Signature::new(Vec::new(), Vec::new()),
            Vec::new(),
            vec![block],
        ));
        module
    }

    fn admit_fixture() -> ProductionSourceIsaCharacteristicCollectionV1 {
        let verified = VerifiedCanonicalKernelIrV8::from_module(characteristic_module(true))
            .expect("valid characteristic fixture KIR");
        let constant = ProductionSourceIsaKirCoordinateV1::new(0, 0, 0).unwrap();
        let barrier = ProductionSourceIsaKirCoordinateV1::new(0, 0, 1).unwrap();
        let duplicate =
            ProductionSourceIsaCatalogRecordV1::characteristic_fixture_with_duplicate_isa_v1(
                id(0x51),
                barrier,
                ProductionSourceIsaCatalogTransformationV1::Duplicated,
            );
        let records = vec![
            duplicate.clone(),
            duplicate,
            ProductionSourceIsaCatalogRecordV1::characteristic_fixture_v1(
                None,
                Some(barrier),
                Some(ProductionSourceIsaCatalogTransformationV1::Eliminated),
            ),
            ProductionSourceIsaCatalogRecordV1::characteristic_fixture_v1(
                Some(id(0x52)),
                Some(constant),
                Some(ProductionSourceIsaCatalogTransformationV1::Preserved),
            ),
            ProductionSourceIsaCatalogRecordV1::characteristic_pre_kir_empty_span_fixture_v1(id(
                0x53,
            )),
        ];
        let counts = ProductionSourceIsaCatalogStructuralCountsV1::new_for_bridge_v1(1, 1, 1, 2);
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
        let ProductionSourceIsaCharacteristicAdmissionV1::Admitted(producer) = admission else {
            panic!("bounded characteristic fixture must be admitted")
        };
        producer
    }

    fn admit_empty_fixture() -> ProductionSourceIsaCharacteristicCollectionV1 {
        let verified = VerifiedCanonicalKernelIrV8::from_module({
            let mut block = BasicBlock::new(BlockId(0));
            block.terminator = Some(Terminator::Return { values: Vec::new() });
            let mut module = Module::new("empty-observer-release-fixture");
            module.functions.push(Function::definition(
                "entry",
                Signature::new(Vec::new(), Vec::new()),
                Vec::new(),
                vec![block],
            ));
            module
        })
        .unwrap();
        let counts = ProductionSourceIsaCatalogStructuralCountsV1::new_for_bridge_v1(1, 1, 1, 0);
        let catalog = crate::production_source_isa_catalog_v1::characteristic_catalog_fixture_v1(
            *verified.identity().digest(),
            verified.identity().canonical_length(),
            counts,
            Vec::new(),
        );
        let bridge =
            crate::production_kir_v7_structural_bridge_v1::characteristic_bridge_fixture_v1(
                verified.canonical_bytes(),
                &catalog,
            )
            .unwrap();
        let ProductionSourceIsaCharacteristicAdmissionV1::Admitted(producer) =
            admit_production_source_isa_characteristics_v1(
                verified.canonical_bytes(),
                &catalog,
                &bridge,
            )
            .unwrap()
        else {
            panic!("empty characteristic fixture must be admitted")
        };
        producer
    }

    #[test]
    fn producer_and_observer_resource_bounds_are_identical() {
        assert_eq!(
            MAX_PRODUCTION_SOURCE_ISA_CATALOG_RECORDS_V1,
            MAX_SOURCE_ISA_CHARACTERISTIC_CATALOG_RECORDS_V1
        );
        assert_eq!(
            MAX_PRODUCTION_SOURCE_ISA_CHARACTERISTICS_V1,
            MAX_SOURCE_ISA_CHARACTERISTIC_TARGETS_V1
        );
        assert_eq!(
            MAX_PRODUCTION_SOURCE_ISA_CHARACTERISTIC_CORRELATIONS_V1,
            MAX_SOURCE_ISA_CHARACTERISTIC_TARGET_CORRELATIONS_V1
        );
        assert_eq!(
            MAX_PRODUCTION_SOURCE_ISA_CHARACTERISTIC_CORRELATIONS_PER_WITNESS_V1,
            MAX_SOURCE_ISA_CHARACTERISTIC_CORRELATIONS_PER_TARGET_V1
        );
        assert_eq!(
            MAX_PRODUCTION_SOURCE_ISA_CHARACTERISTIC_ELIMINATIONS_V1,
            MAX_SOURCE_ISA_CHARACTERISTIC_PRE_KIR_ELIMINATIONS_V1
        );
        assert_eq!(
            MAX_PRODUCTION_SOURCE_ISA_CATALOG_ISA_INTERVALS_V1,
            MAX_SOURCE_ISA_CHARACTERISTIC_INTERVALS_V1
        );
    }

    #[test]
    fn every_target_kind_and_memory_form_maps_exactly() {
        for (producer, observer) in [
            (
                ProductionSourceIsaCharacteristicKindV1::GlobalStore {
                    form: ProductionSourceIsaCharacteristicMemoryFormV1::Plain,
                },
                SourceIsaCharacteristicKindV1::GlobalStore {
                    form: SourceIsaCharacteristicMemoryFormV1::Plain,
                },
            ),
            (
                ProductionSourceIsaCharacteristicKindV1::GlobalStore {
                    form: ProductionSourceIsaCharacteristicMemoryFormV1::Guarded,
                },
                SourceIsaCharacteristicKindV1::GlobalStore {
                    form: SourceIsaCharacteristicMemoryFormV1::Guarded,
                },
            ),
            (
                ProductionSourceIsaCharacteristicKindV1::GlobalStore {
                    form: ProductionSourceIsaCharacteristicMemoryFormV1::MatrixTile,
                },
                SourceIsaCharacteristicKindV1::GlobalStore {
                    form: SourceIsaCharacteristicMemoryFormV1::MatrixTile,
                },
            ),
            (
                ProductionSourceIsaCharacteristicKindV1::WorkgroupLoad {
                    form: ProductionSourceIsaCharacteristicMemoryFormV1::Plain,
                },
                SourceIsaCharacteristicKindV1::WorkgroupLoad {
                    form: SourceIsaCharacteristicMemoryFormV1::Plain,
                },
            ),
            (
                ProductionSourceIsaCharacteristicKindV1::WorkgroupLoad {
                    form: ProductionSourceIsaCharacteristicMemoryFormV1::Guarded,
                },
                SourceIsaCharacteristicKindV1::WorkgroupLoad {
                    form: SourceIsaCharacteristicMemoryFormV1::Guarded,
                },
            ),
            (
                ProductionSourceIsaCharacteristicKindV1::WorkgroupLoad {
                    form: ProductionSourceIsaCharacteristicMemoryFormV1::MatrixTile,
                },
                SourceIsaCharacteristicKindV1::WorkgroupLoad {
                    form: SourceIsaCharacteristicMemoryFormV1::MatrixTile,
                },
            ),
            (
                ProductionSourceIsaCharacteristicKindV1::WorkgroupStore {
                    form: ProductionSourceIsaCharacteristicMemoryFormV1::Plain,
                },
                SourceIsaCharacteristicKindV1::WorkgroupStore {
                    form: SourceIsaCharacteristicMemoryFormV1::Plain,
                },
            ),
            (
                ProductionSourceIsaCharacteristicKindV1::WorkgroupStore {
                    form: ProductionSourceIsaCharacteristicMemoryFormV1::Guarded,
                },
                SourceIsaCharacteristicKindV1::WorkgroupStore {
                    form: SourceIsaCharacteristicMemoryFormV1::Guarded,
                },
            ),
            (
                ProductionSourceIsaCharacteristicKindV1::WorkgroupStore {
                    form: ProductionSourceIsaCharacteristicMemoryFormV1::MatrixTile,
                },
                SourceIsaCharacteristicKindV1::WorkgroupStore {
                    form: SourceIsaCharacteristicMemoryFormV1::MatrixTile,
                },
            ),
            (
                ProductionSourceIsaCharacteristicKindV1::WorkgroupBarrier,
                SourceIsaCharacteristicKindV1::WorkgroupBarrier,
            ),
            (
                ProductionSourceIsaCharacteristicKindV1::Bf16F32M16N16K16Wave64MatrixMultiplyAccumulate,
                SourceIsaCharacteristicKindV1::Bf16F32M16N16K16Wave64MatrixMultiplyAccumulate,
            ),
        ] {
            assert_eq!(map_characteristic_kind(producer), observer);
        }
    }

    #[test]
    fn release_is_lossless_bounded_and_authority_free() {
        let producer = admit_fixture();
        let released = release_production_source_isa_characteristic_projection_v1(&producer)
            .expect("exact producer projection");
        assert_eq!(
            released.binding().kir_version(),
            SourceIsaCharacteristicKirVersionV1::V8
        );
        assert_eq!(released.scan().catalog_record_count(), 5);
        assert_eq!(released.scan().catalog_records_scanned(), 5);
        assert_eq!(released.scan().target_operation_count(), 2);
        assert_eq!(released.scan().target_operations_scanned(), 2);
        assert_eq!(released.scan().classified_target_count(), 1);
        assert_eq!(released.scan().retained_target_correlation_count(), 3);
        assert_eq!(released.scan().pre_kir_elimination_count(), 1);
        assert_eq!(released.scan().correlation_count(), 4);
        assert!(released.scan().is_complete());
        assert_eq!(released.targets().len(), 1);
        assert_eq!(
            released.targets()[0].kind(),
            SourceIsaCharacteristicKindV1::WorkgroupBarrier
        );
        assert_eq!(released.targets()[0].correlations().len(), 3);

        let source_correlations = released.targets()[0]
            .correlations()
            .iter()
            .filter(|record| record.kind() == SourceIsaCharacteristicRecordKindV1::SourceAnchored)
            .collect::<Vec<_>>();
        assert_eq!(source_correlations.len(), 2);
        assert_ne!(
            source_correlations[0].catalog_record_ordinal(),
            source_correlations[1].catalog_record_ordinal()
        );
        assert_eq!(source_correlations[0].isa_intervals().len(), 2);
        assert_eq!(
            source_correlations[0].isa_intervals()[0],
            source_correlations[0].isa_intervals()[1]
        );
        assert_eq!(
            source_correlations[0].isa_intervals(),
            source_correlations[1].isa_intervals()
        );
        let backend_eliminated = released.targets()[0]
            .correlations()
            .iter()
            .find(|record| record.kind() == SourceIsaCharacteristicRecordKindV1::NoSourceProvenance)
            .unwrap();
        assert_eq!(
            backend_eliminated.transformation(),
            SourceIsaCharacteristicTransformationV1::Eliminated
        );
        assert!(backend_eliminated.isa_intervals().is_empty());
        let pre = released.pre_kir_eliminations()[0];
        assert_eq!(
            pre.source().span().byte_start(),
            pre.source().span().byte_end()
        );

        assert!(!released.grants_compiler_authority());
        assert!(!released.grants_proof_authority());
        assert!(!released.grants_publication_authority());
        assert!(!released.grants_runtime_authority());
        assert!(!released.grants_hardware_observation_authority());
        assert!(!released.proves_semantic_refinement());
        assert!(!released.proves_final_llvm_classification());
        assert!(!released.proves_final_isa_opcode_classification());
        assert!(!released.proves_a_schedule());

        let bytes = released.encode_canonical().unwrap();
        assert_eq!(released.canonical_byte_len().unwrap(), bytes.len() as u64);
        let inert = InertSourceIsaCharacteristicCollectionV1::decode_canonical(&bytes).unwrap();
        assert!(!inert.grants_compiler_authority());
        assert!(!inert.grants_runtime_authority());
        assert!(!inert.grants_hardware_observation_authority());
        let readmitted =
            readmit_exact_production_source_isa_characteristic_projection_v1(inert, &producer)
                .unwrap();
        assert_eq!(readmitted.identity(), released.identity());
    }

    #[test]
    fn complete_zero_characteristic_scan_releases_without_sentinels() {
        let producer = admit_empty_fixture();
        let released =
            release_production_source_isa_characteristic_projection_v1(&producer).unwrap();
        assert!(released.targets().is_empty());
        assert!(released.pre_kir_eliminations().is_empty());
        assert_eq!(released.scan().classified_target_count(), 0);
        assert_eq!(released.scan().correlation_count(), 0);
        assert!(released.scan().is_complete());
    }

    #[test]
    fn structural_target_without_catalog_correlation_admits_releases_and_queries_exactly() {
        let verified = VerifiedCanonicalKernelIrV8::from_module(characteristic_module(false))
            .expect("valid structural-only target KIR");
        let counts = ProductionSourceIsaCatalogStructuralCountsV1::new_for_bridge_v1(1, 1, 1, 1);
        let catalog = crate::production_source_isa_catalog_v1::characteristic_catalog_fixture_v1(
            *verified.identity().digest(),
            verified.identity().canonical_length(),
            counts,
            Vec::new(),
        );
        let bridge =
            crate::production_kir_v7_structural_bridge_v1::characteristic_bridge_fixture_v1(
                verified.canonical_bytes(),
                &catalog,
            )
            .unwrap();
        let ProductionSourceIsaCharacteristicAdmissionV1::Admitted(producer) =
            admit_production_source_isa_characteristics_v1(
                verified.canonical_bytes(),
                &catalog,
                &bridge,
            )
            .unwrap()
        else {
            panic!("bounded structural-only characteristic must be admitted")
        };

        assert_eq!(producer.catalog_record_count(), 0);
        assert_eq!(producer.examined_target_operation_count(), 1);
        assert_eq!(producer.classified_target_operation_count(), 1);
        assert_eq!(producer.retained_correlation_count(), 0);
        assert_eq!(producer.pre_kir_elimination_count(), 0);
        assert_eq!(producer.characteristics().len(), 1);
        assert_eq!(
            producer.characteristics()[0].kind(),
            ProductionSourceIsaCharacteristicKindV1::WorkgroupBarrier
        );
        assert_eq!(
            producer.characteristics()[0].attribution(),
            ProductionSourceIsaCharacteristicAttributionV1::StructuralOnly { record_count: 0 }
        );
        assert!(producer.characteristics()[0].correlations().is_empty());

        let released = release_production_source_isa_characteristic_projection_v1(&producer)
            .expect("exact structural-only observer release");
        assert_eq!(released.scan().classified_target_count(), 1);
        assert_eq!(released.scan().retained_target_correlation_count(), 0);
        assert_eq!(released.scan().correlation_count(), 0);
        assert_eq!(released.targets().len(), 1);
        assert!(released.targets()[0].correlations().is_empty());

        let target_page = released
            .query_targets_page(&SourceIsaCharacteristicTargetQueryV1::All, None, 1)
            .unwrap();
        assert_eq!(target_page.total_matches(), 1);
        assert_eq!(target_page.targets().len(), 1);
        assert_eq!(target_page.targets()[0].correlation_count(), 0);
        let fact_page = released
            .query_page(&SourceIsaCharacteristicQueryV1::All, None, 1)
            .unwrap();
        assert_eq!(fact_page.total_matches(), 0);
        assert!(fact_page.matches().is_empty());
        assert_eq!(
            released.interval_page(target_page.targets()[0].identity(), None, 1),
            Err(SourceIsaCharacteristicErrorV1::InvalidClaim)
        );
    }

    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    enum HostileMutation {
        KindAndMemoryForm,
        CatalogRecordOrdinal,
        CorrelationMultiplicity,
        RecordKind,
        Source,
        MirNode,
        MirCoordinate,
        NeutralKirNode,
        NeutralKirCoordinate,
        TargetKirCoordinate,
        SemanticOperation,
        CompilerHandoffLlvm,
        Transformation,
        IsaInterval,
        IsaMultiplicity,
        PreKir,
        ScanCount,
        TargetProfile,
        KirVersion,
        StructuralIdentity,
        StructuralCounts,
        SourceMap,
        ContentByteLength,
        NeutralKirContent,
        TargetKirContent,
        Artifact,
        Catalog,
        StructuralBridge,
        CorrelationIdentity,
        SemanticMapIdentity,
    }

    #[test]
    fn independently_resealed_substitutions_fail_producer_backed_readmission() {
        let producer = admit_fixture();
        let exact = release_production_source_isa_characteristic_projection_v1(&producer).unwrap();
        for mutation in [
            HostileMutation::KindAndMemoryForm,
            HostileMutation::CatalogRecordOrdinal,
            HostileMutation::CorrelationMultiplicity,
            HostileMutation::RecordKind,
            HostileMutation::Source,
            HostileMutation::MirNode,
            HostileMutation::MirCoordinate,
            HostileMutation::NeutralKirNode,
            HostileMutation::NeutralKirCoordinate,
            HostileMutation::TargetKirCoordinate,
            HostileMutation::SemanticOperation,
            HostileMutation::CompilerHandoffLlvm,
            HostileMutation::Transformation,
            HostileMutation::IsaInterval,
            HostileMutation::IsaMultiplicity,
            HostileMutation::PreKir,
            HostileMutation::ScanCount,
            HostileMutation::TargetProfile,
            HostileMutation::KirVersion,
            HostileMutation::StructuralIdentity,
            HostileMutation::StructuralCounts,
            HostileMutation::SourceMap,
            HostileMutation::ContentByteLength,
            HostileMutation::NeutralKirContent,
            HostileMutation::TargetKirContent,
            HostileMutation::Artifact,
            HostileMutation::Catalog,
            HostileMutation::StructuralBridge,
            HostileMutation::CorrelationIdentity,
            HostileMutation::SemanticMapIdentity,
        ] {
            let substituted = rebuild_with_mutation(&exact, mutation);
            assert_ne!(substituted.identity(), exact.identity(), "{mutation:?}");
            let encoded = substituted.encode_canonical().unwrap();
            let inert = InertSourceIsaCharacteristicCollectionV1::decode_canonical(&encoded)
                .expect("hostile projection remains canonical and inert");
            assert!(
                matches!(
                    readmit_exact_production_source_isa_characteristic_projection_v1(
                        inert, &producer
                    ),
                    Err(
                        ProductionSourceIsaCharacteristicProjectionErrorV1::Observation(
                            SourceIsaCharacteristicErrorV1::IdentityMismatch
                        )
                    )
                ),
                "{mutation:?}"
            );
        }
    }

    fn rebuild_with_mutation(
        exact: &SourceIsaCharacteristicCollectionV1,
        mutation: HostileMutation,
    ) -> SourceIsaCharacteristicCollectionV1 {
        let binding = rebuilt_binding(exact.binding(), mutation);
        let original_scan = exact.scan();
        let extra_catalog_record = u64::from(matches!(mutation, HostileMutation::ScanCount));
        let extra_correlation =
            u64::from(matches!(mutation, HostileMutation::CorrelationMultiplicity));
        let scan = SourceIsaCharacteristicScanSummaryV1::new(
            original_scan.catalog_record_count() + extra_catalog_record,
            original_scan.catalog_records_scanned() + extra_catalog_record,
            original_scan.target_operation_count(),
            original_scan.target_operations_scanned(),
            original_scan.classified_target_count(),
            original_scan.retained_target_correlation_count() + extra_correlation,
            original_scan.pre_kir_elimination_count(),
            original_scan.correlation_count() + extra_correlation,
            original_scan.state(),
        )
        .unwrap();
        let unused_catalog_record_ordinal = (0..original_scan.catalog_record_count())
            .find(|candidate| {
                exact.targets().iter().all(|target| {
                    target
                        .correlations()
                        .iter()
                        .all(|correlation| correlation.catalog_record_ordinal() != *candidate)
                }) && exact
                    .pre_kir_eliminations()
                    .iter()
                    .all(|fact| fact.catalog_record_ordinal() != *candidate)
            })
            .expect("hostile fixture retains one unclassified catalog record");

        let mut mutated_correlation = false;
        let targets = exact
            .targets()
            .iter()
            .map(|target| {
                let target_kir = if matches!(mutation, HostileMutation::TargetKirCoordinate) {
                    increment_kir(target.target_kir())
                } else {
                    target.target_kir()
                };
                let kind = if matches!(mutation, HostileMutation::KindAndMemoryForm) {
                    SourceIsaCharacteristicKindV1::WorkgroupLoad {
                        form: SourceIsaCharacteristicMemoryFormV1::MatrixTile,
                    }
                } else {
                    target.kind()
                };
                let mut selected_original = None;
                let mut correlations: Vec<_> = target
                    .correlations()
                    .iter()
                    .map(|correlation| {
                        let selected = !mutated_correlation
                            && correlation.kind()
                                == SourceIsaCharacteristicRecordKindV1::SourceAnchored;
                        if selected {
                            mutated_correlation = true;
                            selected_original = Some(correlation);
                        }
                        let catalog_record_ordinal = if selected
                            && matches!(mutation, HostileMutation::CatalogRecordOrdinal)
                        {
                            unused_catalog_record_ordinal
                        } else {
                            correlation.catalog_record_ordinal()
                        };
                        rebuilt_correlation(
                            correlation,
                            catalog_record_ordinal,
                            target_kir,
                            mutation,
                            selected,
                        )
                    })
                    .collect();
                if matches!(mutation, HostileMutation::CorrelationMultiplicity) {
                    if let Some(original) = selected_original {
                        correlations.push(rebuilt_correlation(
                            original,
                            unused_catalog_record_ordinal,
                            target_kir,
                            mutation,
                            false,
                        ));
                    }
                }
                SourceIsaCharacteristicTargetV1::new(kind, target_kir, correlations).unwrap()
            })
            .collect();
        let pre_kir = exact
            .pre_kir_eliminations()
            .iter()
            .map(|fact| {
                let source = if matches!(mutation, HostileMutation::PreKir) {
                    SourceIsaCharacteristicSourceCoordinateV1::new(
                        changed_id(fact.source().node_identity()),
                        fact.source().span(),
                    )
                    .unwrap()
                } else {
                    fact.source()
                };
                SourceIsaCharacteristicPreKirEliminationV1::new(
                    fact.catalog_record_ordinal(),
                    source,
                    fact.mir_node_identity(),
                    fact.mir(),
                )
                .unwrap()
            })
            .collect();
        SourceIsaCharacteristicCollectionV1::new(binding, scan, targets, pre_kir).unwrap()
    }

    fn rebuilt_binding(
        original: SourceIsaCharacteristicBindingV1,
        mutation: HostileMutation,
    ) -> SourceIsaCharacteristicBindingV1 {
        let mut counts = original.structural_counts();
        if matches!(mutation, HostileMutation::StructuralCounts) {
            counts.functions += 1;
        }
        SourceIsaCharacteristicBindingV1::new(
            if matches!(mutation, HostileMutation::TargetProfile) {
                SourceIsaCharacteristicTargetProfileV1::Gfx950
            } else {
                original.target_profile()
            },
            if matches!(mutation, HostileMutation::KirVersion) {
                SourceIsaCharacteristicKirVersionV1::V9
            } else {
                original.kir_version()
            },
            if matches!(mutation, HostileMutation::StructuralIdentity) {
                changed_id(original.structural_identity())
            } else {
                original.structural_identity()
            },
            counts,
            maybe_changed_content(
                original.source_map_v2(),
                mutation,
                HostileMutation::SourceMap,
            ),
            maybe_changed_content(
                original.neutral_kir(),
                mutation,
                HostileMutation::NeutralKirContent,
            ),
            maybe_changed_content(
                original.target_kir(),
                mutation,
                HostileMutation::TargetKirContent,
            ),
            maybe_changed_content(original.artifact(), mutation, HostileMutation::Artifact),
            maybe_changed_content(original.catalog(), mutation, HostileMutation::Catalog),
            maybe_changed_content(
                original.structural_bridge(),
                mutation,
                HostileMutation::StructuralBridge,
            ),
            if matches!(mutation, HostileMutation::CorrelationIdentity) {
                changed_id(original.correlation_identity())
            } else {
                original.correlation_identity()
            },
            if matches!(mutation, HostileMutation::SemanticMapIdentity) {
                changed_id(original.semantic_map_identity())
            } else {
                original.semantic_map_identity()
            },
        )
        .unwrap()
    }

    fn maybe_changed_content(
        content: SourceIsaCharacteristicContentIdentityV1,
        mutation: HostileMutation,
        selected: HostileMutation,
    ) -> SourceIsaCharacteristicContentIdentityV1 {
        if mutation == selected
            || matches!(mutation, HostileMutation::ContentByteLength)
                && matches!(selected, HostileMutation::SourceMap)
        {
            SourceIsaCharacteristicContentIdentityV1::new(
                if mutation == selected {
                    changed_id(content.sha256())
                } else {
                    content.sha256()
                },
                content.byte_len()
                    + u64::from(matches!(mutation, HostileMutation::ContentByteLength)),
            )
            .unwrap()
        } else {
            content
        }
    }

    fn rebuilt_correlation(
        original: &SourceIsaCharacteristicTargetCorrelationV1,
        catalog_record_ordinal: u64,
        target_kir: SourceIsaCharacteristicKirCoordinateV1,
        mutation: HostileMutation,
        selected: bool,
    ) -> SourceIsaCharacteristicTargetCorrelationV1 {
        let mut kind = original.kind();
        let mut source = original.source();
        let mut mir_node = original.mir_node_identity();
        let mut mir = original.mir();
        let mut neutral_node = original.neutral_kir_node_identity();
        let mut neutral = original.neutral_kir();
        let mut semantic = original.semantic_operation_identity();
        let mut llvm = original.compiler_handoff_llvm();
        let mut transformation = original.transformation();
        let mut intervals = original.isa_intervals().to_vec();

        if selected {
            match mutation {
                HostileMutation::RecordKind => {
                    kind = SourceIsaCharacteristicRecordKindV1::NoSourceProvenance;
                    source = None;
                    mir_node = None;
                    mir = None;
                    neutral_node = None;
                    neutral = None;
                }
                HostileMutation::Source => {
                    let current = source.unwrap();
                    source = Some(
                        SourceIsaCharacteristicSourceCoordinateV1::new(
                            changed_id(current.node_identity()),
                            current.span(),
                        )
                        .unwrap(),
                    );
                }
                HostileMutation::MirNode => mir_node = mir_node.map(changed_id),
                HostileMutation::MirCoordinate => {
                    let current = mir.unwrap();
                    mir = Some(
                        SourceIsaCharacteristicMirCoordinateV1::new(
                            current.body_ordinal(),
                            current.block_ordinal(),
                            current.statement_ordinal() + 1,
                        )
                        .unwrap(),
                    );
                }
                HostileMutation::NeutralKirNode => neutral_node = neutral_node.map(changed_id),
                HostileMutation::NeutralKirCoordinate => {
                    neutral = neutral.map(increment_kir);
                }
                HostileMutation::SemanticOperation => semantic = changed_id(semantic),
                HostileMutation::CompilerHandoffLlvm => {
                    llvm = SourceIsaCharacteristicCompilerHandoffLlvmCoordinateV1::new(
                        llvm.function_ordinal(),
                        llvm.block_ordinal(),
                        llvm.instruction_ordinal() + 1,
                    )
                    .unwrap();
                }
                HostileMutation::Transformation => {
                    transformation = SourceIsaCharacteristicTransformationV1::Coalesced;
                }
                HostileMutation::IsaInterval => {
                    let interval = intervals[0];
                    intervals[0] = SourceIsaCharacteristicIsaIntervalV1::new(
                        interval.kernel_ordinal(),
                        interval.symbol_relative_start() + 4,
                        interval.symbol_relative_end() + 4,
                    )
                    .unwrap();
                }
                HostileMutation::IsaMultiplicity => intervals.push(intervals[0]),
                HostileMutation::KindAndMemoryForm
                | HostileMutation::CatalogRecordOrdinal
                | HostileMutation::CorrelationMultiplicity
                | HostileMutation::TargetKirCoordinate
                | HostileMutation::PreKir
                | HostileMutation::ScanCount
                | HostileMutation::TargetProfile
                | HostileMutation::KirVersion
                | HostileMutation::StructuralIdentity
                | HostileMutation::StructuralCounts
                | HostileMutation::SourceMap
                | HostileMutation::ContentByteLength
                | HostileMutation::NeutralKirContent
                | HostileMutation::TargetKirContent
                | HostileMutation::Artifact
                | HostileMutation::Catalog
                | HostileMutation::StructuralBridge
                | HostileMutation::CorrelationIdentity
                | HostileMutation::SemanticMapIdentity => {}
            }
        }
        SourceIsaCharacteristicTargetCorrelationV1::new(
            catalog_record_ordinal,
            kind,
            source,
            mir_node,
            mir,
            neutral_node,
            neutral,
            target_kir,
            semantic,
            llvm,
            intervals,
            transformation,
        )
        .unwrap()
    }

    fn increment_kir(
        coordinate: SourceIsaCharacteristicKirCoordinateV1,
    ) -> SourceIsaCharacteristicKirCoordinateV1 {
        SourceIsaCharacteristicKirCoordinateV1::new(
            coordinate.function_ordinal(),
            coordinate.block_ordinal(),
            coordinate.operation_ordinal() + 1,
        )
        .unwrap()
    }

    fn changed_id(mut identity: [u8; 32]) -> [u8; 32] {
        identity[0] ^= 0x80;
        identity
    }
}

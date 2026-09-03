use std::collections::{BTreeMap, BTreeSet};
use std::io::{self, Read};

use sha2::{Digest, Sha256};

const BROKER_MAGIC_V3: &[u8] = b"FE2O3/SOURCE-ISA-CHARACTERISTIC-BROKER/V3\0";
const BROKER_CONFIG_DOMAIN_V3: &[u8] = b"FE2O3/SOURCE-ISA-CHARACTERISTIC-BROKER-CONFIG/V3\0";
const BROKER_BODY_FORMAT_V3: &[u8] = b"fe2o3-source-isa-characteristic-collection-v1";
const MAX_BROKER_BODY_BYTES_V3: usize = 128 * 1024 * 1024;
const MAX_FACT_INTERVAL_PAGE_ITEMS_V2: u16 = 64;

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
enum TargetV2 {
    Gfx942,
    Gfx950,
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
enum KernelFamilyV2 {
    ElementwiseFill,
    NeutralWorkgroupReduction,
    TiledBf16Gemm,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct UnitContractV2 {
    family: KernelFamilyV2,
    crate_name: &'static str,
    source: &'static str,
    cargo_arguments: &'static [&'static str],
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
enum MemoryFormV2 {
    Plain,
    Guarded,
    MatrixTile,
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
enum CharacteristicKindV2 {
    GlobalStore { form: MemoryFormV2 },
    WorkgroupLdsRead { form: MemoryFormV2 },
    WorkgroupLdsWrite { form: MemoryFormV2 },
    WorkgroupBarrier,
    Bf16F32M16N16K16Wave64MatrixMultiplyAccumulate,
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
enum CharacteristicFamilyV2 {
    GlobalStore,
    WorkgroupLdsRead,
    WorkgroupLdsWrite,
    WorkgroupBarrier,
    Bf16MfmaExact,
}

impl CharacteristicKindV2 {
    const fn family(self) -> CharacteristicFamilyV2 {
        match self {
            Self::GlobalStore { .. } => CharacteristicFamilyV2::GlobalStore,
            Self::WorkgroupLdsRead { .. } => CharacteristicFamilyV2::WorkgroupLdsRead,
            Self::WorkgroupLdsWrite { .. } => CharacteristicFamilyV2::WorkgroupLdsWrite,
            Self::WorkgroupBarrier => CharacteristicFamilyV2::WorkgroupBarrier,
            Self::Bf16F32M16N16K16Wave64MatrixMultiplyAccumulate => {
                CharacteristicFamilyV2::Bf16MfmaExact
            }
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
enum TransformationV2 {
    Preserved,
    Duplicated,
    Coalesced,
    DuplicatedAndCoalesced,
    Eliminated,
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
enum KirVersionV2 {
    V8,
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
enum CatalogRecordKindV2 {
    SourceAnchored,
    NoSourceProvenance,
    EliminatedBeforeKir,
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
enum HostileSubstitutionV2 {
    Category,
    RecordKind,
    SourceCoordinate,
    MirCoordinate,
    NeutralKirCoordinate,
    TargetKirCoordinate,
    SemanticOperation,
    CompilerHandoffLlvmCoordinate,
    Transformation,
    IsaInterval,
    IsaIntervalMultiplicity,
    PreKirElimination,
    CompleteScan,
    StructuralBridgeIdentity,
    CatalogIdentity,
    ArtifactIdentity,
    ProducerTarget,
    BrokerConfigIdentity,
    BrokerUnitIdentity,
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
struct ContentIdentityV2 {
    sha256: [u8; 32],
    byte_len: u64,
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
struct SourceSpanV2 {
    file: [u8; 32],
    byte_start: u64,
    byte_end: u64,
    line: u32,
    column: u32,
}

impl SourceSpanV2 {
    fn is_valid(self, allow_empty: bool) -> bool {
        self.file != [0; 32]
            && self.byte_start <= self.byte_end
            && (allow_empty || self.byte_start < self.byte_end)
            && self.line != 0
            && self.column != 0
    }
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
struct SourceCoordinateV2 {
    node: [u8; 32],
    span: SourceSpanV2,
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
struct MirCoordinateV2 {
    body: u64,
    block: u64,
    statement: u64,
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
struct KirCoordinateV2 {
    function: u64,
    block: u64,
    operation: u64,
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
struct LlvmCoordinateV2 {
    function: u64,
    block: u64,
    instruction: u64,
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
struct IsaIntervalV2 {
    kernel: u64,
    pc_start: u64,
    pc_end: u64,
}

impl IsaIntervalV2 {
    fn is_valid(self) -> bool {
        self.kernel == 0
            && self.pc_start.is_multiple_of(4)
            && self.pc_start.checked_add(4) == Some(self.pc_end)
    }

    fn contains(self, kernel: u64, pc: u64) -> bool {
        self.kernel == kernel && self.pc_start <= pc && pc < self.pc_end
    }
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
enum TargetCatalogFactV2 {
    SourceAnchored {
        catalog_record_ordinal: u64,
        source: SourceCoordinateV2,
        mir_node: [u8; 32],
        mir: MirCoordinateV2,
        neutral_kir_node: [u8; 32],
        neutral_kir: KirCoordinateV2,
        target_kir: KirCoordinateV2,
        semantic_operation: [u8; 32],
        compiler_handoff_llvm: LlvmCoordinateV2,
        transformation: TransformationV2,
        isa: Vec<IsaIntervalV2>,
    },
    NoSourceProvenance {
        catalog_record_ordinal: u64,
        target_kir: KirCoordinateV2,
        semantic_operation: [u8; 32],
        compiler_handoff_llvm: LlvmCoordinateV2,
        transformation: TransformationV2,
        isa: Vec<IsaIntervalV2>,
    },
}

impl TargetCatalogFactV2 {
    const fn catalog_record_ordinal(&self) -> u64 {
        match self {
            Self::SourceAnchored {
                catalog_record_ordinal,
                ..
            }
            | Self::NoSourceProvenance {
                catalog_record_ordinal,
                ..
            } => *catalog_record_ordinal,
        }
    }

    fn with_catalog_record_ordinal(mut self, ordinal: u64) -> Self {
        match &mut self {
            Self::SourceAnchored {
                catalog_record_ordinal,
                ..
            }
            | Self::NoSourceProvenance {
                catalog_record_ordinal,
                ..
            } => *catalog_record_ordinal = ordinal,
        }
        self
    }

    const fn record_kind(&self) -> CatalogRecordKindV2 {
        match self {
            Self::SourceAnchored { .. } => CatalogRecordKindV2::SourceAnchored,
            Self::NoSourceProvenance { .. } => CatalogRecordKindV2::NoSourceProvenance,
        }
    }

    const fn source(&self) -> Option<SourceCoordinateV2> {
        match self {
            Self::SourceAnchored { source, .. } => Some(*source),
            Self::NoSourceProvenance { .. } => None,
        }
    }

    const fn target_kir(&self) -> KirCoordinateV2 {
        match self {
            Self::SourceAnchored { target_kir, .. }
            | Self::NoSourceProvenance { target_kir, .. } => *target_kir,
        }
    }

    const fn compiler_handoff_llvm(&self) -> LlvmCoordinateV2 {
        match self {
            Self::SourceAnchored {
                compiler_handoff_llvm,
                ..
            }
            | Self::NoSourceProvenance {
                compiler_handoff_llvm,
                ..
            } => *compiler_handoff_llvm,
        }
    }

    const fn transformation(&self) -> TransformationV2 {
        match self {
            Self::SourceAnchored { transformation, .. }
            | Self::NoSourceProvenance { transformation, .. } => *transformation,
        }
    }

    const fn mir(&self) -> Option<([u8; 32], MirCoordinateV2)> {
        match self {
            Self::SourceAnchored { mir_node, mir, .. } => Some((*mir_node, *mir)),
            Self::NoSourceProvenance { .. } => None,
        }
    }

    const fn neutral_kir(&self) -> Option<([u8; 32], KirCoordinateV2)> {
        match self {
            Self::SourceAnchored {
                neutral_kir_node,
                neutral_kir,
                ..
            } => Some((*neutral_kir_node, *neutral_kir)),
            Self::NoSourceProvenance { .. } => None,
        }
    }

    const fn semantic_operation(&self) -> [u8; 32] {
        match self {
            Self::SourceAnchored {
                semantic_operation, ..
            }
            | Self::NoSourceProvenance {
                semantic_operation, ..
            } => *semantic_operation,
        }
    }

    fn isa(&self) -> &[IsaIntervalV2] {
        match self {
            Self::SourceAnchored { isa, .. } | Self::NoSourceProvenance { isa, .. } => isa,
        }
    }

    fn is_valid_for(&self, parent: KirCoordinateV2) -> bool {
        let llvm = self.compiler_handoff_llvm();
        let common = self.target_kir() == parent
            && coordinate_is_bounded(parent.function, parent.block, parent.operation)
            && coordinate_is_bounded(llvm.function, llvm.block, llvm.instruction)
            && (self.transformation() == TransformationV2::Eliminated) == self.isa().is_empty()
            && self.isa().iter().all(|interval| interval.is_valid())
            && self.isa().windows(2).all(|pair| pair[0] <= pair[1]);
        if !common {
            return false;
        }
        match self {
            Self::SourceAnchored {
                source,
                mir_node,
                mir,
                neutral_kir_node,
                neutral_kir,
                semantic_operation,
                ..
            } => {
                source.node != [0; 32]
                    && source.span.is_valid(false)
                    && *mir_node != [0; 32]
                    && coordinate_is_bounded(mir.body, mir.block, mir.statement)
                    && *neutral_kir_node != [0; 32]
                    && coordinate_is_bounded(
                        neutral_kir.function,
                        neutral_kir.block,
                        neutral_kir.operation,
                    )
                    && *semantic_operation != [0; 32]
            }
            Self::NoSourceProvenance {
                semantic_operation, ..
            } => *semantic_operation != [0; 32],
        }
    }
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
struct PreKirEliminationV2 {
    catalog_record_ordinal: u64,
    source: SourceCoordinateV2,
    mir_node: [u8; 32],
    mir: MirCoordinateV2,
}

impl PreKirEliminationV2 {
    fn is_valid(&self) -> bool {
        self.source.node != [0; 32]
            && self.source.span.is_valid(true)
            && self.mir_node != [0; 32]
            && coordinate_is_bounded(self.mir.body, self.mir.block, self.mir.statement)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct TargetCharacteristicV2 {
    target_kir: KirCoordinateV2,
    kind: CharacteristicKindV2,
    correlations: Vec<TargetCatalogFactV2>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct StructuralCountsV2 {
    functions: u64,
    defined_bodies: u64,
    blocks: u64,
    operations: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct ProducerBindingV2 {
    target: TargetV2,
    kir_version: KirVersionV2,
    neutral_kir: ContentIdentityV2,
    target_kir: ContentIdentityV2,
    source_map_v2: ContentIdentityV2,
    artifact: ContentIdentityV2,
    structural_counts: StructuralCountsV2,
    structural_binding: [u8; 32],
    structural_bridge: ContentIdentityV2,
    catalog: ContentIdentityV2,
    correlation: [u8; 32],
    semantic_map: [u8; 32],
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct CompleteScanV2 {
    target_operations_scanned: u64,
    catalog_records_scanned: u64,
    catalog_record_count: u64,
    characteristic_operations: u64,
    characteristic_correlations: u64,
    pre_kir_eliminations: u64,
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
enum FactOccurrenceV2 {
    Target {
        characteristic_ordinal: u32,
        correlation_ordinal: u32,
        catalog_record_ordinal: u64,
    },
    PreKir {
        elimination_ordinal: u32,
        catalog_record_ordinal: u64,
    },
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
struct TargetCharacteristicOccurrenceV2 {
    characteristic_ordinal: u32,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct QueryResultsV2 {
    structural_pages_complete: bool,
    all_pages_complete: bool,
    interval_pages_complete: bool,
    terminal_cursors_absent: bool,
    structural: BTreeSet<TargetCharacteristicOccurrenceV2>,
    structural_category: BTreeMap<CharacteristicKindV2, BTreeSet<TargetCharacteristicOccurrenceV2>>,
    structural_target_kir: BTreeMap<KirCoordinateV2, BTreeSet<TargetCharacteristicOccurrenceV2>>,
    all: BTreeSet<FactOccurrenceV2>,
    category: BTreeMap<CharacteristicKindV2, BTreeSet<FactOccurrenceV2>>,
    record_kind: BTreeMap<CatalogRecordKindV2, BTreeSet<FactOccurrenceV2>>,
    source_node: BTreeMap<[u8; 32], BTreeSet<FactOccurrenceV2>>,
    source_span: BTreeMap<SourceSpanV2, BTreeSet<FactOccurrenceV2>>,
    mir_node: BTreeMap<[u8; 32], BTreeSet<FactOccurrenceV2>>,
    mir: BTreeMap<MirCoordinateV2, BTreeSet<FactOccurrenceV2>>,
    neutral_kir_node: BTreeMap<[u8; 32], BTreeSet<FactOccurrenceV2>>,
    neutral_kir: BTreeMap<KirCoordinateV2, BTreeSet<FactOccurrenceV2>>,
    target_kir: BTreeMap<KirCoordinateV2, BTreeSet<FactOccurrenceV2>>,
    semantic_operation: BTreeMap<[u8; 32], BTreeSet<FactOccurrenceV2>>,
    llvm: BTreeMap<LlvmCoordinateV2, BTreeSet<FactOccurrenceV2>>,
    transformation: BTreeMap<TransformationV2, BTreeSet<FactOccurrenceV2>>,
    exact_pc: BTreeMap<(u64, u64), BTreeSet<FactOccurrenceV2>>,
    intervals: BTreeMap<FactOccurrenceV2, Vec<(u32, IsaIntervalV2)>>,
    pre_kir: BTreeSet<FactOccurrenceV2>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct AdmittedCharacteristicCellV2 {
    family: KernelFamilyV2,
    producer_binding: ProducerBindingV2,
    scan: CompleteScanV2,
    characteristics: Vec<TargetCharacteristicV2>,
    pre_kir_eliminations: Vec<PreKirEliminationV2>,
    queries: QueryResultsV2,
}

impl AdmittedCharacteristicCellV2 {
    const fn grants_compiler_authority(&self) -> bool {
        false
    }

    const fn grants_artifact_admission_authority(&self) -> bool {
        false
    }

    const fn grants_publication_authority(&self) -> bool {
        false
    }

    const fn grants_debugger_control_authority(&self) -> bool {
        false
    }

    const fn grants_profiler_capture_authority(&self) -> bool {
        false
    }

    const fn grants_runtime_authority(&self) -> bool {
        false
    }

    const fn grants_hardware_observation_authority(&self) -> bool {
        false
    }

    const fn proves_optimized_or_final_llvm_custody(&self) -> bool {
        false
    }

    const fn proves_llvm_instruction_semantics(&self) -> bool {
        false
    }

    const fn proves_isa_opcode_semantics(&self) -> bool {
        false
    }

    const fn proves_a_schedule(&self) -> bool {
        false
    }

    const fn proves_complete_machine_instruction_coverage(&self) -> bool {
        false
    }

    const fn proves_live_program_counter_ownership(&self) -> bool {
        false
    }

    const fn proves_gpu_execution_or_performance(&self) -> bool {
        false
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct BrokerCellBindingV3 {
    config: [u8; 32],
    unit: [u8; 32],
    target: TargetV2,
}

#[derive(Debug, Eq, PartialEq)]
struct BrokerBodyV3 {
    binding: BrokerCellBindingV3,
    body: Vec<u8>,
}

trait ProtectedCharacteristicAdapterV2 {
    type Error: std::fmt::Debug;

    fn capture(
        &mut self,
        family: KernelFamilyV2,
        target: TargetV2,
    ) -> Result<
        (
            BrokerCellBindingV3,
            ProducerBindingV2,
            AdmittedCharacteristicCellV2,
        ),
        Self::Error,
    >;

    fn reseal_and_readmit(
        &mut self,
        admitted: &AdmittedCharacteristicCellV2,
        substitution: HostileSubstitutionV2,
    ) -> Result<AdmittedCharacteristicCellV2, Self::Error>;
}

fn assert_protected_characteristic_matrix_v2<A: ProtectedCharacteristicAdapterV2>(adapter: &mut A) {
    let mut cells = BTreeMap::new();
    for family in families_v2() {
        for target in [TargetV2::Gfx942, TargetV2::Gfx950] {
            let (broker, expected, cell) = adapter
                .capture(family, target)
                .unwrap_or_else(|error| panic!("{family:?} {target:?} capture failed: {error:?}"));
            assert_eq!(broker.target, target);
            validate_characteristic_cell_v2(&cell, family, expected)
                .unwrap_or_else(|error| panic!("{family:?} {target:?} admission changed: {error}"));
            assert_exact_bidirectional_equality_v2(&cell);
            for substitution in hostile_substitutions_v2() {
                assert!(
                    adapter.reseal_and_readmit(&cell, substitution).is_err(),
                    "{family:?} {target:?} accepted resealed {substitution:?} substitution",
                );
            }
            assert!(cells.insert((family, target), (broker, cell)).is_none());
        }
    }
    assert_eq!(cells.len(), 6);
    for family in families_v2() {
        let (gfx942_broker, gfx942) = &cells[&(family, TargetV2::Gfx942)];
        let (gfx950_broker, gfx950) = &cells[&(family, TargetV2::Gfx950)];
        assert_eq!(gfx942_broker.config, gfx950_broker.config);
        assert_eq!(gfx942_broker.unit, gfx950_broker.unit);
        assert_eq!(
            gfx942.producer_binding.neutral_kir,
            gfx950.producer_binding.neutral_kir
        );
        assert_eq!(
            gfx942.producer_binding.source_map_v2,
            gfx950.producer_binding.source_map_v2
        );
        assert_ne!(
            gfx942.producer_binding.target_kir,
            gfx950.producer_binding.target_kir
        );
        assert_ne!(
            gfx942.producer_binding.artifact,
            gfx950.producer_binding.artifact
        );
        assert_ne!(
            gfx942.producer_binding.structural_binding,
            gfx950.producer_binding.structural_binding
        );
        assert_ne!(
            gfx942.producer_binding.structural_bridge,
            gfx950.producer_binding.structural_bridge
        );
        assert_ne!(
            gfx942.producer_binding.catalog,
            gfx950.producer_binding.catalog
        );
        assert_ne!(
            gfx942.producer_binding.correlation,
            gfx950.producer_binding.correlation
        );
        assert_ne!(
            gfx942.producer_binding.semantic_map,
            gfx950.producer_binding.semantic_map
        );
    }
}

fn validate_characteristic_cell_v2(
    cell: &AdmittedCharacteristicCellV2,
    expected_family: KernelFamilyV2,
    expected: ProducerBindingV2,
) -> Result<(), &'static str> {
    if cell.family != expected_family {
        return Err("kernel-family substitution");
    }
    if cell.producer_binding != expected || !producer_binding_is_valid(expected) {
        return Err("producer-evidence binding substitution");
    }
    validate_complete_projection_shape_v2(cell)?;
    let actual_families = cell
        .characteristics
        .iter()
        .map(|characteristic| characteristic.kind.family())
        .collect::<BTreeSet<_>>();
    if actual_families != required_categories_v2(cell.family) {
        return Err("target-KIR characteristic set differs from family contract");
    }
    Ok(())
}

fn validate_complete_projection_shape_v2(
    cell: &AdmittedCharacteristicCellV2,
) -> Result<(), &'static str> {
    if !producer_binding_is_valid(cell.producer_binding) {
        return Err("invalid producer-evidence binding");
    }
    let characteristic_count = u64::try_from(cell.characteristics.len())
        .map_err(|_| "characteristic count exceeds u64")?;
    let correlation_count =
        cell.characteristics
            .iter()
            .try_fold(0_u64, |count, characteristic| {
                let retained = u64::try_from(characteristic.correlations.len())
                    .map_err(|_| "correlation count exceeds u64")?;
                count
                    .checked_add(retained)
                    .ok_or("correlation count overflow")
            })?;
    let pre_kir_count =
        u64::try_from(cell.pre_kir_eliminations.len()).map_err(|_| "pre-KIR count exceeds u64")?;
    let retained_record_count = correlation_count
        .checked_add(pre_kir_count)
        .ok_or("retained catalog record count overflow")?;
    if cell.scan.target_operations_scanned != cell.producer_binding.structural_counts.operations
        || cell.scan.catalog_records_scanned != cell.scan.catalog_record_count
        || cell.scan.characteristic_operations != characteristic_count
        || cell.scan.characteristic_correlations != correlation_count
        || cell.scan.pre_kir_eliminations != pre_kir_count
        || cell.scan.target_operations_scanned < characteristic_count
        || cell.scan.catalog_records_scanned < retained_record_count
    {
        return Err("complete scan counts differ from retained facts");
    }
    if cell
        .characteristics
        .windows(2)
        .any(|pair| pair[0].target_kir >= pair[1].target_kir)
    {
        return Err("target characteristic coordinates are not canonical and unique");
    }
    let mut retained_catalog_ordinals = BTreeSet::new();
    for characteristic in &cell.characteristics {
        if !coordinate_is_bounded(
            characteristic.target_kir.function,
            characteristic.target_kir.block,
            characteristic.target_kir.operation,
        ) || characteristic
            .correlations
            .iter()
            .any(|fact| !fact.is_valid_for(characteristic.target_kir))
            || characteristic
                .correlations
                .windows(2)
                .any(|pair| pair[0] > pair[1])
            || characteristic
                .correlations
                .windows(2)
                .any(|pair| pair[0].catalog_record_ordinal() >= pair[1].catalog_record_ordinal())
            || characteristic.correlations.iter().any(|fact| {
                fact.catalog_record_ordinal() >= cell.scan.catalog_record_count
                    || !retained_catalog_ordinals.insert(fact.catalog_record_ordinal())
            })
        {
            return Err("invalid or noncanonical target characteristic correlation");
        }
    }
    if cell
        .pre_kir_eliminations
        .iter()
        .any(|elimination| !elimination.is_valid())
        || cell
            .pre_kir_eliminations
            .windows(2)
            .any(|pair| pair[0] > pair[1])
        || cell
            .pre_kir_eliminations
            .windows(2)
            .any(|pair| pair[0].catalog_record_ordinal >= pair[1].catalog_record_ordinal)
        || cell.pre_kir_eliminations.iter().any(|elimination| {
            elimination.catalog_record_ordinal >= cell.scan.catalog_record_count
                || !retained_catalog_ordinals.insert(elimination.catalog_record_ordinal)
        })
    {
        return Err("invalid or noncanonical pre-KIR elimination");
    }
    if cell.grants_compiler_authority()
        || cell.grants_artifact_admission_authority()
        || cell.grants_publication_authority()
        || cell.grants_debugger_control_authority()
        || cell.grants_profiler_capture_authority()
        || cell.grants_runtime_authority()
        || cell.grants_hardware_observation_authority()
        || cell.proves_optimized_or_final_llvm_custody()
        || cell.proves_llvm_instruction_semantics()
        || cell.proves_isa_opcode_semantics()
        || cell.proves_a_schedule()
        || cell.proves_complete_machine_instruction_coverage()
        || cell.proves_live_program_counter_ownership()
        || cell.proves_gpu_execution_or_performance()
    {
        return Err("coordinate observation acquired forbidden authority");
    }
    Ok(())
}

fn assert_exact_bidirectional_equality_v2(cell: &AdmittedCharacteristicCellV2) {
    assert!(cell.queries.structural_pages_complete);
    assert!(cell.queries.all_pages_complete);
    assert!(cell.queries.interval_pages_complete);
    assert!(cell.queries.terminal_cursors_absent);
    assert_eq!(
        cell.queries,
        reference_query_results_v2(&cell.characteristics, &cell.pre_kir_eliminations),
        "typed query pages differ from the complete occurrence population",
    );
}

fn producer_binding_is_valid(binding: ProducerBindingV2) -> bool {
    [
        binding.neutral_kir,
        binding.target_kir,
        binding.source_map_v2,
        binding.artifact,
        binding.structural_bridge,
        binding.catalog,
    ]
    .into_iter()
    .all(|identity| identity.sha256 != [0; 32] && identity.byte_len != 0)
        && [
            binding.structural_binding,
            binding.correlation,
            binding.semantic_map,
        ]
        .into_iter()
        .all(|identity| identity != [0; 32])
        && binding.structural_counts.defined_bodies <= binding.structural_counts.functions
        && binding.structural_counts.blocks >= binding.structural_counts.defined_bodies
}

fn coordinate_is_bounded(first: u64, second: u64, third: u64) -> bool {
    [first, second, third]
        .into_iter()
        .all(|ordinal| ordinal <= u64::from(u32::MAX))
}

fn families_v2() -> [KernelFamilyV2; 3] {
    [
        KernelFamilyV2::ElementwiseFill,
        KernelFamilyV2::NeutralWorkgroupReduction,
        KernelFamilyV2::TiledBf16Gemm,
    ]
}

fn unit_contracts_v2() -> [UnitContractV2; 3] {
    [
        UnitContractV2 {
            family: KernelFamilyV2::ElementwiseFill,
            crate_name: "fe2o3_production_extraction_fixture",
            source: "crates/rustc-codegen-fe2o3/tests/fixtures/production-extraction-device/src/lib.rs",
            cargo_arguments: &["-p", "fe2o3-production-extraction-fixture"],
        },
        UnitContractV2 {
            family: KernelFamilyV2::NeutralWorkgroupReduction,
            crate_name: "fe2o3_workgroup_sync_v1",
            source: "src/lib.rs",
            cargo_arguments: &["--no-default-features", "--features", "lds-kernel"],
        },
        UnitContractV2 {
            family: KernelFamilyV2::TiledBf16Gemm,
            crate_name: "fe2o3_collected_tiled_gemm_v1_fixture",
            source: "src/lib.rs",
            cargo_arguments: &[],
        },
    ]
}

fn required_categories_v2(family: KernelFamilyV2) -> BTreeSet<CharacteristicFamilyV2> {
    match family {
        KernelFamilyV2::ElementwiseFill => BTreeSet::from([CharacteristicFamilyV2::GlobalStore]),
        KernelFamilyV2::NeutralWorkgroupReduction => BTreeSet::from([
            CharacteristicFamilyV2::GlobalStore,
            CharacteristicFamilyV2::WorkgroupLdsRead,
            CharacteristicFamilyV2::WorkgroupLdsWrite,
            CharacteristicFamilyV2::WorkgroupBarrier,
        ]),
        KernelFamilyV2::TiledBf16Gemm => BTreeSet::from([
            CharacteristicFamilyV2::GlobalStore,
            CharacteristicFamilyV2::Bf16MfmaExact,
        ]),
    }
}

fn hostile_substitutions_v2() -> [HostileSubstitutionV2; 19] {
    [
        HostileSubstitutionV2::Category,
        HostileSubstitutionV2::RecordKind,
        HostileSubstitutionV2::SourceCoordinate,
        HostileSubstitutionV2::MirCoordinate,
        HostileSubstitutionV2::NeutralKirCoordinate,
        HostileSubstitutionV2::TargetKirCoordinate,
        HostileSubstitutionV2::SemanticOperation,
        HostileSubstitutionV2::CompilerHandoffLlvmCoordinate,
        HostileSubstitutionV2::Transformation,
        HostileSubstitutionV2::IsaInterval,
        HostileSubstitutionV2::IsaIntervalMultiplicity,
        HostileSubstitutionV2::PreKirElimination,
        HostileSubstitutionV2::CompleteScan,
        HostileSubstitutionV2::StructuralBridgeIdentity,
        HostileSubstitutionV2::CatalogIdentity,
        HostileSubstitutionV2::ArtifactIdentity,
        HostileSubstitutionV2::ProducerTarget,
        HostileSubstitutionV2::BrokerConfigIdentity,
        HostileSubstitutionV2::BrokerUnitIdentity,
    ]
}

fn broker_config_identity_v3() -> [u8; 32] {
    let mut digest = Sha256::new();
    for field in [
        BROKER_CONFIG_DOMAIN_V3,
        BROKER_MAGIC_V3,
        BROKER_BODY_FORMAT_V3,
        &u64::try_from(MAX_BROKER_BODY_BYTES_V3)
            .expect("Broker bound fits u64")
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

fn lower_hex_v2(bytes: &[u8]) -> String {
    const DIGITS: &[u8; 16] = b"0123456789abcdef";
    let mut encoded = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        encoded.push(DIGITS[(byte >> 4) as usize] as char);
        encoded.push(DIGITS[(byte & 0x0f) as usize] as char);
    }
    encoded
}

fn read_broker_v3(
    mut input: impl Read,
    expected: BrokerCellBindingV3,
) -> Result<BrokerBodyV3, String> {
    let mut magic = vec![0; BROKER_MAGIC_V3.len()];
    input
        .read_exact(&mut magic)
        .map_err(|error| format!("read Broker V3 magic: {error}"))?;
    if magic != BROKER_MAGIC_V3 {
        return Err("Broker V3 magic substitution".to_owned());
    }
    let mut config = [0; 32];
    input
        .read_exact(&mut config)
        .map_err(|error| format!("read Broker V3 config identity: {error}"))?;
    if config != broker_config_identity_v3() {
        return Err("Broker V3 config identity substitution".to_owned());
    }
    let mut production_config = [0; 32];
    input
        .read_exact(&mut production_config)
        .map_err(|error| format!("read Broker V3 production config identity: {error}"))?;
    if production_config != expected.config {
        return Err("Broker V3 production config identity substitution".to_owned());
    }
    let mut unit = [0; 32];
    input
        .read_exact(&mut unit)
        .map_err(|error| format!("read Broker V3 unit identity: {error}"))?;
    if unit != expected.unit {
        return Err("Broker V3 unit identity substitution".to_owned());
    }
    let mut target = [0; 2];
    input
        .read_exact(&mut target)
        .map_err(|error| format!("read Broker V3 target identity: {error}"))?;
    if target != target_code_v3(expected.target).to_le_bytes() {
        return Err("Broker V3 target identity substitution".to_owned());
    }
    let mut length = [0; 8];
    input
        .read_exact(&mut length)
        .map_err(|error| format!("read Broker V3 body length: {error}"))?;
    let length = usize::try_from(u64::from_le_bytes(length))
        .map_err(|_| "Broker V3 body length exceeds usize".to_owned())?;
    if length == 0 || length > MAX_BROKER_BODY_BYTES_V3 {
        return Err("Broker V3 body length is outside its hard bound".to_owned());
    }
    let mut body = Vec::new();
    body.try_reserve_exact(length)
        .map_err(|_| "cannot allocate bounded Broker V3 body".to_owned())?;
    body.resize(length, 0);
    input
        .read_exact(&mut body)
        .map_err(|error| format!("read Broker V3 body: {error}"))?;
    let mut trailing = [0];
    match input.read(&mut trailing) {
        Ok(0) => Ok(BrokerBodyV3 {
            binding: expected,
            body,
        }),
        Ok(_) => Err("Broker V3 input retained trailing bytes".to_owned()),
        Err(error) if error.kind() == io::ErrorKind::Interrupted => {
            Err("Broker V3 exact-EOF read was interrupted".to_owned())
        }
        Err(error) => Err(format!("read Broker V3 exact EOF: {error}")),
    }
}

fn encode_broker_v3(binding: BrokerCellBindingV3, body: &[u8]) -> Vec<u8> {
    let mut encoded = Vec::new();
    encoded.extend_from_slice(BROKER_MAGIC_V3);
    encoded.extend_from_slice(&broker_config_identity_v3());
    encoded.extend_from_slice(&binding.config);
    encoded.extend_from_slice(&binding.unit);
    encoded.extend_from_slice(&target_code_v3(binding.target).to_le_bytes());
    encoded.extend_from_slice(
        &u64::try_from(body.len())
            .expect("bounded Broker body length fits u64")
            .to_le_bytes(),
    );
    encoded.extend_from_slice(body);
    encoded
}

const fn target_code_v3(target: TargetV2) -> u16 {
    match target {
        TargetV2::Gfx942 => 1,
        TargetV2::Gfx950 => 2,
    }
}

fn reference_query_results_v2(
    characteristics: &[TargetCharacteristicV2],
    pre_kir_eliminations: &[PreKirEliminationV2],
) -> QueryResultsV2 {
    let mut results = QueryResultsV2 {
        structural_pages_complete: true,
        all_pages_complete: true,
        interval_pages_complete: true,
        terminal_cursors_absent: true,
        structural: BTreeSet::new(),
        structural_category: BTreeMap::new(),
        structural_target_kir: BTreeMap::new(),
        all: BTreeSet::new(),
        category: BTreeMap::new(),
        record_kind: BTreeMap::new(),
        source_node: BTreeMap::new(),
        source_span: BTreeMap::new(),
        mir_node: BTreeMap::new(),
        mir: BTreeMap::new(),
        neutral_kir_node: BTreeMap::new(),
        neutral_kir: BTreeMap::new(),
        target_kir: BTreeMap::new(),
        semantic_operation: BTreeMap::new(),
        llvm: BTreeMap::new(),
        transformation: BTreeMap::new(),
        exact_pc: BTreeMap::new(),
        intervals: BTreeMap::new(),
        pre_kir: BTreeSet::new(),
    };
    for (characteristic_ordinal, characteristic) in characteristics.iter().enumerate() {
        let structural_occurrence = TargetCharacteristicOccurrenceV2 {
            characteristic_ordinal: u32::try_from(characteristic_ordinal)
                .expect("reference characteristic ordinal is bounded"),
        };
        results.structural.insert(structural_occurrence);
        results
            .structural_category
            .entry(characteristic.kind)
            .or_default()
            .insert(structural_occurrence);
        results
            .structural_target_kir
            .entry(characteristic.target_kir)
            .or_default()
            .insert(structural_occurrence);
        for (correlation_ordinal, fact) in characteristic.correlations.iter().enumerate() {
            let occurrence = FactOccurrenceV2::Target {
                characteristic_ordinal: u32::try_from(characteristic_ordinal)
                    .expect("reference characteristic ordinal is bounded"),
                correlation_ordinal: u32::try_from(correlation_ordinal)
                    .expect("reference correlation ordinal is bounded"),
                catalog_record_ordinal: fact.catalog_record_ordinal(),
            };
            results.all.insert(occurrence);
            results
                .category
                .entry(characteristic.kind)
                .or_default()
                .insert(occurrence);
            results
                .record_kind
                .entry(fact.record_kind())
                .or_default()
                .insert(occurrence);
            if let Some(source) = fact.source() {
                results
                    .source_node
                    .entry(source.node)
                    .or_default()
                    .insert(occurrence);
                results
                    .source_span
                    .entry(source.span)
                    .or_default()
                    .insert(occurrence);
            }
            if let Some((node, coordinate)) = fact.mir() {
                results.mir_node.entry(node).or_default().insert(occurrence);
                results
                    .mir
                    .entry(coordinate)
                    .or_default()
                    .insert(occurrence);
            }
            if let Some((node, coordinate)) = fact.neutral_kir() {
                results
                    .neutral_kir_node
                    .entry(node)
                    .or_default()
                    .insert(occurrence);
                results
                    .neutral_kir
                    .entry(coordinate)
                    .or_default()
                    .insert(occurrence);
            }
            results
                .target_kir
                .entry(fact.target_kir())
                .or_default()
                .insert(occurrence);
            results
                .semantic_operation
                .entry(fact.semantic_operation())
                .or_default()
                .insert(occurrence);
            results
                .llvm
                .entry(fact.compiler_handoff_llvm())
                .or_default()
                .insert(occurrence);
            results
                .transformation
                .entry(fact.transformation())
                .or_default()
                .insert(occurrence);
            let intervals = fact
                .isa()
                .iter()
                .copied()
                .enumerate()
                .map(|(ordinal, interval)| {
                    (
                        u32::try_from(ordinal).expect("reference ISA interval ordinal is bounded"),
                        interval,
                    )
                })
                .collect::<Vec<_>>();
            for (_, interval) in &intervals {
                results
                    .exact_pc
                    .entry((interval.kernel, interval.pc_start))
                    .or_default()
                    .insert(occurrence);
            }
            if !intervals.is_empty() {
                results.intervals.insert(occurrence, intervals);
            }
        }
    }
    for (elimination_ordinal, elimination) in pre_kir_eliminations.iter().enumerate() {
        let occurrence = FactOccurrenceV2::PreKir {
            elimination_ordinal: u32::try_from(elimination_ordinal)
                .expect("reference pre-KIR elimination ordinal is bounded"),
            catalog_record_ordinal: elimination.catalog_record_ordinal,
        };
        results.all.insert(occurrence);
        results.pre_kir.insert(occurrence);
        results
            .record_kind
            .entry(CatalogRecordKindV2::EliminatedBeforeKir)
            .or_default()
            .insert(occurrence);
        results
            .source_node
            .entry(elimination.source.node)
            .or_default()
            .insert(occurrence);
        results
            .source_span
            .entry(elimination.source.span)
            .or_default()
            .insert(occurrence);
        results
            .mir_node
            .entry(elimination.mir_node)
            .or_default()
            .insert(occurrence);
        results
            .mir
            .entry(elimination.mir)
            .or_default()
            .insert(occurrence);
    }
    results
}

#[test]
fn characteristic_matrix_v2_contract_is_lossless_and_production_adapter_is_wired() {
    let source = include_str!("production_source_isa_characteristic_matrix_v2.rs");
    assert!(!source.contains("#[cfg(target_os = \"none\")]"));
    assert!(source.contains("mod production_adapter_v2"));
    assert!(source.contains("run_protected_build_stderr"));
    assert!(source.contains("source-isa-characteristic-v1"));
    assert!(source.contains("ordinary_source_units_preserve_characteristic_facts"));
    assert!(source.contains("admit_production_source_isa_characteristics_v1"));
    assert!(source.contains("ProductionSourceIsaCatalogV1"));
    assert!(source.contains("ProductionKirV7StructuralBridgeV1"));
    for forbidden in [
        ["v_mfma_f32_", "16x16x16_bf16"].concat(),
        ["llvm.amdgcn.mfma.f32.", "16x16x16bf16.1k"].concat(),
        ["llvm_", "module"].concat(),
        ["observation_", "collection"].concat(),
    ] {
        assert!(!source.contains(&forbidden));
    }
    assert_eq!(families_v2().len() * 2, 6);
    assert_eq!(unit_contracts_v2().len(), 3);
    assert_eq!(hostile_substitutions_v2().len(), 19);
    let capabilities = fe2o3_kernel_ir::production_semantic_debug_transformation_capabilities_v1();
    assert_eq!(capabilities.len(), 6);
    assert_eq!(
        capabilities
            .iter()
            .map(|capability| (capability.class(), capability.availability()))
            .collect::<Vec<_>>(),
        vec![
            (
                fe2o3_kernel_ir::ProductionSemanticDebugTransformationClassV1::Duplicated,
                fe2o3_kernel_ir::ProductionSemanticDebugTransformationAvailabilityV1::UnavailableNoProductionEmitter,
            ),
            (
                fe2o3_kernel_ir::ProductionSemanticDebugTransformationClassV1::Fused,
                fe2o3_kernel_ir::ProductionSemanticDebugTransformationAvailabilityV1::UnavailableNoProductionEmitter,
            ),
            (
                fe2o3_kernel_ir::ProductionSemanticDebugTransformationClassV1::Outlined,
                fe2o3_kernel_ir::ProductionSemanticDebugTransformationAvailabilityV1::UnavailableNoSchemaRepresentation,
            ),
            (
                fe2o3_kernel_ir::ProductionSemanticDebugTransformationClassV1::Inlined,
                fe2o3_kernel_ir::ProductionSemanticDebugTransformationAvailabilityV1::UnavailableNoProductionEmitter,
            ),
            (
                fe2o3_kernel_ir::ProductionSemanticDebugTransformationClassV1::Moved,
                fe2o3_kernel_ir::ProductionSemanticDebugTransformationAvailabilityV1::UnavailableNoProductionEmitter,
            ),
            (
                fe2o3_kernel_ir::ProductionSemanticDebugTransformationClassV1::Eliminated,
                fe2o3_kernel_ir::ProductionSemanticDebugTransformationAvailabilityV1::Representable,
            ),
        ]
    );
    let _unbound: Option<&dyn ProtectedCharacteristicAdapterV2<Error = String>> = None;
    let mut reference = ReferenceContractAdapterV2;
    assert_protected_characteristic_matrix_v2(&mut reference);
}

#[test]
fn broker_v3_is_bounded_length_prefixed_transport_bound_and_exact_eof() {
    assert_eq!(
        lower_hex_v2(&broker_config_identity_v3()),
        "d8cda5df0538ddd552b4b93bff3d8f1b9fefc379a0e941e271f0ca508e51ae74"
    );
    let body = b"canonical-characteristic-collection-v1";
    let binding = BrokerCellBindingV3 {
        config: [0x31; 32],
        unit: [0x32; 32],
        target: TargetV2::Gfx942,
    };
    let encoded = encode_broker_v3(binding, body);
    assert_eq!(
        read_broker_v3(encoded.as_slice(), binding).unwrap(),
        BrokerBodyV3 {
            binding,
            body: body.to_vec(),
        }
    );

    let mut wrong_magic = encoded.clone();
    wrong_magic[0] ^= 1;
    assert_eq!(
        read_broker_v3(wrong_magic.as_slice(), binding).unwrap_err(),
        "Broker V3 magic substitution"
    );
    let mut wrong_config = encoded.clone();
    wrong_config[BROKER_MAGIC_V3.len()] ^= 1;
    assert_eq!(
        read_broker_v3(wrong_config.as_slice(), binding).unwrap_err(),
        "Broker V3 config identity substitution"
    );
    let mut wrong_production_config = encoded.clone();
    wrong_production_config[BROKER_MAGIC_V3.len() + 32] ^= 1;
    assert_eq!(
        read_broker_v3(wrong_production_config.as_slice(), binding).unwrap_err(),
        "Broker V3 production config identity substitution"
    );
    let mut wrong_unit = encoded.clone();
    wrong_unit[BROKER_MAGIC_V3.len() + 64] ^= 1;
    assert_eq!(
        read_broker_v3(wrong_unit.as_slice(), binding).unwrap_err(),
        "Broker V3 unit identity substitution"
    );
    let mut wrong_target = encoded.clone();
    wrong_target[BROKER_MAGIC_V3.len() + 96] ^= 1;
    assert_eq!(
        read_broker_v3(wrong_target.as_slice(), binding).unwrap_err(),
        "Broker V3 target identity substitution"
    );
    let mut trailing = encoded.clone();
    trailing.push(0);
    assert_eq!(
        read_broker_v3(trailing.as_slice(), binding).unwrap_err(),
        "Broker V3 input retained trailing bytes"
    );
    assert!(read_broker_v3(&encoded[..encoded.len() - 1], binding).is_err());

    let mut oversized = Vec::new();
    oversized.extend_from_slice(BROKER_MAGIC_V3);
    oversized.extend_from_slice(&broker_config_identity_v3());
    oversized.extend_from_slice(&binding.config);
    oversized.extend_from_slice(&binding.unit);
    oversized.extend_from_slice(&target_code_v3(binding.target).to_le_bytes());
    oversized.extend_from_slice(
        &u64::try_from(MAX_BROKER_BODY_BYTES_V3)
            .expect("Broker bound fits u64")
            .checked_add(1)
            .expect("Broker hostile length fits u64")
            .to_le_bytes(),
    );
    assert_eq!(
        read_broker_v3(oversized.as_slice(), binding).unwrap_err(),
        "Broker V3 body length is outside its hard bound"
    );
}

#[test]
fn complete_zero_characteristic_scan_is_observed_without_missing_surrogates() {
    let producer_binding = reference_producer_binding_v2(0x60, 0x01, TargetV2::Gfx942, 4);
    let characteristics = Vec::new();
    let pre_kir_eliminations = Vec::new();
    let cell = AdmittedCharacteristicCellV2 {
        family: KernelFamilyV2::ElementwiseFill,
        producer_binding,
        scan: CompleteScanV2 {
            target_operations_scanned: 4,
            catalog_records_scanned: 2,
            catalog_record_count: 2,
            characteristic_operations: 0,
            characteristic_correlations: 0,
            pre_kir_eliminations: 0,
        },
        queries: reference_query_results_v2(&characteristics, &pre_kir_eliminations),
        characteristics,
        pre_kir_eliminations,
    };
    validate_complete_projection_shape_v2(&cell).unwrap();
    assert!(cell.queries.structural.is_empty());
    assert!(cell.queries.all.is_empty());
    assert!(cell.queries.structural_pages_complete);
    assert!(cell.queries.all_pages_complete);
    assert!(cell.queries.interval_pages_complete);
    assert!(cell.queries.terminal_cursors_absent);
    assert!(!cell.proves_complete_machine_instruction_coverage());
}

#[test]
fn structural_target_with_zero_catalog_correlations_is_queryable_without_a_fact() {
    let producer_binding = reference_producer_binding_v2(0x61, 0x01, TargetV2::Gfx942, 2);
    let target_kir = KirCoordinateV2 {
        function: 0,
        block: 0,
        operation: 0,
    };
    let kind = CharacteristicKindV2::GlobalStore {
        form: MemoryFormV2::Plain,
    };
    let characteristics = vec![TargetCharacteristicV2 {
        target_kir,
        kind,
        correlations: Vec::new(),
    }];
    let pre_kir_eliminations = Vec::new();
    let cell = AdmittedCharacteristicCellV2 {
        family: KernelFamilyV2::ElementwiseFill,
        producer_binding,
        scan: CompleteScanV2 {
            target_operations_scanned: 2,
            catalog_records_scanned: 0,
            catalog_record_count: 0,
            characteristic_operations: 1,
            characteristic_correlations: 0,
            pre_kir_eliminations: 0,
        },
        queries: reference_query_results_v2(&characteristics, &pre_kir_eliminations),
        characteristics,
        pre_kir_eliminations,
    };
    validate_characteristic_cell_v2(&cell, KernelFamilyV2::ElementwiseFill, producer_binding)
        .unwrap();
    assert_exact_bidirectional_equality_v2(&cell);

    let occurrence = TargetCharacteristicOccurrenceV2 {
        characteristic_ordinal: 0,
    };
    assert_eq!(cell.queries.structural, BTreeSet::from([occurrence]));
    assert_eq!(
        cell.queries.structural_category[&kind],
        BTreeSet::from([occurrence])
    );
    assert_eq!(
        cell.queries.structural_target_kir[&target_kir],
        BTreeSet::from([occurrence])
    );
    assert!(cell.queries.all.is_empty());
    assert!(cell.queries.record_kind.is_empty());
    assert!(cell.queries.llvm.is_empty());
    assert!(cell.queries.transformation.is_empty());
    assert!(cell.queries.intervals.is_empty());
}

#[test]
fn eliminated_optional_source_and_duplicate_multiplicity_are_exact() {
    let (_, _, cell) = ReferenceContractAdapterV2
        .capture(KernelFamilyV2::ElementwiseFill, TargetV2::Gfx942)
        .unwrap();
    let characteristic = &cell.characteristics[0];
    assert_ne!(
        characteristic.correlations[0],
        characteristic.correlations[1]
    );
    assert_eq!(
        characteristic.correlations[0].source(),
        characteristic.correlations[1].source()
    );
    assert_eq!(
        characteristic.correlations[0].target_kir(),
        characteristic.correlations[1].target_kir()
    );
    assert_eq!(
        characteristic.correlations[0].isa(),
        characteristic.correlations[1].isa()
    );
    let source = characteristic.correlations[0].source().unwrap();
    let (mir_node, mir) = characteristic.correlations[0].mir().unwrap();
    let (neutral_node, neutral) = characteristic.correlations[0].neutral_kir().unwrap();
    let semantic_operation = characteristic.correlations[0].semantic_operation();
    assert_eq!(cell.queries.source_node[&source.node].len(), 2);
    assert_eq!(cell.queries.mir_node[&mir_node].len(), 2);
    assert_eq!(cell.queries.mir[&mir].len(), 2);
    assert_eq!(cell.queries.neutral_kir_node[&neutral_node].len(), 2);
    assert_eq!(cell.queries.neutral_kir[&neutral].len(), 2);
    assert_eq!(
        cell.queries.semantic_operation[&semantic_operation].len(),
        2
    );
    assert_ne!(
        cell.queries.source_node[&source.node].iter().next(),
        cell.queries.source_node[&source.node].iter().nth(1)
    );
    let eliminated = characteristic
        .correlations
        .iter()
        .find(|fact| fact.transformation() == TransformationV2::Eliminated)
        .unwrap();
    assert_eq!(
        eliminated.record_kind(),
        CatalogRecordKindV2::NoSourceProvenance
    );
    assert_eq!(eliminated.source(), None);
    assert!(eliminated.isa().is_empty());
    assert!(
        cell.queries
            .llvm
            .contains_key(&eliminated.compiler_handoff_llvm())
    );
    assert!(cell.queries.exact_pc.values().all(|matches| {
        !matches.iter().any(|occurrence| {
            matches!(
                occurrence,
                FactOccurrenceV2::Target {
                    characteristic_ordinal: 0,
                    correlation_ordinal: 2,
                    ..
                }
            )
        })
    }));
    assert_eq!(cell.pre_kir_eliminations[0].source.span.byte_start, 200);
    assert_eq!(cell.pre_kir_eliminations[0].source.span.byte_end, 200);
    let interval = characteristic.correlations[0].isa()[0];
    assert!(interval.contains(interval.kernel, interval.pc_start));
    assert!(!interval.contains(interval.kernel, interval.pc_end));
    let first_occurrence = FactOccurrenceV2::Target {
        characteristic_ordinal: 0,
        correlation_ordinal: 0,
        catalog_record_ordinal: 1,
    };
    assert!(
        cell.queries.exact_pc[&(interval.kernel, interval.pc_start)].contains(&first_occurrence)
    );
    assert!(
        !cell
            .queries
            .exact_pc
            .contains_key(&(interval.kernel, interval.pc_start + 1))
    );
    assert_eq!(cell.queries.intervals[&first_occurrence].len(), 2);
    assert_eq!(
        cell.queries.intervals[&first_occurrence][0].1,
        cell.queries.intervals[&first_occurrence][1].1
    );
    assert_eq!(MAX_FACT_INTERVAL_PAGE_ITEMS_V2, 64);
    let _all_transforms = [
        TransformationV2::Preserved,
        TransformationV2::Duplicated,
        TransformationV2::Coalesced,
        TransformationV2::DuplicatedAndCoalesced,
        TransformationV2::Eliminated,
    ];
    let _all_forms = [
        MemoryFormV2::Plain,
        MemoryFormV2::Guarded,
        MemoryFormV2::MatrixTile,
    ];
}

struct ReferenceContractAdapterV2;

impl ProtectedCharacteristicAdapterV2 for ReferenceContractAdapterV2 {
    type Error = String;

    fn capture(
        &mut self,
        family: KernelFamilyV2,
        target: TargetV2,
    ) -> Result<
        (
            BrokerCellBindingV3,
            ProducerBindingV2,
            AdmittedCharacteristicCellV2,
        ),
        Self::Error,
    > {
        let family_byte = match family {
            KernelFamilyV2::ElementwiseFill => 0x10,
            KernelFamilyV2::NeutralWorkgroupReduction => 0x20,
            KernelFamilyV2::TiledBf16Gemm => 0x30,
        };
        let target_byte = match target {
            TargetV2::Gfx942 => 0x01,
            TargetV2::Gfx950 => 0x02,
        };
        let kinds = reference_kinds_v2(family);
        let kind_count = u64::try_from(kinds.len())
            .map_err(|_| "reference characteristic count exceeds u64".to_owned())?;
        let producer_binding = reference_producer_binding_v2(
            family_byte,
            target_byte,
            target,
            kind_count
                .checked_add(3)
                .ok_or_else(|| "reference target operation count overflow".to_owned())?,
        );
        let broker = BrokerCellBindingV3 {
            config: [family_byte + 3; 32],
            unit: [family_byte + 4; 32],
            target,
        };
        let mut characteristics = Vec::new();
        for (index, kind) in kinds.into_iter().enumerate() {
            characteristics.push(reference_characteristic_v2(
                family_byte,
                target_byte,
                u64::try_from(index)
                    .map_err(|_| "reference operation ordinal exceeds u64".to_owned())?,
                kind,
            ));
        }
        let pre_kir_eliminations = vec![PreKirEliminationV2 {
            catalog_record_ordinal: 0,
            source: SourceCoordinateV2 {
                node: [family_byte + 0x68; 32],
                span: SourceSpanV2 {
                    file: [family_byte + 0x69; 32],
                    byte_start: 200,
                    byte_end: 200,
                    line: 9,
                    column: 3,
                },
            },
            mir_node: [family_byte + 0x6a; 32],
            mir: MirCoordinateV2 {
                body: 0,
                block: 1,
                statement: 2,
            },
        }];
        let correlation_count =
            characteristics
                .iter()
                .try_fold(0_u64, |count, characteristic| {
                    let retained = u64::try_from(characteristic.correlations.len())
                        .map_err(|_| "reference correlation count exceeds u64".to_owned())?;
                    count
                        .checked_add(retained)
                        .ok_or_else(|| "reference correlation count overflow".to_owned())
                })?;
        let pre_kir_count = u64::try_from(pre_kir_eliminations.len())
            .map_err(|_| "reference pre-KIR count exceeds u64".to_owned())?;
        let characteristic_count = u64::try_from(characteristics.len())
            .map_err(|_| "reference characteristic count exceeds u64".to_owned())?;
        let catalog_record_count = correlation_count
            .checked_add(pre_kir_count)
            .and_then(|count| count.checked_add(2))
            .ok_or_else(|| "reference catalog record count overflow".to_owned())?;
        let queries = reference_query_results_v2(&characteristics, &pre_kir_eliminations);
        let cell = AdmittedCharacteristicCellV2 {
            family,
            producer_binding,
            scan: CompleteScanV2 {
                target_operations_scanned: producer_binding.structural_counts.operations,
                catalog_records_scanned: catalog_record_count,
                catalog_record_count,
                characteristic_operations: characteristic_count,
                characteristic_correlations: correlation_count,
                pre_kir_eliminations: pre_kir_count,
            },
            characteristics,
            pre_kir_eliminations,
            queries,
        };
        Ok((broker, producer_binding, cell))
    }

    fn reseal_and_readmit(
        &mut self,
        _admitted: &AdmittedCharacteristicCellV2,
        _substitution: HostileSubstitutionV2,
    ) -> Result<AdmittedCharacteristicCellV2, Self::Error> {
        Err("independent exact projection rejects resealed substitution".to_owned())
    }
}

fn reference_producer_binding_v2(
    family_byte: u8,
    target_byte: u8,
    target: TargetV2,
    operations: u64,
) -> ProducerBindingV2 {
    ProducerBindingV2 {
        target,
        kir_version: KirVersionV2::V8,
        neutral_kir: content_identity_v2(family_byte + 2, 200),
        target_kir: content_identity_v2(family_byte + target_byte + 4, 220),
        source_map_v2: content_identity_v2(family_byte + 5, 240),
        artifact: content_identity_v2(family_byte + target_byte + 8, 260),
        structural_counts: StructuralCountsV2 {
            functions: 1,
            defined_bodies: 1,
            blocks: 1,
            operations,
        },
        structural_binding: [family_byte + target_byte + 9; 32],
        structural_bridge: content_identity_v2(family_byte + target_byte + 10, 280),
        catalog: content_identity_v2(family_byte + target_byte + 12, 300),
        correlation: [family_byte + target_byte + 14; 32],
        semantic_map: [family_byte + target_byte + 16; 32],
    }
}

const fn content_identity_v2(byte: u8, byte_len: u64) -> ContentIdentityV2 {
    ContentIdentityV2 {
        sha256: [byte; 32],
        byte_len,
    }
}

fn reference_kinds_v2(family: KernelFamilyV2) -> Vec<CharacteristicKindV2> {
    match family {
        KernelFamilyV2::ElementwiseFill => vec![CharacteristicKindV2::GlobalStore {
            form: MemoryFormV2::Plain,
        }],
        KernelFamilyV2::NeutralWorkgroupReduction => vec![
            CharacteristicKindV2::GlobalStore {
                form: MemoryFormV2::Plain,
            },
            CharacteristicKindV2::WorkgroupLdsRead {
                form: MemoryFormV2::Plain,
            },
            CharacteristicKindV2::WorkgroupLdsWrite {
                form: MemoryFormV2::Guarded,
            },
            CharacteristicKindV2::WorkgroupBarrier,
        ],
        KernelFamilyV2::TiledBf16Gemm => vec![
            CharacteristicKindV2::GlobalStore {
                form: MemoryFormV2::Guarded,
            },
            CharacteristicKindV2::Bf16F32M16N16K16Wave64MatrixMultiplyAccumulate,
        ],
    }
}

fn reference_characteristic_v2(
    family_byte: u8,
    target_byte: u8,
    operation: u64,
    kind: CharacteristicKindV2,
) -> TargetCharacteristicV2 {
    let catalog_record_ordinal = operation
        .checked_mul(3)
        .and_then(|ordinal| ordinal.checked_add(1))
        .expect("reference catalog ordinal is bounded");
    let target_kir = KirCoordinateV2 {
        function: 0,
        block: 0,
        operation,
    };
    let preserved = TargetCatalogFactV2::SourceAnchored {
        catalog_record_ordinal,
        source: SourceCoordinateV2 {
            node: [family_byte + 0x50; 32],
            span: SourceSpanV2 {
                file: [family_byte + 0x51; 32],
                byte_start: 100,
                byte_end: 120,
                line: 4,
                column: 2,
            },
        },
        mir_node: [family_byte + 0x52; 32],
        mir: MirCoordinateV2 {
            body: 0,
            block: 0,
            statement: operation,
        },
        neutral_kir_node: [family_byte + 0x53; 32],
        neutral_kir: target_kir,
        target_kir,
        semantic_operation: [family_byte + target_byte + 0x54; 32],
        compiler_handoff_llvm: LlvmCoordinateV2 {
            function: 0,
            block: 0,
            instruction: operation * 2,
        },
        transformation: TransformationV2::Preserved,
        // Exact duplicate intervals are catalog-valid and their multiplicity is retained.
        isa: vec![
            IsaIntervalV2 {
                kernel: 0,
                pc_start: operation * 16,
                pc_end: operation * 16 + 4,
            },
            IsaIntervalV2 {
                kernel: 0,
                pc_start: operation * 16,
                pc_end: operation * 16 + 4,
            },
        ],
    };
    let eliminated = TargetCatalogFactV2::NoSourceProvenance {
        catalog_record_ordinal: catalog_record_ordinal
            .checked_add(2)
            .expect("reference catalog ordinal is bounded"),
        target_kir,
        semantic_operation: [family_byte + target_byte + 0x55; 32],
        compiler_handoff_llvm: LlvmCoordinateV2 {
            function: 0,
            block: 0,
            instruction: operation * 2 + 1,
        },
        transformation: TransformationV2::Eliminated,
        isa: Vec::new(),
    };
    TargetCharacteristicV2 {
        target_kir,
        kind,
        correlations: vec![
            preserved.clone(),
            preserved.with_catalog_record_ordinal(
                catalog_record_ordinal
                    .checked_add(1)
                    .expect("reference catalog ordinal is bounded"),
            ),
            eliminated,
        ],
    }
}

#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
mod production_adapter_v2 {
    use super::*;
    use std::path::{Path, PathBuf};

    use fe2o3_source_isa_observation::characteristic_v1::{
        InertSourceIsaCharacteristicCollectionV1, SourceIsaCharacteristicBindingV1,
        SourceIsaCharacteristicCollectionV1,
        SourceIsaCharacteristicCompilerHandoffLlvmCoordinateV1,
        SourceIsaCharacteristicContentIdentityV1, SourceIsaCharacteristicIsaIntervalV1,
        SourceIsaCharacteristicKindV1, SourceIsaCharacteristicKirCoordinateV1,
        SourceIsaCharacteristicKirVersionV1, SourceIsaCharacteristicMatchOutcomeV1,
        SourceIsaCharacteristicMemoryFormV1, SourceIsaCharacteristicMirCoordinateV1,
        SourceIsaCharacteristicPreKirEliminationV1, SourceIsaCharacteristicQueryV1,
        SourceIsaCharacteristicRecordKindV1, SourceIsaCharacteristicScanStateV1,
        SourceIsaCharacteristicScanSummaryV1, SourceIsaCharacteristicSourceCoordinateV1,
        SourceIsaCharacteristicSourceSpanV1, SourceIsaCharacteristicStructuralCountsV1,
        SourceIsaCharacteristicTargetCorrelationV1, SourceIsaCharacteristicTargetProfileV1,
        SourceIsaCharacteristicTargetQueryV1, SourceIsaCharacteristicTargetV1,
        SourceIsaCharacteristicTransformationV1,
    };
    use serde_json::Value;

    use crate::build_config::{PRODUCTION_BUILD_CONFIG_V2_ENV, PreparedProductionBuildConfig};
    use crate::production_source_isa_unit_matrix_v1::{
        ScratchDirectory, UnitCaseV1, assert_sources_unchanged, measured_test_driver,
        read_bounded_template, run_protected_build_stderr, snapshot_sources, unit_cases, workspace,
        write_case_config,
    };

    const CHARACTERISTIC_PREFIX: &str =
        "[cargo-fe2o3] source-isa-characteristic-broker-v3 encoding=hex:";

    struct ProductionCharacteristicAdapterV2 {
        units: [UnitCaseV1; 3],
        driver: crate::pinned_executable::PinnedExecutable,
        template: Value,
        exact_bodies: BTreeMap<(KernelFamilyV2, TargetV2), (BrokerCellBindingV3, Vec<u8>)>,
    }

    impl ProductionCharacteristicAdapterV2 {
        fn from_environment() -> Self {
            let workspace = workspace();
            let template_path = PathBuf::from(
                std::env::var_os(PRODUCTION_BUILD_CONFIG_V2_ENV).unwrap_or_else(|| {
                    panic!(
                        "set {PRODUCTION_BUILD_CONFIG_V2_ENV} to a canonical protected V2 template"
                    )
                }),
            );
            Self {
                units: unit_cases(&workspace),
                driver: measured_test_driver(),
                template: read_bounded_template(&template_path),
                exact_bodies: BTreeMap::new(),
            }
        }

        fn unit(&self, family: KernelFamilyV2) -> &UnitCaseV1 {
            let index = families_v2()
                .iter()
                .position(|candidate| *candidate == family)
                .expect("production family belongs to the exact matrix");
            &self.units[index]
        }
    }

    impl ProtectedCharacteristicAdapterV2 for ProductionCharacteristicAdapterV2 {
        type Error = String;

        fn capture(
            &mut self,
            family: KernelFamilyV2,
            target: TargetV2,
        ) -> Result<
            (
                BrokerCellBindingV3,
                ProducerBindingV2,
                AdmittedCharacteristicCellV2,
            ),
            Self::Error,
        > {
            let unit = self.unit(family);
            let snapshot = snapshot_sources(unit);
            let config_scratch = ScratchDirectory::new(unit.label);
            let mut template = self.template.clone();
            template["observation"] = serde_json::json!({"kind": "source-isa-characteristic-v1"});
            let config_path = write_case_config(
                &config_scratch.0,
                &template,
                unit,
                unit.crate_name,
                unit.source,
            );
            let config = PreparedProductionBuildConfig::from_v2_manifest_for_test(&config_path)
                .map_err(|error| format!("characteristic V2 config rejected: {error}"))?;
            let unit_identity = config
                .source_isa_unit_identity(
                    unit.crate_name,
                    Path::new(unit.source),
                    &unit.working_directory,
                )
                .ok_or_else(|| "characteristic V2 config omitted its exact unit".to_owned())?;
            let cpu = match target {
                TargetV2::Gfx942 => "gfx942",
                TargetV2::Gfx950 => "gfx950",
            };
            let build_scratch =
                ScratchDirectory::new(&format!("{}-{cpu}-characteristic", unit.label));
            let stderr =
                run_protected_build_stderr(&self.driver, unit, cpu, &config_path, &build_scratch.0);
            assert_sources_unchanged(&snapshot);
            let binding = BrokerCellBindingV3 {
                config: *config.identity().as_bytes(),
                unit: *unit_identity.as_bytes(),
                target,
            };
            let encoded = parse_characteristic_line(&stderr)?;
            let broker = read_broker_v3(encoded.as_slice(), binding)?;
            let inert = InertSourceIsaCharacteristicCollectionV1::decode_canonical(&broker.body)
                .map_err(|error| format!("characteristic body rejected: {error}"))?;
            if inert.grants_compiler_authority()
                || inert.grants_runtime_authority()
                || inert.grants_hardware_observation_authority()
            {
                return Err("inert characteristic body acquired authority".to_owned());
            }
            let collection = inert.into_self_claimed_archive_for_agent_inspection_v1();
            if collection
                .encode_canonical()
                .map_err(|error| error.to_string())?
                != broker.body
            {
                return Err("characteristic body is not an exact canonical round trip".to_owned());
            }
            let cell = map_collection(family, &collection)?;
            let producer_binding = cell.producer_binding;
            if self
                .exact_bodies
                .insert((family, target), (binding, broker.body))
                .is_some()
            {
                return Err("duplicate production characteristic cell".to_owned());
            }
            Ok((binding, producer_binding, cell))
        }

        fn reseal_and_readmit(
            &mut self,
            admitted: &AdmittedCharacteristicCellV2,
            substitution: HostileSubstitutionV2,
        ) -> Result<AdmittedCharacteristicCellV2, Self::Error> {
            let target = admitted.producer_binding.target;
            let (broker_binding, exact_body) = self
                .exact_bodies
                .get(&(admitted.family, target))
                .ok_or_else(|| "missing exact authenticated production body".to_owned())?;
            let exact = InertSourceIsaCharacteristicCollectionV1::decode_canonical(exact_body)
                .map_err(|error| error.to_string())?
                .into_self_claimed_archive_for_agent_inspection_v1();
            if matches!(substitution, HostileSubstitutionV2::BrokerConfigIdentity) {
                let mut substituted = encode_broker_v3(*broker_binding, exact_body);
                substituted[BROKER_MAGIC_V3.len() + 32] ^= 1;
                return match read_broker_v3(substituted.as_slice(), *broker_binding) {
                    Ok(_) => map_collection(admitted.family, &exact),
                    Err(error) => Err(format!(
                        "Broker V3 rejected production config substitution: {error}"
                    )),
                };
            }
            if matches!(substitution, HostileSubstitutionV2::BrokerUnitIdentity) {
                let mut substituted = encode_broker_v3(*broker_binding, exact_body);
                substituted[BROKER_MAGIC_V3.len() + 64] ^= 1;
                return match read_broker_v3(substituted.as_slice(), *broker_binding) {
                    Ok(_) => map_collection(admitted.family, &exact),
                    Err(error) => Err(format!("Broker V3 rejected unit substitution: {error}")),
                };
            }
            let mutated = canonically_resealed_substitution(admitted, substitution)?;
            let encoded = rebuild_collection(&mutated)?
                .encode_canonical()
                .map_err(|error| error.to_string())?;
            let inert = InertSourceIsaCharacteristicCollectionV1::decode_canonical(&encoded)
                .map_err(|error| error.to_string())?;
            inert
                .admit_exact_projection_v1(&exact)
                .map_err(|error| format!("exact production projection rejected: {error}"))?;
            map_collection(admitted.family, &exact)
        }
    }

    #[test]
    #[ignore = "requires the protected authority service and measured Worker V3 environment"]
    fn ordinary_source_units_preserve_characteristic_facts_on_both_targets_v2() {
        let mut adapter = ProductionCharacteristicAdapterV2::from_environment();
        assert_protected_characteristic_matrix_v2(&mut adapter);
    }

    #[test]
    fn reconstructed_archive_rejects_every_hostile_binding_and_fact_substitution() {
        let mut reference = ReferenceContractAdapterV2;
        let (broker, _, cell) = reference
            .capture(KernelFamilyV2::ElementwiseFill, TargetV2::Gfx942)
            .unwrap();
        let exact = rebuild_collection(&cell).unwrap();
        let exact_bytes = exact.encode_canonical().unwrap();
        assert_eq!(map_collection(cell.family, &exact).unwrap(), cell);

        for substitution in hostile_substitutions_v2() {
            match substitution {
                HostileSubstitutionV2::BrokerConfigIdentity
                | HostileSubstitutionV2::BrokerUnitIdentity => {
                    let mut encoded = encode_broker_v3(broker, &exact_bytes);
                    let offset = BROKER_MAGIC_V3.len()
                        + if substitution == HostileSubstitutionV2::BrokerConfigIdentity {
                            32
                        } else {
                            64
                        };
                    encoded[offset] ^= 1;
                    assert!(read_broker_v3(encoded.as_slice(), broker).is_err());
                }
                _ => {
                    let changed = canonically_resealed_substitution(&cell, substitution).unwrap();
                    let encoded = rebuild_collection(&changed)
                        .unwrap()
                        .encode_canonical()
                        .unwrap();
                    let inert =
                        InertSourceIsaCharacteristicCollectionV1::decode_canonical(&encoded)
                            .unwrap();
                    assert_eq!(
                        inert.admit_exact_projection_v1(&exact).unwrap_err(),
                        fe2o3_source_isa_observation::characteristic_v1::SourceIsaCharacteristicErrorV1::IdentityMismatch,
                        "{substitution:?} did not fail exact producer readmission"
                    );
                }
            }
        }
    }

    fn parse_characteristic_line(stderr: &str) -> Result<Vec<u8>, String> {
        let mut matches = stderr
            .lines()
            .filter_map(|line| line.strip_prefix(CHARACTERISTIC_PREFIX));
        let encoded = matches
            .next()
            .ok_or_else(|| "missing Source/ISA characteristic Broker V3 line".to_owned())?
            .strip_suffix(" authority=observation-only")
            .ok_or_else(|| "malformed Source/ISA characteristic Broker V3 suffix".to_owned())?;
        if matches.next().is_some() {
            return Err("multiple Source/ISA characteristic Broker V3 lines".to_owned());
        }
        if encoded.is_empty()
            || !encoded.len().is_multiple_of(2)
            || encoded.len()
                > MAX_BROKER_BODY_BYTES_V3
                    .saturating_add(256)
                    .saturating_mul(2)
            || !encoded
                .bytes()
                .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
        {
            return Err("Broker V3 line is not bounded canonical lowercase hex".to_owned());
        }
        encoded
            .as_bytes()
            .chunks_exact(2)
            .map(|pair| {
                u8::from_str_radix(std::str::from_utf8(pair).expect("validated ASCII hex"), 16)
                    .map_err(|error| format!("invalid Broker V3 hex pair: {error}"))
            })
            .collect()
    }

    fn map_collection(
        family: KernelFamilyV2,
        collection: &SourceIsaCharacteristicCollectionV1,
    ) -> Result<AdmittedCharacteristicCellV2, String> {
        let binding = map_binding(collection)?;
        let scan = collection.scan();
        if !matches!(scan.state(), SourceIsaCharacteristicScanStateV1::Complete) {
            return Err(format!(
                "production characteristic scan is incomplete: {:?}",
                scan.state()
            ));
        }
        let characteristics = collection
            .targets()
            .iter()
            .map(map_target)
            .collect::<Result<Vec<_>, _>>()?;
        let pre_kir_eliminations = collection
            .pre_kir_eliminations()
            .iter()
            .copied()
            .map(|fact| PreKirEliminationV2 {
                catalog_record_ordinal: fact.catalog_record_ordinal(),
                source: map_source(fact.source()),
                mir_node: fact.mir_node_identity(),
                mir: map_mir(fact.mir()),
            })
            .collect::<Vec<_>>();
        let queries = reference_query_results_v2(&characteristics, &pre_kir_eliminations);
        exercise_typed_queries(collection, &queries)?;
        Ok(AdmittedCharacteristicCellV2 {
            family,
            producer_binding: binding,
            scan: CompleteScanV2 {
                target_operations_scanned: scan.target_operations_scanned(),
                catalog_records_scanned: scan.catalog_records_scanned(),
                catalog_record_count: scan.catalog_record_count(),
                characteristic_operations: scan.classified_target_count(),
                characteristic_correlations: scan.retained_target_correlation_count(),
                pre_kir_eliminations: scan.pre_kir_elimination_count(),
            },
            characteristics,
            pre_kir_eliminations,
            queries,
        })
    }

    fn map_binding(
        collection: &SourceIsaCharacteristicCollectionV1,
    ) -> Result<ProducerBindingV2, String> {
        let binding = collection.binding();
        let counts = binding.structural_counts();
        Ok(ProducerBindingV2 {
            target: match binding.target_profile() {
                SourceIsaCharacteristicTargetProfileV1::Gfx942 => TargetV2::Gfx942,
                SourceIsaCharacteristicTargetProfileV1::Gfx950 => TargetV2::Gfx950,
            },
            kir_version: match binding.kir_version().code() {
                8 => KirVersionV2::V8,
                _ => return Err("production characteristic KIR version is not V8".to_owned()),
            },
            neutral_kir: map_content(binding.neutral_kir()),
            target_kir: map_content(binding.target_kir()),
            source_map_v2: map_content(binding.source_map_v2()),
            artifact: map_content(binding.artifact()),
            structural_counts: StructuralCountsV2 {
                functions: counts.functions,
                defined_bodies: counts.defined_bodies,
                blocks: counts.blocks,
                operations: counts.operations,
            },
            structural_binding: binding.structural_identity(),
            structural_bridge: map_content(binding.structural_bridge()),
            catalog: map_content(binding.catalog()),
            correlation: binding.correlation_identity(),
            semantic_map: binding.semantic_map_identity(),
        })
    }

    const fn map_content(
        value: fe2o3_source_isa_observation::characteristic_v1::SourceIsaCharacteristicContentIdentityV1,
    ) -> ContentIdentityV2 {
        ContentIdentityV2 {
            sha256: value.sha256(),
            byte_len: value.byte_len(),
        }
    }

    fn map_target(
        target: &fe2o3_source_isa_observation::characteristic_v1::SourceIsaCharacteristicTargetV1,
    ) -> Result<TargetCharacteristicV2, String> {
        let target_kir = map_kir(target.target_kir());
        Ok(TargetCharacteristicV2 {
            target_kir,
            kind: map_kind(target.kind()),
            correlations: target
                .correlations()
                .iter()
                .map(|fact| map_fact(fact, target_kir))
                .collect::<Result<Vec<_>, _>>()?,
        })
    }

    fn map_fact(
        fact: &fe2o3_source_isa_observation::characteristic_v1::SourceIsaCharacteristicTargetCorrelationV1,
        target_kir: KirCoordinateV2,
    ) -> Result<TargetCatalogFactV2, String> {
        if map_kir(fact.target_kir()) != target_kir {
            return Err("correlation target KIR differs from its characteristic".to_owned());
        }
        let isa = fact
            .isa_intervals()
            .iter()
            .map(|interval| IsaIntervalV2 {
                kernel: interval.kernel_ordinal(),
                pc_start: interval.symbol_relative_start(),
                pc_end: interval.symbol_relative_end(),
            })
            .collect();
        let transformation = map_transformation(fact.transformation());
        match fact.kind() {
            SourceIsaCharacteristicRecordKindV1::SourceAnchored => {
                Ok(TargetCatalogFactV2::SourceAnchored {
                    catalog_record_ordinal: fact.catalog_record_ordinal(),
                    source: map_source(fact.source().ok_or_else(|| {
                        "source-anchored characteristic omitted source".to_owned()
                    })?),
                    mir_node: fact.mir_node_identity().ok_or_else(|| {
                        "source-anchored characteristic omitted MIR node".to_owned()
                    })?,
                    mir: map_mir(fact.mir().ok_or_else(|| {
                        "source-anchored characteristic omitted MIR coordinate".to_owned()
                    })?),
                    neutral_kir_node: fact.neutral_kir_node_identity().ok_or_else(|| {
                        "source-anchored characteristic omitted neutral-KIR node".to_owned()
                    })?,
                    neutral_kir: map_kir(fact.neutral_kir().ok_or_else(|| {
                        "source-anchored characteristic omitted neutral-KIR coordinate".to_owned()
                    })?),
                    target_kir,
                    semantic_operation: fact.semantic_operation_identity(),
                    compiler_handoff_llvm: map_llvm(fact.compiler_handoff_llvm()),
                    transformation,
                    isa,
                })
            }
            SourceIsaCharacteristicRecordKindV1::NoSourceProvenance => {
                Ok(TargetCatalogFactV2::NoSourceProvenance {
                    catalog_record_ordinal: fact.catalog_record_ordinal(),
                    target_kir,
                    semantic_operation: fact.semantic_operation_identity(),
                    compiler_handoff_llvm: map_llvm(fact.compiler_handoff_llvm()),
                    transformation,
                    isa,
                })
            }
            SourceIsaCharacteristicRecordKindV1::EliminatedBeforeKir => {
                Err("pre-KIR elimination appeared in a target characteristic".to_owned())
            }
        }
    }

    const fn map_source(
        source: fe2o3_source_isa_observation::characteristic_v1::SourceIsaCharacteristicSourceCoordinateV1,
    ) -> SourceCoordinateV2 {
        let span = source.span();
        SourceCoordinateV2 {
            node: source.node_identity(),
            span: SourceSpanV2 {
                file: span.file_identity(),
                byte_start: span.byte_start(),
                byte_end: span.byte_end(),
                line: span.line(),
                column: span.column(),
            },
        }
    }

    const fn map_mir(
        value: fe2o3_source_isa_observation::characteristic_v1::SourceIsaCharacteristicMirCoordinateV1,
    ) -> MirCoordinateV2 {
        MirCoordinateV2 {
            body: value.body_ordinal(),
            block: value.block_ordinal(),
            statement: value.statement_ordinal(),
        }
    }

    const fn map_kir(
        value: fe2o3_source_isa_observation::characteristic_v1::SourceIsaCharacteristicKirCoordinateV1,
    ) -> KirCoordinateV2 {
        KirCoordinateV2 {
            function: value.function_ordinal(),
            block: value.block_ordinal(),
            operation: value.operation_ordinal(),
        }
    }

    const fn map_llvm(
        value: fe2o3_source_isa_observation::characteristic_v1::SourceIsaCharacteristicCompilerHandoffLlvmCoordinateV1,
    ) -> LlvmCoordinateV2 {
        LlvmCoordinateV2 {
            function: value.function_ordinal(),
            block: value.block_ordinal(),
            instruction: value.instruction_ordinal(),
        }
    }

    const fn map_memory(value: SourceIsaCharacteristicMemoryFormV1) -> MemoryFormV2 {
        match value {
            SourceIsaCharacteristicMemoryFormV1::Plain => MemoryFormV2::Plain,
            SourceIsaCharacteristicMemoryFormV1::Guarded => MemoryFormV2::Guarded,
            SourceIsaCharacteristicMemoryFormV1::MatrixTile => MemoryFormV2::MatrixTile,
        }
    }

    const fn map_kind(value: SourceIsaCharacteristicKindV1) -> CharacteristicKindV2 {
        match value {
            SourceIsaCharacteristicKindV1::GlobalStore { form } => {
                CharacteristicKindV2::GlobalStore {
                    form: map_memory(form),
                }
            }
            SourceIsaCharacteristicKindV1::WorkgroupLoad { form } => {
                CharacteristicKindV2::WorkgroupLdsRead {
                    form: map_memory(form),
                }
            }
            SourceIsaCharacteristicKindV1::WorkgroupStore { form } => {
                CharacteristicKindV2::WorkgroupLdsWrite {
                    form: map_memory(form),
                }
            }
            SourceIsaCharacteristicKindV1::WorkgroupBarrier => {
                CharacteristicKindV2::WorkgroupBarrier
            }
            SourceIsaCharacteristicKindV1::Bf16F32M16N16K16Wave64MatrixMultiplyAccumulate => {
                CharacteristicKindV2::Bf16F32M16N16K16Wave64MatrixMultiplyAccumulate
            }
        }
    }

    const fn map_transformation(
        value: SourceIsaCharacteristicTransformationV1,
    ) -> TransformationV2 {
        match value {
            SourceIsaCharacteristicTransformationV1::Preserved => TransformationV2::Preserved,
            SourceIsaCharacteristicTransformationV1::Duplicated => TransformationV2::Duplicated,
            SourceIsaCharacteristicTransformationV1::Coalesced => TransformationV2::Coalesced,
            SourceIsaCharacteristicTransformationV1::DuplicatedAndCoalesced => {
                TransformationV2::DuplicatedAndCoalesced
            }
            SourceIsaCharacteristicTransformationV1::Eliminated => TransformationV2::Eliminated,
        }
    }

    fn exercise_typed_queries(
        collection: &SourceIsaCharacteristicCollectionV1,
        expected: &QueryResultsV2,
    ) -> Result<(), String> {
        let all_matches = assert_fact_query(
            collection,
            SourceIsaCharacteristicQueryV1::All,
            &expected.all,
        )?;
        assert_fact_query(
            collection,
            SourceIsaCharacteristicQueryV1::PreKirOnly,
            &expected.pre_kir,
        )?;

        for (kind, occurrences) in &expected.category {
            assert_fact_query(
                collection,
                SourceIsaCharacteristicQueryV1::Kind(rebuild_kind(*kind)),
                occurrences,
            )?;
        }
        for family in [
            CharacteristicFamilyV2::GlobalStore,
            CharacteristicFamilyV2::WorkgroupLdsRead,
            CharacteristicFamilyV2::WorkgroupLdsWrite,
            CharacteristicFamilyV2::WorkgroupBarrier,
            CharacteristicFamilyV2::Bf16MfmaExact,
        ] {
            let occurrences = merge_fact_category_occurrences(expected, family);
            if let Some(query) = collection
                .targets()
                .iter()
                .find(|target| map_kind(target.kind()).family() == family)
                .map(|target| SourceIsaCharacteristicQueryV1::Category(target.category()))
            {
                assert_fact_query(collection, query, &occurrences)?;
            }
        }
        for (kind, occurrences) in &expected.record_kind {
            assert_fact_query(
                collection,
                SourceIsaCharacteristicQueryV1::RecordKind(rebuild_record_kind(*kind)),
                occurrences,
            )?;
        }
        for (key, occurrences) in &expected.source_node {
            assert_fact_query(
                collection,
                SourceIsaCharacteristicQueryV1::SourceNode(*key),
                occurrences,
            )?;
        }
        for (key, occurrences) in &expected.source_span {
            assert_fact_query(
                collection,
                SourceIsaCharacteristicQueryV1::SourceSpan(rebuild_span(*key)?),
                occurrences,
            )?;
        }
        for (key, occurrences) in &expected.mir_node {
            assert_fact_query(
                collection,
                SourceIsaCharacteristicQueryV1::MirNode(*key),
                occurrences,
            )?;
        }
        for (key, occurrences) in &expected.mir {
            assert_fact_query(
                collection,
                SourceIsaCharacteristicQueryV1::Mir(rebuild_mir(*key)?),
                occurrences,
            )?;
        }
        for (key, occurrences) in &expected.neutral_kir_node {
            assert_fact_query(
                collection,
                SourceIsaCharacteristicQueryV1::NeutralKirNode(*key),
                occurrences,
            )?;
        }
        for (key, occurrences) in &expected.neutral_kir {
            assert_fact_query(
                collection,
                SourceIsaCharacteristicQueryV1::NeutralKir(rebuild_kir(*key)?),
                occurrences,
            )?;
        }
        for (key, occurrences) in &expected.target_kir {
            assert_fact_query(
                collection,
                SourceIsaCharacteristicQueryV1::TargetKir(rebuild_kir(*key)?),
                occurrences,
            )?;
        }
        for (key, occurrences) in &expected.semantic_operation {
            assert_fact_query(
                collection,
                SourceIsaCharacteristicQueryV1::SemanticOperation(*key),
                occurrences,
            )?;
        }
        for (key, occurrences) in &expected.llvm {
            assert_fact_query(
                collection,
                SourceIsaCharacteristicQueryV1::CompilerHandoffLlvm(rebuild_llvm(*key)?),
                occurrences,
            )?;
        }
        for (key, occurrences) in &expected.transformation {
            assert_fact_query(
                collection,
                SourceIsaCharacteristicQueryV1::Transformation(rebuild_transformation(*key)),
                occurrences,
            )?;
        }
        for ((kernel_ordinal, symbol_relative_pc), occurrences) in &expected.exact_pc {
            assert_fact_query(
                collection,
                SourceIsaCharacteristicQueryV1::ExactPc {
                    kernel_ordinal: *kernel_ordinal,
                    symbol_relative_pc: *symbol_relative_pc,
                },
                occurrences,
            )?;
        }

        assert_target_query(
            collection,
            SourceIsaCharacteristicTargetQueryV1::All,
            &expected.structural,
        )?;
        for (ordinal, target) in collection.targets().iter().enumerate() {
            let occurrence = TargetCharacteristicOccurrenceV2 {
                characteristic_ordinal: u32::try_from(ordinal)
                    .map_err(|_| "target ordinal exceeds u32".to_owned())?,
            };
            assert_target_query(
                collection,
                SourceIsaCharacteristicTargetQueryV1::CharacteristicIdentity(target.identity()),
                &BTreeSet::from([occurrence]),
            )?;
            assert_fact_query(
                collection,
                SourceIsaCharacteristicQueryV1::CharacteristicIdentity(target.identity()),
                &expected
                    .all
                    .iter()
                    .copied()
                    .filter(|candidate| matches!(candidate, FactOccurrenceV2::Target { characteristic_ordinal, .. } if *characteristic_ordinal == occurrence.characteristic_ordinal))
                    .collect(),
            )?;
        }
        for (kind, occurrences) in &expected.structural_category {
            assert_target_query(
                collection,
                SourceIsaCharacteristicTargetQueryV1::Kind(rebuild_kind(*kind)),
                occurrences,
            )?;
        }
        for family in [
            CharacteristicFamilyV2::GlobalStore,
            CharacteristicFamilyV2::WorkgroupLdsRead,
            CharacteristicFamilyV2::WorkgroupLdsWrite,
            CharacteristicFamilyV2::WorkgroupBarrier,
            CharacteristicFamilyV2::Bf16MfmaExact,
        ] {
            let occurrences = merge_target_category_occurrences(expected, family);
            if let Some(query) = collection
                .targets()
                .iter()
                .find(|target| map_kind(target.kind()).family() == family)
                .map(|target| SourceIsaCharacteristicTargetQueryV1::Category(target.category()))
            {
                assert_target_query(collection, query, &occurrences)?;
            }
        }
        for (coordinate, occurrences) in &expected.structural_target_kir {
            assert_target_query(
                collection,
                SourceIsaCharacteristicTargetQueryV1::TargetKir(rebuild_kir(*coordinate)?),
                occurrences,
            )?;
        }

        for (occurrence, match_identity) in all_matches {
            if let FactOccurrenceV2::Target { .. } = occurrence {
                assert_interval_query(
                    collection,
                    match_identity,
                    expected
                        .intervals
                        .get(&occurrence)
                        .map_or(&[][..], Vec::as_slice),
                )?;
            }
        }
        Ok(())
    }

    fn assert_fact_query(
        collection: &SourceIsaCharacteristicCollectionV1,
        query: SourceIsaCharacteristicQueryV1,
        expected: &BTreeSet<FactOccurrenceV2>,
    ) -> Result<Vec<(FactOccurrenceV2, [u8; 32])>, String> {
        let mut cursor = None;
        let mut observed = Vec::new();
        let mut identities = BTreeSet::new();
        loop {
            let page = collection
                .query_page(&query, cursor.as_ref(), 1)
                .map_err(|error| error.to_string())?;
            if page.collection_identity() != collection.identity()
                || page.query_identity() != query.identity()
                || usize::try_from(page.total_matches()).ok() != Some(expected.len())
            {
                return Err("fact query page binding or total differs from expectation".to_owned());
            }
            for matched in page.matches() {
                if !identities.insert(matched.identity()) {
                    return Err("fact query repeated an occurrence identity".to_owned());
                }
                observed.push((
                    map_match_occurrence(collection, *matched)?,
                    matched.identity(),
                ));
            }
            cursor = page.next_cursor();
            if cursor.is_none() {
                if !page.page_exhausted() {
                    return Err("terminal fact page is not exhausted".to_owned());
                }
                break;
            }
        }
        let observed_set = observed
            .iter()
            .map(|(occurrence, _)| *occurrence)
            .collect::<BTreeSet<_>>();
        if &observed_set != expected {
            return Err("typed fact query differs from its exact occurrence set".to_owned());
        }
        Ok(observed)
    }

    fn assert_target_query(
        collection: &SourceIsaCharacteristicCollectionV1,
        query: SourceIsaCharacteristicTargetQueryV1,
        expected: &BTreeSet<TargetCharacteristicOccurrenceV2>,
    ) -> Result<(), String> {
        let mut cursor = None;
        let mut observed = BTreeSet::new();
        let mut identities = BTreeSet::new();
        loop {
            let page = collection
                .query_targets_page(&query, cursor.as_ref(), 1)
                .map_err(|error| error.to_string())?;
            if page.collection_identity() != collection.identity()
                || page.query_identity() != query.identity()
                || usize::try_from(page.total_matches()).ok() != Some(expected.len())
            {
                return Err(
                    "target query page binding or total differs from expectation".to_owned(),
                );
            }
            for matched in page.targets() {
                if !identities.insert(matched.identity()) {
                    return Err("target query repeated an occurrence identity".to_owned());
                }
                let ordinal = collection
                    .targets()
                    .iter()
                    .position(|target| target.identity() == matched.characteristic_identity())
                    .ok_or_else(|| "target query returned an unknown characteristic".to_owned())?;
                observed.insert(TargetCharacteristicOccurrenceV2 {
                    characteristic_ordinal: u32::try_from(ordinal)
                        .map_err(|_| "target ordinal exceeds u32".to_owned())?,
                });
            }
            cursor = page.next_cursor();
            if cursor.is_none() {
                if !page.page_exhausted() {
                    return Err("terminal target page is not exhausted".to_owned());
                }
                break;
            }
        }
        if &observed != expected {
            return Err("typed target query differs from its exact occurrence set".to_owned());
        }
        Ok(())
    }

    fn map_match_occurrence(
        collection: &SourceIsaCharacteristicCollectionV1,
        matched: fe2o3_source_isa_observation::characteristic_v1::SourceIsaCharacteristicMatchV1,
    ) -> Result<FactOccurrenceV2, String> {
        match matched.outcome() {
            SourceIsaCharacteristicMatchOutcomeV1::TargetCorrelation(summary) => {
                let characteristic_identity = matched
                    .characteristic_identity()
                    .ok_or_else(|| "target match omitted characteristic identity".to_owned())?;
                let characteristic_ordinal = collection
                    .targets()
                    .iter()
                    .position(|target| target.identity() == characteristic_identity)
                    .ok_or_else(|| "fact query returned an unknown characteristic".to_owned())?;
                let correlation_ordinal = collection.targets()[characteristic_ordinal]
                    .correlations()
                    .iter()
                    .position(|fact| {
                        fact.catalog_record_ordinal() == summary.catalog_record_ordinal
                    })
                    .ok_or_else(|| "fact query returned an unknown correlation".to_owned())?;
                Ok(FactOccurrenceV2::Target {
                    characteristic_ordinal: u32::try_from(characteristic_ordinal)
                        .map_err(|_| "characteristic ordinal exceeds u32".to_owned())?,
                    correlation_ordinal: u32::try_from(correlation_ordinal)
                        .map_err(|_| "correlation ordinal exceeds u32".to_owned())?,
                    catalog_record_ordinal: summary.catalog_record_ordinal,
                })
            }
            SourceIsaCharacteristicMatchOutcomeV1::PreKirElimination(fact) => {
                let elimination_ordinal = collection
                    .pre_kir_eliminations()
                    .iter()
                    .position(|candidate| {
                        candidate.catalog_record_ordinal() == fact.catalog_record_ordinal()
                    })
                    .ok_or_else(|| {
                        "fact query returned an unknown pre-KIR elimination".to_owned()
                    })?;
                Ok(FactOccurrenceV2::PreKir {
                    elimination_ordinal: u32::try_from(elimination_ordinal)
                        .map_err(|_| "pre-KIR ordinal exceeds u32".to_owned())?,
                    catalog_record_ordinal: fact.catalog_record_ordinal(),
                })
            }
        }
    }

    fn merge_fact_category_occurrences(
        expected: &QueryResultsV2,
        family: CharacteristicFamilyV2,
    ) -> BTreeSet<FactOccurrenceV2> {
        expected
            .category
            .iter()
            .filter(|(kind, _)| kind.family() == family)
            .flat_map(|(_, occurrences)| occurrences.iter().copied())
            .collect()
    }

    fn merge_target_category_occurrences(
        expected: &QueryResultsV2,
        family: CharacteristicFamilyV2,
    ) -> BTreeSet<TargetCharacteristicOccurrenceV2> {
        expected
            .structural_category
            .iter()
            .filter(|(kind, _)| kind.family() == family)
            .flat_map(|(_, occurrences)| occurrences.iter().copied())
            .collect()
    }

    fn assert_interval_query(
        collection: &SourceIsaCharacteristicCollectionV1,
        occurrence_identity: [u8; 32],
        expected: &[(u32, IsaIntervalV2)],
    ) -> Result<(), String> {
        let mut cursor = None;
        let mut observed = Vec::new();
        let mut identities = BTreeSet::new();
        loop {
            let page = collection
                .interval_page(occurrence_identity, cursor.as_ref(), 1)
                .map_err(|error| error.to_string())?;
            if page.occurrence_identity() != occurrence_identity
                || usize::try_from(page.total_intervals()).ok() != Some(expected.len())
            {
                return Err("interval page binding or total differs from expectation".to_owned());
            }
            for item in page.intervals() {
                if !identities.insert(item.identity()) {
                    return Err("interval query repeated an item identity".to_owned());
                }
                let interval = item.interval();
                observed.push((
                    u32::try_from(item.ordinal())
                        .map_err(|_| "interval ordinal exceeds u32".to_owned())?,
                    IsaIntervalV2 {
                        kernel: interval.kernel_ordinal(),
                        pc_start: interval.symbol_relative_start(),
                        pc_end: interval.symbol_relative_end(),
                    },
                ));
            }
            cursor = page.next_cursor();
            if cursor.is_none() {
                if !page.page_exhausted() {
                    return Err("terminal interval page is not exhausted".to_owned());
                }
                break;
            }
        }
        if observed != expected {
            return Err("interval query differs from its exact ordered multiplicity".to_owned());
        }
        Ok(())
    }

    fn canonically_resealed_substitution(
        admitted: &AdmittedCharacteristicCellV2,
        substitution: HostileSubstitutionV2,
    ) -> Result<AdmittedCharacteristicCellV2, String> {
        let mut changed = admitted.clone();
        match substitution {
            HostileSubstitutionV2::Category => {
                let first = changed
                    .characteristics
                    .first_mut()
                    .ok_or("missing characteristic")?;
                first.kind = match first.kind {
                    CharacteristicKindV2::GlobalStore {
                        form: MemoryFormV2::Plain,
                    } => CharacteristicKindV2::GlobalStore {
                        form: MemoryFormV2::Guarded,
                    },
                    CharacteristicKindV2::GlobalStore { .. } => CharacteristicKindV2::GlobalStore {
                        form: MemoryFormV2::Plain,
                    },
                    _ => CharacteristicKindV2::GlobalStore {
                        form: MemoryFormV2::Plain,
                    },
                };
            }
            HostileSubstitutionV2::RecordKind => {
                let fact = first_source_fact_mut(&mut changed)?;
                let TargetCatalogFactV2::SourceAnchored {
                    catalog_record_ordinal,
                    target_kir,
                    semantic_operation,
                    compiler_handoff_llvm,
                    transformation,
                    isa,
                    ..
                } = fact.clone()
                else {
                    return Err("no source-anchored fact to substitute".to_owned());
                };
                *fact = TargetCatalogFactV2::NoSourceProvenance {
                    catalog_record_ordinal,
                    target_kir,
                    semantic_operation,
                    compiler_handoff_llvm,
                    transformation,
                    isa,
                };
            }
            HostileSubstitutionV2::SourceCoordinate => {
                if let TargetCatalogFactV2::SourceAnchored { source, .. } =
                    first_source_fact_mut(&mut changed)?
                {
                    source.node[0] ^= 1;
                }
            }
            HostileSubstitutionV2::MirCoordinate => {
                if let TargetCatalogFactV2::SourceAnchored { mir, .. } =
                    first_source_fact_mut(&mut changed)?
                {
                    mir.statement = mir.statement.checked_add(1).ok_or("MIR ordinal overflow")?;
                }
            }
            HostileSubstitutionV2::NeutralKirCoordinate => {
                if let TargetCatalogFactV2::SourceAnchored { neutral_kir, .. } =
                    first_source_fact_mut(&mut changed)?
                {
                    neutral_kir.operation = neutral_kir
                        .operation
                        .checked_add(1)
                        .ok_or("KIR ordinal overflow")?;
                }
            }
            HostileSubstitutionV2::TargetKirCoordinate => {
                let target = changed
                    .characteristics
                    .first_mut()
                    .ok_or("missing characteristic")?;
                target.target_kir.operation = target
                    .target_kir
                    .operation
                    .checked_add(1)
                    .ok_or("target-KIR ordinal overflow")?;
                for fact in &mut target.correlations {
                    match fact {
                        TargetCatalogFactV2::SourceAnchored { target_kir, .. }
                        | TargetCatalogFactV2::NoSourceProvenance { target_kir, .. } => {
                            *target_kir = target.target_kir
                        }
                    }
                }
            }
            HostileSubstitutionV2::SemanticOperation => match first_fact_mut(&mut changed)? {
                TargetCatalogFactV2::SourceAnchored {
                    semantic_operation, ..
                }
                | TargetCatalogFactV2::NoSourceProvenance {
                    semantic_operation, ..
                } => semantic_operation[0] ^= 1,
            },
            HostileSubstitutionV2::CompilerHandoffLlvmCoordinate => {
                match first_fact_mut(&mut changed)? {
                    TargetCatalogFactV2::SourceAnchored {
                        compiler_handoff_llvm,
                        ..
                    }
                    | TargetCatalogFactV2::NoSourceProvenance {
                        compiler_handoff_llvm,
                        ..
                    } => {
                        compiler_handoff_llvm.instruction = compiler_handoff_llvm
                            .instruction
                            .checked_add(1)
                            .ok_or("LLVM ordinal overflow")?;
                    }
                }
            }
            HostileSubstitutionV2::Transformation => match first_fact_mut(&mut changed)? {
                TargetCatalogFactV2::SourceAnchored { transformation, .. }
                | TargetCatalogFactV2::NoSourceProvenance { transformation, .. } => {
                    if *transformation == TransformationV2::Eliminated {
                        return Err("first fact is eliminated".to_owned());
                    }
                    *transformation = if *transformation == TransformationV2::Duplicated {
                        TransformationV2::Preserved
                    } else {
                        TransformationV2::Duplicated
                    };
                }
            },
            HostileSubstitutionV2::IsaInterval => {
                let interval = first_interval_mut(&mut changed)?;
                interval.pc_start = interval.pc_start.checked_add(4).ok_or("ISA PC overflow")?;
                interval.pc_end = interval.pc_end.checked_add(4).ok_or("ISA PC overflow")?;
            }
            HostileSubstitutionV2::IsaIntervalMultiplicity => {
                let fact = first_fact_mut(&mut changed)?;
                let interval = *fact.isa().first().ok_or("missing ISA interval")?;
                match fact {
                    TargetCatalogFactV2::SourceAnchored { isa, .. }
                    | TargetCatalogFactV2::NoSourceProvenance { isa, .. } => isa.push(interval),
                }
            }
            HostileSubstitutionV2::PreKirElimination => {
                if let Some(fact) = changed.pre_kir_eliminations.first_mut() {
                    fact.source.node[0] ^= 1;
                } else {
                    let TargetCatalogFactV2::SourceAnchored {
                        source,
                        mir_node,
                        mir,
                        ..
                    } = first_source_fact_mut(&mut changed)?.clone()
                    else {
                        unreachable!()
                    };
                    let ordinal = changed.scan.catalog_record_count;
                    changed.scan.catalog_record_count =
                        ordinal.checked_add(1).ok_or("catalog count overflow")?;
                    changed.scan.catalog_records_scanned = changed.scan.catalog_record_count;
                    changed.scan.pre_kir_eliminations = changed
                        .scan
                        .pre_kir_eliminations
                        .checked_add(1)
                        .ok_or("pre-KIR count overflow")?;
                    changed.pre_kir_eliminations.push(PreKirEliminationV2 {
                        catalog_record_ordinal: ordinal,
                        source,
                        mir_node,
                        mir,
                    });
                }
            }
            HostileSubstitutionV2::CompleteScan => {
                changed.scan.catalog_record_count = changed
                    .scan
                    .catalog_record_count
                    .checked_add(1)
                    .ok_or("scan overflow")?;
                changed.scan.catalog_records_scanned = changed.scan.catalog_record_count;
            }
            HostileSubstitutionV2::StructuralBridgeIdentity => {
                changed.producer_binding.structural_bridge.sha256[0] ^= 1
            }
            HostileSubstitutionV2::CatalogIdentity => {
                changed.producer_binding.catalog.sha256[0] ^= 1
            }
            HostileSubstitutionV2::ArtifactIdentity => {
                changed.producer_binding.artifact.sha256[0] ^= 1
            }
            HostileSubstitutionV2::ProducerTarget => {
                changed.producer_binding.target = match changed.producer_binding.target {
                    TargetV2::Gfx942 => TargetV2::Gfx950,
                    TargetV2::Gfx950 => TargetV2::Gfx942,
                };
            }
            HostileSubstitutionV2::BrokerConfigIdentity
            | HostileSubstitutionV2::BrokerUnitIdentity => unreachable!(),
        }
        changed.queries =
            reference_query_results_v2(&changed.characteristics, &changed.pre_kir_eliminations);
        Ok(changed)
    }

    fn first_fact_mut(
        cell: &mut AdmittedCharacteristicCellV2,
    ) -> Result<&mut TargetCatalogFactV2, String> {
        cell.characteristics
            .iter_mut()
            .flat_map(|target| target.correlations.iter_mut())
            .find(|fact| !fact.isa().is_empty())
            .ok_or_else(|| "missing non-eliminated target fact".to_owned())
    }

    fn first_source_fact_mut(
        cell: &mut AdmittedCharacteristicCellV2,
    ) -> Result<&mut TargetCatalogFactV2, String> {
        cell.characteristics
            .iter_mut()
            .flat_map(|target| target.correlations.iter_mut())
            .find(|fact| matches!(fact, TargetCatalogFactV2::SourceAnchored { .. }))
            .ok_or_else(|| "missing source-anchored target fact".to_owned())
    }

    fn first_interval_mut(
        cell: &mut AdmittedCharacteristicCellV2,
    ) -> Result<&mut IsaIntervalV2, String> {
        match first_fact_mut(cell)? {
            TargetCatalogFactV2::SourceAnchored { isa, .. }
            | TargetCatalogFactV2::NoSourceProvenance { isa, .. } => isa
                .first_mut()
                .ok_or_else(|| "missing ISA interval".to_owned()),
        }
    }

    fn rebuild_collection(
        cell: &AdmittedCharacteristicCellV2,
    ) -> Result<SourceIsaCharacteristicCollectionV1, String> {
        let producer = cell.producer_binding;
        let counts = producer.structural_counts;
        let binding = SourceIsaCharacteristicBindingV1::new(
            match producer.target {
                TargetV2::Gfx942 => SourceIsaCharacteristicTargetProfileV1::Gfx942,
                TargetV2::Gfx950 => SourceIsaCharacteristicTargetProfileV1::Gfx950,
            },
            SourceIsaCharacteristicKirVersionV1::V8,
            producer.structural_binding,
            SourceIsaCharacteristicStructuralCountsV1 {
                functions: counts.functions,
                defined_bodies: counts.defined_bodies,
                blocks: counts.blocks,
                operations: counts.operations,
            },
            rebuild_content(producer.source_map_v2)?,
            rebuild_content(producer.neutral_kir)?,
            rebuild_content(producer.target_kir)?,
            rebuild_content(producer.artifact)?,
            rebuild_content(producer.catalog)?,
            rebuild_content(producer.structural_bridge)?,
            producer.correlation,
            producer.semantic_map,
        )
        .map_err(|error| error.to_string())?;
        let scan = SourceIsaCharacteristicScanSummaryV1::new(
            cell.scan.catalog_record_count,
            cell.scan.catalog_records_scanned,
            producer.structural_counts.operations,
            cell.scan.target_operations_scanned,
            cell.scan.characteristic_operations,
            cell.scan.characteristic_correlations,
            cell.scan.pre_kir_eliminations,
            cell.scan
                .characteristic_correlations
                .checked_add(cell.scan.pre_kir_eliminations)
                .ok_or_else(|| "retained correlation count overflow".to_owned())?,
            SourceIsaCharacteristicScanStateV1::Complete,
        )
        .map_err(|error| error.to_string())?;
        let targets = cell
            .characteristics
            .iter()
            .map(|target| {
                SourceIsaCharacteristicTargetV1::new(
                    rebuild_kind(target.kind),
                    rebuild_kir(target.target_kir)?,
                    target
                        .correlations
                        .iter()
                        .map(rebuild_fact)
                        .collect::<Result<Vec<_>, String>>()?,
                )
                .map_err(|error| error.to_string())
            })
            .collect::<Result<Vec<_>, String>>()?;
        let pre_kir = cell
            .pre_kir_eliminations
            .iter()
            .map(|fact| {
                SourceIsaCharacteristicPreKirEliminationV1::new(
                    fact.catalog_record_ordinal,
                    rebuild_source(fact.source)?,
                    fact.mir_node,
                    rebuild_mir(fact.mir)?,
                )
                .map_err(|error| error.to_string())
            })
            .collect::<Result<Vec<_>, String>>()?;
        SourceIsaCharacteristicCollectionV1::new(binding, scan, targets, pre_kir)
            .map_err(|error| error.to_string())
    }

    fn rebuild_fact(
        fact: &TargetCatalogFactV2,
    ) -> Result<SourceIsaCharacteristicTargetCorrelationV1, String> {
        let (
            ordinal,
            kind,
            source,
            mir_node,
            mir,
            neutral_node,
            neutral,
            target,
            semantic,
            llvm,
            transformation,
            isa,
        ) = match fact {
            TargetCatalogFactV2::SourceAnchored {
                catalog_record_ordinal,
                source,
                mir_node,
                mir,
                neutral_kir_node,
                neutral_kir,
                target_kir,
                semantic_operation,
                compiler_handoff_llvm,
                transformation,
                isa,
            } => (
                *catalog_record_ordinal,
                SourceIsaCharacteristicRecordKindV1::SourceAnchored,
                Some(rebuild_source(*source)?),
                Some(*mir_node),
                Some(rebuild_mir(*mir)?),
                Some(*neutral_kir_node),
                Some(rebuild_kir(*neutral_kir)?),
                rebuild_kir(*target_kir)?,
                *semantic_operation,
                rebuild_llvm(*compiler_handoff_llvm)?,
                *transformation,
                isa,
            ),
            TargetCatalogFactV2::NoSourceProvenance {
                catalog_record_ordinal,
                target_kir,
                semantic_operation,
                compiler_handoff_llvm,
                transformation,
                isa,
            } => (
                *catalog_record_ordinal,
                SourceIsaCharacteristicRecordKindV1::NoSourceProvenance,
                None,
                None,
                None,
                None,
                None,
                rebuild_kir(*target_kir)?,
                *semantic_operation,
                rebuild_llvm(*compiler_handoff_llvm)?,
                *transformation,
                isa,
            ),
        };
        SourceIsaCharacteristicTargetCorrelationV1::new(
            ordinal,
            kind,
            source,
            mir_node,
            mir,
            neutral_node,
            neutral,
            target,
            semantic,
            llvm,
            isa.iter()
                .map(|interval| {
                    SourceIsaCharacteristicIsaIntervalV1::new(
                        interval.kernel,
                        interval.pc_start,
                        interval.pc_end,
                    )
                    .map_err(|error| error.to_string())
                })
                .collect::<Result<Vec<_>, String>>()?,
            rebuild_transformation(transformation),
        )
        .map_err(|error| error.to_string())
    }

    fn rebuild_content(
        value: ContentIdentityV2,
    ) -> Result<SourceIsaCharacteristicContentIdentityV1, String> {
        SourceIsaCharacteristicContentIdentityV1::new(value.sha256, value.byte_len)
            .map_err(|error| error.to_string())
    }

    fn rebuild_source(
        value: SourceCoordinateV2,
    ) -> Result<SourceIsaCharacteristicSourceCoordinateV1, String> {
        SourceIsaCharacteristicSourceCoordinateV1::new(value.node, rebuild_span(value.span)?)
            .map_err(|error| error.to_string())
    }

    fn rebuild_span(value: SourceSpanV2) -> Result<SourceIsaCharacteristicSourceSpanV1, String> {
        SourceIsaCharacteristicSourceSpanV1::new(
            value.file,
            value.byte_start,
            value.byte_end,
            value.line,
            value.column,
        )
        .map_err(|error| error.to_string())
    }

    fn rebuild_mir(
        value: MirCoordinateV2,
    ) -> Result<SourceIsaCharacteristicMirCoordinateV1, String> {
        SourceIsaCharacteristicMirCoordinateV1::new(value.body, value.block, value.statement)
            .map_err(|error| error.to_string())
    }

    fn rebuild_kir(
        value: KirCoordinateV2,
    ) -> Result<SourceIsaCharacteristicKirCoordinateV1, String> {
        SourceIsaCharacteristicKirCoordinateV1::new(value.function, value.block, value.operation)
            .map_err(|error| error.to_string())
    }

    fn rebuild_llvm(
        value: LlvmCoordinateV2,
    ) -> Result<SourceIsaCharacteristicCompilerHandoffLlvmCoordinateV1, String> {
        SourceIsaCharacteristicCompilerHandoffLlvmCoordinateV1::new(
            value.function,
            value.block,
            value.instruction,
        )
        .map_err(|error| error.to_string())
    }

    const fn rebuild_memory(value: MemoryFormV2) -> SourceIsaCharacteristicMemoryFormV1 {
        match value {
            MemoryFormV2::Plain => SourceIsaCharacteristicMemoryFormV1::Plain,
            MemoryFormV2::Guarded => SourceIsaCharacteristicMemoryFormV1::Guarded,
            MemoryFormV2::MatrixTile => SourceIsaCharacteristicMemoryFormV1::MatrixTile,
        }
    }

    const fn rebuild_kind(value: CharacteristicKindV2) -> SourceIsaCharacteristicKindV1 {
        match value {
            CharacteristicKindV2::GlobalStore { form } => {
                SourceIsaCharacteristicKindV1::GlobalStore {
                    form: rebuild_memory(form),
                }
            }
            CharacteristicKindV2::WorkgroupLdsRead { form } => {
                SourceIsaCharacteristicKindV1::WorkgroupLoad {
                    form: rebuild_memory(form),
                }
            }
            CharacteristicKindV2::WorkgroupLdsWrite { form } => {
                SourceIsaCharacteristicKindV1::WorkgroupStore {
                    form: rebuild_memory(form),
                }
            }
            CharacteristicKindV2::WorkgroupBarrier => {
                SourceIsaCharacteristicKindV1::WorkgroupBarrier
            }
            CharacteristicKindV2::Bf16F32M16N16K16Wave64MatrixMultiplyAccumulate => {
                SourceIsaCharacteristicKindV1::Bf16F32M16N16K16Wave64MatrixMultiplyAccumulate
            }
        }
    }

    const fn rebuild_transformation(
        value: TransformationV2,
    ) -> SourceIsaCharacteristicTransformationV1 {
        match value {
            TransformationV2::Preserved => SourceIsaCharacteristicTransformationV1::Preserved,
            TransformationV2::Duplicated => SourceIsaCharacteristicTransformationV1::Duplicated,
            TransformationV2::Coalesced => SourceIsaCharacteristicTransformationV1::Coalesced,
            TransformationV2::DuplicatedAndCoalesced => {
                SourceIsaCharacteristicTransformationV1::DuplicatedAndCoalesced
            }
            TransformationV2::Eliminated => SourceIsaCharacteristicTransformationV1::Eliminated,
        }
    }

    const fn rebuild_record_kind(
        value: CatalogRecordKindV2,
    ) -> SourceIsaCharacteristicRecordKindV1 {
        match value {
            CatalogRecordKindV2::SourceAnchored => {
                SourceIsaCharacteristicRecordKindV1::SourceAnchored
            }
            CatalogRecordKindV2::NoSourceProvenance => {
                SourceIsaCharacteristicRecordKindV1::NoSourceProvenance
            }
            CatalogRecordKindV2::EliminatedBeforeKir => {
                SourceIsaCharacteristicRecordKindV1::EliminatedBeforeKir
            }
        }
    }
}

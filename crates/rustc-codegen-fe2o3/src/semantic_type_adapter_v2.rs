//! Exact rustc type-layout observation over the untrusted dialect MIR V2 graph.
//!
//! The three boundaries in this module are intentionally distinct:
//! [`SemanticTypeGraphV2`] bytes are untrusted transport input,
//! [`RustcTypeLayoutObservationV2`] is an exact observation from the active
//! rustc session, and neither value grants manifest, device-copy, code
//! generation, loading, or launch authority.

use std::collections::{HashMap, HashSet};
use std::fmt;

use dialect_mir::{
    GFX942_TARGET_CPU, GFX942_TARGET_DATA_LAYOUT, GFX942_TARGET_FEATURES, GFX942_TARGET_TRIPLE,
    PointerMetadataV2, ScalarValidityRangeV2, SemanticEnumEncodingV2, SemanticFieldV2,
    SemanticMutabilityV2, SemanticNichePathComponentV2, SemanticNicheSourceV2, SemanticScalarV2,
    SemanticTypeGraphBudgetsV2, SemanticTypeGraphBuilderV2, SemanticTypeGraphErrorV2,
    SemanticTypeGraphV2, SemanticTypeKindV2, SemanticTypeLayoutV2, SemanticTypeNodeIdV2,
    SemanticTypeNodeV2, SemanticVariantV2,
};
use rustc_abi::{BackendRepr, Primitive};
use rustc_hir::Mutability;
use rustc_middle::ty::layout::{LayoutCx, LayoutOf};
use rustc_middle::ty::print::with_no_trimmed_paths;
use rustc_middle::ty::{Ty, TyCtxt, TyKind, TypingEnv};
use sha2::{Digest, Sha256};

use crate::rust_type_layout_general::{
    AdtKind, AdtLayoutFacts, AdtRepresentationFacts, BackendRepresentationFacts,
    EnumTagEncodingFacts, ExtractionLimits, FieldLayoutFacts, GeneralLayoutExtractError,
    PointerKind, ScalarLayoutFacts, ScalarPrimitiveFacts, SourceScalarKind, TypeLayoutFacts,
    TypeLayoutKind, VariantLayoutFacts, extract_general_layout_with_limits,
};
use crate::semantic_layout_bridge::{
    SemanticLayoutBridgeError, SemanticLayoutTargetV1, rustc_semantic_layout_target_v1,
};

const OBSERVATION_DOMAIN_V2: &[u8] = b"FE2O3/RUSTC-TYPE-LAYOUT-OBSERVATION/V2\0";
const GFX942_PROJECTION_DOMAIN_V2: &[u8] = b"FE2O3/GFX942-LAYOUT-PROJECTION/V2\0";
const GFX942_CANDIDATE_DOMAIN_V2: &[u8] = b"FE2O3/GFX942-LAYOUT-CANDIDATE/V2\0";
const GFX942_POINTER_WIDTH_BITS: u16 = 64;
const DEFAULT_MAX_SIDECAR_RECORDS: u32 = 32_768;
const DEFAULT_MAX_SIDECAR_BYTES: u32 = 8 * 1024 * 1024;
const DEFAULT_MAX_OBSERVATION_WORK: u64 = 1_000_000;
const DEFAULT_MAX_PROJECTION_WORK: u64 = 1_000_000;
const DEFAULT_MAX_TOTAL_TEXT_BYTES: u64 = 8 * 1024 * 1024;
const DEFAULT_MAX_PATH_BYTES: u32 = 4 * 1024;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SemanticTypeLayoutBudgetsV2 {
    pub graph: SemanticTypeGraphBudgetsV2,
    pub max_sidecar_records: u32,
    pub max_sidecar_bytes: u32,
    pub max_observation_work: u64,
    pub max_projection_work: u64,
    pub max_total_text_bytes: u64,
    pub max_path_bytes: u32,
}

impl Default for SemanticTypeLayoutBudgetsV2 {
    fn default() -> Self {
        Self {
            graph: SemanticTypeGraphBudgetsV2::default(),
            max_sidecar_records: DEFAULT_MAX_SIDECAR_RECORDS,
            max_sidecar_bytes: DEFAULT_MAX_SIDECAR_BYTES,
            max_observation_work: DEFAULT_MAX_OBSERVATION_WORK,
            max_projection_work: DEFAULT_MAX_PROJECTION_WORK,
            max_total_text_bytes: DEFAULT_MAX_TOTAL_TEXT_BYTES,
            max_path_bytes: DEFAULT_MAX_PATH_BYTES,
        }
    }
}

/// Canonical device-layout context used only by the conservative projection.
///
/// It is intentionally separate from the active rustc target under which a
/// host observation was produced.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CanonicalGfx942LayoutTargetV2 {
    triple: String,
    cpu: String,
    features: String,
    data_layout: String,
    pointer_width_bits: u16,
}

impl CanonicalGfx942LayoutTargetV2 {
    pub fn canonical() -> Self {
        Self {
            triple: GFX942_TARGET_TRIPLE.to_owned(),
            cpu: GFX942_TARGET_CPU.to_owned(),
            features: GFX942_TARGET_FEATURES.to_owned(),
            data_layout: GFX942_TARGET_DATA_LAYOUT.to_owned(),
            pointer_width_bits: GFX942_POINTER_WIDTH_BITS,
        }
    }

    pub fn triple(&self) -> &str {
        &self.triple
    }

    pub fn cpu(&self) -> &str {
        &self.cpu
    }

    pub fn features(&self) -> &str {
        &self.features
    }

    pub fn data_layout(&self) -> &str {
        &self.data_layout
    }

    pub const fn pointer_width_bits(&self) -> u16 {
        self.pointer_width_bits
    }

    fn is_canonical(&self) -> bool {
        self.triple == GFX942_TARGET_TRIPLE
            && self.cpu == GFX942_TARGET_CPU
            && self.features == GFX942_TARGET_FEATURES
            && self.data_layout == GFX942_TARGET_DATA_LAYOUT
            && self.pointer_width_bits == GFX942_POINTER_WIDTH_BITS
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RustcRepresentationV2 {
    pub c: bool,
    pub transparent: bool,
    pub explicit_integer: bool,
    pub packed_alignment_bytes: Option<u64>,
    pub requested_alignment_bytes: Option<u64>,
}

impl From<AdtRepresentationFacts> for RustcRepresentationV2 {
    fn from(value: AdtRepresentationFacts) -> Self {
        Self {
            c: value.c,
            transparent: value.transparent,
            explicit_integer: value.explicit_integer,
            packed_alignment_bytes: value.packed_alignment_bytes,
            requested_alignment_bytes: value.requested_alignment_bytes,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RustcByteRangeV2 {
    pub start: u64,
    pub end: u64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RustcAggregateLayoutV2 {
    path: String,
    source_to_memory: Vec<u32>,
    padding: Vec<RustcByteRangeV2>,
}

impl RustcAggregateLayoutV2 {
    pub fn path(&self) -> &str {
        &self.path
    }

    pub fn source_to_memory(&self) -> &[u32] {
        &self.source_to_memory
    }

    pub fn padding(&self) -> &[RustcByteRangeV2] {
        &self.padding
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RustcTypeLayoutRecordV2 {
    key: String,
    representation: Option<RustcRepresentationV2>,
    aggregates: Vec<RustcAggregateLayoutV2>,
    array_stride_bytes: Option<u64>,
    uninhabited: bool,
}

impl RustcTypeLayoutRecordV2 {
    pub fn key(&self) -> &str {
        &self.key
    }

    pub const fn representation(&self) -> Option<RustcRepresentationV2> {
        self.representation
    }

    pub fn aggregates(&self) -> &[RustcAggregateLayoutV2] {
        &self.aggregates
    }

    pub const fn array_stride_bytes(&self) -> Option<u64> {
        self.array_stride_bytes
    }

    pub const fn uninhabited(&self) -> bool {
        self.uninhabited
    }
}

#[derive(Clone, Debug)]
pub struct RustcTypeLayoutObservationV2 {
    rustc_target: SemanticLayoutTargetV1,
    graph: SemanticTypeGraphV2,
    graph_bytes: Vec<u8>,
    layout_records: Vec<RustcTypeLayoutRecordV2>,
    identity_sha256: [u8; 32],
}

impl RustcTypeLayoutObservationV2 {
    pub const fn rustc_target(&self) -> &SemanticLayoutTargetV1 {
        &self.rustc_target
    }

    pub const fn graph(&self) -> &SemanticTypeGraphV2 {
        &self.graph
    }

    pub fn graph_bytes(&self) -> &[u8] {
        &self.graph_bytes
    }

    pub fn layout_records(&self) -> &[RustcTypeLayoutRecordV2] {
        &self.layout_records
    }

    pub const fn identity_sha256(&self) -> &[u8; 32] {
        &self.identity_sha256
    }

    /// True only when the active rustc target is the canonical device target.
    /// Host x86 observations therefore cannot masquerade as gfx942 observations.
    pub fn was_observed_on_canonical_gfx942_target(&self) -> bool {
        self.rustc_target.llvm_target() == GFX942_TARGET_TRIPLE
            && self.rustc_target.data_layout() == GFX942_TARGET_DATA_LAYOUT
            && self.rustc_target.default_pointer_width_bits() == GFX942_POINTER_WIDTH_BITS
    }

    pub fn layout_record(&self, key: &str) -> Option<&RustcTypeLayoutRecordV2> {
        self.layout_records
            .binary_search_by(|record| record.key.as_str().cmp(key))
            .ok()
            .map(|index| &self.layout_records[index])
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Gfx942LayoutProjectionRecordV2 {
    key: String,
    size_bytes: u64,
    alignment_bytes: u64,
    field_offsets: Vec<u64>,
    array_stride_bytes: Option<u64>,
    padding: Vec<RustcByteRangeV2>,
}

impl Gfx942LayoutProjectionRecordV2 {
    pub fn key(&self) -> &str {
        &self.key
    }

    pub const fn size_bytes(&self) -> u64 {
        self.size_bytes
    }

    pub const fn alignment_bytes(&self) -> u64 {
        self.alignment_bytes
    }

    pub fn field_offsets(&self) -> &[u64] {
        &self.field_offsets
    }

    pub const fn array_stride_bytes(&self) -> Option<u64> {
        self.array_stride_bytes
    }

    pub fn padding(&self) -> &[RustcByteRangeV2] {
        &self.padding
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CanonicalGfx942LayoutProjectionV2 {
    target: CanonicalGfx942LayoutTargetV2,
    records: Vec<Gfx942LayoutProjectionRecordV2>,
    canonical_bytes: Vec<u8>,
    identity_sha256: [u8; 32],
}

impl CanonicalGfx942LayoutProjectionV2 {
    pub const fn target(&self) -> &CanonicalGfx942LayoutTargetV2 {
        &self.target
    }

    pub fn records(&self) -> &[Gfx942LayoutProjectionRecordV2] {
        &self.records
    }

    pub fn canonical_bytes(&self) -> &[u8] {
        &self.canonical_bytes
    }

    pub const fn identity_sha256(&self) -> &[u8; 32] {
        &self.identity_sha256
    }
}

/// Inert result of exact host-observation versus canonical-gfx942 comparison.
///
/// It does not implement or grant `DeviceCopy`, bytes, allocation, transfer,
/// artifact, load, launch, compiler-provenance, or proof authority.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Gfx942LayoutCompatibilityCandidateV2 {
    observation_identity_sha256: [u8; 32],
    projection: CanonicalGfx942LayoutProjectionV2,
    candidate_identity_sha256: [u8; 32],
    root_key: String,
}

impl Gfx942LayoutCompatibilityCandidateV2 {
    pub const fn observation_identity_sha256(&self) -> &[u8; 32] {
        &self.observation_identity_sha256
    }

    pub const fn projection(&self) -> &CanonicalGfx942LayoutProjectionV2 {
        &self.projection
    }

    pub const fn candidate_identity_sha256(&self) -> &[u8; 32] {
        &self.candidate_identity_sha256
    }

    pub fn root_key(&self) -> &str {
        &self.root_key
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HostDeviceByteDifferentialV2 {
    candidate_identity_sha256: [u8; 32],
    bytes_sha256: [u8; 32],
    byte_length: u64,
}

impl HostDeviceByteDifferentialV2 {
    pub const fn candidate_identity_sha256(&self) -> &[u8; 32] {
        &self.candidate_identity_sha256
    }

    pub const fn bytes_sha256(&self) -> &[u8; 32] {
        &self.bytes_sha256
    }

    pub const fn byte_length(&self) -> u64 {
        self.byte_length
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Gfx942LayoutCompatibilityErrorV2 {
    TargetMismatch,
    ObservationIdentityMismatch,
    ProjectionMismatch,
    Cycle { key: String },
    ArithmeticOverflow { key: String },
    WorkBoundExceeded { actual: u64, max: u64 },
    AllocationFailed { resource: &'static str },
    Unsupported { key: String, detail: &'static str },
    InconsistentLayout { key: String, detail: &'static str },
    ByteLengthExceeded { actual: usize, max: usize },
    HostDeviceByteMismatch,
}

impl fmt::Display for Gfx942LayoutCompatibilityErrorV2 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::TargetMismatch => {
                formatter.write_str("canonical gfx942 target identity mismatch")
            }
            Self::ObservationIdentityMismatch => {
                formatter.write_str("rustc layout observation identity mismatch")
            }
            Self::ProjectionMismatch => {
                formatter.write_str("canonical gfx942 layout projection mismatch")
            }
            Self::Cycle { key } => {
                write!(formatter, "layout projection contains a cycle at {key:?}")
            }
            Self::ArithmeticOverflow { key } => {
                write!(
                    formatter,
                    "layout projection arithmetic overflow at {key:?}"
                )
            }
            Self::WorkBoundExceeded { actual, max } => {
                write!(
                    formatter,
                    "layout projection work bound exceeded: {actual} > {max}"
                )
            }
            Self::AllocationFailed { resource } => {
                write!(formatter, "allocation failed while building {resource}")
            }
            Self::Unsupported { key, detail } => {
                write!(
                    formatter,
                    "type {key:?} is not gfx942-layout compatible: {detail}"
                )
            }
            Self::InconsistentLayout { key, detail } => {
                write!(
                    formatter,
                    "layout observation {key:?} is inconsistent: {detail}"
                )
            }
            Self::ByteLengthExceeded { actual, max } => {
                write!(
                    formatter,
                    "byte differential bound exceeded: {actual} > {max}"
                )
            }
            Self::HostDeviceByteMismatch => {
                formatter.write_str("host and gfx942 fixture bytes differ")
            }
        }
    }
}

impl std::error::Error for Gfx942LayoutCompatibilityErrorV2 {}

/// Derives and exactly compares the conservative gfx942 layout projection.
///
/// The returned value is an inert compatibility candidate, not an admission
/// token. Caller-declared compiler/source digests and generations are absent
/// from this API and from every identity it computes.
pub fn derive_gfx942_layout_compatibility_candidate_v2(
    observation: &RustcTypeLayoutObservationV2,
    budgets: SemanticTypeLayoutBudgetsV2,
) -> Result<Gfx942LayoutCompatibilityCandidateV2, Gfx942LayoutCompatibilityErrorV2> {
    validate_observation_integrity(observation, budgets)?;
    let target = CanonicalGfx942LayoutTargetV2::canonical();
    let mut builder = ProjectionBuilderV2::new(observation, budgets)?;
    builder.derive(observation.graph().root())?;
    let projection = builder.finish(target)?;
    let candidate_identity_sha256 = candidate_identity(
        observation.identity_sha256(),
        projection.identity_sha256(),
        projection.canonical_bytes(),
    );
    Ok(Gfx942LayoutCompatibilityCandidateV2 {
        observation_identity_sha256: *observation.identity_sha256(),
        projection,
        candidate_identity_sha256,
        root_key: observation.graph().root_key().to_owned(),
    })
}

/// Re-derives every byte of a candidate before accepting it as current.
pub fn validate_gfx942_layout_compatibility_candidate_v2(
    observation: &RustcTypeLayoutObservationV2,
    candidate: &Gfx942LayoutCompatibilityCandidateV2,
    budgets: SemanticTypeLayoutBudgetsV2,
) -> Result<(), Gfx942LayoutCompatibilityErrorV2> {
    let expected = derive_gfx942_layout_compatibility_candidate_v2(observation, budgets)?;
    if candidate != &expected {
        return Err(Gfx942LayoutCompatibilityErrorV2::ProjectionMismatch);
    }
    Ok(())
}

pub fn compare_host_device_fixture_bytes_v2(
    candidate: &Gfx942LayoutCompatibilityCandidateV2,
    host_bytes: &[u8],
    gfx942_bytes: &[u8],
    max_bytes: usize,
) -> Result<HostDeviceByteDifferentialV2, Gfx942LayoutCompatibilityErrorV2> {
    let actual = host_bytes.len().max(gfx942_bytes.len());
    if actual > max_bytes {
        return Err(Gfx942LayoutCompatibilityErrorV2::ByteLengthExceeded {
            actual,
            max: max_bytes,
        });
    }
    if host_bytes != gfx942_bytes {
        return Err(Gfx942LayoutCompatibilityErrorV2::HostDeviceByteMismatch);
    }
    Ok(HostDeviceByteDifferentialV2 {
        candidate_identity_sha256: *candidate.candidate_identity_sha256(),
        bytes_sha256: Sha256::digest(host_bytes).into(),
        byte_length: host_bytes.len() as u64,
    })
}

#[derive(Clone)]
struct DerivedLayoutV2 {
    size_bytes: u64,
    alignment_bytes: u64,
    field_offsets: Vec<u64>,
    array_stride_bytes: Option<u64>,
    padding: Vec<RustcByteRangeV2>,
}

struct ProjectionBuilderV2<'a> {
    observation: &'a RustcTypeLayoutObservationV2,
    budgets: SemanticTypeLayoutBudgetsV2,
    work: u64,
    active: Vec<bool>,
    derived: Vec<Option<DerivedLayoutV2>>,
    records: Vec<Gfx942LayoutProjectionRecordV2>,
}

impl<'a> ProjectionBuilderV2<'a> {
    fn new(
        observation: &'a RustcTypeLayoutObservationV2,
        budgets: SemanticTypeLayoutBudgetsV2,
    ) -> Result<Self, Gfx942LayoutCompatibilityErrorV2> {
        let count = observation.graph().node_count();
        let mut active = Vec::new();
        active.try_reserve_exact(count).map_err(|_| {
            Gfx942LayoutCompatibilityErrorV2::AllocationFailed {
                resource: "gfx942 projection traversal state",
            }
        })?;
        active.resize(count, false);
        let mut derived = Vec::new();
        derived.try_reserve_exact(count).map_err(|_| {
            Gfx942LayoutCompatibilityErrorV2::AllocationFailed {
                resource: "gfx942 derived layouts",
            }
        })?;
        derived.resize_with(count, || None);
        let mut records = Vec::new();
        records.try_reserve_exact(count).map_err(|_| {
            Gfx942LayoutCompatibilityErrorV2::AllocationFailed {
                resource: "gfx942 projection records",
            }
        })?;
        Ok(Self {
            observation,
            budgets,
            work: 0,
            active,
            derived,
            records,
        })
    }

    fn charge(&mut self, amount: u64) -> Result<(), Gfx942LayoutCompatibilityErrorV2> {
        self.work = self.work.checked_add(amount).unwrap_or(u64::MAX);
        if self.work > self.budgets.max_projection_work {
            return Err(Gfx942LayoutCompatibilityErrorV2::WorkBoundExceeded {
                actual: self.work,
                max: self.budgets.max_projection_work,
            });
        }
        Ok(())
    }

    fn derive(
        &mut self,
        id: SemanticTypeNodeIdV2,
    ) -> Result<DerivedLayoutV2, Gfx942LayoutCompatibilityErrorV2> {
        self.charge(1)?;
        let index = id.index() as usize;
        if let Some(layout) = self.derived.get(index).and_then(Clone::clone) {
            return Ok(layout);
        }
        let key = self
            .observation
            .graph()
            .key(id)
            .ok_or_else(|| inconsistent_layout(id.index().to_string(), "node key is missing"))?
            .to_owned();
        if self.active.get(index).copied().unwrap_or(false) {
            return Err(Gfx942LayoutCompatibilityErrorV2::Cycle { key });
        }
        let node = self
            .observation
            .graph()
            .node(id)
            .ok_or_else(|| inconsistent_layout(key.clone(), "node definition is missing"))?
            .clone();
        let record = self
            .observation
            .layout_record(&key)
            .ok_or_else(|| inconsistent_layout(key.clone(), "rustc observation record is missing"))?
            .clone();
        if record.uninhabited() {
            return Err(layout_unsupported(
                &key,
                "uninhabited values are not byte-copy values",
            ));
        }
        self.active[index] = true;
        let layout = self.derive_kind(&key, &node.kind, &record)?;
        self.active[index] = false;
        if node.layout.size != Some(layout.size_bytes)
            || node.layout.align != layout.alignment_bytes
        {
            return Err(inconsistent_layout(
                key,
                "host rustc size/alignment differs from canonical gfx942 projection",
            ));
        }
        let projection_record = Gfx942LayoutProjectionRecordV2 {
            key: key.clone(),
            size_bytes: layout.size_bytes,
            alignment_bytes: layout.alignment_bytes,
            field_offsets: layout.field_offsets.clone(),
            array_stride_bytes: layout.array_stride_bytes,
            padding: layout.padding.clone(),
        };
        self.derived[index] = Some(layout.clone());
        self.records.push(projection_record);
        Ok(layout)
    }

    fn derive_kind(
        &mut self,
        key: &str,
        kind: &SemanticTypeKindV2,
        record: &RustcTypeLayoutRecordV2,
    ) -> Result<DerivedLayoutV2, Gfx942LayoutCompatibilityErrorV2> {
        match kind {
            SemanticTypeKindV2::Unit => Ok(simple_layout(0, 1)),
            SemanticTypeKindV2::Scalar(SemanticScalarV2::Int { bits, .. }) => {
                let (size, align) = match bits {
                    8 => (1, 1),
                    16 => (2, 2),
                    32 => (4, 4),
                    64 => (8, 8),
                    _ => {
                        return Err(layout_unsupported(
                            key,
                            "integer width lacks a reviewed gfx942 ABI rule",
                        ));
                    }
                };
                Ok(simple_layout(size, align))
            }
            SemanticTypeKindV2::Scalar(SemanticScalarV2::Float { bits }) => {
                let (size, align) = match bits {
                    32 => (4, 4),
                    64 => (8, 8),
                    _ => {
                        return Err(layout_unsupported(
                            key,
                            "float width lacks a reviewed gfx942 ABI rule",
                        ));
                    }
                };
                Ok(simple_layout(size, align))
            }
            SemanticTypeKindV2::Scalar(_) | SemanticTypeKindV2::ValidityScalar { .. } => Err(
                layout_unsupported(key, "not every scalar bit pattern is valid or supported"),
            ),
            SemanticTypeKindV2::Array { element, length } => {
                let element = self.derive(*element)?;
                if element.size_bytes == 0 {
                    return Err(layout_unsupported(
                        key,
                        "zero-sized array elements are outside the reviewed ABI subset",
                    ));
                }
                let stride = align_up(element.size_bytes, element.alignment_bytes, key)?;
                let size = stride.checked_mul(*length).ok_or_else(|| {
                    Gfx942LayoutCompatibilityErrorV2::ArithmeticOverflow {
                        key: key.to_owned(),
                    }
                })?;
                if record.array_stride_bytes() != Some(stride) {
                    return Err(inconsistent_layout(
                        key,
                        "host array stride differs from canonical gfx942 stride",
                    ));
                }
                Ok(DerivedLayoutV2 {
                    size_bytes: size,
                    alignment_bytes: element.alignment_bytes,
                    field_offsets: Vec::new(),
                    array_stride_bytes: Some(stride),
                    padding: Vec::new(),
                })
            }
            SemanticTypeKindV2::Struct { fields, .. } => self.derive_struct(key, fields, record),
            SemanticTypeKindV2::Tuple { .. } => Err(layout_unsupported(
                key,
                "tuple repr(Rust) ABI is not admitted",
            )),
            SemanticTypeKindV2::Union { .. } => Err(layout_unsupported(
                key,
                "union object representation is not admitted",
            )),
            SemanticTypeKindV2::Enum { .. } => Err(layout_unsupported(
                key,
                "enum discriminants and niches are not admitted",
            )),
            SemanticTypeKindV2::RawPointer { .. } | SemanticTypeKindV2::Reference { .. } => {
                Err(layout_unsupported(
                    key,
                    "pointer provenance and address spaces are not byte-copy layout facts",
                ))
            }
            SemanticTypeKindV2::Never => Err(layout_unsupported(
                key,
                "never has no inhabited object representation",
            )),
            SemanticTypeKindV2::Slice { .. }
            | SemanticTypeKindV2::Str
            | SemanticTypeKindV2::OpaqueDst { .. } => Err(layout_unsupported(
                key,
                "dynamically sized values are not copied by value",
            )),
        }
    }

    fn derive_struct(
        &mut self,
        key: &str,
        fields: &[SemanticFieldV2],
        record: &RustcTypeLayoutRecordV2,
    ) -> Result<DerivedLayoutV2, Gfx942LayoutCompatibilityErrorV2> {
        let repr = record
            .representation()
            .ok_or_else(|| inconsistent_layout(key, "struct representation is missing"))?;
        if repr.packed_alignment_bytes.is_some()
            || repr.requested_alignment_bytes.is_some()
            || repr.explicit_integer
        {
            return Err(layout_unsupported(
                key,
                "packed, explicitly aligned, or integer representations are not admitted",
            ));
        }
        let aggregate = match record.aggregates() {
            [aggregate] => aggregate,
            _ => {
                return Err(inconsistent_layout(
                    key,
                    "struct must have one rustc aggregate record",
                ));
            }
        };
        if !aggregate.padding().is_empty() {
            return Err(layout_unsupported(
                key,
                "aggregate object representation contains padding",
            ));
        }
        if repr.transparent {
            if repr.c || fields.len() != 1 {
                return Err(layout_unsupported(
                    key,
                    "the reviewed repr(transparent) subset requires exactly one field",
                ));
            }
            let child = self.derive(fields[0].ty)?;
            if child.size_bytes == 0 || fields[0].offset != 0 || aggregate.source_to_memory() != [0]
            {
                return Err(inconsistent_layout(
                    key,
                    "repr(transparent) host layout differs from its single field",
                ));
            }
            return Ok(DerivedLayoutV2 {
                size_bytes: child.size_bytes,
                alignment_bytes: child.alignment_bytes,
                field_offsets: vec![0],
                array_stride_bytes: None,
                padding: Vec::new(),
            });
        }
        if !repr.c {
            return Err(layout_unsupported(
                key,
                "repr(Rust) aggregate ABI is not admitted",
            ));
        }
        let mut offsets = Vec::new();
        offsets.try_reserve_exact(fields.len()).map_err(|_| {
            Gfx942LayoutCompatibilityErrorV2::AllocationFailed {
                resource: "gfx942 field offsets",
            }
        })?;
        let mut cursor = 0_u64;
        let mut alignment = 1_u64;
        for (index, field) in fields.iter().enumerate() {
            self.charge(1)?;
            let child = self.derive(field.ty)?;
            if child.size_bytes == 0 {
                return Err(layout_unsupported(
                    key,
                    "zero-sized aggregate fields are outside the reviewed ABI subset",
                ));
            }
            let offset = align_up(cursor, child.alignment_bytes, key)?;
            if offset != cursor {
                return Err(layout_unsupported(
                    key,
                    "canonical gfx942 aggregate layout contains internal padding",
                ));
            }
            if field.offset != offset {
                return Err(inconsistent_layout(
                    key,
                    "host field offset differs from canonical gfx942 offset",
                ));
            }
            if aggregate.source_to_memory().get(index).copied() != Some(index as u32) {
                return Err(inconsistent_layout(
                    key,
                    "host field memory order differs from repr(C) source order",
                ));
            }
            offsets.push(offset);
            cursor = cursor.checked_add(child.size_bytes).ok_or_else(|| {
                Gfx942LayoutCompatibilityErrorV2::ArithmeticOverflow {
                    key: key.to_owned(),
                }
            })?;
            alignment = alignment.max(child.alignment_bytes);
        }
        let size = align_up(cursor, alignment, key)?;
        if size != cursor {
            return Err(layout_unsupported(
                key,
                "canonical gfx942 aggregate layout contains trailing padding",
            ));
        }
        Ok(DerivedLayoutV2 {
            size_bytes: size,
            alignment_bytes: alignment,
            field_offsets: offsets,
            array_stride_bytes: None,
            padding: Vec::new(),
        })
    }

    fn finish(
        mut self,
        target: CanonicalGfx942LayoutTargetV2,
    ) -> Result<CanonicalGfx942LayoutProjectionV2, Gfx942LayoutCompatibilityErrorV2> {
        if !target.is_canonical() {
            return Err(Gfx942LayoutCompatibilityErrorV2::TargetMismatch);
        }
        self.charge(sort_work(self.records.len()))?;
        self.records.sort_by(|left, right| left.key.cmp(&right.key));
        let canonical_bytes =
            encode_projection(&target, &self.records, self.budgets.max_sidecar_bytes)?;
        let identity_sha256 = projection_identity(&canonical_bytes);
        Ok(CanonicalGfx942LayoutProjectionV2 {
            target,
            records: self.records,
            canonical_bytes,
            identity_sha256,
        })
    }
}

fn simple_layout(size_bytes: u64, alignment_bytes: u64) -> DerivedLayoutV2 {
    DerivedLayoutV2 {
        size_bytes,
        alignment_bytes,
        field_offsets: Vec::new(),
        array_stride_bytes: None,
        padding: Vec::new(),
    }
}

fn align_up(
    value: u64,
    alignment: u64,
    key: &str,
) -> Result<u64, Gfx942LayoutCompatibilityErrorV2> {
    if alignment == 0 || !alignment.is_power_of_two() {
        return Err(inconsistent_layout(
            key,
            "canonical alignment is not a nonzero power of two",
        ));
    }
    value
        .checked_add(alignment - 1)
        .map(|value| value & !(alignment - 1))
        .ok_or_else(|| Gfx942LayoutCompatibilityErrorV2::ArithmeticOverflow {
            key: key.to_owned(),
        })
}

fn sort_work(length: usize) -> u64 {
    if length < 2 {
        return length as u64;
    }
    (length as u64).saturating_mul(u64::from(usize::BITS - (length - 1).leading_zeros()))
}

fn inconsistent_layout(
    key: impl Into<String>,
    detail: &'static str,
) -> Gfx942LayoutCompatibilityErrorV2 {
    Gfx942LayoutCompatibilityErrorV2::InconsistentLayout {
        key: key.into(),
        detail,
    }
}

fn layout_unsupported(key: &str, detail: &'static str) -> Gfx942LayoutCompatibilityErrorV2 {
    Gfx942LayoutCompatibilityErrorV2::Unsupported {
        key: key.to_owned(),
        detail,
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SemanticTypeAdapterErrorV2 {
    TargetMismatch {
        expected: SemanticLayoutTargetV1,
        observed: SemanticLayoutTargetV1,
    },
    Extraction(String),
    Unsupported {
        path: String,
        detail: &'static str,
    },
    Inconsistent {
        path: String,
        detail: String,
    },
    BoundExceeded {
        resource: &'static str,
        actual: u64,
        max: u64,
    },
    AllocationFailed {
        resource: &'static str,
    },
    Graph(SemanticTypeGraphErrorV2),
    UntrustedGraphMismatch,
}

impl fmt::Display for SemanticTypeAdapterErrorV2 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::TargetMismatch { expected, observed } => {
                write!(
                    formatter,
                    "rustc target mismatch: expected {expected:?}, observed {observed:?}"
                )
            }
            Self::Extraction(detail) => write!(formatter, "rustc type extraction failed: {detail}"),
            Self::Unsupported { path, detail } => {
                write!(formatter, "unsupported rustc type fact at {path}: {detail}")
            }
            Self::Inconsistent { path, detail } => {
                write!(
                    formatter,
                    "inconsistent rustc type fact at {path}: {detail}"
                )
            }
            Self::BoundExceeded {
                resource,
                actual,
                max,
            } => {
                write!(formatter, "{resource} bound exceeded: {actual} > {max}")
            }
            Self::AllocationFailed { resource } => {
                write!(formatter, "allocation failed while building {resource}")
            }
            Self::Graph(error) => {
                write!(
                    formatter,
                    "semantic type graph rejected observation: {error}"
                )
            }
            Self::UntrustedGraphMismatch => formatter.write_str(
                "untrusted semantic type graph differs from the exact rustc observation",
            ),
        }
    }
}

impl std::error::Error for SemanticTypeAdapterErrorV2 {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Graph(error) => Some(error),
            _ => None,
        }
    }
}

impl From<SemanticTypeGraphErrorV2> for SemanticTypeAdapterErrorV2 {
    fn from(value: SemanticTypeGraphErrorV2) -> Self {
        Self::Graph(value)
    }
}

impl From<GeneralLayoutExtractError> for SemanticTypeAdapterErrorV2 {
    fn from(value: GeneralLayoutExtractError) -> Self {
        Self::Extraction(value.to_string())
    }
}

impl From<SemanticLayoutBridgeError> for SemanticTypeAdapterErrorV2 {
    fn from(value: SemanticLayoutBridgeError) -> Self {
        Self::Extraction(value.to_string())
    }
}

struct ObservationMeterV2 {
    work: u64,
    text_bytes: u64,
    max_work: u64,
    max_text_bytes: u64,
}

impl ObservationMeterV2 {
    fn new(budgets: SemanticTypeLayoutBudgetsV2) -> Self {
        Self {
            work: 0,
            text_bytes: 0,
            max_work: budgets.max_observation_work,
            max_text_bytes: budgets.max_total_text_bytes,
        }
    }

    fn charge_work(
        &mut self,
        resource: &'static str,
        amount: u64,
    ) -> Result<(), SemanticTypeAdapterErrorV2> {
        self.work = self.work.checked_add(amount).unwrap_or(u64::MAX);
        if self.work > self.max_work {
            return Err(SemanticTypeAdapterErrorV2::BoundExceeded {
                resource,
                actual: self.work,
                max: self.max_work,
            });
        }
        Ok(())
    }

    fn charge_text(&mut self, amount: usize) -> Result<(), SemanticTypeAdapterErrorV2> {
        self.text_bytes = self
            .text_bytes
            .checked_add(amount as u64)
            .unwrap_or(u64::MAX);
        if self.text_bytes > self.max_text_bytes {
            return Err(SemanticTypeAdapterErrorV2::BoundExceeded {
                resource: "rustc layout total text bytes",
                actual: self.text_bytes,
                max: self.max_text_bytes,
            });
        }
        Ok(())
    }
}

fn normalize_and_preflight_layout_type<'tcx>(
    tcx: TyCtxt<'tcx>,
    ty: Ty<'tcx>,
    budgets: SemanticTypeLayoutBudgetsV2,
) -> Result<(Ty<'tcx>, ObservationMeterV2), SemanticTypeAdapterErrorV2> {
    const EXTRACTION_MAX_DEPTH: u64 = 64;
    const MAX_PATH_SEGMENT_BYTES: u64 = 48;
    let required_path_bytes = 4_u64
        .checked_add(EXTRACTION_MAX_DEPTH.saturating_mul(MAX_PATH_SEGMENT_BYTES))
        .unwrap_or(u64::MAX);
    if required_path_bytes > u64::from(budgets.max_path_bytes) {
        return Err(SemanticTypeAdapterErrorV2::BoundExceeded {
            resource: "rustc layout path bytes",
            actual: required_path_bytes,
            max: u64::from(budgets.max_path_bytes),
        });
    }

    let typing_env = TypingEnv::fully_monomorphized();
    let normalized = tcx
        .try_normalize_erasing_regions(typing_env, ty)
        .map_err(|_| {
            SemanticTypeAdapterErrorV2::Extraction("type normalization failed".to_owned())
        })?;
    let max_nodes = budgets.graph.max_nodes as usize;
    let mut stack = Vec::new();
    stack.try_reserve(max_nodes.min(64)).map_err(|_| {
        SemanticTypeAdapterErrorV2::AllocationFailed {
            resource: "rustc layout preflight stack",
        }
    })?;
    stack.push((normalized, 0_u32));
    let mut seen = HashSet::new();
    seen.try_reserve(max_nodes.min(64)).map_err(|_| {
        SemanticTypeAdapterErrorV2::AllocationFailed {
            resource: "rustc layout preflight visited set",
        }
    })?;
    let mut meter = ObservationMeterV2::new(budgets);
    let mut total_fields = 0_u64;
    let mut total_variants = 0_u64;

    while let Some((current, depth)) = stack.pop() {
        meter.charge_work("rustc layout preflight work", 1)?;
        if depth > EXTRACTION_MAX_DEPTH as u32 {
            return Err(SemanticTypeAdapterErrorV2::BoundExceeded {
                resource: "rustc layout depth",
                actual: u64::from(depth),
                max: EXTRACTION_MAX_DEPTH,
            });
        }
        if seen.contains(&current) {
            continue;
        }
        if seen.len() >= max_nodes {
            return Err(SemanticTypeAdapterErrorV2::BoundExceeded {
                resource: "rustc layout preflight nodes",
                actual: seen.len().saturating_add(1) as u64,
                max: max_nodes as u64,
            });
        }
        seen.try_reserve(1)
            .map_err(|_| SemanticTypeAdapterErrorV2::AllocationFailed {
                resource: "rustc layout preflight visited set",
            })?;
        seen.insert(current);
        let name = bounded_type_name(current, budgets.graph.max_name_bytes)?;
        meter.charge_text(name.len())?;
        let child_depth =
            depth
                .checked_add(1)
                .ok_or(SemanticTypeAdapterErrorV2::BoundExceeded {
                    resource: "rustc layout depth",
                    actual: u64::MAX,
                    max: EXTRACTION_MAX_DEPTH,
                })?;
        match *current.kind() {
            TyKind::Array(element, _)
            | TyKind::Slice(element)
            | TyKind::RawPtr(element, _)
            | TyKind::Ref(_, element, _)
            | TyKind::Pat(element, _) => push_preflight_type(&mut stack, element, child_depth)?,
            TyKind::Tuple(elements) => {
                meter.charge_work("rustc layout preflight work", elements.len() as u64)?;
                for element in elements.iter().rev() {
                    push_preflight_type(&mut stack, element, child_depth)?;
                }
            }
            TyKind::Adt(definition, arguments) => {
                let variants = definition.variants();
                total_variants = total_variants
                    .checked_add(variants.len() as u64)
                    .unwrap_or(u64::MAX);
                if total_variants > u64::from(budgets.graph.max_variants) {
                    return Err(SemanticTypeAdapterErrorV2::BoundExceeded {
                        resource: "rustc layout preflight variants",
                        actual: total_variants,
                        max: u64::from(budgets.graph.max_variants),
                    });
                }
                for variant in variants.iter().rev() {
                    meter.charge_text(variant.name.as_str().len())?;
                    total_fields = total_fields
                        .checked_add(variant.fields.len() as u64)
                        .unwrap_or(u64::MAX);
                    if total_fields > u64::from(budgets.graph.max_fields) {
                        return Err(SemanticTypeAdapterErrorV2::BoundExceeded {
                            resource: "rustc layout preflight fields",
                            actual: total_fields,
                            max: u64::from(budgets.graph.max_fields),
                        });
                    }
                    meter
                        .charge_work("rustc layout preflight work", variant.fields.len() as u64)?;
                    for field in variant.fields.iter().rev() {
                        meter.charge_text(field.name.as_str().len())?;
                        let field_ty = field.ty(tcx, arguments);
                        let field_ty = tcx
                            .try_normalize_erasing_regions(typing_env, field_ty)
                            .map_err(|_| {
                                SemanticTypeAdapterErrorV2::Extraction(
                                    "field type normalization failed".to_owned(),
                                )
                            })?;
                        push_preflight_type(&mut stack, field_ty, child_depth)?;
                    }
                }
            }
            _ => {}
        }
    }
    Ok((normalized, meter))
}

fn push_preflight_type<'tcx>(
    stack: &mut Vec<(Ty<'tcx>, u32)>,
    ty: Ty<'tcx>,
    depth: u32,
) -> Result<(), SemanticTypeAdapterErrorV2> {
    stack
        .try_reserve(1)
        .map_err(|_| SemanticTypeAdapterErrorV2::AllocationFailed {
            resource: "rustc layout preflight stack",
        })?;
    stack.push((ty, depth));
    Ok(())
}

fn charge_observation_result(
    meter: &mut ObservationMeterV2,
    observation: &PendingObservationV2,
) -> Result<(), SemanticTypeAdapterErrorV2> {
    meter.charge_work(
        "rustc layout observation work",
        observation.graph.node_count() as u64,
    )?;
    meter.charge_work(
        "rustc layout sidecar sorting work",
        sort_work(observation.layout_records.len()),
    )?;
    for record in &observation.layout_records {
        meter.charge_work("rustc layout sidecar work", 1)?;
        meter.charge_text(record.key.len())?;
        for aggregate in &record.aggregates {
            meter.charge_work(
                "rustc layout sidecar work",
                1_u64
                    .saturating_add(aggregate.source_to_memory.len() as u64)
                    .saturating_add(aggregate.padding.len() as u64),
            )?;
            meter.charge_text(aggregate.path.len())?;
        }
    }
    Ok(())
}

/// Observes a type layout under the exact active rustc target.
///
/// This result is inert and is not compiler/source provenance or freshness
/// evidence. It is a gfx942-target observation only when the active rustc
/// target itself exactly equals the canonical gfx942 triple, data layout, and
/// pointer width.
pub fn observe_rustc_type_layout_v2<'tcx>(
    tcx: TyCtxt<'tcx>,
    ty: Ty<'tcx>,
    expected_rustc_target: &SemanticLayoutTargetV1,
    budgets: SemanticTypeLayoutBudgetsV2,
) -> Result<RustcTypeLayoutObservationV2, SemanticTypeAdapterErrorV2> {
    let observed = rustc_semantic_layout_target_v1(tcx)?;
    if expected_rustc_target != &observed {
        return Err(SemanticTypeAdapterErrorV2::TargetMismatch {
            expected: expected_rustc_target.clone(),
            observed,
        });
    }
    let (ty, mut meter) = normalize_and_preflight_layout_type(tcx, ty, budgets)?;

    let mut capture = if is_unsized_pointer(ty, tcx) {
        observe_unsized_pointer_layout(tcx, ty, budgets)?
    } else {
        let facts = extract_general_layout_with_limits(
            tcx,
            ty,
            ExtractionLimits {
                max_depth: 64,
                max_nodes: usize::try_from(budgets.graph.max_nodes).unwrap_or(usize::MAX),
                max_fields_per_aggregate: usize::try_from(budgets.graph.max_fields)
                    .unwrap_or(usize::MAX),
                max_variants: usize::try_from(budgets.graph.max_variants).unwrap_or(usize::MAX),
                max_array_elements: 1 << 24,
            },
        )?;
        ObservationBuilderV2::new(budgets)?.finish(&facts)?
    };
    charge_observation_result(&mut meter, &capture)?;
    capture
        .layout_records
        .sort_by(|left, right| left.key.cmp(&right.key));
    let sidecar = encode_sidecar(&capture.layout_records, budgets.max_sidecar_bytes)?;
    let identity_sha256 =
        observation_identity(expected_rustc_target, &capture.graph_bytes, &sidecar);
    Ok(RustcTypeLayoutObservationV2 {
        rustc_target: expected_rustc_target.clone(),
        graph: capture.graph,
        graph_bytes: capture.graph_bytes,
        layout_records: capture.layout_records,
        identity_sha256,
    })
}

pub fn compare_untrusted_graph_to_rustc_observation_v2<'tcx>(
    tcx: TyCtxt<'tcx>,
    ty: Ty<'tcx>,
    untrusted_graph_bytes: &[u8],
    expected_rustc_target: &SemanticLayoutTargetV1,
    budgets: SemanticTypeLayoutBudgetsV2,
) -> Result<RustcTypeLayoutObservationV2, SemanticTypeAdapterErrorV2> {
    let decoded = SemanticTypeGraphV2::decode_canonical(untrusted_graph_bytes, budgets.graph)?;
    let canonical = decoded.canonical_bytes()?;
    let observation = observe_rustc_type_layout_v2(tcx, ty, expected_rustc_target, budgets)?;
    if canonical != observation.graph_bytes || untrusted_graph_bytes != observation.graph_bytes {
        return Err(SemanticTypeAdapterErrorV2::UntrustedGraphMismatch);
    }
    Ok(observation)
}

struct PendingObservationV2 {
    graph: SemanticTypeGraphV2,
    graph_bytes: Vec<u8>,
    layout_records: Vec<RustcTypeLayoutRecordV2>,
}

struct ObservationBuilderV2 {
    budgets: SemanticTypeLayoutBudgetsV2,
    graph: SemanticTypeGraphBuilderV2,
    by_type: HashMap<String, SemanticTypeNodeIdV2>,
    records: Vec<RustcTypeLayoutRecordV2>,
}

impl ObservationBuilderV2 {
    fn new(budgets: SemanticTypeLayoutBudgetsV2) -> Result<Self, SemanticTypeAdapterErrorV2> {
        let initial = (budgets.max_sidecar_records as usize).min(64);
        let mut by_type = HashMap::new();
        by_type
            .try_reserve(initial)
            .map_err(|_| SemanticTypeAdapterErrorV2::AllocationFailed {
                resource: "rustc layout type intern table",
            })?;
        let mut records = Vec::new();
        records
            .try_reserve(initial)
            .map_err(|_| SemanticTypeAdapterErrorV2::AllocationFailed {
                resource: "rustc layout records",
            })?;
        Ok(Self {
            budgets,
            graph: SemanticTypeGraphBuilderV2::new(budgets.graph),
            by_type,
            records,
        })
    }

    fn finish(
        mut self,
        root: &TypeLayoutFacts,
    ) -> Result<PendingObservationV2, SemanticTypeAdapterErrorV2> {
        let root = self.intern(root, "root")?;
        let graph = self.graph.finish(root)?;
        let graph_bytes = graph.canonical_bytes()?;
        Ok(PendingObservationV2 {
            graph,
            graph_bytes,
            layout_records: self.records,
        })
    }

    fn intern(
        &mut self,
        facts: &TypeLayoutFacts,
        path: &str,
    ) -> Result<SemanticTypeNodeIdV2, SemanticTypeAdapterErrorV2> {
        if let Some(id) = self.by_type.get(&facts.rust_type) {
            return Ok(*id);
        }
        self.reserve_record()?;
        let id = self.graph.declare(facts.rust_type.clone())?;
        self.by_type.insert(facts.rust_type.clone(), id);
        let (kind, record) = self.convert_kind(facts, path)?;
        self.graph.define(
            id,
            SemanticTypeNodeV2 {
                layout: SemanticTypeLayoutV2::sized(facts.size_bytes, facts.abi_alignment_bytes),
                kind,
            },
        )?;
        self.records.push(record);
        Ok(id)
    }

    fn reserve_record(&mut self) -> Result<(), SemanticTypeAdapterErrorV2> {
        let actual = self.records.len() as u64 + 1;
        let max = u64::from(self.budgets.max_sidecar_records);
        if actual > max {
            return Err(SemanticTypeAdapterErrorV2::BoundExceeded {
                resource: "rustc layout sidecar records",
                actual,
                max,
            });
        }
        self.records
            .try_reserve(1)
            .map_err(|_| SemanticTypeAdapterErrorV2::AllocationFailed {
                resource: "rustc layout records",
            })?;
        self.by_type
            .try_reserve(1)
            .map_err(|_| SemanticTypeAdapterErrorV2::AllocationFailed {
                resource: "rustc layout type intern table",
            })?;
        Ok(())
    }

    fn convert_kind(
        &mut self,
        facts: &TypeLayoutFacts,
        path: &str,
    ) -> Result<(SemanticTypeKindV2, RustcTypeLayoutRecordV2), SemanticTypeAdapterErrorV2> {
        let mut record = RustcTypeLayoutRecordV2 {
            key: facts.rust_type.clone(),
            representation: None,
            aggregates: Vec::new(),
            array_stride_bytes: None,
            uninhabited: facts.uninhabited,
        };
        if facts.uninhabited {
            if let TypeLayoutKind::Adt(adt) = &facts.kind
                && adt.kind == AdtKind::Enum
                && adt.variants.is_empty()
                && facts.size_bytes == 0
                && facts.abi_alignment_bytes == 1
            {
                record.representation = Some(adt.representation.into());
                return Ok((
                    SemanticTypeKindV2::Enum {
                        identity: facts.rust_type.clone(),
                        discriminant: SemanticScalarV2::Int {
                            signed: false,
                            bits: 8,
                        },
                        encoding: SemanticEnumEncodingV2::Uninhabited,
                        variants: Vec::new(),
                    },
                    record,
                ));
            }
            return Err(unsupported(
                path,
                "uninhabited type is not represented by this observation profile",
            ));
        }
        let kind = match &facts.kind {
            TypeLayoutKind::Scalar(source) => scalar_kind(facts, *source, path)?,
            TypeLayoutKind::Pointer(pointer) => {
                let pointee = self.intern(&pointer.pointee, &format!("{path}.pointee"))?;
                let (data_pointer_bytes, address_space) = pointer_storage(facts, path)?;
                let mutability = match pointer.kind {
                    PointerKind::SharedReference | PointerKind::ConstRaw => {
                        SemanticMutabilityV2::Immutable
                    }
                    PointerKind::MutableReference | PointerKind::MutRaw => {
                        SemanticMutabilityV2::Mutable
                    }
                };
                match pointer.kind {
                    PointerKind::SharedReference | PointerKind::MutableReference => {
                        SemanticTypeKindV2::Reference {
                            referent: pointee,
                            mutability,
                            address_space,
                            data_pointer_bytes,
                            metadata: PointerMetadataV2::None,
                        }
                    }
                    PointerKind::ConstRaw | PointerKind::MutRaw => SemanticTypeKindV2::RawPointer {
                        pointee,
                        mutability,
                        address_space,
                        data_pointer_bytes,
                        metadata: PointerMetadataV2::None,
                    },
                }
            }
            TypeLayoutKind::Array(array) => {
                let element = self.intern(&array.element, &format!("{path}.element"))?;
                if array.stride_bytes != array.element.size_bytes {
                    return Err(unsupported(
                        path,
                        "array stride differs from the observed element size",
                    ));
                }
                record.array_stride_bytes = Some(array.stride_bytes);
                SemanticTypeKindV2::Array {
                    element,
                    length: array.length,
                }
            }
            TypeLayoutKind::Tuple(fields) if fields.is_empty() && facts.rust_type == "()" => {
                SemanticTypeKindV2::Unit
            }
            TypeLayoutKind::Tuple(fields) => {
                let semantic_fields = self.convert_fields(fields, path, false)?;
                push_aggregate_record(
                    &mut record,
                    aggregate_record(path, fields, facts.size_bytes, &[])?,
                )?;
                SemanticTypeKindV2::Tuple {
                    fields: semantic_fields,
                }
            }
            TypeLayoutKind::Adt(adt) => {
                record.representation = Some(adt.representation.into());
                self.convert_adt(facts, adt, path, &mut record)?
            }
        };
        Ok((kind, record))
    }

    fn convert_fields(
        &mut self,
        fields: &[FieldLayoutFacts],
        path: &str,
        require_names: bool,
    ) -> Result<Vec<SemanticFieldV2>, SemanticTypeAdapterErrorV2> {
        let mut converted = Vec::new();
        converted.try_reserve_exact(fields.len()).map_err(|_| {
            SemanticTypeAdapterErrorV2::AllocationFailed {
                resource: "semantic aggregate fields",
            }
        })?;
        for (index, field) in fields.iter().enumerate() {
            if field.source_index != index || (require_names && field.name.is_none()) {
                return Err(inconsistent(
                    path,
                    "field source order or name is inconsistent",
                ));
            }
            converted.push(SemanticFieldV2 {
                name: field.name.clone(),
                offset: field.offset_bytes,
                ty: self.intern(&field.layout, &format!("{path}.field[{index}]"))?,
            });
        }
        Ok(converted)
    }

    fn convert_adt(
        &mut self,
        facts: &TypeLayoutFacts,
        adt: &AdtLayoutFacts,
        path: &str,
        record: &mut RustcTypeLayoutRecordV2,
    ) -> Result<SemanticTypeKindV2, SemanticTypeAdapterErrorV2> {
        match adt.kind {
            AdtKind::Struct | AdtKind::Union => {
                if adt.tag.is_some() || adt.variants.len() != 1 {
                    return Err(inconsistent(
                        path,
                        "non-enum ADT does not have exactly one field set",
                    ));
                }
                let fields = &adt.variants[0].fields;
                let converted = self.convert_fields(fields, path, true)?;
                push_aggregate_record(
                    record,
                    aggregate_record(path, fields, facts.size_bytes, &[])?,
                )?;
                if adt.kind == AdtKind::Struct {
                    Ok(SemanticTypeKindV2::Struct {
                        identity: facts.rust_type.clone(),
                        fields: converted,
                    })
                } else {
                    Ok(SemanticTypeKindV2::Union {
                        identity: facts.rust_type.clone(),
                        fields: converted,
                    })
                }
            }
            AdtKind::Enum => self.convert_enum(facts, adt, path, record),
        }
    }

    fn convert_enum(
        &mut self,
        facts: &TypeLayoutFacts,
        adt: &AdtLayoutFacts,
        path: &str,
        record: &mut RustcTypeLayoutRecordV2,
    ) -> Result<SemanticTypeKindV2, SemanticTypeAdapterErrorV2> {
        if adt.variants.is_empty() {
            return Err(unsupported(
                path,
                "empty inhabited enum has no exact encoding",
            ));
        }
        if adt.variants.iter().any(|variant| variant.uninhabited) {
            return Err(unsupported(path, "enum contains an uninhabited variant"));
        }
        let discriminant = enum_discriminant(&adt.variants, path)?;
        let encoding = match adt.tag {
            None if adt.variants.len() == 1 => SemanticEnumEncodingV2::Single { variant: 0 },
            None => {
                return Err(inconsistent(
                    path,
                    "multi-variant enum has no rustc tag encoding",
                ));
            }
            Some(tag) => match tag.encoding {
                EnumTagEncodingFacts::Direct => SemanticEnumEncodingV2::Direct {
                    tag_offset: tag.offset_bytes,
                    tag: backend_integer(tag.scalar, &format!("{path}.tag"))?,
                },
                EnumTagEncodingFacts::Niche {
                    untagged_variant,
                    niche_variants_start,
                    niche_variants_end,
                    niche_start,
                } => {
                    let variant = adt.variants.get(untagged_variant as usize).ok_or_else(|| {
                        inconsistent(path, "niche untagged variant is out of range")
                    })?;
                    let (source, scalar, ranges) =
                        find_niche_source(&variant.fields, tag.offset_bytes, tag.scalar, path)?;
                    SemanticEnumEncodingV2::Niche {
                        source,
                        niche_scalar: scalar,
                        valid_ranges: ranges,
                        untagged_variant,
                        niche_variants_start,
                        niche_variants_end,
                        niche_start,
                    }
                }
            },
        };

        let direct_reserved = match &encoding {
            SemanticEnumEncodingV2::Direct { tag_offset, tag } => vec![(
                *tag_offset,
                checked_end(*tag_offset, scalar_bytes(*tag)?, facts.size_bytes, path)?,
            )],
            _ => Vec::new(),
        };
        let mut variants = Vec::new();
        variants
            .try_reserve_exact(adt.variants.len())
            .map_err(|_| SemanticTypeAdapterErrorV2::AllocationFailed {
                resource: "semantic enum variants",
            })?;
        record
            .aggregates
            .try_reserve(adt.variants.len())
            .map_err(|_| SemanticTypeAdapterErrorV2::AllocationFailed {
                resource: "rustc enum aggregate records",
            })?;
        for (index, variant) in adt.variants.iter().enumerate() {
            if variant.source_index as usize != index {
                return Err(inconsistent(path, "enum variants are not in source order"));
            }
            let discriminant_bits = variant
                .discriminant_bits
                .ok_or_else(|| inconsistent(path, "rustc omitted a logical discriminant"))?;
            let variant_path = format!("{path}.variant[{index}]");
            let fields = self.convert_fields(&variant.fields, &variant_path, true)?;
            record.aggregates.push(aggregate_record(
                &variant_path,
                &variant.fields,
                facts.size_bytes,
                &direct_reserved,
            )?);
            variants.push(SemanticVariantV2 {
                name: variant.name.clone(),
                discriminant: discriminant_bits,
                fields,
            });
        }
        Ok(SemanticTypeKindV2::Enum {
            identity: facts.rust_type.clone(),
            discriminant,
            encoding,
            variants,
        })
    }
}

fn push_aggregate_record(
    record: &mut RustcTypeLayoutRecordV2,
    aggregate: RustcAggregateLayoutV2,
) -> Result<(), SemanticTypeAdapterErrorV2> {
    record
        .aggregates
        .try_reserve(1)
        .map_err(|_| SemanticTypeAdapterErrorV2::AllocationFailed {
            resource: "rustc aggregate records",
        })?;
    record.aggregates.push(aggregate);
    Ok(())
}

fn scalar_kind(
    facts: &TypeLayoutFacts,
    source: SourceScalarKind,
    path: &str,
) -> Result<SemanticTypeKindV2, SemanticTypeAdapterErrorV2> {
    let scalar = source_scalar(source, path)?;
    match source {
        SourceScalarKind::Bool | SourceScalarKind::Char | SourceScalarKind::Float { .. } => {
            Ok(SemanticTypeKindV2::Scalar(scalar))
        }
        SourceScalarKind::SignedInteger { .. } | SourceScalarKind::UnsignedInteger { .. } => {
            let backend = only_scalar(&facts.backend_representation, path)?;
            let ranges = validity_ranges(backend, path)?;
            if is_full_validity(scalar, &ranges) {
                Ok(SemanticTypeKindV2::Scalar(scalar))
            } else {
                Ok(SemanticTypeKindV2::ValidityScalar {
                    scalar,
                    valid_ranges: ranges,
                })
            }
        }
    }
}

fn source_scalar(
    source: SourceScalarKind,
    path: &str,
) -> Result<SemanticScalarV2, SemanticTypeAdapterErrorV2> {
    let scalar = match source {
        SourceScalarKind::Bool => SemanticScalarV2::Bool,
        SourceScalarKind::Char => SemanticScalarV2::Char,
        SourceScalarKind::SignedInteger { bits } => SemanticScalarV2::Int {
            signed: true,
            bits: u16::try_from(bits)
                .map_err(|_| unsupported(path, "integer width exceeds u16"))?,
        },
        SourceScalarKind::UnsignedInteger { bits } => SemanticScalarV2::Int {
            signed: false,
            bits: u16::try_from(bits)
                .map_err(|_| unsupported(path, "integer width exceeds u16"))?,
        },
        SourceScalarKind::Float { bits } => SemanticScalarV2::Float {
            bits: u16::try_from(bits).map_err(|_| unsupported(path, "float width exceeds u16"))?,
        },
    };
    Ok(scalar)
}

fn enum_discriminant(
    variants: &[VariantLayoutFacts],
    path: &str,
) -> Result<SemanticScalarV2, SemanticTypeAdapterErrorV2> {
    let first = variants[0]
        .discriminant_scalar
        .ok_or_else(|| inconsistent(path, "rustc omitted the enum discriminant type"))?;
    if variants
        .iter()
        .any(|variant| variant.discriminant_scalar != Some(first))
    {
        return Err(inconsistent(
            path,
            "enum variants disagree on discriminant type",
        ));
    }
    let scalar = source_scalar(first, path)?;
    if !matches!(scalar, SemanticScalarV2::Int { .. }) {
        return Err(inconsistent(path, "enum discriminant is not an integer"));
    }
    Ok(scalar)
}

fn backend_integer(
    scalar: ScalarLayoutFacts,
    path: &str,
) -> Result<SemanticScalarV2, SemanticTypeAdapterErrorV2> {
    match scalar.primitive {
        ScalarPrimitiveFacts::Integer { bits, signed } => Ok(SemanticScalarV2::Int {
            signed,
            bits: u16::try_from(bits).map_err(|_| unsupported(path, "tag width exceeds u16"))?,
        }),
        ScalarPrimitiveFacts::Pointer { .. } => Err(unsupported(
            path,
            "pointer niche requires provenance-aware pointer validity",
        )),
        ScalarPrimitiveFacts::Float { .. } => Err(unsupported(path, "floating enum tag")),
    }
}

fn only_scalar<'a>(
    backend: &'a BackendRepresentationFacts,
    path: &str,
) -> Result<&'a ScalarLayoutFacts, SemanticTypeAdapterErrorV2> {
    match backend {
        BackendRepresentationFacts::Scalar(scalar) => Ok(scalar),
        _ => Err(inconsistent(
            path,
            "source scalar has a non-scalar backend representation",
        )),
    }
}

fn pointer_storage(
    facts: &TypeLayoutFacts,
    path: &str,
) -> Result<(u8, u32), SemanticTypeAdapterErrorV2> {
    let scalar = only_scalar(&facts.backend_representation, path)?;
    let ScalarPrimitiveFacts::Pointer { address_space } = scalar.primitive else {
        return Err(inconsistent(path, "thin pointer has non-pointer storage"));
    };
    let bytes = u8::try_from(scalar.size_bytes)
        .map_err(|_| unsupported(path, "data pointer width exceeds u8 bytes"))?;
    Ok((bytes, address_space))
}

fn validity_ranges(
    scalar: &ScalarLayoutFacts,
    path: &str,
) -> Result<Vec<ScalarValidityRangeV2>, SemanticTypeAdapterErrorV2> {
    let bits = match scalar.primitive {
        ScalarPrimitiveFacts::Integer { bits, .. } => bits,
        _ => return Err(unsupported(path, "validity scalar is not integer storage")),
    };
    if !matches!(bits, 8 | 16 | 32 | 64 | 128) {
        return Err(unsupported(path, "unsupported validity scalar width"));
    }
    let maximum = bit_max(bits);
    if scalar.valid_range_start <= scalar.valid_range_end {
        Ok(vec![ScalarValidityRangeV2 {
            start: scalar.valid_range_start,
            end: scalar.valid_range_end,
        }])
    } else {
        Ok(vec![
            ScalarValidityRangeV2 {
                start: 0,
                end: scalar.valid_range_end,
            },
            ScalarValidityRangeV2 {
                start: scalar.valid_range_start,
                end: maximum,
            },
        ])
    }
}

fn is_full_validity(scalar: SemanticScalarV2, ranges: &[ScalarValidityRangeV2]) -> bool {
    let SemanticScalarV2::Int { bits, .. } = scalar else {
        return false;
    };
    ranges
        == [ScalarValidityRangeV2 {
            start: 0,
            end: bit_max(u64::from(bits)),
        }]
}

fn bit_max(bits: u64) -> u128 {
    if bits == 128 {
        u128::MAX
    } else {
        (1_u128 << bits) - 1
    }
}

fn find_niche_source(
    fields: &[FieldLayoutFacts],
    expected_offset: u64,
    tag: ScalarLayoutFacts,
    path: &str,
) -> Result<
    (
        SemanticNicheSourceV2,
        SemanticScalarV2,
        Vec<ScalarValidityRangeV2>,
    ),
    SemanticTypeAdapterErrorV2,
> {
    backend_integer(tag, path)?;
    let mut found = Vec::new();
    for (index, field) in fields.iter().enumerate() {
        let mut components = vec![SemanticNichePathComponentV2::Field(index as u32)];
        find_niche_in_type(
            &field.layout,
            field.offset_bytes,
            expected_offset,
            tag,
            &mut components,
            &mut found,
        )?;
    }
    if found.len() != 1 {
        return Err(unsupported(
            path,
            "niche does not resolve to exactly one provenance-free validity scalar",
        ));
    }
    let (components, scalar, ranges) = found.pop().expect("one niche path");
    Ok((
        SemanticNicheSourceV2 {
            path: components,
            expected_offset,
        },
        scalar,
        ranges,
    ))
}

fn find_niche_in_type(
    facts: &TypeLayoutFacts,
    base: u64,
    expected_offset: u64,
    tag: ScalarLayoutFacts,
    path: &mut Vec<SemanticNichePathComponentV2>,
    found: &mut Vec<(
        Vec<SemanticNichePathComponentV2>,
        SemanticScalarV2,
        Vec<ScalarValidityRangeV2>,
    )>,
) -> Result<(), SemanticTypeAdapterErrorV2> {
    match &facts.kind {
        TypeLayoutKind::Scalar(source) => {
            if base == expected_offset {
                let backend = only_scalar(&facts.backend_representation, "niche")?;
                if backend.primitive == tag.primitive && backend.size_bytes == tag.size_bytes {
                    let scalar = source_scalar(*source, "niche")?;
                    let ranges = validity_ranges(backend, "niche")?;
                    if matches!(scalar, SemanticScalarV2::Int { .. })
                        && !is_full_validity(scalar, &ranges)
                    {
                        found.push((path.clone(), scalar, ranges));
                    }
                }
            }
        }
        TypeLayoutKind::Tuple(fields) => {
            find_niche_in_fields(fields, base, expected_offset, tag, path, found)?;
        }
        TypeLayoutKind::Array(array) if array.length != 0 && array.stride_bytes != 0 => {
            if expected_offset >= base {
                let index = (expected_offset - base) / array.stride_bytes;
                if index < array.length {
                    let child_base = base
                        .checked_add(index.checked_mul(array.stride_bytes).ok_or_else(|| {
                            inconsistent("niche", "array niche offset overflows u64")
                        })?)
                        .ok_or_else(|| inconsistent("niche", "array niche offset overflows u64"))?;
                    path.push(SemanticNichePathComponentV2::ArrayElement(index));
                    find_niche_in_type(
                        &array.element,
                        child_base,
                        expected_offset,
                        tag,
                        path,
                        found,
                    )?;
                    path.pop();
                }
            }
        }
        TypeLayoutKind::Adt(adt) if adt.kind == AdtKind::Struct && adt.variants.len() == 1 => {
            find_niche_in_fields(
                &adt.variants[0].fields,
                base,
                expected_offset,
                tag,
                path,
                found,
            )?;
        }
        TypeLayoutKind::Pointer(_) | TypeLayoutKind::Adt(_) | TypeLayoutKind::Array(_) => {}
    }
    Ok(())
}

fn find_niche_in_fields(
    fields: &[FieldLayoutFacts],
    base: u64,
    expected_offset: u64,
    tag: ScalarLayoutFacts,
    path: &mut Vec<SemanticNichePathComponentV2>,
    found: &mut Vec<(
        Vec<SemanticNichePathComponentV2>,
        SemanticScalarV2,
        Vec<ScalarValidityRangeV2>,
    )>,
) -> Result<(), SemanticTypeAdapterErrorV2> {
    for (index, field) in fields.iter().enumerate() {
        let child_base = base
            .checked_add(field.offset_bytes)
            .ok_or_else(|| inconsistent("niche", "nested niche offset overflows u64"))?;
        path.push(SemanticNichePathComponentV2::Field(index as u32));
        find_niche_in_type(&field.layout, child_base, expected_offset, tag, path, found)?;
        path.pop();
    }
    Ok(())
}

fn aggregate_record(
    path: &str,
    fields: &[FieldLayoutFacts],
    container_size: u64,
    reserved: &[(u64, u64)],
) -> Result<RustcAggregateLayoutV2, SemanticTypeAdapterErrorV2> {
    let mut source_to_memory = Vec::new();
    source_to_memory
        .try_reserve_exact(fields.len())
        .map_err(|_| SemanticTypeAdapterErrorV2::AllocationFailed {
            resource: "rustc source-to-memory projection",
        })?;
    let mut seen_memory = Vec::new();
    seen_memory.try_reserve_exact(fields.len()).map_err(|_| {
        SemanticTypeAdapterErrorV2::AllocationFailed {
            resource: "rustc field permutation state",
        }
    })?;
    seen_memory.resize(fields.len(), false);
    let mut occupied = Vec::new();
    occupied
        .try_reserve(reserved.len().saturating_add(fields.len()))
        .map_err(|_| SemanticTypeAdapterErrorV2::AllocationFailed {
            resource: "rustc aggregate occupied ranges",
        })?;
    occupied.extend_from_slice(reserved);
    for (source_index, field) in fields.iter().enumerate() {
        if field.source_index != source_index || field.memory_index >= fields.len() {
            return Err(inconsistent(
                path,
                "field order is not a source/memory permutation",
            ));
        }
        if std::mem::replace(&mut seen_memory[field.memory_index], true) {
            return Err(inconsistent(path, "field memory index is duplicated"));
        }
        source_to_memory.push(
            u32::try_from(field.memory_index)
                .map_err(|_| unsupported(path, "field memory index exceeds u32"))?,
        );
        if field.layout.size_bytes != 0 {
            occupied.push((
                field.offset_bytes,
                checked_end(
                    field.offset_bytes,
                    field.layout.size_bytes,
                    container_size,
                    path,
                )?,
            ));
        }
    }
    occupied.sort_unstable();
    let mut merged: Vec<(u64, u64)> = Vec::new();
    merged.try_reserve(occupied.len()).map_err(|_| {
        SemanticTypeAdapterErrorV2::AllocationFailed {
            resource: "rustc aggregate merged ranges",
        }
    })?;
    for range in occupied {
        if let Some(last) = merged.last_mut()
            && range.0 <= last.1
        {
            last.1 = last.1.max(range.1);
        } else {
            merged.push(range);
        }
    }
    let mut padding = Vec::new();
    padding
        .try_reserve(merged.len().saturating_add(1))
        .map_err(|_| SemanticTypeAdapterErrorV2::AllocationFailed {
            resource: "rustc aggregate padding ranges",
        })?;
    let mut cursor = 0;
    for (start, end) in merged {
        if start > cursor {
            padding.push(RustcByteRangeV2 {
                start: cursor,
                end: start,
            });
        }
        cursor = cursor.max(end);
    }
    if cursor < container_size {
        padding.push(RustcByteRangeV2 {
            start: cursor,
            end: container_size,
        });
    }
    Ok(RustcAggregateLayoutV2 {
        path: path.to_owned(),
        source_to_memory,
        padding,
    })
}

fn checked_end(
    offset: u64,
    size: u64,
    container: u64,
    path: &str,
) -> Result<u64, SemanticTypeAdapterErrorV2> {
    let end = offset
        .checked_add(size)
        .ok_or_else(|| inconsistent(path, "byte extent overflows u64"))?;
    if end > container {
        return Err(inconsistent(path, "byte extent exceeds containing layout"));
    }
    Ok(end)
}

fn scalar_bytes(scalar: SemanticScalarV2) -> Result<u64, SemanticTypeAdapterErrorV2> {
    match scalar {
        SemanticScalarV2::Bool => Ok(1),
        SemanticScalarV2::Char => Ok(4),
        SemanticScalarV2::Int { bits, .. } | SemanticScalarV2::Float { bits }
            if bits != 0 && bits.is_multiple_of(8) =>
        {
            Ok(u64::from(bits / 8))
        }
        _ => Err(unsupported("scalar", "unsupported scalar byte width")),
    }
}

fn is_unsized_pointer<'tcx>(ty: Ty<'tcx>, tcx: TyCtxt<'tcx>) -> bool {
    let pointee = match *ty.kind() {
        TyKind::Ref(_, pointee, _) | TyKind::RawPtr(pointee, _) => pointee,
        _ => return false,
    };
    !pointee.is_sized(tcx, TypingEnv::fully_monomorphized())
}

fn observe_unsized_pointer_layout<'tcx>(
    tcx: TyCtxt<'tcx>,
    ty: Ty<'tcx>,
    budgets: SemanticTypeLayoutBudgetsV2,
) -> Result<PendingObservationV2, SemanticTypeAdapterErrorV2> {
    let typing_env = TypingEnv::fully_monomorphized();
    let layout_cx = LayoutCx::new(tcx, typing_env);
    let layout = layout_cx
        .layout_of(ty)
        .map_err(|error| SemanticTypeAdapterErrorV2::Extraction(error.to_string()))?;
    let (pointee, mutability, reference) = match *ty.kind() {
        TyKind::Ref(_, pointee, mutability) => (pointee, mutability, true),
        TyKind::RawPtr(pointee, mutability) => (pointee, mutability, false),
        _ => return Err(unsupported("root", "expected an unsized pointer")),
    };
    let required_records = if matches!(*pointee.kind(), TyKind::Slice(_)) {
        3_u64
    } else {
        2_u64
    };
    if required_records > u64::from(budgets.max_sidecar_records) {
        return Err(SemanticTypeAdapterErrorV2::BoundExceeded {
            resource: "rustc layout sidecar records",
            actual: required_records,
            max: u64::from(budgets.max_sidecar_records),
        });
    }
    let BackendRepr::ScalarPair(first, second) = layout.backend_repr else {
        return Err(inconsistent(
            "root",
            "wide pointer does not use scalar-pair storage",
        ));
    };
    let Primitive::Pointer(address_space) = first.primitive() else {
        return Err(inconsistent(
            "root",
            "wide pointer data component is not a pointer",
        ));
    };
    let data_pointer_bytes = u8::try_from(first.size(&layout_cx).bytes())
        .map_err(|_| unsupported("root", "pointer width exceeds u8 bytes"))?;
    let mut graph = SemanticTypeGraphBuilderV2::new(budgets.graph);
    let root_key = bounded_type_name(ty, budgets.graph.max_name_bytes)?;
    let pointee_key = bounded_type_name(pointee, budgets.graph.max_name_bytes)?;
    let root = graph.declare(root_key.clone())?;
    let pointee_id = graph.declare(pointee_key.clone())?;
    let pointee_layout = layout_cx
        .layout_of(pointee)
        .map_err(|error| SemanticTypeAdapterErrorV2::Extraction(error.to_string()))?;
    let mut records = Vec::new();
    records
        .try_reserve_exact(required_records as usize)
        .map_err(|_| SemanticTypeAdapterErrorV2::AllocationFailed {
            resource: "rustc DST layout records",
        })?;
    records.push(RustcTypeLayoutRecordV2 {
        key: root_key.clone(),
        representation: None,
        aggregates: Vec::new(),
        array_stride_bytes: None,
        uninhabited: false,
    });
    let (pointee_kind, metadata) = match *pointee.kind() {
        TyKind::Slice(element) => {
            let element_facts =
                extract_general_layout_with_limits(tcx, element, ExtractionLimits::default())?;
            let TypeLayoutKind::Scalar(source) = element_facts.kind else {
                return Err(unsupported(
                    "root.pointee.element",
                    "DST slice elements are currently restricted to scalar types",
                ));
            };
            let element_key = element_facts.rust_type.clone();
            let element_id = graph.declare(element_key.clone())?;
            graph.define(
                element_id,
                SemanticTypeNodeV2 {
                    layout: SemanticTypeLayoutV2::sized(
                        element_facts.size_bytes,
                        element_facts.abi_alignment_bytes,
                    ),
                    kind: scalar_kind(&element_facts, source, "root.pointee.element")?,
                },
            )?;
            records.push(RustcTypeLayoutRecordV2 {
                key: element_key,
                representation: None,
                aggregates: Vec::new(),
                array_stride_bytes: None,
                uninhabited: false,
            });
            (
                SemanticTypeKindV2::Slice {
                    element: element_id,
                },
                PointerMetadataV2::SliceLength,
            )
        }
        TyKind::Str => (SemanticTypeKindV2::Str, PointerMetadataV2::SliceLength),
        TyKind::Dynamic(..) => (
            SemanticTypeKindV2::OpaqueDst {
                identity: pointee_key.clone(),
                metadata: PointerMetadataV2::VTable {
                    trait_identity: pointee_key.clone(),
                },
            },
            PointerMetadataV2::VTable {
                trait_identity: pointee_key.clone(),
            },
        ),
        _ => {
            return Err(unsupported(
                "root.pointee",
                "unsupported dynamically sized pointee",
            ));
        }
    };
    let second_primitive = second.primitive();
    match (&metadata, second_primitive) {
        (PointerMetadataV2::SliceLength, Primitive::Int(..))
        | (PointerMetadataV2::VTable { .. }, Primitive::Pointer(_)) => {}
        _ => {
            return Err(inconsistent(
                "root",
                "wide pointer metadata storage disagrees with pointee",
            ));
        }
    }
    graph.define(
        pointee_id,
        SemanticTypeNodeV2 {
            layout: SemanticTypeLayoutV2::dynamically_sized(pointee_layout.align.abi.bytes()),
            kind: pointee_kind,
        },
    )?;
    let mutability = match mutability {
        Mutability::Not => SemanticMutabilityV2::Immutable,
        Mutability::Mut => SemanticMutabilityV2::Mutable,
    };
    let pointer_kind = if reference {
        SemanticTypeKindV2::Reference {
            referent: pointee_id,
            mutability,
            address_space: address_space.0,
            data_pointer_bytes,
            metadata,
        }
    } else {
        SemanticTypeKindV2::RawPointer {
            pointee: pointee_id,
            mutability,
            address_space: address_space.0,
            data_pointer_bytes,
            metadata,
        }
    };
    graph.define(
        root,
        SemanticTypeNodeV2 {
            layout: SemanticTypeLayoutV2::sized(layout.size.bytes(), layout.align.abi.bytes()),
            kind: pointer_kind,
        },
    )?;
    records.push(RustcTypeLayoutRecordV2 {
        key: pointee_key,
        representation: None,
        aggregates: Vec::new(),
        array_stride_bytes: None,
        uninhabited: false,
    });
    let graph = graph.finish(root)?;
    let graph_bytes = graph.canonical_bytes()?;
    Ok(PendingObservationV2 {
        graph,
        graph_bytes,
        layout_records: records,
    })
}

fn encode_sidecar(
    records: &[RustcTypeLayoutRecordV2],
    max_bytes: u32,
) -> Result<Vec<u8>, SemanticTypeAdapterErrorV2> {
    let mut output = Vec::new();
    output
        .try_reserve((max_bytes as usize).min(4096))
        .map_err(|_| SemanticTypeAdapterErrorV2::AllocationFailed {
            resource: "rustc layout sidecar bytes",
        })?;
    push_bytes(&mut output, b"fe2o3.rustc-layout-sidecar.v2", max_bytes)?;
    push_u32(&mut output, records.len() as u32, max_bytes)?;
    for record in records {
        push_bytes(&mut output, record.key.as_bytes(), max_bytes)?;
        match record.representation {
            None => push_u8(&mut output, 0, max_bytes)?,
            Some(repr) => {
                push_u8(&mut output, 1, max_bytes)?;
                push_u8(&mut output, u8::from(repr.c), max_bytes)?;
                push_u8(&mut output, u8::from(repr.transparent), max_bytes)?;
                push_u8(&mut output, u8::from(repr.explicit_integer), max_bytes)?;
                push_option_u64(&mut output, repr.packed_alignment_bytes, max_bytes)?;
                push_option_u64(&mut output, repr.requested_alignment_bytes, max_bytes)?;
            }
        }
        push_u8(&mut output, u8::from(record.uninhabited), max_bytes)?;
        push_option_u64(&mut output, record.array_stride_bytes, max_bytes)?;
        push_u32(&mut output, record.aggregates.len() as u32, max_bytes)?;
        for aggregate in &record.aggregates {
            push_bytes(&mut output, aggregate.path.as_bytes(), max_bytes)?;
            push_u32(
                &mut output,
                aggregate.source_to_memory.len() as u32,
                max_bytes,
            )?;
            for index in &aggregate.source_to_memory {
                push_u32(&mut output, *index, max_bytes)?;
            }
            push_u32(&mut output, aggregate.padding.len() as u32, max_bytes)?;
            for range in &aggregate.padding {
                push_u64(&mut output, range.start, max_bytes)?;
                push_u64(&mut output, range.end, max_bytes)?;
            }
        }
    }
    Ok(output)
}

fn observation_identity(target: &SemanticLayoutTargetV1, graph: &[u8], sidecar: &[u8]) -> [u8; 32] {
    let mut digest = Sha256::new();
    digest.update(OBSERVATION_DOMAIN_V2);
    hash_component(&mut digest, target.llvm_target().as_bytes());
    hash_component(&mut digest, target.data_layout().as_bytes());
    digest.update(target.default_pointer_width_bits().to_le_bytes());
    hash_component(&mut digest, graph);
    hash_component(&mut digest, sidecar);
    digest.finalize().into()
}

fn projection_identity(canonical_bytes: &[u8]) -> [u8; 32] {
    let mut digest = Sha256::new();
    digest.update(GFX942_PROJECTION_DOMAIN_V2);
    hash_component(&mut digest, canonical_bytes);
    digest.finalize().into()
}

fn candidate_identity(
    observation_identity: &[u8; 32],
    projection_identity: &[u8; 32],
    projection_bytes: &[u8],
) -> [u8; 32] {
    let mut digest = Sha256::new();
    digest.update(GFX942_CANDIDATE_DOMAIN_V2);
    digest.update(observation_identity);
    digest.update(projection_identity);
    hash_component(&mut digest, projection_bytes);
    digest.finalize().into()
}

fn validate_observation_integrity(
    observation: &RustcTypeLayoutObservationV2,
    budgets: SemanticTypeLayoutBudgetsV2,
) -> Result<(), Gfx942LayoutCompatibilityErrorV2> {
    if observation.layout_records.len() > budgets.max_sidecar_records as usize {
        return Err(inconsistent_layout(
            "observation",
            "layout record bound exceeded",
        ));
    }
    if observation.layout_records.len() != observation.graph.node_count() {
        return Err(inconsistent_layout(
            "observation",
            "layout record count differs from graph node count",
        ));
    }
    for pair in observation.layout_records.windows(2) {
        if pair[0].key >= pair[1].key {
            return Err(inconsistent_layout(
                "observation",
                "layout records are not strictly sorted",
            ));
        }
    }
    for (_, key, _) in observation.graph.nodes() {
        if observation.layout_record(key).is_none() {
            return Err(inconsistent_layout(
                key,
                "graph node has no exact layout record",
            ));
        }
    }
    let graph_bytes = observation
        .graph
        .canonical_bytes()
        .map_err(|_| inconsistent_layout("observation", "semantic graph is no longer canonical"))?;
    if graph_bytes != observation.graph_bytes {
        return Err(Gfx942LayoutCompatibilityErrorV2::ObservationIdentityMismatch);
    }
    let sidecar = encode_sidecar(&observation.layout_records, budgets.max_sidecar_bytes)
        .map_err(|_| inconsistent_layout("observation", "layout sidecar exceeds current bounds"))?;
    let identity = observation_identity(&observation.rustc_target, &graph_bytes, &sidecar);
    if identity != observation.identity_sha256 {
        return Err(Gfx942LayoutCompatibilityErrorV2::ObservationIdentityMismatch);
    }
    Ok(())
}

fn encode_projection(
    target: &CanonicalGfx942LayoutTargetV2,
    records: &[Gfx942LayoutProjectionRecordV2],
    max_bytes: u32,
) -> Result<Vec<u8>, Gfx942LayoutCompatibilityErrorV2> {
    if !target.is_canonical() {
        return Err(Gfx942LayoutCompatibilityErrorV2::TargetMismatch);
    }
    let mut output = Vec::new();
    output
        .try_reserve((max_bytes as usize).min(4096))
        .map_err(|_| Gfx942LayoutCompatibilityErrorV2::AllocationFailed {
            resource: "gfx942 projection bytes",
        })?;
    projection_bytes(&mut output, b"fe2o3.gfx942-layout-projection.v2", max_bytes)?;
    projection_bytes(&mut output, target.triple().as_bytes(), max_bytes)?;
    projection_bytes(&mut output, target.cpu().as_bytes(), max_bytes)?;
    projection_bytes(&mut output, target.features().as_bytes(), max_bytes)?;
    projection_bytes(&mut output, target.data_layout().as_bytes(), max_bytes)?;
    projection_extend(
        &mut output,
        &target.pointer_width_bits().to_le_bytes(),
        max_bytes,
    )?;
    let count = u32::try_from(records.len())
        .map_err(|_| inconsistent_layout("projection", "record count exceeds u32"))?;
    projection_extend(&mut output, &count.to_le_bytes(), max_bytes)?;
    for record in records {
        projection_bytes(&mut output, record.key.as_bytes(), max_bytes)?;
        projection_extend(&mut output, &record.size_bytes.to_le_bytes(), max_bytes)?;
        projection_extend(
            &mut output,
            &record.alignment_bytes.to_le_bytes(),
            max_bytes,
        )?;
        projection_option_u64(&mut output, record.array_stride_bytes, max_bytes)?;
        let field_count = u32::try_from(record.field_offsets.len())
            .map_err(|_| inconsistent_layout(&record.key, "field count exceeds u32"))?;
        projection_extend(&mut output, &field_count.to_le_bytes(), max_bytes)?;
        for offset in &record.field_offsets {
            projection_extend(&mut output, &offset.to_le_bytes(), max_bytes)?;
        }
        let padding_count = u32::try_from(record.padding.len())
            .map_err(|_| inconsistent_layout(&record.key, "padding count exceeds u32"))?;
        projection_extend(&mut output, &padding_count.to_le_bytes(), max_bytes)?;
        for range in &record.padding {
            projection_extend(&mut output, &range.start.to_le_bytes(), max_bytes)?;
            projection_extend(&mut output, &range.end.to_le_bytes(), max_bytes)?;
        }
    }
    Ok(output)
}

fn projection_bytes(
    output: &mut Vec<u8>,
    bytes: &[u8],
    max_bytes: u32,
) -> Result<(), Gfx942LayoutCompatibilityErrorV2> {
    let length = u32::try_from(bytes.len())
        .map_err(|_| inconsistent_layout("projection", "text length exceeds u32"))?;
    projection_extend(output, &length.to_le_bytes(), max_bytes)?;
    projection_extend(output, bytes, max_bytes)
}

fn projection_option_u64(
    output: &mut Vec<u8>,
    value: Option<u64>,
    max_bytes: u32,
) -> Result<(), Gfx942LayoutCompatibilityErrorV2> {
    projection_extend(output, &[u8::from(value.is_some())], max_bytes)?;
    if let Some(value) = value {
        projection_extend(output, &value.to_le_bytes(), max_bytes)?;
    }
    Ok(())
}

fn projection_extend(
    output: &mut Vec<u8>,
    bytes: &[u8],
    max_bytes: u32,
) -> Result<(), Gfx942LayoutCompatibilityErrorV2> {
    let actual = output.len().checked_add(bytes.len()).ok_or_else(|| {
        Gfx942LayoutCompatibilityErrorV2::ArithmeticOverflow {
            key: "projection bytes".to_owned(),
        }
    })?;
    if actual > max_bytes as usize {
        return Err(inconsistent_layout(
            "projection",
            "canonical byte bound exceeded",
        ));
    }
    output.try_reserve(bytes.len()).map_err(|_| {
        Gfx942LayoutCompatibilityErrorV2::AllocationFailed {
            resource: "gfx942 projection bytes",
        }
    })?;
    output.extend_from_slice(bytes);
    Ok(())
}

fn hash_component(digest: &mut Sha256, bytes: &[u8]) {
    digest.update((bytes.len() as u64).to_le_bytes());
    digest.update(bytes);
}

fn push_bytes(
    output: &mut Vec<u8>,
    bytes: &[u8],
    max: u32,
) -> Result<(), SemanticTypeAdapterErrorV2> {
    let length =
        u32::try_from(bytes.len()).map_err(|_| SemanticTypeAdapterErrorV2::BoundExceeded {
            resource: "rustc layout sidecar bytes",
            actual: bytes.len() as u64,
            max: u64::from(u32::MAX),
        })?;
    push_u32(output, length, max)?;
    extend_bounded(output, bytes, max)
}

fn push_option_u64(
    output: &mut Vec<u8>,
    value: Option<u64>,
    max: u32,
) -> Result<(), SemanticTypeAdapterErrorV2> {
    push_u8(output, u8::from(value.is_some()), max)?;
    if let Some(value) = value {
        push_u64(output, value, max)?;
    }
    Ok(())
}

fn push_u8(output: &mut Vec<u8>, value: u8, max: u32) -> Result<(), SemanticTypeAdapterErrorV2> {
    extend_bounded(output, &[value], max)
}

fn push_u32(output: &mut Vec<u8>, value: u32, max: u32) -> Result<(), SemanticTypeAdapterErrorV2> {
    extend_bounded(output, &value.to_le_bytes(), max)
}

fn push_u64(output: &mut Vec<u8>, value: u64, max: u32) -> Result<(), SemanticTypeAdapterErrorV2> {
    extend_bounded(output, &value.to_le_bytes(), max)
}

fn extend_bounded(
    output: &mut Vec<u8>,
    bytes: &[u8],
    max: u32,
) -> Result<(), SemanticTypeAdapterErrorV2> {
    let actual = output.len().saturating_add(bytes.len());
    if actual > max as usize {
        return Err(SemanticTypeAdapterErrorV2::BoundExceeded {
            resource: "rustc layout sidecar bytes",
            actual: actual as u64,
            max: u64::from(max),
        });
    }
    output
        .try_reserve(bytes.len())
        .map_err(|_| SemanticTypeAdapterErrorV2::AllocationFailed {
            resource: "rustc layout sidecar bytes",
        })?;
    output.extend_from_slice(bytes);
    Ok(())
}

struct BoundedTypeNameWriter {
    text: String,
    max_bytes: usize,
    exceeded: bool,
}

impl fmt::Write for BoundedTypeNameWriter {
    fn write_str(&mut self, value: &str) -> fmt::Result {
        if self.text.len().saturating_add(value.len()) > self.max_bytes {
            self.exceeded = true;
            return Err(fmt::Error);
        }
        self.text.push_str(value);
        Ok(())
    }
}

fn bounded_type_name(ty: Ty<'_>, max_bytes: u32) -> Result<String, SemanticTypeAdapterErrorV2> {
    let max_bytes = max_bytes as usize;
    let mut text = String::new();
    text.try_reserve(max_bytes.min(256)).map_err(|_| {
        SemanticTypeAdapterErrorV2::AllocationFailed {
            resource: "bounded rustc type name",
        }
    })?;
    let mut output = BoundedTypeNameWriter {
        text,
        max_bytes,
        exceeded: false,
    };
    let rendered = with_no_trimmed_paths!(fmt::write(&mut output, format_args!("{ty}")));
    if output.exceeded {
        return Err(SemanticTypeAdapterErrorV2::BoundExceeded {
            resource: "rustc type name bytes",
            actual: max_bytes.saturating_add(1) as u64,
            max: max_bytes as u64,
        });
    }
    rendered.map_err(|_| SemanticTypeAdapterErrorV2::Inconsistent {
        path: "root".to_owned(),
        detail: "rustc type name formatting failed".to_owned(),
    })?;
    Ok(output.text)
}

fn unsupported(path: &str, detail: &'static str) -> SemanticTypeAdapterErrorV2 {
    SemanticTypeAdapterErrorV2::Unsupported {
        path: path.to_owned(),
        detail,
    }
}

fn inconsistent(path: &str, detail: impl Into<String>) -> SemanticTypeAdapterErrorV2 {
    SemanticTypeAdapterErrorV2::Inconsistent {
        path: path.to_owned(),
        detail: detail.into(),
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;
    use std::fs;
    use std::io::Write as _;
    use std::path::PathBuf;
    use std::process::{Command, Stdio};

    use rustc_driver::{Callbacks, Compilation};
    use rustc_hir::def::DefKind;
    use rustc_interface::interface::Compiler;

    use super::*;

    const FIXTURE: &str = r#"
#![allow(dead_code)]

use core::num::NonZeroU8;

#[repr(C)]
struct Inner { left: u16, right: u16 }

#[repr(C)]
struct CRoot { head: u32, nested: [Inner; 2], tail: u32 }

struct RustRoot { byte: u8, wide: u64, half: u16, word: u32 }

#[repr(C)]
struct Padded { byte: u8, word: u32 }

#[repr(transparent)]
struct Transparent(u32);

struct VeryLongNestedTypeNameForBoundedPreflight(u32);
struct R { nested: VeryLongNestedTypeNameForBoundedPreflight }

#[repr(u8)]
enum Direct { Empty = 3, Payload(u32) = 9 }

enum Niche { Empty, Payload(NonZeroU8) }

#[repr(C)]
union Bits { integer: u32, float: f32 }

trait Marker: Sync {}
struct MarkerValue;
impl Marker for MarkerValue {}

static MARKER: MarkerValue = MarkerValue;
static C_VALUE: CRoot = CRoot {
    head: 1,
    nested: [Inner { left: 2, right: 3 }, Inner { left: 4, right: 5 }],
    tail: 6,
};
static RUST_VALUE: RustRoot = RustRoot { byte: 1, wide: 2, half: 3, word: 4 };
static PADDED_VALUE: Padded = Padded { byte: 1, word: 2 };
static TRANSPARENT_VALUE: Transparent = Transparent(7);
static NESTED_NAME_VALUE: R = R { nested: VeryLongNestedTypeNameForBoundedPreflight(9) };
static TUPLE_VALUE: (u8, u32, [u16; 2]) = (1, 2, [3, 4]);
static ARRAY_VALUE: [Inner; 2] = [Inner { left: 1, right: 2 }, Inner { left: 3, right: 4 }];
static DIRECT_VALUE: Direct = Direct::Empty;
static NICHE_VALUE: Niche = Niche::Empty;
static BITS_VALUE: Bits = Bits { integer: 7 };
static SLICE_VALUE: &[u8] = &[1, 2, 3];
static DYN_VALUE: &dyn Marker = &MARKER;
static BYTE: u8 = 7;
static U64_VALUE: u64 = 11;
static F64_VALUE: f64 = 13.0;
static U128_VALUE: u128 = 17;
static BOOL_VALUE: bool = true;
static POINTER_NICHE: Option<&u8> = Some(&BYTE);
"#;

    #[derive(Default)]
    struct CaptureCallbacks {
        captures: BTreeMap<String, RustcTypeLayoutObservationV2>,
        pointer_niche: Option<SemanticTypeAdapterErrorV2>,
        mismatch: Option<SemanticTypeAdapterErrorV2>,
        bounded: Option<SemanticTypeAdapterErrorV2>,
        name_bounded: Option<SemanticTypeAdapterErrorV2>,
        nested_name_bounded: Option<SemanticTypeAdapterErrorV2>,
        work_bounded: Option<SemanticTypeAdapterErrorV2>,
        path_bounded: Option<SemanticTypeAdapterErrorV2>,
        dst_bounded: Option<SemanticTypeAdapterErrorV2>,
        reobserved: bool,
    }

    impl Callbacks for CaptureCallbacks {
        fn after_analysis<'tcx>(&mut self, _compiler: &Compiler, tcx: TyCtxt<'tcx>) -> Compilation {
            let observed = rustc_semantic_layout_target_v1(tcx).unwrap();
            let budgets = SemanticTypeLayoutBudgetsV2::default();
            for name in [
                "C_VALUE",
                "RUST_VALUE",
                "PADDED_VALUE",
                "TRANSPARENT_VALUE",
                "TUPLE_VALUE",
                "ARRAY_VALUE",
                "DIRECT_VALUE",
                "NICHE_VALUE",
                "BITS_VALUE",
                "SLICE_VALUE",
                "DYN_VALUE",
                "U64_VALUE",
                "F64_VALUE",
                "U128_VALUE",
                "BOOL_VALUE",
            ] {
                let ty = local_static_type(tcx, name);
                let capture = observe_rustc_type_layout_v2(tcx, ty, &observed, budgets)
                    .unwrap_or_else(|error| panic!("capture {name}: {error}"));
                if name == "C_VALUE" {
                    let reobserved = compare_untrusted_graph_to_rustc_observation_v2(
                        tcx,
                        ty,
                        capture.graph_bytes(),
                        &observed,
                        budgets,
                    )
                    .expect("reauthenticate exact rustc graph");
                    self.reobserved = reobserved.identity_sha256() == capture.identity_sha256();
                }
                self.captures.insert(name.to_owned(), capture);
            }

            self.pointer_niche = observe_rustc_type_layout_v2(
                tcx,
                local_static_type(tcx, "POINTER_NICHE"),
                &observed,
                budgets,
            )
            .err();
            let different = SemanticLayoutTargetV1::new(
                "different-rustc-target",
                observed.data_layout(),
                observed.default_pointer_width_bits(),
            )
            .unwrap();
            self.mismatch = observe_rustc_type_layout_v2(
                tcx,
                local_static_type(tcx, "C_VALUE"),
                &different,
                budgets,
            )
            .err();
            self.bounded = observe_rustc_type_layout_v2(
                tcx,
                local_static_type(tcx, "C_VALUE"),
                &observed,
                SemanticTypeLayoutBudgetsV2 {
                    graph: SemanticTypeGraphBudgetsV2 {
                        max_nodes: 1,
                        ..SemanticTypeGraphBudgetsV2::default()
                    },
                    ..SemanticTypeLayoutBudgetsV2::default()
                },
            )
            .err();
            self.name_bounded = observe_rustc_type_layout_v2(
                tcx,
                local_static_type(tcx, "C_VALUE"),
                &observed,
                SemanticTypeLayoutBudgetsV2 {
                    graph: SemanticTypeGraphBudgetsV2 {
                        max_name_bytes: 4,
                        ..SemanticTypeGraphBudgetsV2::default()
                    },
                    ..SemanticTypeLayoutBudgetsV2::default()
                },
            )
            .err();
            let nested_root = local_static_type(tcx, "NESTED_NAME_VALUE");
            let nested_root_name = bounded_type_name(nested_root, u32::MAX).unwrap();
            self.nested_name_bounded = observe_rustc_type_layout_v2(
                tcx,
                nested_root,
                &observed,
                SemanticTypeLayoutBudgetsV2 {
                    graph: SemanticTypeGraphBudgetsV2 {
                        max_name_bytes: nested_root_name.len() as u32,
                        ..SemanticTypeGraphBudgetsV2::default()
                    },
                    ..SemanticTypeLayoutBudgetsV2::default()
                },
            )
            .err();
            self.work_bounded = observe_rustc_type_layout_v2(
                tcx,
                local_static_type(tcx, "C_VALUE"),
                &observed,
                SemanticTypeLayoutBudgetsV2 {
                    max_observation_work: 0,
                    ..SemanticTypeLayoutBudgetsV2::default()
                },
            )
            .err();
            self.path_bounded = observe_rustc_type_layout_v2(
                tcx,
                local_static_type(tcx, "C_VALUE"),
                &observed,
                SemanticTypeLayoutBudgetsV2 {
                    max_path_bytes: 3_075,
                    ..SemanticTypeLayoutBudgetsV2::default()
                },
            )
            .err();
            self.dst_bounded = observe_rustc_type_layout_v2(
                tcx,
                local_static_type(tcx, "SLICE_VALUE"),
                &observed,
                SemanticTypeLayoutBudgetsV2 {
                    max_sidecar_records: 1,
                    ..SemanticTypeLayoutBudgetsV2::default()
                },
            )
            .err();
            Compilation::Stop
        }
    }

    fn local_static_type<'tcx>(tcx: TyCtxt<'tcx>, name: &str) -> Ty<'tcx> {
        let definition = tcx
            .iter_local_def_id()
            .find(|definition| {
                matches!(tcx.def_kind(definition.to_def_id()), DefKind::Static { .. })
                    && tcx.item_name(definition.to_def_id()).as_str() == name
            })
            .unwrap_or_else(|| panic!("missing fixture static {name}"));
        tcx.type_of(definition).instantiate_identity()
    }

    struct FixtureFiles {
        source: PathBuf,
        output: PathBuf,
    }

    impl FixtureFiles {
        fn create() -> Self {
            let stem = format!("fe2o3-semantic-type-v2-{}", std::process::id());
            let source = std::env::temp_dir().join(format!("{stem}.rs"));
            let output = std::env::temp_dir().join(format!("{stem}.rmeta"));
            fs::write(&source, FIXTURE).expect("write semantic type fixture");
            Self { source, output }
        }
    }

    impl Drop for FixtureFiles {
        fn drop(&mut self) {
            let _ = fs::remove_file(&self.source);
            let _ = fs::remove_file(&self.output);
        }
    }

    fn captures() -> CaptureCallbacks {
        let fixture = FixtureFiles::create();
        let sysroot = Command::new("rustc")
            .args(["--print", "sysroot"])
            .output()
            .expect("query pinned rustc sysroot");
        assert!(sysroot.status.success());
        let sysroot = String::from_utf8(sysroot.stdout).unwrap();
        let args = vec![
            "rustc".to_owned(),
            "--crate-name".to_owned(),
            "fe2o3_semantic_type_v2_fixture".to_owned(),
            "--crate-type".to_owned(),
            "lib".to_owned(),
            "--edition".to_owned(),
            "2024".to_owned(),
            "--emit".to_owned(),
            "metadata".to_owned(),
            "--sysroot".to_owned(),
            sysroot.trim().to_owned(),
            "-o".to_owned(),
            fixture.output.display().to_string(),
            fixture.source.display().to_string(),
        ];
        let mut callbacks = CaptureCallbacks::default();
        rustc_driver::run_compiler(&args, &mut callbacks);
        callbacks
    }

    #[derive(Clone, Copy)]
    #[repr(C)]
    struct HostInner {
        left: u16,
        right: u16,
    }

    #[derive(Clone, Copy)]
    #[repr(C)]
    struct HostRoot {
        head: u32,
        nested: [HostInner; 2],
        tail: u32,
    }

    #[test]
    fn active_rustc_observation_preserves_layout_and_trust_boundaries() {
        let results = captures();
        assert!(results.reobserved);
        assert!(matches!(
            results.pointer_niche,
            Some(SemanticTypeAdapterErrorV2::Unsupported { .. })
        ));
        assert!(matches!(
            results.mismatch,
            Some(SemanticTypeAdapterErrorV2::TargetMismatch { .. })
        ));
        assert!(results.bounded.is_some());
        assert!(matches!(
            results.name_bounded,
            Some(SemanticTypeAdapterErrorV2::BoundExceeded {
                resource: "rustc type name bytes",
                actual: 5,
                max: 4,
            })
        ));
        assert!(matches!(
            results.nested_name_bounded,
            Some(SemanticTypeAdapterErrorV2::BoundExceeded {
                resource: "rustc type name bytes",
                ..
            })
        ));
        assert!(matches!(
            results.work_bounded,
            Some(SemanticTypeAdapterErrorV2::BoundExceeded {
                resource: "rustc layout preflight work",
                actual: 1,
                max: 0,
            })
        ));
        assert!(matches!(
            results.path_bounded,
            Some(SemanticTypeAdapterErrorV2::BoundExceeded {
                resource: "rustc layout path bytes",
                actual: 3_076,
                max: 3_075,
            })
        ));
        assert!(matches!(
            results.dst_bounded,
            Some(SemanticTypeAdapterErrorV2::BoundExceeded {
                resource: "rustc layout sidecar records",
                ..
            })
        ));

        let c = &results.captures["C_VALUE"];
        assert!(!c.was_observed_on_canonical_gfx942_target());
        let c_record = c.layout_record(c.graph().root_key()).unwrap();
        assert!(c_record.representation().unwrap().c);
        assert!(!c_record.representation().unwrap().transparent);
        assert_eq!(c_record.aggregates()[0].source_to_memory(), &[0, 1, 2]);
        assert!(c_record.aggregates()[0].padding().is_empty());

        let rust = &results.captures["RUST_VALUE"];
        let rust_record = rust.layout_record(rust.graph().root_key()).unwrap();
        assert!(!rust_record.representation().unwrap().c);
        let order = rust_record.aggregates()[0].source_to_memory();
        let mut sorted = order.to_vec();
        sorted.sort_unstable();
        assert_eq!(sorted, [0, 1, 2, 3]);

        let padded = &results.captures["PADDED_VALUE"];
        let padded_record = padded.layout_record(padded.graph().root_key()).unwrap();
        assert_eq!(
            padded_record.aggregates()[0].padding(),
            &[RustcByteRangeV2 { start: 1, end: 4 }]
        );

        let array = &results.captures["ARRAY_VALUE"];
        let array_record = array.layout_record(array.graph().root_key()).unwrap();
        assert_eq!(array_record.array_stride_bytes(), Some(4));

        let direct = &results.captures["DIRECT_VALUE"];
        assert!(matches!(
            &direct.graph().node(direct.graph().root()).unwrap().kind,
            SemanticTypeKindV2::Enum {
                encoding: SemanticEnumEncodingV2::Direct { .. },
                ..
            }
        ));
        let niche = &results.captures["NICHE_VALUE"];
        assert!(matches!(
            &niche.graph().node(niche.graph().root()).unwrap().kind,
            SemanticTypeKindV2::Enum {
                encoding: SemanticEnumEncodingV2::Niche { .. },
                ..
            }
        ));
        let union = &results.captures["BITS_VALUE"];
        assert!(matches!(
            union.graph().node(union.graph().root()).unwrap().kind,
            SemanticTypeKindV2::Union { .. }
        ));

        let slice = &results.captures["SLICE_VALUE"];
        assert!(matches!(
            &slice.graph().node(slice.graph().root()).unwrap().kind,
            SemanticTypeKindV2::Reference {
                metadata: PointerMetadataV2::SliceLength,
                ..
            }
        ));
        let dynamic = &results.captures["DYN_VALUE"];
        assert!(matches!(
            &dynamic.graph().node(dynamic.graph().root()).unwrap().kind,
            SemanticTypeKindV2::Reference {
                metadata: PointerMetadataV2::VTable { .. },
                ..
            }
        ));
    }

    #[test]
    fn malformed_untrusted_candidates_never_reuse_the_exact_identity() {
        let results = captures();
        let capture = &results.captures["C_VALUE"];
        let canonical = capture.graph_bytes();
        let budgets = SemanticTypeLayoutBudgetsV2::default().graph;
        let mut state = 0x9e37_79b9_u32;
        for round in 0..25_000_u32 {
            state ^= state << 13;
            state ^= state >> 17;
            state ^= state << 5;
            let mut candidate = canonical.to_vec();
            let index = (state as usize) % candidate.len();
            candidate[index] ^= 1 << (round % 8);
            if let Ok(decoded) = SemanticTypeGraphV2::decode_canonical(&candidate, budgets) {
                assert_ne!(decoded.canonical_bytes().unwrap(), canonical);
            }
        }
    }

    #[test]
    fn gfx942_layout_candidate_is_exact_conservative_and_inert() {
        let results = captures();
        let c = &results.captures["C_VALUE"];
        let budgets = SemanticTypeLayoutBudgetsV2::default();
        let candidate = derive_gfx942_layout_compatibility_candidate_v2(c, budgets)
            .expect("padding-free repr(C) fixture is layout compatible");
        assert_eq!(candidate.observation_identity_sha256(), c.identity_sha256());
        assert_eq!(candidate.root_key(), c.graph().root_key());
        validate_gfx942_layout_compatibility_candidate_v2(c, &candidate, budgets).unwrap();

        let array = &results.captures["ARRAY_VALUE"];
        assert!(derive_gfx942_layout_compatibility_candidate_v2(array, budgets).is_ok());
        let transparent = &results.captures["TRANSPARENT_VALUE"];
        assert!(derive_gfx942_layout_compatibility_candidate_v2(transparent, budgets).is_ok());
        for accepted in ["U64_VALUE", "F64_VALUE"] {
            assert!(
                derive_gfx942_layout_compatibility_candidate_v2(
                    &results.captures[accepted],
                    budgets,
                )
                .is_ok()
            );
        }
        for rejected in [
            "RUST_VALUE",
            "PADDED_VALUE",
            "TUPLE_VALUE",
            "DIRECT_VALUE",
            "NICHE_VALUE",
            "BITS_VALUE",
            "SLICE_VALUE",
            "DYN_VALUE",
            "U128_VALUE",
            "BOOL_VALUE",
        ] {
            let capture = &results.captures[rejected];
            assert!(matches!(
                derive_gfx942_layout_compatibility_candidate_v2(capture, budgets),
                Err(Gfx942LayoutCompatibilityErrorV2::Unsupported { .. })
            ));
        }

        let mut substituted_observation = c.clone();
        substituted_observation.rustc_target = SemanticLayoutTargetV1::new(
            "substituted-host-target",
            c.rustc_target().data_layout(),
            c.rustc_target().default_pointer_width_bits(),
        )
        .unwrap();
        assert!(matches!(
            derive_gfx942_layout_compatibility_candidate_v2(&substituted_observation, budgets),
            Err(Gfx942LayoutCompatibilityErrorV2::ObservationIdentityMismatch)
        ));

        let mut mutated_record = c.clone();
        mutated_record.layout_records[0].array_stride_bytes = Some(123);
        assert!(matches!(
            derive_gfx942_layout_compatibility_candidate_v2(&mutated_record, budgets),
            Err(Gfx942LayoutCompatibilityErrorV2::ObservationIdentityMismatch)
        ));

        let mut mutated_graph_bytes = c.clone();
        mutated_graph_bytes.graph_bytes[0] ^= 1;
        assert!(matches!(
            derive_gfx942_layout_compatibility_candidate_v2(&mutated_graph_bytes, budgets),
            Err(Gfx942LayoutCompatibilityErrorV2::ObservationIdentityMismatch)
        ));

        let mut mutated_projection = candidate.clone();
        mutated_projection.projection.target.cpu = "gfx941".to_owned();
        assert!(matches!(
            validate_gfx942_layout_compatibility_candidate_v2(c, &mutated_projection, budgets),
            Err(Gfx942LayoutCompatibilityErrorV2::ProjectionMismatch)
        ));

        // Compiler/source digests and generations are deliberately absent
        // from both APIs and identities; changing caller-local declarations
        // cannot change this candidate.
        let caller_declarations_a = ([0x31_u8; 32], [0x52_u8; 32], 7_u64);
        let caller_declarations_b = ([0x99_u8; 32], [0xaa_u8; 32], u64::MAX);
        assert_ne!(caller_declarations_a, caller_declarations_b);
        let repeated = derive_gfx942_layout_compatibility_candidate_v2(c, budgets).unwrap();
        assert_eq!(candidate, repeated);

        let value = HostRoot {
            head: 1,
            nested: [
                HostInner { left: 2, right: 3 },
                HostInner { left: 4, right: 5 },
            ],
            tail: 6,
        };
        assert_eq!(std::mem::size_of::<HostRoot>(), 16);
        // SAFETY: the asserted repr(C) layout has no padding, and `value`
        // remains alive for the complete immutable slice borrow.
        let host_bytes = unsafe {
            std::slice::from_raw_parts(
                std::ptr::from_ref(&value).cast::<u8>(),
                std::mem::size_of::<HostRoot>(),
            )
        };
        let gfx942_bytes = [1, 0, 0, 0, 2, 0, 3, 0, 4, 0, 5, 0, 6, 0, 0, 0];
        let differential =
            compare_host_device_fixture_bytes_v2(&candidate, host_bytes, &gfx942_bytes, 16)
                .expect("host and gfx942 fixture bytes match");
        assert_eq!(differential.byte_length(), 16);
        assert_eq!(
            differential.candidate_identity_sha256(),
            candidate.candidate_identity_sha256()
        );

        let mut wrong = gfx942_bytes;
        wrong[9] ^= 1;
        assert!(matches!(
            compare_host_device_fixture_bytes_v2(&candidate, host_bytes, &wrong, 16),
            Err(Gfx942LayoutCompatibilityErrorV2::HostDeviceByteMismatch)
        ));
        assert!(matches!(
            compare_host_device_fixture_bytes_v2(&candidate, host_bytes, host_bytes, 15),
            Err(Gfx942LayoutCompatibilityErrorV2::ByteLengthExceeded { .. })
        ));
    }

    #[test]
    fn projection_limits_fail_at_max_plus_one_and_on_overflow() {
        let results = captures();
        let c = &results.captures["C_VALUE"];
        let error = derive_gfx942_layout_compatibility_candidate_v2(
            c,
            SemanticTypeLayoutBudgetsV2 {
                max_projection_work: 0,
                ..SemanticTypeLayoutBudgetsV2::default()
            },
        )
        .unwrap_err();
        assert_eq!(
            error,
            Gfx942LayoutCompatibilityErrorV2::WorkBoundExceeded { actual: 1, max: 0 }
        );
        assert!(matches!(
            align_up(u64::MAX, 8, "overflow"),
            Err(Gfx942LayoutCompatibilityErrorV2::ArithmeticOverflow { .. })
        ));
    }

    #[test]
    #[ignore = "requires ROCm LLVM 22, Ubuntu LLVM 18, and gfx942 target support"]
    fn reviewed_gfx942_projection_matches_llvm_record_and_code_object_probes() {
        const SOURCE: &str = r#"
typedef struct { unsigned short left; unsigned short right; } Inner;
typedef struct { unsigned head; Inner nested[2]; unsigned tail; } Root;
typedef struct { float narrow; double wide; unsigned long long integer; } Wide;
__attribute__((amdgpu_kernel)) void layout_probe(Root *out) { out[0].tail = 7; }
"#;
        for clang in ["/opt/rocm/llvm/bin/clang", "/usr/bin/clang-18"] {
            let mut child = Command::new(clang)
                .args([
                    "-x",
                    "c",
                    "-",
                    "--target=amdgcn-amd-amdhsa",
                    "-mcpu=gfx942",
                    "-nogpulib",
                    "-Xclang",
                    "-fdump-record-layouts-complete",
                    "-fsyntax-only",
                ])
                .stdin(Stdio::piped())
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .spawn()
                .unwrap_or_else(|error| panic!("start {clang}: {error}"));
            child
                .stdin
                .take()
                .unwrap()
                .write_all(SOURCE.as_bytes())
                .unwrap();
            let output = child.wait_with_output().unwrap();
            assert!(
                output.status.success(),
                "{clang}: {}",
                String::from_utf8_lossy(&output.stderr)
            );
            let dump = format!(
                "{}{}",
                String::from_utf8_lossy(&output.stdout),
                String::from_utf8_lossy(&output.stderr)
            );
            for expected in [
                "0 |   unsigned short left",
                "2 |   unsigned short right",
                "[sizeof=4, align=2]",
                "0 |   unsigned int head",
                "4 |   Inner[2] nested",
                "12 |   unsigned int tail",
                "[sizeof=16, align=4]",
                "0 |   float narrow",
                "8 |   double wide",
                "16 |   unsigned long long integer",
                "[sizeof=24, align=8]",
            ] {
                assert!(
                    dump.contains(expected),
                    "{clang} omitted {expected:?}:\n{dump}"
                );
            }
        }

        let object = std::env::temp_dir().join(format!(
            "fe2o3-gfx942-layout-probe-{}.o",
            std::process::id()
        ));
        let mut child = Command::new("/opt/rocm/llvm/bin/clang")
            .args([
                "-x",
                "c",
                "-",
                "--target=amdgcn-amd-amdhsa",
                "-mcpu=gfx942",
                "-nogpulib",
                "-c",
                "-o",
            ])
            .arg(&object)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .expect("start ROCm clang code-object probe");
        child
            .stdin
            .take()
            .unwrap()
            .write_all(SOURCE.as_bytes())
            .unwrap();
        let compile = child.wait_with_output().unwrap();
        assert!(
            compile.status.success(),
            "{}",
            String::from_utf8_lossy(&compile.stderr)
        );
        let readelf = Command::new("/opt/rocm/llvm/bin/llvm-readelf")
            .args(["--file-header", "--notes"])
            .arg(&object)
            .output()
            .expect("inspect gfx942 code object");
        let _ = fs::remove_file(&object);
        assert!(readelf.status.success());
        let inspection = String::from_utf8(readelf.stdout).unwrap();
        assert!(inspection.contains("Machine:                           EM_AMDGPU"));
        assert!(inspection.contains("gfx942"));
        assert!(inspection.contains("amdhsa.target:   amdgcn-amd-amdhsa--gfx942"));
    }
}

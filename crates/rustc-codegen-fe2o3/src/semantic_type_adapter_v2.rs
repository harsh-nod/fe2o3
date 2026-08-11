//! Exact rustc type-layout capture over the untrusted dialect MIR V2 graph.
//!
//! The three boundaries in this module are intentionally distinct:
//! [`SemanticTypeGraphV2`] bytes are untrusted transport input,
//! [`AuthenticatedRustcTypeCaptureV2`] is an exact observation from the active
//! pinned rustc session, and neither value grants manifest, device-copy, code
//! generation, loading, or launch authority.

use std::collections::BTreeMap;
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

const CAPTURE_DOMAIN_V2: &[u8] = b"FE2O3/RUSTC-TYPE-CAPTURE/V2\0";
const DEFAULT_MAX_SIDECAR_RECORDS: u32 = 32_768;
const DEFAULT_MAX_SIDECAR_BYTES: u32 = 8 * 1024 * 1024;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SemanticTypeCaptureBudgetsV2 {
    pub graph: SemanticTypeGraphBudgetsV2,
    pub max_sidecar_records: u32,
    pub max_sidecar_bytes: u32,
}

impl Default for SemanticTypeCaptureBudgetsV2 {
    fn default() -> Self {
        Self {
            graph: SemanticTypeGraphBudgetsV2::default(),
            max_sidecar_records: DEFAULT_MAX_SIDECAR_RECORDS,
            max_sidecar_bytes: DEFAULT_MAX_SIDECAR_BYTES,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Gfx942TypeTargetV2 {
    rustc_target: SemanticLayoutTargetV1,
    device_triple: String,
    device_cpu: String,
    device_features: String,
    device_data_layout: String,
}

impl Gfx942TypeTargetV2 {
    pub fn new(rustc_target: SemanticLayoutTargetV1) -> Self {
        Self {
            rustc_target,
            device_triple: GFX942_TARGET_TRIPLE.to_owned(),
            device_cpu: GFX942_TARGET_CPU.to_owned(),
            device_features: GFX942_TARGET_FEATURES.to_owned(),
            device_data_layout: GFX942_TARGET_DATA_LAYOUT.to_owned(),
        }
    }

    pub const fn rustc_target(&self) -> &SemanticLayoutTargetV1 {
        &self.rustc_target
    }

    pub fn device_triple(&self) -> &str {
        &self.device_triple
    }

    pub fn device_cpu(&self) -> &str {
        &self.device_cpu
    }

    pub fn device_features(&self) -> &str {
        &self.device_features
    }

    pub fn device_data_layout(&self) -> &str {
        &self.device_data_layout
    }

    fn is_exact_gfx942(&self) -> bool {
        self.device_triple == GFX942_TARGET_TRIPLE
            && self.device_cpu == GFX942_TARGET_CPU
            && self.device_features == GFX942_TARGET_FEATURES
            && self.device_data_layout == GFX942_TARGET_DATA_LAYOUT
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RustcCaptureRevisionV2 {
    pub compiler_sha256: [u8; 32],
    pub source_sha256: [u8; 32],
    pub generation: u64,
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
pub struct AuthenticatedRustcTypeCaptureV2 {
    target: Gfx942TypeTargetV2,
    revision: RustcCaptureRevisionV2,
    graph: SemanticTypeGraphV2,
    graph_bytes: Vec<u8>,
    layout_records: Vec<RustcTypeLayoutRecordV2>,
    identity_sha256: [u8; 32],
}

impl AuthenticatedRustcTypeCaptureV2 {
    pub const fn target(&self) -> &Gfx942TypeTargetV2 {
        &self.target
    }

    pub const fn revision(&self) -> RustcCaptureRevisionV2 {
        self.revision
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

    pub fn layout_record(&self, key: &str) -> Option<&RustcTypeLayoutRecordV2> {
        self.layout_records
            .binary_search_by(|record| record.key.as_str().cmp(key))
            .ok()
            .map(|index| &self.layout_records[index])
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SemanticTypeAdapterErrorV2 {
    TargetMismatch {
        expected: SemanticLayoutTargetV1,
        observed: SemanticLayoutTargetV1,
    },
    NotGfx942,
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
            Self::NotGfx942 => formatter.write_str("device target is not the exact gfx942 profile"),
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
            Self::Graph(error) => {
                write!(formatter, "semantic type graph rejected capture: {error}")
            }
            Self::UntrustedGraphMismatch => formatter
                .write_str("untrusted semantic type graph differs from the exact rustc capture"),
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

pub fn capture_rustc_type_for_gfx942_v2<'tcx>(
    tcx: TyCtxt<'tcx>,
    ty: Ty<'tcx>,
    expected_target: &Gfx942TypeTargetV2,
    revision: RustcCaptureRevisionV2,
    budgets: SemanticTypeCaptureBudgetsV2,
) -> Result<AuthenticatedRustcTypeCaptureV2, SemanticTypeAdapterErrorV2> {
    if !expected_target.is_exact_gfx942() {
        return Err(SemanticTypeAdapterErrorV2::NotGfx942);
    }
    let observed = rustc_semantic_layout_target_v1(tcx)?;
    if expected_target.rustc_target() != &observed {
        return Err(SemanticTypeAdapterErrorV2::TargetMismatch {
            expected: expected_target.rustc_target().clone(),
            observed,
        });
    }

    let mut capture = if is_unsized_pointer(ty, tcx) {
        capture_unsized_pointer(tcx, ty, budgets)?
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
        CaptureBuilderV2::new(budgets).finish(&facts)?
    };
    capture
        .layout_records
        .sort_by(|left, right| left.key.cmp(&right.key));
    let sidecar = encode_sidecar(&capture.layout_records, budgets.max_sidecar_bytes)?;
    let identity_sha256 =
        capture_identity(expected_target, revision, &capture.graph_bytes, &sidecar);
    Ok(AuthenticatedRustcTypeCaptureV2 {
        target: expected_target.clone(),
        revision,
        graph: capture.graph,
        graph_bytes: capture.graph_bytes,
        layout_records: capture.layout_records,
        identity_sha256,
    })
}

pub fn authenticate_untrusted_type_graph_for_gfx942_v2<'tcx>(
    tcx: TyCtxt<'tcx>,
    ty: Ty<'tcx>,
    untrusted_graph_bytes: &[u8],
    expected_target: &Gfx942TypeTargetV2,
    revision: RustcCaptureRevisionV2,
    budgets: SemanticTypeCaptureBudgetsV2,
) -> Result<AuthenticatedRustcTypeCaptureV2, SemanticTypeAdapterErrorV2> {
    let decoded = SemanticTypeGraphV2::decode_canonical(untrusted_graph_bytes, budgets.graph)?;
    let canonical = decoded.canonical_bytes()?;
    let capture = capture_rustc_type_for_gfx942_v2(tcx, ty, expected_target, revision, budgets)?;
    if canonical != capture.graph_bytes || untrusted_graph_bytes != capture.graph_bytes {
        return Err(SemanticTypeAdapterErrorV2::UntrustedGraphMismatch);
    }
    Ok(capture)
}

struct PendingCaptureV2 {
    graph: SemanticTypeGraphV2,
    graph_bytes: Vec<u8>,
    layout_records: Vec<RustcTypeLayoutRecordV2>,
}

struct CaptureBuilderV2 {
    budgets: SemanticTypeCaptureBudgetsV2,
    graph: SemanticTypeGraphBuilderV2,
    by_type: BTreeMap<String, SemanticTypeNodeIdV2>,
    records: Vec<RustcTypeLayoutRecordV2>,
}

impl CaptureBuilderV2 {
    fn new(budgets: SemanticTypeCaptureBudgetsV2) -> Self {
        Self {
            budgets,
            graph: SemanticTypeGraphBuilderV2::new(budgets.graph),
            by_type: BTreeMap::new(),
            records: Vec::new(),
        }
    }

    fn finish(
        mut self,
        root: &TypeLayoutFacts,
    ) -> Result<PendingCaptureV2, SemanticTypeAdapterErrorV2> {
        let root = self.intern(root, "root")?;
        let graph = self.graph.finish(root)?;
        let graph_bytes = graph.canonical_bytes()?;
        Ok(PendingCaptureV2 {
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

    fn reserve_record(&self) -> Result<(), SemanticTypeAdapterErrorV2> {
        let actual = self.records.len() as u64 + 1;
        let max = u64::from(self.budgets.max_sidecar_records);
        if actual > max {
            return Err(SemanticTypeAdapterErrorV2::BoundExceeded {
                resource: "rustc layout sidecar records",
                actual,
                max,
            });
        }
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
                "uninhabited type is not represented by this capture profile",
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
                        "array stride differs from the captured element size",
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
                record
                    .aggregates
                    .push(aggregate_record(path, fields, facts.size_bytes, &[])?);
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
        let mut converted = Vec::with_capacity(fields.len());
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
                record
                    .aggregates
                    .push(aggregate_record(path, fields, facts.size_bytes, &[])?);
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
        let mut variants = Vec::with_capacity(adt.variants.len());
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
    let mut source_to_memory = Vec::with_capacity(fields.len());
    let mut seen_memory = vec![false; fields.len()];
    let mut occupied = reserved.to_vec();
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

fn capture_unsized_pointer<'tcx>(
    tcx: TyCtxt<'tcx>,
    ty: Ty<'tcx>,
    budgets: SemanticTypeCaptureBudgetsV2,
) -> Result<PendingCaptureV2, SemanticTypeAdapterErrorV2> {
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
    let root_key = type_name(ty);
    let pointee_key = type_name(pointee);
    let root = graph.declare(root_key.clone())?;
    let pointee_id = graph.declare(pointee_key.clone())?;
    let pointee_layout = layout_cx
        .layout_of(pointee)
        .map_err(|error| SemanticTypeAdapterErrorV2::Extraction(error.to_string()))?;
    let mut records = vec![RustcTypeLayoutRecordV2 {
        key: root_key.clone(),
        representation: None,
        aggregates: Vec::new(),
        array_stride_bytes: None,
        uninhabited: false,
    }];
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
    Ok(PendingCaptureV2 {
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

fn capture_identity(
    target: &Gfx942TypeTargetV2,
    revision: RustcCaptureRevisionV2,
    graph: &[u8],
    sidecar: &[u8],
) -> [u8; 32] {
    let mut digest = Sha256::new();
    digest.update(CAPTURE_DOMAIN_V2);
    hash_component(&mut digest, target.rustc_target.llvm_target().as_bytes());
    hash_component(&mut digest, target.rustc_target.data_layout().as_bytes());
    digest.update(
        target
            .rustc_target
            .default_pointer_width_bits()
            .to_le_bytes(),
    );
    hash_component(&mut digest, target.device_triple.as_bytes());
    hash_component(&mut digest, target.device_cpu.as_bytes());
    hash_component(&mut digest, target.device_features.as_bytes());
    hash_component(&mut digest, target.device_data_layout.as_bytes());
    digest.update(revision.compiler_sha256);
    digest.update(revision.source_sha256);
    digest.update(revision.generation.to_le_bytes());
    hash_component(&mut digest, graph);
    hash_component(&mut digest, sidecar);
    digest.finalize().into()
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
    let actual = output.len().checked_add(bytes.len()).unwrap_or(usize::MAX);
    if actual > max as usize {
        return Err(SemanticTypeAdapterErrorV2::BoundExceeded {
            resource: "rustc layout sidecar bytes",
            actual: actual as u64,
            max: u64::from(max),
        });
    }
    output.extend_from_slice(bytes);
    Ok(())
}

fn type_name(ty: Ty<'_>) -> String {
    with_no_trimmed_paths!(format!("{ty}"))
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
    use std::path::PathBuf;
    use std::process::Command;

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
static TUPLE_VALUE: (u8, u32, [u16; 2]) = (1, 2, [3, 4]);
static ARRAY_VALUE: [Inner; 2] = [Inner { left: 1, right: 2 }, Inner { left: 3, right: 4 }];
static DIRECT_VALUE: Direct = Direct::Empty;
static NICHE_VALUE: Niche = Niche::Empty;
static BITS_VALUE: Bits = Bits { integer: 7 };
static SLICE_VALUE: &[u8] = &[1, 2, 3];
static DYN_VALUE: &dyn Marker = &MARKER;
static BYTE: u8 = 7;
static POINTER_NICHE: Option<&u8> = Some(&BYTE);
"#;

    fn revision() -> RustcCaptureRevisionV2 {
        RustcCaptureRevisionV2 {
            compiler_sha256: [0x31; 32],
            source_sha256: [0x52; 32],
            generation: 7,
        }
    }

    #[derive(Default)]
    struct CaptureCallbacks {
        captures: BTreeMap<String, AuthenticatedRustcTypeCaptureV2>,
        pointer_niche: Option<SemanticTypeAdapterErrorV2>,
        mismatch: Option<SemanticTypeAdapterErrorV2>,
        bounded: Option<SemanticTypeAdapterErrorV2>,
        reauthenticated: bool,
    }

    impl Callbacks for CaptureCallbacks {
        fn after_analysis<'tcx>(&mut self, _compiler: &Compiler, tcx: TyCtxt<'tcx>) -> Compilation {
            let observed = rustc_semantic_layout_target_v1(tcx).unwrap();
            let target = Gfx942TypeTargetV2::new(observed.clone());
            let budgets = SemanticTypeCaptureBudgetsV2::default();
            for name in [
                "C_VALUE",
                "RUST_VALUE",
                "PADDED_VALUE",
                "TUPLE_VALUE",
                "ARRAY_VALUE",
                "DIRECT_VALUE",
                "NICHE_VALUE",
                "BITS_VALUE",
                "SLICE_VALUE",
                "DYN_VALUE",
            ] {
                let ty = local_static_type(tcx, name);
                let capture =
                    capture_rustc_type_for_gfx942_v2(tcx, ty, &target, revision(), budgets)
                        .unwrap_or_else(|error| panic!("capture {name}: {error}"));
                if name == "C_VALUE" {
                    let reauthenticated = authenticate_untrusted_type_graph_for_gfx942_v2(
                        tcx,
                        ty,
                        capture.graph_bytes(),
                        &target,
                        revision(),
                        budgets,
                    )
                    .expect("reauthenticate exact rustc graph");
                    self.reauthenticated =
                        reauthenticated.identity_sha256() == capture.identity_sha256();
                }
                self.captures.insert(name.to_owned(), capture);
            }

            self.pointer_niche = capture_rustc_type_for_gfx942_v2(
                tcx,
                local_static_type(tcx, "POINTER_NICHE"),
                &target,
                revision(),
                budgets,
            )
            .err();
            let different = Gfx942TypeTargetV2::new(
                SemanticLayoutTargetV1::new(
                    "different-rustc-target",
                    observed.data_layout(),
                    observed.default_pointer_width_bits(),
                )
                .unwrap(),
            );
            self.mismatch = capture_rustc_type_for_gfx942_v2(
                tcx,
                local_static_type(tcx, "C_VALUE"),
                &different,
                revision(),
                budgets,
            )
            .err();
            self.bounded = capture_rustc_type_for_gfx942_v2(
                tcx,
                local_static_type(tcx, "C_VALUE"),
                &target,
                revision(),
                SemanticTypeCaptureBudgetsV2 {
                    graph: SemanticTypeGraphBudgetsV2 {
                        max_nodes: 1,
                        ..SemanticTypeGraphBudgetsV2::default()
                    },
                    ..SemanticTypeCaptureBudgetsV2::default()
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

    #[test]
    fn pinned_rustc_capture_preserves_general_layout_and_trust_boundaries() {
        let results = captures();
        assert!(results.reauthenticated);
        assert!(matches!(
            results.pointer_niche,
            Some(SemanticTypeAdapterErrorV2::Unsupported { .. })
        ));
        assert!(matches!(
            results.mismatch,
            Some(SemanticTypeAdapterErrorV2::TargetMismatch { .. })
        ));
        assert!(results.bounded.is_some());

        let c = &results.captures["C_VALUE"];
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
        let budgets = SemanticTypeCaptureBudgetsV2::default().graph;
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
}

//! Target-explicit conversion of bounded rustc layout facts into dialect MIR.
//!
//! The records in this module are observational compiler evidence. They carry
//! no code-generation, artifact, proof, device, or launch authority.

use std::collections::BTreeMap;
use std::fmt::{self, Write};

use dialect_mir::{
    MirAddressSpace, MirAggregateLayout, MirEnumEncoding, MirEnumType, MirField, MirLayout,
    MirMutability, MirPadding, MirScalarType, MirSemanticType, MirStructType, MirTypeKind,
    MirTypeValidationError, MirVariant,
};
use fe2o3_rustc_front::StableTypeIdentityV1;
use rustc_abi::HasDataLayout;
use rustc_middle::ty::layout::LayoutCx;
use rustc_middle::ty::{Ty, TyCtxt, TypingEnv};

use crate::rust_type_layout_general::{
    AdtKind, AdtLayoutFacts, BackendRepresentationFacts, EnumTagEncodingFacts, FieldLayoutFacts,
    GeneralLayoutExtractError, PointerKind, ScalarLayoutFacts, ScalarPrimitiveFacts,
    SourceScalarKind, TypeLayoutFacts, TypeLayoutKind, VariantLayoutFacts, extract_general_layout,
};

/// Schema identity included in every canonical semantic-layout record.
pub const SEMANTIC_LAYOUT_EVIDENCE_SCHEMA_V1: &str = "fe2o3.semantic-layout-evidence.v1";
/// Numeric version paired with [`SEMANTIC_LAYOUT_EVIDENCE_SCHEMA_V1`].
pub const SEMANTIC_LAYOUT_EVIDENCE_VERSION_V1: u16 = 1;
/// Maximum byte length of either exact rustc target identity component.
pub const MAX_SEMANTIC_LAYOUT_TARGET_TEXT_BYTES_V1: usize = 16 * 1024;
/// Maximum nesting depth accepted before recursive dialect validation.
pub const MAX_SEMANTIC_LAYOUT_DEPTH_V1: usize = 64;
/// Maximum number of semantic type nodes in one evidence record.
pub const MAX_SEMANTIC_LAYOUT_TYPE_NODES_V1: usize = 4_096;
/// Maximum fields in any one aggregate.
pub const MAX_SEMANTIC_LAYOUT_FIELDS_V1: usize = 1_024;
/// Maximum variants in one enum.
pub const MAX_SEMANTIC_LAYOUT_VARIANTS_V1: usize = 256;
/// Maximum aggregate byte length of semantic identity and field-name text.
pub const MAX_SEMANTIC_LAYOUT_TYPE_TEXT_BYTES_V1: usize = 2 * 1024 * 1024;
/// Maximum canonical byte length of one evidence record.
pub const MAX_SEMANTIC_LAYOUT_EVIDENCE_BYTES_V1: usize = 4 * 1024 * 1024;

/// Exact rustc target context under which layout facts were observed.
///
/// This is deliberately distinct from an AMDGPU launch target. The rustc
/// target and data-layout strings identify the ABI used to compute the facts.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SemanticLayoutTargetV1 {
    llvm_target: Box<str>,
    data_layout: Box<str>,
    default_pointer_width_bits: u16,
    active_codegen_profile: Option<Box<ActiveCodegenProfileV1>>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct ActiveCodegenProfileV1 {
    cpu: Option<String>,
    features: String,
}

impl SemanticLayoutTargetV1 {
    pub fn new(
        llvm_target: impl Into<String>,
        data_layout: impl Into<String>,
        default_pointer_width_bits: u16,
    ) -> Result<Self, SemanticLayoutBridgeError> {
        let llvm_target = llvm_target.into();
        let data_layout = data_layout.into();
        validate_target_text("rustc LLVM target", &llvm_target)?;
        validate_target_text("rustc data layout", &data_layout)?;
        if default_pointer_width_bits == 0
            || default_pointer_width_bits > 128
            || !default_pointer_width_bits.is_power_of_two()
            || !default_pointer_width_bits.is_multiple_of(8)
        {
            return Err(SemanticLayoutBridgeError::InvalidTarget {
                field: "default pointer width",
                detail: "must be an 8..=128 power-of-two bit width".to_owned(),
            });
        }
        Ok(Self {
            llvm_target: llvm_target.into_boxed_str(),
            data_layout: data_layout.into_boxed_str(),
            default_pointer_width_bits,
            active_codegen_profile: None,
        })
    }

    /// Constructs a target identity with the effective rustc codegen profile.
    ///
    /// `target_features` is the target-spec baseline and `codegen_features` is
    /// the active `-Ctarget-feature` override. Conflicting declarations within
    /// either component are rejected. An override may replace one baseline
    /// feature because that is how rustc computes the effective configuration.
    pub fn new_with_codegen_profile(
        llvm_target: impl Into<String>,
        data_layout: impl Into<String>,
        default_pointer_width_bits: u16,
        active_cpu: &str,
        target_features: &str,
        codegen_features: &str,
    ) -> Result<Self, SemanticLayoutBridgeError> {
        let mut target = Self::new(llvm_target, data_layout, default_pointer_width_bits)?;
        target.active_codegen_profile = Some(Box::new(ActiveCodegenProfileV1 {
            cpu: normalize_active_cpu(active_cpu)?,
            features: normalize_active_features(target_features, codegen_features)?,
        }));
        Ok(target)
    }

    pub fn llvm_target(&self) -> &str {
        &self.llvm_target
    }

    pub fn data_layout(&self) -> &str {
        &self.data_layout
    }

    pub const fn default_pointer_width_bits(&self) -> u16 {
        self.default_pointer_width_bits
    }

    /// Effective target CPU selected by the active rustc session.
    ///
    /// `None` means that rustc exposed only an ambiguous/default/native CPU.
    pub fn active_cpu(&self) -> Option<&str> {
        self.active_codegen_profile
            .as_ref()
            .and_then(|profile| profile.cpu.as_deref())
    }

    /// Canonical effective target-feature set selected by the active session.
    ///
    /// Entries are sorted by feature name and duplicate declarations are
    /// collapsed. `None` means no active-session profile was captured.
    pub fn active_features(&self) -> Option<&str> {
        self.active_codegen_profile
            .as_ref()
            .map(|profile| profile.features.as_str())
    }

    pub fn has_exact_codegen_profile(&self, cpu: &str, features: &str) -> bool {
        self.active_cpu() == Some(cpu) && self.active_features() == Some(features)
    }

    fn write_canonical(&self, output: &mut String) {
        write!(
            output,
            "target(llvm={}:{};data-layout={}:{};default-pointer-bits={};cpu=",
            self.llvm_target.len(),
            self.llvm_target,
            self.data_layout.len(),
            self.data_layout,
            self.default_pointer_width_bits,
        )
        .expect("writing to a String cannot fail");
        write_optional_text(output, self.active_cpu());
        output.push_str(";features=");
        write_optional_text(output, self.active_features());
        output.push(')');
    }
}

fn write_optional_text(output: &mut String, value: Option<&str>) {
    match value {
        Some(value) => {
            write!(output, "{}:{value}", value.len()).expect("writing to a String cannot fail");
        }
        None => output.push_str("unavailable"),
    }
}

fn normalize_active_cpu(cpu: &str) -> Result<Option<String>, SemanticLayoutBridgeError> {
    let cpu = cpu.trim();
    if cpu.is_empty()
        || cpu.eq_ignore_ascii_case("default")
        || cpu.eq_ignore_ascii_case("generic")
        || cpu.eq_ignore_ascii_case("native")
        || cpu.eq_ignore_ascii_case("baseline")
    {
        return Ok(None);
    }
    validate_target_text("rustc active target CPU", cpu)?;
    if cpu
        .bytes()
        .any(|byte| byte.is_ascii_whitespace() || byte == b',')
    {
        return Err(SemanticLayoutBridgeError::InvalidTarget {
            field: "rustc active target CPU",
            detail: "must be one unambiguous CPU name".to_owned(),
        });
    }
    Ok(Some(cpu.to_ascii_lowercase()))
}

fn normalize_active_features(
    target_features: &str,
    codegen_features: &str,
) -> Result<String, SemanticLayoutBridgeError> {
    let mut effective = parse_feature_component("rustc target-spec features", target_features)?;
    for (name, enabled) in parse_feature_component(
        "rustc active -Ctarget-feature configuration",
        codegen_features,
    )? {
        effective.insert(name, enabled);
    }
    let mut output = String::new();
    for (index, (name, enabled)) in effective.into_iter().enumerate() {
        if index != 0 {
            output.push(',');
        }
        output.push(if enabled { '+' } else { '-' });
        output.push_str(&name);
    }
    if !output.is_empty() {
        validate_target_text("rustc normalized target features", &output)?;
    }
    Ok(output)
}

fn parse_feature_component(
    field: &'static str,
    features: &str,
) -> Result<BTreeMap<String, bool>, SemanticLayoutBridgeError> {
    let mut parsed = BTreeMap::new();
    if features.trim().is_empty() {
        return Ok(parsed);
    }
    for declaration in features.split(',') {
        let declaration = declaration.trim();
        let (enabled, name) = match declaration.as_bytes().first() {
            Some(b'+') => (true, &declaration[1..]),
            Some(b'-') => (false, &declaration[1..]),
            _ => {
                return Err(SemanticLayoutBridgeError::InvalidTarget {
                    field,
                    detail: "each feature must have an explicit `+` or `-` state".to_owned(),
                });
            }
        };
        if name.is_empty()
            || !name
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-' | b'.'))
        {
            return Err(SemanticLayoutBridgeError::InvalidTarget {
                field,
                detail: "feature names must be nonempty ASCII identifiers".to_owned(),
            });
        }
        let name = name.to_ascii_lowercase();
        if let Some(previous) = parsed.insert(name.clone(), enabled)
            && previous != enabled
        {
            return Err(SemanticLayoutBridgeError::InvalidTarget {
                field,
                detail: format!("feature `{name}` has conflicting states"),
            });
        }
    }
    Ok(parsed)
}

/// Canonical, target-bound semantic type/layout evidence produced by rustc.
///
/// This type is data only. In particular, constructing or receiving it cannot
/// authorize code generation, module loading, or a GPU launch.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SemanticLayoutEvidenceV1 {
    target: SemanticLayoutTargetV1,
    source_type_identity: StableTypeIdentityV1,
    semantic_type: MirSemanticType,
    canonical_bytes: Vec<u8>,
}

impl SemanticLayoutEvidenceV1 {
    /// Constructs validated evidence from an already observed semantic type.
    ///
    /// This constructor is useful at frontend transport boundaries. It does
    /// not attest that an untrusted caller actually ran rustc; consumers must
    /// treat the result as evidence, never as authority.
    pub fn from_semantic_type(
        expected_target: &SemanticLayoutTargetV1,
        observed_target: SemanticLayoutTargetV1,
        source_type_identity: StableTypeIdentityV1,
        semantic_type: MirSemanticType,
    ) -> Result<Self, SemanticLayoutBridgeError> {
        if expected_target != &observed_target {
            return Err(SemanticLayoutBridgeError::TargetMismatch {
                expected: expected_target.clone(),
                observed: observed_target,
            });
        }
        preflight_semantic_type(&semantic_type)?;
        let semantic_text = semantic_type
            .canonical_text()
            .map_err(SemanticLayoutBridgeError::DialectValidation)?;
        let mut canonical = String::from(SEMANTIC_LAYOUT_EVIDENCE_SCHEMA_V1);
        canonical.push('|');
        observed_target.write_canonical(&mut canonical);
        canonical.push_str("|source-type=");
        write_hex(&mut canonical, source_type_identity.as_bytes());
        canonical.push_str("|semantic-type=");
        canonical.push_str(&semantic_text);
        if canonical.len() > MAX_SEMANTIC_LAYOUT_EVIDENCE_BYTES_V1 {
            return Err(SemanticLayoutBridgeError::BoundExceeded {
                field: "canonical semantic layout evidence",
                actual: canonical.len(),
                limit: MAX_SEMANTIC_LAYOUT_EVIDENCE_BYTES_V1,
            });
        }
        Ok(Self {
            target: observed_target,
            source_type_identity,
            semantic_type,
            canonical_bytes: canonical.into_bytes(),
        })
    }

    pub const fn schema(&self) -> &'static str {
        SEMANTIC_LAYOUT_EVIDENCE_SCHEMA_V1
    }

    pub const fn version(&self) -> u16 {
        SEMANTIC_LAYOUT_EVIDENCE_VERSION_V1
    }

    pub const fn target(&self) -> &SemanticLayoutTargetV1 {
        &self.target
    }

    pub const fn source_type_identity(&self) -> StableTypeIdentityV1 {
        self.source_type_identity
    }

    pub const fn semantic_type(&self) -> &MirSemanticType {
        &self.semantic_type
    }

    pub fn canonical_bytes(&self) -> &[u8] {
        &self.canonical_bytes
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SemanticLayoutBridgeError {
    InvalidTarget {
        field: &'static str,
        detail: String,
    },
    TargetMismatch {
        expected: SemanticLayoutTargetV1,
        observed: SemanticLayoutTargetV1,
    },
    Extraction {
        detail: String,
    },
    Unsupported {
        path: String,
        detail: &'static str,
    },
    Inconsistent {
        path: String,
        detail: String,
    },
    BoundExceeded {
        field: &'static str,
        actual: usize,
        limit: usize,
    },
    DialectValidation(MirTypeValidationError),
}

impl fmt::Display for SemanticLayoutBridgeError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidTarget { field, detail } => {
                write!(formatter, "invalid {field}: {detail}")
            }
            Self::TargetMismatch { expected, observed } => write!(
                formatter,
                "rustc layout target mismatch: expected {expected:?}, observed {observed:?}"
            ),
            Self::Extraction { detail } => {
                write!(formatter, "rustc layout extraction failed: {detail}")
            }
            Self::Unsupported { path, detail } => {
                write!(formatter, "layout at {path} is not representable: {detail}")
            }
            Self::Inconsistent { path, detail } => {
                write!(formatter, "inconsistent layout at {path}: {detail}")
            }
            Self::BoundExceeded {
                field,
                actual,
                limit,
            } => write!(formatter, "{field} bound exceeded: {actual} > {limit}"),
            Self::DialectValidation(error) => {
                write!(formatter, "dialect MIR rejected bridged layout: {error}")
            }
        }
    }
}

impl std::error::Error for SemanticLayoutBridgeError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::DialectValidation(error) => Some(error),
            _ => None,
        }
    }
}

/// Captures the exact target context used by rustc's layout engine.
pub fn rustc_semantic_layout_target_v1(
    tcx: TyCtxt<'_>,
) -> Result<SemanticLayoutTargetV1, SemanticLayoutBridgeError> {
    let layout_cx = LayoutCx::new(tcx, TypingEnv::fully_monomorphized());
    let pointer_width =
        u16::try_from(layout_cx.data_layout().pointer_size().bits()).map_err(|_| {
            SemanticLayoutBridgeError::InvalidTarget {
                field: "default pointer width",
                detail: "rustc pointer width does not fit u16".to_owned(),
            }
        })?;
    let active_cpu = tcx
        .sess
        .opts
        .cg
        .target_cpu
        .as_deref()
        .unwrap_or(tcx.sess.target.cpu.as_ref());
    SemanticLayoutTargetV1::new_with_codegen_profile(
        tcx.sess.target.llvm_target.to_string(),
        tcx.sess.target.data_layout.to_string(),
        pointer_width,
        active_cpu,
        tcx.sess.target.features.as_ref(),
        &tcx.sess.opts.cg.target_feature,
    )
}

/// Extracts and bridges one fully monomorphized rustc type.
///
/// `expected_target` must exactly match the active rustc session. No target
/// normalization or compatibility guessing is performed.
pub fn extract_semantic_layout_evidence_v1<'tcx>(
    tcx: TyCtxt<'tcx>,
    ty: Ty<'tcx>,
    source_type_identity: StableTypeIdentityV1,
    expected_target: &SemanticLayoutTargetV1,
) -> Result<SemanticLayoutEvidenceV1, SemanticLayoutBridgeError> {
    let observed_target = rustc_semantic_layout_target_v1(tcx)?;
    if expected_target != &observed_target {
        return Err(SemanticLayoutBridgeError::TargetMismatch {
            expected: expected_target.clone(),
            observed: observed_target,
        });
    }
    let facts = extract_general_layout(tcx, ty).map_err(extraction_error)?;
    let semantic_type = bridge_type(&facts, expected_target, "root")?;
    SemanticLayoutEvidenceV1::from_semantic_type(
        expected_target,
        observed_target,
        source_type_identity,
        semantic_type,
    )
}

fn extraction_error(error: GeneralLayoutExtractError) -> SemanticLayoutBridgeError {
    SemanticLayoutBridgeError::Extraction {
        detail: error.to_string(),
    }
}

fn bridge_type(
    facts: &TypeLayoutFacts,
    target: &SemanticLayoutTargetV1,
    path: &str,
) -> Result<MirSemanticType, SemanticLayoutBridgeError> {
    if facts.uninhabited {
        return Err(SemanticLayoutBridgeError::Unsupported {
            path: path.to_owned(),
            detail: "the dialect schema cannot preserve uninhabited validity",
        });
    }
    let layout = MirLayout::sized(facts.size_bytes, facts.abi_alignment_bytes);
    let kind = match &facts.kind {
        TypeLayoutKind::Scalar(source) => {
            verify_source_scalar(facts, *source, path)?;
            MirTypeKind::Scalar(source_scalar(*source, path)?)
        }
        TypeLayoutKind::Pointer(pointer) => {
            verify_pointer(facts, pointer.address_space, target, path)?;
            let pointee = Box::new(bridge_type(
                &pointer.pointee,
                target,
                &format!("{path}.pointee"),
            )?);
            let address_space = MirAddressSpace(pointer.address_space);
            match pointer.kind {
                PointerKind::SharedReference => MirTypeKind::Reference {
                    referent: pointee,
                    mutability: MirMutability::Immutable,
                    address_space,
                },
                PointerKind::MutableReference => MirTypeKind::Reference {
                    referent: pointee,
                    mutability: MirMutability::Mutable,
                    address_space,
                },
                PointerKind::ConstRaw => MirTypeKind::RawPointer {
                    pointee,
                    mutability: MirMutability::Immutable,
                    address_space,
                },
                PointerKind::MutRaw => MirTypeKind::RawPointer {
                    pointee,
                    mutability: MirMutability::Mutable,
                    address_space,
                },
            }
        }
        TypeLayoutKind::Array(array) => {
            let element = bridge_type(&array.element, target, &format!("{path}.element"))?;
            let element_size = element
                .layout
                .size
                .expect("the rustc extractor rejects DSTs");
            if array.stride_bytes != element_size {
                return Err(SemanticLayoutBridgeError::Unsupported {
                    path: path.to_owned(),
                    detail: "dialect MIR V1 cannot preserve an array stride different from element size",
                });
            }
            let expected_size = array
                .stride_bytes
                .checked_mul(array.length)
                .ok_or_else(|| inconsistent(path, "array extent overflows u64"))?;
            if expected_size != facts.size_bytes {
                return Err(inconsistent(
                    path,
                    format!(
                        "array stride {} and length {} imply size {expected_size}, found {}",
                        array.stride_bytes, array.length, facts.size_bytes
                    ),
                ));
            }
            MirTypeKind::Array {
                element: Box::new(element),
                length: array.length,
            }
        }
        TypeLayoutKind::Tuple(fields) if fields.is_empty() && facts.rust_type == "()" => {
            MirTypeKind::Unit
        }
        TypeLayoutKind::Tuple(fields) => MirTypeKind::Tuple(bridge_aggregate(
            fields,
            facts.size_bytes,
            target,
            path,
            FieldNames::Unnamed,
            &[],
            false,
        )?),
        TypeLayoutKind::Adt(adt) => bridge_adt(facts, adt, target, path)?,
    };
    let semantic = MirSemanticType { layout, kind };
    semantic
        .validate()
        .map_err(SemanticLayoutBridgeError::DialectValidation)?;
    Ok(semantic)
}

fn bridge_adt(
    facts: &TypeLayoutFacts,
    adt: &AdtLayoutFacts,
    target: &SemanticLayoutTargetV1,
    path: &str,
) -> Result<MirTypeKind, SemanticLayoutBridgeError> {
    if adt.definition.is_empty() {
        return Err(inconsistent(path, "rustc ADT definition identity is empty"));
    }
    match adt.kind {
        AdtKind::Union => Err(SemanticLayoutBridgeError::Unsupported {
            path: path.to_owned(),
            detail: "dialect MIR V1 cannot preserve overlapping union fields",
        }),
        AdtKind::Struct => {
            if adt.tag.is_some() || adt.variants.len() != 1 || adt.variants[0].source_index != 0 {
                return Err(inconsistent(
                    path,
                    "a struct must contain exactly source variant zero and no enum tag",
                ));
            }
            if adt.variants[0].uninhabited {
                return Err(SemanticLayoutBridgeError::Unsupported {
                    path: path.to_owned(),
                    detail: "the dialect schema cannot preserve an uninhabited struct",
                });
            }
            Ok(MirTypeKind::Struct(MirStructType {
                identity: facts.rust_type.clone(),
                aggregate: bridge_aggregate(
                    &adt.variants[0].fields,
                    facts.size_bytes,
                    target,
                    path,
                    FieldNames::Named,
                    &[],
                    false,
                )?,
            }))
        }
        AdtKind::Enum => bridge_enum(facts, adt, target, path),
    }
}

fn bridge_enum(
    facts: &TypeLayoutFacts,
    adt: &AdtLayoutFacts,
    target: &SemanticLayoutTargetV1,
    path: &str,
) -> Result<MirTypeKind, SemanticLayoutBridgeError> {
    if adt.variants.is_empty() {
        return Err(SemanticLayoutBridgeError::Unsupported {
            path: path.to_owned(),
            detail: "rustc provided no logical discriminant representation for an empty enum",
        });
    }
    let discriminant = enum_discriminant(&adt.variants, path)?;
    let (encoding, reserved, reserved_may_overlap) = match adt.tag {
        None if adt.variants.len() == 1 => (MirEnumEncoding::Single { variant: 0 }, None, false),
        None => {
            return Err(inconsistent(
                path,
                "a multi-variant enum has no physical tag encoding",
            ));
        }
        Some(tag) => {
            let tag_bits = scalar_width_bits(tag.scalar, &format!("{path}.tag"))?;
            let tag_size = u64::from(tag_bits / 8);
            let end = tag
                .offset_bytes
                .checked_add(tag_size)
                .ok_or_else(|| inconsistent(path, "enum tag extent overflows u64"))?;
            if end > facts.size_bytes {
                return Err(inconsistent(path, "enum tag extends beyond its layout"));
            }
            match tag.encoding {
                EnumTagEncodingFacts::Direct => {
                    let tag_scalar = backend_integer_scalar(tag.scalar, &format!("{path}.tag"))?;
                    (
                        MirEnumEncoding::Direct {
                            tag_offset: tag.offset_bytes,
                            tag: tag_scalar,
                        },
                        Some((tag.offset_bytes, end)),
                        false,
                    )
                }
                EnumTagEncodingFacts::Niche {
                    untagged_variant,
                    niche_variants_start,
                    niche_variants_end,
                    niche_start,
                } => {
                    match tag.scalar.primitive {
                        ScalarPrimitiveFacts::Integer { bits, .. }
                            if bits == u64::from(tag_bits) => {}
                        ScalarPrimitiveFacts::Pointer { address_space } => {
                            let Some(variant) = adt.variants.get(untagged_variant as usize) else {
                                return Err(inconsistent(
                                    path,
                                    "niche untagged variant is outside the variant set",
                                ));
                            };
                            if !fields_preserve_pointer_niche(
                                &variant.fields,
                                tag.offset_bytes,
                                tag_size,
                                address_space,
                                path,
                            )? {
                                return Err(SemanticLayoutBridgeError::Unsupported {
                                    path: path.to_owned(),
                                    detail: "pointer niche lacks a matching pointer field carrying its address-space provenance",
                                });
                            }
                        }
                        ScalarPrimitiveFacts::Integer { .. } => {
                            return Err(inconsistent(
                                path,
                                "integer niche primitive width disagrees with its storage",
                            ));
                        }
                        ScalarPrimitiveFacts::Float { .. } => {
                            return Err(SemanticLayoutBridgeError::Unsupported {
                                path: path.to_owned(),
                                detail: "floating-point niche tags are not represented by dialect MIR V1",
                            });
                        }
                    }
                    (
                        MirEnumEncoding::Niche {
                            niche_offset: tag.offset_bytes,
                            niche_bits: tag_bits,
                            untagged_variant,
                            niche_variants_start,
                            niche_variants_end,
                            niche_start,
                        },
                        Some((tag.offset_bytes, end)),
                        true,
                    )
                }
            }
        }
    };

    let reserved = reserved.into_iter().collect::<Vec<_>>();
    let mut variants = Vec::with_capacity(adt.variants.len());
    for (expected_index, variant) in adt.variants.iter().enumerate() {
        let expected_index = u32::try_from(expected_index)
            .map_err(|_| inconsistent(path, "variant index does not fit u32"))?;
        if variant.source_index != expected_index {
            return Err(inconsistent(
                path,
                format!(
                    "variant indices must be contiguous: expected {expected_index}, found {}",
                    variant.source_index
                ),
            ));
        }
        if variant.uninhabited {
            return Err(SemanticLayoutBridgeError::Unsupported {
                path: format!("{path}.variant[{expected_index}]"),
                detail: "the dialect schema cannot preserve an uninhabited enum variant",
            });
        }
        let discriminant_bits = variant.discriminant_bits.ok_or_else(|| {
            inconsistent(
                &format!("{path}.variant[{expected_index}]"),
                "rustc omitted the logical discriminant value",
            )
        })?;
        variants.push(MirVariant {
            index: expected_index,
            name: variant.name.clone(),
            discriminant: discriminant_bits,
            aggregate: bridge_aggregate(
                &variant.fields,
                facts.size_bytes,
                target,
                &format!("{path}.variant[{expected_index}]"),
                FieldNames::Named,
                &reserved,
                reserved_may_overlap,
            )?,
        });
    }

    Ok(MirTypeKind::Enum(MirEnumType {
        identity: facts.rust_type.clone(),
        discriminant,
        encoding,
        variants,
    }))
}

fn enum_discriminant(
    variants: &[VariantLayoutFacts],
    path: &str,
) -> Result<MirScalarType, SemanticLayoutBridgeError> {
    let first = variants[0].discriminant_scalar.ok_or_else(|| {
        inconsistent(
            &format!("{path}.variant[0]"),
            "rustc logical discriminant is not a supported scalar",
        )
    })?;
    let scalar = match first {
        SourceScalarKind::PointerSizedSignedInteger { bits } => MirScalarType::Int {
            signed: true,
            bits: bounded_scalar_bits(bits, &format!("{path}.discriminant"))?,
        },
        SourceScalarKind::PointerSizedUnsignedInteger { bits } => MirScalarType::Int {
            signed: false,
            bits: bounded_scalar_bits(bits, &format!("{path}.discriminant"))?,
        },
        _ => source_scalar(first, &format!("{path}.discriminant"))?,
    };
    if !matches!(scalar, MirScalarType::Int { .. }) {
        return Err(inconsistent(path, "enum discriminant is not an integer"));
    }
    for (index, variant) in variants.iter().enumerate() {
        if variant.discriminant_type.is_none() || variant.discriminant_scalar != Some(first) {
            return Err(inconsistent(
                &format!("{path}.variant[{index}]"),
                "enum variants disagree on logical discriminant type",
            ));
        }
    }
    Ok(scalar)
}

#[derive(Clone, Copy)]
enum FieldNames {
    Named,
    Unnamed,
}

#[allow(clippy::too_many_arguments)]
fn bridge_aggregate(
    fields: &[FieldLayoutFacts],
    container_size: u64,
    target: &SemanticLayoutTargetV1,
    path: &str,
    names: FieldNames,
    reserved: &[(u64, u64)],
    reserved_may_overlap: bool,
) -> Result<MirAggregateLayout, SemanticLayoutBridgeError> {
    validate_field_order(fields, path)?;
    let mut bridged = Vec::with_capacity(fields.len());
    let mut occupied = Vec::new();
    for (index, field) in fields.iter().enumerate() {
        let field_path = format!("{path}.field[{index}]");
        let ty = bridge_type(&field.layout, target, &field_path)?;
        let field_size = ty.layout.size.expect("the rustc extractor rejects DSTs");
        let end = field
            .offset_bytes
            .checked_add(field_size)
            .ok_or_else(|| inconsistent(&field_path, "field extent overflows u64"))?;
        if end > container_size {
            return Err(inconsistent(
                &field_path,
                format!("field extent ends at {end}, past container size {container_size}"),
            ));
        }
        if field_size != 0 {
            occupied.push((field.offset_bytes, end));
        }
        let name = match names {
            FieldNames::Named => Some(
                field
                    .name
                    .clone()
                    .ok_or_else(|| inconsistent(&field_path, "named field has no source name"))?,
            ),
            FieldNames::Unnamed if field.name.is_none() => None,
            FieldNames::Unnamed => {
                return Err(inconsistent(
                    &field_path,
                    "tuple field unexpectedly has a name",
                ));
            }
        };
        bridged.push(MirField {
            name,
            offset: field.offset_bytes,
            ty,
        });
    }
    occupied.sort_unstable();
    reject_overlaps(&occupied, path, "non-zero-sized fields overlap")?;
    if !reserved_may_overlap {
        for field in &occupied {
            if reserved.iter().any(|range| ranges_overlap(*field, *range)) {
                return Err(inconsistent(
                    path,
                    "a field overlaps reserved enum tag storage",
                ));
            }
        }
    }
    occupied.extend_from_slice(reserved);
    occupied.sort_unstable();
    let occupied = merge_ranges(occupied);
    let padding = exact_padding(container_size, &occupied, path)?;
    Ok(MirAggregateLayout {
        fields: bridged,
        padding,
    })
}

fn validate_field_order(
    fields: &[FieldLayoutFacts],
    path: &str,
) -> Result<(), SemanticLayoutBridgeError> {
    let mut memory_seen = vec![false; fields.len()];
    let mut memory_offsets = vec![0_u64; fields.len()];
    for (source_index, field) in fields.iter().enumerate() {
        if field.source_index != source_index {
            return Err(inconsistent(
                path,
                format!(
                    "fields are not in source order: expected {source_index}, found {}",
                    field.source_index
                ),
            ));
        }
        let Some(seen) = memory_seen.get_mut(field.memory_index) else {
            return Err(inconsistent(
                path,
                "field memory index is outside the field set",
            ));
        };
        if std::mem::replace(seen, true) {
            return Err(inconsistent(path, "field memory indices are not unique"));
        }
        memory_offsets[field.memory_index] = field.offset_bytes;
    }
    if memory_offsets.windows(2).any(|pair| pair[0] > pair[1]) {
        return Err(inconsistent(
            path,
            "field memory order disagrees with increasing physical offsets",
        ));
    }
    Ok(())
}

fn fields_preserve_pointer_niche(
    fields: &[FieldLayoutFacts],
    niche_offset: u64,
    niche_size: u64,
    address_space: u32,
    path: &str,
) -> Result<bool, SemanticLayoutBridgeError> {
    for field in fields {
        if layout_preserves_pointer_niche(
            &field.layout,
            field.offset_bytes,
            niche_offset,
            niche_size,
            address_space,
            path,
        )? {
            return Ok(true);
        }
    }
    Ok(false)
}

fn layout_preserves_pointer_niche(
    facts: &TypeLayoutFacts,
    base_offset: u64,
    niche_offset: u64,
    niche_size: u64,
    address_space: u32,
    path: &str,
) -> Result<bool, SemanticLayoutBridgeError> {
    match &facts.kind {
        TypeLayoutKind::Pointer(pointer) => Ok(base_offset == niche_offset
            && facts.size_bytes == niche_size
            && pointer.address_space == address_space),
        TypeLayoutKind::Tuple(fields) => fields.iter().try_fold(false, |found, field| {
            let offset = base_offset
                .checked_add(field.offset_bytes)
                .ok_or_else(|| inconsistent(path, "nested niche field offset overflows u64"))?;
            Ok(found
                || layout_preserves_pointer_niche(
                    &field.layout,
                    offset,
                    niche_offset,
                    niche_size,
                    address_space,
                    path,
                )?)
        }),
        TypeLayoutKind::Adt(adt) if adt.kind == AdtKind::Struct && adt.variants.len() == 1 => adt
            .variants[0]
            .fields
            .iter()
            .try_fold(false, |found, field| {
                let offset = base_offset
                    .checked_add(field.offset_bytes)
                    .ok_or_else(|| inconsistent(path, "nested niche field offset overflows u64"))?;
                Ok(found
                    || layout_preserves_pointer_niche(
                        &field.layout,
                        offset,
                        niche_offset,
                        niche_size,
                        address_space,
                        path,
                    )?)
            }),
        TypeLayoutKind::Array(array) if array.length != 0 && array.stride_bytes != 0 => {
            if niche_offset < base_offset {
                return Ok(false);
            }
            let relative = niche_offset - base_offset;
            let index = relative / array.stride_bytes;
            if index >= array.length {
                return Ok(false);
            }
            let offset = base_offset
                .checked_add(
                    index
                        .checked_mul(array.stride_bytes)
                        .ok_or_else(|| inconsistent(path, "niche array offset overflows u64"))?,
                )
                .ok_or_else(|| inconsistent(path, "niche array offset overflows u64"))?;
            layout_preserves_pointer_niche(
                &array.element,
                offset,
                niche_offset,
                niche_size,
                address_space,
                path,
            )
        }
        _ => Ok(false),
    }
}

fn verify_source_scalar(
    facts: &TypeLayoutFacts,
    source: SourceScalarKind,
    path: &str,
) -> Result<(), SemanticLayoutBridgeError> {
    let BackendRepresentationFacts::Scalar(scalar) = facts.backend_representation else {
        return Err(inconsistent(
            path,
            "source scalar lacks a scalar backend representation",
        ));
    };
    if scalar.size_bytes != facts.size_bytes
        || scalar.abi_alignment_bytes != facts.abi_alignment_bytes
    {
        return Err(inconsistent(
            path,
            "source and backend scalar layouts disagree",
        ));
    }
    let expected = match source {
        SourceScalarKind::Bool => ScalarPrimitiveFacts::Integer {
            bits: 8,
            signed: false,
        },
        SourceScalarKind::Char => ScalarPrimitiveFacts::Integer {
            bits: 32,
            signed: false,
        },
        SourceScalarKind::SignedInteger { bits } => {
            ScalarPrimitiveFacts::Integer { bits, signed: true }
        }
        SourceScalarKind::UnsignedInteger { bits } => ScalarPrimitiveFacts::Integer {
            bits,
            signed: false,
        },
        SourceScalarKind::PointerSizedSignedInteger { bits } => {
            ScalarPrimitiveFacts::Integer { bits, signed: true }
        }
        SourceScalarKind::PointerSizedUnsignedInteger { bits } => ScalarPrimitiveFacts::Integer {
            bits,
            signed: false,
        },
        SourceScalarKind::Float { bits } => ScalarPrimitiveFacts::Float { bits },
    };
    if scalar.primitive != expected {
        return Err(inconsistent(
            path,
            "source and backend scalar primitives disagree",
        ));
    }
    Ok(())
}

fn verify_pointer(
    facts: &TypeLayoutFacts,
    address_space: u32,
    target: &SemanticLayoutTargetV1,
    path: &str,
) -> Result<(), SemanticLayoutBridgeError> {
    let BackendRepresentationFacts::Scalar(scalar) = facts.backend_representation else {
        return Err(inconsistent(
            path,
            "thin pointer lacks a scalar backend representation",
        ));
    };
    if scalar.primitive != (ScalarPrimitiveFacts::Pointer { address_space })
        || scalar.size_bytes != facts.size_bytes
        || scalar.abi_alignment_bytes != facts.abi_alignment_bytes
    {
        return Err(inconsistent(
            path,
            "pointer provenance and backend layout disagree",
        ));
    }
    let bits = scalar_width_bits(scalar, path)?;
    if address_space == MirAddressSpace::DEFAULT.0 && bits != target.default_pointer_width_bits {
        return Err(inconsistent(
            path,
            format!(
                "default-address-space pointer is {bits} bits, target declares {} bits",
                target.default_pointer_width_bits
            ),
        ));
    }
    Ok(())
}

fn source_scalar(
    source: SourceScalarKind,
    path: &str,
) -> Result<MirScalarType, SemanticLayoutBridgeError> {
    match source {
        SourceScalarKind::Bool => Ok(MirScalarType::Bool),
        SourceScalarKind::Char => Ok(MirScalarType::Char),
        SourceScalarKind::SignedInteger { bits } => Ok(MirScalarType::Int {
            signed: true,
            bits: bounded_scalar_bits(bits, path)?,
        }),
        SourceScalarKind::UnsignedInteger { bits } => Ok(MirScalarType::Int {
            signed: false,
            bits: bounded_scalar_bits(bits, path)?,
        }),
        SourceScalarKind::PointerSizedSignedInteger { .. }
        | SourceScalarKind::PointerSizedUnsignedInteger { .. } => {
            Err(SemanticLayoutBridgeError::Unsupported {
                path: path.to_owned(),
                detail: "pointer-sized integers require source-kind-preserving schema support",
            })
        }
        SourceScalarKind::Float { bits } => Ok(MirScalarType::Float {
            bits: bounded_scalar_bits(bits, path)?,
        }),
    }
}

fn backend_integer_scalar(
    scalar: ScalarLayoutFacts,
    path: &str,
) -> Result<MirScalarType, SemanticLayoutBridgeError> {
    let ScalarPrimitiveFacts::Integer { bits, signed } = scalar.primitive else {
        return Err(inconsistent(
            path,
            "direct enum tag is not an integer scalar",
        ));
    };
    if u64::from(scalar_width_bits(scalar, path)?) != bits {
        return Err(inconsistent(
            path,
            "enum tag primitive width disagrees with its size",
        ));
    }
    Ok(MirScalarType::Int {
        signed,
        bits: bounded_scalar_bits(bits, path)?,
    })
}

fn scalar_width_bits(
    scalar: ScalarLayoutFacts,
    path: &str,
) -> Result<u16, SemanticLayoutBridgeError> {
    let bits = scalar
        .size_bytes
        .checked_mul(8)
        .ok_or_else(|| inconsistent(path, "scalar bit width overflows u64"))?;
    bounded_scalar_bits(bits, path)
}

fn bounded_scalar_bits(bits: u64, path: &str) -> Result<u16, SemanticLayoutBridgeError> {
    let bits =
        u16::try_from(bits).map_err(|_| inconsistent(path, "scalar bit width does not fit u16"))?;
    if !matches!(bits, 8 | 16 | 32 | 64 | 128) {
        return Err(SemanticLayoutBridgeError::Unsupported {
            path: path.to_owned(),
            detail: "dialect MIR V1 supports only 8/16/32/64/128-bit scalars",
        });
    }
    Ok(bits)
}

fn exact_padding(
    container_size: u64,
    occupied: &[(u64, u64)],
    path: &str,
) -> Result<Vec<MirPadding>, SemanticLayoutBridgeError> {
    let mut padding = Vec::new();
    let mut cursor = 0_u64;
    for &(start, end) in occupied {
        if start > container_size || end > container_size || start > end {
            return Err(inconsistent(
                path,
                "occupied byte range is outside its container",
            ));
        }
        if cursor < start {
            padding.push(MirPadding {
                offset: cursor,
                size: start - cursor,
            });
        }
        cursor = cursor.max(end);
    }
    if cursor < container_size {
        padding.push(MirPadding {
            offset: cursor,
            size: container_size - cursor,
        });
    }
    Ok(padding)
}

fn reject_overlaps(
    ranges: &[(u64, u64)],
    path: &str,
    detail: &str,
) -> Result<(), SemanticLayoutBridgeError> {
    if ranges.windows(2).any(|pair| pair[0].1 > pair[1].0) {
        Err(inconsistent(path, detail))
    } else {
        Ok(())
    }
}

fn merge_ranges(ranges: Vec<(u64, u64)>) -> Vec<(u64, u64)> {
    let mut merged: Vec<(u64, u64)> = Vec::with_capacity(ranges.len());
    for range in ranges {
        if let Some(last) = merged.last_mut()
            && last.1 >= range.0
        {
            last.1 = last.1.max(range.1);
        } else {
            merged.push(range);
        }
    }
    merged
}

fn ranges_overlap(left: (u64, u64), right: (u64, u64)) -> bool {
    left.0 < right.1 && right.0 < left.1
}

fn validate_target_text(field: &'static str, text: &str) -> Result<(), SemanticLayoutBridgeError> {
    if text.is_empty() {
        return Err(SemanticLayoutBridgeError::InvalidTarget {
            field,
            detail: "must not be empty".to_owned(),
        });
    }
    if text.len() > MAX_SEMANTIC_LAYOUT_TARGET_TEXT_BYTES_V1 {
        return Err(SemanticLayoutBridgeError::BoundExceeded {
            field,
            actual: text.len(),
            limit: MAX_SEMANTIC_LAYOUT_TARGET_TEXT_BYTES_V1,
        });
    }
    if text.chars().any(char::is_control) {
        return Err(SemanticLayoutBridgeError::InvalidTarget {
            field,
            detail: "must not contain control characters".to_owned(),
        });
    }
    Ok(())
}

fn preflight_semantic_type(root: &MirSemanticType) -> Result<(), SemanticLayoutBridgeError> {
    let mut stack = vec![(root, 0_usize)];
    let mut nodes = 0_usize;
    let mut text_bytes = 0_usize;
    while let Some((ty, depth)) = stack.pop() {
        if depth > MAX_SEMANTIC_LAYOUT_DEPTH_V1 {
            return Err(SemanticLayoutBridgeError::BoundExceeded {
                field: "semantic layout depth",
                actual: depth,
                limit: MAX_SEMANTIC_LAYOUT_DEPTH_V1,
            });
        }
        nodes = nodes
            .checked_add(1)
            .ok_or(SemanticLayoutBridgeError::BoundExceeded {
                field: "semantic layout type nodes",
                actual: usize::MAX,
                limit: MAX_SEMANTIC_LAYOUT_TYPE_NODES_V1,
            })?;
        check_semantic_bound(
            "semantic layout type nodes",
            nodes,
            MAX_SEMANTIC_LAYOUT_TYPE_NODES_V1,
        )?;
        let child_depth = depth
            .checked_add(1)
            .ok_or(SemanticLayoutBridgeError::BoundExceeded {
                field: "semantic layout depth",
                actual: usize::MAX,
                limit: MAX_SEMANTIC_LAYOUT_DEPTH_V1,
            })?;
        match &ty.kind {
            MirTypeKind::Unit | MirTypeKind::Scalar(_) => {}
            MirTypeKind::RawPointer { pointee, .. } => stack.push((pointee, child_depth)),
            MirTypeKind::Reference { referent, .. } => stack.push((referent, child_depth)),
            MirTypeKind::Slice { element } | MirTypeKind::Array { element, .. } => {
                stack.push((element, child_depth));
            }
            MirTypeKind::Tuple(aggregate) => {
                preflight_aggregate(aggregate, child_depth, &mut stack, &mut text_bytes)?
            }
            MirTypeKind::Struct(struct_ty) => {
                add_semantic_text(&mut text_bytes, struct_ty.identity.len())?;
                preflight_aggregate(
                    &struct_ty.aggregate,
                    child_depth,
                    &mut stack,
                    &mut text_bytes,
                )?;
            }
            MirTypeKind::Enum(enum_ty) => {
                add_semantic_text(&mut text_bytes, enum_ty.identity.len())?;
                check_semantic_bound(
                    "semantic layout enum variants",
                    enum_ty.variants.len(),
                    MAX_SEMANTIC_LAYOUT_VARIANTS_V1,
                )?;
                for variant in &enum_ty.variants {
                    add_semantic_text(&mut text_bytes, variant.name.len())?;
                    preflight_aggregate(
                        &variant.aggregate,
                        child_depth,
                        &mut stack,
                        &mut text_bytes,
                    )?;
                }
            }
        }
    }
    Ok(())
}

fn preflight_aggregate<'a>(
    aggregate: &'a MirAggregateLayout,
    child_depth: usize,
    stack: &mut Vec<(&'a MirSemanticType, usize)>,
    text_bytes: &mut usize,
) -> Result<(), SemanticLayoutBridgeError> {
    check_semantic_bound(
        "semantic layout aggregate fields",
        aggregate.fields.len(),
        MAX_SEMANTIC_LAYOUT_FIELDS_V1,
    )?;
    for field in &aggregate.fields {
        if let Some(name) = &field.name {
            add_semantic_text(text_bytes, name.len())?;
        }
        stack.push((&field.ty, child_depth));
    }
    Ok(())
}

fn add_semantic_text(
    total: &mut usize,
    additional: usize,
) -> Result<(), SemanticLayoutBridgeError> {
    *total = total
        .checked_add(additional)
        .ok_or(SemanticLayoutBridgeError::BoundExceeded {
            field: "semantic layout identity text",
            actual: usize::MAX,
            limit: MAX_SEMANTIC_LAYOUT_TYPE_TEXT_BYTES_V1,
        })?;
    check_semantic_bound(
        "semantic layout identity text",
        *total,
        MAX_SEMANTIC_LAYOUT_TYPE_TEXT_BYTES_V1,
    )
}

fn check_semantic_bound(
    field: &'static str,
    actual: usize,
    limit: usize,
) -> Result<(), SemanticLayoutBridgeError> {
    if actual > limit {
        Err(SemanticLayoutBridgeError::BoundExceeded {
            field,
            actual,
            limit,
        })
    } else {
        Ok(())
    }
}

fn write_hex(output: &mut String, bytes: &[u8]) {
    for byte in bytes {
        write!(output, "{byte:02x}").expect("writing to a String cannot fail");
    }
}

fn inconsistent(path: &str, detail: impl Into<String>) -> SemanticLayoutBridgeError {
    SemanticLayoutBridgeError::Inconsistent {
        path: path.to_owned(),
        detail: detail.into(),
    }
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::PathBuf;
    use std::process::Command;

    use rustc_driver::{Callbacks, Compilation};
    use rustc_hir::def::DefKind;
    use rustc_interface::interface::Compiler;

    use super::*;
    use crate::rust_type_layout_general::{
        AdtRepresentationFacts, ArrayLayoutFacts, EnumTagLayoutFacts, PointerLayoutFacts,
    };

    fn target() -> SemanticLayoutTargetV1 {
        SemanticLayoutTargetV1::new("test-target", "e-p:64:64", 64).unwrap()
    }

    #[test]
    fn active_codegen_profile_is_normalized_and_identity_bound() {
        let first = SemanticLayoutTargetV1::new_with_codegen_profile(
            "amdgcn-amd-amdhsa",
            "e-p:64:64",
            64,
            "GFX942",
            "+wavefrontsize64,-wavefrontsize32",
            "-xnack,+wavefrontsize64",
        )
        .unwrap();
        let reordered = SemanticLayoutTargetV1::new_with_codegen_profile(
            "amdgcn-amd-amdhsa",
            "e-p:64:64",
            64,
            "gfx942",
            "-wavefrontsize32,+wavefrontsize64,+wavefrontsize64",
            "-xnack",
        )
        .unwrap();
        assert_eq!(first, reordered);
        assert_eq!(first.active_cpu(), Some("gfx942"));
        assert_eq!(
            first.active_features(),
            Some("-wavefrontsize32,+wavefrontsize64,-xnack")
        );

        let unavailable = SemanticLayoutTargetV1::new_with_codegen_profile(
            "amdgcn-amd-amdhsa",
            "e-p:64:64",
            64,
            "native",
            "",
            "-xnack",
        )
        .unwrap();
        assert_eq!(unavailable.active_cpu(), None);
        assert_ne!(first, unavailable);
        assert!(
            SemanticLayoutTargetV1::new_with_codegen_profile(
                "amdgcn-amd-amdhsa",
                "e-p:64:64",
                64,
                "gfx942",
                "+xnack,-xnack",
                "",
            )
            .is_err()
        );
        assert!(
            SemanticLayoutTargetV1::new_with_codegen_profile(
                "amdgcn-amd-amdhsa",
                "e-p:64:64",
                64,
                "gfx942",
                "xnack",
                "",
            )
            .is_err()
        );
    }

    fn scalar_facts(source: SourceScalarKind) -> TypeLayoutFacts {
        let (rust_type, primitive, size, align) = match source {
            SourceScalarKind::Bool => (
                "bool",
                ScalarPrimitiveFacts::Integer {
                    bits: 8,
                    signed: false,
                },
                1,
                1,
            ),
            SourceScalarKind::Char => (
                "char",
                ScalarPrimitiveFacts::Integer {
                    bits: 32,
                    signed: false,
                },
                4,
                4,
            ),
            SourceScalarKind::SignedInteger { bits } => (
                "signed",
                ScalarPrimitiveFacts::Integer { bits, signed: true },
                bits / 8,
                (bits / 8).min(8),
            ),
            SourceScalarKind::UnsignedInteger { bits } => (
                "unsigned",
                ScalarPrimitiveFacts::Integer {
                    bits,
                    signed: false,
                },
                bits / 8,
                (bits / 8).min(8),
            ),
            SourceScalarKind::PointerSizedSignedInteger { bits } => (
                "isize",
                ScalarPrimitiveFacts::Integer { bits, signed: true },
                bits / 8,
                (bits / 8).min(8),
            ),
            SourceScalarKind::PointerSizedUnsignedInteger { bits } => (
                "usize",
                ScalarPrimitiveFacts::Integer {
                    bits,
                    signed: false,
                },
                bits / 8,
                (bits / 8).min(8),
            ),
            SourceScalarKind::Float { bits } => (
                "float",
                ScalarPrimitiveFacts::Float { bits },
                bits / 8,
                (bits / 8).min(8),
            ),
        };
        TypeLayoutFacts {
            rust_type: rust_type.to_owned(),
            size_bytes: size,
            abi_alignment_bytes: align,
            unadjusted_abi_alignment_bytes: align,
            maximum_requested_alignment_bytes: None,
            uninhabited: false,
            backend_representation: BackendRepresentationFacts::Scalar(ScalarLayoutFacts {
                primitive,
                size_bytes: size,
                abi_alignment_bytes: align,
                initialized: true,
                valid_range_start: 0,
                valid_range_end: u128::MAX,
            }),
            largest_niche: None,
            kind: TypeLayoutKind::Scalar(source),
        }
    }

    fn u8_facts() -> TypeLayoutFacts {
        scalar_facts(SourceScalarKind::UnsignedInteger { bits: 8 })
    }

    fn u16_facts() -> TypeLayoutFacts {
        scalar_facts(SourceScalarKind::UnsignedInteger { bits: 16 })
    }

    fn u32_facts() -> TypeLayoutFacts {
        scalar_facts(SourceScalarKind::UnsignedInteger { bits: 32 })
    }

    fn pointer_facts(
        address_space: u32,
        size: u64,
        align: u64,
        pointee: TypeLayoutFacts,
    ) -> TypeLayoutFacts {
        TypeLayoutFacts {
            rust_type: "*mut u16".to_owned(),
            size_bytes: size,
            abi_alignment_bytes: align,
            unadjusted_abi_alignment_bytes: align,
            maximum_requested_alignment_bytes: None,
            uninhabited: false,
            backend_representation: BackendRepresentationFacts::Scalar(ScalarLayoutFacts {
                primitive: ScalarPrimitiveFacts::Pointer { address_space },
                size_bytes: size,
                abi_alignment_bytes: align,
                initialized: false,
                valid_range_start: 0,
                valid_range_end: u128::MAX,
            }),
            largest_niche: None,
            kind: TypeLayoutKind::Pointer(PointerLayoutFacts {
                kind: PointerKind::MutRaw,
                address_space,
                pointee: Box::new(pointee),
            }),
        }
    }

    fn field(
        source_index: usize,
        memory_index: usize,
        name: Option<&str>,
        offset_bytes: u64,
        layout: TypeLayoutFacts,
    ) -> FieldLayoutFacts {
        FieldLayoutFacts {
            source_index,
            memory_index,
            name: name.map(str::to_owned),
            offset_bytes,
            layout,
        }
    }

    fn memory_layout(
        rust_type: &str,
        size: u64,
        align: u64,
        kind: TypeLayoutKind,
    ) -> TypeLayoutFacts {
        TypeLayoutFacts {
            rust_type: rust_type.to_owned(),
            size_bytes: size,
            abi_alignment_bytes: align,
            unadjusted_abi_alignment_bytes: align,
            maximum_requested_alignment_bytes: None,
            uninhabited: false,
            backend_representation: BackendRepresentationFacts::Memory,
            largest_niche: None,
            kind,
        }
    }

    fn variant(
        index: u32,
        name: &str,
        discriminant: u128,
        fields: Vec<FieldLayoutFacts>,
    ) -> VariantLayoutFacts {
        VariantLayoutFacts {
            source_index: index,
            name: name.to_owned(),
            discriminant_bits: Some(discriminant),
            discriminant_type: Some("u8".to_owned()),
            discriminant_scalar: Some(SourceScalarKind::UnsignedInteger { bits: 8 }),
            uninhabited: false,
            fields,
        }
    }

    #[test]
    fn nested_layout_preserves_offsets_padding_and_address_space_pointer_width() {
        let array = memory_layout(
            "[u16; 3]",
            6,
            2,
            TypeLayoutKind::Array(ArrayLayoutFacts {
                length: 3,
                stride_bytes: 2,
                element: Box::new(u16_facts()),
            }),
        );
        let tuple = memory_layout(
            "(u8, u32)",
            8,
            4,
            TypeLayoutKind::Tuple(vec![
                field(0, 0, None, 0, u8_facts()),
                field(1, 1, None, 4, u32_facts()),
            ]),
        );
        let root = memory_layout(
            "fixture::Root<u16>",
            24,
            8,
            TypeLayoutKind::Adt(AdtLayoutFacts {
                definition: "fixture::Root".to_owned(),
                kind: AdtKind::Struct,
                representation: AdtRepresentationFacts::rust(),
                tag: None,
                variants: vec![VariantLayoutFacts {
                    source_index: 0,
                    name: "Root".to_owned(),
                    discriminant_bits: Some(0),
                    discriminant_type: Some("isize".to_owned()),
                    discriminant_scalar: Some(SourceScalarKind::SignedInteger { bits: 64 }),
                    uninhabited: false,
                    fields: vec![
                        field(0, 0, Some("byte"), 0, u8_facts()),
                        field(1, 1, Some("values"), 2, array),
                        field(
                            2,
                            2,
                            Some("pointer"),
                            8,
                            pointer_facts(5, 4, 4, u16_facts()),
                        ),
                        field(3, 3, Some("tuple"), 12, tuple),
                    ],
                }],
            }),
        );

        let bridged = bridge_type(&root, &target(), "root").unwrap();
        let MirTypeKind::Struct(struct_ty) = &bridged.kind else {
            panic!("root was not bridged as a struct")
        };
        assert_eq!(struct_ty.identity, "fixture::Root<u16>");
        assert_eq!(
            struct_ty.aggregate.padding,
            vec![
                MirPadding { offset: 1, size: 1 },
                MirPadding {
                    offset: 20,
                    size: 4
                },
            ]
        );
        let MirTypeKind::RawPointer {
            address_space,
            mutability,
            ..
        } = &struct_ty.aggregate.fields[2].ty.kind
        else {
            panic!("field was not bridged as a raw pointer")
        };
        assert_eq!(*address_space, MirAddressSpace(5));
        assert_eq!(*mutability, MirMutability::Mutable);
        assert!(bridged.canonical_text().unwrap().contains("offset=12"));
    }

    #[test]
    fn direct_enum_preserves_tag_discriminants_variants_and_padding() {
        let tag_scalar = ScalarLayoutFacts {
            primitive: ScalarPrimitiveFacts::Integer {
                bits: 8,
                signed: false,
            },
            size_bytes: 1,
            abi_alignment_bytes: 1,
            initialized: true,
            valid_range_start: 0,
            valid_range_end: 1,
        };
        let choice = memory_layout(
            "fixture::Choice",
            8,
            4,
            TypeLayoutKind::Adt(AdtLayoutFacts {
                definition: "fixture::Choice".to_owned(),
                kind: AdtKind::Enum,
                representation: AdtRepresentationFacts::rust(),
                tag: Some(EnumTagLayoutFacts {
                    offset_bytes: 0,
                    scalar: tag_scalar,
                    encoding: EnumTagEncodingFacts::Direct,
                }),
                variants: vec![
                    variant(0, "Empty", 0, vec![]),
                    variant(1, "Value", 1, vec![field(0, 0, Some("0"), 4, u32_facts())]),
                ],
            }),
        );

        let bridged = bridge_type(&choice, &target(), "root").unwrap();
        let MirTypeKind::Enum(enum_ty) = bridged.kind else {
            panic!("choice was not bridged as an enum")
        };
        assert_eq!(
            enum_ty.encoding,
            MirEnumEncoding::Direct {
                tag_offset: 0,
                tag: MirScalarType::Int {
                    signed: false,
                    bits: 8,
                },
            }
        );
        assert_eq!(
            enum_ty
                .variants
                .iter()
                .map(|variant| (variant.index, variant.discriminant))
                .collect::<Vec<_>>(),
            [(0, 0), (1, 1)]
        );
        assert_eq!(
            enum_ty.variants[1].aggregate.padding,
            vec![MirPadding { offset: 1, size: 3 }]
        );
    }

    #[test]
    fn niche_enum_allows_the_recorded_payload_overlap_only() {
        let pointer = pointer_facts(0, 8, 8, u8_facts());
        let pointer_scalar = match pointer.backend_representation {
            BackendRepresentationFacts::Scalar(scalar) => scalar,
            _ => unreachable!(),
        };
        let mut none = variant(0, "None", 0, vec![]);
        let mut some = variant(1, "Some", 1, vec![field(0, 0, Some("0"), 0, pointer)]);
        for item in [&mut none, &mut some] {
            item.discriminant_type = Some("isize".to_owned());
            item.discriminant_scalar = Some(SourceScalarKind::SignedInteger { bits: 64 });
        }
        let option = memory_layout(
            "core::option::Option<*mut u8>",
            8,
            8,
            TypeLayoutKind::Adt(AdtLayoutFacts {
                definition: "core::option::Option".to_owned(),
                kind: AdtKind::Enum,
                representation: AdtRepresentationFacts::rust(),
                tag: Some(EnumTagLayoutFacts {
                    offset_bytes: 0,
                    scalar: pointer_scalar,
                    encoding: EnumTagEncodingFacts::Niche {
                        untagged_variant: 1,
                        niche_variants_start: 0,
                        niche_variants_end: 0,
                        niche_start: 0,
                    },
                }),
                variants: vec![none, some],
            }),
        );

        let bridged = bridge_type(&option, &target(), "root").unwrap();
        let MirTypeKind::Enum(enum_ty) = bridged.kind else {
            panic!("option was not bridged as an enum")
        };
        assert!(matches!(
            enum_ty.encoding,
            MirEnumEncoding::Niche {
                niche_offset: 0,
                niche_bits: 64,
                untagged_variant: 1,
                niche_variants_start: 0,
                niche_variants_end: 0,
                niche_start: 0,
            }
        ));
        assert!(
            enum_ty
                .variants
                .iter()
                .all(|variant| variant.aggregate.padding.is_empty())
        );
    }

    #[test]
    fn default_pointer_width_must_match_the_exact_target() {
        let pointer = pointer_facts(0, 4, 4, u8_facts());
        assert!(matches!(
            bridge_type(&pointer, &target(), "root"),
            Err(SemanticLayoutBridgeError::Inconsistent { .. })
        ));

        let non_default = pointer_facts(3, 4, 4, u8_facts());
        assert!(bridge_type(&non_default, &target(), "root").is_ok());
    }

    #[test]
    fn unsupported_or_lossy_shapes_fail_closed() {
        let union = memory_layout(
            "fixture::Bits",
            4,
            4,
            TypeLayoutKind::Adt(AdtLayoutFacts {
                definition: "fixture::Bits".to_owned(),
                kind: AdtKind::Union,
                representation: AdtRepresentationFacts::rust(),
                tag: None,
                variants: vec![],
            }),
        );
        assert!(matches!(
            bridge_type(&union, &target(), "root"),
            Err(SemanticLayoutBridgeError::Unsupported { .. })
        ));

        let stride = memory_layout(
            "fixture::PaddedArray",
            8,
            4,
            TypeLayoutKind::Array(ArrayLayoutFacts {
                length: 2,
                stride_bytes: 4,
                element: Box::new(u16_facts()),
            }),
        );
        assert!(matches!(
            bridge_type(&stride, &target(), "root"),
            Err(SemanticLayoutBridgeError::Unsupported { .. })
        ));

        let mut uninhabited = u8_facts();
        uninhabited.uninhabited = true;
        assert!(matches!(
            bridge_type(&uninhabited, &target(), "root"),
            Err(SemanticLayoutBridgeError::Unsupported { .. })
        ));
    }

    #[test]
    fn malformed_field_records_and_enum_metadata_fail_closed() {
        let malformed_fields = [
            vec![field(1, 0, None, 0, u32_facts())],
            vec![
                field(0, 0, None, 0, u32_facts()),
                field(1, 0, None, 4, u32_facts()),
            ],
            vec![
                field(0, 1, None, 0, u32_facts()),
                field(1, 0, None, 4, u32_facts()),
            ],
            vec![
                field(0, 0, None, 0, u32_facts()),
                field(1, 1, None, 2, u32_facts()),
            ],
            vec![field(0, 0, None, u64::MAX, u32_facts())],
        ];
        for fields in malformed_fields {
            let tuple = memory_layout("tuple", 8, 4, TypeLayoutKind::Tuple(fields));
            assert!(matches!(
                bridge_type(&tuple, &target(), "root"),
                Err(SemanticLayoutBridgeError::Inconsistent { .. })
            ));
        }

        let mut variants = vec![variant(0, "A", 0, vec![]), variant(1, "B", 1, vec![])];
        variants[1].discriminant_scalar = Some(SourceScalarKind::SignedInteger { bits: 8 });
        let bad_enum = memory_layout(
            "fixture::Bad",
            1,
            1,
            TypeLayoutKind::Adt(AdtLayoutFacts {
                definition: "fixture::Bad".to_owned(),
                kind: AdtKind::Enum,
                representation: AdtRepresentationFacts::rust(),
                tag: Some(EnumTagLayoutFacts {
                    offset_bytes: 0,
                    scalar: ScalarLayoutFacts {
                        primitive: ScalarPrimitiveFacts::Integer {
                            bits: 8,
                            signed: false,
                        },
                        size_bytes: 1,
                        abi_alignment_bytes: 1,
                        initialized: true,
                        valid_range_start: 0,
                        valid_range_end: 1,
                    },
                    encoding: EnumTagEncodingFacts::Direct,
                }),
                variants,
            }),
        );
        assert!(matches!(
            bridge_type(&bad_enum, &target(), "root"),
            Err(SemanticLayoutBridgeError::Inconsistent { .. })
        ));
    }

    #[derive(Default)]
    struct BridgeCallbacks {
        evidence: Option<SemanticLayoutEvidenceV1>,
        mismatch: Option<SemanticLayoutBridgeError>,
    }

    impl Callbacks for BridgeCallbacks {
        fn after_analysis<'tcx>(&mut self, _compiler: &Compiler, tcx: TyCtxt<'tcx>) -> Compilation {
            let definition = tcx
                .iter_local_def_id()
                .find(|definition| {
                    matches!(tcx.def_kind(definition.to_def_id()), DefKind::Static { .. })
                        && tcx.item_name(definition.to_def_id()).as_str() == "VALUE"
                })
                .expect("missing VALUE static");
            let ty = tcx.type_of(definition).instantiate_identity();
            let observed = rustc_semantic_layout_target_v1(tcx).unwrap();
            self.evidence = Some(
                extract_semantic_layout_evidence_v1(
                    tcx,
                    ty,
                    StableTypeIdentityV1::new([7; 32]).unwrap(),
                    &observed,
                )
                .unwrap(),
            );
            let different = SemanticLayoutTargetV1::new(
                "different-target",
                observed.data_layout(),
                observed.default_pointer_width_bits(),
            )
            .unwrap();
            self.mismatch = extract_semantic_layout_evidence_v1(
                tcx,
                ty,
                StableTypeIdentityV1::new([7; 32]).unwrap(),
                &different,
            )
            .err();
            Compilation::Stop
        }
    }

    struct CompilerFixture {
        source: PathBuf,
        output: PathBuf,
    }

    impl CompilerFixture {
        fn create() -> Self {
            let stem = format!("fe2o3-g2-layout-bridge-{}", std::process::id());
            let source = std::env::temp_dir().join(format!("{stem}.rs"));
            let output = std::env::temp_dir().join(format!("{stem}.rmeta"));
            fs::write(
                &source,
                r#"
                    #![allow(dead_code)]
                    #[repr(C)]
                    struct Pair { byte: u8, word: u32 }
                    static VALUE: Pair = Pair { byte: 1, word: 2 };
                "#,
            )
            .expect("write bridge fixture");
            Self { source, output }
        }
    }

    impl Drop for CompilerFixture {
        fn drop(&mut self) {
            let _ = fs::remove_file(&self.source);
            let _ = fs::remove_file(&self.output);
        }
    }

    #[test]
    fn rustc_extraction_is_target_explicit_and_frontend_identity_bound() {
        let fixture = CompilerFixture::create();
        let sysroot = Command::new("rustc")
            .args(["--print", "sysroot"])
            .output()
            .expect("query rustc sysroot");
        assert!(sysroot.status.success());
        let sysroot = String::from_utf8(sysroot.stdout).unwrap();
        let args = vec![
            "rustc".to_owned(),
            "--crate-name".to_owned(),
            "fe2o3_g2_layout_bridge_fixture".to_owned(),
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
        let mut callbacks = BridgeCallbacks::default();
        rustc_driver::run_compiler(&args, &mut callbacks);

        let evidence = callbacks.evidence.expect("bridge callback did not run");
        assert_eq!(evidence.source_type_identity().as_bytes(), &[7; 32]);
        let MirTypeKind::Struct(struct_ty) = &evidence.semantic_type().kind else {
            panic!("fixture was not bridged as a struct")
        };
        assert_eq!(
            struct_ty
                .aggregate
                .fields
                .iter()
                .map(|field| (field.name.as_deref(), field.offset))
                .collect::<Vec<_>>(),
            [(Some("byte"), 0), (Some("word"), 4)]
        );
        assert!(
            std::str::from_utf8(evidence.canonical_bytes())
                .unwrap()
                .contains(evidence.target().llvm_target())
        );
        assert!(matches!(
            callbacks.mismatch,
            Some(SemanticLayoutBridgeError::TargetMismatch { .. })
        ));
    }
}

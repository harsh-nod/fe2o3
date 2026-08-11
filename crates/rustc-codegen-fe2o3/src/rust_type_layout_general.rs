//! Bounded rustc layout facts for general, fully monomorphized Rust types.
//!
//! This module is intentionally independent of the typed vecadd artifact
//! profile. The facts here are a compiler-side foundation, not trusted
//! evidence: no artifact or host authorization path consumes them yet. This
//! type-only layer rejects function-address-bearing types; a future constant
//! extractor must likewise reject allocation relocations until it records them.

use std::fmt;

use rustc_abi::{BackendRepr, HasDataLayout, Primitive, Scalar, TagEncoding, Variants};
use rustc_hir::Mutability;
use rustc_middle::ty::layout::{LayoutCx, LayoutOf, TyAndLayout};
use rustc_middle::ty::print::with_no_trimmed_paths;
use rustc_middle::ty::{FloatTy, IntTy, Ty, TyCtxt, TyKind, TypeVisitableExt, TypingEnv, UintTy};

const DEFAULT_MAX_DEPTH: usize = 64;
const DEFAULT_MAX_NODES: usize = 4_096;
const DEFAULT_MAX_FIELDS: usize = 1_024;
const DEFAULT_MAX_VARIANTS: usize = 256;
const DEFAULT_MAX_ARRAY_ELEMENTS: u64 = 1 << 24;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct ExtractionLimits {
    pub(crate) max_depth: usize,
    pub(crate) max_nodes: usize,
    pub(crate) max_fields_per_aggregate: usize,
    pub(crate) max_variants: usize,
    pub(crate) max_array_elements: u64,
}

impl Default for ExtractionLimits {
    fn default() -> Self {
        Self {
            max_depth: DEFAULT_MAX_DEPTH,
            max_nodes: DEFAULT_MAX_NODES,
            max_fields_per_aggregate: DEFAULT_MAX_FIELDS,
            max_variants: DEFAULT_MAX_VARIANTS,
            max_array_elements: DEFAULT_MAX_ARRAY_ELEMENTS,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum LimitKind {
    Depth,
    Nodes,
    Fields,
    Variants,
    ArrayElements,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum RelocationKind {
    FunctionItem,
    FunctionPointer,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum GeneralLayoutExtractError {
    NotFullyMonomorphized {
        rust_type: String,
    },
    Normalization {
        rust_type: String,
        detail: String,
    },
    Unsized {
        path: String,
        rust_type: String,
    },
    Cycle {
        path: String,
        rust_type: String,
    },
    RelocationsUnsupported {
        path: String,
        rust_type: String,
        kind: RelocationKind,
    },
    UnsupportedType {
        path: String,
        rust_type: String,
        detail: &'static str,
    },
    Layout {
        path: String,
        rust_type: String,
        detail: String,
    },
    InconsistentLayout {
        path: String,
        detail: String,
    },
    BoundExceeded {
        path: String,
        kind: LimitKind,
        actual: u64,
        limit: u64,
    },
}

impl fmt::Display for GeneralLayoutExtractError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotFullyMonomorphized { rust_type } => write!(
                formatter,
                "layout extraction requires a fully monomorphized type, found `{rust_type}`"
            ),
            Self::Normalization { rust_type, detail } => {
                write!(formatter, "failed to normalize `{rust_type}`: {detail}")
            }
            Self::Unsized { path, rust_type } => {
                write!(formatter, "unsized value at {path}: `{rust_type}`")
            }
            Self::Cycle { path, rust_type } => {
                write!(formatter, "recursive type cycle at {path}: `{rust_type}`")
            }
            Self::RelocationsUnsupported {
                path,
                rust_type,
                kind,
            } => write!(
                formatter,
                "unsupported {kind:?} relocation at {path}: `{rust_type}`"
            ),
            Self::UnsupportedType {
                path,
                rust_type,
                detail,
            } => write!(
                formatter,
                "unsupported type at {path}: `{rust_type}` ({detail})"
            ),
            Self::Layout {
                path,
                rust_type,
                detail,
            } => write!(
                formatter,
                "failed to compute layout at {path} for `{rust_type}`: {detail}"
            ),
            Self::InconsistentLayout { path, detail } => {
                write!(formatter, "inconsistent rustc layout at {path}: {detail}")
            }
            Self::BoundExceeded {
                path,
                kind,
                actual,
                limit,
            } => write!(
                formatter,
                "layout extraction {kind:?} bound exceeded at {path}: {actual} > {limit}"
            ),
        }
    }
}

impl std::error::Error for GeneralLayoutExtractError {}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ScalarPrimitiveFacts {
    Pointer { address_space: u32 },
    Integer { bits: u64, signed: bool },
    Float { bits: u64 },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct ScalarLayoutFacts {
    pub(crate) primitive: ScalarPrimitiveFacts,
    pub(crate) size_bytes: u64,
    pub(crate) abi_alignment_bytes: u64,
    pub(crate) initialized: bool,
    pub(crate) valid_range_start: u128,
    pub(crate) valid_range_end: u128,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum BackendRepresentationFacts {
    Scalar(ScalarLayoutFacts),
    ScalarPair {
        first: ScalarLayoutFacts,
        second: ScalarLayoutFacts,
        second_offset_bytes: u64,
    },
    Memory,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum SourceScalarKind {
    Bool,
    Char,
    SignedInteger { bits: u64 },
    UnsignedInteger { bits: u64 },
    Float { bits: u64 },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum PointerKind {
    SharedReference,
    MutableReference,
    ConstRaw,
    MutRaw,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct PointerLayoutFacts {
    pub(crate) kind: PointerKind,
    pub(crate) address_space: u32,
    pub(crate) pointee: Box<TypeLayoutFacts>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct ArrayLayoutFacts {
    pub(crate) length: u64,
    pub(crate) stride_bytes: u64,
    pub(crate) element: Box<TypeLayoutFacts>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct FieldLayoutFacts {
    pub(crate) source_index: usize,
    pub(crate) memory_index: usize,
    pub(crate) name: Option<String>,
    pub(crate) offset_bytes: u64,
    pub(crate) layout: TypeLayoutFacts,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum AdtKind {
    Struct,
    Enum,
    Union,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct AdtRepresentationFacts {
    pub(crate) c: bool,
    pub(crate) transparent: bool,
    pub(crate) explicit_integer: bool,
    pub(crate) packed_alignment_bytes: Option<u64>,
    pub(crate) requested_alignment_bytes: Option<u64>,
}

impl AdtRepresentationFacts {
    #[cfg(test)]
    pub(crate) const fn rust() -> Self {
        Self {
            c: false,
            transparent: false,
            explicit_integer: false,
            packed_alignment_bytes: None,
            requested_alignment_bytes: None,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct VariantLayoutFacts {
    pub(crate) source_index: u32,
    pub(crate) name: String,
    pub(crate) discriminant_bits: Option<u128>,
    pub(crate) discriminant_type: Option<String>,
    pub(crate) discriminant_scalar: Option<SourceScalarKind>,
    pub(crate) uninhabited: bool,
    pub(crate) fields: Vec<FieldLayoutFacts>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum EnumTagEncodingFacts {
    Direct,
    Niche {
        untagged_variant: u32,
        niche_variants_start: u32,
        niche_variants_end: u32,
        niche_start: u128,
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct EnumTagLayoutFacts {
    pub(crate) offset_bytes: u64,
    pub(crate) scalar: ScalarLayoutFacts,
    pub(crate) encoding: EnumTagEncodingFacts,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct NicheLayoutFacts {
    pub(crate) offset_bytes: u64,
    pub(crate) primitive: ScalarPrimitiveFacts,
    pub(crate) valid_range_start: u128,
    pub(crate) valid_range_end: u128,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct AdtLayoutFacts {
    pub(crate) definition: String,
    pub(crate) kind: AdtKind,
    pub(crate) representation: AdtRepresentationFacts,
    pub(crate) tag: Option<EnumTagLayoutFacts>,
    pub(crate) variants: Vec<VariantLayoutFacts>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum TypeLayoutKind {
    Scalar(SourceScalarKind),
    Pointer(PointerLayoutFacts),
    Array(ArrayLayoutFacts),
    Tuple(Vec<FieldLayoutFacts>),
    Adt(AdtLayoutFacts),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct TypeLayoutFacts {
    pub(crate) rust_type: String,
    pub(crate) size_bytes: u64,
    pub(crate) abi_alignment_bytes: u64,
    pub(crate) unadjusted_abi_alignment_bytes: u64,
    pub(crate) maximum_requested_alignment_bytes: Option<u64>,
    pub(crate) uninhabited: bool,
    pub(crate) backend_representation: BackendRepresentationFacts,
    pub(crate) largest_niche: Option<NicheLayoutFacts>,
    pub(crate) kind: TypeLayoutKind,
}

/// Extract target layout facts for a fully monomorphized, sized Rust type.
///
/// References and raw pointers recursively include their pointee layout. This
/// makes unsized pointees and recursive pointer graphs explicit failures until
/// the eventual artifact schema has a representation for them.
pub(crate) fn extract_general_layout<'tcx>(
    tcx: TyCtxt<'tcx>,
    ty: Ty<'tcx>,
) -> Result<TypeLayoutFacts, GeneralLayoutExtractError> {
    extract_general_layout_with_limits(tcx, ty, ExtractionLimits::default())
}

pub(crate) fn extract_general_layout_with_limits<'tcx>(
    tcx: TyCtxt<'tcx>,
    ty: Ty<'tcx>,
    limits: ExtractionLimits,
) -> Result<TypeLayoutFacts, GeneralLayoutExtractError> {
    require_monomorphized_input(ty)?;
    let typing_env = TypingEnv::fully_monomorphized();
    let normalized = tcx
        .try_normalize_erasing_regions(typing_env, ty)
        .map_err(|error| GeneralLayoutExtractError::Normalization {
            rust_type: type_name(ty),
            detail: format!("{error:?}"),
        })?;
    require_normalized_monomorphized(normalized)?;

    let layout_cx = LayoutCx::new(tcx, typing_env);
    let mut extractor = Extractor {
        tcx,
        layout_cx,
        limits,
        nodes: 0,
        active: Vec::new(),
    };
    extractor.extract_type(normalized, "root".to_owned(), 0)
}

fn require_monomorphized_input(ty: Ty<'_>) -> Result<(), GeneralLayoutExtractError> {
    if ty.has_non_region_param()
        || ty.has_infer()
        || ty.has_escaping_bound_vars()
        || ty.has_placeholders()
    {
        return Err(GeneralLayoutExtractError::NotFullyMonomorphized {
            rust_type: type_name(ty),
        });
    }
    Ok(())
}

fn require_normalized_monomorphized(ty: Ty<'_>) -> Result<(), GeneralLayoutExtractError> {
    require_monomorphized_input(ty)?;
    if ty.has_aliases() {
        return Err(GeneralLayoutExtractError::NotFullyMonomorphized {
            rust_type: type_name(ty),
        });
    }
    Ok(())
}

struct Extractor<'tcx> {
    tcx: TyCtxt<'tcx>,
    layout_cx: LayoutCx<'tcx>,
    limits: ExtractionLimits,
    nodes: usize,
    active: Vec<Ty<'tcx>>,
}

impl<'tcx> Extractor<'tcx> {
    fn extract_type(
        &mut self,
        ty: Ty<'tcx>,
        path: String,
        depth: usize,
    ) -> Result<TypeLayoutFacts, GeneralLayoutExtractError> {
        require_normalized_monomorphized(ty)?;
        if self.active.contains(&ty) {
            return Err(GeneralLayoutExtractError::Cycle {
                path,
                rust_type: type_name(ty),
            });
        }
        self.reserve(&path, LimitKind::Depth, depth, self.limits.max_depth)?;
        self.nodes =
            self.nodes
                .checked_add(1)
                .ok_or_else(|| GeneralLayoutExtractError::BoundExceeded {
                    path: path.clone(),
                    kind: LimitKind::Nodes,
                    actual: u64::MAX,
                    limit: self.limits.max_nodes as u64,
                })?;
        self.reserve(&path, LimitKind::Nodes, self.nodes, self.limits.max_nodes)?;

        if !ty.is_sized(self.tcx, self.layout_cx.typing_env) {
            return Err(GeneralLayoutExtractError::Unsized {
                path,
                rust_type: type_name(ty),
            });
        }
        let layout =
            self.layout_cx
                .layout_of(ty)
                .map_err(|error| GeneralLayoutExtractError::Layout {
                    path: path.clone(),
                    rust_type: type_name(ty),
                    detail: error.to_string(),
                })?;
        if layout.backend_repr.is_unsized() {
            return Err(GeneralLayoutExtractError::Unsized {
                path,
                rust_type: type_name(ty),
            });
        }

        self.active.push(ty);
        let result = self.extract_sized_type(ty, layout, path, depth);
        let removed = self.active.pop();
        debug_assert_eq!(removed, Some(ty));
        result
    }

    fn extract_sized_type(
        &mut self,
        ty: Ty<'tcx>,
        layout: TyAndLayout<'tcx>,
        path: String,
        depth: usize,
    ) -> Result<TypeLayoutFacts, GeneralLayoutExtractError> {
        let backend_representation = self.backend_representation(layout, &path)?;
        let largest_niche = layout.largest_niche.map(|niche| NicheLayoutFacts {
            offset_bytes: niche.offset.bytes(),
            primitive: primitive_facts(niche.value),
            valid_range_start: niche.valid_range.start,
            valid_range_end: niche.valid_range.end,
        });
        let kind = match *ty.kind() {
            TyKind::Bool => TypeLayoutKind::Scalar(SourceScalarKind::Bool),
            TyKind::Char => TypeLayoutKind::Scalar(SourceScalarKind::Char),
            TyKind::Int(integer) => TypeLayoutKind::Scalar(SourceScalarKind::SignedInteger {
                bits: integer_bits(&self.layout_cx, integer),
            }),
            TyKind::Uint(integer) => TypeLayoutKind::Scalar(SourceScalarKind::UnsignedInteger {
                bits: unsigned_integer_bits(&self.layout_cx, integer),
            }),
            TyKind::Float(float) => TypeLayoutKind::Scalar(SourceScalarKind::Float {
                bits: float_bits(float),
            }),
            TyKind::Ref(_, pointee, mutability) => TypeLayoutKind::Pointer(self.pointer_facts(
                pointee,
                mutability,
                true,
                &backend_representation,
                &path,
                depth,
            )?),
            TyKind::RawPtr(pointee, mutability) => TypeLayoutKind::Pointer(self.pointer_facts(
                pointee,
                mutability,
                false,
                &backend_representation,
                &path,
                depth,
            )?),
            TyKind::Array(element, length) => {
                let length = length.try_to_target_usize(self.tcx).ok_or_else(|| {
                    GeneralLayoutExtractError::NotFullyMonomorphized {
                        rust_type: type_name(ty),
                    }
                })?;
                self.reserve_u64(
                    &path,
                    LimitKind::ArrayElements,
                    length,
                    self.limits.max_array_elements,
                )?;
                let rustc_abi::FieldsShape::Array { stride, count } = layout.fields else {
                    return Err(GeneralLayoutExtractError::InconsistentLayout {
                        path,
                        detail: format!(
                            "array source type has non-array field placement {:?}",
                            layout.fields
                        ),
                    });
                };
                if count != length {
                    return Err(GeneralLayoutExtractError::InconsistentLayout {
                        path,
                        detail: format!(
                            "array length {length} disagrees with layout field count {count}"
                        ),
                    });
                }
                let element_layout =
                    self.extract_type(element, format!("{path}.element"), depth.saturating_add(1))?;
                validate_array_extent(
                    &path,
                    layout.size.bytes(),
                    stride.bytes(),
                    length,
                    element_layout.size_bytes,
                )?;
                TypeLayoutKind::Array(ArrayLayoutFacts {
                    length,
                    stride_bytes: stride.bytes(),
                    element: Box::new(element_layout),
                })
            }
            TyKind::Tuple(_) => {
                TypeLayoutKind::Tuple(self.aggregate_fields(layout, None, &path, depth)?)
            }
            TyKind::Adt(definition, _) => {
                TypeLayoutKind::Adt(self.adt_facts(ty, definition, layout, &path, depth)?)
            }
            TyKind::FnDef(..) => {
                return Err(GeneralLayoutExtractError::RelocationsUnsupported {
                    path,
                    rust_type: type_name(ty),
                    kind: RelocationKind::FunctionItem,
                });
            }
            TyKind::FnPtr(..) => {
                return Err(GeneralLayoutExtractError::RelocationsUnsupported {
                    path,
                    rust_type: type_name(ty),
                    kind: RelocationKind::FunctionPointer,
                });
            }
            TyKind::Never => {
                return Err(GeneralLayoutExtractError::UnsupportedType {
                    path,
                    rust_type: type_name(ty),
                    detail: "never values have no inhabited scalar or aggregate representation",
                });
            }
            TyKind::Str | TyKind::Slice(_) | TyKind::Dynamic(..) | TyKind::Foreign(..) => {
                return Err(GeneralLayoutExtractError::Unsized {
                    path,
                    rust_type: type_name(ty),
                });
            }
            TyKind::Closure(..)
            | TyKind::CoroutineClosure(..)
            | TyKind::Coroutine(..)
            | TyKind::CoroutineWitness(..) => {
                return Err(GeneralLayoutExtractError::UnsupportedType {
                    path,
                    rust_type: type_name(ty),
                    detail: "closure and coroutine layouts require capture identity facts",
                });
            }
            TyKind::Pat(base, _) => {
                TypeLayoutKind::Scalar(source_scalar_kind(&self.layout_cx, base).ok_or_else(
                    || GeneralLayoutExtractError::UnsupportedType {
                        path: path.clone(),
                        rust_type: type_name(ty),
                        detail: "only scalar pattern types have an exact validity representation",
                    },
                )?)
            }
            TyKind::UnsafeBinder(..) => {
                return Err(GeneralLayoutExtractError::UnsupportedType {
                    path,
                    rust_type: type_name(ty),
                    detail: "unsafe binder types are not layout-authorized",
                });
            }
            TyKind::Alias(..)
            | TyKind::Param(..)
            | TyKind::Bound(..)
            | TyKind::Placeholder(..)
            | TyKind::Infer(..)
            | TyKind::Error(..) => {
                return Err(GeneralLayoutExtractError::NotFullyMonomorphized {
                    rust_type: type_name(ty),
                });
            }
        };

        Ok(TypeLayoutFacts {
            rust_type: type_name(ty),
            size_bytes: layout.size.bytes(),
            abi_alignment_bytes: layout.align.abi.bytes(),
            unadjusted_abi_alignment_bytes: layout.unadjusted_abi_align.bytes(),
            maximum_requested_alignment_bytes: layout.max_repr_align.map(|align| align.bytes()),
            uninhabited: layout.is_uninhabited(),
            backend_representation,
            largest_niche,
            kind,
        })
    }

    fn backend_representation(
        &self,
        layout: TyAndLayout<'tcx>,
        path: &str,
    ) -> Result<BackendRepresentationFacts, GeneralLayoutExtractError> {
        match layout.backend_repr {
            BackendRepr::Scalar(scalar) => Ok(BackendRepresentationFacts::Scalar(scalar_facts(
                &self.layout_cx,
                scalar,
            ))),
            BackendRepr::ScalarPair(first, second) => {
                let first = scalar_facts(&self.layout_cx, first);
                let second = scalar_facts(&self.layout_cx, second);
                let second_offset_bytes = first
                    .size_bytes
                    .next_multiple_of(second.abi_alignment_bytes);
                Ok(BackendRepresentationFacts::ScalarPair {
                    first,
                    second,
                    second_offset_bytes,
                })
            }
            BackendRepr::Memory { sized: true } => Ok(BackendRepresentationFacts::Memory),
            BackendRepr::Memory { sized: false } => Err(GeneralLayoutExtractError::Unsized {
                path: path.to_owned(),
                rust_type: type_name(layout.ty),
            }),
            BackendRepr::SimdVector { .. } | BackendRepr::SimdScalableVector { .. } => {
                Err(GeneralLayoutExtractError::UnsupportedType {
                    path: path.to_owned(),
                    rust_type: type_name(layout.ty),
                    detail: "SIMD backend representations require lane facts",
                })
            }
        }
    }

    fn pointer_facts(
        &mut self,
        pointee: Ty<'tcx>,
        mutability: Mutability,
        reference: bool,
        backend: &BackendRepresentationFacts,
        path: &str,
        depth: usize,
    ) -> Result<PointerLayoutFacts, GeneralLayoutExtractError> {
        let address_space = match backend {
            BackendRepresentationFacts::Scalar(scalar)
            | BackendRepresentationFacts::ScalarPair { first: scalar, .. } => {
                match scalar.primitive {
                    ScalarPrimitiveFacts::Pointer { address_space } => address_space,
                    unexpected => {
                        return Err(GeneralLayoutExtractError::InconsistentLayout {
                            path: path.to_owned(),
                            detail: format!(
                                "pointer source type has non-pointer data scalar {unexpected:?}"
                            ),
                        });
                    }
                }
            }
            BackendRepresentationFacts::Memory => {
                return Err(GeneralLayoutExtractError::InconsistentLayout {
                    path: path.to_owned(),
                    detail: "pointer source type has a memory backend representation".to_owned(),
                });
            }
        };
        let kind = match (reference, mutability) {
            (true, Mutability::Not) => PointerKind::SharedReference,
            (true, Mutability::Mut) => PointerKind::MutableReference,
            (false, Mutability::Not) => PointerKind::ConstRaw,
            (false, Mutability::Mut) => PointerKind::MutRaw,
        };
        let pointee =
            self.extract_type(pointee, format!("{path}.pointee"), depth.saturating_add(1))?;
        Ok(PointerLayoutFacts {
            kind,
            address_space,
            pointee: Box::new(pointee),
        })
    }

    fn adt_facts(
        &mut self,
        ty: Ty<'tcx>,
        definition: rustc_middle::ty::AdtDef<'tcx>,
        layout: TyAndLayout<'tcx>,
        path: &str,
        depth: usize,
    ) -> Result<AdtLayoutFacts, GeneralLayoutExtractError> {
        let variant_count = definition.variants().len();
        self.reserve(
            path,
            LimitKind::Variants,
            variant_count,
            self.limits.max_variants,
        )?;
        let kind = if definition.is_struct() {
            AdtKind::Struct
        } else if definition.is_enum() {
            AdtKind::Enum
        } else if definition.is_union() {
            AdtKind::Union
        } else {
            return Err(GeneralLayoutExtractError::UnsupportedType {
                path: path.to_owned(),
                rust_type: type_name(ty),
                detail: "unknown ADT kind",
            });
        };
        let tag = self.enum_tag(layout, path)?;
        if !definition.is_enum() && tag.is_some() {
            return Err(GeneralLayoutExtractError::InconsistentLayout {
                path: path.to_owned(),
                detail: "non-enum ADT has a multi-variant tag".to_owned(),
            });
        }

        let mut variants = Vec::with_capacity(variant_count);
        for (variant_index, variant) in definition.variants().iter_enumerated() {
            let variant_path = format!("{path}.variant[{}]", variant_index.as_u32());
            let variant_layout = if definition.is_enum() {
                layout.for_variant(&self.layout_cx, variant_index)
            } else {
                layout
            };
            let field_names = variant
                .fields
                .iter()
                .map(|field| field.name.to_string())
                .collect::<Vec<_>>();
            let fields =
                self.aggregate_fields(variant_layout, Some(&field_names), &variant_path, depth)?;
            let discriminant = ty.discriminant_for_variant(self.tcx, variant_index);
            variants.push(VariantLayoutFacts {
                source_index: variant_index.as_u32(),
                name: variant.name.to_string(),
                discriminant_bits: discriminant.map(|value| value.val),
                discriminant_type: discriminant.map(|value| type_name(value.ty)),
                discriminant_scalar: discriminant
                    .and_then(|value| source_scalar_kind(&self.layout_cx, value.ty)),
                uninhabited: variant_layout.is_uninhabited(),
                fields,
            });
        }

        Ok(AdtLayoutFacts {
            definition: self.tcx.def_path_str(definition.did()),
            kind,
            representation: {
                let repr = definition.repr();
                AdtRepresentationFacts {
                    c: repr.c(),
                    transparent: repr.transparent(),
                    explicit_integer: repr.int.is_some(),
                    packed_alignment_bytes: repr.pack.map(|align| align.bytes()),
                    requested_alignment_bytes: repr.align.map(|align| align.bytes()),
                }
            },
            tag,
            variants,
        })
    }

    fn enum_tag(
        &self,
        layout: TyAndLayout<'tcx>,
        path: &str,
    ) -> Result<Option<EnumTagLayoutFacts>, GeneralLayoutExtractError> {
        let variants = &layout.variants;
        let Variants::Multiple {
            tag,
            tag_encoding,
            tag_field,
            ..
        } = variants
        else {
            return Ok(None);
        };
        let tag_field_index = tag_field.as_usize();
        if tag_field_index >= layout.fields.count() {
            return Err(GeneralLayoutExtractError::InconsistentLayout {
                path: path.to_owned(),
                detail: format!(
                    "enum tag field {tag_field_index} is outside {} layout fields",
                    layout.fields.count()
                ),
            });
        }
        let encoding = match tag_encoding {
            TagEncoding::Direct => EnumTagEncodingFacts::Direct,
            TagEncoding::Niche {
                untagged_variant,
                niche_variants,
                niche_start,
            } => EnumTagEncodingFacts::Niche {
                untagged_variant: untagged_variant.as_u32(),
                niche_variants_start: niche_variants.start().as_u32(),
                niche_variants_end: niche_variants.end().as_u32(),
                niche_start: *niche_start,
            },
        };
        Ok(Some(EnumTagLayoutFacts {
            offset_bytes: layout.fields.offset(tag_field_index).bytes(),
            scalar: scalar_facts(&self.layout_cx, *tag),
            encoding,
        }))
    }

    fn aggregate_fields(
        &mut self,
        layout: TyAndLayout<'tcx>,
        names: Option<&[String]>,
        path: &str,
        depth: usize,
    ) -> Result<Vec<FieldLayoutFacts>, GeneralLayoutExtractError> {
        let field_count = layout.fields.count();
        self.reserve(
            path,
            LimitKind::Fields,
            field_count,
            self.limits.max_fields_per_aggregate,
        )?;
        if let Some(names) = names
            && names.len() != field_count
        {
            return Err(GeneralLayoutExtractError::InconsistentLayout {
                path: path.to_owned(),
                detail: format!(
                    "source declares {} fields but layout exposes {field_count}",
                    names.len()
                ),
            });
        }
        let source_indices = layout.fields.index_by_increasing_offset();
        let memory_order = memory_order_from_source_indices(path, field_count, source_indices)?;
        let mut fields = Vec::with_capacity(field_count);
        for (source_index, memory_index) in memory_order.into_iter().enumerate() {
            let field_path = format!("{path}.field[{source_index}]");
            let field_layout = layout.field(&self.layout_cx, source_index);
            let offset_bytes = layout.fields.offset(source_index).bytes();
            validate_field_extent(
                &field_path,
                layout.size.bytes(),
                offset_bytes,
                field_layout.size.bytes(),
            )?;
            let facts =
                self.extract_type(field_layout.ty, field_path.clone(), depth.saturating_add(1))?;
            if facts.size_bytes != field_layout.size.bytes()
                || facts.abi_alignment_bytes != field_layout.align.abi.bytes()
            {
                return Err(GeneralLayoutExtractError::InconsistentLayout {
                    path: field_path,
                    detail: "recursive field layout disagrees with parent projection".to_owned(),
                });
            }
            fields.push(FieldLayoutFacts {
                source_index,
                memory_index,
                name: names.map(|names| names[source_index].clone()),
                offset_bytes,
                layout: facts,
            });
        }
        Ok(fields)
    }

    fn reserve(
        &self,
        path: &str,
        kind: LimitKind,
        actual: usize,
        limit: usize,
    ) -> Result<(), GeneralLayoutExtractError> {
        self.reserve_u64(path, kind, actual as u64, limit as u64)
    }

    fn reserve_u64(
        &self,
        path: &str,
        kind: LimitKind,
        actual: u64,
        limit: u64,
    ) -> Result<(), GeneralLayoutExtractError> {
        if actual > limit {
            return Err(GeneralLayoutExtractError::BoundExceeded {
                path: path.to_owned(),
                kind,
                actual,
                limit,
            });
        }
        Ok(())
    }
}

fn scalar_facts(layout_cx: &LayoutCx<'_>, scalar: Scalar) -> ScalarLayoutFacts {
    let valid_range = scalar.valid_range(layout_cx);
    ScalarLayoutFacts {
        primitive: primitive_facts(scalar.primitive()),
        size_bytes: scalar.size(layout_cx).bytes(),
        abi_alignment_bytes: scalar.align(layout_cx).abi.bytes(),
        initialized: matches!(scalar, Scalar::Initialized { .. }),
        valid_range_start: valid_range.start,
        valid_range_end: valid_range.end,
    }
}

fn primitive_facts(primitive: Primitive) -> ScalarPrimitiveFacts {
    match primitive {
        Primitive::Pointer(address_space) => ScalarPrimitiveFacts::Pointer {
            address_space: address_space.0,
        },
        Primitive::Int(integer, signed) => ScalarPrimitiveFacts::Integer {
            bits: integer.size().bits(),
            signed,
        },
        Primitive::Float(float) => ScalarPrimitiveFacts::Float {
            bits: float.size().bits(),
        },
    }
}

fn integer_bits(layout_cx: &LayoutCx<'_>, integer: IntTy) -> u64 {
    match integer {
        IntTy::I8 => 8,
        IntTy::I16 => 16,
        IntTy::I32 => 32,
        IntTy::I64 => 64,
        IntTy::I128 => 128,
        IntTy::Isize => layout_cx.data_layout().pointer_size().bits(),
    }
}

fn unsigned_integer_bits(layout_cx: &LayoutCx<'_>, integer: UintTy) -> u64 {
    match integer {
        UintTy::U8 => 8,
        UintTy::U16 => 16,
        UintTy::U32 => 32,
        UintTy::U64 => 64,
        UintTy::U128 => 128,
        UintTy::Usize => layout_cx.data_layout().pointer_size().bits(),
    }
}

fn float_bits(float: FloatTy) -> u64 {
    match float {
        FloatTy::F16 => 16,
        FloatTy::F32 => 32,
        FloatTy::F64 => 64,
        FloatTy::F128 => 128,
    }
}

fn source_scalar_kind(layout_cx: &LayoutCx<'_>, ty: Ty<'_>) -> Option<SourceScalarKind> {
    match *ty.kind() {
        TyKind::Bool => Some(SourceScalarKind::Bool),
        TyKind::Char => Some(SourceScalarKind::Char),
        TyKind::Int(integer) => Some(SourceScalarKind::SignedInteger {
            bits: integer_bits(layout_cx, integer),
        }),
        TyKind::Uint(integer) => Some(SourceScalarKind::UnsignedInteger {
            bits: unsigned_integer_bits(layout_cx, integer),
        }),
        TyKind::Float(float) => Some(SourceScalarKind::Float {
            bits: float_bits(float),
        }),
        _ => None,
    }
}

fn memory_order_from_source_indices(
    path: &str,
    field_count: usize,
    source_indices: impl IntoIterator<Item = usize>,
) -> Result<Vec<usize>, GeneralLayoutExtractError> {
    let mut memory_order = vec![usize::MAX; field_count];
    let mut observed = 0usize;
    for (memory_index, source_index) in source_indices.into_iter().enumerate() {
        if source_index >= field_count || memory_order[source_index] != usize::MAX {
            return Err(GeneralLayoutExtractError::InconsistentLayout {
                path: path.to_owned(),
                detail: format!(
                    "field memory order is not a permutation at source index {source_index}"
                ),
            });
        }
        memory_order[source_index] = memory_index;
        observed = observed.saturating_add(1);
    }
    if observed != field_count {
        return Err(GeneralLayoutExtractError::InconsistentLayout {
            path: path.to_owned(),
            detail: format!(
                "field memory order contains {observed} entries for {field_count} fields"
            ),
        });
    }
    Ok(memory_order)
}

fn validate_field_extent(
    path: &str,
    container_size: u64,
    offset: u64,
    field_size: u64,
) -> Result<(), GeneralLayoutExtractError> {
    let end = offset.checked_add(field_size).ok_or_else(|| {
        GeneralLayoutExtractError::InconsistentLayout {
            path: path.to_owned(),
            detail: "field extent overflows u64".to_owned(),
        }
    })?;
    if end > container_size {
        return Err(GeneralLayoutExtractError::InconsistentLayout {
            path: path.to_owned(),
            detail: format!("field extent {offset}..{end} exceeds container size {container_size}"),
        });
    }
    Ok(())
}

fn validate_array_extent(
    path: &str,
    array_size: u64,
    stride: u64,
    count: u64,
    element_size: u64,
) -> Result<(), GeneralLayoutExtractError> {
    if stride < element_size {
        return Err(GeneralLayoutExtractError::InconsistentLayout {
            path: path.to_owned(),
            detail: format!("array stride {stride} is smaller than element size {element_size}"),
        });
    }
    let expected =
        stride
            .checked_mul(count)
            .ok_or_else(|| GeneralLayoutExtractError::InconsistentLayout {
                path: path.to_owned(),
                detail: "array extent overflows u64".to_owned(),
            })?;
    if expected != array_size {
        return Err(GeneralLayoutExtractError::InconsistentLayout {
            path: path.to_owned(),
            detail: format!(
                "array stride {stride} and count {count} imply size {expected}, found {array_size}"
            ),
        });
    }
    Ok(())
}

fn type_name(ty: Ty<'_>) -> String {
    with_no_trimmed_paths!(format!("{ty}"))
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::PathBuf;
    use std::process::Command;

    use rustc_driver::{Callbacks, Compilation};
    use rustc_hir::def::DefKind;
    use rustc_interface::interface::Compiler;
    use rustc_middle::ty::{Ty, TyCtxt};
    use rustc_span::Symbol;

    use super::{
        AdtKind, EnumTagEncodingFacts, ExtractionLimits, GeneralLayoutExtractError, LimitKind,
        RelocationKind, SourceScalarKind, TypeLayoutFacts, TypeLayoutKind, extract_general_layout,
        extract_general_layout_with_limits, memory_order_from_source_indices,
        validate_array_extent, validate_field_extent,
    };

    const FIXTURE_SOURCE: &str = r#"
#![allow(dead_code)]

#[repr(C)]
struct Pair {
    byte: u8,
    word: u32,
}

#[repr(C)]
union Bits {
    integer: u32,
    float: f32,
}

#[repr(u8)]
enum Choice {
    Empty,
    Pair(Pair),
    Pointer(*const u16),
}

#[repr(C)]
struct Root {
    pair: Pair,
    values: [u16; 3],
    tuple: (u8, u32),
    choice: Choice,
    bits: Bits,
    maybe: Option<&'static u8>,
}

struct Node {
    next: *const Node,
}

static BYTE: u8 = 7;
const ROOT: Root = Root {
    pair: Pair { byte: 1, word: 2 },
    values: [3, 4, 5],
    tuple: (6, 7),
    choice: Choice::Empty,
    bits: Bits { integer: 8 },
    maybe: Some(&BYTE),
};
const NODE: Node = Node { next: core::ptr::null() };

fn target() {}
const FUNCTION: fn() = target;
"#;

    struct DriverResults {
        root: Result<TypeLayoutFacts, GeneralLayoutExtractError>,
        cycle: Result<TypeLayoutFacts, GeneralLayoutExtractError>,
        relocation: Result<TypeLayoutFacts, GeneralLayoutExtractError>,
        function_item: Result<TypeLayoutFacts, GeneralLayoutExtractError>,
        unsized_value: Result<TypeLayoutFacts, GeneralLayoutExtractError>,
        unmonomorphized: Result<TypeLayoutFacts, GeneralLayoutExtractError>,
        unit: Result<TypeLayoutFacts, GeneralLayoutExtractError>,
        bounded: Result<TypeLayoutFacts, GeneralLayoutExtractError>,
    }

    #[derive(Default)]
    struct LayoutCallbacks {
        results: Option<DriverResults>,
    }

    impl Callbacks for LayoutCallbacks {
        fn after_analysis<'tcx>(&mut self, _compiler: &Compiler, tcx: TyCtxt<'tcx>) -> Compilation {
            let root = local_item_type(tcx, "ROOT");
            let cycle = local_item_type(tcx, "NODE");
            let relocation = local_item_type(tcx, "FUNCTION");
            let function_item = local_item_type(tcx, "target");
            let slice = Ty::new_slice(tcx, tcx.types.u8);
            let parameter = Ty::new_param(tcx, 0, Symbol::intern("T"));
            self.results = Some(DriverResults {
                root: extract_general_layout(tcx, root),
                cycle: extract_general_layout(tcx, cycle),
                relocation: extract_general_layout(tcx, relocation),
                function_item: extract_general_layout(tcx, function_item),
                unsized_value: extract_general_layout(tcx, slice),
                unmonomorphized: extract_general_layout(tcx, parameter),
                unit: extract_general_layout(tcx, tcx.types.unit),
                bounded: extract_general_layout_with_limits(
                    tcx,
                    root,
                    ExtractionLimits {
                        max_nodes: 1,
                        ..ExtractionLimits::default()
                    },
                ),
            });
            Compilation::Stop
        }
    }

    fn local_item_type<'tcx>(tcx: TyCtxt<'tcx>, name: &str) -> Ty<'tcx> {
        let definition = tcx
            .iter_local_def_id()
            .find(|definition| {
                matches!(
                    tcx.def_kind(definition.to_def_id()),
                    DefKind::Const { .. } | DefKind::Fn | DefKind::Static { .. }
                ) && tcx.item_name(definition.to_def_id()).as_str() == name
            })
            .unwrap_or_else(|| panic!("missing fixture value `{name}`"));
        tcx.type_of(definition).instantiate_identity()
    }

    struct CompilerFixture {
        source: PathBuf,
        output: PathBuf,
    }

    impl CompilerFixture {
        fn create() -> Self {
            let stem = format!("fe2o3-layout-general-{}", std::process::id());
            let source = std::env::temp_dir().join(format!("{stem}.rs"));
            let output = std::env::temp_dir().join(format!("{stem}.rmeta"));
            fs::write(&source, FIXTURE_SOURCE).expect("write rustc layout fixture");
            Self { source, output }
        }
    }

    impl Drop for CompilerFixture {
        fn drop(&mut self) {
            let _ = fs::remove_file(&self.source);
            let _ = fs::remove_file(&self.output);
        }
    }

    fn compiler_results() -> DriverResults {
        let fixture = CompilerFixture::create();
        let sysroot = Command::new("rustc")
            .args(["--print", "sysroot"])
            .output()
            .expect("query rustc sysroot");
        assert!(sysroot.status.success(), "rustc --print sysroot failed");
        let sysroot = String::from_utf8(sysroot.stdout).expect("UTF-8 rustc sysroot");
        let args = vec![
            "rustc".to_owned(),
            "--crate-name".to_owned(),
            "fe2o3_layout_general_fixture".to_owned(),
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
        let mut callbacks = LayoutCallbacks::default();
        rustc_driver::run_compiler(&args, &mut callbacks);
        callbacks.results.expect("layout callback did not run")
    }

    #[test]
    fn memory_order_is_inverted_into_source_order() {
        assert_eq!(
            memory_order_from_source_indices("root", 4, [2, 0, 3, 1]).unwrap(),
            vec![1, 3, 0, 2]
        );
    }

    #[test]
    fn malformed_memory_orders_fail_closed() {
        for order in [vec![0, 0], vec![0], vec![0, 2]] {
            assert!(matches!(
                memory_order_from_source_indices("root", 2, order),
                Err(GeneralLayoutExtractError::InconsistentLayout { .. })
            ));
        }
    }

    #[test]
    fn field_extent_rejects_overflow_and_out_of_bounds() {
        assert!(validate_field_extent("field", 16, 8, 8).is_ok());
        assert!(matches!(
            validate_field_extent("field", 16, 9, 8),
            Err(GeneralLayoutExtractError::InconsistentLayout { .. })
        ));
        assert!(matches!(
            validate_field_extent("field", u64::MAX, u64::MAX, 1),
            Err(GeneralLayoutExtractError::InconsistentLayout { .. })
        ));
    }

    #[test]
    fn array_extent_requires_exact_stride() {
        assert!(validate_array_extent("array", 24, 8, 3, 4).is_ok());
        assert!(matches!(
            validate_array_extent("array", 24, 2, 3, 4),
            Err(GeneralLayoutExtractError::InconsistentLayout { .. })
        ));
        assert!(matches!(
            validate_array_extent("array", 23, 8, 3, 4),
            Err(GeneralLayoutExtractError::InconsistentLayout { .. })
        ));
        assert!(matches!(
            validate_array_extent("array", u64::MAX, u64::MAX, 2, 1),
            Err(GeneralLayoutExtractError::InconsistentLayout { .. })
        ));
    }

    #[test]
    fn bound_error_is_stable_and_specific() {
        let error = GeneralLayoutExtractError::BoundExceeded {
            path: "root.variant[3]".to_owned(),
            kind: LimitKind::Variants,
            actual: 4,
            limit: 3,
        };
        assert_eq!(
            error.to_string(),
            "layout extraction Variants bound exceeded at root.variant[3]: 4 > 3"
        );
    }

    #[test]
    fn rustc_layouts_cover_nested_aggregates_and_fail_closed() {
        let results = compiler_results();
        let root = results.root.unwrap();
        let TypeLayoutKind::Adt(root_adt) = &root.kind else {
            panic!("root was not an ADT: {:?}", root.kind);
        };
        assert_eq!(root_adt.kind, AdtKind::Struct);
        assert_eq!(root_adt.variants.len(), 1);
        let fields = &root_adt.variants[0].fields;
        assert_eq!(
            fields
                .iter()
                .map(|field| field.name.as_deref().unwrap())
                .collect::<Vec<_>>(),
            ["pair", "values", "tuple", "choice", "bits", "maybe"]
        );
        assert_eq!(
            fields
                .iter()
                .map(|field| field.memory_index)
                .collect::<Vec<_>>(),
            [0, 1, 2, 3, 4, 5]
        );

        let TypeLayoutKind::Adt(pair) = &fields[0].layout.kind else {
            panic!("pair field was not an ADT");
        };
        assert_eq!(pair.kind, AdtKind::Struct);
        assert_eq!(pair.variants[0].fields[0].offset_bytes, 0);
        assert_eq!(pair.variants[0].fields[1].offset_bytes, 4);

        let TypeLayoutKind::Array(values) = &fields[1].layout.kind else {
            panic!("values field was not an array");
        };
        assert_eq!((values.length, values.stride_bytes), (3, 2));
        assert!(matches!(
            values.element.kind,
            TypeLayoutKind::Scalar(SourceScalarKind::UnsignedInteger { bits: 16 })
        ));

        let TypeLayoutKind::Tuple(tuple) = &fields[2].layout.kind else {
            panic!("tuple field was not a tuple");
        };
        assert_eq!(tuple.len(), 2);

        let TypeLayoutKind::Adt(choice) = &fields[3].layout.kind else {
            panic!("choice field was not an ADT");
        };
        assert_eq!(choice.kind, AdtKind::Enum);
        assert_eq!(choice.variants.len(), 3);
        assert!(choice.variants.iter().all(|variant| matches!(
            variant.discriminant_scalar,
            Some(SourceScalarKind::UnsignedInteger { bits: 8 })
        )));
        assert!(matches!(
            choice.tag.unwrap().encoding,
            EnumTagEncodingFacts::Direct
        ));

        let TypeLayoutKind::Adt(bits) = &fields[4].layout.kind else {
            panic!("bits field was not an ADT");
        };
        assert_eq!(bits.kind, AdtKind::Union);
        assert_eq!(bits.variants[0].fields.len(), 2);
        assert!(
            bits.variants[0]
                .fields
                .iter()
                .all(|field| field.offset_bytes == 0)
        );

        let TypeLayoutKind::Adt(maybe) = &fields[5].layout.kind else {
            panic!("maybe field was not an ADT");
        };
        assert!(matches!(
            maybe.tag.unwrap().encoding,
            EnumTagEncodingFacts::Niche { .. }
        ));

        assert!(matches!(
            results.cycle,
            Err(GeneralLayoutExtractError::Cycle { .. })
        ));
        assert!(matches!(
            results.relocation,
            Err(GeneralLayoutExtractError::RelocationsUnsupported {
                kind: RelocationKind::FunctionPointer,
                ..
            })
        ));
        assert!(matches!(
            results.function_item,
            Err(GeneralLayoutExtractError::RelocationsUnsupported {
                kind: RelocationKind::FunctionItem,
                ..
            })
        ));
        assert!(matches!(
            results.unsized_value,
            Err(GeneralLayoutExtractError::Unsized { .. })
        ));
        assert!(matches!(
            results.unmonomorphized,
            Err(GeneralLayoutExtractError::NotFullyMonomorphized { .. })
        ));
        let unit = results.unit.unwrap();
        assert_eq!(unit.size_bytes, 0);
        assert!(matches!(unit.kind, TypeLayoutKind::Tuple(fields) if fields.is_empty()));
        assert!(matches!(
            results.bounded,
            Err(GeneralLayoutExtractError::BoundExceeded {
                kind: LimitKind::Nodes,
                actual: 2,
                limit: 1,
                ..
            })
        ));
    }
}

//! Direct rustc-to-semantic type construction for the production importer.
//!
//! These records remain private staging values. Only admission of the complete
//! semantic MIR request may promote them into the production transaction.

use std::collections::BTreeMap;
use std::fmt;

use fe2o3_mir_model::semantic_mir_v1::{
    SemanticAbiPointeeInfoV1, SemanticAbiPointeeKindV1, SemanticAggregateLayoutV1,
    SemanticAggregateTypeV1, SemanticBackendPrimitiveV1, SemanticBackendReprV1,
    SemanticBackendScalarV1, SemanticDirectEnumEncodingV1, SemanticEnumEncodingV1,
    SemanticEnumLayoutV1, SemanticEnumVariantLayoutV1, SemanticEnumVariantV1, SemanticExternAbiV1,
    SemanticFieldsShapeV1, SemanticFunctionSafetyV1, SemanticLayoutNicheV1, SemanticMirErrorV1,
    SemanticMutabilityV1, SemanticNicheEnumEncodingV1, SemanticNichePathComponentV1,
    SemanticNicheSourceV1, SemanticPointerKindV1, SemanticPointerMetadataV1, SemanticPointerTypeV1,
    SemanticRustTypeKindV1, SemanticRustcVariantsV1, SemanticScalarTypeV1,
    SemanticScalarValidityRangeV1, SemanticTypeAbiPropertiesV1, SemanticTypeDeclV1,
    SemanticTypeIdV1, SemanticTypeIdentityV1, SemanticTypeLayoutDetailsV1, SemanticTypeLayoutV1,
    SemanticTypeShapeV1,
};
use rustc_abi::{
    Align, BackendRepr, ExternAbi, FieldIdx, FieldsShape, HasDataLayout, PointeeInfo, PointerKind,
    Primitive, Scalar, Size, TagEncoding, Variants,
};
use rustc_hir::{Mutability, Safety};
use rustc_middle::ty::layout::{LayoutCx, TyAndLayout};
use rustc_middle::ty::util::IntTypeExt;
use rustc_middle::ty::{
    AdtDef, FloatTy, GenericArgsRef, IntTy, Ty, TyCtxt, TyKind, TypingEnv, UintTy,
};

use crate::rustc_semantic_adapter_v1::rustc_type_identity_v1;
use crate::rustc_semantic_plan_v1::RetainedSemanticTypeProducerV1;

const MAX_RUSTC_LAYOUT_NOUNDEF_NODES_V1: usize = 16_384;

#[derive(Debug)]
pub(crate) enum ProductionSemanticTypeErrorV1 {
    MissingReferencedType {
        parent: SemanticTypeIdentityV1,
        referenced: SemanticTypeIdentityV1,
    },
    Normalization {
        parent: SemanticTypeIdentityV1,
    },
    Unsupported {
        identity: SemanticTypeIdentityV1,
        construct: &'static str,
    },
    Schema {
        identity: SemanticTypeIdentityV1,
        error: SemanticMirErrorV1,
    },
    Allocation {
        resource: &'static str,
    },
    Cardinality,
}

impl fmt::Display for ProductionSemanticTypeErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingReferencedType { parent, referenced } => write!(
                formatter,
                "type producer {} references absent type producer {}",
                crate::encode_hex(parent.as_bytes()),
                crate::encode_hex(referenced.as_bytes()),
            ),
            Self::Normalization { parent } => write!(
                formatter,
                "type producer {} failed child normalization",
                crate::encode_hex(parent.as_bytes()),
            ),
            Self::Unsupported {
                identity,
                construct,
            } => write!(
                formatter,
                "type producer {} uses unsupported production semantic construct {construct}",
                crate::encode_hex(identity.as_bytes()),
            ),
            Self::Schema { identity, error } => write!(
                formatter,
                "type producer {} failed semantic schema construction: {error}",
                crate::encode_hex(identity.as_bytes()),
            ),
            Self::Allocation { resource } => {
                write!(
                    formatter,
                    "semantic type construction could not allocate {resource}"
                )
            }
            Self::Cardinality => formatter.write_str("semantic type producer cardinality overflow"),
        }
    }
}

impl std::error::Error for ProductionSemanticTypeErrorV1 {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Schema { error, .. } => Some(error),
            Self::MissingReferencedType { .. }
            | Self::Normalization { .. }
            | Self::Unsupported { .. }
            | Self::Allocation { .. }
            | Self::Cardinality => None,
        }
    }
}

#[derive(Debug)]
pub(crate) struct ConstructedSemanticTypesV1 {
    records: Box<[SemanticTypeDeclV1]>,
}

impl ConstructedSemanticTypesV1 {
    pub(crate) fn into_records(self) -> Vec<SemanticTypeDeclV1> {
        self.records.into_vec()
    }
}

pub(crate) fn construct_production_semantic_types_v1<'tcx>(
    tcx: TyCtxt<'tcx>,
    producers: &[RetainedSemanticTypeProducerV1<'tcx>],
) -> Result<ConstructedSemanticTypesV1, ProductionSemanticTypeErrorV1> {
    let mut ids = BTreeMap::new();
    for (index, producer) in producers.iter().enumerate() {
        let index = u32::try_from(index).map_err(|_| ProductionSemanticTypeErrorV1::Cardinality)?;
        ids.insert(producer.identity, SemanticTypeIdV1::from_index(index));
    }

    let layout_cx = LayoutCx::new(tcx, TypingEnv::fully_monomorphized());
    let mut records = Vec::new();
    records.try_reserve_exact(producers.len()).map_err(|_| {
        ProductionSemanticTypeErrorV1::Allocation {
            resource: "semantic type records",
        }
    })?;
    for producer in producers {
        let context = TypeConstructionContextV1 {
            tcx,
            layout_cx: &layout_cx,
            ids: &ids,
            parent: producer.identity,
        };
        records.push(construct_type_v1(&context, producer)?);
    }
    Ok(ConstructedSemanticTypesV1 {
        records: records.into_boxed_slice(),
    })
}

struct TypeConstructionContextV1<'a, 'tcx> {
    tcx: TyCtxt<'tcx>,
    layout_cx: &'a LayoutCx<'tcx>,
    ids: &'a BTreeMap<SemanticTypeIdentityV1, SemanticTypeIdV1>,
    parent: SemanticTypeIdentityV1,
}

impl<'tcx> TypeConstructionContextV1<'_, 'tcx> {
    fn type_id(&self, ty: Ty<'tcx>) -> Result<SemanticTypeIdV1, ProductionSemanticTypeErrorV1> {
        let ty = self
            .tcx
            .try_normalize_erasing_regions(TypingEnv::fully_monomorphized(), ty)
            .map_err(|_| ProductionSemanticTypeErrorV1::Normalization {
                parent: self.parent,
            })?;
        let identity = rustc_type_identity_v1(self.tcx, ty);
        self.ids.get(&identity).copied().ok_or(
            ProductionSemanticTypeErrorV1::MissingReferencedType {
                parent: self.parent,
                referenced: identity,
            },
        )
    }

    fn aggregate(
        &self,
        types: impl IntoIterator<Item = Ty<'tcx>>,
    ) -> Result<SemanticAggregateTypeV1, ProductionSemanticTypeErrorV1> {
        let types = types.into_iter();
        let (minimum, maximum) = types.size_hint();
        let mut fields = Vec::new();
        fields
            .try_reserve(maximum.unwrap_or(minimum))
            .map_err(|_| ProductionSemanticTypeErrorV1::Allocation {
                resource: "semantic aggregate fields",
            })?;
        for ty in types {
            fields.push(self.type_id(ty)?);
        }
        SemanticAggregateTypeV1::new(fields).map_err(|error| self.schema(error))
    }

    const fn schema(&self, error: SemanticMirErrorV1) -> ProductionSemanticTypeErrorV1 {
        ProductionSemanticTypeErrorV1::Schema {
            identity: self.parent,
            error,
        }
    }

    const fn unsupported(&self, construct: &'static str) -> ProductionSemanticTypeErrorV1 {
        ProductionSemanticTypeErrorV1::Unsupported {
            identity: self.parent,
            construct,
        }
    }
}

fn construct_type_v1<'tcx>(
    context: &TypeConstructionContextV1<'_, 'tcx>,
    producer: &RetainedSemanticTypeProducerV1<'tcx>,
) -> Result<SemanticTypeDeclV1, ProductionSemanticTypeErrorV1> {
    let rust_type_kind = if matches!(producer.ty.kind(), TyKind::Str) {
        SemanticRustTypeKindV1::Str
    } else {
        SemanticRustTypeKindV1::Ordinary
    };
    let shape = match producer.ty.kind() {
        TyKind::Bool => SemanticTypeShapeV1::Scalar(SemanticScalarTypeV1::Bool),
        TyKind::Char => SemanticTypeShapeV1::Scalar(SemanticScalarTypeV1::Char),
        TyKind::Int(width) => SemanticTypeShapeV1::Scalar(SemanticScalarTypeV1::Integer {
            signed: true,
            bits: integer_bits_v1(context.layout_cx, *width),
        }),
        TyKind::Uint(width) => SemanticTypeShapeV1::Scalar(SemanticScalarTypeV1::Integer {
            signed: false,
            bits: unsigned_integer_bits_v1(context.layout_cx, *width),
        }),
        TyKind::Float(width) => SemanticTypeShapeV1::Scalar(SemanticScalarTypeV1::Float {
            bits: float_bits_v1(*width),
        }),
        TyKind::Never => SemanticTypeShapeV1::Never,
        TyKind::Tuple(fields) if fields.is_empty() => SemanticTypeShapeV1::Unit,
        TyKind::Tuple(fields) => SemanticTypeShapeV1::Tuple(context.aggregate(fields.iter())?),
        TyKind::Adt(definition, arguments) if definition.is_struct() => {
            if definition.repr().pack.is_some() {
                return Err(context.unsupported("packed struct layout"));
            }
            if matches!(producer.layout.backend_repr, BackendRepr::SimdVector { .. })
                || matches!(
                    producer.layout.backend_repr,
                    BackendRepr::SimdScalableVector { .. }
                )
                || !matches!(producer.layout.fields, FieldsShape::Arbitrary { .. })
            {
                return Err(context.unsupported("non-aggregate struct field layout"));
            }
            let variant = definition.non_enum_variant();
            SemanticTypeShapeV1::Aggregate(
                context.aggregate(
                    variant
                        .fields
                        .iter()
                        .map(|field| field.ty(context.tcx, arguments)),
                )?,
            )
        }
        TyKind::Adt(definition, arguments) if definition.is_union() => {
            if definition.repr().pack.is_some() {
                return Err(context.unsupported("packed union layout"));
            }
            let variant = definition.non_enum_variant();
            SemanticTypeShapeV1::Union(
                context.aggregate(
                    variant
                        .fields
                        .iter()
                        .map(|field| field.ty(context.tcx, arguments)),
                )?,
            )
        }
        TyKind::Adt(definition, arguments) if definition.is_enum() => construct_enum_shape_v1(
            context,
            producer.ty,
            *definition,
            arguments,
            producer.layout,
        )?,
        TyKind::Adt(..) => return Err(context.unsupported("unknown ADT kind")),
        TyKind::Ref(_, pointee, mutability) => pointer_shape_v1(
            context,
            *pointee,
            *mutability,
            SemanticPointerKindV1::Reference,
            producer.layout.backend_repr,
        )?,
        TyKind::RawPtr(pointee, mutability) => pointer_shape_v1(
            context,
            *pointee,
            *mutability,
            SemanticPointerKindV1::Raw,
            producer.layout.backend_repr,
        )?,
        TyKind::Array(element, length) => SemanticTypeShapeV1::Array {
            element: context.type_id(*element)?,
            length: length
                .try_to_target_usize(context.tcx)
                .ok_or_else(|| context.unsupported("non-value array length"))?,
        },
        TyKind::Slice(element) => SemanticTypeShapeV1::Slice {
            element: context.type_id(*element)?,
        },
        TyKind::Str => SemanticTypeShapeV1::Opaque,
        TyKind::FnDef(..) => SemanticTypeShapeV1::Opaque,
        TyKind::FnPtr(signature, header) => {
            if header.abi != ExternAbi::Rust || header.c_variadic {
                return Err(context.unsupported("non-Rust function pointer ABI"));
            }
            let mut signature_types = signature
                .skip_binder()
                .inputs_and_output
                .iter()
                .map(|ty| context.type_id(ty))
                .collect::<Result<Vec<_>, _>>()?;
            let return_type = signature_types
                .pop()
                .ok_or_else(|| context.unsupported("function pointer without return type"))?;
            SemanticTypeShapeV1::FunctionPointer {
                safety: match header.safety {
                    Safety::Safe => SemanticFunctionSafetyV1::Safe,
                    Safety::Unsafe => SemanticFunctionSafetyV1::Unsafe,
                },
                extern_abi: SemanticExternAbiV1::Rust,
                c_variadic: false,
                arguments: SemanticAggregateTypeV1::new(signature_types)
                    .map_err(|error| context.schema(error))?,
                return_type,
            }
        }
        TyKind::Pat(..)
        | TyKind::Foreign(..)
        | TyKind::UnsafeBinder(..)
        | TyKind::Dynamic(..)
        | TyKind::Closure(..)
        | TyKind::CoroutineClosure(..)
        | TyKind::Coroutine(..)
        | TyKind::CoroutineWitness(..)
        | TyKind::Alias(..)
        | TyKind::Param(..)
        | TyKind::Bound(..)
        | TyKind::Placeholder(..)
        | TyKind::Infer(..)
        | TyKind::Error(..) => return Err(context.unsupported("preflight-rejected type")),
    };
    let layout = construct_layout_v1(context, producer, &shape, rust_type_kind)?;
    let abi_properties = construct_abi_properties_v1(context, producer, &shape)?;
    Ok(SemanticTypeDeclV1::new(
        producer.identity,
        producer.semantic_layout_identity,
        layout,
        shape,
    )
    .with_rustc_abi_properties(abi_properties)
    .with_rust_type_kind(rust_type_kind))
}

fn construct_abi_properties_v1<'tcx>(
    context: &TypeConstructionContextV1<'_, 'tcx>,
    producer: &RetainedSemanticTypeProducerV1<'tcx>,
    shape: &SemanticTypeShapeV1,
) -> Result<SemanticTypeAbiPropertiesV1, ProductionSemanticTypeErrorV1> {
    let layout = producer.layout;
    let pass_indirectly_in_non_rustic_abis =
        layout.pass_indirectly_in_non_rustic_abis(context.layout_cx);
    let has_unsized_foreign_tail = layout.is_unsized()
        && matches!(
            context
                .tcx
                .struct_tail_for_codegen(producer.ty, TypingEnv::fully_monomorphized())
                .kind(),
            TyKind::Foreign(..)
        );

    // Function-pointer pointee facts are implicit in the semantic schema.
    let (first_pointee, second_pointee) =
        if matches!(shape, SemanticTypeShapeV1::FunctionPointer { .. }) {
            (None, None)
        } else {
            scalar_pointee_info_v1(context, layout)?
        };

    Ok(SemanticTypeAbiPropertiesV1::new(
        pass_indirectly_in_non_rustic_abis,
        has_unsized_foreign_tail,
    )
    .with_rustc_layout_is_noundef(rustc_layout_is_noundef_v1(context.layout_cx, layout))
    .with_scalar_pointee_info(first_pointee, second_pointee))
}

fn scalar_pointee_info_v1<'tcx>(
    context: &TypeConstructionContextV1<'_, 'tcx>,
    layout: TyAndLayout<'tcx>,
) -> Result<
    (
        Option<SemanticAbiPointeeInfoV1>,
        Option<SemanticAbiPointeeInfoV1>,
    ),
    ProductionSemanticTypeErrorV1,
> {
    let max_reliable_alignment = context.tcx.sess.target.max_reliable_alignment();
    match layout.backend_repr {
        BackendRepr::Scalar(first) => Ok((
            scalar_pointee_at_v1(
                context.layout_cx,
                layout,
                first,
                Size::ZERO,
                max_reliable_alignment,
            )
            .map_err(|error| context.schema(error))?,
            None,
        )),
        BackendRepr::ScalarPair(first, second) => {
            let second_offset = first
                .size(context.layout_cx)
                .align_to(second.align(context.layout_cx).abi);
            Ok((
                scalar_pointee_at_v1(
                    context.layout_cx,
                    layout,
                    first,
                    Size::ZERO,
                    max_reliable_alignment,
                )
                .map_err(|error| context.schema(error))?,
                scalar_pointee_at_v1(
                    context.layout_cx,
                    layout,
                    second,
                    second_offset,
                    max_reliable_alignment,
                )
                .map_err(|error| context.schema(error))?,
            ))
        }
        BackendRepr::Memory { .. }
        | BackendRepr::SimdVector { .. }
        | BackendRepr::SimdScalableVector { .. } => Ok((None, None)),
    }
}

fn scalar_pointee_at_v1<'tcx>(
    layout_cx: &LayoutCx<'tcx>,
    layout: TyAndLayout<'tcx>,
    scalar: Scalar,
    offset: Size,
    max_reliable_alignment: Align,
) -> Result<Option<SemanticAbiPointeeInfoV1>, SemanticMirErrorV1> {
    if !matches!(
        scalar,
        Scalar::Initialized {
            value: Primitive::Pointer(_),
            ..
        }
    ) {
        return Ok(None);
    }
    layout
        .pointee_info_at(layout_cx, offset)
        .map(|pointee| semantic_pointee_info_v1(pointee, max_reliable_alignment))
        .transpose()
}

fn semantic_pointee_info_v1(
    pointee: PointeeInfo,
    max_reliable_alignment: Align,
) -> Result<SemanticAbiPointeeInfoV1, SemanticMirErrorV1> {
    let kind = match pointee.safe {
        None => SemanticAbiPointeeKindV1::Raw,
        Some(PointerKind::SharedRef { frozen }) => {
            SemanticAbiPointeeKindV1::SharedReference { frozen }
        }
        Some(PointerKind::MutableRef { unpin }) => {
            SemanticAbiPointeeKindV1::MutableReference { unpin }
        }
        Some(PointerKind::Box { unpin, global }) => SemanticAbiPointeeKindV1::Box { unpin, global },
    };
    SemanticAbiPointeeInfoV1::new(
        kind,
        pointee.size.bytes(),
        pointee.align.min(max_reliable_alignment).bytes(),
    )
}

// This is the pinned rustc_target::callconv::layout_is_noundef algorithm,
// evaluated iteratively. Exhausting either work or allocation fails closed.
fn rustc_layout_is_noundef_v1<'tcx>(layout_cx: &LayoutCx<'tcx>, root: TyAndLayout<'tcx>) -> bool {
    let mut pending = Vec::new();
    if pending.try_reserve_exact(1).is_err() {
        return false;
    }
    pending.push(root);
    let mut remaining = MAX_RUSTC_LAYOUT_NOUNDEF_NODES_V1;

    while let Some(layout) = pending.pop() {
        let Some(next_remaining) = remaining.checked_sub(1) else {
            return false;
        };
        remaining = next_remaining;
        match layout.backend_repr {
            BackendRepr::Scalar(scalar) => {
                if scalar.is_uninit_valid() {
                    return false;
                }
            }
            BackendRepr::ScalarPair(first, second) => {
                if !scalar_pair_is_noundef_v1(
                    !first.is_uninit_valid(),
                    !second.is_uninit_valid(),
                    first.size(layout_cx).bytes(),
                    second.size(layout_cx).bytes(),
                    layout.size.bytes(),
                ) {
                    return false;
                }
            }
            BackendRepr::Memory { .. } => match layout.fields {
                FieldsShape::Primitive | FieldsShape::Union(_) => return false,
                FieldsShape::Array { count: 0, .. } => {}
                FieldsShape::Array { .. } => {
                    if !push_noundef_layout_v1(&mut pending, layout.field(layout_cx, 0), remaining)
                    {
                        return false;
                    }
                }
                FieldsShape::Arbitrary { .. } => {
                    if !matches!(layout.variants, Variants::Single { .. }) {
                        return false;
                    }
                    let mut cursor = 0;
                    for index in layout.fields.index_by_increasing_offset() {
                        let field = layout.field(layout_cx, index);
                        let Some(next_cursor) = advance_noundef_cursor_v1(
                            cursor,
                            layout.fields.offset(index).bytes(),
                            field.size.bytes(),
                        ) else {
                            return false;
                        };
                        if field.size != Size::ZERO
                            && !push_noundef_layout_v1(&mut pending, field, remaining)
                        {
                            return false;
                        }
                        cursor = next_cursor;
                    }
                    if cursor != layout.size.bytes() {
                        return false;
                    }
                }
            },
            BackendRepr::SimdVector { .. } | BackendRepr::SimdScalableVector { .. } => {
                return false;
            }
        }
    }
    true
}

fn push_noundef_layout_v1<'tcx>(
    pending: &mut Vec<TyAndLayout<'tcx>>,
    layout: TyAndLayout<'tcx>,
    remaining: usize,
) -> bool {
    if pending.len() >= remaining || pending.try_reserve(1).is_err() {
        return false;
    }
    pending.push(layout);
    true
}

const fn scalar_pair_is_noundef_v1(
    first_initialized: bool,
    second_initialized: bool,
    first_size_bytes: u64,
    second_size_bytes: u64,
    layout_size_bytes: u64,
) -> bool {
    first_initialized
        && second_initialized
        && matches!(
            first_size_bytes.checked_add(second_size_bytes),
            Some(size) if size == layout_size_bytes
        )
}

const fn advance_noundef_cursor_v1(cursor: u64, field_offset: u64, field_size: u64) -> Option<u64> {
    if field_size == 0 {
        Some(cursor)
    } else if field_offset != cursor {
        None
    } else {
        cursor.checked_add(field_size)
    }
}

fn construct_enum_shape_v1<'tcx>(
    context: &TypeConstructionContextV1<'_, 'tcx>,
    ty: Ty<'tcx>,
    definition: AdtDef<'tcx>,
    arguments: GenericArgsRef<'tcx>,
    layout: TyAndLayout<'tcx>,
) -> Result<SemanticTypeShapeV1, ProductionSemanticTypeErrorV1> {
    let discriminant_ty = definition.repr().discr_type().to_ty(context.tcx);
    let discriminant = context.type_id(discriminant_ty)?;
    let mut variants = Vec::new();
    variants
        .try_reserve_exact(definition.variants().len())
        .map_err(|_| context.unsupported("enum variant allocation"))?;
    for (variant_index, variant) in definition.variants().iter_enumerated() {
        let discriminant_value = ty
            .discriminant_for_variant(context.tcx, variant_index)
            .ok_or_else(|| context.unsupported("enum variant without discriminant"))?;
        let fields = context.aggregate(
            variant
                .fields
                .iter()
                .map(|field| field.ty(context.tcx, arguments)),
        )?;
        variants.push(SemanticEnumVariantV1::new_with_inhabitedness(
            discriminant_value.val,
            fields,
            layout
                .for_variant(context.layout_cx, variant_index)
                .is_uninhabited(),
        ));
    }
    SemanticTypeShapeV1::enum_type(discriminant, variants).map_err(|error| context.schema(error))
}

fn pointer_shape_v1<'tcx>(
    context: &TypeConstructionContextV1<'_, 'tcx>,
    pointee: Ty<'tcx>,
    mutability: Mutability,
    kind: SemanticPointerKindV1,
    backend_repr: BackendRepr,
) -> Result<SemanticTypeShapeV1, ProductionSemanticTypeErrorV1> {
    let (first, metadata) = match backend_repr {
        BackendRepr::Scalar(first) => {
            if matches!(pointee.kind(), TyKind::Slice(_) | TyKind::Str) {
                return Err(context.unsupported("slice/string pointer without length metadata"));
            }
            (first, SemanticPointerMetadataV1::None)
        }
        BackendRepr::ScalarPair(first, _)
            if matches!(pointee.kind(), TyKind::Slice(_) | TyKind::Str) =>
        {
            (first, SemanticPointerMetadataV1::SliceLength)
        }
        BackendRepr::ScalarPair(..) => {
            return Err(context.unsupported("fat pointer to non-slice unsized tail"));
        }
        BackendRepr::Memory { .. }
        | BackendRepr::SimdVector { .. }
        | BackendRepr::SimdScalableVector { .. } => {
            return Err(context.unsupported("pointer without scalar backend representation"));
        }
    };
    let Primitive::Pointer(address_space) = first.primitive() else {
        return Err(context.unsupported("pointer with non-pointer data scalar"));
    };
    let pointer_width_bits = u16::try_from(first.size(context.layout_cx).bits())
        .map_err(|_| context.unsupported("pointer width outside u16"))?;
    let pointer = SemanticPointerTypeV1::new_with_kind(
        context.type_id(pointee)?,
        kind,
        match mutability {
            Mutability::Not => SemanticMutabilityV1::Immutable,
            Mutability::Mut => SemanticMutabilityV1::Mutable,
        },
        address_space.0,
        pointer_width_bits,
        metadata,
    )
    .map_err(|error| context.schema(error))?;
    Ok(SemanticTypeShapeV1::Pointer(pointer))
}

fn construct_layout_v1<'tcx>(
    context: &TypeConstructionContextV1<'_, 'tcx>,
    producer: &RetainedSemanticTypeProducerV1<'tcx>,
    shape: &SemanticTypeShapeV1,
    rust_type_kind: SemanticRustTypeKindV1,
) -> Result<SemanticTypeLayoutV1, ProductionSemanticTypeErrorV1> {
    let layout = producer.layout;
    let fields = fields_shape_v1(context, &layout.fields)?;
    let variants = rustc_variants_v1(context, producer.ty, layout)?;
    let backend_repr = backend_repr_v1(context, layout.backend_repr)?;
    let largest_niche = layout
        .largest_niche
        .map(|niche| {
            SemanticLayoutNicheV1::new(
                niche.offset.bytes(),
                primitive_v1(context, niche.value),
                SemanticScalarValidityRangeV1::new(niche.valid_range.start, niche.valid_range.end),
            )
            .map_err(|error| context.schema(error))
        })
        .transpose()?;
    let details = match shape {
        SemanticTypeShapeV1::Tuple(_) | SemanticTypeShapeV1::Aggregate(_) => {
            let offsets = (0..layout.fields.count())
                .map(|index| layout.fields.offset(index).bytes())
                .collect();
            SemanticTypeLayoutDetailsV1::Aggregate(
                SemanticAggregateLayoutV1::new(offsets, Vec::new())
                    .map_err(|error| context.schema(error))?,
            )
        }
        SemanticTypeShapeV1::Enum { .. }
            if matches!(&variants, SemanticRustcVariantsV1::Single { .. }) =>
        {
            let offsets = (0..layout.fields.count())
                .map(|index| layout.fields.offset(index).bytes())
                .collect();
            SemanticTypeLayoutDetailsV1::Aggregate(
                SemanticAggregateLayoutV1::new(offsets, Vec::new())
                    .map_err(|error| context.schema(error))?,
            )
        }
        SemanticTypeShapeV1::Unit
        | SemanticTypeShapeV1::Never
        | SemanticTypeShapeV1::Scalar(_)
        | SemanticTypeShapeV1::ValidityScalar(_)
        | SemanticTypeShapeV1::Pointer(_)
        | SemanticTypeShapeV1::Array { .. }
        | SemanticTypeShapeV1::Slice { .. }
        | SemanticTypeShapeV1::Union(_)
        | SemanticTypeShapeV1::Enum { .. }
        | SemanticTypeShapeV1::FunctionPointer { .. }
        | SemanticTypeShapeV1::Opaque => SemanticTypeLayoutDetailsV1::None,
    };
    SemanticTypeLayoutV1::with_exact_rustc_layout(
        layout.size.bytes(),
        layout.align.abi.bytes(),
        fields,
        variants,
        backend_repr,
        largest_niche,
        layout.is_uninhabited(),
        layout.max_repr_align.map(|alignment| alignment.bytes()),
        layout.unadjusted_abi_align.bytes(),
        if matches!(shape, SemanticTypeShapeV1::Slice { .. })
            || rust_type_kind == SemanticRustTypeKindV1::Str
        {
            0
        } else {
            layout.randomization_seed.as_u64()
        },
        details,
    )
    .map_err(|error| context.schema(error))
}

fn rustc_variants_v1<'tcx>(
    context: &TypeConstructionContextV1<'_, 'tcx>,
    ty: Ty<'tcx>,
    layout: TyAndLayout<'tcx>,
) -> Result<SemanticRustcVariantsV1, ProductionSemanticTypeErrorV1> {
    match &layout.variants {
        Variants::Empty => Ok(SemanticRustcVariantsV1::Empty),
        Variants::Single { index } => Ok(SemanticRustcVariantsV1::Single {
            index: index.as_u32(),
        }),
        Variants::Multiple {
            tag,
            tag_encoding,
            tag_field,
            ..
        } => {
            let TyKind::Adt(definition, arguments) = ty.kind() else {
                return Err(context.unsupported("multi-variant non-enum type"));
            };
            if !definition.is_enum() {
                return Err(context.unsupported("multi-variant non-enum ADT"));
            }
            let mut variants = Vec::new();
            variants
                .try_reserve_exact(definition.variants().len())
                .map_err(|_| context.unsupported("enum layout allocation"))?;
            for (index, variant) in definition.variants().iter_enumerated() {
                let variant_layout = layout.for_variant(context.layout_cx, index);
                if variant_layout.fields.count() != variant.fields.len() {
                    return Err(context.unsupported("enum variant field/layout mismatch"));
                }
                variants.push(enum_variant_layout_v1(
                    context,
                    index.as_u32(),
                    variant_layout,
                )?);
            }

            let tag_field_index = tag_field.as_usize();
            if tag_field_index >= layout.fields.count() {
                return Err(context.unsupported("enum tag field outside outer layout"));
            }
            let tag_offset = layout.fields.offset(tag_field_index).bytes();
            let semantic_tag = backend_scalar_v1(context, *tag);
            let encoding = match tag_encoding {
                TagEncoding::Direct => SemanticEnumEncodingV1::Direct(
                    SemanticDirectEnumEncodingV1::new(tag_field.as_u32(), tag_offset, semantic_tag),
                ),
                TagEncoding::Niche {
                    untagged_variant,
                    niche_variants,
                    niche_start,
                } => {
                    let untagged_layout = layout.for_variant(context.layout_cx, *untagged_variant);
                    let source_niche = semantic_layout_niche_v1(
                        context,
                        untagged_layout.largest_niche.ok_or_else(|| {
                            context.unsupported("niche enum without source niche")
                        })?,
                    )?;
                    let source = find_unique_niche_source_v1(
                        context,
                        ty,
                        *untagged_variant,
                        untagged_layout,
                        tag_offset,
                        source_niche,
                        arguments,
                    )?;
                    SemanticEnumEncodingV1::Niche(
                        SemanticNicheEnumEncodingV1::new(
                            tag_field.as_u32(),
                            source,
                            source_niche,
                            semantic_tag,
                            untagged_variant.as_u32(),
                            niche_variants.start().as_u32(),
                            niche_variants.end().as_u32(),
                            *niche_start,
                        )
                        .map_err(|error| context.schema(error))?,
                    )
                }
            };
            Ok(SemanticRustcVariantsV1::Multiple(Box::new(
                SemanticEnumLayoutV1::new(variants, encoding)
                    .map_err(|error| context.schema(error))?,
            )))
        }
    }
}

fn enum_variant_layout_v1(
    context: &TypeConstructionContextV1<'_, '_>,
    variant_index: u32,
    layout: TyAndLayout<'_>,
) -> Result<SemanticEnumVariantLayoutV1, ProductionSemanticTypeErrorV1> {
    let fields = fields_shape_v1(context, &layout.fields)?;
    let aggregate = SemanticAggregateLayoutV1::new(
        (0..layout.fields.count())
            .map(|index| layout.fields.offset(index).bytes())
            .collect(),
        Vec::new(),
    )
    .map_err(|error| context.schema(error))?;
    let largest_niche = layout
        .largest_niche
        .map(|niche| semantic_layout_niche_v1(context, niche))
        .transpose()?;
    SemanticEnumVariantLayoutV1::from_rustc(
        variant_index,
        layout.size.bytes(),
        layout.align.abi.bytes(),
        fields,
        backend_repr_v1(context, layout.backend_repr)?,
        largest_niche,
        layout.is_uninhabited(),
        layout.max_repr_align.map(|alignment| alignment.bytes()),
        layout.unadjusted_abi_align.bytes(),
        layout.randomization_seed.as_u64(),
        aggregate,
    )
    .map_err(|error| context.schema(error))
}

fn semantic_layout_niche_v1(
    context: &TypeConstructionContextV1<'_, '_>,
    niche: rustc_abi::Niche,
) -> Result<SemanticLayoutNicheV1, ProductionSemanticTypeErrorV1> {
    SemanticLayoutNicheV1::new(
        niche.offset.bytes(),
        primitive_v1(context, niche.value),
        SemanticScalarValidityRangeV1::new(niche.valid_range.start, niche.valid_range.end),
    )
    .map_err(|error| context.schema(error))
}

fn find_unique_niche_source_v1<'tcx>(
    context: &TypeConstructionContextV1<'_, 'tcx>,
    enum_ty: Ty<'tcx>,
    variant_index: rustc_abi::VariantIdx,
    variant_layout: TyAndLayout<'tcx>,
    expected_offset: u64,
    source_niche: SemanticLayoutNicheV1,
    arguments: GenericArgsRef<'tcx>,
) -> Result<SemanticNicheSourceV1, ProductionSemanticTypeErrorV1> {
    let TyKind::Adt(definition, _) = enum_ty.kind() else {
        return Err(context.unsupported("niche source on non-enum"));
    };
    let variant = &definition.variants()[variant_index];
    let mut found = Vec::new();
    for (field_index, field) in variant.fields.iter().enumerate() {
        let child = variant_layout.field(context.layout_cx, field_index);
        let mut path = vec![SemanticNichePathComponentV1::Field(
            u32::try_from(field_index).map_err(|_| ProductionSemanticTypeErrorV1::Cardinality)?,
        )];
        find_niche_in_type_v1(
            context,
            field.ty(context.tcx, arguments),
            child,
            variant_layout.fields.offset(field_index).bytes(),
            expected_offset,
            source_niche,
            &mut path,
            &mut found,
            0,
        )?;
    }
    if found.len() != 1 {
        return Err(context.unsupported("niche source is not unique"));
    }
    SemanticNicheSourceV1::new(found.pop().expect("one niche path"), expected_offset)
        .map_err(|error| context.schema(error))
}

#[allow(clippy::too_many_arguments)]
fn find_niche_in_type_v1<'tcx>(
    context: &TypeConstructionContextV1<'_, 'tcx>,
    ty: Ty<'tcx>,
    layout: TyAndLayout<'tcx>,
    base_offset: u64,
    expected_offset: u64,
    source_niche: SemanticLayoutNicheV1,
    path: &mut Vec<SemanticNichePathComponentV1>,
    found: &mut Vec<Vec<SemanticNichePathComponentV1>>,
    depth: usize,
) -> Result<(), ProductionSemanticTypeErrorV1> {
    if found.len() >= 2 {
        return Ok(());
    }
    if depth >= 64 {
        return Err(context.unsupported("niche source depth"));
    }
    if layout.largest_niche.is_some_and(|niche| {
        base_offset.checked_add(niche.offset.bytes()) == Some(expected_offset)
            && primitive_v1(context, niche.value) == source_niche.primitive()
            && SemanticScalarValidityRangeV1::new(niche.valid_range.start, niche.valid_range.end)
                == source_niche.valid_range()
            && (matches!(
                layout.backend_repr,
                BackendRepr::Scalar(_) | BackendRepr::ScalarPair(..)
            ) || matches!(ty.kind(), TyKind::Adt(definition, _) if definition.is_enum()))
    }) {
        found
            .try_reserve(1)
            .map_err(|_| ProductionSemanticTypeErrorV1::Allocation {
                resource: "niche source candidates",
            })?;
        found.push(path.clone());
        return Ok(());
    }
    match ty.kind() {
        TyKind::Tuple(fields) => {
            for (field_index, field_ty) in fields.iter().enumerate() {
                descend_niche_field_v1(
                    context,
                    field_ty,
                    layout,
                    field_index,
                    base_offset,
                    expected_offset,
                    source_niche,
                    path,
                    found,
                    depth,
                )?;
            }
        }
        TyKind::Adt(definition, arguments) if definition.is_struct() => {
            for (field_index, field) in definition.non_enum_variant().fields.iter().enumerate() {
                descend_niche_field_v1(
                    context,
                    field.ty(context.tcx, arguments),
                    layout,
                    field_index,
                    base_offset,
                    expected_offset,
                    source_niche,
                    path,
                    found,
                    depth,
                )?;
            }
        }
        TyKind::Array(element, length) => {
            let FieldsShape::Array { stride, count } = &layout.fields else {
                return Err(context.unsupported("array niche with non-array layout"));
            };
            let source_count = length
                .try_to_target_usize(context.tcx)
                .ok_or_else(|| context.unsupported("array niche length"))?;
            if *count != source_count || *count == 0 || stride.bytes() == 0 {
                return Ok(());
            }
            let Some(relative) = expected_offset.checked_sub(base_offset) else {
                return Ok(());
            };
            let index = relative / stride.bytes();
            if index >= *count {
                return Ok(());
            }
            let index_usize =
                usize::try_from(index).map_err(|_| ProductionSemanticTypeErrorV1::Cardinality)?;
            let child_offset = index
                .checked_mul(stride.bytes())
                .ok_or_else(|| context.unsupported("array niche offset overflow"))?;
            let child_base = base_offset
                .checked_add(child_offset)
                .ok_or_else(|| context.unsupported("array niche offset overflow"))?;
            path.push(SemanticNichePathComponentV1::ArrayElement(index));
            find_niche_in_type_v1(
                context,
                *element,
                layout.field(context.layout_cx, index_usize),
                child_base,
                expected_offset,
                source_niche,
                path,
                found,
                depth + 1,
            )?;
            path.pop();
        }
        _ => {}
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn descend_niche_field_v1<'tcx>(
    context: &TypeConstructionContextV1<'_, 'tcx>,
    field_ty: Ty<'tcx>,
    layout: TyAndLayout<'tcx>,
    field_index: usize,
    base_offset: u64,
    expected_offset: u64,
    source_niche: SemanticLayoutNicheV1,
    path: &mut Vec<SemanticNichePathComponentV1>,
    found: &mut Vec<Vec<SemanticNichePathComponentV1>>,
    depth: usize,
) -> Result<(), ProductionSemanticTypeErrorV1> {
    let child_base = base_offset
        .checked_add(layout.fields.offset(field_index).bytes())
        .ok_or_else(|| context.unsupported("niche field offset overflow"))?;
    path.push(SemanticNichePathComponentV1::Field(
        u32::try_from(field_index).map_err(|_| ProductionSemanticTypeErrorV1::Cardinality)?,
    ));
    find_niche_in_type_v1(
        context,
        field_ty,
        layout.field(context.layout_cx, field_index),
        child_base,
        expected_offset,
        source_niche,
        path,
        found,
        depth + 1,
    )?;
    path.pop();
    Ok(())
}

fn fields_shape_v1(
    context: &TypeConstructionContextV1<'_, '_>,
    fields: &FieldsShape<FieldIdx>,
) -> Result<SemanticFieldsShapeV1, ProductionSemanticTypeErrorV1> {
    match fields {
        FieldsShape::Primitive => Ok(SemanticFieldsShapeV1::Primitive),
        FieldsShape::Union(_) => SemanticFieldsShapeV1::union(fields.count() as u64)
            .map_err(|error| context.schema(error)),
        FieldsShape::Array { stride, count } => {
            Ok(SemanticFieldsShapeV1::array(stride.bytes(), *count))
        }
        FieldsShape::Arbitrary { .. } => {
            let offsets = (0..fields.count())
                .map(|index| fields.offset(index).bytes())
                .collect();
            let memory_order = fields
                .index_by_increasing_offset()
                .map(|index| {
                    u32::try_from(index).map_err(|_| ProductionSemanticTypeErrorV1::Cardinality)
                })
                .collect::<Result<Vec<_>, _>>()?;
            SemanticFieldsShapeV1::arbitrary(offsets, memory_order)
                .map_err(|error| context.schema(error))
        }
    }
}

fn backend_repr_v1(
    context: &TypeConstructionContextV1<'_, '_>,
    backend: BackendRepr,
) -> Result<SemanticBackendReprV1, ProductionSemanticTypeErrorV1> {
    Ok(match backend {
        BackendRepr::Memory { sized } => SemanticBackendReprV1::memory(sized),
        BackendRepr::Scalar(scalar) => {
            SemanticBackendReprV1::scalar(backend_scalar_v1(context, scalar))
        }
        BackendRepr::ScalarPair(first, second) => SemanticBackendReprV1::scalar_pair(
            backend_scalar_v1(context, first),
            backend_scalar_v1(context, second),
        ),
        BackendRepr::SimdVector { element, count } => {
            SemanticBackendReprV1::simd_vector(backend_scalar_v1(context, element), count)
        }
        BackendRepr::SimdScalableVector { element, count } => {
            SemanticBackendReprV1::simd_scalable_vector(backend_scalar_v1(context, element), count)
        }
    })
}

fn backend_scalar_v1(
    context: &TypeConstructionContextV1<'_, '_>,
    scalar: Scalar,
) -> SemanticBackendScalarV1 {
    let primitive = primitive_v1(context, scalar.primitive());
    match scalar {
        Scalar::Initialized { .. } => {
            let valid_range = scalar.valid_range(context.layout_cx);
            SemanticBackendScalarV1::initialized(
                primitive,
                SemanticScalarValidityRangeV1::new(valid_range.start, valid_range.end),
            )
        }
        Scalar::Union { .. } => SemanticBackendScalarV1::union(primitive),
    }
}

fn primitive_v1(
    context: &TypeConstructionContextV1<'_, '_>,
    primitive: Primitive,
) -> SemanticBackendPrimitiveV1 {
    match primitive {
        Primitive::Pointer(address_space) => SemanticBackendPrimitiveV1::pointer(
            address_space.0,
            primitive.size(context.layout_cx).bytes(),
            primitive.align(context.layout_cx).abi.bytes(),
        ),
        Primitive::Int(integer, signed) => SemanticBackendPrimitiveV1::integer(
            signed,
            integer.size().bits() as u16,
            primitive.align(context.layout_cx).abi.bytes(),
        ),
        Primitive::Float(float) => SemanticBackendPrimitiveV1::float(
            float.size().bits() as u16,
            primitive.align(context.layout_cx).abi.bytes(),
        ),
    }
}

const fn float_bits_v1(width: FloatTy) -> u16 {
    match width {
        FloatTy::F16 => 16,
        FloatTy::F32 => 32,
        FloatTy::F64 => 64,
        FloatTy::F128 => 128,
    }
}

fn integer_bits_v1(layout: &LayoutCx<'_>, width: IntTy) -> u16 {
    match width {
        IntTy::I8 => 8,
        IntTy::I16 => 16,
        IntTy::I32 => 32,
        IntTy::I64 => 64,
        IntTy::I128 => 128,
        IntTy::Isize => layout.data_layout().pointer_size().bits() as u16,
    }
}

fn unsigned_integer_bits_v1(layout: &LayoutCx<'_>, width: UintTy) -> u16 {
    match width {
        UintTy::U8 => 8,
        UintTy::U16 => 16,
        UintTy::U32 => 32,
        UintTy::U64 => 64,
        UintTy::U128 => 128,
        UintTy::Usize => layout.data_layout().pointer_size().bits() as u16,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scalar_pair_noundef_requires_initialized_scalars_without_padding() {
        assert!(scalar_pair_is_noundef_v1(true, true, 4, 4, 8));
        assert!(!scalar_pair_is_noundef_v1(false, true, 4, 4, 8));
        assert!(!scalar_pair_is_noundef_v1(true, false, 4, 4, 8));
        assert!(!scalar_pair_is_noundef_v1(true, true, 4, 4, 12));
        assert!(!scalar_pair_is_noundef_v1(true, true, u64::MAX, 1, 0,));
    }

    #[test]
    fn noundef_field_cursor_ignores_zsts_and_rejects_gaps_or_overflow() {
        assert_eq!(advance_noundef_cursor_v1(4, 99, 0), Some(4));
        assert_eq!(advance_noundef_cursor_v1(4, 4, 8), Some(12));
        assert_eq!(advance_noundef_cursor_v1(4, 8, 8), None);
        assert_eq!(advance_noundef_cursor_v1(u64::MAX, u64::MAX, 1), None);
    }

    #[test]
    fn pointee_normalization_preserves_kind_size_and_reliable_alignment() {
        let shared = semantic_pointee_info_v1(
            PointeeInfo {
                safe: Some(PointerKind::SharedRef { frozen: true }),
                size: Size::from_bytes(32),
                align: Align::from_bytes(16).unwrap(),
            },
            Align::from_bytes(8).unwrap(),
        )
        .unwrap();
        assert_eq!(
            shared.kind(),
            SemanticAbiPointeeKindV1::SharedReference { frozen: true }
        );
        assert_eq!(shared.guaranteed_size_bytes(), 32);
        assert_eq!(shared.reliable_alignment_bytes(), 8);

        let mutable = semantic_pointee_info_v1(
            PointeeInfo {
                safe: Some(PointerKind::MutableRef { unpin: false }),
                size: Size::ZERO,
                align: Align::from_bytes(4).unwrap(),
            },
            Align::MAX,
        )
        .unwrap();
        assert_eq!(
            mutable.kind(),
            SemanticAbiPointeeKindV1::MutableReference { unpin: false }
        );

        let boxed = semantic_pointee_info_v1(
            PointeeInfo {
                safe: Some(PointerKind::Box {
                    unpin: true,
                    global: false,
                }),
                size: Size::ZERO,
                align: Align::from_bytes(2).unwrap(),
            },
            Align::MAX,
        )
        .unwrap();
        assert_eq!(
            boxed.kind(),
            SemanticAbiPointeeKindV1::Box {
                unpin: true,
                global: false,
            }
        );
    }

    #[test]
    fn raw_pointee_normalization_fails_closed_when_schema_cannot_represent_it() {
        let raw = semantic_pointee_info_v1(
            PointeeInfo {
                safe: None,
                size: Size::ZERO,
                align: Align::ONE,
            },
            Align::MAX,
        )
        .unwrap();
        assert_eq!(raw.kind(), SemanticAbiPointeeKindV1::Raw);
        assert_eq!(raw.guaranteed_size_bytes(), 0);
        assert_eq!(raw.reliable_alignment_bytes(), 1);

        assert!(
            semantic_pointee_info_v1(
                PointeeInfo {
                    safe: None,
                    size: Size::ZERO,
                    align: Align::from_bytes(8).unwrap(),
                },
                Align::MAX,
            )
            .is_err()
        );
    }
}

//! rustc-derived evidence for the exact typed vecadd profile.
//!
//! The extractor operates on normalized monomorphized types, records target
//! layout facts from rustc, and converts them into the shared canonical
//! evidence schema consumed by artifact generation.

use std::fmt;

use fe2o3_artifacts::{
    PointerWidth, RustDisjointIndexSpaceV1, RustLayoutEvidenceError, RustLayoutEvidenceV1,
    RustPhysicalComponentKindV1, RustPhysicalComponentV1, RustPointerMutabilityV1,
    RustScalarElementTypeV1, RustSourceTypeShapeV1, RustTypeEvidenceV1, RustcAbiClassV1,
};
use rustc_abi::{BackendRepr, ExternAbi, HasDataLayout, Primitive, Scalar};
use rustc_hir::Safety;
use rustc_hir::def::DefKind;
use rustc_middle::ty::layout::{LayoutCx, LayoutOf};
use rustc_middle::ty::{Instance, InstanceKind, Ty, TyCtxt, TyKind, TypingEnv};
use rustc_span::Symbol;

use crate::trusted_device_items::{self, TrustedDeviceItem};

const INDEX_1D_DIAGNOSTIC_ITEM: &str = "fe2o3_device_thread_index_1d";
const EXPECTED_POINTER_BITS: u64 = 64;
const EXPECTED_ARGUMENT_SIZE_BYTES: u64 = 16;
const EXPECTED_ARGUMENT_ALIGN_BYTES: u64 = 8;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum SourceShape {
    SharedSliceF32,
    DisjointSliceF32Index1d,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum AbiClass {
    ScalarPair,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum PhysicalComponentClass {
    Pointer { address_space: u32 },
    Integer { bits: u64, signed: bool },
    Float { bits: u64 },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct PhysicalComponentFacts {
    pub(crate) offset_bytes: u64,
    pub(crate) size_bytes: u64,
    pub(crate) abi_alignment_bytes: u64,
    pub(crate) class: PhysicalComponentClass,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct ArgumentLayoutFacts {
    pub(crate) source_shape: SourceShape,
    pub(crate) size_bytes: u64,
    pub(crate) abi_alignment_bytes: u64,
    pub(crate) abi_class: AbiClass,
    pub(crate) physical_components: Vec<PhysicalComponentFacts>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct TypedVecaddLayoutFacts {
    pub(crate) arguments: [ArgumentLayoutFacts; 3],
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum ExtractError {
    UnsupportedInstance(String),
    UnexpectedSignature(String),
    MissingDiagnosticItem(&'static str),
    WrongDiagnosticItemIdentity(&'static str),
    UnsupportedTarget(String),
    Layout {
        argument: &'static str,
        detail: String,
    },
    UnexpectedLayout {
        argument: &'static str,
        detail: String,
    },
    Evidence {
        argument: &'static str,
        source: RustLayoutEvidenceError,
    },
}

impl fmt::Display for ExtractError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnsupportedInstance(detail) => {
                write!(formatter, "unsupported instance: {detail}")
            }
            Self::UnexpectedSignature(detail) => {
                write!(formatter, "unexpected typed vecadd signature: {detail}")
            }
            Self::MissingDiagnosticItem(item) => {
                write!(formatter, "missing rustc diagnostic item `{item}`")
            }
            Self::WrongDiagnosticItemIdentity(item) => {
                write!(
                    formatter,
                    "wrong rustc diagnostic-item identity for `{item}`"
                )
            }
            Self::UnsupportedTarget(detail) => write!(formatter, "unsupported target: {detail}"),
            Self::Layout { argument, detail } => {
                write!(formatter, "failed to lay out {argument}: {detail}")
            }
            Self::UnexpectedLayout { argument, detail } => {
                write!(formatter, "unexpected layout for {argument}: {detail}")
            }
            Self::Evidence { argument, source } => {
                write!(
                    formatter,
                    "invalid canonical layout evidence for {argument}: {source}"
                )
            }
        }
    }
}

impl std::error::Error for ExtractError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Evidence { source, .. } => Some(source),
            _ => None,
        }
    }
}

/// Extract rustc's raw source and target-layout facts for the current exact
/// `#[kernel(typed)]` vecadd profile.
///
/// The instance must be an ordinary, fully monomorphized Rust item with the
/// semantic signature
/// `(&[f32], &[f32], fe2o3_device::DisjointSlice<f32, Index1D>) -> ()`.
/// All target-layout decisions use the codegen typing environment appropriate
/// for such an instance.
pub(crate) fn extract_exact_typed_vecadd_layout<'tcx>(
    tcx: TyCtxt<'tcx>,
    instance: Instance<'tcx>,
) -> Result<[RustLayoutEvidenceV1; 3], ExtractError> {
    let facts = extract_exact_typed_vecadd_layout_facts(tcx, instance)?;
    let [input_a, input_b, output] = facts.arguments;
    Ok([
        canonical_evidence(input_a, "argument 1")?,
        canonical_evidence(input_b, "argument 2")?,
        canonical_evidence(output, "argument 3")?,
    ])
}

fn extract_exact_typed_vecadd_layout_facts<'tcx>(
    tcx: TyCtxt<'tcx>,
    instance: Instance<'tcx>,
) -> Result<TypedVecaddLayoutFacts, ExtractError> {
    if !matches!(instance.def, InstanceKind::Item(_)) {
        return Err(ExtractError::UnsupportedInstance(format!(
            "expected InstanceKind::Item, found {:?}",
            instance.def
        )));
    }

    let typing_env = TypingEnv::fully_monomorphized();
    let instance_ty = instance.ty(tcx, typing_env);
    let (def_id, args) = match *instance_ty.kind() {
        TyKind::FnDef(def_id, args) if def_id == instance.def_id() => (def_id, args),
        _ => {
            return Err(ExtractError::UnsupportedInstance(format!(
                "expected the instance's own FnDef, found {instance_ty}"
            )));
        }
    };

    let signature =
        tcx.instantiate_bound_regions_with_erased(tcx.fn_sig(def_id).instantiate(tcx, args));
    if signature.safety != Safety::Safe {
        return Err(ExtractError::UnexpectedSignature(
            "kernel function must be safe".to_string(),
        ));
    }
    if signature.abi != ExternAbi::Rust || signature.c_variadic {
        return Err(ExtractError::UnexpectedSignature(
            "kernel function must use the non-variadic Rust ABI".to_string(),
        ));
    }
    if signature.inputs().len() != 3 || signature.output() != tcx.types.unit {
        return Err(ExtractError::UnexpectedSignature(format!(
            "expected exactly three arguments and unit return, found {signature}"
        )));
    }

    let input_a = signature.inputs()[0];
    let input_b = signature.inputs()[1];
    let output = signature.inputs()[2];
    require_shared_f32_slice(tcx, input_a, "argument 1")?;
    require_shared_f32_slice(tcx, input_b, "argument 2")?;
    require_disjoint_f32_index1d(tcx, output)?;

    let layout_cx = LayoutCx::new(tcx, typing_env);
    require_64_bit_pointer_and_usize(tcx, &layout_cx)?;

    Ok(TypedVecaddLayoutFacts {
        arguments: [
            extract_argument_layout(
                &layout_cx,
                input_a,
                "argument 1",
                SourceShape::SharedSliceF32,
            )?,
            extract_argument_layout(
                &layout_cx,
                input_b,
                "argument 2",
                SourceShape::SharedSliceF32,
            )?,
            extract_argument_layout(
                &layout_cx,
                output,
                "argument 3",
                SourceShape::DisjointSliceF32Index1d,
            )?,
        ],
    })
}

fn canonical_evidence(
    facts: ArgumentLayoutFacts,
    argument: &'static str,
) -> Result<RustLayoutEvidenceV1, ExtractError> {
    let (source_type, pointer_mutability) = match facts.source_shape {
        SourceShape::SharedSliceF32 => (
            RustSourceTypeShapeV1::shared_slice(RustScalarElementTypeV1::F32),
            RustPointerMutabilityV1::Const,
        ),
        SourceShape::DisjointSliceF32Index1d => (
            RustSourceTypeShapeV1::disjoint_slice(
                RustScalarElementTypeV1::F32,
                RustDisjointIndexSpaceV1::Index1D,
            ),
            RustPointerMutabilityV1::Mut,
        ),
    };
    let abi_class = match facts.abi_class {
        AbiClass::ScalarPair => RustcAbiClassV1::ScalarPair,
    };
    let mut components = Vec::with_capacity(facts.physical_components.len());
    for component in facts.physical_components {
        let kind = match component.class {
            PhysicalComponentClass::Pointer { address_space: 0 } => {
                RustPhysicalComponentKindV1::Pointer {
                    mutability: pointer_mutability,
                    pointee: RustScalarElementTypeV1::F32,
                }
            }
            PhysicalComponentClass::Integer {
                bits: 64,
                signed: false,
            } => RustPhysicalComponentKindV1::Usize,
            unexpected => {
                return Err(ExtractError::UnexpectedLayout {
                    argument,
                    detail: format!("unsupported canonical physical component {unexpected:?}"),
                });
            }
        };
        let alignment = u32::try_from(component.abi_alignment_bytes).map_err(|_| {
            ExtractError::UnexpectedLayout {
                argument,
                detail: "component ABI alignment exceeds u32".to_owned(),
            }
        })?;
        components.push(
            RustPhysicalComponentV1::new(
                component.offset_bytes,
                component.size_bytes,
                alignment,
                kind,
            )
            .map_err(|source| ExtractError::Evidence { argument, source })?,
        );
    }
    let alignment =
        u32::try_from(facts.abi_alignment_bytes).map_err(|_| ExtractError::UnexpectedLayout {
            argument,
            detail: "layout ABI alignment exceeds u32".to_owned(),
        })?;
    RustLayoutEvidenceV1::new(
        RustTypeEvidenceV1::new(source_type),
        abi_class,
        PointerWidth::Bits64,
        facts.size_bytes,
        alignment,
        components,
    )
    .map_err(|source| ExtractError::Evidence { argument, source })
}

fn require_shared_f32_slice<'tcx>(
    tcx: TyCtxt<'tcx>,
    ty: Ty<'tcx>,
    argument: &'static str,
) -> Result<(), ExtractError> {
    if let TyKind::Ref(_, pointee, rustc_hir::Mutability::Not) = *ty.kind()
        && let TyKind::Slice(element) = *pointee.kind()
        && element == tcx.types.f32
    {
        return Ok(());
    }

    Err(ExtractError::UnexpectedSignature(format!(
        "{argument} must be exactly `&[f32]`, found `{ty}`"
    )))
}

fn require_disjoint_f32_index1d<'tcx>(tcx: TyCtxt<'tcx>, ty: Ty<'tcx>) -> Result<(), ExtractError> {
    let TyKind::Adt(definition, args) = *ty.kind() else {
        return Err(ExtractError::UnexpectedSignature(format!(
            "argument 3 must be the genuine `DisjointSlice<f32, Index1D>`, found `{ty}`"
        )));
    };

    if trusted_device_items::classify(tcx, definition.did())
        != Some(TrustedDeviceItem::DisjointSlice)
    {
        return Err(ExtractError::WrongDiagnosticItemIdentity(
            "fe2o3_device_disjoint_slice",
        ));
    }

    let [Some(element), Some(index_space)] = [
        args.first().and_then(|arg| arg.as_type()),
        args.get(1).and_then(|arg| arg.as_type()),
    ] else {
        return Err(ExtractError::UnexpectedSignature(
            "genuine DisjointSlice must have exactly two type arguments".to_string(),
        ));
    };
    if args.len() != 2 || element != tcx.types.f32 {
        return Err(ExtractError::UnexpectedSignature(format!(
            "argument 3 must be `DisjointSlice<f32, Index1D>`, found `{ty}`"
        )));
    }

    let expected_index_space = trusted_index1d_type(tcx)?;
    if index_space != expected_index_space {
        return Err(ExtractError::UnexpectedSignature(format!(
            "argument 3 must use the genuine Index1D type, found `{index_space}`"
        )));
    }
    Ok(())
}

fn trusted_index1d_type<'tcx>(tcx: TyCtxt<'tcx>) -> Result<Ty<'tcx>, ExtractError> {
    let marker = tcx
        .get_diagnostic_item(Symbol::intern(INDEX_1D_DIAGNOSTIC_ITEM))
        .ok_or(ExtractError::MissingDiagnosticItem(
            INDEX_1D_DIAGNOSTIC_ITEM,
        ))?;
    if trusted_device_items::classify(tcx, marker) != Some(TrustedDeviceItem::ThreadIndex1d)
        || tcx.def_kind(marker) != DefKind::Fn
    {
        return Err(ExtractError::WrongDiagnosticItemIdentity(
            INDEX_1D_DIAGNOSTIC_ITEM,
        ));
    }

    let signature =
        tcx.instantiate_bound_regions_with_erased(tcx.fn_sig(marker).instantiate_identity());
    if !signature.inputs().is_empty()
        || signature.safety != Safety::Safe
        || signature.abi != ExternAbi::Rust
        || signature.c_variadic
    {
        return Err(ExtractError::WrongDiagnosticItemIdentity(
            INDEX_1D_DIAGNOSTIC_ITEM,
        ));
    }

    let TyKind::Adt(thread_index, args) = *signature.output().kind() else {
        return Err(ExtractError::WrongDiagnosticItemIdentity(
            INDEX_1D_DIAGNOSTIC_ITEM,
        ));
    };
    if trusted_device_items::classify(tcx, thread_index.did())
        != Some(TrustedDeviceItem::ThreadIndex)
        || args.len() != 1
    {
        return Err(ExtractError::WrongDiagnosticItemIdentity(
            INDEX_1D_DIAGNOSTIC_ITEM,
        ));
    }

    args.first()
        .and_then(|arg| arg.as_type())
        .ok_or(ExtractError::WrongDiagnosticItemIdentity(
            INDEX_1D_DIAGNOSTIC_ITEM,
        ))
}

fn require_64_bit_pointer_and_usize<'tcx>(
    tcx: TyCtxt<'tcx>,
    layout_cx: &LayoutCx<'tcx>,
) -> Result<(), ExtractError> {
    let pointer_bits = layout_cx.data_layout().pointer_size().bits();
    if pointer_bits != EXPECTED_POINTER_BITS {
        return Err(ExtractError::UnsupportedTarget(format!(
            "typed vecadd requires 64-bit pointers, found {pointer_bits}-bit pointers"
        )));
    }

    let usize_layout =
        layout_cx
            .layout_of(tcx.types.usize)
            .map_err(|error| ExtractError::Layout {
                argument: "usize",
                detail: error.to_string(),
            })?;
    let BackendRepr::Scalar(usize_scalar) = usize_layout.backend_repr else {
        return Err(ExtractError::UnsupportedTarget(format!(
            "usize must have scalar ABI, found {:?}",
            usize_layout.backend_repr
        )));
    };
    if !matches!(usize_scalar.primitive(), Primitive::Int(integer, false) if integer.size().bits() == 64)
        || usize_layout.size.bytes() != 8
    {
        return Err(ExtractError::UnsupportedTarget(format!(
            "typed vecadd requires a 64-bit unsigned usize, found {usize_layout:?}"
        )));
    }
    Ok(())
}

fn extract_argument_layout<'tcx>(
    layout_cx: &LayoutCx<'tcx>,
    ty: Ty<'tcx>,
    argument: &'static str,
    source_shape: SourceShape,
) -> Result<ArgumentLayoutFacts, ExtractError> {
    let layout = layout_cx
        .layout_of(ty)
        .map_err(|error| ExtractError::Layout {
            argument,
            detail: error.to_string(),
        })?;
    let BackendRepr::ScalarPair(first, second) = layout.backend_repr else {
        return Err(ExtractError::UnexpectedLayout {
            argument,
            detail: format!(
                "expected ScalarPair(pointer, u64), found {:?}",
                layout.backend_repr
            ),
        });
    };

    let first = physical_component(layout_cx, first, 0);
    let second_offset = first
        .size_bytes
        .next_multiple_of(second.align(layout_cx).abi.bytes());
    let second = physical_component(layout_cx, second, second_offset);
    let components = vec![first, second];
    require_exact_components(argument, &components)?;

    let size_bytes = layout.size.bytes();
    let abi_alignment_bytes = layout.align.abi.bytes();
    if size_bytes != EXPECTED_ARGUMENT_SIZE_BYTES
        || abi_alignment_bytes != EXPECTED_ARGUMENT_ALIGN_BYTES
        || layout
            .backend_repr
            .scalar_size(layout_cx)
            .map(|size| size.bytes())
            != Some(size_bytes)
        || layout
            .backend_repr
            .scalar_align(layout_cx)
            .map(|align| align.bytes())
            != Some(abi_alignment_bytes)
    {
        return Err(ExtractError::UnexpectedLayout {
            argument,
            detail: format!(
                "expected a 16-byte, 8-byte-aligned exact scalar pair, found {layout:?}"
            ),
        });
    }

    Ok(ArgumentLayoutFacts {
        source_shape,
        size_bytes,
        abi_alignment_bytes,
        abi_class: AbiClass::ScalarPair,
        physical_components: components,
    })
}

fn physical_component(
    layout_cx: &LayoutCx<'_>,
    scalar: Scalar,
    offset_bytes: u64,
) -> PhysicalComponentFacts {
    let primitive = scalar.primitive();
    let class = match primitive {
        Primitive::Pointer(address_space) => PhysicalComponentClass::Pointer {
            address_space: address_space.0,
        },
        Primitive::Int(integer, signed) => PhysicalComponentClass::Integer {
            bits: integer.size().bits(),
            signed,
        },
        Primitive::Float(float) => PhysicalComponentClass::Float {
            bits: float.size().bits(),
        },
    };
    PhysicalComponentFacts {
        offset_bytes,
        size_bytes: scalar.size(layout_cx).bytes(),
        abi_alignment_bytes: scalar.align(layout_cx).abi.bytes(),
        class,
    }
}

fn require_exact_components(
    argument: &'static str,
    components: &[PhysicalComponentFacts],
) -> Result<(), ExtractError> {
    let expected = [
        PhysicalComponentFacts {
            offset_bytes: 0,
            size_bytes: 8,
            abi_alignment_bytes: 8,
            class: PhysicalComponentClass::Pointer { address_space: 0 },
        },
        PhysicalComponentFacts {
            offset_bytes: 8,
            size_bytes: 8,
            abi_alignment_bytes: 8,
            class: PhysicalComponentClass::Integer {
                bits: 64,
                signed: false,
            },
        },
    ];
    if components != expected {
        return Err(ExtractError::UnexpectedLayout {
            argument,
            detail: format!(
                "expected ordered physical components {expected:?}, found {components:?}"
            ),
        });
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{
        AbiClass, ArgumentLayoutFacts, PhysicalComponentClass, PhysicalComponentFacts, SourceShape,
        canonical_evidence, require_exact_components,
    };
    use fe2o3_artifacts::{RustDisjointIndexSpaceV1, RustSourceTypeShapeV1, RustcAbiClassV1};

    fn exact_facts(source_shape: SourceShape) -> ArgumentLayoutFacts {
        ArgumentLayoutFacts {
            source_shape,
            size_bytes: 16,
            abi_alignment_bytes: 8,
            abi_class: AbiClass::ScalarPair,
            physical_components: vec![
                PhysicalComponentFacts {
                    offset_bytes: 0,
                    size_bytes: 8,
                    abi_alignment_bytes: 8,
                    class: PhysicalComponentClass::Pointer { address_space: 0 },
                },
                PhysicalComponentFacts {
                    offset_bytes: 8,
                    size_bytes: 8,
                    abi_alignment_bytes: 8,
                    class: PhysicalComponentClass::Integer {
                        bits: 64,
                        signed: false,
                    },
                },
            ],
        }
    }

    #[test]
    fn physical_components_fail_closed_on_reordering() {
        let components = [
            PhysicalComponentFacts {
                offset_bytes: 0,
                size_bytes: 8,
                abi_alignment_bytes: 8,
                class: PhysicalComponentClass::Integer {
                    bits: 64,
                    signed: false,
                },
            },
            PhysicalComponentFacts {
                offset_bytes: 8,
                size_bytes: 8,
                abi_alignment_bytes: 8,
                class: PhysicalComponentClass::Pointer { address_space: 0 },
            },
        ];

        assert!(require_exact_components("test argument", &components).is_err());
    }

    #[test]
    fn canonical_evidence_preserves_resolved_source_identity() {
        let shared = canonical_evidence(exact_facts(SourceShape::SharedSliceF32), "input").unwrap();
        let output =
            canonical_evidence(exact_facts(SourceShape::DisjointSliceF32Index1d), "output")
                .unwrap();

        assert!(matches!(
            shared.rust_type().source_type(),
            RustSourceTypeShapeV1::SharedSlice { .. }
        ));
        assert!(matches!(
            output.rust_type().source_type(),
            RustSourceTypeShapeV1::DisjointSlice {
                index_space: RustDisjointIndexSpaceV1::Index1D,
                ..
            }
        ));
        assert_eq!(shared.abi_class(), RustcAbiClassV1::ScalarPair);
        assert_eq!(output.abi_class(), RustcAbiClassV1::ScalarPair);
        assert_ne!(shared.type_identity(), output.type_identity());
    }
}

//! Source layout predicates only. Canonical type/ABI replay remains mandatory.

use rustc_abi::{BackendRepr, Primitive, Scalar, TagEncoding, Variants};
use rustc_middle::ty::{Ty, TyCtxt, TyKind, TypingEnv};

fn phantom<'tcx>(tcx: TyCtxt<'tcx>, ty: Ty<'tcx>) -> Option<Ty<'tcx>> {
    let TyKind::Adt(definition, arguments) = *ty.kind() else {
        return None;
    };
    if Some(definition.did()) != tcx.lang_items().phantom_data() || arguments.len() != 1 {
        return None;
    }
    arguments[0].as_type()
}

fn invariant<'tcx>(tcx: TyCtxt<'tcx>, ty: Ty<'tcx>, marker: Ty<'tcx>) -> bool {
    let Some(ty) = phantom(tcx, ty) else {
        return false;
    };
    invariant_function(ty, marker)
}

fn invariant_function<'tcx>(ty: Ty<'tcx>, marker: Ty<'tcx>) -> bool {
    matches!(ty.kind(), TyKind::FnPtr(signature, header)
        if signature.bound_vars().is_empty()
        && header.safety == rustc_hir::Safety::Safe
        && header.abi == rustc_abi::ExternAbi::Rust
        && !header.c_variadic
        && signature.skip_binder().inputs_and_output.as_slice() == [marker, marker])
}

pub(in crate::collector::production_importer_v1::numerical_policy_v1::defined_body_v1::matrix) fn fragment<
    'tcx,
>(
    tcx: TyCtxt<'tcx>,
    fragment: Ty<'tcx>,
    format: Ty<'tcx>,
    role: Ty<'tcx>,
    brand: Ty<'tcx>,
) -> Option<Ty<'tcx>> {
    let TyKind::Adt(definition, arguments) = *fragment.kind() else {
        return None;
    };
    if !definition.is_struct() || definition.non_enum_variant().fields.len() != 3 {
        return None;
    }
    let fields = definition
        .non_enum_variant()
        .fields
        .iter()
        .map(|field| {
            tcx.try_normalize_erasing_regions(
                TypingEnv::fully_monomorphized(),
                field.ty(tcx, arguments),
            )
        })
        .collect::<Result<Vec<_>, _>>()
        .ok()?;
    let TyKind::Array(element, count) = *fields[0].kind() else {
        return None;
    };
    if element != tcx.types.u32 || count.try_to_target_usize(tcx) != Some(8) {
        return None;
    }
    let contract = phantom(tcx, fields[1])?;
    let TyKind::Tuple(contract) = contract.kind() else {
        return None;
    };
    let [lifetime, association] = contract.as_slice() else {
        return None;
    };
    let TyKind::FnPtr(signature, _) = lifetime.kind() else {
        return None;
    };
    let lifetime_arguments = signature.skip_binder().inputs_and_output;
    let [input, output] = lifetime_arguments.as_slice() else {
        return None;
    };
    if input != output || !matches!(input.kind(), TyKind::Ref(_, unit, rustc_hir::Mutability::Mut) if *unit == tcx.types.unit)
        || !invariant_function(*lifetime, *input)
        || !invariant_function(*association, Ty::new_tup(tcx, &[Ty::new_tup(tcx, &[format, role]), brand]))
        || !phantom(tcx, fields[2]).is_some_and(|ty| matches!(ty.kind(), TyKind::RawPtr(unit, rustc_hir::Mutability::Mut) if *unit == tcx.types.unit))
    { return None; }
    let layout = tcx
        .layout_of(TypingEnv::fully_monomorphized().as_query_input(fragment))
        .ok()?;
    (layout.size.bytes() == 32
        && layout.align.abi.bytes() == 4
        && layout.backend_repr == (BackendRepr::Memory { sized: true })
        && !layout.uninhabited
        && matches!(layout.variants, Variants::Single { index } if index.as_u32() == 0)
        && layout.fields.count() == 3
        && [0, 32, 32]
            .into_iter()
            .enumerate()
            .all(|(i, offset)| layout.fields.offset(i).bytes() == offset))
    .then_some(fields[0])
}

pub(in crate::collector::production_importer_v1::numerical_policy_v1::defined_body_v1::matrix) fn view<'tcx>(
    tcx: TyCtxt<'tcx>,
    view: Ty<'tcx>,
    global_reference: Ty<'tcx>,
    format: Ty<'tcx>,
    role: Ty<'tcx>,
    matrix_brand: Ty<'tcx>,
) -> bool {
    let TyKind::Adt(definition, arguments) = *view.kind() else {
        return false;
    };
    if !definition.is_struct() || definition.non_enum_variant().fields.len() != 9 {
        return false;
    }
    let Ok(fields) = definition
        .non_enum_variant()
        .fields
        .iter()
        .map(|field| {
            tcx.try_normalize_erasing_regions(
                TypingEnv::fully_monomorphized(),
                field.ty(tcx, arguments),
            )
        })
        .collect::<Result<Vec<_>, _>>()
    else {
        return false;
    };
    if fields[..5] != [global_reference, tcx.types.usize, tcx.types.usize, tcx.types.usize, tcx.types.usize]
        || !invariant(tcx, fields[5], format)
        || !invariant(tcx, fields[6], role)
        || !invariant(tcx, fields[7], matrix_brand)
        || !phantom(tcx, fields[8]).is_some_and(|ty| {
            matches!(ty.kind(), TyKind::RawPtr(unit, rustc_hir::Mutability::Mut) if *unit == tcx.types.unit)
        })
    {
        return false;
    }
    let Ok(layout) = tcx.layout_of(TypingEnv::fully_monomorphized().as_query_input(view)) else {
        return false;
    };
    layout.size.bytes() == 40
        && layout.align.abi.bytes() == 8
        && layout.backend_repr == (BackendRepr::Memory { sized: true })
        && !layout.uninhabited
        && matches!(layout.variants, Variants::Single { index } if index.as_u32() == 0)
        && layout.fields.count() == 9
        && [0, 8, 16, 24, 32, 40, 40, 40, 40]
            .into_iter()
            .enumerate()
            .all(|(index, offset)| layout.fields.offset(index).bytes() == offset)
}

pub(super) fn result<'tcx>(tcx: TyCtxt<'tcx>, ty: Ty<'tcx>) -> bool {
    let Ok(layout) = tcx.layout_of(TypingEnv::fully_monomorphized().as_query_input(ty)) else {
        return false;
    };
    if layout.size.bytes() != 40
        || layout.align.abi.bytes() != 8
        || layout.backend_repr != (BackendRepr::Memory { sized: true })
        || layout.uninhabited
    {
        return false;
    }
    let Variants::Multiple {
        tag,
        tag_encoding,
        tag_field,
        variants,
    } = &layout.variants
    else {
        return false;
    };
    let TagEncoding::Niche {
        untagged_variant,
        niche_variants,
        niche_start,
    } = tag_encoding
    else {
        return false;
    };
    if untagged_variant.as_u32() != 0
        || niche_variants.start().as_u32() != 1
        || niche_variants.end().as_u32() != 1
        || *niche_start != 0
        || !matches!(tag, Scalar::Initialized { value: Primitive::Pointer(space), .. } if space.0 == 0)
        || variants.len() != 2
        || layout.fields.count() != 1
        || tag_field.as_u32() != 0
        || layout.fields.offset(0).bytes() != 0
    {
        return false;
    }
    variants.iter().enumerate().all(|(index, variant)| {
        variant.fields.count() == 1
            && variant.fields.offset(0).bytes() == [0, 8][index]
            && variant.size.bytes() == [40, 32][index]
            && variant.align.abi.bytes() == 8
            && !variant.uninhabited
            && matches!(variant.variants, Variants::Single { index: actual } if actual.as_usize() == index)
    })
}

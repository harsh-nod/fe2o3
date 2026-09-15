use super::*;
use rustc_abi::{BackendRepr, Variants};
use rustc_middle::ty::TypingEnv;

fn phantom<'tcx>(tcx: TyCtxt<'tcx>, ty: Ty<'tcx>) -> Option<Ty<'tcx>> {
    let TyKind::Adt(definition, arguments) = *ty.kind() else {
        return None;
    };
    (Some(definition.did()) == tcx.lang_items().phantom_data() && arguments.len() == 1)
        .then(|| arguments[0].as_type())
        .flatten()
}

pub(super) fn private_marker<'tcx>(tcx: TyCtxt<'tcx>, ty: Ty<'tcx>) -> bool {
    phantom(tcx, ty).is_some_and(|ty| {
        matches!(ty.kind(),
        TyKind::RawPtr(unit, rustc_hir::Mutability::Mut) if *unit == tcx.types.unit)
    })
}

fn invariant<'tcx>(ty: Ty<'tcx>, marker: Ty<'tcx>) -> bool {
    matches!(ty.kind(), TyKind::FnPtr(signature, header)
        if signature.bound_vars().is_empty()
            && header.safety == rustc_hir::Safety::Safe
            && header.abi == rustc_abi::ExternAbi::Rust
            && !header.c_variadic
            && signature.skip_binder().inputs_and_output.as_slice() == [marker, marker])
}

pub(super) fn accumulator<'tcx>(
    tcx: TyCtxt<'tcx>,
    ty: Ty<'tcx>,
    format: Ty<'tcx>,
    brand: Ty<'tcx>,
) -> Option<Ty<'tcx>> {
    let fields = source::fields(tcx, ty).ok()?;
    let [values, contract, private] = fields.as_slice() else {
        return None;
    };
    if !rust_f32_array_v1(tcx, *values, 4) || !private_marker(tcx, *private) {
        return None;
    }
    let contract = phantom(tcx, *contract)?;
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
    if input != output
        || !matches!(input.kind(), TyKind::Ref(_, unit, rustc_hir::Mutability::Mut) if *unit == tcx.types.unit)
        || !invariant(*lifetime, *input)
        || !invariant(*association, Ty::new_tup(tcx, &[format, brand]))
    {
        return None;
    }
    let layout = tcx
        .layout_of(TypingEnv::fully_monomorphized().as_query_input(ty))
        .ok()?;
    (layout.size.bytes() == 16
        && layout.align.abi.bytes() == 4
        && layout.backend_repr == (BackendRepr::Memory { sized: true })
        && !layout.uninhabited
        && matches!(layout.variants, Variants::Single { index } if index.as_u32() == 0)
        && layout.fields.count() == 3
        && [0, 16, 16]
            .into_iter()
            .enumerate()
            .all(|(i, offset)| layout.fields.offset(i).bytes() == offset))
    .then_some(*values)
}

pub(super) fn fragment<'tcx>(
    tcx: TyCtxt<'tcx>,
    ty: Ty<'tcx>,
    format: Ty<'tcx>,
    role: Ty<'tcx>,
    brand: Ty<'tcx>,
) -> bool {
    source::global_views::layout::fragment(tcx, ty, format, role, brand).is_some()
}

pub(super) fn context<'tcx>(tcx: TyCtxt<'tcx>, ty: Ty<'tcx>) -> bool {
    let Ok(layout) = tcx.layout_of(TypingEnv::fully_monomorphized().as_query_input(ty)) else {
        return false;
    };
    layout.size.bytes() == 8
        && layout.align.abi.bytes() == 8
        && matches!(layout.backend_repr, BackendRepr::Scalar(_))
        && !layout.uninhabited
        && matches!(layout.variants, Variants::Single { index } if index.as_u32() == 0)
        && layout.fields.count() == 2
        && layout.fields.offset(0).bytes() == 0
        && layout.fields.offset(1).bytes() == 8
}

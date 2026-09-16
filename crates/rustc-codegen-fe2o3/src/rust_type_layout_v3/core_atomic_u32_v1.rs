/// This is nominal compiler evidence, not a spelling or `!Freeze` heuristic.
pub(crate) fn is_genuine_core_atomic_u32_v1<'tcx>(
    tcx: TyCtxt<'tcx>,
    layout_cx: &LayoutCx<'tcx>,
    ty: Ty<'tcx>,
) -> bool {
    let TyKind::Adt(definition, arguments) = *ty.kind() else {
        return false;
    };
    let Some(core_anchor) = tcx.lang_items().sized_trait() else {
        return false;
    };
    if definition.did().krate != core_anchor.krate
        || tcx.crate_name(definition.did().krate).as_str() != "core"
        || !tcx.is_diagnostic_item(Symbol::intern("Atomic"), definition.did())
        || !definition.is_struct()
        || arguments.len() != 1
        || !arguments
            .first()
            .and_then(|argument| argument.as_type())
            .is_some_and(|element| matches!(element.kind(), TyKind::Uint(UintTy::U32)))
    {
        return false;
    }
    let Ok(layout) = layout_cx.layout_of(ty) else {
        return false;
    };
    layout.size.bytes() == 4
        && layout.align.abi.bytes() == 4
        && !layout.is_uninhabited()
        && definition.non_enum_variant().fields.len() == 1
        && layout.fields.count() == 1
        && layout.fields.offset(0).bytes() == 0
}

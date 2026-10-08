//! Authenticated original rustc Atomic<T> field facts, not pointer permission.
use super::*;
use rustc_span::Symbol;

// Follow the original definition keys, not rustc's user-facing re-export path.
// The two fixed callers inspect at most four TypeNs parents plus the crate root;
// no pretty-printer, alias spelling, allocation, or path-prefix acceptance.
fn original_core_type_path(
    tcx: TyCtxt<'_>,
    definition: rustc_hir::def_id::DefId,
    core: rustc_hir::def_id::CrateNum,
    expected: &[&str],
) -> bool {
    use rustc_hir::definitions::DefPathData;
    if definition.krate != core || !matches!(expected.len(), 3 | 4) {
        return false;
    }
    let mut current = definition;
    for name in expected.iter().rev() {
        let key = tcx.def_key(current);
        if key.disambiguated_data.disambiguator != 0
            || !matches!(key.disambiguated_data.data,
                DefPathData::TypeNs(symbol) if symbol.as_str() == *name)
        {
            return false;
        }
        let Some(parent) = key.parent else {
            return false;
        };
        current = rustc_hir::def_id::DefId {
            krate: core,
            index: parent,
        };
    }
    let root = tcx.def_key(current);
    root.parent.is_none()
        && root.disambiguated_data.disambiguator == 0
        && matches!(root.disambiguated_data.data, DefPathData::CrateRoot)
}

/// Observe only the actual core diagnostic/lang-item chain. A same-spelled local
/// struct, arbitrary UnsafeCell, or independently encountered Align4 gets no tag.
pub(super) fn authenticated_shape<'tcx>(
    tcx: TyCtxt<'tcx>,
    ty: Ty<'tcx>,
) -> Result<Option<(SemanticRustTypeKindV1, [Ty<'tcx>; 3])>, &'static str> {
    let TyKind::Adt(atomic, arguments) = *ty.kind() else {
        return Ok(None);
    };
    let Some(core) = tcx.lang_items().sized_trait() else {
        return Ok(None);
    };
    if atomic.did().krate != core.krate
        || tcx.crate_name(core.krate).as_str() != "core"
        || tcx.get_diagnostic_item(Symbol::intern("Atomic")) != Some(atomic.did())
        || !original_core_type_path(tcx, atomic.did(), core.krate, &["sync", "atomic", "Atomic"])
    {
        return Ok(None);
    }
    let [argument] = arguments.as_slice() else {
        return Err("atomic nominal arguments");
    };
    let Some(element) = argument.as_type() else {
        return Err("atomic nominal element");
    };
    let kind = match element.kind() {
        TyKind::Int(IntTy::I32) => SemanticRustTypeKindV1::AtomicI32,
        TyKind::Uint(UintTy::U32) => SemanticRustTypeKindV1::AtomicU32,
        _ => return Ok(None),
    };
    let normalize = |ty| {
        tcx.try_normalize_erasing_regions(TypingEnv::fully_monomorphized(), ty)
            .map_err(|_| "atomic field normalization")
    };
    if !atomic.is_struct() || atomic.non_enum_variant().fields.len() != 1 {
        return Err("atomic wrapper field census");
    }
    let cell = normalize(
        atomic
            .non_enum_variant()
            .fields
            .iter()
            .next()
            .ok_or("atomic field missing")?
            .ty(tcx, arguments),
    )?;
    let TyKind::Adt(cell_definition, cell_args) = *cell.kind() else {
        return Err("atomic cell type");
    };
    if tcx.lang_items().unsafe_cell_type() != Some(cell_definition.did())
        || cell_definition.did().krate != core.krate
        || !cell_definition.is_struct()
        || cell_definition.non_enum_variant().fields.len() != 1
    {
        return Err("atomic cell lang-item identity");
    }
    let [cell_arg] = cell_args.as_slice() else {
        return Err("atomic cell arguments");
    };
    let Some(storage) = cell_arg.as_type() else {
        return Err("atomic cell storage");
    };
    let storage = normalize(storage)?;
    if normalize(
        cell_definition
            .non_enum_variant()
            .fields
            .iter()
            .next()
            .ok_or("atomic field missing")?
            .ty(tcx, cell_args),
    )? != storage
    {
        return Err("atomic cell field differs");
    }
    let TyKind::Adt(aligned, aligned_args) = *storage.kind() else {
        return Err("atomic aligned storage type");
    };
    if aligned.did().krate != core.krate
        || !original_core_type_path(
            tcx,
            aligned.did(),
            core.krate,
            &["sync", "atomic", "private", "Align4"],
        )
        || !aligned.is_struct()
        || aligned.non_enum_variant().fields.len() != 1
    {
        return Err("atomic aligned storage identity");
    }
    let [aligned_arg] = aligned_args.as_slice() else {
        return Err("atomic aligned arguments");
    };
    if aligned_arg.as_type() != Some(element)
        || normalize(
            aligned
                .non_enum_variant()
                .fields
                .iter()
                .next()
                .ok_or("atomic field missing")?
                .ty(tcx, aligned_args),
        )? != element
    {
        return Err("atomic aligned field differs");
    }
    Ok(Some((kind, [cell, storage, element])))
}

#[cfg(test)]
mod tests;

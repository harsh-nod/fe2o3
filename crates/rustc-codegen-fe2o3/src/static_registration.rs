//! Typed reads of producer-owned static registration initializers.

use rustc_abi::{Size, TyAndLayout};
use rustc_hir::def_id::DefId;
use rustc_middle::mir::interpret::{ConstAllocation, GlobalAlloc, alloc_range};
use rustc_middle::ty::layout::{LayoutCx, LayoutOf};
use rustc_middle::ty::{TyCtxt, TypingEnv};

pub(crate) fn integer(tcx: TyCtxt<'_>, def_id: DefId, field: usize) -> Result<u128, String> {
    let (allocation, field_layout, offset) = initializer_field(tcx, def_id, field)?;
    allocation
        .inner()
        .read_scalar(&tcx, alloc_range(offset, field_layout.size), false)
        .map_err(|error| format!("field {field} is not one initialized integer: {error:?}"))?
        .to_bits(field_layout.size)
        .discard_err()
        .ok_or_else(|| format!("field {field} has an invalid integer width"))
}

pub(crate) fn string(tcx: TyCtxt<'_>, def_id: DefId, field: usize) -> Result<String, String> {
    let (allocation, field_layout, offset) = initializer_field(tcx, def_id, field)?;
    let pointer_size = tcx.data_layout.pointer_size();
    if field_layout.size != pointer_size * 2 {
        return Err(format!(
            "field {field} has size {}, expected one string fat pointer",
            field_layout.size.bytes()
        ));
    }
    let pointer = allocation
        .inner()
        .read_scalar(&tcx, alloc_range(offset, pointer_size), true)
        .map_err(|error| format!("field {field} string pointer is invalid: {error:?}"))?
        .to_pointer(&tcx)
        .discard_err()
        .ok_or_else(|| format!("field {field} string pointer has no provenance"))?
        .into_pointer_or_addr()
        .map_err(|_| format!("field {field} string pointer has no allocation provenance"))?;
    let length = allocation
        .inner()
        .read_scalar(
            &tcx,
            alloc_range(offset + pointer_size, pointer_size),
            false,
        )
        .map_err(|error| format!("field {field} string length is invalid: {error:?}"))?
        .to_target_usize(&tcx)
        .discard_err()
        .ok_or_else(|| format!("field {field} string length is not target usize"))?;
    let (provenance, data_offset) = pointer.into_raw_parts();
    let GlobalAlloc::Memory(data) = tcx.global_alloc(provenance.alloc_id()) else {
        return Err(format!(
            "field {field} string does not point to immutable data"
        ));
    };
    let length = Size::from_bytes(length);
    if data_offset
        .checked_add(length, &tcx)
        .is_none_or(|end| end > data.inner().size())
    {
        return Err(format!(
            "field {field} string points outside its allocation"
        ));
    }
    let bytes = data
        .inner()
        .get_bytes_strip_provenance(&tcx, alloc_range(data_offset, length))
        .map_err(|error| format!("field {field} string bytes are invalid: {error:?}"))?;
    std::str::from_utf8(bytes)
        .map(str::to_owned)
        .map_err(|error| format!("field {field} string is not UTF-8: {error}"))
}

pub(crate) fn function<'tcx>(
    tcx: TyCtxt<'tcx>,
    def_id: DefId,
    field: usize,
) -> Result<rustc_middle::ty::Instance<'tcx>, String> {
    let (allocation, field_layout, offset) = initializer_field(tcx, def_id, field)?;
    let pointer_size = tcx.data_layout.pointer_size();
    if field_layout.size != pointer_size {
        return Err(format!(
            "field {field} has size {}, expected one function pointer",
            field_layout.size.bytes()
        ));
    }
    let pointer = allocation
        .inner()
        .read_scalar(&tcx, alloc_range(offset, pointer_size), true)
        .map_err(|error| format!("field {field} function pointer is invalid: {error:?}"))?
        .to_pointer(&tcx)
        .discard_err()
        .ok_or_else(|| format!("field {field} is not a function pointer"))?
        .into_pointer_or_addr()
        .map_err(|_| format!("field {field} function pointer has no provenance"))?;
    let (provenance, function_offset) = pointer.into_raw_parts();
    if function_offset.bytes() != 0 {
        return Err(format!(
            "field {field} function pointer has a nonzero offset"
        ));
    }
    match tcx.global_alloc(provenance.alloc_id()) {
        GlobalAlloc::Function { instance } => Ok(instance),
        _ => Err(format!(
            "field {field} pointer does not identify a function allocation"
        )),
    }
}

fn initializer_field<'tcx>(
    tcx: TyCtxt<'tcx>,
    def_id: DefId,
    field: usize,
) -> Result<
    (
        ConstAllocation<'tcx>,
        TyAndLayout<'tcx, rustc_middle::ty::Ty<'tcx>>,
        Size,
    ),
    String,
> {
    let ty = tcx
        .try_normalize_erasing_regions(
            TypingEnv::fully_monomorphized(),
            tcx.type_of(def_id).instantiate_identity(),
        )
        .map_err(|_| "static registration type did not normalize".to_owned())?;
    let layout_cx = LayoutCx::new(tcx, TypingEnv::fully_monomorphized());
    let layout = layout_cx
        .layout_of(ty)
        .map_err(|error| format!("static registration layout failed: {error}"))?;
    if field >= layout.fields.count() {
        return Err(format!(
            "field {field} is outside {}-field static registration",
            layout.fields.count()
        ));
    }
    let field_layout = layout.field(&layout_cx, field);
    let offset = layout.fields.offset(field);
    if offset
        .checked_add(field_layout.size, &tcx)
        .is_none_or(|end| end > layout.size)
    {
        return Err(format!("field {field} layout is outside the static"));
    }
    let allocation = tcx
        .eval_static_initializer(def_id)
        .map_err(|_| "static registration initializer did not evaluate".to_owned())?;
    if allocation.inner().size() != layout.size {
        return Err(format!(
            "static registration allocation is {} bytes, expected {}",
            allocation.inner().size().bytes(),
            layout.size.bytes()
        ));
    }
    Ok((allocation, field_layout, offset))
}

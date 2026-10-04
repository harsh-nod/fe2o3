//! Reobserve the selected real MIR call, not a diagnostic rendering or source hash.
use super::{Error, Result, file};
use crate::rustc_semantic_adapter_v1::{
    borrowed_rustc_mir_body_sha256_v1, canonical_function_identities_v1,
    canonical_source_provenance_v1, rustc_block_identity_v1,
};
use fe2o3_lower_mir_kernel::OrderedCompositionRegionV1;
use rustc_hir::def::{DefKind, Res};
use rustc_hir::def_id::{CRATE_DEF_INDEX, DefId};
use rustc_middle::mir::{TerminatorKind, UnwindAction};
use rustc_middle::ty::{Instance, TyCtxt};
use rustc_span::{ExpnKind, Span};

pub(super) struct ObservedSite {
    pub(super) macro_definition: DefId,
    pub(super) marker_definition: DefId,
    pub(super) callsite: Span,
}

pub(super) fn direct_macro_site(span: Span, expected: DefId) -> Result<Span> {
    if span.is_dummy() || !span.from_expansion() {
        return Err(Error::refused(
            "publisher requires the actual trusted macro expansion",
        ));
    }
    let data = span.ctxt().outer_expn_data();
    if !matches!(data.kind, ExpnKind::Macro(..))
        || data.macro_def_id != Some(expected)
        || data.call_site.is_dummy()
        || data.call_site.from_expansion()
        || data.call_site.lo() >= data.call_site.hi()
    {
        return Err(Error::refused(
            "publisher refuses wrapper macros or substituted expansions",
        ));
    }
    Ok(data.call_site)
}

fn exported_macro(tcx: TyCtxt<'_>, marker: DefId) -> Result<DefId> {
    let root = DefId {
        krate: marker.krate,
        index: CRATE_DEF_INDEX,
    };
    let children = tcx.module_children(root);
    if children.len() > 4096 {
        return Err(Error::refused(
            "publisher provider export roster bound exceeded",
        ));
    }
    let mut selected = None;
    for child in children {
        if child.ident.name.as_str() == "amdgpu_ordered_program" {
            let Res::Def(DefKind::Macro(..), definition) = child.res else {
                return Err(Error::refused(
                    "publisher provider macro export kind differs",
                ));
            };
            if definition.krate != marker.krate || selected.replace(definition).is_some() {
                return Err(Error::refused(
                    "publisher provider macro export is ambiguous",
                ));
            }
        }
    }
    selected.ok_or_else(|| Error::refused("publisher authenticated provider macro is absent"))
}

pub(super) fn reobserve<'tcx>(
    tcx: TyCtxt<'tcx>,
    instance: Instance<'tcx>,
    region: &OrderedCompositionRegionV1<'_>,
) -> Result<ObservedSite> {
    let function = region.source_function();
    let semantic_block = function
        .blocks()
        .get(region.semantic_block().index() as usize)
        .ok_or_else(|| Error::refused("publisher selected semantic block is absent"))?;
    let identities = canonical_function_identities_v1(tcx, instance);
    if identities.function() != function.identity()
        || identities.function().as_bytes() != &region.program().source().function
    {
        return Err(Error::refused(
            "publisher actual function and selected program differ",
        ));
    }
    let body = tcx.instance_mir(instance.def);
    if body.basic_blocks.len() > 4096 || body.local_decls.len() > 4096 {
        return Err(Error::refused("publisher actual MIR body bounds exceeded"));
    }
    let mut items = body.local_decls.len();
    for block in body.basic_blocks.iter() {
        items = items
            .checked_add(block.statements.len())
            .and_then(|n| n.checked_add(1))
            .ok_or_else(|| Error::refused("publisher MIR scan overflow"))?;
        if items > 131_072 {
            return Err(Error::refused("publisher MIR scan bound exceeded"));
        }
    }
    let hash = borrowed_rustc_mir_body_sha256_v1(tcx, instance, body);
    let mut selected = None;
    for (raw, _) in body.basic_blocks.iter_enumerated() {
        if rustc_block_identity_v1(identities.function(), hash, raw.as_u32())
            == semantic_block.identity()
            && selected.replace(raw).is_some()
        {
            return Err(Error::refused("publisher actual MIR block is ambiguous"));
        }
    }
    let raw =
        selected.ok_or_else(|| Error::refused("publisher actual MIR block identity differs"))?;
    let terminator = body.basic_blocks[raw].terminator();
    let TerminatorKind::Call {
        func,
        args,
        destination,
        target: Some(_),
        unwind: UnwindAction::Continue | UnwindAction::Unreachable,
        ..
    } = &terminator.kind
    else {
        return Err(Error::refused("publisher actual call shape differs"));
    };
    if args.len() != 8 || !destination.projection.is_empty() {
        return Err(Error::refused(
            "publisher actual call arguments or destination differ",
        ));
    }
    let observed = crate::production_ordered_program_v32::observe_call(
        tcx,
        instance,
        body,
        func,
        [
            &args[0].node,
            &args[1].node,
            &args[2].node,
            &args[3].node,
            &args[4].node,
            &args[5].node,
            &args[6].node,
            &args[7].node,
        ],
    )
    .map_err(Error::refused)?;
    if observed.program().count() != region.program().program().count()
        || observed.program().packed_words() != region.program().program().packed_words()
        || observed.registers().scratch() != region.program().registers().scratch()
        || observed.registers().output() != region.program().registers().output()
        || observed.registers().inputs() != region.program().registers().inputs()
    {
        return Err(Error::refused(
            "publisher current call descriptors or physical roles differ",
        ));
    }
    let marker = crate::trusted_device_items::definition(
        tcx,
        crate::trusted_device_items::TrustedDeviceItem::AmdGpuOrderedProgramE32,
    )
    .ok_or_else(|| Error::refused("publisher authenticated marker definition is absent"))?;
    if observed.instance().def_id() != marker {
        return Err(Error::refused(
            "publisher current callee is not the authenticated marker",
        ));
    }
    let macro_definition = exported_macro(tcx, marker)?;
    let span = terminator.source_info.span;
    let callsite = direct_macro_site(span, macro_definition)?;
    // Bound both source-map lookups before the canonical provenance helper scans.
    file::bounded_file(tcx, span)?;
    file::bounded_file(tcx, callsite)?;
    let provenance = canonical_source_provenance_v1(tcx, span, 2)
        .map_err(|_| Error::refused("publisher actual source provenance is unavailable"))?;
    if provenance.provenance() != semantic_block.terminator().source() {
        return Err(Error::refused(
            "publisher selected source provenance differs",
        ));
    }
    Ok(ObservedSite {
        macro_definition,
        marker_definition: marker,
        callsite,
    })
}

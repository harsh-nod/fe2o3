//! Test-only diagnostics; the complete importer still returns its original error.

use super::*;
use fe2o3_mir_model::semantic_mir_v1::{
    SemanticFieldsShapeV1, SemanticMirLocationV1, SemanticRustcVariantsV1,
    layout_diagnostics_v1::diagnose_type_layouts_v1,
};

pub(in crate::collector::production_importer_v1) fn report<'tcx>(
    tcx: TyCtxt<'tcx>,
    plan: &ProductionSemanticPreflightPlanV1<'tcx>,
    request: &InertSemanticMirRequestV1,
) {
    let Err((location, error)) = diagnose_type_layouts_v1(request, SemanticMirLimitsV1::default())
    else {
        eprintln!(
            "BF16 canonical type/layout diagnostic passed; complete admission is still required"
        );
        return;
    };
    eprintln!("BF16 canonical layout failure: location={location:?} error={error:?}");
    let SemanticMirLocationV1::Type(id) = location else {
        // A layout identity can also conflict with an adjusted function/callable
        // ABI. Preserve its exact roster index rather than guessing a type.
        eprintln!("BF16 layout failure is request-wide or in adjusted ABI, not a localized type");
        return;
    };
    let Ok(records) = construct_production_semantic_types_v1(tcx, plan.type_producers()) else {
        eprintln!("BF16 diagnostic could not replay source type construction");
        return;
    };
    let records = records.into_records();
    let Some(record) = records.get(id.index() as usize) else {
        return;
    };
    let Some(producer) = plan.type_producers().get(id.index() as usize) else {
        return;
    };
    eprintln!(
        "BF16 canonical type={} rust={:?} record={record:#?}",
        id.index(),
        producer.ty
    );
    eprintln!(
        "BF16 actual rustc type={} layout={:#?}",
        id.index(),
        producer.layout
    );
    for (other, declaration) in records.iter().enumerate() {
        if other != id.index() as usize
            && declaration.layout_identity() == record.layout_identity()
            && declaration.layout() != record.layout()
        {
            eprintln!(
                "BF16 same layout identity, different record: type={other} rust={:?} record={declaration:#?}",
                plan.type_producers()[other].ty
            );
        }
    }
    let field = |variant: Option<usize>,
                 index: usize,
                 child: SemanticTypeIdV1,
                 offset: Option<u64>| {
        let Some(declaration) = records.get(child.index() as usize) else {
            return;
        };
        eprintln!(
            "BF16 canonical field: parent={} variant={variant:?} field={index} offset={offset:?} type={} rust={:?} size={:?} align={} layout={:?}",
            id.index(),
            child.index(),
            plan.type_producers()[child.index() as usize].ty,
            declaration.layout().size_bytes(),
            declaration.layout().alignment_bytes(),
            declaration.layout()
        );
    };
    match record.shape() {
        SemanticTypeShapeV1::Aggregate(fields) | SemanticTypeShapeV1::Tuple(fields) => {
            let offsets = match record.layout().fields() {
                SemanticFieldsShapeV1::Arbitrary {
                    source_order_offsets_bytes,
                    ..
                } => source_order_offsets_bytes.as_ref(),
                _ => &[],
            };
            for (index, child) in fields.fields().iter().copied().enumerate() {
                field(None, index, child, offsets.get(index).copied());
            }
        }
        SemanticTypeShapeV1::Enum { variants, .. } => {
            for (variant_index, variant) in variants.iter().enumerate() {
                let offsets = match record.layout().variants() {
                    SemanticRustcVariantsV1::Multiple(layout) => layout
                        .variants()
                        .get(variant_index)
                        .map(|variant| variant.aggregate().field_offsets()),
                    _ => None,
                };
                for (index, child) in variant.fields().fields().iter().copied().enumerate() {
                    field(
                        Some(variant_index),
                        index,
                        child,
                        offsets.and_then(|offsets| offsets.get(index).copied()),
                    );
                }
            }
        }
        _ => {}
    }
}

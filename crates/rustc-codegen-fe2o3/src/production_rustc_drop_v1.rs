//! Shared classification of rustc MIR drop terminators for production import.

use rustc_middle::mir::{Body, Place};
use rustc_middle::ty::{EarlyBinder, Instance, TyCtxt, TypingEnv};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ProductionRustcDropClassV1 {
    Trivial,
    RequiresDropGlue,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct ProductionRustcDropNormalizationErrorV1;

pub(crate) fn classify_rustc_drop_v1<'tcx>(
    tcx: TyCtxt<'tcx>,
    instance: Instance<'tcx>,
    body: &Body<'tcx>,
    place: Place<'tcx>,
) -> Result<ProductionRustcDropClassV1, ProductionRustcDropNormalizationErrorV1> {
    let raw = place.ty(body, tcx).ty;
    let ty = instance
        .try_instantiate_mir_and_normalize_erasing_regions(
            tcx,
            TypingEnv::fully_monomorphized(),
            EarlyBinder::bind(raw),
        )
        .map_err(|_| ProductionRustcDropNormalizationErrorV1)?;
    Ok(if ty.needs_drop(tcx, TypingEnv::fully_monomorphized()) {
        ProductionRustcDropClassV1::RequiresDropGlue
    } else {
        ProductionRustcDropClassV1::Trivial
    })
}

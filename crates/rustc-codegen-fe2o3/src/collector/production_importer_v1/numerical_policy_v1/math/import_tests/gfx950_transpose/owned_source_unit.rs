// Separate source-unit filters. Existing V24 Move-only tests remain unchanged.
pub(super) fn check_owned_source<'tcx>(
    tcx: TyCtxt<'tcx>,
    closure: crate::collector::AuthenticatedCollectedKernelClosureV1<'tcx>,
    fp8: bool,
) {
    check_mode(tcx, closure, fp8, true);
}

#[test]
#[ignore = "requires cached actual AMD metadata; source-unit only, no Workgroup lifetime/KIR claim"]
fn transpose_fp4_live_owned_source_unit_gfx950() {
    run_registered_source(
        "gfx950",
        "gfx950_transpose::transpose_fp4_live_owned_source_unit_gfx950",
        true,
        false,
        SourceCase::TransposeOwned(false),
    );
}

#[test]
#[ignore = "requires cached actual AMD metadata; source-unit only, no Workgroup lifetime/KIR claim"]
fn transpose_fp8_live_owned_source_unit_gfx950() {
    run_registered_source(
        "gfx950",
        "gfx950_transpose::transpose_fp8_live_owned_source_unit_gfx950",
        true,
        false,
        SourceCase::TransposeOwned(true),
    );
}

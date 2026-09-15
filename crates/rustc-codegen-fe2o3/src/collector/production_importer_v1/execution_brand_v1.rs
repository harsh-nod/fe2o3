//! Nominal reusable execution brands retain their complete source type.
//! Root coordinates identify the authenticated kernel, not the dynamic phase owner.

use super::*;

pub(super) fn rust_execution_brand_v1<'tcx>(
    tcx: TyCtxt<'tcx>,
    source: Ty<'tcx>,
) -> Option<RustKernelBrandV1<'tcx>> {
    let mut current = source;
    // Every wrapper is a distinct source type, bounded by the existing MIR type ceiling.
    for _ in 0..fe2o3_mir_model::semantic_mir_v1::HARD_MAX_TYPES_V1 {
        if let Some(root) = rust_kernel_brand_v1(tcx, current) {
            return Some(RustKernelBrandV1 { ty: source, ..root });
        }
        let arguments = rust_trusted_adt_type_arguments_v1(
            tcx,
            current,
            TrustedDeviceItem::ReusableWorkgroupBrand,
        )?;
        let [parent] = arguments.as_slice() else {
            return None;
        };
        current = *parent;
    }
    None
}

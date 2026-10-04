//! Exact already-promoted root FnABI. Source occurrence preparation retains the original plan.
use crate::rustc_semantic_plan_v1::ProductionSemanticPreflightPlanV1;
use rustc_abi::{CanonAbi, ExternAbi};
use rustc_target::callconv::PassMode;
pub(crate) fn check(
    plan: &ProductionSemanticPreflightPlanV1<'_>,
) -> Result<([u8; 32], [u8; 32]), &'static str> {
    let [abi] = plan.function_abi_producers() else {
        return Err("physical-entry current root ABI roster differs");
    };
    if abi.function.index() != 0
        || abi.extern_abi != ExternAbi::GpuKernel
        || abi.fn_abi.conv != CanonAbi::GpuKernel
        || abi.fn_abi.c_variadic
        || abi.fn_abi.can_unwind
        || abi.fn_abi.fixed_count != 5
        || abi.source_inputs.len() != 5
        || abi.fn_abi.args.len() != 5
        || !abi.source_output.is_unit()
        || !abi.fn_abi.ret.layout.ty.is_unit()
        || !matches!(abi.fn_abi.ret.mode, PassMode::Ignore)
    {
        return Err("physical-entry promoted kernel ABI differs");
    }
    for (position, (source_ty, argument)) in abi
        .source_inputs
        .iter()
        .zip(abi.fn_abi.args.iter())
        .enumerate()
    {
        if *source_ty != argument.layout.ty {
            return Err("physical-entry adjusted source ABI type differs");
        }
        let valid = if position == 0 {
            matches!(argument.mode, PassMode::Pair(_, _))
                && argument.layout.size.bytes() == 16
                && argument.layout.align.abi.bytes() == 8
        } else {
            matches!(argument.mode, PassMode::Direct(_))
                && matches!(
                    source_ty.kind(),
                    rustc_middle::ty::TyKind::Uint(rustc_middle::ty::UintTy::U32)
                )
                && argument.layout.size.bytes() == 4
                && argument.layout.align.abi.bytes() == 4
        };
        if !valid {
            return Err("physical-entry adjusted source ABI layout or pass mode differs");
        }
    }
    Ok((abi.rustc_source_signature_sha256, abi.rustc_fn_abi_sha256))
}

//! Live, capture-free constant origins. These private observations are not receipts.

use super::*;
use crate::rustc_semantic_adapter_v1::rustc_type_identity_v1;
use fe2o3_mir_model::semantic_mir_v1::SemanticTypeIdentityV1;
use rustc_middle::mir::{BasicBlock, Const, ConstValue};

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct EmptyClosureConstantV1 {
    caller: SemanticFunctionIdentityV1,
    block: usize,
    argument: usize,
    ty: SemanticTypeIdentityV1,
}

impl EmptyClosureConstantV1 {
    pub(super) fn observe<'tcx>(
        tcx: TyCtxt<'tcx>,
        caller: Instance<'tcx>,
        block: usize,
        argument: usize,
        work: &mut SourceClosureWorkV1,
    ) -> Result<Self, ClosureProfileErrorV1> {
        charge_work(work, 3)?;
        let body = tcx.instance_mir(caller.def);
        if block >= body.basic_blocks.len() {
            return Err(ClosureProfileErrorV1::new(
                "closure constant block is absent",
            ));
        }
        let TerminatorKind::Call { args, .. } = &body.basic_blocks[BasicBlock::from_usize(block)]
            .terminator()
            .kind
        else {
            return Err(ClosureProfileErrorV1::new(
                "closure constant occurrence is not a call",
            ));
        };
        let Some(Operand::Constant(constant)) = args.get(argument).map(|arg| &arg.node) else {
            return Err(ClosureProfileErrorV1::new(
                "closure constant argument is absent",
            ));
        };
        let Const::Val(ConstValue::ZeroSized, raw_ty) = constant.const_ else {
            return Err(ClosureProfileErrorV1::new(
                "closure constant must be an evaluated zero-sized value",
            ));
        };
        charge_work(work, 3)?;
        let ty = normalized_ty(tcx, caller, raw_ty, "closure constant")?;
        let TyKind::Closure(_, arguments) = ty.kind() else {
            return Err(ClosureProfileErrorV1::new(
                "zero-sized value is not a concrete closure",
            ));
        };
        closure_kind(arguments.as_closure().kind_ty().to_opt_closure_kind())?;
        if !arguments.as_closure().upvar_tys().is_empty() {
            return Err(ClosureProfileErrorV1::new("closure constant has captures"));
        }
        charge_work(work, 3)?;
        let layout = LayoutCx::new(tcx, TypingEnv::fully_monomorphized())
            .layout_of(ty)
            .map_err(|error| {
                ClosureProfileErrorV1::new(format!("closure constant layout failed: {error}"))
            })?;
        if layout.size.bytes() != 0
            || layout.fields.count() != 0
            || layout.is_uninhabited()
            || layout.align.abi.bytes() > MAX_ENVIRONMENT_ALIGNMENT
            || ty.needs_drop(tcx, TypingEnv::fully_monomorphized())
        {
            return Err(ClosureProfileErrorV1::new(
                "closure constant is not an inhabited empty environment",
            ));
        }
        charge_work(work, 3)?;
        Ok(Self {
            caller: canonical_function_identities_v1(tcx, caller).function(),
            block,
            argument,
            ty: rustc_type_identity_v1(tcx, ty),
        })
    }

    pub(crate) fn revalidate<'tcx>(
        &self,
        tcx: TyCtxt<'tcx>,
        caller: Instance<'tcx>,
        block: usize,
        argument: usize,
        work: &mut SourceClosureWorkV1,
    ) -> Result<(), ClosureProfileErrorV1> {
        if *self != Self::observe(tcx, caller, block, argument, work)? {
            return Err(ClosureProfileErrorV1::new(
                "closure constant occurrence changed",
            ));
        }
        Ok(())
    }
}

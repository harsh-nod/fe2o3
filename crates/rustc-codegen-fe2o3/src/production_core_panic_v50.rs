//! Exact reviewed-core panic call normalization, not a path-name whitelist.
//!
//! Only a pure immutable string literal is currently discharged. A local,
//! projected, moved, copied, formatted or unevaluated message is not erased.

use rustc_abi::Size;
use rustc_hir::Mutability;
use rustc_middle::{
    mir::{
        BasicBlock, Body, Const, ConstValue, Operand, TerminatorKind, UnwindAction,
        interpret::{AllocId, AllocRange, GlobalAlloc},
    },
    ty::{self, Instance, Ty, TyCtxt, TyKind, TypingEnv},
};
use rustc_target::callconv::FnAbi;

const MAX_LITERAL_BYTES: usize = 65_536;

#[derive(Debug)]
pub(crate) enum CorePanicErrorV50<E> {
    Work(E),
    Refused(&'static str),
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct CorePanicV50<'tcx> {
    caller: Instance<'tcx>,
    block: BasicBlock,
    callee: Instance<'tcx>,
    message_type: Ty<'tcx>,
    allocation: AllocId,
    bytes: &'tcx [u8],
    body: &'tcx Body<'tcx>,
    abi: &'tcx FnAbi<'tcx, Ty<'tcx>>,
}

impl<'tcx> CorePanicV50<'tcx> {
    pub(crate) fn instance(self) -> Instance<'tcx> {
        self.callee
    }

    pub(crate) fn body(self) -> &'tcx Body<'tcx> {
        self.body
    }

    pub(crate) fn abi(self) -> &'tcx FnAbi<'tcx, Ty<'tcx>> {
        self.abi
    }

    pub(crate) fn message_type(self) -> Ty<'tcx> {
        self.message_type
    }

    pub(crate) fn bytes(self) -> &'tcx [u8] {
        self.bytes
    }

    pub(crate) fn same_producers(self, other: Self) -> bool {
        self.caller == other.caller
            && self.block == other.block
            && self.callee == other.callee
            && self.message_type == other.message_type
            && self.allocation == other.allocation
            && self.bytes.len() == other.bytes.len()
            && std::ptr::eq(self.body, other.body)
            && std::ptr::eq(self.abi, other.abi)
    }
}

pub(crate) fn is_candidate(tcx: TyCtxt<'_>, operand: &Operand<'_>) -> bool {
    let Operand::Constant(constant) = operand else {
        return false;
    };
    let TyKind::FnDef(definition, _) = constant.const_.ty().kind() else {
        return false;
    };
    tcx.lang_items().panic_fn() == Some(*definition)
}

pub(crate) fn observe<'tcx, E>(
    tcx: TyCtxt<'tcx>,
    caller: Instance<'tcx>,
    body: &Body<'tcx>,
    block: BasicBlock,
    charge: &mut impl FnMut(usize) -> Result<(), E>,
) -> Result<Option<CorePanicV50<'tcx>>, CorePanicErrorV50<E>> {
    use CorePanicErrorV50::{Refused, Work};
    charge(1).map_err(Work)?;
    let Some(data) = body.basic_blocks.get(block) else {
        return Err(Refused("core panic original block is absent"));
    };
    let Some(terminator) = &data.terminator else {
        return Err(Refused("core panic original terminator is absent"));
    };
    let TerminatorKind::Call {
        func,
        args,
        target,
        unwind,
        destination,
        ..
    } = &terminator.kind
    else {
        return Ok(None);
    };
    if !is_candidate(tcx, func) {
        return Ok(None);
    }
    let callee = crate::production_raw_call_audit_v1::audit_raw_call_v1(tcx, caller, func, charge)
        .map_err(Work)?
        .map_err(|_| Refused("core panic original callee identity or ABI differs"))?;
    charge(3).map_err(Work)?;
    if tcx.lang_items().panic_fn() != Some(callee.def_id())
        || !crate::production_rustc_intrinsic_v1::is_reviewed_core_function_v1(tcx, callee)
        || !tcx.is_mir_available(callee.def_id())
        || args.len() != 1
        || target.is_some()
        || !matches!(unwind, UnwindAction::Unreachable)
        || !destination.projection.is_empty()
        || !body
            .local_decls
            .get(destination.local)
            .is_some_and(|local| local.ty.is_never())
    {
        return Err(Refused(
            "core panic requires its exact nonreturning call contract",
        ));
    }
    let signature = crate::rustc_semantic_plan_v1::source_signature_v1(tcx, callee)
        .map_err(|_| Refused("core panic source signature is absent"))?;
    let [message_type] = signature.inputs() else {
        return Err(Refused("core panic source argument census differs"));
    };
    if !signature.output().is_never()
        || !matches!(message_type.kind(), TyKind::Ref(_, pointee, Mutability::Not) if pointee.is_str())
    {
        return Err(Refused("core panic source signature differs"));
    }
    let Operand::Constant(message) = &args[0].node else {
        return Err(Refused(
            "core panic message requires retained argument evaluation",
        ));
    };
    let Const::Val(ConstValue::Slice { alloc_id, meta }, ty) = message.const_ else {
        return Err(Refused(
            "core panic message is not an original string literal",
        ));
    };
    if ty != *message_type {
        return Err(Refused("core panic original literal type differs"));
    }
    charge(2).map_err(Work)?;
    let GlobalAlloc::Memory(allocation) = tcx.global_alloc(alloc_id) else {
        return Err(Refused("core panic literal allocation is absent"));
    };
    let allocation = allocation.inner();
    let length = usize::try_from(meta)
        .map_err(|_| Refused("core panic literal length is not representable"))?;
    if allocation.len() > MAX_LITERAL_BYTES
        || length > allocation.len()
        || allocation.mutability != Mutability::Not
        || !allocation.provenance().ptrs().is_empty()
    {
        return Err(Refused("core panic literal allocation contract differs"));
    }
    charge(allocation.len()).map_err(Work)?;
    if allocation
        .init_mask()
        .is_range_initialized(AllocRange {
            start: Size::ZERO,
            size: allocation.size(),
        })
        .is_err()
    {
        return Err(Refused("core panic literal is not fully initialized"));
    }
    let bytes = allocation.inspect_with_uninit_and_ptr_outside_interpreter(0..length);
    std::str::from_utf8(bytes).map_err(|_| Refused("core panic literal is not valid UTF-8"))?;
    charge(1).map_err(Work)?;
    let abi = tcx
        .fn_abi_of_instance(
            TypingEnv::fully_monomorphized().as_query_input((callee, ty::List::empty())),
        )
        .map_err(|_| Refused("core panic actual ABI is absent"))?;
    Ok(Some(CorePanicV50 {
        caller,
        block,
        callee,
        message_type: ty,
        allocation: alloc_id,
        bytes,
        body: tcx.instance_mir(callee.def),
        abi,
    }))
}

#[cfg(test)]
#[path = "production_core_panic_v50_tests.rs"]
pub(crate) mod tests;

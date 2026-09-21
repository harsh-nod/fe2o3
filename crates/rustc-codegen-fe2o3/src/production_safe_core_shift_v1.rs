//! Closed normalizations of safe primitive methods in the reviewed pinned core.
//!
//! These are ordinary Item instances, not rustc intrinsics. Their semantics are
//! a pinned-core trust boundary, not a formal proof of the core implementation.
//! No unchecked method, panic path, or caller-supplied summary is admitted here.

use rustc_abi::{CanonAbi, ExternAbi};
use rustc_hir::{Safety, def::DefKind};
use rustc_middle::mir::Body;
use rustc_middle::ty::{
    self, Instance, InstanceKind, IntTy, Ty, TyCtxt, TyKind, TypingEnv, UintTy,
};
use rustc_target::callconv::FnAbi;

use crate::production_rustc_intrinsic_v1::ProductionRustcIntrinsicOperationV1;

#[path = "production_safe_core_shift_locals_v1.rs"]
pub(crate) mod locals;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum DirectionV1 {
    Left,
    Right,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct ContractV1<'a> {
    item: bool,
    core: bool,
    mir: bool,
    inherent_method: bool,
    generic_count: usize,
    method: &'a str,
    safe: bool,
    rust_abi: bool,
    variadic: bool,
    exact_self_and_result: bool,
    u32_count: bool,
    argument_count: usize,
    width: Option<u8>,
}

fn contract_v1(contract: ContractV1<'_>) -> Option<(DirectionV1, u8)> {
    let direction = match contract.method {
        "wrapping_shl" => DirectionV1::Left,
        "wrapping_shr" => DirectionV1::Right,
        _ => return None,
    };
    let width = contract.width?;
    (contract.item
        && contract.core
        && contract.mir
        && contract.inherent_method
        && contract.generic_count == 0
        && contract.safe
        && contract.rust_abi
        && !contract.variadic
        && contract.exact_self_and_result
        && contract.u32_count
        && contract.argument_count == 2
        && matches!(width, 8 | 16 | 32 | 64))
    .then_some((direction, width))
}

fn fixed_width_v1(ty: Ty<'_>) -> Option<u8> {
    match ty.kind() {
        TyKind::Int(IntTy::I8) | TyKind::Uint(UintTy::U8) => Some(8),
        TyKind::Int(IntTy::I16) | TyKind::Uint(UintTy::U16) => Some(16),
        TyKind::Int(IntTy::I32) | TyKind::Uint(UintTy::U32) => Some(32),
        TyKind::Int(IntTy::I64) | TyKind::Uint(UintTy::U64) => Some(64),
        _ => None,
    }
}

/// Inert live-session producers. Fields are private so a source recipe must
/// originate at the actual resolved core definition, body and ABI queries.
#[derive(Clone, Copy, Debug)]
pub(crate) struct SafeCoreShiftV1<'tcx> {
    instance: Instance<'tcx>,
    value_type: Ty<'tcx>,
    direction: DirectionV1,
    width: u8,
    body: &'tcx Body<'tcx>,
    abi: &'tcx FnAbi<'tcx, Ty<'tcx>>,
}

impl<'tcx> SafeCoreShiftV1<'tcx> {
    pub(crate) fn classify(
        tcx: TyCtxt<'tcx>,
        instance: Instance<'tcx>,
    ) -> Result<Option<Self>, &'static str> {
        if !matches!(instance.def, InstanceKind::Item(_))
            || !crate::production_rustc_intrinsic_v1::is_reviewed_core_function_v1(tcx, instance)
            || !instance.args.is_empty()
            || !matches!(tcx.def_kind(instance.def_id()), DefKind::AssocFn)
            || !tcx.is_mir_available(instance.def_id())
        {
            return Ok(None);
        }
        let name = tcx.item_name(instance.def_id());
        if !matches!(name.as_str(), "wrapping_shl" | "wrapping_shr") {
            return Ok(None);
        }
        let Some(associated) = tcx.opt_associated_item(instance.def_id()) else {
            return Ok(None);
        };
        let Some(implementation) = tcx.impl_of_assoc(instance.def_id()) else {
            return Ok(None);
        };
        let signature = tcx.instantiate_bound_regions_with_erased(
            tcx.fn_sig(instance.def_id())
                .instantiate(tcx, instance.args),
        );
        let value_type = tcx.type_of(implementation).instantiate_identity();
        let Some((direction, width)) = contract_v1(ContractV1 {
            item: true,
            core: implementation.krate == instance.def_id().krate,
            mir: true,
            inherent_method: associated.is_fn()
                && associated.is_method()
                && associated.impl_container(tcx) == Some(implementation)
                && !tcx.impl_is_of_trait(implementation),
            generic_count: tcx.generics_of(instance.def_id()).count(),
            method: name.as_str(),
            safe: signature.safety == Safety::Safe,
            rust_abi: signature.abi == ExternAbi::Rust,
            variadic: signature.c_variadic,
            exact_self_and_result: signature.inputs().first() == Some(&value_type)
                && signature.output() == value_type,
            u32_count: signature.inputs().get(1) == Some(&tcx.types.u32),
            argument_count: signature.inputs().len(),
            width: fixed_width_v1(value_type),
        }) else {
            return Err("reviewed core wrapping shift has a different method contract");
        };
        let abi = tcx
            .fn_abi_of_instance(
                TypingEnv::fully_monomorphized().as_query_input((instance, ty::List::empty())),
            )
            .map_err(|_| "reviewed core wrapping shift ABI query failed")?;
        if abi.conv != CanonAbi::Rust
            || abi.c_variadic
            || abi.fixed_count != 2
            || abi.args.len() != 2
            || abi.args[0].layout.ty != value_type
            || abi.args[1].layout.ty != tcx.types.u32
            || abi.ret.layout.ty != value_type
        {
            return Err("reviewed core wrapping shift has a different actual ABI");
        }
        Ok(Some(Self {
            instance,
            value_type,
            direction,
            width,
            body: tcx.instance_mir(instance.def),
            abi,
        }))
    }

    pub(crate) fn value_type(self) -> Ty<'tcx> {
        self.value_type
    }
    pub(crate) fn direction(self) -> DirectionV1 {
        self.direction
    }
    pub(crate) fn width(self) -> u8 {
        self.width
    }
    pub(crate) fn body(self) -> &'tcx Body<'tcx> {
        self.body
    }
    pub(crate) fn abi(self) -> &'tcx FnAbi<'tcx, Ty<'tcx>> {
        self.abi
    }

    pub(crate) fn same_producers(self, other: Self) -> bool {
        self.instance == other.instance
            && self.value_type == other.value_type
            && self.direction == other.direction
            && self.width == other.width
            && std::ptr::eq(self.body, other.body)
            && std::ptr::eq(self.abi, other.abi)
    }
}

/// Tagged source-call recipe: safe Item summaries cannot masquerade as an
/// intrinsic classification. Atomic recipe payloads keep their existing tags.
#[derive(Clone, Copy, Debug)]
pub(crate) enum NormalizedCallV1<'tcx> {
    Rustc(ProductionRustcIntrinsicOperationV1),
    SafeCoreShift(SafeCoreShiftV1<'tcx>),
    CheckedPrimitiveFrom(crate::production_primitive_from_v1::CheckedPrimitiveFromV1<'tcx>),
}

impl NormalizedCallV1<'_> {
    pub(crate) fn statement_count(self) -> usize {
        match self {
            Self::Rustc(_) => 1,
            Self::SafeCoreShift(_) => 3,
            Self::CheckedPrimitiveFrom(_) => 1,
        }
    }

    pub(crate) fn operation_tag(self) -> u8 {
        match self {
            Self::Rustc(operation) => operation.operation_tag(),
            Self::SafeCoreShift(shift) => match shift.direction() {
                DirectionV1::Left => 4,
                DirectionV1::Right => 5,
            },
            Self::CheckedPrimitiveFrom(_) => 7,
        }
    }
}

#[cfg(test)]
#[path = "production_safe_core_shift_v1_tests.rs"]
mod tests;

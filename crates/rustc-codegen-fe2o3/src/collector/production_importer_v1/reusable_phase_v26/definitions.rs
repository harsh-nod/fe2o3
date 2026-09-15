//! Live reviewed definitions for the phase source adapter. This module does
//! not mint SSA values and does not by itself authenticate a complete protocol.
use crate::collector::production_importer_v1::{
    rust_exact_reviewed_adt_arguments_v1, rust_execution_brand_v1,
};
use crate::trusted_device_items;
use rustc_hir::{Mutability, Safety};
use rustc_middle::ty::{
    self, Instance, InstanceKind, Ty, TyCtxt, TyKind, TypeVisitableExt, TypingEnv,
};
use rustc_target::callconv::PassMode;

#[path = "raw_body.rs"]
mod raw_body;
pub(super) use raw_body::BodyRecipe;

pub(super) fn normalize<'tcx, T: rustc_middle::ty::TypeFoldable<TyCtxt<'tcx>>>(
    tcx: TyCtxt<'tcx>,
    instance: Instance<'tcx>,
    value: T,
) -> Result<T> {
    raw_body::normalize(tcx, instance, value)
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum Role {
    OwnerConvert,
    Issue,
    WithPhase,
    Bind,
    Finish,
}

impl Role {
    fn path(self) -> &'static str {
        match self {
            Self::OwnerConvert => "fe2o3_device::execution::WorkgroupCapability::into_reusable",
            Self::Issue => "fe2o3_device::execution::ReusableWorkgroup::issue_phase",
            Self::WithPhase => "fe2o3_device::execution::ReusableWorkgroup::with_phase",
            Self::Bind => "fe2o3_device::execution::WorkgroupCapability::bind_reusable_lds",
            Self::Finish => "fe2o3_device::execution::WorkgroupCapability::finish_reusable_phase",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum Error {
    Work,
    Definition,
    Signature,
    Nominal,
    Abi,
    Body(&'static str),
}
type Result<T> = std::result::Result<T, Error>;

pub(super) fn charge(work: &mut usize, amount: usize) -> Result<()> {
    *work = work.checked_sub(amount).ok_or(Error::Work)?;
    Ok(())
}

pub(super) fn classify<'tcx>(
    tcx: TyCtxt<'tcx>,
    instance: Instance<'tcx>,
    work: &mut usize,
) -> Result<Option<Role>> {
    for role in [
        Role::OwnerConvert,
        Role::Issue,
        Role::WithPhase,
        Role::Bind,
        Role::Finish,
    ] {
        charge(work, 1)?;
        if trusted_device_items::is_exact_reviewed_provider_definition_v1(
            tcx,
            instance.def_id(),
            role.path(),
        ) {
            return Ok(Some(role));
        }
    }
    Ok(None)
}

fn arguments<'tcx>(
    tcx: TyCtxt<'tcx>,
    value: Ty<'tcx>,
    path: &'static str,
    count: usize,
) -> Result<ty::GenericArgsRef<'tcx>> {
    let args = rust_exact_reviewed_adt_arguments_v1(tcx, value, path).ok_or(Error::Nominal)?;
    if args.len() != count {
        return Err(Error::Nominal);
    }
    Ok(args)
}

fn reference<'tcx>(value: Ty<'tcx>, kind: Mutability) -> Result<Ty<'tcx>> {
    match *value.kind() {
        TyKind::Ref(_, pointee, actual) if actual == kind => Ok(pointee),
        _ => Err(Error::Nominal),
    }
}

fn workgroup<'tcx>(tcx: TyCtxt<'tcx>, value: Ty<'tcx>) -> Result<ty::GenericArgsRef<'tcx>> {
    let args = arguments(
        tcx,
        value,
        "fe2o3_device::execution::WorkgroupCapability",
        3,
    )?;
    if args[0].as_region().is_none() || args[1].as_type().is_none() || args[2].as_type().is_none() {
        return Err(Error::Nominal);
    }
    Ok(args)
}

fn owner<'tcx>(tcx: TyCtxt<'tcx>, value: Ty<'tcx>) -> Result<ty::GenericArgsRef<'tcx>> {
    let args = arguments(tcx, value, "fe2o3_device::execution::ReusableWorkgroup", 2)?;
    if args[0].as_region().is_none() || args[1].as_type().is_none() {
        return Err(Error::Nominal);
    }
    Ok(args)
}

#[derive(Clone, Copy)]
pub(super) struct PhaseTypes<'tcx> {
    pub workgroup: Ty<'tcx>,
    pub root: Ty<'tcx>,
    pub brand: Ty<'tcx>,
    pub epoch: Ty<'tcx>,
    pub dynamic_epoch: Ty<'tcx>,
}

fn phase<'tcx>(
    tcx: TyCtxt<'tcx>,
    value: Ty<'tcx>,
    initial: bool,
    work: &mut usize,
) -> Result<PhaseTypes<'tcx>> {
    let wg = workgroup(tcx, value)?;
    let brand = wg[1].as_type().ok_or(Error::Nominal)?;
    let b = arguments(
        tcx,
        brand,
        "fe2o3_device::execution::ReusableWorkgroupBrand",
        2,
    )?;
    if b[0].as_region().is_none() {
        return Err(Error::Nominal);
    }
    let root = b[1].as_type().ok_or(Error::Nominal)?;
    if rust_execution_brand_v1(tcx, root).is_none() {
        return Err(Error::Nominal);
    }
    let epoch = wg[2].as_type().ok_or(Error::Nominal)?;
    let mut cursor = epoch;
    // A source-type traversal ceiling, not a runtime phase generation counter.
    for depth in 0..64 {
        charge(work, 1)?;
        if let Ok(e) = arguments(tcx, cursor, "fe2o3_device::execution::DynamicPhaseEpoch", 1) {
            if e[0] != wg[0] || (initial && depth != 0) {
                return Err(Error::Nominal);
            }
            return Ok(PhaseTypes {
                workgroup: value,
                root,
                brand,
                epoch,
                dynamic_epoch: cursor,
            });
        }
        let next = arguments(tcx, cursor, "fe2o3_device::execution::NextEpoch", 1)?;
        cursor = next[0].as_type().ok_or(Error::Nominal)?;
    }
    Err(Error::Nominal)
}

fn completion<'tcx>(tcx: TyCtxt<'tcx>, value: Ty<'tcx>, phase: PhaseTypes<'tcx>) -> Result<()> {
    let args = arguments(
        tcx,
        value,
        "fe2o3_device::execution::ReusablePhaseCompletion",
        3,
    )?;
    let wg = workgroup(tcx, phase.workgroup)?;
    let b = arguments(
        tcx,
        phase.brand,
        "fe2o3_device::execution::ReusableWorkgroupBrand",
        2,
    )?;
    if args[0] != wg[0] || args[1] != b[0] || args[2] != b[1] {
        return Err(Error::Nominal);
    }
    Ok(())
}

#[derive(Clone, Copy)]
pub(super) enum Types<'tcx> {
    OwnerConvert {
        workgroup: Ty<'tcx>,
        owner: Ty<'tcx>,
        root: Ty<'tcx>,
        epoch: Ty<'tcx>,
    },
    Issue {
        owner_reference: Ty<'tcx>,
        owner: Ty<'tcx>,
        phase: PhaseTypes<'tcx>,
    },
    WithPhase {
        owner_reference: Ty<'tcx>,
        owner: Ty<'tcx>,
        closure: Ty<'tcx>,
        result: Ty<'tcx>,
        phase: PhaseTypes<'tcx>,
        call_tuple: Ty<'tcx>,
        result_pair: Ty<'tcx>,
        completion: Ty<'tcx>,
    },
    Bind {
        phase_reference: Ty<'tcx>,
        storage_reference: Ty<'tcx>,
        storage: Ty<'tcx>,
        phase: PhaseTypes<'tcx>,
        lease: Ty<'tcx>,
        element: Ty<'tcx>,
        elements: u64,
    },
    Finish {
        phase: PhaseTypes<'tcx>,
        advanced: Ty<'tcx>,
        completion: Ty<'tcx>,
    },
}

/// A checked original definition only. Complete incoming-site/HIR/expanded
/// custody is supplied by the consuming source plan, never this constructor.
pub(super) struct Definition<'tcx> {
    pub(super) instance: Instance<'tcx>,
    pub(super) role: Role,
    pub(super) body: &'tcx rustc_middle::mir::Body<'tcx>,
    pub(super) recipe: raw_body::BodyRecipe<'tcx>,
    pub(super) types: Types<'tcx>,
}

impl<'tcx> Definition<'tcx> {
    pub(super) fn observe(
        tcx: TyCtxt<'tcx>,
        instance: Instance<'tcx>,
        role: Role,
        work: &mut usize,
    ) -> Result<Self> {
        charge(work, 1)?;
        if !matches!(instance.def, InstanceKind::Item(_))
            || !tcx.is_mir_available(instance.def_id())
            || !trusted_device_items::is_exact_reviewed_provider_definition_v1(
                tcx,
                instance.def_id(),
                role.path(),
            )
            || !trusted_device_items::authenticate_reviewed_safe_external_helper_v1(
                tcx,
                instance.def_id(),
            )
            .map_err(|_| Error::Definition)?
            || instance.args.len() != tcx.generics_of(instance.def_id()).count()
        {
            return Err(Error::Definition);
        }
        let body = tcx.instance_mir(instance.def);
        let signature = tcx.instantiate_bound_regions_with_erased(
            tcx.fn_sig(instance.def_id())
                .instantiate(tcx, instance.args),
        );
        let signature = tcx
            .try_normalize_erasing_regions(TypingEnv::fully_monomorphized(), signature)
            .map_err(|_| Error::Signature)?;
        let count = match role {
            Role::OwnerConvert | Role::Issue | Role::Finish => 1,
            Role::WithPhase | Role::Bind => 2,
        };
        if signature.safety != Safety::Safe
            || signature.abi != rustc_abi::ExternAbi::Rust
            || signature.c_variadic
            || signature.has_non_region_param()
            || signature.has_infer()
            || signature.has_aliases()
            || signature.has_escaping_bound_vars()
            || signature.inputs().len() != count
            || body.arg_count != count
            || raw_body::normalize(tcx, instance, body.return_ty())? != signature.output()
        {
            return Err(Error::Signature);
        }
        for (local, input) in body.args_iter().zip(signature.inputs()) {
            charge(work, 1)?;
            if raw_body::local_type(tcx, instance, body, local)? != *input {
                return Err(Error::Signature);
            }
        }
        let abi = tcx
            .fn_abi_of_instance(
                TypingEnv::fully_monomorphized().as_query_input((instance, ty::List::empty())),
            )
            .map_err(|_| Error::Abi)?;
        if abi.can_unwind || abi.args.len() != count {
            return Err(Error::Abi);
        }
        let recipe = raw_body::observe(tcx, instance, body, role, work)?;
        let input = signature.inputs()[0];
        let output = signature.output();
        let types = match (role, recipe) {
            (Role::OwnerConvert, raw_body::BodyRecipe::OwnerConvert) => {
                let w = workgroup(tcx, input)?;
                let o = owner(tcx, output)?;
                if o[0] != w[0]
                    || o[1] != w[1]
                    || !matches!(abi.args[0].mode, PassMode::Pair(..))
                    || !matches!(abi.ret.mode, PassMode::Pair(..))
                {
                    return Err(Error::Abi);
                }
                let root = w[1].as_type().ok_or(Error::Nominal)?;
                if rust_execution_brand_v1(tcx, root).is_none() {
                    return Err(Error::Nominal);
                }
                Types::OwnerConvert {
                    workgroup: input,
                    owner: output,
                    root,
                    epoch: w[2].as_type().ok_or(Error::Nominal)?,
                }
            }
            (Role::Issue, raw_body::BodyRecipe::Issue) => {
                let owned = reference(input, Mutability::Mut)?;
                let o = owner(tcx, owned)?;
                let p = phase(tcx, output, true, work)?;
                let b = arguments(
                    tcx,
                    p.brand,
                    "fe2o3_device::execution::ReusableWorkgroupBrand",
                    2,
                )?;
                if b != o
                    || !matches!(abi.args[0].mode, PassMode::Direct(..))
                    || !matches!(abi.ret.mode, PassMode::Pair(..))
                {
                    return Err(Error::Abi);
                }
                Types::Issue {
                    owner_reference: input,
                    owner: owned,
                    phase: p,
                }
            }
            (Role::Bind, raw_body::BodyRecipe::Bind) => {
                let p = phase(tcx, reference(input, Mutability::Not)?, true, work)?;
                let storage_reference = signature.inputs()[1];
                let storage = reference(storage_reference, Mutability::Mut)?;
                let s = arguments(
                    tcx,
                    storage,
                    "fe2o3_device::execution::ReusableWorkgroupLds",
                    4,
                )?;
                let l = arguments(tcx, output, "fe2o3_device::execution::WorkgroupLds", 6)?;
                let w = workgroup(tcx, p.workgroup)?;
                let b = arguments(
                    tcx,
                    p.brand,
                    "fe2o3_device::execution::ReusableWorkgroupBrand",
                    2,
                )?;
                if s[0] != b[0]
                    || s[3].as_type() != Some(p.root)
                    || l[0] != w[0]
                    || l[1] != s[1]
                    || l[2] != s[2]
                    || l[4].as_type() != Some(p.brand)
                    || l[5].as_type() != Some(p.epoch)
                {
                    return Err(Error::Nominal);
                }
                arguments(
                    tcx,
                    l[3].as_type().ok_or(Error::Nominal)?,
                    "fe2o3_device::execution::WorkgroupLdsUninitialized",
                    0,
                )?;
                let element = s[1].as_type().ok_or(Error::Nominal)?;
                let elements = s[2]
                    .as_const()
                    .and_then(|n| n.try_to_target_usize(tcx))
                    .filter(|n| *n != 0)
                    .ok_or(Error::Nominal)?;
                if !abi
                    .args
                    .iter()
                    .all(|a| matches!(a.mode, PassMode::Direct(..)))
                    || !matches!(abi.ret.mode, PassMode::Ignore)
                {
                    return Err(Error::Abi);
                }
                Types::Bind {
                    phase_reference: input,
                    storage_reference,
                    storage,
                    phase: p,
                    lease: output,
                    element,
                    elements,
                }
            }
            (
                Role::Finish,
                raw_body::BodyRecipe::Finish {
                    barrier, advanced, ..
                },
            ) => {
                let p = phase(tcx, input, false, work)?;
                completion(tcx, output, p)?;
                let advanced = raw_body::local_type(tcx, instance, body, advanced)?;
                let a = workgroup(tcx, advanced)?;
                let w = workgroup(tcx, input)?;
                let next = arguments(
                    tcx,
                    a[2].as_type().ok_or(Error::Nominal)?,
                    "fe2o3_device::execution::NextEpoch",
                    1,
                )?;
                if a[0] != w[0]
                    || a[1] != w[1]
                    || next[0] != w[2]
                    || trusted_device_items::classify(tcx, barrier.def_id())
                        != Some(trusted_device_items::TrustedDeviceItem::ExecutionWorkgroupBarrier)
                    || !matches!(abi.args[0].mode, PassMode::Pair(..))
                    || !matches!(abi.ret.mode, PassMode::Ignore)
                {
                    return Err(Error::Abi);
                }
                // The source plan also reuses the existing terminal validator
                // with its actual root; this nominal result is not a barrier proof.
                Types::Finish {
                    phase: p,
                    advanced,
                    completion: output,
                }
            }
            (
                Role::WithPhase,
                raw_body::BodyRecipe::WithPhase {
                    issue,
                    invoke,
                    drop,
                    phase: phase_local,
                    tuple,
                    result_pair,
                    ..
                },
            ) => {
                let owned = reference(input, Mutability::Mut)?;
                let o = owner(tcx, owned)?;
                let closure = signature.inputs()[1];
                if !matches!(closure.kind(), TyKind::Closure(..))
                    || !matches!(abi.args[0].mode, PassMode::Direct(..))
                {
                    return Err(Error::Nominal);
                }
                let p = phase(
                    tcx,
                    raw_body::local_type(tcx, instance, body, phase_local)?,
                    true,
                    work,
                )?;
                let b = arguments(
                    tcx,
                    p.brand,
                    "fe2o3_device::execution::ReusableWorkgroupBrand",
                    2,
                )?;
                if b != o
                    || classify(tcx, issue, work)? != Some(Role::Issue)
                    || !trusted_device_items::authenticate_reviewed_safe_core_mem_drop_helper_v1(
                        tcx, drop,
                    )
                {
                    return Err(Error::Definition);
                }
                let call_tuple = raw_body::local_type(tcx, instance, body, tuple)?;
                let result_pair = raw_body::local_type(tcx, instance, body, result_pair)?;
                let TyKind::Tuple(inputs) = call_tuple.kind() else {
                    return Err(Error::Nominal);
                };
                let TyKind::Tuple(outputs) = result_pair.kind() else {
                    return Err(Error::Nominal);
                };
                if inputs.as_slice() != [p.workgroup] || outputs.len() != 2 || outputs[1] != output
                {
                    return Err(Error::Nominal);
                }
                completion(tcx, outputs[0], p)?;
                let TyKind::Closure(closure_id, _) = *closure.kind() else {
                    unreachable!()
                };
                if invoke.def_id() != closure_id {
                    return Err(Error::Definition);
                }
                Types::WithPhase {
                    owner_reference: input,
                    owner: owned,
                    closure,
                    result: output,
                    phase: p,
                    call_tuple,
                    result_pair,
                    completion: outputs[0],
                }
            }
            _ => return Err(Error::Definition),
        };
        Ok(Self {
            instance,
            role,
            body,
            recipe,
            types,
        })
    }
}

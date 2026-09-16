//! Bounded closure admission from the live rustc target and monomorphized MIR.
//!
//! This profile recognizes concrete rustc closure types, records their
//! physical capture layout, and rejects uses outside the supported profile.
//! The observation checks collection/import continuity, not closure transport,
//! semantic equivalence, or authorization to lower the body.

use crate::rust_type_layout_general::{TypeLayoutFacts, TypeLayoutKind, extract_capture_layout};
use crate::rustc_semantic_adapter_v1::{
    canonical_function_identities_v1, canonical_target_layout_v1, rustc_mir_body_sha256_v1,
};
use crate::rustc_semantic_plan_v1::SourceClosureWorkV1;
use crate::semantic_layout_bridge::rustc_semantic_layout_target_v1;
use fe2o3_mir_model::semantic_mir_v1::{SemanticFunctionIdentityV1, SemanticLayoutIdentityV1};
use rustc_hir::Mutability;
use rustc_hir::def_id::DefId;
use rustc_middle::mir::{
    AggregateKind, Body, InlineAsmOperand, Local, NonDivergingIntrinsic, Operand, Place, Rvalue,
    StatementKind, TerminatorKind,
};
use rustc_middle::ty::layout::{LayoutCx, LayoutOf};
use rustc_middle::ty::{
    ClosureKind, EarlyBinder, Instance, InstanceKind, Ty, TyCtxt, TyKind, TypingEnv,
};
use std::collections::{BTreeMap, BTreeSet, HashSet};
use std::fmt;

#[path = "closure_profile_v1/alias_flow_v1.rs"]
mod alias_flow_v1;
#[path = "closure_profile_v1/once_shim_v1.rs"]
mod once_shim_v1;
#[path = "closure_profile_v1/uses_v1.rs"]
mod uses_v1;
pub(crate) use once_shim_v1::{authenticate_once_shim_v1, is_shim_receiver_call_v1};

const MAX_CLOSURES: usize = 8;
const MAX_CAPTURES: usize = 8;
const MAX_ENVIRONMENT_BYTES: u64 = 256;
const MAX_ENVIRONMENT_ALIGNMENT: u64 = 16;
const MAX_CALL_ARGUMENTS: usize = 8;
const MAX_STATIC_CALLS: usize = 64;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ClosureOriginPolicyV1 {
    #[cfg(test)]
    HostArgument,
    #[cfg(test)]
    DeviceInternal,
    Either,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ClosureOriginV1 {
    HostArgument,
    DeviceInternal,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ClosureCallKindV1 {
    Fn,
    FnMut,
    FnOnce,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ClosureCaptureModeV1 {
    ByValue,
    SharedReference,
    MutableReference,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct ClosureCaptureLayoutV1 {
    pub(crate) source_index: usize,
    pub(crate) memory_index: usize,
    pub(crate) offset_bytes: u64,
    pub(crate) mode: ClosureCaptureModeV1,
    pub(crate) layout: TypeLayoutFacts,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct ClosureEnvironmentV1 {
    pub(crate) local: usize,
    pub(crate) origin: ClosureOriginV1,
    pub(crate) call_kind: ClosureCallKindV1,
    pub(crate) definition_hash: [u8; 16],
    pub(crate) size_bytes: u64,
    pub(crate) alignment_bytes: u64,
    pub(crate) captures: Vec<ClosureCaptureLayoutV1>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct StaticClosureCallV1 {
    pub(crate) block: usize,
    pub(crate) closure_local: usize,
    pub(crate) call_kind: ClosureCallKindV1,
    pub(crate) argument_count: usize,
    pub(crate) target_definition_hash: [u8; 16],
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct ClosureForwardV1 {
    pub(crate) block: usize,
    pub(crate) argument: usize,
    pub(crate) closure_local: usize,
}

/// Layout and uses observed before caller provenance is resolved. This cannot
/// become an admission without an origin for every exact environment local.
pub(crate) struct RawClosureObservationV1 {
    environments: Vec<ClosureEnvironmentV1>,
    calls: Vec<StaticClosureCallV1>,
    forwards: Vec<ClosureForwardV1>,
    observation: CompilerClosureObservationV2,
}

impl RawClosureObservationV1 {
    pub(crate) fn environments(&self) -> &[ClosureEnvironmentV1] {
        &self.environments
    }

    pub(crate) fn forwards(&self) -> &[ClosureForwardV1] {
        &self.forwards
    }

    pub(crate) fn calls(&self) -> &[StaticClosureCallV1] {
        &self.calls
    }

    pub(crate) fn admit(
        mut self,
        origins: &BTreeMap<usize, ClosureOriginV1>,
        policy: ClosureOriginPolicyV1,
        work: &mut SourceClosureWorkV1,
    ) -> Result<BoundedClosureAdmissionV2, ClosureProfileErrorV1> {
        if origins.len() != self.environments.len() {
            return Err(ClosureProfileErrorV1::new("closure origin roster changed"));
        }
        for environment in &mut self.environments {
            charge_work(work, 1)?;
            let origin = *origins
                .get(&environment.local)
                .ok_or_else(|| ClosureProfileErrorV1::new("closure origin is unresolved"))?;
            if environment.origin == ClosureOriginV1::DeviceInternal
                && origin != ClosureOriginV1::DeviceInternal
            {
                return Err(ClosureProfileErrorV1::new(
                    "closure aggregate origin changed",
                ));
            }
            require_origin(policy, origin)?;
            for capture in &environment.captures {
                validate_capture_layout_v1(&capture.layout, origin, work)?;
            }
            environment.origin = origin;
        }
        Ok(BoundedClosureAdmissionV2 {
            environments: self.environments,
            calls: self.calls,
            observation: self.observation,
        })
    }
}

/// The existing compiler identity axes under which bounded admission succeeded.
/// No independent lowering recipe or proof identity is introduced.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct CompilerClosureObservationV2 {
    function: SemanticFunctionIdentityV1,
    mir_body: [u8; 32],
    target: SemanticLayoutIdentityV1,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct BoundedClosureAdmissionV2 {
    environments: Vec<ClosureEnvironmentV1>,
    calls: Vec<StaticClosureCallV1>,
    observation: CompilerClosureObservationV2,
}

impl BoundedClosureAdmissionV2 {
    pub(crate) fn environments(&self) -> &[ClosureEnvironmentV1] {
        &self.environments
    }

    pub(crate) fn calls(&self) -> &[StaticClosureCallV1] {
        &self.calls
    }

    pub(crate) fn into_observation(self) -> CompilerClosureObservationV2 {
        self.observation
    }
}

#[cfg(test)]
pub(crate) fn observe_closures_v2<'tcx>(
    tcx: TyCtxt<'tcx>,
    instance: Instance<'tcx>,
) -> Result<Option<BoundedClosureAdmissionV2>, ClosureProfileErrorV1> {
    if !contains_concrete_closure_v1(tcx, instance)? {
        return Ok(None);
    }
    analyze_bounded_closures_v2(tcx, instance, ClosureOriginPolicyV1::Either).map(Some)
}

#[cfg(test)]
pub(crate) fn revalidate_closure_observation_v2<'tcx>(
    tcx: TyCtxt<'tcx>,
    instance: Instance<'tcx>,
    retained: Option<&CompilerClosureObservationV2>,
) -> Result<(), ClosureProfileErrorV1> {
    let observed =
        observe_closures_v2(tcx, instance)?.map(BoundedClosureAdmissionV2::into_observation);
    if retained != observed.as_ref() {
        return Err(ClosureProfileErrorV1::new(
            "collection/import closure presence, function, MIR body, or live target changed",
        ));
    }
    Ok(())
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct ClosureProfileErrorV1(String);

impl ClosureProfileErrorV1 {
    pub(crate) fn new(reason: impl Into<String>) -> Self {
        Self(reason.into())
    }
}

impl fmt::Display for ClosureProfileErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "bounded closure profile rejected MIR: {}",
            self.0
        )
    }
}

impl std::error::Error for ClosureProfileErrorV1 {}

#[cfg(test)]
pub(crate) fn analyze_bounded_closures_v2<'tcx>(
    tcx: TyCtxt<'tcx>,
    instance: Instance<'tcx>,
    policy: ClosureOriginPolicyV1,
) -> Result<BoundedClosureAdmissionV2, ClosureProfileErrorV1> {
    let mut work = SourceClosureWorkV1::default();
    let raw =
        observe_raw_closures_v1(tcx, instance, &BTreeMap::new(), &mut work)?.ok_or_else(|| {
            ClosureProfileErrorV1::new(
                "the requested closure profile contains no concrete closure environment",
            )
        })?;
    let origins = raw
        .environments
        .iter()
        .map(|env| (env.local, env.origin))
        .collect();
    raw.admit(&origins, policy, &mut work)
}

pub(crate) fn observe_raw_closures_v1<'tcx>(
    tcx: TyCtxt<'tcx>,
    instance: Instance<'tcx>,
    forwarding: &BTreeMap<usize, BTreeSet<usize>>,
    work: &mut SourceClosureWorkV1,
) -> Result<Option<RawClosureObservationV1>, ClosureProfileErrorV1> {
    let body = tcx.instance_mir(instance.def);
    charge_work(work, body.local_decls.len())?;
    reject_dynamic_types(tcx, instance, body, work)?;
    if !contains_concrete_closure_v1(tcx, instance)? {
        return Ok(None);
    }
    let target = rustc_semantic_layout_target_v1(tcx).map_err(|error| {
        ClosureProfileErrorV1::new(format!(
            "live closure layout target is unavailable: {error}"
        ))
    })?;
    if tcx.sess.target.pointer_width != 64 {
        return Err(ClosureProfileErrorV1::new(
            "the bounded closure profile requires a 64-bit compiler target",
        ));
    }
    let own_receiver = own_closure_receiver_v1(tcx, instance, body)?;
    let creations = closure_creations(body, work)?;
    charge_work(work, body.local_decls.len())?;
    let typed_closure_locals = body
        .local_decls
        .iter_enumerated()
        .filter(|(local, _)| Some(*local) != own_receiver)
        .filter_map(|(local, declaration)| {
            normalized_ty(tcx, instance, declaration.ty, "closure local")
                .ok()
                .filter(|ty| matches!(ty.kind(), TyKind::Closure(..)))
                .map(|_| local)
        })
        .collect::<BTreeSet<_>>();
    let value_aliases = alias_flow_v1::value_aliases(body, &typed_closure_locals, &mut |amount| {
        charge_work(work, amount)
    })?;
    let mut environments = Vec::new();
    let mut closure_locals = BTreeSet::new();

    for (local, declaration) in body.local_decls.iter_enumerated() {
        charge_work(work, 1)?;
        if Some(local) == own_receiver {
            continue;
        }
        let ty = normalized_ty(tcx, instance, declaration.ty, "closure local")?;
        let TyKind::Closure(def_id, args) = ty.kind() else {
            continue;
        };
        if local.as_usize() == 0 {
            return Err(ClosureProfileErrorV1::new(
                "returning a closure escapes its environment",
            ));
        }
        let origin = if local.as_usize() != 0 && local.as_usize() <= body.arg_count {
            ClosureOriginV1::HostArgument
        } else if creations.contains_key(&local) {
            ClosureOriginV1::DeviceInternal
        } else if value_aliases.contains_key(&local) {
            continue;
        } else {
            return Err(ClosureProfileErrorV1::new(format!(
                "closure local{} has neither a host argument nor one direct closure aggregate; closure escapes and call-result reconstruction are forbidden",
                local.as_usize()
            )));
        };
        if environments.len() == MAX_CLOSURES {
            return Err(ClosureProfileErrorV1::new(format!(
                "closure count exceeds {MAX_CLOSURES}"
            )));
        }
        let call_kind = closure_kind(args.as_closure().kind_ty().to_opt_closure_kind())?;
        let upvars = args.as_closure().upvar_tys();
        if upvars.len() > MAX_CAPTURES {
            return Err(ClosureProfileErrorV1::new(format!(
                "closure local{} capture count {} exceeds {MAX_CAPTURES}",
                local.as_usize(),
                upvars.len()
            )));
        }
        if let Some((creation_def_id, operand_count)) = creations.get(&local)
            && (creation_def_id != def_id || *operand_count != upvars.len())
        {
            return Err(ClosureProfileErrorV1::new(format!(
                "closure local{} aggregate identity or capture arity disagrees with rustc type",
                local.as_usize()
            )));
        }

        let layout_cx = LayoutCx::new(tcx, TypingEnv::fully_monomorphized());
        let layout = layout_cx.layout_of(ty).map_err(|error| {
            ClosureProfileErrorV1::new(format!("closure environment layout failed: {error}"))
        })?;
        let size_bytes = layout.size.bytes();
        let alignment_bytes = layout.align.abi.bytes();
        if size_bytes > MAX_ENVIRONMENT_BYTES || alignment_bytes > MAX_ENVIRONMENT_ALIGNMENT {
            return Err(ClosureProfileErrorV1::new(format!(
                "closure local{} environment is {size_bytes} bytes aligned to {alignment_bytes}; limits are {MAX_ENVIRONMENT_BYTES}/{MAX_ENVIRONMENT_ALIGNMENT}",
                local.as_usize()
            )));
        }
        if layout.fields.count() != upvars.len() {
            return Err(ClosureProfileErrorV1::new(
                "rustc closure layout field count disagrees with capture count",
            ));
        }

        let mut captures = Vec::with_capacity(upvars.len());
        for (source_index, raw_ty) in upvars.iter().enumerate() {
            charge_work(work, 1)?;
            let capture_ty = normalized_ty(tcx, instance, raw_ty, "closure capture")?;
            if capture_ty.needs_drop(tcx, TypingEnv::fully_monomorphized()) {
                return Err(ClosureProfileErrorV1::new(format!(
                    "capture {source_index} requires drop"
                )));
            }
            if matches!(
                capture_ty.kind(),
                TyKind::Closure(..) | TyKind::CoroutineClosure(..)
            ) {
                return Err(ClosureProfileErrorV1::new(
                    "nested closure captures are outside the bounded profile",
                ));
            }
            let mode = match capture_ty.kind() {
                TyKind::Ref(_, _, Mutability::Not) => ClosureCaptureModeV1::SharedReference,
                TyKind::Ref(_, _, Mutability::Mut) => ClosureCaptureModeV1::MutableReference,
                TyKind::RawPtr(..) => {
                    return Err(ClosureProfileErrorV1::new(
                        "raw-pointer captures have no allocation authority",
                    ));
                }
                _ => ClosureCaptureModeV1::ByValue,
            };
            let facts = extract_capture_layout(tcx, capture_ty).map_err(|error| {
                ClosureProfileErrorV1::new(format!(
                    "capture {source_index} has unsupported physical layout: {error}"
                ))
            })?;
            validate_capture_layout_v1(&facts, ClosureOriginV1::DeviceInternal, work)?;
            let field = layout.field(&layout_cx, source_index);
            if field.size.bytes() != facts.size_bytes
                || field.align.abi.bytes() != facts.abi_alignment_bytes
            {
                return Err(ClosureProfileErrorV1::new(
                    "capture layout disagrees with its projected closure field",
                ));
            }
            captures.push(ClosureCaptureLayoutV1 {
                source_index,
                memory_index: layout
                    .fields
                    .index_by_increasing_offset()
                    .position(|index| index == source_index)
                    .expect("rustc field order is a permutation"),
                offset_bytes: layout.fields.offset(source_index).bytes(),
                mode,
                layout: facts,
            });
        }
        closure_locals.insert(local);
        environments.push(ClosureEnvironmentV1 {
            local: local.as_usize(),
            origin,
            call_kind,
            definition_hash: tcx.def_path_hash(*def_id).0.to_le_bytes(),
            size_bytes,
            alignment_bytes,
            captures,
        });
    }

    if environments.is_empty() {
        return Err(ClosureProfileErrorV1::new(
            "the requested closure profile contains no concrete closure environment",
        ));
    }
    environments.sort_by_key(|environment| environment.local);
    let aliases =
        alias_flow_v1::reference_aliases(body, &closure_locals, &value_aliases, &mut |amount| {
            charge_work(work, amount)
        })?;
    let (calls, forwards) = uses_v1::validate_uses_and_calls(
        tcx,
        instance,
        body,
        uses_v1::ClosureUsesV1 {
            environments: &environments,
            closure_locals: &closure_locals,
            aliases: &aliases,
            forwarding,
        },
        work,
    )?;
    Ok(Some(RawClosureObservationV1 {
        environments,
        calls,
        forwards,
        observation: CompilerClosureObservationV2 {
            function: canonical_function_identities_v1(tcx, instance).function(),
            mir_body: rustc_mir_body_sha256_v1(tcx, instance),
            target: canonical_target_layout_v1(&target).identity(),
        },
    }))
}

fn charge_work(work: &mut SourceClosureWorkV1, amount: usize) -> Result<(), ClosureProfileErrorV1> {
    work.charge(amount)
        .map_err(|error| ClosureProfileErrorV1::new(error.to_string()))
}

pub(crate) fn contains_concrete_closure_v1<'tcx>(
    tcx: TyCtxt<'tcx>,
    instance: Instance<'tcx>,
) -> Result<bool, ClosureProfileErrorV1> {
    let body = tcx.instance_mir(instance.def);
    let own_receiver = own_closure_receiver_v1(tcx, instance, body)?;
    for (local, declaration) in body.local_decls.iter_enumerated() {
        if Some(local) == own_receiver {
            continue;
        }
        let ty = normalized_ty(tcx, instance, declaration.ty, "closure presence check")?;
        if matches!(ty.kind(), TyKind::Closure(..)) {
            return Ok(true);
        }
    }
    Ok(false)
}

fn own_closure_receiver_v1<'tcx>(
    tcx: TyCtxt<'tcx>,
    instance: Instance<'tcx>,
    body: &Body<'tcx>,
) -> Result<Option<Local>, ClosureProfileErrorV1> {
    let is_closure_body = matches!(instance.def, InstanceKind::Item(definition)
        if tcx.def_kind(definition) == rustc_hir::def::DefKind::Closure);
    if !is_closure_body && authenticate_once_shim_v1(tcx, instance)?.is_none() {
        return Ok(None);
    }
    let signature =
        crate::rustc_semantic_plan_v1::source_signature_v1(tcx, instance).map_err(|error| {
            ClosureProfileErrorV1::new(format!("closure receiver signature: {error}"))
        })?;
    let local = Local::from_usize(1);
    let declaration = body
        .local_decls
        .get(local)
        .ok_or_else(|| ClosureProfileErrorV1::new("closure body has no receiver"))?;
    let actual = normalized_ty(tcx, instance, declaration.ty, "closure body receiver")?;
    if body.arg_count == 0 || signature.inputs().first() != Some(&actual) {
        return Err(ClosureProfileErrorV1::new(
            "closure body receiver identity changed",
        ));
    }
    // The executing environment is not a new callable value. Its capture
    // accesses still pass through ordinary body construction and SSA checks.
    Ok(Some(local))
}

fn require_origin(
    policy: ClosureOriginPolicyV1,
    _origin: ClosureOriginV1,
) -> Result<(), ClosureProfileErrorV1> {
    match policy {
        ClosureOriginPolicyV1::Either => Ok(()),
        #[cfg(test)]
        ClosureOriginPolicyV1::HostArgument if _origin == ClosureOriginV1::HostArgument => Ok(()),
        #[cfg(test)]
        ClosureOriginPolicyV1::DeviceInternal if _origin == ClosureOriginV1::DeviceInternal => {
            Ok(())
        }
        #[cfg(test)]
        policy => Err(ClosureProfileErrorV1::new(format!(
            "closure origin {_origin:?} does not satisfy policy {policy:?}"
        ))),
    }
}

fn closure_kind(kind: Option<ClosureKind>) -> Result<ClosureCallKindV1, ClosureProfileErrorV1> {
    match kind {
        Some(ClosureKind::Fn) => Ok(ClosureCallKindV1::Fn),
        Some(ClosureKind::FnMut) => Ok(ClosureCallKindV1::FnMut),
        Some(ClosureKind::FnOnce) => Ok(ClosureCallKindV1::FnOnce),
        None => Err(ClosureProfileErrorV1::new(
            "closure kind is not fully monomorphized",
        )),
    }
}

pub(crate) fn normalized_ty<'tcx>(
    tcx: TyCtxt<'tcx>,
    instance: Instance<'tcx>,
    ty: Ty<'tcx>,
    subject: &str,
) -> Result<Ty<'tcx>, ClosureProfileErrorV1> {
    instance
        .try_instantiate_mir_and_normalize_erasing_regions(
            tcx,
            TypingEnv::fully_monomorphized(),
            EarlyBinder::bind(ty),
        )
        .map_err(|_| ClosureProfileErrorV1::new(format!("failed to normalize {subject}")))
}

fn reject_dynamic_types<'tcx>(
    tcx: TyCtxt<'tcx>,
    instance: Instance<'tcx>,
    body: &Body<'tcx>,
    work: &mut SourceClosureWorkV1,
) -> Result<(), ClosureProfileErrorV1> {
    let mut visited = HashSet::new();
    for declaration in &body.local_decls {
        charge_work(work, 1)?;
        let ty = normalized_ty(tcx, instance, declaration.ty, "local type")?;
        if contains_dynamic_type(ty, &mut visited, work)? {
            return Err(ClosureProfileErrorV1::new(
                "dynamic dispatch and dyn callable environments are forbidden",
            ));
        }
    }
    Ok(())
}

fn contains_dynamic_type<'tcx>(
    ty: Ty<'tcx>,
    visited: &mut HashSet<Ty<'tcx>>,
    work: &mut SourceClosureWorkV1,
) -> Result<bool, ClosureProfileErrorV1> {
    charge_work(work, 1)?;
    let mut pending = vec![ty];
    while let Some(ty) = pending.pop() {
        charge_work(work, 1)?;
        if !visited.insert(ty) {
            continue;
        }
        match ty.kind() {
            TyKind::Dynamic(..) => return Ok(true),
            TyKind::Ref(_, pointee, _) | TyKind::RawPtr(pointee, _) => {
                charge_work(work, 1)?;
                pending.push(*pointee);
            }
            TyKind::Tuple(fields) => {
                charge_work(work, fields.len())?;
                pending.extend(fields.iter());
            }
            _ => {}
        }
    }
    Ok(false)
}

fn closure_creations(
    body: &Body<'_>,
    work: &mut SourceClosureWorkV1,
) -> Result<BTreeMap<Local, (DefId, usize)>, ClosureProfileErrorV1> {
    let mut result = BTreeMap::new();
    for block in body.basic_blocks.iter() {
        charge_work(work, 1)?;
        for statement in &block.statements {
            charge_work(work, 1)?;
            let Some((destination, Rvalue::Aggregate(kind, operands))) = statement.kind.as_assign()
            else {
                continue;
            };
            let kind: &AggregateKind<'_> = kind.as_ref();
            let AggregateKind::Closure(def_id, _) = kind else {
                continue;
            };
            let Some(local) = destination.as_local() else {
                return Err(ClosureProfileErrorV1::new(
                    "closure aggregates must initialize one unprojected local",
                ));
            };
            if result.insert(local, (*def_id, operands.len())).is_some() {
                return Err(ClosureProfileErrorV1::new(format!(
                    "closure local{} is initialized more than once",
                    local.as_usize()
                )));
            }
        }
    }
    Ok(result)
}

// Extraction already bounds the depth and node count of this layout tree.
fn validate_capture_layout_v1(
    facts: &TypeLayoutFacts,
    origin: ClosureOriginV1,
    work: &mut SourceClosureWorkV1,
) -> Result<(), ClosureProfileErrorV1> {
    use crate::rust_type_layout_general::PointerKind;

    charge_work(work, 1)?;
    match &facts.kind {
        TypeLayoutKind::Closure { .. } => Err(ClosureProfileErrorV1::new(
            "nested closure captures are outside the bounded profile",
        )),
        TypeLayoutKind::Scalar(_) => Ok(()),
        TypeLayoutKind::SharedSliceReference { element } => {
            if origin == ClosureOriginV1::HostArgument {
                return Err(ClosureProfileErrorV1::new(
                    "host closure references require an eligible allocation/completion token; none is present in V1",
                ));
            }
            validate_capture_layout_v1(element, origin, work)
        }
        TypeLayoutKind::Pointer(pointer) => {
            match pointer.kind {
                PointerKind::ConstRaw | PointerKind::MutRaw => {
                    return Err(ClosureProfileErrorV1::new(
                        "raw-pointer captures have no allocation authority",
                    ));
                }
                PointerKind::SharedReference | PointerKind::MutableReference
                    if origin == ClosureOriginV1::HostArgument =>
                {
                    return Err(ClosureProfileErrorV1::new(
                        "host closure references require an eligible allocation/completion token; none is present in V1",
                    ));
                }
                PointerKind::SharedReference | PointerKind::MutableReference => {}
            }
            validate_capture_layout_v1(&pointer.pointee, origin, work)
        }
        TypeLayoutKind::Array(array) => validate_capture_layout_v1(&array.element, origin, work),
        TypeLayoutKind::Tuple(fields) => fields
            .iter()
            .try_for_each(|field| validate_capture_layout_v1(&field.layout, origin, work)),
        TypeLayoutKind::Adt(adt) => adt
            .variants
            .iter()
            .flat_map(|variant| &variant.fields)
            .try_for_each(|field| validate_capture_layout_v1(&field.layout, origin, work)),
    }
}

fn declared_call_kind(
    tcx: TyCtxt<'_>,
    func: &Operand<'_>,
) -> Result<ClosureCallKindV1, ClosureProfileErrorV1> {
    let Operand::Constant(constant) = func else {
        return Err(ClosureProfileErrorV1::new(
            "indirect closure calls are forbidden",
        ));
    };
    let TyKind::FnDef(def_id, _) = constant.const_.ty().kind() else {
        return Err(ClosureProfileErrorV1::new(
            "closure call operand is not a concrete Fn trait method",
        ));
    };
    let trait_id = tcx
        .trait_of_assoc(*def_id)
        .ok_or_else(|| ClosureProfileErrorV1::new("closure call is not a trait method"))?;
    if Some(trait_id) == tcx.lang_items().fn_trait() {
        Ok(ClosureCallKindV1::Fn)
    } else if Some(trait_id) == tcx.lang_items().fn_mut_trait() {
        Ok(ClosureCallKindV1::FnMut)
    } else if Some(trait_id) == tcx.lang_items().fn_once_trait() {
        Ok(ClosureCallKindV1::FnOnce)
    } else {
        Err(ClosureProfileErrorV1::new(
            "callable trait is not Fn, FnMut, or FnOnce",
        ))
    }
}

fn call_kind_allowed(actual: ClosureCallKindV1, invoked: ClosureCallKindV1) -> bool {
    matches!(
        (actual, invoked),
        (ClosureCallKindV1::Fn, _)
            | (
                ClosureCallKindV1::FnMut,
                ClosureCallKindV1::FnMut | ClosureCallKindV1::FnOnce
            )
            | (ClosureCallKindV1::FnOnce, ClosureCallKindV1::FnOnce)
    )
}

pub(crate) fn resolve_direct_call<'tcx>(
    tcx: TyCtxt<'tcx>,
    caller: Instance<'tcx>,
    func: &Operand<'tcx>,
) -> Result<Instance<'tcx>, ClosureProfileErrorV1> {
    let Operand::Constant(constant) = func else {
        return Err(ClosureProfileErrorV1::new("indirect closure call"));
    };
    let TyKind::FnDef(def_id, args) = constant.const_.ty().kind() else {
        return Err(ClosureProfileErrorV1::new("non-FnDef closure call"));
    };
    let args = caller
        .try_instantiate_mir_and_normalize_erasing_regions(
            tcx,
            TypingEnv::fully_monomorphized(),
            EarlyBinder::bind(*args),
        )
        .map_err(|_| ClosureProfileErrorV1::new("failed to normalize closure call arguments"))?;
    Instance::try_resolve(tcx, TypingEnv::fully_monomorphized(), *def_id, args)
        .ok()
        .flatten()
        .ok_or_else(|| {
            ClosureProfileErrorV1::new("closure call did not resolve to one monomorphic instance")
        })
}

fn tuple_argument_count<'tcx>(
    tcx: TyCtxt<'tcx>,
    instance: Instance<'tcx>,
    body: &Body<'tcx>,
    operand: &Operand<'tcx>,
) -> Result<usize, ClosureProfileErrorV1> {
    let ty = normalized_ty(
        tcx,
        instance,
        operand.ty(body, tcx),
        "closure argument tuple",
    )?;
    let TyKind::Tuple(fields) = ty.kind() else {
        return Err(ClosureProfileErrorV1::new(
            "closure arguments are not represented by one rustc tuple",
        ));
    };
    Ok(fields.len())
}

#[cfg(test)]
mod tests;

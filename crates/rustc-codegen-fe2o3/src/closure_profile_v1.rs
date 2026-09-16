//! Bounded closure admission from the live rustc target and monomorphized MIR.
//!
//! This profile recognizes concrete rustc closure types, records their
//! physical capture layout, and rejects uses outside the supported profile.
//! The observation checks collection/import continuity, not closure transport,
//! semantic equivalence, or authorization to lower the body.

use crate::rust_type_layout_general::{TypeLayoutFacts, TypeLayoutKind, extract_general_layout};
use crate::rustc_semantic_adapter_v1::{
    canonical_function_identities_v1, canonical_target_layout_v1, rustc_mir_body_sha256_v1,
};
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
use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

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

pub(crate) fn observe_closures_v2<'tcx>(
    tcx: TyCtxt<'tcx>,
    instance: Instance<'tcx>,
) -> Result<Option<BoundedClosureAdmissionV2>, ClosureProfileErrorV1> {
    if !contains_concrete_closure_v1(tcx, instance)? {
        return Ok(None);
    }
    analyze_bounded_closures_v2(tcx, instance, ClosureOriginPolicyV1::Either).map(Some)
}

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
    fn new(reason: impl Into<String>) -> Self {
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

pub(crate) fn analyze_bounded_closures_v2<'tcx>(
    tcx: TyCtxt<'tcx>,
    instance: Instance<'tcx>,
    policy: ClosureOriginPolicyV1,
) -> Result<BoundedClosureAdmissionV2, ClosureProfileErrorV1> {
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
    let body = tcx.instance_mir(instance.def);
    reject_dynamic_types(tcx, instance, body)?;
    let own_receiver = own_closure_receiver_v1(tcx, instance, body)?;
    let creations = closure_creations(body)?;
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
    let value_aliases = closure_value_aliases(body, &typed_closure_locals)?;
    let mut environments = Vec::new();
    let mut closure_locals = BTreeSet::new();

    for (local, declaration) in body.local_decls.iter_enumerated() {
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
        if environments.len() == MAX_CLOSURES {
            return Err(ClosureProfileErrorV1::new(format!(
                "closure count exceeds {MAX_CLOSURES}"
            )));
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
        require_origin(policy, origin)?;
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
            if origin == ClosureOriginV1::HostArgument && mode != ClosureCaptureModeV1::ByValue {
                return Err(ClosureProfileErrorV1::new(
                    "host closure references require an eligible allocation/completion token; none is present in V1",
                ));
            }
            let facts = extract_general_layout(tcx, capture_ty).map_err(|error| {
                ClosureProfileErrorV1::new(format!(
                    "capture {source_index} has unsupported physical layout: {error}"
                ))
            })?;
            validate_capture_layout_v1(&facts, origin)?;
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
    let aliases = closure_reference_aliases(body, &closure_locals, &value_aliases)?;
    let calls = validate_uses_and_calls(
        tcx,
        instance,
        body,
        &environments,
        &closure_locals,
        &aliases,
    )?;
    Ok(BoundedClosureAdmissionV2 {
        environments,
        calls,
        observation: CompilerClosureObservationV2 {
            function: canonical_function_identities_v1(tcx, instance).function(),
            mir_body: rustc_mir_body_sha256_v1(tcx, instance),
            target: canonical_target_layout_v1(&target).identity(),
        },
    })
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
    let InstanceKind::Item(definition) = instance.def else {
        return Ok(None);
    };
    if tcx.def_kind(definition) != rustc_hir::def::DefKind::Closure {
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

fn normalized_ty<'tcx>(
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
) -> Result<(), ClosureProfileErrorV1> {
    for declaration in &body.local_decls {
        let ty = normalized_ty(tcx, instance, declaration.ty, "local type")?;
        if contains_dynamic_type(ty) {
            return Err(ClosureProfileErrorV1::new(
                "dynamic dispatch and dyn callable environments are forbidden",
            ));
        }
    }
    Ok(())
}

fn contains_dynamic_type(ty: Ty<'_>) -> bool {
    match ty.kind() {
        TyKind::Dynamic(..) => true,
        TyKind::Ref(_, pointee, _) | TyKind::RawPtr(pointee, _) => contains_dynamic_type(*pointee),
        TyKind::Tuple(fields) => fields.iter().any(contains_dynamic_type),
        _ => false,
    }
}

fn closure_creations(
    body: &Body<'_>,
) -> Result<BTreeMap<Local, (DefId, usize)>, ClosureProfileErrorV1> {
    let mut result = BTreeMap::new();
    for block in body.basic_blocks.iter() {
        for statement in &block.statements {
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
) -> Result<(), ClosureProfileErrorV1> {
    use crate::rust_type_layout_general::PointerKind;

    match &facts.kind {
        TypeLayoutKind::Closure { .. } => Err(ClosureProfileErrorV1::new(
            "nested closure captures are outside the bounded profile",
        )),
        TypeLayoutKind::Scalar(_) => Ok(()),
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
            validate_capture_layout_v1(&pointer.pointee, origin)
        }
        TypeLayoutKind::Array(array) => validate_capture_layout_v1(&array.element, origin),
        TypeLayoutKind::Tuple(fields) => fields
            .iter()
            .try_for_each(|field| validate_capture_layout_v1(&field.layout, origin)),
        TypeLayoutKind::Adt(adt) => adt
            .variants
            .iter()
            .flat_map(|variant| &variant.fields)
            .try_for_each(|field| validate_capture_layout_v1(&field.layout, origin)),
    }
}

fn closure_reference_aliases(
    body: &Body<'_>,
    closure_locals: &BTreeSet<Local>,
    value_aliases: &BTreeMap<Local, Local>,
) -> Result<BTreeMap<Local, Local>, ClosureProfileErrorV1> {
    let mut aliases = value_aliases.clone();
    for block in body.basic_blocks.iter() {
        for statement in &block.statements {
            let Some((destination, Rvalue::Ref(_, _, source))) = statement.kind.as_assign() else {
                continue;
            };
            let (Some(destination), Some(source)) = (destination.as_local(), source.as_local())
            else {
                continue;
            };
            let Some(root) = resolve_alias_root(source, closure_locals, &aliases) else {
                continue;
            };
            if aliases.insert(destination, root).is_some() {
                return Err(ClosureProfileErrorV1::new(
                    "closure receiver alias is assigned more than once",
                ));
            }
        }
    }
    Ok(aliases)
}

fn closure_value_aliases(
    body: &Body<'_>,
    typed_closure_locals: &BTreeSet<Local>,
) -> Result<BTreeMap<Local, Local>, ClosureProfileErrorV1> {
    let mut aliases = BTreeMap::new();
    for block in body.basic_blocks.iter() {
        for statement in &block.statements {
            let Some((destination, Rvalue::Use(operand))) = statement.kind.as_assign() else {
                continue;
            };
            let (Some(destination), Some(source)) =
                (destination.as_local(), operand_local(operand))
            else {
                continue;
            };
            if !typed_closure_locals.contains(&destination)
                || !typed_closure_locals.contains(&source)
            {
                continue;
            }
            if destination.as_usize() == 0 {
                return Err(ClosureProfileErrorV1::new(
                    "returning a forwarded closure escapes its environment",
                ));
            }
            if aliases.insert(destination, source).is_some() {
                return Err(ClosureProfileErrorV1::new(
                    "closure value forwarding local is assigned more than once",
                ));
            }
        }
    }
    Ok(aliases)
}

fn resolve_alias_root(
    mut local: Local,
    roots: &BTreeSet<Local>,
    aliases: &BTreeMap<Local, Local>,
) -> Option<Local> {
    for _ in 0..=MAX_CLOSURES {
        if roots.contains(&local) {
            return Some(local);
        }
        local = *aliases.get(&local)?;
    }
    None
}

fn validate_uses_and_calls<'tcx>(
    tcx: TyCtxt<'tcx>,
    instance: Instance<'tcx>,
    body: &Body<'tcx>,
    environments: &[ClosureEnvironmentV1],
    closure_locals: &BTreeSet<Local>,
    aliases: &BTreeMap<Local, Local>,
) -> Result<Vec<StaticClosureCallV1>, ClosureProfileErrorV1> {
    let by_local = environments
        .iter()
        .map(|environment| (Local::from_usize(environment.local), environment))
        .collect::<BTreeMap<_, _>>();
    let mut calls = Vec::new();
    let mut call_counts = BTreeMap::<Local, usize>::new();
    for (block_index, block) in body.basic_blocks.iter_enumerated() {
        for statement in &block.statements {
            if let Some((destination, value)) = statement.kind.as_assign() {
                if allowed_closure_assignment(*destination, value, closure_locals, aliases) {
                    continue;
                }
                if rvalue_mentions_closure(value, closure_locals, aliases) {
                    return Err(ClosureProfileErrorV1::new(format!(
                        "closure value escapes through an unsupported assignment in bb{}",
                        block_index.as_usize()
                    )));
                }
            } else if statement_mentions_closure(&statement.kind, closure_locals, aliases) {
                return Err(ClosureProfileErrorV1::new(format!(
                    "closure value is used by an unsupported statement in bb{}",
                    block_index.as_usize()
                )));
            }
        }
        let Some(terminator) = &block.terminator else {
            continue;
        };
        match &terminator.kind {
            TerminatorKind::Call { func, args, .. } => {
                if operand_mentions_closure(func, closure_locals, aliases) {
                    return Err(ClosureProfileErrorV1::new(
                        "closure value escapes through an indirect call target",
                    ));
                }
                let receiver = args
                    .first()
                    .and_then(|argument| operand_local(&argument.node));
                let closure_local =
                    receiver.and_then(|local| resolve_alias_root(local, closure_locals, aliases));
                if let Some(closure_local) = closure_local {
                    let environment = by_local[&closure_local];
                    let call_kind = declared_call_kind(tcx, func)?;
                    if !call_kind_allowed(environment.call_kind, call_kind) {
                        return Err(ClosureProfileErrorV1::new(
                            "closure invoked through an incompatible Fn trait",
                        ));
                    }
                    if args.len() != 2 {
                        return Err(ClosureProfileErrorV1::new(
                            "bounded closure calls require receiver plus one tuple argument",
                        ));
                    }
                    let argument_count = tuple_argument_count(tcx, instance, body, &args[1].node)?;
                    if argument_count > MAX_CALL_ARGUMENTS {
                        return Err(ClosureProfileErrorV1::new(format!(
                            "closure call argument count exceeds {MAX_CALL_ARGUMENTS}"
                        )));
                    }
                    let target = resolve_direct_call(tcx, instance, func)?;
                    if tcx.def_path_hash(target.def_id()).0.to_le_bytes()
                        != environment.definition_hash
                        && !matches!(target.def, InstanceKind::ClosureOnceShim { .. })
                    {
                        return Err(ClosureProfileErrorV1::new(
                            "closure call did not resolve to its compiler-generated body or once shim",
                        ));
                    }
                    for argument in args.iter().skip(1) {
                        if operand_mentions_closure(&argument.node, closure_locals, aliases) {
                            return Err(ClosureProfileErrorV1::new(
                                "closure value escapes through a non-receiver call argument",
                            ));
                        }
                    }
                    *call_counts.entry(closure_local).or_default() += 1;
                    calls.push(StaticClosureCallV1 {
                        block: block_index.as_usize(),
                        closure_local: closure_local.as_usize(),
                        call_kind,
                        argument_count,
                        target_definition_hash: tcx.def_path_hash(target.def_id()).0.to_le_bytes(),
                    });
                    if calls.len() > MAX_STATIC_CALLS {
                        return Err(ClosureProfileErrorV1::new(format!(
                            "closure call count exceeds {MAX_STATIC_CALLS}"
                        )));
                    }
                } else if args.iter().any(|argument| {
                    operand_mentions_closure(&argument.node, closure_locals, aliases)
                }) {
                    return Err(ClosureProfileErrorV1::new(
                        "closure value escapes to a non-closure call",
                    ));
                }
            }
            TerminatorKind::TailCall { func, args, .. }
                if operand_mentions_closure(func, closure_locals, aliases)
                    || args.iter().any(|argument| {
                        operand_mentions_closure(&argument.node, closure_locals, aliases)
                    }) =>
            {
                return Err(ClosureProfileErrorV1::new(
                    "closure value escapes through a tail call",
                ));
            }
            TerminatorKind::Drop { place, .. }
                if place_mentions_closure(*place, closure_locals, aliases) =>
            {
                let allowed = place
                    .as_local()
                    .and_then(|local| resolve_alias_root(local, closure_locals, aliases))
                    .is_some();
                if !allowed {
                    return Err(ClosureProfileErrorV1::new(
                        "closure drop must consume one unprojected closure or receiver alias",
                    ));
                }
            }
            TerminatorKind::SwitchInt { discr, .. }
                if operand_mentions_closure(discr, closure_locals, aliases) =>
            {
                return Err(ClosureProfileErrorV1::new(
                    "closure value is used as a switch discriminant",
                ));
            }
            TerminatorKind::Assert { cond, .. }
                if operand_mentions_closure(cond, closure_locals, aliases) =>
            {
                return Err(ClosureProfileErrorV1::new(
                    "closure value is used as an assertion condition",
                ));
            }
            TerminatorKind::Yield {
                value, resume_arg, ..
            } if operand_mentions_closure(value, closure_locals, aliases)
                || place_mentions_closure(*resume_arg, closure_locals, aliases) =>
            {
                return Err(ClosureProfileErrorV1::new(
                    "closure value escapes through a coroutine yield",
                ));
            }
            TerminatorKind::InlineAsm { operands, .. }
                if operands.iter().any(|operand| {
                    inline_asm_mentions_closure(operand, closure_locals, aliases)
                }) =>
            {
                return Err(ClosureProfileErrorV1::new(
                    "closure value escapes through inline assembly",
                ));
            }
            _ => {}
        }
    }
    for environment in environments {
        let local = Local::from_usize(environment.local);
        let count = call_counts.get(&local).copied().unwrap_or(0);
        if count == 0 || (environment.call_kind == ClosureCallKindV1::FnOnce && count != 1) {
            return Err(ClosureProfileErrorV1::new(format!(
                "closure local{} has invalid call count {count} for {:?}",
                environment.local, environment.call_kind
            )));
        }
    }
    calls.sort_by_key(|call| (call.block, call.closure_local));
    Ok(calls)
}

fn allowed_closure_assignment(
    destination: Place<'_>,
    value: &Rvalue<'_>,
    closure_locals: &BTreeSet<Local>,
    aliases: &BTreeMap<Local, Local>,
) -> bool {
    match value {
        Rvalue::Aggregate(kind, _) => {
            matches!(&**kind, AggregateKind::Closure(..))
                && destination
                    .as_local()
                    .is_some_and(|local| closure_locals.contains(&local))
        }
        Rvalue::Ref(_, _, source) => {
            let Some(destination) = destination.as_local() else {
                return false;
            };
            aliases.contains_key(&destination)
                && source
                    .as_local()
                    .and_then(|local| resolve_alias_root(local, closure_locals, aliases))
                    .is_some()
        }
        Rvalue::Use(operand) => {
            let Some(destination) = destination.as_local() else {
                return false;
            };
            let Some(source) = operand_local(operand) else {
                return false;
            };
            source != destination
                && aliases.contains_key(&destination)
                && resolve_alias_root(source, closure_locals, aliases).is_some()
        }
        _ => false,
    }
}

fn statement_mentions_closure(
    statement: &StatementKind<'_>,
    closures: &BTreeSet<Local>,
    aliases: &BTreeMap<Local, Local>,
) -> bool {
    match statement {
        StatementKind::Assign(_) => false,
        StatementKind::FakeRead(contents) => place_mentions_closure(contents.1, closures, aliases),
        StatementKind::SetDiscriminant { place, .. }
        | StatementKind::Retag(_, place)
        | StatementKind::PlaceMention(place)
        | StatementKind::BackwardIncompatibleDropHint { place, .. } => {
            place_mentions_closure(**place, closures, aliases)
        }
        StatementKind::AscribeUserType(contents, _) => {
            place_mentions_closure(contents.0, closures, aliases)
        }
        StatementKind::Intrinsic(intrinsic) => match intrinsic.as_ref() {
            NonDivergingIntrinsic::Assume(operand) => {
                operand_mentions_closure(operand, closures, aliases)
            }
            NonDivergingIntrinsic::CopyNonOverlapping(copy) => {
                operand_mentions_closure(&copy.src, closures, aliases)
                    || operand_mentions_closure(&copy.dst, closures, aliases)
                    || operand_mentions_closure(&copy.count, closures, aliases)
            }
        },
        StatementKind::StorageLive(_)
        | StatementKind::StorageDead(_)
        | StatementKind::Coverage(_)
        | StatementKind::ConstEvalCounter
        | StatementKind::Nop => false,
    }
}

fn inline_asm_mentions_closure(
    operand: &InlineAsmOperand<'_>,
    closures: &BTreeSet<Local>,
    aliases: &BTreeMap<Local, Local>,
) -> bool {
    match operand {
        InlineAsmOperand::In { value, .. } => operand_mentions_closure(value, closures, aliases),
        InlineAsmOperand::Out {
            place: Some(place), ..
        } => place_mentions_closure(*place, closures, aliases),
        InlineAsmOperand::InOut {
            in_value,
            out_place,
            ..
        } => {
            operand_mentions_closure(in_value, closures, aliases)
                || out_place.is_some_and(|place| place_mentions_closure(place, closures, aliases))
        }
        InlineAsmOperand::Out { place: None, .. }
        | InlineAsmOperand::Const { .. }
        | InlineAsmOperand::SymFn { .. }
        | InlineAsmOperand::SymStatic { .. }
        | InlineAsmOperand::Label { .. } => false,
    }
}

fn rvalue_mentions_closure(
    rvalue: &Rvalue<'_>,
    closures: &BTreeSet<Local>,
    aliases: &BTreeMap<Local, Local>,
) -> bool {
    match rvalue {
        Rvalue::Use(operand)
        | Rvalue::Repeat(operand, _)
        | Rvalue::UnaryOp(_, operand)
        | Rvalue::Cast(_, operand, _)
        | Rvalue::WrapUnsafeBinder(operand, _) => {
            operand_mentions_closure(operand, closures, aliases)
        }
        Rvalue::Ref(_, _, place)
        | Rvalue::RawPtr(_, place)
        | Rvalue::Discriminant(place)
        | Rvalue::CopyForDeref(place) => place_mentions_closure(*place, closures, aliases),
        Rvalue::BinaryOp(_, operands) => {
            operand_mentions_closure(&operands.0, closures, aliases)
                || operand_mentions_closure(&operands.1, closures, aliases)
        }
        Rvalue::Aggregate(_, operands) => operands
            .iter()
            .any(|operand| operand_mentions_closure(operand, closures, aliases)),
        Rvalue::ThreadLocalRef(_) => false,
    }
}

fn operand_mentions_closure(
    operand: &Operand<'_>,
    closures: &BTreeSet<Local>,
    aliases: &BTreeMap<Local, Local>,
) -> bool {
    operand_local(operand)
        .is_some_and(|local| closures.contains(&local) || aliases.contains_key(&local))
}

fn place_mentions_closure(
    place: Place<'_>,
    closures: &BTreeSet<Local>,
    aliases: &BTreeMap<Local, Local>,
) -> bool {
    closures.contains(&place.local) || aliases.contains_key(&place.local)
}

fn operand_local(operand: &Operand<'_>) -> Option<Local> {
    match operand {
        Operand::Copy(place) | Operand::Move(place) => place.as_local(),
        Operand::Constant(_) | Operand::RuntimeChecks(_) => None,
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

fn resolve_direct_call<'tcx>(
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

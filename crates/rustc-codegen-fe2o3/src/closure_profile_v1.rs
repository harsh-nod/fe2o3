//! Compiler-authenticated bounded closure admission for production GPU targets.
//!
//! This profile recognizes concrete rustc closure types, records their
//! physical capture layout, and emits a static-call lowering plan only when
//! every use of the closure is understood. It does not authorize arbitrary
//! MIR V2 lowering.

use crate::rust_type_layout_general::{TypeLayoutFacts, extract_general_layout};
use rustc_abi::ExternAbi;
use rustc_hir::Mutability;
use rustc_hir::def::DefKind;
use rustc_hir::def_id::DefId;
use rustc_middle::mir::{
    AggregateKind, Body, InlineAsmOperand, Local, NonDivergingIntrinsic, Operand, Place, Rvalue,
    StatementKind, TerminatorKind,
};
use rustc_middle::ty::layout::{LayoutCx, LayoutOf};
use rustc_middle::ty::{
    self, ClosureKind, EarlyBinder, Instance, InstanceKind, Ty, TyCtxt, TyKind, TypeVisitableExt,
    TypingEnv,
};
use rustc_span::{Span, Spanned};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

const MAX_CLOSURES: usize = 8;
const MAX_CAPTURES: usize = 16;
const MAX_ENVIRONMENT_BYTES: u64 = 256;
const MAX_ENVIRONMENT_ALIGNMENT: u64 = 16;
const MAX_CALL_ARGUMENTS: usize = 8;
const MAX_STATIC_CALLS: usize = 64;
const MAX_MACRO_EXPANSION_DEPTH: usize = 256;

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
    InvocationReceiver,
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
    pub(crate) closure_type_identity: [u8; 32],
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

/// Exact by-value custody transfer into one recursively collected,
/// monomorphized Rust callee. The callee is independently closure-profiled,
/// so this record does not grant a generic higher-order escape.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct ClosureTransportCallV1 {
    pub(crate) block: usize,
    pub(crate) closure_local: usize,
    pub(crate) argument_index: usize,
    pub(crate) target_definition_hash: [u8; 16],
    pub(crate) target_function_identity: [u8; 32],
    pub(crate) target_monomorphization_identity: [u8; 32],
    pub(crate) target_mir_identity: [u8; 32],
    pub(crate) target_fn_abi_identity: [u8; 32],
    pub(crate) call_source_identity: [u8; 32],
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum HigherOrderCapabilityTerminalV1 {
    WithWorkgroup,
    WithMatrix,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum HigherOrderClosureCustodyV1 {
    EnvironmentLocal(usize),
    ZeroSizedConstant {
        definition_hash: [u8; 16],
        closure_type_identity: [u8; 32],
        operand_source_identity: [u8; 32],
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct HigherOrderCapabilityCallV1 {
    pub(crate) block: usize,
    pub(crate) closure_custody: HigherOrderClosureCustodyV1,
    pub(crate) terminal: HigherOrderCapabilityTerminalV1,
    pub(crate) target_definition_hash: [u8; 16],
    pub(crate) target_function_identity: [u8; 32],
    pub(crate) target_monomorphization_identity: [u8; 32],
    pub(crate) target_generic_types_identity: [u8; 32],
    pub(crate) target_const_generics_identity: [u8; 32],
    pub(crate) target_mir_identity: [u8; 32],
    pub(crate) target_fn_abi_identity: [u8; 32],
    pub(crate) call_source_identity: [u8; 32],
}

/// Compiler-sealed plan for direct environment reconstruction and static call.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct ProductionClosureLoweringV1 {
    target: String,
    environments: Vec<ClosureEnvironmentV1>,
    calls: Vec<StaticClosureCallV1>,
    transport_calls: Vec<ClosureTransportCallV1>,
    higher_order_calls: Vec<HigherOrderCapabilityCallV1>,
    identity: [u8; 32],
}

impl ProductionClosureLoweringV1 {
    pub(crate) fn target(&self) -> &str {
        &self.target
    }

    pub(crate) fn environments(&self) -> &[ClosureEnvironmentV1] {
        &self.environments
    }

    pub(crate) fn calls(&self) -> &[StaticClosureCallV1] {
        &self.calls
    }

    pub(crate) fn transport_calls(&self) -> &[ClosureTransportCallV1] {
        &self.transport_calls
    }

    pub(crate) fn higher_order_calls(&self) -> &[HigherOrderCapabilityCallV1] {
        &self.higher_order_calls
    }

    pub(crate) fn authenticated_closure_type_identities(
        &self,
    ) -> impl Iterator<Item = [u8; 32]> + '_ {
        self.environments
            .iter()
            .map(|environment| environment.closure_type_identity)
            .chain(
                self.higher_order_calls
                    .iter()
                    .filter_map(|call| match &call.closure_custody {
                        HigherOrderClosureCustodyV1::ZeroSizedConstant {
                            closure_type_identity,
                            ..
                        } => Some(*closure_type_identity),
                        HigherOrderClosureCustodyV1::EnvironmentLocal(_) => None,
                    }),
            )
    }

    pub(crate) const fn identity(&self) -> [u8; 32] {
        self.identity
    }
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
            "production closure profile rejected MIR: {}",
            self.0
        )
    }
}

impl std::error::Error for ClosureProfileErrorV1 {}

pub(crate) fn analyze_production_closures_v1<'tcx>(
    tcx: TyCtxt<'tcx>,
    instance: Instance<'tcx>,
    policy: ClosureOriginPolicyV1,
    selected_target: &str,
) -> Result<ProductionClosureLoweringV1, ClosureProfileErrorV1> {
    let processor = selected_target
        .split(':')
        .next()
        .unwrap_or(selected_target)
        .trim();
    if !matches!(processor, "gfx942" | "gfx950") {
        return Err(ClosureProfileErrorV1::new(format!(
            "the bounded closure profile supports gfx942 and gfx950 production targets, not `{selected_target}`"
        )));
    }
    if tcx.sess.target.pointer_width != 64 {
        return Err(ClosureProfileErrorV1::new(
            "the bounded production GPU profile requires a 64-bit compiler target",
        ));
    }
    let body = tcx.instance_mir(instance.def);
    reject_dynamic_types(tcx, instance, body)?;
    let creations = closure_creations(body)?;
    let typed_closure_locals = body
        .local_decls
        .iter_enumerated()
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
        let origin = if local.as_usize() == 1 && *def_id == instance.def_id() {
            ClosureOriginV1::InvocationReceiver
        } else if local.as_usize() != 0 && local.as_usize() <= body.arg_count {
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
            if origin == ClosureOriginV1::HostArgument
                && mode != ClosureCaptureModeV1::ByValue
                && !authenticated_higher_order_argument_borrow_v1(tcx, instance, local, ty)?
            {
                return Err(ClosureProfileErrorV1::new(
                    "host closure references require an eligible allocation/completion token; none is present in V1",
                ));
            }
            let facts = extract_general_layout(tcx, capture_ty).map_err(|error| {
                ClosureProfileErrorV1::new(format!(
                    "capture {source_index} has unsupported physical layout: {error}"
                ))
            })?;
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
            closure_type_identity: *crate::rustc_semantic_adapter_v1::rustc_type_identity_v1(
                tcx, ty,
            )
            .as_bytes(),
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
    let (calls, transport_calls, higher_order_calls) = validate_uses_and_calls(
        tcx,
        instance,
        body,
        &environments,
        &closure_locals,
        &aliases,
    )?;
    let identity = lowering_identity(
        selected_target,
        &environments,
        &calls,
        &transport_calls,
        &higher_order_calls,
    );
    Ok(ProductionClosureLoweringV1 {
        target: selected_target.to_owned(),
        environments,
        calls,
        transport_calls,
        higher_order_calls,
        identity,
    })
}

pub(crate) fn contains_concrete_closure_v1<'tcx>(
    tcx: TyCtxt<'tcx>,
    instance: Instance<'tcx>,
) -> Result<bool, ClosureProfileErrorV1> {
    let body = tcx.instance_mir(instance.def);
    for declaration in &body.local_decls {
        let ty = normalized_ty(tcx, instance, declaration.ty, "closure presence check")?;
        if matches!(ty.kind(), TyKind::Closure(..)) {
            return Ok(true);
        }
    }
    Ok(false)
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

fn authenticated_higher_order_argument_borrow_v1<'tcx>(
    tcx: TyCtxt<'tcx>,
    instance: Instance<'tcx>,
    local: Local,
    closure_ty: Ty<'tcx>,
) -> Result<bool, ClosureProfileErrorV1> {
    let expected_self = match tcx.item_name(instance.def_id()).as_str() {
        "with_workgroup" => crate::trusted_device_items::TrustedDeviceItem::KernelContext,
        "with_matrix" => {
            crate::trusted_device_items::TrustedDeviceItem::ExecutionSubgroupCapability
        }
        _ => return Ok(false),
    };
    if !crate::trusted_device_items::authenticate_reviewed_safe_external_helper_v1(
        tcx,
        instance.def_id(),
    )
    .map_err(|detail| {
        ClosureProfileErrorV1::new(format!(
            "higher-order closure argument provider authentication failed: {detail}"
        ))
    })? {
        return Ok(false);
    }
    let Some(self_ty) = inherent_impl_self_ty(tcx, instance.def_id()) else {
        return Err(ClosureProfileErrorV1::new(
            "higher-order closure argument provider has no inherent receiver type",
        ));
    };
    let TyKind::Adt(self_adt, _) = self_ty.kind() else {
        return Err(ClosureProfileErrorV1::new(
            "higher-order closure argument provider receiver is not a concrete device type",
        ));
    };
    if crate::trusted_device_items::classify(tcx, self_adt.did()) != Some(expected_self) {
        return Ok(false);
    }

    let signature = tcx.normalize_erasing_regions(
        TypingEnv::fully_monomorphized(),
        tcx.instantiate_bound_regions_with_erased(
            tcx.fn_sig(instance.def_id())
                .instantiate(tcx, instance.args),
        ),
    );
    if signature.safety != rustc_hir::Safety::Safe
        || signature.abi != ExternAbi::Rust
        || signature.c_variadic
    {
        return Err(ClosureProfileErrorV1::new(
            "authenticated higher-order closure argument provider changed its Rust ABI",
        ));
    }
    let closure_inputs = signature
        .inputs()
        .iter()
        .enumerate()
        .filter_map(|(index, input)| matches!(input.kind(), TyKind::Closure(..)).then_some(index))
        .collect::<Vec<_>>();
    let [closure_input] = closure_inputs.as_slice() else {
        return Err(ClosureProfileErrorV1::new(
            "authenticated higher-order closure argument provider must have one concrete closure input",
        ));
    };
    Ok(local.as_usize().checked_sub(1) == Some(*closure_input)
        && signature.inputs()[*closure_input] == closure_ty)
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
) -> Result<
    (
        Vec<StaticClosureCallV1>,
        Vec<ClosureTransportCallV1>,
        Vec<HigherOrderCapabilityCallV1>,
    ),
    ClosureProfileErrorV1,
> {
    let by_local = environments
        .iter()
        .map(|environment| (Local::from_usize(environment.local), environment))
        .collect::<BTreeMap<_, _>>();
    let mut calls = Vec::new();
    let mut transport_calls = Vec::new();
    let mut higher_order_calls = Vec::new();
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
                if let Some(call) = authenticate_higher_order_capability_call(
                    tcx,
                    instance,
                    body,
                    block_index.as_usize(),
                    terminator.source_info.span,
                    func,
                    args,
                    closure_locals,
                    aliases,
                )? {
                    if let HigherOrderClosureCustodyV1::EnvironmentLocal(local) =
                        &call.closure_custody
                    {
                        *call_counts.entry(Local::from_usize(*local)).or_default() += 1;
                    }
                    higher_order_calls.push(call);
                    if calls.len() + transport_calls.len() + higher_order_calls.len()
                        > MAX_STATIC_CALLS
                    {
                        return Err(ClosureProfileErrorV1::new(format!(
                            "closure call count exceeds {MAX_STATIC_CALLS}"
                        )));
                    }
                    continue;
                }
                let receiver = args
                    .first()
                    .and_then(|argument| operand_local(&argument.node));
                let closure_local =
                    receiver.and_then(|local| resolve_alias_root(local, closure_locals, aliases));
                // Argument zero may carry an environment into an ordinary Rust
                // helper; only the resolved callable can establish invocation.
                let invocation = match closure_local {
                    Some(local) => {
                        let target = resolve_direct_call(tcx, instance, func)?;
                        closure_invocation_kind_v1(tcx, func, target)?
                            .map(|kind| (local, target, kind))
                    }
                    None => None,
                };
                if invocation.is_none()
                    && let Some(call) = authenticate_closure_transport_call_v1(
                        tcx,
                        instance,
                        body,
                        block_index.as_usize(),
                        terminator.source_info.span,
                        func,
                        args,
                        closure_locals,
                        aliases,
                    )?
                {
                    *call_counts
                        .entry(Local::from_usize(call.closure_local))
                        .or_default() += 1;
                    transport_calls.push(call);
                    if calls.len() + transport_calls.len() + higher_order_calls.len()
                        > MAX_STATIC_CALLS
                    {
                        return Err(ClosureProfileErrorV1::new(format!(
                            "closure call count exceeds {MAX_STATIC_CALLS}"
                        )));
                    }
                    continue;
                }
                if let Some((closure_local, target, call_kind)) = invocation {
                    let environment = by_local[&closure_local];
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
                    if calls.len() + transport_calls.len() + higher_order_calls.len()
                        > MAX_STATIC_CALLS
                    {
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
        if (environment.origin != ClosureOriginV1::InvocationReceiver && count == 0)
            || (environment.origin != ClosureOriginV1::InvocationReceiver
                && environment.call_kind == ClosureCallKindV1::FnOnce
                && count != 1)
        {
            return Err(ClosureProfileErrorV1::new(format!(
                "closure local{} has invalid call count {count} for {:?}",
                environment.local, environment.call_kind
            )));
        }
    }
    calls.sort_by_key(|call| (call.block, call.closure_local));
    transport_calls.sort_by_key(|call| (call.block, call.argument_index, call.closure_local));
    higher_order_calls.sort_by_key(|call| call.block);
    Ok((calls, transport_calls, higher_order_calls))
}

#[allow(clippy::too_many_arguments)]
fn authenticate_closure_transport_call_v1<'tcx>(
    tcx: TyCtxt<'tcx>,
    caller: Instance<'tcx>,
    body: &Body<'tcx>,
    block: usize,
    span: Span,
    func: &Operand<'tcx>,
    args: &[Spanned<Operand<'tcx>>],
    closure_locals: &BTreeSet<Local>,
    aliases: &BTreeMap<Local, Local>,
) -> Result<Option<ClosureTransportCallV1>, ClosureProfileErrorV1> {
    let transported = args
        .iter()
        .enumerate()
        .filter_map(|(index, argument)| {
            operand_local(&argument.node)
                .and_then(|local| resolve_alias_root(local, closure_locals, aliases))
                .map(|local| (index, local))
        })
        .collect::<Vec<_>>();
    let [(argument_index, closure_local)] = transported.as_slice() else {
        if transported.is_empty() {
            return Ok(None);
        }
        return Err(ClosureProfileErrorV1::new(
            "one call may transport only one closure environment",
        ));
    };

    let target = resolve_direct_call(tcx, caller, func)?;
    if target.args.has_param() || target.args.has_escaping_bound_vars() {
        return Err(ClosureProfileErrorV1::new(
            "closure transport target did not resolve to one monomorphic instance",
        ));
    }
    if !matches!(target.def, InstanceKind::Item(_)) || !tcx.is_mir_available(target.def_id()) {
        return Err(ClosureProfileErrorV1::new(
            "closure transport target has no recursively traversable Rust MIR",
        ));
    }
    let signature = tcx.normalize_erasing_regions(
        TypingEnv::fully_monomorphized(),
        tcx.instantiate_bound_regions_with_erased(
            tcx.fn_sig(target.def_id()).instantiate(tcx, target.args),
        ),
    );
    if signature.safety != rustc_hir::Safety::Safe
        || signature.abi != ExternAbi::Rust
        || signature.c_variadic
        || signature.inputs().len() != args.len()
    {
        return Err(ClosureProfileErrorV1::new(
            "closure transport target changed its safe Rust ABI",
        ));
    }
    if contains_closure_type_v1(signature.output()) {
        return Err(ClosureProfileErrorV1::new(
            "closure transport target returns a closure-bearing value",
        ));
    }
    let closure_ty = normalized_ty(
        tcx,
        caller,
        args[*argument_index].node.ty(body, tcx),
        "transported closure argument",
    )?;
    if signature.inputs()[*argument_index] != closure_ty
        || !matches!(closure_ty.kind(), TyKind::Closure(..))
    {
        return Err(ClosureProfileErrorV1::new(
            "closure transport must preserve one exact by-value closure type",
        ));
    }
    if signature
        .inputs()
        .iter()
        .enumerate()
        .any(|(index, input)| index != *argument_index && contains_closure_type_v1(*input))
    {
        return Err(ClosureProfileErrorV1::new(
            "closure transport target has an additional closure-bearing input",
        ));
    }

    let query = TypingEnv::fully_monomorphized().as_query_input((target, ty::List::empty()));
    let fn_abi = tcx.fn_abi_of_instance(query).map_err(|error| {
        ClosureProfileErrorV1::new(format!(
            "closure transport target FnAbi is unavailable: {error:?}"
        ))
    })?;
    let identities =
        crate::rustc_semantic_adapter_v1::canonical_function_identities_v1(tcx, target);
    let source = crate::rustc_semantic_adapter_v1::canonical_source_provenance_v1(
        tcx,
        span,
        MAX_MACRO_EXPANSION_DEPTH,
    )
    .map_err(|error| {
        ClosureProfileErrorV1::new(format!(
            "closure transport call has invalid source provenance: {error}"
        ))
    })?;
    Ok(Some(ClosureTransportCallV1 {
        block,
        closure_local: closure_local.as_usize(),
        argument_index: *argument_index,
        target_definition_hash: tcx.def_path_hash(target.def_id()).0.to_le_bytes(),
        target_function_identity: *identities.function().as_bytes(),
        target_monomorphization_identity: *identities.monomorphization().as_bytes(),
        target_mir_identity: crate::rustc_semantic_adapter_v1::rustc_mir_body_sha256_v1(
            tcx, target,
        ),
        target_fn_abi_identity: crate::rustc_semantic_adapter_v1::rustc_fn_abi_sha256_v1(
            tcx, fn_abi,
        ),
        call_source_identity: source.expansion_chain_sha256(),
    }))
}

fn contains_closure_type_v1(ty: Ty<'_>) -> bool {
    ty.walk()
        .filter_map(|argument| argument.as_type())
        .any(|component| {
            matches!(
                component.kind(),
                TyKind::Closure(..) | TyKind::CoroutineClosure(..)
            )
        })
}

#[allow(clippy::too_many_arguments)]
fn authenticate_higher_order_capability_call<'tcx>(
    tcx: TyCtxt<'tcx>,
    caller: Instance<'tcx>,
    body: &Body<'tcx>,
    block: usize,
    span: Span,
    func: &Operand<'tcx>,
    args: &[Spanned<Operand<'tcx>>],
    closure_locals: &BTreeSet<Local>,
    aliases: &BTreeMap<Local, Local>,
) -> Result<Option<HigherOrderCapabilityCallV1>, ClosureProfileErrorV1> {
    let Some((terminal, expected_self)) = higher_order_capability_candidate(tcx, func)? else {
        return Ok(None);
    };
    let target = resolve_direct_call(tcx, caller, func)?;
    if target.args.has_param() || target.args.has_escaping_bound_vars() {
        return Err(ClosureProfileErrorV1::new(
            "higher-order capability terminal did not resolve to one monomorphic instance",
        ));
    }
    if !crate::trusted_device_items::authenticate_reviewed_safe_external_helper_v1(
        tcx,
        target.def_id(),
    )
    .map_err(|detail| {
        ClosureProfileErrorV1::new(format!(
            "higher-order capability terminal provider authentication failed: {detail}"
        ))
    })? {
        return Ok(None);
    }
    let self_ty = inherent_impl_self_ty(tcx, target.def_id()).ok_or_else(|| {
        ClosureProfileErrorV1::new(
            "higher-order capability terminal is not an inherent method on a concrete device type",
        )
    })?;
    let TyKind::Adt(self_adt, _) = self_ty.kind() else {
        return Err(ClosureProfileErrorV1::new(
            "higher-order capability terminal receiver is not a concrete device type",
        ));
    };
    if crate::trusted_device_items::classify(tcx, self_adt.did()) != Some(expected_self) {
        return Ok(None);
    }
    let signature = tcx.normalize_erasing_regions(
        TypingEnv::fully_monomorphized(),
        tcx.instantiate_bound_regions_with_erased(
            tcx.fn_sig(target.def_id()).instantiate(tcx, target.args),
        ),
    );
    if signature.safety != rustc_hir::Safety::Safe
        || signature.abi != ExternAbi::Rust
        || signature.c_variadic
        || signature.inputs().len() != args.len()
    {
        return Err(ClosureProfileErrorV1::new(format!(
            "authenticated {terminal:?} source signature or Rust ABI changed"
        )));
    }
    let mut closure_inputs = signature
        .inputs()
        .iter()
        .enumerate()
        .filter_map(|(index, input)| matches!(input.kind(), TyKind::Closure(..)).then_some(index));
    let closure_argument = closure_inputs.next().ok_or_else(|| {
        ClosureProfileErrorV1::new(format!(
            "authenticated {terminal:?} source signature has no concrete closure input"
        ))
    })?;
    if closure_inputs.next().is_some() {
        return Err(ClosureProfileErrorV1::new(format!(
            "authenticated {terminal:?} source signature has multiple closure inputs"
        )));
    }
    let closure_ty = normalized_ty(
        tcx,
        caller,
        args[closure_argument].node.ty(body, tcx),
        "higher-order capability closure argument",
    )?;
    if closure_ty != signature.inputs()[closure_argument]
        || !matches!(closure_ty.kind(), TyKind::Closure(..))
    {
        return Err(ClosureProfileErrorV1::new(format!(
            "authenticated {terminal:?} closure type disagrees with its monomorphic source signature"
        )));
    }
    for (index, argument) in args.iter().enumerate() {
        if index != closure_argument
            && operand_mentions_closure(&argument.node, closure_locals, aliases)
        {
            return Err(ClosureProfileErrorV1::new(format!(
                "closure value reached a non-operation argument of authenticated {terminal:?}"
            )));
        }
    }
    let closure_custody = resolve_operand_closure_custody(
        tcx,
        caller,
        &args[closure_argument],
        closure_ty,
        closure_locals,
        aliases,
    )?;
    let TyKind::Closure(_, closure_arguments) = closure_ty.kind() else {
        unreachable!("the monomorphic closure input was checked above");
    };
    let environment = closure_arguments.as_closure();
    let actual_kind = closure_kind(environment.kind_ty().to_opt_closure_kind())?;
    if !call_kind_allowed(actual_kind, ClosureCallKindV1::FnOnce) {
        return Err(ClosureProfileErrorV1::new(format!(
            "authenticated {terminal:?} requires a closure consumable as FnOnce"
        )));
    }
    if !tcx.is_mir_available(target.def_id()) {
        return Err(ClosureProfileErrorV1::new(format!(
            "authenticated {terminal:?} helper MIR is unavailable for recursive collection"
        )));
    }
    let query = TypingEnv::fully_monomorphized().as_query_input((target, ty::List::empty()));
    let fn_abi = tcx.fn_abi_of_instance(query).map_err(|error| {
        ClosureProfileErrorV1::new(format!(
            "authenticated {terminal:?} FnAbi is unavailable: {error:?}"
        ))
    })?;
    let identities =
        crate::rustc_semantic_adapter_v1::canonical_function_identities_v1(tcx, target);
    let source = crate::rustc_semantic_adapter_v1::canonical_source_provenance_v1(
        tcx,
        span,
        MAX_MACRO_EXPANSION_DEPTH,
    )
    .map_err(|error| {
        ClosureProfileErrorV1::new(format!(
            "authenticated {terminal:?} call has invalid source provenance: {error}"
        ))
    })?;
    Ok(Some(HigherOrderCapabilityCallV1 {
        block,
        closure_custody,
        terminal,
        target_definition_hash: tcx.def_path_hash(target.def_id()).0.to_le_bytes(),
        target_function_identity: *identities.function().as_bytes(),
        target_monomorphization_identity: *identities.monomorphization().as_bytes(),
        target_generic_types_identity: *identities.generic_type_arguments().as_bytes(),
        target_const_generics_identity: *identities.const_generic_arguments().as_bytes(),
        target_mir_identity: crate::rustc_semantic_adapter_v1::rustc_mir_body_sha256_v1(
            tcx, target,
        ),
        target_fn_abi_identity: crate::rustc_semantic_adapter_v1::rustc_fn_abi_sha256_v1(
            tcx, fn_abi,
        ),
        call_source_identity: source.expansion_chain_sha256(),
    }))
}

fn resolve_operand_closure_custody<'tcx>(
    tcx: TyCtxt<'tcx>,
    instance: Instance<'tcx>,
    operand: &Spanned<Operand<'tcx>>,
    closure_ty: Ty<'tcx>,
    closure_locals: &BTreeSet<Local>,
    aliases: &BTreeMap<Local, Local>,
) -> Result<HigherOrderClosureCustodyV1, ClosureProfileErrorV1> {
    if let Some(root) = operand_local(&operand.node)
        .and_then(|local| resolve_alias_root(local, closure_locals, aliases))
    {
        return Ok(HigherOrderClosureCustodyV1::EnvironmentLocal(
            root.as_usize(),
        ));
    }
    let Operand::Constant(constant) = &operand.node else {
        return Err(ClosureProfileErrorV1::new(
            "higher-order capability closure operand has no tracked environment custody",
        ));
    };
    let constant_ty = normalized_ty(
        tcx,
        instance,
        constant.const_.ty(),
        "zero-sized higher-order capability closure constant",
    )?;
    if constant_ty != closure_ty {
        return Err(ClosureProfileErrorV1::new(
            "higher-order capability closure constant type disagrees with its call operand",
        ));
    }
    let TyKind::Closure(definition, arguments) = closure_ty.kind() else {
        return Err(ClosureProfileErrorV1::new(
            "higher-order capability constant is not a concrete closure",
        ));
    };
    if !arguments.as_closure().upvar_tys().is_empty()
        || closure_ty.needs_drop(tcx, TypingEnv::fully_monomorphized())
    {
        return Err(ClosureProfileErrorV1::new(
            "only a zero-capture, no-drop closure may use constant custody",
        ));
    }
    let layout = LayoutCx::new(tcx, TypingEnv::fully_monomorphized())
        .layout_of(closure_ty)
        .map_err(|error| {
            ClosureProfileErrorV1::new(format!(
                "zero-capture closure constant layout failed: {error}"
            ))
        })?;
    if layout.size.bytes() != 0 || layout.fields.count() != 0 {
        return Err(ClosureProfileErrorV1::new(
            "constant closure custody requires an exact zero-sized, zero-field environment",
        ));
    }
    let source = crate::rustc_semantic_adapter_v1::canonical_source_provenance_v1(
        tcx,
        tcx.def_span(*definition),
        MAX_MACRO_EXPANSION_DEPTH,
    )
    .map_err(|error| {
        ClosureProfileErrorV1::new(format!(
            "zero-sized closure constant has invalid source provenance: {error}"
        ))
    })?;
    Ok(HigherOrderClosureCustodyV1::ZeroSizedConstant {
        definition_hash: tcx.def_path_hash(*definition).0.to_le_bytes(),
        closure_type_identity: *crate::rustc_semantic_adapter_v1::rustc_type_identity_v1(
            tcx, closure_ty,
        )
        .as_bytes(),
        operand_source_identity: source.expansion_chain_sha256(),
    })
}

fn higher_order_capability_candidate(
    tcx: TyCtxt<'_>,
    func: &Operand<'_>,
) -> Result<
    Option<(
        HigherOrderCapabilityTerminalV1,
        crate::trusted_device_items::TrustedDeviceItem,
    )>,
    ClosureProfileErrorV1,
> {
    let Operand::Constant(constant) = func else {
        return Ok(None);
    };
    let TyKind::FnDef(def_id, _) = constant.const_.ty().kind() else {
        return Ok(None);
    };
    let candidate = match tcx.item_name(*def_id).as_str() {
        "with_workgroup" => (
            HigherOrderCapabilityTerminalV1::WithWorkgroup,
            crate::trusted_device_items::TrustedDeviceItem::KernelContext,
        ),
        "with_matrix" => (
            HigherOrderCapabilityTerminalV1::WithMatrix,
            crate::trusted_device_items::TrustedDeviceItem::ExecutionSubgroupCapability,
        ),
        _ => return Ok(None),
    };
    let Some(self_ty) = inherent_impl_self_ty(tcx, *def_id) else {
        return Ok(None);
    };
    let TyKind::Adt(self_adt, _) = self_ty.kind() else {
        return Ok(None);
    };
    if crate::trusted_device_items::classify(tcx, self_adt.did()) != Some(candidate.1) {
        return Ok(None);
    }
    Ok(Some(candidate))
}

fn inherent_impl_self_ty<'tcx>(tcx: TyCtxt<'tcx>, method: DefId) -> Option<Ty<'tcx>> {
    let associated = tcx.opt_associated_item(method)?;
    if !associated.is_fn() {
        return None;
    }
    let impl_id = tcx.impl_of_assoc(method)?;
    if tcx.impl_is_of_trait(impl_id) {
        return None;
    }
    Some(tcx.type_of(impl_id).instantiate_identity())
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

fn closure_invocation_kind_v1<'tcx>(
    tcx: TyCtxt<'tcx>,
    func: &Operand<'_>,
    target: Instance<'tcx>,
) -> Result<Option<ClosureCallKindV1>, ClosureProfileErrorV1> {
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
    if let Some(trait_id) = tcx.trait_of_assoc(*def_id) {
        if Some(trait_id) == tcx.lang_items().fn_trait() {
            return Ok(Some(ClosureCallKindV1::Fn));
        }
        if Some(trait_id) == tcx.lang_items().fn_mut_trait() {
            return Ok(Some(ClosureCallKindV1::FnMut));
        }
        if Some(trait_id) == tcx.lang_items().fn_once_trait() {
            return Ok(Some(ClosureCallKindV1::FnOnce));
        }
    }
    if matches!(target.def, InstanceKind::ClosureOnceShim { .. }) {
        Ok(Some(ClosureCallKindV1::FnOnce))
    } else if tcx.def_kind(target.def_id()) == DefKind::Closure {
        let signature = tcx.normalize_erasing_regions(
            TypingEnv::fully_monomorphized(),
            tcx.instantiate_bound_regions_with_erased(
                tcx.fn_sig(target.def_id()).instantiate(tcx, target.args),
            ),
        );
        match signature.inputs().first().map(|receiver| receiver.kind()) {
            Some(TyKind::Ref(_, closure, Mutability::Not))
                if matches!(closure.kind(), TyKind::Closure(..)) =>
            {
                Ok(Some(ClosureCallKindV1::Fn))
            }
            Some(TyKind::Ref(_, closure, Mutability::Mut))
                if matches!(closure.kind(), TyKind::Closure(..)) =>
            {
                Ok(Some(ClosureCallKindV1::FnMut))
            }
            Some(TyKind::Closure(..)) => Ok(Some(ClosureCallKindV1::FnOnce)),
            _ => Err(ClosureProfileErrorV1::new(
                "compiler-generated closure body has an invalid receiver ABI",
            )),
        }
    } else {
        Ok(None)
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

fn lowering_identity(
    target: &str,
    environments: &[ClosureEnvironmentV1],
    calls: &[StaticClosureCallV1],
    transport_calls: &[ClosureTransportCallV1],
    higher_order_calls: &[HigherOrderCapabilityCallV1],
) -> [u8; 32] {
    let mut hash = Sha256::new();
    hash.update(b"fe2o3.production-closure-lowering.v3\0");
    hash.update((target.len() as u64).to_le_bytes());
    hash.update(target.as_bytes());
    hash.update((environments.len() as u64).to_le_bytes());
    for environment in environments {
        hash.update((environment.local as u64).to_le_bytes());
        hash.update([environment.origin as u8, environment.call_kind as u8]);
        hash.update(environment.definition_hash);
        hash.update(environment.closure_type_identity);
        hash.update(environment.size_bytes.to_le_bytes());
        hash.update(environment.alignment_bytes.to_le_bytes());
        hash.update((environment.captures.len() as u64).to_le_bytes());
        for capture in &environment.captures {
            hash.update((capture.source_index as u64).to_le_bytes());
            hash.update((capture.memory_index as u64).to_le_bytes());
            hash.update(capture.offset_bytes.to_le_bytes());
            hash.update([capture.mode as u8]);
            hash.update(capture.layout.size_bytes.to_le_bytes());
            hash.update(capture.layout.abi_alignment_bytes.to_le_bytes());
        }
    }
    hash.update((calls.len() as u64).to_le_bytes());
    for call in calls {
        hash.update((call.block as u64).to_le_bytes());
        hash.update((call.closure_local as u64).to_le_bytes());
        hash.update([call.call_kind as u8]);
        hash.update((call.argument_count as u64).to_le_bytes());
        hash.update(call.target_definition_hash);
    }
    hash.update((transport_calls.len() as u64).to_le_bytes());
    for call in transport_calls {
        hash.update((call.block as u64).to_le_bytes());
        hash.update((call.closure_local as u64).to_le_bytes());
        hash.update((call.argument_index as u64).to_le_bytes());
        hash.update(call.target_definition_hash);
        hash.update(call.target_function_identity);
        hash.update(call.target_monomorphization_identity);
        hash.update(call.target_mir_identity);
        hash.update(call.target_fn_abi_identity);
        hash.update(call.call_source_identity);
    }
    hash.update((higher_order_calls.len() as u64).to_le_bytes());
    for call in higher_order_calls {
        hash.update((call.block as u64).to_le_bytes());
        match &call.closure_custody {
            HigherOrderClosureCustodyV1::EnvironmentLocal(local) => {
                hash.update([0]);
                hash.update((*local as u64).to_le_bytes());
            }
            HigherOrderClosureCustodyV1::ZeroSizedConstant {
                definition_hash,
                closure_type_identity,
                operand_source_identity,
            } => {
                hash.update([1]);
                hash.update(definition_hash);
                hash.update(closure_type_identity);
                hash.update(operand_source_identity);
            }
        }
        hash.update([call.terminal as u8]);
        hash.update(call.target_definition_hash);
        hash.update(call.target_function_identity);
        hash.update(call.target_monomorphization_identity);
        hash.update(call.target_generic_types_identity);
        hash.update(call.target_const_generics_identity);
        hash.update(call.target_mir_identity);
        hash.update(call.target_fn_abi_identity);
        hash.update(call.call_source_identity);
    }
    hash.finalize().into()
}

#[cfg(test)]
mod tests;

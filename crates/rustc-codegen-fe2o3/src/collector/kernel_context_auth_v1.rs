//! Authenticate generated entry metadata against exact compiler items and MIR.

use super::*;
use crate::rustc_semantic_plan_v1::source_signature_v1;
use crate::trusted_device_items::{self, TrustedDeviceItem};
use rustc_middle::ty::{self, GenericArgKind, Ty};
use rustc_span::sym;
use rustc_target::callconv::PassMode;

#[path = "kernel_context_flow_v1.rs"]
mod flow;

pub(crate) struct CapturedContextProducersV1<'tcx> {
    pub(super) declarations: Vec<kernel_context_frontend_v1::DeclaredContextEntryV1<'tcx>>,
    proofs: Vec<CapturedProducerV1<'tcx>>,
}

struct CapturedProducerV1<'tcx> {
    root: Instance<'tcx>,
    helper: Instance<'tcx>,
    marker: DefId,
    context: Ty<'tcx>,
    flow: flow::SourceFlowV1<'tcx>,
}

pub(crate) fn capture_context_producers_v1<'tcx>(
    tcx: TyCtxt<'tcx>,
) -> Result<CapturedContextProducersV1<'tcx>, CollectError> {
    let declarations = kernel_context_frontend_v1::decode_v1(tcx)?;
    let mut proofs: Vec<CapturedProducerV1<'tcx>> = Vec::with_capacity(declarations.len());
    for declaration in &declarations {
        let root = declaration.target;
        if proofs.iter().any(|proof| proof.root == root) {
            return Err(error("duplicate declared context root"));
        }
        let local = root
            .def_id()
            .as_local()
            .ok_or_else(|| error("context producer must be a local physical root"))?;
        if !matches!(root.def, InstanceKind::Item(_))
            || tcx.def_kind(root.def_id()) != rustc_hir::def::DefKind::Fn
            || tcx.hir_maybe_body_owned_by(local).is_none()
        {
            return Err(error(
                "context producer requires an ordinary local function body",
            ));
        }
        let helper_def = sibling_definition(
            tcx,
            root.def_id(),
            declaration.bound.contract.logical_helper().name(),
        )?;
        if tcx.def_kind(helper_def) != rustc_hir::def::DefKind::Fn
            || !tcx.generics_of(helper_def).own_params.is_empty()
        {
            return Err(error("logical helper must be a nongeneric Rust function"));
        }
        let helper = Instance::mono(tcx, helper_def);
        let (marker, marker_ty) = marker_definition(
            tcx,
            root.def_id(),
            declaration.bound.contract.nominal_kernel_marker().name(),
        )?;
        let context = authenticate_signature(tcx, root, helper, marker_ty)?;
        // This query is borrowed before monomorphization can steal it. No
        // optimized-MIR query or body-hashing helper may run under this borrow.
        let source = tcx.mir_drops_elaborated_and_const_checked(local);
        if source.is_stolen() {
            return Err(error(
                "pre-optimization context producer MIR is already unavailable",
            ));
        }
        let source = source.borrow();
        let flow = flow::authenticate_source(tcx, root, helper, context, &source)?;
        proofs.push(CapturedProducerV1 {
            root,
            helper,
            marker,
            context,
            flow,
        });
    }
    Ok(CapturedContextProducersV1 {
        declarations,
        proofs,
    })
}

pub(super) fn error(detail: impl fmt::Display) -> CollectError {
    CollectError {
        message: format!("[FE2O3-CAP-AUTH001] kernel-context producer: {detail}"),
    }
}

pub(super) fn authenticate_v1(collector: &mut DeviceCollector<'_>) -> Result<(), CollectError> {
    let tcx = collector.tcx;
    let mut authenticated = Vec::new();
    for (index, function) in collector.result.iter().enumerate() {
        let body = tcx.instance_mir(function.instance.def);
        let issuance_count = body.basic_blocks.iter().filter(|block| {
            let Some(terminator) = &block.terminator else { return false };
            let TerminatorKind::Call { func, .. } = &terminator.kind else { return false };
            matches!(func.ty(body, tcx).kind(), TyKind::FnDef(def, _)
                if trusted_device_items::classify(tcx, *def) == Some(TrustedDeviceItem::KernelContextIssue))
        }).count();
        let Some(bound) = &function.kernel_context_contract else {
            if issuance_count != 0 {
                return Err(error(format!(
                    "issuance outside a declared physical root: {}",
                    tcx.def_path_str(function.instance.def_id())
                )));
            }
            continue;
        };
        if function.role != CollectedFunctionRole::KernelEntry
            || issuance_count != 1
            || bound.authenticated_items.is_some()
        {
            return Err(error("a declared physical root must issue exactly once"));
        }
        if fe2o3_rustc_front::decode_kernel_context_frontend_contract_v1(&bound.canonical_bytes)
            .as_ref()
            != Ok(&bound.contract)
        {
            return Err(error("bound declaration bytes changed"));
        }
        let root = function.instance;
        let mut helpers = collector.result.iter().filter(|helper| {
            sibling(
                tcx,
                root.def_id(),
                helper.instance.def_id(),
                bound.contract.logical_helper().name(),
            )
        });
        let helper = helpers.next().ok_or_else(|| {
            error(format!(
                "declaration {} has no reachable logical helper",
                bound.registration_path
            ))
        })?;
        if helpers.next().is_some() || helper.role != CollectedFunctionRole::InternalHelper {
            return Err(error(
                "logical helper is ambiguous or is itself a registered entry",
            ));
        }
        let (marker, marker_ty) = marker_definition(
            tcx,
            root.def_id(),
            bound.contract.nominal_kernel_marker().name(),
        )?;
        let context = authenticate_signature(tcx, root, helper.instance, marker_ty)?;
        let source = collector
            .context_producers
            .proofs
            .iter()
            .find(|source| source.root == root)
            .ok_or_else(|| error("context producer has no retained source-flow evidence"))?;
        if source.helper != helper.instance || source.marker != marker || source.context != context
        {
            return Err(error("retained source-flow item or type identity changed"));
        }
        let issuer = flow::authenticate_optimized(tcx, &source.flow)?;
        // A helper must not call back into a physical root and acquire a second context.
        let identity = collector.instance_identity(root);
        if collector
            .call_edges
            .values()
            .any(|callees| callees.contains(&identity))
        {
            return Err(error("physical context root is reachable as a callee"));
        }
        authenticated.push((
            index,
            [
                root.def_id(),
                helper.instance.def_id(),
                marker,
                issuer.def_id(),
            ],
        ));
    }
    if authenticated.len() != collector.context_producers.proofs.len() {
        return Err(error("retained source-flow evidence has unmatched roots"));
    }
    for (index, items) in authenticated {
        collector.result[index]
            .kernel_context_contract
            .as_mut()
            .ok_or_else(|| error("authenticated declaration disappeared"))?
            .authenticated_items = Some(items);
    }
    Ok(())
}

fn sibling(tcx: TyCtxt<'_>, owner: DefId, candidate: DefId, name: &str) -> bool {
    tcx.opt_parent(owner) == tcx.opt_parent(candidate)
        && tcx
            .def_key(candidate)
            .get_opt_name()
            .is_some_and(|actual| actual.as_str() == name)
}

fn sibling_definition(tcx: TyCtxt<'_>, root: DefId, name: &str) -> Result<DefId, CollectError> {
    let mut definitions = tcx
        .hir_free_items()
        .map(|id| tcx.hir_item(id).owner_id.def_id.to_def_id())
        .filter(|def| sibling(tcx, root, *def, name));
    let definition = definitions
        .next()
        .ok_or_else(|| error(format!("declared sibling {name} is absent")))?;
    if definitions.next().is_some() {
        return Err(error("declared sibling is ambiguous"));
    }
    Ok(definition)
}

fn marker_definition<'tcx>(
    tcx: TyCtxt<'tcx>,
    root: DefId,
    name: &str,
) -> Result<(DefId, Ty<'tcx>), CollectError> {
    let marker = sibling_definition(tcx, root, name)?;
    if tcx.def_kind(marker) != rustc_hir::def::DefKind::Enum
        || !tcx.generics_of(marker).own_params.is_empty()
    {
        return Err(error("nominal marker is not a nongeneric enum"));
    }
    let ty = tcx.type_of(marker).instantiate_identity();
    let TyKind::Adt(adt, arguments) = ty.kind() else {
        return Err(error("nominal marker has no ADT type"));
    };
    if !adt.is_enum() || !adt.variants().is_empty() || !arguments.is_empty() {
        return Err(error(
            "nominal marker must be an uninhabited nongeneric enum",
        ));
    }
    Ok((marker, ty))
}

fn authenticate_signature<'tcx>(
    tcx: TyCtxt<'tcx>,
    root: Instance<'tcx>,
    helper: Instance<'tcx>,
    marker: Ty<'tcx>,
) -> Result<Ty<'tcx>, CollectError> {
    let physical = source_signature_v1(tcx, root).map_err(error)?;
    let logical = source_signature_v1(tcx, helper).map_err(error)?;
    let Some((&context, remaining)) = logical.inputs().split_first() else {
        return Err(error("logical helper has no context argument"));
    };
    if remaining != physical.inputs()
        || logical.abi != physical.abi
        || logical.safety != physical.safety
        || logical.c_variadic
        || physical.c_variadic
    {
        return Err(error(
            "physical/logical signatures differ beyond context ordinal zero",
        ));
    }
    let discard_result = physical.output().is_unit() && exact_kernel_result(tcx, logical.output());
    if logical.output() != physical.output() && !discard_result {
        return Err(error(
            "physical/logical return types differ beyond trusted KernelResult normalization",
        ));
    }
    let TyKind::Adt(context_adt, arguments) = context.kind() else {
        return Err(error("logical input zero is not KernelContext"));
    };
    if trusted_device_items::classify(tcx, context_adt.did())
        != Some(TrustedDeviceItem::KernelContext)
    {
        return Err(error("logical context provider is not authentic"));
    }
    if arguments.len() != 4
        || !matches!(arguments[0].kind(), GenericArgKind::Lifetime(_))
        || arguments[1].as_type() != Some(marker)
        || !exact_marker(
            tcx,
            arguments[2].as_type(),
            context_adt.did(),
            "fe2o3_device::context::CurrentTarget",
        )
        || !exact_marker(
            tcx,
            arguments[3].as_type(),
            context_adt.did(),
            "fe2o3_device::context::RegisteredLaunch",
        )
    {
        return Err(error("logical context kernel/target/launch brands differ"));
    }
    let env = TypingEnv::fully_monomorphized();
    let root_abi = tcx
        .fn_abi_of_instance(env.as_query_input((root, ty::List::empty())))
        .map_err(|e| error(format!("physical FnAbi unavailable: {e:?}")))?;
    let helper_abi = tcx
        .fn_abi_of_instance(env.as_query_input((helper, ty::List::empty())))
        .map_err(|e| error(format!("logical FnAbi unavailable: {e:?}")))?;
    let Some(context_abi) = helper_abi.args.first() else {
        return Err(error("logical FnAbi lost the context"));
    };
    if context_abi.layout.ty != context
        || context_abi.layout.size.bytes() != 0
        || !matches!(context_abi.mode, PassMode::Ignore)
        || root_abi.args.len() != physical.inputs().len()
        || helper_abi.args.len() != logical.inputs().len()
        || helper_abi.args.len().checked_sub(1) != Some(root_abi.args.len())
        || !helper_abi.args[1..]
            .iter()
            .zip(root_abi.args.iter())
            .all(|(a, b)| a.eq_abi(b))
        || helper_abi.conv != root_abi.conv
        || helper_abi.can_unwind != root_abi.can_unwind
        || helper_abi.c_variadic != root_abi.c_variadic
        || helper_abi.fixed_count.checked_sub(1) != Some(root_abi.fixed_count)
    {
        return Err(error(
            "physical/logical FnAbi does not remove only one ignored context",
        ));
    }
    if discard_result {
        if !root_abi.ret.layout.ty.is_unit() || !matches!(root_abi.ret.mode, PassMode::Ignore) {
            return Err(error(
                "discarded KernelResult did not produce an ignored unit return",
            ));
        }
    } else if !helper_abi.ret.eq_abi(&root_abi.ret) {
        return Err(error("physical/logical return FnAbi differs"));
    }
    Ok(context)
}

fn exact_marker<'tcx>(tcx: TyCtxt<'tcx>, ty: Option<Ty<'tcx>>, context: DefId, path: &str) -> bool {
    let Some(ty) = ty else { return false };
    let TyKind::Adt(adt, arguments) = ty.kind() else {
        return false;
    };
    arguments.is_empty()
        && adt.did().krate == context.krate
        && trusted_device_items::reviewed_provider_semantic_definition_v1(tcx, adt.did())
            .is_ok_and(|definition| definition.canonical_definition_path == path)
}

fn exact_kernel_result<'tcx>(tcx: TyCtxt<'tcx>, ty: Ty<'tcx>) -> bool {
    let TyKind::Adt(result, arguments) = ty.kind() else {
        return false;
    };
    if !tcx.is_diagnostic_item(sym::Result, result.did()) || arguments.len() != 2 {
        return false;
    }
    let (Some(ok), Some(err)) = (arguments[0].as_type(), arguments[1].as_type()) else {
        return false;
    };
    let TyKind::Adt(err, arguments) = err.kind() else {
        return false;
    };
    ok.is_unit()
        && arguments.is_empty()
        && trusted_device_items::classify(tcx, err.did()) == Some(TrustedDeviceItem::KernelError)
}

pub(super) fn resolve_call<'tcx>(
    tcx: TyCtxt<'tcx>,
    caller: Instance<'tcx>,
    body: &Body<'tcx>,
    function: &Operand<'tcx>,
) -> Result<Instance<'tcx>, CollectError> {
    let callable = caller
        .try_instantiate_mir_and_normalize_erasing_regions(
            tcx,
            TypingEnv::fully_monomorphized(),
            EarlyBinder::bind(function.ty(body, tcx)),
        )
        .map_err(|_| error("callable type failed monomorphic normalization"))?;
    let TyKind::FnDef(def, arguments) = callable.kind() else {
        return Err(error("entry protocol contains an indirect call"));
    };
    Instance::try_resolve(tcx, TypingEnv::fully_monomorphized(), *def, arguments)
        .map_err(|_| error("entry callee resolution failed"))?
        .ok_or_else(|| error("entry callee has no concrete instance"))
}

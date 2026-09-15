//! Original guarded Grid leader attachment. No terminalization or ZST default.
use super::*;
use fe2o3_mir_model::semantic_mir_v1::{
    SemanticDefinedCapabilityContractV1, SemanticFunctionDeclV1, SemanticGuardedGridLeaderSourceV1,
    SemanticGuardedGridLeaderTypesV1, SemanticGuardedGridLeaderV1,
};
use rustc_middle::ty::{InstanceKind, TypeVisitableExt, TypingEnv};

const GETTER: &str = "fe2o3_device::group::Grid::leader";
const ISSUER: &str = "fe2o3_device::thread::GridLeader::from_grid";
const GRID_GETTER: &str = "fe2o3_device::context::KernelContext::grid";
const GRID_CURRENT: &str = "fe2o3_device::group::Grid::current_branded";

fn rejected(message: &'static str) -> ProductionSemanticImportErrorV1 {
    ProductionSemanticImportErrorV1::KernelContextBinding(message)
}
fn is(tcx: TyCtxt<'_>, instance: Instance<'_>, path: &str) -> bool {
    trusted_device_items::is_exact_reviewed_provider_definition_v1(tcx, instance.def_id(), path)
}
fn checked_signature<'tcx>(
    tcx: TyCtxt<'tcx>,
    instance: Instance<'tcx>,
    path: &str,
) -> Result<rustc_middle::ty::FnSig<'tcx>, ProductionSemanticImportErrorV1> {
    if !is(tcx, instance, path)
        || !matches!(instance.def, InstanceKind::Item(_))
        || !tcx.is_mir_available(instance.def_id())
        || instance.args.len() != tcx.generics_of(instance.def_id()).count()
        || instance.args.consts().next().is_some()
        || !trusted_device_items::authenticate_reviewed_safe_external_helper_v1(
            tcx,
            instance.def_id(),
        )
        .map_err(|_| rejected("guarded Grid leader original provider authentication"))?
    {
        return Err(rejected(
            "guarded Grid leader requires the reviewed original Item body",
        ));
    }
    let signature = tcx.instantiate_bound_regions_with_erased(
        tcx.fn_sig(instance.def_id())
            .instantiate(tcx, instance.args),
    );
    let signature = tcx
        .try_normalize_erasing_regions(TypingEnv::fully_monomorphized(), signature)
        .map_err(|_| rejected("guarded Grid leader signature normalization"))?;
    if signature.safety != rustc_hir::Safety::Safe
        || signature.abi != rustc_abi::ExternAbi::Rust
        || signature.c_variadic
        || signature.has_non_region_param()
        || signature.has_infer()
        || signature.has_aliases()
        || signature.has_escaping_bound_vars()
    {
        return Err(rejected(
            "guarded Grid leader requires exact safe monomorphic Rust ABI",
        ));
    }
    Ok(signature)
}
fn option<'tcx>(
    tcx: TyCtxt<'tcx>,
    ty: Ty<'tcx>,
) -> Result<Ty<'tcx>, ProductionSemanticImportErrorV1> {
    let TyKind::Adt(def, args) = *ty.kind() else {
        return Err(rejected("guarded Grid leader Option nominal identity"));
    };
    if Some(def.did()) != tcx.lang_items().option_type() || args.len() != 1 {
        return Err(rejected(
            "guarded Grid leader requires the original core Option",
        ));
    }
    args[0]
        .as_type()
        .ok_or_else(|| rejected("guarded Grid leader Option type argument"))
}
fn nominal_brand<'tcx>(
    tcx: TyCtxt<'tcx>,
    ty: Ty<'tcx>,
    path: &str,
) -> Result<Ty<'tcx>, ProductionSemanticImportErrorV1> {
    let arguments = rust_exact_reviewed_adt_arguments_v1(tcx, ty, path)
        .ok_or_else(|| rejected("guarded Grid leader exact branded nominal type"))?;
    // Grid carries an erased invocation lifetime; GridLeader has only its brand.
    let mut type_arguments = arguments.types();
    let Some(brand) = type_arguments.next() else {
        return Err(rejected("guarded Grid leader type arity"));
    };
    if type_arguments.next().is_some() || arguments.consts().next().is_some() {
        return Err(rejected("guarded Grid leader type arity"));
    }
    Ok(brand)
}

mod source_work;
use source_work::{one, reserve, spend};

fn allowance() -> Result<usize, ProductionSemanticImportErrorV1> {
    usize::try_from(SemanticMirLimitsV1::default().limit(SemanticMirResourceV1::ValidationWork))
        .map_err(|_| rejected("guarded Grid leader source-work width"))
}

fn derive<'tcx>(
    tcx: TyCtxt<'tcx>,
    plan: &ProductionSemanticPreflightPlanV1<'tcx>,
    types: &[SemanticTypeDeclV1],
    functions: &[SemanticFunctionDeclV1],
    callables: &[SemanticCallableDeclV1],
    contexts: &AuthenticatedProductionKernelContextsV1,
    work: &mut usize,
) -> Result<Vec<(SemanticFunctionIdV1, SemanticGuardedGridLeaderV1)>, ProductionSemanticImportErrorV1>
{
    spend(work, plan.function_producers().len())?;
    let mut candidates = [SemanticFunctionIdV1::from_index(0); 64];
    let mut count = 0;
    for (index, producer) in plan.function_producers().iter().enumerate() {
        if is(tcx, producer.instance, GETTER) {
            let slot = candidates
                .get_mut(count)
                .ok_or_else(|| rejected("guarded Grid leader source-record bound"))?;
            *slot = SemanticFunctionIdV1::from_index(
                u32::try_from(index)
                    .map_err(|_| rejected("guarded Grid leader function index width"))?,
            );
            count += 1;
        }
    }
    if count == 0 {
        return Ok(Vec::new());
    }
    // The shared exact type/FnABI converters retain their existing independent
    // construction limits. Account this observer's table/index walk separately;
    // no claim that these row charges replace recursive converter accounting.
    for rows in [types.len(), functions.len(), callables.len()] {
        spend(
            work,
            rows.checked_mul(16)
                .ok_or_else(|| rejected("guarded Grid leader source-work overflow"))?,
        )?;
    }
    let roster =
        numerical_policy_v1::defined_source_roster_v1(tcx, plan, types, functions, callables)?;
    let mut replay = source_body_v1::Replay::new(tcx, plan, work)?;
    let mut edges = reserve(plan.direct_call_producers().len(), work)?;
    edges.extend(
        plan.direct_call_producers()
            .iter()
            .map(|c| (c.caller, c.callee)),
    );
    let mut records = reserve(count, work)?;
    for function in candidates[..count].iter().copied() {
        spend(
            work,
            functions
                .len()
                .checked_mul(
                    edges
                        .len()
                        .checked_add(1)
                        .ok_or_else(|| rejected("guarded Grid leader source-work overflow"))?,
                )
                .ok_or_else(|| rejected("guarded Grid leader source-work overflow"))?,
        )?;
        // Four complete borrowed call-roster filters and eight type lookups.
        spend(
            work,
            edges
                .len()
                .checked_mul(4)
                .and_then(|n| types.len().checked_mul(8).and_then(|m| n.checked_add(m)))
                .ok_or_else(|| rejected("guarded Grid leader source-work overflow"))?,
        )?;
        let root = authenticate_capability_memory_root_v1(
            contexts,
            &BTreeSet::from([function]),
            &edges,
            false,
        )?;
        roster.root(root, contexts)?;
        let instance = roster.defined(function)?;
        let signature = checked_signature(tcx, instance, GETTER)?;
        let [grid_reference] = signature.inputs() else {
            return Err(rejected("guarded Grid leader receiver arity"));
        };
        let grid = rust_shared_reference_v1(*grid_reference)
            .ok_or_else(|| rejected("guarded Grid leader shared receiver"))?;
        let brand_ty = nominal_brand(tcx, grid, "fe2o3_device::group::Grid")?;
        let leader = option(tcx, signature.output())?;
        if nominal_brand(tcx, leader, "fe2o3_device::thread::GridLeader")? != brand_ty {
            return Err(rejected(
                "guarded Grid leader substituted receiver/output brand",
            ));
        }
        let brand = rust_kernel_brand_v1(tcx, brand_ty)
            .ok_or_else(|| rejected("guarded Grid leader requires a kernel brand"))?;
        if !rust_kernel_brand_matches_root_v1(tcx, brand, root) {
            return Err(rejected(
                "guarded Grid leader receiver belongs to another physical root",
            ));
        }
        for (ty, path) in [
            (brand.target, "fe2o3_device::context::CurrentTarget"),
            (brand.launch, "fe2o3_device::context::RegisteredLaunch"),
        ] {
            if !rust_exact_reviewed_adt_arguments_v1(tcx, ty, path)
                .is_some_and(|args| args.is_empty())
            {
                return Err(rejected("guarded Grid leader target/launch substitution"));
            }
        }
        // One exact incoming source occurrence in this first closed recipe.
        // Repeated dynamic expansion of that occurrence is handled by SSA.
        let incoming = one(
            plan.direct_call_producers()
                .iter()
                .filter(|c| c.callee == function),
            "guarded Grid leader requires one original caller occurrence",
        )?;
        let caller = incoming.caller;
        let caller_map = roster.reconstructed_body(caller, &mut replay, work)?;
        let call_block = caller_map.block(incoming.block)?;
        let grid_call = one(
            plan.direct_call_producers()
                .iter()
                .filter(|c| c.caller == caller)
                .filter(|c| {
                    plan.function_producers()
                        .get(c.callee.index() as usize)
                        .is_some_and(|p| is(tcx, p.instance, GRID_GETTER))
                }),
            "guarded Grid leader requires its exact Context grid occurrence",
        )?;
        let grid_signature =
            checked_signature(tcx, roster.defined(grid_call.callee)?, GRID_GETTER)?;
        let [context_reference] = grid_signature.inputs() else {
            return Err(rejected("guarded Grid leader Context getter arity"));
        };
        let context = rust_shared_reference_v1(*context_reference)
            .ok_or_else(|| rejected("guarded Grid leader shared Context"))?;
        let (kernel, target, launch) = rust_kernel_context_axes_v1(tcx, context)
            .ok_or_else(|| rejected("guarded Grid leader nominal Context"))?;
        if kernel != brand.kernel
            || target != brand.target
            || launch != brand.launch
            || option(tcx, grid_signature.output())? != grid
        {
            return Err(rejected(
                "guarded Grid leader Context/grid exact type mismatch",
            ));
        }
        let issue = one(
            plan.direct_call_producers()
                .iter()
                .filter(|c| c.caller == function),
            "guarded Grid leader exact issuer call roster",
        )?;
        let issue_signature = checked_signature(tcx, roster.defined(issue.callee)?, ISSUER)?;
        if !issue_signature.inputs().is_empty() || issue_signature.output() != leader {
            return Err(rejected("guarded Grid leader issuer ABI"));
        }
        let bridge = one(
            plan.direct_call_producers()
                .iter()
                .filter(|c| c.caller == grid_call.callee),
            "guarded Grid leader exact grid-current call roster",
        )?;
        let current_signature =
            checked_signature(tcx, roster.defined(bridge.callee)?, GRID_CURRENT)?;
        if !current_signature.inputs().is_empty()
            || current_signature.output() != grid_signature.output()
        {
            return Err(rejected("guarded Grid leader current Grid ABI"));
        }
        let ids = roster.types([
            *grid_reference,
            grid,
            leader,
            signature.output(),
            tcx.types.u64,
            tcx.types.bool,
            grid_signature.output(),
            *context_reference,
        ])?;
        // Full reconstruction checks operations and every original source endpoint;
        // nominal identity or a caller-supplied body digest is not sufficient.
        for id in [function, issue.callee, grid_call.callee, bridge.callee] {
            roster.reconstructed_body(id, &mut replay, work)?;
        }
        let source = SemanticGuardedGridLeaderSourceV1 {
            caller,
            call_block,
            grid_getter: grid_call.callee,
            grid_current: bridge.callee,
            grid_call_block: caller_map.block(grid_call.block)?,
        };
        let mut model_work = *work as u64;
        let record = SemanticGuardedGridLeaderV1::for_defined_function(
            function,
            functions,
            callables,
            types,
            SemanticGuardedGridLeaderTypesV1::new(ids),
            source,
            capability_memory_provenance_v1(root, contexts)?,
            rustc_type_identity_v1(tcx, brand_ty),
            &mut model_work,
        )
        .map_err(|_| {
            rejected("guarded Grid leader retained body, receiver or Option custody mismatch")
        })?;
        *work = usize::try_from(model_work)
            .map_err(|_| rejected("guarded Grid leader remaining work width"))?;
        records.push((function, record));
    }
    Ok(records)
}

pub(super) fn attach<'tcx>(
    tcx: TyCtxt<'tcx>,
    plan: &ProductionSemanticPreflightPlanV1<'tcx>,
    types: &[SemanticTypeDeclV1],
    functions: &mut [SemanticFunctionDeclV1],
    callables: &[SemanticCallableDeclV1],
    contexts: &AuthenticatedProductionKernelContextsV1,
) -> Result<(), ProductionSemanticImportErrorV1> {
    let mut work = allowance()?;
    let records = derive(tcx, plan, types, functions, callables, contexts, &mut work)?;
    let mut pending = reserve(records.len(), &mut work)?;
    for (function, record) in records {
        // Only authenticated fixed five-block getter profiles are cloned.
        // The complete function arena is never cloned by this observer.
        spend(&mut work, 128)?;
        let attached = functions[function.index() as usize]
            .clone()
            .with_defined_capability_contract(
                SemanticDefinedCapabilityContractV1::GuardedGridLeader(record),
            )
            .map_err(|_| rejected("guarded Grid leader attachment mismatch"))?;
        pending.push((function, attached));
    }
    for (id, function) in pending {
        functions[id.index() as usize] = function;
    }
    Ok(())
}

pub(super) fn validate_carriage<'tcx>(
    tcx: TyCtxt<'tcx>,
    plan: &ProductionSemanticPreflightPlanV1<'tcx>,
    contexts: &AuthenticatedProductionKernelContextsV1,
    mir: &AdmittedInertSemanticMirV1,
) -> Result<(), ProductionSemanticImportErrorV1> {
    let mut work = allowance()?;
    spend(&mut work, mir.functions().len())?;
    for (i, f) in mir.functions().iter().enumerate() {
        if matches!(
            f.defined_capability_contract(),
            Some(SemanticDefinedCapabilityContractV1::GuardedGridLeader(_))
        ) && !plan
            .function_producers()
            .get(i)
            .is_some_and(|p| is(tcx, p.instance, GETTER))
        {
            return Err(rejected(
                "guarded Grid leader forged original source attachment",
            ));
        }
    }
    let expected = derive(
        tcx,
        plan,
        mir.types(),
        mir.functions(),
        mir.callables(),
        contexts,
        &mut work,
    )?;
    for (function, record) in expected {
        match mir.functions()[function.index() as usize].defined_capability_contract() {
            Some(actual)
                if *actual == SemanticDefinedCapabilityContractV1::GuardedGridLeader(record) => {}
            Some(_) => return Err(rejected("guarded Grid leader attachment mismatch")),
            None => {
                return Err(rejected(
                    "guarded Grid leader omitted or substituted full source carriage",
                ));
            }
        }
    }
    Ok(())
}

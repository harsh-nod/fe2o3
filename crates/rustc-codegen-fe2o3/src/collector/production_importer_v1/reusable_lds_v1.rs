//! Reusable LDS conversion import. Call only after the canonical
//! function/type/callable loop. Replay from the existing live preflight owner
//! and authenticated root context before SSA/lowering. No parallel authority graph.
use super::*;
use crate::rustc_semantic_adapter_v1::rustc_mir_body_sha256_v1;
use fe2o3_mir_model::semantic_mir_v1::{
    SemanticDefinedCapabilityContractV1, SemanticFunctionDeclV1, SemanticReusableLdsConversionV1,
    SemanticReusableLdsSourceV1, SemanticReusableLdsTypesV1,
};
use rustc_middle::ty::TypingEnv;

#[path = "reusable_lds_v1/body.rs"]
mod body;
#[path = "reusable_lds_v1/occurrence.rs"]
mod occurrence;
#[path = "reusable_lds_v1/mapping_diagnostic.rs"]
mod mapping_diagnostic;
#[cfg(test)]
#[path = "reusable_lds_v1/source_replay_tests.rs"]
mod source_replay_tests;

const CONVERSION: &str = "fe2o3_device::execution::WorkgroupLds::into_reusable";
const MAX_CONVERSIONS: usize = 256;

fn charge(work: &mut usize, amount: usize) -> Result<(), ProductionSemanticImportErrorV1> {
    *work = work
        .checked_sub(amount)
        .ok_or_else(|| rejected("reusable LDS source-work ceiling"))?;
    Ok(())
}

#[derive(Debug, Default)]
pub(super) struct AuthenticatedReusableLdsSourcesV1 {
    records: Box<[SemanticReusableLdsConversionV1]>,
}

fn rejected(message: &'static str) -> ProductionSemanticImportErrorV1 {
    ProductionSemanticImportErrorV1::KernelContextBinding(message)
}

fn nominal<'tcx>(
    tcx: TyCtxt<'tcx>,
    ty: Ty<'tcx>,
    path: &'static str,
) -> Result<rustc_hir::def_id::DefId, ProductionSemanticImportErrorV1> {
    if rust_exact_reviewed_adt_arguments_v1(tcx, ty, path).is_none() {
        return Err(rejected(
            "reusable LDS requires the exact reviewed nominal type",
        ));
    }
    let TyKind::Adt(def, _) = *ty.kind() else {
        return Err(rejected("reusable LDS non-ADT type"));
    };
    Ok(def.did())
}

fn definition<'tcx>(
    tcx: TyCtxt<'tcx>,
    instance: Instance<'tcx>,
) -> Result<body::Types<'tcx>, ProductionSemanticImportErrorV1> {
    if !trusted_device_items::is_exact_reviewed_provider_definition_v1(
        tcx,
        instance.def_id(),
        CONVERSION,
    ) || !trusted_device_items::authenticate_reviewed_safe_external_helper_v1(
        tcx,
        instance.def_id(),
    )
    .map_err(|_| rejected("reusable LDS reviewed source authentication failed"))?
        || !matches!(instance.def, rustc_middle::ty::InstanceKind::Item(_))
        || !tcx.is_mir_available(instance.def_id())
    {
        return Err(rejected(
            "reusable LDS requires its reviewed original external definition",
        ));
    }
    let original = tcx.instance_mir(instance.def);
    if original.arg_count != 1
        || original.local_decls.len() != 2
        || original.basic_blocks.len() != 1
    {
        return Err(rejected("reusable LDS original definition role counts"));
    }
    let input = body::normalize(
        tcx,
        instance,
        original.local_decls[rustc_middle::mir::Local::from_usize(1)].ty,
    )
    .ok_or_else(|| rejected("reusable LDS input normalization"))?;
    let output = body::normalize(tcx, instance, original.return_ty())
        .ok_or_else(|| rejected("reusable LDS output normalization"))?;
    let input_id = nominal(tcx, input, "fe2o3_device::execution::WorkgroupLds")?;
    let output_id = nominal(tcx, output, "fe2o3_device::execution::ReusableWorkgroupLds")?;
    let TyKind::Adt(def, args) = *input.kind() else {
        unreachable!()
    };
    if args.len() != 6 || !def.is_struct() || def.non_enum_variant().fields.len() != 5 {
        return Err(rejected("reusable LDS exact nominal generic/field roster"));
    }
    let state = args[3]
        .as_type()
        .ok_or_else(|| rejected("reusable LDS state type"))?;
    let uninitialized = nominal(
        tcx,
        state,
        "fe2o3_device::execution::WorkgroupLdsUninitialized",
    )?;
    let marker = tcx
        .try_normalize_erasing_regions(
            TypingEnv::fully_monomorphized(),
            def.non_enum_variant().fields[rustc_abi::FieldIdx::from_usize(2)].ty(tcx, args),
        )
        .map_err(|_| rejected("reusable LDS Workgroup marker normalization"))?;
    let TyKind::Adt(phantom, params) = *marker.kind() else {
        return Err(rejected("reusable LDS missing Workgroup marker"));
    };
    if Some(phantom.did()) != tcx.lang_items().phantom_data() || params.len() != 1 {
        return Err(rejected("reusable LDS substituted Workgroup marker"));
    }
    let inner = params[0]
        .as_type()
        .ok_or_else(|| rejected("reusable LDS non-type Workgroup marker"))?;
    let TyKind::FnPtr(signature, _) = *inner.kind() else {
        return Err(rejected("reusable LDS Workgroup variance"));
    };
    let wg = signature
        .skip_binder()
        .inputs_and_output
        .first()
        .copied()
        .ok_or_else(|| rejected("reusable LDS Workgroup variance arity"))?;
    let workgroup_brand = nominal(tcx, wg, "fe2o3_device::execution::WorkgroupBrand")?;
    body::observe(
        tcx,
        instance,
        original,
        body::Nominal {
            input: input_id,
            output: output_id,
            uninitialized,
            workgroup_brand,
        },
    )
    .map_err(|_| {
        rejected("reusable LDS original definition body, generic identity, field or ABI mismatch")
    })
}

fn observe<'tcx>(
    tcx: TyCtxt<'tcx>,
    plan: &ProductionSemanticPreflightPlanV1<'tcx>,
    types: &[SemanticTypeDeclV1],
    functions: &[SemanticFunctionDeclV1],
    callables: &[SemanticCallableDeclV1],
    contexts: &AuthenticatedProductionKernelContextsV1,
) -> Result<AuthenticatedReusableLdsSourcesV1, ProductionSemanticImportErrorV1> {
    let mut work = usize::try_from(
        SemanticMirLimitsV1::default().limit(SemanticMirResourceV1::ValidationWork),
    )
    .map_err(|_| rejected("reusable LDS work ceiling does not fit usize"))?;
    observe_with_work(tcx, plan, types, functions, callables, contexts, &mut work)
}

fn observe_with_work<'tcx>(
    tcx: TyCtxt<'tcx>,
    plan: &ProductionSemanticPreflightPlanV1<'tcx>,
    types: &[SemanticTypeDeclV1],
    functions: &[SemanticFunctionDeclV1],
    callables: &[SemanticCallableDeclV1],
    contexts: &AuthenticatedProductionKernelContextsV1,
    work: &mut usize,
) -> Result<AuthenticatedReusableLdsSourcesV1, ProductionSemanticImportErrorV1> {
    if functions.len() != plan.function_producers().len()
        || functions.len() != plan.body_producers().len()
        || callables.len() != functions.len() + plan.terminal_producers().len()
        || types.len() != plan.type_producers().len()
    {
        return Err(rejected(
            "reusable LDS requires complete canonical producer rosters",
        ));
    }
    charge(work, functions.len())?;
    if !plan.function_producers().iter().any(|p| {
        trusted_device_items::is_exact_reviewed_provider_definition_v1(
            tcx,
            p.instance.def_id(),
            CONVERSION,
        )
    }) {
        return Ok(AuthenticatedReusableLdsSourcesV1::default());
    }
    charge(work, plan.direct_call_producers().len())?;
    let roster =
        numerical_policy_v1::defined_source_roster_v1(tcx, plan, types, functions, callables)?;
    let mut source_replay = source_body_v1::Replay::new(tcx, plan, work)?;
    let mut records = Vec::with_capacity(MAX_CONVERSIONS);
    let edges = plan
        .direct_call_producers()
        .iter()
        .map(|edge| (edge.caller, edge.callee))
        .collect::<Vec<_>>();
    for (index, producer) in plan.function_producers().iter().enumerate() {
        *work = work
            .checked_sub(1)
            .ok_or_else(|| rejected("reusable LDS source-work ceiling"))?;
        if !trusted_device_items::is_exact_reviewed_provider_definition_v1(
            tcx,
            producer.instance.def_id(),
            CONVERSION,
        ) {
            continue;
        }
        if records.len() == MAX_CONVERSIONS {
            return Err(rejected("reusable LDS conversion storage ceiling"));
        }
        charge(
            work,
            edges
                .len()
                .checked_mul(functions.len().saturating_add(1))
                .ok_or_else(|| rejected("reusable LDS reachability-work overflow"))?,
        )?;
        let function = SemanticFunctionIdV1::from_index(index as u32);
        if roster.defined(function).map_err(|error| {
            mapping_diagnostic::report(plan, functions, function, "converter");
            error
        })? != producer.instance {
            return Err(rejected("reusable LDS original converter roster"));
        }
        let facts = definition(tcx, producer.instance)?;
        let root = authenticate_capability_memory_root_v1(
            contexts,
            &BTreeSet::from([function]),
            &edges,
            false,
        )?;
        let brand = rust_execution_brand_v1(tcx, facts.brand)
            .ok_or_else(|| rejected("reusable LDS exact execution brand"))?;
        if !rust_kernel_brand_matches_root_v1(tcx, brand, root) {
            return Err(rejected(
                "reusable LDS source brand does not belong to this root",
            ));
        }
        let mut incoming = plan
            .direct_call_producers()
            .iter()
            .filter(|call| call.callee == function);
        let incoming_call = incoming
            .next()
            .ok_or_else(|| rejected("reusable LDS source conversion has no caller"))?;
        if incoming.next().is_some() {
            return Err(rejected(
                "reusable LDS first slice requires one original caller occurrence",
            ));
        }
        let caller = incoming_call.caller;
        let caller_instance = plan.function_producers()[caller.index() as usize].instance;
        let caller_mapping = roster.reconstructed_body(caller, &mut source_replay, work)
            .map_err(|error| {
                mapping_diagnostic::report(plan, functions, caller, "caller-replay");
                error
            })?;
        let checked = occurrence::NestedReceiver::check(
            tcx,
            caller_instance,
            producer.instance,
            facts,
            work,
        )
        .map_err(rejected)?;
        checked.replay(tcx, work).map_err(rejected)?;
        if trusted_device_items::classify(tcx, checked.allocation().def_id())
            != Some(TrustedDeviceItem::ExecutionLdsAllocate)
        {
            return Err(rejected(
                "reusable LDS nested expression is not the exact allocation terminal",
            ));
        }
        let coordinates = checked.coordinates();
        if incoming_call.block != coordinates.conversion_block.as_u32() {
            return Err(rejected(
                "reusable LDS direct-call producer differs from original source",
            ));
        }
        let allocation_block = caller_mapping.block(coordinates.allocation_block.as_u32())?;
        let conversion_block = caller_mapping.block(coordinates.conversion_block.as_u32())?;
        let allocation_local = caller_mapping.local(coordinates.allocation_local.as_u32())?;
        let mut terminal = plan
            .terminal_expansion_producers()
            .iter()
            .filter(|terminal| {
                terminal.caller == caller
                    && terminal.block == coordinates.allocation_block.as_u32()
                    && terminal.instance == checked.allocation()
            });
        charge(work, plan.terminal_expansion_producers().len())?;
        let allocation = terminal
            .next()
            .ok_or_else(|| rejected("reusable LDS missing exact allocation producer"))?;
        if terminal.next().is_some() {
            return Err(rejected("reusable LDS duplicate allocation producer"));
        }
        let terminal = plan
            .terminal_producers()
            .get(allocation.terminal as usize)
            .ok_or_else(|| rejected("reusable LDS allocation terminal outside producer roster"))?;
        let allocation_callable = SemanticCallableIdV1::from_index(
            u32::try_from(functions.len())
                .ok()
                .and_then(|n| n.checked_add(allocation.terminal))
                .ok_or_else(|| rejected("reusable LDS callable index overflow"))?,
        );
        let mut ids = [SemanticTypeIdV1::from_index(0); 8];
        charge(
            work,
            types
                .len()
                .checked_mul(8)
                .ok_or_else(|| rejected("reusable LDS type-work overflow"))?,
        )?;
        for (slot, ty) in ids.iter_mut().zip([
            facts.input,
            facts.output,
            facts.element,
            facts.storage,
            facts.state,
            facts.workgroup,
            facts.epoch_marker,
            facts.thread,
        ]) {
            *slot = semantic_type_for_rust_v1(tcx, types, ty)?;
        }
        require_capability_memory_terminal_abi_v1(
            tcx,
            functions[index].abi(),
            types,
            &[facts.input],
            facts.output,
            &[SemanticSourceArgumentOwnershipV1::ByValue],
        )?;
        let mut digest =
            SemanticIdentityDigestV1::new(b"fe2o3/production/reusable-lds/source-receiver/v1");
        digest.field(&root.root_function_identity);
        digest.field(&root.issuance_identity);
        for instance in [caller_instance, producer.instance, checked.allocation()] {
            digest.field(
                canonical_function_identities_v1(tcx, instance)
                    .function()
                    .as_bytes(),
            );
        }
        digest.field(&rustc_mir_body_sha256_v1(tcx, caller_instance));
        digest.field(&rustc_mir_body_sha256_v1(tcx, producer.instance));
        let nodes = checked.nodes();
        digest.field(
            &tcx.def_path_hash(nodes.lexical_body.to_def_id())
                .0
                .to_le_bytes(),
        );
        for node in [nodes.conversion, nodes.allocation, nodes.workgroup] {
            digest.field(
                &tcx.def_path_hash(node.owner.def_id.to_def_id())
                    .0
                    .to_le_bytes(),
            );
            digest.field(&node.local_id.as_u32().to_le_bytes());
        }
        for value in [
            caller.index(),
            allocation_block.index(),
            allocation_local.index(),
            conversion_block.index(),
        ] {
            digest.field(&value.to_le_bytes());
        }
        for ty in [
            facts.input,
            facts.output,
            facts.element,
            facts.brand,
            facts.epoch,
        ] {
            digest.field(rustc_type_identity_v1(tcx, ty).as_bytes());
        }
        let source = SemanticReusableLdsSourceV1 {
            caller,
            caller_identity: functions[caller.index() as usize].identity(),
            caller_abi: functions[caller.index() as usize].abi().identity(),
            allocation_callable,
            allocation_identity: terminal.identities.function(),
            allocation_abi: terminal.abi.identity,
            allocation_block,
            allocation_local,
            conversion_block,
            source_binding: digest.finish(),
        };
        let record = SemanticReusableLdsConversionV1::for_defined_function(
            function,
            functions,
            callables,
            types,
            SemanticReusableLdsTypesV1::new(ids),
            source,
            capability_memory_provenance_v1(root, contexts)?,
            rustc_type_identity_v1(tcx, facts.brand),
            rustc_type_identity_v1(tcx, facts.epoch),
            facts.elements,
        )
        .map_err(|_| rejected("reusable LDS canonical conversion/source record rejected"))?;
        records.push(record);
    }
    Ok(AuthenticatedReusableLdsSourcesV1 {
        records: records.into_boxed_slice(),
    })
}

pub(super) fn attach<'tcx>(
    tcx: TyCtxt<'tcx>,
    plan: &ProductionSemanticPreflightPlanV1<'tcx>,
    types: &[SemanticTypeDeclV1],
    functions: &mut [SemanticFunctionDeclV1],
    callables: &[SemanticCallableDeclV1],
    contexts: &AuthenticatedProductionKernelContextsV1,
) -> Result<(), ProductionSemanticImportErrorV1> {
    let custody = observe(tcx, plan, types, functions, callables, contexts)?;
    // Only the fixed two-local/one-block converter bodies are cloned. Finish
    // every fallible attachment before mutating any of the caller's records.
    let pending = custody
        .records
        .iter()
        .map(|record| {
            let index = record.function().index() as usize;
            functions[index]
                .clone()
                .with_defined_capability_contract(
                    SemanticDefinedCapabilityContractV1::ReusableLdsConversion(*record),
                )
                .map(|function| (index, function))
                .map_err(|_| rejected("reusable LDS canonical attachment rejected"))
        })
        .collect::<Result<Vec<_>, _>>()?;
    for (index, function) in pending {
        functions[index] = function;
    }
    Ok(())
}

pub(super) fn validate_carriage<'tcx>(
    tcx: TyCtxt<'tcx>,
    plan: &ProductionSemanticPreflightPlanV1<'tcx>,
    contexts: &AuthenticatedProductionKernelContextsV1,
    mir: &AdmittedInertSemanticMirV1,
) -> Result<(), ProductionSemanticImportErrorV1> {
    let fresh = observe(
        tcx,
        plan,
        mir.types(),
        mir.functions(),
        mir.callables(),
        contexts,
    )?;
    let declared = mir
        .functions()
        .iter()
        .filter_map(|f| match f.defined_capability_contract() {
            Some(SemanticDefinedCapabilityContractV1::ReusableLdsConversion(record)) => {
                Some(record)
            }
            _ => None,
        });
    if !fresh.records.iter().eq(declared) {
        return Err(rejected(
            "reusable LDS source custody was forged, erased or substituted",
        ));
    }
    Ok(())
}

/// Reuse the original live source observer with a caller-owned remaining
/// budget; inert converter metadata alone is never storage authority.
pub(super) fn validate_table_carriage<'tcx>(
    tcx: TyCtxt<'tcx>,
    plan: &ProductionSemanticPreflightPlanV1<'tcx>,
    contexts: &AuthenticatedProductionKernelContextsV1,
    types: &[SemanticTypeDeclV1],
    functions: &[SemanticFunctionDeclV1],
    callables: &[SemanticCallableDeclV1],
    work: &mut usize,
) -> Result<(), ProductionSemanticImportErrorV1> {
    let fresh = observe_with_work(tcx, plan, types, functions, callables, contexts, work)?;
    charge(work, functions.len())?;
    let declared = functions.iter().filter_map(|function| match function.defined_capability_contract() {
        Some(SemanticDefinedCapabilityContractV1::ReusableLdsConversion(record)) => Some(record),
        _ => None,
    });
    if !fresh.records.iter().eq(declared) {
        return Err(rejected("reusable LDS source custody was forged, erased or substituted"));
    }
    Ok(())
}

#[cfg(test)]
#[path = "reusable_lds_v1/import_tests.rs"]
mod import_tests;

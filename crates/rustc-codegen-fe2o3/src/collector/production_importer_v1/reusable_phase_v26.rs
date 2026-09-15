//! Live defined9 source attachment/replay draft. This owner authenticates
//! original definitions and source calls, not executable phase transitions.
//! The expanded linear consumer must additionally check the complete protocol.
use super::*;
use crate::rustc_semantic_adapter_v1::rustc_mir_body_sha256_v1;
use fe2o3_mir_model::semantic_mir_v1::{
    SemanticDefinedCapabilityContractV1, SemanticDefinedReusablePhaseV1, SemanticFunctionDeclV1,
    attach_reusable_phase_contracts_v26,
};

mod canonical_recipe;
mod completion;
mod definitions;
mod dependencies;
mod execution_source;
mod expanded_protocol;
mod hir_calls;
mod linear_events;
mod source_calls;
mod source_cfg;
mod source_protocol;
mod ssa_protocol;
pub(super) mod production;
#[cfg(test)]
pub(in crate::collector::production_importer_v1) mod source_tests;

type PhaseResult<T> = std::result::Result<T, ProductionSemanticImportErrorV1>;
fn rejected(detail: &'static str) -> ProductionSemanticImportErrorV1 {
    ProductionSemanticImportErrorV1::KernelContextBinding(detail)
}

pub(crate) fn logical_dependencies<'tcx>(
    tcx: TyCtxt<'tcx>,
    instance: Instance<'tcx>,
    work: &mut usize,
) -> PhaseResult<Option<[Option<Ty<'tcx>>; 32]>> {
    dependencies::observe(tcx, instance, work).map(|types| types.map(|t| t.into_values()))
}

/// Non-Clone, private construction. This result may be used to compare inert
/// source attachments; it never substitutes for an expanded phase/loan owner.
struct LiveDefinitions {
    records: Vec<SemanticDefinedReusablePhaseV1>,
    remaining_work: usize,
}

fn observe<'tcx>(
    tcx: TyCtxt<'tcx>,
    plan: &ProductionSemanticPreflightPlanV1<'tcx>,
    types: &[SemanticTypeDeclV1],
    functions: &[SemanticFunctionDeclV1],
    callables: &[SemanticCallableDeclV1],
    contexts: &AuthenticatedProductionKernelContextsV1,
) -> PhaseResult<LiveDefinitions> {
    use definitions::{Definition, Role, Types};
    use source_calls::{reserve, spend};
    let mut work = usize::try_from(
        SemanticMirLimitsV1::default().limit(SemanticMirResourceV1::ValidationWork),
    )
    .map_err(|_| rejected("phase source work conversion"))?;
    if functions.len() != plan.function_producers().len()
        || functions.len() != plan.body_producers().len()
        || types.len() != plan.type_producers().len()
        || functions.len().checked_add(plan.terminal_producers().len()) != Some(callables.len())
    {
        return Err(rejected("phase source complete canonical producer rosters"));
    }
    let mut count = 0usize;
    for producer in plan.function_producers() {
        if definitions::classify(tcx, producer.instance, &mut work)
            .map_err(|_| rejected("phase reviewed definition scan"))?
            .is_some()
        {
            count = count
                .checked_add(1)
                .ok_or_else(|| rejected("phase definition count overflow"))?;
        }
    }
    if count == 0 {
        return Ok(LiveDefinitions {
            records: Vec::new(),
            remaining_work: work,
        });
    }
    let mut definitions = reserve(count, &mut work)?;
    for (index, producer) in plan.function_producers().iter().enumerate() {
        let Some(role) = definitions::classify(tcx, producer.instance, &mut work)
            .map_err(|_| rejected("phase reviewed definition scan"))?
        else {
            continue;
        };
        let definition = Definition::observe(tcx, producer.instance, role, &mut work)
            .map_err(|_| rejected("phase original nominal definition, body or ABI rejected"))?;
        definitions.push((SemanticFunctionIdV1::from_index(index as u32), definition));
    }
    if definitions.len() != count {
        return Err(rejected("phase reviewed definition roster changed"));
    }
    let roster =
        numerical_policy_v1::defined_source_roster_v1(tcx, plan, types, functions, callables)?;
    let mut replay = source_body_v1::Replay::new(tcx, plan, &mut work)?;
    let calls = source_calls::observe(
        tcx,
        plan,
        functions,
        callables,
        &definitions,
        &roster,
        &mut replay,
        &mut work,
    )?;
    let mut receipts = reserve(calls.len(), &mut work)?;
    for call in &calls {
        spend(&mut work, definitions.len())?;
        let definition = definitions
            .iter()
            .find(|(id, _)| *id == call.callee)
            .ok_or_else(|| rejected("phase original callee definition absent"))?;
        receipts.push(hir_calls::observe(
            tcx,
            call,
            &definition.1,
            &definitions,
            &mut work,
        )?);
    }
    let mut edges = reserve(plan.direct_call_producers().len(), &mut work)?;
    for edge in plan.direct_call_producers() {
        spend(&mut work, 1)?;
        edges.push((edge.caller, edge.callee));
    }
    let mapper = canonical_recipe::Mapper {
        tcx,
        plan,
        functions,
        callables,
        roster: &roster,
    };
    let mut records = reserve(count, &mut work)?;
    // WithPhase depends only on the typed Issue and Finish records. The model
    // validates the dependencies without changing or cloning the function table.
    for wrappers in [false, true] {
        for (function, definition) in &definitions {
            spend(&mut work, 1)?;
            if (definition.role == Role::WithPhase) != wrappers {
                continue;
            }
            let root_work = edges
                .len()
                .checked_mul(functions.len().saturating_add(1))
                .and_then(|n| n.checked_add(8))
                .ok_or_else(|| rejected("phase source root reachability work overflow"))?;
            spend(&mut work, root_work)?;
            let root = authenticate_capability_memory_root_v1(
                contexts,
                &BTreeSet::from([*function]),
                &edges,
                false,
            )?;
            let brand = match definition.types {
                Types::OwnerConvert { root, .. } => root,
                Types::Issue { phase, .. }
                | Types::WithPhase { phase, .. }
                | Types::Bind { phase, .. }
                | Types::Finish { phase, .. } => phase.root,
            };
            let brand = rust_execution_brand_v1(tcx, brand)
                .ok_or_else(|| rejected("phase source exact root brand"))?;
            if !rust_kernel_brand_matches_root_v1(tcx, brand, root) {
                return Err(rejected(
                    "phase definition belongs to a different actual root",
                ));
            }
            let recipe = mapper.recipe(definition, &mut replay, &mut work)?;
            let mut digest =
                SemanticIdentityDigestV1::new(b"fe2o3/production/reusable-phase/defined/v26");
            digest.field(&root.root_function_identity);
            digest.field(&root.issuance_identity);
            digest.field(
                canonical_function_identities_v1(tcx, definition.instance)
                    .function()
                    .as_bytes(),
            );
            digest.field(&rustc_mir_body_sha256_v1(tcx, definition.instance));
            for ty in dependencies::collect(tcx, definition, &mut work)?.iter() {
                spend(&mut work, 1)?;
                digest.field(rustc_type_identity_v1(tcx, ty).as_bytes());
            }
            let mut incoming = 0u32;
            for (call, receipt) in calls.iter().zip(&receipts) {
                spend(&mut work, 1)?;
                if call.callee != *function {
                    continue;
                }
                incoming = incoming
                    .checked_add(1)
                    .ok_or_else(|| rejected("phase source incoming overflow"))?;
                digest.field(
                    canonical_function_identities_v1(tcx, call.caller_instance)
                        .function()
                        .as_bytes(),
                );
                digest.field(&rustc_mir_body_sha256_v1(tcx, call.caller_instance));
                for value in [
                    call.caller.index(),
                    call.raw_block.as_u32(),
                    call.block.index(),
                    call.normal.index(),
                    call.callee.index(),
                ] {
                    digest.field(&value.to_le_bytes());
                }
                match receipt.syntax {
                    hir_calls::Syntax::Method {
                        expression,
                        receiver,
                        argument,
                    } => {
                        digest.field(&[0]);
                        for node in [Some(expression), Some(receiver), argument] {
                            match node {
                                Some(node) => {
                                    digest.field(&[1]);
                                    digest.field(
                                        &tcx.def_path_hash(node.owner.def_id.to_def_id())
                                            .0
                                            .to_le_bytes(),
                                    );
                                    digest.field(&node.local_id.as_u32().to_le_bytes());
                                }
                                None => digest.field(&[0]),
                            }
                        }
                    }
                    hir_calls::Syntax::ReviewedIssue { wrapper } => {
                        digest.field(&[1]);
                        digest.field(&wrapper.index().to_le_bytes());
                    }
                }
                if let Some(method) = &receipt.method {
                    for binding in [Some(method.receiver_binding), method.storage_binding] {
                        source_protocol::commit_hir_binding(tcx, binding, &mut digest, &mut work)?;
                    }
                }
            }
            if incoming == 0 {
                return Err(rejected("phase source definition has no original caller"));
            }
            digest.field(&incoming.to_le_bytes());
            let mut remaining = work as u64;
            let record = SemanticDefinedReusablePhaseV1::for_defined_function_with_dependencies(
                *function,
                functions,
                callables,
                types,
                capability_memory_provenance_v1(root, contexts)?,
                digest.finish(),
                recipe,
                &records,
                &mut remaining,
            );
            work = usize::try_from(remaining)
                .map_err(|_| rejected("phase canonical work conversion"))?;
            let record = record
                .map_err(|_| rejected("phase canonical definition/source record rejected"))?;
            if record.incoming().count() != incoming {
                return Err(rejected(
                    "phase original and canonical incoming counts disagree",
                ));
            }
            // Check each replayed attachment before a wrapper consumes it as a dependency.
            if functions[function.index() as usize]
                .defined_capability_contract()
                .is_some_and(|attached| {
                    attached != &SemanticDefinedCapabilityContractV1::ReusablePhase(record)
                })
            {
                return Err(rejected(
                    "phase source carriage was forged, substituted or omitted",
                ));
            }
            records.push(record);
        }
    }
    for index in 1..records.len() {
        let mut slot = index;
        while slot != 0 {
            spend(&mut work, 1)?;
            if records[slot - 1].function() < records[slot].function() {
                break;
            }
            if records[slot - 1].function() == records[slot].function() {
                return Err(rejected("phase duplicate canonical definition"));
            }
            records.swap(slot - 1, slot);
            slot -= 1;
        }
    }
    Ok(LiveDefinitions {
        records,
        remaining_work: work,
    })
}

pub(super) fn attach<'tcx>(
    tcx: TyCtxt<'tcx>,
    plan: &ProductionSemanticPreflightPlanV1<'tcx>,
    types: &[SemanticTypeDeclV1],
    functions: &mut [SemanticFunctionDeclV1],
    callables: &[SemanticCallableDeclV1],
    contexts: &AuthenticatedProductionKernelContextsV1,
) -> PhaseResult<()> {
    let fresh = observe(tcx, plan, types, functions, callables, contexts)?;
    let mut remaining = fresh.remaining_work as u64;
    attach_reusable_phase_contracts_v26(functions, callables, types, &fresh.records, &mut remaining)
        .map_err(|_| rejected("phase atomic canonical attachment rejected"))
}

pub(super) fn validate_carriage<'tcx>(
    tcx: TyCtxt<'tcx>,
    plan: &ProductionSemanticPreflightPlanV1<'tcx>,
    contexts: &AuthenticatedProductionKernelContextsV1,
    mir: &AdmittedInertSemanticMirV1,
) -> PhaseResult<()> {
    let mut fresh = observe(
        tcx,
        plan,
        mir.types(),
        mir.functions(),
        mir.callables(),
        contexts,
    )?;
    let mut declared = 0usize;
    for function in mir.functions() {
        source_calls::spend(&mut fresh.remaining_work, 1)?;
        if let Some(SemanticDefinedCapabilityContractV1::ReusablePhase(record)) =
            function.defined_capability_contract()
        {
            if fresh.records.get(declared) != Some(record) {
                return Err(rejected(
                    "phase source carriage was forged, substituted or omitted",
                ));
            }
            declared += 1;
        }
    }
    if declared != fresh.records.len() {
        return Err(rejected(
            "phase source carriage erased an original definition",
        ));
    }
    Ok(())
}

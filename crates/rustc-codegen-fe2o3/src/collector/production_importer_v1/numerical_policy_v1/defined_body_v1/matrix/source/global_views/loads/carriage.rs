use super::*;
use crate::production_semantic_terminal_v1::ProductionTerminalExpansionV1 as Expansion;

pub(in crate::collector::production_importer_v1) fn validate<'tcx>(
    tcx: TyCtxt<'tcx>,
    plan: &ProductionSemanticPreflightPlanV1<'tcx>,
    types: &[SemanticTypeDeclV1],
    functions: &[SemanticFunctionDeclV1],
    callables: &[SemanticCallableDeclV1],
    contexts: &AuthenticatedProductionKernelContextsV1,
) -> Result<(), ProductionSemanticImportErrorV1> {
    let candidates = plan
        .function_producers()
        .iter()
        .enumerate()
        .filter_map(|(index, producer)| {
            kind(tcx, producer.instance.def_id())
                .map(|_| SemanticFunctionIdV1::from_index(index as u32))
        })
        .collect::<BTreeSet<_>>();
    if candidates.is_empty() {
        return Ok(());
    }
    let roster = roster::Roster::new(tcx, plan, types, functions, callables)?;
    let edges = plan
        .direct_call_producers()
        .iter()
        .map(|call| (call.caller, call.callee))
        .collect::<Vec<_>>();
    for function in &candidates {
        let instance = roster.defined(*function)?;
        let facts = validate_source(tcx, instance)?
            .ok_or_else(|| rejected("Global FP4/8 load missing source marker"))?;
        let root = authenticate_capability_memory_root_v1(
            contexts,
            &BTreeSet::from([*function]),
            &edges,
            false,
        )?;
        roster.root(root, contexts)?;
        if !rust_kernel_brand_matches_root_v1(tcx, facts.root, root) {
            return Err(rejected(
                "Global FP4/8 load differs from authenticated kernel root",
            ));
        }
        roster.types(facts.inputs)?;
        roster.types([
            facts.view,
            facts.lane,
            facts.global_reference,
            facts.global,
            facts.output,
            facts.registers,
        ])?;
        // Exact nominal identities are retained inside the view, lane and
        // fragment types. No scalar field or side table replaces their custody.
        let helpers = facts
            .helpers
            .map(|helper| roster.defined_instance(helper))
            .into_iter()
            .collect::<Result<BTreeSet<_>, _>>()?;
        let observed = plan
            .direct_call_producers()
            .iter()
            .filter(|call| call.caller == *function)
            .map(|call| call.callee)
            .collect::<Vec<_>>();
        if observed.len() != 3
            || observed.iter().copied().collect::<BTreeSet<_>>() != helpers
            || plan
                .terminal_expansion_producers()
                .iter()
                .any(|call| call.caller == *function)
            || plan
                .normalized_intrinsic_producers()
                .iter()
                .any(|call| call.caller == *function)
        {
            return Err(rejected(
                "Global FP4/8 load original three defined helper edges",
            ));
        }
        require_capability_memory_terminal_abi_v1(
            tcx,
            functions[function.index() as usize].abi(),
            types,
            &facts.inputs,
            facts.output,
            &[
                SemanticSourceArgumentOwnershipV1::SharedBorrow,
                SemanticSourceArgumentOwnershipV1::SharedBorrow,
                SemanticSourceArgumentOwnershipV1::ByValue,
                SemanticSourceArgumentOwnershipV1::ByValue,
            ],
        )?;
        let selected = replay::descendants(plan, functions.len(), BTreeSet::from([*function]))?;
        let reads = plan
            .terminal_expansion_producers()
            .iter()
            .filter(|call| {
                selected.contains(&call.caller) && call.expansion == Expansion::CapabilityGlobalLoad
            })
            .collect::<Vec<_>>();
        if reads.len() != 1 {
            return Err(rejected(
                "Global FP4/8 load retains exactly one checked byte-read source edge",
            ));
        }
        let read = reads[0];
        let (index, producer) = plan
            .terminal_producers()
            .iter()
            .enumerate()
            .find(|(_, producer)| {
                producer.instance == read.instance && producer.expansion == read.expansion
            })
            .ok_or_else(|| rejected("Global FP4/8 load checked byte-read producer"))?;
        let signature = super::super::super::signature(tcx, producer.instance)?;
        if signature.inputs() != [facts.global_reference, tcx.types.usize]
            || rust_option_payload_v1(tcx, signature.output()) != Some(tcx.types.u8)
        {
            return Err(rejected(
                "Global FP4/8 load retains Option<u8> from exact Global reference",
            ));
        }
        let Some(SemanticCallableDeclV1::CompilerIntrinsic {
            binding, operation, ..
        }) = callables.get(functions.len() + index)
        else {
            return Err(rejected("Global FP4/8 checked read canonical callable"));
        };
        if *operation
            != terminal_operation_v1(
                tcx,
                producer.instance,
                producer.expansion,
                binding.abi(),
                types,
                Some(root),
                producer.identities.function(),
                contexts,
            )?
        {
            return Err(rejected(
                "Global FP4/8 checked read memory contract substitution",
            ));
        }
    }
    // Replays every original transitive body once under the existing shared
    // body-owner budget: packing arithmetic, checked indices, Options and reads.
    replay::validate(tcx, plan, types, functions, candidates)
}

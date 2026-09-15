//! Replay real terminal construction before whole-module schema admission.

use super::*;

pub(super) fn check<'tcx>(
    tcx: TyCtxt<'tcx>,
    plan: &ProductionSemanticPreflightPlanV1<'tcx>,
    contexts: &AuthenticatedProductionKernelContextsV1,
) {
    assert!(plan.terminal_producers().len() <= 256);
    let types = construct_production_semantic_types_v1(tcx, plan.type_producers())
        .expect("actual terminal type producers")
        .into_records();
    let producers = plan
        .terminal_producers()
        .iter()
        .map(|terminal| terminal.abi.clone())
        .collect::<Vec<_>>();
    let abis = construct_production_semantic_fn_abis_v1(tcx, &producers, plan.type_producers())
        .expect("actual terminal ABI producers")
        .into_records();
    assert_eq!(abis.len(), plan.terminal_producers().len());
    for (index, (terminal, abi)) in plan.terminal_producers().iter().zip(&abis).enumerate() {
        let root = capability_memory_root_for_terminal_v1(
            tcx,
            plan,
            contexts,
            index as u32,
            terminal.expansion,
        )
        .unwrap_or_else(|error| {
            panic!("terminal {index} {:?} root: {error:?}", terminal.expansion)
        });
        let operation = terminal_operation_v1(
            tcx,
            terminal.instance,
            terminal.expansion,
            abi,
            &types,
            root,
            terminal.identities.function(),
            contexts,
        )
        .unwrap_or_else(|error| {
            panic!(
                "terminal {index} {:?} {:?} ABI {abi:?}: {error:?}",
                terminal.expansion, terminal.instance,
            )
        });
        if let SemanticCompilerIntrinsicOperationV1::ExecutionCapability { contract } = operation {
            assert!(contract.signature().arguments().eq(abi.source_input_types().iter().copied()));
            assert_eq!(contract.signature().output(), abi.source_output_type());
        }
    }
}

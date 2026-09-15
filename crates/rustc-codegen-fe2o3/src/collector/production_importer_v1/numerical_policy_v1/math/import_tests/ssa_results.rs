//! Actual AMD source -> canonical MIR -> checked SSA, not ranked/KIR proof.

use super::*;
use fe2o3_mir_model::{SemanticExpandedStatementOriginV1, SsaBlockIdV1, SsaResolvedEventV1};
use fe2o3_pliron::{
    ProductionSemanticMirLimitsV1, ProductionSemanticMirOwnerV1, ProductionSemanticSsaErrorV1,
    ProductionSemanticSsaLimitsV1, ProductionSemanticSsaOwnerV1,
};

fn source_owner(mir: &AdmittedInertSemanticMirV1) -> ProductionSemanticMirOwnerV1 {
    let decoded = AdmittedInertSemanticMirV1::decode_exact_v21_canonical(
        mir.canonical_encoding(),
        SemanticMirLimitsV1::default(),
    )
    .unwrap();
    ProductionSemanticMirOwnerV1::try_new(decoded, ProductionSemanticMirLimitsV1::default())
        .unwrap()
}

pub(super) fn check(mir: &AdmittedInertSemanticMirV1) {
    let owner = ProductionSemanticSsaOwnerV1::try_new(
        source_owner(mir),
        ProductionSemanticSsaLimitsV1::default(),
    )
    .expect("actual retained getter/bridge body must produce its checked SSA result");
    owner.verify_replay().unwrap();
    assert_eq!(
        owner.source_semantic().canonical_encoding(),
        mir.canonical_encoding()
    );
    let mut results = 0;
    let mut consumers = 0;
    for &root in mir.roots() {
        let view = owner.execution_view_for_root(root).unwrap();
        let plan = owner.execution_plan_for_root(root).unwrap();
        for result in plan.defined_math_results() {
            let instance = &view.instances()[result.getter().index() as usize];
            let Some(SemanticDefinedCapabilityContractV1::KernelMathDerive(record)) =
                mir.functions()[instance.function().index() as usize].defined_capability_contract()
            else {
                panic!("checked result must retain original getter metadata")
            };
            assert_eq!(record.provenance().root(), root);
            assert_eq!(
                view.instances()[result.bridge().index() as usize].function(),
                record.bridge().function()
            );
            assert_eq!(result.math(), record.types().math);
            assert_eq!(
                view.block_origins()[result.block().index() as usize].statements()
                    [result.statement() as usize],
                SemanticExpandedStatementOriginV1::ReturnTransfer {
                    callee: result.bridge()
                }
            );
            assert!(
                !plan
                    .implicit_entry_variables()
                    .iter()
                    .any(|local| local.get() == result.return_local().index())
            );
            assert!(
                !plan
                    .frame_initializations()
                    .iter()
                    .any(|row| row.local() == result.return_local())
            );
            let events = plan
                .plan()
                .resolved_events(SsaBlockIdV1::new(result.block().index()))
                .unwrap();
            for local in [result.receiver(), result.current()] {
                assert!(events.iter().any(|(_, event)| matches!(event,
                    SsaResolvedEventV1::Use { variable, .. } if variable.get() == local.index())));
            }
            assert!(events.iter().any(|(_, event)| matches!(event,
                SsaResolvedEventV1::Define { variable, .. } if variable.get() == result.return_local().index())));
            results += 1;
        }
        consumers += view.body().blocks().iter().filter(|block| matches!(block.terminator().kind(),
            SemanticTerminatorKindV1::Call(call) if matches!(mir.callables()[call.callee().index() as usize],
                SemanticCallableDeclV1::CompilerIntrinsic { operation: SemanticCompilerIntrinsicOperationV1::PolicyMathF32 { .. }, .. }))).count();
    }
    assert_eq!(results, 1);
    assert_eq!(consumers, 13);

    let mut functions = mir.functions().to_vec();
    let mut removed = 0;
    for function in &mut functions {
        if !matches!(
            function.defined_capability_contract(),
            Some(SemanticDefinedCapabilityContractV1::KernelMathDerive(_))
        ) {
            continue;
        }
        *function = SemanticFunctionDeclV1::new(
            function.identity(),
            function.role(),
            function.item_definition_identity(),
            function.monomorphization_identity(),
            function.generic_type_arguments_identity(),
            function.const_generic_arguments_identity(),
            function.source(),
            function.abi().clone(),
            function.locals().to_vec(),
            function.entry(),
            function.blocks().to_vec(),
        )
        .unwrap();
        removed += 1;
    }
    assert_eq!(removed, 1);
    let changed = admit_changed_with_functions(mir, functions, |_, _| {}).unwrap();
    let error = match ProductionSemanticSsaOwnerV1::try_new(
        source_owner(&changed),
        ProductionSemanticSsaLimitsV1::default(),
    ) {
        Err(error) => error,
        Ok(_) => {
            panic!("an ignored return without the exact getter record cannot become branded Math")
        }
    };
    assert!(
        matches!(error, ProductionSemanticSsaErrorV1::ExpandedExecution {
            source_statement: Some(SemanticExpandedStatementOriginV1::ReturnTransfer { .. }), error, ..
        } if matches!(*error, ProductionSemanticSsaErrorV1::Planner {
            error: fe2o3_mir_model::SsaPlannerErrorV1::UndefinedAtUse { .. }, ..
        }))
    );
}

#[test]
#[ignore = "requires cached AMD metadata; invokes rustc only, never Cargo"]
fn policy_math_all13_source_ssa_gfx942() {
    run_with_ssa(
        "gfx942",
        "ssa_results::policy_math_all13_source_ssa_gfx942",
        true,
    );
}

#[test]
#[ignore = "requires cached AMD metadata; invokes rustc only, never Cargo"]
fn policy_math_all13_source_ssa_gfx950() {
    run_with_ssa(
        "gfx950",
        "ssa_results::policy_math_all13_source_ssa_gfx950",
        true,
    );
}

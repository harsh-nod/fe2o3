//! Synthetic source/SSA components; not live provider or launch authentication.
use super::super::capability_ssa_graph_01::{
    CapabilityDefinitionSiteV1 as Site, CapabilitySsaGraphV1 as Graph,
};
use super::super::numerical_policy_math_custody_01::checked_context_source_v1;
use super::*;
use fe2o3_mir_model::semantic_mir_v1::*;

fn owner(nested: bool) -> ProductionSemanticSsaOwnerV1 {
    ProductionSemanticSsaOwnerV1::try_new(
        ProductionSemanticMirOwnerV1::try_new(
            fixture::context_reborrow_source(nested, false, false),
            ProductionSemanticMirLimitsV1::default(),
        )
        .unwrap(),
        ProductionSemanticSsaLimitsV1::default(),
    )
    .unwrap()
}

fn check(nested: bool, changed: u8) -> Result<usize, ProductionSemanticKirErrorV1> {
    let owner = owner(nested);
    owner.verify_replay().unwrap();
    let input = inputs().remove(0);
    let context = checked_root_context_source_v1(&owner, &input, |_| Ok(None))?;
    let view = owner
        .execution_view_for_root(input.selected_root())
        .unwrap();
    let plan = owner
        .execution_plan_for_root(input.selected_root())
        .unwrap();
    let mut graph = Graph::new(
        view.body(),
        plan.plan(),
        if changed == 3 { 0 } else { 524_288 },
    )?;
    let mut count = 0;
    for (block, body) in view.body().blocks().iter().enumerate() {
        let SemanticTerminatorKindV1::Call(call) = body.terminator().kind() else {
            continue;
        };
        let Some(SemanticCallableDeclV1::CompilerIntrinsic {
            operation: SemanticCompilerIntrinsicOperationV1::ExecutionCapability { contract },
            ..
        }) = owner
            .source_semantic()
            .callables()
            .get(call.callee().index() as usize)
        else {
            continue;
        };
        let SemanticExecutionCapabilityOperationV1::WorkgroupDerive { workgroup, .. } =
            contract.operation()
        else {
            continue;
        };
        let [SemanticOperandV1::Copy(place) | SemanticOperandV1::Move(place)] = call.arguments()
        else {
            panic!("component Workgroup needs its real Context reference use")
        };
        assert!(place.projections().is_empty());
        let value = graph.use_value(block as u32, place.local().index())?;
        let p = contract.provenance();
        let provenance = if changed == 1 {
            SemanticKernelCapabilityProvenanceV1::new(
                p.root(),
                p.kernel_binding(),
                p.frontend_unit(),
                SemanticTypeIdentityV1::from_sha256([199; 32]),
                p.target_brand(),
                p.launch_brand(),
                p.issuance(),
            )
            .unwrap()
        } else {
            p
        };
        let source = checked_context_source_v1(
            &owner,
            &context,
            &mut graph,
            provenance,
            value,
            Some(if changed == 2 { workgroup } else { place.ty() }),
        )?;
        assert_eq!(source.issuer, source.context);
        assert!(!source.loans.is_empty());
        for loan in source.loans {
            graph.loan_live(
                loan,
                Site {
                    block: block as u32,
                    statement: Some(body.statements().len() as u32),
                    local: 0,
                },
            )?;
        }
        count += 1;
    }
    Ok(count)
}

#[test]
fn context_only_query_uses_actual_workgroup_provenance_and_reborrows() {
    for nested in [false, true] {
        assert_eq!(check(nested, 0).unwrap(), 1);
    }
}

#[test]
fn context_only_query_rejects_changed_root_provenance() {
    let error = check(true, 1).unwrap_err();
    assert!(
        matches!(error, ProductionSemanticKirErrorV1::Unsupported { detail, .. }
        if detail == "Policy source query changed its owner, root or role"),
        "{error:?}"
    );
}

#[test]
fn context_only_query_rejects_an_owned_type_in_place_of_reference() {
    let error = check(true, 2).unwrap_err();
    assert!(
        matches!(error, ProductionSemanticKirErrorV1::Unsupported { detail, .. }
        if detail == "policy Math reference has no exact permitted pointee edge"),
        "{error:?}"
    );
}

#[test]
fn context_only_query_keeps_the_shared_work_limit() {
    let error = check(true, 3).unwrap_err();
    assert!(
        matches!(
            error,
            ProductionSemanticKirErrorV1::ResourceLimit {
                resource: ProductionSemanticKirResourceV1::AnalysisWork,
                actual: 1,
                limit: 0,
            }
        ),
        "{error:?}"
    );
}

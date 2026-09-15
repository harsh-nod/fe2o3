//! Full private authentication-path tests with an independently admitted
//! component owner. Actual frontend custody remains the separate AMD callback.
use super::*;
use crate::production_semantic_kir_v1::borrowed_workgroup_01::{
    WorkgroupSourceResolverV1, checked_execution_source_call_v1,
};
use crate::production_semantic_kir_v1::capability_ssa_graph_01::CapabilitySsaGraphV1 as Graph;
use fe2o3_mir_model::semantic_mir_v1::SemanticMirLimitsV1;

fn check(kill: bool, backedge: bool, mutation: u8) -> Result<(), ProductionSemanticKirErrorV1> {
    let source = fixture::transpose_endpoint_source(kill, backedge, mutation == 7);
    let owner = ProductionSemanticSsaOwnerV1::try_new(
        ProductionSemanticMirOwnerV1::try_new(source, ProductionSemanticMirLimitsV1::default())
            .unwrap(),
        ProductionSemanticSsaLimitsV1::default(),
    )
    .unwrap();
    let view = owner
        .execution_view_for_root(SemanticFunctionIdV1::from_index(0))
        .unwrap();
    let plan = owner.execution_plan_for_root(view.root()).unwrap();
    let input = inputs().remove(0);
    let mut context = checked_root_context_source_v1(&owner, &input, |_| Ok(None))?;
    let mut graph = Graph::new(view.body(), plan.plan(), 1_048_576)?;
    let publish = view
        .block_origins()
        .iter()
        .position(|origin| {
            origin.function().index() == 0
                && origin.block().index() == (if mutation == 7 { 9 } else { 8 })
                && origin.terminator()
                    == fe2o3_mir_model::SemanticExpandedTerminatorOriginV1::Source
        })
        .unwrap() as u32;
    let borrowed = view
        .block_origins()
        .iter()
        .position(|origin| {
            origin.function().index() == 0
                && origin.block().index() == 7
                && origin.terminator()
                    == fe2o3_mir_model::SemanticExpandedTerminatorOriginV1::Source
        })
        .unwrap() as u32;
    let (call, contract) = checked_execution_source_call_v1(&owner, view, &context, borrowed)?;
    let mut origin = {
        let mut resolver = WorkgroupSourceResolverV1 {
            owner: &owner,
            view,
            context: &context,
            graph: &mut graph,
            epochs: &[],
        };
        resolver.resolve_operand(borrowed, &call.arguments()[0], &[], contract, 0)?
    };
    assert!(!origin.loans.is_empty());
    if mutation == 6 {
        let decoded = AdmittedInertSemanticMirV1::decode_current_production_canonical(
            owner.source_semantic().canonical_encoding(),
            SemanticMirLimitsV1::default(),
        )
        .unwrap();
        let foreign = ProductionSemanticSsaOwnerV1::try_new(
            ProductionSemanticMirOwnerV1::try_new(
                decoded,
                ProductionSemanticMirLimitsV1::default(),
            )
            .unwrap(),
            ProductionSemanticSsaLimitsV1::default(),
        )
        .unwrap();
        let mut resolver = WorkgroupSourceResolverV1 {
            owner: &foreign,
            view,
            context: &context,
            graph: &mut graph,
            epochs: &[],
        };
        return resolver.before_transpose_publish(&origin, publish, borrowed);
    }
    match mutation {
        0 | 7 => {}
        1 => context.selected_root = SemanticFunctionIdV1::from_index(u32::MAX),
        2 => {
            let old = origin.contract;
            origin.contract = SemanticExecutionCapabilityContractV1::new(
                old.operation(),
                old.signature(),
                old.provenance(),
                old.workgroup_brand().unwrap(),
                owner.source_semantic().types()[27].identity(),
                old.epoch_after(),
                old.source_identity(),
            )
            .unwrap();
        }
        3 => origin.loans.clear(),
        4 => origin.loans[0].owner_local = u32::MAX,
        5 => {} // Feed an authenticated source Publish occurrence where the earlier subgroup is required.
        _ => panic!("unknown test mutation"),
    }
    let mut resolver = WorkgroupSourceResolverV1 {
        owner: &owner,
        view,
        context: &context,
        graph: &mut graph,
        epochs: &[],
    };
    resolver.before_transpose_publish(
        &origin,
        publish,
        if mutation == 5 { publish } else { borrowed },
    )
}

fn rejected(mutation: u8, detail: &'static str) {
    let error = check(false, false, mutation).unwrap_err();
    assert!(
        matches!(error, ProductionSemanticKirErrorV1::Unsupported { detail: actual, .. } if actual == detail),
        "{error:?}"
    );
}

#[test]
fn full_transpose_publish_endpoint_accepts_the_exact_retained_shared_owner() {
    check(false, false, 0).unwrap();
}

#[test]
fn full_transpose_publish_endpoint_rejects_wrong_root_epoch_loan_roster_and_source_call() {
    assert!(matches!(
        check(false, false, 1),
        Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch)
    ));
    assert!(matches!(
        check(false, false, 6),
        Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch)
    ));
    rejected(2, "transpose Publish changed its Workgroup root or epoch");
    rejected(
        3,
        "transpose Publish lacks its retained Workgroup owner loans",
    );
    rejected(
        4,
        "transpose Publish omitted an original shared Workgroup loan",
    );
    rejected(
        5,
        "transpose Publish lacks its exact earlier borrowed subgroup call",
    );
    rejected(
        7,
        "transpose Workgroup crosses a prior matching epoch transition",
    );
}

#[test]
fn full_transpose_publish_endpoint_does_not_skip_preceding_overwrite_or_nonempty_cycle() {
    let error = check(true, false, 0).unwrap_err();
    assert!(
        matches!(
            error,
            ProductionSemanticKirErrorV1::Unsupported {
                detail: "capability loan crosses a move, overwrite, deinitialization or storage death",
                ..
            }
        ),
        "{error:?}"
    );
    let error = check(false, true, 0).unwrap_err();
    assert!(
        matches!(
            error,
            ProductionSemanticKirErrorV1::Unsupported {
                detail: "transpose Publish crosses a cycle without a proven epoch generation",
                ..
            }
        ),
        "{error:?}"
    );
}

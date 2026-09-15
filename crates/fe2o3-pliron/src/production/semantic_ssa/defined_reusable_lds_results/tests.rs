use super::*;
use fe2o3_mir_model::semantic_mir_v1::*;
use fe2o3_mir_model::{SemanticCallExpansionLimitsV1, SsaPlannerErrorV1};

#[path = "source.rs"]
mod source;
use source::source;
include!("frame_tests.rs");

#[test]
fn reusable_lds_whole_document_uses_v24_and_roundtrips_exact_bytes() {
    let original = source(true);
    assert_eq!(original.wire_version(), SemanticMirWireVersionV1::V24);
    let decoded = AdmittedInertSemanticMirV1::decode_current_production_canonical(
        original.canonical_encoding(),
        SemanticMirLimitsV1::default(),
    )
    .unwrap();
    assert_eq!(decoded.canonical_encoding(), original.canonical_encoding());
    assert_eq!(
        decoded.functions()[1].defined_capability_contract(),
        original.functions()[1].defined_capability_contract()
    );
}

#[test]
fn reusable_lds_checked_expansion_retains_original_body_and_allocation() {
    let source = source(true);
    let expansion =
        SemanticCallExpansionV1::try_new(&source, SemanticCallExpansionLimitsV1::default())
            .unwrap();
    let view = expansion.root(SemanticFunctionIdV1::from_index(0)).unwrap();
    let relation = DefinedReusableLdsResultsV1::derive(
        &source,
        &expansion,
        view,
        ProductionSemanticSsaLimitsV1::default(),
    )
    .unwrap();
    let [row] = relation.entries() else {
        panic!("one conversion")
    };
    assert!(row.erased());
    assert!(source.functions()[1].blocks()[0].statements().is_empty());
    assert_eq!(
        source.functions()[1].blocks()[0].terminator().kind(),
        &SemanticTerminatorKindV1::Return
    );
    assert_ne!(row.allocation(), row.parameter());
    assert_ne!(row.parameter(), row.return_local());
    relation.verify_view(view).unwrap();
}

#[test]
fn reusable_lds_events_consume_input_and_do_not_define_an_ambient_return() {
    let source = source(true);
    let expansion =
        SemanticCallExpansionV1::try_new(&source, SemanticCallExpansionLimitsV1::default())
            .unwrap();
    let view = expansion.root(SemanticFunctionIdV1::from_index(0)).unwrap();
    let relation = DefinedReusableLdsResultsV1::derive(
        &source,
        &expansion,
        view,
        ProductionSemanticSsaLimitsV1::default(),
    )
    .unwrap();
    let row = relation.entries()[0];
    let mut input = Vec::new();
    relation.append_result_events(
        row.parameter_block().index(),
        row.parameter_statement(),
        &mut input,
    );
    assert_eq!(
        input,
        vec![
            SsaEventV1::Use(SsaVariableIdV1::new(row.allocation().index())),
            SsaEventV1::Kill(SsaVariableIdV1::new(row.allocation().index()))
        ]
    );
    let mut output = Vec::new();
    relation.append_result_events(
        row.return_block().index(),
        row.return_statement(),
        &mut output,
    );
    assert_eq!(
        output,
        vec![
            SsaEventV1::Use(SsaVariableIdV1::new(row.parameter().index())),
            SsaEventV1::Kill(SsaVariableIdV1::new(row.parameter().index())),
            SsaEventV1::Define(SsaVariableIdV1::new(row.return_local().index()))
        ]
    );
}

#[test]
fn reusable_lds_whole_owner_replays_and_missing_contract_still_rejects_undefined_return() {
    let owner = ProductionSemanticSsaOwnerV1::try_new(
        crate::ProductionSemanticMirOwnerV1::try_new(
            source(true),
            crate::ProductionSemanticMirLimitsV1::default(),
        )
        .unwrap(),
        ProductionSemanticSsaLimitsV1::default(),
    )
    .unwrap();
    owner.verify_replay().unwrap();
    assert_eq!(
        owner
            .execution_plan_for_root(SemanticFunctionIdV1::from_index(0))
            .unwrap()
            .defined_reusable_lds_results()
            .len(),
        1
    );
    let missing = ProductionSemanticSsaOwnerV1::try_new(
        crate::ProductionSemanticMirOwnerV1::try_new(
            source(false),
            crate::ProductionSemanticMirLimitsV1::default(),
        )
        .unwrap(),
        ProductionSemanticSsaLimitsV1::default(),
    );
    assert!(
        matches!(&missing, Err(ProductionSemanticSsaErrorV1::ExpandedExecution { error, .. })
        if matches!(error.as_ref(), ProductionSemanticSsaErrorV1::Planner {
            error: SsaPlannerErrorV1::UndefinedAtUse { variable, .. }, ..
        } if variable.get() == 4)),
        "{missing:?}"
    );
}

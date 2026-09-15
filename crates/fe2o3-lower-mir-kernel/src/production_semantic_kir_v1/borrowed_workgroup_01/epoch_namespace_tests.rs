use fe2o3_mir_model::semantic_mir_v1::*;
use fe2o3_mir_model::{
    SsaBlockInputV1, SsaConstructionInputV1, SsaConstructionPlanV1, SsaEventV1, SsaPlannerErrorV1,
    SsaVariableIdV1, plan_ssa_v1,
};

// Namespace/planner tests only: these coordinate pairs are not replay receipts.
// Actual phase67 emission and Workgroup full-import tests retain source custody.
fn locals() -> Vec<SemanticLocalDeclV1> {
    (0u8..4)
        .map(|index| {
            SemanticLocalDeclV1::new(
                SemanticLocalIdentityV1::from_sha256([index + 40; 32]),
                SemanticTypeIdV1::from_index(u32::from(index)),
                SemanticLocalRoleV1::Temporary,
                SemanticSourceProvenanceV1::unavailable(),
            )
        })
        .collect()
}

fn v(index: u32) -> SsaVariableIdV1 {
    SsaVariableIdV1::new(index)
}

fn classify<'a>(
    locals: &'a [SemanticLocalDeclV1],
    variable: u32,
    block: u32,
    relays: &[(u32, u32)],
) -> Result<Option<&'a SemanticLocalDeclV1>> {
    epoch_definition_source_local_v1(locals, v(variable), SsaBlockIdV1::new(block), |index| {
        relays
            .get(index)
            .map(|&(variable, pack)| (v(variable), SemanticBlockIdV1::from_index(pack)))
    })
}

#[test]
fn epoch_namespace_source_types_are_exact_and_never_query_the_relay_roster() {
    let locals = locals();
    for (index, expected) in locals.iter().enumerate() {
        let actual = epoch_definition_source_local_v1(
            &locals,
            v(index as u32),
            SsaBlockIdV1::new(0),
            |_| panic!("source local was confused with a completion relay"),
        )
        .unwrap()
        .unwrap();
        assert!(std::ptr::eq(actual, expected));
        assert_eq!(actual.ty(), SemanticTypeIdV1::from_index(index as u32));
    }
}

#[test]
fn epoch_namespace_two_relay_rows_have_no_source_type() {
    let locals = locals();
    let relays = [(4, 8), (5, 3)];
    assert_eq!(classify(&locals, 4, 8, &relays).unwrap(), None);
    assert_eq!(classify(&locals, 5, 3, &relays).unwrap(), None);
    // A high valid ordinal is checked by one lookup, not a scan or a threshold.
    let mut looked_up = None;
    assert!(
        epoch_definition_source_local_v1(&locals, v(101), SsaBlockIdV1::new(9), |index| {
            looked_up = Some(index);
            Some((v(101), SemanticBlockIdV1::from_index(9)))
        },)
        .unwrap()
        .is_none()
    );
    assert_eq!(looked_up, Some(97));
}

#[test]
fn epoch_namespace_unknown_ids_wrong_rows_and_wrong_pack_blocks_reject() {
    let locals = locals();
    for (variable, block, rows) in [
        (4, 8, vec![]),
        (6, 8, vec![(4, 8), (5, 3)]),
        (u32::MAX, 8, vec![(4, 8)]),
        (4, 8, vec![(5, 8)]),
        (5, 3, vec![(5, 3), (4, 8)]),
        (4, 3, vec![(4, 8), (5, 3)]),
    ] {
        assert!(
            matches!(
                classify(&locals, variable, block, &rows),
                Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch)
            ),
            "variable={variable} block={block} rows={rows:?}"
        );
    }
}

fn mixed_events() -> Vec<SsaEventV1> {
    vec![
        SsaEventV1::Define(v(1)),
        SsaEventV1::Define(v(4)),
        SsaEventV1::Use(v(4)),
        SsaEventV1::Kill(v(4)),
        SsaEventV1::Define(v(2)),
        SsaEventV1::Define(v(5)),
        SsaEventV1::Use(v(5)),
        SsaEventV1::Kill(v(5)),
        SsaEventV1::Define(v(3)),
        SsaEventV1::Use(v(1)),
        SsaEventV1::Use(v(2)),
        SsaEventV1::Use(v(3)),
    ]
}

fn planned(
    events: Vec<SsaEventV1>,
) -> std::result::Result<SsaConstructionPlanV1, SsaPlannerErrorV1> {
    plan_ssa_v1(&SsaConstructionInputV1::new(
        SsaBlockIdV1::new(0),
        6,
        vec![true; 6],
        vec![],
        vec![SsaBlockInputV1::new(events, vec![])],
    ))
}

#[test]
fn epoch_namespace_source_definition_selection_continues_on_both_sides_of_relays() {
    let locals = locals();
    let plan = planned(mixed_events()).unwrap();
    let mut selected = Vec::new();
    let mut excluded = Vec::new();
    for (_, event) in plan.resolved_events(SsaBlockIdV1::new(0)).unwrap() {
        if let SsaResolvedEventV1::Define { variable, value } = event {
            match classify(&locals, variable.get(), 0, &[(4, 0), (5, 0)]).unwrap() {
                Some(local) => selected.push((variable.get(), local.ty(), *value)),
                None => excluded.push(variable.get()),
            }
        }
    }
    assert_eq!(excluded, [4, 5]);
    assert_eq!(
        selected
            .iter()
            .map(|(v, ty, _)| (*v, ty.index()))
            .collect::<Vec<_>>(),
        [(1, 1), (2, 2), (3, 3)]
    );
    assert_ne!(selected[0].2, selected[1].2);
    assert_ne!(selected[1].2, selected[2].2);
    // This is the nonempty source-type selection branch, not an epoch issuer.
    assert_eq!(
        selected.iter().filter(|(_, ty, _)| ty.index() == 2).count(),
        1
    );
}

#[test]
fn epoch_namespace_relay_classification_never_replaces_lifecycle_ssa_checks() {
    for variable in [4, 5] {
        let mut missing = mixed_events();
        missing.retain(|event| *event != SsaEventV1::Define(v(variable)));
        assert!(
            matches!(planned(missing), Err(SsaPlannerErrorV1::UndefinedAtUse { variable: actual, .. })
            if actual == v(variable))
        );
        let mut reused = mixed_events();
        reused.push(SsaEventV1::Use(v(variable)));
        assert!(
            matches!(planned(reused), Err(SsaPlannerErrorV1::UndefinedAtUse { variable: actual, .. })
            if actual == v(variable))
        );
    }
}

fn no_epoch_owner() -> ProductionSemanticSsaOwnerV1 {
    ProductionSemanticSsaOwnerV1::try_new(
        super::super::resource_tests::noop_semantic_owner(&["epoch_namespace"]),
        ProductionSemanticSsaLimitsV1::default(),
    )
    .unwrap()
}

fn inert_context() -> RootKernelContextLoweringV1 {
    // The empty-epoch scan does not authenticate this context or issue a value.
    RootKernelContextLoweringV1 {
        selected_root: SemanticFunctionIdV1::from_index(0),
        semantic_type: SemanticTypeIdV1::from_index(0),
        context_type: KernelContextTypeV1::new("epoch_namespace", [1; 32], [2; 32], [3; 32]),
        source: KernelContextSourceIdentityV1::new([4; 32], [5; 32], [6; 32], [7; 32]),
        entry_transfer: None,
    }
}

#[test]
fn epoch_namespace_scan_requires_the_original_owner_plan_even_with_no_epochs() {
    let owner = no_epoch_owner();
    let context = inert_context();
    let mut plan = BorrowedWorkgroupPlanV1::new(&owner, &context, 10_000).unwrap();
    assert!(plan.epochs.is_empty() && plan.epoch_transports.is_empty());
    plan.prepare_epoch_transports().unwrap();
    let copied_ssa: SsaConstructionPlanV1 = (*plan.graph.ssa).clone();
    assert_eq!(&copied_ssa, plan.graph.ssa);
    assert!(!std::ptr::eq(&copied_ssa, plan.graph.ssa));
    plan.graph.ssa = &copied_ssa;
    assert!(matches!(
        plan.prepare_epoch_transports(),
        Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch)
    ));
    assert!(plan.epoch_transports.is_empty());
}

#[test]
fn epoch_namespace_classifier_keeps_the_shared_event_debit_exact() {
    let owner = no_epoch_owner();
    let body = owner
        .execution_view_for_root(SemanticFunctionIdV1::from_index(0))
        .unwrap()
        .body();
    let plan = planned(mixed_events()).unwrap();
    let events = plan.resolved_events(SsaBlockIdV1::new(0)).unwrap();
    let locals = locals();
    let run = |limit: usize, rounds: usize| -> Result<()> {
        // This graph checks the unchanged logical debit only, not source custody.
        let mut graph = Graph::new(body, &plan, limit)?;
        for _ in 0..rounds {
            graph.charge(events.len())?;
            for (_, event) in events {
                if let SsaResolvedEventV1::Define { variable, .. } = event {
                    classify(&locals, variable.get(), 0, &[(4, 0), (5, 0)])?;
                }
            }
        }
        Ok(())
    };
    let mut exact = 0;
    while run(exact, 2).is_err() {
        exact += 1;
        assert!(exact < 1000);
    }
    assert!(run(exact, 2).is_ok());
    assert!(matches!(
        run(exact - 1, 2),
        Err(ProductionSemanticKirErrorV1::ResourceLimit {
            resource: ProductionSemanticKirResourceV1::AnalysisWork,
            ..
        })
    ));
    assert!(
        run(exact, 3).is_err(),
        "another scan must not replenish the graph budget"
    );
}

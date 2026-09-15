use super::*;
use fe2o3_mir_model::SemanticCallExpansionLimitsV1;

#[path = "fixture.rs"]
mod fixture;

fn root() -> SemanticFunctionIdV1 {
    SemanticFunctionIdV1::from_index(0)
}

pub(crate) fn owner() -> ProductionSemanticSsaOwnerV1 {
    ProductionSemanticSsaOwnerV1::try_new(
        ProductionSemanticMirOwnerV1::try_new(
            fixture::source(true, true),
            crate::ProductionSemanticMirLimitsV1::default(),
        )
        .unwrap(),
        ProductionSemanticSsaLimitsV1::default(),
    )
    .unwrap()
}

#[test]
fn checked_result_occurrences_keep_distinct_getter_and_bridge_instances() {
    let source = fixture::source(true, true);
    let expansion =
        SemanticCallExpansionV1::try_new(&source, SemanticCallExpansionLimitsV1::default())
            .unwrap();
    let view = expansion.root(root()).unwrap();
    let relation = DefinedMathResultsV1::derive(
        &source,
        &expansion,
        view,
        ProductionSemanticSsaLimitsV1::default(),
    )
    .unwrap();
    assert_eq!(relation.entries().len(), 2);
    let [first, second] = relation.entries() else {
        unreachable!()
    };
    assert_ne!(first.getter(), second.getter());
    assert_ne!(first.bridge(), second.bridge());
    assert_ne!(first.return_local(), second.return_local());
    for entry in relation.entries() {
        assert_eq!(
            view.instances()[entry.bridge().index() as usize].parent(),
            Some(entry.getter())
        );
        assert_eq!(
            view.local_origins()[entry.return_local().index() as usize].function(),
            SemanticFunctionIdV1::from_index(2)
        );
        assert_eq!(
            view.local_origins()[entry.return_local().index() as usize]
                .local()
                .index(),
            0
        );
        assert_eq!(
            view.local_origins()[entry.current().index() as usize]
                .local()
                .index(),
            1
        );
    }
    relation.verify_view(view).unwrap();
}

#[test]
fn result_prefix_exists_only_at_the_checked_bridge_return() {
    let source = fixture::source(false, true);
    let expansion =
        SemanticCallExpansionV1::try_new(&source, SemanticCallExpansionLimitsV1::default())
            .unwrap();
    let view = expansion.root(root()).unwrap();
    let relation = DefinedMathResultsV1::derive(
        &source,
        &expansion,
        view,
        ProductionSemanticSsaLimitsV1::default(),
    )
    .unwrap();
    let entry = relation.entries()[0];
    let mut events = Vec::new();
    relation.append_result_events(entry.block().index(), entry.statement(), &mut events);
    assert_eq!(
        events,
        [
            SsaEventV1::Use(SsaVariableIdV1::new(entry.receiver().index())),
            SsaEventV1::Use(SsaVariableIdV1::new(entry.current().index())),
            SsaEventV1::Define(SsaVariableIdV1::new(entry.return_local().index())),
        ]
    );
    for (block, statement) in [
        (entry.getter_block().index(), entry.getter_statement()),
        (u32::MAX, 0),
    ] {
        events.clear();
        relation.append_result_events(block, statement, &mut events);
        assert!(events.is_empty());
    }
}

#[test]
fn absent_metadata_does_not_initialize_an_ignored_math_return() {
    let source = fixture::source(false, false);
    let expansion =
        SemanticCallExpansionV1::try_new(&source, SemanticCallExpansionLimitsV1::default())
            .unwrap();
    let relation = DefinedMathResultsV1::derive(
        &source,
        &expansion,
        expansion.root(root()).unwrap(),
        ProductionSemanticSsaLimitsV1::default(),
    )
    .unwrap();
    assert!(relation.entries().is_empty());
    let original = Sha256::new();
    let mut changed = original.clone();
    relation.hash_into(&mut changed);
    assert_eq!(original.finalize(), changed.finalize());
    let error = match ProductionSemanticSsaOwnerV1::try_new(
        ProductionSemanticMirOwnerV1::try_new(
            source,
            crate::ProductionSemanticMirLimitsV1::default(),
        )
        .unwrap(),
        ProductionSemanticSsaLimitsV1::default(),
    ) {
        Err(error) => error,
        Ok(_) => panic!("ordinary ignored return must not receive a fabricated definition"),
    };
    assert!(
        matches!(error, ProductionSemanticSsaErrorV1::ExpandedExecution {
        source_statement: Some(SemanticExpandedStatementOriginV1::ReturnTransfer { .. }), error, ..
    } if matches!(*error, ProductionSemanticSsaErrorV1::Planner { error: SsaPlannerErrorV1::UndefinedAtUse { .. }, .. }))
    );
}

#[test]
fn checked_owner_preserves_original_return_move_and_frame_kills() {
    let owner = owner();
    owner.verify_replay().unwrap();
    let view = owner.execution_view_for_root(root()).unwrap();
    let plan = owner.execution_plan_for_root(root()).unwrap();
    assert!(plan.implicit_entry_variables().is_empty());
    assert!(plan.frame_initializations().is_empty());
    assert_eq!(plan.defined_math_results().len(), 2);
    let sites = adapter::transparent_borrow_sites_for_execution_v1(
        owner.source_semantic(),
        owner.execution_expansion(),
        view,
        ProductionSemanticSsaLimitsV1::default()
            .planner()
            .max_work_units(),
    )
    .unwrap();
    let mut ranges = Vec::new();
    let (input, implicit, _) = adapter::semantic_function_ssa_input_with_frame_initializations_v1(
        view.body(),
        Some(owner.source_semantic().types()),
        owner.source_semantic().callables(),
        &sites,
        &plan.frame_initializations,
        &plan.defined_math_results,
        |block, statement, range| {
            if let Some(entry) = plan.defined_math_results().iter().find(|entry| {
                entry.block().index() == block && Some(entry.statement()) == statement
            }) {
                ranges.push((*entry, range));
            }
        },
    )
    .unwrap();
    assert!(implicit.is_empty());
    assert_eq!(ranges.len(), 2);
    for (entry, range) in ranges {
        let variable = SsaVariableIdV1::new(entry.return_local().index());
        assert_eq!(
            &input.blocks()[entry.block().index() as usize].events()[range],
            &[
                SsaEventV1::Use(SsaVariableIdV1::new(entry.receiver().index())),
                SsaEventV1::Use(SsaVariableIdV1::new(entry.current().index())),
                SsaEventV1::Define(variable),
                SsaEventV1::Use(variable),
                SsaEventV1::Kill(variable),
                SsaEventV1::Define(SsaVariableIdV1::new(entry.destination().index())),
            ]
        );
    }
}

#[test]
fn result_relation_rejects_foreign_view_and_changed_call_coordinates() {
    let source = fixture::source(true, true);
    let expansion =
        SemanticCallExpansionV1::try_new(&source, SemanticCallExpansionLimitsV1::default())
            .unwrap();
    let view = expansion.root(root()).unwrap();
    let expected = DefinedMathResultsV1::derive(
        &source,
        &expansion,
        view,
        ProductionSemanticSsaLimitsV1::default(),
    )
    .unwrap();
    let other = fixture::source(false, true);
    let other_expansion =
        SemanticCallExpansionV1::try_new(&other, SemanticCallExpansionLimitsV1::default()).unwrap();
    let foreign = other_expansion.root(root()).unwrap();
    assert!(matches!(
        DefinedMathResultsV1::derive(
            &source,
            &expansion,
            foreign,
            ProductionSemanticSsaLimitsV1::default()
        ),
        Err(ProductionSemanticSsaErrorV1::ReplayMismatch)
    ));
    assert_eq!(
        expected.verify_view(foreign),
        Err(ProductionSemanticSsaErrorV1::ReplayMismatch)
    );
    for field in 0..5 {
        let mut changed = expected.clone();
        let other = changed.entries[1];
        match field {
            0 => changed.entries[0].getter = other.getter,
            1 => changed.entries[0].receiver = other.receiver,
            2 => changed.entries[0].current = other.current,
            3 => changed.entries[0].return_local = other.return_local,
            4 => changed.entries[0].getter_statement = u32::MAX,
            _ => unreachable!(),
        }
        assert_eq!(
            changed.verify_view(view),
            Err(ProductionSemanticSsaErrorV1::ReplayMismatch)
        );
        let mut old_hash = Sha256::new();
        let mut new_hash = Sha256::new();
        expected.hash_into(&mut old_hash);
        changed.hash_into(&mut new_hash);
        assert_ne!(old_hash.finalize(), new_hash.finalize());
    }
}

#[test]
fn result_relation_resource_limits_are_inclusive() {
    let source = fixture::source(true, true);
    let expansion =
        SemanticCallExpansionV1::try_new(&source, SemanticCallExpansionLimitsV1::default())
            .unwrap();
    let view = expansion.root(root()).unwrap();
    let expected = DefinedMathResultsV1::derive(
        &source,
        &expansion,
        view,
        ProductionSemanticSsaLimitsV1::default(),
    )
    .unwrap();
    let defaults = SsaPlannerLimitsV1::default();
    let limits = |storage, work| {
        ProductionSemanticSsaLimitsV1::new(
            SsaPlannerLimitsV1::try_new(
                defaults.max_variables(),
                defaults.max_blocks(),
                defaults.max_edges(),
                defaults.max_events(),
                defaults.max_edge_definitions(),
                defaults.max_output_items(),
                storage,
                work,
            )
            .unwrap(),
        )
    };
    let resources = expected.resources();
    assert_eq!(
        DefinedMathResultsV1::derive(
            &source,
            &expansion,
            view,
            limits(resources.storage_words, resources.work_units)
        )
        .unwrap(),
        expected
    );
    for (storage, work, expected) in [
        (
            resources.storage_words - 1,
            resources.work_units,
            SsaPlannerResourceV1::StorageWords,
        ),
        (
            resources.storage_words,
            resources.work_units - 1,
            SsaPlannerResourceV1::WorkUnits,
        ),
    ] {
        assert!(
            matches!(DefinedMathResultsV1::derive(&source, &expansion, view, limits(storage, work)),
            Err(ProductionSemanticSsaErrorV1::PartialMoveResourceLimit { resource, .. }) if resource == expected)
        );
    }
}

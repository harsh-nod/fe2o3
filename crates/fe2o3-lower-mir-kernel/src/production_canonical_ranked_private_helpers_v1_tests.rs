use super::*;

fn changed_ssa(kill: Option<bool>, alias: bool) -> ProductionSemanticSsaOwnerV1 {
    let erased_owner = erased();
    let owner = erased_owner.original_source();
    let semantic = owner.semantic_ssa.source_semantic();
    let helper = &semantic.functions()[1];
    let mut statements = helper.blocks()[0].statements().to_vec();
    if let Some(dead) = kill {
        statements.insert(
            1,
            SemanticStatementV1::new(
                helper.source(),
                if dead {
                    SemanticStatementKindV1::StorageDead(SemanticLocalIdV1::from_index(1))
                } else {
                    SemanticStatementKindV1::StorageLive(SemanticLocalIdV1::from_index(1))
                },
            ),
        );
    }
    let fresh = SemanticFunctionDeclV1::new(
        helper.identity(),
        helper.role(),
        helper.item_definition_identity(),
        helper.monomorphization_identity(),
        helper.generic_type_arguments_identity(),
        helper.const_generic_arguments_identity(),
        helper.source(),
        helper.abi().clone(),
        helper.locals().to_vec(),
        SemanticBlockIdV1::from_index(0),
        vec![
            SemanticBasicBlockV1::new(
                helper.blocks()[0].identity(),
                helper.source(),
                statements,
                helper.blocks()[0].terminator().clone(),
            )
            .unwrap(),
        ],
    )
    .unwrap();
    let root = &semantic.functions()[0];
    let mut functions = vec![root.clone(), fresh];
    let mut roots = semantic.roots().to_vec();
    if alias {
        let entry = root.kernel_entry().unwrap();
        let second = SemanticFunctionDeclV1::new(
            SemanticFunctionIdentityV1::from_sha256([230; 32]),
            root.role(),
            SemanticItemDefinitionIdentityV1::from_sha256([231; 32]),
            SemanticMonomorphizationIdentityV1::from_sha256([232; 32]),
            SemanticGenericTypeArgumentsIdentityV1::from_sha256([233; 32]),
            SemanticConstGenericArgumentsIdentityV1::from_sha256([234; 32]),
            root.source(),
            root.abi().clone(),
            root.locals().to_vec(),
            SemanticBlockIdV1::from_index(0),
            root.blocks().to_vec(),
        )
        .unwrap()
        .with_kernel_entry(SemanticKernelEntryV1::new(
            SemanticLinkSymbolV1::new(b"second_private_root".to_vec()).unwrap(),
            SemanticKernelBindingIdentityV1::from_sha256([235; 32]),
            entry.source_contract(),
        ));
        functions.push(second);
        roots.push(SemanticFunctionIdV1::from_index(2));
    }
    let admitted = InertSemanticMirRequestV1::new(
        semantic.target(),
        semantic.types().to_vec(),
        vec![],
        vec![],
        vec![],
        functions,
        roots,
    )
    .unwrap()
    .admit(SemanticMirLimitsV1::default())
    .unwrap();
    let source =
        ProductionSemanticMirOwnerV1::try_new(admitted, ProductionSemanticMirLimitsV1::default())
            .unwrap();
    ProductionSemanticSsaOwnerV1::try_new(source, ProductionSemanticSsaLimitsV1::default()).unwrap()
}

#[test]
fn actual_storage_dead_and_live_between_store_load_are_refused_before_private_admission() {
    for dead in [false, true] {
        let ssa = changed_ssa(Some(dead), false);
        let semantic = ssa.source_semantic();
        let entry = semantic.functions()[0].kernel_entry().unwrap();
        let inputs = [crate::ProductionSourceLaunchRootInputV1::new(
            std::str::from_utf8(entry.export_symbol().as_bytes()).unwrap(),
            *entry.kernel_binding_identity().as_bytes(),
            crate::ProductionSourceLaunchInputV1::new(1, Some([64, 1, 1]), [1, 1, 1]),
        )];
        let launch = crate::ProductionSourceLaunchRosterV1::try_new(semantic, &inputs).unwrap();
        let mut work = Work::new(1 << 40);
        let mut budget = Budget::new(&mut work, S);
        budget.reserve_storage(SIBLING).unwrap();
        // The actual producer must reject this source. There is no admitted
        // private owner and no claim that the later private reader is reached.
        let error = ProductionPreRankedKirOwnerV1::try_materialize_with_budget(
            ssa,
            launch,
            ProductionSemanticKirLimitsV1::default(),
            &mut budget,
        )
        .err()
        .expect("the source-only kill invalidates the following real Load");
        assert!(
            matches!(
                error,
                ProductionPreRankedKirErrorV1::Lowering(
                    ProductionSemanticKirErrorV1::MissingLocalDefinition {
                        function: 1,
                        block: 0,
                        statement: Some(2),
                        ..
                    }
                )
            ),
            "{error:?}"
        );
        assert_eq!(budget.storage(), SIBLING);
    }
}

#[test]
fn every_root_alias_is_paired_with_one_shared_retained_helper_graph() {
    let owner = cr_owner_from_ssa(changed_ssa(None, true));
    assert_eq!(counts(&owner), [1, 1, 1, 2]);
    run_private(&owner, |view, budget| {
        assert_eq!(view.census(budget)?, (1, 2, 2));
        let metadata = view.metadata(budget)?;
        assert_eq!(metadata.function_count(budget)?, 4);
        assert_eq!(metadata.inventory(budget)?.functions().len(), 3);
        assert_eq!(metadata.launches(budget)?.len(), 2);
        let mut helper_coordinates = Vec::new();
        for association in 0..metadata.function_count(budget)? {
            let function = metadata.function(association, budget)?;
            if function.source().role == SemanticKirFunctionRoleV1::InternalHelper {
                helper_coordinates.push(function.canonical().coordinate);
                let frame = metadata
                    .helper_frame(association, budget)?
                    .expect("retained helper frame");
                assert_eq!(frame.allocations().len(), 1);
                assert_eq!(frame.accesses().len(), 2);
            }
        }
        assert_eq!(helper_coordinates.len(), 2);
        assert_eq!(helper_coordinates[0], helper_coordinates[1]);
        assert_eq!(view.policies(budget)?.function_count(budget)?, 3);
        Ok(())
    })
    .unwrap();
}

#[test]
fn zero_emission_source_memory_or_call_is_not_accepted_as_absent_requirements() {
    let erased_owner = erased();
    let owner = erased_owner.original_source();
    let (result, _, _, _) = cr_run_owner(owner, 1 << 40, S, |source, budget| {
        for kind in 0..3 {
            let mut spans = source
                .source
                .spans
                .iter()
                .map(|span| ProductionCanonicalRankedSpanV1 {
                    association: span.association,
                    block: span.block,
                    operations: span.operations.clone(),
                    site: span.site,
                })
                .collect::<Vec<_>>();
            let index = spans.iter().position(|span| match (kind, span.site) {
                (0, ProductionCanonicalRankedSourceSiteV1::Statement { source, .. }) =>
                    matches!(source.kind(), SemanticStatementKindV1::Assign(a) if matches!(a.value().kind(), SemanticRvalueKindV1::Load(_))),
                (1, ProductionCanonicalRankedSourceSiteV1::Statement { source, .. }) =>
                    matches!(source.kind(), SemanticStatementKindV1::Store(_)),
                (2, ProductionCanonicalRankedSourceSiteV1::Terminator { source, .. }) =>
                    matches!(source.kind(), SemanticTerminatorKindV1::Call(_)),
                _ => false,
            }).expect("actual retained source occurrence");
            spans[index].operations.end = spans[index].operations.start;
            let rows = CrSourceRowsV1 {
                associations: source.source.associations.clone(),
                spans,
                origins: source.source.origins.clone(),
                operation_origins: source.source.operation_origins.clone(),
            };
            let hostile = ProductionCanonicalRankedMetadataV1 {
                owner: source.owner,
                inventory: source.inventory,
                calls: source.calls,
                source: &rows,
                arguments: source.arguments,
                contracts: source.contracts,
                guard: source.guard,
            };
            assert!(matches!(
                cr_private_source_profile_v1(&hostile, budget),
                Err(PolicyError::Unsupported {
                    requirement: ProductionCanonicalRankedSourceRequirementV1::GraphIdentity,
                    ..
                })
            ));
        }
        Ok(())
    });
    result.unwrap();
}

#[test]
fn private_reader_component_rejects_test_only_unauthenticated_store_origin_rows() {
    let erased_owner = erased();
    let owner = erased_owner.original_source();
    let (result, _, _, _) = cr_run_owner(owner, 1 << 40, S, |source, budget| {
        let floor = budget.storage();
        let baseline = cr_private_protected_v1(budget, |budget| {
            checked_output_admission_policy3_v1::with_canonical_private_source_reader_v1(
                source,
                budget,
                |memory, _| {
                    assert_eq!(memory, [1, 2]);
                    Ok(())
                },
            )
        });
        baseline.unwrap();
        assert_eq!(budget.storage(), floor);
        let mut spans = source
            .source
            .spans
            .iter()
            .map(|span| ProductionCanonicalRankedSpanV1 {
                association: span.association,
                block: span.block,
                operations: span.operations.clone(),
                site: span.site,
            })
            .collect::<Vec<_>>();
        let store = spans
            .iter()
            .position(|span| {
                matches!(
                    span.site,
                    ProductionCanonicalRankedSourceSiteV1::Statement { source, .. }
                        if matches!(source.kind(), SemanticStatementKindV1::Store(_))
                )
            })
            .expect("real original source Store");
        let synthetic = spans
            .iter()
            .find(|span| {
                span.association == spans[store].association
                    && matches!(
                        span.site,
                        ProductionCanonicalRankedSourceSiteV1::Synthetic(_)
                    )
            })
            .expect("same helper has an actual retained-storage synthetic origin")
            .site;
        // Unauthenticated component fault injection only. The real owner and
        // executable are unchanged; these rows are never called admitted facts.
        spans[store].site = synthetic;
        let rows = CrSourceRowsV1 {
            associations: source.source.associations.clone(),
            spans,
            origins: source.source.origins.clone(),
            operation_origins: source.source.operation_origins.clone(),
        };
        let hostile = ProductionCanonicalRankedMetadataV1 {
            owner: source.owner,
            inventory: source.inventory,
            calls: source.calls,
            source: &rows,
            arguments: source.arguments,
            contracts: source.contracts,
            guard: source.guard,
        };
        let entered = std::cell::Cell::new(false);
        let rejected = cr_private_protected_v1(budget, |budget| {
            checked_output_admission_policy3_v1::with_canonical_private_source_reader_v1(
                &hostile,
                budget,
                |_, _| {
                    entered.set(true);
                    Ok(())
                },
            )
        });
        assert!(
            matches!(
                rejected,
                Err(PolicyError::PrivateSource(
                    ProductionCheckedOutputAdmissionErrorPolicy3V1::Unsupported {
                        phase: "private source",
                        detail: "initializing Store has an actual source statement",
                    }
                ))
            ),
            "{rejected:?}"
        );
        assert!(!entered.get());
        assert_eq!(budget.storage(), floor);
        Ok(())
    });
    result.unwrap();
}

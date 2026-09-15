#[test]
fn unranked_dereference_diagnostic_preserves_guard_and_singleton_gate() {
    let function = projection_function(vec![block(
        97,
        vec![typed_assignment(
            0,
            SCALAR_TYPE,
            SemanticRvalueKindV1::Use(SemanticOperandV1::Copy(dereferenced_place())),
        )],
        SemanticTerminatorKindV1::Return,
    )]);
    let mut contracts = synthetic_local_contracts(&function);
    let error = audit_function_with_local_contracts(&function, &contracts).unwrap_err();
    assert!(matches!(
        error,
        ProductionRankedProjectionErrorV1::UnrankedDereference(_)
    ));
    let text = error.to_string();
    for expected in [
        unranked_dereference_diagnostic_v1::REASON,
        "bb0 site=unavailable local=_3",
        "access=Read requirement=IfMemory",
        "Dereference",
        "projection_count=1 omitted=0",
        "checked_origin=None checked_place_matches=false",
        "checked_expansion=unavailable",
    ] {
        assert!(text.contains(expected), "{expected}: {text}");
    }
    contracts.allocations[3].as_mut().unwrap().singleton_object = true;
    let (operations, sources, _) =
        audit_function_with_local_contracts(&function, &contracts).unwrap();
    assert_eq!(sources.len(), 1);
    assert!(operations.iter().any(|operation| matches!(
        operation,
        ProductionRankedOperationV1::Access {
            kind: AccessKindAttr::Read,
            ..
        }
    )));
}

#[test]
fn unranked_dereference_diagnostic_bounds_projections_and_reports_observed_origins() {
    let function = projection_function(vec![block(97, vec![], SemanticTerminatorKindV1::Return)]);
    let mut contracts = synthetic_local_contracts(&function);
    contracts.allocations[3] = None;
    contracts.allocation_provenance[3] = Some(LocalAllocationProvenanceV1::Private(
        SemanticLocalIdV1::from_index(2),
    ));
    contracts.checked_references.origins[3] = Some(CheckedReferenceOriginV1 {
        source: CheckedReferenceSourceV1::ProjectedSharedBorrow,
        availability: None,
    });
    // Exercise bounded recording directly; these synthetic observations do not
    // bypass the place validator or establish a checked dereference.
    let place = SemanticPlaceV1::new(
        SemanticLocalIdV1::from_index(3),
        vec![
            SemanticProjectionV1::new(SemanticProjectionKindV1::Field(0), SCALAR_TYPE).unwrap();
            1024
        ],
        SCALAR_TYPE,
    )
    .unwrap();
    let error = unranked_dereference_diagnostic_v1::reject(
        &function,
        0,
        &place,
        AccessKindAttr::Read,
        PlaceAccessRequirementV1::IfMemory,
        None,
        Some(MemorySpaceAttr::Global),
        &contracts,
        SemanticSourceProvenanceV1::unavailable(),
    );
    let text = error.to_string();
    assert_eq!(
        text.matches("Field(0)").count(),
        unranked_dereference_diagnostic_v1::MAX_PROJECTIONS
    );
    assert!(text.contains("projection_count=1024 omitted=1016"));
    assert!(text.contains("private_or_argument_origin=Some(Private("));
    assert!(
        text.contains(
            "checked_origin=Some(CheckedReferenceOriginV1 { source: ProjectedSharedBorrow"
        )
    );
    assert!(text.contains("checked_place_matches=false"));
    assert!(text.len() < 4096);
}

fn unranked_expanded_diagnostic_v1(
    owner: &ProductionSemanticSsaOwnerV1,
    site: ProjectedSemanticAccessSiteV1,
) -> ProductionRankedProjectionErrorV1 {
    let view = owner
        .execution_view_for_root(SemanticFunctionIdV1::from_index(0))
        .unwrap();
    let function = view.body();
    let instance = view.block_origins()[site.block].instance();
    let local = view
        .local_origins()
        .iter()
        .position(|origin| origin.instance() == instance)
        .unwrap();
    let place = SemanticPlaceV1::new(
        SemanticLocalIdV1::from_index(local as u32),
        vec![],
        function.locals()[local].ty(),
    )
    .unwrap();
    let contracts = ProjectionLocalContractsV1 {
        checked_references: CheckedReferencesV1 {
            origins: vec![None; function.locals().len()],
            option_dominance: SemanticOptionDominanceV1::analyze(function, &[]).unwrap(),
            enum_payload_dominance: SemanticEnumPayloadDominanceV1::analyze(
                function,
                owner.source_semantic().types(),
            )
            .unwrap(),
        },
        allocations: vec![None; function.locals().len()],
        allocation_provenance: vec![None; function.locals().len()],
    };
    unranked_dereference_diagnostic_v1::reject(
        function,
        site.block,
        &place,
        AccessKindAttr::Read,
        PlaceAccessRequirementV1::IfMemory,
        None,
        None,
        &contracts,
        function.blocks()[site.block].source(),
    )
}

#[test]
fn unranked_dereference_diagnostic_maps_repeated_checked_source_instances() {
    let (owner, _) = expanded_ranked_fixture_v1(false);
    let original_bytes = owner.source_semantic().canonical_encoding().to_vec();
    let view = owner
        .execution_view_for_root(SemanticFunctionIdV1::from_index(0))
        .unwrap();
    let mut instances = BTreeSet::new();
    for (block, origin) in view.block_origins().iter().enumerate() {
        if origin.function().index() != 1 || instances.contains(&origin.instance()) {
            continue;
        }
        let Some((statement, original_statement)) = origin
            .statements()
            .iter()
            .enumerate()
            .find_map(|(index, origin)| match origin {
                fe2o3_mir_model::SemanticExpandedStatementOriginV1::Source { statement } => {
                    Some((index, *statement))
                }
                _ => None,
            })
        else {
            continue;
        };
        instances.insert(origin.instance());
        let site = ProjectedSemanticAccessSiteV1 {
            block,
            statement: Some(statement),
        };
        let error = unranked_expanded_diagnostic_v1(&owner, site);
        let error = unranked_dereference_diagnostic_v1::attach_site(
            error,
            owner.source_semantic(),
            view,
            site,
        );
        let text = error.to_string();
        assert!(
            text.contains(&format!("bb{block} statement={statement}")),
            "{text}"
        );
        assert!(
            text.contains(&format!(
                "original_statement=Some(Source {{ statement: {original_statement} }})"
            )),
            "{text}"
        );
        assert!(
            text.contains(&format!(
                "original_instance_function_block=Some(({}, 1, {}))",
                origin.instance().index(),
                origin.block().index()
            )),
            "{text}"
        );
        assert!(
            text.contains(&format!(
                "frame instance={} function=1",
                origin.instance().index()
            )),
            "{text}"
        );
        assert!(text.contains("frame instance=0 function=0"), "{text}");
        assert!(text.contains("original_local=Some("), "{text}");
        assert!(text.contains("original_location=(Rust source"), "{text}");
        assert!(text.len() < 8192);
    }
    assert_eq!(instances.len(), 2);
    assert_eq!(owner.source_semantic().canonical_encoding(), original_bytes);
}

#[test]
fn unranked_dereference_diagnostic_distinguishes_terminators_and_unavailable_sites() {
    let (owner, _) = expanded_ranked_fixture_v1(false);
    let view = owner
        .execution_view_for_root(SemanticFunctionIdV1::from_index(0))
        .unwrap();
    let block = view
        .block_origins()
        .iter()
        .position(|origin| origin.function().index() == 1)
        .unwrap();
    let site = ProjectedSemanticAccessSiteV1 {
        block,
        statement: None,
    };
    let error = unranked_expanded_diagnostic_v1(&owner, site);
    let text =
        unranked_dereference_diagnostic_v1::attach_site(error, owner.source_semantic(), view, site)
            .to_string();
    assert!(text.contains(&format!("bb{block} terminator")), "{text}");
    assert!(
        text.contains("original_statement=None original_terminator=Some("),
        "{text}"
    );
    let error = unranked_expanded_diagnostic_v1(&owner, site);
    let mismatched = ProjectedSemanticAccessSiteV1 {
        block: usize::MAX,
        statement: None,
    };
    let text = unranked_dereference_diagnostic_v1::attach_site(
        error,
        owner.source_semantic(),
        view,
        mismatched,
    )
    .to_string();
    assert!(text.contains("site=unavailable"));
    assert!(text.contains("checked_expansion=unavailable"));
}

#[test]
fn unranked_dereference_diagnostic_preserves_first_incomplete_and_other_errors() {
    let function = projection_function(vec![block(97, vec![], SemanticTerminatorKindV1::Return)]);
    let contracts = synthetic_local_contracts(&function);
    let failure = || {
        unranked_dereference_diagnostic_v1::reject(
            &function,
            0,
            &dereferenced_place(),
            AccessKindAttr::Read,
            PlaceAccessRequirementV1::IfMemory,
            None,
            Some(MemorySpaceAttr::Global),
            &contracts,
            SemanticSourceProvenanceV1::unavailable(),
        )
    };
    let mut first = Some(ProductionRankedProjectionErrorV1::Incomplete("earlier"));
    retain_incomplete(Err(failure()), &mut first).unwrap();
    assert!(matches!(
        first,
        Some(ProductionRankedProjectionErrorV1::Incomplete("earlier"))
    ));
    let mut first = None;
    retain_incomplete(Err(failure()), &mut first).unwrap();
    retain_incomplete(
        Err(ProductionRankedProjectionErrorV1::Incomplete("later")),
        &mut first,
    )
    .unwrap();
    assert!(matches!(
        first,
        Some(ProductionRankedProjectionErrorV1::UnrankedDereference(_))
    ));
    assert!(matches!(
        retain_incomplete(
            Err(ProductionRankedProjectionErrorV1::Unsupported("hard")),
            &mut first
        ),
        Err(ProductionRankedProjectionErrorV1::Unsupported("hard"))
    ));
}

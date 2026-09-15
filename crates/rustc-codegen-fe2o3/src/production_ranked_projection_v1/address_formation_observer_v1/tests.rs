use super::*;
use fe2o3_mir_model::semantic_mir_v1::*;
use global_enum_transport_v1::tests::{build_owner, build_owner_from_function};

const ROOT: SemanticFunctionIdV1 = SemanticFunctionIdV1::from_index(0);
const STORAGE: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(2);
const REFERENCE: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(3);

fn owner() -> ProductionSemanticSsaOwnerV1 {
    let base = build_owner();
    let original = &base.source_semantic().functions()[0];
    let mut blocks = original.blocks().to_vec();
    let mut statements = blocks[4].statements().to_vec();
    statements[3] = SemanticStatementV1::new(
        original.source(),
        SemanticStatementKindV1::Assign(SemanticAssignmentV1::new(
            SemanticPlaceV1::new(SemanticLocalIdV1::from_index(10), vec![], REFERENCE).unwrap(),
            SemanticRvalueV1::new(
                REFERENCE,
                SemanticRvalueKindV1::Borrow {
                    kind: SemanticBorrowKindV1::Shared,
                    place: SemanticPlaceV1::new(
                        SemanticLocalIdV1::from_index(2),
                        vec![
                            SemanticProjectionV1::new(
                                SemanticProjectionKindV1::Dereference,
                                STORAGE,
                            )
                            .unwrap(),
                        ],
                        STORAGE,
                    )
                    .unwrap(),
                },
            ),
        )),
    );
    blocks[4] = SemanticBasicBlockV1::new(
        blocks[4].identity(),
        blocks[4].source(),
        statements,
        blocks[4].terminator().clone(),
    )
    .unwrap();
    build_owner_from_function(
        SemanticFunctionDeclV1::new(
            original.identity(),
            original.role(),
            original.item_definition_identity(),
            original.monomorphization_identity(),
            original.generic_type_arguments_identity(),
            original.const_generic_arguments_identity(),
            original.source(),
            original.abi().clone(),
            original.locals().to_vec(),
            original.entry(),
            blocks,
        )
        .unwrap()
        .with_kernel_entry(original.kernel_entry().unwrap().clone()),
    )
}

fn site(function: &SemanticFunctionDeclV1) -> (ProjectedSemanticAccessSiteV1, &SemanticPlaceV1) {
    let mut result = None;
    for (block, body) in function.blocks().iter().enumerate() {
        for (statement, source) in body.statements().iter().enumerate() {
            let SemanticStatementKindV1::Assign(assignment) = source.kind() else {
                continue;
            };
            let SemanticRvalueKindV1::Borrow { place, .. } = assignment.value().kind() else {
                continue;
            };
            if place.projections().first().map(|p| p.kind())
                == Some(SemanticProjectionKindV1::Dereference)
            {
                assert!(
                    result
                        .replace((
                            ProjectedSemanticAccessSiteV1 {
                                block,
                                statement: Some(statement)
                            },
                            place
                        ))
                        .is_none()
                );
            }
        }
    }
    result.unwrap()
}

fn contracts(
    owner: &ProductionSemanticSsaOwnerV1,
    function: &SemanticFunctionDeclV1,
) -> ProjectionLocalContractsV1 {
    ProjectionLocalContractsV1 {
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
    }
}

fn error(
    function: &SemanticFunctionDeclV1,
    place: &SemanticPlaceV1,
) -> ProductionRankedProjectionErrorV1 {
    ProductionRankedProjectionErrorV1::MissingAllocationProvenance {
        local: place.local().index(),
        ty: function.locals()[place.local().index() as usize]
            .ty()
            .index(),
        projections: place.projections().len(),
    }
}

#[test]
fn address_observer_preserves_real_missing_provenance_and_exact_retained_query_outcome() {
    let owner = owner();
    owner.verify_replay().unwrap();
    let function = owner.execution_view_for_root(ROOT).unwrap().body();
    let (site, place) = site(function);
    let contracts = contracts(&owner, function);
    let expected =
        project_address_formation(owner.source_semantic().types(), function, place, &contracts)
            .unwrap_err();
    assert!(matches!(
        expected,
        ProductionRankedProjectionErrorV1::MissingAllocationProvenance { .. }
    ));
    let before = format!("{expected}");
    let identity = *owner.identity().as_bytes();
    let resources = format!(
        "{:?}",
        owner.execution_plan_for_root(ROOT).unwrap().resources()
    );
    for enabled in [false, true] {
        let mut output = Vec::new();
        write(
            &expected,
            &owner,
            ROOT,
            function,
            site,
            &contracts,
            enabled,
            MAX_QUERY_STEPS,
            &mut output,
        )
        .unwrap();
        let text = String::from_utf8(output).unwrap();
        if enabled {
            assert!(text.contains("capability-address-place declared_type=3"));
            assert!(text.contains("rvalue=Borrow kind=Shared place=(local=2,type=2,"));
            assert!(text.contains("allocation=None provenance=None checked=None"));
            assert!(text.contains("pointer_kind=Reference pointee=2 address_space=0"));
            assert!(text.contains("original_statement=Some(Source"));
            let query = owner.source_query_for_root(ROOT, function).unwrap();
            let mut remaining = MAX_QUERY_STEPS;
            let expected_use = query.borrow_place_use(
                Site::new(
                    SemanticBlockIdV1::from_index(site.block as u32),
                    site.statement.map(|s| s as u32),
                ),
                place,
                &mut || {
                    if remaining == 0 {
                        false
                    } else {
                        remaining -= 1;
                        true
                    }
                },
            );
            assert!(text.contains(&format!("original_borrow={expected_use:?}")));
            assert!(text.lines().count() <= 5 && text.len() < 4096);
        } else {
            assert!(text.is_empty());
        }
        assert_eq!(
            project_address_formation(owner.source_semantic().types(), function, place, &contracts)
                .unwrap_err()
                .to_string(),
            before
        );
        assert_eq!(*owner.identity().as_bytes(), identity);
        assert_eq!(
            format!(
                "{:?}",
                owner.execution_plan_for_root(ROOT).unwrap().resources()
            ),
            resources
        );
    }
    owner.verify_replay().unwrap();
}

#[test]
fn address_observer_empty_query_allowance_reports_worklimit_without_origin_fallback() {
    let owner = owner();
    let function = owner.execution_view_for_root(ROOT).unwrap().body();
    let (site, place) = site(function);
    let mut output = Vec::new();
    write(
        &error(function, place),
        &owner,
        ROOT,
        function,
        site,
        &contracts(&owner, function),
        true,
        0,
        &mut output,
    )
    .unwrap();
    let text = String::from_utf8(output).unwrap();
    assert!(text.contains("original_borrow=Err(WorkLimit)"));
    assert!(text.contains("steps=0 remaining=0 proof_authority=false"));
    assert!(!text.contains("capability-address-definition"));
}

#[test]
fn address_observer_foreign_body_and_changed_error_coordinates_do_not_report_a_source() {
    let owner = owner();
    let foreign = build_owner();
    let function = owner.execution_view_for_root(ROOT).unwrap().body();
    let (site, place) = site(function);
    let contracts = contracts(&owner, function);
    for (actual_owner, actual_error, actual_site) in [
        (&foreign, error(function, place), site),
        (
            &owner,
            ProductionRankedProjectionErrorV1::Incomplete("unrelated"),
            site,
        ),
        (
            &owner,
            ProductionRankedProjectionErrorV1::MissingAllocationProvenance {
                local: 99,
                ty: 3,
                projections: 1,
            },
            site,
        ),
        (
            &owner,
            ProductionRankedProjectionErrorV1::MissingAllocationProvenance {
                local: 2,
                ty: 99,
                projections: 1,
            },
            site,
        ),
        (
            &owner,
            ProductionRankedProjectionErrorV1::MissingAllocationProvenance {
                local: 2,
                ty: 3,
                projections: 2,
            },
            site,
        ),
        (
            &owner,
            error(function, place),
            ProjectedSemanticAccessSiteV1 {
                block: site.block,
                statement: None,
            },
        ),
        (
            &owner,
            error(function, place),
            ProjectedSemanticAccessSiteV1 {
                block: usize::MAX,
                statement: Some(0),
            },
        ),
    ] {
        let mut output = Vec::new();
        write(
            &actual_error,
            actual_owner,
            ROOT,
            function,
            actual_site,
            &contracts,
            true,
            MAX_QUERY_STEPS,
            &mut output,
        )
        .unwrap();
        assert!(output.is_empty());
    }
}

#[test]
fn address_observer_writer_failure_does_not_mutate_the_original_rejection() {
    struct Refuse;
    impl Write for Refuse {
        fn write(&mut self, _: &[u8]) -> io::Result<usize> {
            Err(io::ErrorKind::BrokenPipe.into())
        }
        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }
    let owner = owner();
    let function = owner.execution_view_for_root(ROOT).unwrap().body();
    let (site, place) = site(function);
    let error = error(function, place);
    let before = error.to_string();
    assert_eq!(
        write(
            &error,
            &owner,
            ROOT,
            function,
            site,
            &contracts(&owner, function),
            true,
            MAX_QUERY_STEPS,
            &mut Refuse
        )
        .unwrap_err()
        .kind(),
        io::ErrorKind::BrokenPipe
    );
    assert_eq!(error.to_string(), before);
    owner.verify_replay().unwrap();
}

#[test]
fn address_observer_assignment_payload_and_projection_output_are_bounded() {
    let scalar = SemanticTypeIdV1::from_index(1);
    let place = SemanticPlaceV1::new(
        SemanticLocalIdV1::from_index(11),
        (0..32)
            .map(|_| SemanticProjectionV1::new(SemanticProjectionKindV1::Field(0), scalar).unwrap())
            .collect(),
        scalar,
    )
    .unwrap();
    let assignment = SemanticAssignmentV1::new(
        SemanticPlaceV1::new(SemanticLocalIdV1::from_index(11), vec![], scalar).unwrap(),
        SemanticRvalueV1::new(
            scalar,
            SemanticRvalueKindV1::aggregate(
                SemanticAggregateKindV1::Tuple,
                vec![SemanticOperandV1::Move(place); 128],
            )
            .unwrap(),
        ),
    );
    let mut output = Vec::new();
    describe_assignment(&mut output, &assignment).unwrap();
    let text = String::from_utf8(output).unwrap();
    assert!(text.contains("arity=128") && text.contains("operands_truncated=true"));
    assert_eq!(text.matches("Move(local=11,").count(), 2);
    assert_eq!(text.matches("Field(0)").count(), 8);
    assert_eq!(text.matches("projection_count=32").count(), 2);
    assert!(text.len() < 2048 && !text.contains('\n'));
}

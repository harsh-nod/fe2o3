use super::super::tests::{test_function, test_types};
use super::*;
use fe2o3_mir_model::semantic_mir_v1::{
    SemanticBasicBlockV1, SemanticBlockIdentityV1, SemanticLocalIdV1, SemanticProjectionV1,
    SemanticSourceProvenanceV1, SemanticTerminatorV1, SemanticTypeLayoutV1,
};

fn fixture() -> (SemanticFunctionDeclV1, Vec<SemanticTypeDeclV1>) {
    let source = SemanticSourceProvenanceV1::unavailable();
    let function = test_function(vec![
        SemanticBasicBlockV1::new(
            SemanticBlockIdentityV1::from_sha256([120; 32]),
            source,
            vec![],
            SemanticTerminatorV1::new(source, SemanticTerminatorKindV1::Return),
        )
        .unwrap(),
    ]);
    let mut types = test_types(false);
    types[0] = SemanticTypeDeclV1::new(
        types[0].identity(),
        types[0].layout_identity(),
        SemanticTypeLayoutV1::new(Some(32), 4).unwrap(),
        SemanticTypeShapeV1::Array {
            element: SemanticTypeIdV1::from_index(1),
            length: 8,
        },
    );
    (function, types)
}

fn location() -> SemanticPartialMoveLocationV1 {
    SemanticPartialMoveLocationV1 {
        function: SemanticFunctionIdV1::from_index(0),
        block: 0,
        statement: Some(0),
    }
}

fn budget(remaining: usize) -> SemanticPartialMoveBudgetV1 {
    let limits = SsaPlannerLimitsV1::default();
    SemanticPartialMoveBudgetV1 {
        function: SemanticFunctionIdV1::from_index(0),
        base_storage_words: 0,
        base_work_units: limits.max_work_units() - remaining,
        state_entries: 0,
        work_units: 0,
        limits,
    }
}

fn projection(kind: SemanticProjectionKindV1, ty: u32) -> SemanticProjectionV1 {
    SemanticProjectionV1::new(kind, SemanticTypeIdV1::from_index(ty)).unwrap()
}

fn index() -> SemanticProjectionV1 {
    projection(
        SemanticProjectionKindV1::Index(SemanticLocalIdV1::from_index(2)),
        1,
    )
}

fn place(projections: Vec<SemanticProjectionV1>) -> SemanticPlaceV1 {
    let result = projections.last().unwrap().result_type();
    SemanticPlaceV1::new(SemanticLocalIdV1::from_index(1), projections, result).unwrap()
}

#[test]
fn indexed_path_scan_has_an_exact_work_boundary() {
    let (function, types) = fixture();
    let place = place(vec![index()]);
    let mut exact = budget(1);
    assert!(
        is_supported_indexed_destination_v1(
            &function,
            Some(&types),
            &place,
            location(),
            &mut exact
        )
        .unwrap()
    );
    assert_eq!(exact.work_units, 1);
    assert!(matches!(
        is_supported_indexed_destination_v1(
            &function,
            Some(&types),
            &place,
            location(),
            &mut budget(0)
        ),
        Err(ProductionSemanticSsaErrorV1::PartialMoveResourceLimit {
            resource: SsaPlannerResourceV1::WorkUnits,
            ..
        })
    ));
}

#[test]
fn indexed_path_cannot_hide_unsupported_prefixes_or_suffixes() {
    let (function, types) = fixture();
    for kind in [
        SemanticProjectionKindV1::OpaqueCast,
        SemanticProjectionKindV1::Subtype,
        SemanticProjectionKindV1::Downcast(0),
        SemanticProjectionKindV1::Dereference,
    ] {
        for projections in [
            vec![projection(kind, 0), index()],
            vec![index(), projection(kind, 1)],
        ] {
            assert!(
                !is_supported_indexed_destination_v1(
                    &function,
                    Some(&types),
                    &place(projections),
                    location(),
                    &mut budget(8)
                )
                .unwrap()
            );
        }
    }
    assert!(
        !is_supported_indexed_destination_v1(
            &function,
            None,
            &place(vec![index()]),
            location(),
            &mut budget(8)
        )
        .unwrap()
    );
}

#[test]
fn indexed_path_rejects_union_fields_after_the_index_and_wrong_element_type() {
    let (function, mut types) = fixture();
    let union = test_types(true).remove(0);
    types[1] = union;
    assert!(matches!(
        is_supported_indexed_destination_v1(
            &function,
            Some(&types),
            &place(vec![
                index(),
                projection(SemanticProjectionKindV1::Field(0), 1)
            ]),
            location(),
            &mut budget(8),
        ),
        Err(ProductionSemanticSsaErrorV1::PartialMove {
            violation: SemanticPartialMoveViolationV1::UnionField,
            ..
        })
    ));
    let wrong = place(vec![projection(
        SemanticProjectionKindV1::Index(SemanticLocalIdV1::from_index(2)),
        0,
    )]);
    assert!(
        !is_supported_indexed_destination_v1(
            &function,
            Some(&types),
            &wrong,
            location(),
            &mut budget(8)
        )
        .unwrap()
    );
}

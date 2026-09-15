use super::*;

fn unit(tag: u8) -> SemanticTypeDeclV1 {
    SemanticTypeDeclV1::new(
        SemanticTypeIdentityV1::from_sha256([tag; 32]),
        SemanticLayoutIdentityV1::from_sha256([tag; 32]),
        SemanticTypeLayoutV1::with_exact_rustc_layout(
            0,
            1,
            SemanticFieldsShapeV1::arbitrary(vec![], vec![]).unwrap(),
            SemanticRustcVariantsV1::Single { index: 0 },
            SemanticBackendReprV1::memory(true),
            None,
            false,
            None,
            1,
            0,
            SemanticTypeLayoutDetailsV1::None,
        )
        .unwrap(),
        SemanticTypeShapeV1::Unit,
    )
}

fn request(types: Vec<SemanticTypeDeclV1>) -> InertSemanticMirRequestV1 {
    InertSemanticMirRequestV1::new(
        SemanticTargetDataLayoutV1::gfx942(SemanticLayoutIdentityV1::from_sha256([90; 32])),
        types,
        vec![],
        vec![],
        vec![],
        vec![],
        vec![],
    )
    .unwrap()
}

#[test]
fn layout_diagnostic_returns_the_failing_type_without_admitting_a_request() {
    let good = request(vec![unit(1)]);
    assert_eq!(
        diagnose_type_layouts_v1(&good, SemanticMirLimitsV1::default()),
        Ok(())
    );
    assert!(
        good.admit_current_production(SemanticMirLimitsV1::default())
            .is_err()
    );

    let mut primitive_unit = unit(1);
    primitive_unit.layout = SemanticTypeLayoutV1::new(Some(0), 1).unwrap();
    assert_eq!(
        diagnose_type_layouts_v1(
            &request(vec![primitive_unit]),
            SemanticMirLimitsV1::default()
        ),
        Err((
            SemanticMirLocationV1::Type(SemanticTypeIdV1(0)),
            SemanticMirErrorV1::InvalidTypeLayout
        ))
    );

    let mut bad = unit(2);
    bad.shape = SemanticTypeShapeV1::Scalar(SemanticScalarTypeV1::Float { bits: 32 });
    let bad = request(vec![unit(1), bad]);
    assert_eq!(
        diagnose_type_layouts_v1(&bad, SemanticMirLimitsV1::default()),
        Err((
            SemanticMirLocationV1::Type(SemanticTypeIdV1(1)),
            SemanticMirErrorV1::InvalidTypeLayout
        ))
    );
}

#[test]
fn layout_diagnostic_finds_conflicting_layout_identity() {
    let mut conflict = unit(2);
    conflict.layout_identity = unit(1).layout_identity;
    conflict.layout = SemanticTypeLayoutV1::new(Some(8), 8).unwrap();
    assert_eq!(
        diagnose_type_layouts_v1(
            &request(vec![unit(1), conflict]),
            SemanticMirLimitsV1::default()
        ),
        Err((
            SemanticMirLocationV1::Type(SemanticTypeIdV1(1)),
            SemanticMirErrorV1::InvalidTypeLayout
        ))
    );
}

#[test]
fn layout_diagnostic_obeys_the_requested_work_bound() {
    let limits = SemanticMirLimitsV1::default()
        .with_limit(SemanticMirResourceV1::ValidationWork, 0)
        .unwrap();
    assert!(matches!(
        diagnose_type_layouts_v1(&request(vec![unit(1)]), limits),
        Err((
            SemanticMirLocationV1::Type(SemanticTypeIdV1(0)),
            SemanticMirErrorV1::LimitExceeded {
                resource: SemanticMirResourceV1::ValidationWork,
                ..
            }
        ))
    ));
}

#[test]
fn layout_diagnostic_localizes_a_misaligned_aggregate_field() {
    let scalar = SemanticTypeDeclV1::new(
        SemanticTypeIdentityV1::from_sha256([2; 32]),
        SemanticLayoutIdentityV1::from_sha256([2; 32]),
        SemanticTypeLayoutV1::new_with_backend_repr(
            Some(4),
            4,
            SemanticBackendReprV1::scalar(SemanticBackendScalarV1::initialized(
                SemanticBackendPrimitiveV1::integer(false, 32, 4),
                SemanticScalarValidityRangeV1::new(0, u32::MAX.into()),
            )),
            false,
        )
        .unwrap(),
        SemanticTypeShapeV1::Scalar(SemanticScalarTypeV1::Integer {
            signed: false,
            bits: 32,
        }),
    );
    let aggregate = |offset| {
        SemanticTypeDeclV1::new(
            SemanticTypeIdentityV1::from_sha256([3; 32]),
            SemanticLayoutIdentityV1::from_sha256([3; 32]),
            SemanticTypeLayoutV1::aggregate(
                Some(8),
                8,
                SemanticAggregateLayoutV1::new(vec![offset], vec![]).unwrap(),
            )
            .unwrap(),
            SemanticTypeShapeV1::Aggregate(
                SemanticAggregateTypeV1::new(vec![SemanticTypeIdV1(1)]).unwrap(),
            ),
        )
    };
    assert_eq!(
        diagnose_type_layouts_v1(
            &request(vec![unit(1), scalar.clone(), aggregate(4)]),
            SemanticMirLimitsV1::default()
        ),
        Ok(())
    );
    assert_eq!(
        diagnose_type_layouts_v1(
            &request(vec![unit(1), scalar, aggregate(1)]),
            SemanticMirLimitsV1::default()
        ),
        Err((
            SemanticMirLocationV1::Type(SemanticTypeIdV1(2)),
            SemanticMirErrorV1::InvalidTypeLayout
        ))
    );
}

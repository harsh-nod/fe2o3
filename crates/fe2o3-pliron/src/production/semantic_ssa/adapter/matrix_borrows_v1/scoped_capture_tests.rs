// Pointer/site and accounting components only, not defined-binding authority.
fn assignment() -> SemanticStatementKindV1 {
    let ty = SemanticTypeIdV1::from_index(0);
    SemanticStatementKindV1::Assign(SemanticAssignmentV1::new(
        SemanticPlaceV1::new(SemanticLocalIdV1::from_index(3), vec![], ty).unwrap(),
        SemanticRvalueV1::new(
            ty,
            SemanticRvalueKindV1::Use(SemanticOperandV1::Constant(SemanticConstantV1::new(
                ty,
                SemanticConstantValueV1::ZeroSized,
            ))),
        ),
    ))
}

#[test]
fn scoped_matrix_capture_requires_original_node_and_exact_site() {
    let original = assignment();
    let cloned = original.clone();
    let SemanticStatementKindV1::Assign(a) = &original else {
        unreachable!()
    };
    let site = SemanticTransparentBorrowSiteV1 {
        block: 4,
        statement: 2,
    };
    let mut captures = ScopedCaptures::default();
    captures.sites.insert(
        site,
        Capture {
            assignment: a,
            locals: [7, 9],
            len: 2,
        },
    );
    let mut charged = 0;
    assert_eq!(
        captures
            .captured(site, &original, &mut |n| {
                charged += n;
                Ok(())
            })
            .unwrap(),
        Some(&[7, 9][..])
    );
    assert_eq!(charged, key_work(1));
    assert!(
        captures
            .captured(site, &cloned, &mut |_| Ok(()))
            .unwrap()
            .is_none()
    );
    assert!(
        captures
            .captured(
                SemanticTransparentBorrowSiteV1 { block: 5, ..site },
                &original,
                &mut |_| Ok(())
            )
            .unwrap()
            .is_none()
    );
    assert!(
        captures
            .captured(
                SemanticTransparentBorrowSiteV1 {
                    statement: 3,
                    ..site
                },
                &original,
                &mut |_| Ok(())
            )
            .unwrap()
            .is_none()
    );
    assert!(
        captures
            .captured(site, &original, &mut |_| Err(
                ProductionSemanticSsaErrorV1::ResourceOverflow
            ))
            .is_err()
    );
}

#[test]
fn scoped_matrix_capture_registration_rejects_over_budget_without_refund() {
    let mut captures = ScopedCaptures::default();
    captures.charge(4, 4).unwrap();
    assert_eq!(captures.work_units(), 4);
    assert!(matches!(
        captures.charge(1, 4),
        Err(ProductionSemanticSsaErrorV1::AggregateResourceLimit {
            required: 5,
            limit: 4,
            ..
        })
    ));
    assert_eq!(captures.work_units(), 4);
}

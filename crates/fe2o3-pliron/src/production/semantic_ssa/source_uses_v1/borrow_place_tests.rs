#[test]
fn retained_borrow_query_requires_a_borrow_node_not_a_copy_use_or_foreign_site() {
    for expanded in [false, true] {
        let owner = owner(expanded);
        owner.verify_replay().unwrap();
        let view = owner.execution_view_for_root(ROOT).unwrap();
        let query = owner.source_query_for_root(ROOT, view.body()).unwrap();
        let site = source_site(view, u32::from(expanded), 1);
        let operand = operand(view.body(), site);
        let SemanticOperandV1::Copy(place) = operand else {
            panic!("Copy");
        };
        let mut remaining = 4096;
        query
            .operand_use(site, operand, &mut charger(&mut remaining))
            .unwrap();
        assert!(matches!(
            query.borrow_place_use(site, place, &mut charger(&mut remaining)),
            Err(QueryError::OperandOutsideSite)
        ));
        assert!(matches!(
            query.borrow_place_use(
                Site::new(site.block(), None),
                place,
                &mut charger(&mut remaining)
            ),
            Err(QueryError::InvalidSite)
        ));
        assert!(matches!(
            query.borrow_place_use(
                Site::new(SemanticBlockIdV1::from_index(u32::MAX), Some(0)),
                place,
                &mut charger(&mut remaining)
            ),
            Err(QueryError::InvalidSite)
        ));
    }
}

#[test]
fn retained_borrow_query_spends_the_callers_allowance_without_definition_fallback() {
    let owner = owner(false);
    let view = owner.execution_view_for_root(ROOT).unwrap();
    let query = owner.source_query_for_root(ROOT, view.body()).unwrap();
    let site = source_site(view, 0, 1);
    let SemanticOperandV1::Copy(place) = operand(view.body(), site) else {
        panic!("Copy");
    };
    let mut remaining = 0;
    assert!(matches!(
        query.borrow_place_use(site, place, &mut charger(&mut remaining)),
        Err(QueryError::WorkLimit)
    ));
    assert_eq!(remaining, 0);
    remaining = 1;
    assert!(matches!(
        query.borrow_place_use(site, place, &mut charger(&mut remaining)),
        Err(QueryError::OperandOutsideSite)
    ));
    assert_eq!(remaining, 0);
}

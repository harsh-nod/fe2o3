use super::*;

#[test]
fn retained_source_operand_does_not_invent_a_promoted_storage_use() {
    for expanded in [false, true] {
        let source = source_with_statements(
            expanded,
            vec![
                assign(1, constant(7)),
                assign(2, copy(1)),
                SemanticStatementV1::new(
                    SemanticSourceProvenanceV1::unavailable(),
                    SemanticStatementKindV1::Deinitialize(place(1)),
                ),
            ],
        );
        let owner = make_owner(source, ProductionSemanticSsaLimitsV1::default()).unwrap();
        owner.verify_replay().unwrap();
        let view = owner.execution_view_for_root(ROOT).unwrap();
        let query = owner.source_query_for_root(ROOT, view.body()).unwrap();
        let site = source_site(view, u32::from(expanded), 1);
        let operand = operand(view.body(), site);
        let SemanticOperandV1::Copy(place) = operand else {
            panic!("original Copy");
        };
        assert!(
            !query
                .plan()
                .plan()
                .promoted_variables()
                .contains(&SsaVariableIdV1::new(place.local().index()),)
        );
        let mut remaining = 4096;
        let selected = query
            .operand_source(site, operand, &mut charger(&mut remaining))
            .unwrap();
        assert!(selected.belongs_to(&query));
        assert!(std::ptr::eq(selected.operand(), operand));
        assert_eq!(selected.site(), site);
        assert_eq!(selected.local(), place.local());
        assert!(matches!(
            query.operand_use(site, operand, &mut charger(&mut remaining)),
            Err(QueryError::NoPromotedUse)
        ));
    }
}

#[test]
fn retained_source_operand_preserves_owner_site_and_original_pointer() {
    let owner = owner(false);
    let other = self::owner(false);
    let view = owner.execution_view_for_root(ROOT).unwrap();
    let query = owner.source_query_for_root(ROOT, view.body()).unwrap();
    let other_view = other.execution_view_for_root(ROOT).unwrap();
    let other_query = other
        .source_query_for_root(ROOT, other_view.body())
        .unwrap();
    let site = source_site(view, 0, 1);
    let operand = operand(view.body(), site);
    let mut remaining = 4096;
    let selected = query
        .operand_source(site, operand, &mut charger(&mut remaining))
        .unwrap();
    assert!(!selected.belongs_to(&other_query));
    let cloned = operand.clone();
    assert!(matches!(
        query.operand_source(site, &cloned, &mut charger(&mut remaining)),
        Err(QueryError::OperandOutsideSite)
    ));
    assert!(matches!(
        query.operand_source(
            source_site(view, 0, 3),
            operand,
            &mut charger(&mut remaining)
        ),
        Err(QueryError::OperandOutsideSite)
    ));
    assert!(matches!(
        other_query.operand_source(site, operand, &mut charger(&mut remaining)),
        Err(QueryError::OperandOutsideSite)
    ));
    let changed_kind = match operand {
        SemanticOperandV1::Copy(place) => SemanticOperandV1::Move(place.clone()),
        _ => panic!("Copy"),
    };
    assert!(matches!(
        query.operand_source(site, &changed_kind, &mut charger(&mut remaining)),
        Err(QueryError::OperandOutsideSite)
    ));
    assert!(matches!(
        query.operand_source(site, &constant(0), &mut charger(&mut remaining)),
        Err(QueryError::UnsupportedOperand)
    ));
    assert!(matches!(
        query.operand_source(
            Site::new(SemanticBlockIdV1::from_index(u32::MAX), None),
            operand,
            &mut charger(&mut remaining),
        ),
        Err(QueryError::InvalidSite)
    ));
}

#[test]
fn retained_source_operand_and_promoted_use_share_exact_membership_work() {
    let owner = owner(false);
    let view = owner.execution_view_for_root(ROOT).unwrap();
    let query = owner.source_query_for_root(ROOT, view.body()).unwrap();
    let site = source_site(view, 0, 1);
    let operand = operand(view.body(), site);
    for allowance in 0..=1 {
        let mut source_work = allowance;
        let mut ssa_work = allowance;
        assert!(matches!(
            query.operand_source(site, operand, &mut charger(&mut source_work)),
            Err(QueryError::WorkLimit)
        ));
        assert!(matches!(
            query.operand_use(site, operand, &mut charger(&mut ssa_work)),
            Err(QueryError::WorkLimit)
        ));
        assert_eq!(source_work, ssa_work);
        assert_eq!(source_work, 0);
    }
    let mut remaining = 4096;
    let selected = query
        .operand_source(site, operand, &mut charger(&mut remaining))
        .unwrap();
    let use_ = query
        .operand_use(site, operand, &mut charger(&mut remaining))
        .unwrap();
    assert_eq!(selected.site(), use_.site());
    assert_eq!(selected.local().index(), use_.variable().get());
    assert!(std::ptr::eq(selected.operand(), use_.operand()));
    assert!(matches!(use_.value(), SsaValueV1::Definition(_)));
}

use super::*;

#[test]
fn retained_source_event_window_preserves_complete_copy_move_and_repeated_uses() {
    for expanded in [false, true] {
        let owner = owner(expanded);
        owner.verify_replay().unwrap();
        let view = owner.execution_view_for_root(ROOT).unwrap();
        let query = owner.source_query_for_root(ROOT, view.body()).unwrap();
        let mut budget = 8192;
        for statement in [1, 3, 5] {
            let site = source_site(view, u32::from(expanded), statement);
            let rows = query
                .resolved_events_at(site, &mut charger(&mut budget))
                .unwrap();
            match (statement, rows) {
                (
                    1,
                    [
                        (_, SsaResolvedEventV1::Use { .. }),
                        (_, SsaResolvedEventV1::Define { .. }),
                    ],
                ) => {}
                (
                    3,
                    [
                        (_, SsaResolvedEventV1::Use { variable: a, value }),
                        (
                            _,
                            SsaResolvedEventV1::Kill {
                                variable: b,
                                previous: Some(killed),
                            },
                        ),
                        (_, SsaResolvedEventV1::Define { .. }),
                    ],
                ) => {
                    assert_eq!(a, b);
                    assert_eq!(value, killed);
                }
                (
                    5,
                    [
                        (
                            _,
                            SsaResolvedEventV1::Use {
                                variable: a,
                                value: av,
                            },
                        ),
                        (
                            _,
                            SsaResolvedEventV1::Use {
                                variable: b,
                                value: bv,
                            },
                        ),
                        (_, SsaResolvedEventV1::Define { .. }),
                    ],
                ) => {
                    assert_eq!(a, b);
                    assert_eq!(av, bv);
                }
                _ => {
                    panic!("original adapter window changed: statement={statement}, rows={rows:?}")
                }
            }
            let use_ = query
                .operand_use(site, operand(view.body(), site), &mut charger(&mut budget))
                .unwrap();
            let all = query
                .plan()
                .plan()
                .resolved_events(SsaBlockIdV1::new(site.block.index()))
                .unwrap();
            let expected = all
                .iter()
                .filter(|(event, _)| use_.event_range().contains(&(*event as usize)))
                .collect::<Vec<_>>();
            assert_eq!(rows.len(), expected.len());
            for (row, expected) in rows.iter().zip(expected) {
                assert!(std::ptr::eq(row, expected));
                assert_eq!(
                    query
                        .event_site(
                            SsaBlockIdV1::new(site.block.index()),
                            row.0,
                            &mut charger(&mut budget)
                        )
                        .unwrap(),
                    site
                );
            }
        }
    }
}

#[test]
fn retained_source_event_window_empty_site_does_not_borrow_neighbor_events() {
    for expanded in [false, true] {
        let owner = owner(expanded);
        let view = owner.execution_view_for_root(ROOT).unwrap();
        let query = owner.source_query_for_root(ROOT, view.body()).unwrap();
        let mut budget = 8192;
        let previous = source_site(view, u32::from(expanded), 5);
        assert_eq!(
            query
                .resolved_events_at(previous, &mut charger(&mut budget))
                .unwrap()
                .len(),
            3
        );
        let empty = source_site(view, u32::from(expanded), 6);
        assert!(matches!(
            query.resolved_events_at(empty, &mut charger(&mut budget)),
            Err(QueryError::NoPromotedUse)
        ));
        let invalid = Site::new(empty.block(), Some(u32::MAX));
        assert!(matches!(
            query.resolved_events_at(invalid, &mut charger(&mut budget)),
            Err(QueryError::NoPromotedUse)
        ));
        assert!(matches!(
            query.resolved_events_at(
                Site::new(SemanticBlockIdV1::from_index(u32::MAX), None),
                &mut charger(&mut budget)
            ),
            Err(QueryError::InvalidSite)
        ));
    }
}

#[test]
fn retained_source_event_window_uses_one_exact_shared_budget() {
    let owner = owner(true);
    let view = owner.execution_view_for_root(ROOT).unwrap();
    let query = owner.source_query_for_root(ROOT, view.body()).unwrap();
    let site = source_site(view, 1, 3);
    let mut remaining = 8192;
    query
        .resolved_events_at(site, &mut charger(&mut remaining))
        .unwrap();
    let cost = 8192 - remaining;
    assert!(cost > 0);
    for limit in 0..cost {
        let mut remaining = limit;
        assert!(matches!(
            query.resolved_events_at(site, &mut charger(&mut remaining)),
            Err(QueryError::WorkLimit)
        ));
        assert_eq!(remaining, 0);
    }
    let mut remaining = 2 * cost;
    let first = query
        .resolved_events_at(site, &mut charger(&mut remaining))
        .unwrap();
    assert_eq!(remaining, cost);
    let second = query
        .resolved_events_at(site, &mut charger(&mut remaining))
        .unwrap();
    assert!(std::ptr::eq(first, second));
    assert_eq!(remaining, 0);
    assert!(matches!(
        query.resolved_events_at(site, &mut charger(&mut remaining)),
        Err(QueryError::WorkLimit)
    ));
    owner.verify_replay().unwrap();
}

// Dispatch/storage tests only. Real source registration is covered separately.
use super::*;

fn site(block: u32, statement: u32) -> Site {
    Site { block, statement }
}

fn node() -> SemanticStatementKindV1 {
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

fn records<'a>(node: &'a SemanticStatementKindV1, sites: &[Site]) -> ScopedCaptures<'a> {
    let SemanticStatementKindV1::Assign(assignment) = node else {
        unreachable!()
    };
    let mut records = ScopedCaptures::default();
    for (index, &site) in sites.iter().enumerate() {
        records.sites.insert(
            site,
            Capture {
                assignment,
                locals: [index as u32, 99],
                len: 2,
            },
        );
    }
    records
}

#[test]
fn ordered_cursor_matches_point_queries_for_every_sparse_subsequence() {
    let original = node();
    let cloned = original.clone();
    let sites = [site(0, 1), site(1, 0), site(1, 4), site(3, 2)];
    let records = records(&original, &sites);
    for selected in 0..16 {
        for foreign in 0..16 {
            let mut cursor = records.cursor(&mut |_| Ok(())).unwrap();
            for (index, &point) in sites.iter().enumerate() {
                if selected & (1 << index) == 0 {
                    continue;
                }
                let node = if foreign & (1 << index) == 0 {
                    &original
                } else {
                    &cloned
                };
                assert_eq!(
                    cursor.captured(point, node, &mut |_| Ok(())).unwrap(),
                    records.captured(point, node, &mut |_| Ok(())).unwrap(),
                );
            }
            assert!(
                cursor
                    .captured(site(4, 0), &original, &mut |_| Ok(()))
                    .unwrap()
                    .is_none()
            );
        }
    }
}

#[test]
fn ordered_cursor_requires_original_node_exact_site_and_order() {
    let original = node();
    let cloned = original.clone();
    let records = records(&original, &[site(2, 3), site(4, 7)]);
    let mut cursor = records.cursor(&mut |_| Ok(())).unwrap();
    assert!(
        cursor
            .captured(site(2, 2), &original, &mut |_| Ok(()))
            .unwrap()
            .is_none()
    );
    assert!(
        cursor
            .captured(site(2, 3), &cloned, &mut |_| Ok(()))
            .unwrap()
            .is_none()
    );
    assert!(
        cursor
            .captured(site(3, 7), &original, &mut |_| Ok(()))
            .unwrap()
            .is_none()
    );
    assert_eq!(
        cursor
            .captured(site(4, 7), &original, &mut |_| Ok(()))
            .unwrap(),
        Some(&[1, 99][..])
    );
    for repeated in [site(4, 7), site(0, 0)] {
        let mut cursor = records.cursor(&mut |_| Ok(())).unwrap();
        cursor
            .captured(site(4, 7), &original, &mut |_| Ok(()))
            .unwrap();
        assert!(matches!(
            cursor.captured(repeated, &original, &mut |_| Ok(())),
            Err(ProductionSemanticSsaErrorV1::ReplayMismatch)
        ));
        assert!(
            cursor
                .captured(site(5, 0), &original, &mut |_| Ok(()))
                .is_err()
        );
    }
}

#[test]
fn ordered_cursor_accepts_maximum_coordinates_without_increment_or_alias() {
    let original = node();
    let records = records(&original, &[site(u32::MAX, u32::MAX)]);
    let mut cursor = records.cursor(&mut |_| Ok(())).unwrap();
    assert!(
        cursor
            .captured(site(u32::MAX, u32::MAX - 1), &original, &mut |_| Ok(()))
            .unwrap()
            .is_none()
    );
    assert_eq!(
        cursor
            .captured(site(u32::MAX, u32::MAX), &original, &mut |_| Ok(()))
            .unwrap(),
        Some(&[0, 99][..])
    );
}

#[test]
fn empty_capture_inventory_retains_zero_dispatch_work() {
    let records = ScopedCaptures::default();
    let mut cursor = records
        .cursor(&mut |_| panic!("empty construction charged work"))
        .unwrap();
    assert!(
        cursor
            .captured(site(0, 0), &node(), &mut |_| panic!(
                "empty lookup charged work"
            ))
            .unwrap()
            .is_none()
    );
}

#[test]
fn ordered_cursor_charges_logical_stream_work_without_per_statement_tree_lookup() {
    let original = node();
    const STATEMENTS: u32 = 20_000;
    for captures in [1, 8, 64] {
        let sites = (0..captures).map(|i| site(0, i * 271)).collect::<Vec<_>>();
        let records = records(&original, &sites);
        let mut work = 0;
        let mut charge = |n| {
            work += n;
            Ok(())
        };
        let mut cursor = records.cursor(&mut charge).unwrap();
        let mut matched = 0;
        for statement in 0..STATEMENTS {
            matched += usize::from(
                cursor
                    .captured(site(0, statement), &original, &mut charge)
                    .unwrap()
                    .is_some(),
            );
        }
        assert_eq!(matched, captures as usize);
        let setup = size_of::<CaptureCursor<'_, '_>>().div_ceil(size_of::<usize>())
            + key_work(captures as usize);
        assert_eq!(work, setup + 2 * STATEMENTS as usize + captures as usize);
        let baseline = STATEMENTS as usize * key_work(captures as usize);
        if captures > 1 {
            assert!(work < baseline);
        }
        eprintln!(
            "synthetic ordered capture stream statements={STATEMENTS} captures={captures} baseline={baseline} cursor={work}"
        );
    }
}

#[test]
fn skipped_capture_records_pay_for_each_advance_and_cannot_match_later() {
    let original = node();
    let records = records(&original, &[site(0, 1), site(0, 3), site(2, 0)]);
    let mut cursor = records.cursor(&mut |_| Ok(())).unwrap();
    let mut work = 0;
    assert!(
        cursor
            .captured(site(3, 0), &original, &mut |n| {
                work += n;
                Ok(())
            })
            .unwrap()
            .is_none()
    );
    assert_eq!(work, 2 + 2 * records.sites.len());
    assert!(
        cursor
            .captured(site(0, 1), &original, &mut |_| Ok(()))
            .is_err()
    );
}

#[test]
fn cursor_work_failure_has_no_refund_partial_result_or_retry_with_fresh_budget() {
    let original = node();
    let records = records(&original, &[site(0, 1), site(1, 0), site(2, 3)]);
    let mut required = 0;
    let mut cursor = records
        .cursor(&mut |n| {
            required += n;
            Ok(())
        })
        .unwrap();
    assert!(
        cursor
            .captured(site(2, 3), &original, &mut |n| {
                required += n;
                Ok(())
            })
            .unwrap()
            .is_some()
    );
    const PREFIX: usize = 41;
    for allowance in 0..required {
        let limit = PREFIX + allowance;
        let mut remaining = limit - PREFIX;
        let mut spent = PREFIX;
        let mut charge = |n| {
            if n > remaining {
                return Err(ProductionSemanticSsaErrorV1::AggregateResourceLimit {
                    resource: SsaPlannerResourceV1::WorkUnits,
                    required: limit + 1,
                    limit,
                });
            }
            remaining -= n;
            spent += n;
            Ok(())
        };
        match records.cursor(&mut charge) {
            Err(ProductionSemanticSsaErrorV1::AggregateResourceLimit {
                required,
                limit: actual,
                ..
            }) => {
                assert_eq!(required, limit + 1);
                assert_eq!(actual, limit);
            }
            Ok(mut cursor) => {
                assert!(
                    matches!(cursor.captured(site(2, 3), &original, &mut charge),
                    Err(ProductionSemanticSsaErrorV1::AggregateResourceLimit { required, limit: actual, .. })
                    if required == limit + 1 && actual == limit)
                );
                assert!(matches!(
                    cursor.captured(site(2, 3), &original, &mut |_| Ok(())),
                    Err(ProductionSemanticSsaErrorV1::ReplayMismatch)
                ));
            }
            Err(error) => panic!("unexpected error: {error:?}"),
        }
        assert_eq!(remaining + spent, limit);
        assert!(spent >= PREFIX);
    }
    let mut remaining = required;
    let mut charge = |n| {
        remaining = remaining
            .checked_sub(n)
            .expect("exact boundary must suffice");
        Ok(())
    };
    let mut cursor = records.cursor(&mut charge).unwrap();
    assert!(
        cursor
            .captured(site(2, 3), &original, &mut charge)
            .unwrap()
            .is_some()
    );
    assert_eq!(remaining, 0);
}

use super::super::super::math_capture_flow_v1::Routes;
use super::*;

fn budget(limit: usize) -> Budget {
    Budget {
        remaining: limit,
        limit,
        profile: FlowWorkProfile::default(),
    }
}

fn carrier_fixture(fields: &[u32]) -> (SemanticFunctionDeclV1, Vec<SemanticTypeDeclV1>) {
    let mut types = types();
    let offsets = (0..fields.len())
        .map(|index| index as u64 * 8)
        .collect::<Vec<_>>();
    types.push(aggregate(
        10,
        fields,
        &offsets,
        fields.len() as u64 * 8,
        8,
        false,
    ));
    types.push(aggregate(
        11,
        &[10],
        &[0],
        fields.len() as u64 * 8,
        8,
        false,
    ));
    // Private route-shape controls, not an admitted source or issuer proof.
    let original = fixture(Mutation::None);
    let mut locals = original.locals().to_vec();
    for index in [10, 11] {
        locals.push(local(
            locals.len().try_into().unwrap(),
            index,
            SemanticLocalRoleV1::Temporary,
        ));
    }
    let body = function(
        180,
        original.abi().clone(),
        locals,
        original.blocks().to_vec(),
    );
    (body, types)
}

#[test]
fn only_exact_matrix_access_subgroup_seeds_carriers_not_epoch() {
    for (fields, expected) in [
        (&[4, 1][..], Some(ty(3))),
        (&[1, 4][..], Some(ty(3))),
        (&[5][..], None),
    ] {
        let (body, types) = carrier_fixture(fields);
        for (valid, width, shared) in [
            (true, 64, true),
            (false, 64, true),
            (true, 32, true),
            (true, 64, false),
        ] {
            let callable = access(valid, width, shared);
            let fact = MatrixAccessBorrow::for_callable(&types, &callable);
            let authenticated = valid && width == 64 && shared;
            assert_eq!(fact.is_some(), authenticated);
            if let Some(fact) = fact {
                assert_eq!(fact.pairs(), [(ty(4), ty(3)), (ty(5), ty(0))]);
            }
            let matrix = MatrixBorrowSitesV1::default();
            let routes = Routes::new_with_matrix_and_leaves(
                &body,
                Some(&types),
                &[],
                &matrix,
                fact.into_iter().map(|fact| fact.pairs()[0]),
                &mut budget(MAX_FLOW_WORK),
            )
            .unwrap();
            for index in [10, 11] {
                assert_eq!(
                    routes.owned(ty(index)).copied(),
                    expected.filter(|_| authenticated),
                    "fields={fields:?}, valid={valid}, width={width}, shared={shared}"
                );
            }
            assert!(matrix.pairs.is_empty());
            assert!(matrix.carrier_leaves().is_empty());
            assert!(matrix.policy_carrier_leaves().is_empty());
            assert!(matrix.carrier_barriers().is_empty());
        }
    }
}

#[test]
fn subgroup_leaf_does_not_hide_a_second_primary_in_either_order_or_nested() {
    for fields in [&[4, 8][..], &[8, 4], &[4, 4]] {
        let (body, types) = carrier_fixture(fields);
        let callable = access(true, 64, true);
        let subgroup = MatrixAccessBorrow::for_callable(&types, &callable)
            .unwrap()
            .pairs()[0];
        // The additional Matrix leaf is a private matcher control only.
        let routes = Routes::new_with_matrix_and_leaves(
            &body,
            Some(&types),
            &[],
            &MatrixBorrowSitesV1::default(),
            [subgroup, (ty(8), ty(6))],
            &mut budget(MAX_FLOW_WORK),
        )
        .unwrap();
        assert_eq!(routes.owned(ty(4)), Some(&ty(3)));
        assert_eq!(routes.owned(ty(8)), Some(&ty(6)));
        for index in [10, 11] {
            assert_eq!(routes.owned(ty(index)), None, "fields={fields:?}");
        }
    }
}

#[test]
fn every_added_leaf_is_charged_before_registration_including_duplicates() {
    let (body, types) = carrier_fixture(&[4, 1]);
    let callable = access(true, 64, true);
    let pair = MatrixAccessBorrow::for_callable(&types, &callable)
        .unwrap()
        .pairs()[0];
    let matrix = MatrixBorrowSitesV1::default();
    // Independent prefix ledger: empty ordered map costs five; one-entry lookup
    // costs six even if the next entry duplicates the already registered pair.
    for (pairs, before, requested) in [(vec![pair], 0, 5), (vec![pair, pair], 5, 6)] {
        for remaining in 0..requested {
            let limit = before + remaining;
            let result = Routes::new_with_matrix_and_leaves(
                &body,
                Some(&types),
                &[],
                &matrix,
                pairs.iter().copied(),
                &mut budget(limit),
            );
            let Err(ProductionSemanticSsaErrorV1::BorrowFlowWork {
                remaining_work_units,
                requested_work_units,
                phase_work_units,
                error,
                ..
            }) = result
            else {
                panic!("short registration budget accepted")
            };
            assert_eq!(
                (remaining_work_units, requested_work_units),
                (remaining, requested)
            );
            assert_eq!(phase_work_units.iter().sum::<usize>(), before);
            assert_eq!(
                *error,
                ProductionSemanticSsaErrorV1::AggregateResourceLimit {
                    resource: SsaPlannerResourceV1::WorkUnits,
                    required: limit + 1,
                    limit
                }
            );
        }
        let mut full = budget(MAX_FLOW_WORK);
        Routes::new_with_matrix_and_leaves(
            &body,
            Some(&types),
            &[],
            &matrix,
            pairs.iter().copied(),
            &mut full,
        )
        .unwrap();
        let required = MAX_FLOW_WORK - full.remaining;
        let mut exact = budget(required);
        let routes = Routes::new_with_matrix_and_leaves(
            &body,
            Some(&types),
            &[],
            &matrix,
            pairs.iter().copied(),
            &mut exact,
        )
        .unwrap();
        assert_eq!(exact.remaining, 0);
        assert_eq!(routes.owned(ty(10)), Some(&ty(3)));
        assert!(
            Routes::new_with_matrix_and_leaves(
                &body,
                Some(&types),
                &[],
                &matrix,
                pairs,
                &mut budget(required - 1)
            )
            .is_err()
        );
    }
    assert!(matches!(
        Routes::new_with_matrix_and_leaves(
            &body,
            Some(&types),
            &[],
            &matrix,
            [pair, (pair.0, ty(6))],
            &mut budget(MAX_FLOW_WORK)
        ),
        Err(ProductionSemanticSsaErrorV1::ReplayMismatch)
    ));
}

//! Inert count inputs only; no rows enter a checked owner or Runtime.
use super::*;
use fe2o3_mir_model::SsaVariableIdV1;

fn row(action: PhaseEmissionActionV1) -> PhaseEmissionRowV1 {
    let variable = SsaVariableIdV1::new(0);
    PhaseEmissionRowV1 {
        phase: 0,
        action,
        boundary: PhaseEmissionBoundaryV1 {
            site: Site::new(SemanticBlockIdV1::from_index(0), Some(0)),
            event: 0,
            variable,
            value: SsaValueV1::BlockArgument {
                block: SsaBlockIdV1::new(0),
                variable,
            },
            kind: PhaseBoundaryKindV1::Define,
        },
    }
}

fn assert_work_error(result: PhaseResult<usize>) {
    match result.unwrap_err() {
        ProductionSemanticKirErrorV1::Unsupported { detail, .. } => {
            assert_eq!(detail, "phase emission exhausted existing shared work");
        }
        error => panic!("unexpected constructor census error: {error:?}"),
    }
}

#[test]
fn constructor_census_preserves_every_action_filter_and_charges_all_rows() {
    use PhaseEmissionActionV1 as Action;
    let cases = [
        (Action::OwnerConvert, 1),
        (Action::Begin, 1),
        (Action::Bind { lease: 0 }, 0),
        (Action::Seal, 0),
        (Action::RelayClosure, 0),
        (Action::RelayDrop, 0),
        (Action::CloseStorage { lease: 0 }, 0),
        (Action::End, 0),
    ];
    for (action, expected) in cases {
        let mut work = 1;
        assert_eq!(
            constructor_count(&[row(action)], &mut work).unwrap(),
            expected
        );
        assert_eq!(work, 0);
    }
    let rows: Vec<_> = cases.into_iter().map(|(action, _)| row(action)).collect();
    let mut work = rows.len();
    assert_eq!(constructor_count(&rows, &mut work).unwrap(), 2);
    assert_eq!(work, 0);
}

#[test]
fn constructor_census_exact_and_one_less_have_no_post_hoc_or_duplicate_debit() {
    use PhaseEmissionActionV1 as Action;
    let rows = [
        row(Action::OwnerConvert),
        row(Action::Seal),
        row(Action::Begin),
        row(Action::CloseStorage { lease: 0 }),
        row(Action::End),
    ];
    let mut one_less = rows.len() - 1;
    assert_work_error(constructor_count(&rows, &mut one_less));
    // The single preflight debit fails before the scan and retains the balance.
    assert_eq!(one_less, rows.len() - 1);
    let mut exact = rows.len();
    assert_eq!(constructor_count(&rows, &mut exact).unwrap(), 2);
    assert_eq!(exact, 0);
}

#[test]
fn empty_and_unselected_rows_keep_exact_cumulative_census_cost() {
    let mut zero = 0;
    assert_eq!(constructor_count(&[], &mut zero).unwrap(), 0);
    assert_eq!(zero, 0);
    let rows = [row(PhaseEmissionActionV1::End); 7];
    let mut shared = 14;
    for remaining in [7, 0] {
        assert_eq!(constructor_count(&rows, &mut shared).unwrap(), 0);
        assert_eq!(shared, remaining);
    }
    assert_work_error(constructor_count(&rows, &mut shared));
    assert_eq!(shared, 0);
}

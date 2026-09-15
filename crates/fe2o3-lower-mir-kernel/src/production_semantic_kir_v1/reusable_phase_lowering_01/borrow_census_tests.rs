//! Count-reader tests exercise the production census, not source proof. Lengths
//! are observed only after a debit, without fabricating closed instance IDs.
use super::*;

const WORK: &str = "phase emission exhausted existing shared work";
const OVERFLOW: &str = "phase original borrow count overflow";

fn assert_reason(result: PhaseResult<usize>, expected: &str) {
    match result.unwrap_err() {
        ProductionSemanticKirErrorV1::Unsupported { detail, .. } => assert_eq!(detail, expected),
        error => panic!("unexpected borrow census error: {error:?}"),
    }
}

#[test]
fn borrow_census_matches_old_count_and_exact_per_phase_work() {
    for counts in [vec![], vec![0], vec![3], vec![0, 3, 17, 129]] {
        let expected = counts
            .iter()
            .try_fold(0usize, |sum, count| sum.checked_add(1)?.checked_add(*count))
            .unwrap();
        let mut work = counts.len();
        let mut visits = Vec::new();
        let actual = original_borrow_count(counts.len(), &mut work, |index| {
            visits.push(index);
            counts[index]
        })
        .unwrap();
        assert_eq!(actual, expected);
        assert_eq!(work, 0);
        assert_eq!(visits, (0..counts.len()).collect::<Vec<_>>());
    }
}

#[test]
fn one_less_borrow_census_stops_before_reading_the_unfunded_phase() {
    let counts = [2, 0, 7, 3];
    for available in 0..counts.len() {
        let mut work = available;
        let mut visits = Vec::new();
        assert_reason(
            original_borrow_count(counts.len(), &mut work, |index| {
                assert!(index < available, "unfunded phase length was read");
                visits.push(index);
                counts[index]
            }),
            WORK,
        );
        assert_eq!(work, 0);
        assert_eq!(visits, (0..available).collect::<Vec<_>>());
    }
}

#[test]
fn borrow_count_overflow_retains_checked_add_order_and_exact_rejection() {
    let mut exact = 1;
    assert_eq!(
        original_borrow_count(1, &mut exact, |_| usize::MAX - 1).unwrap(),
        usize::MAX
    );
    assert_eq!(exact, 0);
    for (counts, expected_visits) in [
        (vec![usize::MAX], vec![0]),
        (vec![usize::MAX - 1, 0], vec![0]),
        (vec![usize::MAX - 2, 1], vec![0, 1]),
    ] {
        let mut work = counts.len();
        let mut visits = Vec::new();
        assert_reason(
            original_borrow_count(counts.len(), &mut work, |index| {
                visits.push(index);
                counts[index]
            }),
            OVERFLOW,
        );
        assert_eq!(work, 0);
        // Owner-count overflow still short-circuits before reading lease len.
        assert_eq!(visits, expected_visits);
    }
}

#[test]
fn missing_visit_debit_precedes_overflow_and_later_reads() {
    let mut work = 0;
    assert_reason(
        original_borrow_count(1, &mut work, |_| panic!("unfunded length read")),
        WORK,
    );
    let mut work = 1;
    assert_reason(
        original_borrow_count(1, &mut work, |_| usize::MAX),
        OVERFLOW,
    );
    assert_eq!(work, 0);
    let mut work = 1;
    let mut visits = Vec::new();
    assert_reason(
        original_borrow_count(2, &mut work, |index| {
            visits.push(index);
            assert_eq!(index, 0);
            usize::MAX - 1
        }),
        WORK,
    );
    assert_eq!(visits, [0]);
    assert_eq!(work, 0);
}

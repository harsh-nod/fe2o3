//! Pure budget controls; actual HIR cases live in the public prefix ladder.
use super::*;

#[test]
fn source_bitselect_prefix_count_is_closed_before_any_prefix_work() {
    for count in [0, 1, 8] {
        let mut meter = ScanMeter::default();
        prefix_count(count, &mut meter).unwrap();
        assert_eq!(meter.work, count);
        assert_eq!(
            meter.charged_storage,
            std::mem::size_of::<[Option<rustc_span::Symbol>; MAX_PREFIX_BINDINGS]>()
        );
    }
    let mut meter = ScanMeter::default();
    assert_eq!(
        prefix_count(9, &mut meter).unwrap_err(),
        "source-boundary prefix binding limit"
    );
    assert_eq!(meter.work, 0);
    assert_eq!(meter.charged_storage, 0);
}

#[test]
fn source_bitselect_prefix_count_preserves_the_existing_cumulative_budget() {
    let mut meter = ScanMeter::default();
    meter.scan(WORK_CAP).unwrap();
    assert_eq!(
        prefix_count(1, &mut meter).unwrap_err(),
        "source-boundary work limit"
    );
    assert_eq!(meter.charged_storage, 0);
    let mut meter = ScanMeter::default();
    meter.storage(STORAGE_CAP).unwrap();
    assert_eq!(
        prefix_count(0, &mut meter).unwrap_err(),
        "source-boundary storage limit"
    );
}

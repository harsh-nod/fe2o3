#![cfg(test)]
use super::*;

pub(super) fn successful() -> (Case, Vec<(u64, u16)>, Vec<Row>) {
    let case = Case {
        producer_source: "/original/source".into(),
        devices: vec![7, 9, 11],
    };
    let observed = vec![(7, 128), (9, 129), (11, 130)];
    let rows = (0..3)
        .map(|index| Row {
            source: observed[index],
            destination: observed[(index + 1) % 3],
            nodes: nodes(index),
            compute: Compute::Succeeded,
            staging: Transfer::Succeeded,
            copy: Transfer::Succeeded,
            checked_values: case.elements(index),
            checked_copy_bytes: case.elements(index) * 4,
            staged_version_committed: true,
            copy_input_available: true,
            copy_version_committed: true,
        })
        .collect();
    (case, observed, rows)
}

#[test]
fn staged_ring_requires_its_explicit_closed_selector() {
    let args = [
        "--native-v5-staged-ring",
        "--producer-source",
        "/original/source",
        "--devices",
        "0x0000000000000007",
        "0x0000000000000009",
    ]
    .map(OsString::from);
    assert_eq!(parse(args.clone()).unwrap().devices, [7, 9]);
    let mut scalar = args;
    scalar[0] = "--native-v5-shards".into();
    assert!(parse(scalar).is_err());
}

#[test]
fn exact_ring_has_one_original_source_and_next_destination_per_admitted_device() {
    let (case, observed, mut rows) = successful();
    assert!(report(&case, &observed, &rows, false).is_ok());
    rows[0].destination = observed[2];
    assert!(report(&case, &observed, &rows, false).is_err());
    rows[0].destination = observed[1];
    rows[1].nodes = nodes(0);
    assert!(report(&case, &observed, &rows, false).is_err());
}

#[test]
fn ring_report_rejects_subset_duplicate_or_foreign_original_rosters() {
    let (case, observed, rows) = successful();
    assert!(report(&case, &observed, &rows[..2], false).is_err());
    let mut duplicate = rows.clone();
    duplicate[1] = duplicate[0].clone();
    assert!(report(&case, &observed, &duplicate, false).is_err());
    let mut foreign = observed.clone();
    foreign[1].0 = 13;
    assert!(report(&case, &foreign, &rows, false).is_err());
}

#[test]
fn failed_original_producer_preserves_both_other_compute_and_transfer_rows() {
    let (case, observed, mut rows) = successful();
    rows[0].compute = Compute::ReadbackFailed;
    rows[0].staging = Transfer::DependencyFailed;
    rows[0].copy = Transfer::DependencyFailed;
    rows[0].checked_values = 0;
    rows[0].checked_copy_bytes = 0;
    rows[0].staged_version_committed = false;
    rows[0].copy_input_available = false;
    rows[0].copy_version_committed = false;
    let fields = report(&case, &observed, &rows, true).unwrap();
    assert!(fields.contains("\"successful_transfers\":2"));
    assert!(fields.contains("\"failed_transfers\":1"));
    assert!(report(&case, &observed, &rows, false).is_err());
    rows[0].copy = Transfer::Succeeded;
    assert!(report(&case, &observed, &rows, true).is_err());
}

#[test]
fn definite_staging_refusal_has_no_successor_copy_or_committed_version() {
    let (case, observed, mut rows) = successful();
    rows[1].staging = Transfer::SettledFailure;
    rows[1].copy = Transfer::DependencyFailed;
    rows[1].checked_copy_bytes = 0;
    rows[1].staged_version_committed = false;
    rows[1].copy_input_available = false;
    rows[1].copy_version_committed = false;
    assert!(report(&case, &observed, &rows, false).is_ok());
    rows[1].staged_version_committed = true;
    assert!(report(&case, &observed, &rows, false).is_err());
}

#[test]
fn settled_copy_failure_is_not_success_but_retains_its_actual_producer_input() {
    let (case, observed, mut rows) = successful();
    rows[2].copy = Transfer::SettledFailure;
    rows[2].checked_copy_bytes = 0;
    rows[2].copy_version_committed = false;
    assert!(report(&case, &observed, &rows, false).is_ok());
    rows[2].copy_input_available = false;
    assert!(report(&case, &observed, &rows, false).is_err());
}

#[test]
fn copied_bytes_are_exact_little_endian_values_not_hash_or_route_equality() {
    let (case, _, _) = successful();
    let mut bytes: Vec<_> = (0..case.elements(1) as u32)
        .flat_map(u32::to_le_bytes)
        .collect();
    check_bytes(&case, 1, &bytes).unwrap();
    assert!(check_bytes(&case, 0, &bytes).is_err());
    bytes[4] ^= 1;
    assert!(check_bytes(&case, 1, &bytes).is_err());
}

#[test]
fn report_rejects_unchecked_values_bytes_and_uncommitted_success() {
    let (case, observed, rows) = successful();
    for change in 0..4 {
        let mut changed = rows.clone();
        match change {
            0 => changed[0].checked_values -= 1,
            1 => changed[0].checked_copy_bytes -= 1,
            2 => changed[0].copy_version_committed = false,
            _ => changed[0].staged_version_committed = false,
        }
        assert!(report(&case, &observed, &changed, false).is_err());
    }
}

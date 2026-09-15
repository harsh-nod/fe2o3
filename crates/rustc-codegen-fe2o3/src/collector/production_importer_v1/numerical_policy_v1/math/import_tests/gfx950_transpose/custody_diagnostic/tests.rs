use super::*;
use std::fmt::Write;

#[test]
fn transpose_custody_runtime_checks_are_distinct_and_not_evaluated() {
    for (check, name) in [
        (raw::RuntimeChecks::UbChecks, "UbChecks"),
        (raw::RuntimeChecks::ContractChecks, "ContractChecks"),
        (raw::RuntimeChecks::OverflowChecks, "OverflowChecks"),
    ] {
        let mut out = Output::new();
        dump_operand("arg", &raw::Operand::RuntimeChecks(check), &mut out).unwrap();
        assert_eq!(
            out.finish(Ok(())),
            format!(" raw.arg.kind=RuntimeChecks check={name}; session value not evaluated\n")
        );
    }
}

#[test]
fn transpose_custody_identity_join_uses_identity_not_roster_position() {
    let mut out = Output::new();
    assert_eq!(unique_index(&7u8, [9, 2, 7, 4], &mut out), Ok(2));
    assert_eq!(unique_index(&7u8, [7, 4, 9, 2], &mut out), Ok(0));
    assert_eq!(
        unique_index(&7u8, [9, 2], &mut out),
        Err("source identity has no retained producer")
    );
    assert_eq!(
        unique_index(&7u8, [7, 2, 7], &mut out),
        Err("source identity matches duplicate retained producers")
    );
}

#[test]
fn transpose_custody_parent_selection_includes_relays_without_sibling_guessing() {
    let parents = [None, Some(0), Some(1), Some(1), Some(3)];
    let mut selected = BTreeSet::new();
    let mut out = Output::new();
    retain_ancestors(4, parents.len(), |i| parents[i], &mut selected, &mut out).unwrap();
    assert_eq!(selected, BTreeSet::from([0, 1, 3, 4]));
    retain_ancestors(2, parents.len(), |i| parents[i], &mut selected, &mut out).unwrap();
    assert_eq!(selected, BTreeSet::from([0, 1, 2, 3, 4]));
}

#[test]
fn transpose_custody_parent_selection_rejects_invalid_and_excessive_paths() {
    let mut out = Output::new();
    assert_eq!(
        retain_ancestors(2, 2, |_| None, &mut BTreeSet::new(), &mut out),
        Err("call instance outside retained roster")
    );
    assert_eq!(
        retain_ancestors(1, 2, |_| Some(1), &mut BTreeSet::new(), &mut out),
        Err("retained call parent is not an earlier instance")
    );
    let mut selected = BTreeSet::new();
    assert_eq!(
        retain_ancestors(
            MAX_INSTANCES,
            MAX_INSTANCES + 1,
            |i| i.checked_sub(1),
            &mut selected,
            &mut out
        ),
        Err("selected call-instance bound reached")
    );
    assert_eq!(selected.len(), MAX_INSTANCES);
}

#[test]
fn transpose_custody_output_preserves_utf8_and_reserves_incomplete_marker() {
    let mut out = Output::new();
    while out.write_str("abcd").is_ok() {}
    let length = out.text.len();
    assert!(out.write_str("abcd").is_err());
    assert_eq!(out.text.len(), length);
    let text = out.finish(Err("diagnostic byte bound reached"));
    assert!(text.len() <= bounded::MAX_BYTES);
    assert!(text.contains("TRANSPOSE_CUSTODY_INCOMPLETE"));
    assert!(text.contains("not a complete source/return observation"));
    let mut out = Output::new();
    out.write_str("\u{e9}").unwrap();
    assert_eq!(out.finish(Ok(())), "\u{e9}");
}

#[test]
fn transpose_custody_work_exhaustion_stops_without_reset_or_underflow() {
    let mut out = Output::new();
    out.charge(bounded::MAX_ITEMS).unwrap();
    assert_eq!(out.charge(1), Err("diagnostic item bound reached"));
    assert_eq!(out.remaining, 0);
    assert_eq!(
        unique_index(&7, [7], &mut out),
        Err("diagnostic item bound reached")
    );
    assert_eq!(out.remaining, 0);
}

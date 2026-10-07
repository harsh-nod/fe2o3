#[test]
fn settlement_routes_bounded_preflight_before_plan_and_commit() {
    let source = include_str!("../settlement.rs");
    let wrappers = include_str!("../settlement_wrapper_bodies.rs");
    for forbidden in [
        "try_reserve",
        ".reserve(",
        ".reserve_exact(",
        ".resize",
        ".extend(",
        ".collect(",
        "Vec::",
        "Box::",
        "vec!",
        "to_vec",
        "to_owned",
        ".clone(",
        "while ",
        "loop {",
        ".iter(",
        ".iter_mut(",
        ".into_iter(",
        "0..self.allocation_capacity",
        "0..self.writer_capacity",
        "self.registration_watermark =",
        "self.reserved_count =",
        "$journal.registration_watermark =",
        "$journal.reserved_count =",
    ] {
        assert!(
            !source.contains(forbidden) && !wrappers.contains(forbidden),
            "settlement contains {forbidden}"
        );
    }
    assert_eq!(source.matches("for ").count(), 0);
    assert_eq!(source.matches(".push(").count(), 0);
    assert_eq!(wrappers.matches("for ").count(), 0);
    assert_eq!(wrappers.matches(".push(").count(), 0);
    assert!(
        source.contains(
            "settlement_outcome_body!(self, settle_retained, writer, evidence, true, [])"
        )
    );
    assert!(
        source.contains(
            "settlement_outcome_body!(self, settle_retained, writer, evidence, false, [])"
        )
    );

    let release = wrappers
        .split("macro_rules! settlement_execute_body")
        .nth(1)
        .unwrap();
    let plan = release.find("$stage($journal, $head, $count)").unwrap();
    assert!(
        release
            .find("$journal.$preflight($writer, $evidence $($observations)*)")
            .unwrap()
            < plan
    );
    assert!(
        plan < release
            .find("$commit($journal, $writer, $count, $success $($commit_extra)*)")
            .unwrap()
    );
    let preflight = source
        .split("fn preflight_settlement(")
        .nth(1)
        .unwrap()
        .split("fn settle_retained(")
        .next()
        .unwrap();
    assert!(preflight.contains("&self,"));
    assert!(!preflight.contains("&mut self"));
    for observation in ["self.free.capacity()", "self.member_free.capacity()"] {
        assert_eq!(preflight.matches(observation).count(), 1);
    }
    for adapter in [
        "retained::shared_retained_header_v1",
        "retained::shared_retained_writer_key_v1",
        "retained::shared_retained_chain_v1",
        "settlement_scratch::shared_settlement_scratch_scan_v1",
    ] {
        assert_eq!(preflight.matches(adapter).count(), 1);
    }
    let preflight = wrappers
        .split("macro_rules! settlement_execute_body")
        .next()
        .unwrap();
    let mut previous = 0;
    for validation in [
        "$header($journal, $writer, false)",
        "SettlementEvidenceMismatch",
        "$chain($journal, $writer, head, count)",
        "SettlementReturnStorageV1 {",
        "storage.check(count)",
        "$scan($journal, count)",
    ] {
        let position = preflight.find(validation).unwrap();
        assert!(position >= previous, "out-of-order preflight: {validation}");
        previous = position;
    }
    for observation in [
        "writer_free_len: $journal.free.len()",
        "member_free_len: $journal.member_free.len()",
        "writer_limit: $journal.writer_capacity",
        "writer_storage: $writer_capacity",
        "member_limit: $journal.allocation_capacity",
        "member_storage: $member_capacity",
        "scratch_len: $journal.scratch.len()",
    ] {
        assert_eq!(preflight.matches(observation).count(), 1);
    }
    assert_eq!(wrappers.matches("storage.check(count)").count(), 1);
    assert!(source.contains("settlement_execute_body!("));
    assert_eq!(
        source
            .matches("settlement_scratch::shared_settlement_scratch_stage_v1")
            .count(),
        1
    );
    assert_eq!(
        source
            .matches("settlement_commit::shared_settlement_commit_v1")
            .count(),
        1
    );
    let body = include_str!("../settlement_return_body.rs");
    let storage = include_str!("../settlement_storage.rs")
        .split("#[cfg(test)]")
        .next()
        .unwrap();
    assert!(storage.contains("include!(\"settlement_return_body.rs\")"));
    let compact_storage = storage
        .split_whitespace()
        .collect::<alloc::string::String>();
    assert!(compact_storage.contains(
        "settlement_return_admission_body!(self,count,ContextVersionJournalErrorV1::InvalidState)"
    ));
    let mut previous = 0;
    for validation in [
        "$storage.writer_free_len.checked_add(1)",
        "$storage.member_free_len.checked_add($count)",
        "writer_returns > $storage.writer_limit",
        "writer_returns > $storage.writer_storage",
        "member_returns > $storage.member_limit",
        "member_returns > $storage.member_storage",
        "$count > $storage.scratch_len",
    ] {
        let position = body.find(validation).unwrap();
        assert!(
            position >= previous,
            "out-of-order shared admission: {validation}"
        );
        previous = position;
    }
    for forbidden in [
        "Vec",
        "Box",
        "for ",
        "while ",
        "unsafe",
        ".push(",
        ".reserve(",
        "&mut",
    ] {
        assert!(!body.contains(forbidden));
        assert!(!storage.contains(forbidden));
    }
    let unknown = source
        .split("pub fn mark_unknown(")
        .nth(1)
        .unwrap()
        .split("fn retained_header(")
        .next()
        .unwrap();
    assert!(unknown.contains(
        "journal_unknown_wrapper_body!(self, writer, retained::shared_retained_unknown_v1)"
    ));
    let adapter = include_str!("../unknown_wrapper_bodies.rs");
    assert!(adapter.contains("$execute($journal, $writer)"));
    assert!(source.contains("retained::shared_retained_header_v1(self, writer, allow_unknown)"));
    assert!(source.contains("retained::shared_retained_chain_v1(self, writer, head, count)"));
    for forbidden in [
        "self.scratch",
        "self.free",
        "self.member_free",
        "self.store_plan",
    ] {
        assert!(!unknown.contains(forbidden));
    }
}

use super::*;

#[test]
fn artifact_scan_accepts_exact_total_entry_bound() {
    let mut visited = 0_usize;
    let names = collect_envelope_names(|visit| {
        visit(b".")?;
        visit(b"..")?;
        for _ in 0..MAX_APPLICATION_ARTIFACT_DIRECTORY_ENTRIES_V1 {
            visited += 1;
            visit(b"unrelated")?;
        }
        Ok(())
    })
    .unwrap();

    assert_eq!(visited, MAX_APPLICATION_ARTIFACT_DIRECTORY_ENTRIES_V1);
    assert!(names.is_empty());
}

#[test]
fn artifact_scan_rejects_limit_plus_one_unrelated_entry_early() {
    let mut visited = 0_usize;
    let error = collect_envelope_names(|visit| {
        visit(b".")?;
        visit(b"..")?;
        for _ in 0..MAX_APPLICATION_ARTIFACT_DIRECTORY_ENTRIES_V1 + 100 {
            visited += 1;
            visit(b"unrelated")?;
        }
        panic!("scan continued after the first over-limit entry");
    })
    .unwrap_err();

    assert_eq!(visited, MAX_APPLICATION_ARTIFACT_DIRECTORY_ENTRIES_V1 + 1);
    assert_eq!(
        error,
        format!(
            "artifact directory exceeds {MAX_APPLICATION_ARTIFACT_DIRECTORY_ENTRIES_V1} visible entries"
        )
    );
}

#[test]
fn artifact_scan_counts_mixed_entries_and_sorts_canonical_candidates() {
    let first = canonical_retired_worker_v2_envelope_name(b'0');
    let last = canonical_retired_worker_v2_envelope_name(b'f');
    let names = collect_envelope_names(|visit| {
        for _ in 0..MAX_APPLICATION_ARTIFACT_DIRECTORY_ENTRIES_V1 - 3 {
            visit(b"unrelated")?;
        }
        visit(&last)?;
        visit(b"also-unrelated")?;
        visit(&first)
    })
    .unwrap();

    assert_eq!(
        names,
        [
            String::from_utf8(first).unwrap(),
            String::from_utf8(last).unwrap(),
        ]
    );
}

#[test]
fn artifact_scan_finds_v3_envelopes_and_ignores_v3_readiness_siblings() {
    let envelope = canonical_v3_envelope_name(b'a');
    let claim = format!(
        "{}{}.claim",
        std::str::from_utf8(V3_ENVELOPE_PREFIX).unwrap(),
        "b".repeat(64)
    );
    let receipt = format!(
        "{}{}.receipt",
        std::str::from_utf8(V3_ENVELOPE_PREFIX).unwrap(),
        "c".repeat(64)
    );
    let names = collect_envelope_names(|visit| {
        visit(claim.as_bytes())?;
        visit(&envelope)?;
        visit(receipt.as_bytes())
    })
    .unwrap();
    assert_eq!(names, [String::from_utf8(envelope).unwrap()]);
    reject_retired_worker_v2_envelopes(&names).unwrap();
}

#[test]
fn application_handoff_rejects_retired_v2_even_beside_v3() {
    let names = [
        String::from_utf8(canonical_retired_worker_v2_envelope_name(b'a')).unwrap(),
        String::from_utf8(canonical_v3_envelope_name(b'b')).unwrap(),
    ];
    assert_eq!(
        reject_retired_worker_v2_envelopes(&names),
        Err(RETIRED_WORKER_V2_ENVELOPE_ERROR.to_string())
    );
}

#[test]
fn sole_schema_policy_rejects_worker_v2_and_accepts_worker_v3() {
    let worker_v2 = [String::from_utf8(canonical_retired_worker_v2_envelope_name(b'a')).unwrap()];
    assert_eq!(
        reject_retired_worker_v2_envelopes(&worker_v2),
        Err(RETIRED_WORKER_V2_ENVELOPE_ERROR.to_string())
    );

    let worker_v3 = [String::from_utf8(canonical_v3_envelope_name(b'b')).unwrap()];
    reject_retired_worker_v2_envelopes(&worker_v3).unwrap();
}

#[test]
fn artifact_scan_preserves_deterministic_duplicate_candidates() {
    let duplicate = canonical_retired_worker_v2_envelope_name(b'a');
    let names = collect_envelope_names(|visit| {
        visit(&duplicate)?;
        visit(&duplicate)
    })
    .unwrap();

    let duplicate = String::from_utf8(duplicate).unwrap();
    assert_eq!(names, [duplicate.clone(), duplicate]);
}

#[test]
fn artifact_scan_fails_closed_on_entry_error() {
    let error = collect_envelope_names(|visit| {
        visit(b"unrelated")?;
        Err("failed to read artifact entry: injected EIO".to_string())
    })
    .unwrap_err();

    assert_eq!(error, "failed to read artifact entry: injected EIO");
}

#[test]
fn artifact_scan_does_not_retain_huge_unrelated_names() {
    let hostile = vec![b'x'; 1024 * 1024];
    let names = collect_envelope_names(|visit| visit(&hostile)).unwrap();
    assert!(names.is_empty());
}

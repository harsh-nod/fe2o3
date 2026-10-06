//! Independent finite representation/boundary controls, not live-rustc evidence.
use super::*;
use fe2o3_mir_model::semantic_mir_v1::{SemanticSourceFileIdentityV1, SemanticSourceProvenanceV1};
fn source(file: u8) -> SemanticSourceOriginV1 {
    SemanticSourceOriginV1::new(
        SemanticSourceFileIdentityV1::from_sha256([file; 32]),
        1,
        3,
        2,
        4,
        2,
        6,
    )
    .unwrap()
}
fn frame(ordinal: usize) -> Frame {
    Frame {
        ordinal,
        kind: "macro",
        macro_name: None,
        expansion_identity: [9; 32],
        expansion: Origin::known(source(1)),
        call_site: Origin::known(source(2)),
        definition_site: Origin::UnavailableDummyDefinition,
    }
}
#[test]
fn fixed_frame_and_visited_storage_fits_original_extra_prepay() {
    assert!(
        std::mem::size_of::<OrderedMacroFramesV1>() + std::mem::size_of::<Visited>()
            <= MAX_MACRO_REPORT_BYTES_V1
    );
    assert_eq!(MACRO_LOGICAL_PREPAY_V1, 144 * 1024);
}
#[test]
fn exact_32_frames_accept_and_33_refuses_without_replacing_last() {
    let mut rows = Frames::new();
    for n in 0..32 {
        rows.push(frame(n)).unwrap();
    }
    assert_eq!(rows.push(frame(32)), Err("macro frame depth bound"));
    assert_eq!(rows.len, 32);
    assert_eq!(rows.rows[31].unwrap().ordinal, 31);
}
#[test]
fn frame_order_is_not_silently_repaired() {
    let mut rows = Frames::new();
    assert_eq!(rows.push(frame(1)), Err("macro frame order mismatch"));
    assert_eq!(rows.len, 0);
    rows.push(frame(0)).unwrap();
    assert_eq!(rows.push(frame(0)), Err("macro frame order mismatch"));
}
#[test]
fn name_utf8_is_preserved_and_exact_byte_bound_is_inclusive() {
    let mut work = Work::new(4096);
    let name = Name::new("é", &mut work).unwrap();
    assert_eq!(encode(&name).unwrap(), "\"é\"".as_bytes());
    assert!(Name::new(&"x".repeat(256), &mut work).is_ok());
    assert!(Name::new(&"x".repeat(257), &mut work).is_err());
    assert!(Name::new("", &mut work).is_err());
}
#[test]
fn name_work_refuses_before_copy_and_keeps_previous_usage() {
    let mut work = Work::new(2);
    Name::new("é", &mut work).unwrap();
    assert!(Name::new("x", &mut work).is_err());
    assert_eq!(work.used, 2);
}
#[test]
fn repeated_span_is_a_cycle_not_an_additional_frame() {
    let mut visited = Visited::new();
    let mut work = Work::new(100);
    let span = Span::with_root_ctxt(rustc_span::BytePos(1), rustc_span::BytePos(3));
    visited.insert(span, &mut work).unwrap();
    assert_eq!(visited.insert(span, &mut work), Err("macro frame cycle"));
    assert_eq!(visited.len, 1);
    assert_eq!(work.used, 3);
}
#[test]
fn visited_depth_inclusive_and_prepaid_search_refuses() {
    let mut visited = Visited::new();
    let mut work = Work::new(10_000);
    for n in 0..33 {
        visited
            .insert(
                Span::with_root_ctxt(
                    rustc_span::BytePos(2 * n + 1),
                    rustc_span::BytePos(2 * n + 2),
                ),
                &mut work,
            )
            .unwrap();
    }
    assert_eq!(
        visited.insert(rustc_span::DUMMY_SP, &mut work),
        Err("macro frame depth bound")
    );
    let mut visited = Visited::new();
    let mut tiny = Work::new(0);
    assert_eq!(
        visited.insert(rustc_span::DUMMY_SP, &mut tiny),
        Err("origin work bound exceeded")
    );
    assert_eq!(visited.len, 0);
}
#[test]
fn stale_chain_depth_and_provenance_all_refuse() {
    let p = SemanticSourceProvenanceV1::new(Some(source(1)), Some(source(2)));
    let q = SemanticSourceProvenanceV1::new(Some(source(2)), Some(source(1)));
    let expected = (p, [3; 32], 2);
    assert!(check_binding(expected, expected, 2).is_ok());
    assert!(check_binding(expected, (p, [4; 32], 2), 2).is_err());
    assert!(check_binding(expected, (p, [3; 32], 1), 2).is_err());
    assert!(check_binding(expected, (q, [3; 32], 2), 2).is_err());
    assert!(check_binding(expected, expected, 1).is_err());
}
#[test]
fn empty_chain_has_no_fabricated_root_or_inline_frame() {
    assert_eq!(encode(&Frames::new()).unwrap(), b"[]");
    let p = SemanticSourceProvenanceV1::new(Some(source(1)), Some(source(1)));
    assert!(check_binding((p, [3; 32], 0), (p, [3; 32], 0), 0).is_ok());
}
#[test]
fn dummy_definition_is_explicit_and_does_not_reuse_callsite() {
    let row: serde_json::Value = serde_json::from_slice(&encode(&frame(0)).unwrap()).unwrap();
    assert_eq!(
        row["definition_site"]["availability"],
        "unavailable_dummy_definition"
    );
    assert_eq!(row["call_site"]["availability"], "available");
    assert_ne!(row["expansion"], row["call_site"]);
}
#[test]
fn report_byte_bound_refuses_before_vector_extension() {
    let mut out = Limited(Vec::new());
    out.write_all(&vec![0; MAX_MACRO_REPORT_BYTES_V1]).unwrap();
    assert!(out.write_all(&[1]).is_err());
    assert_eq!(out.0.len(), MAX_MACRO_REPORT_BYTES_V1);
    assert!(encode(&"x".repeat(MAX_MACRO_REPORT_BYTES_V1)).is_err());
}
#[test]
fn serializer_rejects_missing_frame_instead_of_serializing_null() {
    let mut rows = Frames::new();
    rows.len = 1;
    assert!(encode(&rows).is_err());
}

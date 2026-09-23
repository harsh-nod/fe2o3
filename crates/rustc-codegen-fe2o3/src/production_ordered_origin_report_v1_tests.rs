//! DTO serialization and strict bounded-output unit tests, not source qualification.
use super::*;
use fe2o3_mir_model::semantic_mir_v1::SemanticSourceFileIdentityV1;
#[test]
fn identities_are_exact_lowercase_hex_not_truncated_display_ids() {
    assert_eq!(hex(&[0xab; 32]), "ab".repeat(32));
    assert_eq!(hex(&[0; 32]), "00".repeat(32));
}
#[test]
fn origin_preserves_file_byte_line_and_column_coordinates() {
    let source = SemanticSourceOriginV1::new(
        SemanticSourceFileIdentityV1::from_sha256([7; 32]),
        19,
        30,
        2,
        4,
        3,
        8,
    )
    .unwrap();
    let value = serde_json::to_value(Origin::new(source)).unwrap();
    assert_eq!(value["file_identity"], "07".repeat(32));
    assert_eq!(value["byte_start"], 19);
    assert_eq!(value["byte_end"], 30);
    assert_eq!(value["line_start"], 2);
    assert_eq!(value["column_start"], 4);
    assert_eq!(value["line_end"], 3);
    assert_eq!(value["column_end"], 8);
}
#[test]
fn whole_region_instruction_association_does_not_claim_fine_step_origin() {
    let value = serde_json::to_value(Instruction {
        ordinal: 15,
        descriptor: 133,
        source_association: "whole_ordered_region_only",
    })
    .unwrap();
    assert_eq!(value["ordinal"], 15);
    assert_eq!(value["source_association"], "whole_ordered_region_only");
    assert!(value.get("span").is_none());
    assert!(value.get("repeat_iteration").is_none());
}
#[test]
fn writer_accepts_exact_cap_and_refuses_next_byte_without_retaining_it() {
    let mut output = LimitedBytes(Vec::new());
    output
        .write_all(&vec![1; MAX_ORIGIN_REPORT_BYTES_V1])
        .unwrap();
    assert!(output.write_all(&[2]).is_err());
    assert_eq!(output.0.len(), MAX_ORIGIN_REPORT_BYTES_V1);
    assert_eq!(output.0.last(), Some(&1));
}
#[test]
fn escaped_json_is_bounded_as_encoded_bytes() {
    let text = "\u{0001}".repeat(MAX_ORIGIN_REPORT_BYTES_V1 / 2);
    assert!(encode_bounded(&text).is_err());
    let small = encode_bounded(&["actual-region", "fine-step-unavailable"]).unwrap();
    assert_eq!(
        serde_json::from_slice::<Vec<String>>(&small).unwrap(),
        ["actual-region", "fine-step-unavailable"]
    );
}
#[test]
fn oversized_single_chunk_is_not_partially_retained() {
    let mut output = LimitedBytes(vec![1, 2, 3]);
    assert!(output.write(&vec![4; MAX_ORIGIN_REPORT_BYTES_V1]).is_err());
    assert_eq!(output.0, [1, 2, 3]);
}

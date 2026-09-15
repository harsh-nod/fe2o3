//! Small wire boundary tests, not live source or lifecycle evidence.
use super::*;

#[path = "common_v25_tests/owner.rs"]
mod owner;

#[test]
fn common_v25_empty_footer_never_changes_historical_bytes() {
    for raw in 2..=24 {
        let version = SemanticMirWireVersionV1::from_u16(raw).unwrap();
        let mut writer = CanonicalWriterV1::new(2);
        writer.raw(&[0x71, 0x29]).unwrap();
        transpose_owned_flow_v25::encode_footer(&mut writer, version, &[]).unwrap();
        assert_eq!(writer.finish(), [0x71, 0x29], "historical version {raw}");
    }
}

#[test]
fn common_v25_empty_footer_is_exactly_one_bounded_u32_count() {
    let mut writer = CanonicalWriterV1::new(4);
    transpose_owned_flow_v25::encode_footer(&mut writer, SemanticMirWireVersionV1::V25, &[])
        .unwrap();
    assert_eq!(writer.finish(), [0, 0, 0, 0]);
    let mut writer = CanonicalWriterV1::new(3);
    assert!(matches!(
        transpose_owned_flow_v25::encode_footer(&mut writer, SemanticMirWireVersionV1::V25, &[]),
        Err(SemanticMirErrorV1::LimitExceeded {
            resource: SemanticMirResourceV1::CanonicalBytes,
            max: 3,
            actual: 4
        })
    ));
}


#[path = "common_v25_tests/compatibility.rs"]
mod compatibility;

#[path = "common_v25_tests/attachment.rs"]
mod attachment;

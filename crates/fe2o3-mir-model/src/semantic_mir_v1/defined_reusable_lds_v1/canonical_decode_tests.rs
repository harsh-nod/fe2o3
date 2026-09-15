use super::*;
use crate::semantic_mir_v1::defined_reusable_lds_v1::tests::record_fixture;

#[test]
fn reusable_lds_fixed_payload_roundtrips_and_rejects_every_truncation() {
    let record = record_fixture();
    let mut writer = CanonicalWriterV1::new(4096);
    record.encode_payload(&mut writer).unwrap();
    let mut bytes = writer.finish();
    let mut reader = CanonicalDecoderV1::new(&bytes, SemanticMirLimitsV1::default());
    assert_eq!(reader.reusable_lds_conversion_payload().unwrap(), record);
    reader.finish().unwrap();
    for end in 0..bytes.len() {
        let mut reader = CanonicalDecoderV1::new(&bytes[..end], SemanticMirLimitsV1::default());
        assert!(
            reader.reusable_lds_conversion_payload().is_err(),
            "truncated {end}"
        );
    }
    bytes.push(0);
    let mut reader = CanonicalDecoderV1::new(&bytes, SemanticMirLimitsV1::default());
    let _ = reader.reusable_lds_conversion_payload().unwrap();
    assert!(reader.finish().is_err());
}

#[test]
fn reusable_lds_tag_eight_is_v24_only_and_does_not_change_old_tags() {
    let contract = SemanticDefinedCapabilityContractV1::ReusableLdsConversion(record_fixture());
    let mut writer = CanonicalWriterV1::new(4096);
    contract.encode(&mut writer).unwrap();
    let bytes = writer.finish();
    assert_eq!(bytes[0], 8);
    for version in [
        SemanticMirWireVersionV1::V20,
        SemanticMirWireVersionV1::V21,
        SemanticMirWireVersionV1::V22,
        SemanticMirWireVersionV1::V23,
    ] {
        let mut reader = CanonicalDecoderV1::new(&bytes, SemanticMirLimitsV1::default());
        reader.wire_version = version;
        assert!(matches!(
            reader.defined_capability_contract(),
            Err(SemanticMirDecodeErrorV1::InvalidTag {
                context: "defined capability contract",
                value: 8,
                ..
            })
        ));
    }
    let mut reader = CanonicalDecoderV1::new(&bytes, SemanticMirLimitsV1::default());
    reader.wire_version = SemanticMirWireVersionV1::V24;
    assert_eq!(reader.defined_capability_contract().unwrap(), contract);
    reader.finish().unwrap();
    assert_eq!(SemanticMirWireVersionV1::V23.as_u16(), 23);
}

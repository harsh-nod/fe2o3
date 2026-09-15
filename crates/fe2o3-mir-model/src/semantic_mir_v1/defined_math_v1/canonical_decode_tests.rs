use super::*;
use crate::semantic_mir_v1::defined_math_v1::tests::fixture;

fn payloads() -> (Vec<u8>, Vec<u8>) {
    let fixture = fixture();
    let mut derive = CanonicalWriterV1::new(512);
    let mut bind = CanonicalWriterV1::new(380);
    fixture
        .derive()
        .unwrap()
        .encode_payload(&mut derive)
        .unwrap();
    fixture.bind().unwrap().encode_payload(&mut bind).unwrap();
    (derive.finish(), bind.finish())
}

#[test]
fn defined_math_payload_decoder_round_trips_exactly() {
    let (derive, bind) = payloads();
    let mut decoder = CanonicalDecoderV1::new(&derive, SemanticMirLimitsV1::default());
    let record = decoder.kernel_math_derive_payload().unwrap();
    decoder.finish().unwrap();
    assert_eq!(record, fixture().derive().unwrap());
    let mut writer = CanonicalWriterV1::new(512);
    record.encode_payload(&mut writer).unwrap();
    assert_eq!(derive, writer.finish());

    let mut decoder = CanonicalDecoderV1::new(&bind, SemanticMirLimitsV1::default());
    let record = decoder.policy_math_bind_payload().unwrap();
    decoder.finish().unwrap();
    assert_eq!(record, fixture().bind().unwrap());
    let mut writer = CanonicalWriterV1::new(380);
    record.encode_payload(&mut writer).unwrap();
    assert_eq!(bind, writer.finish());
}

#[test]
fn defined_math_payload_decoder_rejects_every_truncation_and_suffix() {
    let (mut derive, mut bind) = payloads();
    for end in 0..derive.len() {
        let mut decoder = CanonicalDecoderV1::new(&derive[..end], SemanticMirLimitsV1::default());
        assert!(
            decoder.kernel_math_derive_payload().is_err(),
            "derive truncation {end}"
        );
    }
    for end in 0..bind.len() {
        let mut decoder = CanonicalDecoderV1::new(&bind[..end], SemanticMirLimitsV1::default());
        assert!(
            decoder.policy_math_bind_payload().is_err(),
            "bind truncation {end}"
        );
    }
    derive.push(0);
    bind.push(0);
    let mut decoder = CanonicalDecoderV1::new(&derive, SemanticMirLimitsV1::default());
    let _ = decoder.kernel_math_derive_payload().unwrap();
    assert!(decoder.finish().is_err());
    let mut decoder = CanonicalDecoderV1::new(&bind, SemanticMirLimitsV1::default());
    let _ = decoder.policy_math_bind_payload().unwrap();
    assert!(decoder.finish().is_err());
}

#[test]
fn defined_math_payload_decoder_rejects_zero_and_aliased_identities() {
    let (derive, bind) = payloads();
    for offset in [
        4, 36, 68, 104, 136, 168, 204, 236, 288, 320, 352, 384, 416, 448, 480,
    ] {
        let mut bytes = derive.clone();
        bytes[offset..offset + 32].fill(0);
        assert!(
            CanonicalDecoderV1::new(&bytes, SemanticMirLimitsV1::default())
                .kernel_math_derive_payload()
                .is_err(),
            "derive zero identity at {offset}"
        );
    }
    for offset in [4, 36, 68, 124, 156, 188, 220, 252, 284, 316, 348] {
        let mut bytes = bind.clone();
        bytes[offset..offset + 32].fill(0);
        assert!(
            CanonicalDecoderV1::new(&bytes, SemanticMirLimitsV1::default())
                .policy_math_bind_payload()
                .is_err(),
            "bind zero identity at {offset}"
        );
    }
    let mut bytes = derive;
    bytes[100..104].copy_from_slice(&0u32.to_le_bytes());
    assert!(
        CanonicalDecoderV1::new(&bytes, SemanticMirLimitsV1::default())
            .kernel_math_derive_payload()
            .is_err()
    );
    let mut bytes = bind;
    bytes[348..380].copy_from_slice(fixture().policy.as_bytes());
    assert!(
        CanonicalDecoderV1::new(&bytes, SemanticMirLimitsV1::default())
            .policy_math_bind_payload()
            .is_err()
    );
}

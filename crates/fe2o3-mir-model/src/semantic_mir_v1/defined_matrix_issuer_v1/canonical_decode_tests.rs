use super::*;
use crate::semantic_mir_v1::defined_matrix_issuer_v1::tests::record_fixture;

fn payload() -> Vec<u8> {
    let mut writer = CanonicalWriterV1::new(520);
    record_fixture().encode_payload(&mut writer).unwrap();
    writer.finish()
}

#[test]
fn exact_issuer_payload_round_trips_without_acquiring_authority() {
    let bytes = payload();
    assert_eq!(bytes.len(), 520);
    let mut decoder = CanonicalDecoderV1::new(&bytes, SemanticMirLimitsV1::default());
    let record = decoder.kernel_matrix_derive_payload().unwrap();
    decoder.finish().unwrap();
    assert_eq!(record, record_fixture());
    let mut writer = CanonicalWriterV1::new(520);
    record.encode_payload(&mut writer).unwrap();
    assert_eq!(writer.finish(), bytes);
}

#[test]
fn issuer_payload_rejects_every_truncation_and_trailing_bytes() {
    let mut bytes = payload();
    for end in 0..bytes.len() {
        let mut decoder = CanonicalDecoderV1::new(&bytes[..end], SemanticMirLimitsV1::default());
        assert!(
            decoder.kernel_matrix_derive_payload().is_err(),
            "truncation {end}"
        );
    }
    bytes.push(0);
    let mut decoder = CanonicalDecoderV1::new(&bytes, SemanticMirLimitsV1::default());
    decoder.kernel_matrix_derive_payload().unwrap();
    assert!(decoder.finish().is_err());
}

#[test]
fn issuer_payload_rejects_zero_identities_and_aliased_types() {
    let original = payload();
    // Two 100-byte body commitments, 68-byte Current binding, six u32 types,
    // then root plus six provenance identities and the invariant kernel brand.
    for offset in [
        4, 36, 68, 104, 136, 168, 204, 236, 296, 328, 360, 392, 424, 456, 488,
    ] {
        let mut bytes = original.clone();
        bytes[offset..offset + 32].fill(0);
        assert!(
            CanonicalDecoderV1::new(&bytes, SemanticMirLimitsV1::default())
                .kernel_matrix_derive_payload()
                .is_err(),
            "zero identity at {offset}",
        );
    }
    for offset in [100, 268, 272, 276, 280, 284, 288] {
        let mut bytes = original.clone();
        // Bridge function becomes getter function; each type becomes another
        // member of the six-type recipe, never an unchecked sentinel.
        let value = if offset == 100 {
            0
        } else if offset == 272 {
            1
        } else {
            0u32
        };
        bytes[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
        assert!(
            CanonicalDecoderV1::new(&bytes, SemanticMirLimitsV1::default())
                .kernel_matrix_derive_payload()
                .is_err(),
            "aliased role at {offset}",
        );
    }
}

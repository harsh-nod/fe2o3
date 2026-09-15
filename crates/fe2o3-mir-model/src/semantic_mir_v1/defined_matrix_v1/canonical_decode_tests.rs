use super::*;
use crate::semantic_mir_v1::defined_matrix_v1::tests::{fixture, phase_fixture};

fn payloads() -> [Vec<u8>; 2] {
    let f = fixture();
    let mut bind = CanonicalWriterV1::new(1024);
    f.bind().unwrap().encode_payload(&mut bind).unwrap();
    let mut narrow = CanonicalWriterV1::new(1024);
    f.narrow().unwrap().encode_payload(&mut narrow).unwrap();
    [bind.finish(), narrow.finish()]
}

fn decode(bytes: &[u8], narrow: bool) -> Result<Vec<u8>, SemanticMirDecodeErrorV1> {
    let mut decoder = CanonicalDecoderV1::new(bytes, SemanticMirLimitsV1::default());
    let mut writer = CanonicalWriterV1::new(1024);
    if narrow {
        decoder
            .policy_gfx950_narrow_payload()?
            .encode_payload(&mut writer)?;
    } else {
        decoder
            .policy_matrix_bind_payload()?
            .encode_payload(&mut writer)?;
    }
    decoder.finish()?;
    Ok(writer.finish())
}

#[test]
fn defined_matrix_payload_exact_roundtrip_and_every_truncation() {
    for (index, bytes) in payloads().into_iter().enumerate() {
        assert_eq!(decode(&bytes, index == 1).unwrap(), bytes);
        for end in 0..bytes.len() {
            assert!(
                decode(&bytes[..end], index == 1).is_err(),
                "payload {index}, prefix {end}"
            );
        }
        let mut suffixed = bytes;
        suffixed.push(0);
        assert!(decode(&suffixed, index == 1).is_err());
    }
}

#[test]
fn defined_matrix_payload_rejects_zero_and_aliased_identity_axes() {
    for (index, bytes) in payloads().into_iter().enumerate() {
        // Four trailing nominal axes are policy, root, subgroup brand and epoch.
        for offset in [
            4,
            36,
            68,
            bytes.len() - 128,
            bytes.len() - 96,
            bytes.len() - 64,
            bytes.len() - 32,
        ] {
            let mut changed = bytes.clone();
            changed[offset..offset + 32].fill(0);
            assert!(
                decode(&changed, index == 1).is_err(),
                "payload {index}, identity {offset}"
            );
        }
        let mut changed = bytes.clone();
        let epoch = changed.len() - 32;
        changed[epoch..].copy_from_slice(&bytes[epoch - 32..epoch]);
        assert!(decode(&changed, index == 1).is_err());
        if index == 1 {
            let mut changed = bytes;
            changed[100..104].copy_from_slice(&0u32.to_le_bytes());
            assert!(
                decode(&changed, true).is_err(),
                "projection cannot be its caller"
            );
        }
    }
}

#[test]
fn defined_matrix_closed_tags_require_v22() {
    let f = fixture();
    for (tag, contract) in [
        (
            3,
            SemanticDefinedCapabilityContractV1::PolicyMatrixBind(f.bind().unwrap()),
        ),
        (
            4,
            SemanticDefinedCapabilityContractV1::PolicyGfx950Narrow(f.narrow().unwrap()),
        ),
    ] {
        assert_eq!(
            contract.minimum_wire_version(),
            SemanticMirWireVersionV1::V22
        );
        let mut writer = CanonicalWriterV1::new(1024);
        contract.encode(&mut writer).unwrap();
        let bytes = writer.finish();
        assert_eq!(bytes[0], tag);
        for version in [
            SemanticMirWireVersionV1::V20,
            SemanticMirWireVersionV1::V21,
            SemanticMirWireVersionV1::V22,
            SemanticMirWireVersionV1::V23,
        ] {
            let mut decoder = CanonicalDecoderV1::new(&bytes, SemanticMirLimitsV1::default());
            decoder.wire_version = version;
            let result = decoder.defined_capability_contract();
            if version >= SemanticMirWireVersionV1::V22 {
                assert_eq!(result.unwrap(), contract);
                decoder.finish().unwrap();
            } else {
                assert!(
                    result.is_err(),
                    "matrix tag {tag} is not a {version:?} recipe"
                );
            }
        }
    }
}

fn decode_phase(
    bytes: &[u8],
    version: SemanticMirWireVersionV1,
) -> Result<SemanticDefinedCapabilityContractV1, SemanticMirDecodeErrorV1> {
    let mut decoder = CanonicalDecoderV1::new(bytes, SemanticMirLimitsV1::default());
    decoder.wire_version = version;
    let record = decoder.defined_capability_contract()?;
    decoder.finish()?;
    Ok(record)
}

fn phase_records() -> [SemanticDefinedCapabilityContractV1; 2] {
    let f = phase_fixture();
    [
        SemanticDefinedCapabilityContractV1::PolicyMatrixBind(f.bind().unwrap()),
        SemanticDefinedCapabilityContractV1::PolicyGfx950Narrow(f.narrow().unwrap()),
    ]
}

#[test]
fn defined_matrix_issuer_tag7_requires_v23_and_stays_distinct_from_phase() {
    let record = SemanticDefinedCapabilityContractV1::KernelMatrixDerive(
        crate::semantic_mir_v1::defined_matrix_issuer_v1::tests::record_fixture(),
    );
    assert_eq!(record.minimum_wire_version(), SemanticMirWireVersionV1::V23);
    let mut writer = CanonicalWriterV1::new(521);
    record.encode(&mut writer).unwrap();
    let bytes = writer.finish();
    assert_eq!(bytes.len(), 521);
    assert_eq!(bytes[0], 7);
    assert_eq!(
        decode_phase(&bytes, SemanticMirWireVersionV1::V23).unwrap(),
        record
    );
    for version in [
        SemanticMirWireVersionV1::V20,
        SemanticMirWireVersionV1::V21,
        SemanticMirWireVersionV1::V22,
    ] {
        assert!(decode_phase(&bytes, version).is_err());
    }
    for tag in [3, 4, 5, 6, 8] {
        let mut changed = bytes.clone();
        changed[0] = tag;
        assert!(decode_phase(&changed, SemanticMirWireVersionV1::V23).is_err());
    }
}

#[test]
fn defined_matrix_phase_tags_v23_append_only_execution_identity() {
    for ((index, record), legacy) in phase_records().into_iter().enumerate().zip(payloads()) {
        assert_eq!(record.minimum_wire_version(), SemanticMirWireVersionV1::V23);
        let mut writer = CanonicalWriterV1::new(1024);
        record.encode(&mut writer).unwrap();
        let bytes = writer.finish();
        assert_eq!(bytes[0], 5 + index as u8);
        assert_eq!(bytes.len(), 1 + legacy.len() + 32);
        assert_eq!(&bytes[1..1 + legacy.len()], legacy.as_slice());
        assert_eq!(
            &bytes[1 + legacy.len()..],
            phase_fixture()
                .bind()
                .unwrap()
                .identity()
                .execution_brand()
                .as_bytes()
        );
        assert_eq!(
            decode_phase(&bytes, SemanticMirWireVersionV1::V23).unwrap(),
            record
        );
        for version in [
            SemanticMirWireVersionV1::V20,
            SemanticMirWireVersionV1::V21,
            SemanticMirWireVersionV1::V22,
        ] {
            assert!(decode_phase(&bytes, version).is_err());
        }
        let mut old_tag_with_extension = bytes.clone();
        old_tag_with_extension[0] -= 2;
        assert!(decode_phase(&old_tag_with_extension, SemanticMirWireVersionV1::V23).is_err());
    }
}

#[test]
fn defined_matrix_phase_payload_rejects_every_truncation_and_suffix() {
    for record in phase_records() {
        let mut writer = CanonicalWriterV1::new(1024);
        record.encode(&mut writer).unwrap();
        let bytes = writer.finish();
        for end in 0..bytes.len() {
            assert!(
                decode_phase(&bytes[..end], SemanticMirWireVersionV1::V23).is_err(),
                "prefix {end}"
            );
        }
        let mut changed = bytes;
        changed.push(0);
        assert!(decode_phase(&changed, SemanticMirWireVersionV1::V23).is_err());
    }
}

#[test]
fn defined_matrix_phase_payload_rejects_zero_or_redundant_execution_identity() {
    let identity = phase_fixture().bind().unwrap().identity();
    for record in phase_records() {
        let mut writer = CanonicalWriterV1::new(1024);
        record.encode(&mut writer).unwrap();
        let bytes = writer.finish();
        for marker in [
            SemanticTypeIdentityV1::from_sha256([0; 32]),
            identity.policy(),
            identity.kernel_brand(),
            identity.matrix_brand(),
            identity.epoch(),
        ] {
            let mut changed = bytes.clone();
            let start = changed.len() - 32;
            changed[start..].copy_from_slice(marker.as_bytes());
            assert!(decode_phase(&changed, SemanticMirWireVersionV1::V23).is_err());
        }
    }
}

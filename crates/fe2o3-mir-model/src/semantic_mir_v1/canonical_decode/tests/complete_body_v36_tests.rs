//! Synthetic inert codec controls only; no authenticated Rust source is claimed.
use super::*;
use crate::semantic_mir_v1::complete_body_v36 as body;

fn packed() -> SemanticCompleteBodyPackingVNext {
    SemanticCompleteBodyPackingVNext {
        block_count: 1,
        instruction_count: 1,
        block_words: [0x41ff, 0, 0, 0],
        instruction_words: [8, 0, 0, 0],
    }
}
fn literal_frame() -> Vec<u8> {
    let mut bytes = vec![92, 0, 1, 1];
    bytes.extend([0xff, 0x41, 0, 0, 0, 0, 0, 0]);
    bytes.extend([0; 24]);
    bytes.extend([8, 0, 0, 0, 0, 0, 0, 0]);
    bytes.extend([0; 24]);
    assert_eq!(bytes.len(), 68);
    bytes
}
fn decode(
    bytes: &[u8],
    version: SemanticMirWireVersionV1,
) -> Result<SemanticCompilerIntrinsicOperationV1, SemanticMirDecodeErrorV1> {
    let mut decoder = CanonicalDecoderV1::new(bytes, SemanticMirLimitsV1::default());
    decoder.wire_version = version;
    let result = decoder.compiler_intrinsic()?;
    decoder.finish()?;
    Ok(result)
}
fn source() -> SemanticCompleteBodySourceVNext {
    SemanticCompleteBodySourceVNext::new(
        [[1; 32], [2; 32], [3; 32], [4; 32], [5; 32]],
        [6; 32],
        [7; 32],
        [8; 32],
        [9; 32],
        [10; 32],
        0x12345678,
    )
    .unwrap()
}
fn source_bytes(value: Option<SemanticCompleteBodySourceVNext>) -> Vec<u8> {
    let mut writer = CanonicalWriterV1::new(HARD_MAX_CANONICAL_BYTES_V1);
    body::encode_source(&mut writer, value).unwrap();
    writer.finish()
}
fn decode_source(
    bytes: &[u8],
) -> Result<Option<SemanticCompleteBodySourceVNext>, SemanticMirDecodeErrorV1> {
    let mut decoder = CanonicalDecoderV1::new(bytes, SemanticMirLimitsV1::default());
    decoder.wire_version = SemanticMirWireVersionV1::V36;
    let result = decoder.complete_body_source_v36()?;
    decoder.finish()?;
    Ok(result)
}
#[test]
fn version_slot_and_literal_frame_are_exact() {
    assert_eq!(
        SemanticMirWireVersionV1::from_u16(36),
        Some(SemanticMirWireVersionV1::V36)
    );
    assert_eq!(SemanticMirWireVersionV1::V36.as_u16(), 36);
    assert_eq!(SemanticMirWireVersionV1::from_u16(37), None);
    let operation = SemanticCompilerIntrinsicOperationV1::Gfx942CompleteBody(packed());
    let mut writer = CanonicalWriterV1::new(HARD_MAX_CANONICAL_BYTES_V1);
    encode_compiler_intrinsic_operation(&mut writer, operation, SemanticMirWireVersionV1::V36)
        .unwrap();
    assert_eq!(writer.finish(), literal_frame());
    assert_eq!(
        decode(&literal_frame(), SemanticMirWireVersionV1::V36).unwrap(),
        operation
    );
}
#[test]
fn every_old_profile_rejects_the_new_intrinsic_before_payload_decode() {
    for number in (2..=15).chain(28..=35) {
        let version = SemanticMirWireVersionV1::from_u16(number).unwrap();
        assert!(matches!(
            decode(&literal_frame(), version),
            Err(SemanticMirDecodeErrorV1::InvalidTag {
                context: "compiler intrinsic",
                value: 92,
                ..
            })
        ));
        let mut writer = CanonicalWriterV1::new(HARD_MAX_CANONICAL_BYTES_V1);
        assert!(
            matches!(body::encode_packing(&mut writer, packed(), version),
            Err(SemanticMirErrorV1::WireVersionCannotRepresent {
                requested, required: SemanticMirWireVersionV1::V36
            }) if requested == version)
        );
        assert!(writer.finish().is_empty());
    }
}
#[test]
fn v36_does_not_inherit_any_sibling_intrinsic_allocation() {
    for tag in 69..=255 {
        if tag == 92 {
            continue;
        }
        assert!(
            matches!(decode(&[tag], SemanticMirWireVersionV1::V36),
            Err(SemanticMirDecodeErrorV1::InvalidTag {context:"compiler intrinsic", value, ..}) if value == tag),
            "tag={tag}"
        );
    }
}
#[test]
fn revision_truncation_and_trailing_bytes_are_closed() {
    let frame = literal_frame();
    for length in 0..frame.len() {
        assert!(decode(&frame[..length], SemanticMirWireVersionV1::V36).is_err());
    }
    for revision in 1..=255 {
        let mut altered = frame.clone();
        altered[1] = revision;
        assert!(decode(&altered, SemanticMirWireVersionV1::V36).is_err());
    }
    let mut longer = frame;
    longer.push(0);
    assert!(decode(&longer, SemanticMirWireVersionV1::V36).is_err());
}
#[test]
fn counts_reserved_bits_and_target_padding_are_closed() {
    for (field, bad) in [(2, 0), (2, 9), (2, 255), (3, 0), (3, 17), (3, 255)] {
        let mut frame = literal_frame();
        frame[field] = bad;
        assert!(decode(&frame, SemanticMirWireVersionV1::V36).is_err());
    }
    for word in [
        0x800041ffu32,
        0x61ff,
        0x0000c1ff,
        0x008041ff,
        0x008001ff,
        0x51ff,
        0x42ff,
    ] {
        let mut frame = literal_frame();
        frame[4..8].copy_from_slice(&word.to_le_bytes());
        assert!(
            decode(&frame, SemanticMirWireVersionV1::V36).is_err(),
            "word={word:08x}"
        );
    }
}
#[test]
fn every_inactive_block_and_instruction_bit_is_rejected() {
    for slot in 1..8 {
        for bit in 0..32 {
            let mut frame = literal_frame();
            frame[4 + slot * 4..8 + slot * 4].copy_from_slice(&(1u32 << bit).to_le_bytes());
            assert!(decode(&frame, SemanticMirWireVersionV1::V36).is_err());
        }
    }
    for slot in 1..16 {
        for bit in 0..16 {
            let mut frame = literal_frame();
            frame[36 + slot * 2..38 + slot * 2].copy_from_slice(&(1u16 << bit).to_le_bytes());
            assert!(decode(&frame, SemanticMirWireVersionV1::V36).is_err());
        }
    }
}
#[test]
fn malformed_active_instruction_grammar_is_rejected() {
    for word in [0x0408u16, 0x8008, 0x000e, 0x000f, 0x0058, 0x0389, 0x0088] {
        let mut frame = literal_frame();
        frame[36..38].copy_from_slice(&word.to_le_bytes());
        assert!(
            decode(&frame, SemanticMirWireVersionV1::V36).is_err(),
            "word={word:04x}"
        );
    }
    // Structural grammar deliberately does not pretend to prove initialization.
    for word in [0x0038u16, 0x0048, 0] {
        let mut frame = literal_frame();
        frame[36..38].copy_from_slice(&word.to_le_bytes());
        assert!(decode(&frame, SemanticMirWireVersionV1::V36).is_ok());
    }
}
#[test]
fn maximum_packing_keeps_all_active_slots_distinct() {
    let mut value = packed();
    value.block_count = 8;
    value.instruction_count = 16;
    value.block_words = [0; 4];
    value.instruction_words = [0; 4];
    for i in 0..8 {
        value.block_words[i / 2] |= u64::from(0x4200u32 + i as u32) << (32 * (i % 2));
    }
    for i in 0..16 {
        value.instruction_words[i / 4] |= u64::from(8u16 + 16 * (i % 3) as u16) << (16 * (i % 4));
    }
    let mut writer = CanonicalWriterV1::new(HARD_MAX_CANONICAL_BYTES_V1);
    body::encode_packing(&mut writer, value, SemanticMirWireVersionV1::V36).unwrap();
    assert_eq!(
        decode(&writer.finish(), SemanticMirWireVersionV1::V36).unwrap(),
        SemanticCompilerIntrinsicOperationV1::Gfx942CompleteBody(value)
    );
}
#[test]
fn source_tail_contains_all_ten_identities_and_little_endian_block() {
    let mut expected = vec![1];
    for tag in 1..=10 {
        expected.extend([tag; 32]);
    }
    expected.extend([0x78, 0x56, 0x34, 0x12]);
    assert_eq!(expected.len(), 325);
    assert_eq!(source_bytes(Some(source())), expected);
    assert_eq!(decode_source(&expected).unwrap(), Some(source()));
    assert_eq!(source_bytes(None), [0]);
    assert_eq!(decode_source(&[0]).unwrap(), None);
}
#[test]
fn empty_identity_and_malformed_source_frames_are_rejected() {
    let bytes = source_bytes(Some(source()));
    for axis in 0..10 {
        let mut bad = bytes.clone();
        bad[1 + axis * 32..33 + axis * 32].fill(0);
        assert!(decode_source(&bad).is_err(), "axis={axis}");
    }
    for length in 0..bytes.len() {
        assert!(decode_source(&bytes[..length]).is_err());
    }
    assert!(decode_source(&[2]).is_err());
    let mut longer = bytes;
    longer.push(0);
    assert!(decode_source(&longer).is_err());
}
fn ordinary_call() -> SemanticDirectCallV1 {
    SemanticDirectCallV1::new_callable(
        SemanticCallableIdV1(0),
        vec![],
        None,
        SemanticUnwindActionV1::Unreachable,
    )
    .unwrap()
}
#[test]
fn source_tail_never_silently_disappears_in_an_older_encoder() {
    for number in (2..=15).chain(28..=35) {
        let version = SemanticMirWireVersionV1::from_u16(number).unwrap();
        let call = ordinary_call().with_complete_body_source_vnext(source());
        let mut writer = CanonicalWriterV1::new(HARD_MAX_CANONICAL_BYTES_V1);
        assert!(matches!(
            wire_schema_membership_v1::encode_direct_call(&mut writer, &call, version),
            Err(SemanticMirErrorV1::WireVersionCannotRepresent {
                required: SemanticMirWireVersionV1::V36,
                ..
            })
        ));
        assert!(writer.finish().is_empty());
    }
}
#[test]
fn no_source_tail_preserves_old_call_bytes_and_adds_only_v36_none() {
    let call = ordinary_call();
    let mut old = CanonicalWriterV1::new(HARD_MAX_CANONICAL_BYTES_V1);
    wire_schema_membership_v1::encode_direct_call(&mut old, &call, SemanticMirWireVersionV1::V28)
        .unwrap();
    let old = old.finish();
    let mut expected = vec![2];
    expected.extend(0u32.to_le_bytes());
    expected.extend(0u32.to_le_bytes());
    expected.extend(0u32.to_le_bytes());
    expected.extend([0, 1]);
    assert_eq!(old, expected);
    let mut new = CanonicalWriterV1::new(HARD_MAX_CANONICAL_BYTES_V1);
    wire_schema_membership_v1::encode_direct_call(&mut new, &call, SemanticMirWireVersionV1::V36)
        .unwrap();
    let mut expected = old;
    expected.push(0);
    assert_eq!(new.finish(), expected);
}
#[test]
fn sibling_source_tails_cannot_mix_with_v36() {
    let old = SemanticInlineAssemblySourceV30::new(
        [11; 32],
        SemanticFunctionIdentityV1([12; 32]),
        [13; 32],
        [14; 32],
    )
    .unwrap();
    let call = ordinary_call()
        .with_complete_body_source_vnext(source())
        .with_inline_assembly_source_v30(old);
    let mut writer = CanonicalWriterV1::new(HARD_MAX_CANONICAL_BYTES_V1);
    assert!(matches!(
        wire_schema_membership_v1::encode_direct_call(
            &mut writer,
            &call,
            SemanticMirWireVersionV1::V36
        ),
        Err(SemanticMirErrorV1::InvalidCompleteBodyV36)
    ));
    assert!(writer.finish().is_empty());
}
#[test]
fn explicit_v36_roundtrip_does_not_change_ordinary_default_selection() {
    let request = minimal_request();
    let default = request
        .clone()
        .admit_current_production(SemanticMirLimitsV1::default())
        .unwrap();
    assert_eq!(default.wire_version(), SemanticMirWireVersionV1::V5);
    let exact = request
        .admit_exact_v36(SemanticMirLimitsV1::default())
        .unwrap();
    let decoded = AdmittedInertSemanticMirV1::decode_exact_v36_canonical(
        exact.canonical_encoding(),
        SemanticMirLimitsV1::default(),
    )
    .unwrap();
    assert_eq!(decoded.request, exact.request);
    assert_eq!(decoded.semantic_sha256(), exact.semantic_sha256());
    let mut substituted = exact.canonical_encoding().to_vec();
    substituted[MAGIC.len()..MAGIC.len() + 2].copy_from_slice(&35u16.to_le_bytes());
    assert!(matches!(
        AdmittedInertSemanticMirV1::decode_exact_v36_canonical(
            &substituted,
            SemanticMirLimitsV1::default()
        ),
        Err(SemanticMirDecodeErrorV1::WireVersionMismatch { .. })
    ));
}

#[test]
fn ordinary_default_never_selects_v36_from_inert_source_content() {
    let mut request = minimal_request();
    request.functions[0].blocks[0].terminator.kind =
        SemanticTerminatorKindV1::Call(ordinary_call().with_complete_body_source_vnext(source()));
    assert!(matches!(
        request.admit_current_production(SemanticMirLimitsV1::default()),
        Err(SemanticMirErrorV1::InvalidCompleteBodyV36)
    ));
}

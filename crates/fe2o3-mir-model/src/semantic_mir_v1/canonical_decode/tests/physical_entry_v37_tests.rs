//! Inert codec controls only; these records cannot authenticate Rust source.
use super::*;
use crate::semantic_mir_v1::physical_entry_v37 as physical;

const ROWS: [[u8; 8]; 19] = [
    [0, 4, 0, 0, 0, 0, 0, 0],
    [1, 3, 0, 0, 16, 0, 0, 0],
    [2, 0, 0, 0, 0, 0, 0, 0],
    [3, 3, 2, 0, 6, 0, 0, 0],
    [4, 1, 3, 0, 0, 0, 0, 0],
    [5, 1, 3, 0, 0, 0, 0, 0],
    [6, 0, 3, 0, 0, 0, 0, 0],
    [7, 0, 0, 0, 2, 0, 0, 0],
    [8, 0, 0, 0, 3, 0, 0, 0],
    [9, 2, 4, 0, 2, 0, 0, 0],
    [10, 1, 3, 0, 0, 0, 0, 0],
    [11, 1, 3, 0, 0, 0, 0, 0],
    [12, 0, 4, 2, 0, 0, 0, 0],
    [13, 4, 0, 0, 0, 0, 0, 0],
    [14, 0, 2, 1, 0, 0, 0, 0],
    [15, 0, 0, 0, 0, 0, 0, 0],
    [16, 0, 4, 0, 0, 0, 0, 0],
    [17, 0, 0, 0, 0, 0, 0, 0],
    [18, 0, 0, 0, 3, 0, 0, 0],
];
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
fn encode(op: SemanticCompilerIntrinsicOperationV1) -> Vec<u8> {
    let mut writer = CanonicalWriterV1::new(HARD_MAX_CANONICAL_BYTES_V1);
    encode_compiler_intrinsic_operation(&mut writer, op, SemanticMirWireVersionV1::V37).unwrap();
    writer.finish()
}
fn source() -> SemanticPhysicalEntrySourceV37 {
    SemanticPhysicalEntrySourceV37::new(
        [[1; 32], [2; 32], [3; 32], [4; 32], [5; 32]],
        [6; 32],
        [7; 32],
        [8; 32],
        [9; 32],
        [10; 32],
        (0x12345678, 72),
    )
    .unwrap()
}
fn source_bytes(source: Option<SemanticPhysicalEntrySourceV37>) -> Vec<u8> {
    let mut writer = CanonicalWriterV1::new(HARD_MAX_CANONICAL_BYTES_V1);
    physical::encode_source(&mut writer, source).unwrap();
    writer.finish()
}
fn decode_source(
    bytes: &[u8],
) -> Result<Option<SemanticPhysicalEntrySourceV37>, SemanticMirDecodeErrorV1> {
    let mut decoder = CanonicalDecoderV1::new(bytes, SemanticMirLimitsV1::default());
    decoder.wire_version = SemanticMirWireVersionV1::V37;
    let source = decoder.physical_entry_source_v37()?;
    decoder.finish()?;
    Ok(source)
}
fn call() -> SemanticDirectCallV1 {
    SemanticDirectCallV1::new_callable(
        SemanticCallableIdV1(0),
        vec![],
        None,
        SemanticUnwindActionV1::Unreachable,
    )
    .unwrap()
}
#[test]
fn physical_entry_v37_exact_allocations_and_nineteen_descriptor_frames() {
    assert_eq!(
        SemanticMirWireVersionV1::from_u16(37),
        Some(SemanticMirWireVersionV1::V37)
    );
    assert_eq!(SemanticMirWireVersionV1::from_u16(40), None);
    assert_eq!(
        SemanticMirWireVersionV1::from_u16(38),
        Some(SemanticMirWireVersionV1::V38)
    );
    assert_eq!(
        encode(SemanticCompilerIntrinsicOperationV1::Gfx942PhysicalEntryBegin),
        [93, 0]
    );
    assert_eq!(
        encode(SemanticCompilerIntrinsicOperationV1::Gfx942PhysicalEntryLabel(255)),
        [94, 0, 255]
    );
    for row in ROWS {
        let instruction = SemanticPhysicalEntryInstructionV37::from_descriptor(row).unwrap();
        let op = SemanticCompilerIntrinsicOperationV1::Gfx942PhysicalEntryStep(instruction);
        let mut frame = vec![95, 0];
        frame.extend(row);
        assert_eq!(encode(op), frame);
        assert_eq!(decode(&frame, SemanticMirWireVersionV1::V37).unwrap(), op);
    }
}
#[test]
fn physical_entry_v37_all_old_schemas_reject_new_tags_before_payload() {
    for n in (2..=15).chain(28..=36) {
        let version = SemanticMirWireVersionV1::from_u16(n).unwrap();
        for tag in 93..=95 {
            assert!(
                matches!(decode(&[tag],version),Err(SemanticMirDecodeErrorV1::InvalidTag{context:"compiler intrinsic",value,..}) if value==tag)
            );
        }
        let mut writer = CanonicalWriterV1::new(HARD_MAX_CANONICAL_BYTES_V1);
        assert!(matches!(
            encode_compiler_intrinsic_operation(
                &mut writer,
                SemanticCompilerIntrinsicOperationV1::Gfx942PhysicalEntryBegin,
                version
            ),
            Err(SemanticMirErrorV1::WireVersionCannotRepresent {
                required: SemanticMirWireVersionV1::V37,
                ..
            })
        ));
        assert!(writer.finish().is_empty());
    }
}
#[test]
fn physical_entry_v37_does_not_inherit_sibling_intrinsics() {
    for tag in 69..=255 {
        if (93..=95).contains(&tag) {
            continue;
        }
        assert!(
            matches!(decode(&[tag],SemanticMirWireVersionV1::V37),Err(SemanticMirDecodeErrorV1::InvalidTag{context:"compiler intrinsic",value,..}) if value==tag)
        );
    }
}
#[test]
fn physical_entry_v37_revision_truncation_and_suffix_are_closed() {
    for row in ROWS {
        let mut frame = vec![95, 0];
        frame.extend(row);
        for length in 0..frame.len() {
            assert!(decode(&frame[..length], SemanticMirWireVersionV1::V37).is_err());
        }
        for revision in 1..=255 {
            frame[1] = revision;
            assert!(decode(&frame, SemanticMirWireVersionV1::V37).is_err());
        }
        frame[1] = 0;
        frame.push(0);
        assert!(decode(&frame, SemanticMirWireVersionV1::V37).is_err());
    }
}
#[test]
fn physical_entry_v37_reserved_units_pairs_immediates_and_padding_refuse() {
    let invalid = [
        [0, 2, 0, 0, 0, 0, 0, 0],
        [0, 5, 0, 0, 0, 0, 0, 0],
        [0, 64, 0, 0, 0, 0, 0, 0],
        [0, 4, 0, 0, 16, 0, 0, 0],
        [1, 2, 0, 0, 16, 0, 0, 0],
        [1, 3, 0, 0, 8, 0, 0, 0],
        [2, 1, 0, 0, 0, 0, 0, 0],
        [3, 3, 2, 0, 5, 0, 0, 0],
        [4, 0, 3, 0, 0, 0, 0, 0],
        [5, 1, 64, 0, 0, 0, 0, 0],
        [6, 1, 3, 0, 0, 0, 0, 0],
        [7, 0, 0, 0, 0, 1, 0, 0],
        [9, 3, 4, 0, 2, 0, 0, 0],
        [9, 2, 5, 0, 2, 0, 0, 0],
        [10, 1, 64, 0, 0, 0, 0, 0],
        [11, 1, 3, 64, 0, 0, 0, 0],
        [12, 0, 3, 2, 0, 0, 0, 0],
        [13, 2, 0, 0, 0, 0, 0, 0],
        [14, 0, 3, 1, 0, 0, 0, 0],
        [15, 0, 0, 0, 1, 0, 0, 0],
        [16, 0, 5, 0, 0, 0, 0, 0],
        [17, 0, 0, 1, 0, 0, 0, 0],
        [18, 0, 0, 0, 0, 0, 1, 0],
        [19, 0, 0, 0, 0, 0, 0, 0],
    ];
    for row in invalid {
        assert!(
            SemanticPhysicalEntryInstructionV37::from_descriptor(row).is_err(),
            "{row:?}"
        );
    }
    let zero = SemanticPhysicalEntryInstructionV37::new(5, 63, 255, 0, 0).unwrap();
    assert_eq!(zero.descriptor(), [5, 63, 255, 0, 0, 0, 0, 0]);
}
#[test]
fn physical_entry_v37_source_tail_exactly_retains_occurrence_and_ten_identities() {
    let mut expected = vec![1];
    for tag in 1..=10 {
        expected.extend([tag; 32]);
    }
    expected.extend([0x78, 0x56, 0x34, 0x12, 72]);
    assert_eq!(expected.len(), 326);
    assert_eq!(source_bytes(Some(source())), expected);
    assert_eq!(decode_source(&expected).unwrap(), Some(source()));
    assert_eq!(decode_source(&[0]).unwrap(), None);
    assert_eq!(source_bytes(None), [0]);
}
#[test]
fn physical_entry_v37_source_tail_bounds_and_truncation_refuse() {
    let bytes = source_bytes(Some(source()));
    for axis in 0..10 {
        let mut bad = bytes.clone();
        bad[1 + axis * 32..33 + axis * 32].fill(0);
        assert!(decode_source(&bad).is_err());
    }
    for occurrence in 73..=255 {
        let mut bad = bytes.clone();
        bad[325] = occurrence;
        assert!(decode_source(&bad).is_err());
    }
    for length in 0..bytes.len() {
        assert!(decode_source(&bytes[..length]).is_err());
    }
    assert!(decode_source(&[2]).is_err());
    let mut longer = bytes;
    longer.push(0);
    assert!(decode_source(&longer).is_err());
}
#[test]
fn physical_entry_v37_old_encoders_cannot_drop_source_tail() {
    for n in (2..=15).chain(28..=36) {
        let version = SemanticMirWireVersionV1::from_u16(n).unwrap();
        let mut writer = CanonicalWriterV1::new(HARD_MAX_CANONICAL_BYTES_V1);
        assert!(matches!(
            wire_schema_membership_v1::encode_direct_call(
                &mut writer,
                &call().with_physical_entry_source_v37(source()),
                version
            ),
            Err(SemanticMirErrorV1::WireVersionCannotRepresent {
                required: SemanticMirWireVersionV1::V37,
                ..
            })
        ));
        assert!(writer.finish().is_empty());
    }
}
#[test]
fn physical_entry_v37_none_tail_preserves_ordinary_call_prefix() {
    let mut old = CanonicalWriterV1::new(HARD_MAX_CANONICAL_BYTES_V1);
    wire_schema_membership_v1::encode_direct_call(&mut old, &call(), SemanticMirWireVersionV1::V28)
        .unwrap();
    let mut expected = old.finish();
    expected.push(0);
    let mut new = CanonicalWriterV1::new(HARD_MAX_CANONICAL_BYTES_V1);
    wire_schema_membership_v1::encode_direct_call(&mut new, &call(), SemanticMirWireVersionV1::V37)
        .unwrap();
    assert_eq!(new.finish(), expected);
}
#[test]
fn physical_entry_v37_cannot_mix_occurrence_families() {
    let old = SemanticInlineAssemblySourceV30::new(
        [11; 32],
        SemanticFunctionIdentityV1([12; 32]),
        [13; 32],
        [14; 32],
    )
    .unwrap();
    let mixed = call()
        .with_physical_entry_source_v37(source())
        .with_inline_assembly_source_v30(old);
    let mut writer = CanonicalWriterV1::new(HARD_MAX_CANONICAL_BYTES_V1);
    assert!(matches!(
        wire_schema_membership_v1::encode_direct_call(
            &mut writer,
            &mixed,
            SemanticMirWireVersionV1::V37
        ),
        Err(SemanticMirErrorV1::InvalidPhysicalEntryV37)
    ));
    assert!(writer.finish().is_empty());
}
#[test]
fn physical_entry_v37_explicit_roundtrip_does_not_change_ordinary_default() {
    let request = minimal_request();
    assert_eq!(
        request
            .clone()
            .admit_current_production(SemanticMirLimitsV1::default())
            .unwrap()
            .wire_version(),
        SemanticMirWireVersionV1::V5
    );
    let exact = request
        .admit_exact_v37(SemanticMirLimitsV1::default())
        .unwrap();
    let decoded = AdmittedInertSemanticMirV1::decode_exact_v37_canonical(
        exact.canonical_encoding(),
        SemanticMirLimitsV1::default(),
    )
    .unwrap();
    assert_eq!(decoded.request, exact.request);
    assert_eq!(decoded.semantic_sha256(), exact.semantic_sha256());
}
#[test]
fn physical_entry_v37_ordinary_default_never_selects_new_profile_from_inert_content() {
    let mut request = minimal_request();
    request.functions[0].blocks[0].terminator.kind =
        SemanticTerminatorKindV1::Call(call().with_physical_entry_source_v37(source()));
    assert!(matches!(
        request.admit_current_production(SemanticMirLimitsV1::default()),
        Err(SemanticMirErrorV1::InvalidPhysicalEntryV37)
    ));
}

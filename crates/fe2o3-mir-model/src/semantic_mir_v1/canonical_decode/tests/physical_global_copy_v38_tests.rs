//! Inert codec controls only; these records cannot authenticate Rust source.
use super::*;
use crate::semantic_mir_v1::physical_global_copy_v38 as physical;
#[path = "physical_global_copy_abi_v38_tests.rs"]
mod abi;

const ROWS: [[u8; 8]; 15] = [
    [0, 4, 0, 0, 0, 0, 0, 0],
    [1, 0, 0, 0, 0, 0, 0, 0],
    [2, 3, 2, 0, 6, 0, 0, 0],
    [3, 1, 3, 0, 0, 0, 0, 0],
    [4, 1, 3, 0, 0, 0, 0, 0],
    [5, 2, 4, 0, 2, 0, 0, 0],
    [6, 1, 3, 0, 0, 0, 0, 0],
    [7, 1, 3, 0, 0, 0, 0, 0],
    [8, 1, 2, 0, 0, 0, 0, 0],
    [9, 0, 0, 0, 0, 0, 0, 0],
    [10, 0, 4, 2, 0, 0, 0, 0],
    [11, 4, 0, 0, 0, 0, 0, 0],
    [12, 0, 2, 1, 0, 0, 0, 0],
    [13, 0, 4, 0, 0, 0, 0, 0],
    [14, 0, 0, 0, 0, 0, 0, 0],
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
    encode_compiler_intrinsic_operation(&mut writer, op, SemanticMirWireVersionV1::V38).unwrap();
    writer.finish()
}
fn source() -> SemanticPhysicalGlobalCopySourceV38 {
    SemanticPhysicalGlobalCopySourceV38::new(
        [[1; 32], [2; 32], [3; 32], [4; 32], [5; 32]],
        [6; 32],
        [7; 32],
        [8; 32],
        [9; 32],
        [10; 32],
        (0x12345678, 33),
    )
    .unwrap()
}
fn source_bytes(source: Option<SemanticPhysicalGlobalCopySourceV38>) -> Vec<u8> {
    let mut writer = CanonicalWriterV1::new(HARD_MAX_CANONICAL_BYTES_V1);
    physical::encode_source(&mut writer, source).unwrap();
    writer.finish()
}
fn decode_source(
    bytes: &[u8],
) -> Result<Option<SemanticPhysicalGlobalCopySourceV38>, SemanticMirDecodeErrorV1> {
    let mut decoder = CanonicalDecoderV1::new(bytes, SemanticMirLimitsV1::default());
    decoder.wire_version = SemanticMirWireVersionV1::V38;
    let source = decoder.physical_global_copy_source_v38()?;
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
fn physical_global_copy_v38_exact_allocations_and_fifteen_descriptor_frames() {
    assert_eq!(
        SemanticMirWireVersionV1::from_u16(38),
        Some(SemanticMirWireVersionV1::V38)
    );
    assert_eq!(SemanticMirWireVersionV1::from_u16(40), None);
    assert_eq!(
        encode(SemanticCompilerIntrinsicOperationV1::Gfx942PhysicalGlobalCopyBegin),
        [96, 0]
    );
    assert_eq!(
        encode(SemanticCompilerIntrinsicOperationV1::Gfx942PhysicalGlobalCopyLabel(0)),
        [97, 0, 0]
    );
    for row in ROWS {
        let instruction = SemanticPhysicalGlobalCopyInstructionV38::from_descriptor(row).unwrap();
        let op = SemanticCompilerIntrinsicOperationV1::Gfx942PhysicalGlobalCopyStep(instruction);
        let mut frame = vec![98, 0];
        frame.extend(row);
        assert_eq!(encode(op), frame);
        assert_eq!(decode(&frame, SemanticMirWireVersionV1::V38).unwrap(), op);
    }
}
#[test]
fn physical_global_copy_v38_all_old_schemas_reject_new_tags_before_payload() {
    for n in (2..=15).chain(28..=37) {
        let version = SemanticMirWireVersionV1::from_u16(n).unwrap();
        for tag in 96..=98 {
            assert!(
                matches!(decode(&[tag],version),Err(SemanticMirDecodeErrorV1::InvalidTag{context:"compiler intrinsic",value,..}) if value==tag)
            );
        }
        let mut writer = CanonicalWriterV1::new(HARD_MAX_CANONICAL_BYTES_V1);
        assert!(matches!(
            encode_compiler_intrinsic_operation(
                &mut writer,
                SemanticCompilerIntrinsicOperationV1::Gfx942PhysicalGlobalCopyBegin,
                version
            ),
            Err(SemanticMirErrorV1::WireVersionCannotRepresent {
                required: SemanticMirWireVersionV1::V38,
                ..
            })
        ));
        assert!(writer.finish().is_empty());
    }
}
#[test]
fn physical_global_copy_v38_does_not_inherit_sibling_intrinsics() {
    for tag in 69..=255 {
        if (96..=98).contains(&tag) {
            continue;
        }
        assert!(
            matches!(decode(&[tag],SemanticMirWireVersionV1::V38),Err(SemanticMirDecodeErrorV1::InvalidTag{context:"compiler intrinsic",value,..}) if value==tag)
        );
    }
}
#[test]
fn physical_global_copy_v38_revision_truncation_and_suffix_are_closed() {
    for row in ROWS {
        let mut frame = vec![98, 0];
        frame.extend(row);
        for length in 0..frame.len() {
            assert!(decode(&frame[..length], SemanticMirWireVersionV1::V38).is_err());
        }
        for revision in 1..=255 {
            frame[1] = revision;
            assert!(decode(&frame, SemanticMirWireVersionV1::V38).is_err());
        }
        frame[1] = 0;
        frame.push(0);
        assert!(decode(&frame, SemanticMirWireVersionV1::V38).is_err());
    }
}
#[test]
fn physical_global_copy_v38_reserved_units_pairs_immediates_and_padding_refuse() {
    let invalid = [
        [0, 2, 0, 0, 0, 0, 0, 0],
        [0, 5, 0, 0, 0, 0, 0, 0],
        [0, 64, 0, 0, 0, 0, 0, 0],
        [0, 4, 0, 0, 20, 0, 0, 0],
        [1, 1, 0, 0, 0, 0, 0, 0],
        [2, 3, 2, 0, 5, 0, 0, 0],
        [3, 0, 3, 0, 0, 0, 0, 0],
        [4, 1, 64, 0, 0, 0, 0, 0],
        [5, 3, 4, 0, 2, 0, 0, 0],
        [5, 2, 5, 0, 2, 0, 0, 0],
        [6, 1, 64, 0, 0, 0, 0, 0],
        [7, 1, 3, 64, 0, 0, 0, 0],
        [8, 0, 2, 0, 0, 0, 0, 0],
        [8, 1, 3, 0, 0, 0, 0, 0],
        [8, 1, 2, 1, 0, 0, 0, 0],
        [9, 0, 0, 0, 1, 0, 0, 0],
        [10, 0, 3, 2, 0, 0, 0, 0],
        [11, 2, 0, 0, 0, 0, 0, 0],
        [12, 0, 3, 1, 0, 0, 0, 0],
        [13, 0, 5, 0, 0, 0, 0, 0],
        [14, 0, 0, 1, 0, 0, 0, 0],
        [15, 0, 0, 0, 0, 0, 0, 0],
    ];
    for row in invalid {
        assert!(
            SemanticPhysicalGlobalCopyInstructionV38::from_descriptor(row).is_err(),
            "{row:?}"
        );
    }
    let zero = SemanticPhysicalGlobalCopyInstructionV38::new(4, 63, 255, 0, 0).unwrap();
    assert_eq!(zero.descriptor(), [4, 63, 255, 0, 0, 0, 0, 0]);
}
#[test]
fn physical_global_copy_v38_source_tail_exactly_retains_occurrence_and_ten_identities() {
    let mut expected = vec![1];
    for tag in 1..=10 {
        expected.extend([tag; 32]);
    }
    expected.extend([0x78, 0x56, 0x34, 0x12, 33]);
    assert_eq!(expected.len(), 326);
    assert_eq!(source_bytes(Some(source())), expected);
    assert_eq!(decode_source(&expected).unwrap(), Some(source()));
    assert_eq!(decode_source(&[0]).unwrap(), None);
    assert_eq!(source_bytes(None), [0]);
}
#[test]
fn physical_global_copy_v38_source_tail_bounds_and_truncation_refuse() {
    let bytes = source_bytes(Some(source()));
    for axis in 0..10 {
        let mut bad = bytes.clone();
        bad[1 + axis * 32..33 + axis * 32].fill(0);
        assert!(decode_source(&bad).is_err());
    }
    for occurrence in 34..=255 {
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
fn physical_global_copy_v38_old_encoders_cannot_drop_source_tail() {
    for n in (2..=15).chain(28..=37) {
        let version = SemanticMirWireVersionV1::from_u16(n).unwrap();
        let mut writer = CanonicalWriterV1::new(HARD_MAX_CANONICAL_BYTES_V1);
        assert!(matches!(
            wire_schema_membership_v1::encode_direct_call(
                &mut writer,
                &call().with_physical_global_copy_source_v38(source()),
                version
            ),
            Err(SemanticMirErrorV1::WireVersionCannotRepresent {
                required: SemanticMirWireVersionV1::V38,
                ..
            })
        ));
        assert!(writer.finish().is_empty());
    }
}
#[test]
fn physical_global_copy_v38_none_tail_preserves_ordinary_call_prefix() {
    let mut old = CanonicalWriterV1::new(HARD_MAX_CANONICAL_BYTES_V1);
    wire_schema_membership_v1::encode_direct_call(&mut old, &call(), SemanticMirWireVersionV1::V28)
        .unwrap();
    let mut expected = old.finish();
    expected.push(0);
    let mut new = CanonicalWriterV1::new(HARD_MAX_CANONICAL_BYTES_V1);
    wire_schema_membership_v1::encode_direct_call(&mut new, &call(), SemanticMirWireVersionV1::V38)
        .unwrap();
    assert_eq!(new.finish(), expected);
}
#[test]
fn physical_global_copy_v38_cannot_mix_occurrence_families() {
    let old = SemanticInlineAssemblySourceV30::new(
        [11; 32],
        SemanticFunctionIdentityV1([12; 32]),
        [13; 32],
        [14; 32],
    )
    .unwrap();
    let mixed = call()
        .with_physical_global_copy_source_v38(source())
        .with_inline_assembly_source_v30(old);
    let mut writer = CanonicalWriterV1::new(HARD_MAX_CANONICAL_BYTES_V1);
    assert!(matches!(
        wire_schema_membership_v1::encode_direct_call(
            &mut writer,
            &mixed,
            SemanticMirWireVersionV1::V38
        ),
        Err(SemanticMirErrorV1::InvalidPhysicalGlobalCopyV38)
    ));
    assert!(writer.finish().is_empty());
}
#[test]
fn physical_global_copy_v38_explicit_roundtrip_does_not_change_ordinary_default() {
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
        .admit_exact_v38(SemanticMirLimitsV1::default())
        .unwrap();
    let decoded = AdmittedInertSemanticMirV1::decode_exact_v38_canonical(
        exact.canonical_encoding(),
        SemanticMirLimitsV1::default(),
    )
    .unwrap();
    assert_eq!(decoded.request, exact.request);
    assert_eq!(decoded.semantic_sha256(), exact.semantic_sha256());
}
#[test]
fn physical_global_copy_v38_ordinary_default_never_selects_new_profile_from_inert_content() {
    let mut request = minimal_request();
    request.functions[0].blocks[0].terminator.kind =
        SemanticTerminatorKindV1::Call(call().with_physical_global_copy_source_v38(source()));
    assert!(matches!(
        request.admit_current_production(SemanticMirLimitsV1::default()),
        Err(SemanticMirErrorV1::InvalidPhysicalGlobalCopyV38)
    ));
}

#[test]
fn physical_global_copy_v38_label_is_exact_zero() {
    for label in 1..=255 {
        assert!(decode(&[97, 0, label], SemanticMirWireVersionV1::V38).is_err());
    }
}
#[test]
fn physical_global_copy_v38_mixed_physical_tails_are_not_silently_replaced() {
    let old = SemanticPhysicalEntrySourceV37::new(
        [[1; 32]; 5],
        [2; 32],
        [3; 32],
        [4; 32],
        [5; 32],
        [6; 32],
        (0, 0),
    )
    .unwrap();
    let new = source();
    let variants = [
        call()
            .with_physical_entry_source_v37(old)
            .with_physical_global_copy_source_v38(new),
        call()
            .with_physical_global_copy_source_v38(new)
            .with_physical_entry_source_v37(old),
    ];
    for mixed in variants {
        for version in [
            SemanticMirWireVersionV1::V28,
            SemanticMirWireVersionV1::V37,
            SemanticMirWireVersionV1::V38,
        ] {
            let mut writer = CanonicalWriterV1::new(HARD_MAX_CANONICAL_BYTES_V1);
            assert!(matches!(
                wire_schema_membership_v1::encode_direct_call(&mut writer, &mixed, version),
                Err(SemanticMirErrorV1::InvalidPhysicalGlobalCopyV38)
            ));
            assert!(writer.finish().is_empty());
        }
    }
}

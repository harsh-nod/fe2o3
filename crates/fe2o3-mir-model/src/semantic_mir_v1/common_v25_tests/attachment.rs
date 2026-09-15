use super::*;

const MAX_FRAGMENT: u64 = 1_048_576;

fn attached_fixture() -> (SemanticFunctionDeclV1, SemanticFunctionDeclV1) {
    let fixture = crate::semantic_mir_v1::defined_math_v1::tests::fixture();
    let record = fixture.derive().unwrap();
    let contract = SemanticDefinedCapabilityContractV1::KernelMathDerive(record);
    let plain = fixture.functions[record.function().index() as usize].clone();
    assert_eq!(plain.defined_capability_contract(), None);
    let attached = plain
        .clone()
        .with_defined_capability_contract(contract)
        .unwrap();
    assert_eq!(attached.defined_capability_contract(), Some(&contract));
    let mut detached = attached.clone();
    detached.defined_capability_contract = None;
    assert_eq!(
        detached, plain,
        "only the validated optional attachment differs"
    );
    (plain, attached)
}

fn ordinary_bytes(function: &SemanticFunctionDeclV1, version: SemanticMirWireVersionV1) -> Vec<u8> {
    let mut writer = CanonicalWriterV1::new(MAX_FRAGMENT);
    encode_function(&mut writer, function, version).unwrap();
    writer.finish()
}

#[test]
fn common_v25_attachment_exclusion_keeps_real_some_in_ordinary_commitments() {
    let (plain, attached) = attached_fixture();
    let source = canonical_semantic_source_body_sha256_v25(&plain, MAX_FRAGMENT).unwrap();
    assert_eq!(
        canonical_semantic_source_body_sha256_v25(&attached, MAX_FRAGMENT).unwrap(),
        source,
        "the new source fragment excludes the actual Some attachment, including its byte count"
    );
    let ordinary_plain = canonical_semantic_function_fragment_sha256_v1(
        &plain,
        SemanticMirWireVersionV1::V25,
        MAX_FRAGMENT,
    )
    .unwrap();
    let ordinary_attached = canonical_semantic_function_fragment_sha256_v1(
        &attached,
        SemanticMirWireVersionV1::V25,
        MAX_FRAGMENT,
    )
    .unwrap();
    assert_eq!(source, ordinary_plain);
    assert_ne!(source.0, ordinary_attached.0);
    let plain_bytes = ordinary_bytes(&plain, SemanticMirWireVersionV1::V25);
    let attached_bytes = ordinary_bytes(&attached, SemanticMirWireVersionV1::V25);
    assert_ne!(plain_bytes, attached_bytes);
    let mut payload = CanonicalWriterV1::new(MAX_FRAGMENT);
    attached
        .defined_capability_contract()
        .unwrap()
        .encode(&mut payload)
        .unwrap();
    assert_eq!(
        attached_bytes.len() - plain_bytes.len(),
        payload.finish().len()
    );
    assert_eq!(
        ordinary_attached.1 - ordinary_plain.1,
        attached_bytes.len() - plain_bytes.len()
    );
    assert!(
        attached.defined_capability_contract().is_some(),
        "hashing does not detach the live record"
    );
}

#[test]
fn common_v25_attachment_exclusion_preserves_historical_attachment_version_gate() {
    let (plain, attached) = attached_fixture();
    for raw in 21..=25 {
        let version = SemanticMirWireVersionV1::from_u16(raw).unwrap();
        let detached =
            canonical_semantic_function_fragment_sha256_v1(&plain, version, MAX_FRAGMENT).unwrap();
        let retained =
            canonical_semantic_function_fragment_sha256_v1(&attached, version, MAX_FRAGMENT)
                .unwrap();
        assert_ne!(
            detached.0, retained.0,
            "ordinary {version:?} must retain Some"
        );
        assert!(retained.1 > detached.1);
    }
    assert_eq!(
        canonical_semantic_function_fragment_sha256_v1(
            &attached,
            SemanticMirWireVersionV1::V20,
            MAX_FRAGMENT
        ),
        Err(SemanticMirErrorV1::WireVersionCannotRepresent {
            requested: SemanticMirWireVersionV1::V20,
            required: SemanticMirWireVersionV1::V21,
        })
    );
    let (_, bytes) = canonical_semantic_source_body_sha256_v25(&attached, MAX_FRAGMENT).unwrap();
    assert!(matches!(
        canonical_semantic_source_body_sha256_v25(&attached, bytes as u64 - 1),
        Err(SemanticMirErrorV1::LimitExceeded {
            resource: SemanticMirResourceV1::CanonicalBytes,
            ..
        })
    ));
}

#[test]
fn common_v25_attachment_exclusion_still_binds_body_abi_and_source() {
    let (_, attached) = attached_fixture();
    let source = canonical_semantic_source_body_sha256_v25(&attached, MAX_FRAGMENT)
        .unwrap()
        .0;
    let ordinary = canonical_semantic_function_fragment_sha256_v1(
        &attached,
        SemanticMirWireVersionV1::V25,
        MAX_FRAGMENT,
    )
    .unwrap()
    .0;
    for mutation in 0..3 {
        let mut changed = attached.clone();
        match mutation {
            0 => {
                let block = &mut changed.blocks[changed.entry.index() as usize];
                let mut statements = block.statements.to_vec();
                statements.push(SemanticStatementV1::new(
                    SemanticSourceProvenanceV1::unavailable(),
                    SemanticStatementKindV1::Nop,
                ));
                block.statements = statements.into_boxed_slice();
            }
            1 => changed.abi.identity = SemanticAbiIdentityV1::from_sha256([239; 32]),
            2 => {
                changed.source = SemanticSourceProvenanceV1::new(
                    Some(
                        SemanticSourceOriginV1::new(
                            SemanticSourceFileIdentityV1::from_sha256([238; 32]),
                            0,
                            1,
                            1,
                            1,
                            1,
                            2,
                        )
                        .unwrap(),
                    ),
                    None,
                )
            }
            _ => unreachable!(),
        }
        assert_eq!(
            changed.defined_capability_contract(),
            attached.defined_capability_contract()
        );
        assert_ne!(
            canonical_semantic_source_body_sha256_v25(&changed, MAX_FRAGMENT)
                .unwrap()
                .0,
            source,
            "source mutation {mutation}"
        );
        assert_ne!(
            canonical_semantic_function_fragment_sha256_v1(
                &changed,
                SemanticMirWireVersionV1::V25,
                MAX_FRAGMENT
            )
            .unwrap()
            .0,
            ordinary,
            "ordinary mutation {mutation}"
        );
    }
}

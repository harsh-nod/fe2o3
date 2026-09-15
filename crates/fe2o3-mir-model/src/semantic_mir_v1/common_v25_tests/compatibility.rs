use super::*;

#[test]
fn common_v25_does_not_change_default_admission() {
    let request = owner::preliminary().request;
    assert!(minimum_wire_version(&request) < SemanticMirWireVersionV1::V25);
    let admitted = request.admit_current_production(SemanticMirLimitsV1::default()).unwrap();
    assert!(admitted.wire_version() < SemanticMirWireVersionV1::V25);
    assert!(admitted.transpose_owned_flows().is_empty());
}

#[test]
fn common_v25_footer_minimum_is_compositional() {
    let mut request = owner::preliminary().request;
    request.transpose_owned_flows = vec![owner::unbound_row()].into_boxed_slice();
    assert_eq!(minimum_wire_version(&request), SemanticMirWireVersionV1::V25);
    for raw in 2..=24 {
        let version = SemanticMirWireVersionV1::from_u16(raw).unwrap();
        let mut writer = CanonicalWriterV1::new(4096);
        assert_eq!(
            transpose_owned_flow_v25::encode_footer(&mut writer, version, &request.transpose_owned_flows),
            Err(SemanticMirErrorV1::WireVersionCannotRepresent { requested: version, required: SemanticMirWireVersionV1::V25 }),
        );
    }
}

#[test]
fn common_v25_current_decoder_is_declared_version_aware() {
    let old = owner::preliminary();
    let old_bytes = old.canonical_encoding().to_vec();
    let new = old.with_transpose_owned_flows_v25(vec![], SemanticMirLimitsV1::default()).unwrap();
    for bytes in [&old_bytes[..], new.canonical_encoding()] {
        let decoded = AdmittedInertSemanticMirV1::decode_current_production_canonical(bytes, SemanticMirLimitsV1::default()).unwrap();
        assert_eq!(decoded.canonical_encoding(), bytes);
    }
    assert!(AdmittedInertSemanticMirV1::decode_exact_v24_canonical(new.canonical_encoding(), SemanticMirLimitsV1::default()).is_err());
    assert!(AdmittedInertSemanticMirV1::decode_exact_v25_canonical(&old_bytes, SemanticMirLimitsV1::default()).is_err());
    let mut absent = new.canonical_encoding().to_vec();
    absent.truncate(absent.len() - 4);
    assert!(AdmittedInertSemanticMirV1::decode_exact_v25_canonical(&absent, SemanticMirLimitsV1::default()).is_err());
    let mut trailing = old_bytes;
    trailing.extend_from_slice(&[0; 4]);
    assert!(AdmittedInertSemanticMirV1::decode_exact_v24_canonical(&trailing, SemanticMirLimitsV1::default()).is_err());
}

#[test]
fn common_v25_source_fragment_is_fixed_and_binds_the_full_body() {
    let source = owner::preliminary();
    let body = &source.functions()[0];
    let digest = canonical_semantic_source_body_sha256_v25(body, 1_048_576).unwrap();
    assert_eq!(digest, canonical_semantic_function_fragment_sha256_v1(body, SemanticMirWireVersionV1::V25, 1_048_576).unwrap());
    assert_ne!(digest.0, canonical_semantic_function_fragment_sha256_v1(body, SemanticMirWireVersionV1::V24, 1_048_576).unwrap().0);
    let mut changed = body.clone();
    changed.blocks[0].statements = vec![SemanticStatementV1::new(
        SemanticSourceProvenanceV1::unavailable(), SemanticStatementKindV1::Nop,
    )].into_boxed_slice();
    assert_ne!(digest.0, canonical_semantic_source_body_sha256_v25(&changed, 1_048_576).unwrap().0);
    assert!(matches!(canonical_semantic_source_body_sha256_v25(body, 1), Err(SemanticMirErrorV1::LimitExceeded { resource: SemanticMirResourceV1::CanonicalBytes, max: 1, .. })));
}

#[test]
fn common_v25_historical_documents_and_fragments_match_prechange_goldens() {
    let rows: Vec<(u16, String, String)> = serde_json::from_str(include_str!("goldens.json")).unwrap();
    assert_eq!(rows.len(), 23);
    for (version, document, fragment) in rows {
        let version = SemanticMirWireVersionV1::from_u16(version).unwrap();
        let admitted = owner::preliminary().request.admit_for_wire_version(version, SemanticMirLimitsV1::default()).unwrap();
        assert_eq!(Sha256::digest(admitted.canonical_encoding()).iter().map(|b| format!("{b:02x}")).collect::<String>(), document, "document {version:?}");
        let (hash, _) = canonical_semantic_function_fragment_sha256_v1(&admitted.functions()[0], version, 1_048_576).unwrap();
        assert_eq!(hash.iter().map(|b| format!("{b:02x}")).collect::<String>(), fragment, "fragment {version:?}");
    }
}

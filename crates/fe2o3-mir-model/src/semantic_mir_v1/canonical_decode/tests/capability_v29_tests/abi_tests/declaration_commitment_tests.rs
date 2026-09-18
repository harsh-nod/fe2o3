//! Encoding identities are inert even when the input is structurally admitted.
use super::*;
use sha2::{Digest, Sha256};

const DOMAIN: &[u8] = b"fe2o3.inert-semantic-mir.declaration-tables-commitment.v1";

fn commit(request: &InertSemanticMirRequestV1) -> SemanticDeclarationTablesCommitmentV1 {
    canonical_declaration_tables_commitment_v1(
        &request.types,
        &request.callables,
        SemanticMirWireVersionV1::V29,
        SemanticMirLimitsV1::default(),
        &mut |_| Ok(()),
    )
    .unwrap()
}

#[test]
fn declaration_commitments_match_existing_encoders() {
    for ordinal in 0..5 {
        let request = callable_request(ordinal);
        request
            .clone()
            .admit_exact_v29(SemanticMirLimitsV1::default())
            .unwrap();
        let mut writer = CanonicalWriterV1::new(HARD_MAX_CANONICAL_BYTES_V1);
        writer.count(request.types.len()).unwrap();
        for ty in &request.types {
            encode_type(&mut writer, ty, SemanticMirWireVersionV1::V29).unwrap();
        }
        writer.count(request.callables.len()).unwrap();
        for callable in &request.callables {
            encode_callable(&mut writer, callable, SemanticMirWireVersionV1::V29).unwrap();
        }
        let bytes = writer.finish();
        let expected: [u8; 32] = Sha256::new()
            .chain_update(DOMAIN)
            .chain_update(29u16.to_le_bytes())
            .chain_update(&bytes)
            .finalize()
            .into();
        let actual = commit(&request);
        assert_eq!(actual.wire_version(), SemanticMirWireVersionV1::V29);
        assert_eq!(actual.canonical_bytes(), bytes.len() as u64);
        assert_eq!(actual.sha256(), expected);
    }
}

#[test]
fn declaration_commitments_bind_version_domain_and_empty_table_counts() {
    let limits = SemanticMirLimitsV1::default();
    let mut seen = std::collections::BTreeSet::new();
    for version in [
        SemanticMirWireVersionV1::V2,
        SemanticMirWireVersionV1::V3,
        SemanticMirWireVersionV1::V4,
        SemanticMirWireVersionV1::V5,
        SemanticMirWireVersionV1::V6,
        SemanticMirWireVersionV1::V7,
        SemanticMirWireVersionV1::V8,
        SemanticMirWireVersionV1::V9,
        SemanticMirWireVersionV1::V10,
        SemanticMirWireVersionV1::V11,
        SemanticMirWireVersionV1::V12,
        SemanticMirWireVersionV1::V13,
        SemanticMirWireVersionV1::V14,
        SemanticMirWireVersionV1::V15,
        SemanticMirWireVersionV1::V28,
        SemanticMirWireVersionV1::V29,
        SemanticMirWireVersionV1::V30,
    ] {
        let actual =
            canonical_declaration_tables_commitment_v1(&[], &[], version, limits, &mut |_| Ok(()))
                .unwrap();
        let mut writer = CanonicalWriterV1::new(HARD_MAX_CANONICAL_BYTES_V1);
        writer.count(0).unwrap();
        writer.count(0).unwrap();
        let bytes = writer.finish();
        let expected: [u8; 32] = Sha256::new()
            .chain_update(DOMAIN)
            .chain_update(version.as_u16().to_le_bytes())
            .chain_update(&bytes)
            .finalize()
            .into();
        assert_eq!(actual.canonical_bytes(), bytes.len() as u64);
        assert_eq!(actual.sha256(), expected);
        assert!(seen.insert(expected));
        let other_domain: [u8; 32] = Sha256::new()
            .chain_update(b"fe2o3.inert-semantic-mir.function-commitment.v1")
            .chain_update(version.as_u16().to_le_bytes())
            .chain_update(&bytes)
            .finalize()
            .into();
        assert_ne!(actual.sha256(), other_domain);
    }
}

fn changed_admitted(name: &str, change: impl FnOnce(&mut InertSemanticMirRequestV1)) {
    let original = callable_request(1);
    let expected = commit(&original);
    let mut changed = original;
    change(&mut changed);
    // A stale commitment must fail even when fresh structural admission succeeds.
    changed
        .clone()
        .admit_exact_v29(SemanticMirLimitsV1::default())
        .unwrap_or_else(|error| panic!("{name}: {error:?}"));
    assert_ne!(expected, commit(&changed), "{name}");
}

fn changed_binding(name: &str, change: impl FnOnce(&mut SemanticNonBodyCallableBindingV1)) {
    changed_admitted(name, |request| {
        let SemanticCallableDeclV1::CompilerIntrinsic { binding, .. } = &mut request.callables[1]
        else {
            panic!()
        };
        change(binding);
    });
}

#[test]
fn declaration_commitments_detect_fresh_admitted_binding_substitutions() {
    changed_binding("function", |b| b.identity.0[0] ^= 1);
    changed_binding("item", |b| b.item_definition_identity.0[0] ^= 1);
    changed_binding("instance", |b| b.monomorphization_identity.0[0] ^= 1);
    changed_binding("types", |b| b.generic_type_arguments_identity.0[0] ^= 1);
    changed_binding("consts", |b| b.const_generic_arguments_identity.0[0] ^= 1);
    changed_binding("ABI identity", |b| b.abi.identity.0[0] ^= 1);
    changed_binding("ABI layout", |b| b.abi.layout_identity.0[0] ^= 1);
    changed_binding("source", |b| {
        b.source = SemanticSourceProvenanceV1::new(
            Some(
                SemanticSourceOriginV1::new(
                    SemanticSourceFileIdentityV1([90; 32]),
                    0,
                    4,
                    1,
                    0,
                    1,
                    4,
                )
                .unwrap(),
            ),
            None,
        );
    });
    changed_admitted("operation identity", |request| {
        let SemanticCallableDeclV1::CompilerIntrinsic {
            operation_identity, ..
        } = &mut request.callables[1]
        else {
            panic!()
        };
        operation_identity.0[0] ^= 1;
    });
}

#[test]
fn declaration_encoding_does_not_grant_admission_to_unsupported_abi() {
    let original = callable_request(1);
    let mut changed = original.clone();
    let SemanticCallableDeclV1::CompilerIntrinsic { binding, .. } = &mut changed.callables[1]
    else {
        panic!()
    };
    binding.abi.can_unwind = true;
    assert!(
        changed
            .clone()
            .admit_exact_v29(SemanticMirLimitsV1::default())
            .is_err()
    );
    assert_ne!(commit(&original), commit(&changed));
}

#[test]
fn declaration_commitments_detect_fresh_admitted_nominal_type_substitutions() {
    changed_admitted("context nominal identity", |r| {
        r.types[CONTEXT.0 as usize].identity.0[0] ^= 1
    });
    changed_admitted("workgroup layout identity", |r| {
        r.types[WORKGROUP.0 as usize].layout_identity.0[0] ^= 1
    });
    changed_admitted("layout randomization", |r| {
        r.types[CONTEXT.0 as usize].layout.randomization_seed ^= 1
    });
    changed_admitted("fragment distribution", |r| {
        r.types[FRAGMENT.0 as usize].rust_type_kind =
            SemanticRustTypeKindV1::Execution(Role::LaneFragmentU32 {
                lanes: 4,
                elements: 2,
            });
    });
}

#[test]
fn declaration_commitments_bind_complete_ordered_tables_not_function_bodies() {
    let request = callable_request(1);
    let expected = commit(&request);
    let mut changed = request.clone();
    changed.types.swap(CONTEXT.0 as usize, WORKGROUP.0 as usize);
    assert_ne!(expected, commit(&changed));
    changed = request.clone();
    changed.callables.swap(0, 1);
    assert_ne!(expected, commit(&changed));
    changed = request.clone();
    changed.callables = changed.callables[..1].into();
    assert_ne!(expected, commit(&changed));
    changed = request;
    changed.functions[0].identity.0[0] ^= 1;
    assert_eq!(expected, commit(&changed));
}

#[test]
fn declaration_commitments_enforce_cumulative_byte_and_work_limits() {
    let request = callable_request(1);
    let expected = commit(&request);
    let limits = SemanticMirLimitsV1::default();
    let encode = |limits, charge: &mut dyn FnMut(usize) -> Result<(), SemanticMirErrorV1>| {
        canonical_declaration_tables_commitment_v1(
            &request.types,
            &request.callables,
            SemanticMirWireVersionV1::V29,
            limits,
            charge,
        )
    };
    assert_eq!(
        encode(
            limits
                .with_limit(
                    SemanticMirResourceV1::CanonicalBytes,
                    expected.canonical_bytes()
                )
                .unwrap(),
            &mut |_| Ok(())
        )
        .unwrap(),
        expected
    );
    assert!(matches!(
        encode(
            limits
                .with_limit(
                    SemanticMirResourceV1::CanonicalBytes,
                    expected.canonical_bytes() - 1
                )
                .unwrap(),
            &mut |_| Ok(())
        ),
        Err(SemanticMirErrorV1::LimitExceeded {
            resource: SemanticMirResourceV1::CanonicalBytes,
            ..
        })
    ));
    let mut transcript = Vec::new();
    assert_eq!(
        encode(limits, &mut |amount| {
            transcript.push(amount);
            Ok(())
        })
        .unwrap(),
        expected
    );
    let required = transcript.iter().sum::<usize>();
    for max in [required, required - 1] {
        let mut used = 0usize;
        let result = encode(limits, &mut |amount| {
            used += amount;
            if used > max {
                Err(SemanticMirErrorV1::LimitExceeded {
                    resource: SemanticMirResourceV1::ValidationWork,
                    actual: used as u64,
                    max: max as u64,
                })
            } else {
                Ok(())
            }
        });
        assert_eq!(result.is_ok(), max == required);
        assert_eq!(used, required);
    }
    for failed in 0..transcript.len() {
        let mut seen = Vec::new();
        let result = encode(limits, &mut |amount| {
            seen.push(amount);
            if seen.len() == failed + 1 {
                Err(SemanticMirErrorV1::InvalidSourceOrigin)
            } else {
                Ok(())
            }
        });
        assert_eq!(result.unwrap_err(), SemanticMirErrorV1::InvalidSourceOrigin);
        assert_eq!(seen, transcript[..=failed]);
    }
}

//! Public-document compatibility for the frozen V13 and V14 intrinsic grammars.

use fe2o3_mir_model::semantic_mir_v1::*;
use sha2::{Digest, Sha256};

const MAGIC: &[u8] = b"fe2o3.inert-semantic-mir";
const SCOPE_OPERATION_IDENTITY: [u8; 32] = [0xa7; 32];

// Frozen at df04c62e90b277416ed44698b8916f7b446bc0a7 and independently
// reconstructed from the field grammar. Only version byte 24 differs.
const FROZEN_BYTE_LENGTH: usize = 965;
const FROZEN_SCOPE_TAG_OFFSET: usize = 920;
const V13_SHA256: [u8; 32] = [
    0x6e, 0xd5, 0x71, 0xbd, 0x94, 0x61, 0x77, 0xab, 0x23, 0xf9, 0x6d, 0x7b, 0x7a, 0xea, 0x75, 0x8a,
    0x4c, 0x1b, 0x27, 0x3b, 0xc5, 0x8f, 0xb9, 0x9d, 0x3f, 0x4d, 0x96, 0x51, 0x06, 0xe5, 0xbf, 0x53,
];
const V14_SHA256: [u8; 32] = [
    0xd1, 0x2e, 0x9d, 0x33, 0x55, 0xed, 0xbe, 0xb6, 0x3d, 0x37, 0xe6, 0x10, 0x66, 0x25, 0x17, 0x3a,
    0x20, 0x22, 0xed, 0x96, 0xd0, 0xb4, 0xf4, 0xc6, 0xd6, 0x8c, 0x0f, 0xff, 0xd4, 0x28, 0x0d, 0xbf,
];

fn scope_abi(identity: u8) -> SemanticFunctionAbiV1 {
    SemanticFunctionAbiV1::new(
        SemanticAbiIdentityV1::from_sha256([identity; 32]),
        SemanticLayoutIdentityV1::from_sha256([identity + 1; 32]),
        SemanticCanonAbiV1::Rust,
        false,
        false,
        vec![],
        SemanticAbiValueV1::new(
            SemanticTypeIdV1::from_index(0),
            SemanticAbiPassModeV1::Ignore,
        ),
    )
    .unwrap()
}

fn scope_request() -> InertSemanticMirRequestV1 {
    let scope = SemanticTypeIdV1::from_index(0);
    let source = SemanticSourceProvenanceV1::unavailable();
    let scope_type = SemanticTypeDeclV1::new(
        SemanticTypeIdentityV1::from_sha256([1; 32]),
        SemanticLayoutIdentityV1::from_sha256([2; 32]),
        SemanticTypeLayoutV1::aggregate_with_backend_repr(
            Some(0),
            1,
            SemanticBackendReprV1::memory(true),
            false,
            SemanticAggregateLayoutV1::new(vec![], vec![]).unwrap(),
        )
        .unwrap(),
        SemanticTypeShapeV1::Aggregate(SemanticAggregateTypeV1::new(vec![]).unwrap()),
    );
    let call = SemanticDirectCallV1::new_callable(
        SemanticCallableIdV1::from_index(1),
        vec![],
        Some(SemanticCallDestinationV1::new(
            SemanticPlaceV1::new(SemanticLocalIdV1::from_index(0), vec![], scope).unwrap(),
            SemanticControlFlowEdgeV1::new(
                SemanticEdgeRoleV1::CallReturn,
                SemanticBlockIdV1::from_index(1),
            ),
        )),
        SemanticUnwindActionV1::Unreachable,
    )
    .unwrap();
    let function = SemanticFunctionDeclV1::new(
        SemanticFunctionIdentityV1::from_sha256([5; 32]),
        SemanticFunctionRoleV1::KernelRoot,
        SemanticItemDefinitionIdentityV1::from_sha256([6; 32]),
        SemanticMonomorphizationIdentityV1::from_sha256([7; 32]),
        SemanticGenericTypeArgumentsIdentityV1::from_sha256([8; 32]),
        SemanticConstGenericArgumentsIdentityV1::from_sha256([9; 32]),
        source,
        scope_abi(3),
        vec![SemanticLocalDeclV1::new(
            SemanticLocalIdentityV1::from_sha256([10; 32]),
            scope,
            SemanticLocalRoleV1::Return,
            source,
        )],
        SemanticBlockIdV1::from_index(0),
        vec![
            SemanticBasicBlockV1::new(
                SemanticBlockIdentityV1::from_sha256([11; 32]),
                source,
                vec![],
                SemanticTerminatorV1::new(source, SemanticTerminatorKindV1::Call(call)),
            )
            .unwrap(),
            SemanticBasicBlockV1::new(
                SemanticBlockIdentityV1::from_sha256([12; 32]),
                source,
                vec![],
                SemanticTerminatorV1::new(source, SemanticTerminatorKindV1::Return),
            )
            .unwrap(),
        ],
    )
    .unwrap();
    let callable = SemanticCallableDeclV1::CompilerIntrinsic {
        binding: SemanticNonBodyCallableBindingV1::new(
            SemanticFunctionIdentityV1::from_sha256([13; 32]),
            SemanticItemDefinitionIdentityV1::from_sha256([14; 32]),
            SemanticMonomorphizationIdentityV1::from_sha256([15; 32]),
            SemanticGenericTypeArgumentsIdentityV1::from_sha256([16; 32]),
            SemanticConstGenericArgumentsIdentityV1::from_sha256([17; 32]),
            source,
            scope_abi(18),
        ),
        operation: SemanticCompilerIntrinsicOperationV1::WorkgroupLdsScopeCurrent { scope },
        operation_identity: SemanticCompilerIntrinsicIdentityV1::from_sha256(
            SCOPE_OPERATION_IDENTITY,
        ),
    };
    InertSemanticMirRequestV1::new_with_callables(
        SemanticTargetDataLayoutV1::gfx942(SemanticLayoutIdentityV1::from_sha256([20; 32])),
        vec![scope_type],
        vec![],
        vec![],
        vec![],
        vec![function],
        vec![
            SemanticCallableDeclV1::defined(SemanticFunctionIdV1::from_index(0)),
            callable,
        ],
        vec![SemanticFunctionIdV1::from_index(0)],
    )
    .unwrap()
}

fn admit(version: SemanticMirWireVersionV1) -> AdmittedInertSemanticMirV1 {
    let limits = SemanticMirLimitsV1::default();
    match version {
        SemanticMirWireVersionV1::V13 => scope_request().admit_exact_v13(limits),
        SemanticMirWireVersionV1::V14 => scope_request().admit_exact_v14(limits),
        _ => unreachable!("this fixture freezes only V13 and V14"),
    }
    .unwrap()
}

fn decode_exact(
    version: SemanticMirWireVersionV1,
    bytes: &[u8],
) -> Result<AdmittedInertSemanticMirV1, SemanticMirDecodeErrorV1> {
    let limits = SemanticMirLimitsV1::default();
    match version {
        SemanticMirWireVersionV1::V13 => {
            AdmittedInertSemanticMirV1::decode_exact_v13_canonical(bytes, limits)
        }
        SemanticMirWireVersionV1::V14 => {
            AdmittedInertSemanticMirV1::decode_exact_v14_canonical(bytes, limits)
        }
        _ => unreachable!("this fixture freezes only V13 and V14"),
    }
}

fn scope_tag_offset(bytes: &[u8]) -> usize {
    // Final callable: tag66, scope TypeId0, operation identity, then one root0.
    // This literal suffix comes from the frozen grammar, not a decoder search.
    let mut suffix = vec![66, 0, 0, 0, 0];
    suffix.extend_from_slice(&SCOPE_OPERATION_IDENTITY);
    suffix.extend_from_slice(&[1, 0, 0, 0, 0, 0, 0, 0]);
    assert_eq!(suffix.len(), 45);
    let offset = bytes.len().checked_sub(45).unwrap();
    assert_eq!(offset, FROZEN_SCOPE_TAG_OFFSET);
    assert_eq!(&bytes[offset..], suffix.as_slice());
    assert_eq!(
        bytes
            .windows(suffix.len())
            .filter(|window| *window == suffix.as_slice())
            .count(),
        1,
        "the typed scope record must have exactly one frozen suffix",
    );
    offset
}

fn check_frozen_scope_document(version: SemanticMirWireVersionV1, expected_hash: [u8; 32]) {
    let admitted = admit(version);
    let bytes = admitted.canonical_encoding();
    let hash: [u8; 32] = Sha256::digest(bytes).into();
    scope_tag_offset(bytes);
    assert_eq!(bytes.len(), FROZEN_BYTE_LENGTH);
    assert_eq!(hash, expected_hash);
    assert_eq!(admitted.semantic_sha256().as_bytes(), &expected_hash);
    for decoded in [
        decode_exact(version, bytes).unwrap(),
        AdmittedInertSemanticMirV1::decode_current_production_canonical(
            bytes,
            SemanticMirLimitsV1::default(),
        )
        .unwrap(),
    ] {
        assert_eq!(decoded.wire_version(), version);
        assert_eq!(decoded.canonical_encoding(), bytes);
        assert_eq!(decoded.semantic_sha256(), admitted.semantic_sha256());
        assert!(matches!(
            decoded.callables().get(1),
            Some(SemanticCallableDeclV1::CompilerIntrinsic {
                operation: SemanticCompilerIntrinsicOperationV1::WorkgroupLdsScopeCurrent {
                    scope,
                },
                ..
            }) if *scope == SemanticTypeIdV1::from_index(0)
        ));
    }
}

#[test]
fn complete_scope_document_preserves_frozen_v13_bytes() {
    check_frozen_scope_document(SemanticMirWireVersionV1::V13, V13_SHA256);
}

#[test]
fn complete_scope_document_preserves_frozen_v14_bytes() {
    check_frozen_scope_document(SemanticMirWireVersionV1::V14, V14_SHA256);
}

#[test]
fn every_successor_intrinsic_tag_is_rejected_in_its_frozen_document() {
    for (version, first_forbidden) in [
        (SemanticMirWireVersionV1::V13, 67_u8),
        (SemanticMirWireVersionV1::V14, 68_u8),
    ] {
        let admitted = admit(version);
        let original = admitted.canonical_encoding();
        let offset = scope_tag_offset(original);
        for tag in first_forbidden..=u8::MAX {
            let mut mutated = original.to_vec();
            mutated[offset] = tag;
            for result in [
                decode_exact(version, &mutated),
                AdmittedInertSemanticMirV1::decode_current_production_canonical(
                    &mutated,
                    SemanticMirLimitsV1::default(),
                ),
            ] {
                assert!(
                    matches!(
                        result,
                        Err(SemanticMirDecodeErrorV1::InvalidTag {
                            context: "compiler intrinsic",
                            value,
                            offset: actual_offset,
                        }) if value == tag && actual_offset == offset
                    ),
                    "{version:?} tag {tag} did not fail at its intrinsic byte: {result:?}",
                );
            }
        }
    }
}

#[test]
fn wrong_envelope_version_is_distinct_from_a_forbidden_intrinsic_tag() {
    for (actual, expected) in [
        (SemanticMirWireVersionV1::V13, SemanticMirWireVersionV1::V14),
        (SemanticMirWireVersionV1::V14, SemanticMirWireVersionV1::V13),
    ] {
        let admitted = admit(actual);
        assert!(matches!(
            decode_exact(expected, admitted.canonical_encoding()),
            Err(SemanticMirDecodeErrorV1::WireVersionMismatch {
                expected: found_expected,
                actual: found_actual,
            }) if found_expected == expected && found_actual == actual
        ));
    }

    let admitted = admit(SemanticMirWireVersionV1::V13);
    let mut forged = admitted.canonical_encoding().to_vec();
    let offset = scope_tag_offset(&forged);
    assert_eq!(&forged[..MAGIC.len()], MAGIC);
    assert_eq!(&forged[MAGIC.len()..MAGIC.len() + 2], &[13, 0]);
    forged[MAGIC.len()..MAGIC.len() + 2].copy_from_slice(&[12, 0]);
    assert!(matches!(
        AdmittedInertSemanticMirV1::decode_exact_v12_canonical(
            &forged,
            SemanticMirLimitsV1::default(),
        ),
        Err(SemanticMirDecodeErrorV1::InvalidTag {
            context: "compiler intrinsic",
            value: 66,
            offset: actual_offset,
        }) if actual_offset == offset
    ));
}

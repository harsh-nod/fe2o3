#![feature(rustc_private)]

use dialect_mir::{
    MirAddressSpace, MirAggregateLayout, MirLayout, MirMutability, MirScalarType, MirSemanticType,
    MirStructType, MirTypeKind,
};
use fe2o3_rustc_front::StableTypeIdentityV1;
use rustc_codegen_fe2o3::semantic_layout_bridge::{
    MAX_SEMANTIC_LAYOUT_DEPTH_V1, MAX_SEMANTIC_LAYOUT_FIELDS_V1,
    MAX_SEMANTIC_LAYOUT_TARGET_TEXT_BYTES_V1, MAX_SEMANTIC_LAYOUT_TYPE_TEXT_BYTES_V1,
    SEMANTIC_LAYOUT_EVIDENCE_SCHEMA_V2, SEMANTIC_LAYOUT_EVIDENCE_VERSION_V2,
    SemanticLayoutBridgeError, SemanticLayoutEvidenceV1, SemanticLayoutTargetV1,
};

fn target() -> SemanticLayoutTargetV1 {
    SemanticLayoutTargetV1::new("test-target", "e-p:64:64", 64).unwrap()
}

fn source_identity() -> StableTypeIdentityV1 {
    StableTypeIdentityV1::new([0x5a; 32]).unwrap()
}

fn u32_type() -> MirSemanticType {
    MirSemanticType {
        layout: MirLayout::sized(4, 4),
        kind: MirTypeKind::Scalar(MirScalarType::Int {
            signed: false,
            bits: 32,
        }),
    }
}

fn address_space_pointer() -> MirSemanticType {
    MirSemanticType {
        layout: MirLayout::sized(8, 8),
        kind: MirTypeKind::RawPointer {
            pointee: Box::new(u32_type()),
            mutability: MirMutability::Mutable,
            address_space: MirAddressSpace(5),
        },
    }
}

#[test]
fn g2_layout_evidence_is_versioned_target_bound_and_canonical() {
    let target = target();
    let first = SemanticLayoutEvidenceV1::from_semantic_type(
        &target,
        target.clone(),
        source_identity(),
        address_space_pointer(),
    )
    .unwrap();
    let second = SemanticLayoutEvidenceV1::from_semantic_type(
        &target,
        target.clone(),
        source_identity(),
        address_space_pointer(),
    )
    .unwrap();

    assert_eq!(first.schema(), SEMANTIC_LAYOUT_EVIDENCE_SCHEMA_V2);
    assert_eq!(first.version(), SEMANTIC_LAYOUT_EVIDENCE_VERSION_V2);
    assert_eq!(first.target(), &target);
    assert_eq!(first.source_type_identity(), source_identity());
    assert_eq!(first.semantic_type(), &address_space_pointer());
    assert_eq!(first.canonical_bytes(), second.canonical_bytes());
    assert_eq!(
        std::str::from_utf8(first.canonical_bytes()).unwrap(),
        concat!(
            "fe2o3.semantic-layout-evidence.v2|",
            "target(llvm=11:test-target;data-layout=9:e-p:64:64;default-pointer-bits=64;",
            "cpu=unavailable;features=unavailable)|",
            "source-type=5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a",
            "5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a|",
            "semantic-type=mir.type.v1{layout(size=8;align=8);",
            "kind=raw(mut=mut;addrspace=5;type={layout(size=4;align=4);",
            "kind=scalar(u32)})}"
        )
    );
}

#[test]
fn g2_layout_rejects_every_target_identity_mismatch() {
    let expected = target();
    for observed in [
        SemanticLayoutTargetV1::new("other-target", "e-p:64:64", 64).unwrap(),
        SemanticLayoutTargetV1::new("test-target", "E-p:64:64", 64).unwrap(),
        SemanticLayoutTargetV1::new("test-target", "e-p:64:64", 32).unwrap(),
    ] {
        assert!(matches!(
            SemanticLayoutEvidenceV1::from_semantic_type(
                &expected,
                observed,
                source_identity(),
                u32_type(),
            ),
            Err(SemanticLayoutBridgeError::TargetMismatch { .. })
        ));
    }
}

#[test]
fn g2_layout_target_identity_is_bounded_and_exact() {
    for (llvm, data_layout, width) in [
        ("", "e-p:64:64", 64),
        ("test-target", "", 64),
        ("test\ntarget", "e-p:64:64", 64),
        ("test-target", "e-p:\t64", 64),
        ("test-target", "e-p:64:64", 0),
        ("test-target", "e-p:64:64", 24),
        ("test-target", "e-p:64:64", 256),
    ] {
        assert!(SemanticLayoutTargetV1::new(llvm, data_layout, width).is_err());
    }

    let oversized = "x".repeat(MAX_SEMANTIC_LAYOUT_TARGET_TEXT_BYTES_V1 + 1);
    assert!(matches!(
        SemanticLayoutTargetV1::new(oversized, "e-p:64:64", 64),
        Err(SemanticLayoutBridgeError::BoundExceeded { .. })
    ));
}

#[test]
fn g2_layout_rejects_invalid_dialect_type_before_transport() {
    let target = target();
    let malformed = MirSemanticType {
        layout: MirLayout::sized(3, 4),
        kind: MirTypeKind::Scalar(MirScalarType::Int {
            signed: false,
            bits: 32,
        }),
    };
    assert!(matches!(
        SemanticLayoutEvidenceV1::from_semantic_type(
            &target,
            target.clone(),
            source_identity(),
            malformed,
        ),
        Err(SemanticLayoutBridgeError::DialectValidation(_))
    ));
}

#[test]
fn g2_layout_semantic_input_is_preflight_bounded_before_recursive_validation() {
    let target = target();
    let oversized = MirSemanticType {
        layout: MirLayout::sized(0, 1),
        kind: MirTypeKind::Struct(MirStructType {
            identity: "x".repeat(MAX_SEMANTIC_LAYOUT_TYPE_TEXT_BYTES_V1 + 1),
            aggregate: MirAggregateLayout {
                fields: vec![],
                padding: vec![],
            },
        }),
    };
    assert!(matches!(
        SemanticLayoutEvidenceV1::from_semantic_type(
            &target,
            target.clone(),
            source_identity(),
            oversized,
        ),
        Err(SemanticLayoutBridgeError::BoundExceeded {
            field: "semantic layout identity text",
            ..
        })
    ));

    let mut too_deep = u32_type();
    for _ in 0..=MAX_SEMANTIC_LAYOUT_DEPTH_V1 {
        too_deep = MirSemanticType {
            layout: MirLayout::sized(8, 8),
            kind: MirTypeKind::RawPointer {
                pointee: Box::new(too_deep),
                mutability: MirMutability::Immutable,
                address_space: MirAddressSpace::DEFAULT,
            },
        };
    }
    assert!(matches!(
        SemanticLayoutEvidenceV1::from_semantic_type(
            &target,
            target.clone(),
            source_identity(),
            too_deep,
        ),
        Err(SemanticLayoutBridgeError::BoundExceeded {
            field: "semantic layout depth",
            ..
        })
    ));

    let too_many_fields = MirSemanticType {
        layout: MirLayout::sized(0, 1),
        kind: MirTypeKind::Struct(MirStructType {
            identity: "fixture::Wide".to_owned(),
            aggregate: MirAggregateLayout {
                fields: (0..=MAX_SEMANTIC_LAYOUT_FIELDS_V1)
                    .map(|index| dialect_mir::MirField {
                        name: Some(format!("field{index}")),
                        offset: 0,
                        ty: MirSemanticType {
                            layout: MirLayout::sized(0, 1),
                            kind: MirTypeKind::Unit,
                        },
                    })
                    .collect(),
                padding: vec![],
            },
        }),
    };
    assert!(matches!(
        SemanticLayoutEvidenceV1::from_semantic_type(
            &target,
            target.clone(),
            source_identity(),
            too_many_fields,
        ),
        Err(SemanticLayoutBridgeError::BoundExceeded {
            field: "semantic layout aggregate fields",
            ..
        })
    ));
}

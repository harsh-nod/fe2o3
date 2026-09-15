mod global_bf16_matrix_tests {
    use super::*;
    use crate::semantic_mir_v1::global_bf16_matrix_v1::{abi_matches, record_claim};

    fn contract(role: SemanticMfmaOperandRoleV1) -> SemanticGlobalBf16MatrixLoadV1 {
        SemanticGlobalBf16MatrixLoadV1::new(
            SemanticGlobalBf16MatrixTypesV1 {
                matrix: SemanticTypeIdV1(8),
                global: SemanticTypeIdV1(6),
                lane: SemanticTypeIdV1(10),
                fragment: SemanticTypeIdV1(12),
                element: SemanticTypeIdV1(1),
                index: SemanticTypeIdV1(2),
            },
            SemanticMfmaOperandContractV1 {
                role,
                profile: SemanticMfmaProfileV1::Bf16F32M16N16K16,
                register_distribution: SemanticMfmaRegisterDistributionV1::Tile16x16,
                wave_width: 64,
            },
            SemanticTypeIdentityV1(identity(60)),
            SemanticTypeIdentityV1(identity(61)),
            numerical_contract().provenance(),
            SemanticFunctionIdentityV1(identity(62)),
        )
        .unwrap()
    }

    fn decode(
        bytes: &[u8],
        version: SemanticMirWireVersionV1,
    ) -> Result<SemanticCompilerIntrinsicOperationV1, SemanticMirDecodeErrorV1> {
        let mut decoder = CanonicalDecoderV1::new(bytes, SemanticMirLimitsV1::default());
        decoder.wire_version = version;
        let operation = decoder.compiler_intrinsic()?;
        decoder.finish()?;
        Ok(operation)
    }

    #[test]
    fn global_bf16_codec_is_closed_versioned_and_rejects_every_truncation() {
        for role in [SemanticMfmaOperandRoleV1::A, SemanticMfmaOperandRoleV1::B] {
            let operation = SemanticCompilerIntrinsicOperationV1::GlobalBf16MatrixLoad {
                contract: contract(role),
            };
            let bytes = compiler_intrinsic_round_trip(operation, SemanticMirWireVersionV1::V22);
            assert_eq!(bytes[0], 80);
            assert_eq!(
                minimum_wire_version(&version_selection_request([operation])),
                SemanticMirWireVersionV1::V22
            );
            for end in 0..bytes.len() {
                assert!(
                    decode(&bytes[..end], SemanticMirWireVersionV1::V22).is_err(),
                    "accepted prefix {end}"
                );
            }
            let mut suffixed = bytes.clone();
            suffixed.push(0);
            assert!(decode(&suffixed, SemanticMirWireVersionV1::V22).is_err());
            for revision in 2..22 {
                let version = SemanticMirWireVersionV1::from_u16(revision).unwrap();
                assert!(decode(&bytes, version).is_err());
                assert!(
                    encode_compiler_intrinsic_operation(
                        &mut CanonicalWriterV1::new(HARD_MAX_CANONICAL_BYTES_V1),
                        operation,
                        version,
                    )
                    .is_err()
                );
            }
            // Explicit role/profile/distribution/width/layout bytes follow six type IDs.
            for offset in 25..33 {
                let mut changed = bytes.clone();
                changed[offset] = 255;
                assert!(
                    decode(&changed, SemanticMirWireVersionV1::V22).is_err(),
                    "accepted tag at {offset}"
                );
            }
            let mut exclusive = bytes.clone();
            exclusive[98] = 2;
            exclusive[99] = 2;
            assert!(decode(&exclusive, SemanticMirWireVersionV1::V22).is_err());
        }
        let old = SemanticCompilerIntrinsicOperationV1::Bf16MatrixLoadZeroFilledV2 {
            fragment: SemanticTypeIdV1(12),
            view: SemanticTypeIdV1(8),
            lane: SemanticTypeIdV1(10),
            contract: contract(SemanticMfmaOperandRoleV1::A).operand(),
            storage_layout: SemanticMfmaStorageLayoutV1::RowMajor,
        };
        assert_eq!(
            compiler_intrinsic_round_trip(old, SemanticMirWireVersionV1::V21),
            compiler_intrinsic_round_trip(old, SemanticMirWireVersionV1::V22)
        );
    }

    #[test]
    fn global_bf16_contract_rejects_open_profiles_aliases_and_empty_identities() {
        let c = contract(SemanticMfmaOperandRoleV1::A);
        for change in 0..6 {
            let mut t = c.types();
            let mut operand = c.operand();
            let mut matrix_brand = c.matrix_brand();
            let mut global_brand = c.global_brand();
            let mut source = c.source_identity();
            match change {
                0 => operand.wave_width = 32,
                1 => operand.profile = SemanticMfmaProfileV1::Fp4E2M1F32M16N16K128,
                2 => t.global = t.matrix,
                3 => matrix_brand = SemanticTypeIdentityV1(identity(0)),
                4 => global_brand = SemanticTypeIdentityV1(identity(0)),
                5 => source = SemanticFunctionIdentityV1(identity(0)),
                _ => unreachable!(),
            }
            assert!(
                SemanticGlobalBf16MatrixLoadV1::new(
                    t,
                    operand,
                    matrix_brand,
                    global_brand,
                    c.provenance(),
                    source
                )
                .is_err()
            );
        }
        let operation = SemanticCompilerIntrinsicOperationV1::GlobalBf16MatrixLoad { contract: c };
        assert!(compiler_intrinsic_source_identity_matches(
            operation,
            c.source_identity()
        ));
        assert!(!compiler_intrinsic_source_identity_matches(
            operation,
            SemanticFunctionIdentityV1(identity(63))
        ));
    }

    #[test]
    fn global_bf16_read_cannot_supply_its_own_binding() {
        let c = contract(SemanticMfmaOperandRoleV1::A);
        let expected = BoundCapabilityMemoryViewV1 {
            element: c.types().element,
            contract: c.memory(),
            provenance: c.provenance(),
        };
        let mut claims = IntrinsicCapabilityClaimsV1::default();
        assert!(record_claim(&mut claims, c));
        assert!(!claims.memory_accesses_match_bindings());
        for change in 0..3 {
            let mut claims = IntrinsicCapabilityClaimsV1::default();
            assert!(record_claim(&mut claims, c));
            let mut binding = expected;
            let mut view = c.types().global;
            match change {
                0 => {
                    binding.contract =
                        SemanticCapabilityMemoryContractV1::global_exclusive_read_write()
                }
                1 => binding.element = c.types().index,
                2 => view = c.types().matrix,
                _ => unreachable!(),
            }
            assert!(claims.bind_memory_view(view, binding));
            assert!(!claims.memory_accesses_match_bindings());
        }
        assert!(claims.bind_memory_view(c.types().global, expected));
        assert!(claims.memory_accesses_match_bindings());
    }

    // Synthetic schema fixture only. No authentication or live graph proof is issued here.
    fn layout_request() -> InertSemanticMirRequestV1 {
        let mut request = numerical_request();
        let decl = |tag, layout, shape| {
            SemanticTypeDeclV1::new(
                SemanticTypeIdentityV1(identity(tag)),
                SemanticLayoutIdentityV1(identity(tag)),
                layout,
                shape,
            )
        };
        let scalar = |tag, bits| {
            decl(
                tag,
                SemanticTypeLayoutV1::new(Some(u64::from(bits / 8)), u64::from(bits / 8)).unwrap(),
                SemanticTypeShapeV1::Scalar(SemanticScalarTypeV1::Integer {
                    signed: false,
                    bits,
                }),
            )
        };
        let aggregate = |tag, fields, offsets, bytes, align| {
            decl(
                tag,
                SemanticTypeLayoutV1::aggregate(
                    Some(bytes),
                    align,
                    SemanticAggregateLayoutV1::new(offsets, vec![]).unwrap(),
                )
                .unwrap(),
                SemanticTypeShapeV1::Aggregate(SemanticAggregateTypeV1::new(fields).unwrap()),
            )
        };
        let reference = |tag, pointee, metadata, bytes| {
            decl(
                tag,
                SemanticTypeLayoutV1::new(Some(bytes), 8).unwrap(),
                SemanticTypeShapeV1::Pointer(
                    SemanticPointerTypeV1::new_with_kind(
                        pointee,
                        SemanticPointerKindV1::Reference,
                        SemanticMutabilityV1::Immutable,
                        0,
                        64,
                        metadata,
                    )
                    .unwrap(),
                ),
            )
        };
        let id = SemanticTypeIdV1;
        request.types = vec![
            aggregate(20, vec![], vec![], 0, 1),
            scalar(21, 16),
            scalar(22, 64),
            scalar(23, 32),
            decl(
                24,
                SemanticTypeLayoutV1::new(None, 2).unwrap(),
                SemanticTypeShapeV1::Slice { element: id(1) },
            ),
            reference(25, id(4), SemanticPointerMetadataV1::SliceLength, 16),
            aggregate(26, vec![id(5), id(0)], vec![0, 16], 16, 8),
            reference(27, id(6), SemanticPointerMetadataV1::None, 8),
            aggregate(
                28,
                vec![id(7), id(2), id(2), id(2), id(2), id(0), id(0), id(0)],
                vec![0, 8, 16, 24, 32, 40, 40, 40],
                40,
                8,
            ),
            reference(29, id(8), SemanticPointerMetadataV1::None, 8),
            aggregate(30, vec![id(3), id(0), id(0), id(0)], vec![0, 4, 4, 4], 4, 4),
            reference(31, id(10), SemanticPointerMetadataV1::None, 8),
            aggregate(32, vec![id(14), id(0), id(0)], vec![0, 8, 8], 8, 2),
            aggregate(33, vec![id(1)], vec![0], 2, 2),
            decl(
                34,
                SemanticTypeLayoutV1::new(Some(8), 2).unwrap(),
                SemanticTypeShapeV1::Array {
                    element: id(13),
                    length: 4,
                },
            ),
        ]
        .into_boxed_slice();
        request
    }

    #[test]
    fn global_bf16_layout_preserves_borrow_coordinates_and_marker_fields() {
        let c = contract(SemanticMfmaOperandRoleV1::A);
        let original = layout_request();
        assert!(semantic_global_bf16_matrix_layout_matches_v1(
            &original.types,
            c.types()
        ));
        for change in 0..6 {
            let mut r = original.clone();
            match change {
                0 => {
                    let SemanticTypeShapeV1::Pointer(ref mut p) = r.types[7].shape else {
                        unreachable!()
                    };
                    p.mutability = SemanticMutabilityV1::Mutable;
                }
                1 => {
                    let SemanticTypeShapeV1::Aggregate(ref mut a) = r.types[8].shape else {
                        unreachable!()
                    };
                    a.fields[1] = c.types().element;
                }
                2 => {
                    let SemanticTypeShapeV1::Aggregate(ref mut a) = r.types[8].shape else {
                        unreachable!()
                    };
                    a.fields[5] = c.types().index;
                }
                3 => r.types[7].shape = r.types[5].shape.clone(),
                4 => {
                    r.types[1].shape =
                        SemanticTypeShapeV1::Scalar(SemanticScalarTypeV1::Float { bits: 16 })
                }
                5 => {
                    r.types[2].shape = SemanticTypeShapeV1::Scalar(SemanticScalarTypeV1::Integer {
                        signed: false,
                        bits: 32,
                    })
                }
                _ => unreachable!(),
            }
            assert!(
                !semantic_global_bf16_matrix_layout_matches_v1(&r.types, c.types()),
                "mutation {change}"
            );
        }
    }

    #[test]
    fn global_bf16_abi_preserves_shared_ownership_exact_source_types_and_root() {
        let c = contract(SemanticMfmaOperandRoleV1::A);
        let request = layout_request();
        let value = |ty| SemanticAbiValueV1::new(ty, SemanticAbiPassModeV1::Ignore);
        let abi = SemanticFunctionAbiV1::new(
            SemanticAbiIdentityV1(identity(70)),
            SemanticLayoutIdentityV1(identity(71)),
            SemanticCanonAbiV1::Rust,
            false,
            false,
            vec![
                value(SemanticTypeIdV1(9)),
                value(SemanticTypeIdV1(11)),
                value(c.types().index),
                value(c.types().index),
            ],
            value(c.types().fragment),
        )
        .unwrap()
        .with_source_argument_ownership(vec![
            SemanticSourceArgumentOwnershipV1::SharedBorrow,
            SemanticSourceArgumentOwnershipV1::SharedBorrow,
            SemanticSourceArgumentOwnershipV1::ByValue,
            SemanticSourceArgumentOwnershipV1::ByValue,
        ])
        .unwrap();
        assert!(abi_matches(&request, &abi, c));
        for argument in 0..4 {
            let mut changed = abi.clone();
            changed.source_argument_ownership[argument] =
                SemanticSourceArgumentOwnershipV1::Unspecified;
            assert!(!abi_matches(&request, &changed, c));
        }
        let mut changed = abi.clone();
        changed.source_signature.inputs.swap(0, 1);
        assert!(!abi_matches(&request, &changed, c));
        let mut request = request;
        request.functions[0].export = None;
        assert!(!abi_matches(&request, &abi, c));
    }
}

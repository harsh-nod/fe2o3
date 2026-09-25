use super::*;

fn assert_lossless(f: &Fixture) {
    let (bytes, hash) = encode(f.input()).unwrap();
    let mut work = Work::new(usize::MAX);
    let mut budget = Budget::new(&mut work, usize::MAX);
    with_decoded_native_cpu_input_v1(&bytes, &mut budget, |owner, budget| {
        assert_eq!(owner.input_v1().replay.effect_ir, &f.ir);
        assert_eq!(owner.input_v1().replay.signature_preimage, &f.signature);
        with_encoded_native_cpu_input_v1(owner.input_v1(), budget, |again, commitment, _| {
            assert_eq!(again, &bytes);
            assert_eq!(commitment, hash);
        })
        .unwrap();
    })
    .unwrap();
    assert_eq!(budget.storage(), 0);
}

#[test]
fn full_commitment_binds_association_and_every_identity_field_not_legacy_hash() {
    let f = fixture();
    let baseline = encode(f.input()).unwrap().1;
    for component in 0..4 {
        let mut input = f.input();
        match component {
            0 => input.association.semantic_mir_sha256[31] ^= 1,
            1 => input.association.semantic_root += 1,
            2 => input.association.registration_path = "other::register",
            _ => input.association.logical_kernel_name = "other",
        }
        assert_ne!(encode(input).unwrap().1, baseline);
    }
    for kernel in [false, true] {
        for field in 0..7 {
            let mut changed = fixture();
            let id = if kernel {
                &mut changed.kernel
            } else {
                &mut changed.reference
            };
            match field {
                0 => id.def_path_hash[15] ^= 1,
                1 => id.function_sha256[31] ^= 1,
                2 => id.item_definition_sha256[31] ^= 1,
                3 => id.monomorphization_sha256[31] ^= 1,
                4 => id.generic_type_arguments_sha256[31] ^= 1,
                5 => id.const_generic_arguments_sha256[31] ^= 1,
                _ => id.rustc_mir_body_sha256[31] ^= 1,
            }
            assert_eq!(changed.digest, f.digest);
            assert_ne!(encode(changed.input()).unwrap().1, baseline);
        }
    }
}

#[test]
fn signature_distinctions_survive_with_identical_legacy_effect_digest() {
    let f = fixture();
    let baseline = encode(f.input()).unwrap().1;
    for change_region in [false, true] {
        let mut changed = fixture();
        let mut kernel = changed.signature.kernel_inputs().to_vec();
        let mut reference = changed.signature.reference_inputs().to_vec();
        if change_region {
            if let ReferenceSignatureInputV1::Reference { region, .. } = &mut reference[1] {
                *region = ReferenceRegionV1::Static;
            }
        } else if let ReferenceSignatureInputV1::NominalOutput { carrier, .. } = &mut kernel[0] {
            *carrier = ReferenceCarrierV1::WriteOnlyDisjointSlice;
        }
        changed.signature = ReferenceLogicalSignaturePreimageV1::new(
            kernel.into_boxed_slice(),
            reference.into_boxed_slice(),
            ReferenceReturnShapeV1::Unit,
            SemanticExternAbiV1::Rust,
            SemanticFunctionSafetyV1::Safe,
            false,
        )
        .unwrap();
        assert_eq!(changed.ir.canonical_sha256_v1(), f.digest);
        assert_ne!(encode(changed.input()).unwrap().1, baseline);
        assert_lossless(&changed);
    }
}

#[test]
fn full_frame_includes_effect_value_omitted_by_legacy_digest() {
    let f = fixture();
    let original = encode(f.input()).unwrap();
    let mut changed = fixture();
    changed.ir.observable_output_effects[0].value =
        ReferenceValueV1::Use(ReferenceOperandV1::Constant(ReferenceConstantV1::ZeroSized));
    assert_eq!(changed.ir.canonical_sha256_v1(), f.digest);
    // The public encoder must reject this changed occurrence value. Independently
    // mutate the canonical frame to confirm its commitment includes that byte.
    assert!(encode(changed.input()).is_err());
    let mut wire = original.0.clone();
    wire[686] ^= 1;
    let mut work = Work::new(usize::MAX);
    let mut budget = Budget::new(&mut work, usize::MAX);
    let mutated = scoped(&mut budget, |s| commitment(&wire, s)).unwrap();
    assert_ne!(mutated, original.1);
    assert!(decode(&wire).is_err());
}

#[test]
fn exact_text_is_preserved_without_normalization() {
    let f = fixture();
    for registration in ["", "a\0b", "e\u{301}", "\u{e9}"] {
        let mut input = f.input();
        input.association.registration_path = registration;
        let (bytes, hash) = encode(input).unwrap();
        let mut work = Work::new(usize::MAX);
        let mut budget = Budget::new(&mut work, usize::MAX);
        with_decoded_native_cpu_input_v1(&bytes, &mut budget, |owner, _| {
            assert_eq!(owner.input_v1().association.registration_path, registration);
            assert_eq!(owner.commitment_v1(), hash);
        })
        .unwrap();
    }
    let mut a = f.input();
    a.association.registration_path = "e\u{301}";
    let mut b = f.input();
    b.association.registration_path = "\u{e9}";
    assert_ne!(encode(a).unwrap().1, encode(b).unwrap().1);
}

fn scalar_constant(scalar: ReferenceScalarTypeV1) -> ReferenceConstantV1 {
    use ReferenceScalarTypeV1::*;
    let bits = match scalar {
        Bool => 1,
        U8 | I8 => 255,
        U16 | I16 => 65535,
        U32 | I32 | F32 => u32::MAX as u128,
        U64 | Usize | I64 | Isize | F64 => u64::MAX as u128,
    };
    ReferenceConstantV1::Scalar { scalar, bits }
}
fn use_value(f: &mut Fixture, value: ReferenceValueV1, rhs: ReferenceEffectExpressionV1) {
    f.ir.blocks[0].assignments[0].value = value.clone();
    f.ir.observable_output_effects[0].value = value;
    f.ir.observable_output_effects[0].rhs = rhs;
    f.refresh();
}

#[test]
fn all_scalar_and_operator_tags_roundtrip_in_values_and_expressions() {
    for (_, scalar) in SCALARS {
        let mut f = fixture();
        let constant = scalar_constant(*scalar);
        use_value(
            &mut f,
            ReferenceValueV1::Use(ReferenceOperandV1::Constant(constant.clone())),
            ReferenceEffectExpressionV1::Constant(constant),
        );
        assert_lossless(&f);
    }
    let c = || ReferenceConstantV1::Scalar {
        scalar: ReferenceScalarTypeV1::U32,
        bits: 1,
    };
    let operand = || ReferenceOperandV1::Constant(c());
    let expr = || ReferenceEffectExpressionV1::Constant(c());
    for (_, operation) in BINARY {
        for checked in [false, true] {
            let mut f = fixture();
            use_value(
                &mut f,
                ReferenceValueV1::Binary {
                    operation: *operation,
                    checked,
                    lhs: operand(),
                    rhs: operand(),
                },
                ReferenceEffectExpressionV1::Binary {
                    operation: *operation,
                    checked,
                    lhs: Box::new(expr()),
                    rhs: Box::new(expr()),
                },
            );
            assert_lossless(&f);
        }
    }
    for (_, operation) in UNARY {
        let mut f = fixture();
        use_value(
            &mut f,
            ReferenceValueV1::Unary {
                operation: *operation,
                operand: operand(),
            },
            ReferenceEffectExpressionV1::Unary {
                operation: *operation,
                operand: Box::new(expr()),
            },
        );
        assert_lossless(&f);
    }
    for (_, kind) in CASTS {
        let mut f = fixture();
        use_value(
            &mut f,
            ReferenceValueV1::Cast {
                kind: *kind,
                source: ReferenceScalarTypeV1::U32,
                target: ReferenceScalarTypeV1::U64,
                operand: operand(),
            },
            ReferenceEffectExpressionV1::Cast {
                kind: *kind,
                source: ReferenceScalarTypeV1::U32,
                target: ReferenceScalarTypeV1::U64,
                operand: Box::new(expr()),
            },
        );
        assert_lossless(&f);
    }
}

#[test]
fn places_coordinates_predicates_and_terminators_roundtrip() {
    for projection in [
        ReferencePlaceProjectionV1::Dereference,
        ReferencePlaceProjectionV1::Field(0),
        ReferencePlaceProjectionV1::Field(1),
        ReferencePlaceProjectionV1::Index(1),
        ReferencePlaceProjectionV1::ConstantIndex {
            offset: 1,
            minimum_length: 2,
            from_end: false,
        },
        ReferencePlaceProjectionV1::ConstantIndex {
            offset: 1,
            minimum_length: 2,
            from_end: true,
        },
    ] {
        for moving in [false, true] {
            let mut f = fixture();
            let place = ReferencePlaceV1 {
                local: 2,
                projection: vec![projection.clone()].into_boxed_slice(),
            };
            let value = if moving {
                ReferenceOperandV1::Move(place)
            } else {
                ReferenceOperandV1::Copy(place)
            };
            let rhs = f.ir.observable_output_effects[0].rhs.clone();
            use_value(&mut f, ReferenceValueV1::Use(value), rhs);
            assert_lossless(&f);
        }
    }
    let constant = || ReferenceEffectExpressionV1::Constant(ReferenceConstantV1::ZeroSized);
    for coordinate in [
        ReferenceOutputCoordinateV1::Dynamic(constant()),
        ReferenceOutputCoordinateV1::Constant {
            offset: 1,
            minimum_length: 2,
            from_end: true,
        },
    ] {
        let mut f = fixture();
        f.ir.observable_output_effects[0].coordinate = coordinate;
        f.refresh();
        assert_lossless(&f);
    }
    for guard in [
        ReferencePathPredicateV1::unreachable_v1(),
        ReferencePathPredicateV1 {
            clauses: vec![
                ReferenceGuardClauseV1 {
                    atoms: vec![ReferenceGuardAtomV1::SwitchValueSet {
                        discriminant: constant(),
                        values: vec![1, 2].into_boxed_slice(),
                        inside_set: false,
                    }]
                    .into_boxed_slice(),
                },
                ReferenceGuardClauseV1 {
                    atoms: vec![ReferenceGuardAtomV1::Assert {
                        condition: constant(),
                        expected: true,
                    }]
                    .into_boxed_slice(),
                },
            ]
            .into_boxed_slice(),
        },
    ] {
        let mut f = fixture();
        f.ir.observable_output_effects[0].guard = guard;
        f.refresh();
        assert_lossless(&f);
    }
    let op = || ReferenceOperandV1::Constant(ReferenceConstantV1::ZeroSized);
    for terminator in [
        ReferenceTerminatorV1::Goto { target: 1 },
        ReferenceTerminatorV1::Switch {
            discriminant: op(),
            values: vec![(7, 1), (1, 1)].into_boxed_slice(),
            otherwise: 1,
        },
        ReferenceTerminatorV1::Assert {
            condition: op(),
            expected: false,
            success: 1,
            bounds_check: None,
        },
        ReferenceTerminatorV1::Assert {
            condition: op(),
            expected: true,
            success: 1,
            bounds_check: Some(ReferenceBoundsCheckV1 {
                index: op(),
                length: op(),
            }),
        },
    ] {
        let mut f = fixture();
        let mut blocks = f.ir.blocks.to_vec();
        blocks[0].terminator = terminator;
        blocks.push(ReferenceBlockV1 {
            block: 1,
            assignments: Box::default(),
            terminator: ReferenceTerminatorV1::Return,
        });
        f.ir.blocks = blocks.into_boxed_slice();
        f.refresh();
        assert_lossless(&f);
    }
}

#[test]
fn slice_scalar_relations_load_length_and_single_coordinate_roundtrip() {
    let mut f = fixture();
    let shared = ReferenceSignatureInputV1::Reference {
        region: ReferenceRegionV1::Static,
        mutability: SemanticMutabilityV1::Immutable,
        pointee: ReferencePointeeV1::Slice(ReferenceScalarTypeV1::F32),
    };
    let scalar = ReferenceSignatureInputV1::Scalar(ReferenceScalarTypeV1::U32);
    f.signature = ReferenceLogicalSignaturePreimageV1::new(
        vec![shared, scalar, f.signature.kernel_inputs()[0]].into_boxed_slice(),
        vec![
            shared,
            scalar,
            ReferenceSignatureInputV1::Reference {
                region: ReferenceRegionV1::Erased,
                mutability: SemanticMutabilityV1::Mutable,
                pointee: ReferencePointeeV1::Slice(ReferenceScalarTypeV1::F32),
            },
        ]
        .into_boxed_slice(),
        ReferenceReturnShapeV1::Unit,
        SemanticExternAbiV1::Rust,
        SemanticFunctionSafetyV1::Safe,
        false,
    )
    .unwrap();
    let derived = f.signature.derive_relations_v1().unwrap();
    f.ir.relations = (0..derived.len())
        .map(|raw| derived.relation_at_raw_argument_v1(raw as u32).unwrap())
        .collect();
    f.ir.argument_count = 3;
    f.ir.local_count = 4;
    f.ir.observable_output_effects[0].argument = 2;
    f.ir.observable_output_effects[0].coordinate = ReferenceOutputCoordinateV1::SingleCoordinate;
    f.ir.blocks[0].assignments[0].destination.local = 3;
    for rhs in [
        ReferenceEffectExpressionV1::KernelScalarArgument { argument: 1 },
        ReferenceEffectExpressionV1::InputLength {
            reference_argument: 0,
        },
        ReferenceEffectExpressionV1::InputLoad {
            reference_argument: 0,
            index: Box::new(ReferenceEffectExpressionV1::KernelScalarArgument { argument: 1 }),
        },
    ] {
        use_value(
            &mut f,
            ReferenceValueV1::InputLength {
                reference_argument: 2,
            },
            rhs,
        );
        assert_lossless(&f);
    }
}

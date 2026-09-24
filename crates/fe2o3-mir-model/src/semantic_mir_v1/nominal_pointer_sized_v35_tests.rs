use super::*;

const V35: SemanticMirWireVersionV1 = SemanticMirWireVersionV1::V35;

fn request(signed: bool) -> InertSemanticMirRequestV1 {
    let mut request = minimal_request();
    request.types[0] = SemanticTypeDeclV1::new(
        SemanticTypeIdentityV1(identity(1)),
        SemanticLayoutIdentityV1(identity(2)),
        SemanticTypeLayoutV1::new_with_backend_repr(
            Some(8),
            8,
            SemanticBackendReprV1::scalar(SemanticBackendScalarV1::initialized(
                SemanticBackendPrimitiveV1::integer(signed, 64, 8),
                SemanticScalarValidityRangeV1::new(0, u128::from(u64::MAX)),
            )),
            false,
        )
        .unwrap(),
        SemanticTypeShapeV1::Scalar(SemanticScalarTypeV1::Integer { signed, bits: 64 }),
    )
    .with_rustc_abi_properties(
        SemanticTypeAbiPropertiesV1::new(false, false).with_rustc_layout_is_noundef(true),
    )
    .with_rust_type_kind(if signed {
        SemanticRustTypeKindV1::Isize
    } else {
        SemanticRustTypeKindV1::Usize
    });
    request
}

fn type_bytes(ty: &SemanticTypeDeclV1, version: SemanticMirWireVersionV1) -> Vec<u8> {
    let mut writer = CanonicalWriterV1::new(4096);
    encode_type(&mut writer, ty, version).unwrap();
    writer.finish()
}

fn legacy_versions() -> impl Iterator<Item = SemanticMirWireVersionV1> {
    (2..=15)
        .chain(28..=34)
        .map(|version| SemanticMirWireVersionV1::from_u16(version).unwrap())
}

// Independent byte recipe: two identities, exact initialized integer layout,
// ABI properties, then the nominal shape tag and unchanged scalar payload.
fn literal_type(signed: bool, tag: u8) -> Vec<u8> {
    let mut bytes = Vec::new();
    bytes.extend_from_slice(&[1; 32]);
    bytes.extend_from_slice(&[2; 32]);
    bytes.extend_from_slice(&8_u64.to_le_bytes());
    bytes.push(1);
    bytes.extend_from_slice(&8_u64.to_le_bytes());
    bytes.extend_from_slice(&8_u64.to_le_bytes());
    bytes.extend_from_slice(&[0, 1]);
    bytes.extend_from_slice(&0_u32.to_le_bytes());
    bytes.extend_from_slice(&[0, 1, 0, 0, u8::from(signed)]);
    bytes.extend_from_slice(&64_u16.to_le_bytes());
    bytes.extend_from_slice(&8_u64.to_le_bytes());
    bytes.extend_from_slice(&0_u128.to_le_bytes());
    bytes.extend_from_slice(&u128::from(u64::MAX).to_le_bytes());
    bytes.extend_from_slice(&[0, 0]);
    bytes.extend_from_slice(&8_u64.to_le_bytes());
    bytes.extend_from_slice(
        &(if signed {
            0x1_0000_0007_u64
        } else {
            0x2_0000_0007_u64
        })
        .to_le_bytes(),
    );
    bytes.extend_from_slice(&[0, 0, 0, 1, 0, 0]);
    bytes.extend_from_slice(&[tag, 2, u8::from(signed)]);
    bytes.extend_from_slice(&64_u16.to_le_bytes());
    bytes
}

#[test]
fn nominal_v35_roundtrip_preserves_kind_shape_and_both_identities() {
    for signed in [false, true] {
        let request = request(signed);
        let expected = request.types[0].clone();
        let admitted = request
            .clone()
            .admit_exact_v35(SemanticMirLimitsV1::default())
            .unwrap();
        assert_eq!(admitted.wire_version(), V35);
        for decoded in [
            AdmittedInertSemanticMirV1::decode_exact_v35_canonical(
                admitted.canonical_encoding(),
                SemanticMirLimitsV1::default(),
            )
            .unwrap(),
            AdmittedInertSemanticMirV1::decode_minimal_compatible_canonical(
                admitted.canonical_encoding(),
                SemanticMirLimitsV1::default(),
            )
            .unwrap(),
            AdmittedInertSemanticMirV1::decode_current_production_canonical(
                admitted.canonical_encoding(),
                SemanticMirLimitsV1::default(),
            )
            .unwrap(),
        ] {
            assert_eq!(decoded.wire_version(), V35);
            assert_eq!(decoded.types(), &[expected.clone()]);
            assert_eq!(decoded.canonical_encoding(), admitted.canonical_encoding());
            assert_eq!(decoded.semantic_sha256(), admitted.semantic_sha256());
        }
        let minimal = request
            .clone()
            .admit(SemanticMirLimitsV1::default())
            .unwrap();
        assert_eq!(minimal.canonical_encoding(), admitted.canonical_encoding());
        let current = request
            .clone()
            .admit_current_production(SemanticMirLimitsV1::default())
            .unwrap();
        assert_eq!(current.canonical_encoding(), admitted.canonical_encoding());
        let mut ordinary = request;
        ordinary.types[0].rust_type_kind = SemanticRustTypeKindV1::Ordinary;
        let ordinary = ordinary
            .admit_exact_v35(SemanticMirLimitsV1::default())
            .unwrap();
        assert_ne!(ordinary.semantic_sha256(), admitted.semantic_sha256());
        assert_eq!(
            ordinary.types()[0].identity(),
            admitted.types()[0].identity()
        );
        assert_eq!(
            ordinary.types()[0].layout_identity(),
            admitted.types()[0].layout_identity()
        );
    }
}

#[test]
fn nominal_v35_literal_tags_and_old_scalar_bytes_are_exact() {
    for (signed, tag) in [(false, 18), (true, 19)] {
        let request = request(signed);
        let bytes = type_bytes(&request.types[0], V35);
        assert_eq!(bytes, literal_type(signed, tag));
        let mut decoder = CanonicalDecoderV1::new(&bytes, SemanticMirLimitsV1::default());
        decoder.wire_version = V35;
        assert_eq!(decoder.ty().unwrap(), request.types[0]);
        decoder.finish().unwrap();
        let ordinary = request.types[0]
            .clone()
            .with_rust_type_kind(SemanticRustTypeKindV1::Ordinary);
        for version in legacy_versions().chain([V35]) {
            assert_eq!(type_bytes(&ordinary, version), literal_type(signed, 2));
        }
    }
}

#[test]
fn nominal_v35_old_versions_refuse_before_any_type_bytes_are_written() {
    for signed in [false, true] {
        for version in legacy_versions() {
            let request = request(signed);
            let expected = SemanticMirErrorV1::WireVersionCannotRepresent {
                requested: version,
                required: V35,
            };
            assert_eq!(
                request
                    .clone()
                    .admit_for_wire_version(version, SemanticMirLimitsV1::default())
                    .unwrap_err(),
                expected
            );
            let mut writer = CanonicalWriterV1::new(4096);
            writer.raw(&[71, 72]).unwrap();
            assert_eq!(
                encode_type(&mut writer, &request.types[0], version),
                Err(expected)
            );
            assert_eq!(writer.finish(), [71, 72]);
        }
    }
}

#[test]
fn nominal_v35_type_tag_holes_and_historical_parsers_remain_closed() {
    let original = literal_type(false, 18);
    let offset = original.len() - 5;
    for tag in (14..=17).chain(20..=u8::MAX) {
        let mut bytes = original.clone();
        bytes[offset] = tag;
        let mut decoder = CanonicalDecoderV1::new(&bytes, SemanticMirLimitsV1::default());
        decoder.wire_version = V35;
        assert_eq!(
            decoder.ty(),
            Err(SemanticMirDecodeErrorV1::InvalidTag {
                context: "type shape",
                offset,
                value: tag
            })
        );
    }
    for version in legacy_versions() {
        for tag in [18, 19] {
            let mut bytes = original.clone();
            bytes[offset] = tag;
            let mut decoder = CanonicalDecoderV1::new(&bytes, SemanticMirLimitsV1::default());
            decoder.wire_version = version;
            assert_eq!(
                decoder.ty(),
                Err(SemanticMirDecodeErrorV1::InvalidTag {
                    context: "type shape",
                    offset,
                    value: tag
                })
            );
        }
    }
}

#[test]
fn nominal_v35_specialized_intrinsic_tags_and_rust_call_roles_stay_closed() {
    for tag in 69..=u8::MAX {
        let bytes = [tag];
        let mut decoder = CanonicalDecoderV1::new(&bytes, SemanticMirLimitsV1::default());
        decoder.wire_version = V35;
        assert_eq!(
            decoder.compiler_intrinsic(),
            Err(SemanticMirDecodeErrorV1::InvalidTag {
                context: "compiler intrinsic",
                offset: 0,
                value: tag
            })
        );
    }
    let mut mixed = request(false);
    mixed.functions[0].locals[1].role = SemanticLocalRoleV1::RustCallTupleField {
        argument: 0,
        field: 0,
    };
    assert_eq!(
        mixed
            .admit_exact_v35(SemanticMirLimitsV1::default())
            .unwrap_err(),
        SemanticMirErrorV1::WireVersionCannotRepresent {
            requested: V35,
            required: SemanticMirWireVersionV1::V28
        }
    );
}

#[test]
fn nominal_v35_specialized_schema_selection_never_erases_nominality() {
    let mut descriptors = [0; SEMANTIC_GFX942_U32_PROGRAM_MAX_STEPS_V32];
    descriptors[0] = 8;
    let operations = [
        (
            SemanticCompilerIntrinsicOperationV1::Execution(
                SemanticExecutionOperationV29::ContextIssue {
                    context: SemanticTypeIdV1::from_index(0),
                },
            ),
            SemanticMirWireVersionV1::V29,
        ),
        (
            SemanticCompilerIntrinsicOperationV1::SaturatingInteger(
                SemanticSaturatingIntegerOpV1::Add,
            ),
            SemanticMirWireVersionV1::V30,
        ),
        (
            SemanticCompilerIntrinsicOperationV1::Gfx942OrderedRegion(
                SemanticGfx942OrderedRegionProfileV31::XorAddU32E32,
            ),
            SemanticMirWireVersionV1::V31,
        ),
        (
            SemanticCompilerIntrinsicOperationV1::Gfx942OrderedProgram(
                SemanticGfx942U32ProgramV32::from_descriptors(1, descriptors).unwrap(),
            ),
            SemanticMirWireVersionV1::V32,
        ),
        (
            SemanticCompilerIntrinsicOperationV1::Gfx942Wave64ShuffleIndex {
                context: SemanticTypeIdV1::from_index(0),
                element: SemanticTypeIdV1::from_index(0),
            },
            SemanticMirWireVersionV1::V33,
        ),
        (
            SemanticCompilerIntrinsicOperationV1::Gfx942InlineU32(
                SemanticGfx942InlineU32V30::new(SemanticGfx942InlineInstructionV30::VMovB32, 1)
                    .unwrap(),
            ),
            SemanticMirWireVersionV1::V34,
        ),
    ];
    // Schema checks intentionally precede semantic callable validation: these
    // are refusal fixtures, not claims of an admitted execution operation.
    for (operation, required) in operations {
        let mut mixed = version_selection_request([operation]);
        mixed.types[0] = request(false).types[0].clone();
        assert_eq!(minimum_wire_version(&mixed), V35);
        assert_eq!(
            mixed
                .clone()
                .admit(SemanticMirLimitsV1::default())
                .unwrap_err(),
            SemanticMirErrorV1::WireVersionCannotRepresent {
                requested: V35,
                required
            }
        );
        assert_eq!(
            mixed
                .admit_exact_v35(SemanticMirLimitsV1::default())
                .unwrap_err(),
            SemanticMirErrorV1::WireVersionCannotRepresent {
                requested: V35,
                required
            }
        );
    }
}

#[test]
fn nominal_v35_physical_layout_and_definedness_mismatches_refuse() {
    for signed in [false, true] {
        for mutation in 0..11 {
            let mut request = request(signed);
            let ty = &mut request.types[0];
            match mutation {
                0 => {
                    ty.rust_type_kind = if signed {
                        SemanticRustTypeKindV1::Usize
                    } else {
                        SemanticRustTypeKindV1::Isize
                    }
                }
                1 => {
                    ty.shape = SemanticTypeShapeV1::Scalar(SemanticScalarTypeV1::Integer {
                        signed,
                        bits: 32,
                    })
                }
                2 => ty.layout.size_bytes = Some(4),
                3 => ty.layout.rustc_size_bytes = 4,
                4 => ty.layout.alignment_bytes = 4,
                5 => ty.layout.unadjusted_abi_alignment_bytes = 4,
                6 => ty.abi_properties.pass_indirectly_in_non_rustic_abis = true,
                7 => ty.abi_properties.has_unsized_foreign_tail = true,
                8 => ty.abi_properties.rustc_layout_is_noundef = false,
                9 => {
                    ty.layout.backend_repr =
                        SemanticBackendReprV1::scalar(SemanticBackendScalarV1::union(
                            SemanticBackendPrimitiveV1::integer(signed, 64, 8),
                        ))
                }
                10 => {
                    ty.layout.backend_repr =
                        SemanticBackendReprV1::scalar(SemanticBackendScalarV1::initialized(
                            SemanticBackendPrimitiveV1::integer(!signed, 64, 8),
                            SemanticScalarValidityRangeV1::new(0, u128::from(u64::MAX)),
                        ))
                }
                _ => unreachable!(),
            }
            assert_eq!(
                request
                    .admit_exact_v35(SemanticMirLimitsV1::default())
                    .unwrap_err(),
                SemanticMirErrorV1::InvalidTypeLayout,
                "signed={signed} mutation={mutation}"
            );
        }
    }
}

#[test]
fn nominal_v35_pointee_metadata_cannot_relabel_an_integer() {
    for second in [false, true] {
        let mut request = request(false);
        let pointee = SemanticAbiPointeeInfoV1::new(SemanticAbiPointeeKindV1::Raw, 0, 1).unwrap();
        request.types[0].abi_properties = request.types[0]
            .abi_properties
            .with_scalar_pointee_info((!second).then_some(pointee), second.then_some(pointee));
        assert_eq!(
            request
                .admit_exact_v35(SemanticMirLimitsV1::default())
                .unwrap_err(),
            SemanticMirErrorV1::InvalidFunctionAbi
        );
    }
}

#[test]
fn nominal_v35_legacy_ordinary_request_encoding_changes_only_version() {
    let old = minimal_request()
        .admit_exact_v15(SemanticMirLimitsV1::default())
        .unwrap();
    let new = minimal_request()
        .admit_exact_v35(SemanticMirLimitsV1::default())
        .unwrap();
    let mut expected = old.canonical_encoding().to_vec();
    expected[MAGIC.len()..MAGIC.len() + 2].copy_from_slice(&35_u16.to_le_bytes());
    assert_eq!(new.canonical_encoding(), expected);
    for version in legacy_versions() {
        let ordinary = minimal_request()
            .admit_for_wire_version(version, SemanticMirLimitsV1::default())
            .unwrap();
        let decoded = AdmittedInertSemanticMirV1::decode_with_policy(
            ordinary.canonical_encoding(),
            SemanticMirLimitsV1::default(),
            CanonicalDecodePolicyV1::Exact(version),
        )
        .unwrap();
        assert_eq!(decoded.canonical_encoding(), ordinary.canonical_encoding());
        assert_eq!(decoded.types(), ordinary.types());
    }
}

fn context(request: &InertSemanticMirRequestV1, work: u64, max: u64) -> ValidationContextV1<'_> {
    ValidationContextV1 {
        request,
        limits: SemanticMirLimitsV1::default()
            .with_limit(SemanticMirResourceV1::ValidationWork, max)
            .unwrap(),
        totals: ValidationTotalsV1::default(),
        work,
        owned_execution_roles: Vec::new(),
    }
}

#[test]
fn nominal_v35_work_suffix_is_exact_and_old_kinds_do_not_charge_it() {
    let request = request(false);
    let mut ordinary = request.types[0].clone();
    ordinary.rust_type_kind = SemanticRustTypeKindV1::Ordinary;
    for permitted in 0..=8 {
        let mut context = context(&request, 17, 17 + permitted);
        nominal_pointer_sized_v35::validate_type(&mut context, &ordinary).unwrap();
        assert_eq!(context.work, 17);
        let result = nominal_pointer_sized_v35::validate_type(&mut context, &request.types[0]);
        if permitted == 8 {
            result.unwrap();
            assert_eq!(context.work, 25);
        } else {
            assert_eq!(
                result,
                Err(SemanticMirErrorV1::LimitExceeded {
                    resource: SemanticMirResourceV1::ValidationWork,
                    actual: 18 + permitted,
                    max: 17 + permitted
                })
            );
            assert_eq!(context.work, 18 + permitted);
        }
        assert!(context.owned_execution_roles.is_empty());
    }
    assert_eq!(nominal_pointer_sized_v35::NOMINAL_TYPE_WORK_V35, 8);
    let mut old_context = context(&request, 17, 10_000);
    let mut new_context = context(&request, 17, 10_000);
    validate_type(&mut old_context, SemanticTypeIdV1::from_index(0), &ordinary).unwrap();
    validate_type(
        &mut new_context,
        SemanticTypeIdV1::from_index(0),
        &request.types[0],
    )
    .unwrap();
    assert_eq!(new_context.work, old_context.work + 8);
}

#[test]
fn nominal_v35_full_admission_preserves_existing_first_work_denial() {
    let limits = SemanticMirLimitsV1::default()
        .with_limit(SemanticMirResourceV1::ValidationWork, 0)
        .unwrap();
    for request in [minimal_request(), request(false), request(true)] {
        assert_eq!(
            request.admit_exact_v35(limits).unwrap_err(),
            SemanticMirErrorV1::LimitExceeded {
                resource: SemanticMirResourceV1::ValidationWork,
                actual: 1,
                max: 0
            }
        );
    }
}

#[test]
fn nominal_v35_exact_canonical_byte_and_declared_type_caps() {
    for signed in [false, true] {
        let request = request(signed);
        let admitted = request
            .clone()
            .admit_exact_v35(SemanticMirLimitsV1::default())
            .unwrap();
        let bytes = admitted.canonical_encoding();
        let count = u64::try_from(bytes.len()).unwrap();
        let exact = SemanticMirLimitsV1::default()
            .with_limit(SemanticMirResourceV1::CanonicalBytes, count)
            .unwrap();
        assert_eq!(
            request
                .clone()
                .admit_exact_v35(exact)
                .unwrap()
                .canonical_encoding(),
            bytes
        );
        AdmittedInertSemanticMirV1::decode_exact_v35_canonical(bytes, exact).unwrap();
        let short = exact
            .with_limit(SemanticMirResourceV1::CanonicalBytes, count - 1)
            .unwrap();
        assert_eq!(
            request.clone().admit_exact_v35(short).unwrap_err(),
            SemanticMirErrorV1::LimitExceeded {
                resource: SemanticMirResourceV1::CanonicalBytes,
                actual: count,
                max: count - 1
            }
        );
        assert_eq!(
            AdmittedInertSemanticMirV1::decode_exact_v35_canonical(bytes, short).unwrap_err(),
            SemanticMirDecodeErrorV1::InputLimitExceeded {
                actual: count,
                max: count - 1
            }
        );
        let no_types = SemanticMirLimitsV1::default()
            .with_limit(SemanticMirResourceV1::Types, 0)
            .unwrap();
        assert_eq!(
            request.admit_exact_v35(no_types).unwrap_err(),
            SemanticMirErrorV1::LimitExceeded {
                resource: SemanticMirResourceV1::Types,
                actual: 1,
                max: 0
            }
        );
        assert_eq!(
            AdmittedInertSemanticMirV1::decode_exact_v35_canonical(bytes, no_types).unwrap_err(),
            SemanticMirDecodeErrorV1::Validation(SemanticMirErrorV1::LimitExceeded {
                resource: SemanticMirResourceV1::Types,
                actual: 1,
                max: 0
            })
        );
    }
}

#[test]
fn nominal_v35_unknown_versions_truncation_and_trailing_bytes_refuse() {
    let admitted = request(false)
        .admit_exact_v35(SemanticMirLimitsV1::default())
        .unwrap();
    let bytes = admitted.canonical_encoding();
    for version in [0, 1, 16, 27, 37, u16::MAX] {
        let mut invalid = bytes.to_vec();
        invalid[MAGIC.len()..MAGIC.len() + 2].copy_from_slice(&version.to_le_bytes());
        assert_eq!(
            AdmittedInertSemanticMirV1::decode_exact_v35_canonical(
                &invalid,
                SemanticMirLimitsV1::default()
            )
            .unwrap_err(),
            SemanticMirDecodeErrorV1::UnsupportedVersion(version)
        );
    }
    // V36 is allocated, but remains invalid at the frozen exact-V35 boundary.
    let mut wrong_known_version = bytes.to_vec();
    wrong_known_version[MAGIC.len()..MAGIC.len() + 2].copy_from_slice(&36_u16.to_le_bytes());
    assert_eq!(
        AdmittedInertSemanticMirV1::decode_exact_v35_canonical(
            &wrong_known_version,
            SemanticMirLimitsV1::default()
        )
        .unwrap_err(),
        SemanticMirDecodeErrorV1::WireVersionMismatch {
            expected: SemanticMirWireVersionV1::V35,
            actual: SemanticMirWireVersionV1::V36,
        }
    );
    let mut trailing = bytes.to_vec();
    trailing.push(0);
    assert_eq!(
        AdmittedInertSemanticMirV1::decode_exact_v35_canonical(
            &trailing,
            SemanticMirLimitsV1::default()
        )
        .unwrap_err(),
        SemanticMirDecodeErrorV1::TrailingBytes {
            offset: bytes.len(),
            trailing: 1
        }
    );
    assert_eq!(
        AdmittedInertSemanticMirV1::decode_exact_v35_canonical(
            &bytes[..bytes.len() - 1],
            SemanticMirLimitsV1::default()
        )
        .unwrap_err(),
        SemanticMirDecodeErrorV1::UnexpectedEnd {
            offset: bytes.len() - 4,
            requested: 4
        }
    );
    assert_eq!(
        AdmittedInertSemanticMirV1::decode_exact_v15_canonical(
            bytes,
            SemanticMirLimitsV1::default()
        )
        .unwrap_err(),
        SemanticMirDecodeErrorV1::WireVersionMismatch {
            expected: SemanticMirWireVersionV1::V15,
            actual: V35
        }
    );
}

#[test]
fn nominal_v35_kind_and_type_row_layout_remain_unchanged() {
    #[allow(dead_code)]
    enum OldKind {
        Ordinary,
        Str,
        Execution(SemanticExecutionRoleV29),
    }
    #[allow(dead_code)]
    struct OldType {
        identity: SemanticTypeIdentityV1,
        layout_identity: SemanticLayoutIdentityV1,
        layout: SemanticTypeLayoutV1,
        shape: SemanticTypeShapeV1,
        abi_properties: SemanticTypeAbiPropertiesV1,
        rust_type_kind: OldKind,
    }
    assert_eq!(
        std::mem::size_of::<OldKind>(),
        std::mem::size_of::<SemanticRustTypeKindV1>()
    );
    assert_eq!(
        std::mem::align_of::<OldKind>(),
        std::mem::align_of::<SemanticRustTypeKindV1>()
    );
    assert_eq!(
        std::mem::size_of::<OldType>(),
        std::mem::size_of::<SemanticTypeDeclV1>()
    );
    assert_eq!(
        std::mem::align_of::<OldType>(),
        std::mem::align_of::<SemanticTypeDeclV1>()
    );
}
